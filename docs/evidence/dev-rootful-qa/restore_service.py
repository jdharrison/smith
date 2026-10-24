"""Restore only the explicitly approved manifest-matching service; preserve live state."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import stat
import struct
import subprocess
import tempfile
import time
import uuid

ROOT = Path(__file__).resolve().parents[3]
EVIDENCE = ROOT / "docs/evidence/dev-rootful-qa"
BACKUP = Path.home() / ".local/state/smith-qa-backups/dev-rootful-20261018"
RESTORE = BACKUP / "smith-service.before"
EXPECTED_CURRENT = "805c16d433ce88a7fb3d84a3186095ae6d61c3d66720d0a362b88762427b58ba"
EXPECTED_RESTORED = "8abaf73724dbc0f04b4fbe1279f64f69ad315fc11874eba5a40a0faf1444f50d"
STATE_NAMES = ["app-state.json", "dev-jobs.json", "dev-local-jobs.json"]
HELD_LOCKS = []


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def property_value(name):
    return subprocess.check_output(
        ["systemctl", "--user", "show", "smith-next.service", f"--property={name}", "--value"],
        text=True, timeout=10,
    ).strip()


def manager(action):
    result = subprocess.run(
        ["systemctl", "--user", action, "smith-next.service"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=20,
    )
    assert result.returncode == 0, f"Named service {action} failed; no automatic retry"


def safe_file(path):
    meta = path.lstat()
    assert stat.S_ISREG(meta.st_mode) and meta.st_uid == os.geteuid()
    assert meta.st_nlink == 1 and meta.st_mode & 0o022 == 0
    return meta


def root_identities(roots):
    identities = {}
    private_namespaces = [roots["--data-root"] / "smith-next", roots["--data-root"] / "smith-next/bin", roots["--runtime-root"] / "smith-next"]
    for root in [*roots.values(), *private_namespaces]:
        assert root.is_absolute() and ".." not in root.parts
        for path in [*reversed(root.parents), root]:
            meta = path.lstat()
            assert stat.S_ISDIR(meta.st_mode) and meta.st_uid in [0, os.geteuid()]
            assert meta.st_mode & 0o022 == 0
            identities[path] = (meta.st_dev, meta.st_ino, meta.st_uid, meta.st_mode)
    for path in [roots["--runtime-root"], *private_namespaces]:
        meta = path.lstat()
        assert meta.st_uid == os.geteuid() and meta.st_mode & 0o077 == 0
    return identities


def acquire_lock(path, directory=False):
    flags = os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK
    if directory:
        fd = os.open(path, flags | os.O_RDONLY | os.O_DIRECTORY)
    else:
        try:
            fd = os.open(path, flags | os.O_RDWR | os.O_CREAT | os.O_EXCL, 0o600)
            os.fsync(fd)
            parent = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
            try:
                os.fsync(parent)
            finally:
                os.close(parent)
        except FileExistsError:
            fd = os.open(path, flags | os.O_RDWR)
    try:
        meta = os.fstat(fd)
        assert meta.st_uid == os.geteuid()
        if directory:
            assert stat.S_ISDIR(meta.st_mode) and meta.st_mode & 0o022 == 0
        else:
            assert stat.S_ISREG(meta.st_mode) and meta.st_nlink == 1
            assert stat.S_IMODE(meta.st_mode) == 0o600
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        named = path.lstat()
        assert (meta.st_dev, meta.st_ino, meta.st_uid, meta.st_mode) == (named.st_dev, named.st_ino, named.st_uid, named.st_mode)
        HELD_LOCKS.append((fd, path, directory))
    except BaseException:
        os.close(fd)
        raise


def validate_locks():
    for fd, path, directory in HELD_LOCKS:
        held, named = os.fstat(fd), path.lstat()
        assert (held.st_dev, held.st_ino, held.st_uid, held.st_mode) == (named.st_dev, named.st_ino, named.st_uid, named.st_mode)
        assert held.st_uid == os.geteuid()
        if directory:
            assert stat.S_ISDIR(held.st_mode) and held.st_mode & 0o022 == 0
        else:
            assert stat.S_ISREG(held.st_mode) and held.st_nlink == 1
            assert stat.S_IMODE(held.st_mode) == 0o600


def idle(namespace):
    app = json.loads((namespace / "app-state.json").read_text())
    assert all(job["operation"]["status"] not in ["queued", "running"] for job in app["jobs"])
    for name in STATE_NAMES[1:]:
        path = namespace / name
        if path.exists():
            for record in json.loads(path.read_text())["records"]:
                assert record["summary"]["status"] in ["failed", "completed", "recorded"]
                assert record["cleanup_required"] is False


def exchange(path, request, pid, limit=256 * 1024):
    before = path.lstat()
    assert stat.S_ISSOCK(before.st_mode) and before.st_uid == os.geteuid()
    assert stat.S_IMODE(before.st_mode) == 0o600
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as stream:
        deadline = time.monotonic() + 3
        stream.settimeout(3)
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
        result = json.loads(frame)
    assert result["service"] == "smith-next" and result["pid"] == pid
    assert result["schema_version"] == request["schema_version"] and result["ok"] is True
    assert result["error"] is None
    if request.get("protocol") == "app":
        assert result["protocol"] == "app" and result["call_id"] == request["call_id"]
        assert len(result["instance_id"]) == 32 and all(c in "0123456789abcdef" for c in result["instance_id"])
    return result


def app_request():
    return {"schema_version": 3, "service": "smith-next", "protocol": "app", "call_id": uuid.uuid4().hex, "op": "dev_jobs_state_v2", "args": {}}


def wait_app(runtime, child):
    for _ in range(100):
        assert child.poll() is None, "Compatibility daemon exited; no live replacement"
        if (runtime / "smith-next/app.sock").is_socket():
            return
        time.sleep(0.05)
    raise RuntimeError("Compatibility readiness deadline exceeded")


def fail_enumeration(error):
    raise error


def selected_release(namespace):
    selector = namespace / "client/current"
    target = os.readlink(selector)
    parts = Path(target).parts
    assert len(parts) == 2 and parts[0] == "releases"
    assert len(parts[1]) == 64 and all(c in "0123456789abcdef" for c in parts[1])
    release = selector.parent / target
    assert release.resolve() == release
    manifest_path = release / ".smith-next-manifest.json"
    safe_file(manifest_path)
    manifest_bytes = manifest_path.read_bytes()
    assert hashlib.sha256(manifest_bytes).hexdigest() == parts[1]
    manifest = json.loads(manifest_bytes)
    assert manifest_bytes == (json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n").encode()
    assert manifest["schema"] == 1
    for relative, entry in manifest["entries"].items():
        assert not Path(relative).is_absolute() and ".." not in Path(relative).parts
        path = release / relative
        meta = path.lstat()
        assert meta.st_uid == os.geteuid() and stat.S_IMODE(meta.st_mode) == entry["mode"]
        assert meta.st_mode & 0o022 == 0
        if entry["type"] == "file":
            assert stat.S_ISREG(meta.st_mode) and digest(path) == entry["sha256"]
        else:
            assert entry["type"] == "directory" and stat.S_ISDIR(meta.st_mode)
    helper = release / "smith_backend/smith-service"
    assert digest(helper) == EXPECTED_RESTORED
    assert stat.S_IMODE(safe_file(helper).st_mode) == 0o755
    return target, digest(manifest_path)


def main():
    safe_file(RESTORE)
    assert digest(RESTORE) == EXPECTED_RESTORED
    assert BACKUP.is_dir() and not BACKUP.is_symlink()
    assert BACKUP.stat().st_uid == os.geteuid() and BACKUP.stat().st_mode & 0o077 == 0
    assert property_value("ActiveState") == "active" and property_value("UnitFileState") == "enabled"
    assert property_value("NeedDaemonReload") == "no" and property_value("DropInPaths") == ""
    pid = int(property_value("MainPID"))
    proc = Path("/proc") / str(pid)
    exe = (proc / "exe").resolve()
    mode = stat.S_IMODE(safe_file(exe).st_mode)
    assert digest(proc / "exe") == EXPECTED_CURRENT and digest(exe) == EXPECTED_CURRENT
    args = [os.fsdecode(v) for v in (proc / "cmdline").read_bytes().split(b"\0") if v]
    assert len(args) == 8 and args[1] == "serve"
    assert args[2::2] == ["--data-root", "--config-root", "--runtime-root"]
    roots = dict(zip(args[2::2], map(Path, args[3::2])))
    namespace = roots["--data-root"] / "smith-next"
    assert exe == namespace / "bin/smith-service"
    root_facts = root_identities(roots)
    # Match publisher -> Core directory -> runner-after-Stop lock order.
    acquire_lock(roots["--data-root"] / ".smith-next-install.lock")
    acquire_lock(roots["--data-root"], directory=True)
    validate_locks()
    assert root_identities(roots) == root_facts
    assert property_value("MainPID") == str(pid) and digest(exe) == EXPECTED_CURRENT
    unit = Path(property_value("FragmentPath"))
    safe_file(unit)
    unit_bytes = unit.read_bytes()
    assert "ExecStart=" + " ".join(args) in unit_bytes.decode().splitlines()
    effective_start = property_value("ExecStart")
    assert f"path={exe} ;" in effective_start and f"argv[]={' '.join(args)} ;" in effective_start
    release_identity = selected_release(namespace)
    idle(namespace)
    state_before = {name: digest(namespace / name) for name in STATE_NAMES if (namespace / name).exists()}
    assert all(safe_file(namespace / name) for name in state_before)

    # Prove the preserved reader accepts CURRENT data on isolated roots, without
    # using the live manager, Docker, credentials, models, replay or inference.
    with tempfile.TemporaryDirectory(prefix="smith-restore-check-", dir=roots["--runtime-root"]) as temporary:
        base = Path(temporary)
        data, config, runtime = [base / name for name in ["data", "config", "runtime"]]
        for path in [data, config, runtime, data / "smith-next", data / "smith-next/bin"]:
            path.mkdir(mode=0o700)
        isolated_binary = data / "smith-next/bin/smith-service"
        shutil.copy2(RESTORE, isolated_binary)
        for name, value in state_before.items():
            shutil.copy2(namespace / name, data / "smith-next" / name)
            assert digest(data / "smith-next" / name) == value
        child = subprocess.Popen(
            [str(isolated_binary), "serve", "--data-root", str(data), "--config-root", str(config), "--runtime-root", str(runtime)],
            env={"PATH": "/usr/bin:/bin", "HOME": str(base), "XDG_DATA_HOME": str(data), "XDG_CONFIG_HOME": str(config), "XDG_RUNTIME_DIR": str(runtime)},
            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        try:
            wait_app(runtime, child)
            exchange(runtime / "smith-next/service.sock", {"schema_version": 1, "service": "smith-next", "command": "identity"}, child.pid, 4096)
            exchange(runtime / "smith-next/app.sock", app_request(), child.pid)
            assert all(digest(data / "smith-next" / name) == value for name, value in state_before.items())
        finally:
            if child.poll() is None:
                child.terminate()
            child.wait(timeout=10)
    print("Current-state compatibility verified with the preserved binary on isolated roots")

    assert all(digest(namespace / name) == value for name, value in state_before.items())
    assert selected_release(namespace) == release_identity
    assert unit.read_bytes() == unit_bytes
    validate_locks()
    assert root_identities(roots) == root_facts
    idle(namespace)
    group = property_value("ControlGroup")
    assert group.startswith("/user.slice/") and group.endswith("/smith-next.service")
    assert ".." not in Path(group).parts and (proc / "cgroup").read_text() == f"0::{group}\n"
    manager("stop")
    assert property_value("ActiveState") == "inactive" and property_value("MainPID") == "0"
    group_root = Path("/sys/fs/cgroup") / group.lstrip("/")
    try:
        group_root.stat()
    except FileNotFoundError:
        pass
    else:
        for directory, _, _ in os.walk(group_root, onerror=fail_enumeration):
            assert not (Path(directory) / "cgroup.procs").read_text().strip()
    sockets = [roots["--runtime-root"] / "smith-next" / name for name in ["service.sock", "app.sock"]]
    assert all(not os.path.lexists(path) for path in sockets)
    acquire_lock(roots["--runtime-root"] / "smith-next/runner.lock")
    validate_locks()
    assert root_identities(roots) == root_facts
    idle(namespace)
    assert all(digest(namespace / name) == value for name, value in state_before.items())
    assert digest(exe) == EXPECTED_CURRENT and selected_release(namespace) == release_identity

    snapshot = BACKUP / "before-approved-restore"
    snapshot.mkdir(mode=0o700)
    shutil.copy2(exe, snapshot / "smith-service.qa-candidate")
    shutil.copy2(unit, snapshot / "smith-next.service")
    for name in state_before:
        shutil.copy2(namespace / name, snapshot / name)
        os.chmod(snapshot / name, 0o600)
    staged = exe.with_name("smith-service.approved-restore-staging")
    with staged.open("xb") as output:
        output.write(RESTORE.read_bytes())
        output.flush()
        os.fchmod(output.fileno(), mode)
        os.fsync(output.fileno())
    assert digest(staged) == EXPECTED_RESTORED
    assert unit.read_bytes() == unit_bytes and selected_release(namespace) == release_identity
    validate_locks()
    assert root_identities(roots) == root_facts
    os.replace(staged, exe)
    directory_fd = os.open(exe.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(directory_fd)
    finally:
        os.close(directory_fd)
    validate_locks()
    assert root_identities(roots) == root_facts
    manager("start")
    for _ in range(100):
        if property_value("ActiveState") == "active" and all(path.is_socket() for path in sockets):
            break
        time.sleep(0.05)
    assert property_value("ActiveState") == "active" and property_value("SubState") == "running"
    new_pid = int(property_value("MainPID"))
    assert digest(Path("/proc") / str(new_pid) / "exe") == EXPECTED_RESTORED
    assert digest(exe) == EXPECTED_RESTORED and selected_release(namespace) == release_identity
    assert unit.read_bytes() == unit_bytes and property_value("UnitFileState") == "enabled"
    exchange(sockets[0], {"schema_version": 1, "service": "smith-next", "command": "identity"}, new_pid, 4096)
    exchange(sockets[1], app_request(), new_pid)
    assert all(digest(namespace / name) == value for name, value in state_before.items())
    validate_locks()
    assert root_identities(roots) == root_facts
    result = {
        "restored_binary_sha256": EXPECTED_RESTORED,
        "matches_manifest_selected_helper": True,
        "selected_release_manifest_verified": True,
        "current_state_backward_readability_verified_in_isolation": True,
        "current_state_preserved_byte_for_byte": True,
        "unit_and_enablement_unchanged": True,
        "active_state": "active", "sub_state": "running",
        "core_and_app_peer_verified": True,
        "fresh_private_pre_restore_snapshot_retained": True,
        "publisher_core_runner_locks_held_in_order": True,
        "installer_upgrade_retried": False,
        "job_submitted": False, "inference_run": False,
    }
    (EVIDENCE / "service-restore.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    try:
        main()
    finally:
        for descriptor, _, _ in reversed(HELD_LOCKS):
            os.close(descriptor)
