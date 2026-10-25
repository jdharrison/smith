"""Unwired Dagger entry points; coding requires an explicit opt-in."""

import dagger
from dagger import dag, function, object_type

from .artifacts import phase_payload, validate_submission
from .graph import OPENCODE_VERSION, build_setup_graph
from .protocol import (
    Invalid, MAX_REQUEST, decode, digest, encode, fields, parse_request, response,
    text_bytes,
)


def trusted_inputs(baseline, checker_source):
    """Bind explicit trusted byte snapshots, independent of package installation."""
    if type(baseline) is not bytes or type(checker_source) is not bytes:
        raise Invalid("invalid_trusted_inputs")
    # Compile exactly the trusted module bytes we hash, without a second open.
    # No candidate/request bytes ever enter compile/exec; candidates go only to
    # this trusted checker's restricted AST interpreter.
    namespace = {"__name__": "smith_trusted_clamp"}
    exec(compile(checker_source, "<trusted-clamp>", "exec"), namespace)
    return baseline, checker_source, namespace["check_source"]


def run_source_request(raw, baseline, checker_source):
    """Pure source check with explicit assets, never runtime/environment readiness."""
    try:
        request = parse_request(raw)
        baseline, checker_source, check_source = trusted_inputs(baseline, checker_source)
        payload = phase_payload(request, baseline)
        if request.phase == "execute":
            return response(raw, error="policy_denied")
        if request.phase == "setup":
            return response(raw, body=encode({
                "schema_version": 1, "status": "source_ready",
                "identity": decode(request.identity.encode()),
                "expected_engine_version": "v0.21.9",
                "expected_opencode_version": OPENCODE_VERSION,
                "baseline_check": check_source(baseline),
                "inference": "policy_denied", "runtime_verified": False,
                "semantic_review": "not_implemented",
            }))
        validate_submission(request, payload, checker_source, check_source)
        # Supplied reviews cannot establish that semantic review was executed.
        return response(raw, error="prerequisite")
    except (Invalid, SyntaxError):
        return response(raw, error="prerequisite")


async def read_file(file, limit):
    """Bound a UTF-8 SDK File before reading, preserving its exact bytes."""
    size = await file.size()
    if type(size) is not int or not 0 <= size <= limit:
        raise Invalid("file_size")
    raw = text_bytes(await file.contents(), limit)
    if len(raw) != size:
        raise Invalid("file_encoding")
    return raw


def setup_observations(request, baseline, task, checker_source, check_source,
                       version_exit, version_raw, probe_exit, probe_raw,
                       checker_exit, checker_raw):
    """Require actual command observations; constants alone cannot establish PASS."""
    if (type(version_exit) is not int or version_exit != 0
            or version_raw.strip() != OPENCODE_VERSION.encode()
            or type(probe_exit) is not int or probe_exit != 0
            or type(checker_exit) is not int or checker_exit != 1):
        raise Invalid("setup_command_failed")
    probe = fields(decode(probe_raw), (
        "uid", "gid", "python_version", "workspace", "workspace_ready",
        "baseline_sha256", "task_sha256",
    ))
    version = probe["python_version"]
    if (type(version) is not list or len(version) != 3
            or any(type(part) is not int or part < 0 for part in version)
            or version[:2] != [3, 12]
            or type(probe["uid"]) is not int or probe["uid"] != 1000
            or type(probe["gid"]) is not int or probe["gid"] != 1000
            or probe["workspace"] != "/workspace" or probe["workspace_ready"] is not True
            or probe["baseline_sha256"] != digest(baseline)
            or probe["task_sha256"] != digest(task)):
        raise Invalid("setup_probe_failed")
    expected = check_source(baseline)
    observed = decode(checker_raw)
    if (expected["passed"] is not False or not expected["cases"]
            or expected["error"] is not None or encode(observed) != encode(expected)):
        raise Invalid("baseline_check_mismatch")
    return {
        "schema_version": 1, "status": "runtime_ready",
        "identity": decode(request.identity.encode()),
        "expected_engine_version": "v0.21.9",
        "opencode_version": version_raw.strip().decode("ascii"),
        "probe": probe, "checker_sha256": digest(checker_source),
        "baseline_check": observed, "runtime_verified": True,
        "inference": "policy_denied", "semantic_review": "not_implemented",
    }


async def evaluate_setup(request, baseline, task, checker_source, check_source):
    graph = build_setup_graph(dag, request, baseline, task, checker_source,
                              dagger.ReturnType.ANY)
    version_exit = await graph.version.exit_code()
    version_raw = await read_file(graph.version.file("/private/version.txt"), 128)
    probe_exit = await graph.probe.exit_code()
    probe_raw = await read_file(graph.probe.file("/private/probe.json"), 4096)
    checker_exit = await graph.checker.exit_code()
    checker_raw = await read_file(graph.checker.file("/private/baseline.json"), 16384)
    return setup_observations(
        request, baseline, task, checker_source, check_source,
        version_exit, version_raw, probe_exit, probe_raw, checker_exit, checker_raw,
    )


@object_type
class SmithAgent:
    @function
    async def run_coding_fixture(
        self, request: dagger.File, enable_coding: bool = False,
    ) -> "dagger.Directory":
        """Run the baked coding fixture and return validated result artifacts.

        This opt-in does not grant engine, download or provider-call authority.
        The host must separately authorize and supervise the complete live run.
        """
        from .coding import run_coding_fixture as evaluate
        raw = await read_file(request, MAX_REQUEST) if enable_coding else b""
        return await evaluate(dag, raw, enabled=enable_coding,
                              any_exit=dagger.ReturnType.ANY)

    @function
    async def run_fixture(self, request: dagger.File) -> str:
        """Read schema1 File and return a JSON scalar; no inference is enabled.

        Setup evaluates only fixed version/Python/workspace/baseline commands.
        Live SDK/CLI/container acceptance still requires separate authorization.
        """
        # File read failures cannot fabricate a hash of unread original bytes.
        raw = await read_file(request, MAX_REQUEST)
        try:
            parsed = parse_request(raw)
            source = dag.current_module().source()
            baseline = await read_file(source.file("fixtures/clamp/clamp.py"), 4096)
            checker_source = await read_file(source.file("tests/check_clamp.py"), 32768)
            if parsed.phase != "setup":
                # No image/exec/inference graph construction on Execute or Submit.
                return run_source_request(raw, baseline, checker_source)
            baseline, checker_source, check_source = trusted_inputs(baseline, checker_source)
            phase_payload(parsed, baseline)
            task = await read_file(source.file("fixtures/clamp/TASK.md"), 4096)
            body = await evaluate_setup(parsed, baseline, task, checker_source, check_source)
            return response(raw, body=encode(body))
        except (Invalid, SyntaxError, dagger.QueryError):
            # Never include raw engine/image/runtime diagnostics or retry effects.
            return response(raw, error="prerequisite")
