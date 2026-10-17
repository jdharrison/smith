"""Isolated source/archive fixtures, not native execution or clean-source build evidence."""

import contextlib
import importlib.util
import io
import json
import os
import subprocess
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/prepare-installer-release.py"


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


release = load(SCRIPT, "installer_release")
# Reuse inert header/payload fixtures, never the native execution tests.
fixtures = load(ROOT / "dagger/tests/test_package.py", "release_package_fixtures")


class PackageLoaderTests(unittest.TestCase):
    def test_root_tooling_ignores_next_decoy(self):
        with tempfile.TemporaryDirectory(prefix="smith-release-loader-") as temporary:
            root = Path(temporary)
            (root / "scripts").mkdir()
            (root / "dagger").mkdir()
            (root / "dagger/runtime.py").write_text("MARKER = 'root runtime'\n")
            (root / "dagger/package.py").write_text("import runtime\nMARKER = runtime.MARKER\n")
            (root / "next/dagger").mkdir(parents=True)
            for name in ("runtime", "package"):
                (root / "next/dagger" / (name + ".py")).write_text(
                    "raise AssertionError('obsolete next tooling loaded')\n")
            with mock.patch.object(release, "__file__", str(root / "scripts/prepare-installer-release.py")):
                self.assertEqual(release.load_package().MARKER, "root runtime")

    def test_next_only_tooling_refused(self):
        with tempfile.TemporaryDirectory(prefix="smith-release-loader-") as temporary:
            root = Path(temporary)
            (root / "scripts").mkdir()
            (root / "next/dagger").mkdir(parents=True)
            for name in ("runtime", "package"):
                (root / "next/dagger" / (name + ".py")).write_text("MARKER = 'obsolete'\n")
            with mock.patch.object(release, "__file__", str(root / "scripts/prepare-installer-release.py")):
                with self.assertRaises(FileNotFoundError):
                    release.load_package()


class SourceFixture(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="smith-release-test-")
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.repo = self.base / "source"
        self.repo.mkdir()
        self.env = {name: value for name, value in os.environ.items()
                    if not name.startswith("GIT_")}
        self.env.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL="/dev/null",
                        GIT_AUTHOR_NAME="Fixture", GIT_AUTHOR_EMAIL="fixture@example.invalid",
                        GIT_COMMITTER_NAME="Fixture", GIT_COMMITTER_EMAIL="fixture@example.invalid",
                        GIT_EDITOR="true", GIT_TERMINAL_PROMPT="0")
        self.git("init", "--initial-branch=main")
        for component in ("installer", "service"):
            self.write(component + "/Cargo.toml",
                       '[package]\nname = "smith-' + component + '"\nversion = "0.1.0"\n')
            names = ("installer", "service") if component == "installer" else ("service",)
            self.write(component + "/Cargo.lock", "\n".join(
                '[[package]]\nname = "smith-' + name + '"\nversion = "0.1.0"\n'
                for name in names))
        self.write("client/pubspec.yaml", "name: smith_client\nversion: 0.1.0+42\n")
        self.commit = self.save()
        self.git("update-ref", "refs/remotes/origin/main", self.commit)
        self.tag = "release/v0.1.0"
        self.git("-c", "tag.gpgSign=false", "tag", "-a", self.tag, "-m", "fixture only")

    def git(self, *args, repo=None):
        return subprocess.run(["git", "--no-pager", "-C", str(repo or self.repo), *args],
                              env=self.env, check=True, capture_output=True, text=True,
                              timeout=15).stdout.strip()

    def write(self, name, content):
        path = self.repo / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)

    def save(self):
        self.git("add", ".")
        self.git("-c", "commit.gpgSign=false", "commit", "-m", "isolated fixture")
        return self.git("rev-parse", "HEAD")

    def identity(self, tag=None, commit=None):
        return release.source_identity(self.repo, tag or self.tag, commit or self.commit)


class SourceTests(SourceFixture):
    def test_valid_annotated_identity_and_cli(self):
        identity = self.identity()
        self.assertEqual(identity["components"]["client"], "0.1.0+42")
        self.assertEqual(identity["tag_object_sha"], self.git("rev-parse", "refs/tags/" + self.tag))
        result = subprocess.run([sys.executable, str(SCRIPT), "--repo", str(self.repo),
                                 "--tag", self.tag, "--commit", self.commit, "--check-only"],
                                capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "smith-installer_v0.1.0\n")
        self.assertFalse((self.repo / "build-record.json").exists())

    def test_root_versions_ignore_next_decoys(self):
        for path in self.repo.glob("*/Cargo.*"):
            self.write("next/" + str(path.relative_to(self.repo)),
                       path.read_text().replace("0.1.0", "9.9.9"))
        self.write("next/client/pubspec.yaml", "version: 9.9.9\n")
        self.commit = self.save()
        self.git("update-ref", "refs/remotes/origin/main", self.commit)
        self.git("-c", "tag.gpgSign=false", "tag", "-a", "-f", self.tag, "-m", "fixture")
        self.assertEqual(self.identity()["components"], {
            "installer": "0.1.0", "service": "0.1.0", "client": "0.1.0+42",
        })

    def test_next_only_layout_refused(self):
        (self.repo / "next").mkdir()
        for component in ("installer", "service", "client"):
            (self.repo / component).rename(self.repo / "next" / component)
        self.commit = self.save()
        self.git("update-ref", "refs/remotes/origin/main", self.commit)
        self.git("-c", "tag.gpgSign=false", "tag", "-a", "-f", self.tag, "-m", "fixture")
        with self.assertRaises(FileNotFoundError):
            self.identity()

    def test_bad_tags_and_branch_only(self):
        self.git("branch", "release/v1.0.0")
        for tag in ("v0.1.0", "release/v01.1.0", "release/v0.01.0", "release/v0.1.00",
                    "release/v0.1.0-rc.1", "release/v0.1.0+1", "release/v1.0.0", "--help"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                self.identity(tag=tag)

    def test_lightweight_tag(self):
        self.git("tag", "release/v1.0.0")
        with self.assertRaisesRegex(ValueError, "annotated"):
            self.identity(tag="release/v1.0.0")

    def test_checkout_commit_fallback_loses_annotation_but_explicit_tag_preserves_it(self):
        """Replay v4.2.2 fetch/ref semantics with real local Git, not a hosted action."""
        ref = "refs/tags/" + self.tag
        tag_object = self.git("rev-parse", ref)
        self.assertNotEqual(tag_object, self.commit)
        self.assertEqual(self.git("rev-parse", ref + "^{commit}"), self.commit)
        # Put the release in main history, rather than requiring it to be main's tip.
        self.write("later-main-change", "fixture main history\n")
        main_tip = self.save()
        remote = self.base / "remote.git"
        self.git("clone", "--bare", "--no-local", str(self.repo), str(remote))

        for explicit_tag in (False, True):
            with self.subTest(explicit_tag=explicit_tag):
                checkout = self.base / ("explicit-tag" if explicit_tag else "commit-fallback")
                checkout.mkdir()
                self.git("init", "--initial-branch=main", repo=checkout)
                self.git("remote", "add", "origin", str(remote), repo=checkout)
                # checkout@v4.2.2's all-history refspec fetches annotated tags intact.
                self.git("fetch", "--prune", "--no-recurse-submodules", "origin",
                         "+refs/heads/*:refs/remotes/origin/*",
                         "+refs/tags/*:refs/tags/*", repo=checkout)
                self.assertEqual(self.git("cat-file", "-t", ref, repo=checkout), "tag")
                self.assertEqual(self.git("rev-parse", ref, repo=checkout), tag_object)
                self.assertEqual(self.git("rev-parse", "refs/remotes/origin/main",
                                          repo=checkout), main_tip)

                if not explicit_tag:
                    # Default event checkout supplies both ref and event commit. The
                    # action's unpeeled tag comparison fails despite the correct commit.
                    self.assertNotEqual(self.git("rev-parse", ref, repo=checkout), self.commit)
                    self.assertEqual(self.git("rev-parse", ref + "^{commit}",
                                              repo=checkout), self.commit)
                    # Its targeted depth-one fallback force-fetches the COMMIT into
                    # the tag ref, replacing the annotation only in this disposable repo.
                    self.git("fetch", "--no-tags", "--prune", "--no-recurse-submodules",
                             "--depth=1", "origin", "+" + self.commit + ":" + ref,
                             repo=checkout)
                    self.assertEqual(self.git("cat-file", "-t", ref, repo=checkout), "commit")
                    self.assertEqual(self.git("rev-parse", ref, repo=checkout), self.commit)

                # Explicit full event-tag input has no supplied commit, so the
                # action skips that comparison/fallback and checks out the intact ref.
                self.git("checkout", "--progress", "--force", ref, repo=checkout)
                self.assertEqual(self.git("rev-parse", "HEAD", repo=checkout), self.commit)
                self.assertEqual(self.git("status", "--porcelain=v1", repo=checkout), "")
                if explicit_tag:
                    self.assertEqual(self.git("cat-file", "-t", ref, repo=checkout), "tag")
                    self.git("merge-base", "--is-ancestor", self.commit,
                             "refs/remotes/origin/main", repo=checkout)
                    identity = release.source_identity(checkout, self.tag, self.commit)
                    self.assertEqual(identity["source_sha"], self.commit)
                    self.assertEqual(identity["tag_object_sha"], tag_object)
                    self.assertEqual(identity["components"], {
                        "installer": "0.1.0", "service": "0.1.0", "client": "0.1.0+42",
                    })
                else:
                    with self.assertRaisesRegex(ValueError, "annotated"):
                        release.source_identity(checkout, self.tag, self.commit)

        # The source and bare remote annotations were never overwritten.
        self.assertEqual(self.git("rev-parse", ref), tag_object)
        self.assertEqual(self.git("rev-parse", ref, repo=remote), tag_object)

    def test_wrong_or_abbreviated_sha(self):
        for sha in ("0" * 40, self.commit[:12], self.commit.upper()):
            with self.subTest(sha=sha), self.assertRaises(ValueError):
                self.identity(commit=sha)

    def test_wrong_head(self):
        self.write("new", "new")
        self.save()
        with self.assertRaisesRegex(ValueError, "HEAD"):
            self.identity()

    def test_missing_main_ref(self):
        self.git("update-ref", "-d", "refs/remotes/origin/main")
        with self.assertRaises(ValueError):
            self.identity()

    def test_tagged_side_branch_not_in_main_history(self):
        self.git("checkout", "-b", "side")
        self.write("side", "side")
        self.commit = self.save()
        self.tag = "release/v0.1.1"
        self.git("-c", "tag.gpgSign=false", "tag", "-a", self.tag, "-m", "fixture")
        with self.assertRaises(ValueError):
            self.identity()

    def test_dirty_staged_and_unstaged(self):
        self.write("client/pubspec.yaml", "version: 0.1.0+2\n")
        with self.assertRaisesRegex(ValueError, "clean"):
            self.identity()
        self.git("add", ".")
        with self.assertRaisesRegex(ValueError, "clean"):
            self.identity()

    def test_untracked_allowed(self):
        self.write("untracked", "not source")
        self.identity()

    def check_committed_version_failure(self, name, content):
        self.write(name, content)
        self.commit = self.save()
        self.git("update-ref", "refs/remotes/origin/main", self.commit)
        self.git("-c", "tag.gpgSign=false", "tag", "-a", "-f", self.tag, "-m", "fixture")
        with self.assertRaises(ValueError):
            self.identity()

    def test_version_mismatch(self):
        self.check_committed_version_failure("installer/Cargo.toml",
                                            '[package]\nname="smith-installer"\nversion="0.2.0"\n')

    def test_strict_pubspec_and_lock_versions(self):
        # Modify tracked fixture content, then bind a new annotated tag to that content.
        for name, content in (
            ("client/pubspec.yaml", "version: 0.1.0 # comment\n"),
            ("client/pubspec.yaml", "version: 0.1.0\nversion: 0.1.0\n"),
            ("client/pubspec.yaml", "version: '0.1.0'\n"),
            ("client/pubspec.yaml", "version: 0.1.0+abc\n"),
            ("client/pubspec.yaml", "version: 0.2.0\n"),
            ("installer/Cargo.lock", '[[package]]\nname="smith-installer"\nversion="0.2.0"\n'),
            ("service/Cargo.lock", '[[package]]\nname="smith-service"\nversion="0.2.0"\n'),
            ("service/Cargo.toml", '[package]\nname="smith-service"\nversion="0.2.0"\n'),
        ):
            with self.subTest(content=content):
                original = (self.repo / name).read_text()
                self.write(name, content)
                sha = self.save()
                self.git("update-ref", "refs/remotes/origin/main", sha)
                tag = "release/v0.1.0"
                # A fresh fixture-only tag ref; real repository refs are never touched.
                self.git("-c", "tag.gpgSign=false", "tag", "-a", "-f", tag, "-m", "fixture")
                with self.assertRaises(ValueError):
                    release.source_identity(self.repo, tag, sha)
                self.write(name, original)


class ArchiveTests(SourceFixture):
    def payload(self):
        fixture = fixtures.PackageTests(methodName="test_complete_native_payload_and_checksums")
        with mock.patch.dict(os.environ, {"SMITH_TEST_INSTALLER": "", "SMITH_TEST_INSTALLER_UI": ""}):
            fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        with contextlib.redirect_stdout(io.StringIO()):
            fixture.assemble()
        return fixture.root

    def test_full_roundtrip_metadata_and_modes(self):
        root = self.payload()
        output = self.base / "output"
        stem = release.prepare(self.repo, self.tag, self.commit, root, output,
                               "https://example.invalid/run/1")
        self.assertEqual(stem, "smith-installer_v0.1.0")
        archive = output / (stem + ".tar.gz")
        record = json.loads((output / "build-record.json").read_text())
        self.assertEqual(record["archive_sha256"], release.digest(archive))
        self.assertEqual(record["package_manifest_sha256"], release.digest(root / "manifest.json"))
        self.assertEqual(record["run_url"], "https://example.invalid/run/1")
        self.assertEqual(record["self_check"]["native_execution"], "not_run")
        self.assertEqual((output / (archive.name + ".sha256")).read_text(),
                         release.digest(archive) + "  " + archive.name + "\n")
        extracted = release.extract_archive(archive, self.base / "extract", stem)
        self.assertEqual(release.snapshot(root, release.load_package()),
                         release.snapshot(extracted, release.load_package()))
        self.assertFalse((extracted / "build-record.json").exists())

    def test_cli_normal_default_repo_and_import_from_unrelated_cwd(self):
        root = self.payload()
        output = self.base / "cli-output"
        result = subprocess.run(
            [sys.executable, str(SCRIPT), "--tag", self.tag, "--commit", self.commit,
             "--package", str(root), "--output", str(output)],
            cwd=self.repo, capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "smith-installer_v0.1.0\n")
        self.assertTrue((output / "build-record.json").is_file())

    def test_invalid_source_and_nested_output_write_nothing(self):
        root = self.payload()
        output = self.base / "invalid-source-output"
        with self.assertRaises(ValueError):
            release.prepare(self.repo, "release/v00.1.0", self.commit, root, output)
        self.assertFalse(output.exists())
        nested = root / "output"
        with self.assertRaises(ValueError):
            release.prepare(self.repo, self.tag, self.commit, root, nested)
        self.assertFalse(nested.exists())

    def test_existing_output_and_dangling_symlink_refused(self):
        root = self.payload()
        for kind in ("directory", "file", "symlink"):
            output = self.base / kind
            if kind == "directory":
                output.mkdir()
            elif kind == "file":
                output.write_text("retain")
            else:
                output.symlink_to(self.base / "absent")
            with self.subTest(kind=kind), self.assertRaisesRegex(ValueError, "absent"):
                release.prepare(self.repo, self.tag, self.commit, root, output)
        self.assertEqual((self.base / "file").read_text(), "retain")

    def test_invalid_payloads_leave_no_output(self):
        for mutation in ("tamper", "missing", "mixed", "mode", "symlink", "hardlink"):
            root = self.payload()
            path = root / "payload/service/smith-service"
            if mutation == "tamper":
                (root / "README.txt").write_text("tampered")
            elif mutation == "missing":
                path.unlink()
            elif mutation == "mixed":
                path.write_bytes(path.read_bytes() + b"other version")
            elif mutation == "mode":
                path.chmod(0o777)
            elif mutation == "symlink":
                path.unlink()
                path.symlink_to(root / "smith-installer")
            else:
                os.link(path, self.base / "hardlink")
            output = self.base / ("output-" + mutation)
            with self.subTest(mutation=mutation), self.assertRaises((ValueError, OSError)):
                release.prepare(self.repo, self.tag, self.commit, root, output)
            self.assertFalse(output.exists())

    def malicious(self, names):
        archive = self.base / "malicious.tar.gz"
        with tarfile.open(archive, "w:gz") as handle:
            for name, kind, mode in names:
                member = tarfile.TarInfo(name)
                member.type = kind
                member.mode = mode
                if kind in (tarfile.SYMTYPE, tarfile.LNKTYPE):
                    member.linkname = "outside"
                handle.addfile(member)
        return archive

    def test_archive_unsafe_paths_links_modes_and_duplicates(self):
        stem = "smith-installer_v0.1.0"
        root = (stem, tarfile.DIRTYPE, 0o755)
        for name, kind, mode in (
            ("../escape", tarfile.REGTYPE, 0o644),
            ("/absolute", tarfile.REGTYPE, 0o644),
            (stem + "/../escape", tarfile.REGTYPE, 0o644),
            (stem + "//file", tarfile.REGTYPE, 0o644),
            (stem + "/./file", tarfile.REGTYPE, 0o644),
            (stem + "/bad\\file", tarfile.REGTYPE, 0o644),
            (stem + "/file", tarfile.SYMTYPE, 0o644),
            (stem + "/file", tarfile.LNKTYPE, 0o644),
            (stem + "/file", tarfile.FIFOTYPE, 0o644),
            (stem + "/file", tarfile.REGTYPE, 0o4755),
            (stem + "/file", tarfile.REGTYPE, 0o666),
            (stem + "/missing/file", tarfile.REGTYPE, 0o644),
            root,
        ):
            archive = self.malicious([root, (name, kind, mode)])
            destination = self.base / "unsafe-extract"
            with self.subTest(name=name, kind=kind, mode=mode), self.assertRaises(ValueError):
                release.extract_archive(archive, destination, stem)
            self.assertFalse(destination.exists())

    def test_archive_tamper_missing_and_mode_fail_reverification(self):
        root = self.payload()
        package = release.load_package()
        stem = "smith-installer_v0.1.0"
        original = self.base / "original.tar.gz"
        release.create_archive(root, original, stem, package)
        for mutation in ("tamper", "missing", "mode"):
            altered = self.base / (mutation + ".tar.gz")
            with tarfile.open(original, "r:gz") as source, tarfile.open(altered, "w:gz") as target:
                for member in source:
                    data = source.extractfile(member).read() if member.isfile() else None
                    if member.name == stem + "/README.txt":
                        if mutation == "missing":
                            continue
                        if mutation == "tamper":
                            data = b"changed"
                            member.size = len(data)
                        else:
                            member.mode = 0o600
                    target.addfile(member, io.BytesIO(data) if data is not None else None)
            extracted = release.extract_archive(altered, self.base / mutation, stem)
            with self.subTest(mutation=mutation), self.assertRaises((ValueError, OSError)):
                package.verify(extracted)

    @unittest.skipUnless(os.environ.get("SMITH_TEST_PACKAGE"), "SMITH_TEST_PACKAGE not supplied")
    def test_actual_package_roundtrip_with_synthetic_source_identity(self):
        # Real artifact integrity only: the temporary Git fixture did not build this package.
        root = Path(os.environ["SMITH_TEST_PACKAGE"])
        output = self.base / "actual-output"
        release.prepare(self.repo, self.tag, self.commit, root, output)
        record = json.loads((output / "build-record.json").read_text())
        self.assertEqual(record["self_check"]["package_source_binding"],
                         "supplied_package_not_proven_by_this_helper")
        print("Actual package fixture roundtrip (NOT clean-source provenance): "
              + record["archive_sha256"])


if __name__ == "__main__":
    unittest.main()
