"""Builder package checks; actual native help/dry-run use private roots only."""

import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import package


def synthetic_elf():
    header = bytearray(64)
    header[:7] = b"\x7fELF\x02\x01\x01"
    header[16:18] = (3).to_bytes(2, "little")
    header[18:20] = (62).to_bytes(2, "little")
    return bytes(header)


class PackageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="smith-native-package-")
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.root = self.base / "recipient"
        client = self.root / "payload/client"
        for name in ("lib", "data/flutter_assets", "smith_backend", "share/applications", "share/pixmaps"):
            (client / name).mkdir(parents=True, mode=0o755)
        for name, mode in (
            ("smith_client", 0o755), ("smith_backend/smith-service", 0o755),
            ("lib/libflutter_linux_gtk.so", 0o644), ("lib/libapp.so", 0o644),
            ("data/icudtl.dat", 0o644), ("data/flutter_assets/AssetManifest.bin", 0o644),
            ("data/flutter_assets/pubspec.yaml", 0o644),
            ("share/applications/" + package.APP + ".desktop", 0o644),
            ("share/pixmaps/" + package.APP + ".png", 0o644),
        ):
            path = client / name
            path.write_bytes(synthetic_elf() if mode == 0o755 else b"synthetic")
            path.chmod(mode)
        # Synthetic content tests package policy, not font/SVG decoding or licensing.
        for name in package.UI_ASSET_FILES:
            path = client / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"synthetic UI asset fixture")
            path.chmod(0o644)
        native_runtime = client / "smith_backend/runtime"
        native_runtime.mkdir(mode=0o755)
        for backend, names in package.BACKEND_RUNTIME_FILES.items():
            subtree = native_runtime / backend
            subtree.mkdir(mode=0o755)
            for name in names:
                path = subtree / name
                path.write_bytes(b"license fixture" if name.startswith("LICENSE") else synthetic_elf())
                path.chmod(0o644 if name.startswith("LICENSE") else 0o755)
        service = self.root / "payload/service/smith-service"
        service.parent.mkdir(mode=0o755)
        service.write_bytes(synthetic_elf())
        service.chmod(0o755)
        executable = self.root / "smith-installer"
        self.native = os.environ.get("SMITH_TEST_INSTALLER")
        if self.native:
            shutil.copyfile(self.native, executable)
        else:
            executable.write_bytes(synthetic_elf())
        executable.chmod(0o755)
        graphical = self.root / "smith-installer-ui"
        self.native_ui = os.environ.get("SMITH_TEST_INSTALLER_UI")
        if self.native_ui:
            shutil.copyfile(self.native_ui, graphical)
        else:
            graphical.write_bytes(synthetic_elf())
        graphical.chmod(0o755)
        self.root.chmod(0o755)
        for directory in self.root.rglob("*"):
            if directory.is_dir():
                directory.chmod(0o755)
        self.readme = self.base / "operator.txt"
        self.readme.write_text("Native executable and CPU runner; no weights. No Ollama required.\n")
        self.home = self.base / "home"
        self.data = self.base / "data"
        self.home.mkdir(mode=0o700)
        self.data.mkdir(mode=0o700)
        self.env = dict(os.environ, HOME=str(self.home), XDG_DATA_HOME=str(self.data), PATH="/unavailable")

    def assemble(self):
        package.build(self.root, self.readme)

    def snapshot(self):
        return {str(p.relative_to(self.base)): (p.stat().st_mode, p.read_bytes())
                for root in (self.home, self.data) for p in root.rglob("*") if p.is_file()}

    def native_required(self):
        if not self.native:
            self.skipTest("SMITH_TEST_INSTALLER required for compiled native package execution")
        if os.geteuid() == 0:
            self.skipTest("Native installer package execution requires the non-root builder")

    def test_pinned_dual_runtime_contract(self):
        self.assertEqual(package.runtime.PREFIX, "llama-b11429/")
        self.assertEqual(package.runtime.BACKENDS["cpu"]["size"], 17693462)
        self.assertEqual(package.runtime.BACKENDS["cpu"]["sha256"],
                         "f6d25dde8f51133143d1453da4fd5f73b145127177612a283bf7995957af3392")
        self.assertEqual(package.runtime.BACKENDS["vulkan"]["url"],
                         "https://github.com/ggml-org/llama.cpp/releases/download/b11429/llama-b11429-bin-ubuntu-vulkan-x64.tar.gz")
        self.assertEqual(package.runtime.BACKENDS["vulkan"]["size"], 31636673)
        self.assertEqual(package.runtime.BACKENDS["vulkan"]["sha256"],
                         "632c4e98feba2b94407a2130e3133e0c3aefb0ea1ab41337e926d8bfafdd0b74")
        self.assertNotIn("libggml-vulkan.so", package.runtime.BACKENDS["cpu"]["files"])
        self.assertEqual(package.runtime.BACKENDS["vulkan"]["files"],
                         package.runtime.FILES + ("libggml-vulkan.so",))
        self.assertEqual(package.runtime.MEMBER_LIMIT, 16 * 1024 * 1024)
        self.assertEqual(package.runtime.VULKAN_LIBRARY_SIZE, 44315144)

    def test_complete_native_payload_and_checksums(self):
        self.assemble()
        package.verify(self.root)
        record = json.loads((self.root / "manifest.json").read_text())
        self.assertEqual(record["installer_runtime"], "native")
        self.assertEqual(record["installer_ui"], "gtk3")
        self.assertEqual(record["model_runner"], "llama.cpp-b11429-cpu+vulkan")
        for backend, names in package.BACKEND_RUNTIME_FILES.items():
            runtime = self.root / "payload/client/smith_backend/runtime" / backend
            self.assertEqual({path.name for path in runtime.iterdir()}, names)
            self.assertEqual((runtime / "llama-server").read_bytes()[:4], b"\x7fELF")
        self.assertEqual((self.root / "smith-installer-ui").read_bytes()[:4], b"\x7fELF")
        self.assertFalse(record["models_bundled"])
        self.assertFalse(record["voice_bundled"])
        self.assertEqual(record["service_sha256"], hashlib.sha256(synthetic_elf()).hexdigest())
        self.assertEqual((self.root / "smith-installer").read_bytes()[:4], b"\x7fELF")
        self.assertFalse(any(p.suffix in (".py", ".pyz") for p in self.root.rglob("*")))

    def test_exact_ui_asset_contract_and_manifest_coverage(self):
        prefix = "data/flutter_assets/assets/"
        expected = {prefix + "README.md"}
        for directory, family in (("jetbrains-mono", "JetBrainsMono"),
                                  ("ibm-plex-mono", "IBMPlexMono")):
            expected.add(prefix + "fonts/" + directory + "/OFL.txt")
            for weight in ("Regular", "Medium", "SemiBold"):
                expected.add(prefix + "fonts/" + directory + "/" + family + "-" + weight + ".ttf")
        for icon in ("chevron-right", "message-square", "settings", "archive", "plus",
                     "ellipsis-vertical", "pencil", "trash-2", "send", "x", "menu",
                     "rotate-ccw", "info", "check", "chevron-down"):
            expected.add(prefix + "icons/" + icon + ".svg")
        self.assertEqual(len(expected), 24)
        self.assertEqual(package.UI_ASSET_FILES, expected)
        self.assertEqual({name for name in package.CLIENT_FILES if name.startswith(prefix)}, expected)
        self.assemble()
        record = json.loads((self.root / "manifest.json").read_text())
        for name in expected:
            with self.subTest(asset=name):
                meta = record["files"]["payload/client/" + name]
                self.assertEqual(meta["mode"], 0o644)
                self.assertEqual(meta["sha256"],
                                 hashlib.sha256(b"synthetic UI asset fixture").hexdigest())
        package.verify(self.root)

    def test_every_ui_asset_is_required_and_nonempty(self):
        for name in sorted(package.UI_ASSET_FILES):
            path = self.root / "payload/client" / name
            original = path.read_bytes()
            for absent in (True, False):
                with self.subTest(asset=name, absent=absent):
                    if absent:
                        path.unlink()
                    else:
                        path.write_bytes(b"")
                    try:
                        with self.assertRaisesRegex(ValueError, "Missing or empty required UI asset"):
                            package.check_payload(self.root)
                    finally:
                        path.write_bytes(original)
                        path.chmod(0o644)

    def test_declared_version_asset_is_required_and_nonempty(self):
        path = self.root / "payload/client/data/flutter_assets/pubspec.yaml"
        for absent in (True, False):
            with self.subTest(absent=absent):
                if absent:
                    path.unlink()
                else:
                    path.write_bytes(b"")
                with self.assertRaisesRegex(ValueError, "Incomplete client payload"):
                    package.check_payload(self.root)
                path.write_bytes(b"version: 0.1.0+1\n")
                path.chmod(0o644)

    def test_ui_asset_tamper_blocks_verification(self):
        self.assemble()
        path = self.root / "payload/client/data/flutter_assets/assets/icons/check.svg"
        path.write_bytes(b"altered icon")
        with self.assertRaisesRegex(ValueError, "manifest mismatch"):
            package.verify(self.root)

    def test_ui_asset_size_limit_is_preserved(self):
        self.assertEqual(package.MAX_FILE, 128 * 1024 * 1024)
        path = self.root / "payload/client/data/flutter_assets/assets/README.md"
        with path.open("wb") as handle:
            handle.truncate(package.MAX_FILE + 1)
        with self.assertRaisesRegex(ValueError, "Unsafe or oversized package file"):
            package.check_payload(self.root)

    def test_ui_asset_link_is_rejected(self):
        path = self.root / "payload/client/data/flutter_assets/assets/icons/check.svg"
        path.unlink()
        path.symlink_to("send.svg")
        with self.assertRaisesRegex(ValueError, "Unsafe package input"):
            package.check_payload(self.root)

    def test_native_help_runs_without_interpreter_or_build_tools(self):
        self.native_required()
        self.assemble()
        result = subprocess.run([str(self.root / "smith-installer"), "--help"],
                                env=self.env, capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("--artifacts-only", result.stdout)
        self.assertIn("--pull-model", result.stdout)

    def test_packaged_dry_run_uses_real_executable_root_and_writes_nothing(self):
        self.native_required()
        self.assemble()
        before = self.snapshot()
        result = subprocess.run([str(self.root / "smith-installer"), "--dry-run"],
                                cwd=str(self.base), env=self.env, capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("Dry run", result.stdout)
        self.assertEqual(self.snapshot(), before)
        self.assertEqual(list(self.data.iterdir()), [])
        self.assertEqual(list(self.home.iterdir()), [])

    def test_alias_does_not_choose_a_cwd_or_alias_directory_payload(self):
        self.native_required()
        self.assemble()
        alias = self.base / "unrelated-alias"
        alias.symlink_to(self.root / "smith-installer")
        result = subprocess.run([str(alias), "--dry-run"], cwd=str(self.base), env=self.env,
                                capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(list(self.data.iterdir()), [])

    def test_initial_native_decline_executes_no_synthetic_service(self):
        self.native_required()
        self.assemble()
        result = subprocess.run([str(self.root / "smith-installer")], input="n\n",
                                env=self.env, capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("Declin", result.stdout)
        self.assertEqual(list(self.data.iterdir()), [])
        self.assertEqual(list(self.home.iterdir()), [])

    def test_script_entrypoint_is_rejected(self):
        (self.root / "smith-installer").write_bytes(b"#!/usr/bin/env python3\n")
        with self.assertRaisesRegex(ValueError, "ELF"):
            self.assemble()

    def test_visual_help_and_version_need_no_display_or_interpreter(self):
        if not self.native_ui:
            self.skipTest("SMITH_TEST_INSTALLER_UI required for actual visual entrypoint")
        self.assemble()
        env = dict(self.env, DISPLAY="", WAYLAND_DISPLAY="")
        before = self.snapshot()
        for flag in ("--help", "--version"):
            result = subprocess.run([str(self.root / "smith-installer-ui"), flag],
                                    env=env, capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn("smith-installer", result.stdout.lower())
        self.assertEqual(self.snapshot(), before)
        self.assertEqual(list(self.data.iterdir()), [])
        self.assertEqual(list(self.home.iterdir()), [])

    def test_visual_without_display_exits_before_any_installation(self):
        if not self.native_ui:
            self.skipTest("SMITH_TEST_INSTALLER_UI required for actual visual entrypoint")
        self.assemble()
        result = subprocess.run([str(self.root / "smith-installer-ui")],
                                env=dict(self.env, DISPLAY="", WAYLAND_DISPLAY=""),
                                capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("No graphical session", result.stderr)
        self.assertEqual(list(self.data.iterdir()), [])
        self.assertEqual(list(self.home.iterdir()), [])

    def test_visual_script_entrypoint_is_rejected(self):
        (self.root / "smith-installer-ui").write_bytes(b"#!/bin/sh\n")
        with self.assertRaisesRegex(ValueError, "ELF"):
            self.assemble()

    def test_visual_mode_tamper_blocks_verification(self):
        self.assemble()
        (self.root / "smith-installer-ui").chmod(0o644)
        with self.assertRaises(ValueError):
            package.verify(self.root)

    def test_missing_visual_native_payload_is_rejected(self):
        (self.root / "smith-installer-ui").unlink()
        with self.assertRaises(FileNotFoundError):
            self.assemble()

    def test_incomplete_native_runtime_is_rejected(self):
        (self.root / "payload/client/smith_backend/runtime/cpu/libgomp.so.1").unlink()
        with self.assertRaisesRegex(ValueError, "Incomplete or mixed native runtime"):
            self.assemble()

    def test_missing_vulkan_library_is_rejected(self):
        (self.root / "payload/client/smith_backend/runtime/vulkan/libggml-vulkan.so").unlink()
        with self.assertRaisesRegex(ValueError, "Incomplete or mixed native runtime"):
            self.assemble()

    def test_flat_runtime_file_is_rejected(self):
        self.prohibited_payload("smith_backend/runtime/llama-server")

    def test_vulkan_library_in_cpu_runtime_is_rejected(self):
        self.prohibited_payload("smith_backend/runtime/cpu/libggml-vulkan.so")

    def test_unreviewed_runtime_backend_is_rejected(self):
        self.prohibited_payload("smith_backend/runtime/cuda/llama-server")

    def test_extra_vulkan_backend_file_is_rejected(self):
        self.prohibited_payload("smith_backend/runtime/vulkan/libggml-rpc.so")

    def test_backend_runtime_link_is_rejected(self):
        server = self.root / "payload/client/smith_backend/runtime/cpu/llama-server"
        server.unlink()
        server.symlink_to("../vulkan/llama-server")
        with self.assertRaises(ValueError):
            self.assemble()

    def test_both_backend_servers_must_be_native_elfs(self):
        for backend in ("cpu", "vulkan"):
            with self.subTest(backend=backend):
                server = self.root / "payload/client/smith_backend/runtime" / backend / "llama-server"
                original = server.read_bytes()
                server.write_bytes(b"not an ELF")
                with self.assertRaisesRegex(ValueError, "ELF"):
                    self.assemble()
                server.write_bytes(original)
                (self.root / "README.txt").unlink()

    def test_payload_tamper_blocks_verification(self):
        self.assemble()
        (self.root / "payload/client/data/flutter_assets/AssetManifest.bin").write_bytes(b"altered")
        with self.assertRaises(ValueError):
            package.verify(self.root)

    def test_mode_tamper_blocks_verification(self):
        self.assemble()
        (self.root / "payload/service/smith-service").chmod(0o644)
        with self.assertRaises(ValueError):
            package.verify(self.root)

    def test_mismatched_pair_blocks_assembly(self):
        (self.root / "payload/service/smith-service").write_bytes(synthetic_elf() + b"different")
        with self.assertRaises(ValueError):
            self.assemble()

    def prohibited_payload(self, relative):
        path = self.root / "payload/client" / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        for directory in self.root.rglob("*"):
            if directory.is_dir():
                directory.chmod(0o755)
        path.write_bytes(b"synthetic prohibited payload")
        path.chmod(0o644)
        with self.assertRaisesRegex(ValueError, "Unreviewed client payload"):
            self.assemble()

    def test_model_gguf_is_rejected(self):
        self.prohibited_payload("models/example.gguf")

    def test_nested_safetensors_are_rejected(self):
        self.prohibited_payload("data/flutter_assets/weights/example.safetensors")

    def test_voice_onnx_is_rejected(self):
        self.prohibited_payload("data/flutter_assets/voice/kokoro.onnx")

    def test_unreviewed_opaque_asset_is_rejected(self):
        self.prohibited_payload("data/flutter_assets/opaque.bin")

    def test_unknown_icon_is_rejected(self):
        self.prohibited_payload("data/flutter_assets/assets/icons/activity.svg")

    def test_unreviewed_font_weight_is_rejected(self):
        self.prohibited_payload("data/flutter_assets/assets/fonts/jetbrains-mono/JetBrainsMono-Bold.ttf")

    def test_model_weights_inside_ui_assets_are_rejected(self):
        self.prohibited_payload("data/flutter_assets/assets/fonts/ibm-plex-mono/model.gguf")

    def test_opaque_file_inside_approved_icon_directory_is_rejected(self):
        self.prohibited_payload("data/flutter_assets/assets/icons/opaque.bin")

    def test_raster_reference_is_rejected(self):
        self.prohibited_payload("data/flutter_assets/assets/reference.png")

    def test_empty_unreviewed_ui_directory_is_rejected(self):
        (self.root / "payload/client/data/flutter_assets/assets/weights").mkdir(mode=0o755)
        with self.assertRaisesRegex(ValueError, "Unreviewed client payload"):
            self.assemble()

    def test_empty_model_directory_is_rejected(self):
        (self.root / "payload/client/models").mkdir(mode=0o755)
        with self.assertRaisesRegex(ValueError, "Unreviewed client payload"):
            self.assemble()

    def test_symlink_payload_is_rejected(self):
        (self.root / "payload/client/data/foreign").symlink_to(self.readme)
        with self.assertRaises(ValueError):
            self.assemble()


if __name__ == "__main__":
    unittest.main()
