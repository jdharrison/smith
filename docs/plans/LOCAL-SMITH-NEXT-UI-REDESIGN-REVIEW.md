# UI redesign — scoped independent review

Initial review date: 2026-10-14; final evidence closure follows the recorded 2026-10-15 UTC gate run. Base: `2cc71a94fe924fbb9d324f29332e9f24c8c5991c`, branch `feature/autonomy`, uncommitted worktree. This resumes the same independent assurance context; the prior review context was available.

## Scope

- Review the UI source/package slice and the subsequent stale-menu authority remediation, not repository history or unrelated legacy behavior.
- Implemented production scope remains the existing single conversation, read-only current-conversation history entry and Connection & About, with existing Send/recovery behavior. Unsupported multi-chat mutation callbacks are absent.
- Multi-chat management is target/fixture-only. This record does not pass full multi-chat acceptance, authorize publication/deployment, or accept residual risk.
- No implementation, Settings, golden, or other documentation edits were made by this reviewer. Only this expressly authorized review record was written. Tests used local widget fixtures; no real service, host GUI, network, installation or commit was used.

## Findings and closure

### Critical / high

No blocking production code regression identified in the scoped inspection and reproduced checks. The prior final-build/package evidence hold is now closed for this implemented source/package slice after independent identity verification below. Full redesign acceptance remains outside this closure; the remaining matrix, multi-chat, publication and live-operation gates are not passed.

### Medium — prior stale-menu finding CLOSED for the reviewed presentation contract

`next/client/lib/ui/chat_sidebar.dart:203–254` now maintains an interaction revision. Changes to ID, archived state, busy state or callbacks invalidate previously captured interactions. Revocation followed by restoration does not revive an old confirmation. `_available` checks mounted state, revision, current busy state, stable ID, archived state and current action availability. `_dispatch` validates and invokes the current callback without an asynchronous gap.

`chat_sidebar.dart:257–313` revalidates after menu selection and supplies guarded callbacks to confirmation/immediate-action dialogs. Removed anchors can report an unavailable action without dispatch. `next/client/lib/ui/archived_chats_dialog.dart:214–239` distinguishes an authority rejection (nothing sent) from an unconfirmed mutation, retains duplicate-submission protection and performs no automatic replay.

Reproduced all 27 cases in `next/client/test/shell_action_authority_test.dart` as part of the 82-test run below: busy/revoked open menus for all four actions; identity/archive/removal/controller changes; Rename/Delete confirmation changes; title-only changes preserving original stable IDs; revoke/restore rejection. No callback effects or exceptions in the rejected scenarios. These checks close the prior null-unwrap/stale-dispatch concern at the widget boundary; they do not prove backend admission, authorization or persistence.

### Low — nonblocking future integration limitation remains

`next/client/lib/chat_panel.dart:38–60` resets the one-shot initial-scroll state when the controller is replaced. Reproduction scenario: scroll controller A upward in long history, replace A with B, then return to A through the same panel state; initial scrolling targets the newest content rather than restoring A's previous reading offset. This is a source-derived limitation, not an additional executed test in this resumption.

Current production does not switch conversations; Settings/drawer round-trips retain the same panel and pass. Per-conversation reading-position restoration belongs to the real multi-chat handoff and must not be inferred from listener-rebinding tests.

### Low — historical capture metadata, not final source identity

The hashes in `next/client/test/goldens/ui-redesign/README.md` describe a historical capture snapshot, including pre-authority-fix action files. They must not be presented as final current source hashes; parent-owned clarification remains separate. Fresh non-update golden comparison passed against the fixed source during this review. Final implementation/test/asset hashes are bound by `LOCAL-SMITH-NEXT-UI-REDESIGN-EVIDENCE.json` and independently checked below. No golden README or other document was edited by this reviewer.

## Required Checks Evidence

Commands below ran from `next/client` unless stated otherwise. Expected outcome for automated checks: all selected assertions pass / no diagnostics or whitespace errors.

| Check | Command/scenario | Actual result | Status / evidence |
|---|---|---|---|
| Authority, shell, chat, Settings and golden regressions | `flutter test --no-pub test/shell_action_authority_test.dart test/shell_pure_widgets_test.dart test/chat_presentation_test.dart test/settings_view_test.dart test/ui_redesign_golden_test.dart --reporter expanded` | 82 passed; includes all 27 authority cases and 16 non-update golden scenarios | PASS, exit 0; review tool transcript and named test files |
| Analyzer | `flutter analyze --no-pub` | No issues found | PASS; analyzer output in review transcript |
| Whitespace | `git --no-pager diff --check` | No errors | PASS, combined analyzer/diff command exit 0 |
| Protected controller/protocol/service diff | `git --no-pager diff --stat 2cc71a9 -- lib/text_chat.dart lib/app_api.dart ../service` | No delta | PASS, exit 0; review transcript |
| Snapshot identity | `sha256sum` of the seven files below, checked twice during resumption | Identical on both checks | PASS, exit 0; table below |
| Visual inspection | Opened the ten PNGs listed below | No new blocking composition defect identified in these captures; small-window recovery/Settings/archive content uses scrolling | Scoped visual inspection complete, not installed-desktop evidence |
| Final Dagger candidate checks | Parent: `DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make client/test client/build package/test installer/build` from `next` | Recorded exit 0, no timeout; 176 Flutter tests, 40 package tests, clean analysis, 28 formatted files / zero changes, Linux client and installer-package builds successful | PASS evidence inspected in `LOCAL-SMITH-NEXT-UI-REDESIGN-EVIDENCE.json`; not a reviewer rerun of Dagger |
| Exported package verification | Reviewer: `PYTHONDONTWRITEBYTECODE=1 python3 dagger/package.py --root installer/dist --verify` from `next` | Native recipient package verified: 94 files; reviewed text-only payload paths | PASS, independently reproduced, exit 0 |
| Source inventory verification | Reviewer Python SHA-256 comparison of recorded before/after maps, current files and Git tracked/nonignored file selection with recorded exclusions | 170 recorded before/after entries identical; current selection exactly matches; 169 current hashes match and only `next/client/README.md` differs after documentation update | PASS for validated implementation/test/assets; documentation drift explicitly accounted for below, exit 0 |
| Artifact identity verification | Reviewer Python checks of exact file sets, regular-file type, size, mode and SHA-256; canonical JSON inventory digests; paired payload equality; manifest equality | 90 client files and 96 installer-package files match evidence; all 90 client files equal paired-client payload; actual manifest equals retained content and hash, with 94 entries | PASS, independently reproduced, exit 0 |

Earlier independent review evidence, not rerun in this resumption: 84 scoped Flutter checks passed; package unittest suite ran 40 checks with 34 passed and six native-executable checks skipped; all 23 recorded asset hashes matched, and a limited scan of 15 SVGs found only checked geometry elements without href/event attributes. This does not substitute for the final paired build/package or native integration gates.

### Screenshots actually inspected in this resumption

All under `next/client/test/goldens/ui-redesign/`:

- `fixture-production-shell-1280x720.png`
- `fixture-chat-1440x900.png`
- `fixture-chat-1440x900-ibm.png`
- `fixture-chat-4200x2400.png`
- `fixture-chat-400x400-text2.png`
- `fixture-recovery-400x400-text2.png`
- `fixture-recovery-details-400x400-text2.png`
- `fixture-archive-400x400-text2.png`
- `fixture-settings-400x400-text2.png`
- `fixture-overflow-400x400-text2.png`

Desktop captures preserve both role directions, bounded message width and anchored composer. Ultrawide adds whitespace instead of stretching message text. The production-shell capture has one readable current-conversation entry and no management controls. At 400×400/200%, the composer shows a complete line within its scrollable viewport; recovery exposes Needs attention and a scrollable details dialog. Close is visible initially, and the reproduced golden scenario scrolls to a hit-testable Retry, with no operation from opening/scrolling/closing and one explicit Retry callback. A partially visible long document in a scrollable viewport is not by itself a clipping defect. Settings and archive captures likewise intentionally require scrolling.

These are synthetic DPR-1 widget captures, not observed service readiness or actual desktop/font-weight-selection evidence. Full fractional/DPR-2, threshold and wide-short matrix closure was not established by this resumption.

## Reviewed snapshot

Paths relative to `next/client`; SHA-256:

| File | Hash |
|---|---|
| `lib/ui/chat_sidebar.dart` | `535e0ae1e4370312b02f8a16b3155fb570c63ee9c859975977b36c326745a2cd` |
| `lib/ui/archived_chats_dialog.dart` | `0f5e93c44d67116af0777a401a1ba043f200337fa5895f91e8187d1104a5cbf0` |
| `lib/chat_panel.dart` | `2d244fb9582bb94ab5ef365601fe4a02c747c569a7ce073d8d45684b7f8e62a2` |
| `lib/main.dart` | `c0856325f07a91a85b186fd396f25f12a454ff0c88e0d118432bf23dfe6f72a4` |
| `lib/ui/settings_view.dart` | `b8381a58f5e00f7dfa2a7935729160b59654618ef1c228060a29092d79e21887` |
| `test/shell_action_authority_test.dart` | `63b12f68727927d26b7bdc3753ddfa355e43fb708e723a65849f0ad2ff0f5af3` |
| `test/ui_redesign_golden_test.dart` | `d96e328cb20b7ec032de3a92a1acefcc40bd55a3ee978f97a8384e1ddb2f01e2` |

## Final evidence and identity closure

Inspected `docs/plans/LOCAL-SMITH-NEXT-UI-REDESIGN-EVIDENCE.json`, SHA-256 `6464f78de5b2623af72cbd21256e0a0f1f43293fbe2b060c3bb7a50442626116`. Both recorded commands exited 0 without timeout. The Dagger command ran from 2026-10-15 00:03:41.978123 UTC through 00:06:11.521289 UTC, followed by successful package verification. Retained output reports 176 Flutter tests and 40 package tests passing without a skipped-test suffix. The six skips in the earlier local synthetic-only package run are not the final actual-ELF gate result. Repeated Dagger progress output is not additional execution; synthetic package fixtures have 87 entries, whereas the actual candidate has 94 manifest entries. Raw Dagger logs were not retained; this review inspected the structured command/results evidence and independently checked artifacts, rather than claiming to have rerun the full Dagger invocation or reconstructed all raw logs.

Independent read-only verification established:

- Current Git base remains `2cc71a94fe924fbb9d324f29332e9f24c8c5991c` on `feature/autonomy`.
- Recorded before/after source maps are identical, contain 170 entries and have canonical JSON inventory digest `659cc5fb2270e1a26b195c731ac13332b21923d925b5b75b4cabba3fd9780906`. Re-enumerating tracked/nonignored files with the recorded exclusions yields the same membership, with no missing or extra files.
- **Documentation correction:** `next/client/README.md` is included in that validation inventory, contrary to the handoff shorthand that all updated documentation was outside it. Its current hash differs after the parent's documentation-only update; the other 169 current files match. The inspected README diff documents the presentation and exclusions and does not modify executable validation inputs. DESIGN/PROCESS/plan documentation is outside this inventory. Thus 170 entries were unchanged during the recorded run, not 170 current files unchanged afterward. Do not rewrite historical evidence to hide this distinction. Any subsequent golden README clarification is likewise documentation drift, not a newly validated source snapshot.
- All 90 files under `next/client/dist` match recorded bytes, mode and SHA-256, with exact file membership. Canonical inventory digest: `f54ab0d09c8983cfcd3d967b4180777f3da4048647b7bc7ef3a704762d09d3ba`.
- All 96 files under `next/installer/dist` match recorded bytes, mode and SHA-256, with exact file membership. Canonical inventory digest: `c2099add9bd3ccfaae272b8c21a1951b09dec048a693a609f4a83809278b72ce`.
- All 90 standalone-client file records equal the corresponding paired-client payload records. Actual `manifest.json` matches the retained JSON content and its recorded SHA-256; its 94 entries exclude the manifest/checksum files themselves. Independent package verification checks content/modes, exact payload policy, paired service identity and checksum list and passed.
- Packaged `version.json` contains version `0.1.0`, build number `1`; widget-harness Development build is not the packaged release metadata.

Identity checks used local Python standard-library hashing/stat/JSON and read-only Git enumeration; package verification executed the reviewed builder verifier, not a packaged installer or service. No clean-clone, hermetic-build, signature/authenticity or installed-runtime claim follows from matching hashes. Dagger may reuse cached steps.

## Evidence Gaps and separate gates retained

1. Full fractional/DPR-2, adaptive-threshold and wide-short visual/accessibility matrix closure remains unverified here. The verified named DPR-1 screenshots/scenarios are not full visual acceptance. This gate is explicitly outside the present source/package closure, as retained in the updated plan and process documentation.
2. Per-conversation scroll restoration and real multi-chat contract/admission/mutation/storage integration remain separate. Fixture management is not implemented production functionality or full multi-chat acceptance.
3. Icon redistribution rights remain explicitly unresolved in `next/client/assets/README.md:11–17`; publication is not authorized or cleared.
4. Live GUI/service/model/installation checks and release/deployment approval remain excluded. This review neither ran them nor waives them.
5. Full raw Dagger logs were not retained; the final record contains selected result lines, counts, exit statuses, timing and source/artifact bindings. The Dagger gates are evidence-verified, while focused widget/golden checks and exported-package identity/policy checks were independently reproduced.

## Verdict

**Remediation assessment:** prior stale-menu authority finding closed; reproduced scoped tests and non-update goldens pass; no new blocking production code defect identified.

**Scoped assurance verdict: pass — implemented UI source/package slice only.** The prior pending final-verification hold is superseded by inspected successful gate evidence and independently verified current implementation/artifact identities and exported-package verification. No blocking finding remains within this bounded closure. This is not full redesign/visual-matrix acceptance or acceptance of residual risk; the separate gates above remain open.

**Release Gate Decision: no-go.** Source/package review closure grants no multi-chat acceptance, icon redistribution clearance, live-operation permission, publication, deployment or human approval. Required follow-up is completion of the explicitly retained separate gates before claiming their acceptance. Preserve this snapshot binding and re-review consequential implementation changes.
