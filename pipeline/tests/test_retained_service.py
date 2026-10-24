"""Execute baked service/bridge logic against fakes, with no live sockets."""

import contextlib
from email.message import Message
import io
import json
from pathlib import Path
import tempfile
import types
import unittest
from unittest.mock import patch
import urllib.request

from smith_agent import retained as r
from smith_agent import retained_service as s

JOB, SOURCE, SHA = "a" * 32, "b" * 32, "c" * 64


class Backend:
    def __init__(self):
        self.exit = None

    def poll(self):
        return self.exit


def envelope(boot, path="/session", body=None):
    return {"expected": boot, "operation_id": "d" * 32, "path": path,
            "body": {"title": "Smith work", "permission": []} if body is None else body}


class FenceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="smith-fence-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.workspace = self.root / "workspace"
        self.workspace.mkdir()
        (self.workspace / "repo").mkdir()
        (self.workspace / "reports").mkdir()
        self.private = self.root / "private"
        self.fence = s.BootFence(self.private, self.workspace, JOB, SOURCE, SHA, "read")
        self.expected = self.fence.inspect()
        self.backend, self.calls = Backend(), []
        def forward(path, body):
            self.calls.append((path, body))
            return {"kind": "session", "id": "ses_retained"}
        self.app = s.Application(self.fence, self.backend, forward)

    def call(self, value):
        return self.app.dispatch("POST", "/_smith/request", s.encoded(value))

    def test_private_boot_identity_and_handshake_are_observation_only(self):
        response = self.app.dispatch("GET", "/_smith/identity")
        self.assertEqual(response, {"kind": "identity", "identity": self.expected})
        self.assertEqual(self.fence.path.stat().st_mode & 0o777, 0o600)
        self.assertEqual(self.private.stat().st_mode & 0o777, 0o700)
        self.assertEqual(self.calls, [])
        self.assertEqual(s.load(self.fence.path.read_bytes()), self.expected)

    def test_restart_changes_boot_and_workspace_identity_and_refuses_old_before_mutation(self):
        replacement = s.BootFence(self.private, self.workspace, JOB, SOURCE, SHA, "read")
        new_identity = replacement.inspect()
        self.assertNotEqual(new_identity["boot_id"], self.expected["boot_id"])
        self.assertNotEqual(new_identity["workspace_id"], self.expected["workspace_id"])
        self.app.fence = replacement
        with self.assertRaises(s.FenceError):
            self.call(envelope(self.expected))
        self.assertEqual(self.calls, [])
        # Also fence the original proxy if its private boot file is replaced.
        self.app.fence = self.fence
        with self.assertRaises(s.FenceError):
            self.call(envelope(self.expected))
        self.assertEqual(self.calls, [])

    def test_mismatched_job_source_workspace_or_server_never_forwards(self):
        for key in s.IDENTITY_KEYS:
            boot = dict(self.expected)
            boot[key] = "wrong"
            with self.subTest(key=key), self.assertRaises(s.FenceError):
                self.call(envelope(boot))
        self.assertEqual(self.calls, [])

    def test_workspace_replacement_and_symlink_are_fenced(self):
        repo = self.workspace / "repo"
        repo.rename(self.workspace / "old-repo")
        repo.mkdir()
        with self.assertRaises(s.FenceError):
            self.call(envelope(self.expected))
        repo.rmdir()
        repo.symlink_to(self.workspace / "old-repo", target_is_directory=True)
        with self.assertRaises(s.FenceError):
            self.call(envelope(self.expected))
        self.assertEqual(self.calls, [])

    def test_boot_file_symlink_corruption_and_bounds_fail_closed(self):
        original = self.fence.path.read_bytes()
        for raw in (b"{}", b"x" * 4097, b'{"boot_id":"a","boot_id":"a"}'):
            self.fence.path.write_bytes(raw)
            with self.assertRaises((ValueError, UnicodeError)):
                self.call(envelope(self.expected))
        self.fence.path.write_bytes(original)
        copy = self.private / "copy.json"
        copy.write_bytes(original)
        self.fence.path.unlink()
        self.fence.path.symlink_to(copy)
        with self.assertRaises(OSError):
            self.call(envelope(self.expected))
        self.assertEqual(self.calls, [])

    def test_backend_exit_never_restarts_or_forwards(self):
        self.backend.exit = 1
        with self.assertRaises(s.FenceError):
            self.call(envelope(self.expected))
        self.assertEqual(self.calls, [])

    def test_backend_or_workspace_loss_during_forward_is_unknown_not_success(self):
        def changed(path, body):
            self.calls.append(path)
            self.backend.exit = 1
            return {"kind": "session", "id": "ses_retained"}
        self.app.forward = changed
        with self.assertRaises(s.FenceError):
            self.call(envelope(self.expected))
        self.assertEqual(self.calls, ["/session"])

    def test_proxy_is_not_an_arbitrary_opencode_or_model_override_endpoint(self):
        bad = [envelope(self.expected, path="/session/ses_a/shell"),
               envelope(self.expected, path="http://host/"),
               envelope(self.expected, body={"title": "Smith work", "permission": [], "model": "override"}),
               envelope(self.expected, body={"title": "Smith work", "permission": [{"action": "allow"}]}),
               envelope(self.expected, path="/session/ses_a/message", body={"parts": [], "model": {}}),
               envelope(self.expected, path="/session/ses_a/message", body={"parts": [{"type": "text", "text": "x" * 16385}]}),
               dict(envelope(self.expected), expected=None)]
        for value in bad:
            with self.subTest(value=str(value)[:160]), self.assertRaises(s.FenceError):
                self.call(value)
        self.assertEqual(self.calls, [])

    def test_fenced_valid_prompt_forwards_once_and_keeps_identity(self):
        body = {"parts": [{"type": "text", "text": "Continue after failure"}]}
        self.call(envelope(self.expected, "/session/ses_retained/message", body))
        self.assertEqual(self.calls, [("/session/ses_retained/message", body)])
        self.assertEqual(self.fence.inspect(), self.expected)

    def test_actual_bridge_code_sends_expected_identity_in_same_request(self):
        captured = []
        app = self.app
        class Response(io.BytesIO):
            pass
        class Opener:
            def open(self, request, timeout):
                captured.append((request.full_url, request.get_method(), request.data, timeout))
                value = app.dispatch(request.get_method(), request.selector, request.data or b"")
                return Response(s.encoded(value))
        raw = json.dumps(envelope(self.expected))
        output = io.StringIO()
        with (patch("sys.stdin", io.StringIO(raw)), contextlib.redirect_stdout(output),
              patch.object(urllib.request, "build_opener", return_value=Opener())):
            exec(compile(r.BRIDGE, "<baked-bridge>", "exec"), {})
        self.assertEqual(json.loads(output.getvalue())["kind"], "session")
        self.assertEqual(captured[0][:2], ("http://retained:4096/_smith/request", "POST"))
        self.assertEqual(json.loads(captured[0][2])["expected"], self.expected)
        self.assertEqual(len(self.calls), 1)
        # Same bridge against a new boot cannot recreate the conversation.
        self.app.fence = s.BootFence(self.private, self.workspace, JOB, SOURCE, SHA, "read")
        output = io.StringIO()
        with (patch("sys.stdin", io.StringIO(raw)), contextlib.redirect_stdout(output),
              patch.object(urllib.request, "build_opener", return_value=Opener())):
            exec(compile(r.BRIDGE, "<baked-bridge>", "exec"), {})
        self.assertEqual(json.loads(output.getvalue())["kind"], "unknown")
        self.assertEqual(len(self.calls), 1)


class ForwardTests(unittest.TestCase):
    def test_fixed_loopback_forward_failure_text_encoding_and_bounds(self):
        cases = [
            ({"info": {"error": {"message": "private-canary"}}, "parts": []}, "failed"),
            ({"info": {}, "parts": [{"type": "text", "text": "\u4e00" * 2730}]}, "succeeded"),
            ({"info": {}, "parts": [{"type": "text", "text": "\u4e00" * 2731}]}, None),
            ({"info": {}, "parts": [{"type": "text", "text": "\ud800"}]}, None),
        ]
        for value, kind in cases:
            with self.subTest(kind=kind):
                response = io.BytesIO(s.encoded(value))
                response.status = 200
                connection = types.SimpleNamespace(request=lambda *a, **kw: None,
                                                   getresponse=lambda: response, close=lambda: None)
                with patch.object(s.http.client, "HTTPConnection", return_value=connection) as factory:
                    if kind is None:
                        with self.assertRaises((s.FenceError, UnicodeError)):
                            s.forward("/session/ses_a/message", {"parts": []})
                    else:
                        result = s.forward("/session/ses_a/message", {"parts": []})
                        self.assertEqual(result["kind"], kind)
                        self.assertNotIn("private-canary", str(result))
                    factory.assert_called_once_with("127.0.0.1", 4097, timeout=120)


class HandlerTests(unittest.TestCase):
    def handler(self, raw, lengths, transfer=None):
        calls = []
        handler = object.__new__(s.Handler)
        handler.headers = Message()
        for value in lengths:
            handler.headers.add_header("Content-Length", value)
        if transfer:
            handler.headers.add_header("Transfer-Encoding", transfer)
        handler.path = "/_smith/request"
        handler.rfile, handler.wfile = io.BytesIO(raw), io.BytesIO()
        def dispatch(*args):
            calls.append(args)
            return {"kind": "failed", "text": ""}
        handler.server = types.SimpleNamespace(application=types.SimpleNamespace(dispatch=dispatch))
        handler.send_response = lambda status: None
        handler.send_header = lambda name, value: None
        handler.end_headers = lambda: None
        return handler, calls

    def test_bad_content_framing_never_reaches_dispatch(self):
        cases = [(b"{}", [], None), (b"{}", ["2", "2"], None),
                 (b"{}", ["-1"], None), (b"{}", ["32769"], None),
                 (b"{}", ["3"], None), (b"{}", ["2"], "chunked")]
        for raw, lengths, transfer in cases:
            with self.subTest(lengths=lengths, transfer=transfer):
                handler, calls = self.handler(raw, lengths, transfer)
                handler.respond("POST")
                self.assertEqual(calls, [])
                self.assertEqual(json.loads(handler.wfile.getvalue()), {"kind": "unknown", "text": ""})

    def test_exact_utf8_body_and_safe_error_response(self):
        raw = '{"text":"\u4e00"}'.encode("utf-8")
        handler, calls = self.handler(raw, [str(len(raw))])
        handler.respond("POST")
        self.assertEqual(calls[0][2], raw)
        self.assertEqual(json.loads(handler.wfile.getvalue())["kind"], "failed")
        handler, calls = self.handler(raw, [str(len(raw))])
        def error(*args):
            raise RuntimeError("private-canary")
        handler.server.application.dispatch = error
        handler.respond("POST")
        self.assertEqual(json.loads(handler.wfile.getvalue()), {"kind": "unknown", "text": ""})
        self.assertNotIn(b"private-canary", handler.wfile.getvalue())


class SDKFenceTests(unittest.IsolatedAsyncioTestCase):
    async def test_failed_session_creation_retains_live_sdk_and_validated_source_binding(self):
        permissions = (r.RepositoryCapability(".", False),)
        binding = r.source_binding(r.ValidatedSource(SOURCE, object(), SHA, permissions))
        boot = {"boot_id": "e" * 32, "workspace_id": "f" * 32, "job_id": JOB,
                "source_id": SOURCE, "source_sha256": binding, "mode": "write"}
        calls = []
        directory = object()
        class Service:
            async def id(self):
                return "owned-service"
            async def start(self):
                calls.append("start")
            async def stop(self):
                calls.append("stop")
        service = Service()
        class Node:
            def with_user(self, name):
                return self
            def with_workdir(self, path):
                return self
            def with_env_variable(self, name, value):
                return self
            def with_new_file(self, path, *, contents, owner, permissions):
                return self
            def with_directory(self, path, source, *, owner):
                calls.append(("source", source))
                return self
            def with_exposed_port(self, port):
                return self
            def as_service(self, *, args, use_entrypoint):
                return service
            def with_service_binding(self, alias, bound):
                return self
            def with_exec(self, args, *, stdin=None):
                class Reply:
                    async def stdout(self):
                        if stdin is None:
                            return r.OPENCODE_VERSION
                        wire = json.loads(stdin)
                        calls.append(wire)
                        if wire["path"] == "/_smith/identity":
                            return r.encode({"kind": "identity", "identity": boot})
                        return '{"kind":"unknown","text":""}'
                return Reply()
        class Client:
            async def version(self):
                return r.DAGGER_VERSION
        class Connection:
            async def __aenter__(self):
                calls.append("connect")
                return Client()
            async def __aexit__(self, *args):
                calls.append("disconnect")
        async def validator(source_id, candidate):
            self.assertIs(candidate.directory, directory)
            return r.ValidatedSource(source_id, candidate.directory, SHA, candidate.repositories)
        runtime = r.DaggerRuntime(Connection, lambda client: Node(),
                                  {SOURCE: r.SourceInput(directory, permissions)},
                                  source_validator=validator)
        with self.assertRaises(r.Invalid):
            await runtime._start_reviewed(JOB, "write", SOURCE)
        self.assertIs(runtime.service, service)
        self.assertIsNotNone(runtime.client)
        self.assertEqual(runtime.expected_identity, boot)
        self.assertIsNone(runtime.session)
        self.assertEqual(runtime.cleanup_target.source_id, SOURCE)
        self.assertNotIn("stop", calls)
        self.assertNotIn("disconnect", calls)
        self.assertIn(("source", directory), calls)
        mutation = next(c for c in calls if isinstance(c, dict) and c["path"] == "/session")
        self.assertEqual(mutation["expected"], boot)
        self.assertFalse(await runtime.finish())
        self.assertEqual(calls[-2:], ["stop", "disconnect"])

    async def test_fake_sdk_exec_carries_fence_and_unknown_preserves_session(self):
        boot = {"boot_id": "e" * 32, "workspace_id": "f" * 32, "job_id": JOB,
                "source_id": SOURCE, "source_sha256": SHA, "mode": "write"}
        calls = []
        class Bridge:
            def with_env_variable(self, name, value):
                calls.append((name, value))
                return self
            def with_exec(self, args, *, stdin):
                calls.append((args, json.loads(stdin)))
                class Reply:
                    async def stdout(self):
                        return '{"kind":"unknown","text":""}'
                return Reply()
        runtime = r.DaggerRuntime()
        runtime.bridge = Bridge()
        runtime.expected_identity = dict(boot)
        runtime.session = "ses_prior"
        with self.assertRaises(r.Invalid):
            await runtime.prompt("d" * 32, "\u4e00" * 5461, False)
        self.assertEqual(calls[1][1]["expected"], boot)
        self.assertEqual(calls[1][1]["path"], "/session/ses_prior/message")
        self.assertEqual(calls[0], ("SMITH_OPERATION", "d" * 32))
        self.assertEqual(runtime.session, "ses_prior")
        self.assertEqual(runtime.expected_identity, boot)


if __name__ == "__main__":
    unittest.main()
