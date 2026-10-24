"""Offline fixtures only: no Docker, subprocess, host proc/cgroup or network."""

import importlib.util
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

# Load only this source-only helper; historical supervisors remain untouched.
SPEC = importlib.util.spec_from_file_location(
    "owned_cgroup_verifier", Path(__file__).with_name("owned_cgroup_verifier.py")
)
v = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = v
SPEC.loader.exec_module(v)
CID = "c" * 64
PID = 123
SCOPE = "docker-" + CID + ".scope"
MEMBERSHIP = "/system.slice/" + SCOPE


class OwnedCgroupTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.cg = self.base / "cgroup"
        self.proc = self.base / "proc"
        self.root = self.cg / "system.slice" / SCOPE
        self.root.mkdir(parents=True)
        (self.proc / str(PID)).mkdir(parents=True)
        (self.proc / "1").mkdir()
        (self.proc / str(os.getpid())).mkdir(exist_ok=True)
        self.env = v._Environment(str(self.cg), str(self.proc))
        self.node(self.root, root=True, procs=str(PID) + "\n")
        self.membership(MEMBERSHIP)
        self.put(self.proc / str(PID) / "stat", self.proc_stat())
        self.put(self.proc / "1" / "comm", "systemd\n")
        self.put(self.proc / "1" / "cgroup", "0::/init.scope\n")
        dev = self.root.stat().st_dev
        self.mount = "32 24 %d:%d / /sys/fs/cgroup rw - cgroup2 cgroup rw\n" % (
            os.major(dev), os.minor(dev)
        )
        self.mountfile = self.proc / str(os.getpid()) / "mountinfo"
        self.put(self.mountfile, self.mount)

    def put(self, path, value):
        path.write_text(value, encoding="ascii")

    def node(self, path, *, root=False, procs="", populated="0"):
        path.mkdir(parents=True, exist_ok=True)
        self.put(path / "cpu.max", "400000 100000\n" if root else "max 100000\n")
        self.put(path / "memory.max", "4294967296\n" if root else "max\n")
        self.put(path / "pids.max", "1024\n" if root else "max\n")
        self.put(path / "cgroup.procs", procs)
        self.put(path / "cgroup.events", "populated " + populated + "\nfrozen 0\n")

    def membership(self, path):
        self.put(self.proc / str(PID) / "cgroup", "0::" + path + "\n")

    def proc_stat(self, start="42"):
        # Include a ')' inside comm: starttime is field22, not whitespace22.
        return str(PID) + " (engine ) worker) S " + "0 " * 18 + start + " 0\n"

    def capture(self, **kwargs):
        held = v.capture(PID, CID, _env=self.env, **kwargs)
        self.addCleanup(held.close)
        return held

    def refusal(self, code, call):
        with self.assertRaises(v.Refusal) as caught:
            call()
        self.assertEqual(caught.exception.code, code)
        self.assertEqual(str(caught.exception), code.value)
        self.assertLess(len(str(caught.exception)), 64)
        self.assertIsNone(caught.exception.__cause__)

    def test_root_capture_identity_and_close(self):
        held = self.capture()
        self.assertIsInstance(held, v.OwnedCgroup)
        self.assertEqual(held.identity, (self.root.stat().st_dev, self.root.stat().st_ino))
        self.assertFalse(held.quiescent())
        self.put(self.root / "cgroup.procs", "")
        self.assertTrue(held.quiescent())
        held.close()
        held.close()
        self.refusal(v.Code.CLOSED, held.quiescent)

    def test_inherited_leaf_and_intermediate_limits(self):
        self.node(self.root / "init")
        self.node(self.root / "init" / "worker", procs="123\n")
        self.put(self.root / "cgroup.procs", "")
        self.membership(MEMBERSHIP + "/init/worker")
        held = self.capture()
        self.assertFalse(held.quiescent())
        self.put(self.root / "init" / "worker" / "cgroup.procs", "")
        self.assertTrue(held.quiescent())

    def test_finite_children_and_cpu_at_or_below_four(self):
        self.node(self.root / "init", procs="123\n")
        self.membership(MEMBERSHIP + "/init")
        self.put(self.root / "init" / "cpu.max", "1000 1000\n")
        self.put(self.root / "init" / "memory.max", "1024\n")
        self.put(self.root / "init" / "pids.max", "1\n")
        self.put(self.root / "cpu.max", "300000 100000\n")
        self.capture()

    def test_aggregate_limits_cannot_be_replaced_by_leaf_or_shared_parent(self):
        for name, bad in (("cpu.max", "max 100000\n"),
                          ("cpu.max", "400001 100000\n"),
                          ("memory.max", "max\n"),
                          ("memory.max", "4294967295\n"),
                          ("pids.max", "1023\n"), ("pids.max", "1025\n")):
            with self.subTest(name=name, bad=bad):
                self.node(self.root, root=True, procs="123\n")
                self.put(self.root / name, bad)
                self.refusal(v.Code.LIMIT, self.capture)

    def test_inputs_full_lowercase_cid_only(self):
        for cid in (CID[:-1], CID + "a", CID.upper(), "../" + CID, "g" * 64, None):
            with self.subTest(cid=cid):
                self.refusal(v.Code.INPUT, lambda: v.capture(PID, cid, _env=self.env))
        for pid in (0, 1, -1, True, "123", 2147483648):
            with self.subTest(pid=pid):
                self.refusal(v.Code.INPUT, lambda: v.capture(pid, CID, _env=self.env))

    def test_membership_component_containment(self):
        for suffix in ("-sibling", "/../other", "//init", "/.", "/init/", "/bad name"):
            with self.subTest(suffix=suffix):
                self.membership(MEMBERSHIP + suffix)
                self.refusal(v.Code.LAYOUT, self.capture)
        self.membership("/user.slice/" + SCOPE)
        self.refusal(v.Code.LAYOUT, self.capture)

    def test_pid_must_be_in_membership_leaf(self):
        self.put(self.root / "cgroup.procs", "456\n")
        self.refusal(v.Code.RACE, self.capture)

    def test_sibling_population_and_root_events(self):
        held = self.capture()
        self.put(self.root / "cgroup.procs", "")
        self.node(self.root / "init")
        self.node(self.root / "sibling", procs="456\n")
        self.assertFalse(held.quiescent())
        self.put(self.root / "sibling" / "cgroup.procs", "")
        self.put(self.root / "cgroup.events", "populated 1\n")
        self.assertFalse(held.quiescent())
        self.put(self.root / "cgroup.events", "populated 0\n")
        self.assertTrue(held.quiescent())

    def test_replaced_or_missing_root_is_unknown(self):
        held = self.capture()
        self.root.rename(self.root.with_name("old"))
        self.refusal(v.Code.IO, held.quiescent)
        self.node(self.root, root=True)
        self.refusal(v.Code.RACE, held.quiescent)

    def test_deleted_held_root_is_unknown(self):
        held = self.capture()
        for entry in self.root.iterdir():
            entry.unlink()
        self.root.rmdir()
        self.refusal(v.Code.IO, held.quiescent)

    def test_replaced_mount_root_is_unknown(self):
        held = self.capture()
        self.cg.rename(self.base / "old-cgroup")
        self.root.mkdir(parents=True)
        self.refusal(v.Code.RACE, held.quiescent)

    def test_host_mount_mapping_and_systemd_evidence(self):
        for bad in (self.mount.replace(" / /sys", " /delegated /sys"),
                    self.mount.replace("cgroup2", "tmpfs"),
                    self.mount.replace(self.mount.split()[2], "999:999"),
                    self.mount + self.mount, "",
                    self.mount + "33 32 0:1 / /sys/fs/cgroup/system.slice rw - tmpfs tmpfs rw\n"):
            with self.subTest(mount=bad):
                self.put(self.mountfile, bad)
                self.refusal(v.Code.LAYOUT, self.capture)
        self.put(self.mountfile, self.mount)
        self.put(self.proc / "1" / "cgroup", "0::/\n")
        self.refusal(v.Code.LAYOUT, self.capture)
        self.put(self.proc / "1" / "cgroup", "0::/init.scope\n")
        self.put(self.proc / "1" / "comm", "init\n")
        self.refusal(v.Code.LAYOUT, self.capture)

    def test_large_mountinfo_separate_bound(self):
        unrelated = "40 24 0:2 / /unrelated rw - tmpfs tmpfs rw\n"
        text = self.mount + unrelated * 200
        self.assertGreater(len(text), 4096)
        self.put(self.mountfile, text)
        held = self.capture()
        self.put(self.root / "cgroup.procs", "")
        self.assertTrue(held.quiescent())
        # Exactly 128 KiB remains accepted, but one extra byte refuses.
        exact = text[:-1] + " " * (128 * 1024 - len(text)) + "\n"
        self.assertEqual(len(exact), 128 * 1024)
        self.put(self.mountfile, exact)
        self.assertTrue(held.quiescent())
        self.put(self.mountfile, exact + "\n")
        self.refusal(v.Code.BOUND, held.quiescent)
        self.refusal(v.Code.BOUND, self.capture)

    def test_mount_mapping_rechecked_for_cleanup(self):
        held = self.capture()
        self.put(self.mountfile, self.mount.replace("cgroup2", "tmpfs"))
        self.refusal(v.Code.LAYOUT, held.quiescent)

    def test_malformed_controls_root_and_intermediate(self):
        bads = (("cpu.max", "max\n"), ("cpu.max", "0 100000\n"),
                ("cpu.max", "max 0\n"), ("cpu.max", "1 100000\n"),
                ("cpu.max", "-1000 100000\n"),
                ("memory.max", "-1\n"), ("memory.max", "MAX\n"),
                ("pids.max", "-1\n"), ("pids.max", "1 2\n"),
                                ("memory.max", "18446744073709551616\n"),
                                ("cpu.max", "18446744073709551616 100000\n"))
        for name, value in bads:
            with self.subTest(name=name, value=value):
                self.node(self.root, root=True, procs="123\n")
                self.put(self.root / name, value)
                self.refusal(v.Code.MALFORMED, self.capture)
        self.node(self.root, root=True)
        self.node(self.root / "init")
        self.node(self.root / "init" / "leaf", procs="123\n")
        self.membership(MEMBERSHIP + "/init/leaf")
        self.put(self.root / "init" / "memory.max", "nonsense\n")
        self.refusal(v.Code.MALFORMED, self.capture)

    def test_malformed_events_procs_stat_and_membership(self):
        held = self.capture()
        for text in ("", "populated 2\n", "populated 0\npopulated 0\n", "populated x\n"):
            self.put(self.root / "cgroup.events", text)
            self.refusal(v.Code.MALFORMED, held.quiescent)
        self.put(self.root / "cgroup.events", "populated 0\n")
        self.put(self.root / "cgroup.procs", "not-a-pid\n")
        self.refusal(v.Code.MALFORMED, held.quiescent)
        self.put(self.proc / str(PID) / "stat", "broken\n")
        self.refusal(v.Code.MALFORMED, self.capture)
        self.put(self.proc / str(PID) / "stat", self.proc_stat())
        self.put(self.proc / str(PID) / "cgroup", "0::" + MEMBERSHIP + "\n1:cpu:/else\n")
        self.refusal(v.Code.LAYOUT, self.capture)

    def test_bounded_reads_and_non_ascii(self):
        for value, code in (("1" * 4097, v.Code.BOUND), ("1" * 4096, v.Code.MALFORMED)):
            self.put(self.root / "pids.max", value)
            self.refusal(code, self.capture)
        (self.root / "pids.max").write_bytes(b"\xff")
        self.refusal(v.Code.MALFORMED, self.capture)

    def test_zero_child_limits_are_valid_restrictions(self):
        self.node(self.root / "init", procs="123\n")
        self.membership(MEMBERSHIP + "/init")
        self.put(self.root / "init" / "memory.max", "0\n")
        self.put(self.root / "init" / "pids.max", "0\n")
        self.capture()

    def test_exact_read_bound(self):
        held = self.capture()
        self.put(self.root / "fixture", "x" * 4096)
        self.assertEqual(len(held._read(held._root, "fixture")), 4096)
        self.put(self.root / "fixture", "x" * 4097)
        self.refusal(v.Code.BOUND, lambda: held._read(held._root, "fixture"))

    def test_no_follow_control_directory_and_proc(self):
        source = self.root / "pids.max"
        source.rename(self.base / "limit")
        source.symlink_to(self.base / "limit")
        self.refusal(v.Code.IO, self.capture)
        source.unlink()
        self.put(source, "1024\n")
        self.node(self.base / "external", procs="123\n")
        (self.root / "init").symlink_to(self.base / "external", target_is_directory=True)
        self.membership(MEMBERSHIP + "/init")
        self.refusal(v.Code.IO, self.capture)
        self.membership(MEMBERSHIP)
        process = self.proc / str(PID)
        process.rename(self.proc / "old-pid")
        process.symlink_to(self.proc / "old-pid", target_is_directory=True)
        self.refusal(v.Code.IO, self.capture)

    def test_no_follow_cleanup_even_unrelated_symlinks_and_fifo(self):
        held = self.capture()
        link = self.root / "unrelated"
        link.symlink_to(self.base / "missing")
        self.refusal(v.Code.IO, held.quiescent)
        link.unlink()
        os.mkfifo(link)
        self.refusal(v.Code.IO, held.quiescent)

    def test_control_fifo_does_not_block(self):
        path = self.root / "pids.max"
        path.unlink()
        os.mkfifo(path)
        self.refusal(v.Code.IO, self.capture)

    def test_tree_depth_bound_and_capture_depth_bound(self):
        held = self.capture()
        path = self.root
        self.put(self.root / "cgroup.procs", "")
        for i in range(64):
            path /= "n"
            self.node(path)
        self.assertTrue(held.quiescent())
        self.node(path / "n")
        self.refusal(v.Code.BOUND, held.quiescent)
        self.membership(MEMBERSHIP + "/n" * 65)
        self.refusal(v.Code.BOUND, self.capture)

    def test_tree_node_and_directory_entry_bounds(self):
        held = self.capture()
        # Distributed below two branches so no per-directory entry bound hides
        # the independent whole-tree node bound.
        for branch in ("a", "b"):
            self.node(self.root / branch)
            for i in range(127):
                self.node(self.root / branch / str(i))
        self.refusal(v.Code.BOUND, held.quiescent)

    def test_exact_node_bound(self):
        held = self.capture()
        self.put(self.root / "cgroup.procs", "")
        self.node(self.root / "a")
        self.node(self.root / "b")
        for i in range(126):
            self.node(self.root / "a" / str(i))
            self.node(self.root / "b" / str(i))
        self.node(self.root / "a" / "last")
        # Root + two branches + 253 leaves = 256 nodes.
        self.assertTrue(held.quiescent())
        self.node(self.root / "b" / "excess")
        self.refusal(v.Code.BOUND, held.quiescent)

    def test_directory_entry_bound(self):
        held = self.capture()
        for i in range(257):
            self.put(self.root / ("extra" + str(i)), "")
        self.refusal(v.Code.BOUND, held.quiescent)

    def test_deadline_and_cancel_capture_and_cleanup(self):
        self.refusal(v.Code.INTERRUPTED, lambda: self.capture(clock=lambda: 10, deadline=10))
        self.refusal(v.Code.INTERRUPTED, lambda: self.capture(cancelled=lambda: True))
        for invalid in (float("nan"), float("inf"), "later"):
            self.refusal(v.Code.INPUT, lambda: self.capture(deadline=invalid))
        now = [1]
        cancelled = [False]
        held = self.capture(clock=lambda: now[0], deadline=5, cancelled=lambda: cancelled[0])
        cancelled[0] = True
        self.refusal(v.Code.INTERRUPTED, held.quiescent)
        cancelled[0] = False
        now[0] = 5
        self.refusal(v.Code.INTERRUPTED, held.quiescent)

    def test_fresh_cleanup_budget_after_fourteen_minutes(self):
        now = [0]
        stopped = [False]
        held = self.capture(clock=lambda: now[0], run_deadline=900,
                            cancelled=lambda: stopped[0])
        self.put(self.root / "cgroup.procs", "")
        now[0] = 840
        stopped[0] = True
        self.refusal(v.Code.INTERRUPTED, held.quiescent)
        self.assertTrue(held.quiescent(deadline=845, cancelled=lambda: False))
        # Overrides are per-observation, not persistent changes to capture.
        self.refusal(v.Code.INTERRUPTED, held.quiescent)
        self.refusal(v.Code.INTERRUPTED,
                     lambda: held.quiescent(deadline=839, cancelled=lambda: False))
        self.refusal(v.Code.INTERRUPTED,
                     lambda: held.quiescent(deadline=845, cancelled=lambda: True))
        self.assertTrue(held.quiescent(deadline=845, cancelled=lambda: False))

    def test_cleanup_budget_never_extends_original_run_deadline(self):
        now = [0]
        held = self.capture(clock=lambda: now[0], deadline=5, run_deadline=900)
        self.put(self.root / "cgroup.procs", "")
        now[0] = 899
        original = v.OwnedCgroup._events

        def events(obj):
            result = original(obj)
            now[0] = 900
            return result

        with patch.object(v.OwnedCgroup, "_events", events):
            self.refusal(v.Code.INTERRUPTED,
                         lambda: held.quiescent(deadline=1000, cancelled=lambda: False))
        self.refusal(v.Code.INTERRUPTED,
                     lambda: held.quiescent(deadline=1000, cancelled=lambda: False))

    def test_capture_run_ceiling_and_default_no_extension(self):
        now = [0]
        held = self.capture(clock=lambda: now[0])
        now[0] = 6
        self.refusal(v.Code.INTERRUPTED,
                     lambda: held.quiescent(deadline=20, cancelled=lambda: False))
        self.refusal(v.Code.INTERRUPTED,
                     lambda: self.capture(clock=lambda: 10, deadline=20, run_deadline=10))

    def test_observation_budget_validation_and_closed_root(self):
        held = self.capture(clock=lambda: 0, run_deadline=900)
        for invalid in (float("nan"), float("inf"), "later", True):
            self.refusal(v.Code.INPUT, lambda: held.quiescent(deadline=invalid))
            self.refusal(v.Code.INPUT, lambda: self.capture(run_deadline=invalid))
        self.refusal(v.Code.INPUT, lambda: held.quiescent(cancelled=False))
        self.refusal(v.Code.INPUT, lambda: self.capture(cancelled=False))
        self.root.rename(self.base / "missing-root")
        self.refusal(v.Code.IO,
                     lambda: held.quiescent(deadline=800, cancelled=lambda: False))
        held.close()
        self.refusal(v.Code.CLOSED,
                     lambda: held.quiescent(deadline=800, cancelled=lambda: False))

    def test_cancel_during_observation(self):
        stopped = [False]
        held = self.capture(cancelled=lambda: stopped[0])
        original = v.OwnedCgroup._events

        def events(obj):
            result = original(obj)
            stopped[0] = True
            return result

        with patch.object(v.OwnedCgroup, "_events", events):
            self.refusal(v.Code.INTERRUPTED, held.quiescent)

    def test_start_and_membership_races(self):
        original = v.OwnedCgroup._process
        for change in ("start", "membership"):
            self.membership(MEMBERSHIP)
            self.put(self.proc / str(PID) / "stat", self.proc_stat())
            calls = [0]

            def process(obj, fd):
                result = original(obj, fd)
                calls[0] += 1
                if calls[0] == 1:
                    if change == "start":
                        self.put(self.proc / str(PID) / "stat", self.proc_stat("43"))
                    else:
                        self.membership(MEMBERSHIP + "/init")
                return result

            with self.subTest(change=change), patch.object(v.OwnedCgroup, "_process", process):
                self.refusal(v.Code.RACE, self.capture)

    def test_pid_directory_replacement_race(self):
        original = v.OwnedCgroup._process
        calls = [0]

        def process(obj, fd):
            result = original(obj, fd)
            calls[0] += 1
            if calls[0] == 2:
                old = self.proc / str(PID)
                old.rename(self.proc / "old-pid")
                old.mkdir()
                self.put(old / "stat", self.proc_stat())
                self.membership(MEMBERSHIP)
            return result

        with patch.object(v.OwnedCgroup, "_process", process):
            self.refusal(v.Code.RACE, self.capture)

    def test_child_replacement_during_cleanup(self):
        held = self.capture()
        self.node(self.root / "child")
        original = v.OwnedCgroup._procs

        def procs(obj, fd):
            result = original(obj, fd)
            if v._identity(fd)[1] == (self.root / "child").stat().st_ino:
                (self.root / "child").rename(self.base / "old-child")
                self.node(self.root / "child")
            return result

        with patch.object(v.OwnedCgroup, "_procs", procs):
            self.refusal(v.Code.RACE, held.quiescent)

    def test_capture_leaf_replacement(self):
        self.node(self.root / "init", procs="123\n")
        self.membership(MEMBERSHIP + "/init")
        original = v.OwnedCgroup._procs

        def procs(obj, fd):
            result = original(obj, fd)
            (self.root / "init").rename(self.base / "old-init")
            self.node(self.root / "init", procs="123\n")
            return result

        with patch.object(v.OwnedCgroup, "_procs", procs):
            self.refusal(v.Code.RACE, self.capture)

    def test_refusal_closes_all_owned_descriptors(self):
        original = os.close
        opened = []
        closed = []
        real_open = os.open

        def tracked_open(*args, **kwargs):
            fd = real_open(*args, **kwargs)
            opened.append(fd)
            return fd

        def tracked_close(fd):
            closed.append(fd)
            return original(fd)

        self.put(self.root / "pids.max", "max\n")
        with patch.object(v.os, "open", tracked_open), patch.object(v.os, "close", tracked_close):
            self.refusal(v.Code.LIMIT, self.capture)
        # Duplicated root descriptor is also closed, so close count is larger.
        self.assertGreaterEqual(len(closed), len(opened))
        for fd in set(opened):
            with self.assertRaises(OSError):
                os.fstat(fd)


if __name__ == "__main__":
    unittest.main()
