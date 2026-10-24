"""Run only the trusted fixture tests in a disposable, private source copy."""

import io
import os
from pathlib import Path
import shutil
import sys
import tempfile
import unittest


def main():
    if os.environ.get("PYTHONDONTWRITEBYTECODE") != "1" or not sys.dont_write_bytecode:
        raise SystemExit("Use PYTHONDONTWRITEBYTECODE=1 python3 -B pipeline/tests/run_isolated.py")
    repository = Path(__file__).absolute().parents[2]
    log_path = repository / "docs/evidence/artifact-pipeline/fixture-tests.log"
    original_directory = Path.cwd()
    original_path = sys.path[:]
    output = io.StringIO()
    with tempfile.TemporaryDirectory(prefix="smith-clamp-") as temporary:
        isolated = Path(temporary)
        tests = isolated / "pipeline/tests"
        fixture = isolated / "pipeline/fixtures/clamp"
        tests.mkdir(parents=True)
        fixture.mkdir(parents=True)
        for name in ("check_clamp.py", "test_clamp.py"):
            shutil.copyfile(repository / "pipeline/tests" / name, tests / name)
        shutil.copyfile(repository / "pipeline/fixtures/clamp/clamp.py", fixture / "clamp.py")
        try:
            os.chdir(isolated)
            sys.path.insert(0, str(tests))
            suite = unittest.defaultTestLoader.discover(str(tests), pattern="test_clamp.py")
            result = unittest.TextTestRunner(stream=output, verbosity=2).run(suite)
        finally:
            os.chdir(original_directory)
            sys.path[:] = original_path
        log = output.getvalue().replace(str(isolated), "<isolated>")
    log = log.replace(str(repository), "<repository>")
    command = "PYTHONDONTWRITEBYTECODE=1 python3 -B pipeline/tests/run_isolated.py"
    exit_code = 0 if result.wasSuccessful() else 1
    log = f"Command: {command}\nPython: {sys.version.split()[0]}\n" + log
    log += f"Exit: {exit_code}\n"
    # Retain the full secret-safe log, but bound every terminal response and line.
    with log_path.open("a", encoding="utf-8") as evidence:
            evidence.write(log + "\n")
    excerpt = "\n".join(line[:1800] for line in log.splitlines())
    print(excerpt[:30000])
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
