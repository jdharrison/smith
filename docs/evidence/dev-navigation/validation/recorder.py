#!/usr/bin/env python3
"""Validation-only recorder adapted from project-repositories/run_{focused,gates}.py."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import stat
import subprocess
import threading
import time

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
EVIDENCE = ROOT / "docs/evidence/dev-navigation/validation"
CLI = ROOT / "target/installer-pipeline/smith-dagger-0.21.9/dagger"
CLI_SHA256 = "609fad4ab7ec88bd62ed79dbb4baf479154f7df4047149476b8f854a8f5f04af"
ENGINE = "smith-installer-pipeline-dagger-v0.21.9"
PRIOR_TREES = {
    "first-target": "target/projects-layout/validation-run",
    "final-target": "target/projects-layout/validation-final",
    "review-target": "target/projects-layout/assurance",
    "first-evidence": "docs/evidence/projects-layout/validation",
    "final-evidence": "docs/evidence/projects-layout/validation-final",
    "first-review": "docs/evidence/projects-layout/review",
    "final-review": "docs/evidence/projects-layout/review-final",
    "remediation": "docs/evidence/projects-layout/remediation",
}
PRIOR_REPORTS = ["docs/evidence/projects-layout/REVIEW.md", "docs/evidence/projects-layout/REVIEW-FINAL.md"]
SOURCE_ROOTS = ["service", "client", "installer", "dagger", ".github", "scripts",
                "Makefile", "rust-toolchain.toml", "dagger.toml", "dagger.lock",
                ".gitignore", ".gitattributes"]
EXCLUDED_PREFIXES = ["service/target/", "service/build/", "service/dist/",
                     "client/build/", "client/dist/", "client/.dart_tool/",
                     "client/.pub-cache/", "client/.pub/", "client/.idea/",
                     "client/linux/flutter/ephemeral/", "client/test/failures/",
                     "installer/target/", "installer/build/", "installer/dist/"]
BASE_ENV = {
    "HOME": str(Path.home()),
    "PATH": ":".join([str(Path.home() / ".cargo/bin"), "/usr/local/sbin",
                      "/usr/local/bin", "/usr/sbin", "/usr/bin", "/sbin", "/bin"]),
    "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "TZ": "UTC", "TERM": "dumb",
    "CI": "true", "PYTHONDONTWRITEBYTECODE": "1", "GIT_PAGER": "cat",
    "GIT_TERMINAL_PROMPT": "0", "GIT_OPTIONAL_LOCKS": "0",
    "DAGGER_NO_NAG": "1", "DAGGER_CLOUD_TOKEN": "", "DAGGER_PROGRESS": "plain",
    "DO_NOT_TRACK": "1", "DNT": "1",
    "_EXPERIMENTAL_DAGGER_RUNNER_HOST": "docker-container://" + ENGINE,
}
for key in ("USER", "LOGNAME"):
    if key in os.environ:
        BASE_ENV[key] = os.environ[key]


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def safe(value):
    if isinstance(value, str):
        return value.replace(str(ROOT), "<repo>").replace(str(Path.home()), "<home>")
    if isinstance(value, dict):
        return {safe(k): safe(v) for k, v in value.items()}
    if isinstance(value, (list, tuple)):
        return [safe(v) for v in value]
    return value


def digest(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def canonical_sha(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def save(name, value):
    text = json.dumps(safe(value), indent=2, sort_keys=True) + "\n"
    for directory in (OUT, EVIDENCE):
        with (directory / name).open("x") as handle:
            handle.write(text)


def git(*argv):
    return subprocess.check_output(["git", "--no-pager", *argv], cwd=ROOT,
                                   env=BASE_ENV, timeout=15)


def file_record(path):
    path = Path(path)
    relative = path.relative_to(ROOT).as_posix() if path.is_relative_to(ROOT) else str(path)
    if not path.exists() and not path.is_symlink():
        return {"path": safe(relative), "type": "missing"}
    info = path.lstat()
    record = {"path": safe(relative), "mode": stat.S_IMODE(info.st_mode),
              "mode_octal": oct(stat.S_IMODE(info.st_mode)), "size": info.st_size}
    if stat.S_ISLNK(info.st_mode):
        data = os.readlink(path).encode()
        record.update(type="symlink", target=safe(os.readlink(path)),
                      sha256=hashlib.sha256(data).hexdigest())
    elif stat.S_ISREG(info.st_mode):
        record.update(type="file", sha256=digest(path))
    elif stat.S_ISDIR(info.st_mode):
        record.update(type="directory")
        record.pop("size")
    else:
        record.update(type="special")
    return record


def manifest():
    tracked = set(git("ls-files", "--cached", "-z", "--", *SOURCE_ROOTS).split(b"\0"))
    candidates = set(git("ls-files", "--cached", "--others", "--exclude-standard", "-z",
                         "--", *SOURCE_ROOTS).split(b"\0"))
    records = []
    for value in sorted(candidates):
        if not value:
            continue
        name = os.fsdecode(value)
        if (any(name.startswith(prefix) for prefix in EXCLUDED_PREFIXES)
                or "__pycache__" in Path(name).parts or name.endswith(".pyc")
                or name == "client/.flutter-plugins-dependencies"):
            continue
        record = file_record(ROOT / name)
        record["tracked"] = value in tracked
        records.append(record)
    return {"sha256": canonical_sha(records), "count": len(records),
            "tracked_count": sum(r["tracked"] for r in records),
            "untracked_count": sum(not r["tracked"] for r in records),
            "source_roots": SOURCE_ROOTS, "excluded_prefixes": EXCLUDED_PREFIXES,
            "other_exclusions": ["**/__pycache__/**", "**/*.pyc", "client/.flutter-plugins-dependencies",
                                 "docs/** (parent-owned policy/evidence; not executable core)",
                                 "target/** (recorders/cache/generated evidence bound separately)"],
            "files": records}


def source_delta(before, after):
    a = {v["path"]: v for v in before["files"]}
    b = {v["path"]: v for v in after["files"]}
    return [name for name in sorted(a.keys() | b.keys()) if a.get(name) != b.get(name)]


def record_tree(name, directory):
    directory = ROOT / directory
    records = [file_record(p) for p in sorted(directory.rglob("*"))] if directory.is_dir() else []
    value = {"recorded_at": now(), "root": str(directory.relative_to(ROOT)),
             "exists": directory.is_dir(), "sha256": canonical_sha(records),
             "file_count": sum(r["type"] == "file" for r in records),
             "directory_count": sum(r["type"] == "directory" for r in records),
             "total_file_bytes": sum(r.get("size", 0) for r in records if r["type"] == "file"),
             "entries": records}
    save(name, value)
    return value


def run(name, argv, seconds, cwd=ROOT, extra=None, skip_reason=None):
    # Exclusive-create markers prohibit rerunning/overwriting any attempt in this bundle.
    for suffix in (".started.json", ".json", ".log"):
        if (OUT / (name + suffix)).exists() or (EVIDENCE / (name + suffix)).exists():
            raise RuntimeError("Refusing to rerun recorded attempt: " + name)
    env = dict(BASE_ENV, **(extra or {}))
    started = now()
    clock = time.monotonic()
    before = manifest()
    save(name + "-source-before.json", before)
    executable = shutil.which(argv[0], path=env["PATH"]) if not Path(argv[0]).is_absolute() else argv[0]
    metadata = {"name": name, "argv": argv, "cwd": str(cwd), "environment": env,
                "environment_policy": "explicit complete allowlist; no inherited provider credentials, session bus, display, analytics/cloud tokens",
                "started_at": started, "timeout_seconds": seconds,
                "executable": file_record(Path(executable).resolve()) if executable else None,
                "core_before_sha256": before["sha256"]}
    save(name + ".started.json", metadata)
    print("RUN", name, json.dumps(safe(argv)), flush=True)
    timed_out = threading.Event()
    code = None
    error = None
    with (OUT / (name + ".log")).open("x") as log:
        if skip_reason:
            log.write("NOT RUN: " + safe(skip_reason) + "\n")
        else:
            proc = None
            timer = None
            try:
                proc = subprocess.Popen(argv, cwd=cwd, env=env, stdout=subprocess.PIPE,
                                        stderr=subprocess.STDOUT, text=True, errors="replace",
                                        start_new_session=True)

                def expire():
                    if proc.poll() is not None:
                        return
                    timed_out.set()
                    try:
                        os.killpg(proc.pid, signal.SIGTERM)
                        time.sleep(3)
                        os.killpg(proc.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass

                timer = threading.Timer(seconds, expire)
                timer.daemon = True
                timer.start()
                for line in proc.stdout:
                    clean = safe(line)
                    log.write(clean)
                    log.flush()
                    print(clean, end="", flush=True)
                code = proc.wait(timeout=5)
            except Exception as exc:
                error = safe(type(exc).__name__ + ": " + str(exc))
                log.write("RECORDER ERROR: " + error + "\n")
                if proc is not None and proc.poll() is None:
                    os.killpg(proc.pid, signal.SIGKILL)
                    code = proc.wait(timeout=5)
            finally:
                if timer is not None:
                    timer.cancel()
    after = manifest()
    save(name + "-source-after.json", after)
    record = dict(metadata, finished_at=now(), duration_seconds=round(time.monotonic() - clock, 3),
                  exit_code=code, timed_out=timed_out.is_set(), recorder_error=error,
                  status="not_run" if skip_reason else ("pass" if code == 0 and not timed_out.is_set() and error is None else "fail"),
                  skip_reason=skip_reason, core_after_sha256=after["sha256"],
                  stable_inputs=before == after, changed_core_paths=source_delta(before, after),
                  log="docs/evidence/dev-navigation/validation/" + name + ".log",
                  log_sha256=digest(OUT / (name + ".log")))
    save(name + ".json", record)
    shutil.copyfile(OUT / (name + ".log"), EVIDENCE / (name + ".log"))
    print("RESULT", json.dumps(safe({k: record[k] for k in
          ["name", "status", "exit_code", "timed_out", "stable_inputs", "duration_seconds"]})), flush=True)
    return record


def test_summary(name):
    text = (OUT / (name + ".log")).read_text()
    plain = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", text)
    flutter = re.findall(r"\+([0-9]+)(?:\s+~([0-9]+))?(?:\s+-([0-9]+))?:\s+(All tests passed!|Some tests failed\.)", plain)
    python = re.findall(r"Ran (\d+) tests? in ([\d.]+)s", plain)
    rust = re.findall(r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out", plain)
    return {"counting_note": "Raw terminal matches; aggregate component totals must be grouped/deduplicated by Dagger vertex, not summed across failure replay.", "flutter_terminal_results": [{"passed": int(p), "skipped": int(s or 0), "failed": int(f or 0), "message": m} for p, s, f, m in flutter],
            "python_suites": [{"ran": int(n), "seconds": float(t)} for n, t in python],
            "rust_suites": [{"status": a, "passed": int(b), "failed": int(c), "ignored": int(d), "measured": int(e), "filtered": int(f)} for a,b,c,d,e,f in rust],
            "skip_or_ignore_lines": [line for line in plain.splitlines() if re.search(r"\b(skipped|SKIP|ignored)\b", line)],
            "failure_lines": [line for line in plain.splitlines() if re.search(r"FAILED|FAIL:|ERROR:|Some tests failed|make: \*\*\*", line)]}
