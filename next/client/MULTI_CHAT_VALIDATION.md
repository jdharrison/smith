# Multi-chat client self-check

**Implemented client source; self-check, not independent assurance or deployment approval.** Work record: [LOCAL-SMITH-NEXT-MULTI-CHAT](../../docs/plans/LOCAL-SMITH-NEXT-MULTI-CHAT.md). Base `2cc71a94fe924fbb9d324f29332e9f24c8c5991c`, existing `feature/autonomy`, uncommitted combined worktree. Existing UI-redesign/theme/assets/layout work was retained. This workstream changed only `next/client/**`; service, installer, packaging and canonical policy are parent/other-owner scope.

## Implemented contract

- Desktop application envelopes are v2 only. Exact request keys, response envelopes, conversation/operation/detail/lifecycle shapes, IDs, revisions, UTF-8 bounds and explicit nullable fields are checked. Core lifecycle handling is unchanged.
- A bounded FIFO arbiter serializes individual native exchanges. It never holds the helper during polling sleeps. One workspace owns the API; conversation adapters never dispose it.
- Global configuration/current-instance inventory is separate from per-chat draft/history/pending/error/provisional/reading-position state. Unsent drafts are process-local. The original `ChatController` remains an isolated legacy presentation/test seam, not the production controller.
- New reuses one ephemeral view. Atomic `submit_new` acknowledgement or origin/request-bound observation promotes the same controller. Unknown requests are not resent. Verified replies survive failed detail reads; newer drafts survive completion.
- Boot synchronizes index, selected detail and known requests without waiting for generation. Four outstanding chats are centrally observed with a ten-minute queue plus 210-second execution allowance; expiry remains query-only. Inventory missing on reattach is observed only once work becomes idle.
- Existing workspace/sidebar/archive dialog/panel implement target-specific actions. Dialogs capture IDs and revisions before confirmation. Archive/reactivate retain immediate progress dialogs, not extra confirmations. Delete has one explicit logical-deletion confirmation. Rename may run during generation with fresh metadata. Selected archive/delete returns to the preserved blank; restore does not select.

## Initial self-check evidence — historical, superseded by review

All commands ran locally from `next/client`, except Git's repository-wide whitespace check. No aggregate Dagger, model, manager, installed-service, installer or package action was run by this workstream.

| Check | Actual result |
|---|---|
| `flutter analyze --no-pub` | PASS: no issues found |
| `dart format --output=none --set-exit-if-changed lib test` | PASS: 32 files, zero changes |
| `flutter test --no-pub --reporter expanded`, with `SMITH_TEST_SERVICE` explicitly set to the repository's existing `next/service/target/debug/smith-service` | PASS: **200 tests**, no skips; 16 seconds reported |
| `git --no-pager diff --check` | PASS: no whitespace errors |
| Real captured Rust Unix-socket fixture | PASS within full suite; desktop v2 state/detach/restart and separate installer-only exact v1 state shape; production v2 parser rejects the v1 envelope |
| Historical visual references | PASS without updating any golden PNG |

Local full-suite output is retained at `build/multi-chat-final-tests.log` (ignored build evidence). The service binary was already built by the service owner; the client fixture copies it into private temporary roots and runs only captured foreground children. It does not contact the workstation's service or providers.

Tested service binary SHA-256: `59fe345074b98f4ef4a0ed2153368704cdc6e568df54c5f6389b70d47d12e97e`.

Focused new coverage lives in `test/workspace_controller_test.dart` and `test/multi_chat_widget_test.dart`: A/B interleaving, four-job admission, finite observation, idle-only inventory refresh, lost submit/new/lifecycle acknowledgement, provisional reply retention, no newer-draft clearing, shared API lifetime, FIFO exchange serialization including failure, target-only restrictions, stale revisions at dialog opening, Boot during A, distinct drafts/scroll, Settings, and 400×400 at 2× text. Existing keyboard/repeat/IME, native-helper, Boot/Start, recovery, authority, layout and golden tests remain in the passing full suite.

## Intentionally changed fixtures

`FakeAppApi` now implements real v2 DTOs. `LegacyFakeAppApi` is named explicitly and retained only for isolated legacy controller/presentation regressions. Boot/production conversation/Settings fixtures use v2. The Settings long-history fixture now uses 32 messages instead of 40 to respect the frozen 16-turn-pair wire bound; it still asserts independent scrolling and retained reading position. The real Rust fixture uses v2 for desktop state and separately asserts the original v1 installer state keys without routing v1 through the production parser.

The old `fixture-production-shell-1280x720.png` depicts the former single-conversation/no-management contract. Its pixels were **not regenerated**. Its test is now explicitly a historical presentation reference using the legacy `ConversationScreen` seam. Production multi-chat capability, lifecycle and interaction assertions are in the new production-widget tests instead; this historical image is not claimed as a current production screenshot.

## Initial snapshot hashes — historical

| Client source | SHA-256 |
|---|---|
| `lib/app_api.dart` | `e2d55ab544675d2d08bd30c689df04bd678221e0f9bb83282e4838e0898d81dd` |
| `lib/app_v2.dart` | `8abae2c0e9c09072fa2f9b15c0fbca44f574d79ddcec1c2dc5cb55c741b220fe` |
| `lib/workspace_controller.dart` | `0d4264acae6e55879e7a186eb4bd8e32f6fca86373d8e7952b151784d551ebab` |
| `lib/main.dart` | `ff58a4cba2afee07a0bacd05297bfa7f3ed29c413ff51fb9c3f8149b0a2b2ce7` |
| `lib/chat_panel.dart` | `d6a56a470499b967735ce57f32b51527b5395ad7f723bff74e58b86dd7ce21a6` |
| `lib/ui/chat_sidebar.dart` | `41dd51fb48b1b18a6b3e8d84951aa97761817650acfff29bf6b9cf88cb3ffb7c` |
| `lib/ui/archived_chats_dialog.dart` | `cd1f29c0a5f3363d3a8fb26ad1e24b4df45ec30f34d3ec615f33990d12531480` |

## Independent review and remediation

Independent review `2e24cdde-9a2c-4514-9d8e-afe2d27300dc` returned **FAIL**. The initial 200-test self-check and hashes above are historical evidence, not assurance or proof that the review findings were absent. The supplied isolated probes were read and reproduced; permanent regressions are now in `test/assurance_regression_test.dart`.

Implemented remediation within `next/client/**` only:

- Separate acknowledged admission from subsequent observation failure. A failed first poll cannot discard an accepted request identity; it remains uncertain/query-only and stale, with no replay. A definitive rejection of admission itself can still release the unaccepted local request.
- Accept the service's inert migrated submit `speech_error` and succeeded `speak`/`AUDIO_EXPIRED` shape. Voice submission/playback remain unavailable. The legacy transcription result is also strictly bounded and inert.
- Reconcile the latest relevant failed/interrupted submission for each chat even when history detail succeeds. Retained jobs are consumed in the service's admission order; older receipts/outcomes cannot replace a newer success or active job, clear newer drafts, or fabricate prompts.
- Treat lifecycle receipts as historical acknowledgements. Merge only greater metadata/history revisions independently, preserve newer authoritative title/archive state, and never recreate a target absent from the current index.
- Preserve full valid v1 config revisions through the coordinated v2 refinement: safe JSON integer, otherwise canonical decimal string greater than `9007199254740991` and at most `18446744073709551615`. `wireConfigRevision` validates via `BigInt`, while original wire values are echoed unchanged. Other revisions/sequences remain safe integers. V1 stays numeric.
- Require coherent readiness/errors and current-instance inventory, plus complete alternating bounded history pairs while preserving valid legacy empty messages. Production test histories were corrected to valid pairs; no golden references or presentation contracts were changed during remediation.

The Rust fixture now seeds private v1 state for revisions `1`, `9007199254740992` and `18446744073709551615`. It verifies actual Rust migration, inert voice/error results, interrupted jobs visible through the workspace, empty legacy messages, exact v1 numeric tokens, v2 config/operation/poll revision types, and exact submission revision echo rejected at the missing-model gate without admission/inference. It does not modify service sources or installed state.

## Remediation self-check — current source snapshot

Base/branch remain `2cc71a94fe924fbb9d324f29332e9f24c8c5991c` / `feature/autonomy`, uncommitted. No service, installer, packaging or canonical-document writes were made by this client remediation workstream.

| Check | Command / actual result |
|---|---|
| Formatting | `dart format --output=none --set-exit-if-changed lib test` — PASS, **33 files / zero changes** |
| Analyzer | `flutter analyze --no-pub` — PASS, **no issues** |
| Permanent regressions + supplied independent probes | `flutter test --no-pub test/assurance_regression_test.dart build/assurance_probe_test.dart build/assurance_lifecycle_probe_test.dart --reporter expanded` — PASS, **26 tests** |
| Full suite with refreshed backend binary | `flutter test --no-pub --reporter expanded`, with `SMITH_TEST_SERVICE` set explicitly to the repository's refreshed `next/service/target/debug/smith-service` — PASS, **223 tests / no skips**, 16 seconds reported |
| Real Rust fixtures, separately reproduced | `flutter test --no-pub test/rust_socket_integration_test.dart --reporter expanded` with the same explicit binary — PASS, **four tests**, including high and u64-max migration/echo |
| Whitespace | `git --no-pager diff --check` — PASS |
| Golden references | Existing comparisons PASS in full suite; no PNG or golden test modifications during this remediation |

The service owner's refreshed binary was used, not rebuilt or modified by the client workstream. Tested SHA-256: `11c26b704a64e8360d665b7d8f4b344e21bdf9ee9b7f8d7d798748cc70cacab2`. Earlier in remediation, the local suite passed with absent-binary skips and the ordinary-revision legacy fixture passed against the original binary; those partial checks are superseded by this no-skip refreshed-binary run.

Current raw local logs: `build/remediation-focused.log` and `build/remediation-suite.log` (ignored build evidence). The independent build-directory probes were not edited; their cases also have permanent regressions in `test/assurance_regression_test.dart`.

### Remediation source/test hashes

| Path relative to `next/client` | SHA-256 |
|---|---|
| `lib/app_api.dart` | `9a6ad13a0b79dd0bc9346765900b5fa759bfdbd09ae524fac4853f487da95568` |
| `lib/app_v2.dart` | `244be3f11c1466d49cf0eb8c653ff09cad16929eba1dfaf94da47011396b4ccf` |
| `lib/workspace_controller.dart` | `f155aa4f3dc394ce9f72fa04efdb1545a39bc0c5f0bc1bbec533c73d63b890d6` |
| `test/assurance_regression_test.dart` | `99f7a897f3ccb1c64ddadc65c1588c58e4c677d68d4a8ff016ceed6af3265f9d` |
| `test/rust_socket_integration_test.dart` | `0aec031d284b0f85db0275312132cd374fac26afcf0a989cb14a910a1b37307a` |
| `test/app_fakes.dart` | `ca9badbd3bda514fe9cac17b518d30020c3ebfadaca3e58ceeef5b12fa9a35aa` |
| `test/boot_test.dart` | `a837ed6f5e8082ea7a4c7698cc5a6d40cf375c17897eb13bb2570bf36d24223b` |
| `test/conversation_test.dart` | `f2efc6af479650c6063f8eb8bdefab0ef9c0f12329a7c8638d53ca4e7bb12039` |
| `test/multi_chat_widget_test.dart` | `3a157f9d2b65e6f62964e6b7d29a9157aa4318264ab6f4e5d01fc07092814160` |
| `test/settings_view_test.dart` | `933248f1ed80ff3192153c6baf3f8cdcd042d9d72fded7b90de262a01f512cbf` |
| `test/workspace_controller_test.dart` | `3e8876b953597e349691ed8dd8e99095a268f8704b7dd6f74480b77a6e821f8e` |

## Remaining gates

Fresh independent re-review must assess the remediation; passing self-checks do not close the FAIL verdict. Parent-owned aggregate paired service/client/installer/package tests/build, canonical documentation and production visual acceptance remain separate gates. Source tests do not establish real inference, installed GUI behavior, migration rollout, release or deployment readiness. No commits or branches were created.
