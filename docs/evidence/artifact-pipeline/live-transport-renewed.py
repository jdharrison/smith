#!/usr/bin/env python3
"""Separate, one-shot renewed no-model transport supervisor (source only).

Local file-only API: Inputs(root=..., installed=...), stage(inputs, run_id),
check(inputs). stage exclusively creates run-02; check returns its review binding
without writes or engine commands. Production CLI: stage --run-id <32 lower hex>,
check, or run --review FILE --review-sha256 SHA --approval FILE
--approval-sha256 SHA. Every mode requires --root CHECKOUT (use --root . from
checkout when executing the parent's private supervisor-02 source snapshot).
No root discovery or fallback scanning. No receipt creation API. Parent authenticates actors outside
this process; receipts are attestations, not signatures or a same-UID boundary.
Review uses the original receipt schema plus independent:true, verdict:"PASS";
operator uses review_sha256. Both bind check output, are <=30min old and unexpired.
Do not use this builder as reviewer/operator. Review and renewed operator signoff
must explicitly cover the scope's retained previous Unknown and privileged-engine
containment limitation. Old receipts/expiry are evidence only, never authority.

A durable consumed record starts a NEW 900-second wall+monotonic window before
commands, including 60 seconds reserved cleanup. Markers never reset or resume.
Only the exact stopped owned CID may start, one fresh Execute may dispatch, and
that CID must stop even on ambiguous start, drift, cancellation or expired work
authority. All captures use the immutable Commands process-group join contract.
Missing/replaced cgroup scope is Unknown, not proof of cleanup. Old Unknown is
never resolved retroactively. SIGKILL/host loss retains uncertainty and fences;
cooperative deadlines cannot interrupt blocked kernel I/O or confine hostile root.
No pull/create/Setup/image/model/auth/repository/installed operation is available.
"""
import argparse
import copy
import hashlib
import importlib.util
import os
from pathlib import Path
import re
import signal
import sys
import time

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve()
PINS = {
    "live-transport-qa.py": "3f6f16e7aef5a0264d7d3a0d2695e4c212eb2a37214ccd1e88904970edbec6bc",
    "live-transport-continue.py": "4306dd47b486c26761eaddc7b6d9312ed132716f84ceb799e13bd7dba1912f47",
    "owned_cgroup_verifier.py": "af1e8e5197a295e127490904cc4fc8358bd1e12d8425089d9a884fcdcfa6d460",
}


def load(name, filename):
    path = HERE.with_name(filename)
    if hashlib.sha256(path.read_bytes()).hexdigest() != PINS[filename]:
        raise RuntimeError("immutable_source_mismatch")
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


M = load("renewed_original_helpers", "live-transport-qa.py")
C = load("renewed_continuation_evidence", "live-transport-continue.py")
V = load("renewed_owned_cgroup", "owned_cgroup_verifier.py")
CID, IMAGE = C.CID, C.IMAGE
SCOPE = dict(M.SCOPE, engine_cid=CID, engine_image_id=IMAGE, staging="run-02",
             previous_uncertainty="acknowledged_not_resolved_retroactively",
             privileged_engine_containment="unresolved_not_a_hostile_root_sandbox",
             original_window="not_reused", no_pull_create_image_repo=True)
MARKERS = ("consumed.json", "pending.json", "running.json", "dispatch.json", "result.json")
BUILDERS = {"builder", "live-transport-qa-builder", "live-transport-continue-builder",
            "live-transport-renewed-builder"}


class Refusal(Exception):
    def __init__(self, reason):
        self.reason = reason
        super().__init__(reason)


def require(value, reason):
    if not value:
        raise Refusal(reason)


def stopped(value):
    state = value["State"]
    require(state["Status"] == "exited" and state["Running"] is False
            and type(state["Pid"]) is int and state["Pid"] == 0, "not_stopped")


def code_hashes():
    names = (*PINS, "test_live_transport_qa.py", "test_live_transport_continue.py",
             "test_owned_cgroup_verifier.py", HERE.name, "test_live_transport_renewed.py")
    result = {}
    for name in names:
        path = HERE.with_name(name)
        M.safe(path)
        result[name] = M.sha(M.read(path))
    require(all(result[name] == digest for name, digest in PINS.items()), "source_drift")
    return result


class Inputs:
    def __init__(self, root=M.ROOT, installed=None):
        self.root = Path(root)
        self.base = self.root / ".smith-artifact-functional-qa"
        self.old = self.base / "run-01"
        self.work = self.base / "run-02"
        self.cli = self.base / "dagger"
        # Reuse the already prepared snapshot, not generated SDK/source repair.
        self.module = self.old / "module"
        self.source = self.root / "pipeline"
        self.installed = Path(installed) if installed is not None else Path.home() / ".local/bin/dagger"

    def history(self):
        files = M.tree(self.old, True)
        for name, digest in C.PINS.items():
            require(files.get(name) == digest, "history_drift")
        require(files.get("capture-008.out") == C.CAPTURE_PIN, "history_drift")
        owned = M.decode(M.read(self.old / "owned.json"))
        require(owned["cid"] == CID and owned["image"] == IMAGE, "ownership")
        require(owned["nonce"] == M.decode(M.read(self.old / "pending.json"))["nonce"]
                == M.decode(M.read(self.old / "consumed.json"))["nonce"], "ownership")
        require(M.decode(M.read(self.old / "result.json"))["cleanup"] == "unknown"
                and M.decode(M.read(self.old / "continuation-result.json"))["cleanup"] == "unknown",
                "history_uncertainty")
        require(not os.path.lexists(self.old / "dispatch.json")
                and not os.path.lexists(self.old / "continuation-dispatch.json"), "previous_dispatch")
        require(M.decode(M.read(self.old / "engine.json")) == M.CONFIG, "engine_config")
        return files

    def prerequisites(self):
        """Read-only source/input validation, independent of run-02 existence."""
        code = code_hashes()
        M.safe(self.cli)
        M.safe(self.installed)
        require(os.access(self.cli, os.X_OK), "executable")
        history = self.history()
        source = M.tree(self.source)
        module = M.sha(M.encoded(M.tree(self.module, True)))
        require(M.decode(M.read(self.module / "dagger.json"))["engineVersion"] == "v0.21.9", "module_version")
        preflight = self.root / "docs/evidence/artifact-pipeline/functional-qa-readonly-preflight.json"
        raw = M.read(preflight)
        installed = M.sha(M.read(self.installed, 128 * 1024 * 1024))
        require(installed == M.decode(raw)["installed_dagger"]["sha256"], "installed_drift")
        return {"original_files": history, "code": code, "source": source, "module": module,
                "cli": M.sha(M.read(self.cli, 128 * 1024 * 1024)), "installed_cli": installed,
                "preflight": M.sha(raw)}

    def snapshot(self):
        prepared = self.prerequisites()
        M.safe(self.work, True)
        for name in ("home", "config", "cache", "docker", "docker/config.json", "request.json"):
            M.safe(self.work / name, True)
        require(M.decode(M.read(self.work / "docker/config.json")) == {}, "docker_config")
        request = M.read(self.work / "request.json", 32768)
        value = M.decode(request)
        old_request = M.decode(M.read(self.old / "request.json", 32768))
        require(re.fullmatch(r"[0-9a-f]{32}", value["run_id"]) is not None
                and value["run_id"] != old_request["run_id"], "request_identity")
        expected = M.decode(M.expected_request(prepared, M.read(self.source / "fixtures/clamp/clamp.py")))
        expected.update(run_id=value["run_id"], contract_sha256=M.sha(M.encoded(SCOPE)))
        require(request == M.encoded(expected), "request_drift")
        return {"scope": SCOPE, **prepared, "request": M.sha(request), "run_id": value["run_id"],
                "docker_config": M.sha(M.read(self.work / "docker/config.json")),
                "destination": M.sha(str(self.work.absolute()).encode())}

    def unchanged(self, bound):
        raw = M.read(self.work / "staged.json")
        require(M.sha(raw) == bound["staging"] and M.decode(raw) == self.snapshot()
                == {k: v for k, v in bound.items() if k != "staging"}, "binding_drift")


def pristine(inputs):
    require(not any(os.path.lexists(inputs.work / name) for name in (*MARKERS, "captures")), "already_consumed")
    require(all(not list((inputs.work / name).iterdir()) for name in ("home", "config", "cache")), "not_fresh")
    require({p.name for p in (inputs.work / "docker").iterdir()} == {"config.json"}, "not_fresh")


def stage(inputs, run_id):
    """Local-only, exclusive staging. Partial failures are retained, never repaired."""
    require(type(run_id) is str and re.fullmatch(r"[0-9a-f]{32}", run_id), "request_identity")
    M.safe(inputs.base, True)
    prepared = inputs.prerequisites()
    require(run_id != M.decode(M.read(inputs.old / "request.json"))["run_id"], "request_identity")
    request = M.decode(M.expected_request(prepared, M.read(inputs.source / "fixtures/clamp/clamp.py")))
    request.update(run_id=run_id, contract_sha256=M.sha(M.encoded(SCOPE)))
    require(len(M.encoded(request)) <= 32768, "request_bound")
    inputs.work.mkdir(mode=0o700)  # No exist_ok, replacement or cleanup.
    for name in ("home", "config", "cache", "docker"):
        (inputs.work / name).mkdir(mode=0o700)
    M.durable(inputs.work / "docker/config.json", {})

    M.durable(inputs.work / "request.json", request)
    M.durable(inputs.work / "staged.json", inputs.snapshot())
    return check(inputs)


def check(inputs):
    """Read-only binding check; never calls Commands or Docker."""
    pristine(inputs)
    raw = M.read(inputs.work / "staged.json")
    bound = dict(M.decode(raw), staging=M.sha(raw))
    inputs.unchanged(bound)
    return bound


def authorize(bound, review, review_sha256, operator, operator_sha256):
    now = time.time()
    first = M.receipt(review, review_sha256, "independent-review", bound, now)
    second = M.receipt(operator, operator_sha256, "operator", bound, now, review_sha256)
    require(first.get("verdict") == "PASS" and first["actor"] != second["actor"]
            and not {first["actor"], second["actor"]} & BUILDERS, "approval")


class Renewed(M.Supervisor):
    def __init__(self, inputs, bound, command=None, capture=V.capture):
        super().__init__(inputs, command=command)
        self.bound, self.capture = bound, capture
        self.cid, self.image = CID, IMAGE
        owned = M.decode(M.read(inputs.old / "owned.json"))
        self.nonce = owned["nonce"]
        self.baseline = M.decode(M.read(inputs.old / "capture-008.out"))[0]
        self.held = None
        self.anchor = None
        self.stage_name = "admission"
        self.approval = None
        if command is None:
            self.command = M.Commands(self.work / "captures", self.env, lambda: self.cancelled)

    def execution_config(self, value):
        require(value["HostConfig"]["NetworkMode"] == "bridge"
                and value["HostConfig"]["CgroupParent"] == "", "resource_contract")
        normalized = copy.deepcopy(value)
        normalized["HostConfig"]["NetworkMode"] = "default"
        # Original contract expects the run-01 engine.json mount, NOT run-02.
        old_work = self.work
        try:
            self.work = self.inputs.old
            super().execution_config(normalized)
        finally:
            self.work = old_work
        require(all(value[key] == self.baseline[key]
                    for key in ("Config", "HostConfig", "Mounts", "Path", "Args")), "config_drift")
        return value

    def gate(self):
        require(not self.cancelled, "cancelled")
        if self.anchor is not None:
            require(self.anchor == M.decode(M.read(self.work / "consumed.json")), "consumed_drift")
            require(self.anchor["wall_start"] <= time.time() < self.anchor["wall_end"] - 60
                    and time.monotonic() < self.work_end, "deadline")
        self.inputs.unchanged(self.bound)
        self.approval()
        require(not self.cancelled, "cancelled")
        if self.anchor is not None:
            require(self.anchor["wall_start"] <= time.time() < self.anchor["wall_end"] - 60
                    and time.monotonic() < self.work_end, "deadline")

    def docker(self, *args, cleanup=False, long=False):
        if not cleanup:
            self.gate()
        return super().docker(*args, cleanup=cleanup, long=long)

    def observe(self, pid, cleanup=False):
        return self.capture(pid, self.cid,
                            deadline=min(self.end if cleanup else self.work_end, time.monotonic() + 5),
                            run_deadline=self.end,
                            cancelled=(lambda: False) if cleanup else (lambda: self.cancelled))

    def cleanup(self):
        """Stop is unconditional once owed; no authorization/config/source gate."""
        proof, config_ok = False, False
        fresh = None
        try:
            try:
                before = self.inspect_owned(cleanup=True)
                self.execution_config(before)
                config_ok = True
            except Exception:
                before = None
            # Recapture independently even when execution configuration has drifted.
            if before is None:
                try:
                    before = self.inspect_owned(cleanup=True)
                except Exception:
                    pass
            if before is not None and before["State"]["Running"] is True:
                try:
                    pid = before["State"]["Pid"]
                    fresh = self.observe(pid, cleanup=True)
                    if self.held is None:
                        self.held, fresh = fresh, None
                        self.pid = pid
                    proof = self.pid == pid and (fresh is None or fresh.identity == self.held.identity)
                except Exception:
                    proof = False
        finally:
            try:
                if fresh is not None:
                    fresh.close()
            finally:
                # Even failed observation/close cannot suppress the stop attempt.
                self.docker("stop", "--time", "10", self.cid, cleanup=True)
        after = self.inspect_owned(cleanup=True)
        self.execution_config(after)
        stopped(after)
        require(config_ok and proof and self.held is not None, "cleanup_identity")
        require(self.held.quiescent(deadline=min(self.end, time.monotonic() + 5),
                                    cancelled=lambda: False) is True, "scope_populated")
        require(not getattr(self.command, "unknown", False), "process_group_unknown")
        require(time.monotonic() < self.end and self.anchor["wall_start"] <= time.time()
                < self.anchor["wall_end"], "cleanup_deadline")

    def run(self, bound, approval):
        result = {"status": "failed", "cleanup": "not_started", "stage": "admission", "reason": "refused"}
        marked, owed = False, False
        self.approval = approval
        try:
            require(bound == self.bound, "binding_drift")
            self.gate()
            pristine(self.inputs)
            wall, mono = time.time(), time.monotonic()
            self.end, self.work_end = mono + 900, mono + 840
            self.anchor = {"binding": bound, "wall_start": wall, "wall_end": wall + 900,
                           "monotonic_start": mono, "monotonic_end": self.end}
            M.durable(self.work / "consumed.json", self.anchor)
            marked = True
            M.durable(self.work / "pending.json", {"cid": self.cid, "nonce": self.nonce})
            (self.work / "captures").mkdir(mode=0o700)
            self.stage_name = "ownership"
            stopped(self.inspect())
            volume = M.decode(self.docker("volume", "inspect", M.VOLUME))
            require(type(volume) is list and len(volume) == 1 and volume[0]["Name"] == M.VOLUME
                    and volume[0]["Labels"].get(M.LABEL) == self.nonce
                    and volume[0]["Driver"] == "local" and not volume[0].get("Options"), "volume_ownership")
            stopped(self.inspect())
            self.stage_name = "start"
            self.gate()
            owed = True  # Before start: an ambiguous start still incurs cleanup.
            self.docker("start", self.cid)
            self.stage_name = "capture"
            live = self.inspect()
            require(live["State"]["Running"] is True and type(live["State"]["Pid"]) is int
                    and live["State"]["Pid"] > 1, "runtime_identity")
            self.pid = live["State"]["Pid"]
            self.held = self.observe(self.pid)
            M.durable(self.work / "running.json", {"pid": self.pid, "identity": self.held.identity})
            require(self.inspect()["State"]["Pid"] == self.pid, "runtime_identity")
            self.env["_EXPERIMENTAL_DAGGER_RUNNER_HOST"] = "container+docker://" + self.cid
            self.stage_name = "dispatch"
            self.gate()
            M.durable(self.work / "dispatch.json", {"request": bound["request"], "run_id": bound["run_id"]})
            self.gate()
            rc, raw, _ = self.command([str(self.inputs.cli), "--mod", str(self.inputs.module),
                                      "call", "run-fixture", "--request", str(self.work / "request.json")],
                                     self.work_end, cap=65536)
            require(not getattr(self.command, "unknown", False), "process_group_unknown")
            require(rc == 0 and M.decode(raw) == {"schema_version": 1, "request_sha256": bound["request"],
                                                "outcome": "failed", "error": "policy_denied"}
                    and type(M.decode(raw)["schema_version"]) is int, "transport_envelope")
            self.gate()
            result.update(status="policy_denied", stage="complete", reason="expected_policy_denied")
        except Exception as error:
            result.update(stage=self.stage_name, reason=reason(error))
        finally:
            if owed:
                result["cleanup"] = "unknown"
                try:
                    self.cleanup()
                    result["cleanup"] = "verified_stopped"
                except Exception as error:
                    result.update(status="failed", stage="cleanup", reason=reason(error))
            if self.held is not None:
                try:
                    self.held.close()
                except Exception:
                    result.update(status="failed", cleanup="unknown", stage="cleanup", reason="close_failed")
            if self.cancelled:
                result.update(status="failed", stage="cancelled", reason="cancelled")
            if marked:
                try:
                    M.durable(self.work / "result.json", dict(result, binding=bound,
                              previous_cleanup="unknown_not_retroactively_resolved"))
                except Exception:
                    result.update(status="failed", cleanup="unknown", stage="record", reason="record_failed")
        return result


def reason(error):
    if isinstance(error, Refusal):
        return error.reason
    if isinstance(error, V.Refusal):
        return "cgroup_" + error.code.value
    return "invalid_or_unavailable"  # Never stringify raw diagnostics/paths/secrets.


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("mode", choices=("stage", "check", "run"))
    parser.add_argument("--root", type=Path, required=True, help="Explicit checkout root; never inferred from the snapshot")
    parser.add_argument("--run-id")
    parser.add_argument("--review", type=Path)
    parser.add_argument("--review-sha256")
    parser.add_argument("--approval", type=Path)
    parser.add_argument("--approval-sha256")
    args = parser.parse_args()
    os.umask(0o077)
    try:
        inputs = Inputs(root=args.root.resolve())
        if args.mode == "stage":
            print(M.encoded(stage(inputs, args.run_id)).decode())
            return 0
        bound = check(inputs)
        if args.mode == "check":
            print(M.encoded(bound).decode())
            return 0
        require(all((args.review, args.review_sha256, args.approval, args.approval_sha256)), "approval")
        approval = lambda: authorize(bound, args.review, args.review_sha256, args.approval, args.approval_sha256)
        approval()
        supervisor = Renewed(inputs, bound)
        def cancel(_sig, _frame):
            supervisor.cancelled = True
        for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            signal.signal(sig, cancel)
        result = supervisor.run(bound, approval)
        print(M.encoded(result).decode())
        return 0 if result["status"] == "policy_denied" and result["cleanup"] == "verified_stopped" else 1
    except Exception as error:
        print(M.encoded({"status": "refused", "stage": args.mode, "reason": reason(error)}).decode())
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
