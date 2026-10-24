"""Offline fake commands only; original operational files are never mutated."""
import copy
import importlib.util
import io
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve()

def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, HERE.with_name(filename))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

C = load("renewed_continue_subject", "live-transport-renewed-continue.py")
T = load("renewed_continue_fixture_helpers", "test_live_transport_renewed.py")
# Reuse synthetic fixture machinery with the actual continuation's helper objects.
T.R, T.M = C.R, C.M
M, R = C.M, C.R


class Fixtures(T.Fixtures):
    def setUp(self):
        baseline = T.baseline
        def oom_baseline(old):
            value = baseline(old)
            value["HostConfig"]["OomKillDisable"] = False
            return value
        with patch.object(T, "baseline", side_effect=oom_baseline):
            super().setUp()
        original = R.stage(self.inputs, T.RUN_ID)
        self.anchor = dict(binding=original, **C.ENDPOINTS)
        self.wall = C.ENDPOINTS["wall_start"] + 100
        self.mono = C.ENDPOINTS["monotonic_start"] + 100
        M.durable(self.inputs.work / "consumed.json", self.anchor)
        M.durable(self.inputs.work / "pending.json", {"cid": R.CID, "nonce": "owned-nonce"})
        M.durable(self.inputs.work / "result.json", dict(status="failed", cleanup="not_started",
            stage="ownership", reason="config_drift", binding=original,
            previous_cleanup="unknown_not_retroactively_resolved"))
        for name in ("review.json", "approval.json"):
            M.durable(self.inputs.work / name, {"old": name})
        (self.inputs.work / "captures").mkdir(mode=0o700)
        current = copy.deepcopy(M.decode(M.read(self.inputs.old / "capture-008.out")))
        current[0]["HostConfig"]["OomKillDisable"] = None
        M.durable(self.inputs.work / "captures/capture-001.out", current)
        (self.inputs.work / "captures/capture-001.err").write_bytes(b"")
        (self.inputs.work / "captures/capture-001.err").chmod(0o600)
        self.original_snapshot = T.snapshot(self.inputs.work)
        self.pins = patch.dict(C.PINS, {n: M.sha(M.read(self.inputs.work / n)) for n in C.PINS})
        self.pins.start()
        self.addCleanup(self.pins.stop)
        def preserved():
            now = T.snapshot(self.inputs.work)
            self.assertEqual(self.original_snapshot,
                {k: v for k, v in now.items() if not k.startswith("continuation-")})
        self.addCleanup(preserved)

        self.bound = C.check(self.inputs)

    def supervisor(self):
        self.held, self.fresh = T.Held(), T.Held()
        self.observations = []
        def capture(pid, cid, **kwargs):
            self.observations.append((pid, cid, kwargs))
            self.assertEqual((pid, cid), (123, R.CID))
            self.assertEqual(kwargs["run_deadline"], C.ENDPOINTS["monotonic_end"])
            return self.held if len(self.observations) == 1 else self.fresh
        self.s = C.Continuation(self.inputs, self.bound, command=lambda *a, **k: None, capture=capture)
        self.commands = T.Commands(self.s)
        self.commands.value["HostConfig"]["OomKillDisable"] = None
        # Existing fake enforces the exact unchanged request/env/argv contract.
        self.commands.envelope = M.encoded({"schema_version": 1,
            "request_sha256": self.bound["original"]["request"], "outcome": "failed", "error": "policy_denied"})
        self.s.command = self.commands
        self.approval = lambda: None
        return self.s


class BindingTests(Fixtures):
    def test_check_and_stage_read_only(self):
        before = T.snapshot(self.root)
        self.assertEqual(C.stage(self.inputs), self.bound)
        self.assertEqual(C.check(self.inputs), self.bound)
        self.assertEqual(T.snapshot(self.root), before)
        self.assertEqual(set(self.bound["continuation_code"]), {HERE.name, C.HERE.name})

    def test_expired_check_still_read_only(self):
        self.wall += 10000
        self.mono += 10000
        self.assertEqual(C.check(self.inputs), self.bound)

    def test_each_original_file_drift(self):
        for name in C.PINS:
            with self.subTest(name=name):
                path = self.inputs.work / name
                raw, meta = path.read_bytes(), path.stat()
                path.write_bytes(raw + b" ")
                with self.assertRaises(Exception):
                    C.check(self.inputs)
                path.write_bytes(raw)
                import os
                os.utime(path, ns=(meta.st_atime_ns, meta.st_mtime_ns))

    def test_unexpected_file_directory_and_original_dispatch(self):
        for name in ("running.json", "dispatch.json", "extra"):
            path = self.inputs.work / name
            path.mkdir(mode=0o700)
            with self.assertRaises(Exception):
                C.check(self.inputs)
            path.rmdir()

    def test_all_continuation_markers_fence(self):
        for name in (*C.NAMES, "continuation-unknown"):
            path = self.inputs.work / name
            path.mkdir(mode=0o700)
            with self.assertRaises(Exception):
                C.check(self.inputs)
            path.rmdir()

    def test_explicit_root_required_no_process(self):
        with patch.object(sys, "argv", [str(C.HERE), "check"]), patch("sys.stderr", new=io.StringIO()):
            with self.assertRaises(SystemExit):
                C.main()

    def test_cli_uses_explicit_root(self):
        with patch.object(sys, "argv", [str(C.HERE), "check", "--root", str(self.root)]), \
                patch.object(C, "Inputs", return_value=self.inputs) as inputs, \
                patch("sys.stdout", new=io.StringIO()):
            self.assertEqual(C.main(), 0)
            inputs.assert_called_once_with(root=self.root.resolve())


class RunTests(Fixtures):
    def setUp(self):
        super().setUp()
        self.supervisor()

    def test_success_fixed_endpoints_single_dispatch_stop_and_replay_fence(self):
        self.assertEqual(self.s.end, C.ENDPOINTS["monotonic_end"])
        self.assertEqual(self.s.work_end, C.ENDPOINTS["monotonic_end"] - 60)
        result = self.run_fake()
        self.assertEqual((result["status"], result["cleanup"]), ("policy_denied", "verified_stopped"))
        for verb in ("start", "cli", "stop"):
            self.assertEqual(self.commands.count(verb), 1)
        self.assertEqual(M.decode(M.read(self.inputs.work / "continuation-consumed.json"))["anchor"], self.anchor)
        calls = len(self.commands.calls)
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(len(self.commands.calls), calls)
        other = C.Continuation(self.inputs, self.bound, command=self.commands)
        self.assertEqual(other.run(self.bound, self.approval)["status"], "failed")
        self.assertEqual(len(self.commands.calls), calls)

    def test_exact_oom_types_and_no_mutation(self):
        original_baseline = copy.deepcopy(self.s.baseline)
        for value in (False, None):
            obj = copy.deepcopy(self.commands.value)
            obj["HostConfig"]["OomKillDisable"] = value
            before = copy.deepcopy(obj)
            self.assertIs(self.s.execution_config(obj), obj)
            self.assertEqual(obj, before)
            self.assertEqual(self.s.baseline, original_baseline)
        for value in (True, 0, 1, "false", "", [], {}):
            with self.subTest(value=value):
                obj = copy.deepcopy(self.commands.value)
                obj["HostConfig"]["OomKillDisable"] = value
                with self.assertRaises(Exception):
                    self.s.execution_config(obj)
        del obj["HostConfig"]["OomKillDisable"]
        with self.assertRaises(Exception):
            self.s.execution_config(obj)
        self.assertEqual(self.s.baseline, original_baseline)

    def test_invalid_baseline_oom_rejected(self):
        self.s.baseline["HostConfig"]["OomKillDisable"] = 0
        with self.assertRaises(Exception):
            self.s.execution_config(self.commands.value)

    def test_other_config_comparisons_unchanged(self):
        for key in ("Config", "HostConfig", "Mounts", "Path", "Args"):
            with self.subTest(key=key):
                obj = copy.deepcopy(self.commands.value)
                if key in ("Config", "HostConfig"):
                    obj[key]["unexpected"] = True
                elif key == "Path":
                    obj[key] += "-changed"
                else:
                    obj[key].append("changed")
                with self.assertRaises(Exception):
                    self.s.execution_config(obj)

    def test_deadlines_and_rollback_before_commands(self):
        for wall, mono in ((self.anchor["wall_end"] - 60, self.mono),
                           (self.wall, self.anchor["monotonic_end"] - 60),
                           (self.anchor["wall_start"] - 1, self.mono),
                           (self.wall, self.anchor["monotonic_start"] - 1)):
            with self.subTest(wall=wall, mono=mono):
                self.wall, self.mono = wall, mono
                self.assertEqual(self.run_fake()["status"], "failed")
                self.assertEqual(self.commands.calls, [])

    def test_observed_wall_rollback_rejected(self):
        self.s.approval = self.approval
        self.s.gate()
        self.wall -= 1
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(self.commands.calls, [])

    def test_own_digest_drift_no_commands(self):
        self.bound["continuation_code"][C.HERE.name] = "0" * 64
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(self.commands.calls, [])

    def test_original_bound_drift_no_commands(self):
        self.bound["original"]["request"] = "0" * 64
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(self.commands.calls, [])

    def test_authorization_refusal_no_commands(self):
        self.approval = lambda: C.require(False, "approval")
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(self.commands.calls, [])

    def test_ambiguous_start_still_stops(self):
        self.commands.fail = "start"
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(self.commands.count("stop"), 1)
        self.assertEqual(self.commands.count("cli"), 0)

    def test_expiry_after_start_still_stops(self):
        self.commands.after_start = lambda: setattr(self, "wall", self.anchor["wall_end"])
        result = self.run_fake()
        self.assertEqual((result["status"], result["cleanup"]), ("failed", "unknown"))
        self.assertEqual(self.commands.count("stop"), 1)
        self.assertEqual(self.commands.count("cli"), 0)

    def test_cancel_after_start_still_stops(self):
        self.commands.after_start = lambda: setattr(self.s, "cancelled", True)
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(self.commands.count("stop"), 1)
        self.assertEqual(self.commands.count("cli"), 0)

    def test_approval_drift_after_start_still_stops(self):
        def revoke():
            self.approval = lambda: C.require(False, "approval")
            self.s.approval = self.approval
        self.commands.after_start = revoke
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(self.commands.count("stop"), 1)

    def test_config_drift_after_start_still_stops_unknown(self):
        self.commands.after_start = lambda: self.commands.value["HostConfig"].update(Memory=1)
        result = self.run_fake()
        self.assertEqual((result["status"], result["cleanup"]), ("failed", "unknown"))
        self.assertEqual(self.commands.count("stop"), 1)

    def test_cleanup_inspect_failure_still_stops(self):
        self.commands.fail = "inspect_cleanup"
        self.assertEqual(self.run_fake()["cleanup"], "unknown")
        self.assertEqual(self.commands.count("stop"), 1)

    def test_bad_envelope_not_success(self):
        self.commands.envelope = b"{}"
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(self.commands.count("cli"), 1)
        self.assertEqual(self.commands.count("stop"), 1)

    def test_missing_scope_unknown(self):
        self.s.capture = lambda *a, **k: C.require(False, "missing_scope")
        self.assertEqual(self.run_fake()["cleanup"], "unknown")
        self.assertEqual(self.commands.count("stop"), 1)

    def test_populated_scope_unknown(self):
        self.held.quiet = False
        self.assertEqual(self.run_fake()["cleanup"], "unknown")

    def test_quiescence_cancel_suppresses_success(self):
        self.held.after = lambda: setattr(self.s, "cancelled", True)
        self.assertEqual(self.run_fake()["status"], "failed")

    def test_foreign_identity_never_started(self):
        self.commands.value["Id"] = "foreign"
        self.assertEqual(self.run_fake()["cleanup"], "not_started")
        self.assertEqual(self.commands.count("start"), 0)

    def test_cli_workspace_generation_allowed_only_after_dispatch(self):
        path = self.inputs.work / "cache/generated.json"
        def generated():
            M.durable(path, {"fake_cli": True})
        self.commands.after_cli = generated
        result = self.run_fake()
        self.assertEqual((result["status"], result["cleanup"]), ("policy_denied", "verified_stopped"))
        # Remove only synthetic generated data before the immutable-fixture check.
        path.unlink()

    def test_workspace_generation_before_dispatch_refused(self):
        path = self.inputs.work / "cache/unexpected.json"
        M.durable(path, {})
        self.assertEqual(self.run_fake()["status"], "failed")
        self.assertEqual(self.commands.calls, [])
        path.unlink()

    def test_original_cleanup_and_observe_reused(self):
        self.assertIs(C.Continuation.cleanup, R.Renewed.cleanup)
        self.assertIs(C.Continuation.observe, R.Renewed.observe)


class ReceiptTests(Fixtures):
    def setUp(self):
        super().setUp()
        self.snapshot_dir = self.root / "supervisor-continuation"
        self.snapshot_dir.mkdir(mode=0o700)
        self.review = self.snapshot_dir / "review.json"
        self.operator = self.snapshot_dir / "approval.json"
        here = patch.object(C, "HERE", self.snapshot_dir / C.HERE.name)
        here.start()
        self.addCleanup(here.stop)
        self.first = dict(binding=self.bound, role="independent-review", decision="approved",
                          actor="independent-assurance", independent=True, verdict="PASS",
                          issued_at=self.wall - 1, expires_at=self.wall + 1799)
        self.second = dict(binding=self.bound, role="operator", decision="approved", actor="operator",
                           issued_at=self.wall - 1, expires_at=self.wall + 1799)

    def authorize(self):
        M.durable(self.review, self.first)
        review_digest = M.sha(M.read(self.review))
        self.second["review_sha256"] = review_digest
        M.durable(self.operator, self.second)
        C.authorize(self.inputs, self.bound, self.review, review_digest,
                    self.operator, M.sha(M.read(self.operator)))

    def test_fresh_receipts_valid_but_do_not_renew_end(self):
        self.authorize()
        self.assertGreater(self.first["expires_at"], self.anchor["wall_end"])
        self.assertEqual(self.bound["anchor"]["monotonic_end"], C.ENDPOINTS["monotonic_end"])
        self.assertEqual(self.bound["anchor"]["wall_end"], C.ENDPOINTS["wall_end"])

    def test_expired_receipt(self):
        self.first["expires_at"] = self.wall
        with self.assertRaises(Exception):
            self.authorize()

    def test_stale_receipt(self):
        self.first["issued_at"] = self.wall - 1801
        with self.assertRaises(Exception):
            self.authorize()

    def test_wrong_exact_binding(self):
        self.first["binding"] = {"wrong": True}
        with self.assertRaises(Exception):
            self.authorize()

    def test_not_independent(self):
        self.first["independent"] = False
        with self.assertRaises(Exception):
            self.authorize()

    def test_same_actor_rejected(self):
        self.second["actor"] = self.first["actor"]
        with self.assertRaises(Exception):
            self.authorize()

    def test_builder_rejected(self):
        self.first["actor"] = "live-transport-renewed-continue-builder"
        with self.assertRaises(Exception):
            self.authorize()

    def test_receipts_outside_snapshot_rejected(self):
        self.review = self.root / "wrong-review.json"
        with self.assertRaises(Exception):
            self.authorize()

    def test_receipt_hash_drift_rejected(self):
        self.authorize()
        with self.assertRaises(Exception):
            C.authorize(self.inputs, self.bound, self.review, "0" * 64,
                        self.operator, M.sha(M.read(self.operator)))


if __name__ == "__main__":
    unittest.main()
