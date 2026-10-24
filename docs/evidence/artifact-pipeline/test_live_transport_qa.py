"""Offline supervisor checks: fake Docker/CLI only; no effect-mode entry point."""

from contextlib import redirect_stderr
import importlib.util
import io
import json
import os
import re
from pathlib import Path

import sys
import tempfile
import time
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
SOURCE = Path(__file__).with_name("live-transport-qa.py")
SPEC = importlib.util.spec_from_file_location("live_transport_qa", SOURCE)
qa = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(qa)
CID = "c" * 64
IMAGE_ID = "sha256:" + "d" * 64


class FakeInputs:
    def __init__(self, work):
        self.work = work
        self.cli = work / "dagger"
        self.module = work / "module"
        self.changes = 0
        self.drift_at = None

    def preserve_snapshot(self, binding):
        qa.durable(self.work / "module-manifest.json", {"offline-fake": "0" * 64})

    def unchanged(self, binding):
        self.changes += 1
        qa.need(self.drift_at != self.changes)


class FakeCommands:
    unknown = False

    def __init__(self, supervisor):
        self.s = supervisor
        self.calls = []
        self.running = False
        self.existing = None
        self.foreign = False
        self.failure = None
        self.response = None
        self.cli_exit = 0
        self.cli_joined = False
        self.cancel = False
        self.create_failure = False

    def __call__(self, argv, deadline, cleanup=False, cap=1024 * 1024):
        self.calls.append(argv)
        qa.need(deadline <= self.s.end and (cleanup or deadline <= self.s.work_end))
        qa.need((self.s.work / "consumed.json").exists())
        qa.need((self.s.work / "pending.json").exists())
        if argv[0] != "/usr/bin/docker":
            self.cli_joined = True  # Fake process runner returns only after join.
            if self.failure == "capture" or self.cancel:
                self.s.cancelled = self.cancel
                raise qa.Refuse()
            value = self.response or {"schema_version": 1, "request_sha256": "a" * 64,
                                      "outcome": "failed", "error": "policy_denied"}
            return self.cli_exit, qa.encoded(value), b"private diagnostic"
        self.assert_prefix(argv)
        args = argv[5:]
        head = tuple(args[:2])
        if head == ("container", "ls"):
            raw = qa.NAME if self.existing == "engine" else ""
        elif head == ("volume", "ls"):
            raw = qa.VOLUME if self.existing == "volume" else ""
        elif head == ("pull", qa.IMAGE):
            raw = ""
        elif head == ("image", "inspect"):
            raw = qa.encoded([{"Id": IMAGE_ID}])
        elif head == ("volume", "create"):
            raw = qa.VOLUME
        elif head == ("volume", "inspect"):
            raw = qa.encoded([{"Name": qa.VOLUME, "Driver": "local", "Options": None,
                               "Labels": {qa.LABEL: self.s.nonce}}])
        elif args[0] == "create":
            (self.s.work / "engine.cid").write_text(CID)
            if self.create_failure:
                raise qa.Refuse()
            raw = CID
        elif head == ("container", "inspect"):
            raw = qa.encoded([self.inspect()])
        elif args[0] == "start":
            qa.need((self.s.work / "owned.json").exists())
            qa.need(qa.decode((self.s.work / "owned.json").read_bytes())["cid"] == CID)
            self.running = True
            raw = CID
        elif args[0] == "stop":
            qa.need(not any("run-fixture" in call for call in self.calls) or self.cli_joined)
            self.running = False
            raw = CID
        else:
            raise AssertionError("unrecognized fake command")
        return 0, raw.encode() if isinstance(raw, str) else raw, b""

    def assert_prefix(self, argv):
        qa.need(argv[:5] == ["/usr/bin/docker", "--host", qa.SCOPE["endpoint"],
                              "--config", str(self.s.work / "docker")])

    def inspect(self):
        return {"Id": CID, "Image": IMAGE_ID, "Name": "/" + qa.NAME,
                "Config": {"Labels": {qa.LABEL: "foreign" if self.foreign else self.s.nonce}},
                "State": {"Running": self.running, "Pid": 1234 if self.running else 0},
                "HostConfig": {"Privileged": True, "CgroupnsMode": "private", "NanoCpus": 4000000000,
                               "Memory": 4294967296, "MemorySwap": 4294967296, "PidsLimit": 1024,
                               "PortBindings": {}, "Binds": None, "NetworkMode": "default",
                               "RestartPolicy": {"Name": "no"}},
                "Mounts": [{"Type": "volume", "Name": qa.VOLUME, "Destination": "/var/lib/dagger", "RW": True},
                           {"Type": "bind", "Source": str(self.s.work / "engine.json"),
                            "Destination": "/etc/dagger/engine.json", "RW": False}]}


class SupervisorTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.work = Path(self.temp.name)
        self.inputs = FakeInputs(self.work)
        self.s = qa.Supervisor(self.inputs, command=lambda *a, **k: None,
                               probe=lambda pid: Path("/fake/cgroup"), empty=lambda path: True)
        self.fake = FakeCommands(self.s)
        self.s.command = self.fake
        self.binding = {"request": "a" * 64}

    def run_fake(self):
        return self.s.run(self.binding, lambda: None)

    def stops(self):
        return [c for c in self.fake.calls if c[0] == "/usr/bin/docker" and c[5] == "stop"]

    def test_success_exact_invocation_and_markers(self):
        self.assertEqual(self.run_fake(), {"status": "policy_denied", "cleanup": "verified_stopped"})
        cli = [c for c in self.fake.calls if c[0] != "/usr/bin/docker"]
        self.assertEqual(cli, [[str(self.inputs.cli), "--mod", str(self.inputs.module),
                               "call", "run-fixture", "--request", str(self.work / "request.json")]])
        self.assertEqual(self.s.env["_EXPERIMENTAL_DAGGER_RUNNER_HOST"], "container+docker://" + CID)
        self.assertEqual(len(self.stops()), 1)
        self.assertTrue((self.work / "pending.json").exists())
        self.assertTrue((self.work / "dispatch.json").exists())
        self.assertFalse(any("rm" in c or "prune" in c or "run" in c for c in self.fake.calls))
        self.assertNotIn("TOKEN", " ".join(self.s.env))

    def test_local_log_driver_single_file_requires_no_compression(self):
        self.assertEqual(self.run_fake()["status"], "policy_denied")
        creates = [c for c in self.fake.calls if c[0] == "/usr/bin/docker" and c[5] == "create"]
        self.assertEqual(len(creates), 1)
        create = creates[0]
        self.assertEqual(create[create.index("--log-driver") + 1], "local")
        options = [create[i + 1] for i, value in enumerate(create) if value == "--log-opt"]
        self.assertEqual(options, ["max-size=1m", "max-file=1", "compress=false"])

    def test_failed_dispatch_is_not_retried(self):
        self.fake.cli_exit = 1
        self.assertEqual(self.run_fake()["status"], "failed")
        cli = [c for c in self.fake.calls if c[0] != "/usr/bin/docker"]
        self.assertEqual(len(cli), 1)
        for operation in ("pull", "create", "start"):
            self.assertEqual(sum(c[0] == "/usr/bin/docker" and c[5] == operation for c in self.fake.calls), 1)
        count = len(self.fake.calls)
        self.run_fake()
        self.assertEqual(len(self.fake.calls), count)

    def test_no_resume_retry_or_other_effect_modes(self):
        for mode in ("resume", "retry", "prepare", "setup"):
            with self.subTest(mode=mode), patch.object(sys, "argv", [str(SOURCE), mode]), \
                    patch.object(qa, "Inputs") as inputs, redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as raised:
                    qa.main()
                self.assertEqual(raised.exception.code, 2)
                inputs.assert_not_called()

    def test_existing_names_refused_before_mutations(self):
        for existing in ("engine", "volume"):
            with self.subTest(existing=existing):
                self.fake.existing = existing
                self.assertEqual(self.run_fake()["status"], "failed")
                self.assertEqual(len(self.fake.calls), 2)
                self.assertFalse(self.stops())
                # Reuse is refused locally, without even a Docker query.
                before = len(self.fake.calls)
                self.run_fake()
                self.assertEqual(before, len(self.fake.calls))
                if existing == "engine":
                    self.setUp()

    def test_no_retry_after_success(self):
        self.run_fake()
        count = len(self.fake.calls)
        self.assertNotEqual(self.run_fake()["status"], "policy_denied")
        self.assertEqual(count, len(self.fake.calls))

    def test_foreign_cleanup_never_stops(self):
        self.fake.foreign = True
        self.assertEqual(self.run_fake()["cleanup"], "unknown")
        self.assertFalse(self.stops())

    def test_no_stop_on_changed_image(self):
        original = self.fake.inspect
        def foreign_image():
            value = original()
            value["Image"] = "sha256:" + "e" * 64
            return value
        self.fake.inspect = foreign_image
        self.assertEqual(self.run_fake()["cleanup"], "unknown")
        self.assertFalse(self.stops())

    def test_pids_drift_after_join_still_stops_owned_engine(self):
        original = self.fake.inspect
        def drifted():
            value = original()
            if self.fake.cli_joined:
                value["HostConfig"]["PidsLimit"] = 0
            return value
        self.fake.inspect = drifted
        result = self.run_fake()
        self.assertEqual(len(self.stops()), 1)
        self.assertFalse(self.fake.running)
        self.assertEqual(result, {"status": "failed", "cleanup": "unknown"})
        self.assertTrue((self.work / "pending.json").exists())
        self.assertEqual(qa.decode((self.work / "result.json").read_bytes())["cleanup"], "unknown")

    def test_foreign_owner_after_join_never_stopped(self):
        original = self.fake.inspect
        def foreign():
            value = original()
            if self.fake.cli_joined:
                value["Config"]["Labels"][qa.LABEL] = "foreign"
            return value
        self.fake.inspect = foreign
        self.assertEqual(self.run_fake()["cleanup"], "unknown")
        self.assertFalse(self.stops())
        self.assertTrue(self.fake.running)

    def invalidated_run(self, after, kind):
        original_command, original_unchanged = self.fake.__call__, self.inputs.unchanged
        invalid = False
        def command(argv, deadline, **kw):
            nonlocal invalid
            result = original_command(argv, deadline, **kw)
            if argv[0] == "/usr/bin/docker" and argv[5:5 + len(after)] == list(after):
                invalid = True
                if kind == "cancel":
                    self.s.cancelled = True
                elif kind == "deadline":
                    self.s.work_end = time.monotonic() - 1
            return result
        def unchanged(binding):
            original_unchanged(binding)
            qa.need(not (invalid and kind == "source"))
        def authorize():
            qa.need(not (invalid and kind == "approval"))
        self.s.command = command
        self.inputs.unchanged = unchanged
        return self.s.run(self.binding, authorize)

    def assert_no_creates(self):
        self.assertFalse(any(c[0] == "/usr/bin/docker" and
                             (c[5] == "create" or c[5:7] == ["volume", "create"])
                             for c in self.fake.calls))
        self.assertFalse((self.work / "engine.cid").exists())
        self.assertTrue((self.work / "pending.json").exists())

    def test_source_invalidated_during_pull_blocks_creates(self):
        self.assertEqual(self.invalidated_run(("pull",), "source")["status"], "failed")
        self.assert_no_creates()

    def test_approval_invalidated_during_pull_blocks_creates(self):
        self.assertEqual(self.invalidated_run(("pull",), "approval")["status"], "failed")
        self.assert_no_creates()

    def test_cancel_during_pull_blocks_creates(self):
        self.assertEqual(self.invalidated_run(("pull",), "cancel")["status"], "failed")
        self.assert_no_creates()

    def test_deadline_during_pull_blocks_creates(self):
        self.assertEqual(self.invalidated_run(("pull",), "deadline")["status"], "failed")
        self.assert_no_creates()

    def test_source_invalidated_after_volume_inspect_blocks_container_create(self):
        self.assertEqual(self.invalidated_run(("volume", "inspect"), "source")["status"], "failed")
        self.assertFalse(any(c[5] == "create" for c in self.fake.calls))

    def test_approval_invalidated_after_volume_inspect_blocks_container_create(self):
        self.assertEqual(self.invalidated_run(("volume", "inspect"), "approval")["status"], "failed")
        self.assertFalse(any(c[5] == "create" for c in self.fake.calls))

    def test_approval_invalidated_after_create_blocks_start_but_not_cleanup(self):
        self.assertEqual(self.invalidated_run(("create",), "approval")["status"], "failed")
        self.assertFalse(any(c[5] == "start" for c in self.fake.calls))
        self.assertEqual(len(self.stops()), 1)

    def test_source_invalidated_after_start_blocks_dispatch_but_not_cleanup(self):
        self.assertEqual(self.invalidated_run(("start",), "source")["status"], "failed")
        self.assertTrue(all(c[0] == "/usr/bin/docker" for c in self.fake.calls))
        self.assertEqual(len(self.stops()), 1)
        self.assertFalse(self.fake.running)

    def test_capture_failure_still_stops_after_join(self):
        self.fake.failure = "capture"
        self.assertEqual(self.run_fake(), {"status": "failed", "cleanup": "verified_stopped"})
        self.assertEqual(len(self.stops()), 1)

    def test_cancel_still_stops_after_join(self):
        self.fake.cancel = True
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(len(self.stops()), 1)

    def test_missing_cgroup_proof_still_stops_but_unknown(self):
        self.s.probe = lambda pid: (_ for _ in ()).throw(qa.Refuse())
        self.assertEqual(self.run_fake()["cleanup"], "unknown")
        self.assertEqual(len(self.stops()), 1)
        self.assertFalse((self.work / "dispatch.json").exists())

    def test_unknown_cli_group_does_not_prevent_engine_stop(self):
        original = self.fake.__call__
        def unknown(argv, deadline, **kw):
            result = original(argv, deadline, **kw)
            if argv[0] != "/usr/bin/docker":
                self.fake.unknown = True
            return result
        self.s.command = unknown
        # Functions can carry the same liveness flag as the real runner.
        unknown.unknown = False
        def probe(pid):
            if self.fake.unknown:
                unknown.unknown = True
            return Path("/fake/cgroup")
        self.s.probe = probe
        self.assertEqual(self.run_fake()["cleanup"], "unknown")
        self.assertEqual(len(self.stops()), 1)

    def test_nonempty_cgroup_unknown(self):
        self.s.empty = lambda path: False
        self.assertEqual(self.run_fake()["cleanup"], "unknown")
        self.assertEqual(len(self.stops()), 1)

    def test_ambiguous_create_records_cid_never_starts(self):
        self.fake.create_failure = True
        self.assertEqual(self.run_fake()["cleanup"], "unknown")
        self.assertTrue((self.work / "owned.json").exists())
        self.assertFalse(any(c[5] == "start" for c in self.fake.calls))
        self.assertEqual(len(self.stops()), 1)

    def test_bad_correlation_and_scalar_codec_rejected(self):
        self.fake.response = {"schema_version": 1, "request_sha256": "b" * 64,
                              "outcome": "failed", "error": "policy_denied"}
        self.assertEqual(self.run_fake()["status"], "failed")
        for raw in (b'"{\\"schema_version\\":1}"', b'{}\n{}', b'{"x":1,"x":2}', b'NaN'):
            with self.subTest(raw=raw):
                try:
                    value = qa.decode(raw)
                except (ValueError, qa.Refuse):
                    continue
                self.assertNotEqual(value, {"schema_version": 1, "request_sha256": "a" * 64,
                                             "outcome": "failed", "error": "policy_denied"})

    def test_receipt_failure_has_no_docker_effect(self):
        def deny():
            raise qa.Refuse()
        self.assertEqual(self.s.run(self.binding, deny)["status"], "failed")
        self.assertFalse(self.fake.calls)
        self.assertFalse((self.work / "consumed.json").exists())

    def test_changed_pid_still_stops_but_cleanup_unknown(self):
        original = self.fake.inspect
        def changed():
            value = original()
            if self.fake.cli_joined and self.fake.running:
                value["State"]["Pid"] = 5678
            return value
        self.fake.inspect = changed
        self.assertEqual(self.run_fake()["cleanup"], "unknown")
        self.assertEqual(len(self.stops()), 1)

    def test_resource_drift_refused_before_start(self):
        original = self.fake.inspect
        def unbounded():
            value = original()
            value["HostConfig"]["PidsLimit"] = 0
            return value
        self.fake.inspect = unbounded
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertFalse(any(c[5] == "start" for c in self.fake.calls))

    def test_nonzero_cli_not_success(self):
        self.fake.cli_exit = 1
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(len(self.stops()), 1)

    def test_source_drift_before_effect(self):
        self.inputs.drift_at = 1
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(self.fake.calls, [])

    def test_cli_or_source_drift_after_start_stops(self):
        original = self.inputs.unchanged
        def drifted(binding):
            original(binding)
            qa.need(not self.fake.running)
        self.inputs.unchanged = drifted
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(len(self.stops()), 1)

    def test_expired_work_deadline_cannot_dispatch(self):
        self.s.work_end = time.monotonic() - 1
        original = self.fake.__call__
        def bounded(argv, deadline, **kw):
            qa.need(time.monotonic() < deadline)
            return original(argv, deadline, **kw)
        self.s.command = bounded
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertFalse((self.work / "dispatch.json").exists())

    def test_no_self_review_or_unbound_receipt(self):
        path = self.work / "receipt.json"
        value = {"binding": self.binding, "issued_at": 100, "expires_at": 200,
                 "actor": "reviewer", "role": "independent-review", "decision": "approved", "independent": True}
        path.write_bytes(qa.encoded(value))
        path.chmod(0o600)
        # Patch only ancestry check: disposable /tmp may be world-writable.
        with patch.object(qa, "safe"):
            qa.receipt(path, qa.sha(path.read_bytes()), "independent-review", self.binding, 150)
            for change in ({"actor": "live-transport-qa-builder"}, {"independent": False},
                           {"binding": {}}, {"expires_at": 149}, {"issued_at": 151}):
                bad = dict(value, **change)
                path.write_bytes(qa.encoded(bad))
                with self.assertRaises(qa.Refuse):
                    qa.receipt(path, qa.sha(path.read_bytes()), "independent-review", self.binding, 150)


class LocalCaptureTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.work = Path(self.temp.name)
        self.runner = qa.Commands(self.work, {"PATH": "/usr/bin:/bin"}, lambda: False)

    def test_both_streams_captured_private_and_bounded(self):
        rc, out, err = self.runner([sys.executable, "-B", "-c",
                                    "import os; os.write(1,b'a'*4096); os.write(2,b'b'*4096)"],
                                   time.monotonic() + 3, cap=8192)
        self.assertEqual((rc, len(out), len(err)), (0, 4096, 4096))
        self.assertTrue(all(p.stat().st_mode & 0o077 == 0 for p in self.work.iterdir()))
        with self.assertRaises(qa.Refuse):
            self.runner([sys.executable, "-B", "-c", "import os; os.write(1,b'x'*100000)"],
                        time.monotonic() + 3, cap=100)
        self.assertLessEqual((self.work / "capture-002.out").stat().st_size, 100)

    def test_cleanup_survives_capture_open_failure_and_cancel(self):
        self.runner.cancelled = lambda: True
        real_open = os.open
        def broken_log(path, flags, *args):
            if str(path).endswith((".out", ".err")):
                raise OSError("fake storage failure")
            return real_open(path, flags, *args)
        with patch.object(os, "open", side_effect=broken_log):
            rc, out, err = self.runner([sys.executable, "-B", "-c", "print('private')"],
                                       time.monotonic() + 3, cleanup=True)
        self.assertEqual((rc, out, err), (0, b"private\n", b""))

    def test_deadline_terminates_and_joins_group(self):
        pidfile = self.work / "pid"
        code = "import os,time; open(%r,'w').write(str(os.getpid())); time.sleep(30)" % str(pidfile)
        with self.assertRaises(qa.Refuse):
            self.runner([sys.executable, "-B", "-c", code], time.monotonic() + 0.3)
        pid = int(pidfile.read_text())
        with self.assertRaises(ProcessLookupError):
            os.killpg(pid, 0)
        self.assertFalse(self.runner.unknown)

    def test_cancel_terminates_and_joins_group(self):
        pidfile = self.work / "pid"
        self.runner.cancelled = lambda: pidfile.exists()
        code = "import os,time; open(%r,'w').write(str(os.getpid())); time.sleep(30)" % str(pidfile)
        with self.assertRaises(qa.Refuse):
            self.runner([sys.executable, "-B", "-c", code], time.monotonic() + 3)
        with self.assertRaises(ProcessLookupError):
            os.killpg(int(pidfile.read_text()), 0)


class SnapshotTests(unittest.TestCase):
    def test_staging_uses_private_repository_leaf_not_existing_target(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            target = root / "target"
            target.mkdir()
            target.chmod(0o775)
            inputs = qa.Inputs(root)
            self.assertEqual(inputs.base, root / ".smith-artifact-functional-qa")
            self.assertEqual(inputs.work, inputs.base / "run-01")
            self.assertEqual(inputs.cli, inputs.base / "dagger")
            inputs.base.mkdir(mode=0o700)
            self.assertEqual(inputs.base.stat().st_mode & 0o777, 0o700)
            self.assertEqual(target.stat().st_mode & 0o777, 0o775)
            old_work = target / "artifact-pipeline-functional-qa/run-01"
            self.assertNotEqual(qa.sha(str(inputs.work.absolute()).encode()),
                                qa.sha(str(old_work.absolute()).encode()))

    def test_complete_snapshot_and_actual_drift_checks(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            inputs = qa.Inputs(root)
            inputs.source.mkdir()
            inputs.module.mkdir(parents=True)
            inputs.cli.write_bytes(b"staged-cli")
            inputs.installed = root / "installed"
            inputs.installed.write_bytes(b"installed-cli")
            for folder in (inputs.source, inputs.module):
                (folder / "a").write_bytes(b"source")
            for name, value in (("request.json", b"request"), ("engine.json", b"engine"),
                                ("docker/config.json", b"{}")):
                path = inputs.work / name
                path.parent.mkdir(exist_ok=True)
                path.write_bytes(value)
            with patch.object(qa, "safe"):
                binding = {"source": qa.sha(qa.encoded(qa.tree(inputs.source))),
                           "module": qa.sha(qa.encoded(qa.tree(inputs.module))),
                           "cli": qa.sha(b"staged-cli"), "installed_cli": qa.sha(b"installed-cli"),
                           "request": qa.sha(b"request"), "engine_config": qa.sha(b"engine"),
                           "docker_config": qa.sha(b"{}"), "script": qa.sha(SOURCE.read_bytes())}
                inputs.unchanged(binding)
                for path in (inputs.source / "a", inputs.module / "a", inputs.cli, inputs.installed):
                    original = path.read_bytes()
                    path.write_bytes(b"drift")
                    with self.assertRaises(qa.Refuse):
                        inputs.unchanged(binding)
                    path.write_bytes(original)
                # No inferred SDK exclusion: even generated-looking additions fail closed.
                (inputs.module / "sdk").mkdir()
                (inputs.module / "sdk/client.py").write_bytes(b"generated")
                with self.assertRaises(qa.Refuse):
                    inputs.unchanged(binding)
                (inputs.module / "sdk/client.py").unlink()
                (inputs.module / "sdk").rmdir()
                (inputs.module / "extra").write_bytes(b"extra")
                with self.assertRaises(qa.Refuse):
                    inputs.unchanged(binding)

    def test_cgroup_descendants_must_all_be_empty(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp)
            (path / "cgroup.events").write_text("populated 0\nfrozen 0\n")
            (path / "cgroup.procs").write_text("")
            child = path / "child"
            child.mkdir()
            (child / "cgroup.procs").write_text("123\n")
            with self.assertRaises(qa.Refuse):
                qa.empty_cgroup(path)
            (child / "cgroup.procs").write_text("")
            self.assertTrue(qa.empty_cgroup(path))
            (path / "cgroup.events").write_text("populated 1\nfrozen 0\n")
            with self.assertRaises(qa.Refuse):
                qa.empty_cgroup(path)
            self.assertTrue(qa.empty_cgroup(path / "absent"))

    def test_request_is_execute_exact_baseline_and_hashes(self):
        binding = {"cli": "a" * 64, "module": "b" * 64}
        raw = qa.expected_request(binding, b"baseline\n")
        value = qa.decode(raw)
        self.assertEqual(value["phase"], "execute")
        self.assertEqual(value["attempt"], 1)
        self.assertEqual(value["executable_sha256"], binding["cli"])
        self.assertEqual(qa.decode(value["input"].encode())["baseline"]["sha256"], qa.sha(b"baseline\n"))


if __name__ == "__main__":
    # Evidence is bounded/redacted; never invoke the supervisor's run/check CLI.
    result = io.StringIO()
    suite = unittest.defaultTestLoader.loadTestsFromModule(sys.modules[__name__])
    outcome = unittest.TextTestRunner(stream=result, verbosity=2).run(suite)
    raw = result.getvalue()
    if not outcome.wasSuccessful():
        # Preserve failure details but redact disposable host paths.
        raw = re.sub(r'"/[^"\n]+"', '"<local-path>"', raw)
    log = Path(__file__).with_name("live-transport-qa-offline.log")
    with log.open("a") as stream:
        stream.write("OFFLINE ONLY: injected commands and local Python children; no Docker/network.\n")
        stream.write("source_sha256=" + qa.sha(SOURCE.read_bytes()) + "\n")
        stream.write("tests_sha256=" + qa.sha(Path(__file__).read_bytes()) + "\n")
        stream.write(raw)
    print(json.dumps({"offline_tests": outcome.testsRun, "successful": outcome.wasSuccessful(),
                      "failures": len(outcome.failures), "errors": len(outcome.errors)}))
    raise SystemExit(not outcome.wasSuccessful())
