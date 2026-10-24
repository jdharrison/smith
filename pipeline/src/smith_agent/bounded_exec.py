"""Trusted, single-threaded Linux coding-image supervisor; not a host launcher."""

import argparse
import contextlib
import ctypes
import json
import math
import os
from pathlib import Path
import resource
import selectors
import signal
import stat
import subprocess
import time

STDOUT_LIMIT = 64 * 1024
STDERR_LIMIT = 16 * 1024
FILE_LIMIT = 16 * 1024 * 1024
WORKSPACE_LIMIT = 128 * 1024 * 1024
FILE_COUNT = 4096
MAX_DEPTH = 32
REASONS = frozenset(("exited", "timed_out", "output_limit", "workspace_limit",
                     "process_group_unknown"))


def _directory(path):
    path = Path(path)
    if not path.is_absolute() or ".." in path.parts:
        raise ValueError("invalid_path")
    fd = os.open("/", os.O_RDONLY | os.O_DIRECTORY)
    try:
        for part in path.parts[1:]:
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                            dir_fd=fd)
            os.close(fd)
            fd = child
        return fd
    except BaseException:
        os.close(fd)
        raise


def _capture(path, private):
    path = Path(path)
    if (not path.is_absolute() or ".." in path.parts
            or not path.is_relative_to(private) or path == private):
        raise ValueError("invalid_capture_path")
    parent = _directory(path.parent)
    try:
        fd = os.open(path.name, os.O_WRONLY | os.O_CREAT | os.O_EXCL
                     | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600, dir_fd=parent)
        os.fchmod(fd, 0o600)
        return os.fdopen(fd, "wb", buffering=0)
    finally:
        os.close(parent)


def _within_budget(roots):
    total, count = 1024, 0  # Reserve the bounded status record before publication.

    def walk(fd, depth):
        nonlocal total, count
        with os.scandir(fd) as entries:
            for entry in entries:
                count += 1
                if count > FILE_COUNT or depth > MAX_DEPTH:
                    return False
                info = entry.stat(follow_symlinks=False)
                total += info.st_size
                if total > WORKSPACE_LIMIT:
                    return False
                if stat.S_ISREG(info.st_mode) and info.st_size > FILE_LIMIT:
                    return False
                if stat.S_ISDIR(info.st_mode):
                    child = os.open(entry.name, os.O_RDONLY | os.O_DIRECTORY
                                    | os.O_NOFOLLOW, dir_fd=fd)
                    try:
                        if not walk(child, depth + 1):
                            return False
                    finally:
                        os.close(child)
        return True

    try:
        return all(walk(fd, 1) for fd in roots)
    except OSError:
        # Concurrent replacement/removal cannot establish a safe observation.
        return False


def _file_limit():
    resource.setrlimit(resource.RLIMIT_FSIZE, (FILE_LIMIT, FILE_LIMIT))


def _identity(pid):
    # comm may contain spaces and parentheses; fields after its final ')' start
    # at field 3. Bind both direct parentage and field 22 (kernel start time).
    fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
    return int(fields[1]), int(fields[19])


def _children(deadline=None):
    parent = os.getpid()
    with open(f"/proc/self/task/{parent}/children") as file:
        raw = file.read(65537)
    if len(raw) > 65536:
        raise OSError("child_snapshot_limit")
    children = {}
    for pid in map(int, raw.split()):
        if deadline is not None and time.monotonic() >= deadline:
            raise OSError("cleanup_deadline")
        owner, started = _identity(pid)
        if owner != parent:
            raise OSError("child_ownership_unknown")
        children[pid] = started
    return children


def _cleanup(process):
    """Kill/reap direct children, then their adopted descendants, within 2s.

    run() admits only a single-threaded, child-free owner before spawn. Thus
    every later direct child belongs to this invocation, including setsid escapees.
    No process-group broadcast or unqualified wait(-1) can touch unrelated work.
    """
    deadline = time.monotonic() + 2
    try:
        while time.monotonic() < deadline:
            process.poll()  # Only Popen reaps the original leader.
            children = _children(deadline)
            if not children and process.returncode is not None:
                return time.monotonic() < deadline
            for pid, started in children.items():
                if time.monotonic() >= deadline:
                    return False
                # Pin before verifying identity; signal through the fd so PID
                # reuse between verification and signal cannot target a stranger.
                fd = os.pidfd_open(pid)
                try:
                    if _identity(pid) != (os.getpid(), started):
                        return False
                    signal.pidfd_send_signal(fd, signal.SIGKILL)
                finally:
                    os.close(fd)
                if pid == process.pid:
                    process.poll()
                else:
                    os.waitpid(pid, os.WNOHANG)
            time.sleep(0.005)
    except (OSError, ValueError, IndexError):
        return False
    return False


def run(argv, *, seconds, stdout, stderr, status,
        workspace="/workspace", private="/private"):
    """Run caller-owned argv; return and persist {exit_code: int|None, reason: str}.

    Paths/roots are injectable for isolated tests. All three outputs must be new
    files beneath private. The caller supplies a clean inherited environment and
    immutable wrapper/argv; candidate validation/export is outside this API.
    The dedicated owner must have one thread and no preexisting children; other
    contexts fail closed before spawn because adoption ownership is ambiguous.
    """
    if (isinstance(seconds, bool) or not isinstance(seconds, (int, float))
            or not math.isfinite(seconds) or not 0 < seconds <= 120
            or not argv or any(not isinstance(arg, str) or "\0" in arg for arg in argv)):
        raise ValueError("invalid_arguments")
    private, workspace = Path(private), Path(workspace)
    if (private == workspace or private in workspace.parents
            or workspace in private.parents):
        raise ValueError("overlapping_roots")
    result = {"exit_code": None, "reason": "process_group_unknown"}
    with contextlib.ExitStack() as stack:
        roots = [_directory(workspace)]
        stack.callback(os.close, roots[0])
        roots.append(_directory(private))
        stack.callback(os.close, roots[1])
        out, err, report = [stack.enter_context(_capture(path, private))
                            for path in (stdout, stderr, status)]
        # Linux subreaping is needed to reap orphaned grandchildren, not merely
        # signal them. Restore the caller's setting; reap only owned children.
        libc = ctypes.CDLL(None, use_errno=True)
        previous = ctypes.c_int()
        if (libc.prctl(37, ctypes.byref(previous), 0, 0, 0) == 0
                and libc.prctl(36, 1, 0, 0, 0) == 0):
            process = None
            try:
                if len(os.listdir("/proc/self/task")) != 1 or _children():
                    pass  # Cannot distinguish future unrelated adoptions safely.
                elif not _within_budget(roots):
                    result["reason"] = "workspace_limit"
                else:
                    deadline = time.monotonic() + seconds
                    process = subprocess.Popen(
                        argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                        stderr=subprocess.PIPE, start_new_session=True,
                        preexec_fn=_file_limit, cwd=workspace,
                    )
                    result["reason"] = "exited"
                    with selectors.DefaultSelector() as selector:
                        for pipe, sink, cap in ((process.stdout, out, STDOUT_LIMIT),
                                                (process.stderr, err, STDERR_LIMIT)):
                            stack.callback(pipe.close)
                            os.set_blocking(pipe.fileno(), False)
                            selector.register(pipe, selectors.EVENT_READ, [sink, cap, 0])
                        while True:
                            if not _within_budget(roots):
                                result["reason"] = "workspace_limit"
                                break
                            if time.monotonic() >= deadline:
                                result["reason"] = "timed_out"
                                break
                            for key, _ in selector.select(min(0.02, max(
                                    0, deadline - time.monotonic()))):
                                sink, cap, used = key.data
                                data = os.read(key.fd, min(8192, cap - used + 1))
                                if not data:
                                    selector.unregister(key.fileobj)
                                    continue
                                sink.write(data[:cap - used])
                                key.data[2] += len(data)
                                if key.data[2] > cap:
                                    result["reason"] = "output_limit"
                                    break
                            if result["reason"] != "exited":
                                break
                            if process.poll() is not None and not selector.get_map():
                                break
            except (OSError, ValueError, IndexError, subprocess.SubprocessError):
                result["reason"] = "process_group_unknown"
            finally:
                if process is not None:
                    if not _cleanup(process):
                        result["reason"] = "process_group_unknown"
                    result["exit_code"] = process.returncode
                if (result["reason"] == "exited" and not _within_budget(roots)):
                    result["reason"] = "workspace_limit"
                if libc.prctl(36, previous.value, 0, 0, 0) != 0:
                    result["reason"] = "process_group_unknown"
        report.write(json.dumps(result, separators=(",", ":")).encode("ascii") + b"\n")
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seconds", type=float, required=True)
    for name in ("stdout", "stderr", "status"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)
    if not args.command or args.command[0] != "--" or len(args.command) < 2:
        parser.error("fixed command required after --")
    try:
        result = run(args.command[1:], seconds=args.seconds, stdout=args.stdout,
                     stderr=args.stderr, status=args.status)
    except (ValueError, OSError):
        return 2
    return 0 if result["reason"] == "exited" else 1


if __name__ == "__main__":
    raise SystemExit(main())
