"""Unwired retained-job supervisor; run with python -B -m smith_agent.retained.

JSONL v1 (exact keys; IDs are 32 lowercase hex characters):
  create:  {v, op, operation_id, job_id, mode: read|write, source_id}
  prompt:  {v, op, operation_id, job_id, purpose: work|review, text}
  observe: {v, op, operation_id, job_id, target_operation_id: ID|null}
  finish:  {v, op, operation_id, job_id}
Every response: {v, operation_id, job_id, ok, code, job, operation}.
Job: {state, mode, retained, conversation, ready, blockers} or null.
Operation: {operation_id, op, state, code, text} or null. Observe is query-only.
Create/prompt/finish acknowledge pending; observe the original operation ID.
Exact duplicates return the existing operation; conflicting reuse is refused.

One job/connection per process, 256 consumed mutation IDs, 16KiB prompt,
32KiB input frame and 64KiB output frame (including LF). No durable replay
store: the service owner MUST persist dispatch intent before writing stdin,
retain the process across UI detach, and fence lost-process identities. EOF is
owner loss, NOT finish; no reconnect, service restart, or prompt replay occurs.
Process death cannot guarantee container retention or cleanup. Finish stops the
owned service; it neither approves artifacts nor erases Dagger caches/history.

Official API research (v0.21.9 sdk/python/src/dagger/client/gen.py):
Container.as_service(args, use_entrypoint), with_mounted_directory(read_only),
with_service_binding; Service.start/stop. Service has NO with_exec. The bridge
is a separate fixed client container; all work/state lives in the service.
OpenCode v1.19.3 server/routes/instance/httpapi/groups/session.ts defines
POST /session and POST /session/:sessionID/message. No model/provider is set.

Live admission is deliberately blocked in source, not controlled by JSON or an
environment flag. This module is functional protocol/SDK source, NOT evidence
of live interoperability, enforced egress, resource limits or independent review.
The tool policy does NOT implement general coding: build/test execution, trusted
check results, patch validation and accepted-artifact publication are not supplied.
"""

import asyncio
from contextlib import AsyncExitStack
from dataclasses import dataclass, replace
from pathlib import Path, PurePosixPath
import json
import re
import sys
from typing import Protocol

from .graph import ENVIRONMENT
from .protocol import Invalid, decode, digest, encode, fields, text_bytes

DAGGER_VERSION = "0.21.9"
OPENCODE_VERSION = "1.19.3"
MAX_FRAME = 32768
MAX_REPLY = 65536
MAX_TEXT = 16384
MAX_OPERATIONS = 256
BLOCKERS = (
    "default_provider_egress_unresolved",
    "runtime_resource_enforcement_unresolved",
    "owned_cleanup_verification_unresolved",
    "live_interoperability_unverified",
    "source_containment_unverified",
    "service_incarnation_live_verification_pending",
)
ID = re.compile(r"[0-9a-f]{32}")
SESSION = re.compile(r"ses_[A-Za-z0-9_-]{1,128}")


def identity(value):
    if type(value) is not str or ID.fullmatch(value) is None:
        raise Invalid("invalid_identity")
    return value


def request(raw):
    value = decode(raw, limit=MAX_FRAME - 1)
    if type(value) is not dict or type(value.get("op")) is not str:
        raise Invalid("invalid_request")
    extra = {
        "create": ("mode", "source_id"),
        "prompt": ("purpose", "text"),
        "observe": ("target_operation_id",),
        "finish": (),
    }
    if value["op"] not in extra:
        raise Invalid("invalid_request")
    fields(value, ("v", "op", "operation_id", "job_id", *extra[value["op"]]))
    if type(value["v"]) is not int or value["v"] != 1:
        raise Invalid("invalid_version")
    identity(value["operation_id"])
    identity(value["job_id"])
    if value["op"] == "create":
        identity(value["source_id"])
        if type(value["mode"]) is not str or value["mode"] not in ("read", "write"):
            raise Invalid("invalid_mode")
    elif value["op"] == "prompt":
        if type(value["purpose"]) is not str or value["purpose"] not in ("work", "review"):
            raise Invalid("invalid_purpose")
        if not text_bytes(value["text"], MAX_TEXT).strip():
            raise Invalid("invalid_text")
    elif value["op"] == "observe" and value["target_operation_id"] is not None:
        identity(value["target_operation_id"])
    return value


def frame(value):
    raw = (encode(value) + "\n").encode("ascii")
    if len(raw) > MAX_REPLY:
        raise Invalid("response_limit")
    return raw


@dataclass(frozen=True)
class Outcome:
    state: str
    text: str = ""


class RetainedRuntime(Protocol):
    """Trusted in-process boundary, never a model/JSON supplied implementation.

    create binds a prevalidated immutable snapshot token, not a host path/URL.
    prompt must reuse the live workspace and work conversation after known
    failure. Review must use a fresh conversation without producer history.
    Unknown effects raise; never hide them as a known failed prompt.
    """

    async def create(self, job_id: str, mode: str, source_id: str) -> None: ...
    async def prompt(self, operation_id: str, text: str, review: bool) -> Outcome: ...
    async def finish(self) -> bool: ...


class PolicyBlocked(Exception):
    """Policy refusal does not, by itself, establish absence of effects."""


class PreEffectPolicyBlocked(PolicyBlocked):
    """Trusted runtime proof that CREATE refused before acquiring ownership."""


@dataclass(frozen=True)
class RepositoryCapability:
    path: str
    read_only: bool


@dataclass(frozen=True)
class SourceInput:
    """Trusted registry entry: snapshot plus authoritative repository permissions.

    Paths are relative to the snapshot root, not host paths. The host owner must
    freeze these permissions from the admitted project, never from model text.
    """

    directory: object
    repositories: tuple[RepositoryCapability, ...]


def checked_capabilities(repositories):
    if type(repositories) is not tuple or not 1 <= len(repositories) <= 16:
        raise PreEffectPolicyBlocked()
    paths = []
    for repo in repositories:
        if type(repo) is not RepositoryCapability or type(repo.read_only) is not bool:
            raise PreEffectPolicyBlocked()
        try:
            raw = text_bytes(repo.path, 1024)
        except Invalid:
            raise PreEffectPolicyBlocked() from None
        path = PurePosixPath(repo.path)
        if (not raw or "\\" in repo.path or any(ord(c) < 32 or ord(c) == 127 for c in repo.path)
                or path.is_absolute() or path.as_posix() != repo.path or ".." in path.parts):
            raise PreEffectPolicyBlocked()
        if any(path == other or path in other.parents or other in path.parents for other in paths):
            raise PreEffectPolicyBlocked()
        paths.append(path)
    return repositories


@dataclass(frozen=True)
class ValidatedSource:
    """Result from a trusted source validator, never an stdin attestation.

    The validator must bind an immutable sanitized Directory to its canonical
    manifest digest, bound file/count/total bytes, reject symlinks/special files,
    exclude credentials, OpenCode config/plugins/instructions and Git hooks or
    executable config, and preserve the requested checkout contents otherwise.
    This type checks binding/shape only, NOT those semantic security properties.
    repositories must exactly match the trusted SourceInput permissions, not
    inferred defaults. Write mode currently refuses ANY read-only repository;
    mixed-capability write trees are unsupported rather than silently writable.
    Without the validator, even the reserved SDK start path refuses pre-effect.
    """

    source_id: str
    directory: object
    manifest_sha256: str
    repositories: tuple[RepositoryCapability, ...]


def source_binding(source):
    """Bind snapshot evidence AND the exact verified repository capabilities."""
    checked_capabilities(source.repositories)
    return digest(encode({
        "source_id": source.source_id, "manifest_sha256": source.manifest_sha256,
        "repositories": [{"path": repo.path, "read_only": repo.read_only}
                         for repo in source.repositories],
    }).encode("ascii"))


@dataclass(frozen=True)
class CleanupTarget:
    job_id: str
    source_id: str
    service_id: str
    incarnation: str | None = None


def opencode_config(mode, *, review=False):
    # No bash, task/subagents, external MCP, plugins, installs, Git or web tools.
    # Permissions are defense in depth, not the missing OS egress/resource gates.
    edits = {"*": "deny"}
    if not review:
        edits["/workspace/reports/*"] = "allow"
        if mode == "write":
            edits["/workspace/repo/*"] = "allow"
    return encode({
        "share": "disabled", "permission": {
            "*": "deny", "read": {
                "*": "deny", "/workspace/repo/*": "allow",
                "/workspace/reports/*": "allow",
            },
            "glob": "allow", "grep": "allow", "edit": edits,
        },
    })


def service_graph(image, source, job_id, mode, source_id, source_sha256):
    """Pure graph construction from trusted image and sanitized Directory only.

    _start_reviewed requires a trusted ValidatedSource before calling this.
    Service bindings can restart failed services; the baked proxy's per-boot
    fence rejects the previous incarnation before forwarding mutations. No host
    import or Git fetch exists here. Write gets a copy, not a writable host mount.
    """
    identity(job_id)
    identity(source_id)
    if type(source_sha256) is not str or re.fullmatch(r"[0-9a-f]{64}", source_sha256) is None:
        raise Invalid("invalid_source")
    if mode not in ("read", "write"):
        raise Invalid("invalid_mode")
    # This is package-owned source, not anything from the candidate Directory.
    service_source = Path(__file__).with_name("retained_service.py").read_bytes()
    if len(service_source) > 65536:
        raise Invalid("service_source_limit")
    node = (image.with_user("1000:1000").with_workdir("/workspace")
            .with_env_variable("SMITH_RETAINED_JOB", job_id)
            .with_new_file("/workspace/reports/.keep", contents="",
                           owner="1000:1000", permissions=0o600))
    if mode == "read":
        node = node.with_mounted_directory("/workspace/repo", source, read_only=True)
    else:
        node = node.with_directory("/workspace/repo", source, owner="1000:1000")
    node = node.with_new_file("/trusted/retained_service.py", contents=service_source.decode("utf-8"),
                              owner="root:root", permissions=0o444)
    command = ["/usr/bin/env", "-i", *ENVIRONMENT,
               "OPENCODE_CONFIG_CONTENT=" + opencode_config(mode),
               "/usr/bin/python3", "-I", "-B", "/trusted/retained_service.py",
               job_id, source_id, source_sha256, mode]
    # Internal service port only; never host.tunnel(), up(), privileged nesting
    # or a Docker socket. env -i also discards image/provider environment.
    return node.with_exposed_port(4096).as_service(args=command, use_entrypoint=False)


# This runs ONLY in a fixed bridge container. A bounded response is emitted even
# for remote HTTP failure; no raw diagnostics escape. There is no URL argument.
BRIDGE = '''import json, sys, urllib.request
try:
    q = json.loads(sys.stdin.read(32768))
    if q["path"] == "/_smith/identity":
        if q["expected"] is not None or q["body"] is not None:
            raise ValueError()
        req = urllib.request.Request("http://retained:4096/_smith/identity", method="GET")
    else:
        body = json.dumps(q, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        req = urllib.request.Request("http://retained:4096/_smith/request", data=body,
                                     headers={"Content-Type": "application/json"}, method="POST")
    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, *args, **kwargs):
            raise ValueError()
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    with opener.open(req, timeout=120) as response:
        raw = response.read(60001)
    if len(raw) > 60000:
        raise ValueError()
    result = json.loads(raw.decode("utf-8", errors="strict"))
    out = json.dumps(result, ensure_ascii=True, separators=(",", ":"))
    if len(out) > 60000:
        raise ValueError()
    print(out)
except Exception:
    print('{"kind":"unknown","text":""}')
'''


class DaggerRuntime:
    """Owns one connection and service; no auto-connect on module import.

    connection_factory, image_factory and sources belong to the trusted host
    launcher, never stdin. The launcher must clear host credentials/config,
    pin SDK/CLI bytes and disable traces before connection. Current create
    refuses BEFORE even calling that factory; no bool/flag can enable it.
    """

    def __init__(self, connection_factory=None, image_factory=None, sources=None,
                 *, source_validator=None, cleanup_verifier=None):
        self.connection_factory = connection_factory
        self.image_factory = image_factory
        self.sources = {} if sources is None else sources
        self.source_validator = source_validator
        # Trusted independent verifier(target), not model/stdin data. It must
        # verify exact owned runtime quiescence externally, never infer it from
        # stop/connection exit. No verifier means no successful runtime finish.
        self.cleanup_verifier = cleanup_verifier
        self.expected_identity = None
        self.cleanup_target = None
        self.closed = False
        self.effects_possible = False
        self.stack = AsyncExitStack()
        self.client = self.service = self.bridge = None
        self.session = None
        self.mode = None

    async def create(self, job_id, mode, source_id):
        raise PreEffectPolicyBlocked()

    async def _start_reviewed(self, job_id, mode, source_id):
        """Not an admission entry point; reserved for a future reviewed gate.

        Concrete SDK lifecycle, intentionally unreachable from create/stdin.
        Call only after replacing the unresolved source admission contract.
        """
        identity(job_id)
        identity(source_id)
        if mode not in ("read", "write") or source_id not in self.sources:
            raise Invalid("invalid_source")
        if self.client is not None:
            raise Invalid("already_started")
        if self.source_validator is None:
            raise PreEffectPolicyBlocked()
        admitted = self.sources[source_id]
        if type(admitted) is not SourceInput or admitted.directory is None:
            raise PreEffectPolicyBlocked()
        capabilities = checked_capabilities(admitted.repositories)
        if mode == "write" and any(repo.read_only for repo in capabilities):
            raise PreEffectPolicyBlocked()
        source = await self.source_validator(source_id, admitted)
        if (not isinstance(source, ValidatedSource) or source.source_id != source_id
                or source.directory is None or type(source.manifest_sha256) is not str
                or re.fullmatch(r"[0-9a-f]{64}", source.manifest_sha256) is None
                or checked_capabilities(source.repositories) != capabilities):
            raise PreEffectPolicyBlocked()
        binding = source_binding(source)
        # Connection establishment itself can partially acquire engine resources
        # even if __aenter__ fails before returning a client.
        self.effects_possible = True
        self.client = await self.stack.enter_async_context(self.connection_factory())
        if await self.client.version() not in (DAGGER_VERSION, "v" + DAGGER_VERSION):
            raise Invalid("runtime_version")
        image = self.image_factory(self.client)
        self.mode = mode
        async with asyncio.timeout(15):
            version = await image.with_exec([
                "/usr/bin/env", "-i", "PATH=/usr/local/bin:/usr/bin:/bin",
                "/usr/local/bin/opencode", "--version",
            ]).stdout()
        if version.strip() != OPENCODE_VERSION:
            raise Invalid("harness_version")
        self.service = service_graph(image, source.directory, job_id, mode,
                                     source_id, binding)
        self.cleanup_target = CleanupTarget(job_id, source_id, await self.service.id())
        await self.service.start()
        self.bridge = (image.with_service_binding("retained", self.service)
                       .with_workdir("/private").with_user("1000:1000"))
        handshake = await self._exchange(job_id, "/_smith/identity", None)
        fields(handshake, ("kind", "identity"))
        boot = fields(handshake["identity"], (
            "boot_id", "workspace_id", "job_id", "source_id", "source_sha256", "mode",
        ))
        identity(boot["boot_id"])
        identity(boot["workspace_id"])
        if (handshake["kind"] != "identity" or boot["job_id"] != job_id
                or boot["source_id"] != source_id or boot["mode"] != mode
                or boot["source_sha256"] != binding):
            raise Invalid("runtime_incarnation")
        self.expected_identity = dict(boot)
        self.cleanup_target = replace(self.cleanup_target, incarnation=encode(boot))
        self.session = await self._session(job_id)

    async def _exchange(self, operation_id, path, body):
        identity(operation_id)
        if self.closed or (path != "/_smith/identity" and self.expected_identity is None):
            raise Invalid("runtime_incarnation")
        wire = json.dumps({"path": path, "body": body,
                           "operation_id": operation_id, "expected": self.expected_identity},
                          ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        if len(wire.encode("utf-8", errors="strict")) > MAX_FRAME - 1:
            raise Invalid("request_limit")
        # Identity is a cache-affecting input, NOT a volatile variable. No
        # discarded with_exec snapshots are treated as live container state.
        node = (self.bridge.with_env_variable("SMITH_OPERATION", operation_id)
                .with_exec(["/usr/bin/env", "-i", "PATH=/usr/bin:/bin",
                            "/usr/bin/python3", "-I", "-B", "-c", BRIDGE], stdin=wire))
        async with asyncio.timeout(125):
            raw = await node.stdout()
        return decode(text_bytes(raw, 60000), limit=60000)

    async def _session(self, operation_id, *, review=False):
        # Create before inference, so provider failure cannot lose conversation
        # identity. Review sessions share no message history with production.
        reply = await self._exchange(operation_id, "/session", {
            "title": "Smith review" if review else "Smith work",
            "permission": ([{"permission": "*", "pattern": "*", "action": "deny"},
                            {"permission": "read", "pattern": "/workspace/repo/*", "action": "allow"},
                            {"permission": "read", "pattern": "/workspace/reports/*", "action": "allow"}]
                           if review else []),
        })
        fields(reply, ("kind", "id"))
        if reply["kind"] != "session" or not isinstance(reply["id"], str) or not SESSION.fullmatch(reply["id"]):
            raise Invalid("runtime_session")
        return reply["id"]

    async def prompt(self, operation_id, text, review):
        if self.session is None or self.closed:
            raise Invalid("runtime_session")
        session = await self._session(operation_id, review=True) if review else self.session
        reply = await self._exchange(operation_id, "/session/" + session + "/message", {
            "parts": [{"type": "text", "text": text}],
        })
        fields(reply, ("kind", "text"))
        if reply["kind"] not in ("succeeded", "failed"):
            raise Invalid("runtime_unknown")
        text_bytes(reply["text"], 8192)
        return Outcome(reply["kind"], reply["text"])

    async def finish(self):
        if not self.effects_possible and self.client is None and self.service is None:
            return True
        self.closed = True
        if self.service is not None:
            async with asyncio.timeout(15):
                await self.service.stop()
        await self.stack.aclose()
        # Stop acknowledgement is NOT independent engine quiescence evidence.
        if self.cleanup_verifier is None or self.cleanup_target is None:
            return False
        return await self.cleanup_verifier(self.cleanup_target) is True


class Supervisor:
    def __init__(self, runtime=None):
        self.runtime = DaggerRuntime() if runtime is None else runtime
        self.job_id = self.mode = None
        self.state = "absent"
        self.retained = False
        self.conversation = False
        self.operations = {}
        self.active = None
        self.task = None

    def _reply(self, req, ok=True, code=None, operation=None):
        job = None if self.job_id is None else {
            "state": self.state, "mode": self.mode, "retained": self.retained,
            "conversation": self.conversation, "ready": False,
            "blockers": list(BLOCKERS),
        }
        return {"v": 1, "operation_id": req.get("operation_id"),
                "job_id": req.get("job_id"), "ok": ok, "code": code,
                "job": job, "operation": operation}

    async def handle(self, req):
        # Direct callers must obey the same strict contract as stdin.
        try:
            raw = json.dumps(req, ensure_ascii=False, sort_keys=True,
                             separators=(",", ":")).encode("utf-8", errors="strict")
        except (UnicodeError, TypeError, ValueError):
            raise Invalid("invalid_request") from None
        req = request(raw)
        op_id, op = req["operation_id"], req["op"]
        fingerprint = digest(encode(req).encode("ascii"))
        if op != "observe" and op_id in self.operations:
            previous, result = self.operations[op_id]
            if fingerprint != previous:
                return self._reply(req, False, "identity_conflict")
            return self._reply(req, operation=dict(result))
        if self.job_id is not None and req["job_id"] != self.job_id:
            return self._reply(req, False, "foreign_job")
        if op == "observe":
            target = req["target_operation_id"]
            result = self.operations.get(target)
            return self._reply(req, operation=None if result is None else dict(result[1]))
        # Reserve one final mutation identity for explicit cleanup.
        limit = MAX_OPERATIONS if op == "finish" else MAX_OPERATIONS - 1
        if len(self.operations) >= limit:
            return self._reply(req, False, "capacity")
        if self.active is not None:
            return self._reply(req, False, "busy")
        if op == "create" and self.job_id is not None:
            return self._reply(req, False, "already_created")
        if op == "prompt" and self.state not in ("active", "failed"):
            return self._reply(req, False, "not_promptable")
        if op == "finish" and self.state not in ("active", "failed", "blocked"):
            return self._reply(req, False, "not_finishable")
        if op == "create":
            self.job_id, self.mode = req["job_id"], req["mode"]
        result = {"operation_id": op_id, "op": op, "state": "pending", "code": None, "text": ""}
        self.operations[op_id] = (fingerprint, result)
        self.active = op_id
        self.task = asyncio.create_task(self._run(req, result))
        return self._reply(req, operation=dict(result))

    async def _run(self, req, result):
        try:
            if req["op"] == "create":
                # Conservatively retain ownership even if start has partial effects.
                self.retained = True
                async with asyncio.timeout(150):
                    await self.runtime.create(req["job_id"], req["mode"], req["source_id"])
                self.conversation, self.state = True, "active"
            elif req["op"] == "prompt":
                async with asyncio.timeout(150):
                    outcome = await self.runtime.prompt(req["operation_id"], req["text"], req["purpose"] == "review")
                if outcome.state not in ("succeeded", "failed"):
                    raise Invalid("runtime_result")
                text_bytes(outcome.text, 8192)
                result["text"] = outcome.text
                result["state"] = outcome.state
                self.state = "active" if outcome.state == "succeeded" else "failed"
            else:
                async with asyncio.timeout(20):
                    verified = await self.runtime.finish()
                if verified is not True:
                    raise Invalid("cleanup_unknown")
                self.state, self.retained = "finished", False
            if result["state"] == "pending":
                result["state"] = "succeeded"
        except PolicyBlocked as error:
            if req["op"] == "create" and isinstance(error, PreEffectPolicyBlocked):
                self.state, self.retained = "blocked", False
                result.update(state="failed", code="policy_denied")
            else:
                # A refused finish/prompt never relinquishes existing ownership.
                # Even CREATE may have partial effects without pre-effect proof.
                self.state = "unknown"
                result.update(state="unknown", code="policy_denied", text="")
        except (Exception, asyncio.CancelledError):
            # Do not expose upstream exception strings, classify credentials,
            # retry, drop ownership, or replace the conversation after ambiguity.
            self.state = "unknown"
            result.update(state="unknown", code="runtime_unknown", text="")
        finally:
            self.active = None


async def serve(reader, writer, supervisor):
    """Binary stdin/stdout; one bounded frame/ack, prompt work runs separately.

    Oversized/unterminated frames terminate transport without draining an
    attacker-controlled stream. Invalid JSON returns one uncorrelated refusal.
    EOF preserves ownership until process loss; caller decides how to fence it.
    """
    while True:
        raw = await asyncio.to_thread(reader.readline, MAX_FRAME + 1)
        if not raw:
            return
        if len(raw) > MAX_FRAME or not raw.endswith(b"\n"):
            return
        try:
            req = request(raw[:-1])
            response = await supervisor.handle(req)
        except Invalid:
            response = {"v": 1, "operation_id": None, "job_id": None,
                        "ok": False, "code": "invalid_request", "job": None, "operation": None}
        writer.write(frame(response))
        writer.flush()


def main():
    # No runtime connections, host config/credential reads or CLI downloads.
    # The unchanged package initializer requires the existing Dagger SDK;
    # backend process ownership/durable fencing is a separate slice.
    asyncio.run(serve(sys.stdin.buffer, sys.stdout.buffer, Supervisor()))


if __name__ == "__main__":
    main()
