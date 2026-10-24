# Fresh-profile Smith starter — implementation and blocked combined validation

**Implemented fresh-only behavior; bounded feature-source assurance PASS; overall assurance and combined quality/acceptance gate BLOCKED.** The operator selected an ordinary editable/deletable project, then explicitly selected **new profiles**. [Approved plan](../../plans/LOCAL-SMITH-DEFAULT-PROJECT.md). No backfill, protected built-in, UI redesign or contribution executor is authorized.

## Implemented behavior

Only a safe absent application-state read invokes the explicit service bootstrap. It creates this ordinary record, with a normal generated opaque project ID:

- Name **Smith**, fixed agent `opencode`.
- One repository: Name null, URL `https://github.com/jdharrison/smith`, Base/Work `main`/`main`, all four type leaves null in their required object.
- Initial application/registry revision1 and one content-free internal receipt with an ordinary metadata fingerprint; no expired IDs.

Full state validation/capacity preflight and encoded-state bounds precede private0600 no-clobber publication before readiness. Shared empty defaults, existing load/migration paths, schema/API/Project shapes, validators and normal CRUD remain unchanged. An appeared destination causes failure without replacement/adoption/retry. A publication failure is not proof that no bytes became visible.

Existing current, legacy, empty and full profiles receive no starter. Normal edits, rename, repository removal and confirmed deletion persist across restart, even after receipt eviction; empty registry never reseeds. Removing the entire state file creates a genuinely absent profile again. Production Flutter, selection/order/New form behavior and global shell/theme/fonts are unchanged by this task. Public repository metadata is a contribution setup starting point—not a checkout, permission grant, installed agent, configured test environment, Git/PR executor or automatic project-context export. No live profile was initialized/modified, and no product launch, install, commit, branch/remote action or live provider/model/keyring/manager/repository operation occurred.

## Source and ownership

| Identity | Binding |
|---|---|
| Branch / HEAD | `feature/autonomy` / `14d960975e76f906efb5e447ff3dece0e8e4385b` |
| Actual pre-task baseline |1,994 source/policy/evidence records; canonical SHA256 `aeb1ce67ebb10c0af866313901b7060a921018be60fd267901323a4d69d56ca2` |
| Baseline archive | `target/default-project/baseline-source.tar.gz`;1,316 source/policy files, prior evidence bytes excluded; SHA256 `6073eea510d781fb3acd83aa9afe11c93ce57851bf7fd6c07d1cc3e132a1c6b8` |
| Validated/reviewed executable core |1,275 records/33 untracked; SHA256 `e51a718bbc9d990622ff64e3847e7bae095416d66f344fb48fb7e6be5e8d12d9` |
| Actual rebuilt compiled fixture | `target/default-project/validation/debug_service-fixture`; SHA256 `79b38c181580772aec9c8d97ea20008f211787cbc68d85023b573b8eae6e7737` |
| Candidate core archive | `target/default-project/validation/candidate-core.tar.gz`; SHA256 `6290a3abceb7a4bba342a00818a5bc0886524b52622de48396d1c20bf536ce51` |

[Baseline records](baseline-manifest.json), [final core](validation/core-manifest-final.json) and [independent review](REVIEW.md) bind the actual dirty/untracked worktree, not HEAD alone. Core and evidence selectors differ; digests must not be equated across shapes. Observed stability is not a lock against future edits. Concurrent Global Context source/schema7 already belongs to other work; this starter adds no schema change. Older disk6 documentation/fixture reconciliation and the combined Global Context contract remain with that work, not silently repaired here.

Feature changes: `service/src/app_projects.rs` explicit bootstrap, exact fresh `None` arm in `service/src/app.rs`, seven additions to `app_projects_tests.rs`; one fresh compiled Dart case plus explicit existing-empty preparation for the seven original cases; only isolated `dart_cloud` compatibility/control hunks in `service/tests/debug_service.rs`. Frozen disk5 fixture construction excludes unused synthetic disk7 instruction data only after assertions and retains exact unrelated-state comparisons. No public migration or validation exception was added. Affected canonical documentation describes the initialization exception separately from blank ordinary forms.

## Parent-owned recorded checks

[RESULTS](validation/RESULTS.md), [exact commands](validation/COMMANDS.md), [machine results](validation/final-results.json) and [checksums](validation/SHA256SUMS) retain sanitized argv/environment, times/exits/budgets, logs, source and artifact identities. Official Dagger v0.21.9 checksum `609fad4ab7ec88bd62ed79dbb4baf479154f7df4047149476b8f854a8f5f04af` and the existing running pinned engine were verified; incompatible PATH Dagger was not used. Dependencies/setup caches and configured pipeline egress were used; no cold-cache/hermetic build claim.

| Check | Actual outcome |
|---|---|
| New Rust `projects_bootstrap` cases |7 passed, no ignored cases |
| All Rust Projects cases |24 passed /5 failed; required failure |
| Production library Clippy / service format |PASS |
| All-target warning-denied Clippy |FAIL: instruction-test `field_reassign_with_default`, line21 |
| Newly rebuilt real Rust↔Dart Projects |8 passed, no skips |
| Exact Rust fixture controls |1+1 passed |
| Projects view/navigation/layout/golden/controller/protocol group |168 passed /1 failed,169 total; no skipped cases reported |
| Asset cases / full Flutter analyzer / Dart format |5 passed /PASS /PASS (100 files,0 changes) |
| Once-only root `make … test build` |FAIL exit2 at service Clippy in31.696s; parallel results canceled, build not reached |
| Current-package verification and real-package helpers |NOT RUN; no successful current rebuild |
| Diff whitespace check |PASS |

Do not sum overlapping focused/reviewer cases as unique aggregate coverage. Canceled vertices are not suite passes. Existing service/client/installer exports stayed unchanged and are **stale for this candidate**, not current-package evidence.

## Bounded blame, preserved failures and independent assurance

The five Rust failures concern existing schema6/legacy/configuration/instruction fixtures. A valid control overlays **only** exact pre-task `app.rs`, `app_projects.rs`, `app_projects_tests.rs` on the otherwise current service snapshot and compiles using a distinct initially absent Cargo target. It runs17 pass/five fail, with all five normalized failure bodies identical to the candidate, and reproduces the same instruction-test lint. [Control comparison](validation/negative-control-comparison.json). An earlier shared-target control reused candidate artifacts and is explicitly invalid; its original logs remain retained and are excluded from causal evidence. This proves independence from this feature under that control, not a universal historical attribution or permission to waive gates.

The Flutter navigation failure expects no `TextField` after switching to Settings but sees the three current Global Context fields. It is retained as a required failure; no baseline causal attribution was established. No unrelated test, lint, schema policy or implementation was changed/suppressed to obtain a pass.

[Service owner](service/RESULTS.md) and [integration owner](integration/README.md) retain their compile/fixture/analyzer failures and corrections, source archives, commands, source-drift limitations and scoped successes. The parent rebuilt and reran after those changes; old fixture binaries were not counted as current passes.

[Fresh independent assurance](REVIEW.md) verified the actual delta and all167 parent evidence checksum entries. It independently passed the unchanged seven bootstrap cases and five private-copy adversarial probes (receipt conflict/eviction/delete, whole-state removal, unsafe/appeared destinations and synthetic post-link uncertainty). The fault-hook result is not a real hardware/fsync-failure experiment. It reproduced the five broader Rust failures and retained two exploratory app_v2 failures with unverified causal attribution. Earlier reviewer sandbox/group-writable-ancestor harness failures remain recorded; host permissions were not weakened. **Bounded feature-source PASS; overall assurance FAIL / combined BLOCKED / release NO-GO.** No feature-owned defect was identified, and no accepted risk or external approval was granted.

## Remaining gates

Concurrent-owner reconciliation of the instruction/schema fixtures, lint, navigation assertions and older schema documentation; a fresh stable combined-source full component/root run; rebuilt paired package verification/helper evidence; whole-candidate assurance and human signoff. Installed/native graphical/IME/accessibility/restore, live provider/model/keyring/manager behavior, clean-source provenance/SBOM/imported-icon rights/publication/deployment remain separate authorized gates. No current release/build-package acceptance is claimed.
