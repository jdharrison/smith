#!/usr/bin/env python3
"""Independent read-only input verification; only reviewer-owned outputs are written."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
BUNDLE = ROOT / "docs/evidence/projects"
EXPECTED_RESULTS = "c3c8c5b9719fb7e83d281429236f94958149fa2bd32124669f14385fec83d2b2"
EXPECTED_CORE = "5226a499e634e06d284f14b841c061d71c470cb903eedba95f228c40d9779d23"
REPRO_HASHES = {
    "assurance_recovery_test.dart": "b395fb737e3452339f5f87dc625e293a50c1310cf46fc6c260df312145252579",
    "assurance_delete_restart_test.dart": "07aa62ba88bbc95197ebb905d07564b990e4de235b9ce6a1926e34e718fb6761",
    "projects_view_test.dart": "64040a17600bea783cf41fbe62fc9eedb2a0eaf410e7c4d450d3a5f94db0b93d",
    "app_fakes.dart": "ca9badbd3bda514fe9cac17b518d30020c3ebfadaca3e58ceeef5b12fa9a35aa",
}


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def path(value):
    p = Path(value)
    assert not p.is_absolute() and ".." not in p.parts, value
    return ROOT / p


def record(value):
    p = path(value)
    mode = p.lstat().st_mode
    assert stat.S_ISREG(mode) or stat.S_ISLNK(mode), value
    data = os.readlink(p).encode() if stat.S_ISLNK(mode) else p.read_bytes()
    return {"path": value, "mode": stat.S_IMODE(mode), "size": len(data), "sha256": sha(data)}


def verify(records):
    assert len({r["path"] for r in records}) == len(records), "Duplicate input path"
    for r in records:
        assert record(r["path"]) == r, "Changed input: " + r["path"]


def load_lines(p):
    return [json.loads(line) for line in p.read_text().splitlines() if line]


def write(name, value):
    with (OUT / name).open("x") as f:
        f.write(json.dumps(value, indent=2, sort_keys=True) + "\n")


def safe(text):
    return text.replace(str(ROOT), "<repo>").replace(str(Path.home()), "<home>")


def current_core():
    raw = subprocess.check_output(
        ["git", "--no-pager", "ls-files", "--cached", "--others", "--exclude-standard", "-z", "--",
         "service", "client", "installer", "dagger", "Makefile", "rust-toolchain.toml", "dagger.toml", "dagger.lock"],
        cwd=ROOT,
    )
    return [record(os.fsdecode(v)) for v in sorted(set(raw.split(b"\0"))) if v]


def canonical(records):
    return sha(json.dumps(records, sort_keys=True, separators=(",", ":")).encode())


def run(name, argv, cwd=ROOT, env=None, seconds=120):
    started = now()
    result = subprocess.run(argv, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                            text=True, timeout=seconds)
    output = safe(result.stdout)
    with (OUT / (name + ".log")).open("x") as f:
        f.write(output)
    data = {"argv": [safe(str(v)) for v in argv], "cwd": str(cwd.relative_to(ROOT)),
            "started_at": started, "finished_at": now(), "exit_code": result.returncode,
            "timed_out": False, "log": "reviewer-records/" + name + ".log", "log_sha256": sha(output.encode())}
    write(name + ".json", data)
    print(name, "exit", result.returncode, flush=True)
    assert result.returncode == 0, output
    return output, data


def static_checks():
    assert sha((BUNDLE / "results.json").read_bytes()) == EXPECTED_RESULTS
    d = json.loads((BUNDLE / "results.json").read_text())
    core = load_lines(BUNDLE / "core-source-manifest.jsonl")
    supporting = load_lines(BUNDLE / "supporting-inputs.jsonl")
    assert len(core) == 1242 and canonical(core) == EXPECTED_CORE
    assert core == current_core()
    for version in ["before", "after"]:
        saved = json.loads(path("target/projects-validation/core-manifest-" + version + ".json").read_text())
        assert saved == {"count": 1242, "files": core, "sha256": EXPECTED_CORE}
    assert sha((BUNDLE / "core-source-manifest.jsonl").read_bytes()) == d["source"]["manifest_file_sha256"]
    assert sha((BUNDLE / "supporting-inputs.jsonl").read_bytes()) == d["source"]["supporting_snapshot_sha256"]
    assert len(d["raw_evidence"]) == 59
    verify(d["raw_evidence"])
    verify(supporting)
    gates = json.loads(path("target/projects-validation/results.json").read_text())
    assert gates == d["final_gates"] and gates["passed"] and gates["stable_inputs"]
    assert all(c["exit_code"] == 0 and not c["timed_out"] for c in gates["commands"])
    assert record("target/installer-pipeline/smith-dagger-0.21.9/dagger")["sha256"] == gates["cli_sha256"]
    artifact_path = BUNDLE / "artifacts.json"
    assert sha(artifact_path.read_bytes()) == d["artifacts"]["sha256"]
    artifacts = json.loads(artifact_path.read_text())
    verify(artifacts["files"])
    actual_outputs = []
    for directory, count in artifacts["file_counts"].items():
        items = [record(str(p.relative_to(ROOT))) for p in path(directory).rglob("*") if p.is_file() or p.is_symlink()]
        assert len(items) == count
        actual_outputs.extend(items)
    assert sorted(actual_outputs, key=lambda r: r["path"]) == sorted(artifacts["files"], key=lambda r: r["path"])
    services = {p: record(p)["sha256"] for p in artifacts["service_sha256"]}
    assert services == artifacts["service_sha256"] and len(set(services.values())) == 1
    assert path("installer/dist/README.txt").read_bytes() == path("dagger/INSTALLER.md").read_bytes()
    assert record("installer/dist/manifest.json")["sha256"] == artifacts["package_manifest_sha256"]
    assert record("installer/dist/SHA256SUMS")["sha256"] == artifacts["package_checksums_sha256"]
    client = {str(p.relative_to(path("client/dist"))): (p.read_bytes(), stat.S_IMODE(p.stat().st_mode))
              for p in path("client/dist").rglob("*") if p.is_file()}
    pair = {str(p.relative_to(path("installer/dist/payload/client"))): (p.read_bytes(), stat.S_IMODE(p.stat().st_mode))
            for p in path("installer/dist/payload/client").rglob("*") if p.is_file()}
    assert client == pair
    compiled = d["compiled_projects"]
    assert compiled == json.loads(path("target/projects-validation/compiled-projects.json").read_text())
    assert compiled["inputs_before"] == compiled["inputs_after"] and compiled["exit_code"] == 0
    for p, digest in compiled["inputs_after"].items():
        assert record(p)["sha256"] == digest
    compiled_log = path(compiled["log"]).read_bytes()
    assert sha(compiled_log) == compiled["log_sha256"] and b"+4: All tests passed!" in compiled_log
    # Only the first aggregate session: later build sessions reuse vertex IDs.
    text = re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", "", path("target/projects-validation/root-gates.log").read_text())
    lines = text.splitlines()
    end = next(i for i, line in enumerate(lines) if re.search(r"SmithNext\.test DONE", line))
    phase = lines[:end + 1]
    labels = {22: "service", 72: "installer", 63: "gtk", 94: "flutter", 124: "package"}
    rows = []
    sums = {key: {"passed": 0, "failed": 0, "controlled_ignores": 0} for key in ["service", "installer", "gtk"]}
    for line in phase:
        m = re.match(r"^(22|72|63)\s+:.*test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored", line)
        if m:
            vertex, passed, failed, ignored = map(int, m.groups())
            rows.append({"vertex": vertex, "passed": passed, "failed": failed,
                         "controlled_ignores": ignored, "stdout": line})
            for key, value in [("passed", passed), ("failed", failed), ("controlled_ignores", ignored)]:
                sums[labels[vertex]][key] += value
    assert rows == d["rust_stdout_summaries"]
    assert sums == {"service": {"passed": 289, "failed": 0, "controlled_ignores": 3},
                    "installer": {"passed": 283, "failed": 0, "controlled_ignores": 2},
                    "gtk": {"passed": 9, "failed": 0, "controlled_ignores": 0}}
    for vertex in labels:
        status = [line for line in phase if re.match(rf"^{vertex}\s+:.*Container.withExec (CACHED|DONE)", line)]
        assert status and all("CACHED" not in line for line in status), vertex
    joined = "\n".join(phase)
    assert re.search(r"^94\s+:.*\+773: All tests passed!", joined, re.M)
    assert not re.search(r"^94\s+:.*(?:\s~\d+|\(skipped\)|SKIP)", joined, re.M)
    assert re.search(r"^124\s+:.*Ran 41 tests", joined, re.M)
    assert not re.search(r"^124\s+:.*(?:skipped=|FAILED|FAIL:)", joined, re.M)
    helpers = path("target/projects-validation/pipeline-helpers.log").read_text()
    assert re.search(r"Ran 34 tests", helpers) and re.search(r"^OK$", helpers, re.M)
    assert "skipped=" not in helpers
    ignores = [line for line in phase if re.search(r"test .* \.\.\. ignored,", line)]
    assert len(ignores) == 5
    assert "Formatted 89 files (0 changed)" in joined and "No issues found!" in joined
    assert d["counts"] == {"aggregate": 1395, "controlled_fixture_ignores": 5, "failures": 0,
                           "flutter": 773, "gtk": 9, "installer": 283, "other_skips": 0,
                           "package": 41, "pipeline_helpers": 34, "service": 289}
    for directory, passed, key in [("attempt-1", False, "preserved_failed_attempt"),
                                    ("attempt-2", True, "superseded_passing_attempt")]:
        saved = json.loads(path("target/projects-validation/" + directory + "/results.json").read_text())
        assert saved == d[key]["result"] and saved["passed"] == passed
    head = subprocess.check_output(["git", "--no-pager", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    assert head == d["final_gate_head"] == "14d960975e76f906efb5e447ff3dece0e8e4385b"
    mirror = path("target/projects-assurance/client")
    mirror_files = {str(p.relative_to(mirror / "lib")): p.read_bytes() for p in (mirror / "lib").rglob("*") if p.is_file()}
    production_files = {str(p.relative_to(path("client/lib"))): p.read_bytes() for p in path("client/lib").rglob("*") if p.is_file()}
    assert mirror_files == production_files and len(mirror_files) == 34
    for name, digest in REPRO_HASHES.items():
        assert record("docs/evidence/projects/reviewer-repros/" + name)["sha256"] == digest
        assert record("target/projects-assurance/client/test/" + name)["sha256"] == digest
    independent = json.loads(path("target/projects-assurance/recovery-rereview-2/results.json").read_text())
    assert independent["all_passed"] and all(c["exit_code"] == 0 for c in independent["commands"])
    for r in json.loads(path("target/projects-assurance/recovery-rereview-2/source-after.json").read_text()):
        assert record(r["path"]) == r
    for which in ["controller", "view"]:
        assert path("client/test/projects_" + which + "_test.dart").read_bytes() == (mirror / ("test/recovery_" + which + "_test.dart")).read_bytes()
    return d, core, supporting, artifacts, {"head": head, "core_count": len(core), "core_sha256": EXPECTED_CORE,
        "raw_records_verified": len(d["raw_evidence"]), "supporting_records_verified": len(supporting),
        "artifact_records_verified": len(artifacts["files"]), "artifact_file_counts": artifacts["file_counts"],
        "counts": d["counts"], "rust_sums": sums, "execution_log_vertices": labels,
        "test_vertices_non_cached": True, "controlled_ignore_lines": ignores,
        "compiled_cases": 4, "prior_independent_cases": 82, "runtime_mirror_files_equal": 34,
        "guide_sha256": record("dagger/INSTALLER.md")["sha256"], "all_checks_passed": True}


def input_records(d, core, supporting, artifacts):
    extra = ["docs/evidence/projects/" + name for name in ["results.json", "README.md", "artifacts.json", "core-source-manifest.jsonl", "supporting-inputs.jsonl"]]
    extra += ["docs/ARCHITECTURE_DETAILS.md", "docs/SCOPE_DETAILS.md", "docs/HOST_PROFILE_DETAILS.md"]
    values = {r["path"] for r in core + supporting + artifacts["files"] + d["raw_evidence"]}
    values.update(extra)
    values.update(d["compiled_projects"]["inputs_before"])
    return [record(p) for p in sorted(values)]


def main():
    d, core, supporting, artifacts, checks = static_checks()
    current = input_records(d, core, supporting, artifacts)
    if sys.argv[1:] == ["--seal"]:
        saved = load_lines(OUT / "review-inputs.jsonl")
        assert current == saved, "Review inputs changed during writing"
        review = record("docs/evidence/projects/REVIEW.md")
        owned = [record(str(p.relative_to(ROOT))) for directory in [OUT, BUNDLE / "reviewer-repros"]
                 for p in sorted(directory.rglob("*")) if p.is_file() and p.name != "review-bindings.json"]
        write("review-bindings.json", {"work_id": "LOCAL-SMITH-PROJECTS", "sealed_at": now(),
            "approved_initial_base": d["approved_initial_base"], "final_head": checks["head"],
            "review": review, "parent_results_sha256": EXPECTED_RESULTS, "core_sha256": EXPECTED_CORE,
            "review_inputs_manifest": record(str((OUT / "review-inputs.jsonl").relative_to(ROOT))),
            "parent_inputs_unchanged_through_review": True, "checks": checks, "reviewer_owned_records": owned,
            "authority": "Source-limited assurance recommendation only; no installed/external/release approval.",
            "integrity_limit": "Owner-writable hashes detect drift; independently retain the final review digest."})
        print("SEALED REVIEW", review["sha256"])
        return
    assert not sys.argv[1:]
    assert not (OUT / "verification.json").exists(), "Preserve earlier verification"
    with (OUT / "review-inputs.jsonl").open("x") as f:
        for r in current:
            f.write(json.dumps(r, sort_keys=True) + "\n")
    started = now()
    mirror = path("target/projects-assurance/client")
    sdk = Path.home() / "snap/flutter/common/flutter/bin/cache"
    argv = [str(sdk / "dart-sdk/bin/dart"), str(sdk / "flutter_tools.snapshot"), "--suppress-analytics", "test", "--no-pub",
            "--reporter", "expanded", "--concurrency", "1", "test/assurance_recovery_test.dart", "test/assurance_delete_restart_test.dart"]
    overrides = {"CI": "true", "FLUTTER_ALREADY_LOCKED": "true", "FLUTTER_SUPPRESS_ANALYTICS": "true", "DART_SUPPRESS_ANALYTICS": "true"}
    output, repro = run("final-repros", argv, cwd=mirror, env=os.environ | overrides)
    assert "+5: All tests passed!" in output
    package_output, package = run("package-verify", ["python3", "-B", "dagger/package.py", "--root", "installer/dist", "--verify"], seconds=60)
    assert "95 files" in package_output
    _, diff = run("diff-check", ["git", "--no-pager", "diff", "--check"], seconds=20)
    assert input_records(*static_checks()[:4]) == current
    checks.update(started_at=started, finished_at=now(), parent_results_sha256=EXPECTED_RESULTS,
                  final_repro_cases=5, final_repro_environment=overrides,
                  cached_sdk_sha256={"dart": sha((sdk / "dart-sdk/bin/dart").read_bytes()),
                                     "flutter_tools.snapshot": sha((sdk / "flutter_tools.snapshot").read_bytes())},
                  independent_commands=[repro, package, diff], input_records=len(current))
    write("verification.json", checks)
    print(json.dumps(checks, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
