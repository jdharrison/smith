"""Fixed container-only OpenCode proxy. No host commands or configurable URLs.

The exposed Dagger service runs THIS process, not OpenCode directly. Every boot
writes a fresh private identity; backend death is terminal, never restarted.
Expected identity is checked in the same handler that forwards a mutation.
This is an incarnation/accidental replacement fence, not a hostile-same-UID
sandbox. Resource limits, provider egress and live interoperability stay gated.
"""

import http.client
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import os
from pathlib import Path
import re
import secrets
import signal
import socket
import stat
import subprocess
import sys
import threading
import time

MAX_BODY = 32768
MAX_UPSTREAM = 32768
IDENTITY_KEYS = {"boot_id", "workspace_id", "job_id", "source_id", "source_sha256", "mode"}


class FenceError(ValueError):
    pass


def _pairs(items):
    result = {}
    for key, value in items:
        if key in result:
            raise FenceError()
        result[key] = value
    return result


def load(raw):
    if len(raw) > MAX_BODY:
        raise FenceError()
    return json.loads(raw.decode("utf-8", errors="strict"), object_pairs_hook=_pairs,
                      parse_constant=lambda value: (_ for _ in ()).throw(FenceError()))


def encoded(value):
    return json.dumps(value, ensure_ascii=True, separators=(",", ":")).encode("ascii")


def directory_identity(path):
    info = os.lstat(path)
    if not stat.S_ISDIR(info.st_mode):
        raise FenceError()
    return info.st_dev, info.st_ino


class BootFence:
    def __init__(self, private, workspace, job_id, source_id, source_sha256, mode):
        for value, size in ((job_id, 32), (source_id, 32), (source_sha256, 64)):
            if type(value) is not str or re.fullmatch("[0-9a-f]{" + str(size) + "}", value) is None:
                raise FenceError()
        if mode not in ("read", "write"):
            raise FenceError()
        self.roots = tuple(Path(workspace) / name for name in (".", "repo", "reports"))
        self.roots_identity = tuple(directory_identity(path) for path in self.roots)
        self.identity = {
            "boot_id": secrets.token_hex(16), "workspace_id": secrets.token_hex(16),
            "job_id": job_id, "source_id": source_id,
            "source_sha256": source_sha256, "mode": mode,
        }
        private = Path(private)
        private.mkdir(mode=0o700, exist_ok=True)
        info = private.lstat()
        if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or info.st_mode & 0o077:
            raise FenceError()
        self.private, self.private_identity = private, directory_identity(private)
        self.path = private / "boot.json"
        temporary = private / (".boot-" + secrets.token_hex(16))
        fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        try:
            with os.fdopen(fd, "wb") as output:
                output.write(encoded(self.identity))
                output.flush()
                os.fsync(output.fileno())
            os.replace(temporary, self.path)
        finally:
            if temporary.exists():
                temporary.unlink()

    def inspect(self):
        if directory_identity(self.private) != self.private_identity:
            raise FenceError()
        if tuple(directory_identity(path) for path in self.roots) != self.roots_identity:
            raise FenceError()
        fd = os.open(self.path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        try:
            info = os.fstat(fd)
            if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid()
                    or info.st_mode & 0o077 or info.st_size > 4096):
                raise FenceError()
            raw = os.read(fd, 4097)
        finally:
            os.close(fd)
        if load(raw) != self.identity:
            raise FenceError()
        return dict(self.identity)

    def validate(self, expected):
        if type(expected) is not dict or expected != self.inspect():
            raise FenceError()


def validate_body(path, body):
    if type(body) is not dict:
        raise FenceError()
    if path == "/session":
        review_rules = [
            {"permission": "*", "pattern": "*", "action": "deny"},
            {"permission": "read", "pattern": "/workspace/repo/*", "action": "allow"},
            {"permission": "read", "pattern": "/workspace/reports/*", "action": "allow"},
        ]
        if set(body) != {"title", "permission"}:
            raise FenceError()
        if body["title"] == "Smith work" and body["permission"] == []:
            return
        if body["title"] == "Smith review" and body["permission"] == review_rules:
            return
        raise FenceError()
    if set(body) != {"parts"} or type(body["parts"]) is not list or len(body["parts"]) != 1:
        raise FenceError()
    part = body["parts"][0]
    if type(part) is not dict or set(part) != {"type", "text"} or part["type"] != "text":
        raise FenceError()
    text = part["text"]
    if (type(text) is not str or "\0" in text or not text.strip()
            or len(text.encode("utf-8", errors="strict")) > 16384):
        raise FenceError()


class Application:
    def __init__(self, fence, backend, forward):
        self.fence, self.backend, self.forward = fence, backend, forward

    def dispatch(self, method, path, raw=b""):
        if self.backend.poll() is not None:
            raise FenceError()
        if method == "GET" and path == "/_smith/identity":
            return {"kind": "identity", "identity": self.fence.inspect()}
        if method != "POST" or path != "/_smith/request":
            raise FenceError()
        request = load(raw)
        if type(request) is not dict or set(request) != {"expected", "operation_id", "path", "body"}:
            raise FenceError()
        if type(request["operation_id"]) is not str or re.fullmatch("[0-9a-f]{32}", request["operation_id"]) is None:
            raise FenceError()
        target = request["path"]
        if type(target) is not str or (target != "/session" and re.fullmatch(r"/session/ses_[A-Za-z0-9_-]{1,128}/message", target) is None):
            raise FenceError()
        validate_body(target, request["body"])
        # No intervening HTTP round trip, restart, or replacement backend. The
        # backend is loopback-only and this single-threaded handler is its sole
        # external mutation path. If it dies after this check, outcome is unknown.
        self.fence.validate(request["expected"])
        if self.backend.poll() is not None:
            raise FenceError()
        result = self.forward(target, request["body"])
        self.fence.validate(request["expected"])
        if self.backend.poll() is not None:
            raise FenceError()
        return result


def forward(path, body):
    connection = http.client.HTTPConnection("127.0.0.1", 4097, timeout=120)
    try:
        connection.request("POST", path, body=encoded(body), headers={"Content-Type": "application/json"})
        response = connection.getresponse()
        raw = response.read(MAX_UPSTREAM + 1)
        if response.status != 200 or len(raw) > MAX_UPSTREAM:
            raise FenceError()
        value = load(raw)
        if path == "/session":
            return {"kind": "session", "id": value["id"]}
        info = value["info"]
        if info.get("error"):
            return {"kind": "failed", "text": ""}
        text = "".join(part["text"] for part in value["parts"] if part.get("type") == "text")
        if "\0" in text or len(text.encode("utf-8", errors="strict")) > 8192:
            raise FenceError()
        return {"kind": "succeeded", "text": text}
    finally:
        connection.close()


class Handler(BaseHTTPRequestHandler):
    # Fixed small service protocol, not a general HTTP reverse proxy.
    timeout = 5

    def do_GET(self):
        self.respond("GET")

    def do_POST(self):
        self.respond("POST")

    def respond(self, method):
        try:
            lengths = self.headers.get_all("Content-Length", [])
            if self.headers.get("Transfer-Encoding") is not None:
                raise FenceError()
            if method == "POST":
                if len(lengths) != 1 or not lengths[0].isdigit():
                    raise FenceError()
                length = int(lengths[0])
                if not 0 < length <= MAX_BODY:
                    raise FenceError()
                raw = self.rfile.read(length)
                if len(raw) != length:
                    raise FenceError()
            else:
                if lengths:
                    raise FenceError()
                raw = b""
            result = self.server.application.dispatch(method, self.path, raw)
        except Exception:
            result = {"kind": "unknown", "text": ""}
        raw = encoded(result)
        if len(raw) > 60000:
            raw = b'{"kind":"unknown","text":""}'
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(raw)
        self.close_connection = True

    def log_message(self, *args):
        pass


def main():
    # All arguments are emitted by trusted service_graph, never stdin/model.
    job_id, source_id, source_sha256, mode = sys.argv[1:]
    fence = BootFence("/private/retained", "/workspace", job_id, source_id, source_sha256, mode)
    backend = subprocess.Popen([
        "/usr/local/bin/opencode", "serve", "--hostname", "127.0.0.1", "--port", "4097",
    ], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
       env=dict(os.environ), start_new_session=True)
    server = None
    try:
        # Bounded loopback readiness only, never a prompt/provider retry. The
        # proxy is not exposed until this exact child is listening.
        deadline = time.monotonic() + 10
        while True:
            if backend.poll() is not None or time.monotonic() >= deadline:
                raise FenceError()
            try:
                with socket.create_connection(("127.0.0.1", 4097), timeout=0.2):
                    break
            except OSError:
                time.sleep(0.05)
        # A single process lifetime and no backend retry. Dagger may recreate
        # the entire service, but its fresh fence rejects the old expectation.
        server = HTTPServer(("0.0.0.0", 4096), Handler)
        server.application = Application(fence, backend, forward)
        def backend_exited():
            backend.wait()
            server.shutdown()
        threading.Thread(target=backend_exited, daemon=True).start()
        server.serve_forever(poll_interval=0.1)
    finally:
        if server is not None:
            server.server_close()
        try:
            os.killpg(backend.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        backend.wait(timeout=5)


if __name__ == "__main__":
    main()
