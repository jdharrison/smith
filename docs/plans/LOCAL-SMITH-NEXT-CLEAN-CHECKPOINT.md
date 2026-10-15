# Local clean checkpoint

## Approval binding and ownership

**Approved local intent, not product acceptance:** the current user request asks to prepare a clean local commit of accumulated Smith work. Repository: `smith`; existing branch: `feature/autonomy`; base/HEAD: `2cc71a94fe924fbb9d324f29332e9f24c8c5991c`. The target is the final parent-reviewed accumulated worktree, not a fabricated clean baseline or a revision already created. The user's current request includes a local checkpoint commit in this session only; it authorizes no push, PR, tag, release, installation, service/model operation, deployment or accepted-risk decision. Approval expires when this session ends or the repository, branch, base, intended effects or agreed checkpoint scope materially changes. The parent must bind the final selected files, validation/review results and target identity immediately before any separately executed local commit.

**Evidence worker's delegated scope is evidence hygiene only.** It may add `docs/plans/LOCAL-SMITH-NEXT-CLEAN-CHECKPOINT*` and `docs/evidence/clean-checkpoint/`, preserve original GPU logs under ignored `next/service/build/`, and sanitize the explicitly authorized GPU plan logs. It must not stage or commit, edit code/tests/lockfiles, run tests/builds, change `.gitignore`, alter historical evidence manifests or hashes, or update canonical documentation or `next/VALIDATION.md`. The parent owns ignore cleanup, final source validation, independent review and commit execution. Existing dirty work is preserved. This is routine documentation/evidence maintenance supporting elevated source changes; it grants no new runtime capability or policy exception.

Canonical authority: [Process evidence contract](../PROCESS.md#evidence-contract), [approval boundaries](../PROCESS.md#approval-boundaries), [Scope](../SCOPE.md) and [agent instructions](../../AGENTS.md). The preceding read-only inventory supplies the producer need (a portable, privacy-safe checkpoint), architecture constraint (preserve original evidence identity and existing authority boundaries), and operator constraint (do not confuse source self-checks with live acceptance). This hygiene worker does not claim independent assurance of its own changes or re-review of the accumulated implementation.

## Requirements and acceptance traceability

| Requirement | Bounded implementation | Hygiene acceptance evidence |
|---|---|---|
| CP1 Preserve original evidence | Leave ignored build/target originals unchanged; back up GPU originals before sanitizing versionable copies | Original-to-derivative SHA-256, byte counts, retained-original locations and verification |
| CP2 Retain required supporting records | Curate four launch logs, all 72 redesign evidence files, preserved multi-chat logs and practical client combined results | Manifest entries and explicit missing/unresolved reference list; no duplicate screenshot exports |
| CP3 Remove sensitive locator content | Sanitize private home paths, cloud trace locators, credential/header/signature candidates and terminal control sequences | Per-file transformation counts and post-write pattern scans; no sensitive matched contents in reports |
| CP4 Keep historical claims truthful | Do not edit old manifests, original digests, source snapshots, outcomes or failure history | New mapping distinguishes original bytes from sanitized derivatives and documents relocated historical logs |
| CP5 Preserve implementation/ownership | No staging, commit, code/test/ignore/canonical edits or test execution | Scoped file-hash/index checks and parent handoff |
| CP6 Bounded portable evidence | Keep the curated derivative bundle below 20,000,000 bytes | Manifest sizes and existing fixture references rather than duplicate captures |

## Retention and sanitization contract

[Bundle index](../evidence/clean-checkpoint/README.md) and [mapping manifest](../evidence/clean-checkpoint/manifest.json) identify every retained derivative, original repository-relative path/digest, retained original location, transformation categories/counts, and expected-digest reference resolution. Paths in the manifest are repository-relative; private prefixes and trace locators are never stored as redaction examples. Original ignored files are local retained evidence, not content available in a clean clone. The manifest is an owner-controlled integrity/relocation record, not independent provenance, signature authenticity or proof of continuous source stability.

GPU logs at their existing versionable paths become explicitly mapped sanitized derivatives only after original bytes are copied into ignored `next/service/build/clean-checkpoint-originals/`. Old manifests/reviews continue to identify the original bytes; their hashes are not rewritten to assert that the sanitized copy is identical. Reviewers must resolve those old references through the new mapping. Sanitization is not test execution or a successful replacement for a failed gate.

Existing installer screenshot fixtures, per-image metadata, source manifests, current checksum index and historical retention records remain in `next/installer/tests/fixtures/ui-redesign/`; client golden fixtures remain in `next/client/test/goldens/ui-redesign/`. Do not duplicate, prune, regenerate or relabel those captures as current production screenshots. Previously documented pruning is not repaired by invented files. Unrecoverable references, if any, must be explicit in the new manifest.

## Parent-owned source gates and limitations

The source checkpoint combines GPU startup/fallback corrections, service-owned multi-chat/storage/protocol changes, client polling/send/title/Settings/split-workspace/preferences changes, installer presentation/explicit launch and packaging changes. Their separate plans/reviews remain authoritative for scope and historical outcomes. The hygiene mapping does not rebind older source PASS verdicts to subsequently changed production inputs.

Before claiming combined source acceptance, the parent owns final-source Rust formatting, warning-denied Clippy, locked build/tests; Flutter dependency/analyzer/unit/widget/golden and Rust-Dart socket checks; isolated native GTK/no-interpreter gates; actual-ELF package checks, paired build/export verification; whitespace validation and fresh independent architecture/assurance review of consequential changes and remediations. Use the commands required by [Process](../PROCESS.md#required-gates) and each slice's plan on one recorded stable snapshot. No source gates are executed by this hygiene worker, and existing failed or incomplete combined runs remain failures/incomplete evidence until the parent supplies a separately bound disposition. A local checkpoint is not evidence that those gates passed.

Supplied icon redistribution rights remain unresolved; bundled OFL notices and provenance do not establish rights to supplied SVG/PNG artwork. Full license/SBOM/provenance clearance, native graphical/keyboard/IME/accessibility/fractional-scale QA, clean-host installation, live manager/login/reboot/maintenance and real model/GPU inference remain separate limits and approval gates. No release/deployment or accepted-risk approval follows from a clean local commit. Do not trigger the legacy release workflow as a shortcut.

## Rollback and handoff

Rollback of this hygiene slice means removing only its new derivative/index records and, if explicitly requested, restoring a GPU log from its verified ignored original. Do not reset accumulated work, delete original logs, clean ignored trees, remove diagnostic failures, rewrite historical manifests or alter unrelated files. Preserve the mapping with any retained derivative.

## Final parent-owned checkpoint disposition

The user's explicit local-commit instruction binds to this accumulated source on the existing branch and base for this session. Final selection includes source/tests/assets/lockfiles, canonical docs/plans, licensing/provenance notices, reviewed synthetic baselines/capture evidence and sanitized validation records. It excludes generated failure images, Python bytecode and build/dist/target outputs via `.gitignore`; all excluded bytes remain locally preserved. No source or evidence is discarded to obtain a clean status.

Cleanup changed only stale client test interactions/expectations and visually reviewed obsolete golden baselines, plus one reproduced production defect: duplicate automatic desktop scrollbars on the split canvas. New Linux overflow/drag/count tests failed before the fix and pass afterward; automatic bars are suppressed only around the canvas, with inherited behavior restored inside tiles. Independent client-test/golden review PASS; independent integrated architecture review recommended the checkpoint and cleared the scrollbar remediation. A stale golden README fixture label was corrected; no behavior was changed for that documentation fix.

Final gate results (retained commands, exit records, source inventories and artifact/log hashes in [final mapping](../evidence/clean-checkpoint/final-manifest.json)):

| Gate | Outcome |
|---|---|
| Final isolated client test/build | 341 passed, 0 failed/skipped; format49 unchanged; analyzer clean; paired Linux release/export91 files PASS |
| Isolated service | 169 passed, 1 controlled ignored; format and warning-denied Clippy PASS |
| Isolated installer | 259 native +9GTK passed, 2 controlled ignored; format/Clippy and interpreter-free CLI runtime PASS |
| Actual-ELF package gate | 41 passed; relevant source groups unchanged by final client-only fix |
| Stable final input snapshot | 1,167 next/ entries unchanged during final client run; no HEAD/index mutation by validation workers |

All enabled tests above pass; ignored probes are explicit finite fixtures, not skipped Rust↔Dart coverage. Historical failures remain recorded rather than rewritten as successes. Sanitized historical and final evidence maps preserve original hashes and transformations, and independent final staged review must verify the selected bytes before commit. No push, release, deployment, installation, live manager/model/GPU or normal-host UI action is authorized or performed.

Nonblocking follow-ups remain separate: successful v2 mutation/admission Rust↔Dart wire integration using a fake provider, native graphical/IME/window-edge acceptance, installed-host lifecycle/real inference and redistribution/provenance clearance. They are not claimed as passing by this local source checkpoint. Rollback is a future explicit source revert, never deletion of user state or diagnostic evidence.

The final hygiene verification record is [LOCAL-SMITH-NEXT-CLEAN-CHECKPOINT-HYGIENE.json](LOCAL-SMITH-NEXT-CLEAN-CHECKPOINT-HYGIENE.json). It reports observed counts, scope checks, scans and exceptions only; final validation/review/commit identity remains parent-owned.
