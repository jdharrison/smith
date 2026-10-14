"""Recipient package tests: synthetic payload, no helper/manager/provider execution."""

import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import install
import package


@unittest.skipIf(os.geteuid() == 0, "Recipient filesystem tests require the non-root builder")
class PackageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="smith-package-")
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.code = self.base / "code"
        self.code.mkdir(mode=0o700)
        source = Path(__file__).resolve().parents[1]
        for name in ("install.py", "installer.py"):
            shutil.copyfile(source / name, self.code / name)
        self.root = self.base / "recipient"
        client = self.root / "payload/client"
        for name in ("lib", "data/flutter_assets", "smith_backend", "share/applications", "share/pixmaps"):
            (client / name).mkdir(parents=True, mode=0o755)
        for name, mode in (
            ("smith_client", 0o755), ("smith_backend/smith-service", 0o755),
            ("lib/libflutter_linux_gtk.so", 0o644), ("lib/libapp.so", 0o644),
            ("data/icudtl.dat", 0o644), ("data/flutter_assets/AssetManifest.bin", 0o644),
            ("share/applications/" + install.APP + ".desktop", 0o644),
            ("share/pixmaps/" + install.APP + ".png", 0o644),
        ):
            path = client / name
            path.write_bytes(b"service" if name == "smith_backend/smith-service" else b"synthetic")
            path.chmod(mode)
        service = self.root / "payload/service/smith-service"
        service.parent.mkdir(mode=0o755)
        service.write_bytes(b"service")
        service.chmod(0o755)
        self.root.chmod(0o755)
        for directory in self.root.rglob("*"):
            if directory.is_dir():
                directory.chmod(0o755)
        self.readme = self.base / "operator.txt"
        self.readme.write_text("No weights bundled. Existing Ollama required.\n")
        self.home = self.base / "home"
        self.data = self.base / "data"
        self.home.mkdir(mode=0o700)
        self.data.mkdir(mode=0o700)
        self.env = dict(os.environ, HOME=str(self.home), XDG_DATA_HOME=str(self.data),
                        PYTHONDONTWRITEBYTECODE="1")

    def assemble(self):
        package.build(self.code, self.root, self.readme)

    def snapshot(self):
        return {str(p.relative_to(self.base)): p.read_bytes()
                for root in (self.home, self.data) for p in root.rglob("*") if p.is_file()}

    def test_complete_zipapp_and_checksums_without_build_tools(self):
        self.assemble()
        package.verify(self.root)
        record = json.loads((self.root / "manifest.json").read_text())
        self.assertFalse(record["models_bundled"])
        self.assertFalse(record["voice_bundled"])
        self.assertEqual(record["service_sha256"], hashlib.sha256(b"service").hexdigest())
        with zipfile.ZipFile(self.root / "smith-installer") as archive:
            self.assertEqual(set(archive.namelist()), {"install.py", "installer.py", "__main__.py"})
        result = subprocess.run([sys.executable, str(self.root / "smith-installer"), "--help"],
                                env=self.env, capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("--pull-model", result.stdout)

    def test_packaged_dry_run_resolves_adjacent_payload_and_writes_nothing(self):
        self.assemble()
        before = self.snapshot()
        result = subprocess.run([sys.executable, str(self.root / "smith-installer"), "--dry-run"],
                                cwd=str(self.base), env=self.env, capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("Dry run: no mutation", result.stdout)
        self.assertEqual(self.snapshot(), before)
        self.assertEqual(list(self.data.iterdir()), [])
        self.assertEqual(list(self.home.iterdir()), [])

    def test_declined_packaged_install_does_not_execute_synthetic_helper(self):
        self.assemble()
        result = subprocess.run([sys.executable, str(self.root / "smith-installer")],
                                input="n\n", env=self.env, capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("Declined: no mutation", result.stdout)
        self.assertEqual(list(self.data.iterdir()), [])
        self.assertEqual(list(self.home.iterdir()), [])

    def test_payload_tamper_blocks_verification(self):
        self.assemble()
        (self.root / "payload/client/data/flutter_assets/AssetManifest.bin").write_bytes(b"altered")
        with self.assertRaises(install.Unsafe):
            package.verify(self.root)

    def test_mode_tamper_blocks_verification(self):
        self.assemble()
        (self.root / "payload/service/smith-service").chmod(0o644)
        with self.assertRaises(install.Unsafe):
            package.verify(self.root)

    def test_mismatched_pair_blocks_assembly(self):
        (self.root / "payload/service/smith-service").write_bytes(b"different")
        with self.assertRaises(install.Unsafe):
            self.assemble()

    def prohibited_payload(self, relative):
        path = self.root / "payload/client" / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        for directory in self.root.rglob("*"):
            if directory.is_dir():
                directory.chmod(0o755)
        path.write_bytes(b"synthetic prohibited payload")
        path.chmod(0o644)
        with self.assertRaisesRegex(install.Unsafe, "Unreviewed client payload"):
            self.assemble()

    def test_model_gguf_is_rejected_before_weight_free_declaration(self):
        self.prohibited_payload("models/example.gguf")

    def test_nested_safetensors_are_rejected(self):
        self.prohibited_payload("data/flutter_assets/weights/example.safetensors")

    def test_voice_onnx_is_rejected(self):
        self.prohibited_payload("data/flutter_assets/voice/kokoro.onnx")

    def test_opaque_unreviewed_asset_and_empty_model_directory_are_rejected(self):
        self.prohibited_payload("data/flutter_assets/opaque.bin")
        (self.root / "payload/client/data/flutter_assets/opaque.bin").unlink()
        (self.root / "payload/client/models").mkdir(mode=0o755)
        with self.assertRaisesRegex(install.Unsafe, "Unreviewed client payload"):
            package.check_payload(self.root)

    def test_unknown_code_and_symlink_payload_are_not_packaged(self):
        (self.code / "unreviewed.py").write_text("raise RuntimeError\n")
        with self.assertRaises(install.Unsafe):
            self.assemble()
        (self.code / "unreviewed.py").unlink()
        (self.root / "payload/client/data/foreign").symlink_to(self.readme)
        with self.assertRaises(install.Unsafe):
            self.assemble()


if __name__ == "__main__":
    unittest.main()
