"""Isolated installer tests: no real HOME, manager, GUI or artifact execution."""

import contextlib
import fcntl
import importlib.util
import io
import os
import resource
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

INSTALLER_PATH = Path(__file__).parents[1] / "install.py"
SPEC = importlib.util.spec_from_file_location("smith_next_install", INSTALLER_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("Cannot load installer under test")
install = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(install)


@unittest.skipIf(os.geteuid() == 0, "Run installer integration tests as a non-root user")
class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="smith-install-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.home = self.root / "home"
        self.data = self.root / "data"
        self.bundle = self.root / "bundle"
        for path in (self.home, self.data, self.bundle):
            path.mkdir(mode=0o700)
        self.env = {"HOME": str(self.home), "XDG_DATA_HOME": str(self.data)}
        self.service = self.root / "smith-service"
        self.write(self.service, b"service-v1", 0o755)
        for name in ("lib", "data/flutter_assets", "smith_backend",
                     "share/pixmaps", "share/applications"):
            (self.bundle / name).mkdir(mode=0o755, parents=True, exist_ok=True)
        for name, content, mode in (
            ("smith_client", b"client-v1", 0o755),
            ("smith_backend/smith-service", b"service-v1", 0o755),
            ("lib/libflutter_linux_gtk.so", b"flutter", 0o644),
            ("lib/libapp.so", b"app", 0o644),
            ("data/icudtl.dat", b"icu", 0o644),
            ("data/flutter_assets/asset", b"asset", 0o644),
            ("share/pixmaps/" + install.APP + ".png", b"icon", 0o644),
            ("share/applications/" + install.APP + ".desktop", b"desktop", 0o644),
        ):
            self.write(self.bundle / name, content, mode)
        for path in self.bundle.rglob("*"):
            if path.is_dir():
                path.chmod(0o755)
        self.log = io.StringIO()

    @staticmethod
    def write(path, content, mode):
        path.write_bytes(content)
        path.chmod(mode)

    def make_private_parents(self, path):
        path.mkdir(mode=0o700, parents=True, exist_ok=True)
        while path not in (self.home, self.data, self.root):
            path.chmod(0o700)
            path = path.parent

    def installer(self, client=True, service=True):
        return install.Installer(str(self.bundle) if client else None,
                                 str(self.service) if service else None, self.env)

    def run_install(self, client=True, service=True, **kwargs):
        return self.installer(client, service).run(output=self.log, **kwargs)

    def snapshot(self):
        return {str(p.relative_to(self.root)): (stat.S_IMODE(p.lstat().st_mode),
                os.readlink(p) if p.is_symlink() else p.read_bytes() if p.is_file() else None)
                for p in self.root.rglob("*")}

    def test_dry_run_and_decline_are_read_only(self):
        before = self.snapshot()
        self.assertEqual(self.run_install(dry_run=True), 0)
        self.assertEqual(self.snapshot(), before)
        self.assertEqual(self.run_install(confirm=lambda _: "n"), 0)
        self.assertEqual(self.snapshot(), before)

    def test_both_install_complete_and_idempotent(self):
        self.assertEqual(self.run_install(yes=True), 0, self.log.getvalue())
        base = self.data / "smith-next"
        binary = base / "bin/smith-service"
        self.assertEqual(binary.read_bytes(), self.service.read_bytes())
        self.assertEqual(stat.S_IMODE(binary.stat().st_mode), 0o755)
        self.assertEqual(binary.stat().st_nlink, 1)
        current = base / "client/current"
        self.assertRegex(os.readlink(current), r"^releases/[a-f0-9]{64}$")
        with install.Filesystem() as fs:
            install.verify_release(fs, str(current.parent / os.readlink(current)))
        self.assertEqual((current / "data/flutter_assets/asset").read_bytes(), b"asset")
        desktop = self.data / "applications" / (install.APP + ".desktop")
        self.assertIn("Name=smith\n", desktop.read_text())
        self.assertIn("Exec=" + str(current / "smith_client"), desktop.read_text())
        self.assertIn("Icon=" + str(current / "share/pixmaps" / (install.APP + ".png")),
                      desktop.read_text())
        shim = self.home / ".local/bin/smith-next"
        self.assertIn('"$@"', shim.read_text())
        self.assertEqual(stat.S_IMODE(shim.stat().st_mode), 0o755)
        before = self.snapshot()
        inode = binary.stat().st_ino
        self.assertEqual(self.run_install(yes=True), 0, self.log.getvalue())
        self.assertEqual(self.snapshot(), before)
        self.assertEqual(binary.stat().st_ino, inode)
        self.assertFalse((self.data / "systemd").exists())

    def test_service_then_client(self):
        self.assertEqual(self.run_install(client=False, yes=True), 0, self.log.getvalue())
        self.assertEqual(self.run_install(service=False, yes=True), 0, self.log.getvalue())

    def test_client_then_service(self):
        self.assertEqual(self.run_install(service=False, yes=True), 0, self.log.getvalue())
        self.assertEqual(self.run_install(client=False, yes=True), 0, self.log.getvalue())

    def test_service_only_rejects_selected_client_helper_mismatch(self):
        self.assertEqual(self.run_install(service=False, yes=True), 0, self.log.getvalue())
        self.write(self.service, b"different", 0o755)
        before = self.snapshot()
        self.assertEqual(self.run_install(client=False, yes=True), 1)
        self.assertEqual(self.snapshot(), before)
        self.assertFalse((self.data / "smith-next/bin/smith-service").exists())

    def test_service_only_does_not_require_home(self):
        item = self.installer(client=False)
        item.home = "/missing-home"
        self.assertEqual(item.run(yes=True, output=self.log), 0, self.log.getvalue())
        self.assertFalse((self.data / "applications").exists())

    def test_service_only_post_publication_changes_fail_final_verification(self):
        binary = self.data / "smith-next/bin/smith-service"
        original = install.Filesystem.publish
        for change in ("bytes", "mode", "delete"):
            with self.subTest(change=change):
                self.log = io.StringIO()

                def changed(fs, path, data, mode, change=change):
                    original(fs, path, data, mode)
                    if path == str(binary):
                        if change == "bytes":
                            self.write(binary, b"post-publication foreign bytes", 0o755)
                        elif change == "mode":
                            binary.chmod(0o750)
                        else:
                            binary.unlink()
                with mock.patch.object(install.Filesystem, "publish", changed):
                    self.assertEqual(self.run_install(client=False, yes=True), 1)
                self.assertNotIn("Installation complete", self.log.getvalue())
                self.assertIn("published " + str(binary), self.log.getvalue())
                self.assertIn("No rollback assumed", self.log.getvalue())
                if change == "bytes":
                    self.assertEqual(binary.read_bytes(), b"post-publication foreign bytes")
                elif change == "mode":
                    self.assertEqual(stat.S_IMODE(binary.stat().st_mode), 0o750)
                else:
                    self.assertFalse(binary.exists())
                if binary.exists():
                    binary.unlink()

    def test_existing_service_only_changes_at_apply_fail_final_verification(self):
        self.assertEqual(self.run_install(client=False, yes=True), 0, self.log.getvalue())
        binary = self.data / "smith-next/bin/smith-service"
        original = install.Installer.apply
        for change in ("bytes", "mode", "delete"):
            with self.subTest(change=change):
                self.log = io.StringIO()

                def changed(item, fs, plan, change=change):
                    self.assertIsNotNone(plan["installed"])
                    if change == "bytes":
                        self.write(binary, b"late existing-service bytes", 0o755)
                    elif change == "mode":
                        binary.chmod(0o750)
                    else:
                        binary.unlink()
                    return original(item, fs, plan)
                with (mock.patch.object(install.Installer, "apply", changed),
                      mock.patch.object(install.Filesystem, "publish") as publication):
                    self.assertEqual(self.run_install(client=False, yes=True), 1)
                    publication.assert_not_called()
                self.assertNotIn("Installation complete", self.log.getvalue())
                if change == "bytes":
                    self.assertEqual(binary.read_bytes(), b"late existing-service bytes")
                elif change == "mode":
                    self.assertEqual(stat.S_IMODE(binary.stat().st_mode), 0o750)
                else:
                    self.assertFalse(binary.exists())
                self.write(binary, self.service.read_bytes(), 0o755)

    def test_service_only_verified_idempotent_install_has_no_publication(self):
        self.assertEqual(self.run_install(client=False, yes=True), 0, self.log.getvalue())
        before = self.snapshot()
        binary = self.data / "smith-next/bin/smith-service"
        inode = binary.stat().st_ino
        with mock.patch.object(install.Filesystem, "publish") as publication:
            self.assertEqual(self.run_install(client=False, yes=True), 0, self.log.getvalue())
            publication.assert_not_called()
        self.assertEqual(self.snapshot(), before)
        self.assertEqual(binary.stat().st_ino, inode)

    def test_preserves_old_release_on_client_update(self):
        self.assertEqual(self.run_install(yes=True), 0)
        current = self.data / "smith-next/client/current"
        old = os.readlink(current)
        old_selector_inode = current.lstat().st_ino
        self.write(self.bundle / "data/flutter_assets/asset", b"new asset", 0o644)
        self.assertEqual(self.run_install(yes=True), 0, self.log.getvalue())
        self.assertNotEqual(old, os.readlink(current))
        self.assertEqual((current.parent / old / "data/flutter_assets/asset").read_bytes(), b"asset")
        retained = list(current.parent.glob(".smith-next-selector-*"))
        self.assertEqual(len(retained), 1)
        self.assertEqual(os.readlink(retained[0]), old)
        self.assertEqual(retained[0].lstat().st_ino, old_selector_inode)

    def test_no_unknown_destination_overwrite(self):
        for destination in (self.data / "applications" / (install.APP + ".desktop"),
                            self.home / ".local/bin/smith-next",
                            self.data / "smith-next/bin/smith-service"):
            with self.subTest(destination=destination):
                self.make_private_parents(destination.parent)
                self.write(destination, b"unknown", 0o755 if destination.name != install.APP + ".desktop" else 0o644)
                before = self.snapshot()
                self.assertEqual(self.run_install(yes=True), 1)
                self.assertEqual(self.snapshot(), before)
                destination.unlink()

    def test_different_service_blocks_client_switch(self):
        self.assertEqual(self.run_install(yes=True), 0)
        before = self.snapshot()
        self.write(self.service, b"v2", 0o755)
        self.write(self.bundle / "smith_backend/smith-service", b"v2", 0o755)
        expected = self.snapshot()
        self.assertEqual(self.run_install(yes=True), 1)
        self.assertEqual(self.snapshot(), expected)
        self.assertEqual((self.data / "smith-next/bin/smith-service").read_bytes(), b"service-v1")
        self.assertEqual(before[str(Path("data/smith-next/client/current"))],
                         expected[str(Path("data/smith-next/client/current"))])

    def test_mismatched_helpers_block_all_writes(self):
        self.write(self.service, b"different", 0o755)
        before = self.snapshot()
        self.assertEqual(self.run_install(yes=True), 1)
        self.assertEqual(self.snapshot(), before)

    def test_source_mode_hardlinks_and_special_files(self):
        self.service.chmod(0o750)
        self.assertEqual(self.run_install(client=False, yes=True), 1)
        self.service.chmod(0o755)
        alias = self.root / "alias"
        os.link(self.service, alias)
        self.assertEqual(self.run_install(client=False, yes=True), 1)
        alias.unlink()
        fifo = self.bundle / "fifo"
        os.mkfifo(fifo)
        self.assertEqual(self.run_install(yes=True), 1)
        fifo.unlink()
        os.symlink(self.service, self.bundle / "alias")
        self.assertEqual(self.run_install(yes=True), 1)

    def test_symlink_ancestors_and_destinations(self):
        outside = self.root / "outside"
        outside.mkdir(mode=0o700)
        (self.data / "smith-next").symlink_to(outside, target_is_directory=True)
        before = self.snapshot()
        self.assertEqual(self.run_install(yes=True), 1)
        self.assertEqual(self.snapshot(), before)
        (self.data / "smith-next").unlink()
        applications = self.data / "applications"
        applications.symlink_to(outside, target_is_directory=True)
        self.assertEqual(self.run_install(yes=True), 1)
        self.assertEqual(list(outside.iterdir()), [])

    def test_unsafe_and_missing_roots(self):
        self.data.chmod(0o777)
        self.assertEqual(self.run_install(yes=True), 1)
        self.data.chmod(0o700)
        self.data.rmdir()
        self.assertEqual(self.run_install(yes=True), 1)
        self.assertFalse(self.data.exists())

    def test_private_namespace_required(self):
        namespace = self.data / "smith-next"
        namespace.mkdir(mode=0o755)
        self.assertEqual(self.run_install(yes=True), 1)

    def test_root_refusal(self):
        before = self.snapshot()
        with mock.patch.object(install.os, "geteuid", return_value=0):
            self.assertEqual(self.run_install(yes=True), 1)
        self.assertEqual(self.snapshot(), before)

    def test_changed_source_after_confirmation(self):
        def confirm(_):
            self.write(self.service, b"changed", 0o755)
            return "y"
        self.assertEqual(self.run_install(confirm=confirm), 1)
        self.assertFalse((self.data / ".smith-next-install.lock").exists())
        self.assertFalse((self.data / "smith-next").exists())

    def test_changed_destination_after_confirmation(self):
        def confirm(_):
            path = self.data / "smith-next/bin"
            self.make_private_parents(path)
            self.write(path / "smith-service", b"foreign", 0o755)
            return "y"
        self.assertEqual(self.run_install(confirm=confirm), 1)
        self.assertEqual((self.data / "smith-next/bin/smith-service").read_bytes(), b"foreign")

    def test_revalidate_under_lock(self):
        original = install.Installer.lock

        @contextlib.contextmanager
        def changed(item, fs):
            with original(item, fs):
                self.write(self.service, b"changed", 0o755)
                yield
        with mock.patch.object(install.Installer, "lock", changed):
            self.assertEqual(self.run_install(yes=True), 1)
        self.assertFalse((self.data / "smith-next").exists())

    def test_bounded_concurrent_lock(self):
        lock = self.data / ".smith-next-install.lock"
        self.write(lock, b"", 0o600)
        with lock.open("rb") as held:
            fcntl.flock(held, fcntl.LOCK_EX)
            with mock.patch.object(install, "LOCK_SECONDS", 0.05):
                self.assertEqual(self.run_install(yes=True), 1)
        self.assertIn("lock timeout", self.log.getvalue())
        self.assertFalse((self.data / "smith-next").exists())

    def test_unsafe_lock(self):
        (self.data / ".smith-next-install.lock").symlink_to(self.service)
        self.assertEqual(self.run_install(yes=True), 1)
        self.assertFalse((self.data / "smith-next").exists())

    def test_current_must_be_managed_and_verified(self):
        self.assertEqual(self.run_install(yes=True), 0)
        current = self.data / "smith-next/client/current"
        target = os.readlink(current)
        current.unlink()
        current.symlink_to(self.bundle)
        self.assertEqual(self.run_install(yes=True), 1)
        current.unlink()
        current.symlink_to(target)
        self.write(current / "data/icudtl.dat", b"tampered", 0o644)
        self.assertEqual(self.run_install(yes=True), 1)

    def test_existing_release_rejects_extra_files_modes_and_manifest_drift(self):
        self.assertEqual(self.run_install(yes=True), 0)
        current = self.data / "smith-next/client/current"
        extra = current / "extra"
        self.write(extra, b"extra", 0o644)
        self.assertEqual(self.run_install(yes=True), 1)
        extra.unlink()
        (current / "lib").chmod(0o700)
        self.assertEqual(self.run_install(yes=True), 1)
        (current / "lib").chmod(0o755)
        self.write(current / install.MANIFEST, b"{}", 0o600)
        self.assertEqual(self.run_install(yes=True), 1)

    def test_incomplete_bundle_blocks_all_writes(self):
        (self.bundle / "lib/libapp.so").unlink()
        before = self.snapshot()
        self.assertEqual(self.run_install(yes=True), 1)
        self.assertEqual(self.snapshot(), before)

    def test_no_clobber_service_publication_race(self):
        original = install.Filesystem.publish

        def conflict(fs, path, data, mode):
            if path.endswith("/bin/smith-service"):
                self.write(Path(path), b"racer", 0o755)
            return original(fs, path, data, mode)
        with mock.patch.object(install.Filesystem, "publish", conflict):
            self.assertEqual(self.run_install(yes=True), 1)
        self.assertEqual((self.data / "smith-next/bin/smith-service").read_bytes(), b"racer")
        self.assertFalse((self.data / "smith-next/client/current").exists())

    def test_partial_failure_preserved_reported_and_retry(self):
        original = install.Filesystem.publish

        def failure(fs, path, data, mode):
            if "/applications/" in path and not "/releases/" in path:
                raise OSError("injected launcher failure")
            return original(fs, path, data, mode)
        with mock.patch.object(install.Filesystem, "publish", failure):
            self.assertEqual(self.run_install(yes=True), 1)
        self.assertIn("published immutable release", self.log.getvalue())
        self.assertIn("No rollback assumed", self.log.getvalue())
        self.assertTrue((self.data / "smith-next/bin/smith-service").exists())
        self.assertFalse((self.data / "smith-next/client/current").exists())
        self.assertEqual(self.run_install(yes=True), 0, self.log.getvalue())

    def test_directory_no_replace_rejects_even_empty_unknown_destination(self):
        with install.Filesystem() as fs:
            fd = fs.directory(str(self.root))
            os.mkdir("stage", dir_fd=fd)
            os.mkdir("unknown", dir_fd=fd)
            with self.assertRaises(FileExistsError):
                install.no_replace(fd, "stage", fd, "unknown")
            self.assertTrue((self.root / "stage").is_dir())
            self.assertTrue((self.root / "unknown").is_dir())

    def test_descriptor_relative_traversal_blocks_symlink_swap(self):
        original = install.os.open
        swapped = False

        def swapped_open(path, flags, *args, **kwargs):
            nonlocal swapped
            if path == "smith-next" and kwargs.get("dir_fd") is not None and not swapped:
                swapped = True
                (self.data / "smith-next").symlink_to(self.bundle)
            return original(path, flags, *args, **kwargs)
        with mock.patch.object(install.os, "open", swapped_open):
            self.assertEqual(self.run_install(yes=True), 1)
        self.assertFalse((self.bundle / "bin").exists())

    def test_current_no_clobber_race(self):
        original = install.no_replace

        def conflict(src_fd, src, dst_fd, dst):
            if dst == "current":
                os.symlink("foreign", "current", dir_fd=dst_fd)
            return original(src_fd, src, dst_fd, dst)
        with mock.patch.object(install, "no_replace", conflict):
            self.assertEqual(self.run_install(yes=True), 1)
        self.assertEqual(os.readlink(self.data / "smith-next/client/current"), "foreign")

    def test_verified_current_replacement_revalidated_before_switch(self):
        self.assertEqual(self.run_install(yes=True), 0)
        self.write(self.bundle / "data/flutter_assets/asset", b"updated", 0o644)
        current = self.data / "smith-next/client/current"
        original = install.os.symlink

        def conflict(target, name, *args, **kwargs):
            result = original(target, name, *args, **kwargs)
            if name.startswith(".smith-next-selector-"):
                current.unlink()
                original("foreign", current)
            return result
        with mock.patch.object(install.os, "symlink", conflict):
            self.assertEqual(self.run_install(yes=True), 1)
        self.assertEqual(os.readlink(current), "foreign")

    def test_late_foreign_regular_current_preserved_by_exchange(self):
        self.assertEqual(self.run_install(yes=True), 0)
        self.log = io.StringIO()
        self.write(self.bundle / "data/flutter_assets/asset", b"updated", 0o644)
        current = self.data / "smith-next/client/current"
        foreign = self.root / "foreign"
        self.write(foreign, b"foreign data must survive", 0o640)
        captured = foreign.stat()
        original = install.exchange

        def conflict(src_fd, src, dst_fd, dst):
            # Inject after every pre-exchange check, at the publication syscall.
            os.replace(foreign, current)
            return original(src_fd, src, dst_fd, dst)
        with mock.patch.object(install, "exchange", conflict):
            self.assertEqual(self.run_install(yes=True), 1)
        retained = list(current.parent.glob(".smith-next-selector-*"))
        self.assertEqual(len(retained), 1)
        self.assertEqual(retained[0].read_bytes(), b"foreign data must survive")
        self.assertEqual(retained[0].stat().st_ino, captured.st_ino)
        self.assertEqual(stat.S_IMODE(retained[0].stat().st_mode), 0o640)
        self.assertTrue(current.is_symlink())
        self.assertEqual((current / "data/flutter_assets/asset").read_bytes(), b"updated")
        self.assertIn("Displaced selector conflict", self.log.getvalue())
        self.assertIn(str(retained[0]), self.log.getvalue())
        self.assertIn("current may have changed, no rollback", self.log.getvalue())
        self.assertNotIn("Installation complete", self.log.getvalue())
        self.assertNotIn("selected verified current release", self.log.getvalue())

    def test_late_same_target_new_selector_inode_is_conflict(self):
        self.assertEqual(self.run_install(yes=True), 0)
        self.log = io.StringIO()
        self.write(self.bundle / "data/flutter_assets/asset", b"updated", 0o644)
        current = self.data / "smith-next/client/current"
        old_target = os.readlink(current)
        foreign = self.root / "foreign"
        foreign.symlink_to(old_target)
        captured = foreign.lstat()
        original = install.exchange

        def conflict(src_fd, src, dst_fd, dst):
            os.replace(foreign, current)
            return original(src_fd, src, dst_fd, dst)
        with mock.patch.object(install, "exchange", conflict):
            self.assertEqual(self.run_install(yes=True), 1)
        retained = list(current.parent.glob(".smith-next-selector-*"))
        self.assertEqual(len(retained), 1)
        self.assertEqual(retained[0].lstat().st_ino, captured.st_ino)
        self.assertEqual(os.readlink(retained[0]), old_target)
        self.assertIn("Displaced selector conflict", self.log.getvalue())

    def test_exchange_never_rolls_back_over_newer_current(self):
        self.assertEqual(self.run_install(yes=True), 0)
        self.log = io.StringIO()
        self.write(self.bundle / "data/flutter_assets/asset", b"updated", 0o644)
        current = self.data / "smith-next/client/current"
        old_inode = current.lstat().st_ino
        original = install.exchange

        def conflict(src_fd, src, dst_fd, dst):
            result = original(src_fd, src, dst_fd, dst)
            current.unlink()
            self.write(current, b"newer foreign data", 0o600)
            return result
        with mock.patch.object(install, "exchange", conflict):
            self.assertEqual(self.run_install(yes=True), 1)
        self.assertEqual(current.read_bytes(), b"newer foreign data")
        retained = list(current.parent.glob(".smith-next-selector-*"))
        self.assertEqual(len(retained), 1)
        self.assertEqual(retained[0].lstat().st_ino, old_inode)
        self.assertNotIn("Installation complete", self.log.getvalue())

    def test_exchange_preserves_unknown_replacement_at_displaced_path(self):
        self.assertEqual(self.run_install(yes=True), 0)
        self.log = io.StringIO()
        self.write(self.bundle / "data/flutter_assets/asset", b"updated", 0o644)
        current = self.data / "smith-next/client/current"
        original = install.exchange

        def conflict(src_fd, src, dst_fd, dst):
            result = original(src_fd, src, dst_fd, dst)
            displaced = current.parent / src
            displaced.unlink()
            self.write(displaced, b"late displaced-path foreign data", 0o600)
            return result
        with mock.patch.object(install, "exchange", conflict):
            self.assertEqual(self.run_install(yes=True), 1)
        retained = list(current.parent.glob(".smith-next-selector-*"))
        self.assertEqual(len(retained), 1)
        self.assertEqual(retained[0].read_bytes(), b"late displaced-path foreign data")
        self.assertIn("Displaced selector conflict", self.log.getvalue())

    def test_exchange_sync_failure_reports_partial_selection_and_retains_old(self):
        self.assertEqual(self.run_install(yes=True), 0)
        self.log = io.StringIO()
        self.write(self.bundle / "data/flutter_assets/asset", b"updated", 0o644)
        current = self.data / "smith-next/client/current"
        old_inode = current.lstat().st_ino
        original = install.exchange

        def failure(src_fd, src, dst_fd, dst):
            result = original(src_fd, src, dst_fd, dst)
            # The next sync, after successful exchange, must not delete or roll back.
            sync_failure.start()
            return result
        sync_failure = mock.patch.object(install.os, "fsync", side_effect=OSError("sync failure"))
        try:
            with mock.patch.object(install, "exchange", failure):
                self.assertEqual(self.run_install(yes=True), 1)
        finally:
            sync_failure.stop()
        retained = list(current.parent.glob(".smith-next-selector-*"))
        self.assertEqual(len(retained), 1)
        self.assertEqual(retained[0].lstat().st_ino, old_inode)
        self.assertEqual((current / "data/flutter_assets/asset").read_bytes(), b"updated")
        self.assertIn("exchanged current selector", self.log.getvalue())
        self.assertIn("verification pending, no rollback", self.log.getvalue())
        self.assertNotIn("Installation complete", self.log.getvalue())

    def test_dependencies_reverified_before_selection(self):
        self.assertEqual(self.run_install(yes=True), 0)
        current = self.data / "smith-next/client/current"
        old_target = os.readlink(current)
        self.write(self.bundle / "data/flutter_assets/asset", b"updated", 0o644)
        original = install.os.symlink
        desktop = self.data / "applications" / (install.APP + ".desktop")
        wrapper = self.home / ".local/bin/smith-next"
        for dependency in ("release", "desktop", "wrapper", "service"):
            with self.subTest(dependency=dependency):
                changed = []

                def conflict(target, name, *args, dependency=dependency, changed=changed, **kwargs):
                    result = original(target, name, *args, **kwargs)
                    if name.startswith(".smith-next-selector-"):
                        path = {"release": current.parent / target / "lib/libapp.so",
                                "desktop": desktop, "wrapper": wrapper,
                                "service": self.data / "smith-next/bin/smith-service"}[dependency]
                        changed.append((path, path.read_bytes(), stat.S_IMODE(path.stat().st_mode)))
                        self.write(path, b"late dependency tampering", changed[0][2])
                    return result
                with mock.patch.object(install.os, "symlink", conflict):
                    self.assertEqual(self.run_install(yes=True), 1)
                self.assertEqual(os.readlink(current), old_target)
                self.assertEqual(len(changed), 1)
                self.write(*changed[0])

    def test_dependencies_reverified_after_exchange_before_success(self):
        self.assertEqual(self.run_install(yes=True), 0)
        self.log = io.StringIO()
        self.write(self.bundle / "data/flutter_assets/asset", b"updated", 0o644)
        current = self.data / "smith-next/client/current"
        original = install.exchange

        def conflict(src_fd, src, dst_fd, dst):
            result = original(src_fd, src, dst_fd, dst)
            self.write(current / "lib/libapp.so", b"late tampering at exchange", 0o644)
            return result
        with mock.patch.object(install, "exchange", conflict):
            self.assertEqual(self.run_install(yes=True), 1)
        self.assertIn("Release manifest/content identity mismatch", self.log.getvalue())
        self.assertIn("exchanged current selector", self.log.getvalue())
        self.assertNotIn("Installation complete", self.log.getvalue())

    def test_staging_failure_retained_and_retry_uses_fresh_stage(self):
        original = install.Filesystem.publish

        def failure(fs, path, data, mode):
            if path.endswith("/data/icudtl.dat"):
                raise OSError("injected stage failure")
            return original(fs, path, data, mode)
        with mock.patch.object(install.Filesystem, "publish", failure):
            self.assertEqual(self.run_install(yes=True), 1)
        releases = self.data / "smith-next/client/releases"
        stages = list(releases.glob(".smith-next-stage-*"))
        self.assertEqual(len(stages), 1)
        self.assertFalse((releases.parent / "current").exists())
        self.assertEqual(self.run_install(yes=True), 0, self.log.getvalue())
        self.assertTrue(stages[0].is_dir())

    def test_large_bundle_with_bounded_file_descriptors(self):
        for i in range(600):
            self.write(self.bundle / "data/flutter_assets" / ("asset-" + str(i)), b"asset", 0o644)
        result = subprocess.run(
            [sys.executable, str(INSTALLER_PATH), "--client", str(self.bundle), "--yes"],
            env=dict(os.environ, **self.env, PYTHONDONTWRITEBYTECODE="1"),
            preexec_fn=lambda: resource.setrlimit(resource.RLIMIT_NOFILE, (128, 128)),
            capture_output=True, timeout=30, check=False)
        self.assertEqual(result.returncode, 0, result.stdout)

    def test_simultaneous_cli_installers_have_safe_outcomes(self):
        command = [sys.executable, str(INSTALLER_PATH), "--client", str(self.bundle),
                   "--service", str(self.service), "--yes"]
        env = dict(os.environ, **self.env, PYTHONDONTWRITEBYTECODE="1")
        children = [subprocess.Popen(command, env=env, stdout=subprocess.PIPE,
                                     stderr=subprocess.PIPE) for _ in range(2)]
        try:
            for child in children:
                child.communicate(timeout=15)
                self.assertIn(child.returncode, (0, 1))
            self.assertIn(0, [child.returncode for child in children])
            self.assertEqual(self.run_install(yes=True), 0, self.log.getvalue())
        finally:
            for child in children:
                if child.poll() is None:
                    child.kill()
                    child.communicate(timeout=5)

    def test_cli_help_required_arguments_and_dry_run(self):
        command = [sys.executable, str(INSTALLER_PATH)]
        env = dict(os.environ, **self.env, PYTHONDONTWRITEBYTECODE="1")
        result = subprocess.run(command + ["--help"], env=env, capture_output=True, timeout=5, check=False)
        self.assertEqual(result.returncode, 0)
        self.assertIn(b"--dry-run", result.stdout)
        self.assertEqual(subprocess.run(command, env=env, capture_output=True, timeout=5, check=False).returncode, 2)
        before = self.snapshot()
        result = subprocess.run(command + ["--client", str(self.bundle), "--service", str(self.service),
                                           "--dry-run", "--yes"], env=env, capture_output=True, timeout=5, check=False)
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual(self.snapshot(), before)


if __name__ == "__main__":
    unittest.main()
