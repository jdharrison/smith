"""Offline SDK/orchestration tests: no Docker, processes, network or model calls."""

import asyncio
import io
import socket
import stat
import types
import subprocess
import unittest
from unittest.mock import AsyncMock, patch

from test_module_source import (
    AsyncFakeDag, AsyncFile, BASELINE, CHECK_SOURCE, CHECKER_SOURCE, Node,
    SDK, TASK, VALID, operations, wire,
)

SDK.Directory = type("Directory", (), {})
from smith_agent import coding, main
from smith_agent.artifacts import checked_artifact
from smith_agent.protocol import Invalid, decode, digest, encode, parse_request


def event(review):
    return encode({"type": "text", "part": {"text": encode(review)}}).encode() + b"\n"


def expected_from(prompt):
    return decode(prompt.split("exact bindings: ", 1)[1].split(". Add only", 1)[0].encode())


class Directory:
    def __init__(self, files=None):
        self.files = files or {}

    def with_new_file(self, path, *, contents, permissions):
        assert permissions == 0o600
        return Directory({**self.files, path: contents.encode()})


class CodingNode(Node):
    def __init__(self, sdk, root, steps=()):
        super().__init__(root, steps)
        self.sdk = sdk

    def step(self, name, *args, **kwargs):
        return CodingNode(self.sdk, self.root, self.steps + ((name, args, kwargs),))

    def role(self):
        return next(args[1] for args, _ in operations(self, "env")
                    if args[0] == "SMITH_FIXED_ROLE")

    def candidate(self):
        return next(options["contents"].encode() for args, options in operations(self, "new_file")
                    if args[0] == "/candidate/clamp.py")

    async def exit_code(self):
        self.sdk.evaluated.append(self)
        if self.sdk.evaluation_error:
            raise SDK.QueryError("PRIVATE provider error credential")
        if self.role().startswith("setup-"):
            return self.sdk.observations[self.role()][0]
        if self.role() == "checker":
            if self.sdk.checker_exit is not None:
                return self.sdk.checker_exit
            return 0 if CHECK_SOURCE(self.candidate())["passed"] else 1
        return self.sdk.model_exit

    def file(self, path):
        if self.role().startswith("setup-"):
            return AsyncFile(self.sdk.observations[self.role()][1])
        if self.role() == "checker":
            report = CHECK_SOURCE(self.candidate())
            if self.sdk.checker_tamper:
                report["passed"] = not report["passed"]
            return AsyncFile(encode(report).encode())
        return AsyncFile(self.sdk.model_files[path])

    async def stdout(self):
        return VALID.decode()


class FakeDag(AsyncFakeDag):
    def __init__(self):
        super().__init__()
        self.assets["src/smith_agent/bounded_exec.py"] = AsyncFile(b"# trusted wrapper fixture\n")
        self.checker_exit, self.checker_tamper = None, False
        self.model_exit = 0
        self.model_files = {"/private/status.json": b'{"exit_code":0,"reason":"exited"}',
                            "/private/stdout.jsonl": b"{}\n", "/private/stderr.txt": b""}

    def directory(self, path=None):
        return Directory() if path is None else super().directory(path)

    def container(self):
        self.containers += 1
        return CodingNode(self, self.containers)


class ReviewTests(unittest.TestCase):
    def setUp(self):
        artifact = checked_artifact(parse_request(wire("setup")), BASELINE, VALID,
                                    CHECKER_SOURCE, CHECK_SOURCE)
        self.expected = coding._binding("coding", 1, artifact)
        self.review = {**self.expected, "verdict": "pass", "findings": []}

    def test_actual_text_events_ignore_nontext_and_allow_text_chunks(self):
        raw = encode(self.review)
        stream = (b'{"type":"step_finish","part":{}}\n'
                  + encode({"type": "text", "part": {"text": raw[:30]}}).encode() + b"\n"
                  + encode({"type": "text", "part": {"text": raw[30:]}}).encode())
        self.assertEqual(coding.strict_review(stream, self.expected), self.review)

    def test_prose_fences_multiple_objects_duplicates_and_non_json_rejected(self):
        for text in ("hello " + encode(self.review), "```json\n" + encode(self.review) + "\n```",
                     encode(self.review) * 2, '{"schema_version":1,"schema_version":1}', "null"):
            with self.subTest(text=text[:30]), self.assertRaises(Invalid):
                coding.strict_review(encode({"type": "text", "part": {"text": text}}).encode(),
                                     self.expected)
        for raw in (b"", b"private error", b"[]", b'{"type":"text"}', b"x" * 65537):
            with self.subTest(raw=raw[:30]), self.assertRaises(Invalid):
                coding.strict_review(raw, self.expected)

    def test_error_events_reject_even_with_valid_pass_text(self):
        passed = event(self.review)
        error = b'{"type":"error","error":{"message":"PRIVATE provider failure"}}\n'
        for stream in (passed + error, error + passed):
            with self.subTest(error_last=stream.endswith(error)), self.assertRaises(Invalid):
                coding.strict_review(stream, self.expected)

    def test_structured_text_4096_total_cap_is_separate_from_stream_64kib(self):
        raw = encode(self.review)
        padded = raw + " " * (4096 - len(raw.encode()))
        def chunks(text):
            return b"\n".join(encode({"type": "text", "part": {"text": part}}).encode()
                              for part in (text[:2048], text[2048:]))
        stream = chunks(padded)
        self.assertGreater(len(stream), 4096)
        self.assertEqual(coding.strict_review(stream, self.expected), self.review)
        for oversized in (chunks(padded + " "),
                          encode({"type": "text", "part": {"text": padded + " "}}).encode()):
            self.assertLess(len(oversized), 65536)
            with self.assertRaises(Invalid):
                coding.strict_review(oversized, self.expected)
        metadata = encode({"type": "step_finish", "metadata": "x" * 8000}).encode() + b"\n"
        self.assertEqual(coding.strict_review(metadata + stream, self.expected), self.review)
        with self.assertRaises(Invalid):
            coding.strict_review(metadata * 9 + stream, self.expected)

    def test_closed_schema_and_all_bindings_including_types(self):
        changes = {"extra": 1, "schema_version": True, "attempt": 2, "stage": "response",
                   "candidate_sha256": "e" * 64, "checks_sha256": "f" * 64,
                   "response_sha256": "0" * 64}
        for key, value in changes.items():
            with self.subTest(key=key), self.assertRaises(Invalid):
                coding.strict_review(event({**self.review, key: value}), self.expected)

    def test_findings_and_verdict_are_strict(self):
        finding = {"code": "task_defect", "detail": "wrong result"}
        for findings, verdict in (([], "reject"), ([finding], "pass"), ([], "PASS"),
                                  ([finding] * 9, "reject"), ({}, "reject"),
                                  ([{"code": "other", "detail": "x"}], "reject"),
                                  ([{"code": "task_defect", "detail": "x" * 257}], "reject"),
                                  ([{"code": "task_defect", "detail": ""}], "reject"),
                                  ([{**finding, "extra": 1}], "reject")):
            with self.subTest(verdict=verdict, findings=str(findings)[:40]), self.assertRaises(Invalid):
                coding.strict_review(event({**self.review, "verdict": verdict, "findings": findings}),
                                     self.expected)
        accepted = {**self.review, "verdict": "reject", "findings": [finding]}
        self.assertEqual(coding.strict_review(event(accepted), self.expected), accepted)


class OrchestrationTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.sdk, self.calls = FakeDag(), []
        self.reject_code, self.reject_response = set(), set()
        self.response_code = "response_defect"
        self.candidates = [VALID, VALID]
        self.model_failure = None
        self.patches = [
            patch.object(socket.socket, "connect", side_effect=AssertionError("network denied")),
            patch.object(socket.socket, "connect_ex", side_effect=AssertionError("network denied")),
            patch.object(subprocess, "Popen", side_effect=AssertionError("process denied")),
        ]
        for item in self.patches:
            item.start()
            self.addCleanup(item.stop)

    async def invoke(self, runtime, stage, attempt, candidate, prompt, evidence):
        self.calls.append((stage, attempt, candidate, prompt, evidence))
        if self.model_failure:
            raise self.model_failure
        if stage == "coding":
            return self.candidates[attempt - 1]
        expected = expected_from(prompt)
        reject = attempt in (self.reject_code if stage == "review" else self.reject_response)
        code = "task_defect" if stage == "review" else self.response_code
        return event({**expected, "verdict": "reject" if reject else "pass",
                      "findings": [{"code": code, "detail": "bounded finding"}] if reject else []})

    async def run_fixture(self, raw=None, enabled=True):
        async def invoke(runtime, *args):
            return await self.invoke(runtime, *args)
        with patch.object(coding.Runtime, "invoke", invoke):
            directory = await coding.run_coding_fixture(
                self.sdk, wire("setup") if raw is None else raw, enabled=enabled, any_exit="ANY")
        self.result = decode(directory.files["result.json"])
        return directory

    async def test_default_disabled_no_request_source_image_exec_or_model(self):
        result = await self.run_fixture(b"not json", enabled=False)
        self.assertEqual(self.result["reason"], "policy_denied")
        self.assertEqual(set(result.files), {"result.json", "manifest.json"})
        self.assertEqual((self.sdk.containers, self.sdk.asset_reads, self.calls), (0, [], []))
        await self.run_fixture(enabled=1)
        self.assertEqual(self.result["reason"], "policy_denied")

    async def test_only_initial_strict_setup_schema_before_assets(self):
        for raw in (wire("execute"), wire("submit"), wire("setup", attempt=2), b"{}",
                    wire("setup", extra=True)):
            await self.run_fixture(raw)
            self.assertEqual(self.result["status"], "failed")
        self.assertEqual((self.sdk.containers, self.sdk.asset_reads, self.calls), (0, [], []))

    async def test_wrong_baseline_and_oversized_assets_fail_before_image(self):
        await self.run_fixture(wire("setup", payload={"schema_version": 1, "baseline": {}}))
        self.assertEqual(self.sdk.containers, 0)
        self.sdk.assets["fixtures/clamp/clamp.py"] = AsyncFile(b"x" * 4097)
        await self.run_fixture()
        self.assertEqual(self.sdk.containers, 0)
        self.assertEqual(self.calls, [])

    async def test_happy_path_actual_setup_checker_three_sessions_and_retention(self):
        directory = await self.run_fixture()
        self.assertEqual(self.result["status"], "validated")
        self.assertEqual(self.result["phase"], "response")
        self.assertEqual(self.result["cleanup"], "not_observed")
        self.assertEqual(self.result["model_sessions"], 3)
        self.assertEqual([call[0] for call in self.calls], ["coding", "review", "response-review"])
        self.assertEqual(self.result["identity"], decode(parse_request(wire("setup")).identity.encode()))
        self.assertEqual([node.role() for node in self.sdk.evaluated],
                         ["setup-version", "setup-probe", "setup-checker", "checker"])
        self.assertEqual(directory.files["attempts/coding-1/candidate.py"], VALID)
        setup = decode(directory.files["setup.json"])
        self.assertEqual(setup["status"], "runtime_ready")
        self.assertIs(setup["runtime_verified"], True)
        self.assertEqual(setup["identity"], self.result["identity"])
        self.assertEqual(setup["probe"]["baseline_sha256"], digest(BASELINE))
        self.assertEqual(setup["probe"]["task_sha256"], digest(TASK))
        self.assertEqual(setup["checker_sha256"], digest(CHECKER_SOURCE))
        self.assertEqual(setup["baseline_check"], CHECK_SOURCE(BASELINE))
        manifest = decode(directory.files["manifest.json"])
        for entry in manifest["files"]:
            raw = directory.files[entry["path"]]
            self.assertEqual((entry["size"], entry["sha256"]), (len(raw), digest(raw)))
        self.assertLessEqual(sum(map(len, directory.files.values())), 256 * 1024)
        self.assertLessEqual(len(directory.files), 24)
        self.assertTrue(all("stdout" not in path and "stderr" not in path for path in directory.files))
        self.assertIn(b"not general correctness", directory.files["attempts/response-1/response.txt"])
        self.assertLessEqual(len(directory.files["attempts/response-1/response.txt"]), 2048)

    async def test_setup_fault_never_replayed_or_model_invoked(self):
        self.sdk.evaluation_error = True
        await self.run_fixture()
        self.assertEqual(len(self.sdk.evaluated), 1)
        self.assertEqual(self.calls, [])
        self.assertEqual(self.result["reason"], "prerequisite")
        self.assertNotIn("PRIVATE", encode(self.result))

    async def test_rejected_observations_are_not_readiness(self):
        self.sdk.observations["setup-version"] = (0, b"wrong-version")
        await self.run_fixture()
        self.assertEqual(self.calls, [])
        self.assertEqual(self.result["status"], "failed")

    async def test_coding_retry_receives_validated_findings_and_prior_candidate(self):
        self.reject_code = {1}
        directory = await self.run_fixture()
        self.assertEqual(self.result["model_sessions"], 5)
        self.assertEqual(self.result["status"], "validated")
        self.assertEqual(self.calls[2][2], VALID)
        self.assertIn("bounded finding", self.calls[2][3])
        self.assertIn("attempts/coding-1/review.json", directory.files)
        self.assertIn("attempts/coding-2/review.json", directory.files)

    async def test_failed_checker_cannot_be_overridden_by_review_pass(self):
        self.candidates = [BASELINE, BASELINE]
        await self.run_fixture()
        self.assertEqual(self.result["reason"], "coding_rejected")
        self.assertEqual(self.result["model_sessions"], 4)
        self.assertIn("Fixed checker failed", self.calls[2][3])
        self.assertFalse(any(call[0] == "response-review" for call in self.calls))

    async def test_checker_report_and_exit_must_both_match_host_recomputation(self):
        for code, tamper in ((1, False), (True, False), (None, True)):
            self.calls.clear()
            self.sdk.checker_exit, self.sdk.checker_tamper = code, tamper
            await self.run_fixture()
            self.assertEqual(self.result["reason"], "prerequisite")
            self.assertEqual([call[0] for call in self.calls], ["coding"])

    async def test_response_only_repair_with_structured_findings_max_six_sessions(self):
        self.reject_code, self.reject_response = {1}, {1}
        directory = await self.run_fixture()
        self.assertEqual(self.result["status"], "validated")
        self.assertEqual(self.result["model_sessions"], 6)
        first = directory.files["attempts/response-1/response.txt"]
        second = directory.files["attempts/response-2/response.txt"]
        self.assertNotEqual(first, second)
        self.assertEqual([call[0] for call in self.calls].count("coding"), 2)
        self.assertEqual(self.calls[-1][2], self.calls[-2][2])
        self.assertEqual(decode(self.calls[-1][4]["prior-findings.json"]),
                         [{"code": "response_defect", "detail": "bounded finding"}])
        self.assertLessEqual(len(directory.files), 24)

    async def test_response_hashes_bind_final_artifact_and_scope_across_prose_repair(self):
        self.reject_code, self.reject_response = {1}, {1}
        self.candidates = [VALID + b"\n", VALID]
        directory = await self.run_fixture()
        artifact = checked_artifact(parse_request(wire("setup")), BASELINE, VALID,
                                    CHECKER_SOURCE, CHECK_SOURCE)
        for attempt in (1, 2):
            response = directory.files[f"attempts/response-{attempt}/response.txt"]
            review = decode(directory.files[f"attempts/response-{attempt}/response-review.json"])
            self.assertEqual(response, coding._response(artifact, repair=attempt == 2))
            self.assertEqual(review["response_sha256"], digest(response))
            self.assertEqual(review["candidate_sha256"], digest(VALID))
            self.assertEqual(review["checks_sha256"], digest(coding._json(artifact["check"])))
            for name, value in (("candidate_sha256", digest(VALID)),
                                ("checks_sha256", review["checks_sha256"]),
                                ("patch_sha256", artifact["patch_sha256"])):
                self.assertIn(f"{name}={value}".encode(), response)
            self.assertNotIn(digest(self.candidates[0]).encode(), response)
            self.assertIn(b"fixture", response.lower())
            self.assertIn(b"triangulation", response)
            self.assertIn(b"Effective model/provider unknown", response)
            call = next(call for call in self.calls if call[:2] == ("response-review", attempt))
            self.assertEqual(call[2], VALID)
            self.assertEqual(call[4]["check.json"], directory.files["attempts/coding-2/check.json"])
            self.assertEqual(call[4]["patch.json"], directory.files["attempts/coding-2/patch.json"])
            self.assertEqual(call[4]["response.txt"], response)

    async def test_response_defect_exhaustion_is_not_validated(self):
        self.reject_response = {1, 2}
        await self.run_fixture()
        self.assertEqual(self.result["reason"], "response_rejected")
        self.assertEqual(self.result["model_sessions"], 4)

    async def test_response_artifact_findings_invalidate_coding_without_prose_repair(self):
        for code in ("task_defect", "check_mismatch", "unsupported_claim"):
            self.calls.clear()
            self.reject_response, self.response_code = {1}, code
            await self.run_fixture()
            self.assertEqual(self.result["reason"], "artifact_invalidated")
            self.assertEqual(self.result["phase"], "coding")
            self.assertEqual(self.result["model_sessions"], 3)

    async def test_invalid_review_or_candidate_stops_without_response_or_raw_logs(self):
        self.candidates = [b"x" * 4097, VALID]
        directory = await self.run_fixture()
        self.assertEqual(self.result["status"], "failed")
        self.assertEqual(len(self.calls), 1)
        self.assertEqual(set(directory.files), {"result.json", "manifest.json", "setup.json"})
        async def bad_review(runtime, stage, *args):
            return VALID if stage == "coding" else b'PRIVATE provider error'
        with patch.object(coding.Runtime, "invoke", bad_review):
            directory = await coding.run_coding_fixture(self.sdk, wire("setup"), enabled=True, any_exit="ANY")
        self.assertEqual(decode(directory.files["result.json"])["model_sessions"], 2)
        self.assertNotIn(b"PRIVATE", b"".join(directory.files.values()))

    async def test_coding_error_after_edit_exit_zero_stops_before_candidate_review_or_repair(self):
        self.sdk.model_files["/private/stdout.jsonl"] = b"\n".join(encode(item).encode() for item in (
            {"type": "step_start"},
            {"type": "tool_use", "part": {"tool": "edit", "content": VALID.decode()}},
            {"type": "text", "part": {"text": "Edited clamp.py"}},
            {"type": "error", "error": {"message": "PRIVATE provider diagnostic"}},
        ))
        original_invoke = coding.Runtime.invoke
        async def invoke(runtime, stage, attempt, candidate, prompt, evidence):
            if stage != "coding":
                return await self.invoke(runtime, stage, attempt, candidate, prompt, evidence)
            self.calls.append((stage, attempt, candidate, prompt, evidence))
            return await original_invoke(runtime, stage, attempt, candidate, prompt, evidence)
        with (patch.object(coding.Runtime, "invoke", invoke),
              patch.object(CodingNode, "stdout", new_callable=AsyncMock,
                           return_value=VALID.decode()) as candidate_read):
            directory = await coding.run_coding_fixture(self.sdk, wire("setup"), enabled=True, any_exit="ANY")
        result = decode(directory.files["result.json"])
        self.assertEqual((result["phase"], result["status"], result["reason"], result["model_sessions"]),
                         ("coding", "failed", "prerequisite", 1))
        self.assertEqual([call[:2] for call in self.calls], [("coding", 1)])
        candidate_read.assert_not_awaited()
        self.assertEqual([node.role() for node in self.sdk.evaluated],
                         ["setup-version", "setup-probe", "setup-checker", "coding"])
        self.assertEqual(set(directory.files), {"setup.json", "result.json", "manifest.json"})
        self.assertNotIn(b"PRIVATE", b"".join(directory.files.values()))

    async def test_model_error_never_returns_private_provider_messages(self):
        self.model_failure = SDK.QueryError("PRIVATE provider error credential")
        directory = await self.run_fixture()
        self.assertEqual(self.result["status"], "failed")
        self.assertNotIn(b"PRIVATE", b"".join(directory.files.values()))
        self.assertEqual(decode(directory.files["setup.json"])["status"], "runtime_ready")

    async def test_deadline_is_original_and_covers_asset_reads_and_later_work(self):
        with patch.object(coding, "MAX_SECONDS", 0.001):
            async def stalled():
                await asyncio.sleep(0.02)
                return len(BASELINE)
            self.sdk.assets["fixtures/clamp/clamp.py"].size = stalled
            await self.run_fixture()
        self.assertEqual(self.result["reason"], "deadline_exceeded")
        self.assertEqual(self.sdk.containers, 0)
        self.sdk.assets["fixtures/clamp/clamp.py"] = AsyncFile(BASELINE)
        async def late(runtime, *args):
            runtime.budget.deadline = 0
            return VALID
        with patch.object(coding.Runtime, "invoke", late):
            directory = await coding.run_coding_fixture(self.sdk, wire("setup"), enabled=True, any_exit="ANY")
        self.assertEqual(decode(directory.files["result.json"])["reason"], "deadline_exceeded")


class ExportTests(unittest.IsolatedAsyncioTestCase):
    async def test_default_disabled_does_not_read_request_or_construct_image_or_model(self):
        sdk = FakeDag()
        request = types.SimpleNamespace(size=AsyncMock(side_effect=AssertionError("File read")),
                                        contents=AsyncMock(side_effect=AssertionError("File read")))
        with (patch.object(main, "dag", sdk),
              patch.object(coding.Runtime, "invoke", new_callable=AsyncMock) as invoke):
            directory = await main.SmithAgent().run_coding_fixture(request)
        self.assertEqual(decode(directory.files["result.json"])["reason"], "policy_denied")
        self.assertEqual(set(directory.files), {"result.json", "manifest.json"})
        request.size.assert_not_awaited()
        request.contents.assert_not_awaited()
        invoke.assert_not_awaited()
        self.assertEqual((sdk.containers, sdk.asset_reads, sdk.evaluated), (0, [], []))

    async def test_enabled_reads_exact_request_and_forwards_to_orchestrator(self):
        sdk, raw, expected = FakeDag(), wire("setup") + b"\n", Directory()
        request = AsyncFile(raw)
        with (patch.object(main, "dag", sdk),
              patch.object(coding, "run_coding_fixture", new_callable=AsyncMock,
                           return_value=expected) as evaluate):
            directory = await main.SmithAgent().run_coding_fixture(request, enable_coding=True)
        self.assertIs(directory, expected)
        self.assertEqual(request.reads, 1)
        evaluate.assert_awaited_once_with(sdk, raw, enabled=True, any_exit=SDK.ReturnType.ANY)
        self.assertEqual(sdk.containers, 0)


class RuntimeTests(unittest.IsolatedAsyncioTestCase):
    def runtime(self):
        sdk = FakeDag()
        runtime = coding.Runtime(sdk, parse_request(wire("setup")),
                                 (BASELINE, TASK, CHECKER_SOURCE, b"# trusted"), coding.Budget(), "ANY")
        return sdk, runtime

    async def test_model_command_wrapper_environment_permissions_and_fresh_siblings(self):
        sdk, runtime = self.runtime()
        self.assertEqual(await runtime.invoke("coding", 1, BASELINE, "fixed prompt", {}), VALID)
        await runtime.invoke("review", 1, VALID, "review prompt", {"check.json": b"{}"})
        await runtime.invoke("response-review", 1, VALID, "response prompt", {})
        nodes = [node for node in sdk.evaluated if len(operations(node, "exec")) == 1]
        self.assertEqual(len(nodes), 3)
        self.assertEqual(sdk.containers, 1)  # Cached base, sibling immutable graph chains.
        for node in nodes:
            commands = operations(node, "exec")
            argv, options = commands[0][0][0], commands[0][1]
            self.assertEqual(options["expect"], "ANY")
            self.assertEqual(argv[:2], ["/usr/bin/env", "-i"])
            position = argv.index("/usr/bin/python3")
            self.assertEqual(argv[position:position + 4],
                             ["/usr/bin/python3", "-I", "-B", "/trusted/bounded_exec.py"])
            seconds = float(argv[argv.index("--seconds") + 1])
            self.assertTrue(0 < seconds <= 120)
            for flag in ("--stdout", "--stderr", "--status"):
                self.assertIn(flag, argv)
            inner = argv[argv.index("--") + 1:]
            self.assertEqual(inner[:2], ["/usr/bin/env", "-i"])
            self.assertNotIn("--model", inner)
            self.assertNotIn("--session", inner)
            self.assertNotIn("--continue", inner)
            self.assertIn("SMITH_ATTEMPT_IDENTITY=" + runtime.request.identity, inner)
            self.assertIn("SMITH_STAGE=" + node.role(), inner)
            self.assertIn("SMITH_ATTEMPT=1", inner)
            self.assertFalse(any("API_KEY" in arg or "AUTH_TOKEN" in arg for arg in argv))
            for env in coding.ENVIRONMENT:
                self.assertIn(env, argv)
            wrapper = [(args, options) for args, options in operations(node, "new_file")
                       if args[0] == "/trusted/bounded_exec.py"][0]
            self.assertEqual(wrapper[1]["owner"], "root:root")
            self.assertEqual(wrapper[1]["permissions"], 0o644)
            self.assertEqual(len(operations(node, "build")), 1)
            identity = dict(args for args, _ in operations(node, "env"))
            self.assertEqual(identity["SMITH_ATTEMPT_IDENTITY"], runtime.request.identity)
            self.assertEqual(identity["SMITH_ATTEMPT"], "1")
        self.assertIn('"edit":"deny"', " ".join(operations(nodes[-1], "exec")[0][0][0]))
        self.assertIn('"edit":"allow"', " ".join(operations(nodes[0], "exec")[0][0][0]))
        compile(coding.READ_CANDIDATE, "<trusted-reader>", "exec")
        self.assertIn("O_NOFOLLOW", coding.READ_CANDIDATE)
        self.assertIn("S_ISREG", coding.READ_CANDIDATE)
        self.assertIn("4097", coding.READ_CANDIDATE)

    async def test_coding_stream_rejects_malformed_nonobject_or_error_before_candidate_read(self):
        for stream in (b"not json\n", b"[]\n", b"null\n", b"42\n",
                       b'{"type":"text","type":"error"}\n', b'{"type":"error"}\n'):
            sdk, runtime = self.runtime()
            sdk.model_files["/private/stdout.jsonl"] = b'{"type":"step_start"}\n' + stream
            with (self.subTest(stream=stream),
                  patch.object(CodingNode, "stdout", new_callable=AsyncMock) as candidate_read):
                with self.assertRaises(Invalid):
                    await runtime.invoke("coding", 1, BASELINE, "prompt", {})
                candidate_read.assert_not_awaited()
                self.assertEqual(len(sdk.evaluated), 1)

    async def test_coding_stream_accepts_success_events_without_rigid_type_allowlist(self):
        sdk, runtime = self.runtime()
        sdk.model_files["/private/stdout.jsonl"] = b"\n".join(
            encode({"type": kind, "part": {"text": "unstructured coding text"}}).encode()
            for kind in ("step_start", "tool_use", "text", "step_finish", "future_event"))
        self.assertEqual(await runtime.invoke("coding", 1, BASELINE, "prompt", {}), VALID)

    async def test_wrapper_failure_status_output_limits_fail_closed(self):
        cases = [({"exit_code": 0, "reason": "timed_out"}, 0, b"", b""),
                 ({"exit_code": 1, "reason": "exited"}, 0, b"", b""),
                 ({"exit_code": True, "reason": "exited"}, 0, b"", b""),
                 ({"exit_code": 0, "reason": "exited"}, 1, b"", b""),
                 ({"exit_code": 0, "reason": "exited"}, 0, b"x" * 65537, b""),
                 ({"exit_code": 0, "reason": "exited"}, 0, b"", b"x" * 16385)]
        for status, code, out, err in cases:
            sdk, runtime = self.runtime()
            sdk.model_exit = code
            sdk.model_files.update({"/private/status.json": encode(status).encode(),
                                    "/private/stdout.jsonl": out, "/private/stderr.txt": err})
            with self.subTest(status=status, code=code, out=len(out), err=len(err)), self.assertRaises(Invalid):
                await runtime.invoke("review", 1, VALID, "prompt", {})

    async def test_budget_checks_after_await_and_before_construction(self):
        sdk, runtime = self.runtime()
        runtime.budget.deadline = 0
        with self.assertRaises(TimeoutError):
            await runtime.invoke("coding", 1, BASELINE, "prompt", {})
        self.assertEqual(sdk.containers, 0)
        budget = coding.Budget()
        async def overrun():
            budget.deadline = 0
        with self.assertRaises(TimeoutError):
            await budget.call(overrun)

    async def test_sdk_size_checks_precede_contents_and_match_exact_bytes(self):
        budget = coding.Budget()
        for raw, size, cap in ((b"abc", 4097, 4096), (b"abc", True, 4096), (b"abc", 2, 4096)):
            file = AsyncFile(raw, size=size)
            with self.assertRaises(Invalid):
                await budget.read(file, cap)
            self.assertEqual(file.reads, 1 if size == 2 else 0)


class CandidateReaderTests(unittest.TestCase):
    def read(self, raw, *, mode=stat.S_IFREG, declared_size=None, inode=1):
        size = len(raw) if declared_size is None else declared_size
        info = types.SimpleNamespace(st_mode=mode, st_size=size, st_dev=1, st_ino=1)
        opened = types.SimpleNamespace(st_mode=mode, st_size=size, st_dev=1, st_ino=inode)
        calls = []
        def opened_fd(path, flags, **options):
            calls.append((path, flags, options))
            return len(calls)
        fake_os = types.SimpleNamespace(
            O_RDONLY=0, O_DIRECTORY=1, O_NOFOLLOW=2, O_NONBLOCK=4,
            open=opened_fd, stat=lambda *args, **kwargs: info,
            fstat=lambda fd: opened, read=lambda fd, count: raw[:count])
        output = io.BytesIO()
        modules = {"os": fake_os, "stat": stat,
                   "sys": types.SimpleNamespace(stdout=types.SimpleNamespace(buffer=output))}
        builtins = {"__import__": lambda name, *args: modules[name], "len": len,
                    "AssertionError": AssertionError}
        exec(compile(coding.READ_CANDIDATE, "<trusted-reader>", "exec"),
             {"__builtins__": builtins})
        self.assertEqual(calls[0][1] & fake_os.O_NOFOLLOW, fake_os.O_NOFOLLOW)
        self.assertEqual(calls[1][1] & fake_os.O_NOFOLLOW, fake_os.O_NOFOLLOW)
        return output.getvalue()

    def test_regular_utf8_exact_bytes_only_never_imports_candidate(self):
        self.assertEqual(self.read(VALID), VALID)
        self.assertEqual(self.read(b"import forbidden_candidate\n"), b"import forbidden_candidate\n")

    def test_symlink_fifo_directory_oversize_encoding_and_replacement_rejected(self):
        for mode in (stat.S_IFLNK, stat.S_IFIFO, stat.S_IFDIR):
            with self.subTest(mode=mode), self.assertRaises(AssertionError):
                self.read(VALID, mode=mode)
        for raw, options in ((b"x" * 4097, {}), (bytes([255]), {}), (bytes([0]), {}),
                             (VALID, {"inode": 2}), (VALID, {"declared_size": 2})):
            with self.subTest(options=options, size=len(raw)), self.assertRaises((AssertionError, UnicodeError)):
                self.read(raw, **options)


if __name__ == "__main__":
    unittest.main()
