#!/usr/bin/env python3
"""Bind layout-only closeout to preserved baseline, final core and documentation."""
import datetime
import hashlib
import json
from pathlib import Path
import re
import stat
import subprocess

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
PREFIX = "docs/evidence/projects-layout/"
CORE = "f00306ef6fb418e4c3a53b12261599cc290ede874d12822e5a647e58fdf54164"
BASE = "740cc41d9ba4bc8265d5ea69a34fe5bf5ac8e11596ef4223357232a6756a44c3"
PACKAGE = "d168fdf131402ee099b516ea752fb510c8b86f2eece1c7aca6b4f0e0e048f725"


def canonical(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def load(relative):
    return json.loads((ROOT / relative).read_text())


def record(relative):
    path = ROOT / relative
    info = path.lstat()
    assert stat.S_ISREG(info.st_mode), relative
    data = path.read_bytes()
    return {"path": relative, "size": len(data), "mode": stat.S_IMODE(info.st_mode),
            "sha256": hashlib.sha256(data).hexdigest()}


def same(expected):
    current = record(expected["path"])
    return all(current[key] == expected[key] for key in ("size", "mode", "sha256"))


def git(*args):
    return subprocess.check_output(["git", "--no-pager", *args], cwd=ROOT, timeout=15)


baseline = load(PREFIX + "baseline-source-manifest.json")
assert canonical(baseline["files"]) == BASE
allowed = {
    "client/assets/README.md", "client/lib/ui/projects_view.dart", "client/lib/ui/smith_icon.dart",
    "client/test/projects_navigation_test.dart", "client/test/projects_view_test.dart",
    "dagger/package.py", "dagger/tests/test_package.py", "docs/DESIGN.md", "docs/PROCESS.md",
    "docs/SMITH_LINUX_ARCHITECTURE_REPORT.md", "docs/plans/LOCAL-SMITH-PROJECTS-EDITOR-LAYOUT.md",
    "VALIDATION.md", "docs/VALIDATION_DETAILS.md",
}
changed = [entry["path"] for entry in baseline["files"] if not same(entry)]
assert set(changed) <= allowed, changed
original_paths = {entry["path"] for entry in baseline["files"]}
paths = set(filter(None, git("ls-files", "--cached", "--others", "--exclude-standard", "-z").decode().split("\0")))
additions = paths - original_paths
omitted = {".gitattributes", ".gitignore", ".smith/project.yaml", "LICENSE", "SECURITY.md"}
for path in omitted:
    assert (ROOT / path).read_bytes() == git("show", "HEAD:" + path), path
new_source = {"client/assets/icons/layers.svg", "client/assets/icons/git-branch.svg",
              "client/test/projects_layout_test.dart", "client/test/projects_layout_golden_test.dart"}
new_source.update(path for path in additions if path.startswith("client/test/goldens/projects-layout/") and path.endswith(".png"))
assert len(new_source) == 12
assert not additions - omitted - new_source - {path for path in additions if path.startswith(PREFIX)}
historical = [entry for entry in baseline["files"] if entry["path"].startswith(("docs/evidence/projects/", "docs/evidence/project-repositories/"))]
goldens = [entry for entry in baseline["files"] if entry["path"].startswith("client/test/goldens/") and entry["path"].endswith(".png")]
assert len(historical) == 127 and all(same(entry) for entry in historical)
assert len(goldens) == 28 and all(same(entry) for entry in goldens)

core_ref = load(PREFIX + "validation-rebound/core-manifest-final.json")
core = load(core_ref["manifest"])
assert record(core_ref["manifest"])["sha256"] == core_ref["manifest_file_sha256"]
assert canonical(core["files"]) == core["sha256"] == core_ref["sha256"] == CORE
assert len(core["files"]) == 1265 and all(same(entry) for entry in core["files"])
gates = load(PREFIX + "validation-rebound/final-results.json")
review = load(PREFIX + "review-closeout/review-summary.json")
artifacts = load(PREFIX + "validation-rebound/final-artifact-binding.json")
assert gates["result"] == "PASS" and gates["stable_inputs"]
assert review["verdict"] == "pass" and review["source_review_gate"] == "go"
assert review["independently_checked_gate_core_sha256"] == CORE
assert artifacts["installer_dist_tree_sha256"] == PACKAGE
assert review["independently_checked_package_tree_sha256"] == PACKAGE

markdown = sorted(path for path in allowed if path.endswith(".md")) + [PREFIX + "README.md"]
links = []
for path in markdown:
    for target in re.findall(r"!?\[[^\]]*\]\(([^\s)]+)\)", (ROOT / path).read_text()):
        if "://" in target or target.startswith(("#", "mailto:")):
            continue
        target_path = target.split("#", 1)[0]
        if target_path:
            assert (ROOT / path).parent.joinpath(target_path).exists(), (path, target)
            links.append({"source": path, "target": target})

scope = {"baseline_sha256": BASE, "baseline_files": len(baseline["files"]),
         "changed_baseline_paths": changed, "out_of_scope_changes": [],
         "new_source_files": [record(path) for path in sorted(new_source)],
         "omitted_baseline_paths_verified_against_HEAD": sorted(omitted),
         "unchanged_historical_evidence_files": len(historical),
         "unchanged_preexisting_golden_PNGs": len(goldens),
         "core_files_reverified": len(core["files"]), "core_sha256": CORE,
         "local_markdown_file_links_checked": links, "anchor_validation": "not performed"}
result = {"work_id": "LOCAL-SMITH-PROJECTS-EDITOR-LAYOUT", "approved_slice": "0",
          "recorded_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
          "status": "implemented; local configured gates and independent bounded-source review pass",
          "branch": git("branch", "--show-current").decode().strip(),
          "HEAD": git("rev-parse", "HEAD").decode().strip(), "baseline_sha256": BASE,
          "core_sha256": CORE, "package_tree_sha256": PACKAGE,
          "aggregate_coverage": 1497, "fresh_component_passes": 1214,
          "cached_installer_passes": 283, "fresh_helpers": 34,
          "controlled_ignores_fresh": 3, "controlled_ignores_cached": 2,
          "other_skips": 0, "final_required_failures": [],
          "gate_record": record(PREFIX + "validation-rebound/final-results.json"),
          "review_record": record(PREFIX + "review-closeout/review-summary.json"),
          "review_report": record(PREFIX + "REVIEW-CLOSEOUT.md"),
          "artifact_record": record(PREFIX + "validation-rebound/final-artifact-binding.json"),
          "approval": "local implementation/testing only; no external effects or accepted risk",
          "native_installed_release_acceptance": False,
          "failures_preserved": ["validation/", "review/", "remediation/", "validation-final/",
                                 "review-final/", "validation-closeout/"]}
for name, value in (("scope-audit.json", scope), ("results.json", result)):
    with (OUT / name).open("x") as handle:
        handle.write(json.dumps(value, sort_keys=True, indent=2) + "\n")

identities = sorted(allowed | {"AGENTS.md", "docs/SCOPE.md", "docs/ARCHITECTURE.md", "docs/HOST_PROFILE.md",
                    PREFIX + "README.md", PREFIX + "REVIEW-CLOSEOUT.md", PREFIX + "results.json",
                    PREFIX + "scope-audit.json", PREFIX + "record_closeout.py",
                    PREFIX + "baseline-source-manifest.json", PREFIX + "ui-owner-validation.json",
                    PREFIX + "asset-owner-validation.json", PREFIX + "review-closeout/evidence-manifest.json",
                    PREFIX + "validation-rebound/evidence-index-final.json"})
files = [record(path) for path in identities]
binding = {"recorded_at": result["recorded_at"], "files": files,
           "canonical_files_sha256": canonical(files), "core_sha256": CORE,
           "package_tree_sha256": PACKAGE, "independent_final_docs_binding": "separate reviewer record",
           "no_approval_or_risk_waiver": True}
with (OUT / "closeout-binding.json").open("x") as handle:
    handle.write(json.dumps(binding, sort_keys=True, indent=2) + "\n")
print(json.dumps({"result": "PASS", "core_files": 1265, "changed_baseline_paths": changed,
                  "historical_files_unchanged": len(historical), "golden_PNGs_unchanged": len(goldens),
                  "local_file_links_checked": len(links), "closeout_sha256": binding["canonical_files_sha256"]}, indent=2))
