#!/usr/bin/env python3
"""Source-only continuation; never renew the consumed run-02 window.

Copy these two new files alongside the immutable original renewed sources in a
separate private supervisor snapshot. stage/check --root CHECKOUT are identical,
read-only binding operations: no staging writes and no engine commands. Parent
must independently review that binding and authenticate fresh review/operator
receipts in that separate snapshot (original receipt schema, <=1800 seconds).
run additionally requires --review/--review-sha256/--approval/--approval-sha256.
The existing run-02 request is used unchanged. Only continuation-* records and
captures are created, exclusively. One start, one Execute->policy_denied, stop;
no pull/create/image/Setup/model/auth/repository/installed effects. Receipt
freshness never extends the original wall or monotonic endpoint; 60 seconds
remain reserved for cleanup. Old uncertainty is not retroactively resolved.
"""
import argparse
import copy
import hashlib
import importlib.util
import os
from pathlib import Path
import signal
import sys
import time

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve()
ORIGINAL_PIN = "607a5161ad0715112f9def06775209198677b168546d1f5357e92fe7b8ab241f"
source = HERE.with_name("live-transport-renewed.py")
if hashlib.sha256(source.read_bytes()).hexdigest() != ORIGINAL_PIN:
    raise RuntimeError("immutable_source_mismatch")
spec = importlib.util.spec_from_file_location("immutable_renewed_continuation_parent", source)
R = importlib.util.module_from_spec(spec)
spec.loader.exec_module(R)
M, require = R.M, R.require
Inputs = R.Inputs
PINS = {
    "approval.json": "9f993e85c4ddf0c368776985899b24e5942f1085cbc61086c3263317d22d0acd",
    "captures/capture-001.err": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "captures/capture-001.out": "6d3c81e4ac805379a9d9d73928345ba644a063890b412a2b0c79d4209b3ff945",
    "consumed.json": "8c1147a9cf5b1a89b673d368def9b3494623b5bafd533ef473e05949701de0b8",
    "docker/config.json": "44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a",
    "pending.json": "7a56b49e0ddb21c105ef9f995f3711554e2384347cb85b2ff8f607937a07b232",
    "request.json": "fcaa215e472a9339e44fc9ee0d95c57ff9507649168b368622cdf074d499842b",
    "result.json": "ca993243d528c2def981f3a4140cbb9ff2839f5c804898a0529b74d978192de7",
    "review.json": "7d9363467bbb4e31879c922a03bf92d42ca4a25d3f2927313307178ac28662e7",
    "staged.json": "4a01c19c62c3f3abb8415b20d6db6440406b22dde7cf89f5812748476fdc87a3",
}
ENDPOINTS = {"wall_start": 1792549237.173685, "wall_end": 1792550137.173685,
             "monotonic_start": 43511.880761323, "monotonic_end": 44411.880761323}
SCOPE = {"original_window": "consumed_run02_not_reset", "cleanup_seconds": 60,
         "oom_normalization": "exact_false_or_null_only", "attempts": 1,
         "operation": "same_request_execute_policy_denied_then_stop",
         "previous_uncertainty": "not_retroactively_resolved"}
NAMES = ("continuation-consumed.json", "continuation-pending.json", "continuation-running.json",
         "continuation-dispatch.json", "continuation-result.json", "continuation-captures")


def own_hashes():
    result = {}
    for path in (HERE, HERE.with_name("test_live_transport_renewed_continue.py")):
        M.safe(path)
        result[path.name] = M.sha(M.read(path))
    return result


def originals(inputs, allow_workspace=False):
    """Check every old run-02 file; ignore only the named new records at runtime."""
    M.safe(inputs.work, True)
    files = {}
    directories = []
    for path in sorted(inputs.work.rglob("*")):
        relative = path.relative_to(inputs.work)
        if relative.parts[0] in NAMES:
            continue
        M.safe(path, True)
        require(path.is_file() or path.is_dir(), "original_drift")
        # Only the dispatched CLI may populate its initially empty private state.
        # Existing files (including docker config) remain pinned without exception.
        if (allow_workspace and len(relative.parts) > 1
                and relative.parts[0] in ("home", "config", "cache")
                and relative.as_posix() not in PINS):
            continue
        if path.is_file():
            files[relative.as_posix()] = M.sha(M.read(path))
        else:
            directories.append(relative.as_posix())
    require(files == PINS and directories == ["cache", "captures", "config", "docker", "home"],
            "original_drift")
    anchor = M.decode(M.read(inputs.work / "consumed.json"))
    require({k: anchor[k] for k in ENDPOINTS} == ENDPOINTS, "original_deadline_drift")
    original = anchor["binding"]
    inputs.unchanged(original)  # NOT R.check(): its pristine check is already consumed.
    require(M.decode(M.read(inputs.work / "result.json")) == dict(
        status="failed", cleanup="not_started", stage="ownership", reason="config_drift",
        binding=original, previous_cleanup="unknown_not_retroactively_resolved"), "original_result")
    pending = M.decode(M.read(inputs.work / "pending.json"))
    owned = M.decode(M.read(inputs.old / "owned.json"))
    require(pending == {"cid": R.CID, "nonce": owned["nonce"]}, "ownership")
    return anchor


def pristine(inputs):
    require(not any(inputs.work.glob("continuation-*")), "already_consumed")


def check(inputs):
    pristine(inputs)
    anchor = originals(inputs)
    return {"original": anchor["binding"], "anchor": anchor, "run02_files": dict(PINS),
            "continuation_code": own_hashes(), "scope": SCOPE}


stage = check  # Both modes deliberately read-only, including after expiry.


def authorize(inputs, bound, review, review_sha256, operator, operator_sha256):
    # Fresh receipts belong to the separate supervisor snapshot, never bound run-02.
    M.safe(HERE.parent, True)
    require(not HERE.parent.is_relative_to(inputs.work)
            and not HERE.parent.is_relative_to(inputs.old), "receipt_location")
    for path in (review, operator):
        require(path.absolute().parent == HERE.parent and not path.is_symlink(), "receipt_location")
    R.authorize(bound, review, review_sha256, operator, operator_sha256)
    for path in (review, operator):
        require(M.decode(M.read(path))["actor"] != "live-transport-renewed-continue-builder", "approval")


class Continuation(R.Renewed):
    def __init__(self, inputs, bound, command=None, capture=R.V.capture):
        super().__init__(inputs, bound, command=command, capture=capture)
        self.anchor = copy.deepcopy(bound["anchor"])
        self.end = self.anchor["monotonic_end"]
        self.work_end = self.end - 60
        self.last_wall = self.anchor["wall_start"]
        self.consumed = None
        self.dispatched = False
        if command is None:
            self.command = M.Commands(self.work / "continuation-captures", self.env, lambda: self.cancelled)

    def execution_config(self, value):
        baseline = self.baseline
        normalized = copy.deepcopy(value)
        normalized_baseline = copy.deepcopy(baseline)
        for item in (normalized, normalized_baseline):
            host = item["HostConfig"]
            require("OomKillDisable" in host and (host["OomKillDisable"] is False
                    or host["OomKillDisable"] is None), "oom_contract")
            host["OomKillDisable"] = False
        try:
            self.baseline = normalized_baseline
            super().execution_config(normalized)
        finally:
            self.baseline = baseline
        return value

    def gate(self):
        def deadline():
            wall, mono = time.time(), time.monotonic()
            require(not self.cancelled, "cancelled")
            require(self.last_wall <= wall < self.anchor["wall_end"] - 60
                    and self.anchor["monotonic_start"] <= mono < self.work_end, "deadline")
            self.last_wall = wall
        deadline()
        require(originals(self.inputs, allow_workspace=self.dispatched) == self.anchor
                and self.bound["original"] == self.anchor["binding"]
                and self.bound["run02_files"] == PINS and self.bound["scope"] == SCOPE
                and own_hashes() == self.bound["continuation_code"], "binding_drift")
        if self.consumed is None:
            pristine(self.inputs)
        else:
            require(M.decode(M.read(self.work / "continuation-consumed.json")) == self.consumed,
                    "consumed_drift")
        self.approval()
        deadline()

    def run(self, bound, approval):
        result = {"status": "failed", "cleanup": "not_started", "stage": "admission", "reason": "refused"}
        marked, owed = False, False
        self.approval = approval
        try:
            require(bound == self.bound, "binding_drift")
            self.gate()
            self.consumed = {"binding": bound, "anchor": self.anchor}
            M.durable(self.work / "continuation-consumed.json", self.consumed)
            marked = True
            M.durable(self.work / "continuation-pending.json", {"cid": self.cid, "nonce": self.nonce})
            (self.work / "continuation-captures").mkdir(mode=0o700)
            self.stage_name = "ownership"
            R.stopped(self.inspect())
            volume = M.decode(self.docker("volume", "inspect", M.VOLUME))
            require(type(volume) is list and len(volume) == 1 and volume[0]["Name"] == M.VOLUME
                    and volume[0]["Labels"].get(M.LABEL) == self.nonce
                    and volume[0]["Driver"] == "local" and not volume[0].get("Options"), "volume_ownership")
            R.stopped(self.inspect())
            self.stage_name = "start"
            self.gate()
            owed = True
            self.docker("start", self.cid)
            self.stage_name = "capture"
            live = self.inspect()
            require(live["State"]["Running"] is True and type(live["State"]["Pid"]) is int
                    and live["State"]["Pid"] > 1, "runtime_identity")
            self.pid = live["State"]["Pid"]
            self.held = self.observe(self.pid)
            M.durable(self.work / "continuation-running.json", {"pid": self.pid, "identity": self.held.identity})
            require(self.inspect()["State"]["Pid"] == self.pid, "runtime_identity")
            self.env["_EXPERIMENTAL_DAGGER_RUNNER_HOST"] = "container+docker://" + self.cid
            self.stage_name = "dispatch"
            self.gate()
            M.durable(self.work / "continuation-dispatch.json", {"request": bound["original"]["request"],
                                                               "run_id": bound["original"]["run_id"]})
            self.dispatched = True
            self.gate()
            rc, raw, _ = self.command([str(self.inputs.cli), "--mod", str(self.inputs.module),
                                      "call", "run-fixture", "--request", str(self.work / "request.json")],
                                     self.work_end, cap=65536)
            require(not getattr(self.command, "unknown", False), "process_group_unknown")
            require(rc == 0 and M.decode(raw) == {"schema_version": 1,
                    "request_sha256": bound["original"]["request"], "outcome": "failed", "error": "policy_denied"}
                    and type(M.decode(raw)["schema_version"]) is int, "transport_envelope")
            self.gate()
            result.update(status="policy_denied", stage="complete", reason="expected_policy_denied")
        except Exception as error:
            result.update(stage=self.stage_name, reason=R.reason(error))
        finally:
            if owed:
                result["cleanup"] = "unknown"
                try:
                    self.cleanup()  # Original stop/whole-scope observation, no approval gate.
                    result["cleanup"] = "verified_stopped"
                except Exception as error:
                    result.update(status="failed", stage="cleanup", reason=R.reason(error))
            if self.held is not None:
                try:
                    self.held.close()
                except Exception:
                    result.update(status="failed", cleanup="unknown", stage="cleanup", reason="close_failed")
            if self.cancelled:
                result.update(status="failed", stage="cancelled", reason="cancelled")
            if marked:
                try:
                    M.durable(self.work / "continuation-result.json", dict(result, binding=bound,
                              previous_cleanup="unknown_not_retroactively_resolved"))
                except Exception:
                    result.update(status="failed", cleanup="unknown", stage="record", reason="record_failed")
        return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("stage", "check", "run"))
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--review", type=Path)
    parser.add_argument("--review-sha256")
    parser.add_argument("--approval", type=Path)
    parser.add_argument("--approval-sha256")
    args = parser.parse_args()
    os.umask(0o077)
    try:
        inputs = Inputs(root=args.root.resolve())
        bound = check(inputs)
        if args.mode != "run":
            print(M.encoded(bound).decode())
            return 0
        require(all((args.review, args.review_sha256, args.approval, args.approval_sha256)), "approval")
        approval = lambda: authorize(inputs, bound, args.review, args.review_sha256,
                                      args.approval, args.approval_sha256)
        approval()
        supervisor = Continuation(inputs, bound)
        def cancel(_sig, _frame):
            supervisor.cancelled = True
        for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            signal.signal(sig, cancel)
        result = supervisor.run(bound, approval)
        print(M.encoded(result).decode())
        return 0 if result["status"] == "policy_denied" and result["cleanup"] == "verified_stopped" else 1
    except Exception as error:
        print(M.encoded({"status": "refused", "stage": args.mode, "reason": R.reason(error)}).decode())
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
