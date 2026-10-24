#!/usr/bin/env python3
"""Source-only, one-shot continuation of the outstanding run-01 effects.

Usage (check is local/read-only, including when the original window is expired):
  python3 -B docs/evidence/artifact-pipeline/live-transport-continue.py check
  python3 -B docs/evidence/artifact-pipeline/live-transport-continue.py run \
      --review FILE --review-sha256 SHA256
The parent must authenticate an independent PASS, then supply a private receipt
using the original receipt schema, binding equal to check output, independent:true,
verdict:"PASS", role:"independent-review". No builder may attest that receipt.
Original operator/review receipts remain unchanged; no renewed approval or window.
Only start/one original Execute/stop are available. No setup, pull, create, replay,
model/provider/auth/installed effects. Original captures and markers stay intact.
All new captures use run-01/continuation-captures (private, exclusively created).
Cancellation and strict bounded capture/reap use the pinned original helpers.
Receipts are parent attestations, not signatures or a same-UID security boundary.
"""
import argparse
import copy
import fcntl
import importlib.util
import os
from pathlib import Path
import signal
import sys
import time

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve()
PIN = "3f6f16e7aef5a0264d7d3a0d2695e4c212eb2a37214ccd1e88904970edbec6bc"
TEST_PIN = "c603d8bf647e7daa4aef6987ca92b75ae0c6e0bc0cc75200784a27a474891aec"
import hashlib
SOURCE = HERE.with_name("live-transport-qa.py")
if hashlib.sha256(SOURCE.read_bytes()).hexdigest() != PIN:
    raise RuntimeError("immutable helper mismatch")
SPEC = importlib.util.spec_from_file_location("immutable_live_transport_qa", SOURCE)
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)
CID = "0884539ca66f915d98ba20f3543c8cf33c07d865a0f0c619a7d2e0980ff01be3"
IMAGE = "sha256:3c4e0cedaea400d7e30155e050f94910ea5f381b6f5942dd2172382657628cb4"
ANCHOR_NS = 1792545470863107632
PINS = {
    "consumed.json": "ea50b2c7a2b630c3038a4c4a5a9b8ca4f38d0901a7861aa01de68485f786bac4",
    "pending.json": "2c0c9451aef45b8e1bda7818afb116aec2cb7afa304156e58e0f652325e4e578",
    "owned.json": "ae8739a239bb46b64706c610103edc7745997c277cb69c46d16411289684628e",
    "result.json": "3a2884f6185420060ed31048d7acf6b7cc1851448d14e096e2a0dda189f14a19",
    "review.json": "f77b98a990efe17c3aa748c5344a37eab53e12a4ddf9e158db68009fd236fdd8",
    "approval.json": "efbd53fedf2ddc7b637a94ee76b8f2a0006669827103bb3d35748097f8e10440",
}
CAPTURE_PIN = "99f2deaee1303381e2d3084a35135a3525b53bef5dc3c5bd89476f6b82829dc8"
ZERO = "0001-01-01T00:00:00Z"


def originals(inputs):
    values = {}
    for name, digest in PINS.items():
        path = inputs.work / name
        M.safe(path, True)
        raw = M.read(path)
        M.need(M.sha(raw) == digest)
        values[name] = M.decode(raw)
    M.need((inputs.work / "consumed.json").stat().st_mtime_ns == ANCHOR_NS)
    M.need(M.sha(M.read(SOURCE)) == PIN)
    M.need(M.sha(M.read(HERE.with_name("test_live_transport_qa.py"))) == TEST_PIN)
    return values


def pristine(work):
    for name in ("running.json", "dispatch.json"):
        M.need(not os.path.lexists(work / name))
    M.need(not any(work.glob("continuation-*")))


def binding(inputs):
    old = originals(inputs)
    original = inputs.binding()  # Original complete Inputs binding, no Docker.
    M.need(old["consumed.json"]["binding"] == original)
    owned = old["owned.json"]
    M.need(owned["cid"] == CID and owned["image"] == IMAGE)
    M.need(owned["nonce"] == old["consumed.json"]["nonce"] == old["pending.json"]["nonce"])
    M.need(old["result.json"] == {"binding": original, "status": "failed", "cleanup": "unknown"})
    pristine(inputs.work)
    captures = {}
    for name in ("capture-008.out", "capture-009.out", "capture-011.out"):
        M.safe(inputs.work / name, True)
        raw = M.read(inputs.work / name)
        M.need(M.sha(raw) == CAPTURE_PIN)
        value = M.decode(raw)[0]
        M.need(value["Id"] == CID and value["Image"] == IMAGE)
        created(value)
        captures[name] = M.sha(raw)
    return {"original": original, "original_files": PINS, "captures": captures,
            "anchor_ns": ANCHOR_NS, "safety_seconds": 5,
            "script": M.sha(M.read(HERE)),
            "tests": M.sha(M.read(HERE.with_name("test_live_transport_continue.py")))}


def created(value):
    state = value["State"]
    M.need(state["Status"] == "created" and state["Running"] is False)
    M.need(type(state["Pid"]) is int and state["Pid"] == 0 and state["StartedAt"] == ZERO)


def budget(work, now):
    anchor = (work / "consumed.json").stat().st_mtime_ns
    M.need(anchor == ANCHOR_NS and anchor / 1e9 <= now)
    expiry = M.decode(M.read(work / "approval.json"))["expires_at"]
    remaining = min(anchor / 1e9 + 900, expiry) - now - 5
    M.need(remaining > M.SCOPE["cleanup_seconds"])
    return remaining


def authorize(inputs, bound, review, digest):
    now = time.time()
    old = bound["original"]
    first = M.receipt(inputs.work / "review.json", PINS["review.json"], "independent-review", old, now)
    operator = M.receipt(inputs.work / "approval.json", PINS["approval.json"], "operator", old, now, PINS["review.json"])
    fresh = M.receipt(review, digest, "independent-review", bound, now)
    M.need(fresh.get("verdict") == "PASS")
    M.need(fresh["actor"] not in {operator["actor"], "live-transport-continue-builder", "builder"})
    M.need(first["actor"] != operator["actor"])


class Continuation(M.Supervisor):
    def __init__(self, inputs, bound, command=None, probe=M.cgroup, empty=M.empty_cgroup):
        super().__init__(inputs, command=command, probe=probe, empty=empty)
        self.bound = bound
        self.end = time.monotonic() + budget(self.work, time.time())
        self.work_end = self.end - M.SCOPE["cleanup_seconds"]
        owned = M.decode(M.read(self.work / "owned.json"))
        self.cid, self.image, self.nonce = owned["cid"], owned["image"], owned["nonce"]
        self.baseline = M.decode(M.read(self.work / "capture-008.out"))[0]
        self.capture_dir = self.work / "continuation-captures"
        if command is None:
            self.command = M.Commands(self.capture_dir, self.env, lambda: self.cancelled)

    def execution_config(self, value):
        M.need(value["HostConfig"]["NetworkMode"] == "bridge")
        # Only the audited default-vs-bridge representation is adapted, in memory.
        normalized = copy.deepcopy(value)
        normalized["HostConfig"]["NetworkMode"] = "default"
        super().execution_config(normalized)
        for key in ("Config", "HostConfig", "Mounts", "Path", "Args"):
            M.need(value[key] == self.baseline[key])
        return value

    def gate(self, approval):
        M.need(not self.cancelled and time.monotonic() < self.work_end)
        M.need(ANCHOR_NS / 1e9 <= time.time() < ANCHOR_NS / 1e9 + 895)
        for name in ("running.json", "dispatch.json"):
            M.need(not os.path.lexists(self.work / name))
        originals(self.inputs)
        self.inputs.unchanged(self.bound["original"])
        M.need(M.sha(M.read(HERE)) == self.bound["script"])
        M.need(M.sha(M.read(HERE.with_name("test_live_transport_continue.py"))) == self.bound["tests"])
        for name, digest in self.bound["captures"].items():
            M.need(M.sha(M.read(self.work / name)) == digest)
        approval()
        M.need(not self.cancelled and time.monotonic() < self.work_end)

    def run(self, bound, approval):
        result = {"status": "failed", "cleanup": "not_started"}
        lock, marked, owed = None, False, False
        try:
            M.need(bound == self.bound)
            self.gate(approval)
            pristine(self.work)
            lock = os.open(self.work / "continuation-lock", os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            M.durable(self.work / "continuation-consumed.json", {"binding": bound})
            marked = True
            self.capture_dir.mkdir(mode=0o700)
            M.durable(self.work / "continuation-pending.json", {"cid": self.cid, "nonce": self.nonce})
            self.gate(approval)
            live = self.inspect()
            created(live)
            volume = M.decode(self.docker("volume", "inspect", M.VOLUME))
            M.need(len(volume) == 1 and volume[0]["Name"] == M.VOLUME)
            M.need(volume[0]["Labels"].get(M.LABEL) == self.nonce)
            M.need(volume[0]["Driver"] == "local" and not volume[0].get("Options"))
            created(self.inspect())
            self.gate(approval)
            owed = True  # A failed/ambiguous start must still stop the exact owned CID.
            self.docker("start", self.cid)
            live = self.inspect()
            M.need(live["State"]["Running"] is True)
            self.pid = live["State"]["Pid"]
            self.cg = self.probe(self.pid)
            M.durable(self.work / "continuation-running.json", {"pid": self.pid, "cgroup": str(self.cg)})
            M.need(self.inspect()["State"]["Pid"] == self.pid)
            self.env["_EXPERIMENTAL_DAGGER_RUNNER_HOST"] = "container+docker://" + self.cid
            self.gate(approval)
            M.durable(self.work / "continuation-dispatch.json", {"request": bound["original"]["request"]})
            rc, raw, _ = self.command([str(self.inputs.cli), "--mod", str(self.inputs.module),
                                      "call", "run-fixture", "--request", str(self.work / "request.json")],
                                     self.work_end, cap=65536)
            value = M.decode(raw)
            M.need(rc == 0 and type(value["schema_version"]) is int and value == {
                "schema_version": 1, "request_sha256": bound["original"]["request"],
                "outcome": "failed", "error": "policy_denied"})
            self.gate(approval)
            result["status"] = "policy_denied"
        except Exception:
            result["status"] = "failed"
        finally:
            if owed:
                result["cleanup"] = "unknown"
                try:
                    before = self.inspect_owned(cleanup=True)
                    valid, proof = True, self.cg is not None
                    try:
                        self.execution_config(before)
                    except Exception:
                        valid = False
                    try:
                        if before["State"]["Running"]:
                            current = self.probe(before["State"]["Pid"])
                            if self.cg is None:
                                self.cg, self.pid = current, before["State"]["Pid"]
                            proof = current == self.cg and self.pid == before["State"]["Pid"]
                    except Exception:
                        proof = False
                    self.docker("stop", "--time", "10", self.cid, cleanup=True)
                    stopped = self.inspect_owned(cleanup=True)
                    self.execution_config(stopped)
                    M.need(stopped["State"]["Running"] is False and stopped["State"]["Pid"] == 0)
                    M.need(valid and proof and self.cg is not None and self.empty(self.cg))
                    M.need(not getattr(self.command, "unknown", False))
                    originals(self.inputs)
                    self.inputs.unchanged(bound["original"])
                    M.need(time.monotonic() < self.end)
                    result["cleanup"] = "verified_stopped"
                except Exception:
                    result["status"] = "failed"
            if self.cancelled:
                result["status"] = "failed"
            if marked:
                try:
                    M.durable(self.work / "continuation-result.json", dict(result, binding=bound))
                except Exception:
                    result = {"status": "failed", "cleanup": "unknown"}
            if lock is not None:
                os.close(lock)  # Permanent exclusive markers fence replay, including failed runs.
        return result


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("mode", choices=("check", "run"))
    parser.add_argument("--review", type=Path)
    parser.add_argument("--review-sha256")
    args = parser.parse_args()
    os.umask(0o077)
    try:
        inputs = M.Inputs()
        bound = binding(inputs)
        if args.mode == "check":
            print(M.encoded(bound).decode())
            return 0
        M.need(args.review is not None and args.review_sha256 is not None)
        approval = lambda: authorize(inputs, bound, args.review, args.review_sha256)
        approval()
        supervisor = Continuation(inputs, bound)
        def cancel(_sig, _frame):
            supervisor.cancelled = True
        for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            signal.signal(sig, cancel)
        result = supervisor.run(bound, approval)
        print(M.encoded(result).decode())
        return 0 if result == {"status": "policy_denied", "cleanup": "verified_stopped"} else 1
    except Exception:
        print('{"status":"refused"}')
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
