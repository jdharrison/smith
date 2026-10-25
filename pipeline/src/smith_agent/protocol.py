"""Bounded wire codec. No SDK, filesystem, process or provider dependencies."""

import hashlib
import json
import re
from dataclasses import dataclass

MAX_INPUT = 16 * 1024
MAX_REQUEST = 32 * 1024
MAX_RESPONSE = 64 * 1024
ERRORS = frozenset({
    "authentication", "default_unavailable", "quota", "provider",
    "policy_denied", "prerequisite",
})


class Invalid(ValueError):
    """Fixed safe diagnostic; never includes untrusted input."""


def digest(data):
    return hashlib.sha256(data).hexdigest()


def text_bytes(value, limit=MAX_INPUT):
    if type(value) is not str or "\0" in value:
        raise Invalid("invalid_text")
    try:
        raw = value.encode("utf-8", errors="strict")
    except UnicodeError:
        raise Invalid("invalid_text") from None
    if len(raw) > limit:
        raise Invalid("invalid_text")
    return raw


def fields(value, names):
    if type(value) is not dict or set(value) != set(names):
        raise Invalid("invalid_fields")
    return value


def integer(value, allowed):
    if type(value) is not int or value not in allowed:
        raise Invalid("invalid_integer")
    return value


def sha256(value):
    if type(value) is not str or re.fullmatch(r"[0-9a-f]{64}", value) is None:
        raise Invalid("invalid_digest")
    return value


def _unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise Invalid("duplicate_key")
        result[key] = value
    return result


def _constant(_value):
    raise Invalid("invalid_number")


def decode(raw, limit=MAX_REQUEST):
    if type(raw) is not bytes or len(raw) > limit:
        raise Invalid("invalid_json")
    try:
        return json.loads(raw.decode("utf-8", errors="strict"),
                          object_pairs_hook=_unique, parse_constant=_constant)
    except (UnicodeError, ValueError, RecursionError):
        raise Invalid("invalid_json") from None


def encode(value):
    return json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":"))


@dataclass(frozen=True)
class Request:
    raw: bytes
    run_id: str
    phase: str
    attempt: int
    contract_sha256: str
    executable_sha256: str
    module_sha256: str
    input: str

    @property
    def request_sha256(self):
        return digest(self.raw)

    @property
    def identity(self):
        # Fixed JSON, not model text. Applied before every effectful graph node.
        return encode({
            "run_id": self.run_id, "phase": self.phase, "attempt": self.attempt,
            "request_sha256": self.request_sha256,
            "contract_sha256": self.contract_sha256,
            "executable_sha256": self.executable_sha256,
            "module_sha256": self.module_sha256,
        })


def parse_request(raw):
    value = fields(decode(raw), (
        "schema_version", "run_id", "phase", "attempt", "contract_sha256",
        "executable_sha256", "module_sha256", "input",
    ))
    integer(value["schema_version"], {1})
    run_id = value["run_id"]
    if type(run_id) is not str or re.fullmatch(r"[0-9a-f]{32}", run_id) is None:
        raise Invalid("invalid_run")
    if type(value["phase"]) is not str or value["phase"] not in {"setup", "execute", "submit"}:
        raise Invalid("invalid_phase")
    integer(value["attempt"], {1, 2})
    for key in ("contract_sha256", "executable_sha256", "module_sha256"):
        sha256(value[key])
    text_bytes(value["input"])
    return Request(raw, **{key: value[key] for key in value if key != "schema_version"})


def response(raw, *, body=None, error=None):
    if (body is None) == (error is None):
        raise Invalid("invalid_response")
    result = {"schema_version": 1, "request_sha256": digest(raw)}
    if error is not None:
        if error not in ERRORS:
            raise Invalid("invalid_error")
        result.update(outcome="failed", error=error)
    else:
        text_bytes(body)
        if not body.strip():
            raise Invalid("invalid_response")
        result.update(outcome="produced", body=body)
    encoded = encode(result)
    if len(encoded.encode("utf-8")) > MAX_RESPONSE:
        raise Invalid("response_size")
    return encoded
