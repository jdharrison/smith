#!/usr/bin/env python3
"""Export the final local repository-configuration evidence; no publication."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess

ROOT = Path(__file__).resolve().parents[2]
RAW = ROOT / "target/project-repositories"
FINAL = RAW / "attempt-2"
OUT = ROOT / "docs/evidence/project-repositories"
CORE = ["service", "client", "installer", "dagger", "Makefile", "rust-toolchain.toml", "dagger.toml", "dagger.lock"]
CONTROLS = [".github", "scripts", "AGENTS.md", "roles", "README.md", "VALIDATION.md", "docs/VALIDATION_DETAILS.md", "docs/SCOPE.md", "docs/SCOPE_DETAILS.md", "docs/ARCHITECTURE.md", "docs/ARCHITECTURE_DETAILS.md", "docs/PROCESS.md", "docs/DESIGN.md", "docs/HOST_PROFILE.md", "docs/HOST_PROFILE_DETAILS.md", "docs/SMITH_LINUX_ARCHITECTURE_REPORT.md", "docs/plans/LOCAL-SMITH-PROJECT-REPOSITORIES.md", "docs/evidence/project-repositories/README.md"]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load(path):
    return json.loads(path.read_text())


def git(*args):
    return subprocess.check_output(["git", "--no-pager", *args], cwd=ROOT)


def paths(*roots):
    return [Path(os.fsdecode(value)) for value in sorted(set(git("ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", *roots).split(b"\0"))) if value]


def record(path):
    full = ROOT / path
    mode = full.lstat().st_mode
    data = os.readlink(full).encode() if stat.S_ISLNK(mode) else full.read_bytes()
    return {"path": str(path), "mode": stat.S_IMODE(mode), "size": len(data), "sha256": hashlib.sha256(data).hexdigest()}


def jsonl(rows):
    return "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows)


def save(name, value):
    (OUT / name).write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


assert OUT.is_dir()
before = load(FINAL / "core-manifest-before.json")
after = load(FINAL / "core-manifest-after.json")
current = [record(path) for path in paths(*CORE)]
canonical = hashlib.sha256(json.dumps(current, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
assert before == after and current == before["files"] and canonical == before["sha256"]
assert len(current) == 1243 and canonical == "ed6dc46ef3938f7753ffb9a7a0c76e6be1a2ea88ed0ff760835610fa7cd53481"
final = load(FINAL / "results.json")
assert final["passed"] and final["stable_inputs"] and final["core_before_sha256"] == canonical
compiled = load(FINAL / "compiled-projects.json")
assert compiled["passed"] and compiled["inputs_before"] == compiled["inputs_after"]
for path, value in compiled["inputs_after"].items():
    assert sha(ROOT / path) == value
assert sha(ROOT / compiled["log"]) == compiled["log_sha256"]
assert "+7: All tests passed!" in (ROOT / compiled["log"]).read_text()

plain = re.sub(r"\x1b\[[0-9;]*m", "", (FINAL / "root-gates.log").read_text())
summaries = []
for line in plain.splitlines():
    match = re.match(r"(\d+)\s+:.*test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;", line)
    if match:
        vertex, passed, failed, ignored = map(int, match.groups())
        summaries.append({"vertex": vertex, "passed": passed, "failed": failed, "controlled_ignores": ignored, "stdout": line})
assert sum(row["passed"] for row in summaries if row["vertex"] == 29) == 295
assert sum(row["passed"] for row in summaries if row["vertex"] == 62) == 283
assert sum(row["passed"] for row in summaries if row["vertex"] == 76) == 9
assert sum(row["failed"] for row in summaries) == 0
assert sum(row["controlled_ignores"] for row in summaries) == 5
assert "+834: All tests passed!" in plain and "Ran 41 tests" in plain
assert "Ran 34 tests" in (FINAL / "pipeline-helpers.log").read_text()
assert "Formatted 90 files (0 changed)" in plain and "No issues found!" in plain

baseline = load(RAW / "baseline-manifest.json")
baseline_by_path = {row["path"]: row for row in baseline["files"]}
assert baseline["sha256"] == "41bea2ed038625478da27b4cc5a8b276eeec2ef5b51cb9d3b4334b2b416c215c"
prior_evidence = [row for row in baseline["files"] if row["path"].startswith("docs/evidence/projects/")]
assert prior_evidence and all(record(Path(row["path"])) == row for row in prior_evidence)
controls = [record(path) for path in paths(*CONTROLS)]
changes = [{"path": row["path"], "before": baseline_by_path.get(row["path"]), "after": row} for row in current + controls if baseline_by_path.get(row["path"]) != row]

artifacts = []
for name in ["service/dist", "client/dist", "installer/dist"]:
    for path in sorted((ROOT / name).rglob("*")):
        assert not path.is_symlink()
        if path.is_file():
            artifacts.append(record(path.relative_to(ROOT)))
assert len(artifacts) == 189
service_paths = ["service/dist/smith-service", "client/dist/smith_backend/smith-service", "installer/dist/payload/service/smith-service", "installer/dist/payload/client/smith_backend/smith-service"]
services = {name: sha(ROOT / name) for name in service_paths}
assert len(set(services.values())) == 1
client = {str(path.relative_to(ROOT / "client/dist")): (sha(path), stat.S_IMODE(path.stat().st_mode)) for path in (ROOT / "client/dist").rglob("*") if path.is_file()}
paired = {str(path.relative_to(ROOT / "installer/dist/payload/client")): (sha(path), stat.S_IMODE(path.stat().st_mode)) for path in (ROOT / "installer/dist/payload/client").rglob("*") if path.is_file()}
assert client == paired
assert (ROOT / "dagger/INSTALLER.md").read_bytes() == (ROOT / "installer/dist/README.txt").read_bytes()
package = load(ROOT / "installer/dist/manifest.json")
assert len(package["files"]) == 95
artifact_record = {"recorded_at": datetime.datetime.now(datetime.timezone.utc).isoformat(), "files": artifacts, "file_counts": {name: sum(row["path"].startswith(name + "/") for row in artifacts) for name in ["service/dist", "client/dist", "installer/dist"]}, "service_sha256": services, "all_four_services_equal": True, "paired_client_bytes_and_modes_equal": True, "packaged_guide_equals_current_source": True, "package_manifest_entries": 95, "package_manifest_sha256": sha(ROOT / "installer/dist/manifest.json"), "package_checksums_sha256": sha(ROOT / "installer/dist/SHA256SUMS"), "publication": "Ignored local build exports only; no clean-source provenance or publication claim."}

roots = [RAW, RAW / "attempt-1", FINAL, RAW / "focused-attempt-1", RAW / "assurance", RAW / "assurance/parser-followup-1", RAW / "assurance/final-raw-gates-1"]
raw_files = sorted({path for root in roots if root.is_dir() for path in root.iterdir() if path.is_file() and path.suffix in {".json", ".jsonl", ".log", ".py", ".dart", ".rs", ".diff"}})
raw_records = [record(path.relative_to(ROOT)) for path in raw_files]

(OUT / "core-source-manifest.jsonl").write_text(jsonl(current))
(OUT / "supporting-inputs.jsonl").write_text(jsonl(controls))
save("changes-from-baseline.json", {"comparison": "Selected final core/control paths against preserved actual working-tree baseline; unselected paths are not claimed removed.", "changes": changes})
save("artifacts.json", artifact_record)
result = {
    "work_id": "LOCAL-SMITH-PROJECT-REPOSITORIES",
    "status": "Final configured local source gates PASS; independent verdict/binding retained separately in REVIEW.md. Not installed/release acceptance.",
    "head": git("rev-parse", "HEAD").decode().strip(),
    "branch": git("branch", "--show-current").decode().strip(),
    "baseline": {"selected_file_count": len(baseline["files"]), "canonical_manifest_sha256": baseline["sha256"], "manifest_path": "target/project-repositories/baseline-manifest.json", "manifest_file_sha256": sha(RAW / "baseline-manifest.json"), "archive_path": "target/project-repositories/baseline-source.tar.gz", "archive_sha256": sha(RAW / "baseline-source.tar.gz"), "prior_projects_evidence_files_unchanged": len(prior_evidence)},
    "source": {"core_count": len(current), "canonical_array_sha256": canonical, "before_after_current_equal": True, "core_manifest_sha256": sha(OUT / "core-source-manifest.jsonl"), "supporting_snapshot_sha256": sha(OUT / "supporting-inputs.jsonl"), "supporting_snapshot_note": "Final post-gate controls/documentation snapshot; not retrospective before/after documentation stability.", "baseline_changes_sha256": sha(OUT / "changes-from-baseline.json")},
    "final_gates": final,
    "environment_overrides": {"CI": "true", "DAGGER_NO_NAG": "1", "DAGGER_CLOUD_TOKEN": "", "DAGGER_PROGRESS": "plain", "_EXPERIMENTAL_DAGGER_RUNNER_HOST": "docker-container://smith-installer-pipeline-dagger-v0.21.9", "pipeline_helpers_SMITH_TEST_PACKAGE": "<repo>/installer/dist"},
    "counts": {"service": 295, "installer": 283, "gtk": 9, "flutter": 834, "package": 41, "aggregate_executions": 1462, "pipeline_helpers": 34, "controlled_ignores": 5, "other_skips": 0, "failures": 0},
    "rust_stdout_summaries": summaries,
    "cache_boundary": "Fresh test-execution vertices; dependency/setup/build cache reuse. Not a clean-cache/hermetic build.",
    "configured_service_environment": "Rust1.98 Bookworm; not additional Ubuntu service proof. Other configured component/GTK/Flutter/runtime gates use Ubuntu24.04.",
    "compiled_projects": compiled,
    "compiled_counts": {"passed": 7, "failed": 0, "skipped": 0, "coverage_note": "Standalone corroboration; compiled Projects coverage is also included in834 aggregate Flutter passes."},
    "failure_history": {
        "aggregate_attempt_1": load(RAW / "attempt-1/results.json"),
        "aggregate_attempt_1_note": "Pre-test Dagger connection failure: existing builder stopped before this attempt. No component tests/builds or later verifier/helpers ran.",
        "focused_attempt_1": load(RAW / "focused-attempt-1/compiled-projects.json"),
        "focused_attempt_1_note": "Incorrect parent test compared restart-cleared inventory as durable. Exact failing source/log/record retained; corrected test explicitly checks volatile reset plus durable projection preservation.",
        "service_owner_retrospective": load(RAW / "service-owner-validation.json"),
        "client_owner_notes": "Initial missing-brace style lints were corrected; original standalone owner logs unavailable. New duplicate-key reviewer repro initially failed3/3, now passes unchanged after preflight/depth fix.",
        "ui_owner_notes": "Intermediate focus assertions and a test-edit syntax error were corrected; original standalone owner logs unavailable. Final49-case owner UI/navigation self-check reported passing, corroborated by final aggregate and independent tests.",
        "reviewer_history": "Original duplicate-key failures and two reviewer harness setup failures retained under assurance; final parser-followup-1 closes the finding without overwriting originals. Counts/repros are separately bound by REVIEW.md.",
        "file_tool_setup_note": "File-copy tools could not discover retained ignored recorder files; finite filesystem check verified their actual hashes, then adapted recorder scripts were prepared locally. Prior records were not modified.",
    },
    "builder_recovery": load(RAW / "builder-recovery.json"),
    "builder_probe": load(RAW / "builder-probe.json"),
    "builder_limit": "Only existing exact pinned validation container restarted; immediate unsuitable buildctl probe failed, later official CLI probe succeeded. No cause established for prior exit255 or initial installer artifact-guard failures.",
    "artifacts": {"path": "artifacts.json", "sha256": sha(OUT / "artifacts.json"), "regular_files": len(artifacts), "package_regular_files": 97, "manifest_entries": 95},
    "raw_evidence": raw_records,
    "limitations": ["No Git mutations, repository connections or workflow/PR execution", "No runtime agent/container, Smith installation/launch or live migration/restore", "No normal-session manager/keyring/provider/model/GPU validation", "No native graphical/IME/accessibility/multi-monitor or real recipient acceptance", "No clean-host/source provenance, effective lock/hermeticity, license/icon/SBOM or publication clearance", "Historical unexplained installer guard failures remain unexplained; passing reruns are not causal proof", "Owner-writable digests detect drift, not authenticity or human approval"],
}
save("results.json", result)
print(json.dumps({"core_count": len(current), "core_sha256": canonical, "changes_from_actual_baseline": len(changes), "artifacts": len(artifacts), "raw_records": len(raw_records), "results_sha256": sha(OUT / "results.json")}, indent=2))
