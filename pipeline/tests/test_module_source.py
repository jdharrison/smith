"""Offline source contracts. The fake SDK never evaluates an engine graph."""

import asyncio
import copy
import hashlib
import json
from pathlib import Path
import sys
import tomllib
import types
import unittest
from unittest.mock import patch

# Decorator/type shape only. No installed Dagger package is imported.
SDK = types.ModuleType("dagger")
SDK.function = lambda function: function
SDK.object_type = lambda cls: cls
SDK.File = type("File", (), {})
SDK.dag = None
SDK.ReturnType = types.SimpleNamespace(ANY="ANY")
SDK.QueryError = type("QueryError", (Exception,), {})
sys.modules["dagger"] = SDK
from smith_agent import SmithAgent
from smith_agent.artifacts import (
    checked_artifact, patch_for, phase_payload, read_snapshot, reconstruct,
    snapshot, validate_submission,
)
from smith_agent.graph import (
    CODE_PROMPT, ENVIRONMENT, FLAGS, OPENCODE_ARCHIVE_SHA256, REVIEW_PROMPT,
    SETUP_PROBE, build_graph, build_setup_graph, opencode_command,
)
from smith_agent.main import run_source_request as run_source_request_bytes, trusted_inputs
from smith_agent.protocol import (
    ERRORS, Invalid, decode as decode_bytes, digest, encode, parse_request, response,
)

TEST_ROOT = Path(__file__).absolute().parents[1]
BASELINE = (TEST_ROOT / "fixtures/clamp/clamp.py").read_bytes()
CHECKER_SOURCE = (TEST_ROOT / "tests/check_clamp.py").read_bytes()
TASK = (TEST_ROOT / "fixtures/clamp/TASK.md").read_bytes()
BASELINE, CHECKER_SOURCE, CHECK_SOURCE = trusted_inputs(BASELINE, CHECKER_SOURCE)


def run_source_request(raw):
    return run_source_request_bytes(raw, BASELINE, CHECKER_SOURCE)
VALID = b'''def clamp(value, low, high):
    if low > high:
        raise ValueError()
    if value < low:
        return low
    if value > high:
        return high
    return value
'''


def decode(value):
    """Read either the UTF-8 wire bytes or the exported JSON scalar in tests."""
    return decode_bytes(value.encode() if isinstance(value, str) else value)


def wire(phase="execute", payload=None, **changes):
    if payload is None:
        payload = {"schema_version": 1, "baseline": snapshot(BASELINE)}
    value = {"schema_version": 1, "run_id": "a" * 32, "phase": phase,
             "attempt": 1, "contract_sha256": "b" * 64,
             "executable_sha256": "c" * 64, "module_sha256": "d" * 64,
             "input": encode(payload)}
    value.update(changes)
    return encode(value).encode()


def submission(candidate=VALID):
    change = patch_for(BASELINE, candidate)
    text = "Candidate passes the fixed deterministic checker; semantic review is pending."
    return {"schema_version": 1, "baseline": snapshot(BASELINE),
            "candidate": snapshot(candidate), "patch": change,
            "patch_sha256": digest(encode(change).encode()),
            "checker_sha256": digest(CHECKER_SOURCE), "check": CHECK_SOURCE(candidate),
            "response": text, "response_sha256": digest(text.encode()),
            "execute_identity": decode(parse_request(wire()).identity)}


class ProtocolTests(unittest.TestCase):
    def test_original_bytes_not_reserialization(self):
        original = wire()
        spaced = b"\n  " + original + b"\n"
        parsed = parse_request(spaced)
        self.assertEqual(parsed.request_sha256, hashlib.sha256(spaced).hexdigest())
        self.assertNotEqual(parsed.request_sha256, parse_request(original).request_sha256)
        self.assertEqual(decode(response(spaced, error="policy_denied"))["request_sha256"],
                         parsed.request_sha256)

    def test_top_level_duplicate_rejected(self):
        raw = wire().replace(b'{', b'{"schema_version":1,', 1)
        with self.assertRaises(Invalid):
            parse_request(raw)

    def test_nested_duplicates_rejected(self):
        raw = wire(input='{"schema_version":1,"baseline":{"path":"clamp.py","path":"evil"}}')
        with self.assertRaises(Invalid):
            phase_payload(parse_request(raw), BASELINE)

    def test_strict_envelope(self):
        cases = [dict(schema_version=True), dict(schema_version=2), dict(attempt=True),
                 dict(attempt=0), dict(attempt=3), dict(attempt=1.0), dict(phase="review"),
                 dict(phase=[]), dict(run_id="A" * 32), dict(run_id="a" * 31),
                 dict(contract_sha256="B" * 64), dict(module_sha256="0" * 63),
                 dict(executable_sha256="x" * 64), dict(model="injected"),
                 dict(input={}), dict(input="\0"), dict(input="\ud800"),
                 dict(input="é" * 8193)]
        for changes in cases:
            with self.subTest(changes=list(changes)):
                with self.assertRaises(Invalid):
                    parse_request(wire(**changes))

    def test_missing_fields(self):
        for key in decode(wire()):
            value = decode(wire())
            del value[key]
            with self.subTest(key=key), self.assertRaises(Invalid):
                parse_request(encode(value).encode())

    def test_invalid_json(self):
        for raw in (b"[]", b"null", b"{}\0", b"\xff", b'{"a":NaN}',
                    b'{"a":Infinity}', b" " * 32769, b"[" * 2000):
            with self.subTest(raw=raw[:30]), self.assertRaises(Invalid):
                parse_request(raw)

    def test_qa_encoded_response_cannot_exceed_host_capture_bound(self):
        for body in (chr(1) * 16384, '\\' * 16384):
            with self.subTest(kind=ord(body[0])):
                try:
                    encoded = response(wire(), body=body).encode('utf-8')
                except Invalid:
                    continue
                self.assertLessEqual(len(encoded), 65536)
        for body in ('é' * 8192, '😀' * 4096, 'x' * 16384):
            encoded = response(wire(), body=body).encode('utf-8')
            self.assertLessEqual(len(encoded), 65536)
            self.assertEqual(decode_bytes(encoded, limit=65536)['body'], body)

    def test_response_exclusivity_bounds_and_error_allowlist(self):
        for error in ERRORS:
            result = decode(response(wire(), error=error))
            self.assertEqual(result["outcome"], "failed")
            self.assertNotIn("body", result)
        for arguments in ({}, {"body": "x", "error": "provider"}, {"error": "raw secret"},
                          {"body": ""}, {"body": " "}, {"body": "x" * 16385}):
            with self.subTest(arguments=list(arguments)), self.assertRaises(Invalid):
                response(wire(), **arguments)
        self.assertEqual(decode(response(wire(), body="x" * 16384))["outcome"], "produced")


class ArtifactTests(unittest.TestCase):
    def test_exact_patch_reconstruction(self):
        candidate = VALID.replace(b"\n", b"\r\n") + b"# exact trailing bytes\n\n"
        change = patch_for(BASELINE, candidate)
        self.assertEqual(reconstruct(BASELINE, change), candidate)
        self.assertEqual(read_snapshot(snapshot(candidate)), candidate)
        self.assertNotEqual(digest(candidate), digest(VALID))

    def test_snapshot_rejects_paths_hashes_and_limits(self):
        for changes in ({"path": "../clamp.py"}, {"path": "/workspace/clamp.py"},
                        {"sha256": "0" * 64}, {"content": "x" * 4097},
                        {"content": "\ud800"}, {"content": "\0"}, {"extra": True}):
            value = snapshot(VALID)
            value.update(changes)
            with self.subTest(changes=list(changes)), self.assertRaises(Invalid):
                read_snapshot(value)
        with self.assertRaises(Invalid):
            snapshot(b"\xff")

    def test_patch_rejects_tampering(self):
        for key, replacement in (("format", "unified"), ("path", "tests/check_clamp.py"),
                                 ("base_sha256", "0" * 64), ("candidate_sha256", "0" * 64),
                                 ("replacement", VALID.decode() + "\n"), ("command", "evil")):
            change = patch_for(BASELINE, VALID)
            change[key] = replacement
            with self.subTest(key=key), self.assertRaises(Invalid):
                reconstruct(BASELINE, change)

    def test_trusted_checker_compiles_the_exact_hashed_snapshot(self):
        modified = CHECKER_SOURCE.replace(b"smith.clamp.restricted-ast.v1",
                                          b"smith.test.byte-binding.v1")
        with patch.object(Path, "read_bytes", side_effect=AssertionError("filesystem read")):
            baseline, source, checker = trusted_inputs(BASELINE, modified)
        self.assertEqual(baseline, BASELINE)
        self.assertEqual(source, modified)
        self.assertEqual(checker(VALID)["check_id"], "smith.test.byte-binding.v1")
        artifact = checked_artifact(parse_request(wire()), baseline, VALID, source, checker)
        self.assertEqual(artifact["checker_sha256"], digest(modified))

    def test_real_checker_bound_to_exact_bytes(self):
        request = parse_request(wire())
        for candidate, expected in ((VALID, True), (BASELINE, False),
                                    (b"import os\n", False)):
            artifact = checked_artifact(request, BASELINE, candidate,
                                        CHECKER_SOURCE, CHECK_SOURCE)
            self.assertEqual(artifact["check"]["passed"], expected)
            self.assertEqual(artifact["check"]["source_sha256"], digest(candidate))
            self.assertEqual(artifact["checker_sha256"], digest(CHECKER_SOURCE))
            self.assertEqual(artifact["identity"]["request_sha256"], digest(request.raw))
            self.assertEqual(reconstruct(BASELINE, artifact["patch"]), candidate)

    def test_checker_claim_cannot_replace_real_check(self):
        payload = submission(BASELINE)
        payload["check"]["passed"] = True
        request = parse_request(wire("submit", payload))
        with self.assertRaises(Invalid):
            validate_submission(request, phase_payload(request, BASELINE),
                                CHECKER_SOURCE, CHECK_SOURCE)

    def test_phase_payloads_are_explicit_and_closed(self):
        for phase in ("setup", "execute", "submit"):
            for payload in ({}, {"schema_version": True, "baseline": snapshot(BASELINE)},
                            {"schema_version": 1, "baseline": snapshot(VALID)},
                            {"schema_version": 1, "baseline": snapshot(BASELINE),
                             "allow_network": True}):
                with self.subTest(phase=phase, keys=list(payload)), self.assertRaises(Invalid):
                    phase_payload(parse_request(wire(phase, payload)), BASELINE)

    def test_submit_prior_identity_must_bind_same_run_and_contract(self):
        for key, value in (("run_id", "0" * 32), ("phase", "setup"), ("attempt", True),
                           ("request_sha256", "invalid"), ("module_sha256", "0" * 64),
                           ("contract_sha256", "0" * 64), ("executable_sha256", "0" * 64),
                           ("extra", True)):
            payload = submission()
            payload["execute_identity"][key] = value
            request = parse_request(wire("submit", payload))
            with self.subTest(key=key), self.assertRaises(Invalid):
                phase_payload(request, BASELINE)

    def test_submit_tampering_is_recomputed(self):
        for key in ("candidate", "patch", "patch_sha256", "checker_sha256",
                    "check", "response", "response_sha256"):
            payload = submission()
            if key == "candidate":
                payload[key] = snapshot(BASELINE)
            elif key == "patch":
                payload[key]["replacement"] += "\n"
            elif key == "check":
                payload[key]["source_sha256"] = "0" * 64
            elif key == "response":
                payload[key] += " unsupported"
            else:
                payload[key] = "0" * 64
            request = parse_request(wire("submit", payload))
            with self.subTest(key=key), self.assertRaises(Invalid):
                checked = phase_payload(request, BASELINE)
                validate_submission(request, checked, CHECKER_SOURCE, CHECK_SOURCE)


class Node:
    """Persistent fake: sibling chains do not share later filesystem mutations."""

    def __init__(self, root, steps=()):
        self.root = root
        self.steps = steps

    def step(self, name, *args, **kwargs):
        return Node(self.root, self.steps + ((name, args, kwargs),))

    def build(self, *args, **kwargs):
        return self.step("build", *args, **kwargs)

    def with_user(self, *args, **kwargs):
        return self.step("user", *args, **kwargs)

    def with_workdir(self, *args, **kwargs):
        return self.step("workdir", *args, **kwargs)

    def with_env_variable(self, *args, **kwargs):
        return self.step("env", *args, **kwargs)

    def with_new_file(self, *args, **kwargs):
        return self.step("new_file", *args, **kwargs)

    def with_file(self, *args, **kwargs):
        return self.step("file", *args, **kwargs)

    def with_exec(self, *args, **kwargs):
        return self.step("exec", *args, **kwargs)

    def file(self, path):
        return (self, path)


class FakeDag:
    def __init__(self):
        self.containers = 0

    def container(self):
        self.containers += 1
        return Node(self.containers)

    def current_module(self):
        return self

    def source(self):
        return self

    def directory(self, path):
        if path != "image":
            raise AssertionError("unexpected source access")
        return ("fixed_module_source", path)


def operations(node, kind):
    return [(args, kwargs) for name, args, kwargs in node.steps if name == kind]


class AsyncFile:
    def __init__(self, raw, size=None):
        self.raw = raw
        self.declared_size = len(raw) if size is None else size
        self.reads = 0

    async def size(self):
        return self.declared_size

    async def contents(self):
        self.reads += 1
        return self.raw.decode("utf-8")


class EvaluatedNode(Node):
    def __init__(self, sdk, root, steps=()):
        super().__init__(root, steps)
        self.sdk = sdk

    def step(self, name, *args, **kwargs):
        return EvaluatedNode(self.sdk, self.root, self.steps + ((name, args, kwargs),))

    def role(self):
        return next(args[1] for args, _ in operations(self, "env")
                    if args[0] == "SMITH_FIXED_ROLE")

    async def exit_code(self):
        self.sdk.evaluated.append(self)
        if self.sdk.evaluation_error:
            raise SDK.QueryError("private engine failure")
        return self.sdk.observations[self.role()][0]

    def file(self, path):
        options = operations(self, "exec")[0][1]
        if path != options["redirect_stdout"]:
            raise AssertionError("unexpected output path")
        output = AsyncFile(self.sdk.observations[self.role()][1])
        self.sdk.outputs.append(output)
        return output


class AsyncFakeDag(FakeDag):
    """Simulated observations only: never runs a command or image build."""

    def __init__(self):
        super().__init__()
        self.assets = {
            "fixtures/clamp/clamp.py": AsyncFile(BASELINE),
            "tests/check_clamp.py": AsyncFile(CHECKER_SOURCE),
            "fixtures/clamp/TASK.md": AsyncFile(TASK),
        }
        self.asset_reads, self.evaluated, self.outputs = [], [], []
        self.evaluation_error = False
        self.observations = {
            "setup-version": (0, b"1.19.3\n"),
            "setup-probe": (0, encode({
                "uid": 1000, "gid": 1000, "python_version": [3, 12, 99],
                "workspace": "/workspace", "workspace_ready": True,
                "baseline_sha256": digest(BASELINE), "task_sha256": digest(TASK),
            }).encode()),
            "setup-checker": (1, encode(CHECK_SOURCE(BASELINE)).encode()),
        }

    def file(self, path):
        self.asset_reads.append(path)
        if path not in self.assets:
            raise SDK.QueryError("private missing module asset")
        return self.assets[path]

    def container(self):
        self.containers += 1
        return EvaluatedNode(self, self.containers)


class GraphTests(unittest.TestCase):
    def graph(self, **changes):
        request = parse_request(wire(**changes))
        return request, build_graph(FakeDag(), request, BASELINE, "fixed task",
                                    CHECKER_SOURCE, "ANY")

    def test_fixed_image_and_checker_separation(self):
        _, graph = self.graph()
        for node in (graph.coding, graph.checker, graph.review):
            self.assertEqual(operations(node, "build"),
                             [((("fixed_module_source", "image"),), {})])
            self.assertEqual(len(operations(node, "exec")), 1)
        coding_files = [args[0] for args, _ in operations(graph.coding, "new_file")]
        self.assertEqual(coding_files, ["/workspace/clamp.py", "/workspace/TASK.md"])
        checker_files = [args[0] for args, _ in operations(graph.checker, "new_file")]
        self.assertEqual(checker_files, ["/trusted/check_clamp.py"])
        checker_input = operations(graph.checker, "file")[0][0]
        self.assertEqual(checker_input[0], "/candidate/clamp.py")
        self.assertIs(checker_input[1][0], graph.coding)
        self.assertEqual(checker_input[1][1], "/workspace/clamp.py")
        self.assertNotIn(CODE_PROMPT, repr(graph.checker.steps))
        check_args, check_options = operations(graph.checker, "exec")[0]
        self.assertEqual(check_options["expect"], "ANY")
        self.assertIn("/trusted/check_clamp.py", check_args[0])
        review_inputs = operations(graph.review, "file")
        self.assertIs(review_inputs[1][0][1][0], graph.checker)
        self.assertNotIn(CODE_PROMPT, repr(graph.review.steps))

    def test_all_effect_nodes_bind_identity_and_cache_inputs(self):
        request, graph = self.graph()
        identities = set()
        for changes in ({}, {"attempt": 2}, {"run_id": "e" * 32},
                        {"input": encode({"baseline": snapshot(BASELINE), "schema_version": 1}) + " "},
                        {"contract_sha256": "f" * 64}, {"module_sha256": "e" * 64}):
            changed, nodes = self.graph(**changes)
            identities.add(changed.identity)
            for node in (nodes.coding, nodes.checker, nodes.review):
                self.assertIn((("SMITH_ATTEMPT_IDENTITY", changed.identity), {}),
                              operations(node, "env"))
                names = [name for name, _, _ in node.steps]
                self.assertLess(names.index("env"), names.index("exec"))
        self.assertEqual(len(identities), 6)
        for node, role in ((graph.coding, "coding"), (graph.checker, "checker"),
                           (graph.review, "review")):
            self.assertIn((("SMITH_FIXED_ROLE", role), {}), operations(node, "env"))
        self.assertIn('"phase":"execute"', request.identity)
        with self.assertRaises(Invalid):
            build_graph(FakeDag(), parse_request(wire("setup")), BASELINE,
                        "task", CHECKER_SOURCE, "ANY")

    def test_clean_default_args_and_no_host_secrets(self):
        for review, prompt in ((False, CODE_PROMPT), (True, REVIEW_PROMPT)):
            command = opencode_command(prompt, review=review)
            self.assertEqual(command[:2], ["/usr/bin/env", "-i"])

            self.assertEqual(command[-5:],
                             ["/usr/local/bin/opencode", "run", "--format", "json", prompt])
            config_arg = next(arg for arg in command if arg.startswith("OPENCODE_CONFIG_CONTENT="))
            config = json.loads(config_arg.split("=", 1)[1])
            self.assertNotIn("model", config)
            self.assertNotIn("provider", config)
            self.assertEqual(config["share"], "disabled")
            self.assertEqual(config["permission"]["*"], "deny")
            self.assertEqual(config["permission"]["edit"], "deny" if review else "allow")
            self.assertNotIn("OPENCODE_DISABLE_MODELS_FETCH", FLAGS)
            self.assertFalse(any(arg.startswith("OPENCODE_DISABLE_MODELS_FETCH=") for arg in command))
            for flag in FLAGS:
                self.assertIn(flag + "=true", command)
            for forbidden in ("--model", "--session", "--continue", "--attach", "--agent"):
                self.assertNotIn(forbidden, command)
            for arg in ENVIRONMENT:
                self.assertNotIn("TOKEN", arg)
                self.assertNotIn("API_KEY", arg)
                self.assertNotIn("PROXY", arg)
        _, graph = self.graph()
        self.assertFalse(any("mount" in name or "secret" in name
                             for node in (graph.coding, graph.checker, graph.review)
                             for name, _, _ in node.steps))

    def test_recipe_pin_and_source_configuration(self):
        root = Path(__file__).absolute().parents[1]
        recipe = (root / "image/Dockerfile").read_text()
        self.assertIn(OPENCODE_ARCHIVE_SHA256, recipe)
        self.assertIn("/v1.19.3/", recipe)
        self.assertNotIn("ARG ", recipe)
        self.assertNotIn("COPY ", recipe)
        self.assertEqual(json.loads((root / "dagger.json").read_text())["engineVersion"], "v0.21.9")
        project = tomllib.loads((root / "pyproject.toml").read_text())
        # Dagger installs its supplied SDK editable; distribution and engine
        # release versions need not match.
        self.assertEqual(project["project"]["dependencies"], ["dagger-io"])


class SetupGraphTests(unittest.TestCase):
    def test_fixed_nonroot_no_model_graph_and_effect_identity(self):
        identities = set()
        for changes in ({}, {"attempt": 2}, {"run_id": "e" * 32},
                        {"contract_sha256": "f" * 64}, {"module_sha256": "e" * 64},
                        {"executable_sha256": "f" * 64},
                        {"input": encode({"schema_version": 1, "baseline": snapshot(BASELINE)}) + " "}):
            request = parse_request(wire("setup", **changes))
            identities.add(request.identity)
            graph = build_setup_graph(FakeDag(), request, BASELINE, TASK, CHECKER_SOURCE, "ANY")
            for node, role in ((graph.version, "setup-version"), (graph.probe, "setup-probe"),
                               (graph.checker, "setup-checker")):
                self.assertEqual(operations(node, "build"),
                                 [((("fixed_module_source", "image"),), {})])
                self.assertEqual(operations(node, "user"), [(("1000:1000",), {})])
                self.assertIn((("SMITH_ATTEMPT_IDENTITY", request.identity), {}), operations(node, "env"))
                self.assertIn((("SMITH_FIXED_ROLE", role), {}), operations(node, "env"))
                names = [name for name, _, _ in node.steps]
                self.assertLess(names.index("env"), names.index("exec"))
                self.assertEqual(len(operations(node, "exec")), 1)
                command, options = operations(node, "exec")[0]
                self.assertEqual(command[0][:2], ["/usr/bin/env", "-i"])
                self.assertEqual(options["expect"], "ANY")
                self.assertNotIn("run", command[0])
                self.assertFalse(any("mount" in name or "secret" in name for name in names))
            version_command = operations(graph.version, "exec")[0][0][0]
            self.assertEqual(version_command[-2:], ["/usr/local/bin/opencode", "--version"])
            self.assertFalse(any(arg.startswith("OPENCODE_DISABLE_MODELS_FETCH=")
                                 for arg in version_command))
            probe_files = operations(graph.probe, "new_file")
            self.assertEqual([args[0] for args, _ in probe_files],
                             ["/workspace/clamp.py", "/workspace/TASK.md"])
            self.assertEqual(probe_files[0][1]["contents"].encode(), BASELINE)
            self.assertEqual(probe_files[0][1]["owner"], "1000:1000")
            self.assertEqual(probe_files[1][1]["contents"].encode(), TASK)
            self.assertIn(SETUP_PROBE, operations(graph.probe, "exec")[0][0][0])
            checker_files = operations(graph.checker, "new_file")
            self.assertEqual([args[0] for args, _ in checker_files],
                             ["/trusted/check_clamp.py", "/candidate/clamp.py"])
            self.assertEqual(checker_files[0][1]["contents"].encode(), CHECKER_SOURCE)
            self.assertEqual(checker_files[1][1]["contents"].encode(), BASELINE)
            self.assertNotIn("/trusted/check_clamp.py", repr(probe_files))
        self.assertEqual(len(identities), 7)
        with self.assertRaises(Invalid):
            build_setup_graph(FakeDag(), parse_request(wire()), BASELINE, TASK, CHECKER_SOURCE, "ANY")


class AsyncExportTests(unittest.IsolatedAsyncioTestCase):
    async def invoke(self, sdk, raw):
        # Installed-package layout cannot accidentally fall back to repository paths.
        with (patch("smith_agent.main.dag", sdk),
              patch("smith_agent.main.__file__", "/not-module-source/site-packages/main.py"),
              patch.object(Path, "read_bytes", side_effect=AssertionError("local asset read")),
              patch.object(Path, "read_text", side_effect=AssertionError("local asset read"))):
            return decode(await SmithAgent().run_fixture(AsyncFile(raw)))

    async def test_setup_reads_sdk_assets_and_validates_simulated_runtime_observations(self):
        sdk = AsyncFakeDag()
        raw = b"  " + wire("setup") + b"\n"
        result = await self.invoke(sdk, raw)
        self.assertEqual(result["outcome"], "produced")
        self.assertEqual(result["request_sha256"], digest(raw))
        body = decode(result["body"])
        self.assertEqual(body["status"], "runtime_ready")
        self.assertTrue(body["runtime_verified"])
        self.assertEqual(body["opencode_version"], "1.19.3")
        self.assertEqual(body["probe"]["python_version"], [3, 12, 99])
        self.assertEqual(body["checker_sha256"], digest(CHECKER_SOURCE))
        self.assertEqual(body["baseline_check"], CHECK_SOURCE(BASELINE))
        self.assertFalse(body["baseline_check"]["passed"])
        self.assertEqual(body["inference"], "policy_denied")
        self.assertEqual(body["semantic_review"], "not_implemented")
        self.assertEqual(sdk.asset_reads, ["fixtures/clamp/clamp.py", "tests/check_clamp.py",
                                           "fixtures/clamp/TASK.md"])
        self.assertEqual([node.role() for node in sdk.evaluated],
                         ["setup-version", "setup-probe", "setup-checker"])
        self.assertTrue(all(file.reads == 1 for file in sdk.assets.values()))

    async def test_setup_requires_version_exit_and_exact_observed_version(self):
        for exit_code, output in ((1, b"1.19.3\n"), (0, b"1.19.2\n"), (0, b""),
                                  (False, b"1.19.3\n"), (0, b"1.19.3 extra\n"),
                                  (0, b"x" * 129)):
            sdk = AsyncFakeDag()
            sdk.observations["setup-version"] = (exit_code, output)
            with self.subTest(exit_code=exit_code, output=output[:30]):
                result = await self.invoke(sdk, wire("setup"))
                self.assertEqual(result["error"], "prerequisite")
                self.assertNotIn("body", result)
                if len(output) > 128:
                    self.assertEqual(sdk.outputs[0].reads, 0)

    async def test_setup_rejects_missing_root_wrong_python_and_workspace_observations(self):
        changes = ({"uid": 0}, {"gid": 0}, {"uid": True}, {"workspace_ready": False},
                   {"workspace_ready": 1}, {"workspace": "/tmp"},
                   {"python_version": [3, 11, 9]}, {"python_version": [3, 12, True]},
                   {"python_version": [3, 12]}, {"baseline_sha256": "0" * 64},
                   {"task_sha256": "0" * 64}, {"extra": True})
        for change in changes:
            sdk = AsyncFakeDag()
            probe = decode(sdk.observations["setup-probe"][1])
            probe.update(change)
            sdk.observations["setup-probe"] = (0, encode(probe).encode())
            with self.subTest(keys=list(change)):
                self.assertEqual((await self.invoke(sdk, wire("setup")))["error"], "prerequisite")
        for exit_code, output in ((1, b"{}"), (0, b"{}"), (0, b"not json"),
                                  (0, b'{"uid":1000,"uid":0}'), (0, b"x" * 4097)):
            sdk = AsyncFakeDag()
            sdk.observations["setup-probe"] = (exit_code, output)
            self.assertEqual((await self.invoke(sdk, wire("setup")))["error"], "prerequisite")

    async def test_setup_requires_actual_expected_baseline_failures_and_checker_exit1(self):
        for exit_code, output in ((0, encode(CHECK_SOURCE(BASELINE)).encode()),
                                  (2, encode(CHECK_SOURCE(BASELINE)).encode()),
                                  (1, b"{}"), (1, encode(CHECK_SOURCE(VALID)).encode()),
                                  (1, b'{"passed":false,"passed":true}'),
                                  (1, b"x" * 16385)):
            sdk = AsyncFakeDag()
            sdk.observations["setup-checker"] = (exit_code, output)
            self.assertEqual((await self.invoke(sdk, wire("setup")))["error"], "prerequisite")
        for change in ({"passed": True}, {"source_sha256": "0" * 64}, {"cases": []}):
            sdk = AsyncFakeDag()
            report = CHECK_SOURCE(BASELINE)
            report.update(change)
            sdk.observations["setup-checker"] = (1, encode(report).encode())
            self.assertEqual((await self.invoke(sdk, wire("setup")))["error"], "prerequisite")

    async def test_missing_assets_and_sdk_failures_return_safe_failure_without_fallback(self):
        for path in ("fixtures/clamp/clamp.py", "tests/check_clamp.py", "fixtures/clamp/TASK.md"):
            sdk = AsyncFakeDag()
            del sdk.assets[path]
            result = await self.invoke(sdk, wire("setup"))
            self.assertEqual(result["error"], "prerequisite")
            self.assertEqual(sdk.containers, 0)
            self.assertNotIn("private", encode(result))
        sdk = AsyncFakeDag()
        sdk.assets["tests/check_clamp.py"] = AsyncFile(b"def invalid syntax")
        self.assertEqual((await self.invoke(sdk, wire("setup")))["error"], "prerequisite")
        self.assertEqual(sdk.containers, 0)
        sdk = AsyncFakeDag()
        sdk.evaluation_error = True
        result = await self.invoke(sdk, wire("setup"))
        self.assertEqual(result["error"], "prerequisite")
        self.assertNotIn("private", encode(result))
        self.assertEqual(len(sdk.evaluated), 1)

    async def test_module_asset_size_bounds_precede_contents_and_graph_construction(self):
        for path, limit in (("fixtures/clamp/clamp.py", 4096),
                            ("tests/check_clamp.py", 32768),
                            ("fixtures/clamp/TASK.md", 4096)):
            sdk = AsyncFakeDag()
            oversized = AsyncFile(b"", size=limit + 1)
            sdk.assets[path] = oversized
            with self.subTest(path=path):
                result = await self.invoke(sdk, wire("setup"))
                self.assertEqual(result["error"], "prerequisite")
                self.assertEqual(oversized.reads, 0)
                self.assertEqual(sdk.containers, 0)

    async def test_execute_and_submit_never_construct_or_evaluate_any_container(self):
        for raw, error in ((wire(), "policy_denied"), (wire(attempt=2), "policy_denied"),
                           (wire("submit", submission()), "prerequisite")):
            sdk = AsyncFakeDag()
            with patch.object(sdk, "container", side_effect=AssertionError("container forbidden")):
                result = await self.invoke(sdk, raw)
            self.assertEqual(result["error"], error)
            self.assertEqual(sdk.evaluated, [])
            self.assertEqual(sdk.asset_reads, ["fixtures/clamp/clamp.py", "tests/check_clamp.py"])

    async def test_invalid_request_or_payload_never_launches_setup(self):
        for raw in (wire("setup", allow_network=True),
                    wire("setup", {"schema_version": 1, "baseline": snapshot(VALID)})):
            sdk = AsyncFakeDag()
            result = await self.invoke(sdk, raw)
            self.assertEqual(result["error"], "prerequisite")
            self.assertEqual(sdk.containers, 0)
        sdk = AsyncFakeDag()
        await self.invoke(sdk, wire("setup", model="injected"))
        self.assertEqual(sdk.asset_reads, [])


class ExportTests(unittest.TestCase):
    def test_setup_is_readiness_and_preserves_actual_baseline_failures(self):
        result = decode(run_source_request(wire("setup")))
        self.assertEqual(result["outcome"], "produced")
        body = decode(result["body"].encode())
        self.assertEqual(body["status"], "source_ready")
        self.assertFalse(body["runtime_verified"])
        self.assertFalse(body["baseline_check"]["passed"])
        self.assertEqual(body["inference"], "policy_denied")
        self.assertTrue(body["baseline_check"]["cases"])

    def test_execute_unconditionally_policy_denied_without_graph(self):
        for attempt in (1, 2):
            with patch("smith_agent.graph.build_graph", side_effect=AssertionError("evaluated")):
                result = decode(run_source_request(wire(attempt=attempt)))
            self.assertEqual(result["outcome"], "failed")
            self.assertEqual(result["error"], "policy_denied")
            self.assertNotIn("body", result)

    def test_submit_checks_bindings_but_never_fabricates_review_or_success(self):
        payload = submission()
        request = parse_request(wire("submit", payload))
        artifact = validate_submission(request, phase_payload(request, BASELINE),
                                       CHECKER_SOURCE, CHECK_SOURCE)
        self.assertTrue(artifact["check"]["passed"])
        result = decode(run_source_request(request.raw))
        self.assertEqual(result["error"], "prerequisite")
        for review in ({"passed": True}, "PASS", None):
            fake = copy.deepcopy(payload)
            fake["review"] = review
            self.assertEqual(decode(run_source_request(wire("submit", fake)))["error"],
                             "prerequisite")

    def test_invalid_requests_return_safe_failure(self):
        for raw in (wire(model="private secret"), b'{"invalid":"private secret"}',
                    wire("submit", submission(BASELINE))):
            result = run_source_request(raw)
            self.assertNotIn("private secret", result)
            self.assertEqual(decode(result)["error"], "prerequisite")
            self.assertEqual(decode(result)["request_sha256"], digest(raw))

    def test_sdk_file_scalar_shape_and_read_bound(self):
        class File:
            def __init__(self, raw, size=None):
                self.raw = raw
                self.declared_size = len(raw) if size is None else size
                self.reads = 0

            async def size(self):
                return self.declared_size

            async def contents(self):
                self.reads += 1
                return self.raw.decode("utf-8")

        file = File(b"  " + wire() + b"\n")
        with patch("smith_agent.main.dag", AsyncFakeDag()):
            scalar = asyncio.run(SmithAgent().run_fixture(file))
        self.assertIsInstance(scalar, str)
        self.assertEqual(decode(scalar)["request_sha256"], digest(file.raw))
        self.assertEqual(file.reads, 1)
        oversized = File(b"", 32769)
        with self.assertRaises(ValueError):
            asyncio.run(SmithAgent().run_fixture(oversized))
        self.assertEqual(oversized.reads, 0)
        with self.assertRaises(ValueError):
            asyncio.run(SmithAgent().run_fixture(File(wire(), 1)))


if __name__ == "__main__":
    unittest.main()
