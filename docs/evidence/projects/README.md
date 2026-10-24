# Projects metadata foundation

**Implemented metadata-only source slice; configured local gates PASS.** Independent assurance and its final source/evidence binding are recorded separately in [REVIEW](REVIEW.md). This is not installed migration, native graphical acceptance or release approval.

Work record: [LOCAL-SMITH-PROJECTS](../../plans/LOCAL-SMITH-PROJECTS.md). The approved starting point was `c1339f3670c70582db05a158b46821353662b87b` on `feature/autonomy`. Concurrent, unrelated runner work was committed separately as `14d960975e76f906efb5e447ff3dece0e8e4385b` before the final gates. The shared candidate includes that runner repair, but it is not a Projects implementation/review claim. This task performed no commit, branch operation or remote mutation.

## Implemented boundary

Chat / Projects / Settings exposes persistent project names, optional repository locators and fixed per-project `opencode` metadata. There is one service-owned registry and one desktop-wide controller, with per-view volatile selection/editor state. Explicit Save/Cancel and confirmed deletion affect metadata only. Projects remain independent of chat jobs, provider context and presentation preferences.

App3/Core2 remain unchanged. Disk5 explicitly migrates disk1–4 with an empty registry and preserves existing histories/configuration/cloud references/provenance/timestamps/receipts. Older binaries cannot read disk5. Presentation preferences strictly read v1/v2 and write v2; project data/editor content are never stored there. Any installed rollout/rollback requires separately approved compatible state and paired binaries, including exact keyring-generation relationships.

No repository probing/cloning/path access, project conversation, credentials, models, prompt templates, tasks, OpenCode/Docker installation, containers or agent execution are supplied. The agent label is not an installed/ready status.

## Recovery review and preserved failures

Fresh assurance initially blocked dirty editors/open confirmations silently adopting a refreshed instance at the same revision, and uncertain receipts preventing current-list observation. The fixes bind captured instance **and** revision, preserve dirty input until explicit review, and allow query-only Refresh/Resolve without clearing pending identity or enabling writes on unknown/expired receipts.

A subsequent independent probe found that a confirmed deletion resolved after restart left a dirty deleted project selected. Confirmed deletion plus authoritative absence now reconciles before dirty-origin preservation; absence alone or a historical acknowledgement while the target is present cannot discard the editor. The original repros were not weakened. Independent before/fail/after evidence and final verdict are in [REVIEW](REVIEW.md).

The first full gate remains **failed**: a pre-existing cloud test's expected persisted schema was still4 after this slice introduced5. Its actual5/expected4 assertion was corrected, the focused test passed, and the final full cloud suite passed34/34. That first run stopped before build/package/helper stages; parallel cancelled tasks are not counted as passes. Its original recorder, manifests and log remain under `target/projects-validation/attempt-1/`, hash-bound in [results.json](results.json).

An earlier checksum-verification command contained a mistyped expected CLI digest and failed before gate execution. The corrected verification and both recorded gate attempts use the actual official v0.21.9 digest below; the PATH beta CLI was not substituted. An initial evidence-export parser also rejected ANSI-colored summary lines; stripping colors only in parser input fixed export without changing raw logs or rerunning tests. That exporter failure record and explicitly labelled source reconstruction are retained.

## Commands and results

The passing second attempt (05:52:54–05:57:58UTC) is preserved under `target/projects-validation/attempt-2/`. A concurrent simplification of root `README.md` and packaged `dagger/INSTALLER.md` arrived during closeout. The source-binding exporter correctly refused that drift: only the packaged guide differed among the1,242 core inputs. Those edits were preserved, and the complete configured gates/build were rerun for the final snapshot. The generated package's `README.txt` now exactly matches the current guide. Repeated gate passes are not added to unique aggregate coverage.

Actual final UTC execution: **2026-10-17 06:07:39–06:13:16**. Exact commands, timestamps, exit codes, input identities, environment overrides and log hashes are in [results.json](results.json).

```sh
python3 -B target/projects-validation/run_gates.py
# Recorder children, from repository root:
make DAGGER=target/installer-pipeline/smith-dagger-0.21.9/dagger test build
python3 -B dagger/package.py --root installer/dist --verify
SMITH_TEST_PACKAGE=installer/dist python3 -B -m unittest discover -s scripts/tests -p 'test_*.py' -v
git --no-pager diff --check
```

The recorder resolves the package variable to the absolute verified repository package. The four commands exited0 without timeout. It used the existing isolated engine `docker-container://smith-installer-pipeline-dagger-v0.21.9`, explicit module selection and no cloud token. Official CLI SHA256: `609fad4ab7ec88bd62ed79dbb4baf479154f7df4047149476b8f854a8f5f04af`.

| Suite | Passed | Failed | Controlled ignores | Other skips |
|---|---:|---:|---:|---:|
| Rust service |289|0|3|0|
| Native installer |283|0|2|0|
| Isolated GTK |9|0|0|0|
| Flutter, including compiled fixtures |773|0|0|0|
| Actual native package |41|0|0|0|
| **Aggregate** |**1,395**|**0**|**5**|**0**|
| Separate actual-package pipeline helpers |34|0|0|0|

The1,395 aggregate tests have current non-CACHED execution output. Dependency/build vertices used cache; this is not a clean-cache/hermetic build. Formatting (89 Dart files/zero changes), analyzer, locked warning-denied Rust Clippy/tests, Linux build, isolated GTK and no-Python Ubuntu CLI runtime gates passed. Service tests use configured Rust1.98 Bookworm; Ubuntu24.04 installer/GTK/Flutter/runtime checks are distinct, not an additional Ubuntu service proof. The helper archive test uses a synthetic source identity and is not clean-source release provenance.

The parent also ran the actual compiled Rust↔Dart Projects fixture separately: **4/4 passed**, covering CRUD/restart, lost acknowledgement across restart without replay, a historical create receipt after deletion, stale writers/instances and refusal to adopt an old editor instance after refresh. Synthetic local-inference/HTTP/keyring counters stayed zero. This repeats coverage already included in the773 Flutter passes and is not added to the aggregate. The fixture was built with locked/offline Cargo; exact cached-SDK argv/environment, executable/input hashes and output are recorded in `target/projects-validation/compiled-projects.json` and [results.json](results.json).

## Binding and retention

- [results.json](results.json): source/gate/command/compiled-fixture identities, outcomes, limits and raw evidence hashes, including the preserved failed attempt.
- [core-source-manifest.jsonl](core-source-manifest.jsonl): all**1,242** core path/hash/mode/size records. Before/after/current canonical array SHA256: `5226a499e634e06d284f14b841c061d71c470cb903eedba95f228c40d9779d23`. This includes untracked Projects code/tests and the shared runner source, not just `git diff`.
- [artifacts.json](artifacts.json): all generated service/client/package file identities, exact paired service equality and package manifest identity. `installer/dist/` contains97 regular files,95 manifest-listed entries; outputs are local ignored build artifacts, not published distributables.
- [supporting-inputs.jsonl](supporting-inputs.jsonl): supplementary final workflow/helper/approval/policy snapshot. It is taken after the aggregate, not a claim of before/after stability for documentation edited during it. Final review separately binds the completed documentation.
- Sanitized raw gate scripts/logs/manifests remain under ignored `target/projects-validation/`; independent repros/logs/bindings remain under `target/projects-assurance/`. These are local evidence, not a Git backup or public log retention. Retained hashes detect ordinary drift, not adversarial authenticity; external acceptance still needs an independently retained anchor.

## Remaining boundaries

Source/fake/widget/golden/compiled-socket/build evidence does not establish real graphical/IME/multi-monitor/accessibility acceptance, live manager/keyring/provider/model/GPU operation, installed disk5 migration/restore, clean-host qualification, license/icon/SBOM/provenance clearance or deployment readiness. No host application/installer was launched and no installation, push, PR, merge, release or deployment was performed by this task. Dev/container execution needs its own approved design and implementation slice.
