"""Builder package checks; actual native help/dry-run use private roots only."""

import hashlib
import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

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

    def location_snapshot(self):
        return {str(path.relative_to(self.base)):
                (path.lstat().st_mode, path.lstat().st_uid,
                 path.read_bytes() if path.is_file() and not path.is_symlink() else None)
                for path in self.base.rglob("*")}

    def test_location_unsafe_ancestor_preserves_modes_contents_and_absence(self):
        installer = self.base / "installer"
        installer.mkdir(mode=0o700)
        dist = installer / "dist"
        dist.mkdir(mode=0o700)
        (dist / "keep").write_bytes(b"unchanged")
        installer.chmod(0o775)
        before = self.location_snapshot()
        for root in (dist, installer / "absent"):
            for create in (False, True):
                with self.subTest(root=root.name, create=create):
                    with self.assertRaisesRegex(ValueError, "Build location blocked") as raised:
                        package.check_location(root, create=create)
                    message = str(raised.exception)
                    self.assertIn(str(installer), message)
                    self.assertIn("0775", message)
                    self.assertIn(str(os.geteuid()), message)
                    self.assertIn(f"chmod go-w -- {installer}", message)
                    self.assertNotIn("chmod -R", message)
                    self.assertEqual(self.location_snapshot(), before)

    def test_location_preserves_safe_existing_modes(self):
        for mode in (0o700, 0o755):
            self.root.chmod(mode)
            before = self.location_snapshot()
            for create in (False, True):
                with self.subTest(mode=oct(mode), create=create):
                    self.assertEqual(package.check_location(self.root, create=create), self.root)
                    self.assertEqual(self.location_snapshot(), before)

    def test_location_relative_result_is_absolute(self):
        with mock.patch.object(package.os, "getcwd", return_value=str(self.base)):
            result = package.check_location(Path("recipient"))
        self.assertIsInstance(result, Path)
        self.assertEqual(result, self.root)
        self.assertTrue(result.is_absolute())
        # Exercise the same relative spelling through the real CLI in a private cwd.
        result = subprocess.run(
            [sys.executable, "-B", str(Path(package.__file__).resolve()),
             "--root", "recipient", "--check-location"],
            cwd=self.base, capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(str(self.root), result.stdout)

    def test_location_rejects_links_files_and_missing_intermediate(self):
        link = self.base / "alias"
        link.symlink_to(self.root, target_is_directory=True)
        leaf = self.base / "ordinary-file"
        leaf.write_bytes(b"keep")
        for root in (link, link / "child", leaf, leaf / "child",
                     self.base / "missing" / "dist"):
            for create in (False, True):
                with self.subTest(root=root, create=create):
                    before = self.location_snapshot()
                    with self.assertRaises(ValueError):
                        package.check_location(root, create=create)
                    self.assertEqual(self.location_snapshot(), before)

    def test_location_rejects_restricted_spelling_and_parent_components(self):
        for name in ("has space", "nonascii-é", "has:colon", "line\nbreak", "../escape"):
            with self.subTest(name=name):
                with self.assertRaises(ValueError):
                    package.check_location(self.base / name, create=True)
        self.assertFalse((self.base / "escape").exists())

    def test_location_rejects_raw_noncanonical_spelling_before_normalization(self):
        spellings = (f"{self.base}/./recipient", f"{self.base}//recipient",
                     f"{self.root}/", "./recipient", "recipient//", "recipient/./missing")
        command = [sys.executable, "-B", str(Path(package.__file__).resolve())]
        before = self.location_snapshot()
        for spelling in spellings:
            for create in (False, True):
                with self.subTest(spelling=spelling, create=create):
                    with mock.patch.object(package.os, "open", side_effect=AssertionError("path open")):
                        with self.assertRaisesRegex(ValueError, "normalized spelling"):
                            package.check_location(spelling, create=create)
                    action = "--prepare-location" if create else "--check-location"
                    result = subprocess.run(command + ["--root", spelling, action],
                                            cwd=self.base, capture_output=True,
                                            text=True, timeout=10)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("normalized spelling", result.stderr)
                    self.assertNotIn("Traceback", result.stderr)
                    self.assertEqual(self.location_snapshot(), before)

    def test_location_creates_only_private_final_leaf_without_chmod_or_umask_change(self):
        leaf = self.base / "generated"
        previous = os.umask(0o002)
        try:
            with mock.patch.object(package.os, "chmod", side_effect=AssertionError("no repair")):
                self.assertEqual(package.check_location(leaf, create=True), leaf)
            self.assertEqual(stat.S_IMODE(leaf.stat().st_mode), 0o700)
            self.assertEqual(os.umask(0o002), 0o002)
        finally:
            os.umask(previous)
        before = self.location_snapshot()
        with self.assertRaises(ValueError):
            package.check_location(self.base / "read-only-absent")
        self.assertEqual(self.location_snapshot(), before)

    def test_location_concurrent_creator_is_reopened_and_validated(self):
        mkdir = os.mkdir
        for kind in ("safe", "writable", "symlink", "file"):
            leaf = self.base / ("race-" + kind)

            def competing_creator(path, mode=0o777, *, dir_fd=None):
                if kind in ("safe", "writable"):
                    mkdir(path, mode=0o700, dir_fd=dir_fd)
                    leaf.chmod(0o755 if kind == "safe" else 0o777)
                elif kind == "symlink":
                    leaf.symlink_to(self.root, target_is_directory=True)
                else:
                    leaf.write_bytes(b"competitor")
                raise FileExistsError("competing creator won")

            with self.subTest(kind=kind), mock.patch.object(
                    package.os, "mkdir", side_effect=competing_creator) as create:
                if kind == "safe":
                    self.assertEqual(package.check_location(leaf, create=True), leaf)
                    self.assertEqual(stat.S_IMODE(leaf.stat().st_mode), 0o755)
                else:
                    with self.assertRaises(ValueError):
                        package.check_location(leaf, create=True)
                create.assert_called_once()
            self.assertEqual(leaf.is_symlink(), kind == "symlink")
            if kind == "writable":
                self.assertEqual(stat.S_IMODE(leaf.stat().st_mode), 0o777)
            if kind == "file":
                self.assertEqual(leaf.read_bytes(), b"competitor")

    def test_location_ownership_and_sticky_exception_are_component_scoped(self):
        fstat = os.fstat
        ancestor_inode = self.base.stat().st_ino
        leaf_inode = self.root.stat().st_ino
        uid = 12345
        for component, owner, mode, accepted in (
                (ancestor_inode, uid, 0o755, True),
                (ancestor_inode, 0, 0o755, True),
                (ancestor_inode, 0, 0o1777, True),
                (ancestor_inode, uid, 0o1777, False),
                (ancestor_inode, 54321, 0o755, False),
                (ancestor_inode, 0, 0o777, False),
                (leaf_inode, 0, 0o755, False),
                (leaf_inode, 54321, 0o700, False),
                (leaf_inode, uid, 0o1777, False)):
            def metadata(fd):
                info = fstat(fd)
                return SimpleNamespace(
                    st_uid=owner if info.st_ino == component else
                    (uid if info.st_uid == os.getuid() and info.st_uid != 0 else info.st_uid),
                    st_mode=stat.S_IFDIR | mode if info.st_ino == component else info.st_mode)

            # Make the leaf current-owned even when this fixture is run as root.
            def fixture_metadata(fd):
                info = metadata(fd)
                if fstat(fd).st_ino == leaf_inode and component != leaf_inode:
                    info.st_uid = uid
                return info

            with self.subTest(component=component, owner=owner, mode=oct(mode)), \
                    mock.patch.object(package.os, "geteuid", return_value=uid), \
                    mock.patch.object(package.os, "fstat", side_effect=fixture_metadata):
                if accepted:
                    self.assertEqual(package.check_location(self.root), self.root)
                else:
                    with self.assertRaises(ValueError) as raised:
                        package.check_location(self.root)
                    if owner != uid:
                        self.assertNotIn("chmod", str(raised.exception))

    def test_location_traversal_uses_relative_no_follow_directory_fds(self):
        real_open = os.open
        with mock.patch.object(package.os, "open", wraps=real_open) as opened:
            package.check_location(self.root)
        calls = opened.call_args_list
        self.assertEqual(calls[0].args[0], "/")
        for call in calls:
            self.assertEqual(call.args[1] & (os.O_DIRECTORY | os.O_NOFOLLOW),
                             os.O_DIRECTORY | os.O_NOFOLLOW)
        for call in calls[1:]:
            self.assertNotIn("/", call.args[0])
            self.assertIsInstance(call.kwargs["dir_fd"], int)

    def test_location_build_refuses_before_readme_read_or_write(self):
        self.base.chmod(0o775)
        before = self.location_snapshot()
        with mock.patch.object(Path, "read_text", side_effect=AssertionError("README read")), \
                mock.patch.object(package, "write_new", side_effect=AssertionError("metadata write")):
            with self.assertRaisesRegex(ValueError, "Build location blocked"):
                self.assemble()
        self.assertEqual(self.location_snapshot(), before)

    def test_location_verify_refuses_before_manifest_read(self):
        self.base.chmod(0o775)
        before = self.location_snapshot()
        with mock.patch.object(package, "file_bytes", side_effect=AssertionError("manifest read")):
            with self.assertRaisesRegex(ValueError, "Build location blocked"):
                package.verify(self.root)
        self.assertEqual(self.location_snapshot(), before)

    def test_location_cli_read_only_no_bytecode_and_exclusive_actions(self):
        tooling = self.base / "tooling"
        tooling.mkdir(mode=0o700)
        for name in ("package.py", "runtime.py"):
            shutil.copyfile(Path(package.__file__).parent / name, tooling / name)
        command = [sys.executable, "-B", str(tooling / "package.py"), "--root", str(self.root)]
        before = self.location_snapshot()
        result = subprocess.run(command + ["--check-location"], capture_output=True,
                                text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.location_snapshot(), before)
        self.assertFalse(list(tooling.rglob("*.pyc")))
        self.assertFalse((tooling / "__pycache__").exists())
        for first, second in (("--prepare-location", "--check-location"),
                              ("--prepare-location", "--verify"),
                              ("--check-location", "--verify")):
            result = subprocess.run(command + [first, second], capture_output=True,
                                    text=True, timeout=10)
            self.assertNotEqual(result.returncode, 0)
            self.assertNotIn("Traceback", result.stderr)
        self.base.chmod(0o775)
        result = subprocess.run(command + ["--check-location"], capture_output=True,
                                text=True, timeout=10)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Build location blocked", result.stderr)
        self.assertNotIn("Traceback", result.stderr)

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
                     "rotate-ccw", "info", "check", "chevron-down", "layers", "git-branch"):
            expected.add(prefix + "icons/" + icon + ".svg")
        self.assertEqual(len(expected), 26)
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
