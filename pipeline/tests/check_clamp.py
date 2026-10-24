"""Trusted, bounded clamp-fixture interpreter. Not a general Python sandbox."""

import ast
import hashlib
import json
import math
import os
import stat
import sys

CHECK_ID = "smith.clamp.restricted-ast.v1"
MAX_SOURCE_BYTES = 4096
MAX_AST_NODES = 128
MAX_AST_DEPTH = 16
MAX_NUMBER = 1_000_000
PARAMETERS = ("value", "low", "high")
# name, inputs, expected numeric result (None means ValueError)
CASES = (
    ("below_lower", (-1, 0, 10), 0),
    ("at_lower", (0, 0, 10), 0),
    ("inside", (4, 0, 10), 4),
    ("at_upper", (10, 0, 10), 10),
    ("above_upper", (11, 0, 10), 10),
    ("negative_below", (-20, -10, -2), -10),
    ("negative_inside", (-5, -10, -2), -5),
    ("fractional_above", (2.5, -0.5, 1.5), 1.5),
    ("equal_bounds_below", (-1, 3, 3), 3),
    ("equal_bounds_at", (3, 3, 3), 3),
    ("equal_bounds_above", (8, 3, 3), 3),
    ("reversed_below", (-5, 10, 0), None),
    ("reversed_between", (5, 10, 0), None),
    ("reversed_above", (15, 10, 0), None),
)


class Rejected(ValueError):
    """A fixed, non-candidate-derived validation diagnostic."""


def _expression(node):
    if isinstance(node, ast.Name) and node.id in PARAMETERS:
        return
    if isinstance(node, ast.Constant):
        if (type(node.value) in (int, float)
                and abs(node.value) <= MAX_NUMBER and math.isfinite(node.value)):
            return
    elif isinstance(node, ast.UnaryOp) and isinstance(node.op, (ast.UAdd, ast.USub, ast.Not)):
        _expression(node.operand)
        return
    elif isinstance(node, ast.BoolOp) and isinstance(node.op, (ast.And, ast.Or)):
        for value in node.values:
            _expression(value)
        return
    elif isinstance(node, ast.Compare) and all(
        isinstance(op, (ast.Lt, ast.LtE, ast.Gt, ast.GtE, ast.Eq, ast.NotEq))
        for op in node.ops
    ):
        for value in (node.left, *node.comparators):
            _expression(value)
        return
    elif isinstance(node, ast.IfExp):
        for value in (node.test, node.body, node.orelse):
            _expression(value)
        return
    raise Rejected("forbidden_source")


def _statements(statements):
    for node in statements:
        if isinstance(node, ast.Return) and node.value is not None:
            _expression(node.value)
        elif isinstance(node, ast.If):
            _expression(node.test)
            _statements(node.body)
            _statements(node.orelse)
        elif isinstance(node, ast.Raise) and node.cause is None:
            call = node.exc
            if not (isinstance(call, ast.Call) and isinstance(call.func, ast.Name)
                    and call.func.id == "ValueError" and not call.args and not call.keywords):
                raise Rejected("forbidden_source")
        else:
            raise Rejected("forbidden_source")


def _validate(text):
    try:
        tree = ast.parse(text, mode="exec")
    except (SyntaxError, ValueError, RecursionError):
        raise Rejected("invalid_syntax") from None
    pending = [(tree, 1)]
    count = 0
    while pending:
        node, depth = pending.pop()
        count += 1
        if count > MAX_AST_NODES or depth > MAX_AST_DEPTH:
            raise Rejected("ast_limit")
        pending.extend((child, depth + 1) for child in ast.iter_child_nodes(node))
    if len(tree.body) != 1 or not isinstance(tree.body[0], ast.FunctionDef):
        raise Rejected("forbidden_source")
    function = tree.body[0]
    args = function.args
    if (function.name != "clamp" or function.decorator_list or function.returns
            or function.type_comment or getattr(function, "type_params", ())
            or args.posonlyargs or args.kwonlyargs or args.vararg or args.kwarg
            or args.defaults or args.kw_defaults
            or tuple(arg.arg for arg in args.args) != PARAMETERS
            or any(arg.annotation or arg.type_comment for arg in args.args)):
        raise Rejected("forbidden_source")
    _statements(function.body)
    return function.body


def _value(node, values):
    if isinstance(node, ast.Name):
        return values[node.id]
    if isinstance(node, ast.Constant):
        return node.value
    if isinstance(node, ast.UnaryOp):
        value = _value(node.operand, values)
        if isinstance(node.op, ast.Not):
            return not value
        return +value if isinstance(node.op, ast.UAdd) else -value
    if isinstance(node, ast.BoolOp):
        for item in node.values:
            value = _value(item, values)
            if isinstance(node.op, ast.And) and not value:
                return value
            if isinstance(node.op, ast.Or) and value:
                return value
        return value
    if isinstance(node, ast.IfExp):
        return _value(node.body if _value(node.test, values) else node.orelse, values)
    left = _value(node.left, values)
    for op, item in zip(node.ops, node.comparators):
        right = _value(item, values)
        if isinstance(op, ast.Lt):
            passed = left < right
        elif isinstance(op, ast.LtE):
            passed = left <= right
        elif isinstance(op, ast.Gt):
            passed = left > right
        elif isinstance(op, ast.GtE):
            passed = left >= right
        elif isinstance(op, ast.Eq):
            passed = left == right
        else:
            passed = left != right
        if not passed:
            return False
        left = right
    return True


def _run(statements, values):
    for node in statements:
        if isinstance(node, ast.Return):
            return True, _value(node.value, values)
        if isinstance(node, ast.Raise):
            raise ValueError
        branch = node.body if _value(node.test, values) else node.orelse
        returned, value = _run(branch, values)
        if returned:
            return True, value
    return False, None


def _result(source_digest=None, error=None, cases=()):
    return {
        "check_id": CHECK_ID,
        "source_sha256": source_digest,
        "passed": error is None and bool(cases) and all(case["passed"] for case in cases),
        "error": error,
        "cases": list(cases),
    }


def check_source(source: bytes):
    """Pure actual evaluation of bounded source bytes, never reported evidence."""
    if type(source) is not bytes:
        return _result(error="bytes_required")
    if len(source) > MAX_SOURCE_BYTES:
        return _result(error="source_too_large")
    digest = hashlib.sha256(source).hexdigest()
    try:
        text = source.decode("utf-8", errors="strict")
    except UnicodeDecodeError:
        return _result(digest, "invalid_utf8")
    try:
        statements = _validate(text)
    except Rejected as error:
        return _result(digest, str(error))
    cases = []
    for name, inputs, expected in CASES:
        try:
            _, actual = _run(statements, dict(zip(PARAMETERS, inputs)))
            outcome = "number" if type(actual) in (int, float) else "invalid_return"
            passed = expected is not None and outcome == "number" and actual == expected
        except ValueError:
            outcome, actual = "ValueError", None
            passed = expected is None
        cases.append({
            "name": name, "passed": passed, "outcome": outcome,
            "actual": actual, "expected": expected,
        })
    return _result(digest, cases=cases)


def _read_candidate(path):
    # Walk directory descriptors without resolve(): symlink ancestors must fail too.
    if type(path) is not str or not path or "\x00" in path:
        raise Rejected("invalid_path")
    parts = path.split("/")
    if any(part in (".", "..") for part in parts) or parts[-1] != "clamp.py":
        raise Rejected("invalid_path")
    parts = [part for part in parts if part]
    directory = os.open("/" if path.startswith("/") else ".",
                        os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        for part in parts[:-1]:
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                            dir_fd=directory)
            os.close(directory)
            directory = child
        entry = os.stat(parts[-1], dir_fd=directory, follow_symlinks=False)
        if stat.S_ISLNK(entry.st_mode):
            raise Rejected("path_denied")
        if not stat.S_ISREG(entry.st_mode):
            raise Rejected("regular_file_required")
        descriptor = os.open(parts[-1], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK,
                             dir_fd=directory)
        try:
            before = os.fstat(descriptor)
            if not stat.S_ISREG(before.st_mode):
                raise Rejected("regular_file_required")
            if before.st_size > MAX_SOURCE_BYTES:
                raise Rejected("source_too_large")
            source = bytearray()
            while len(source) <= MAX_SOURCE_BYTES:
                chunk = os.read(descriptor, MAX_SOURCE_BYTES + 1 - len(source))
                if not chunk:
                    break
                source.extend(chunk)
            if len(source) > MAX_SOURCE_BYTES:
                raise Rejected("source_too_large")
            after = os.fstat(descriptor)
            if (before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (
                    after.st_size, after.st_mtime_ns, after.st_ctime_ns):
                raise Rejected("candidate_changed")
            return bytes(source)
        finally:
            os.close(descriptor)
    finally:
        os.close(directory)


def check_path(path):
    """Ubuntu-only no-follow, bounded, single-clamp.py command boundary."""
    try:
        return check_source(_read_candidate(path))
    except Rejected as error:
        return _result(error=str(error))
    except OSError:
        return _result(error="path_denied")


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    result = check_path(argv[0]) if len(argv) == 1 else _result(error="one_candidate_path_required")
    print(json.dumps(result, sort_keys=True, separators=(",", ":"), allow_nan=False))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
