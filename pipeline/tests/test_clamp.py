"""Trusted fixture tests; run a copied pipeline tree from an isolated temp root."""

import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import check_clamp as checker

FIXTURE = Path(__file__).parent.parent / "fixtures" / "clamp" / "clamp.py"
FIXED = b"""def clamp(value, low, high):
    if low > high:
        raise ValueError()
    if value < low:
        return low
    if value > high:
        return high
    return value
"""


class SourceTests(unittest.TestCase):
    def test_baseline_fails_expected_cases(self):
        result = checker.check_source(FIXTURE.read_bytes())
        self.assertIsNone(result["error"])
        self.assertFalse(result["passed"])
        self.assertEqual(
            {case["name"] for case in result["cases"] if not case["passed"]},
            {"below_lower", "above_upper", "negative_below", "fractional_above",
             "reversed_below",
             "reversed_between", "reversed_above"},
        )

    def test_fixed_candidate_passes_actual_named_cases(self):
        result = checker.check_source(FIXED)
        self.assertTrue(result["passed"])
        names = [case["name"] for case in result["cases"]]
        self.assertEqual(names, [case[0] for case in checker.CASES])
        self.assertEqual(len(names), len(set(names)))
        self.assertGreaterEqual(len(names), 8)
        self.assertTrue(all(case["passed"] for case in result["cases"]))

    def test_reversed_bounds_must_raise_not_return(self):
        source = FIXED.replace(b"raise ValueError()", b"return low")
        failed = [case["name"] for case in checker.check_source(source)["cases"]
                  if not case["passed"]]
        self.assertEqual(failed, ["reversed_below", "reversed_between", "reversed_above"])

    def test_unconditional_exception_is_not_success(self):
        result = checker.check_source(b"def clamp(value, low, high):\n    raise ValueError()\n")
        self.assertFalse(result["passed"])
        self.assertEqual(sum(case["passed"] for case in result["cases"]), 3)

    def test_invalid_returns(self):
        for expression in ("True", "None", "'passed'", "[]"):
            with self.subTest(expression=expression):
                result = checker.check_source(
                    f"def clamp(value, low, high):\n    return {expression}\n".encode())
                self.assertEqual(result["error"], "forbidden_source")
        for body in ("return value == low", "if value < low:\n        return low"):
            with self.subTest(body=body):
                result = checker.check_source(f"def clamp(value, low, high):\n    {body}\n".encode())
                self.assertFalse(result["passed"])
                self.assertTrue(any(case["outcome"] == "invalid_return" for case in result["cases"]))

    def test_forbidden_source_never_executes(self):
        payloads = (
            "import os\n" + FIXED.decode(),
            "exit()\n" + FIXED.decode(),
            "raise SystemExit()\n" + FIXED.decode(),
            "def clamp(value, low, high):\n    return __import__('os').system('exit')\n",
            "def clamp(value, low, high):\n    exit()\n",
            "def clamp(value, low, high):\n    return value.__class__\n",
            "def clamp(value, low, high):\n    while True:\n        pass\n",
            "def clamp(value, low, high):\n    return clamp(value, low, high)\n",
            "def clamp(value, low, high):\n    global CASES\n    CASES = []\n    return value\n",
            "def clamp(value, low, high):\n    open('test_clamp.py', 'w').write('PASS')\n",
            "def clamp(value, low, high):\n    return 2 ** 999999\n",
            "@exit()\n" + FIXED.decode(),
            FIXED.decode().replace("value, low, high", "value=exit(), low=0, high=1"),
            FIXED.decode().replace("value, low, high", "value: exit(), low, high"),
            FIXED.decode() + "def test_pass():\n    return True\n",
            "def clamp(value, low, high):\n    raise ValueError('candidate text')\n",
            "def clamp(value, low, high):\n    assert False\n",
            "def clamp(value, low, high):\n    return [x for x in value]\n",
        )
        original_cases = checker.CASES
        with mock.patch("builtins.exec", side_effect=AssertionError("exec forbidden")), \
                mock.patch("builtins.eval", side_effect=AssertionError("eval forbidden")), \
                mock.patch("builtins.exit", side_effect=AssertionError("exit forbidden")):
            for source in payloads:
                with self.subTest(source=source[:100]):
                    result = checker.check_source(source.encode())
                    self.assertFalse(result["passed"])
                    self.assertEqual(result["cases"], [])
                    self.assertIsNotNone(result["error"])
        self.assertIs(checker.CASES, original_cases)

    def test_signature_is_exact(self):
        for args in ("value, low, high, extra", "value, low, high=0", "value, low, /, high",
                     "value, low, *, high", "*args", "value, low, high: int"):
            with self.subTest(args=args):
                source = f"def clamp({args}):\n    return 0\n".encode()
                self.assertEqual(checker.check_source(source)["error"], "forbidden_source")

    def test_oversized_bytes_denied_before_parse(self):
        with mock.patch.object(checker.ast, "parse", side_effect=AssertionError("must not parse")):
            result = checker.check_source(b" " * (checker.MAX_SOURCE_BYTES + 1))
        self.assertEqual(result["error"], "source_too_large")
        self.assertIsNone(result["source_sha256"])

    def test_exact_byte_bound_is_accepted(self):
        source = FIXED + b" " * (checker.MAX_SOURCE_BYTES - len(FIXED))
        self.assertTrue(checker.check_source(source)["passed"])

    def test_invalid_encoding(self):
        result = checker.check_source(b"\xff\xfe")
        self.assertEqual(result["error"], "invalid_utf8")
        self.assertEqual(result["source_sha256"], hashlib.sha256(b"\xff\xfe").hexdigest())

    def test_bytes_only(self):
        for source in (FIXED.decode(), bytearray(FIXED), None):
            self.assertEqual(checker.check_source(source)["error"], "bytes_required")

    def test_syntax_errors(self):
        for source in (b"def", b"\x00", b"", b"(" * 300):
            self.assertFalse(checker.check_source(source)["passed"])

    def test_numeric_complexity(self):
        for number in ("1000001", "1e999", "9" * 2000, "0x" + "f" * 1000):
            with self.subTest(number=number[:40]):
                result = checker.check_source(f"def clamp(value, low, high):\n    return {number}\n".encode())
                self.assertEqual(result["error"], "forbidden_source")

    def test_ast_node_and_depth_limits(self):
        for expression in (" or ".join(["value"] * 100), "not " * 20 + "value"):
            result = checker.check_source(f"def clamp(value, low, high):\n    return {expression}\n".encode())
            self.assertEqual(result["error"], "ast_limit")

    def test_allowed_expression_forms(self):
        source = b"""def clamp(value, low, high):
    if not low <= high:
        raise ValueError()
    return +low if value < low or value == low else (high if value >= high and low <= high else -(-value))
"""
        self.assertTrue(checker.check_source(source)["passed"])
        chained = FIXED.replace(b"if value < low:", b"if value < low <= high:")
        self.assertTrue(checker.check_source(chained)["passed"])

    def test_unary_sign_converts_comparison_in_correct_candidate(self):
        for expression in (b"+(value != value)", b"-(value != value)"):
            with self.subTest(expression=expression):
                source = FIXED.replace(
                    b"return low", b"return " + expression + b" if low == 0 else low")
                result = checker.check_source(source)
                self.assertIsNone(result["error"])
                self.assertTrue(result["passed"])
                self.assertTrue(all(case["passed"] for case in result["cases"]))
                below = next(case for case in result["cases"] if case["name"] == "below_lower")
                self.assertEqual(below["actual"], 0)
                self.assertIs(type(below["actual"]), int)

    def test_unconverted_comparison_is_not_numeric_return(self):
        source = FIXED.replace(
            b"return low", b"return (value != value) if low == 0 else low")
        result = checker.check_source(source)
        self.assertIsNone(result["error"])
        self.assertFalse(result["passed"])
        failed = [case for case in result["cases"] if not case["passed"]]
        self.assertEqual([case["name"] for case in failed], ["below_lower"])
        self.assertEqual(failed[0]["outcome"], "invalid_return")
        self.assertIs(failed[0]["actual"], False)

    def test_hashes_bind_exact_bytes_not_claims(self):
        first = checker.check_source(FIXED)
        second = checker.check_source(FIXED + b"# passed=false\n")
        self.assertTrue(first["passed"] and second["passed"])
        self.assertEqual(first["check_id"], checker.CHECK_ID)
        self.assertEqual(first["source_sha256"], hashlib.sha256(FIXED).hexdigest())
        self.assertNotEqual(first["source_sha256"], second["source_sha256"])
        self.assertEqual(first, checker.check_source(FIXED))
        fake_report = b'{"passed":true,"check_id":"smith.clamp.restricted-ast.v1"}'
        self.assertFalse(checker.check_source(fake_report)["passed"])
        # Caller edits to one response cannot change the next actual evaluation.
        first["cases"][0]["passed"] = False
        self.assertTrue(checker.check_source(FIXED)["passed"])


class PathTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="clamp-path-", dir=Path.cwd())
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.candidate = self.root / "clamp.py"

    def check(self, path=None):
        return checker.check_path(str(self.candidate if path is None else path))

    def test_regular_file_matches_pure_check(self):
        self.candidate.write_bytes(FIXED)
        self.assertEqual(self.check(), checker.check_source(FIXED))

    def test_candidate_supplied_tests_are_never_read(self):
        self.candidate.write_bytes(FIXED)
        (self.root / "test_clamp.py").write_text("raise SystemExit('tamper')\n")
        (self.root / "check_clamp.py").write_text("raise SystemExit('tamper')\n")
        (self.root / "evidence.json").write_text('{"passed":false}')
        self.assertTrue(self.check()["passed"])
        self.assertFalse(self.check(self.root / "test_clamp.py")["passed"])

    def test_leaf_symlink_denied(self):
        target = self.root / "target.py"
        target.write_bytes(FIXED)
        self.candidate.symlink_to(target)
        self.assertEqual(self.check()["error"], "path_denied")

    def test_ancestor_symlink_denied(self):
        directory = self.root / "real"
        directory.mkdir()
        (directory / "clamp.py").write_bytes(FIXED)
        link = self.root / "link"
        link.symlink_to(directory, target_is_directory=True)
        self.assertEqual(self.check(link / "clamp.py")["error"], "path_denied")

    def test_directory_denied(self):
        self.candidate.mkdir()
        self.assertFalse(self.check()["passed"])

    def test_fifo_denied_without_blocking(self):
        os.mkfifo(self.candidate)
        with mock.patch.object(checker.os, "open", wraps=os.open) as opened:
            self.assertEqual(self.check()["error"], "regular_file_required")
        self.assertNotIn("clamp.py", [call.args[0] for call in opened.call_args_list])

    def test_traversal_and_wrong_names_denied(self):
        self.candidate.write_bytes(FIXED)
        for path in (str(self.root) + "/../" + self.root.name + "/clamp.py",
                     str(self.root) + "/./clamp.py", str(self.candidate) + "/",
                     str(self.root / "other.py"), "", "\x00"):
            with self.subTest(path=path):
                self.assertEqual(self.check(path)["error"], "invalid_path")

    def test_missing_path_denied(self):
        self.assertEqual(self.check()["error"], "path_denied")

    def test_oversized_file_denied(self):
        self.candidate.write_bytes(b" " * (checker.MAX_SOURCE_BYTES + 1))
        self.assertEqual(self.check()["error"], "source_too_large")

    def test_invalid_utf8_file_denied(self):
        self.candidate.write_bytes(b"\xff")
        self.assertEqual(self.check()["error"], "invalid_utf8")

    def test_growth_during_read_is_bounded(self):
        self.candidate.write_bytes(FIXED)
        with mock.patch.object(checker.os, "read", return_value=b" " * (checker.MAX_SOURCE_BYTES + 1)):
            self.assertEqual(self.check()["error"], "source_too_large")

    def test_file_change_denied(self):
        self.candidate.write_bytes(FIXED)
        original = os.fstat
        calls = []

        def changed(descriptor):
            info = original(descriptor)
            calls.append(descriptor)
            if len(calls) == 2:
                return type("Changed", (), {"st_size": info.st_size + 1,
                                           "st_mtime_ns": info.st_mtime_ns,
                                           "st_ctime_ns": info.st_ctime_ns})()
            return info

        with mock.patch.object(checker.os, "fstat", side_effect=changed):
            self.assertEqual(self.check()["error"], "candidate_changed")

    def test_command_wrapper_actual_evaluation_and_exit_codes(self):
        for source, expected_exit in ((FIXED, 0), (FIXTURE.read_bytes(), 1)):
            self.candidate.write_bytes(source)
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                exit_code = checker.main([str(self.candidate)])
            self.assertEqual(exit_code, expected_exit)
            self.assertEqual(json.loads(output.getvalue()), checker.check_source(source))

    def test_command_requires_exactly_one_path(self):
        for arguments in ([], ["clamp.py", "test_clamp.py"]):
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                self.assertEqual(checker.main(arguments), 1)
            self.assertEqual(json.loads(output.getvalue())["error"], "one_candidate_path_required")


if __name__ == "__main__":
    unittest.main()
