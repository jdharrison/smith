"""Offline wrapper checks: temporary roots, harmless local Python children only."""

import importlib.util
import json
import os
from pathlib import Path
import signal
import stat
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).resolve().parents[1] / "src/smith_agent/bounded_exec.py"
SPEC = importlib.util.spec_from_file_location("bounded_exec", SOURCE)
bounded = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(bounded)


class BoundedExecTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        root = Path(self.temp.name)
        self.workspace = root / "workspace"
        self.private = root / "private"
        self.workspace.mkdir()
        self.private.mkdir()
        self.paths = {name: self.private / name for name in ("stdout", "stderr", "status")}

    def run_child(self, code, seconds=3):
        result = bounded.run(
            [sys.executable, "-I", "-B", "-c", code], seconds=seconds,
            workspace=self.workspace, private=self.private, **self.paths,
        )
        raw = self.paths["status"].read_bytes()
        self.assertLessEqual(len(raw), 1024)
        self.assertEqual(json.loads(raw), result)
        self.assertEqual(set(result), {"exit_code", "reason"})
        self.assertIn(result["reason"], bounded.REASONS)
        self.assertTrue(result["exit_code"] is None or type(result["exit_code"]) is int)
        for path in self.paths.values():
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
        self.assertLessEqual(self.paths["stdout"].stat().st_size, bounded.STDOUT_LIMIT)
        self.assertLessEqual(self.paths["stderr"].stat().st_size, bounded.STDERR_LIMIT)
        return result

    def test_exit_nonzero_stdin_and_separate_streams(self):
        result = self.run_child(
            "import sys; assert sys.stdin.read() == ''; "
            "print('out'); print('err', file=sys.stderr); sys.exit(7)"
        )
        self.assertEqual(result, {"exit_code": 7, "reason": "exited"})
        self.assertEqual(self.paths["stdout"].read_bytes(), b"out\n")
        self.assertEqual(self.paths["stderr"].read_bytes(), b"err\n")

    def test_timeout(self):
        started = time.monotonic()
        result = self.run_child("import time; time.sleep(30)", seconds=0.15)
        self.assertEqual(result["reason"], "timed_out")
        self.assertEqual(result["exit_code"], -signal.SIGKILL)
        self.assertLess(time.monotonic() - started, 3)

    def test_stdout_flood(self):
        result = self.run_child("import os;\nwhile True: os.write(1, b'x' * 8192)")
        self.assertEqual(result["reason"], "output_limit")
        self.assertEqual(self.paths["stdout"].stat().st_size, bounded.STDOUT_LIMIT)

    def test_stderr_flood(self):
        result = self.run_child("import os;\nwhile True: os.write(2, b'x' * 8192)")
        self.assertEqual(result["reason"], "output_limit")
        self.assertEqual(self.paths["stderr"].stat().st_size, bounded.STDERR_LIMIT)

    def test_concurrent_streams_at_exact_caps(self):
        result = self.run_child(
            "import os, threading; "
            "t = threading.Thread(target=lambda: os.write(2, b'e' * 16384)); "
            "t.start(); os.write(1, b'o' * 65536); t.join()"
        )
        self.assertEqual(result, {"exit_code": 0, "reason": "exited"})
        self.assertEqual(self.paths["stdout"].stat().st_size, bounded.STDOUT_LIMIT)
        self.assertEqual(self.paths["stderr"].stat().st_size, bounded.STDERR_LIMIT)

    def assert_reaped(self, pid):
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            return
        # Only signal an unreaped direct test child, whose PID cannot be reused.
        try:
            if os.waitpid(pid, os.WNOHANG) == (0, 0):
                os.kill(pid, signal.SIGKILL)
                os.waitpid(pid, 0)
        except ChildProcessError:
            pass
        self.fail("descendant was not killed and reaped")

    def fork_child(self, *, leader_waits, seconds):
        code = (
            "import os, time\n"
            "pid = os.fork()\n"
            "if pid == 0:\n"
            " os.close(1); os.close(2); time.sleep(30); os._exit(0)\n"
            "open('child.pid', 'w').write(str(pid))\n"
            + ("time.sleep(30)\n" if leader_waits else "os._exit(0)\n")
        )
        result = self.run_child(code, seconds=seconds)
        self.assert_reaped(int((self.workspace / "child.pid").read_text()))
        return result

    def test_exited_leader_closed_pipe_descendant_cleanup(self):
        self.assertEqual(self.fork_child(leader_waits=False, seconds=3)["reason"], "exited")

    def test_timed_out_leader_closed_pipe_descendant_cleanup(self):
        self.assertEqual(self.fork_child(leader_waits=True, seconds=0.3)["reason"], "timed_out")

    def test_output_limit_descendant_cleanup(self):
        result = self.run_child(
            "import os, time\n"
            "pid = os.fork()\n"
            "if pid == 0:\n"
            " os.close(1); os.close(2); time.sleep(30); os._exit(0)\n"
            "open('child.pid', 'w').write(str(pid))\n"
            "while True: os.write(1, b'x' * 8192)\n"
        )
        self.assertEqual(result["reason"], "output_limit")
        self.assert_reaped(int((self.workspace / "child.pid").read_text()))

    def detached_child(self, *, leader_waits=False, nested=False, flood=False):
        record = self.workspace / "detached.pid"

        def fallback():
            if not record.exists():
                return
            pid, started = map(int, record.read_text().split())
            try:
                fd = os.pidfd_open(pid)
            except ProcessLookupError:
                return
            try:
                if bounded._identity(pid)[1] == started:
                    signal.pidfd_send_signal(fd, signal.SIGKILL)
                    try:
                        os.waitpid(pid, 0)
                    except ChildProcessError:
                        pass
            except (ProcessLookupError, FileNotFoundError):
                pass
            finally:
                os.close(fd)

        self.addCleanup(fallback)  # Even an assertion/exception must not leak it.
        code = (
            "import os, time\n"
            "r, w = os.pipe()\n"
            "pid = os.fork()\n"
            "if pid == 0:\n"
            " os.close(r); os.setsid(); os.close(1); os.close(2)\n"
            + (" if os.fork(): os._exit(0)\n" if nested else "")
            + " started = open('/proc/self/stat').read().rsplit(')', 1)[1].split()[19]\n"
            " open('detached.pid', 'w').write(str(os.getpid()) + ' ' + started)\n"
            " os.write(w, b'1'); os.close(w); time.sleep(30); os._exit(0)\n"
            "os.close(w); assert os.read(r, 1) == b'1'; os.close(r)\n"
            + ("while True: os.write(1, b'x' * 8192)\n" if flood else
               "time.sleep(30)\n" if leader_waits else "os._exit(0)\n")
        )
        result = self.run_child(code, seconds=0.3 if leader_waits else 3)
        pid, _ = map(int, record.read_text().split())
        self.assert_reaped(pid)
        return result

    def test_setsid_closed_pipes_exited_leader_descendant_reaped(self):
        self.assertEqual(self.detached_child(), {"exit_code": 0, "reason": "exited"})

    def test_setsid_closed_pipes_timed_out_leader_descendant_reaped(self):
        self.assertEqual(self.detached_child(leader_waits=True)["reason"], "timed_out")

    def test_output_limit_setsid_closed_pipes_descendant_reaped(self):
        self.assertEqual(self.detached_child(flood=True)["reason"], "output_limit")

    def test_double_fork_setsid_closed_pipes_descendant_reaped(self):
        self.assertEqual(self.detached_child(nested=True)["reason"], "exited")

    def test_preexisting_child_is_not_signalled_or_reaped(self):
        unrelated = subprocess.Popen([sys.executable, "-I", "-B", "-c",
                                      "import time; time.sleep(30)"])
        try:
            with patch.object(bounded.os, "killpg", side_effect=AssertionError), \
                    patch.object(bounded.signal, "pidfd_send_signal", side_effect=AssertionError):
                result = self.run_child("raise AssertionError('must not launch')")
            self.assertEqual(result, {"exit_code": None, "reason": "process_group_unknown"})
            self.assertIsNone(unrelated.poll())
        finally:
            unrelated.kill()
            unrelated.wait(timeout=3)

    def test_child_snapshot_failure_prevents_spawn(self):
        with patch.object(bounded, "_children", side_effect=OSError), \
                patch.object(bounded.subprocess, "Popen") as spawn:
            self.assertEqual(self.run_child("pass")["reason"], "process_group_unknown")
            spawn.assert_not_called()

    def test_cleanup_identity_mismatch_never_signals(self):
        process = unittest.mock.Mock(pid=123, returncode=0)
        with patch.object(bounded, "_children", return_value={456: 1}), \
                patch.object(bounded.os, "pidfd_open", return_value=99), \
                patch.object(bounded.os, "close"), \
                patch.object(bounded, "_identity", return_value=(os.getpid(), 2)), \
                patch.object(bounded.signal, "pidfd_send_signal") as send:
            self.assertFalse(bounded._cleanup(process))
            send.assert_not_called()

    def test_cleanup_deadline_is_fail_closed(self):
        process = unittest.mock.Mock(pid=123, returncode=None)
        with patch.object(bounded, "_children", return_value={123: 1}), \
                patch.object(bounded.os, "pidfd_open", return_value=99), \
                patch.object(bounded.os, "close"), \
                patch.object(bounded, "_identity", return_value=(os.getpid(), 1)), \
                patch.object(bounded.signal, "pidfd_send_signal"), \
                patch.object(bounded.time, "sleep"), \
                patch.object(bounded.time, "monotonic", side_effect=[10, 10, 10, 12]):
            self.assertFalse(bounded._cleanup(process))

    def test_unreadable_cleanup_children_is_fail_closed(self):
        process = unittest.mock.Mock(pid=123, returncode=0)
        with patch.object(bounded, "_children", side_effect=OSError), \
                patch.object(bounded.signal, "pidfd_send_signal") as send:
            self.assertFalse(bounded._cleanup(process))
            send.assert_not_called()

    def test_workspace_budget_combines_private_and_workspace(self):
        for root in (self.workspace, self.private):
            for index in range(5):
                with (root / str(index)).open("wb") as file:
                    file.truncate(bounded.FILE_LIMIT)
        result = self.run_child("raise AssertionError('must not launch')")
        self.assertEqual(result, {"exit_code": None, "reason": "workspace_limit"})

    def test_file_count(self):
        for index in range(bounded.FILE_COUNT):
            (self.workspace / str(index)).touch()
        self.assertEqual(self.run_child("pass")["reason"], "workspace_limit")

    def test_depth(self):
        directory = self.workspace
        for _ in range(bounded.MAX_DEPTH + 1):
            directory /= "d"
            directory.mkdir()
        self.assertEqual(self.run_child("pass")["reason"], "workspace_limit")

    def test_workspace_changes_monitored_after_spawn(self):
        result = self.run_child(
            "import pathlib, time; p = pathlib.Path('.'); "
            "[(p / str(i)).touch() for i in range(4100)]; time.sleep(30)"
        )
        self.assertEqual(result["reason"], "workspace_limit")

    def test_no_symlink_follow_during_budget_scan(self):
        outside = Path(self.temp.name) / "outside"
        outside.mkdir()
        with (outside / "large").open("wb") as file:
            file.truncate(bounded.WORKSPACE_LIMIT * 2)
        (self.workspace / "link").symlink_to(outside, target_is_directory=True)
        self.assertEqual(self.run_child("pass")["reason"], "exited")
        self.assertEqual((outside / "large").stat().st_size, bounded.WORKSPACE_LIMIT * 2)

    def test_individual_file_rlimit(self):
        result = self.run_child(
            "import resource; "
            "assert resource.getrlimit(resource.RLIMIT_FSIZE) == (16777216, 16777216); "
            "f = open('large', 'wb'); f.truncate(16777217)"
        )
        self.assertEqual(result["reason"], "exited")
        self.assertNotEqual(result["exit_code"], 0)
        self.assertLessEqual((self.workspace / "large").stat().st_size, bounded.FILE_LIMIT)

    def test_existing_capture_not_overwritten(self):
        self.paths["stdout"].write_bytes(b"preserve")
        with self.assertRaises(FileExistsError):
            self.run_child("pass")
        self.assertEqual(self.paths["stdout"].read_bytes(), b"preserve")

    def test_capture_symlink_and_parent_symlink_rejected(self):
        target = Path(self.temp.name) / "preserve"
        target.write_bytes(b"preserve")
        self.paths["stdout"].symlink_to(target)
        with self.assertRaises(FileExistsError):
            bounded._capture(self.paths["stdout"], self.private)
        (self.private / "alias").symlink_to(self.workspace, target_is_directory=True)
        with self.assertRaises(OSError):
            bounded._capture(self.private / "alias/new", self.private)
        self.assertEqual(target.read_bytes(), b"preserve")
        self.assertFalse((self.workspace / "new").exists())

    def test_production_capture_path_validation(self):
        for path in ("relative", "/tmp/out", "/private/../tmp/out", "/private"):
            with self.subTest(path=path), self.assertRaises(ValueError):
                bounded._capture(path, Path("/private"))

    def test_invalid_seconds(self):
        for seconds in (0, -1, 121, float("nan"), float("inf"), True):
            with self.subTest(seconds=seconds), self.assertRaises(ValueError):
                self.run_child("pass", seconds=seconds)
        self.assertEqual(list(self.private.iterdir()), [])

    def test_spawn_failure_fixed_status_without_diagnostics(self):
        with patch.object(bounded.subprocess, "Popen", side_effect=OSError("SECRET")):
            result = self.run_child("pass")
        self.assertEqual(result, {"exit_code": None, "reason": "process_group_unknown"})
        self.assertNotIn(b"SECRET", self.paths["status"].read_bytes())

    def test_unverified_cleanup_is_unknown(self):
        original = bounded._cleanup
        with patch.object(bounded, "_cleanup", side_effect=lambda process: (original(process) and False)):
            self.assertEqual(self.run_child("pass")["reason"], "process_group_unknown")

    def test_final_budget_check_after_reaping(self):
        original = bounded._cleanup

        def cleanup(process):
            known = original(process)
            with (self.private / "oversized").open("wb") as file:
                file.truncate(bounded.FILE_LIMIT + 1)
            return known

        with patch.object(bounded, "_cleanup", side_effect=cleanup):
            self.assertEqual(self.run_child("pass")["reason"], "workspace_limit")

    def test_clean_caller_environment_is_inherited(self):
        with patch.dict(os.environ, {"SMITH_TEST_ONLY": "inherited"}, clear=True):
            result = self.run_child(
                "import os; assert os.environ['SMITH_TEST_ONLY'] == 'inherited'; "
                "assert 'HOME' not in os.environ"
            )
        self.assertEqual(result, {"exit_code": 0, "reason": "exited"})

    def test_subreaper_setting_restored(self):
        libc = bounded.ctypes.CDLL(None)
        before, after = bounded.ctypes.c_int(), bounded.ctypes.c_int()
        self.assertEqual(libc.prctl(37, bounded.ctypes.byref(before), 0, 0, 0), 0)
        self.run_child("pass")
        self.assertEqual(libc.prctl(37, bounded.ctypes.byref(after), 0, 0, 0), 0)
        self.assertEqual(before.value, after.value)

    def test_cli_fixed_delimiter_and_status_contract(self):
        with patch.object(bounded, "run", return_value={"exit_code": 7, "reason": "exited"}) as run:
            self.assertEqual(bounded.main([
                "--seconds", "10", "--stdout", "/private/out", "--stderr", "/private/err",
                "--status", "/private/status", "--", sys.executable, "-c", "pass",
            ]), 0)
            self.assertEqual(run.call_args.args[0], [sys.executable, "-c", "pass"])
            self.assertEqual(run.call_args.kwargs["status"], "/private/status")

    def test_standalone_isolated_import_has_no_runtime_effects(self):
        code = (
            "import importlib.util, unittest.mock as m; "
            f"s = importlib.util.spec_from_file_location('wrapper', {str(SOURCE)!r}); "
            "module = importlib.util.module_from_spec(s)\n"
            "with m.patch('os.open', side_effect=AssertionError), "
            "m.patch('subprocess.Popen', side_effect=AssertionError), "
            "m.patch('ctypes.CDLL', side_effect=AssertionError): s.loader.exec_module(module)"
        )
        completed = subprocess.run([sys.executable, "-I", "-B", "-c", code],
                                   capture_output=True, timeout=5)
        self.assertEqual(completed.returncode, 0, completed.stderr[:1000])


if __name__ == "__main__":
    unittest.main()
