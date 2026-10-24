"""Offline only: injected Docker/CLI fakes; subprocess/network effects forbidden."""
import copy
import hashlib
import importlib.util
import os
from pathlib import Path
import shutil
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
SOURCE = Path(__file__).with_name("live-transport-continue.py")
SPEC = importlib.util.spec_from_file_location("continuation", SOURCE)
C = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(C)
M = C.M
REAL_WORK = M.Inputs().work


def snapshot(path):
    return {p.relative_to(path).as_posix(): (M.sha(p.read_bytes()), p.stat().st_mtime_ns)
            for p in path.rglob("*") if p.is_file()}


class Inputs:
    def __init__(self, work):
        self.work, self.cli, self.module = work, work / "dagger", work / "module"
        self.drift = False

    def unchanged(self, _binding):
        M.need(not self.drift)


class Commands:
    unknown = False

    def __init__(self, supervisor):
        self.s, self.calls = supervisor, []
        self.value = copy.deepcopy(supervisor.baseline)
        self.volume_foreign = False
        self.failure = None
        self.joined = False
        self.after_start = lambda: None

    def __call__(self, argv, deadline, cleanup=False, cap=1024 * 1024):
        self.calls.append(argv)
        M.need(deadline <= self.s.end and (cleanup or deadline <= self.s.work_end))
        M.need((self.s.work / "continuation-consumed.json").exists())
        M.need((self.s.work / "continuation-pending.json").exists())
        if argv[0] != "/usr/bin/docker":
            self.joined = True
            self.assert_cli(argv)
            if self.failure == "capture":
                raise M.Refuse()
            if self.failure == "cancel":
                self.s.cancelled = True
                raise M.Refuse()
            raw = M.encoded({"schema_version": 1, "request_sha256": self.s.bound["original"]["request"],
                             "outcome": "failed", "error": "policy_denied"})
            return (1 if self.failure == "exit" else 0), raw, b"never print private diagnostics"
        M.need(argv[:5] == ["/usr/bin/docker", "--host", M.SCOPE["endpoint"],
                            "--config", str(self.s.work / "docker")])
        args = argv[5:]
        if args[:2] == ["container", "inspect"]:
            M.need(args[2:] == [C.CID])
            raw = M.encoded([self.value])
        elif args == ["volume", "inspect", M.VOLUME]:
            raw = M.encoded([{"Name": M.VOLUME, "Driver": "local", "Options": None,
                              "Labels": {M.LABEL: "foreign" if self.volume_foreign else self.s.nonce}}])
        elif args == ["start", C.CID]:
            self.value["State"].update(Running=True, Pid=123, Status="running", StartedAt="started")
            self.after_start()
            if self.failure == "start":
                raise M.Refuse()
            raw = C.CID.encode()
        elif args == ["stop", "--time", "10", C.CID]:
            M.need(not any("run-fixture" in c for c in self.calls) or self.joined)
            self.value["State"].update(Running=False, Pid=0, Status="exited")
            raw = C.CID.encode()
        else:
            raise AssertionError("forbidden command: pull/create/setup/replay is not available")
        return 0, raw, b""

    def assert_cli(self, argv):
        M.need(argv == [str(self.s.inputs.cli), "--mod", str(self.s.inputs.module),
                       "call", "run-fixture", "--request", str(self.s.work / "request.json")])
        M.need(self.s.env["_EXPERIMENTAL_DAGGER_RUNNER_HOST"] == "container+docker://" + C.CID)
        M.need(self.s.env["PATH"] == "/usr/bin:/bin")
        M.need(self.s.env["DOCKER_CONFIG"] == str(self.s.work / "docker"))
        M.need(cap_keys(self.s.env))


def cap_keys(env):
    return set(env) == {"HOME", "XDG_CONFIG_HOME", "XDG_CACHE_HOME", "PATH", "DOCKER_HOST",
                        "DOCKER_CONFIG", "_EXPERIMENTAL_DAGGER_RUNNER_HOST"}


class ContinuationTests(unittest.TestCase):
    def setUp(self):
        self.real_before = snapshot(REAL_WORK)
        self.addCleanup(lambda: self.assertEqual(self.real_before, snapshot(REAL_WORK)))
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.work = Path(self.temp.name)
        for name in C.PINS:
            shutil.copy2(REAL_WORK / name, self.work / name)
        os.utime(self.work / "consumed.json", ns=(C.ANCHOR_NS, C.ANCHOR_NS))
        self.old_before = snapshot(self.work)
        baseline = M.decode(M.read(REAL_WORK / "capture-008.out"))[0]
        for mount in baseline["Mounts"]:
            if mount["Type"] == "bind":
                mount["Source"] = str(self.work / "engine.json")
        self.bound = {"original": M.decode(M.read(self.work / "consumed.json"))["binding"],
                      "script": M.sha(M.read(SOURCE)),
                      "tests": M.sha(M.read(Path(__file__))), "captures": {}}
        for name in ("capture-008.out", "capture-009.out", "capture-011.out"):
            (self.work / name).write_bytes(M.encoded([baseline]))
            self.bound["captures"][name] = M.sha(M.read(self.work / name))
        self.inputs = Inputs(self.work)
        self.now = C.ANCHOR_NS / 1e9 + 108
        self.clock = patch.object(C.time, "time", side_effect=lambda: self.now)
        self.clock.start()
        self.addCleanup(self.clock.stop)
        # Private fixture trees under /tmp are not operational input paths.
        self.safe = patch.object(M, "safe", return_value=None)
        self.safe.start()
        self.addCleanup(self.safe.stop)
        self.spawn = patch.object(M.subprocess, "Popen", side_effect=AssertionError("real subprocess forbidden"))
        self.spawn.start()
        self.addCleanup(self.spawn.stop)
        self.s = C.Continuation(self.inputs, self.bound, command=lambda *a, **k: None,
                                probe=lambda pid: Path("/fake/cgroup"), empty=lambda cg: True)
        self.fake = Commands(self.s)
        self.s.command = self.fake
        self.approval = lambda: None

    def run_fake(self):
        result = self.s.run(self.bound, self.approval)
        after = snapshot(self.work)
        self.assertTrue(all(after[name] == value for name, value in self.old_before.items()))
        return result

    def starts(self):
        return [c for c in self.fake.calls if c[0] == "/usr/bin/docker" and c[5] == "start"]

    def stops(self):
        return [c for c in self.fake.calls if c[0] == "/usr/bin/docker" and c[5] == "stop"]

    def test_normal_one_request_and_no_replay(self):
        self.assertEqual(self.run_fake(), {"status": "policy_denied", "cleanup": "verified_stopped"})
        self.assertEqual(len(self.starts()), 1)
        self.assertEqual(len(self.stops()), 1)
        self.assertEqual(sum("run-fixture" in c for c in self.fake.calls), 1)
        count = len(self.fake.calls)
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(len(self.fake.calls), count)

    def test_expired_and_future_budget(self):
        for now in (C.ANCHOR_NS / 1e9 - 1, C.ANCHOR_NS / 1e9 + 835,
                    C.ANCHOR_NS / 1e9 + 901):
            with self.subTest(now=now), self.assertRaises(M.Refuse):
                C.budget(self.work, now)
        self.assertAlmostEqual(self.s.end - time.monotonic(), 787, delta=1)

    def test_expired_after_construction_has_no_commands(self):
        self.now += 900
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertFalse(self.fake.calls)

    def test_expired_approval_refuses(self):
        self.now = M.decode(M.read(self.work / "approval.json"))["expires_at"] + 1
        with self.assertRaises(M.Refuse):
            C.authorize(self.inputs, self.bound, self.work / "fresh.json", "0" * 64)
        self.assertFalse(self.fake.calls)

    def test_source_drift_refuses_before_commands(self):
        self.inputs.drift = True
        self.run_fake()
        self.assertFalse(self.fake.calls)

    def test_receipt_failure_refuses_before_commands(self):
        def deny():
            raise M.Refuse()
        self.approval = deny
        self.run_fake()
        self.assertFalse(self.fake.calls)

    def test_foreign_identity_never_stopped(self):
        self.fake.value["Config"]["Labels"][M.LABEL] = "foreign"
        self.run_fake()
        self.assertFalse(self.starts() or self.stops())

    def test_foreign_volume_refused(self):
        self.fake.volume_foreign = True
        self.run_fake()
        self.assertFalse(self.starts() or self.stops())

    def test_created_pid_and_started_guards(self):
        for change in ({"StartedAt": "started"}, {"Running": True}, {"Pid": 1}, {"Status": "exited"}):
            value = copy.deepcopy(self.s.baseline)
            value["State"].update(change)
            with self.subTest(change=change), self.assertRaises(M.Refuse):
                C.created(value)

    def test_started_engine_refuses_start_and_request(self):
        self.fake.value["State"]["StartedAt"] = "started"
        self.run_fake()
        self.assertFalse(self.starts() or self.stops())
        self.assertFalse(any("run-fixture" in c for c in self.fake.calls))

    def test_prior_markers_refuse(self):
        for name in ("dispatch.json", "running.json", "continuation-dispatch.json", "continuation-lock"):
            with self.subTest(name=name):
                path = self.work / name
                path.write_bytes(b"{}")
                self.run_fake()
                self.assertFalse(self.fake.calls)
                path.unlink()

    def test_only_explicit_bridge_accepted(self):
        for network in ("default", "host", "none", "foreign"):
            value = copy.deepcopy(self.s.baseline)
            value["HostConfig"]["NetworkMode"] = network
            with self.subTest(network=network), self.assertRaises(M.Refuse):
                self.s.execution_config(value)
        self.s.execution_config(self.s.baseline)

    def test_failure_always_stops_after_join(self):
        self.fake.failure = "capture"
        self.assertEqual(self.run_fake(), {"status": "failed", "cleanup": "verified_stopped"})
        self.assertTrue(self.fake.joined)
        self.assertEqual(len(self.stops()), 1)

    def test_ambiguous_start_still_stops(self):
        self.fake.failure = "start"
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(len(self.stops()), 1)
        self.assertFalse(any("run-fixture" in c for c in self.fake.calls))

    def test_drift_after_start_still_stops(self):
        self.fake.after_start = lambda: self.fake.value["HostConfig"].update(PidsLimit=0)
        self.assertEqual(self.run_fake(), {"status": "failed", "cleanup": "unknown"})
        self.assertEqual(len(self.stops()), 1)

    def test_foreign_after_start_never_stopped(self):
        self.fake.after_start = lambda: self.fake.value["Config"]["Labels"].update({M.LABEL: "foreign"})
        self.assertEqual(self.run_fake()["cleanup"], "unknown")
        self.assertFalse(self.stops())

    def test_cancel_stops(self):
        self.fake.failure = "cancel"
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(len(self.stops()), 1)

    def test_source_drift_after_start_blocks_dispatch_not_stop(self):
        self.fake.after_start = lambda: setattr(self.inputs, "drift", True)
        self.run_fake()
        self.assertEqual(len(self.stops()), 1)
        self.assertFalse(any("run-fixture" in c for c in self.fake.calls))

    def test_unknown_child_group_still_stops_but_cannot_pass(self):
        self.fake.unknown = True
        self.run_fake()
        # The initial inspect refuses unknown capture state before start.
        self.assertFalse(self.starts())

    def test_unknown_after_start_still_attempts_stop(self):
        self.fake.after_start = lambda: setattr(self.fake, "unknown", True)
        self.assertEqual(self.run_fake()["cleanup"], "unknown")
        self.assertEqual(len(self.stops()), 1)

    def test_nonempty_cgroup_blocks_cleanup_proof(self):
        self.s.empty = lambda cg: False
        self.assertEqual(self.run_fake()["cleanup"], "unknown")
        self.assertEqual(len(self.stops()), 1)

    def test_nonzero_cli_not_policy_denied_success(self):
        self.fake.failure = "exit"
        self.assertEqual(self.run_fake(), {"status": "failed", "cleanup": "verified_stopped"})
        self.assertEqual(len(self.stops()), 1)

    def test_deadline_after_start_blocks_request_but_stops(self):
        self.fake.after_start = lambda: setattr(self.s, "work_end", time.monotonic() - 1)
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(len(self.stops()), 1)
        self.assertFalse(any("run-fixture" in c for c in self.fake.calls))

    def test_original_marker_appearing_after_start_blocks_request(self):
        self.fake.after_start = lambda: (self.work / "dispatch.json").write_bytes(b"{}")
        self.run_fake()
        self.assertEqual(len(self.stops()), 1)
        self.assertFalse(any("run-fixture" in c for c in self.fake.calls))

    def test_anchor_mtime_reset_refused(self):
        os.utime(self.work / "consumed.json", ns=(C.ANCHOR_NS + 1, C.ANCHOR_NS + 1))
        with self.assertRaises(M.Refuse):
            C.budget(self.work, self.now)
        os.utime(self.work / "consumed.json", ns=(C.ANCHOR_NS, C.ANCHOR_NS))

    def test_fresh_receipt_must_be_independent_pass(self):
        old = M.decode(M.read(self.work / "approval.json"))
        value = {"binding": self.bound, "issued_at": self.now - 1, "expires_at": self.now + 60,
                 "role": "independent-review", "decision": "approved", "actor": "independent-parent-attested",
                 "independent": True, "verdict": "PASS"}
        path = self.work / "fresh.json"
        def verify():
            path.write_bytes(M.encoded(value))
            C.authorize(self.inputs, self.bound, path, M.sha(M.read(path)))
        verify()
        for change in ({"actor": "live-transport-continue-builder"}, {"actor": "live-transport-qa-builder"},
                       {"actor": old["actor"]}, {"independent": False}, {"verdict": "FAIL"},
                       {"binding": {}}, {"expires_at": self.now - 1}):
            saved = copy.deepcopy(value)
            value.update(change)
            with self.subTest(change=change), self.assertRaises(M.Refuse):
                verify()
            value.clear()
            value.update(saved)

    def test_private_new_capture_directory_and_original_runner(self):
        supervisor = C.Continuation(self.inputs, self.bound)
        self.assertIsInstance(supervisor.command, M.Commands)
        self.assertEqual(supervisor.command.work, self.work / "continuation-captures")
        self.assertIsNot(supervisor.command.work, self.work)
        self.assertEqual(supervisor.command.env, supervisor.env)


class LocalBindingTests(unittest.TestCase):
    def test_check_binding_readonly_no_docker(self):
        inputs = M.Inputs()
        before = snapshot(inputs.work)
        with patch.object(M.subprocess, "Popen", side_effect=AssertionError("no Docker")):
            bound = C.binding(inputs)
        self.assertEqual(snapshot(inputs.work), before)
        self.assertEqual(bound["original"]["script"], C.PIN)
        self.assertEqual(bound["original"]["tests"], C.TEST_PIN)
        self.assertEqual(bound["original_files"], C.PINS)
        self.assertEqual(bound["anchor_ns"], C.ANCHOR_NS)
        self.assertEqual(bound["script"], M.sha(M.read(SOURCE)))
        self.assertEqual(bound["tests"], M.sha(M.read(Path(__file__))))


if __name__ == "__main__":
    unittest.main(verbosity=2)
