"""Offline tooling regressions and static workflow contracts, not CI simulation.

Workflow assertions cover the repository's current block-style YAML only; actionlint
and actual GitHub execution remain separate validation gates.
"""

import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
VERSION = "v0.21.9"
DIGEST = "33eea0b08d6be444bada18e64b2216459100d6070abcb4b5345cee40c9fff982"


def block(text, key, indent):
    """Select one mapping block at a known indentation, refusing missing/duplicate keys."""
    lines = text.splitlines()
    starts = [i for i, line in enumerate(lines)
              if line == " " * indent + key + ":"]
    if len(starts) != 1:
        raise AssertionError(f"Expected exactly one {key!r} block at indent {indent}")
    start = starts[0] + 1
    end = start
    while end < len(lines):
        line = lines[end]
        if line.strip() and not line.lstrip().startswith("#"):
            if len(line) - len(line.lstrip()) <= indent:
                break
        end += 1
    return "\n".join(line for line in lines[start:end]
                     if not line.lstrip().startswith("#"))


def steps(job):
    return re.split(r"(?m)^      - ", block(job, "steps", 4))[1:]


class DaggerSetupTests(unittest.TestCase):
    def test_module_and_installer_have_reviewed_version_and_digest(self):
        module = json.loads((ROOT / "dagger/dagger.json").read_text())
        script = (ROOT / "scripts/install-dagger-ci.sh").read_text()
        self.assertEqual(module["engineVersion"], VERSION)
        self.assertEqual(re.findall(r"^version=(\S+)$", script, re.M), [VERSION[1:]])
        self.assertEqual(re.findall(r"^sha256=(\S+)$", script, re.M), [DIGEST])
        self.assertIn('printf \'%s  %s\\n\' "$sha256" "$archive" | '
                      'sha256sum --check --strict', script)

    def run_installer(self, system="Linux", machine="x86_64"):
        temp = tempfile.TemporaryDirectory(prefix="smith-pipeline-config-")
        self.addCleanup(temp.cleanup)
        base = Path(temp.name)
        tools = base / "bin"
        tools.mkdir()
        runner = base / "runner"
        runner.mkdir()
        path_file = base / "github-path"
        path_file.write_text("existing-path-entry\n")
        for name in ("mkdir", "sha256sum"):
            executable = shutil.which(name)
            self.assertIsNotNone(executable, f"Required Ubuntu tool missing: {name}")
            (tools / name).symlink_to(executable)

        def fake(name, code):
            path = tools / name
            path.write_text(f"#!{sys.executable}\n" + code)
            path.chmod(0o700)

        fake("uname", "import sys\n"
             f"print({system!r} if sys.argv[1] == '-s' else {machine!r})\n")
        executed = base / "dagger-executed"
        cli = (f"#!{sys.executable}\nimport pathlib\n"
               f"pathlib.Path({str(executed)!r}).touch()\n")
        fake("curl", "import json, pathlib, sys\n"
             f"pathlib.Path({str(base / 'curl-args')!r}).write_text(json.dumps(sys.argv[1:]))\n"
             "archive = pathlib.Path(sys.argv[sys.argv.index('--output') + 1])\n"
             "archive.write_bytes(b'not the reviewed Dagger archive\\n')\n"
             f"cli = {cli!r}\n"
             "dagger = archive.parent / 'dagger'\n"
             "dagger.write_text(cli)\n"
             "dagger.chmod(0o700)\n")
        fake("tar", "import pathlib\n"
             f"pathlib.Path({str(base / 'tar-executed')!r}).touch()\n")
        result = subprocess.run(
            ["/bin/sh", str(ROOT / "scripts/install-dagger-ci.sh")],
            env={"PATH": str(tools), "RUNNER_TEMP": str(runner),
                 "GITHUB_PATH": str(path_file), "LC_ALL": "C"},
            cwd=base, capture_output=True, text=True, timeout=10,
        )
        return base, result

    def test_bad_download_fails_checksum_before_extraction_execution_or_path_write(self):
        base, result = self.run_installer()
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("FAILED", result.stdout + result.stderr)
        self.assertIn("checksum", result.stderr)
        args = json.loads((base / "curl-args").read_text())
        for flag in ("--fail", "--silent", "--show-error", "--location"):
            self.assertIn(flag, args)
        for flag, value in (("--proto", "=https"), ("--proto-redir", "=https"),
                            ("--connect-timeout", "15"), ("--max-time", "180")):
            self.assertEqual(args[args.index(flag) + 1], value)
        self.assertEqual(args[-1],
                         "https://github.com/dagger/dagger/releases/download/"
                         f"{VERSION}/dagger_{VERSION}_linux_amd64.tar.gz")
        self.assertFalse((base / "tar-executed").exists())
        self.assertFalse((base / "dagger-executed").exists())
        self.assertEqual((base / "github-path").read_text(), "existing-path-entry\n")

    def test_wrong_platform_refuses_before_download(self):
        for system, machine in (("Unsupported", "x86_64"), ("Linux", "aarch64")):
            with self.subTest(system=system, machine=machine):
                base, result = self.run_installer(system, machine)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("supports Linux x86_64 only", result.stderr)
                self.assertFalse((base / "curl-args").exists())
                self.assertFalse((base / "tar-executed").exists())
                self.assertFalse((base / "dagger-executed").exists())
                self.assertEqual(list((base / "runner").iterdir()), [])
                self.assertEqual((base / "github-path").read_text(), "existing-path-entry\n")


class MakeConfigTests(unittest.TestCase):
    def check_dry_run(self, executable, override=()):
        source = (ROOT / "Makefile").read_text()
        targets = []
        current = None
        recipe_count = 0
        for line in source.splitlines():
            match = re.match(r"^([\w/-]+):", line)
            if match:
                current = match[1]
            if line.startswith("\t") and "$(DAGGER)" in line:
                self.assertIsNotNone(current)
                targets.append(current)
                recipe_count += 1
        self.assertGreater(recipe_count, 0)
        result = subprocess.run(
            ["make", "-C", str(ROOT), "-n", *override, *dict.fromkeys(targets)],
            env={"PATH": os.defpath, "LC_ALL": "C"},
            capture_output=True, text=True, timeout=10,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = [shlex.split(line) for line in result.stdout.splitlines()
                 if line.startswith(executable + " ")]
        self.assertEqual(len(calls), recipe_count, result.stdout)
        for call in calls:
            with self.subTest(call=call):
                self.assertEqual(call[:4], [executable, "--mod", "dagger", "call"])

    def test_every_dagger_recipe_has_explicit_module(self):
        self.check_dry_run("dagger")

    def test_custom_dagger_executable_preserves_module(self):
        self.check_dry_run("/nonexistent/smith-test-dagger",
                           ("DAGGER=/nonexistent/smith-test-dagger",))


class WorkflowContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.ci = (ROOT / ".github/workflows/ci.yml").read_text()
        cls.release = (ROOT / ".github/workflows/release.yml").read_text()

    def test_main_and_pr_validate_without_candidate_default(self):
        events = block(self.ci, "on", 0)
        for event in ("push", "pull_request"):
            self.assertEqual(block(events, event, 2).strip(), "branches: [main]")
        self.assertEqual(re.findall(r"^  ([\w_]+):", events, re.M),
                         ["push", "pull_request", "workflow_call"])
        candidate = block(block(block(events, "workflow_call", 2), "inputs", 4),
                          "release-candidate", 6)
        self.assertRegex(candidate, r"(?m)^        type: boolean$")
        self.assertRegex(candidate, r"(?m)^        default: false$")

    def test_release_is_tag_only_reusable_ci_caller(self):
        events = block(self.release, "on", 0)
        self.assertEqual(events.strip(), "push:\n    tags:\n      - 'release/*'")
        jobs = block(self.release, "jobs", 0)
        self.assertEqual(re.findall(r"^  ([\w-]+):", jobs, re.M), ["installer"])
        job = block(jobs, "installer", 2)
        self.assertIn("    uses: ./.github/workflows/ci.yml", job)
        self.assertEqual(block(job, "with", 4).strip(), "release-candidate: true")
        self.assertNotIn("steps:", job)

    def test_ubuntu_only_read_only_credential_free_pinned_actions(self):
        for workflow in (self.ci, self.release):
            with self.subTest(workflow=workflow.splitlines()[0]):
                self.assertEqual(block(workflow, "permissions", 0).strip(), "contents: read")
                self.assertNotRegex(workflow, r"(?im)\b(write|write-all|secrets|pull_request_target|gh-release)\b")
                self.assertNotRegex(workflow, r"(?im)\bgh\s+release\b|\b(git\s+push|sudo)\b")
                permissions = re.findall(r"(?m)^\s*permissions:", workflow)
                self.assertEqual(len(permissions), 1)
                for action in re.findall(r"(?m)^\s*(?:- )?uses:\s*(\S+)", workflow):
                    if action.startswith("./"):
                        self.assertEqual(action, "./.github/workflows/ci.yml")
                    else:
                        self.assertRegex(action, r"^[\w.-]+/[\w./-]+@[0-9a-f]{40}$")
        runners = re.findall(r"(?m)^\s*runs-on:\s*(.+)$", self.ci)
        self.assertEqual(runners, ["ubuntu-24.04", "ubuntu-24.04"])
        self.assertNotRegex(self.release, r"(?m)^\s*runs-on:")
        checkouts = [step for job in ("test", "installer")
                     for step in steps(block(block(self.ci, "jobs", 0), job, 2))
                     if "uses: actions/checkout@" in step]
        self.assertEqual(len(checkouts), 2)
        for checkout in checkouts:
            self.assertIn("persist-credentials: false", checkout)

    def test_both_checkouts_select_full_tag_ref_only_for_candidates(self):
        jobs = block(self.ci, "jobs", 0)
        for name in ("test", "installer"):
            with self.subTest(job=name):
                checkouts = [step for step in steps(block(jobs, name, 2))
                             if "uses: actions/checkout@" in step]
                self.assertEqual(len(checkouts), 1)
                inputs = block(checkouts[0], "with", 8)
                self.assertEqual(
                    re.findall(r"(?m)^          ref: (.+)$", inputs),
                    ["${{ inputs.release-candidate && github.ref || '' }}"],
                )
                self.assertRegex(inputs, r"(?m)^          fetch-depth: 0$")
                self.assertRegex(inputs, r"(?m)^          persist-credentials: false$")

    def test_candidate_upload_is_gated_after_checks_build_and_archive(self):
        jobs = block(self.ci, "jobs", 0)
        self.assertEqual(re.findall(r"^  ([\w-]+):", jobs, re.M), ["test", "installer"])
        test_job = block(jobs, "test", 2)
        installer = block(jobs, "installer", 2)
        self.assertRegex(installer, r"(?m)^    needs: test$")
        self.assertIn("python3 -m unittest discover -s scripts/tests", test_job)
        self.assertNotRegex(self.ci, r"\b(?:cargo|rustup)\s")
        self.assertNotIn("next/", self.ci)
        self.assertNotIn("make -C next", self.ci)
        self.assertIn("test \"$GITHUB_EVENT_NAME\" = push", test_job)
        self.assertIn("test \"$GITHUB_REF_TYPE\" = tag", test_job)
        identity = next(step for step in steps(test_job) if "id: identity" in step)
        self.assertIn("if: inputs.release-candidate", identity)
        self.assertIn("--check-only", identity)
        ordered = steps(installer)
        checks = ("sh scripts/install-dagger-ci.sh",
                  "make test installer/build",
                  "python3 dagger/package.py --root installer/dist --verify",
                  "python3 scripts/prepare-installer-release.py",
                  "uses: actions/upload-artifact@")
        positions = []
        for check in checks:
            matches = [i for i, step in enumerate(ordered) if check in step]
            self.assertEqual(len(matches), 1, check)
            positions.append(matches[0])
        self.assertEqual(positions, sorted(positions))
        self.assertIn("set -euo pipefail", ordered[positions[1]])
        archive = ordered[positions[3]]
        self.assertIn("if: inputs.release-candidate", archive)
        self.assertIn("set -euo pipefail", archive)
        self.assertIn("--package installer/dist --output target/installer-release", archive)
        upload = ordered[positions[4]]
        self.assertIn("if: inputs.release-candidate\n", upload)
        self.assertIn("name: ${{ needs.test.outputs.stem }}", upload)
        self.assertRegex(upload, r"(?m)^          path: target/installer-release/$")
        self.assertRegex(upload, r"(?m)^          retention-days: 14$")
        self.assertRegex(upload, r"(?m)^          if-no-files-found: error$")
        self.assertEqual((self.ci + self.release).count("uses: actions/upload-artifact@"), 1)
        self.assertNotRegex(self.ci + self.release,
                            r"(?im)continue-on-error:|always\(\)|if:\s*failure\(\)")
        self.assertNotRegex(upload, r"(?m)^\s*path:.*\bdist\b")


if __name__ == "__main__":
    unittest.main()
