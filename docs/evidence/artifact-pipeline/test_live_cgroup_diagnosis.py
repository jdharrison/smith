"""Offline forensic reproductions of the immutable transport verifier's defects.

Passing tests confirm defective historical behavior, not a corrected verifier or
live acceptance. No engine, process, provider or operational receipt is touched.
"""
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).with_name("live-transport-qa.py")
SPEC = importlib.util.spec_from_file_location("historical_transport", SOURCE)
qa = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(qa)

ROOT = "/sys/fs/cgroup/system.slice/docker-" + "c" * 64 + ".scope"


class HistoricalCgroupDiagnosis(unittest.TestCase):
    def limits(self, membership):
        return {
            "/proc/123/cgroup": "0::" + membership + "\n",
            ROOT + "/cpu.max": "400000 100000\n",
            ROOT + "/memory.max": "4294967296\n",
            ROOT + "/pids.max": "1024\n",
            ROOT + "/init/cpu.max": "max 100000\n",
            ROOT + "/init/memory.max": "max\n",
            ROOT + "/init/pids.max": "max\n",
        }

    def probe(self, membership):
        files = self.limits(membership)
        with patch.object(Path, "is_file", return_value=True), patch.object(
            Path, "read_text", autospec=True,
            side_effect=lambda path: files[str(path)],
        ):
            return qa.cgroup(123)

    def test_original_accepts_limits_at_container_root(self):
        self.assertEqual(self.probe(ROOT.removeprefix("/sys/fs/cgroup")), Path(ROOT))

    def test_original_rejects_unlimited_leaf_despite_bounded_parent(self):
        with self.assertRaises(qa.Refuse):
            self.probe(ROOT.removeprefix("/sys/fs/cgroup") + "/init")

    def test_original_leaf_cleanup_does_not_inspect_populated_sibling(self):
        files = {
            ROOT + "/init/cgroup.events": "populated 0\nfrozen 0\n",
            ROOT + "/init/cgroup.procs": "",
            ROOT + "/worker/cgroup.events": "populated 1\nfrozen 0\n",
            ROOT + "/worker/cgroup.procs": "456\n",
        }
        observed = []

        def read(path):
            observed.append(str(path))
            return files[str(path)]

        with patch.object(Path, "exists", return_value=True), patch.object(
            Path, "is_dir", return_value=True,
        ), patch.object(Path, "rglob", return_value=iter(())), patch.object(
            Path, "read_text", autospec=True, side_effect=read,
        ):
            self.assertTrue(qa.empty_cgroup(Path(ROOT) / "init"))
        self.assertNotIn(ROOT + "/worker/cgroup.procs", observed)


if __name__ == "__main__":
    unittest.main()
