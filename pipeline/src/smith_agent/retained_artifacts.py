"""Unwired retained candidate artifacts and one fixed stdlib unittest profile.

Only trusted Python callers may supply roots, images, runtime policy or evidence.
Nothing here grants a model host execution, publication or runtime authority.
Snapshots bind the sanitized export, NOT an atomic observation of a live tree:
stop/freeze its writer before export. Exclusions are explicit in the manifest.
Check PASS means the fixed test process exited zero, not semantic review; hostile
Python can deliberately exit zero. Candidate stdout is never a check receipt.

Dagger has no asserted network/resource enforcement here. The default policy
refuses before graph execution. A parent-owned policy must actually enforce and
independently verify CHECKER_CONTROLS, bound to this exact graph, before execution.
The root supervisor and private captures must be inaccessible to the test UID.
"""

import asyncio
import base64
from dataclasses import dataclass
import difflib
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys
from typing import Protocol

try:
    from . import bounded_exec
except ImportError:  # -I deliberately excludes even the script directory.
    import importlib.util
    _spec = importlib.util.spec_from_file_location(
        "bounded_exec", Path(__file__).with_name("bounded_exec.py"))
    bounded_exec = importlib.util.module_from_spec(_spec)
    _spec.loader.exec_module(bounded_exec)


class ArtifactError(ValueError):
    """Fixed secret-safe validation code (never includes source paths/content)."""


@dataclass(frozen=True)
class Limits:
    files: int = 2048
    depth: int = 16
    per_file: int = 1024 * 1024
    total: int = 8 * 1024 * 1024
    diff: int = 256 * 1024
    patch: int = 12 * 1024 * 1024

    def __post_init__(self):
        for name in self.__dataclass_fields__:
            if type(getattr(self, name)) is not int or getattr(self, name) <= 0:
                raise ArtifactError("invalid_limits")
        # Callers may tighten, never expand, the retained profile.
        caps = (2048, 16, 1048576, 8388608, 262144, 12582912)
        if any(getattr(self, name) > cap for name, cap in
               zip(self.__dataclass_fields__, caps)):
            raise ArtifactError("invalid_limits")


DEFAULT_LIMITS = Limits()
PROFILE = "python-stdlib-unittest-discover-v1"
# Instructions, agent plugins and ambient executable/tool configuration are not
# exported. Ordinary source, tests, package manifests and report assets remain.
EXCLUDED = frozenset({
    ".git", ".hg", ".svn", "__pycache__", ".venv", "venv", "node_modules",
    ".ssh", ".aws", ".azure", ".kube", ".gnupg", ".config", ".docker",
    ".opencode", "opencode.json", "opencode.jsonc", "agents.md", "claude.md",
    ".mcp.json", ".env", ".netrc", ".npmrc", ".pypirc", ".gitconfig",
    ".gitmodules", "sitecustomize.py", "usercustomize.py", "conftest.py",
    "pytest.ini", "tox.ini", "setup.cfg", "setup.py", ".github", ".gitlab",
})
SECRET = re.compile(r"(?:.*\.(?:pem|key|p12|pfx|keystore)|id_(?:rsa|dsa|ecdsa|ed25519)(?:\.pub)?|credentials(?:\..*)?|secrets?(?:\..*)?|\.env(?:\..*)?)", re.I)


def _json(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=True,
                      separators=(",", ":"), allow_nan=False).encode("ascii")


def _hash(raw):
    return hashlib.sha256(raw).hexdigest()


def _path(value, limits=DEFAULT_LIMITS):
    if (type(value) is not str or not value or len(value.encode("utf-8")) > 1024
            or "\\" in value or any(ord(c) < 32 or ord(c) == 127 for c in value)):
        raise ArtifactError("unsafe_path")
    parts = value.split("/")
    if len(parts) > limits.depth or any(p in ("", ".", "..") for p in parts):
        raise ArtifactError("unsafe_path")
    return parts


def _excluded(parts):
    return any(p.casefold() in EXCLUDED or SECRET.fullmatch(p)
               or p.casefold().endswith(".pth") for p in parts)


@dataclass(frozen=True)
class Snapshot:
    """Immutable exact-byte export; construct with snapshot_files/export_snapshot."""

    files: tuple[tuple[str, bytes], ...]
    excluded: tuple[str, ...] = ()

    def manifest(self):
        return {"format": "smith.retained.manifest.v1",
                "files": [{"path": p, "size": len(b), "sha256": _hash(b)}
                          for p, b in self.files],
                "excluded": list(self.excluded)}

    @property
    def sha256(self):
        return _hash(_json(self.manifest()))


def snapshot_files(files, *, excluded=(), limits=DEFAULT_LIMITS):
    """Validate an in-memory export; no silent omissions or mutable byte buffers."""
    if type(files) not in (dict, tuple):
        raise ArtifactError("invalid_snapshot")
    items = tuple(files.items()) if type(files) is dict else files
    if len(items) > limits.files:
        raise ArtifactError("file_count_limit")
    retained, seen, total = [], set(), 0
    for item in items:
        if type(item) is not tuple or len(item) != 2:
            raise ArtifactError("invalid_snapshot")
        path, raw = item
        parts = _path(path, limits)
        if _excluded(parts) or type(raw) is not bytes:
            raise ArtifactError("forbidden_file")
        if path in seen or any("/".join(parts[:i]) in seen for i in range(1, len(parts))):
            raise ArtifactError("path_collision")
        if len(raw) > limits.per_file:
            raise ArtifactError("per_file_limit")
        total += len(raw)
        if total > limits.total:
            raise ArtifactError("total_limit")
        seen.add(path)
        retained.append((path, raw))
    for path in seen:
        if any(path.startswith(other + "/") for other in seen if other != path):
            raise ArtifactError("path_collision")
    exclusions = tuple(excluded)
    if len(exclusions) + len(items) > limits.files or len(set(exclusions)) != len(exclusions):
        raise ArtifactError("file_count_limit")
    for path in exclusions:
        if not _excluded(_path(path, limits)) or path in seen:
            raise ArtifactError("invalid_exclusion")
    return Snapshot(tuple(sorted(retained)), tuple(sorted(exclusions)))


def validate_snapshot(snapshot, *, limits=DEFAULT_LIMITS):
    if type(snapshot) is not Snapshot:
        raise ArtifactError("invalid_snapshot")
    clean = snapshot_files(snapshot.files, excluded=snapshot.excluded, limits=limits)
    if clean != snapshot:
        raise ArtifactError("noncanonical_snapshot")
    return clean


def _stable(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_nlink,
            info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def export_snapshot(root, *, limits=DEFAULT_LIMITS):
    """Bounded fd-relative nofollow export, rejecting links/devices/hardlinks.

    Even excluded entries are lstat'd and symlinks/special files refused. Excluded
    directories are not traversed. All visited entries, including directories,
    consume the count budget. A concurrent mutation observed anywhere refuses.
    The owner must quiesce the source; this is not a filesystem snapshot primitive.
    """
    files, excluded, count, total = {}, [], 0, 0

    def walk(fd, prefix):
        nonlocal count, total
        before = _stable(os.fstat(fd))
        with os.scandir(fd) as entries:
            for entry in entries:
                count += 1
                if count > limits.files:
                    raise ArtifactError("file_count_limit")
                path = prefix + entry.name
                parts = _path(path, limits)
                info = os.stat(entry.name, dir_fd=fd, follow_symlinks=False)
                directory = stat.S_ISDIR(info.st_mode)
                if not directory and (not stat.S_ISREG(info.st_mode) or info.st_nlink != 1):
                    raise ArtifactError("unsafe_file_type")
                if _excluded(parts):
                    excluded.append(path)
                    continue
                flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
                if directory:
                    flags |= os.O_DIRECTORY
                child = os.open(entry.name, flags, dir_fd=fd)
                try:
                    if _stable(info) != _stable(os.fstat(child)):
                        raise ArtifactError("source_changed")
                    if directory:
                        walk(child, path + "/")
                    else:
                        if info.st_size > limits.per_file:
                            raise ArtifactError("per_file_limit")
                        total += info.st_size
                        if total > limits.total:
                            raise ArtifactError("total_limit")
                        chunks, size = [], 0
                        while True:
                            raw = os.read(child, min(65536, limits.per_file + 1 - size))
                            if not raw:
                                break
                            chunks.append(raw)
                            size += len(raw)
                            if size > limits.per_file:
                                raise ArtifactError("per_file_limit")
                        if size != info.st_size:
                            raise ArtifactError("source_changed")
                        files[path] = b"".join(chunks)
                    if _stable(info) != _stable(os.fstat(child)):
                        raise ArtifactError("source_changed")
                    if _stable(info) != _stable(os.stat(entry.name, dir_fd=fd, follow_symlinks=False)):
                        raise ArtifactError("source_changed")
                finally:
                    os.close(child)
        if before != _stable(os.fstat(fd)):
            raise ArtifactError("source_changed")

    try:
        fd = bounded_exec._directory(root)
        try:
            walk(fd, "")
        finally:
            os.close(fd)
    except (OSError, UnicodeError):
        raise ArtifactError("unsafe_source") from None
    return snapshot_files(files, excluded=excluded, limits=limits)


def patch_for(base, candidate, *, limits=DEFAULT_LIMITS):
    """Canonical complete patch: UTF-8 unified diff plus exact replacements.

    Binary changes have explicit binary-replacement entries, never a pretend
    text diff. Missing final newlines are marked. Oversize patches/diffs refuse;
    there is no accepted truncation. Replacements make reconstruction exact.
    """
    validate_snapshot(base, limits=limits)
    validate_snapshot(candidate, limits=limits)
    before, after = dict(base.files), dict(candidate.files)
    changes, lines, used = [], [], 0
    for path in sorted(before.keys() | after.keys()):
        old, new = before.get(path), after.get(path)
        if old == new:
            continue
        binary = False
        try:
            a = (old or b"").decode("utf-8", "strict")
            b = (new or b"").decode("utf-8", "strict")
            binary = "\0" in a or "\0" in b
        except UnicodeError:
            binary = True
        if not binary:
            for line in difflib.unified_diff(a.splitlines(keepends=True), b.splitlines(keepends=True),
                                             fromfile="a/" + path if old is not None else "/dev/null",
                                             tofile="b/" + path if new is not None else "/dev/null"):
                if not line.endswith("\n"):
                    line += "\n\\ No newline at end of file\n"
                used += len(line.encode("utf-8"))
                if used > limits.diff:
                    raise ArtifactError("diff_limit")
                lines.append(line)
        changes.append({"path": path, "base_sha256": None if old is None else _hash(old),
                        "candidate_sha256": None if new is None else _hash(new),
                        "kind": "binary" if binary else "utf8",
                        "replacement_b64": None if new is None else base64.b64encode(new).decode("ascii")})
    patch = {"format": "smith.retained.patch.v1", "base_sha256": base.sha256,
             "candidate_sha256": candidate.sha256, "candidate_excluded": list(candidate.excluded),
             "changes": changes, "diff": "".join(lines)}
    if len(_json(patch)) > limits.patch:
        raise ArtifactError("patch_limit")
    return patch


def validate_patch(base, candidate, patch, *, limits=DEFAULT_LIMITS):
    """Recompute the entire diff/replacements/bindings; supplied hashes aren't proof."""
    expected = patch_for(base, candidate, limits=limits)
    if type(patch) is not dict or patch != expected:
        raise ArtifactError("patch_mismatch")
    return _hash(_json(expected))


def reconstruct(base, patch, *, limits=DEFAULT_LIMITS):
    validate_snapshot(base, limits=limits)
    try:
        if type(patch) is not dict or len(_json(patch)) > limits.patch:
            raise ArtifactError("invalid_patch")
        files = dict(base.files)
        if type(patch["changes"]) is not list or len(patch["changes"]) > limits.files * 2:
            raise ArtifactError("invalid_patch")
        for change in patch["changes"]:
            path = change["path"]
            _path(path, limits)
            raw = change["replacement_b64"]
            if raw is None:
                files.pop(path)
            else:
                if type(raw) is not str or len(raw) > (limits.per_file + 2) // 3 * 4:
                    raise ArtifactError("invalid_patch")
                files[path] = base64.b64decode(raw, validate=True)
        candidate = snapshot_files(files, excluded=patch["candidate_excluded"], limits=limits)
        validate_patch(base, candidate, patch, limits=limits)
        return candidate
    except (KeyError, TypeError, ValueError, UnicodeError):
        raise ArtifactError("patch_mismatch") from None


# Fixed script outside the candidate. Discovery imports untrusted Python only in
# the separate checker process. No model command, setup.py, pip or shell is used.
DISCOVER = '''import sys, unittest
sys.path.insert(0, "/workspace")
suite = unittest.TestLoader().discover("/workspace", pattern="test*.py", top_level_dir="/workspace")
if suite.countTestCases() == 0:
    sys.exit(11)
result = unittest.TextTestRunner(verbosity=1).run(suite)
sys.exit(0 if result.wasSuccessful() else 10)
'''
CHECKER_CONTROLS = (
    "deny_all_network_including_loopback_and_metadata", "no_host_mounts_sockets_or_secrets",
    "bounded_memory_cpu_pids_disk", "unprivileged_tests_no_privilege_escalation",
    "readonly_trusted_code_and_root_private_captures", "owned_engine_cleanup",
)


@dataclass(frozen=True)
class CheckResult:
    snapshot_sha256: str
    profile_sha256: str
    policy_receipt: str
    state: str
    reason: str
    exit_code: int | None
    stdout_sha256: str
    stderr_sha256: str


def profile_sha256():
    # Bind supervisor implementation AND bounded_exec, not merely test argv.
    return _hash(_json({"profile": PROFILE, "driver": DISCOVER,
                       "supervisor": _hash(Path(__file__).read_bytes()),
                       "bounded_exec": _hash(Path(bounded_exec.__file__).read_bytes()),
                       "seconds": 30, "controls": CHECKER_CONTROLS}))


@dataclass(frozen=True)
class CheckEvidence:
    """Only a trusted supervisor/runtime may construct this, never candidate JSON.

    verify_check recomputes the structured result, not the historical execution.
    The parent must retain provenance of evidence and the external policy receipt.
    Re-execution, when required, goes through run_check on a new checker graph.
    """

    snapshot_sha256: str
    profile_sha256: str
    policy_receipt: str
    status: bytes
    stdout: bytes
    stderr: bytes


def result_from_evidence(snapshot, evidence):
    validate_snapshot(snapshot)
    if (type(evidence) is not CheckEvidence or evidence.snapshot_sha256 != snapshot.sha256
            or evidence.profile_sha256 != profile_sha256()
            or re.fullmatch(r"[0-9a-f]{64}", evidence.policy_receipt) is None):
        raise ArtifactError("check_binding_mismatch")
    if (type(evidence.status) is not bytes or len(evidence.status) > 1024
            or type(evidence.stdout) is not bytes or len(evidence.stdout) > bounded_exec.STDOUT_LIMIT
            or type(evidence.stderr) is not bytes or len(evidence.stderr) > bounded_exec.STDERR_LIMIT):
        raise ArtifactError("invalid_check_evidence")
    try:
        status = json.loads(evidence.status)
        code, reason = status["exit_code"], status["reason"]
        if (set(status) != {"exit_code", "reason"} or reason not in bounded_exec.REASONS
                or code is not None and type(code) is not int
                or evidence.status != _json(status) + b"\n"):
            raise ValueError()
    except (ValueError, KeyError, TypeError):
        raise ArtifactError("invalid_check_evidence") from None
    state = "unknown"
    if reason == "exited" and type(code) is int and code in (0, 10, 11):
        state = "passed" if code == 0 else "failed"
        if code == 11:
            reason = "no_tests"
    return CheckResult(snapshot.sha256, evidence.profile_sha256, evidence.policy_receipt,
                       state, reason, code, _hash(evidence.stdout), _hash(evidence.stderr))


def verify_check(snapshot, result, evidence):
    expected = result_from_evidence(snapshot, evidence)
    if type(result) is not CheckResult or result != expected:
        raise ArtifactError("check_result_mismatch")
    return expected


class RuntimePolicy(Protocol):
    """Trusted external enforcement, NOT a supplied-model boolean/JSON assertion.

    prepare must apply controls to this fresh graph before ANY exec. verify must
    independently inspect enforcement and bind an opaque receipt to the exact
    prepared graph and controls. No implementation/enforcement ships here.
    """

    async def prepare(self, container, controls: tuple[str, ...]): ...
    async def verify(self, container, controls: tuple[str, ...]) -> str: ...


class BlockedPolicy:
    async def prepare(self, container, controls):
        raise ArtifactError("runtime_controls_unimplemented")

    async def verify(self, container, controls):
        raise ArtifactError("runtime_controls_unimplemented")


def _payload(snapshot):
    validate_snapshot(snapshot)
    return _json({"files": [[p, base64.b64encode(b).decode("ascii")] for p, b in snapshot.files],
                  "excluded": list(snapshot.excluded), "sha256": snapshot.sha256}).decode("ascii")


async def checker_graph(dag, snapshot, *, image, policy=None, any_exit):
    """Build a NEW checker, never extend/bind/mount the OpenCode worker.

    image is a trusted pre-provisioned Python+setpriv image pinned by sha256.
    Pulling/preparing it remains parent-owned authority. Dagger network/resource
    guarantees are NOT inferred from absence of service bindings. Policy must
    enforce them externally; no fake SDK network/resource switches are invented.
    Returns (executed graph, verified opaque policy receipt).
    """
    validate_snapshot(snapshot)
    if type(image) is not str or re.fullmatch(r"[a-zA-Z0-9./:_-]+@sha256:[0-9a-f]{64}", image) is None:
        raise ArtifactError("unpinned_checker_image")
    policy = BlockedPolicy() if policy is None else policy
    node = (dag.container().from_(image).with_user("0:0").with_workdir("/")
            .with_new_file("/trusted/retained_artifacts.py", contents=Path(__file__).read_text(), permissions=0o444)
            .with_new_file("/trusted/bounded_exec.py", contents=Path(bounded_exec.__file__).read_text(), permissions=0o444)
            .with_new_file("/trusted/input.json", contents=_payload(snapshot), permissions=0o400)
            .with_new_file("/trusted/discover.py", contents=DISCOVER, permissions=0o444))
    node = await policy.prepare(node, CHECKER_CONTROLS)
    receipt = await policy.verify(node, CHECKER_CONTROLS)
    if type(receipt) is not str or re.fullmatch(r"[0-9a-f]{64}", receipt) is None:
        raise ArtifactError("invalid_policy_receipt")
    # Empty environment; image entrypoint is bypassed. No model-owned argv.
    node = node.with_exec(["/usr/bin/env", "-i", "PATH=/usr/bin:/bin",
                           "/usr/bin/python3", "-I", "-B", "/trusted/retained_artifacts.py"],
                          use_entrypoint=False, expect=any_exit)
    return node, receipt


async def run_check(dag, snapshot, *, image, any_exit, policy=None):
    """Run fixed profile with 40s outer observation bound; errors remain unknown.

    A graph/RPC timeout is NOT engine cleanup proof. The external policy/owner
    must retain ownership and reconcile cleanup; do not retry automatically.
    """
    async def execute():
        node, receipt = await checker_graph(dag, snapshot, image=image,
                                            policy=policy, any_exit=any_exit)
        supervisor_exit = await node.exit_code()
        if supervisor_exit != 0:
            raise ArtifactError("checker_supervisor_unknown")
        async def capture(name, cap):
            file = node.file("/private/" + name)
            if await file.size() > cap:
                raise ArtifactError("invalid_check_evidence")
            # Captures are base64: arbitrary untrusted test output need not UTF-8.
            value = await file.contents()
            if len(value) > cap:
                raise ArtifactError("invalid_check_evidence")
            return base64.b64decode(value, validate=True)
        evidence = CheckEvidence(snapshot.sha256, profile_sha256(), receipt,
                                 await capture("status.b64", 2048),
                                 await capture("stdout.b64", 4 * ((bounded_exec.STDOUT_LIMIT + 2) // 3)),
                                 await capture("stderr.b64", 4 * ((bounded_exec.STDERR_LIMIT + 2) // 3)))
        return result_from_evidence(snapshot, evidence), evidence
    try:
        return await asyncio.wait_for(execute(), timeout=40)
    except ArtifactError:
        raise
    except Exception:
        raise ArtifactError("checker_supervisor_unknown") from None


def _supervise():
    """Root-only container entry. Never invoked as a local candidate launcher."""
    if os.getuid() != 0:
        raise ArtifactError("checker_supervisor_unknown")
    payload = json.loads(Path("/trusted/input.json").read_bytes())
    snapshot = snapshot_files({p: base64.b64decode(b, validate=True) for p, b in payload["files"]},
                              excluded=payload["excluded"])
    if snapshot.sha256 != payload["sha256"]:
        raise ArtifactError("check_binding_mismatch")
    os.mkdir("/workspace", 0o755)
    os.mkdir("/private", 0o700)
    # Fresh roots in a trusted image, no candidate links; only plain byte files.
    for path, raw in snapshot.files:
        target = Path("/workspace") / path
        target.parent.mkdir(parents=True, exist_ok=True)
        fd = os.open(target, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o444)
        with os.fdopen(fd, "wb") as file:
            file.write(raw)
    status = bounded_exec.run([
        "/usr/bin/setpriv", "--reuid=65534", "--regid=65534", "--clear-groups",
        "--no-new-privs", "--bounding-set=-all", "/usr/bin/python3", "-I", "-B",
        "/trusted/discover.py"], seconds=30, workspace="/workspace", private="/private",
        stdout="/private/stdout", stderr="/private/stderr", status="/private/status")
    # This publication is in the trusted parent, not in the discovery process.
    Path("/private/status").write_bytes(_json(status) + b"\n")
    for name in ("stdout", "stderr", "status"):
        Path("/private/" + name + ".b64").write_bytes(base64.b64encode(Path("/private/" + name).read_bytes()))


if __name__ == "__main__":
    try:
        _supervise()
    except Exception:
        sys.exit(2)
