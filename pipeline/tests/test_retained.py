"""Isolated protocol/lifecycle/graph checks, never engine/provider acceptance."""

import asyncio
import io
import json
import unittest
from unittest.mock import patch

from smith_agent import retained as r

JOB, SOURCE = "a" * 32, "b" * 32
SOURCE_SHA = "d" * 64
WRITABLE = (r.RepositoryCapability(".", False),)
BINDING = r.source_binding(r.ValidatedSource(SOURCE, object(), SOURCE_SHA, WRITABLE))
BOOT = {"boot_id": "e" * 32, "workspace_id": "f" * 32, "job_id": JOB,
        "source_id": SOURCE, "source_sha256": BINDING, "mode": "write"}


async def validated_source(source_id, source):
    return r.ValidatedSource(source_id, source.directory, SOURCE_SHA, source.repositories)


def req(op, number=1, **extra):
    return {"v": 1, "op": op, "job_id": JOB,
            "operation_id": f"{number:032x}", **extra}


def create(number=1, mode="read"):
    return req("create", number, mode=mode, source_id=SOURCE)


def prompt(number=2, purpose="work", text="Inspect source"):
    return req("prompt", number, purpose=purpose, text=text)


class FakeRuntime:
    def __init__(self):
        self.calls = []
        self.outputs = [r.Outcome("succeeded", "reply")]
        self.event = None
        self.verified = True

    async def create(self, job_id, mode, source_id):
        self.calls.append(("create", job_id, mode, source_id))

    async def prompt(self, operation_id, text, review):
        self.calls.append(("prompt", operation_id, text, review))
        if self.event is not None:
            await self.event.wait()
        outcome = self.outputs.pop(0)
        if isinstance(outcome, Exception):
            raise outcome
        return outcome

    async def finish(self):
        self.calls.append(("finish",))
        return self.verified


class SupervisorTests(unittest.IsolatedAsyncioTestCase):
    async def settle(self, supervisor):
        if supervisor.task:
            await supervisor.task

    async def started(self):
        runtime = FakeRuntime()
        supervisor = r.Supervisor(runtime)
        await supervisor.handle(create())
        await self.settle(supervisor)
        return supervisor, runtime

    async def test_failure_keeps_runtime_workspace_conversation_until_finish(self):
        supervisor, runtime = await self.started()
        runtime.outputs = [r.Outcome("failed"), r.Outcome("succeeded", "fixed")]
        first = await supervisor.handle(prompt())
        self.assertEqual(first["operation"]["state"], "pending")
        await self.settle(supervisor)
        self.assertEqual(supervisor.state, "failed")
        self.assertTrue(supervisor.retained)
        self.assertTrue(supervisor.conversation)
        await supervisor.handle(prompt(3, text="Continue"))
        await self.settle(supervisor)
        self.assertEqual(supervisor.state, "active")
        self.assertEqual([c[0] for c in runtime.calls], ["create", "prompt", "prompt"])
        await supervisor.handle(req("finish", 4))
        await self.settle(supervisor)
        self.assertEqual(supervisor.state, "finished")
        self.assertFalse(supervisor.retained)
        self.assertFalse((await supervisor.handle(prompt(5)))["ok"])

    async def test_observe_inflight_and_exact_duplicate_does_not_replay(self):
        supervisor, runtime = await self.started()
        runtime.event = asyncio.Event()
        await supervisor.handle(prompt())
        await asyncio.sleep(0)
        duplicate = await supervisor.handle(prompt())
        observed = await supervisor.handle(req("observe", 7, target_operation_id=f"{2:032x}"))
        self.assertEqual(observed["operation"], duplicate["operation"])
        self.assertEqual(observed["operation"]["state"], "pending")
        self.assertEqual((await supervisor.handle(prompt(3)))["code"], "busy")
        self.assertEqual((await supervisor.handle(prompt(text="different")))["code"], "identity_conflict")
        runtime.event.set()
        await self.settle(supervisor)
        again = await supervisor.handle(prompt())
        self.assertEqual(again["operation"]["text"], "reply")
        self.assertEqual(len(runtime.calls), 2)

    async def test_unknown_fences_prompts_and_retains_ownership(self):
        supervisor, runtime = await self.started()
        runtime.outputs = [RuntimeError("secret-canary")]
        await supervisor.handle(prompt())
        await self.settle(supervisor)
        self.assertEqual(supervisor.state, "unknown")
        reply = await supervisor.handle(prompt(3))
        self.assertEqual(reply["code"], "not_promptable")
        self.assertTrue(reply["job"]["retained"])
        self.assertNotIn("secret-canary", r.frame(reply).decode())
        refused = await supervisor.handle(req("finish", 4))
        self.assertEqual(refused["code"], "not_finishable")
        self.assertTrue(refused["job"]["retained"])
        duplicate = await supervisor.handle(prompt())
        observed = await supervisor.handle(req("observe", 5, target_operation_id=f"{2:032x}"))
        self.assertEqual(duplicate["operation"], observed["operation"])
        self.assertEqual(observed["operation"]["state"], "unknown")
        self.assertNotIn(f"{4:032x}", supervisor.operations)
        self.assertEqual(len(runtime.calls), 2)

    async def test_fresh_review_requested_separately(self):
        supervisor, runtime = await self.started()
        await supervisor.handle(prompt(purpose="review"))
        await self.settle(supervisor)
        self.assertTrue(runtime.calls[-1][-1])
        self.assertTrue(supervisor.conversation)

    async def test_unknown_cleanup_is_not_finished(self):
        supervisor, runtime = await self.started()
        runtime.verified = False
        await supervisor.handle(req("finish", 2))
        await self.settle(supervisor)
        self.assertEqual(supervisor.state, "unknown")
        self.assertTrue(supervisor.retained)
        duplicate = await supervisor.handle(req("finish", 2))
        refused = await supervisor.handle(req("finish", 3))
        self.assertEqual(refused["code"], "not_finishable")
        self.assertTrue(refused["job"]["retained"])
        observed = await supervisor.handle(req("observe", 4, target_operation_id=f"{2:032x}"))
        self.assertEqual(duplicate["operation"], observed["operation"])
        self.assertEqual(observed["operation"]["state"], "unknown")
        self.assertNotIn(f"{3:032x}", supervisor.operations)
        self.assertEqual(len(runtime.calls), 2)

    async def test_foreign_job_observation_and_mutation_rejected(self):
        supervisor, runtime = await self.started()
        for op in (prompt(), req("observe", 3, target_operation_id=None)):
            op["job_id"] = "c" * 32
            self.assertEqual((await supervisor.handle(op))["code"], "foreign_job")
        self.assertEqual(len(runtime.calls), 1)

    async def test_default_policy_blocks_before_any_connection_or_image(self):
        def forbidden():
            raise AssertionError("effect")
        runtime = r.DaggerRuntime(forbidden, forbidden, {SOURCE: object()})
        supervisor = r.Supervisor(runtime)
        await supervisor.handle(create())
        await self.settle(supervisor)
        reply = await supervisor.handle(req("observe", 2, target_operation_id=f"{1:032x}"))
        self.assertEqual(reply["operation"]["code"], "policy_denied")
        self.assertEqual(reply["job"]["state"], "blocked")
        self.assertFalse(reply["job"]["ready"])
        self.assertFalse(reply["job"]["retained"])
        self.assertIsNone(runtime.client)

    async def test_new_process_never_recovers_or_replays_unknown_job(self):
        supervisor = r.Supervisor(FakeRuntime())
        reply = await supervisor.handle(req("observe", 2, target_operation_id=f"{1:032x}"))
        self.assertIsNone(reply["operation"])
        self.assertIsNone(reply["job"])
        self.assertEqual((await supervisor.handle(prompt()))["code"], "not_promptable")

    async def test_transport_strict_frames_and_no_eof_finish(self):
        supervisor, runtime = await self.started()
        incoming = (r.frame(req("observe", 3, target_operation_id=None))
                    + b'{"v":1,"v":1}\n' + b"x" * (r.MAX_FRAME + 1))
        output = io.BytesIO()
        await r.serve(io.BytesIO(incoming), output, supervisor)
        lines = output.getvalue().splitlines()
        self.assertEqual(len(lines), 2)
        self.assertTrue(json.loads(lines[0])["job"]["retained"])
        self.assertEqual(json.loads(lines[1])["code"], "invalid_request")
        self.assertEqual(len(runtime.calls), 1)
        self.assertTrue(supervisor.retained)

    async def test_capacity_refuses_without_effect(self):
        supervisor, runtime = await self.started()
        with patch.object(r, "MAX_OPERATIONS", 1):
            self.assertEqual((await supervisor.handle(prompt()))["code"], "capacity")
        self.assertEqual(len(runtime.calls), 1)


class CodecTests(unittest.TestCase):
    def test_strict_shapes_and_types(self):
        bad = [dict(create(), v=True), dict(create(), model="override"),
               dict(create(), source_id="/host/repo"), dict(create(), mode=[]),
               dict(prompt(), purpose=[]), dict(prompt(), text="\0"),
               dict(prompt(), text="x" * 16385), dict(prompt(), text="\ud800"),
               dict(create(), operation_id="A" * 32)]
        for value in bad:
            with self.subTest(value=str(value)[:120]), self.assertRaises(r.Invalid):
                r.request(json.dumps(value).encode())
        with self.assertRaises(r.Invalid):
            r.request(b'{"v":1,"v":1}')
        with self.assertRaises(r.Invalid):
            r.request(b"[]")

    def test_response_bound_with_maximum_unicode_reply(self):
        text = "\U0001f600" * 2048
        result = {"operation_id": "a" * 32, "op": "prompt", "state": "succeeded",
                  "code": None, "text": text}
        supervisor = r.Supervisor()
        self.assertLessEqual(len(r.frame(supervisor._reply(prompt(), operation=result))), r.MAX_REPLY)
        with self.assertRaises(r.Invalid):
            r.frame({"text": "x" * r.MAX_REPLY})

    def test_clean_config_and_no_shell_git_network_tools(self):
        for mode in ("read", "write"):
            config = json.loads(r.opencode_config(mode))
            self.assertEqual(set(config), {"share", "permission"})
            self.assertEqual(config["permission"]["*"], "deny")
            self.assertEqual(config["permission"]["edit"]["*"], "deny")
            self.assertEqual("/workspace/repo/*" in config["permission"]["edit"], mode == "write")
            review = json.loads(r.opencode_config(mode, review=True))
            self.assertEqual(review["permission"]["edit"], {"*": "deny"})


class Service:
    # Deliberately NO with_exec: do not invent APIs to satisfy a fake.
    def __init__(self):
        self.calls = []

    async def start(self):
        self.calls.append("start")
        return self

    async def stop(self):
        self.calls.append("stop")
        return self

    async def id(self):
        return "service-test"


class Image:
    def __init__(self):
        self.calls = []
        self.service = Service()

    def with_user(self, name):
        self.calls.append(("user", name))
        return self

    def with_workdir(self, path):
        self.calls.append(("workdir", path))
        return self

    def with_env_variable(self, name, value):
        self.calls.append(("env", name, value))
        return self

    def with_new_file(self, path, *, contents, owner, permissions):
        self.calls.append(("file", path, contents, owner, permissions))
        return self

    def with_mounted_directory(self, path, source, *, read_only):
        self.calls.append(("mount", path, source, read_only))
        return self

    def with_directory(self, path, source, *, owner):
        self.calls.append(("copy", path, source, owner))
        return self

    def with_exposed_port(self, port):
        self.calls.append(("port", port))
        return self

    def as_service(self, *, args, use_entrypoint):
        self.calls.append(("service", args, use_entrypoint))
        return self.service


class GraphTests(unittest.TestCase):
    def test_read_real_service_api_and_immutable_repo_writable_reports(self):
        image, source = Image(), object()
        result = r.service_graph(image, source, JOB, "read", SOURCE, SOURCE_SHA)
        self.assertIs(result, image.service)
        self.assertIn(("mount", "/workspace/repo", source, True), image.calls)
        self.assertTrue(any(c[:2] == ("file", "/workspace/reports/.keep") for c in image.calls))
        _, command, entrypoint = image.calls[-1]
        self.assertFalse(entrypoint)
        self.assertEqual(command[:2], ["/usr/bin/env", "-i"])
        self.assertEqual(command[-8:], ["/usr/bin/python3", "-I", "-B", "/trusted/retained_service.py",
                                        JOB, SOURCE, SOURCE_SHA, "read"])
        baked = next(c for c in image.calls if c[:2] == ("file", "/trusted/retained_service.py"))
        self.assertEqual(baked[-2:], ("root:root", 0o444))
        self.assertIn("class BootFence", baked[2])
        self.assertNotIn("--model", command)
        self.assertTrue(any(v == "OPENCODE_DISABLE_PROJECT_CONFIG=true" for v in command))
        self.assertTrue(any(v == "OPENCODE_DISABLE_DEFAULT_PLUGINS=true" for v in command))
        self.assertFalse(hasattr(result, "with_exec"))

    def test_write_copies_instead_of_writable_host_mount(self):
        image, source = Image(), object()
        r.service_graph(image, source, JOB, "write", SOURCE, SOURCE_SHA)
        self.assertIn(("copy", "/workspace/repo", source, "1000:1000"), image.calls)
        self.assertFalse(any(c[0] == "mount" for c in image.calls))


class RuntimeTests(unittest.IsolatedAsyncioTestCase):
    async def test_reviewed_sdk_lifecycle_holds_connection_until_explicit_finish(self):
        events = []
        image = Image()
        class Probe:
            async def stdout(self):
                return r.OPENCODE_VERSION + "\n"
        class Client:
            async def version(self):
                return r.DAGGER_VERSION
        class Connection:
            async def __aenter__(self):
                events.append("connect")
                return Client()
            async def __aexit__(self, *args):
                events.append("disconnect")
        image.with_exec = lambda args: Probe()
        image.with_service_binding = lambda alias, service: image
        runtime = r.DaggerRuntime(Connection, lambda client: image,
                                  {SOURCE: r.SourceInput(object(), WRITABLE)},
                                  source_validator=validated_source)
        exchanges = []
        async def exchange(operation_id, path, body):
            exchanges.append((operation_id, path, body))
            if path == "/_smith/identity":
                return {"kind": "identity", "identity": dict(BOOT)}
            return ({"kind": "session", "id": "ses_same"} if path == "/session"
                    else {"kind": "failed", "text": ""})
        runtime._exchange = exchange
        await runtime._start_reviewed(JOB, "write", SOURCE)
        self.assertEqual(events, ["connect"])
        self.assertEqual(image.service.calls, ["start"])
        for number in (2, 3):
            await runtime.prompt(f"{number:032x}", "next", False)
        self.assertEqual(events, ["connect"])
        self.assertEqual([x[1] for x in exchanges], ["/_smith/identity", "/session",
                                                  "/session/ses_same/message", "/session/ses_same/message"])
        self.assertEqual(runtime.expected_identity, BOOT)
        self.assertFalse(await runtime.finish())
        self.assertEqual(events, ["connect", "disconnect"])
        self.assertEqual(image.service.calls, ["start", "stop"])

    async def test_pin_mismatch_prevents_service_start(self):
        image = Image()
        class Client:
            async def version(self):
                return "1.0.0-beta.14"
        class Connection:
            async def __aenter__(self):
                return Client()
            async def __aexit__(self, *args):
                pass
        runtime = r.DaggerRuntime(Connection, lambda client: image,
                                  {SOURCE: r.SourceInput(object(), WRITABLE)},
                                  source_validator=validated_source)
        with self.assertRaises(r.Invalid):
            await runtime._start_reviewed(JOB, "read", SOURCE)
        self.assertFalse(image.service.calls)
        await runtime.stack.aclose()

    async def test_blocked_job_can_be_finished_without_any_runtime_effect(self):
        supervisor = r.Supervisor()
        await supervisor.handle(create())
        await supervisor.task
        await supervisor.handle(req("finish", 2))
        await supervisor.task
        self.assertEqual(supervisor.state, "finished")
        self.assertFalse(supervisor.retained)

    async def test_capacity_reserves_explicit_finish(self):
        runtime = FakeRuntime()
        supervisor = r.Supervisor(runtime)
        await supervisor.handle(create())
        await supervisor.task
        with patch.object(r, "MAX_OPERATIONS", 2):
            self.assertEqual((await supervisor.handle(prompt()))["code"], "capacity")
            response = await supervisor.handle(req("finish", 3))
            self.assertTrue(response["ok"])
            await supervisor.task
        self.assertEqual(supervisor.state, "finished")

    async def test_session_exists_before_failed_prompt_and_reused_afterwards(self):
        runtime = r.DaggerRuntime()
        runtime.session = "ses_test"
        calls = []
        async def exchange(operation_id, path, body):
            calls.append((operation_id, path, body))
            return {"kind": "failed", "text": ""}
        runtime._exchange = exchange
        for number in (2, 3):
            result = await runtime.prompt(f"{number:032x}", "next", False)
            self.assertEqual(result.state, "failed")
        self.assertEqual([c[1] for c in calls], ["/session/ses_test/message"] * 2)
        self.assertEqual(runtime.session, "ses_test")
        self.assertEqual(set(calls[0][2]), {"parts"})

    async def test_review_creates_distinct_session_and_never_changes_work_session(self):
        runtime = r.DaggerRuntime()
        runtime.session = "ses_work"
        calls = []
        async def exchange(operation_id, path, body):
            calls.append((path, body))
            return ({"kind": "session", "id": "ses_review"} if path == "/session"
                    else {"kind": "succeeded", "text": "review data, not approval"})
        runtime._exchange = exchange
        await runtime.prompt("c" * 32, "review", True)
        self.assertEqual(runtime.session, "ses_work")
        self.assertEqual([c[0] for c in calls], ["/session", "/session/ses_review/message"])
        self.assertEqual(calls[0][1]["permission"][0]["action"], "deny")

    async def test_malformed_or_ambiguous_runtime_response_is_not_known_failure(self):
        runtime = r.DaggerRuntime()
        runtime.session = "ses_test"
        async def exchange(*args):
            return {"kind": "unknown", "text": ""}
        runtime._exchange = exchange
        with self.assertRaises(r.Invalid):
            await runtime.prompt("c" * 32, "next", False)
        self.assertEqual(runtime.session, "ses_test")


class PreservationTests(unittest.IsolatedAsyncioTestCase):
    async def started(self):
        runtime = FakeRuntime()
        supervisor = r.Supervisor(runtime)
        await supervisor.handle(create())
        await supervisor.task
        return supervisor, runtime

    async def test_policy_blocked_finish_keeps_ownership_and_conversation(self):
        supervisor, runtime = await self.started()
        async def denied():
            raise r.PolicyBlocked()
        runtime.finish = denied
        first = await supervisor.handle(req("finish", 2))
        self.assertEqual(first["operation"]["state"], "pending")
        self.assertEqual((await supervisor.handle(prompt(3)))["code"], "busy")
        await supervisor.task
        observed = await supervisor.handle(req("observe", 4, target_operation_id=f"{2:032x}"))
        self.assertEqual(observed["operation"]["state"], "unknown")
        self.assertEqual(observed["operation"]["code"], "policy_denied")
        self.assertTrue(observed["job"]["retained"])
        self.assertTrue(observed["job"]["conversation"])
        self.assertEqual((await supervisor.handle(prompt(3)))["code"], "not_promptable")
        self.assertEqual((await supervisor.handle(req("finish", 2)))["operation"], observed["operation"])

    async def test_policy_blocked_prompt_and_partial_create_preserve_ownership(self):
        supervisor, runtime = await self.started()
        runtime.outputs = [r.PreEffectPolicyBlocked()]
        await supervisor.handle(prompt())
        await supervisor.task
        self.assertTrue(supervisor.retained)
        self.assertTrue(supervisor.conversation)
        self.assertEqual(supervisor.state, "unknown")
        runtime = FakeRuntime()
        async def partial(*args):
            raise r.PolicyBlocked()
        runtime.create = partial
        supervisor = r.Supervisor(runtime)
        await supervisor.handle(create())
        await supervisor.task
        self.assertTrue(supervisor.retained)
        self.assertEqual(supervisor.state, "unknown")

    async def test_pending_finish_duplicate_and_new_mutations_never_overlap(self):
        supervisor, runtime = await self.started()
        done = asyncio.Event()
        calls = []
        async def finish():
            calls.append("stop")
            await done.wait()
            return True
        runtime.finish = finish
        await supervisor.handle(req("finish", 2))
        await asyncio.sleep(0)
        self.assertEqual((await supervisor.handle(req("finish", 2)))["operation"]["state"], "pending")
        self.assertEqual((await supervisor.handle(req("finish", 3)))["code"], "busy")
        self.assertEqual((await supervisor.handle(prompt(4)))["code"], "busy")
        done.set()
        await supervisor.task
        self.assertEqual(calls, ["stop"])
        self.assertFalse(supervisor.retained)

    async def test_bad_response_utf8_preserves_prior_evidence_and_session(self):
        supervisor, runtime = await self.started()
        runtime.outputs = [r.Outcome("succeeded", "prior"), r.Outcome("succeeded", "\ud800")]
        await supervisor.handle(prompt())
        await supervisor.task
        await supervisor.handle(prompt(3))
        await supervisor.task
        self.assertEqual(supervisor.state, "unknown")
        self.assertTrue(supervisor.retained)
        self.assertTrue(supervisor.conversation)
        self.assertEqual(supervisor.operations[f"{2:032x}"][1]["text"], "prior")
        self.assertEqual(supervisor.operations[f"{3:032x}"][1]["text"], "")

    async def test_utf8_input_and_reply_limits_are_bytes_not_ascii_escaping(self):
        supervisor, runtime = await self.started()
        runtime.outputs = [r.Outcome("succeeded", "\u0001" * 8192)]
        text = "\u4e00" * 5461
        await supervisor.handle(prompt(text=text))
        await supervisor.task
        response = await supervisor.handle(req("observe", 3, target_operation_id=f"{2:032x}"))
        raw = r.frame(response)
        self.assertLessEqual(len(raw), r.MAX_REPLY)
        self.assertEqual(json.loads(raw)["operation"]["text"], "\u0001" * 8192)
        self.assertEqual(runtime.calls[-1][2], text)

    async def test_source_validator_is_mandatory_and_bound_before_connection(self):
        calls = []
        def forbidden():
            calls.append("connection")
            raise AssertionError()
        runtime = r.DaggerRuntime(forbidden, forbidden,
                                  {SOURCE: r.SourceInput(object(), WRITABLE)})
        with self.assertRaises(r.PreEffectPolicyBlocked):
            await runtime._start_reviewed(JOB, "write", SOURCE)
        async def mismatched(source_id, source):
            return r.ValidatedSource("c" * 32, source.directory, SOURCE_SHA, source.repositories)
        runtime.source_validator = mismatched
        with self.assertRaises(r.PreEffectPolicyBlocked):
            await runtime._start_reviewed(JOB, "write", SOURCE)
        self.assertEqual(calls, [])
        self.assertIsNone(runtime.client)

    async def test_partial_connection_failure_is_not_preeffect_or_verified_cleanup(self):
        class Connection:
            async def __aenter__(self):
                raise RuntimeError("partial engine acquisition")
            async def __aexit__(self, *args):
                pass
        runtime = r.DaggerRuntime(Connection, sources={SOURCE: r.SourceInput(object(), WRITABLE)},
                                  source_validator=validated_source)
        with self.assertRaises(RuntimeError):
            await runtime._start_reviewed(JOB, "write", SOURCE)
        self.assertIsNone(runtime.client)
        self.assertTrue(runtime.effects_possible)
        self.assertFalse(await runtime.finish())

    async def test_cleanup_only_literal_independent_true_can_finish(self):
        for verdict in (True, False, 1, None):
            with self.subTest(verdict=verdict):
                events = []
                runtime = r.DaggerRuntime()
                runtime.client = object()
                runtime.service = Service()
                runtime.cleanup_target = r.CleanupTarget(JOB, SOURCE, "service-test", r.encode(BOOT))
                async def closed():
                    events.append("disconnect")
                runtime.stack.push_async_callback(closed)
                async def verify(target):
                    self.assertEqual(runtime.service.calls, ["stop"])
                    self.assertEqual(events, ["disconnect"])
                    self.assertEqual(target.incarnation, r.encode(BOOT))
                    events.append("verify")
                    return verdict
                runtime.cleanup_verifier = verify
                self.assertEqual(await runtime.finish(), verdict is True)
                self.assertEqual(events, ["disconnect", "verify"])
                with self.assertRaises(r.Invalid):
                    await runtime._exchange("c" * 32, "/session", {})

    async def test_stop_failure_or_verifier_exception_never_claims_cleanup(self):
        runtime = r.DaggerRuntime()
        runtime.client = object()
        runtime.service = Service()
        runtime.cleanup_target = r.CleanupTarget(JOB, SOURCE, "service-test")
        calls = []
        async def denied():
            raise r.PolicyBlocked()
        async def verify(target):
            calls.append(target)
            raise RuntimeError("private diagnostic")
        runtime.service.stop = denied
        runtime.cleanup_verifier = verify
        with self.assertRaises(r.PolicyBlocked):
            await runtime.finish()
        self.assertEqual(calls, [])
        runtime.service = Service()
        with self.assertRaises(RuntimeError):
            await runtime.finish()
        self.assertEqual(calls, [runtime.cleanup_target])


class SourceCapabilityTests(unittest.IsolatedAsyncioTestCase):
    async def test_write_readonly_or_mixed_sources_refused_before_validator_or_connection(self):
        for repositories in (
            (r.RepositoryCapability(".", True),),
            (r.RepositoryCapability("editable", False), r.RepositoryCapability("reference", True)),
        ):
            with self.subTest(repositories=repositories):
                calls = []
                def forbidden():
                    calls.append("connect")
                    raise AssertionError()
                async def validator(*args):
                    calls.append("validator")
                    raise AssertionError()
                runtime = r.DaggerRuntime(forbidden, forbidden,
                                          {SOURCE: r.SourceInput(object(), repositories)},
                                          source_validator=validator)
                with self.assertRaises(r.PreEffectPolicyBlocked):
                    await runtime._start_reviewed(JOB, "write", SOURCE)
                self.assertEqual(calls, [])
                self.assertFalse(runtime.effects_possible)
                self.assertIsNone(runtime.client)

    async def test_unclassified_sources_and_invalid_paths_fail_closed(self):
        bad_maps = [(), [], None, (r.RepositoryCapability(".", 1),),
                    (r.RepositoryCapability("a", False), r.RepositoryCapability("a", False)),
                    (r.RepositoryCapability("a", False), r.RepositoryCapability("a/b", False)),
                    (r.RepositoryCapability(".", False), r.RepositoryCapability("a", False))]
        bad_maps += [(r.RepositoryCapability(path, False),) for path in (
            "", "/absolute", "../escape", "a/../b", "a//b", "./a", "a/", "a\\b", "a\n",
        )]
        bad_maps += [tuple(r.RepositoryCapability(str(i), False) for i in range(17))]
        entries = [object()] + [r.SourceInput(object(), capabilities) for capabilities in bad_maps]
        for admitted in entries:
            with self.subTest(admitted=admitted):
                calls = []
                def forbidden():
                    calls.append("connect")
                    raise AssertionError()
                async def validator(*args):
                    calls.append("validator")
                    raise AssertionError()
                runtime = r.DaggerRuntime(forbidden, forbidden, {SOURCE: admitted}, source_validator=validator)
                with self.assertRaises(r.PreEffectPolicyBlocked):
                    await runtime._start_reviewed(JOB, "write", SOURCE)
                self.assertEqual(calls, [])
                self.assertFalse(runtime.effects_possible)

    async def test_validator_cannot_change_authoritative_repository_permissions(self):
        for returned in (
            (r.RepositoryCapability(".", True),),
            (r.RepositoryCapability("different", False),),
            (),
        ):
            calls = []
            def forbidden():
                calls.append("connect")
                raise AssertionError()
            async def validator(source_id, admitted):
                return r.ValidatedSource(source_id, admitted.directory, SOURCE_SHA, returned)
            runtime = r.DaggerRuntime(forbidden, forbidden,
                                      {SOURCE: r.SourceInput(object(), WRITABLE)}, source_validator=validator)
            with self.subTest(returned=returned), self.assertRaises(r.PreEffectPolicyBlocked):
                await runtime._start_reviewed(JOB, "write", SOURCE)
            self.assertEqual(calls, [])
            self.assertFalse(runtime.effects_possible)

    async def test_read_mixed_and_write_all_writable_require_matching_validation(self):
        class ReachedConnection(Exception):
            pass
        for mode, repositories in (
            ("read", (r.RepositoryCapability("a", False), r.RepositoryCapability("b", True))),
            ("write", (r.RepositoryCapability("a", False), r.RepositoryCapability("b", False))),
        ):
            calls = []
            def connection():
                calls.append("connect")
                raise ReachedConnection()
            admitted = r.SourceInput(object(), repositories)
            async def validator(source_id, candidate):
                self.assertIs(candidate, admitted)
                calls.append("validate")
                return r.ValidatedSource(source_id, candidate.directory, SOURCE_SHA, candidate.repositories)
            runtime = r.DaggerRuntime(connection, sources={SOURCE: admitted}, source_validator=validator)
            with self.subTest(mode=mode), self.assertRaises(ReachedConnection):
                await runtime._start_reviewed(JOB, mode, SOURCE)
            self.assertEqual(calls, ["validate", "connect"])

    def test_runtime_source_binding_changes_with_verified_permissions(self):
        writable = r.ValidatedSource(SOURCE, object(), SOURCE_SHA,
                                     (r.RepositoryCapability("a", False),))
        readonly = r.ValidatedSource(SOURCE, writable.directory, SOURCE_SHA,
                                     (r.RepositoryCapability("a", True),))
        moved = r.ValidatedSource(SOURCE, writable.directory, SOURCE_SHA,
                                  (r.RepositoryCapability("b", False),))
        self.assertEqual(len({r.source_binding(source) for source in (writable, readonly, moved)}), 3)


if __name__ == "__main__":
    unittest.main()
