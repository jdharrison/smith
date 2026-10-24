"""Run module/fake-SDK and unchanged fixture tests without Dagger or network."""


import io
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

FILES = (
    "dagger.json", "pyproject.toml", "image/Dockerfile",
    "src/smith_agent/__init__.py", "src/smith_agent/protocol.py",
    "src/smith_agent/artifacts.py", "src/smith_agent/graph.py", "src/smith_agent/main.py",
    "fixtures/clamp/TASK.md", "fixtures/clamp/clamp.py", "tests/check_clamp.py",
    "tests/test_clamp.py", "tests/test_module_source.py", "tests/run_module_isolated.py",
)


def main():
    if os.environ.get("PYTHONDONTWRITEBYTECODE") != "1" or not sys.dont_write_bytecode:
        raise SystemExit("Use PYTHONDONTWRITEBYTECODE=1 python3 -B pipeline/tests/run_module_isolated.py")
    repository = Path(__file__).absolute().parents[2]

    original_directory, original_path = Path.cwd(), sys.path[:]
    output = io.StringIO()
    with tempfile.TemporaryDirectory(prefix="smith-module-") as temporary:
        isolated = Path(temporary)
        for name in FILES:
            source = repository / "pipeline" / name
            destination = isolated / "pipeline" / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, destination)

        try:
            os.chdir(isolated)
            tests = isolated / "pipeline/tests"
            sys.path[:0] = [str(isolated / "pipeline/src"), str(tests)]
            with (patch.object(socket.socket, "connect", side_effect=AssertionError("network denied")),
                  patch.object(socket.socket, "connect_ex", side_effect=AssertionError("network denied")),
                  patch.object(subprocess, "Popen", side_effect=AssertionError("process denied"))):
                suite = unittest.defaultTestLoader.discover(str(tests), pattern="test_*.py")
                result = unittest.TextTestRunner(stream=output, verbosity=2).run(suite)
        finally:
            os.chdir(original_directory)
            sys.path[:] = original_path
        log = output.getvalue().replace(str(isolated), "<isolated>")
    log = log.replace(str(repository), "<repository>")
    command = "PYTHONDONTWRITEBYTECODE=1 python3 -B pipeline/tests/run_module_isolated.py"
    exit_code = 0 if result.wasSuccessful() else 1
    log = (f"Command: {command}\nPython: {sys.version.split()[0]}\n"
           + log + f"Exit: {exit_code}\n")
    excerpt = "\n".join(line[:1800] for line in log.splitlines())
    print(excerpt[:30000])
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
