"""Offline Make export gates: real location helper, fake exporter/GUI/publisher.

Only the package-content phase of --verify is stubbed; its location gate is real.
No Dagger engine, native product executable, or host service is used.
"""

import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
EXPORTS = {
    "installer/build": ("installer/dist", "build-installer"),
    "installer/ui": ("installer/build", "build-installer-ui"),
    "installer/binary": ("installer/build", "build-installer-binary"),
    "client/build": ("client/dist", "build-client"),
    "service/build": ("service/dist", "build-service"),
}


class BuildLocationMakeTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory(prefix="smith-make-locations-")
        self.addCleanup(temp.cleanup)
        self.base = Path(temp.name)
        self.repo = self.base / "repo"
        self.repo.mkdir(mode=0o700)
        for name in ("dagger", "installer", "client", "service", "bin"):
            (self.repo / name).mkdir(mode=0o700)
        shutil.copyfile(ROOT / "Makefile", self.repo / "Makefile")
        for name in ("package.py", "runtime.py"):
            shutil.copyfile(ROOT / "dagger" / name, self.repo / "dagger" / name)
        self.log = self.repo / "events.jsonl"
        self.env = {
            "PATH": str(self.repo / "bin") + os.pathsep + os.defpath,
            "LC_ALL": "C", "HOME": str(self.base),
            "EVENTS": str(self.log), "REAL_PYTHON": sys.executable,
        }
        self.fake("python3", '''import json, os, pathlib, runpy, subprocess, sys
args = sys.argv[1:]
with open(os.environ["EVENTS"], "a") as log:
    log.write(json.dumps(["python", args]) + "\\n")
if "--verify" not in args:
    sys.exit(subprocess.run([os.environ["REAL_PYTHON"], *args], timeout=10).returncode)
# Deliberately skip payload contents, never the real location validation.
sys.path.insert(0, str(pathlib.Path("dagger").absolute()))
helper = runpy.run_path("dagger/package.py", run_name="location_fixture")
root = pathlib.Path(args[args.index("--root") + 1])
try:
    helper["check_location"](root)
except (ValueError, OSError) as error:
    print(error, file=sys.stderr)
    sys.exit(1)
with open(os.environ["EVENTS"], "a") as log:
    log.write(json.dumps(["content-verified", str(root)]) + "\\n")
if os.environ.get("PRELAUNCH_DRIFT"):
    root.parent.chmod(0o775)
''')
        self.fake("fake-dagger", '''import json, os, pathlib, sys
args = sys.argv[1:]
with open(os.environ["EVENTS"], "a") as log:
    log.write(json.dumps(["dagger", args]) + "\\n")
output = pathlib.Path(args[args.index("-o") + 1])
command = next(arg for arg in args if arg.startswith("build-"))
leaf = output if command in ("build-installer", "build-client") else output.parent
leaf.mkdir(exist_ok=True)
# The probe's mode records the recipe's actual umask, not an explicit chmod.
(leaf / "export-probe").mkdir()
def sentinel(path, event):
    path.write_text("#!" + os.environ["REAL_PYTHON"] + " -B\\n"
                    "import json, os\\n"
                    "with open(os.environ['EVENTS'], 'a') as log:\\n"
                    "    log.write(json.dumps([" + repr(event) + "]) + '\\\\n')\\n")
    path.chmod(0o700)
if command == "build-installer":
    sentinel(leaf / "smith-installer-ui", "gui")
elif command == "build-installer-binary":
    sentinel(output, "publisher")
elif command in ("build-service", "build-installer-ui"):
    sentinel(output, "component")
if os.environ.get("POSTEXPORT_DRIFT") == command:
    leaf.chmod(0o775)
''')

    def fake(self, name, code):
        path = self.repo / "bin" / name
        path.write_text(f"#!{sys.executable} -B\n" + code)
        path.chmod(0o700)

    def make(self, *targets, jobs=1, **environment):
        return subprocess.run(
            ["make", "--no-print-directory", f"-j{jobs}",
             "DAGGER=" + str(self.repo / "bin/fake-dagger"), *targets],
            cwd=self.repo, env=dict(self.env, **environment), umask=0o002,
            capture_output=True, text=True, timeout=25,
        )

    def events(self):
        return [json.loads(line) for line in self.log.read_text().splitlines()] if self.log.exists() else []

    def assert_blocked(self, result):
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("Build location blocked", result.stdout + result.stderr)
        self.assertNotIn("Traceback", result.stderr)

    def test_all_five_exports_refuse_unsafe_source_before_own_dagger(self):
        for target, (output, _) in EXPORTS.items():
            with self.subTest(target=target):
                source = self.repo / output.split("/")[0]
                leaf = self.repo / output
                leaf.mkdir(mode=0o700, exist_ok=True)
                (leaf / "keep").write_bytes(b"unchanged")
                source.chmod(0o775)
                before = {str(p.relative_to(source)): (p.stat().st_mode,
                          p.read_bytes() if p.is_file() else None) for p in source.rglob("*")}
                result = self.make(target)
                self.assert_blocked(result)
                self.assertFalse(any(event[0] in ("dagger", "gui", "publisher")
                                     for event in self.events()))
                self.assertEqual(stat.S_IMODE(source.stat().st_mode), 0o775)
                self.assertEqual(before, {str(p.relative_to(source)): (p.stat().st_mode,
                                 p.read_bytes() if p.is_file() else None) for p in source.rglob("*")})
                source.chmod(0o700)
                shutil.rmtree(leaf)
                self.log.unlink()

    def test_all_five_fresh_exports_are_private_under_umask_002_and_ordered(self):
        for target, (output, command) in EXPORTS.items():
            with self.subTest(target=target):
                leaf = self.repo / output
                result = self.make(target)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual(stat.S_IMODE(leaf.stat().st_mode), 0o700)
                self.assertEqual(stat.S_IMODE((leaf / "export-probe").stat().st_mode), 0o700)
                events = self.events()
                self.assertEqual([event[0] for event in events[:3]], ["python", "dagger", "python"])
                self.assertEqual(events[0][1], ["-B", "dagger/package.py", "--root", output,
                                               "--prepare-location"])
                self.assertIn(command, events[1][1])
                self.assertEqual(events[1][1][:3], ["--mod", "dagger", "call"])
                self.assertEqual(events[2][1], ["-B", "dagger/package.py", "--root", output,
                                               "--verify" if target == "installer/build"
                                               else "--check-location"])
                self.assertFalse(list((self.repo / "dagger").rglob("*.pyc")))
                shutil.rmtree(leaf)
                self.log.unlink()

    def test_unsafe_installer_run_never_exports_or_launches_gui(self):
        source = self.repo / "installer"
        source.chmod(0o775)
        result = self.make("installer/run")
        self.assert_blocked(result)
        self.assertEqual([event[0] for event in self.events()], ["python"])
        self.assertFalse((source / "dist").exists())
        self.assertEqual(stat.S_IMODE(source.stat().st_mode), 0o775)

    def test_postexport_drift_blocks_all_five_targets(self):
        for target, (output, command) in EXPORTS.items():
            with self.subTest(target=target):
                result = self.make(target, POSTEXPORT_DRIFT=command)
                self.assert_blocked(result)
                self.assertEqual(sum(event[0] == "dagger" for event in self.events()), 1)
                self.assertFalse(any(event[0] in ("gui", "publisher", "content-verified")
                                     for event in self.events()))
                self.assertEqual(stat.S_IMODE((self.repo / output).stat().st_mode), 0o775)
                shutil.rmtree(self.repo / output)
                self.log.unlink()

    def test_postexport_drift_stops_gui_before_content_verification(self):
        result = self.make("installer/run", POSTEXPORT_DRIFT="build-installer")
        self.assert_blocked(result)
        self.assertEqual([event[0] for event in self.events()], ["python", "dagger", "python"])

    def test_prelaunch_drift_is_rechecked_after_successful_content_phase(self):
        result = self.make("installer/run", PRELAUNCH_DRIFT="1")
        self.assert_blocked(result)
        events = self.events()
        self.assertEqual([event[0] for event in events],
                         ["python", "dagger", "python", "content-verified", "python"])
        self.assertEqual(events[-1][1][-1], "--check-location")
        self.assertFalse(any(event[0] == "gui" for event in events))

    def test_safe_run_launches_only_stub_gui_after_final_location_check(self):
        result = self.make("installer/run")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        events = self.events()
        self.assertEqual([event[0] for event in events],
                         ["python", "dagger", "python", "content-verified", "python", "gui"])
        self.assertEqual(events[-2][1][-1], "--check-location")

    def test_safe_parallel_install_runs_only_stub_publisher_after_all_checks(self):
        result = self.make("install", jobs=3)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        events = self.events()
        self.assertEqual(events[-1], ["publisher"])
        self.assertEqual(sum(event[0] == "dagger" for event in events), 3)
        checks = [event[1] for event in events
                  if event[0] == "python" and event[1][-1] == "--check-location"]
        self.assertEqual({args[args.index("--root") + 1] for args in checks},
                         {"service/dist", "client/dist", "installer/build"})
        self.assertEqual(len(checks), 3)
        self.assertFalse(any(event[0] == "gui" for event in events))

    def test_parallel_install_failed_prerequisite_blocks_publisher_not_unrelated_builds(self):
        (self.repo / "client").chmod(0o775)
        result = self.make("install", jobs=3)
        self.assert_blocked(result)
        events = self.events()
        self.assertFalse(any(event[0] in ("publisher", "gui") for event in events))
        calls = [event[1] for event in events if event[0] == "dagger"]
        self.assertFalse(any("build-client" in call for call in calls))
        # Safe parallel prerequisites are permitted to finish, not globally cancelled.
        self.assertTrue(any("build-service" in call for call in calls))
        self.assertTrue(any("build-installer-binary" in call for call in calls))


if __name__ == "__main__":
    unittest.main()
