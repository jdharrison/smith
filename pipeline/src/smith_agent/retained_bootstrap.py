"""Owner-prepared retained bootstrap, not a live-engine implementation.

Integration calls load_prepared(work_root, authority=...) explicitly. Only
work_root/prepared.json is considered; missing means unavailable. No stdin,
ambient environment switch, SDK auto-discovery, engine start or download exists.

Wire v1 (all objects have exact keys):
 profile = {v, registry, launcher, sdk_version, image, engine, approval}
 registry = {path, sha256}; its file is {v, sources: [entry, ...]}.
 entry = {v, source_id, project_sha256, repositories, files, manifest_sha256,
          snapshot}. snapshot is a work-root-relative private directory.
 repository = {path, url, reference, commit, read_only}.
 file = {path, sha256, size, executable}. Paths are POSIX-relative.
 manifest_sha256 = SHA256(canonical JSON of entry excluding snapshot and
 manifest_sha256); canonical JSON is protocol.encode (ASCII, sorted keys).
 launcher = {interpreter: {path, sha256}, cli: {path, sha256}, trees: [...]}.
 tree = {role: python|dependencies|agent, root, files: [file, ...]}.
 Exactly one complete tree of each role is required (including stdlib/native
 dependencies, SDK/transitives and agent/CLI support files, not just .py files).
 Interpreter, CLI and tree roots are absolute, non-symlink paths. The trusted
 authority must independently verify that these inventories cover the *actual*
 launcher/import/native-library closure; a manifest cannot attest completeness.
 image is an immutable name@sha256:digest. engine and approval are opaque bounded
 identities, never commands, endpoints, credentials or booleans enabling policy.

PreparedAuthority is installed trusted code, NEVER loaded from these documents.
Its callbacks must raise on missing/stale evidence. verify_preparation binds the
actual interpreter/import closure, CLI, SDK, locally prepared image and already
running engine to the exact profile digest. verify_live independently verifies
owner/signed approval (revision/effect/destinations/expiry), engine network and
resource enforcement, provider denial, owned cleanup capability and incarnation
against the supplied scope, immediately before effects. It may not accept JSON
claims as observations. No implementation is provided: default remains blocked.
"""

from contextlib import asynccontextmanager
from dataclasses import dataclass, replace
import os
from pathlib import Path, PurePosixPath
import re
import stat
import tempfile
from types import MappingProxyType
from typing import Protocol

from . import retained as r
from .protocol import Invalid, decode, digest, encode, fields, sha256, text_bytes

MAX_JSON = 4 * 1024 * 1024
MAX_FILES = 8192
MAX_FILE = 64 * 1024 * 1024
MAX_TOTAL = 256 * 1024 * 1024
MAX_SOURCES = 16


class PreparationError(ValueError):
    """Fixed diagnostic; no paths or untrusted values in public errors."""


def _require(condition):
    if not condition:
        raise PreparationError("invalid_preparation")


def _text(value, limit=1024):
    raw = text_bytes(value, limit)
    _require(bool(raw) and all(ord(c) >= 32 and ord(c) != 127 for c in value))
    return value


def _relative(value, *, dot=False):
    _text(value)
    path = PurePosixPath(value)
    _require(not path.is_absolute() and path.as_posix() == value
             and ".." not in path.parts and "\\" not in value
             and (dot or value != "."))
    return path


def _private(info):
    _require(info.st_uid == os.getuid() and not info.st_mode & 0o077)


def _safe_ancestor(info):
    _require(stat.S_ISDIR(info.st_mode) and info.st_uid in (0, os.getuid())
             and not info.st_mode & 0o022)


def _directory(path, *, private):
    """Walk every ancestor by descriptor: no symlink or writable-parent bypass."""
    path = Path(path)
    _require(path.is_absolute() and str(path) == os.path.normpath(str(path)))
    fd = os.open("/", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        for part in path.parts[1:]:
            parent = os.fstat(fd)
            # /tmp-like sticky root-owned parents are safe only for a private,
            # owner-controlled next component. All other writable parents fail.
            sticky = (parent.st_uid == 0 and parent.st_mode & stat.S_ISVTX)
            if not sticky:
                _safe_ancestor(parent)
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                            dir_fd=fd)
            os.close(fd)
            fd = child
            if sticky:
                _private(os.fstat(fd))
        info = os.fstat(fd)
        if private:
            _private(info)
        else:
            _safe_ancestor(info)
        return fd
    except BaseException:
        os.close(fd)
        raise


def _read_at(root, path, limit, *, private):
    parts = _relative(path).parts
    fd = os.dup(root)
    try:
        for part in parts[:-1]:
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                            dir_fd=fd)
            os.close(fd)
            fd = child
            if private:
                _private(os.fstat(fd))
            else:
                _safe_ancestor(os.fstat(fd))
        leaf = os.open(parts[-1], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK,
                       dir_fd=fd)
        try:
            before = os.fstat(leaf)
            _require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1
                     and before.st_size <= limit and not before.st_mode & 0o7000)
            if private:
                _private(before)
            else:
                _require(before.st_uid in (0, os.getuid()) and not before.st_mode & 0o022)
            chunks, count = [], 0
            while True:
                chunk = os.read(leaf, min(65536, limit + 1 - count))
                if not chunk:
                    break
                chunks.append(chunk)
                count += len(chunk)
                _require(count <= limit)
            after = os.fstat(leaf)
            _require((before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns,
                      before.st_ctime_ns) ==
                     (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns,
                      after.st_ctime_ns))
            return b"".join(chunks), bool(before.st_mode & 0o111)
        finally:
            os.close(leaf)
    finally:
        os.close(fd)


def _json_at(fd, path):
    return decode(_read_at(fd, path, MAX_JSON, private=True)[0], limit=MAX_JSON)


# Refuse active configuration/credential inputs rather than silently dropping
# them and importing a tree that no longer matches its signed manifest.
FORBIDDEN = frozenset({
    ".git", ".hg", ".svn", ".ssh", ".aws", ".azure", ".gnupg", ".kube",
    ".opencode", ".claude", ".cursor", ".mcp.json", ".npmrc", ".pypirc",
    ".netrc", "auth.json", "credentials.json", "credentials", "id_rsa",
    "id_ed25519", "agents.md", "claude.md", "gemini.md", "opencode.json",
    "opencode.jsonc", "dagger.json", ".envrc", ".pythonrc", "sitecustomize.py",
    "usercustomize.py",
})


def _source_path(path):
    parts = _relative(path).parts
    _require(len(parts) <= 32)
    for part in parts:
        lower = part.lower()
        _require(lower not in FORBIDDEN and not lower.startswith(".env")
                 and not lower.endswith((".pem", ".key", ".p12", ".pfx", ".pth")))


@dataclass(frozen=True)
class FileRecord:
    path: str
    sha256: str
    size: int
    executable: bool


def _records(value, *, source):
    _require(type(value) is list and 1 <= len(value) <= MAX_FILES)
    records = []
    for item in value:
        fields(item, ("path", "sha256", "size", "executable"))
        _relative(item["path"])
        if source:
            _source_path(item["path"])
        sha256(item["sha256"])
        _require(type(item["size"]) is int and 0 <= item["size"] <= MAX_FILE
                 and type(item["executable"]) is bool)
        records.append(FileRecord(**item))
    paths = [item.path for item in records]
    _require(paths == sorted(set(paths)) and sum(item.size for item in records) <= MAX_TOTAL)
    return tuple(records)


def _inventory(fd, *, private, source=False, prefix="", depth=0, budget=None):
    _require(depth <= 32)
    if budget is None:
        budget = [MAX_FILES * 2]
    result = set()
    for name in os.listdir(fd):
        budget[0] -= 1
        _require(budget[0] >= 0)
        path = prefix + name
        _relative(path)
        if source:
            _source_path(path)
        info = os.stat(name, dir_fd=fd, follow_symlinks=False)
        if stat.S_ISDIR(info.st_mode):
            child = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
            try:
                if private:
                    _private(os.fstat(child))
                else:
                    _safe_ancestor(os.fstat(child))
                result.update(_inventory(child, private=private, source=source, prefix=path + "/",
                                                         depth=depth + 1, budget=budget))
            finally:
                os.close(child)
        else:
            _require(stat.S_ISREG(info.st_mode))
            result.add(path)
        _require(len(result) <= MAX_FILES)
    return result


def _tree(fd, records, *, private, source=False):
    _require(_inventory(fd, private=private, source=source) == {item.path for item in records})
    result = []
    for item in records:
        raw, executable = _read_at(fd, item.path, MAX_FILE, private=private)
        _require(len(raw) == item.size and digest(raw) == item.sha256
                 and executable == item.executable)
        result.append((item.path, raw, executable))
    _require(_inventory(fd, private=private, source=source) == {item.path for item in records})
    return tuple(result)


@dataclass(frozen=True)
class PreparedSource:
    source_id: str
    project_sha256: str
    manifest_sha256: str
    repositories: tuple[r.RepositoryCapability, ...]
    repository_json: str
    files: tuple[tuple[str, bytes, bool], ...]


@dataclass(frozen=True)
class Preparation:
    profile_sha256: str
    registry_sha256: str
    profile_json: str
    sources: tuple[PreparedSource, ...]


@dataclass(frozen=True)
class ApprovalScope:
    profile_sha256: str
    registry_sha256: str
    job_id: str
    source_id: str
    project_sha256: str
    manifest_sha256: str
    source_binding: str
    mode: str
    effect: str = "create"
    operation_id: str | None = None
    text_sha256: str | None = None
    review: bool = False


class PreparedAuthority(Protocol):
    async def verify_preparation(self, preparation: Preparation) -> None: ...
    async def verify_live(self, preparation: Preparation, scope: ApprovalScope) -> None: ...
    def connection(self, preparation: Preparation):
        """Verified SDK context for an existing engine; never provision/start it."""
        ...
    async def verify_cleanup(self, preparation: Preparation, scope: ApprovalScope,
                             target: r.CleanupTarget) -> bool: ...


def _launcher(value):
    fields(value, ("interpreter", "cli", "trees"))
    for name in ("interpreter", "cli"):
        item = fields(value[name], ("path", "sha256"))
        _text(item["path"])
        sha256(item["sha256"])
        path = Path(item["path"])
        _require(path.is_absolute())
        fd = _directory(path.parent, private=False)
        try:
            raw, executable = _read_at(fd, path.name, MAX_FILE, private=False)
            _require(executable and digest(raw) == item["sha256"])
        finally:
            os.close(fd)
    trees = value["trees"]
    _require(type(trees) is list and len(trees) == 3)
    roles = []
    roots = []
    for tree in trees:
        fields(tree, ("role", "root", "files"))
        roles.append(tree["role"])
        root = Path(_text(tree["root"]))
        _require(root.is_absolute() and all(root != old and root not in old.parents
                                          and old not in root.parents for old in roots))
        roots.append(root)
        records = _records(tree["files"], source=False)
        fd = _directory(root, private=False)
        try:
            _tree(fd, records, private=False)
        finally:
            os.close(fd)
    _require(sorted(roles) == ["agent", "dependencies", "python"])


def _load(work_root):
    fd = _directory(work_root, private=True)
    try:
        try:
            raw = _read_at(fd, "prepared.json", MAX_JSON, private=True)[0]
        except FileNotFoundError:
            return None
        profile = fields(decode(raw, limit=MAX_JSON), (
            "v", "registry", "launcher", "sdk_version", "image", "engine", "approval"))
        _require(type(profile["v"]) is int and profile["v"] == 1
                 and profile["sdk_version"] == r.DAGGER_VERSION)
        _require(re.fullmatch(r"[a-z0-9][a-z0-9./:_-]*@sha256:[0-9a-f]{64}",
                              _text(profile["image"])) is not None)
        for key in ("engine", "approval"):
            _require(re.fullmatch(r"[A-Za-z0-9._:-]{1,128}", _text(profile[key])) is not None)
        registry = fields(profile["registry"], ("path", "sha256"))
        _relative(registry["path"])
        sha256(registry["sha256"])
        raw_registry = _read_at(fd, registry["path"], MAX_JSON, private=True)[0]
        _require(digest(raw_registry) == registry["sha256"])
        document = fields(decode(raw_registry, limit=MAX_JSON), ("v", "sources"))
        _require(type(document["v"]) is int and document["v"] == 1
                 and type(document["sources"]) is list
                 and 1 <= len(document["sources"]) <= MAX_SOURCES)
        sources = []
        for entry in document["sources"]:
            fields(entry, ("v", "source_id", "project_sha256", "repositories", "files",
                           "manifest_sha256", "snapshot"))
            _require(type(entry["v"]) is int and entry["v"] == 1)
            r.identity(entry["source_id"])
            sha256(entry["project_sha256"])
            sha256(entry["manifest_sha256"])
            manifest = {k: v for k, v in entry.items() if k not in ("snapshot", "manifest_sha256")}
            _require(digest(encode(manifest).encode("ascii")) == entry["manifest_sha256"])
            repos = entry["repositories"]
            _require(type(repos) is list and 1 <= len(repos) <= 16)
            caps = []
            for repo in repos:
                fields(repo, ("path", "url", "reference", "commit", "read_only"))
                _relative(repo["path"], dot=True)
                # Public HTTPS metadata only; never fetched or executed here.
                _require(re.fullmatch(r"https://[A-Za-z0-9.-]+(?::443)?/[A-Za-z0-9._/~%-]+",
                                      _text(repo["url"])) is not None)
                _text(repo["reference"])
                _require(re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", _text(repo["commit"])) is not None
                         and type(repo["read_only"]) is bool)
                caps.append(r.RepositoryCapability(repo["path"], repo["read_only"]))
            caps = r.checked_capabilities(tuple(caps))
            records = _records(entry["files"], source=True)
            for item in records:
                _require(sum(repo.path == "." or item.path.startswith(repo.path + "/")
                             for repo in caps) == 1)
            _relative(entry["snapshot"])
            snapshot = _directory(Path(work_root) / entry["snapshot"], private=True)
            try:
                contents = _tree(snapshot, records, private=True, source=True)
            finally:
                os.close(snapshot)
            sources.append(PreparedSource(entry["source_id"], entry["project_sha256"],
                                          entry["manifest_sha256"], caps, encode(repos), contents))
        _require(len({s.source_id for s in sources}) == len(sources)
                 and sum(len(raw) for s in sources for _, raw, _ in s.files) <= MAX_TOTAL)
        _launcher(profile["launcher"])
        return Preparation(digest(raw), digest(raw_registry), encode(profile), tuple(sources))
    finally:
        os.close(fd)


def load_prepared(work_root, *, authority: PreparedAuthority | None = None):
    """Return a concrete runtime, or None for absent profile. Invalid fails closed.

    Construction performs bounded local reads only, including all declared
    interpreter dependency and CLI bytes. No authority means create is denied.
    Parent owns main integration and independent production verifier installation.
    """
    try:
        preparation = _load(work_root)
        return None if preparation is None else PreparedDaggerRuntime(preparation, authority)
    except (OSError, Invalid, r.PolicyBlocked, ValueError, TypeError, RecursionError):
        raise PreparationError("invalid_preparation") from None


class PreparedDaggerRuntime(r.DaggerRuntime):
    def __init__(self, preparation: Preparation, authority: PreparedAuthority | None):
        self.preparation = preparation
        self.authority = authority
        self.scope = None
        self.attempted = False
        self._staging = None
        self._prepared = {s.source_id: s for s in preparation.sources}
        super().__init__(self._connection, self._image, MappingProxyType({}),
                         source_validator=self._validate_source, cleanup_verifier=self._cleanup)

    async def create(self, job_id, mode, source_id):
        r.identity(job_id)
        r.identity(source_id)
        if (self.authority is None or self.attempted or self.closed
                or source_id not in self._prepared or mode not in ("read", "write")):
            raise r.PreEffectPolicyBlocked()
        source = self._prepared[source_id]
        if mode == "write" and any(repo.read_only for repo in source.repositories):
            raise r.PreEffectPolicyBlocked()
        self.attempted = True
        binding = r.source_binding(r.ValidatedSource(source_id, object(),
                                                     source.manifest_sha256, source.repositories))
        self.scope = ApprovalScope(self.preparation.profile_sha256, self.preparation.registry_sha256,
                                   job_id, source_id, source.project_sha256,
                                   source.manifest_sha256, binding, mode)
        try:
            _require(await self.authority.verify_preparation(self.preparation) is None)
            _require(await self.authority.verify_live(self.preparation, self.scope) is None)
        except Exception:
            raise r.PreEffectPolicyBlocked() from None
        # These bytes were hashed and frozen at load; never re-read a checkout.
        self._staging = tempfile.TemporaryDirectory(prefix="smith-prepared-")
        root = Path(self._staging.name)
        try:
            for path, raw, executable in source.files:
                leaf = root / path
                leaf.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                with leaf.open("xb") as stream:
                    stream.write(raw)
                leaf.chmod(0o500 if executable else 0o400)
            self.sources = MappingProxyType({source_id: r.SourceInput(root, source.repositories)})
            await self._start_reviewed(job_id, mode, source_id)
        except BaseException:
            if not self.effects_possible:
                self._discard_staging()
            raise

    async def _validate_source(self, source_id, admitted):
        source = self._prepared[source_id]
        if admitted.repositories != source.repositories or admitted.directory != Path(self._staging.name):
            raise r.PreEffectPolicyBlocked()
        # _start_reviewed validates before opening its connection. The SDK
        # Directory is installed into this holder by our connection wrapper.
        self._directory_holder = _ImportedDirectory()
        return r.ValidatedSource(source_id, self._directory_holder,
                                 source.manifest_sha256, source.repositories)

    @asynccontextmanager
    async def _connection(self):
        _require(await self.authority.verify_live(self.preparation, self.scope) is None)
        async with self.authority.connection(self.preparation) as client:
            source = self._prepared[self.scope.source_id]
            directory = client.host().directory(self._staging.name,
                                                include=[path for path, _, _ in source.files])
            # Force content-addressed import while the private stage exists;
            # pass the immutable returned ID, not an ambient host checkout.
            directory_id = await directory.id()
            self._directory_holder.directory = client.load_directory_from_id(directory_id)
            _require(await self.authority.verify_live(self.preparation, self.scope) is None)
            yield client

    def _image(self, client):
        # service_graph needs an SDK Directory, not a Python forwarding wrapper.
        # Its source validator result already exists, so unwrap through a graph
        # argument adapter on the image instead of modifying the reviewed base.
        image = client.container().from_(decode(self.preparation.profile_json.encode("ascii"),
                                                limit=MAX_JSON)["image"])
        return _ImageArguments(image)

    async def prompt(self, operation_id, text, review):
        r.identity(operation_id)
        raw = text_bytes(text, r.MAX_TEXT)
        if self.authority is None or self.scope is None or type(review) is not bool:
            raise r.PolicyBlocked()
        scope = replace(self.scope, effect="prompt", operation_id=operation_id,
                        text_sha256=digest(raw), review=review)
        try:
            _require(await self.authority.verify_live(self.preparation, scope) is None)
        except Exception:
            raise r.PolicyBlocked() from None
        return await super().prompt(operation_id, text, review)

    async def _cleanup(self, target):
        return await self.authority.verify_cleanup(self.preparation, self.scope, target) is True

    def _discard_staging(self):
        if self._staging is not None:
            self._staging.cleanup()
            self._staging = None

    async def finish(self):
        try:
            return await super().finish()
        finally:
            self._discard_staging()


class _ImportedDirectory:
    directory = None


class _ImageArguments:
    """Unwrap only our import holder when the reviewed graph consumes it."""

    def __init__(self, node):
        self.node = node

    def __getattr__(self, name):
        method = getattr(self.node, name)

        def call(*args, **kwargs):
            args = tuple(arg.directory if isinstance(arg, _ImportedDirectory) else arg for arg in args)
            result = method(*args, **kwargs)
            # Only fluent Container methods remain wrapped. Service/stdout/etc
            # retain their real SDK types and async behavior.
            return _ImageArguments(result) if name.startswith("with_") else result
        return call
