"""Copy only retained source/tests, deny sockets/processes, never write docs.

Run: PYTHONDONTWRITEBYTECODE=1 python3 -B pipeline/tests/run_retained_isolated.py
The namespace package bypasses the unchanged Dagger module initializer; this
checks dependency-free supervisor source, not installed SDK/import acceptance.
"""

import importlib.util
import io
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import patch


def main():
    if not sys.dont_write_bytecode or os.environ.get("PYTHONDONTWRITEBYTECODE") != "1":
        raise SystemExit("Use PYTHONDONTWRITEBYTECODE=1 python3 -B")
    root = Path(__file__).absolute().parents[1]
    output = io.StringIO()
    with tempfile.TemporaryDirectory(prefix="smith-retained-") as temporary:
        isolated = Path(temporary)
        for name in ("retained.py", "retained_service.py", "protocol.py", "graph.py"):
            shutil.copyfile(root / "src/smith_agent" / name, isolated / name)
        test_names = ("test_retained", "test_retained_service")
        for name in test_names:
            shutil.copyfile(root / ("tests/" + name + ".py"), isolated / (name + ".py"))
        package = types.ModuleType("smith_agent")
        package.__path__ = [str(isolated)]
        previous_directory = Path.cwd()
        try:
            os.chdir(isolated)
            with (patch.dict(sys.modules, {"smith_agent": package}),
                  patch.object(socket.socket, "connect", side_effect=AssertionError("network denied")),
                  patch.object(socket.socket, "connect_ex", side_effect=AssertionError("network denied")),
                  patch.object(socket.socket, "bind", side_effect=AssertionError("listen denied")),
                  patch.object(subprocess, "Popen", side_effect=AssertionError("process denied"))):
                suite = unittest.TestSuite()
                for name in test_names:
                    spec = importlib.util.spec_from_file_location(name, isolated / (name + ".py"))
                    tests = importlib.util.module_from_spec(spec)
                    spec.loader.exec_module(tests)
                    suite.addTests(unittest.defaultTestLoader.loadTestsFromModule(tests))
                result = unittest.TextTestRunner(stream=output, verbosity=2).run(suite)
        finally:
            os.chdir(previous_directory)
        log = output.getvalue().replace(str(isolated), "<isolated>")
    print("\n".join(line[:1500] for line in log.splitlines())[:24000])
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    sys.exit(main())
