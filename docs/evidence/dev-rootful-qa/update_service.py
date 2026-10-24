"""Apply the explicitly approved one-off QA binary update; never submit a job."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import stat
import struct
import subprocess
import time
import uuid


ROOT = Path(__file__).resolve().parents[3]
EVIDENCE = ROOT / "docs/evidence/dev-rootful-qa"
BACKUP = Path.home() / ".local/state/smith-qa-backups/dev-rootful-20261018"
CANDIDATE = ROOT / ".smith-dev-qa-rootful/smith-service.candidate"
EXPECTED_OLD = "8abaf73724dbc0f04b4fbe1279f64f69ad315fc11874eba5a40a0faf1444f50d"
EXPECTED_NEW = "805c16d433ce88a7fb3d84a3186095ae6d61c3d66720d0a362b88762427b58ba"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def property_value(name):
    return subprocess.check_output(
        ["systemctl", "--user", "show", "smith-next.service", f"--property={name}", "--value"],
        text=True,
        timeout=10,
    ).strip()


def manager(action):
    result = subprocess.run(
        ["systemctl", "--user", action, "smith-next.service"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        timeout=20,
    )
    if result.returncode:
        raise RuntimeError(f"Named service {action} failed; inspect state before further effects")


def assert_safe_file(path):
    meta = path.lstat()
    assert stat.S_ISREG(meta.st_mode) and meta.st_uid == os.geteuid()
    assert meta.st_nlink == 1 and meta.st_mode & 0o022 == 0


def assert_idle(namespace):
    state = json.loads((namespace / "app-state.json").read_text())
    assert all(job["operation"]["status"] not in ["queued", "running"] for job in state["jobs"])
    for name in ["dev-jobs.json", "dev-local-jobs.json"]:
        path = namespace / name
        if path.exists():
            for record in json.loads(path.read_text())["records"]:
                assert record["summary"]["status"] in ["failed", "completed", "recorded"]
                assert record["cleanup_required"] is False


def enumeration_failed(error):
    raise error


def exchange(path, request, pid, limit):
    before = path.lstat()
    assert stat.S_ISSOCK(before.st_mode) and before.st_uid == os.geteuid()
    assert stat.S_IMODE(before.st_mode) == 0o600
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as stream:
        deadline = time.monotonic() + 2
        stream.settimeout(2)
        stream.connect(str(path))
        peer_pid, peer_uid, _ = struct.unpack("3i", stream.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
        after = path.lstat()
        assert (before.st_dev, before.st_ino) == (after.st_dev, after.st_ino)
        assert peer_pid == pid and peer_uid == os.geteuid()
        stream.sendall(json.dumps(request).encode() + b"\n")
        data = bytearray()
        while b"\n" not in data:
            remaining = deadline - time.monotonic()
            assert remaining > 0 and len(data) <= limit
            stream.settimeout(remaining)
            chunk = stream.recv(min(8192, limit + 1 - len(data)))
            assert chunk
            data.extend(chunk)
        frame, extra = data.split(b"\n", 1)
        assert not extra and len(frame) <= limit
        reply = json.loads(frame)
    assert reply["service"] == "smith-next" and reply["pid"] == pid
    assert reply["schema_version"] == request["schema_version"] and reply["ok"] is True
    return reply


# Bind the reviewed sources and image to the exact artifact before external mutation.
assert digest(CANDIDATE) == EXPECTED_NEW
source_manifest = json.loads((EVIDENCE / "source-sha256.json").read_text())
assert all(digest(ROOT / name) == value for name, value in source_manifest.items())
image = json.loads((EVIDENCE / "image-identity.json").read_text())
assert image["candidate_sha256"] == EXPECTED_NEW
assert BACKUP.is_dir() and not BACKUP.is_symlink()
assert BACKUP.stat().st_uid == os.geteuid() and BACKUP.stat().st_mode & 0o077 == 0
assert not any(BACKUP.iterdir()), "Refuse to overwrite a previous rollback snapshot"
assert property_value("ActiveState") == "active"
assert property_value("UnitFileState") == "enabled"
pid = int(property_value("MainPID"))
proc = Path("/proc") / str(pid)
exe = (proc / "exe").resolve()
assert digest(proc / "exe") == EXPECTED_OLD
assert_safe_file(exe)
args = [os.fsdecode(part) for part in (proc / "cmdline").read_bytes().split(b"\0") if part]
assert len(args) == 8 and args[1] == "serve"
assert args[2::2] == ["--data-root", "--config-root", "--runtime-root"]
roots = dict(zip(args[2::2], map(Path, args[3::2])))
namespace = roots["--data-root"] / "smith-next"
assert exe == namespace / "bin/smith-service"
unit = Path(property_value("FragmentPath"))
assert_safe_file(unit)
unit_bytes = unit.read_bytes()
assert property_value("NeedDaemonReload") == "no"
assert property_value("DropInPaths") == ""
assert "ExecStart=" + " ".join(args) in unit_bytes.decode().splitlines()
effective_start = property_value("ExecStart")
assert f"path={exe} ;" in effective_start and f"argv[]={' '.join(args)} ;" in effective_start
state_paths = [namespace / name for name in ["app-state.json", "dev-jobs.json", "dev-local-jobs.json"]]
assert_idle(namespace)

control_group = property_value("ControlGroup")
assert control_group.startswith("/user.slice/") and control_group.endswith("/smith-next.service")
assert ".." not in Path(control_group).parts
assert (proc / "cgroup").read_text() == f"0::{control_group}\n"
sockets = [roots["--runtime-root"] / "smith-next" / name for name in ["service.sock", "app.sock"]]
print("Preflight passed: exact candidate/source binding and no active work")
manager("stop")
assert property_value("ActiveState") == "inactive" and property_value("MainPID") == "0"
group_root = Path("/sys/fs/cgroup") / control_group.lstrip("/")
try:
    group_root.stat()
except FileNotFoundError:
    pass  # A removed captured cgroup has no remaining descendants.
else:
    for directory, _, _ in os.walk(group_root, onerror=enumeration_failed):
        assert not (Path(directory) / "cgroup.procs").read_text().strip(), "Service cgroup subtree not quiescent"
assert all(not os.path.lexists(path) for path in sockets), "Stale socket preserved; do not publish"
assert_idle(namespace)  # Recheck the stopped snapshot; pre-stop observations can race submissions.

# Private, stopped-state snapshot. Never copy prompts or host roots into repo evidence.
shutil.copy2(exe, BACKUP / "smith-service.before")
shutil.copy2(unit, BACKUP / "smith-next.service.before")
before = {}
for path in state_paths:
    if path.exists():
        assert_safe_file(path)
        before[path.name] = digest(path)
        shutil.copy2(path, BACKUP / path.name)
        os.chmod(BACKUP / path.name, 0o600)
assert digest(exe) == EXPECTED_OLD and unit.read_bytes() == unit_bytes
staged = exe.with_name("smith-service.qa-staging")
assert not staged.exists()
with staged.open("xb") as output:
    output.write(CANDIDATE.read_bytes())
    output.flush()
    os.fchmod(output.fileno(), stat.S_IMODE(exe.stat().st_mode))
    os.fsync(output.fileno())
assert digest(staged) == EXPECTED_NEW
os.replace(staged, exe)
parent_fd = os.open(exe.parent, os.O_DIRECTORY | os.O_RDONLY)
try:
    os.fsync(parent_fd)
finally:
    os.close(parent_fd)
print("Private rollback snapshot retained; matching service binary published")
manager("start")
for _ in range(50):
    if (
        property_value("ActiveState") == "active"
        and property_value("MainPID") != "0"
        and (roots["--runtime-root"] / "smith-next/app.sock").is_socket()
    ):
        break
    time.sleep(0.1)
assert property_value("ActiveState") == "active"
new_pid = int(property_value("MainPID"))
assert digest(Path("/proc") / str(new_pid) / "exe") == EXPECTED_NEW
assert property_value("UnitFileState") == "enabled" and unit.read_bytes() == unit_bytes
assert all(digest(namespace / name) == value for name, value in before.items()), "State changed on restart; preserve snapshot and inspect"
core = exchange(sockets[0], {"schema_version": 1, "service": "smith-next", "command": "identity"}, new_pid, 4096)
assert core["error"] is None
call_id = uuid.uuid4().hex
request = {"schema_version": 3, "service": "smith-next", "protocol": "app", "call_id": call_id, "op": "dev_jobs_state_v2", "args": {}}
app = exchange(sockets[1], request, new_pid, 256 * 1024)
assert app["protocol"] == "app" and app["call_id"] == call_id and app["error"] is None
assert len(app["instance_id"]) == 32 and all(c in "0123456789abcdef" for c in app["instance_id"])
assert isinstance(app["result"]["jobs"], list)
assert all(digest(namespace / name) == value for name, value in before.items())
result = {
    "old_binary_sha256": EXPECTED_OLD,
    "candidate_sha256": EXPECTED_NEW,
    "active_state": property_value("ActiveState"),
    "sub_state": property_value("SubState"),
    "enablement": property_value("UnitFileState"),
    "unit_unchanged": True,
    "app_and_dev_state_unchanged": True,
    "private_rollback_snapshot_retained": True,
    "app_socket_present": True,
    "core_peer_identity_verified": True,
    "app_peer_identity_and_state_verified": True,
    "local_model": app["result"]["model"],
    "job_submitted": False,
}
(EVIDENCE / "service-update.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result, indent=2))
