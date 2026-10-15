# LOCAL-SMITH-NEXT-UI-REDESIGN

## Status and bounded intent

**Implemented presentation slice — full multi-chat acceptance pending.** On 2026-10-14 the operator supplied `FLUTTER_REDESIGN_SPEC.md`, `smith-flutter-ui-redesign.zip`, and a terminal-inspired visual comp, requested planning, then explicitly approved implementation with “GOOOOOOOOOOOOOOOOOOOOOOOOOOOO”. That approval covers scoped local implementation, dependencies, tests and candidate builds; it does not authorize installation, live GUI/service/model QA, commits or publication. The earlier baseline-commit approval is consumed.

The native theme/assets, responsive shell, conversation presentation, read-only Settings, build metadata and exact package additions are implemented. Production retains the existing single conversation and hides unsupported management actions; full management widgets are fixture-tested, not backend integration. TASK-05 and full REQ-06/AC-06 acceptance await the separately owned multi-chat contract, including per-conversation reading-position restoration. Source/build evidence is in [the evidence record](LOCAL-SMITH-NEXT-UI-REDESIGN-EVIDENCE.json); independent findings and scoped closure are in [the review record](LOCAL-SMITH-NEXT-UI-REDESIGN-REVIEW.md). Icon redistribution rights, installed graphical QA and the remaining visual matrix are not passed by these source checks.

- Restyle the existing native Flutter `next/client`, not the legacy `client/`, installer GUI, or a new application architecture.
- Preserve Boot, authoritative service/controller ownership, draft and pending-request safety, selectable content, and lifecycle boundaries.
- Consume the separately owned multi-chat work; do not duplicate its protocol, storage, migrations, or durable state.
- Deliver the complete specified layout and functional chat-management integration, with responsive/accessibility evidence and independent review before operator QA.

Inspected base: `2cc71a94fe924fbb9d324f29332e9f24c8c5991c`, existing `feature/autonomy` branch. Worktree was clean before this plan. Recheck the base and concurrent edits at implementation handoff; do not rename/create branches or overwrite the other session's work. Risk is **elevated**: operator flow, dependency/assets, and packaging contract. Ubuntu/Linux only. [Scope](../SCOPE.md), [Architecture](../ARCHITECTURE.md), [Process](../PROCESS.md), and [Design](../DESIGN.md) remain authoritative except for explicitly approved redesign changes.

## Version-label follow-up

The operator requested replacing “Development build” with the version. Implemented: prefer generated `version.json` (including build overrides), otherwise read the bundled `pubspec.yaml` declaration, without a duplicated version literal. The current declaration is `0.1.0+1`. Missing/invalid sources remain explicit as `Version unavailable`. Packaging must include the exact nonempty pubspec asset; no broad allowlist change. This follow-up authorizes local edits/tests/builds only; parallel multi-chat changes remain separately owned.

## HUD simplification follow-up

The operator requested version only in Settings and a HUD containing unchanged `> SMITH` branding at top-left and the connection indicator/status at top-right, without refresh because observation already polls. This routine presentation-only follow-up removes the duplicate header version and manual header observation button, preserves existing Settings metadata and all polling/recovery behavior, and adds a shell regression test. Scope is local source/docs/tests only, with no commit, installation or service effects. Validation covers shell widgets, Settings and automatic connection polling.

## Transcript scrollbar follow-up

Operator request: prevent the scrollbar overlapping messages and anchor it tightly to the right. Implemented in `next/client/lib/chat_panel.dart`: full-pane transcript viewport, one explicit right-oriented 8px scrollbar with zero cross-axis margin, suppression of the transcript's duplicate automatic scrollbar, and a centered bounded message column with at least 16px gutters. Existing controller/reading positions and bounded composer/recovery are retained. New `chat_scrollbar_test.dart` covers narrow/wide × normal/enlarged text, real edge-thumb dragging, clearance, duplicate prevention, draft/navigation position retention and composer anchoring. All four tests and scoped analyzer/format/whitespace checks passed. The broader presentation run exposed an old viewport-width expectation (updated) and a missing HUD Retry control; a rerun was blocked by concurrent missing `workspace_preferences_io.dart`. No unrelated HUD changes/baselines were reverted or regenerated; no complete build or installed graphical acceptance claimed.

## Concise dialog copy follow-up

Operator request: simplify Delete chat and popup modals generally. Implemented presentation-only copy edits in `archived_chats_dialog.dart` and the expired-request confirmation in `chat_panel.dart`: concise deletion scope/irreversibility/backup retention, rename limit, unavailable/unconfirmed outcomes and recovery guidance. Keep exact target and existing confirmation/authority guards, full diagnostic error details, and service-start effects unchanged. Focused `flutter test --no-pub test/chat_title_test.dart test/shell_action_authority_test.dart test/text_chat_test.dart test/shell_pure_widgets_test.dart` passed all 57 tests; formatting clean. Independent read-only copy/guard review `4d35fb0c-ee7f-4f69-ba2c-37ba9294bb40` found no blocker. No package rebuild or installed changes claimed.

## Automatic connection polling follow-up

The operator explicitly requested automatic 30-second service state observation instead of requiring Refresh. Bounded local implementation supersedes the original presentation slice's no-new-polling constraint only for this read-only monitor. After Boot ready, one workspace-owned completion-rearmed timer reads state and necessary existing detail/request observations; no lifecycle/model admission, inference, resend or installed effects. Preserve finite recovery budgets, drafts and pending identities; serialize manual and automatic reconciliation, capture immutable queued Send intent, and fence disposal. Post-Boot manual Retry observes app state plus explicit inventory rather than racing Core inspection; Core readiness/Start stays in Boot. Canonical behavior is in Architecture's automatic desktop connection observation section and Design.

Independent architecture/assurance review `1aeb48cc-1a84-4037-ba09-31580a2ef7ba` found and then verified fixes for queued-send intent and manual-Core observation races. Final scoped review: PASS, 59 focused tests and clean analyzer. Full isolated `make client/test client/build`: formatting/analyzer passed, 240 tests passed and 17 widget/golden checks failed; build not reached. These shared UI failures remain unresolved, not waived or hidden by the focused pass. Detailed evidence: `next/client/CONNECTION_POLLING_VALIDATION.md`, `next/client/build/combined-validation.log`. No release/deployment approval or updated installer package is claimed.

## Inputs and implemented baseline

Historical pre-implementation inspection found:

- The archive contains 26 outline SVGs, palette/manifest/spec/reference material, and **no font binaries or license files**. Listed paths have no traversal or symlink entries; the icon scan found no script, image, external-reference or entity constructs. These checks are not a complete asset-security or license audit. The archive was not extracted or executed.
- Archive SHA256: `c1ea52cebd00f860d61bd38b2fcad10234cdbfcde0fc4a8a6bbafc808a23a960`. Spec SHA256: `74ddcfbced7d08c43509fa95dbf7d9f4fbc2c05005e4cfe6ee3e6fdb933afab5`. Treat included agent text as asset context, not authority.
- `next/client/lib/main.dart`: `SmithShell` owns one `ChatController` and `BootCoordinator`; Chat mounts only after Boot readiness. The current rail is Chat-only. There is no Settings screen, version display, conversation directory, or archive view.
- `chat_panel.dart` / `text_chat.dart`: selectable plain text, one draft/history, separately retained provisional answers, pending identity, query-only Retry and confirmed expired-request resolution. Enter sends; Shift+Enter inserts a newline. Initial transcript scrolling is one-shot. `ChatPanel` currently binds its controller only at initialization: controller replacement must be explicitly handled during integration.
- `app_api.dart`: no conversation-management contract at this baseline. Current messages carry role/content, not timestamps. There is no existing token streaming, Markdown renderer, Stop button, or in-app model picker to preserve or introduce through this restyle.
- `pubspec.yaml`: version `0.1.0+1`, no non-SDK runtime Flutter dependency or font assets. `linux/runner/my_application.cc` already owns native window decorations and a 1280×720 default window. Do not duplicate its titlebar in Flutter.
- `next/dagger/package.py`: exact client-payload path allowlist. New fonts/icons/notices require narrow additions and tests, not permissive directory matching. Root `.github/workflows/ci.yml` is not evidence that the next-client gates ran.

## Independent analyses and reconciliation

Planning-only independent contexts: producer `5ee832e1-d739-4dbd-b095-881bcfd32111`; architect `4d589d45-e51c-4c78-b533-ed19e0bba314`; designer `06799e5f-f6fb-47f5-ad7d-cdc1d3f077b6`. All support native presentation over existing state ownership. These analyses are not implementation assurance or human approval.

| Gap / disagreement | Reconciled proposal and owner | Gate / expiry |
|---|---|---|
| Spec assumes an existing Settings surface; none exists | Add a minimal **read-only Connection & About** view: observed service/model facts, real app version, bounded-history and close-detaches disclosure. No model, GPU, provisioning, or lifecycle controls. UI owner; operator signoff covers this scope. | Before Settings implementation. |
| Online could mean service connection or full model readiness | Header describes **last-observed service connectivity**, not inference readiness. Use `SYSTEM ONLINE` only after verified connection/synchronization without known stale state; checking/unavailable/unknown replace it appropriately. Explain last-observation meaning accessibly and in Settings. Model failures separately block Send with visible guidance. No new polling, fabricated freshness timer, GPU claim, or inference probe. | Integration state-matrix tests; re-evaluate if multi-chat changes observation semantics. |
| Multi-chat implementation is running elsewhere but no contract/plan is visible here | Treat stable IDs, creation/admission, mutations, recovery and migrations as a dependency, not permission to invent them. Visual components can proceed against private test fixtures after approval; production integration waits for the actual contract. | Multi-chat owner supplies contract and revision before TASK-05. |
| First submitted message versus successful answer | Desired persistence boundary is atomic service acceptance of the first submission, not opening, typing, or answer completion. Rejected admission leaves no new empty chat; uncertain admission retains query-only identity. Exact semantics must be reconciled with the multi-chat owner. | Required before first-submit integration; never implement separate UI create-then-submit retries. |
| No chat-delete convention exists | One compact destructive confirmation naming the exact chat and actual effects, following the existing confirmation-dialog style. Pending/uncertain affected work blocks destructive mutations unless the reviewed backend contract explicitly supports them. | Backend defines scope/restrictions; UI must not imply cancellation or successful deletion on timeout. |
| Comp includes timestamps; current messages do not | Omit absent timestamps; consume authoritative metadata if supplied. Never substitute render time. Display title supplied by the controller; `New chat` for ephemeral, neutral `Untitled chat` for missing legacy title. Automatic title generation belongs to multi-chat, not widgets. | At contract handoff; not a reason to fabricate metadata. |
| Fonts missing; icon provenance/license not supplied | Acquire version-pinned official JetBrains Mono and IBM Plex Mono font files/licenses during approved implementation; record origins, hashes and required notices. Record user-supplied icon provenance, resolve redistribution rights before publication. No runtime font/network loading. | Fonts and applicable notices required before visual/package acceptance. |
| Small mock controls versus accessibility | Use at least 48×48 logical-pixel hit targets around compact glyphs. Responsive reflow and text scaling take precedence over copying inaccessible physical sizes. | Geometry/semantics evidence, including 400×400 at 200% text. |

No unresolved functional dependency is silently downgraded to cosmetic. Approval of this plan can start independent visual work; it cannot make absent multi-chat APIs implemented.

## Target visual and interaction contract

### Theme and assets

Use a `ThemeExtension`/`SmithTheme` with semantic color, spacing, radius, typography and state tokens. Initial palette follows the supplied spec: background `#031514`, surface `#04201D`, raised `#092A27`, selected `#063D34`, stroke `#0B6557`, subtle `#164A43`, accent `#00E9C6`, bright `#00FFE0`, primary text `#E3F0EE`, secondary `#99B5B2`, muted `#76908E`, online `#00E6CA`, danger `#FF4D58`. Add any necessary semantic distinctions centrally, not widget-local colors; tune using screenshots and contrast checks.

Bundle JetBrains Mono as default and IBM Plex Mono as a theme/build configuration alternative; include actual 400/500/600 weights as needed and their notices. No persisted font-preference store is needed. Use `flutter_svg` at a Flutter/Dart-compatible stable version with reviewed locked transitive dependencies; import only used supplied icons and tint them explicitly. Do not rely on CSS `currentColor`. Keep the raster comp as design/test reference only, never runtime chrome. Do not import unused feature/window-control icons into visible navigation.

Actual text remains native selectable/editable glyphs. No glass, gradients, ornamental glow, raster panels, webview, additional rendering engine, or new state-management framework. The existing launcher icon is not replaced by this scope.

### Geometry (starting targets, refined by measured screenshots)

| Surface | Target |
|---|---|
| Top bar | About 56 high, `> SMITH` left; build-derived version and status dot/text right. Flexible height at enlarged text; no global clock or duplicate slogan. |
| Rail | 56 wide, always icon-only; Chat and functional Settings only. 20–24 glyphs in 48×48 targets; labelled tooltips and selected semantics. |
| History | About 280 wide on desktop; compact `Chats` header with Archive and New chat; scrollable rows at least 48 high with restrained active fill/accent edge. Overflow available by pointer, focus, keyboard and touch. |
| Main heading | About 48 minimum, `Chat / {title}` with truncation plus full accessible title; no additional dashboard. |
| Timeline | Independently scrollable centred lane, max width about 960; bubbles max about 720 and 80% of lane, up to 95% on narrow windows. SMITH left, YOU right; compact role/dot and real metadata only. |
| Composer | Anchored below timeline, aligned to reading lane; prompt chevron, native multiline field, adjacent Send icon. Roughly 64 minimum, bounded growth then internal scroll. No footer. |
| Details | 1 logical-pixel borders; radii around 4/6/8; spacing 8/12/16/24. Brand 22, section 16, body/input 14, metadata 12; body line height about 1.5. |

Below approximately 960 logical pixels wide, keep rail/main and move history to a labelled modal drawer. The Chat rail control returns from Settings and opens history when already on compact Chat; its hint/expanded semantics must make that behavior discoverable. Desktop keeps the list visible. Increase the breakpoint/list space when enlarged text cannot leave a usable main region (approximately 1280 at 200% is a starting point, not a rigid device check). At 400×400, reduce gutters, bound composer growth relative to available height, wrap compact status, and make dialogs scroll/stack actions. If necessary omit the header version at the smallest/enlarged layout while retaining it in About; never hide connection meaning. Ultrawide adds whitespace rather than wider text or globally scaled UI.

### Behaviors and recovery

- New chat opens/reuses one ephemeral blank workspace without overwriting existing drafts or writing a durable empty chat. Draft ownership and first-submit promotion come from the multi-chat controller.
- Select/rename/archive/reactivate/delete target stable IDs, never titles or list positions. Keep menu/dialog identity bound while background state changes. Only acknowledged mutations change authoritative visible membership; uncertainty requires observation, not automatic replay.
- Active rows offer Rename / Archive / Delete. Archive header opens the complete archived collection modal only on explicit action; archived rows offer Reactivate / Rename / Delete. Reactivation need not switch the underlying active view. Close restores prior focus/view unless an acknowledged action changed it.
- Recommended selection fallback after archiving/deleting the selected chat is an ephemeral blank view, not an unrelated chat or a persisted replacement. Unsaved draft disposition and mutation restrictions must be explicit in the multi-chat contract; no silent loss.
- Keep draft, reading position, provisional answer and pending-work identity when opening Settings, menus, dialogs, drawers, or resizing. Switching conversations must not let late results enter another chat. Mount/rebind listeners deliberately; use stable UI identity through ephemeral-to-durable promotion.
- Preserve Enter send / Shift+Enter newline, disabled-send preservation, text selection/copy, and initial newest-content scroll. Test key repeat and IME composition; do not submit while confirming composition. Returning to a chat restores its reading position; polling/draft notifications do not force-scroll.
- Show actual pending phase, error/stale state and existing expired-request resolution near the affected conversation. Query-only Retry remains reachable through the status surface and inline error state. Do not bury blocking failures in Settings or remove them to match the comp.
- Settings replaces main content without destroying chat state. Move the old footer's bounded-history/continued-work disclosure here. Boot inherits theme/mark tokens but keeps current coordinator, observation bounds, reduced-motion behavior and exact Start consent unchanged.
- Menus/dialogs support Tab, Enter/Space, arrows as appropriate, Escape and focus restoration. Chat-row keyboard traversal must not select a new chat merely by moving focus. Use status text as well as color, visible focus distinct from selection, and chronological transcript semantics.

## Multi-chat integration handoff

The following describes required data/actions, **not new protocol operation names**. Before implementation touches shared integration files, record the other session's actual types and revision in this plan.

| Data / action required | Owner / invariant |
|---|---|
| Active and archived collections: stable ID, title, archived state, authoritative ordering, selection | Multi-chat controller/service. UI never maintains a competing durable list. |
| Current conversation: stable presentation identity, optional durable ID while ephemeral, authoritative messages, optional real timestamps | Multi-chat owns identity and persistence. Widgets key lifecycle by identity, not mutable title. |
| Per-chat draft, pending request/kind/phase, uncertainty/expired state, provisional answer, send eligibility | Existing/multi-chat state owner. UI has editing/selection/scroll handles, not an independent copy of durable truth. |
| New/select/edit/submit; rename/archive/reactivate/delete; read-only refresh/reconciliation | Semantic callbacks through the existing controller pattern. Creation/submission identity and any idempotency belong below widgets. |
| Action availability, confirmed/failure/uncertain outcomes, mutation restriction and selection/draft fallback | Backend/controller contract, surfaced truthfully with no automatic mutation retry or inferred success. |
| Switch/restart/lost acknowledgement, legacy migration, bounded history, active-job behavior | Separate multi-chat owner provides implementation and tests. UI integration adds presentation regression tests, not a storage design. |

No placeholder controls or sample chat records may masquerade as supported production functionality. Private widget/screenshot fixtures may exercise target states before the backend lands. Full acceptance waits for real contract integration; do not add an interim production compatibility store merely to make the mock appear complete.

## Requirements, tasks, ownership and acceptance traceability

Proposed new paths below are implementation destinations, not claims that files already exist. Each workstream gets exclusive write ownership; final integration is serialized.

| Priority / task | REQ / AC and scope | Likely write set / owner | Definition of done and evidence |
|---|---|---|---|
| P0 TASK-01: freeze handoff | REQ-01/AC-01 preserve behavior and ownership; REQ-06/AC-06 real multi-chat actions | Coordinator: this plan; existing interface inspection only | User signoff, fresh revision/status, explicit UI/backend ownership, real contract mapped before integration. No parallel edits to shared files. |
| P0 TASK-02: visual foundation | REQ-02/AC-02 flat terminal visual system; REQ-03/AC-03 offline fonts/vector assets | Foundation owner: new `next/client/lib/ui/smith_theme.dart`, `smith_icon.dart`, assets, `pubspec.yaml`/lock, private asset/theme tests | Tokens, both local families, supplied tinted SVGs, notices/provenance, focus/hover/disabled treatments; asset load and contrast tests. Dependency resolution with lockfile; no runtime CDN. |
| P0 TASK-03: shell/navigation | REQ-04/AC-04 reference layout and responsive navigation; REQ-05/AC-05 factual status/Settings | Shell owner: new pure `ui/smith_workspace.dart`, `chat_sidebar.dart`, `archived_chats_dialog.dart`, `settings_view.dart`, their dedicated tests | Header, rail, responsive list/drawer, menus/modal, read-only Settings using data/callbacks; no API/store construction. Geometry/semantics/empty/error fixture tests at required sizes. Existing lifecycle `SmithShell` is not duplicated. |
| P0 TASK-04: conversation presentation | REQ-01/AC-01 preserved chat; REQ-02/AC-02 bubbles/composer; REQ-07/AC-07 keyboard/a11y | Conversation UI owner: `chat_panel.dart`, optional pure `ui/message_bubble.dart`, dedicated presentation tests | Reference-aligned role bubbles, selectable text, bounded composer/icon Send; pending/error/recovery retained, keyboard/IME and scroll behavior verified. No `text_chat.dart`/wire edits. |
| P0 TASK-05: integrate actual state | REQ-06/AC-06 ephemeral/create/select/rename/archive/reactivate/delete; REQ-05/AC-05 truthful state | Coordinator only: `main.dart`, final `chat_panel.dart` seam, `conversation_test.dart`; shared fixture edits by agreement | Actual multi-chat callbacks wired by stable ID; no empty persistence, cross-chat leakage, lost draft/listeners, fake timestamp, or mutation replay. Full archive and Settings round-trips. Depends on TASK-02/03/04 and accepted multi-chat contract. |
| P0 TASK-06: version/build/package | REQ-03/AC-03 complete assets; REQ-05/AC-05 dynamic version; REQ-08/AC-08 reproducible candidate | Build owner: narrow version injection in client build scripts, new build-info presentation module; `next/dagger/package.py`, package tests; Dagger wiring only if necessary | Derive version once from pubspec/build metadata, inject into Flutter, truthful development/unknown fallback. No native metadata plugin merely for a version string. Enumerate approved new asset/license paths; require complete assets and retain rejection of arbitrary weights/voice/reference files. Locked build/package tests pass. |
| P0 TASK-07: visual QA and assurance | REQ-04/AC-04 responsive; REQ-07/AC-07 accessible; REQ-08/AC-08 complete evidence | Coordinator/test owner: dedicated redesign fixtures/tests, screenshot evidence; `docs/DESIGN.md`, relevant narrow Architecture/Process notes, `next/client/README.md`, `next/VALIDATION.md` | Required screenshots inspected against app-only comp, all relevant regressions and packaged Linux build pass, fresh independent functional/visual/architecture review, exact revision/evidence retained. No self-approval. |

No P1/P2 product features are proposed. Streaming, Markdown, task execution, voice, projects/pair, inline model/GPU controls, persistent appearance settings, custom window decorations, service monitoring changes, and backend rearchitecture are out of scope. Any optional cosmetic deferral requires a named owner and follow-up at handoff; it cannot defer essential layout, assets, accessibility or functionality.

### Parallel execution and collision control

After signoff and shared token/interface definitions:

1. **Foundation/assets + shell + conversation presentation** can run as three disjoint workstreams; dependent widgets use the agreed token names/contracts. Their tests use private fixtures instead of changing the shared API fakes.
2. **Build/package** can run alongside visual work after asset filenames are frozen. Coordinate `pubspec` and build metadata with the foundation owner; one writer per file.
3. **Multi-chat remains the other session's workstream:** it owns `next/service/**`, `app_api.dart`, `text_chat.dart`, shared protocol fixtures, migrations and controller tests. This session does not edit these to bypass missing APIs.
4. `main.dart`, `chat_panel.dart`, shared conversation tests/fakes, Boot integration and canonical docs are explicit handoff points. Only the coordinator integrates after other owners stop editing them. Boot logic, native control/transport, installer behavior and GPU/runtime remain protected.
5. Recheck status/diffs immediately before each handoff and validation snapshot. Do not stage/commit the other session's work or regenerate historic evidence hashes. Final test/build results bind the actual combined snapshot, not the planning baseline.

## Validation plan (not executed during planning)

### Functional and negative gates

Preserve existing client tests, including Boot/Start eligibility, strict transport identity, unavailable models, lost acknowledgements, query-only Retry, expired-request confirmation, newer-draft preservation, provisional answer deduplication, initial scroll/reading position, close detach, and the real compiled Rust↔Dart fixture. Update assertions only where intended layout/labels change; do not remove negative safety coverage to satisfy the new screen.

Add tests for stable-ID selection with duplicate/renamed/reordered titles; zero persistence from New/typing/closing; one first-submit admission; rejection/uncertainty; switching while work completes; menus bound to the intended row; archive/reactivate/delete success and denied/failed/unknown results; no active-work cancellation claim; Settings/modal/drawer and breakpoint transitions retaining state; controller rebinding; no invented version/timestamps/readiness; no implicit model/lifecycle action. Multi-chat provides storage/migration/idempotency evidence; UI integration must consume those tests or a reviewed real-contract fixture, not substitute presentation fakes for them.

### Visual/accessibility matrix

| Geometry / variation | Required evidence |
|---|---|
| 400×400 at 100% and 200% text | Empty/populated Chat, multiline composer, unavailable/pending/recovery, drawer, menus, rename/delete, archived list and Settings; no clipped/unreachable action. |
| 1280×720 (native default) and 1440×900 | Primary app-only comp comparison: proportions, selected row, both bubble directions, composer, overflow/archived modal; both font families. |
| 4200×2400 and representative wide-short window | Readable maximum message width, natural expanded regions, fixed composer and independent scroll. |
| Existing 360×800 and 800×600 at 200% text | Retain previous enlarged-text and expired-request regression coverage. |
| Just below/at/above adaptive thresholds, DPR 1/fractional/2, reduced motion | No state/focus loss across reflow; crisp vector glyphs; Boot static equivalent and no forced scale-down. |

Use real bundled fonts in a deterministic Flutter widget/screenshot harness (not Ahem defaults) with clearly labelled synthetic data, fixed optional timestamps, and no live service/model access. Retain screenshots with logical/physical size, text scale, font and revision metadata. Compare the application content only; native window/desktop scenery is excluded. Inspect images, not just overflow assertions; resolve composition/spacing/color/type differences before acceptance. Fixture screenshots are not installed graphical QA.

Check accessible labels/roles/selected/expanded/disabled state, ≥48 logical-pixel targets, keyboard/touch menu discovery, focus restoration, chronological selectable message semantics, reduced motion, ≥4.5:1 normal text and ≥3:1 meaningful control/focus contrast. Decorative strokes need not carry meaning by themselves. Tests must prove the composer and modal actions remain reachable on small windows.

### Commands and evidence

After implementation approval, run scoped Dart formatting, `flutter analyze --no-pub`, and focused tests after locked dependency resolution. Use existing project gates from `next/`:

- `DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make client/test client/build`
- `DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make package/test installer/build`
- `python3 dagger/package.py --root installer/dist --verify`

Run final aggregate `make test build` once against the stable combined candidate when the parallel multi-chat work is integrated, rather than repeatedly building conflicting snapshots. Dagger/dependency/font acquisition can use approved package sources and write development caches; no model weights, runtime CDN, host GUI/service effects, or cloud trace sharing are required. Record actual commands, exit status, source/asset/artifact identities and screenshots in `next/VALIDATION.md` plus a focused evidence artifact. Historical baseline passes are not redesign validation. Missing tools/network or failed/time-limited gates must be reported, not represented as passes.

## Stage gates and review checklist

| Gate | Required inputs / completion | Blocking conditions / output |
|---|---|---|
| Plan → develop | Operator signs off this proposal, read-only Settings, status semantics, responsive rules and bounded dependency/build work. Fresh base and writer ownership recorded. | No implementation before signoff. Multi-chat may still be pending for independent visual tasks; its integration contract is mandatory for TASK-05. |
| Develop → self_check | Required widgets/assets implemented, real contract integrated, documentation updated. | Missing functionality, data safety/regression, counterfeit readiness or placeholder actions block. Output scoped diff and test/screenshot candidate. |
| self_check → assurance | Format/analyzer/tests/Linux build/package and visual matrix pass on identified snapshot. | Missing or failing required evidence blocks; fixtures cannot prove persistence/live functionality. Output retained checks and screenshots. |
| Assurance → human review | Fresh independent reviewer inspects requirements, changed boundaries, screenshots and reproduces relevant checks. | Resolve blocking findings; preserve disagreements instead of averaging verdicts. Output review findings/closure and operator QA handoff. |
| Human review → release/deploy | Separate exact approval for any install, live service/model/GUI operation, commit, push, release or deployment. | Plan/code/test success does not grant external-effect authority. No release with failed or missing required evidence. |

Checklist: all REQ/AC mapped; Settings is useful and read-only; assets local/licensed and package-complete; no parallel writer collisions; durable behavior owned once; ephemeral/mutation/recovery semantics tested; 400×400 and ultrawide screenshots visually reviewed; uncertainty and unavailable states remain truthful; rollback limited to the UI delta.

Rollback/compatibility: retain baseline and narrowly reversible presentation/assets/build changes. Do not revert independent multi-chat storage migrations or reset the worktree as a UI rollback. Ensure the integrated client and service are built/packaged together; do not claim the redesigned client supports an old protocol unless actual compatibility tests establish it. Installation is a separate operator action.

## Implementation handoff

1. Implementation approval received; scoped presentation work and local validation completed. Human visual acceptance and external-effect approvals remain separate.
2. Reviewed fonts/notices and selected supplied icons are imported. Multi-chat contract/writer handoff remains pending; no service/controller rearchitecture was added here.
3. Establish tokens, icon/font wrappers and build metadata; run disjoint visual workstreams and their focused tests.
4. Integrate real multi-chat and factual status once its contract is ready. Do not replace missing functionality with another local store.
5. Complete responsive/error/keyboard/accessibility polish and iterate on the required screenshot comparisons.
6. Run scoped then combined validation, record evidence, and obtain fresh independent review.
7. Present the candidate and exact remaining QA/deployment steps to the operator. No automatic commit, install, launch, push or release.
