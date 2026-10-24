"""Exact-byte fixture artifacts; checker reports are recomputed, never trusted."""

from .protocol import (
    Invalid, decode, digest, encode, fields, integer, sha256, text_bytes,
)

MAX_SOURCE = 4096


def snapshot(source):
    if type(source) is not bytes or len(source) > MAX_SOURCE:
        raise Invalid("invalid_snapshot")
    try:
        content = source.decode("utf-8", errors="strict")
    except UnicodeError:
        raise Invalid("invalid_snapshot") from None
    text_bytes(content, MAX_SOURCE)
    return {"path": "clamp.py", "content": content, "sha256": digest(source)}


def read_snapshot(value):
    fields(value, ("path", "content", "sha256"))
    if value["path"] != "clamp.py":
        raise Invalid("invalid_path")
    raw = text_bytes(value["content"], MAX_SOURCE)
    if sha256(value["sha256"]) != digest(raw):
        raise Invalid("snapshot_mismatch")
    return raw


def patch_for(baseline, candidate):
    before, after = snapshot(baseline), snapshot(candidate)
    return {"format": "smith.replace.v1", "path": "clamp.py",
            "base_sha256": before["sha256"], "candidate_sha256": after["sha256"],
            "replacement": after["content"]}


def reconstruct(baseline, patch):
    fields(patch, ("format", "path", "base_sha256", "candidate_sha256", "replacement"))
    if patch["format"] != "smith.replace.v1" or patch["path"] != "clamp.py":
        raise Invalid("invalid_patch")
    if sha256(patch["base_sha256"]) != digest(baseline):
        raise Invalid("patch_mismatch")
    candidate = text_bytes(patch["replacement"], MAX_SOURCE)
    if sha256(patch["candidate_sha256"]) != digest(candidate):
        raise Invalid("patch_mismatch")
    return candidate


def checked_artifact(request, baseline, candidate, checker_source, check_source):
    # check_source is the trusted function selected by the module, not request data.
    patch = patch_for(baseline, candidate)
    if reconstruct(baseline, patch) != candidate:
        raise Invalid("patch_mismatch")
    report = check_source(candidate)
    if report["source_sha256"] != digest(candidate):
        raise Invalid("checker_mismatch")
    return {
        "identity": decode(request.identity.encode()),
        "baseline_sha256": digest(baseline), "candidate_sha256": digest(candidate),
        "patch": patch, "patch_sha256": digest(encode(patch).encode()),
        "checker_sha256": digest(checker_source), "check": report,
    }


def phase_payload(request, baked_baseline):
    value = decode(text_bytes(request.input), limit=16 * 1024)
    common = ("schema_version", "baseline")
    names = common if request.phase != "submit" else common + (
        "candidate", "patch", "patch_sha256", "checker_sha256", "check",
        "response", "response_sha256", "execute_identity",
    )
    fields(value, names)
    integer(value["schema_version"], {1})
    if read_snapshot(value["baseline"]) != baked_baseline:
        raise Invalid("baseline_mismatch")
    if request.phase == "submit":
        identity = fields(value["execute_identity"], (
            "run_id", "phase", "attempt", "request_sha256", "contract_sha256",
            "executable_sha256", "module_sha256",
        ))
        for key in ("run_id", "contract_sha256", "executable_sha256", "module_sha256"):
            if identity[key] != getattr(request, key):
                raise Invalid("execute_binding_mismatch")
        if identity["phase"] != "execute":
            raise Invalid("execute_binding_mismatch")
        integer(identity["attempt"], {1, 2})
        sha256(identity["request_sha256"])
        candidate = read_snapshot(value["candidate"])
        if reconstruct(baked_baseline, value["patch"]) != candidate:
            raise Invalid("patch_mismatch")
        for key in ("patch_sha256", "checker_sha256", "response_sha256"):
            sha256(value[key])
        if value["patch_sha256"] != digest(encode(value["patch"]).encode()):
            raise Invalid("patch_mismatch")
        response_bytes = text_bytes(value["response"])
        if not response_bytes.strip() or value["response_sha256"] != digest(response_bytes):
            raise Invalid("response_mismatch")
    return value


def validate_submission(request, payload, checker_source, check_source):
    """Deterministic binding only; this deliberately does NOT implement model review."""
    baseline = read_snapshot(payload["baseline"])
    candidate = read_snapshot(payload["candidate"])
    artifact = checked_artifact(request, baseline, candidate, checker_source, check_source)
    if (payload["checker_sha256"] != artifact["checker_sha256"]
            or encode(payload["check"]) != encode(artifact["check"])):
        raise Invalid("checker_mismatch")
    if not artifact["check"]["passed"]:
        raise Invalid("check_failed")
    # A supplied prior identity is not proof that Execute ran or was accepted.
    # The host must bind it to its retained attempt; Submit still fails closed.
    artifact["execute_identity"] = payload["execute_identity"]
    artifact["response_sha256"] = payload["response_sha256"]
    return artifact
