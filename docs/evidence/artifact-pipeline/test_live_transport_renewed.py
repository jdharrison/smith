"""Offline only. Synthetic private fixtures; no operational state or subprocesses."""
import copy
import importlib.util
import io
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location("renewed_tests_subject", Path(__file__).with_name("live-transport-renewed.py"))
R = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(R)
M = R.M
RUN_ID = "20261021020000000000000000000002"


def write(path, value):
    M.durable(path, value)


def snapshot(path):
    return {p.relative_to(path).as_posix(): (M.sha(p.read_bytes()), p.stat().st_mtime_ns)
            for p in path.rglob("*") if p.is_file()}


def baseline(old):
    return {"Id": R.CID, "Image": R.IMAGE, "Name": "/" + M.NAME,
            "Config": {"Labels": {M.LABEL: "owned-nonce"}},
            "HostConfig": {"NetworkMode": "bridge", "CgroupParent": "", "Privileged": True,
                           "CgroupnsMode": "private", "NanoCpus": 4000000000, "Memory": 4294967296,
                           "MemorySwap": 4294967296, "PidsLimit": 1024, "PortBindings": {}, "Binds": None,
                           "RestartPolicy": {"Name": "no"}},
            "Mounts": [{"Type": "volume", "Destination": "/var/lib/dagger", "Name": M.VOLUME, "RW": True},
                       {"Type": "bind", "Destination": "/etc/dagger/engine.json",
                        "Source": str(old / "engine.json"), "RW": False}],
            "Path": "/entrypoint.sh", "Args": [],
            "State": {"Status": "exited", "Running": False, "Pid": 0}}


class Held:
    def __init__(self, identity=(7, 42)):
        self.identity = identity
        self.closed = False
        self.calls = []
        self.quiet = True
        self.error = None
        self.after = lambda: None

    def quiescent(self, **kwargs):
        self.calls.append(kwargs)
        assert kwargs["cancelled"]() is False
        self.after()
        if self.error:
            raise self.error
        return self.quiet

    def close(self):
        self.closed = True


class Commands:
    unknown = False

    def __init__(self, supervisor):
        self.s = supervisor
        self.calls = []
        self.value = copy.deepcopy(supervisor.baseline)
        self.after_start = lambda: None
        self.after_cli = lambda: None
        self.fail = None
        self.joined = False
        self.volume_foreign = False
        self.rc = 0
        self.envelope = None

    def __call__(self, argv, deadline, cleanup=False, cap=1024 * 1024):
        self.calls.append((argv, cleanup))
        assert deadline <= self.s.end
        assert cleanup or deadline <= self.s.work_end
        assert (self.s.work / "consumed.json").is_file()
        assert (self.s.work / "pending.json").is_file()
        if argv[0] != "/usr/bin/docker":
            assert argv == [str(self.s.inputs.cli), "--mod", str(self.s.inputs.module), "call", "run-fixture",
                            "--request", str(self.s.work / "request.json")]
            assert cap == 65536
            assert self.s.env["_EXPERIMENTAL_DAGGER_RUNNER_HOST"] == "container+docker://" + R.CID
            assert set(self.s.env) == {"HOME", "XDG_CONFIG_HOME", "XDG_CACHE_HOME", "PATH", "DOCKER_HOST",
                                       "DOCKER_CONFIG", "_EXPERIMENTAL_DAGGER_RUNNER_HOST"}
            assert self.s.env["HOME"] == str(self.s.work / "home")
            assert self.s.env["PATH"] == "/usr/bin:/bin"
            self.joined = True  # Fake the synchronous Commands join/finally contract.
            self.after_cli()
            if self.fail == "cli":
                raise RuntimeError("private diagnostics must never appear")
            raw = self.envelope if self.envelope is not None else M.encoded({
                "schema_version": 1, "request_sha256": self.s.bound["request"],
                "outcome": "failed", "error": "policy_denied"})
            return self.rc, raw, b"private stderr"
        assert argv[:5] == ["/usr/bin/docker", "--host", M.SCOPE["endpoint"], "--config", str(self.s.work / "docker")]
        args = argv[5:]
        if args == ["container", "inspect", R.CID]:
            if cleanup and self.fail == "inspect_cleanup":
                raise RuntimeError("unavailable")
            return 0, M.encoded([self.value]), b""
        if args == ["volume", "inspect", M.VOLUME]:
            return 0, M.encoded([{"Name": M.VOLUME, "Driver": "local", "Options": None,
                                   "Labels": {M.LABEL: "foreign" if self.volume_foreign else self.s.nonce}}]), b""
        if args == ["start", R.CID]:
            self.value["State"].update(Running=True, Status="running", Pid=123)
            self.after_start()
            if self.fail == "start":
                raise RuntimeError("ambiguous start")
            return 0, R.CID.encode(), b""
        if args == ["stop", "--time", "10", R.CID]:
            assert cleanup
            assert not any("run-fixture" in call for call, _ in self.calls) or self.joined
            if self.fail == "stop":
                raise RuntimeError("ambiguous stop")
            self.value["State"].update(Running=False, Status="exited", Pid=0)
            return 0, R.CID.encode(), b""
        raise AssertionError("forbidden command")

    def count(self, verb):
        return sum((a[0] == "/usr/bin/docker" and a[5] == verb) or
                   (verb == "cli" and a[0] != "/usr/bin/docker") for a, _ in self.calls)


class Fixtures(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.root.chmod(0o700)
        self.installed = self.root / "installed-dagger"
        self.installed.write_bytes(b"unchanged installed executable")
        self.installed.chmod(0o600)
        self.inputs = R.Inputs(self.root, self.installed)
        self.inputs.base.mkdir(mode=0o700)
        self.inputs.old.mkdir(mode=0o700)
        self.inputs.module.mkdir(mode=0o700)
        write(self.inputs.module / "dagger.json", {"engineVersion": "v0.21.9"})
        self.inputs.cli.write_bytes(b"fake CLI bytes, never executable code")
        self.inputs.cli.chmod(0o700)
        fixture = self.inputs.source / "fixtures/clamp"
        fixture.mkdir(parents=True, mode=0o700)
        (fixture / "clamp.py").write_bytes(b"def clamp(x): return x\n")
        preflight = self.root / "docs/evidence/artifact-pipeline"
        preflight.mkdir(parents=True, mode=0o700)
        write(preflight / "functional-qa-readonly-preflight.json", {
            "installed_dagger": {"sha256": M.sha(self.installed.read_bytes())}})
        records = {"owned.json": {"cid": R.CID, "image": R.IMAGE, "nonce": "owned-nonce"},
                   "pending.json": {"nonce": "owned-nonce"}, "consumed.json": {"nonce": "owned-nonce"},
                   "result.json": {"status": "failed", "cleanup": "unknown"},
                   "continuation-result.json": {"status": "failed", "cleanup": "unknown"},
                   "approval.json": {"expires_at": 1}, "review.json": {"expires_at": 1},
                   "continuation-consumed.json": {"old": True},
                   "request.json": {"run_id": "0" * 32}, "engine.json": M.CONFIG,
                   "capture-008.out": [baseline(self.inputs.old)]}
        for name, value in records.items():
            write(self.inputs.old / name, value)
        for path in self.root.rglob("*"):
            path.chmod(0o700 if path.is_dir() or path == self.inputs.cli else 0o600)
        # Historical pins are replaced ONLY for synthetic fixture bytes. No real
        # run-01 path is opened or read by these tests, even for test setup.
        self.pin = patch.dict(R.C.PINS, {n: M.sha(M.read(self.inputs.old / n)) for n in R.C.PINS})
        self.pin.start()
        self.addCleanup(self.pin.stop)
        self.capture_pin = patch.object(R.C, "CAPTURE_PIN", M.sha(M.read(self.inputs.old / "capture-008.out")))
        self.capture_pin.start()
        self.addCleanup(self.capture_pin.stop)
        real_safe = M.safe
        def safe(path, private=False):
            path = Path(path)
            if path.is_relative_to(self.root):
                # /tmp is intentionally shared; fixture ancestry stops at the
                # private test root. Production safety checks remain unmodified.
                for item in (path, *path.parents):
                    meta = item.lstat()
                    M.need(not item.is_symlink() and not meta.st_mode & 0o022)
                    if private and item == path:
                        M.need(not meta.st_mode & 0o077)
                    if item == self.root:
                        break

            elif path.parent == R.HERE.parent:
                # Read-only source imports may live in a group-writable checkout.
                # Do not relax production ancestry checks or chmod the checkout.
                M.need(path.is_file() and not path.is_symlink())
            else:
                real_safe(path, private)
        self.safe = patch.object(M, "safe", side_effect=safe)
        self.safe.start()
        self.addCleanup(self.safe.stop)
        self.spawn = patch.object(M.subprocess, "Popen", side_effect=AssertionError("real process forbidden"))
        self.spawn.start()
        self.addCleanup(self.spawn.stop)
        self.old_snapshot = snapshot(self.inputs.old)
        self.addCleanup(lambda: self.assertEqual(self.old_snapshot, snapshot(self.inputs.old)))
        self.wall, self.mono = 2000000000.0, 1000.0
        self.clocks = [patch.object(R.time, "time", side_effect=lambda: self.wall),
                       patch.object(R.time, "monotonic", side_effect=lambda: self.mono)]
        for clock in self.clocks:
            clock.start()
            self.addCleanup(clock.stop)

    def staged(self):
        self.bound = R.stage(self.inputs, RUN_ID)
        return self.bound

    def supervisor(self):
        self.staged()
        self.held, self.fresh = Held(), Held()
        self.observations = []
        def capture(pid, cid, **kwargs):
            self.observations.append((pid, cid, kwargs))
            assert pid == 123 and cid == R.CID
            assert kwargs["deadline"] <= min(self.s.end, self.mono + 5)
            assert kwargs["run_deadline"] == self.s.end
            return self.held if len(self.observations) == 1 else self.fresh
        self.s = R.Renewed(self.inputs, self.bound, command=lambda *a, **k: None, capture=capture)
        self.commands = Commands(self.s)
        self.s.command = self.commands
        self.approval = lambda: None
        return self.s

    def run_fake(self):
        return self.s.run(self.bound, self.approval)


class StagingTests(Fixtures):
    def test_stage_check_file_only_binding_and_no_writes(self):
        bound = self.staged()
        before = snapshot(self.root)
        self.assertEqual(bound, R.check(self.inputs))
        self.assertEqual(before, snapshot(self.root))
        self.assertEqual(bound["original_files"], M.tree(self.inputs.old, True))
        self.assertEqual(bound["scope"]["previous_uncertainty"], "acknowledged_not_resolved_retroactively")
        self.assertEqual(bound["run_id"], RUN_ID)
        self.assertEqual(M.decode(M.read(self.inputs.work / "request.json"))["phase"], "execute")
        for name in ("home", "config", "cache"):
            self.assertEqual(list((self.inputs.work / name).iterdir()), [])
        self.assertEqual(M.decode(M.read(self.inputs.work / "docker/config.json")), {})
        self.assertIn("owned_cgroup_verifier.py", bound["code"])
        self.assertIn("test_live_transport_renewed.py", bound["code"])

    def test_unsafe_code_gate_precedes_workdir_creation(self):
        before = snapshot(self.root)
        fixture_safe = M.safe.side_effect
        def safe(path, private=False):
            if Path(path).parent == R.HERE.parent:
                raise M.Refuse()  # Production's unsafe docs/script ancestry refusal.
            fixture_safe(path, private)
        with patch.object(M, "safe", side_effect=safe):
            with self.assertRaises(M.Refuse):
                R.stage(self.inputs, RUN_ID)
        self.assertFalse(self.inputs.work.exists())
        self.assertEqual(before, snapshot(self.root))

    def test_invalid_prerequisites_do_not_consume_workdir(self):
        changes = ((self.inputs.source / "fixtures/clamp/clamp.py", bytes([255]), None),
                   (self.inputs.source, None, 0o775),
                   (self.inputs.module / "dagger.json", b'{"engineVersion":"wrong"}', None),
                   (self.inputs.cli, None, 0o600),
                   (self.installed, b"changed installed executable", None),
                   (self.inputs.old / "owned.json", b"{}", None))
        for path, replacement, mode in changes:
            raw = path.read_bytes() if path.is_file() else None
            meta = path.stat()
            before = snapshot(self.root)
            try:
                if replacement is not None:
                    path.write_bytes(replacement)
                if mode is not None:
                    path.chmod(mode)
                with self.subTest(path=path.name), self.assertRaises(Exception):
                    R.stage(self.inputs, RUN_ID)
                self.assertFalse(self.inputs.work.exists())
            finally:
                if raw is not None:
                    path.write_bytes(raw)
                path.chmod(meta.st_mode & 0o777)
                os.utime(path, ns=(meta.st_atime_ns, meta.st_mtime_ns))
            self.assertEqual(before, snapshot(self.root))

    def test_cli_explicit_root_resolves_checkout_not_moved_helper_root(self):
        before = snapshot(self.root)
        # CLI plumbing only: no staging, real checkout reads, or engine commands.
        with patch.object(sys, "argv", [str(R.HERE), "check", "--root", "."]), \
                patch.object(R, "Inputs", return_value=self.inputs) as inputs, \
                patch.object(R, "check", return_value={"fixture": True}) as check, \
                patch.object(M, "ROOT", self.root / "wrong-moved-helper-root"), \
                patch.object(R.os, "umask"), patch.object(sys, "stdout", new_callable=io.StringIO):
            self.assertEqual(R.main(), 0)
        inputs.assert_called_once_with(root=Path(".").resolve())
        check.assert_called_once_with(self.inputs)
        self.assertEqual(before, snapshot(self.root))

    def test_cli_requires_root_without_fallback(self):
        with patch.object(sys, "argv", [str(R.HERE), "check"]), \
                patch.object(R, "Inputs") as inputs, \
                patch.object(sys, "stderr", new_callable=io.StringIO):
            with self.assertRaises(SystemExit) as error:
                R.main()
        self.assertEqual(error.exception.code, 2)
        inputs.assert_not_called()
        self.assertFalse(self.inputs.work.exists())

    def test_stage_never_overwrites(self):
        self.staged()
        before = snapshot(self.root)
        with self.assertRaises(FileExistsError):
            R.stage(self.inputs, RUN_ID)
        self.assertEqual(before, snapshot(self.root))

    def test_old_or_invalid_request_id_refused(self):
        for value in (None, "x", "A" * 32, "0" * 32):
            with self.subTest(value=value), self.assertRaises(R.Refusal):
                R.stage(self.inputs, value)
        self.assertFalse(self.inputs.work.exists())

    def test_each_snapshot_drift_refuses(self):
        self.staged()
        paths = [self.inputs.cli, self.installed, self.inputs.source / "fixtures/clamp/clamp.py",
                 self.inputs.module / "dagger.json", self.inputs.work / "request.json",
                 self.inputs.work / "docker/config.json", self.inputs.work / "staged.json"]
        for path in paths:
            raw, meta = path.read_bytes(), path.stat()
            try:
                path.write_bytes(raw + b" ")
                with self.subTest(path=path.name), self.assertRaises(Exception):
                    self.inputs.unchanged(self.bound)
                if path.name == "staged.json":
                    # A different check hash requires new receipts even if only
                    # JSON whitespace changed; old authorization cannot bind it.
                    self.assertNotEqual(R.check(self.inputs), self.bound)
            finally:
                path.write_bytes(raw)
                os.utime(path, ns=(meta.st_atime_ns, meta.st_mtime_ns))

    def test_history_snapshot_detects_any_new_file(self):
        self.staged()
        extra = self.inputs.old / "unexpected"
        try:
            write(extra, {})
            with self.assertRaises(R.Refusal):
                R.check(self.inputs)
        finally:
            extra.unlink()  # Only a synthetic fixture, never operational history.

    def test_new_markers_fence_check_without_writes(self):
        self.staged()
        for name in (*R.MARKERS, "captures"):
            path = self.inputs.work / name
            write(path, {})
            before = snapshot(self.root)
            with self.subTest(name=name), self.assertRaises(R.Refusal):
                R.check(self.inputs)
            self.assertEqual(before, snapshot(self.root))
            path.unlink()

    def test_nonempty_home_and_docker_refused(self):
        self.staged()
        for name in ("home", "config", "cache", "docker"):
            path = self.inputs.work / name / "ambient"
            write(path, {})
            with self.subTest(name=name), self.assertRaises(R.Refusal):
                R.check(self.inputs)
            path.unlink()


class SupervisorTests(Fixtures):
    def setUp(self):
        super().setUp()
        self.supervisor()

    def test_success_new_deadline_records_and_no_replay(self):
        result = self.run_fake()
        self.assertEqual((result["status"], result["cleanup"]), ("policy_denied", "verified_stopped"))
        for verb in ("start", "stop", "cli"):
            self.assertEqual(self.commands.count(verb), 1)
        for name in R.MARKERS:
            self.assertTrue((self.inputs.work / name).is_file())
        anchor = M.decode(M.read(self.inputs.work / "consumed.json"))
        self.assertEqual(anchor["wall_end"] - anchor["wall_start"], 900)
        self.assertEqual(anchor["monotonic_end"] - anchor["monotonic_start"], 900)
        self.assertEqual(self.s.end - self.s.work_end, 60)
        self.assertEqual(len(self.observations), 2)
        self.assertEqual(len(self.held.calls), 1)
        self.assertTrue(self.held.closed and self.fresh.closed)
        before = snapshot(self.inputs.work)
        calls = len(self.commands.calls)
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(calls, len(self.commands.calls))
        self.assertEqual(before, snapshot(self.inputs.work))
        record = M.decode(M.read(self.inputs.work / "result.json"))
        self.assertEqual(record["previous_cleanup"], "unknown_not_retroactively_resolved")

    def test_construction_does_not_start_window(self):
        self.wall += 10000
        self.mono += 10000
        self.assertEqual(self.run_fake()["status"], "policy_denied")
        self.assertEqual(self.s.anchor["wall_start"], self.wall)

    def test_drift_before_effect_no_commands(self):
        self.inputs.cli.write_bytes(b"drift")
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(self.commands.calls, [])

    def test_authorization_failure_before_effect(self):
        self.approval = lambda: (_ for _ in ()).throw(R.Refusal("approval"))
        self.assertEqual(self.run_fake()["reason"], "approval")
        self.assertEqual(self.commands.calls, [])

    def test_foreign_engine_never_started_or_stopped(self):
        self.commands.value["Config"]["Labels"][M.LABEL] = "foreign"
        self.run_fake()
        self.assertEqual(self.commands.count("start") + self.commands.count("stop"), 0)

    def test_foreign_volume_never_started(self):
        self.commands.volume_foreign = True
        self.run_fake()
        self.assertEqual(self.commands.count("start") + self.commands.count("stop"), 0)

    def test_exact_exited_pid_zero_required(self):
        for change in ({"Status": "created"}, {"Status": "running"}, {"Running": True}, {"Pid": 1}, {"Pid": False}):
            value = copy.deepcopy(self.commands.value)
            value["State"].update(change)
            with self.subTest(change=change), self.assertRaises(R.Refusal):
                R.stopped(value)

    def test_config_contract_and_exact_baseline(self):
        for key, value in (("NetworkMode", "default"), ("NetworkMode", "host"), ("CgroupParent", "foreign"),
                           ("Memory", 0), ("MemorySwap", -1), ("PidsLimit", 0), ("NanoCpus", 0),
                           ("CgroupnsMode", "host"), ("Privileged", False), ("Binds", ["/host:/host"])):
            changed = copy.deepcopy(self.commands.value)
            changed["HostConfig"][key] = value
            with self.subTest(key=key, value=value), self.assertRaises(Exception):
                self.s.execution_config(changed)
        for key in ("Config", "HostConfig", "Mounts", "Path", "Args"):
            changed = copy.deepcopy(self.commands.value)
            if isinstance(changed[key], dict):
                changed[key]["extra"] = True
            elif isinstance(changed[key], list):
                changed[key].append("extra")
            else:
                changed[key] += "extra"
            with self.subTest(key=key), self.assertRaises(Exception):
                self.s.execution_config(changed)
        self.s.execution_config(self.commands.value)

    def test_ambiguous_start_always_stops_and_captures(self):
        self.commands.fail = "start"
        result = self.run_fake()
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["cleanup"], "verified_stopped")
        self.assertEqual(self.commands.count("stop"), 1)
        self.assertEqual(self.commands.count("cli"), 0)

    def test_cleanup_stop_independent_of_inspect_failure(self):
        self.commands.fail = "inspect_cleanup"
        self.assertEqual(self.run_fake()["cleanup"], "unknown")
        self.assertEqual(self.commands.count("stop"), 1)

    def test_resource_drift_still_stops_unknown(self):
        self.commands.after_cli = lambda: self.commands.value["HostConfig"].update(Memory=0)
        self.assertEqual(self.run_fake()["cleanup"], "unknown")
        self.assertEqual(self.commands.count("stop"), 1)

    def test_source_drift_after_start_still_stops_no_cli(self):
        self.commands.after_start = lambda: self.inputs.cli.write_bytes(b"drift")
        result = self.run_fake()
        self.assertEqual(result["status"], "failed")
        self.assertEqual(self.commands.count("stop"), 1)
        self.assertEqual(self.commands.count("cli"), 0)

    def test_approval_drift_after_start_still_stops(self):
        def approval():
            if self.commands.count("start"):
                raise R.Refusal("approval")
        self.approval = approval
        result = self.run_fake()
        self.assertEqual(result["reason"], "approval")
        self.assertEqual(self.commands.count("stop"), 1)
        self.assertEqual(self.commands.count("cli"), 0)

    def test_wall_expiry_preserves_cleanup_authority(self):
        self.commands.after_start = lambda: setattr(self, "wall", self.wall + 901)
        result = self.run_fake()
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["cleanup"], "unknown")
        self.assertEqual(self.commands.count("stop"), 1)
        self.assertEqual(self.s.end, 1900)

    def test_monotonic_reserve_stops_work(self):
        self.commands.after_start = lambda: setattr(self, "mono", self.mono + 841)
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(self.commands.count("cli"), 0)
        self.assertEqual(self.commands.count("stop"), 1)

    def test_wall_rollback_refused_not_reset(self):
        self.commands.after_start = lambda: setattr(self, "wall", self.wall - 1)
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(self.commands.count("stop"), 1)

    def test_cancel_after_start_cleanup_ignores_work_cancel(self):
        self.commands.after_start = lambda: setattr(self.s, "cancelled", True)
        result = self.run_fake()
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["cleanup"], "verified_stopped")
        self.assertEqual(self.commands.count("stop"), 1)

    def test_cancellation_during_quiescence_suppresses_success(self):
        self.held.after = lambda: setattr(self.s, "cancelled", True)
        self.assertEqual(self.run_fake()["status"], "failed")

    def test_missing_scope_unknown(self):
        self.held.error = R.V.Refusal(R.V.Code.IO)
        result = self.run_fake()
        self.assertEqual(result["cleanup"], "unknown")
        self.assertEqual(result["reason"], "cgroup_unavailable_or_unsafe")
        self.assertEqual(self.commands.count("stop"), 1)

    def test_capture_refusal_still_stops_unknown(self):
        self.s.capture = lambda *a, **k: (_ for _ in ()).throw(R.V.Refusal(R.V.Code.LIMIT))
        self.assertEqual(self.run_fake()["cleanup"], "unknown")
        self.assertEqual(self.commands.count("stop"), 1)
        self.assertEqual(self.commands.count("cli"), 0)

    def test_root_replaced_unknown(self):
        self.fresh.identity = (7, 43)
        self.assertEqual(self.run_fake()["cleanup"], "unknown")

    def test_pid_changed_unknown(self):
        self.commands.after_cli = lambda: self.commands.value["State"].update(Pid=124)
        self.assertEqual(self.run_fake()["cleanup"], "unknown")
        self.assertEqual(self.commands.count("stop"), 1)

    def test_populated_sibling_unknown(self):
        self.held.quiet = False
        self.assertEqual(self.run_fake()["cleanup"], "unknown")

    def test_cli_error_safe_typed_and_joined(self):
        self.commands.fail = "cli"
        result = self.run_fake()
        self.assertEqual(result["stage"], "dispatch")
        self.assertNotIn("private", M.encoded(result).decode())
        self.assertTrue(self.commands.joined)
        self.assertEqual(result["cleanup"], "verified_stopped")

    def test_malformed_or_wrong_envelope_never_success(self):
        # Each variant gets a separate one-shot fixture in a child test instance.
        self.commands.envelope = b'{"schema_version":true,"request_sha256":"wrong","outcome":"failed","error":"policy_denied"}'
        self.assertEqual(self.run_fake()["reason"], "transport_envelope")
        self.assertEqual(self.commands.count("cli"), 1)

    def test_nonzero_exit_not_auth_classifier(self):
        self.commands.rc = 1
        self.assertEqual(self.run_fake()["reason"], "transport_envelope")

    def test_unjoined_process_group_unknown(self):
        self.commands.after_cli = lambda: setattr(self.commands, "unknown", True)
        self.assertEqual(self.run_fake()["cleanup"], "unknown")
        self.assertEqual(self.commands.count("cli"), 1)
        self.assertEqual(self.commands.count("stop"), 1)

    def test_observation_close_failure_cannot_skip_stop(self):
        def close():
            raise OSError("close unavailable")
        self.fresh.close = close
        self.assertEqual(self.run_fake()["cleanup"], "unknown")
        self.assertEqual(self.commands.count("stop"), 1)
        self.assertTrue(self.held.closed)

    def test_new_supervisor_cannot_replay_consumed_identity(self):
        self.run_fake()
        before = snapshot(self.inputs.work)
        second = R.Renewed(self.inputs, self.bound, command=lambda *a, **k: self.fail("replay command"))
        self.assertEqual(second.run(self.bound, self.approval)["status"], "failed")
        self.assertEqual(before, snapshot(self.inputs.work))

    def test_consumed_drift_cannot_renew_window_or_skip_stop(self):
        def drift():
            path = self.inputs.work / "consumed.json"
            value = M.decode(M.read(path))
            value["wall_end"] += 900
            path.write_bytes(M.encoded(value))
        self.commands.after_start = drift
        result = self.run_fake()
        self.assertEqual(result["reason"], "consumed_drift")
        self.assertEqual(self.s.end, 1900)
        self.assertEqual(self.commands.count("stop"), 1)
        self.assertEqual(self.commands.count("cli"), 0)

    def test_docker_config_drift_still_stops(self):
        self.commands.after_start = lambda: (self.inputs.work / "docker/config.json").write_bytes(b"{\"changed\":true}")
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(self.commands.count("stop"), 1)
        self.assertEqual(self.commands.count("cli"), 0)

    def test_bound_source_hash_drift_refuses_before_commands(self):
        hashes = R.code_hashes()
        hashes[R.HERE.name] = "0" * 64
        with patch.object(R, "code_hashes", return_value=hashes):
            self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(self.commands.calls, [])

    def test_stop_failure_unknown(self):
        self.commands.fail = "stop"
        self.assertEqual(self.run_fake()["cleanup"], "unknown")
        self.assertTrue(self.held.closed)

    def test_result_write_failure_retains_consumed(self):
        real = M.durable
        def durable(path, value):
            if path.name == "result.json":
                raise OSError("disk full")
            real(path, value)
        with patch.object(M, "durable", side_effect=durable):
            self.assertEqual(self.run_fake()["reason"], "record_failed")
        self.assertTrue((self.inputs.work / "consumed.json").exists())
        count = len(self.commands.calls)
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(count, len(self.commands.calls))


class ReceiptTests(Fixtures):
    def setUp(self):
        super().setUp()
        self.staged()
        self.review = self.root / "fresh-review.json"
        self.operator = self.root / "fresh-approval.json"
        self.review_value = {"binding": self.bound, "issued_at": self.wall - 1, "expires_at": self.wall + 1000,
                             "decision": "approved", "role": "independent-review", "actor": "independent-human",
                             "independent": True, "verdict": "PASS"}
        self.operator_value = dict(self.review_value, role="operator", actor="operator-human")

    def receipts(self):
        for path in (self.review, self.operator):
            if path.exists():
                path.unlink()  # Synthetic receipts only, never production records.
        write(self.review, self.review_value)
        digest = M.sha(M.read(self.review))
        self.operator_value.setdefault("review_sha256", digest)
        write(self.operator, self.operator_value)
        return digest, M.sha(M.read(self.operator))

    def authorize(self):
        review_hash, operator_hash = self.receipts()
        R.authorize(self.bound, self.review, review_hash, self.operator, operator_hash)

    def test_renewed_receipts_ignore_old_expired_receipts(self):
        self.authorize()

    def test_invalid_review_actor_verdict_independence_expiry_or_binding(self):
        original = copy.deepcopy(self.review_value)
        for change in ({"actor": "operator-human"}, {"actor": "builder"}, {"verdict": "FAIL"},
                       {"independent": False}, {"issued_at": self.wall - 1801},
                       {"issued_at": self.wall + 1}, {"expires_at": self.wall},
                       {"expires_at": self.wall + 1801}, {"binding": {"old": True}}):
            self.review_value = dict(original, **change)
            self.operator_value.pop("review_sha256", None)
            with self.subTest(change=change), self.assertRaises(Exception):
                self.authorize()

    def test_operator_requires_exact_review_hash_and_distinct_actor(self):
        for change in ({"review_sha256": "0" * 64}, {"actor": "independent-human"},
                       {"actor": "live-transport-renewed-builder"}, {"expires_at": self.wall}):
            self.operator_value.update(change)
            with self.subTest(change=change), self.assertRaises(Exception):
                self.authorize()
            self.operator_value = dict(self.review_value, role="operator", actor="operator-human")

    def test_receipt_hash_mismatch_refuses(self):
        _, operator_hash = self.receipts()
        with self.assertRaises(M.Refuse):
            R.authorize(self.bound, self.review, "0" * 64, self.operator, operator_hash)


if __name__ == "__main__":
    unittest.main()
