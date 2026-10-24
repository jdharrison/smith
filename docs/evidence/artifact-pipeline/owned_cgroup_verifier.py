"""Source-only, read-only verifier for the supported Ubuntu/systemd cgroup2 layout.

API: capture(pid, full_cid) -> OwnedCgroup; retain it through cleanup, call
quiescent(deadline=..., cancelled=...) -> bool, then close() (or use a context
manager). Capture accepts a separate run_deadline ceiling for fresh cleanup
observations after its short capture deadline expires. False means observed
population; Refusal means Unknown, never cleanup success. identity is (dev, ino).
No CLI, import-time probes, subprocesses, writes, or resource-control operations.

Only /system.slice/docker-<lowercase full CID>.scope is supported. Root controls
must provide aggregate <=4 CPUs, exactly 4 GiB and 1024 PIDs; child controls may
inherit. Mount evidence must expose the unified host root at /sys/fs/cgroup,
with host systemd PID1 in /init.scope. Other layouts are refused, not discovered.
_Private _Environment paths are an offline testing seam, not operational config.

Observations are bounded, cooperative and non-atomic: a privileged engine can
change controls, migrate/escape, or repopulate after observation. This is not a
hostile-root sandbox, Docker ownership attestation, authorization, or acceptance.
Caller must supply an independently captured Docker CID/PID and retain fences on
Unknown. Deadlines cannot interrupt a blocked kernel read; use local proc/cgroup
filesystems only. Control/proc records are limited to 4096 bytes; mountinfo has a
separate 128 KiB bound. Objects are single-caller, not thread-safe. Source fixtures
do not establish live-host compatibility.
"""

import os
import re
import stat
import time
from dataclasses import dataclass
from enum import Enum


class Code(str, Enum):
    INPUT = "invalid_input"
    LAYOUT = "unsupported_layout"
    IO = "unavailable_or_unsafe"
    MALFORMED = "malformed_evidence"
    LIMIT = "resource_limit"
    BOUND = "observation_bound"
    RACE = "identity_changed"
    INTERRUPTED = "interrupted"
    CLOSED = "closed"


class Refusal(RuntimeError):
    """Typed Unknown with fixed, bounded diagnostics; no underlying paths/errors."""

    def __init__(self, code):
        self.code = Code(code)
        super().__init__(self.code.value)


@dataclass(frozen=True)
class _Environment:
    cgroup: str = "/sys/fs/cgroup"
    proc: str = "/proc"


_DIR = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
_FILE = os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK
_NUMBER = re.compile(r"[0-9]{1,20}\Z")


def _uint(value):
    return bool(_NUMBER.fullmatch(value)) and int(value) <= 18446744073709551615


def _deadline_value(value):
    if type(value) not in (int, float) or not float("-inf") < value < float("inf"):
        raise Refusal(Code.INPUT)
    return value


def _identity(fd):
    s = os.fstat(fd)
    if s.st_nlink == 0:
        raise Refusal(Code.RACE)
    return s.st_dev, s.st_ino


def _absolute_dir(path):
    # Traverse even the fixed absolute prefixes without following symlinks.
    fd = os.open("/", _DIR)
    try:
        for part in path.split("/")[1:]:
            if not part or part in (".", ".."):
                raise Refusal(Code.LAYOUT)
            child = os.open(part, _DIR, dir_fd=fd)
            os.close(fd)
            fd = child
        return fd
    except BaseException:
        os.close(fd)
        raise


class OwnedCgroup:
    """Held owned-root identity. Instances are created only by capture()."""

    def _check(self):
        if self._closed:
            raise Refusal(Code.CLOSED)
        if self._cancelled() or self._clock() >= self._deadline:
            raise Refusal(Code.INTERRUPTED)

    def _read(self, fd, name, *, limit=4096):
        self._check()
        child = os.open(name, _FILE, dir_fd=fd)
        try:
            if not stat.S_ISREG(os.fstat(child).st_mode):
                raise Refusal(Code.IO)
            data = bytearray()
            while len(data) <= limit:
                self._check()
                chunk = os.read(child, limit + 1 - len(data))
                if not chunk:
                    break
                data.extend(chunk)
            if len(data) > limit:
                raise Refusal(Code.BOUND)
            self._check()
            try:
                return data.decode("ascii")
            except UnicodeError:
                raise Refusal(Code.MALFORMED) from None
        finally:
            os.close(child)

    def _open(self, fd, name):
        self._check()
        child = os.open(name, _DIR, dir_fd=fd)
        if os.fstat(child).st_dev != self.identity[0]:
            os.close(child)
            raise Refusal(Code.LAYOUT)
        return child

    def _mount(self):
        current = os.open(str(os.getpid()), _DIR, dir_fd=self._proc)
        try:
            text = self._read(current, "mountinfo", limit=128 * 1024)
        finally:
            os.close(current)
        found = 0
        for line in text.splitlines():
            self._check()
            fields = line.split()
            if len(fields) < 10 or fields.count("-") != 1:
                raise Refusal(Code.MALFORMED)
            sep = fields.index("-")
            if sep < 6 or len(fields) != sep + 4:
                raise Refusal(Code.MALFORMED)
            point = fields[4]
            if point.startswith("/sys/fs/cgroup/"):
                raise Refusal(Code.LAYOUT)
            if point == "/sys/fs/cgroup":
                expected = "%d:%d" % (os.major(self.identity[0]), os.minor(self.identity[0]))
                if fields[3] != "/" or fields[2] != expected or fields[sep + 1] != "cgroup2":
                    raise Refusal(Code.LAYOUT)
                found += 1
        if found != 1:
            raise Refusal(Code.LAYOUT)
        init = os.open("1", _DIR, dir_fd=self._proc)
        try:
            if (self._read(init, "comm") != "systemd\n"
                    or self._read(init, "cgroup") != "0::/init.scope\n"):
                raise Refusal(Code.LAYOUT)
        finally:
            os.close(init)

    def _root_check(self):
        self._check()
        self._mount()
        cg = _absolute_dir(self._env.cgroup)
        try:
            if _identity(cg) != _identity(self._cg):
                raise Refusal(Code.RACE)
            parent = self._open(cg, "system.slice")
            try:
                root = self._open(parent, self._scope)
                try:
                    if _identity(root) != self.identity or _identity(self._root) != self.identity:
                        raise Refusal(Code.RACE)
                finally:
                    os.close(root)
            finally:
                os.close(parent)
        finally:
            os.close(cg)

    def _controls(self, fd, root=False):
        cpu = self._read(fd, "cpu.max").split()
        if len(cpu) != 2 or not _NUMBER.fullmatch(cpu[1]):
            raise Refusal(Code.MALFORMED)
        period = int(cpu[1])
        if not 1000 <= period <= 1000000:
            raise Refusal(Code.MALFORMED)
        quota = cpu[0]
        if quota != "max" and (not _uint(quota) or int(quota) < 1000):
            raise Refusal(Code.MALFORMED)
        if root and (quota == "max" or int(quota) > 4 * period):
            raise Refusal(Code.LIMIT)
        for name, expected in (("memory.max", 4294967296), ("pids.max", 1024)):
            value = self._read(fd, name).strip()
            if value != "max" and not _uint(value):
                raise Refusal(Code.MALFORMED)
            if root and value != str(expected):
                raise Refusal(Code.LIMIT)

    def _process(self, fd):
        text = self._read(fd, "stat")
        end = text.rfind(") ")
        fields = text[end + 2:].split()
        if (not text.startswith(str(self._pid) + " (") or end < 0
                or len(fields) < 20 or not _NUMBER.fullmatch(fields[19])):
            raise Refusal(Code.MALFORMED)
        membership = self._read(fd, "cgroup")
        if not membership.startswith("0::/") or membership.count("\n") != 1 or not membership.endswith("\n"):
            raise Refusal(Code.LAYOUT)
        parts = membership[4:-1].split("/")
        if (len(parts) < 2 or parts[:2] != ["system.slice", self._scope]
                or any(not p or p in (".", "..") or len(p) > 255
                       or not re.fullmatch(r"[A-Za-z0-9_.:-]+", p) for p in parts)):
            raise Refusal(Code.LAYOUT)
        if len(parts) - 2 > 64:
            raise Refusal(Code.BOUND)
        return fields[19], tuple(parts[2:])

    def _procs(self, fd):
        values = self._read(fd, "cgroup.procs").splitlines()
        if any(not _NUMBER.fullmatch(v) or int(v) < 1 for v in values):
            raise Refusal(Code.MALFORMED)
        return values

    def _events(self):
        values = {}
        for line in self._read(self._root, "cgroup.events").splitlines():
            pair = line.split()
            if len(pair) != 2 or pair[0] in values or not _NUMBER.fullmatch(pair[1]):
                raise Refusal(Code.MALFORMED)
            values[pair[0]] = pair[1]
        if values.get("populated") not in ("0", "1") or values.get("frozen", "0") not in ("0", "1"):
            raise Refusal(Code.MALFORMED)
        return values["populated"] == "0"

    def quiescent(self, *, deadline=None, cancelled=None):
        """Observe the entire held scope using an optional fresh observation budget.

        An explicit absolute deadline is capped at capture's run_deadline. Without
        an override, use the capture observation deadline/cancellation callback.
        A supplied cancellation callback replaces the capture callback only for
        this call, allowing cleanup despite cancelled work. Neither argument can
        extend the run ceiling. Missing/replaced/unsafe evidence remains Unknown.
        """
        observation_end = self._deadline if deadline is None else _deadline_value(deadline)
        if cancelled is not None and not callable(cancelled):
            raise Refusal(Code.INPUT)
        previous = self._deadline, self._cancelled
        self._deadline = min(observation_end, self._run_deadline)
        if cancelled is not None:
            self._cancelled = cancelled
        try:
            self._root_check()
            empty = self._events()
            count = 0

            def visit(fd, depth):
                nonlocal count, empty
                self._check()
                count += 1
                if count > 256 or depth > 64:
                    raise Refusal(Code.BOUND)
                before = _identity(fd)
                self._controls(fd, depth == 0)
                if self._procs(fd):
                    empty = False
                # scandir is streamed: even non-directory entries are bounded.
                names = []
                with os.scandir(fd) as entries:
                    for entry in entries:
                        self._check()
                        if len(names) >= 256:
                            raise Refusal(Code.BOUND)
                        s = entry.stat(follow_symlinks=False)
                        if stat.S_ISLNK(s.st_mode):
                            raise Refusal(Code.IO)
                        names.append((entry.name, s.st_dev, s.st_ino, s.st_mode))
                for name, dev, ino, mode in names:
                    if stat.S_ISDIR(mode):
                        child = self._open(fd, name)
                        try:
                            if _identity(child) != (dev, ino):
                                raise Refusal(Code.RACE)
                            visit(child, depth + 1)
                            s = os.stat(name, dir_fd=fd, follow_symlinks=False)
                            if (s.st_dev, s.st_ino) != (dev, ino):
                                raise Refusal(Code.RACE)
                        finally:
                            os.close(child)
                    elif not stat.S_ISREG(mode):
                        raise Refusal(Code.IO)
                with os.scandir(fd) as entries:
                    after = set()
                    for entry in entries:
                        self._check()
                        if len(after) >= 256:
                            raise Refusal(Code.BOUND)
                        s = entry.stat(follow_symlinks=False)
                        after.add((entry.name, s.st_dev, s.st_ino, s.st_mode))
                if after != set(names) or _identity(fd) != before:
                    raise Refusal(Code.RACE)

            visit(self._root, 0)
            empty = self._events() and empty
            self._root_check()
            self._check()
            return empty
        except OSError:
            raise Refusal(Code.IO) from None
        finally:
            self._deadline, self._cancelled = previous

    def close(self):
        if not self._closed:
            self._closed = True
            for fd in self._fds:
                os.close(fd)
            self._fds.clear()

    def __enter__(self):
        self._check()
        return self

    def __exit__(self, *args):
        self.close()


def capture(pid, cid, *, deadline=None, cancelled=lambda: False,
            run_deadline=None, clock=time.monotonic, _env=None):
    """Bind PID start/membership, root identity and aggregate controls read-only.

    All deadlines are absolute in clock's units. deadline bounds capture (default:
    now + 5 seconds). run_deadline is the original overall run ceiling; capture
    and all cleanup observations are capped to it. If omitted, the ceiling is the
    capture deadline, so renewal cannot extend it. Pass run_deadline=end when
    retaining the object through a long CLI run, then request fresh short budgets
    with quiescent(deadline=min(end, now + 5), cancelled=lambda: False).
    Cancellation callbacks must be callable. This API grants no live authority.
    """
    if type(pid) is not int or not 1 < pid <= 2147483647 or not isinstance(cid, str) or not re.fullmatch(r"[0-9a-f]{64}", cid):
        raise Refusal(Code.INPUT)
    obj = object.__new__(OwnedCgroup)
    obj._closed = False
    obj._fds = []
    obj._env = _env or _Environment()
    obj._clock, obj._cancelled = clock, cancelled
    obj._deadline = _deadline_value(clock() + 5 if deadline is None else deadline)
    obj._run_deadline = (obj._deadline if run_deadline is None
                         else _deadline_value(run_deadline))
    obj._deadline = min(obj._deadline, obj._run_deadline)
    if not callable(cancelled):
        raise Refusal(Code.INPUT)
    obj._pid = pid
    obj._scope = "docker-" + cid + ".scope"
    try:
        obj._check()
        obj._proc = _absolute_dir(obj._env.proc)
        obj._fds.append(obj._proc)
        obj._cg = _absolute_dir(obj._env.cgroup)
        obj._fds.append(obj._cg)
        # Root identity is needed by the same-device traversal guard.
        obj.identity = _identity(obj._cg)
        parent = obj._open(obj._cg, "system.slice")
        try:
            obj._root = obj._open(parent, obj._scope)
            obj._fds.append(obj._root)
        finally:
            os.close(parent)
        obj.identity = _identity(obj._root)
        obj._root_check()
        process = os.open(str(pid), _DIR, dir_fd=obj._proc)
        try:
            process_identity = _identity(process)
            before = obj._process(process)
            leaf = os.dup(obj._root)
            try:
                obj._controls(leaf, True)
                for part in before[1]:
                    child = obj._open(leaf, part)
                    os.close(leaf)
                    leaf = child
                    obj._controls(leaf)
                if str(pid) not in obj._procs(leaf):
                    raise Refusal(Code.RACE)
                if before != obj._process(process):
                    raise Refusal(Code.RACE)
                # Reopen the membership chain to detect moved/replaced leaves.
                check = os.dup(obj._root)
                try:
                    for part in before[1]:
                        child = obj._open(check, part)
                        os.close(check)
                        check = child
                    if _identity(check) != _identity(leaf):
                        raise Refusal(Code.RACE)
                finally:
                    os.close(check)
            finally:
                os.close(leaf)
            fresh = os.open(str(pid), _DIR, dir_fd=obj._proc)
            try:
                if _identity(fresh) != process_identity or obj._process(fresh) != before:
                    raise Refusal(Code.RACE)
            finally:
                os.close(fresh)
        finally:
            os.close(process)
        obj._root_check()
        obj._controls(obj._root, True)
        obj._check()
        return obj
    except OSError:
        obj.close()
        raise Refusal(Code.IO) from None
    except BaseException:
        obj.close()
        raise
