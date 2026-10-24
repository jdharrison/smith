"""One explicitly approved, never-started logging-failure resolution; no job replay."""
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
EVIDENCE = ROOT / "docs/evidence/dev-rootful-qa"
SUPPORT = EVIDENCE / "restore_service.py"
assert hashlib.sha256(SUPPORT.read_bytes()).hexdigest() == "d609474fbbcdf63c74e6129f278b4cd1e3893d6bd3d3b843fc81b2a3044bff51"
spec = importlib.util.spec_from_file_location("approved_restore_support", SUPPORT)
H = importlib.util.module_from_spec(spec)
spec.loader.exec_module(H)  # Its main is guarded; imports only definitions/constants.

REQUEST = "9a8d5ecb22fa4b85629e69f7a1e624b1"
CONTAINER = "0ca7c8870d282ec2100ba2ad8a0d8f0573c8500094a4fe9ec5a95f8ae79df05b"
IMAGE = "sha256:8f22784f23d66441888b41a154e6c768d35975bbfe5d54e4443e9b1c0eb0a4bc"
BINARY = "493616d4172236d160e2789697d53b3f72c30eac2bba804a42f5625e47d61288"
ERROR = "compression cannot be enabled when max file count is 1"
DOCKER = ["/usr/bin/docker", "--config", "/nonexistent/smith-docker-config", "--host", "unix:///run/docker.sock"]
PROGRESS = {"invocation_id": H.uuid.uuid4().hex, "request_id": REQUEST, "container_id": CONTAINER, "phase": "preflight", "service_stop_requested": False, "ledger_publish_attempted": False, "service_start_requested": False}
PROGRESS_OWNED = False


def write_evidence(path, value):
    staged = path.with_name(path.name + ".staging")
    with staged.open("xb") as output:
        output.write((json.dumps(value, indent=2) + "\n").encode())
        output.flush()
        os.fchmod(output.fileno(), 0o600)
        os.fsync(output.fileno())
    os.replace(staged, path)
    fd = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def claim_progress():
    global PROGRESS_OWNED
    H.validate_locks()
    assert not (EVIDENCE / "logging-reconciliation.json").exists()
    path = EVIDENCE / "logging-reconciliation-progress.json"
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600)
    PROGRESS_OWNED = True
    try:
        with os.fdopen(fd, "wb") as output:
            output.write((json.dumps(PROGRESS, indent=2) + "\n").encode())
            output.flush()
            os.fsync(output.fileno())
    finally:
        parent = os.open(EVIDENCE, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        try:
            os.fsync(parent)
        finally:
            os.close(parent)


def progress(phase, **facts):
    assert PROGRESS_OWNED
    path = EVIDENCE / "logging-reconciliation-progress.json"
    H.safe_file(path)
    existing = json.loads(path.read_text())
    assert existing["invocation_id"] == PROGRESS["invocation_id"]
    PROGRESS.update(facts, phase=phase)
    write_evidence(path, PROGRESS)


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("Duplicate JSON member")
        result[key] = value
    return result


def resolved_ledger(ledger):
    updated = copy.deepcopy(ledger)
    r = target(updated)
    r["cleanup_required"] = False
    r["summary"]["cleanup_verified"] = True
    r["summary"]["status"] = "failed"
    reverse = copy.deepcopy(updated)
    original = next(x for x in ledger["records"] if x["summary"]["request_id"] == REQUEST)
    changed = next(x for x in reverse["records"] if x["summary"]["request_id"] == REQUEST)
    for key in ["cleanup_verified", "status"]:
        changed["summary"][key] = original["summary"][key]
    changed["cleanup_required"] = original["cleanup_required"]
    assert reverse == ledger
    encoded = (json.dumps(updated, ensure_ascii=False, separators=(",", ":")) + "\n").encode()
    assert len(encoded) <= 4 * 1024 * 1024
    return encoded


def validate_ledger_with_installed_reader(exe, roots, namespace, variants):
    for ledger_bytes, expected_status in variants:
        assert len(ledger_bytes) <= 4 * 1024 * 1024
        json.loads(ledger_bytes, object_pairs_hook=unique_object)
        with tempfile.TemporaryDirectory(prefix="smith-reconcile-check-", dir=roots["--runtime-root"]) as temporary:
            base = Path(temporary)
            data, config, runtime = [base / name for name in ["data", "config", "runtime"]]
            for path in [data, config, runtime, data / "smith-next", data / "smith-next/bin"]:
                path.mkdir(mode=0o700)
            binary = data / "smith-next/bin/smith-service"
            shutil.copy2(exe, binary)
            for name in ["app-state.json", "dev-jobs.json"]:
                shutil.copy2(namespace / name, data / "smith-next" / name)
            local = data / "smith-next/dev-local-jobs.json"
            local.write_bytes(ledger_bytes)
            os.chmod(local, 0o600)
            child = subprocess.Popen(
                [str(binary), "serve", "--data-root", str(data), "--config-root", str(config), "--runtime-root", str(runtime)],
                env={"PATH": "/usr/bin:/bin", "HOME": str(base), "XDG_DATA_HOME": str(data), "XDG_CONFIG_HOME": str(config), "XDG_RUNTIME_DIR": str(runtime)},
                stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True,
            )
            try:
                H.wait_app(runtime, child)
                reply = H.exchange(runtime / "smith-next/app.sock", H.app_request(), child.pid)
                found = [j for j in reply["result"]["jobs"] if j["request_id"] == REQUEST]
                assert len(found) == 1 and found[0]["status"] == expected_status
                assert local.read_bytes() == ledger_bytes
            finally:
                if child.poll() is None:
                    child.terminate()
                try:
                    child.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    child.kill()  # Only our isolated, no-inference fixture child.
                    child.wait(timeout=5)
                    raise


def stopped_proof(group_root, sockets):
    assert H.property_value("ActiveState") == "inactive"
    assert H.property_value("SubState") == "dead" and H.property_value("MainPID") == "0"
    assert H.property_value("Job") in ["", "0"]
    assert H.property_value("NeedDaemonReload") == "no" and H.property_value("DropInPaths") == ""
    try:
        group_root.stat()
    except FileNotFoundError:
        pass
    else:
        for directory, _, _ in os.walk(group_root, onerror=H.fail_enumeration):
            assert not (Path(directory) / "cgroup.procs").read_text().strip()
    assert all(not os.path.lexists(path) for path in sockets)


def container_evidence():
    value = json.loads(subprocess.check_output(
        DOCKER + ["container", "inspect", CONTAINER], text=True, timeout=10,
        env={"PATH": "/usr/bin:/bin", "HOME": "/nonexistent", "LANG": "C"},
        stderr=subprocess.DEVNULL,
    ))
    assert len(value) == 1
    c = value[0]
    state, config = c["State"], c["Config"]
    assert c["Id"] == CONTAINER and c["Name"] == "/smith-dev-" + REQUEST
    assert c["Image"] == IMAGE
    labels = config["Labels"]
    assert labels["org.smith.dev.request"] == REQUEST
    assert labels["org.smith.dev.kind"] == "smith-receipt-worker-v1"
    assert re.fullmatch(r"[0-9a-f]{32}", labels["org.smith.dev.owner"])
    assert state["Status"] == "created" and state["Running"] is False and state["Pid"] == 0
    assert state["Paused"] is False and state["Restarting"] is False and state["Dead"] is False
    assert state["StartedAt"] == "0001-01-01T00:00:00Z" and state["ExitCode"] == 128
    assert ERROR in state["Error"]
    assert c["HostConfig"]["RestartPolicy"]["Name"] == "no"
    logs = c["HostConfig"]["LogConfig"]
    assert logs["Type"] == "local" and logs["Config"]["max-file"] == "1"
    assert logs["Config"].get("compress", "true") == "true"
    return (c["Id"], c["Image"], labels["org.smith.dev.owner"], state["Status"], state["StartedAt"], state["Error"], logs)


def target(ledger):
    records = [r for r in ledger["records"] if r["summary"]["request_id"] == REQUEST]
    assert len(records) == 1
    r = records[0]
    assert r["summary"]["status"] == "cleanup_unknown" and r["cleanup_required"] is True
    assert r["summary"]["cleanup_verified"] is False and r["may_have_sent"] is False
    assert r["image_id"] == IMAGE and r["failed_stage"] == "cloning"
    assert r["summary"]["reply_available"] is False and r["reply"] is None
    assert r["summary"]["session_id"] is None and r["message_id"] is None and r["repositories"] == []
    return r


def idle_except_target(namespace):
    app = json.loads((namespace / "app-state.json").read_text())
    assert all(j["operation"]["status"] not in ["queued", "running"] for j in app["jobs"])
    for name in ["dev-jobs.json", "dev-local-jobs.json"]:
        for r in json.loads((namespace / name).read_text())["records"]:
            if name == "dev-local-jobs.json" and r["summary"]["request_id"] == REQUEST:
                continue
            assert r["summary"]["status"] in ["failed", "completed", "recorded"]
            assert r["cleanup_required"] is False


def pair(namespace):
    selector = namespace / "client/current"
    release = selector.resolve()
    assert release.parent == namespace / "client/releases"
    manifest = release / ".smith-next-manifest.json"
    H.safe_file(manifest)
    payload = manifest.read_bytes()
    assert hashlib.sha256(payload).hexdigest() == release.name
    value = json.loads(payload)
    assert payload == (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()
    assert value["schema"] == 1
    for name, expected in value["entries"].items():
        assert not Path(name).is_absolute() and ".." not in Path(name).parts
        path = release / name
        meta = path.lstat()
        assert meta.st_uid == os.geteuid() and meta.st_mode & 0o022 == 0
        assert stat.S_IMODE(meta.st_mode) == expected["mode"]
        if expected["type"] == "file":
            H.safe_file(path)
            assert H.digest(path) == expected["sha256"]
        else:
            assert expected["type"] == "directory" and stat.S_ISDIR(meta.st_mode)
    helper = release / "smith_backend/smith-service"
    assert H.digest(helper) == BINARY and H.digest(namespace / "bin/smith-service") == BINARY
    return (os.readlink(selector), H.digest(manifest))


def main():
    assert H.BACKUP.resolve() == H.BACKUP
    for path in [*H.BACKUP.parents, H.BACKUP]:
        meta = path.lstat()
        assert stat.S_ISDIR(meta.st_mode) and meta.st_uid in [0, os.geteuid()]
        assert meta.st_mode & 0o022 == 0
    assert H.BACKUP.stat().st_uid == os.geteuid() and H.BACKUP.stat().st_mode & 0o077 == 0
    assert H.property_value("ActiveState") == "active" and H.property_value("UnitFileState") == "enabled"
    assert H.property_value("NeedDaemonReload") == "no" and H.property_value("DropInPaths") == ""
    assert H.property_value("Job") in ["", "0"]
    assert not (EVIDENCE / "logging-reconciliation.json").exists()
    assert not (EVIDENCE / "logging-reconciliation-progress.json").exists()
    pid = int(H.property_value("MainPID"))
    proc = Path("/proc") / str(pid)
    exe = (proc / "exe").resolve()
    assert H.digest(proc / "exe") == BINARY and H.digest(exe) == BINARY
    assert b"--log-opt=compress=false" in exe.read_bytes()
    args = [os.fsdecode(p) for p in (proc / "cmdline").read_bytes().split(b"\0") if p]
    assert len(args) == 8 and args[1] == "serve"
    assert args[2::2] == ["--data-root", "--config-root", "--runtime-root"]
    roots = dict(zip(args[2::2], map(Path, args[3::2])))
    namespace = roots["--data-root"] / "smith-next"
    assert exe == namespace / "bin/smith-service"
    root_facts = H.root_identities(roots)
    H.acquire_lock(roots["--data-root"] / ".smith-next-install.lock")
    H.acquire_lock(roots["--data-root"], directory=True)
    H.validate_locks()
    assert H.root_identities(roots) == root_facts
    claim_progress()
    selected = pair(namespace)
    unit = Path(H.property_value("FragmentPath"))
    H.safe_file(unit)
    unit_bytes = unit.read_bytes()
    assert "ExecStart=" + " ".join(args) in unit_bytes.decode().splitlines()
    effective = H.property_value("ExecStart")
    assert f"path={exe} ;" in effective and f"argv[]={' '.join(args)} ;" in effective
    ledger_path = namespace / "dev-local-jobs.json"
    H.safe_file(ledger_path)
    ledger_bytes = ledger_path.read_bytes()
    assert len(ledger_bytes) <= 4 * 1024 * 1024
    ledger = json.loads(ledger_bytes, object_pairs_hook=unique_object)
    assert ledger["schema_version"] == 1
    target(ledger)
    idle_except_target(namespace)
    evidence = container_evidence()
    before = {name: H.digest(namespace / name) for name in ["app-state.json", "dev-jobs.json", "dev-local-jobs.json"]}
    updated_bytes = resolved_ledger(ledger)
    validate_ledger_with_installed_reader(exe, roots, namespace, [(ledger_bytes, "cleanup_unknown"), (updated_bytes, "failed")])
    assert all(H.digest(namespace / name) == value for name, value in before.items())
    assert H.property_value("MainPID") == str(pid) and H.property_value("Job") in ["", "0"]
    assert pair(namespace) == selected and unit.read_bytes() == unit_bytes
    H.validate_locks()
    assert H.root_identities(roots) == root_facts
    idle_except_target(namespace)
    group = H.property_value("ControlGroup")
    assert group.startswith("/user.slice/") and group.endswith("/smith-next.service")
    assert ".." not in Path(group).parts and (proc / "cgroup").read_text() == f"0::{group}\n"
    progress("stop-requested", service_stop_requested=True)
    H.manager("stop")
    group_root = Path("/sys/fs/cgroup") / group.lstrip("/")
    sockets = [roots["--runtime-root"] / "smith-next" / name for name in ["service.sock", "app.sock"]]
    stopped_proof(group_root, sockets)
    progress("stopped-verified")
    H.acquire_lock(roots["--runtime-root"] / "smith-next/runner.lock")
    H.validate_locks()
    assert H.root_identities(roots) == root_facts and pair(namespace) == selected
    idle_except_target(namespace)
    assert all(H.digest(namespace / name) == value for name, value in before.items())
    assert container_evidence() == evidence
    snapshot = H.BACKUP / "before-logging-reconciliation"
    snapshot.mkdir(mode=0o700)
    shutil.copy2(exe, snapshot / "smith-service")
    shutil.copy2(unit, snapshot / "smith-next.service")
    for name in before:
        shutil.copy2(namespace / name, snapshot / name)
        os.chmod(snapshot / name, 0o600)
    snapshot_expected = {**before, "smith-service": BINARY, "smith-next.service": hashlib.sha256(unit_bytes).hexdigest()}
    for name, expected in snapshot_expected.items():
        path = snapshot / name
        H.safe_file(path)
        assert H.digest(path) == expected
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)
    for path in [snapshot, H.BACKUP]:
        fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)
    progress("snapshot-durable", original_ledger_sha256=hashlib.sha256(ledger_bytes).hexdigest())

    staged = ledger_path.with_name("dev-local-jobs.logging-reconciliation-staging")
    with staged.open("xb") as output:
        output.write(updated_bytes)
        output.flush()
        os.fchmod(output.fileno(), 0o600)
        os.fsync(output.fileno())
    H.validate_locks()
    assert H.root_identities(roots) == root_facts and pair(namespace) == selected
    assert ledger_path.read_bytes() == ledger_bytes and unit.read_bytes() == unit_bytes
    assert container_evidence() == evidence
    assert all(H.digest(namespace / name) == value for name, value in before.items())
    stopped_proof(group_root, sockets)
    progress("ledger-publish-requested", ledger_publish_attempted=True)
    os.replace(staged, ledger_path)
    directory_fd = os.open(namespace, os.O_DIRECTORY | os.O_RDONLY)
    try:
        os.fsync(directory_fd)
    finally:
        os.close(directory_fd)
    after_digest = H.digest(ledger_path)
    assert after_digest == hashlib.sha256(updated_bytes).hexdigest()
    progress("ledger-published", resolved_ledger_sha256=after_digest)
    stopped_proof(group_root, sockets)
    H.validate_locks()
    assert H.root_identities(roots) == root_facts and pair(namespace) == selected
    assert unit.read_bytes() == unit_bytes and container_evidence() == evidence
    assert all(H.digest(namespace / name) == value for name, value in before.items() if name != "dev-local-jobs.json")
    assert H.digest(ledger_path) == after_digest
    progress("start-requested", service_start_requested=True)
    H.manager("start")
    for _ in range(100):
        if H.property_value("ActiveState") == "active" and all(p.is_socket() for p in sockets):
            break
        time.sleep(0.05)
    assert H.property_value("ActiveState") == "active"
    new_pid = int(H.property_value("MainPID"))
    assert H.digest(Path("/proc") / str(new_pid) / "exe") == BINARY
    assert pair(namespace) == selected and unit.read_bytes() == unit_bytes
    assert H.property_value("UnitFileState") == "enabled"
    H.exchange(sockets[0], {"schema_version": 1, "service": "smith-next", "command": "identity"}, new_pid, 4096)
    reply = H.exchange(sockets[1], H.app_request(), new_pid)
    found = [j for j in reply["result"]["jobs"] if j["request_id"] == REQUEST]
    assert len(found) == 1 and found[0]["status"] == "failed" and found[0]["cleanup_verified"] is True
    assert all(H.digest(namespace / name) == value for name, value in before.items() if name != "dev-local-jobs.json")
    assert H.digest(ledger_path) == after_digest and container_evidence() == evidence
    H.validate_locks()
    result = {"request_id": REQUEST, "container_id": CONTAINER, "same_paired_binary_sha256": BINARY,
              "original_ledger_sha256": hashlib.sha256(ledger_bytes).hexdigest(), "resolved_ledger_sha256": after_digest,
              "status": "failed", "cleanup_verified": True, "original_failure_and_other_records_preserved": True,
              "container_retained_unchanged": True, "private_snapshot_retained": True, "unit_and_pair_unchanged": True,
              "no_other_state_changed": True, "job_replayed": False, "new_job_submitted": False}
    write_evidence(EVIDENCE / "logging-reconciliation.json", result)
    progress("completed", completed=True)
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    try:
        main()
    except BaseException as error:
        if PROGRESS_OWNED:
            try:
                progress(PROGRESS["phase"], failed=True, error_type=type(error).__name__)
            except BaseException:
                print("Partial-effect evidence write failed; preserve all state and inspect before any further effect")
        else:
            print("Preflight refused before progress ownership; prior evidence and live state left untouched")
        raise
    finally:
        for descriptor, _, _ in reversed(H.HELD_LOCKS):
            os.close(descriptor)
