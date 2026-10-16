# UI redesign — synthetic visual fixtures

These PNGs are **test fixtures**, not production records, observed service readiness, live inference, or installed-desktop evidence. Mock messages and service/model observations are synthetic. Multi-chat titles and capabilities here use test-only in-memory callbacks. The historically named production-shell fixture mounts the real `ConversationScreen` with `FakeControlApi` and a legacy `ChatController`/`LegacyFakeAppApi`; it is not the current production v2 `SmithApp` composition or its capability acceptance gate. `test/ui_redesign_golden_test.dart` combines the real `SmithWorkspace` and real `ChatPanel` with a private fake controller. No native service/API/network is used by these golden scenarios.

## Accumulated-work cleanup — 2026-10-15

**Implemented test-only cleanup; not independent assurance or release approval.** Base `2cc71a94fe924fbb9d324f29332e9f24c8c5991c`, existing dirty `feature/autonomy` checkout. Production source and assets were read-only; no staging, commits, installation, manager/provider/model operations or feature changes.

The initial full run (`flutter test --no-pub --concurrency=1 --reporter=expanded`) reported **315 passed, 21 failed, 4 skipped**. Failures comprised 13 obsolete pixel comparisons, one ambiguous compact archive finder, and seven stale interaction/copy expectations. The four Rust socket fixtures skipped because no explicit candidate was supplied in that first run.

Test corrections:

- `chat_title_test.dart`: compact title tooltip now asserts `Choose chat:` rather than the wide breadcrumb, preserving the exact 40-scalar display assertion.
- `multi_chat_widget_test.dart`: assert the current stale-action explanation, no service mutation, unchanged target, and removal of the failed confirmation action. Scroll the minimum-size workspace canvas before asserting/tapping Return to Chat.
- `chat_presentation_test.dart`: inject an outage through the implemented 30-second read-only observer, not the removed healthy-header Retry tooltip. Keep draft, caret geometry, no-mutation and navigation checks; scroll Return to Chat into view and assert hit-testability.
- `conversation_test.dart`: observe the seeded expired request through the same timer; follow expanded/compact recovery as rendered. Assert the original pending ID survives Keep pending and only clears after explicit Forget, with the multiline draft unchanged. After Boot's existing model observation, only state/detail/poll calls are permitted.
- `ui_redesign_golden_test.dart`: scope archive lookup to the opened `ChatSidebar`; load the already-bundled MaterialIcons font so the compact chevron is a real glyph, not a missing-font square. Assert one header, no header version/manual Retry, reachable compact actions, and scroll-reachable Settings version. Preserve all recovery no-send/no-retry, draft, identity and explicit-Retry checks.

### Visual verification and bounded refresh

Source contract: [shared header](../../../../../docs/plans/LOCAL-SMITH-NEXT-SHARED-HEADER.md), [compact navigation](../../../../../docs/plans/LOCAL-SMITH-NEXT-COMPACT-WORKSPACE-NAV.md), and [current operator design](../../../../../docs/DESIGN.md). Inspected `SmithAppHeader`, `SmithWorkspace`, `SplitWorkspace`, `ChatPanel` and `SettingsView`, plus old/new rendered images. The standalone synthetic/legacy fixtures intentionally have no Layout owner; current production shared-header/Layout coverage remains in `shared_header_test.dart` and split/compact tests.

Reviewed changes: version/refresh disappear from the app header; its 56px geometry shifts underlying content by one pixel; compact chat headers use the title chooser, New and archive controls; the transcript uses its current full-pane viewport/bounded reading lane. Archive/history/overflow/recovery modals retain their expected content, scrollability and explicit actions. The narrow archive's small difference is the underlying header visible around the modal, not changed archive content. Settings images remain byte-identical. All changed images were opened for visual inspection; this is agent fixture review, not native graphical or human acceptance.

Only the **14 changed** fixtures were regenerated; the two Settings baselines were not touched:

```sh
flutter test --no-pub --concurrency=1 --update-goldens test/ui_redesign_golden_test.dart --name 'historical single-conversation|redesign golden fixture-(chat|archive|overflow|history|recovery)' --reporter=expanded
```

### Validation evidence

All commands run from `next/client`, bounded to at most ten minutes:

| Check | Command / scenario | Final result |
|---|---|---|
| Format | `dart format --output=none --set-exit-if-changed lib test` | PASS, 49 files, 0 changed |
| Analyze | `flutter analyze --no-pub` | PASS, no issues |
| Focused interaction cleanup | `flutter test --no-pub --concurrency=1 test/chat_title_test.dart test/multi_chat_widget_test.dart test/chat_presentation_test.dart test/conversation_test.dart --reporter=expanded` | PASS, 58 tests |
| Golden comparison, no update | `flutter test --no-pub --concurrency=1 test/ui_redesign_golden_test.dart --reporter=expanded` | PASS, 17 tests (16 images + font manifest) |
| Full client suite, no update | `flutter test --no-pub --concurrency=1 --reporter=expanded`, explicit `SMITH_TEST_SERVICE` | PASS, 340 tests, 0 failures/skips |
| Whitespace | `git diff --check -- next/client/test` from repository root | PASS |

The explicit candidate was the pre-existing `next/service/target/release/smith-service`, SHA-256 `e42833da9be3f426b6c79c91e897988e912222c46fa49738425a2003a9d3c0b7`. It was not rebuilt by this cleanup. Socket tests use captured foreground children and private temporary roots; their PASS is candidate-bound compatibility evidence, not a new final-source Rust build or installed-service validation.

Local generated evidence: `next/client/build/client-cleanup-initial-tests.log`, `client-cleanup-focused-final.log`, `client-cleanup-golden-before.log`, `client-cleanup-golden-update.log`, `client-cleanup-golden-final.log`, and `client-cleanup-final-tests.log` (all latter names relative to the same build directory). Pre-refresh baselines are preserved in `next/client/build/client-cleanup-before-goldens/`; comparator old/new/masked/isolated images remain under `next/client/test/failures/`. Build evidence is ignored/generated, not promised durable release evidence. Existing failure artifacts were not deleted.

Current source SHA-256 at final validation (supersedes the historical snapshot below):

| Source | SHA-256 |
|---|---|
| `lib/main.dart` | `d6b5d9585550d6f602a6b9e8134c20191b7e65abfbc75ed4cf7f63299ed71f36` |
| `lib/chat_panel.dart` | `1e0659059e5ca319598390f78d83f0fc1750eab905e37d0fef68fca4efb4cf7a` |
| `lib/ui/smith_app_header.dart` | `dee399f1bddcb282d937c120da0fb8fee760072c10deff7bf480f98915da1fd8` |
| `lib/ui/smith_workspace.dart` | `a7691d7ae79c3d6ca17fefe680075b14c883b7e2dea494b97ac5157dfa331886` |
| `lib/ui/split_workspace.dart` | `b9470813c898883be6cf417d1e30d84aafc678a7f550a9129b2558acd2f4c1c3` |
| `lib/ui/settings_view.dart` | `79d736effd12fc0a7069c4fe6d3644a4eb41c1d2b3ba735a9af5e19d26fb2ca0` |
| `test/ui_redesign_golden_test.dart` | `348bbe1571648d42442ff28e7efb3ad40ed3539725c52735f6c13a5c23ea1636` |

No production defect requiring an out-of-scope edit was established. Linux rebuild, package/release gates, fresh independent review and native window-manager/IME/graphical acceptance remain parent-owned; this cleanup does not claim them.

## Rendering and reproduction

- Ubuntu/Linux Flutter widget renderer; Flutter 3.47.2 / Dart 3.13.2.
- Physical dimensions equal logical dimensions, DPR 1.
- Bundled JetBrains Mono is displayed by default; `fixture-chat-1440x900-ibm.png` uses IBM Plex Mono.
- Golden `FontLoader` registration loads **only Regular** for each family. Inspection of Flutter 3.47.2 `packages/flutter/lib/src/services/font_loader.dart` verified `addFont(bytes)` → `loadFontFromList(bytes, fontFamily: family)` has no explicit weight parameter. This does not prove every previous glyph was semibold, but avoids ambiguous multi-weight dynamic registration in captures. Both Regular TTF OS/2 `usWeightClass` values were independently checked as **400**.
- A test verifies body text defaults to normal and production `FontManifest.json` retains explicit 400/500/600 entries with Regular at 400. Production fonts/assets are unchanged; these widget images are not evidence of production weight-selection fidelity.
- `text2` means a 2.0 linear text scaler; other captures use 1.0.
- No desktop/window decorations. The captured repaint boundary includes modal routes and menus.
- All filenames begin `fixture-` to distinguish them from live evidence.

From `next/client`:

```sh
flutter test --no-pub --update-goldens test/ui_redesign_golden_test.dart
flutter test --no-pub test/ui_redesign_golden_test.dart test/shell_pure_widgets_test.dart
```

Historical initial-generation evidence (superseded by the cleanup results above): **34 checks passed** — 16 golden scenarios, one production font-manifest check, and 17 shell regressions. Scoped analyzer: **no issues**. Screenshots were opened and visually inspected, not accepted only from overflow assertions.

## Capture matrix

| Fixture | Dimensions | Text scale |
|---|---|---|
| `fixture-chat-400x400.png` | 400×400 | 1 |
| `fixture-chat-1440x900.png` | 1440×900 | 1 |
| `fixture-chat-1440x900-ibm.png` | 1440×900, IBM Plex Mono | 1 |
| `fixture-chat-1280x720.png` | 1280×720 native default | 1 |
| `fixture-production-shell-1280x720.png` | 1280×720 legacy ConversationScreen, fake APIs | 1 |
| `fixture-chat-4200x2400.png` | 4200×2400 | 1 |
| `fixture-chat-400x400-text2.png` | 400×400 | 2 |
| `fixture-archive-1440x900.png` | 1440×900 | 1 |
| `fixture-settings-1440x900.png` | 1440×900 | 1 |
| `fixture-overflow-1440x900.png` | 1440×900 | 1 |
| `fixture-archive-400x400-text2.png` | 400×400 | 2 |
| `fixture-settings-400x400-text2.png` | 400×400 | 2 |
| `fixture-overflow-400x400-text2.png` | 400×400 | 2 |
| `fixture-history-400x400-text2.png` | 400×400 | 2 |
| `fixture-recovery-400x400-text2.png` | 400×400 | 2 |
| `fixture-recovery-details-400x400-text2.png` | 400×400 scrollable recovery modal | 2 |

## Observations and limitations

Implemented shell refinement: 1px outlined main/history panels, 6px radii, 8px workspace gutter and panel separation, individually outlined chat rows separated by 4px, selected accent edge, accent Chat breadcrumb, approximately 56px desktop header. The 400px/100% header now fits on one row. Short-window breadcrumbs ellipsize without reducing font size; full titles remain in semantics/tooltips. Archive/Settings headings use section typography to preserve useful space at enlarged text.

Target fixtures contain 12 active synthetic chat titles plus three archived entries; management callbacks are fixture-only. The production-shell capture instead verifies one actual `Current conversation` presentation entry, absent New/Archive/row-mutation controls, only inspect/state/models/poll fake calls, and disposal without native effects. Its unsupported selection action is rendered as readable text, not a fake disabled button, and the Chats header retains its minimum height.

The selected Chat/Settings rail has a 48px outlined square using selected surface/stroke tokens; keyboard focus uses a distinct brighter 2px border. Chat's semantic label stays `Chat`, with the compact action hint/tooltip `Open chat history`. Status retains the parent's exact label and adds a tooltip/semantic hint explaining last-observed connectivity, not continuous monitoring, model readiness or completed work.

Desktop captures show both role directions and a bounded reading/composer lane. Ultrawide keeps that lane centered rather than stretching text. Narrow captures intentionally begin at the newest retained content, following the real ChatPanel's initial-scroll behavior. Dialog actions and Send remain reachable and no RenderFlex overflow was reported.

**Final small2 inspection:** the current compact recovery surface shows the factual `Needs attention` control rather than a cropped error excerpt. Opening it presents a scrollable Recovery details modal; its initial screenshot shows the beginning of the full error, with Close visible and further guidance/Retry available by scrolling. The golden scenario asserts the complete error string is present, scrolls to a hit-testable Retry without overflow, verifies zero retry/send effects while opening/scrolling/closing, preserves the child/draft, and verifies exactly one callback only after explicit Retry.

The composer intentionally bounds its viewport and scrolls longer wrapped drafts. Seeing one complete line rather than the whole draft is not by itself a clipping defect. Settings and archive collections likewise require scrolling at this geometry. Final regenerated small2 and desktop screenshots were inspected; the previously recorded recovery-excerpt finding is superseded by the compact-details flow. Independent acceptance and installed UI validation remain separate gates.

The supplied composition was implemented from the approved geometry/visual contract; these are not a pixel-diff comparison to a checked-in reference image.

## Historical source snapshot (superseded)

Captured against worktree based on `2cc71a94fe924fbb9d324f29332e9f24c8c5991c`, including the current uncommitted integrated presentation work after the chat-owner handoff. SHA-256 at final comparison:

| Source | SHA-256 |
|---|---|
| `lib/main.dart` (read-only dependency) | `c0856325f07a91a85b186fd396f25f12a454ff0c88e0d118432bf23dfe6f72a4` |
| `lib/chat_panel.dart` (read-only dependency) | `2d244fb9582bb94ab5ef365601fe4a02c747c569a7ce073d8d45684b7f8e62a2` |
| `lib/boot.dart` (read-only dependency; Boot not captured) | `ba21232a1d2d902c13c5184ef22d1ad080e031546f36b9e6e219739316f00bff` |
| `lib/ui/smith_workspace.dart` | `55c93d6d61b8b3e5d9df2093ff5b36b900f998fc54f23696d48fffab489e97b0` |
| `lib/ui/chat_sidebar.dart` | `11b9c8afdca18a95404af63efb78a173af8e5eaedf08c916e6a81e1aef6b9df7` |
| `lib/ui/archived_chats_dialog.dart` | `abf529ac590c4bb0955a981c653291ea5a4ce46730bb66f1d1cf4ce9ce00dade` |
| `lib/ui/settings_view.dart` | `b8381a58f5e00f7dfa2a7935729160b59654618ef1c228060a29092d79e21887` |
| `test/ui_redesign_golden_test.dart` | `d96e328cb20b7ec032de3a92a1acefcc40bd55a3ee978f97a8384e1ddb2f01e2` |
| `test/shell_pure_widgets_test.dart` | `ea7b2f522a9d616fd46051863160ab7ab3c309ac5b41a7349a95037134529b39` |
| `lib/ui/smith_theme.dart` (read-only dependency) | `fac3af932fbc5e179e7a5a6edcc6b3c21be40277b814fea3aa4da2c2765c9ae7` |
| `lib/ui/smith_icon.dart` (read-only dependency) | `dd44720657e5d640bfeb82c91985cf4f06090dfa85c9210250caa6256689989e` |

Main/ChatPanel/Boot hashes were recorded before generation and after the final non-update comparison and were unchanged. All 16 goldens were regenerated against this source snapshot. Formatting and scoped analysis of the golden test passed; comparison plus shell regressions passed all 34 checks. Regenerate and reinspect after upstream presentation/theme/font changes; these are reproducible widget-fixture baselines, not installed-desktop or independent human signoff.
