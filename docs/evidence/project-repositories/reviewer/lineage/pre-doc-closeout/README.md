# Structured project repositories

**Implemented configuration-only extension; final configured local gates PASS.** The independent verdict and final source/evidence binding are recorded separately in [REVIEW](REVIEW.md). This is not installed migration, native graphical acceptance, release or deployment approval.

Work record: [LOCAL-SMITH-PROJECT-REPOSITORIES](../../plans/LOCAL-SMITH-PROJECT-REPOSITORIES.md). HEAD/branch at discovery and final gates: `14d960975e76f906efb5e447ff3dece0e8e4385b` / `feature/autonomy`. The actual baseline includes the prior **uncommitted** Projects foundation and separate concise-documentation cleanup. A preserved archive/manifest covers1,357 baseline source/policy files; independently verified content is not replaced by `git diff HEAD`. Prior [Projects evidence](../projects/README.md) remains unchanged and historical.

## Implemented slice

Repositories are ordered objects rather than URL strings:

```json
{
  "name": "backend",
  "url": "git@example.invalid:team/backend.git",
  "branch": {
    "base": "main",
    "work": "dev",
    "types": {
      "feature": "feature/*",
      "fix": "fix/*",
      "release": "release/*",
      "hotfix": "hotfix/*"
    }
  }
}
```

Every object/key is required. Name and the six branch/type leaves are independently nullable. Blank optional UI fields serialize null, not example defaults; no preset is silently applied. URL-only legacy entries retain their exact URLs/order and unknown names/branches. Names and URLs are independently unique within a project by exact case-sensitive comparison; multiple unnamed entries remain valid.

The editor exposes **Name (optional)**, **URL**, **Base branch**, **Work branch** and collapsed **Branch types**. All eight fields participate in dirty detection, immutable submission capture and current-instance/revision review. Save/Cancel, draft-only Add/Remove, readable migration/unknown states, query-only uncertainty and exact-target deletion remain. Fixed OpenCode information, mounted chats and preference boundaries are unchanged.

App3/Core2 remain. Disk6 freezes and strictly migrates disk1–5 without inventing configuration or discarding receipts/fingerprints/expired IDs. Current domain operations are `projects_list_v2`, `project_create_v2`, `project_update_v2`, `project_delete_v2` and unchanged `project_receipt`. Old list/writes always reject with `UPGRADE_REQUIRED`, including empty-array updates, deletes and historical mutation requests; old receipts remain queryable and IDs remain protected across versions. No client fallback is supplied.

This is branch configuration—not branch matching, creation, checkout, PR routing or workflow execution. No repository connection, local root/mount, credentials, provider/model context, agent installation, container execution, Dev page or additional project metadata was added.

## Validation

Final aggregate UTC window: **2026-10-17 19:29:55–19:36:31**. Exact argv, environment overrides, timestamps, exit codes and log digests are in [results.json](results.json).

```sh
python3 -B target/project-repositories/run_gates.py
# Recorded children, from repository root:
make DAGGER=target/installer-pipeline/smith-dagger-0.21.9/dagger test build
python3 -B dagger/package.py --root installer/dist --verify
SMITH_TEST_PACKAGE=installer/dist python3 -B -m unittest discover -s scripts/tests -p 'test_*.py' -v
git --no-pager diff --check
```

The recorder resolves the package variable to the actual absolute repository package, represented as `<repo>/installer/dist` in sanitized evidence. All four children exited0 without timeout. Official Dagger v0.21.9 CLI SHA256: `609fad4ab7ec88bd62ed79dbb4baf479154f7df4047149476b8f854a8f5f04af`; explicit module and existing isolated engine were used, with no cloud token or beta fallback.

| Suite | Passing executions | Failures | Controlled ignores | Other skips |
|---|---:|---:|---:|---:|
| Rust service |295|0|3|0|
| Native installer |283|0|2|0|
| Isolated GTK |9|0|0|0|
| Flutter, including compiled fixtures |834|0|0|0|
| Actual native package |41|0|0|0|
| **Aggregate** |**1,462**|**0**|**5**|**0**|
| Separate actual-package pipeline helpers |34|0|0|0|

Dart formatting covered90 files with zero changes; analyzer, locked warning-denied Rust Clippy/tests, native/Linux builds, isolated Xvfb GTK and no-Python Ubuntu CLI runtime passed. Test execution vertices were fresh; dependency/build layers reused cache. Counts are test executions, not a claim of distinct semantic scenarios or an uncached/hermetic build. Service quality uses configured Rust1.98 Bookworm; Ubuntu installer/GTK/Flutter/runtime evidence is distinct, not supplemental Ubuntu service proof.

The standalone compiled Rust↔Dart Projects fixture passed **7/7** on final source: structured CRUD/restart, lost acknowledgement without replay, historical receipt after deletion, stale writers/instances, actual disk5 migration without invented settings, legacy erasure/shared-ID refusal and branch-only uncertain-update recovery. Synthetic inference/HTTP/keyring counters stayed zero. This corroborates coverage already included in834 Flutter passes and is not added to the aggregate.

## Independent finding and preserved failures

Fresh assurance found Dart JSON decoding silently collapsed duplicate nested reply members before strict Projects validation. The fix preflights **decoded member-name uniqueness per object** and a64-container depth bound before whole-document `jsonDecode`, retaining the frame/correlation and uncertain-mutation boundaries. Unicode-escaped aliases, nested arrays/objects and punctuation-bearing strings are covered. The original three failed repros pass unchanged; additional independent parser and neighboring tests passed. Architecture follow-up found no supported payload needing deeper nesting and no blocking resource concern; the depth cap is not a heap quota or hostile-same-UID sandbox.

The retained failure record distinguishes:

1. **Focused parent integration:** first migration assertion incorrectly compared volatile inventory across restart. The test now explicitly asserts cache reset and compares the unchanged durable projection. Original failing source/log/result remain under `focused-attempt-1/`.
2. **Developer self-checks:** initial compile/style/current-schema assertions were corrected. Two initial installer artifact-guard failures later passed without unrelated installer repairs; their cause remains **unestablished**. The owner's record is a retrospective transcript summary, not an original log/source-bound run, and says so explicitly.
3. **Reviewer tests:** original duplicate-key failures and two reviewer harness setup mistakes are preserved; the repro source was not weakened or overwritten.
4. **Aggregate attempt1:** failed before component tests because the existing pinned Dagger engine was stopped. No cancelled/unexecuted stage is counted as a pass. The original stop cause is unknown; inspect reported prior exit255 without OOM.
5. **Builder recovery:** only the same existing builder was started, with unchanged image/configuration. An immediate unsuitable `buildctl` probe failed, then the bounded official CLI/module probe succeeded. The builder is validation infrastructure, not an OpenCode/runtime worker or a Smith service. No image replacement, permission broadening, host installation or daemon reconfiguration occurred.

Passing later checks do not retroactively explain or erase earlier failures. Original timestamps/logs/source bindings are retained where available; missing historical logs or timestamps are not invented.

## Binding and retained artifacts

- [results.json](results.json): commands/outcomes, baseline/current source identities, preserved failure records and raw-file digests.
- [core-source-manifest.jsonl](core-source-manifest.jsonl): **1,243** final core inputs, including the new parser regression test and untracked Projects sources. Before/after/current canonical-array SHA256: `ed6dc46ef3938f7753ffb9a7a0c76e6be1a2ea88ed0ff760835610fa7cd53481`.
- [supporting-inputs.jsonl](supporting-inputs.jsonl): final workflow/helper/policy/approval documentation snapshot. This is post-gate documentation binding, not a retrospective claim that documentation stayed unchanged throughout the build. Independent assurance additionally bound helper/package inputs during its supplemental rerun.
- [changes-from-baseline.json](changes-from-baseline.json): selected core/control changes against the preserved actual working-tree baseline, not unrelated pre-existing changes against HEAD.
- [artifacts.json](artifacts.json):189 service/client/package file records, paired service/client equality, current packaged-guide equality and exact package identities. The package verifies95 manifest-listed entries/97 regular files. Ignored exports are local artifacts, not published releases or clean-source provenance.

Sanitized raw scripts/logs/manifests and baseline archive remain under ignored `target/project-repositories/`, including both aggregate attempts and reviewer evidence. Selected reviewer repros and final bindings are retained with [REVIEW](REVIEW.md). Ignored output is not a Git backup or public retention guarantee. Owner-writable hashes detect drift, not authenticity or human approval; retain an independent digest anchor before consequential external acceptance.

## Remaining boundaries

No Smith launch/installation, live state migration/restore, normal-session manager/keyring/provider/model/GPU operation, runtime agent/container run, Git mutation, push/PR/merge/release or deployment was performed. Starting the existing approved Dagger validation builder is explicitly recorded above. Native graphical/IME/accessibility/multi-monitor, real recipient/model/provider acceptance, installed disk6 rollout/restore, clean-host/source provenance, effective dependency locking, license/icon rights, SBOM and publication remain separately authorized gates. Old binaries reject disk6; binary-only downgrade is not rollback.
