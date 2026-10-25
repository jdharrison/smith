# LOCAL-SMITH-PROJECTS-EDITOR-LAYOUT

## Status and authority

**LOCAL_PLAN_APPROVED — layout-only subset.** After reviewing this draft, the operator confirmed: “yes. just ensure layout and organization matches then we can iterate more.” This selects the existing-field Projects layout pass, not the full metadata handoff. The controlling instruction remains: ignore all sidebar and other UI changes; use the image only as an interior layout reference within current UI/components, with no redesign or major tweaks.

The broader draft below is retained for traceability, but only the active slice in section0 is authorized. Unsupported metadata must not be simulated in widgets. ID/Name semantic changes, description, prompt, read-only, Duplicate, requiredness changes, new storage/operation versions and Dev execution remain deferred. D0 is resolved to layout-only; D1–D4 do not block that subset and grant no authority to implement those features.

Discovery HEAD: `14d960975e76f906efb5e447ff3dece0e8e4385b`, existing `feature/autonomy` checkout. Prior structured Projects and separate documentation cleanup are modified/untracked working-tree inputs, not HEAD content. Preserve them and their historical evidence. No branch/commit/remote mutation, installation, application launch, live-state migration, credentials/provider/model probe or repository/agent/container execution is included. Follow [Process](../PROCESS.md#approval-boundaries); implementation needs a bounded approved local plan.

Risk: UI-only layout is a client-flow change; the full handoff is elevated because it adds persistent fields, validation and compatibility/recovery behavior. This proposal changes no implemented policy or capability.

## 0. Active approved slice

- Keep the exact current project/repository models, validators, App3/Core2/disk6, current operations/controller and presentation preferences unchanged. No Rust or model/API edits.
- Reorganize existing Projects content into a list heading/New/search/cards region and an independently scrollable editor with local Save Changes/Cancel/Delete actions, Basic Information (existing Name only), existing OpenCode information, then Repositories. Do not add empty ID/description/prompt/read-only/Duplicate controls or change Work into a prefix.
- Local search filters current project Name only, without API calls, selection substitution or draft loss. Cards show the real Name, layered glyph and derived right-aligned repository count/branch glyph; no fabricated descriptions.
- Compact layout uses list-to-editor drilling and Back to projects; Back/resizing/destination changes preserve the current editor/draft/disclosures/meaningful scroll state. When no matching saved card supplies a return route, Return to project editor reopens the retained form without reload/rebinding, including new/missing/filtered/write-blocked editors. New remains an explicit replacement/create action with its discard guard; it is not the resume action. Keep editing after declining replacement returns to the retained editor. Keyboard traversal must reveal focused controls at either edge, including long-list Tab/Shift+Tab wrap. Retain existing stale-instance/revision, uncertainty, delete and immutable capture safeguards.
- Pair existing repository Name/URL and Base/Work fields at usable widths, stack otherwise; four optional type fields remain a disclosure. Add Repository moves to its section header; Add/Remove remain draft-only.
- Only two needed new local SVGs (`layers`, `git-branch`) may be added through the existing wrapper with exact package/provenance/test updates. No duplicate/search glyph is necessary in this slice; no global icon or theme change.
- Production write scope: `client/lib/ui/projects_view.dart`; `client/lib/ui/smith_icon.dart` names plus the two SVGs; `client/assets/README.md`; exact asset entries in `dagger/package.py`. Tests/goldens, affected Design documentation and source assessment, the mechanical Process asset-count correction (24→26), validation ledgers and this plan/evidence may change. Global shell/sidebar/header/theme/fonts/navigation/Chat/Settings and all service/model/controller behavior are frozen.
- Actual selected pre-layout baseline is preserved under `target/projects-layout/`:1,459 source/policy files, canonical manifest SHA256 `740cc41d9ba4bc8265d5ea69a34fe5bf5ac8e11596ef4223357232a6756a44c3`. New evidence belongs to a separate layout bundle; do not rewrite earlier Projects/structured-repository evidence.
- Implement T2, applicable layout-only T3, T4(two glyphs only) and T5. T1 and full-field AC3/AC4 are deferred; existing-field validation/recovery regressions remain blocking. No commit, host GUI/service launch, live installation, runtime workers or external effects are included.

## Independent planning inputs

- Producer `97a05945-b402-404d-8b81-2dd52bdfcf68`: requirements/acceptance, strict outer-UI exclusion, unsupported metadata and migration/duplicate decisions.
- Architect `09b9cfab-7819-40c7-8f37-e86472818494`: current authority chain is sufficient; keep stable identity, preserve old data and reject incompatible writers if the schema changes. No new storage system or execution service.
- Designer `a15c83b3-a60b-43e4-8a34-0768f7d7c50c`: layout using existing tokens/components, per-pane responsive drill-in, draft/focus/scroll retention, local SVG/provenance/package implications.
- Planner reconciliation: retain all three scope boundaries and blocking decisions. No disagreement is voted away. The operator has now approved the independently deliverable layout-only subset in section0; full-handoff acceptance still requires separate metadata decisions and compatible persistence work.

## 1. Scope lock

**In scope:** the content body of Projects and the minimum service/model support for explicitly approved new project configuration fields/actions. The image supplies composition and density—not new visual styling.

**Unchanged:** global rail/sidebar, navigation destinations/order/icons, shell/header/status/version/breadcrumb, `SmithTheme`, global `SmithTokens`, fonts/font weights, palette, animations, Chat, Settings, model/provider controls, lifecycle and connection behavior. Chat / Projects / Settings already exists; no premature Development entry was found to replace. Do not add one or change navigation speculatively.

Do not copy screenshot glow, decorative chrome, sample projects, URLs, prompt text or cross-platform claims. No drag handles/reordering, sidebar overhaul, workflow strategy, branch execution, model/provider picker, arbitrary host roots or runtime containers. Existing font and SVG dependencies remain; no WebView, new styling framework or runtime asset downloads.

## 2. Verified current implementation

| Current source | Implemented behavior |
|---|---|
| `client/lib/ui/projects_view.dart` | Stateful per-view editor, wide list/editor, 256px list, text-scale-aware split threshold starting at880px of available body width; compact chooser rather than drill-in. Existing New/Save/Cancel/Delete, optional named repository cards, visible Base/Work and disclosed four-type fields. |
| `client/lib/projects.dart` | Exact project `{project_id,name,repositories,agent}`; stable opaque32-hex internal identity; required Name≤40 Unicode scalars; fixed OpenCode. Ordered repositories contain optional name, URL, independently nullable `branch.base/work/types` values. |
| `client/lib/projects_controller.dart` | Shared desktop registry owner; immutable captures, independent revisions, captured instance/revision writes, query-only uncertainty and authoritative refresh. |
| `service/src/app_projects.rs`, `app_v2.rs`, `app_v3.rs` | Atomic disk6, Projects `*_v2` operations, fixed receipt lookup, bounded replay protection, strict migration/validation and incompatible older-writer refusal. |
| `client/lib/ui/smith_theme.dart`, `smith_icon.dart`, `client/pubspec.yaml` | Existing native theme/tokens, bundled JetBrains Mono/IBM Plex Mono400/500/600, local `flutter_svg` wrapper and15 exact icon names. |

Not implemented: editable kebab-case project key, description, prompt, repository read-only flag, search or Duplicate. Counts are derivable from existing repositories without schema changes. The current Work field is an exact shared working branch—not a prefix. Repo names and branch leaves are currently optional.

## 3. Proposed interior composition

### Project list panel

- Heading **Projects** with **New Project**, then a local **Search projects** input.
- Dense selectable rows/cards using existing selected surfaces, strokes, typography and hit targets.
- Layered project glyph on the left; approved display ID/name and description text; repository count and small branch glyph aligned top-right. Counts include every configured repository and support0/1/many.
- Retain existing read-only Refresh/recovery access as a secondary Projects action; do not add global refresh/header changes.
- Filter locally by the approved human ID and description. Layout-only fallback searches current Name; never fabricate an ID/description field. Distinguish no projects, no matches, loading and unavailable.
- Filtering cannot change selection, discard drafts, issue a mutation or auto-select the first match. A filtered-out selected project retains its editor.

### Editor

- Local heading **Edit Project** or **New Project · not saved**.
- Local Delete / Duplicate / Save Changes actions, wrapping when needed. Retain an explicit secondary Cancel and unsaved indicator. Preserve existing action eligibility and confirmation/recovery guards.
- **Basic Information:** approved ID and Description side-by-side where space permits; Prompt full-width below. Stack on narrow/enlarged layouts. Do not render unbound saveable-looking fields before service support exists.
- **Agent:** existing OpenCode-only information in its own compact section before repositories. A one-option field/selector may be used if approved, without implying installation/readiness or adding models/providers.
- **Repositories:** section heading and right-aligned Add Repository; repeat compact existing-style groups. Header identifies the repository with Remove; Name/URL share a row when usable; Read Only follows once supported; Base/Work and the four-type disclosure follow beneath.
- Branch types remain four independently configurable Feature/Fix/Release/Hotfix patterns—not the screenshot's single dropdown. Preserve `branch.{base,work,types}` and the current Work meaning. A prefix reinterpretation is a different excluded change.
- Keep loading/errors/stale-origin/uncertain-change recovery reachable outside collapsed repository sections. Do not hide destructive effects or require scrolling inside each repository card.

### Responsive, scrolling and focus

- Use each Projects pane's actual constraints after existing shell chrome, not whole-window width. Four-pane workspaces may contain wide and compact Projects views simultaneously.
- Split view only when both regions remain usable. Proposed list width is bounded around256–384 logical pixels using existing gutter multiples; editor uses remaining space and the existing reading/form-width constraint. These are layout bounds, not new global design tokens.
- At narrow/enlarged widths, show list first and drill into the editor with **Back to projects**. Resizing an open editor keeps that editor open. Back is not Cancel and does not save/discard.
- Own separate list/editor scroll controllers in retained per-view state, outside responsive rebuilds. Preserve query, selection, draft, field selection, disclosure state and meaningful scroll offsets through Back, resize, destination changes and pane hiding. Clamp only offsets no longer representable.
- Stack Name/URL, ID/Description and Base/Work rows independently as their local constraints require. Allow local headings/actions to scroll at short heights rather than consume the viewport as fixed chrome.
- Keep48px action targets, visible keyboard focus, selected semantics, wrapping errors and accessible labels despite visual density. Hidden views cannot retain focus/traversal. Enter in fields must not trigger Save or Chat Send; multiline Prompt/IME retains normal editing semantics.

## 4. Contract decisions required for the full handoff

These are recommendations/questions, **not silently approved schema changes**.

| Decision | Proposed resolution / remaining human choice |
|---|---|
| D0 — Next implementation scope | Choose layout-only using current fields, or the full textual handoff with metadata extensions. The top instruction freezes outer UI in either case. Do not call layout-only complete acceptance of the full handoff. |
| D1 — Editable ID and current Name | Never edit/replace `project_id`; it remains the immutable service identity for targets, receipts and selection. Recommend a separate human project key displayed as ID. Confirm whether current Name remains, becomes the human key under an explicit semantic migration, or is retained as legacy display metadata. Do not add an unrequested second visible name field or relabel the hex identity as an editable slug. Define slug length/grammar and atomic uniqueness before implementation. |
| D2 — Requiredness and legacy records | Description is required/single-line≤60; Prompt may be blank/multiline≤1000. Recommend Unicode-scalar counters matching current Rust/Dart rules; preserve Prompt newlines/spacing without silent truncation. Repository Name becomes required under the handoff. Existing projects lack the new values and may have unnamed/partially configured repositories: choose completion-on-next-Save versus grandfathering unchanged incomplete values during unrelated edits. Never invent slugs/descriptions/names or drop data to satisfy validation. |
| D3 — Read Only and writable validation | Treat this as future repository-write classification, not locking metadata edits or claiming an implemented sandbox. Choose the initial/migrated value explicitly; do not silently grant write intent. Hide/disable inactive branch controls without clearing, omitting or replacing values. Retained values must remain syntactically valid even when inactive. Confirm whether writable Base/Work are required; do not infer that all four type patterns must be present. |
| D4 — Duplicate | Recommend copying the selected **saved** configuration into an unsaved draft after resolving dirty source input. Clear the new human ID for explicit entry; obtain a fresh internal identity only on Save. No automatic suffix policy or immediate backend mutation. Do not copy request/receipt/pending/history state. Deep-copy all ordered repositories, inactive settings and fixed agent. Use the existing create path; a dedicated duplicate endpoint is not assumed. |

A lower-kebab ID requires a service-side uniqueness check; client validation alone cannot prevent concurrent duplicate saves. Description validation must be separate from the existing40-scalar repository-name predicate. Agree control/newline/counting and empty/null representations in both Rust and Dart, including Unicode line separators and IME composition.

Prompt is inert stored configuration: saving never sends it to Chat, adds model context, executes tools or broadens authority. Read Only being off likewise authorizes no repository execution in this slice.

## 5. Work packages and execution order

| Priority / task | Scope and files | Depends on | Done when |
|---|---|---|---|
| P0 / T0 — Contract and baseline | Resolve D0–D4 as applicable; record an approved bounded scope and capture actual modified/untracked baseline. | Human decisions | No ambiguous identity/requiredness/migration/copy semantics remain for the selected slice. |
| P1 / T1 — Approved metadata support | Existing `service/src/app_projects.rs`, `app_v2.rs`, `app_v3.rs`; `client/lib/projects.dart`, `projects_controller.dart`, `app_api.dart`, Projects dispatch in `cloud_routing.dart`; related fixtures/tests. | T0; full slice only | All approved fields persist through the current service authority, validation and recovery paths. No separate storage/controller/API owner. |
| P1 / T2 — Projects composition | `client/lib/ui/projects_view.dart`; small presentation-only extraction files only if they improve ownership clarity. Keep `_ProjectsViewState` and repository drafts as the state owners. | T0; T1 API contract for new fields | List/search/cards/editor sections/local actions and compact drill-in match the composition while outer UI is unchanged. No fake fields/seed data. |
| P1 / T3 — Behavior integration | Same view/controller and existing tests. Draft Duplicate, counters, read-only retention, all-field capture/review, focus and scroll state. | T1/T2 as applicable | Requested behavior is backed by real service operations and existing recovery guarantees. |
| P1 / T4 — Local icons and package closure | Only needed local SVGs through `SmithIcon`; asset provenance, exact `dagger/package.py::UI_ASSET_FILES` and asset/package tests. | T2 approved glyph needs | Layered-project/branch/duplicate glyphs are reviewed and bundled; no sidebar icon replacement, arbitrary asset allowlist or license inference. Search glyph is optional. |
| P1 / T5 — Self-check, visual evidence and assurance | Extend existing Projects protocol/controller/view/navigation/compiled tests; add Projects-only bundled-font goldens. Update affected canonical docs after behavior approval. | T1–T4 | Selected slice passes source-bound required checks and fresh independent review; failures/skips are accurately retained. |

T1/T2 can be implemented in disjoint source scopes after the contract is fixed. Do not parallel-edit the same view/controller. For layout-only, omit T1 and unsupported parts of T3, retain current Name/OpenCode/repository semantics, and explicitly defer the unsupported handoff requirements. Do not create a Git branch, commit or PR merely because the plan has work packages.

If fields are approved, use an explicit next storage/domain-operations version boundary—likely disk7 and Projects `*_v3`, subject to contract review—not App4/global protocol churn. Freeze disk6 before migration; preserve identities/order/registry revision/old fingerprints/receipts/tombstones and unrelated data. Refuse incompatible older whole-record writers, including empty updates/deletes, so prompt/read-only/key metadata cannot be erased. Historical receipt queries and query-only recovery remain. No binary-only installed rollback; live rollout/restore is separate approval.

## 6. Requirements-to-evidence traceability

| Requirement / acceptance | Task | Blocking evidence |
|---|---|---|
| R0/AC0 — Outer UI, components and inertness unchanged | T0/T2/T4/T5 | Scoped diff and unchanged Chat/Settings/shell goldens, preserved drafts/queues, no new lifecycle/provider/repository effects. |
| R1/AC1 — Interior arrangement and accurate cards | T2/T4/T5 | Structural widget checks and Projects-only real-font fixtures at1536×1024 plus compact/ultrawide;0/1/many repository counts align correctly. |
| R2/AC2 — Search/drill/Back/scroll stability | T2/T3/T5 | Local search no writes; empty/no-results distinctions; filtered selection persists; independent scroll/focus/draft state through resizing and all four pane layouts. |
| R3/AC3 — Approved fields and counters | T1/T3/T5 | ID format/atomic uniqueness/rename; required fields;60/61 and1000/1001 scalar boundaries; Unicode/control parity; preserved Prompt newlines; real compiled persistence round trip. |
| R4/AC4 — Read-only and type-set behavior | T1/T3/T5 | Configure all four types, toggle read-only, save/restart/toggle back with values/nulls intact; hidden invalid values remain recoverable; no single-type dropdown or prefix reinterpretation. |
| R5/AC5 — Explicit draft/actions | T3/T5 | New/Add/Remove/Duplicate cause zero durable writes until Save; duplicate isolation/new identity; dirty replacement guards, Cancel and exact-target Delete; full-registry/action eligibility. |
| R6/AC6 — Compatibility and recovery | T1/T3/T5 | Lossless/corrupt-preserving migration, old-writer refusal, shared replay identities, delete scrubbing and state/frame bounds. Same-revision restart/open-dialog fencing, nested-only edits, newer input preserved, lost acknowledgements resolved without replay. |

Required states: loading, empty, no results, unselected, clean, dirty, saving, invalid, unavailable, stale/restarted, uncertain, missing target and capacity exhaustion. Test400×400,1536×1024,4200×2400, enlarged text, short heights and both sides of local fit thresholds. Use current font assets/weights and SVG wrapper; do not regenerate unrelated goldens to conceal outer-UI changes.

## 7. Stage gates and approval boundaries

- **Plan:** resolve the selected slice and applicable D0–D4, preserve independent concerns, map ACs and obtain explicit local implementation approval. This draft alone grants none.
- **Develop → self_check:** focused view/navigation/model tests first, then applicable component formatting/analyzer/locked warning-denied checks, actual Rust↔Dart integration when the contract changes, Linux build/package validation and stable source/artifact binding. Use existing configured workflows and pinned tooling; no GUI/service launch as a shortcut.
- **Assurance:** fresh context tests critical UX, migration/compatibility/recovery and unchanged outer UI against the approved source/evidence. Required failing or missing evidence blocks acceptance; no reviewer accepts residual risk on the operator's behalf.
- **Release/installed:** no-go without separate evidence/approval. Native Ubuntu graphical/IME/accessibility QA, installed migration/restore, clean-host/source provenance, license/icon/SBOM and publication remain separate from widget/golden/source passes.

Future validation should extend `client/test/projects_view_test.dart`, `projects_navigation_test.dart`, `projects_protocol_test.dart`, `projects_controller_test.dart`, `rust_projects_integration_test.dart` and `service/src/app_projects_tests.rs`, plus asset/package checks if SVGs change. Existing tests/evidence are historical baseline, not passes for this proposed UI. No implementation tests/builds were run during this planning request.

## Implementation handoff

1. Confirm the layout-only versus full-fields scope and the applicable contract decisions; do not guess.
2. Capture the real shared baseline, approve the bounded record, then assign disjoint data and presentation ownership.
3. Keep all changes within Projects and necessary existing-service/asset integration; freeze shell/navigation/theme/font behavior.
4. Implement real field bindings and preserved draft/recovery semantics before declaring the complete handoff delivered.
5. Produce responsive, keyboard, negative-compatibility and compiled evidence as applicable; obtain fresh assurance.
6. Stop before source-repository Git mutations, host operations or runtime Development/orchestration work.

## Execution closeout — approved section0 only

**Implemented and locally validated; not human acceptance of external effects.** T0's actual pre-layout baseline, T2, applicable existing-field T3, T4(two glyphs) and T5 are complete for the selected layout slice. D1–D4/T1 and unsupported full-field AC3/AC4 remain deferred. Broader draft sections above are historical planning context, not implemented field claims.

| Selected acceptance | Implemented evidence |
|---|---|
| R0/AC0 | Baseline scope audit and independent review confirm unchanged global UI, fonts/theme, service/model/controller/API/preferences and127 historical evidence files. |
| R1/AC1 | Structural tests and eight new bundled-font goldens cover wide/compact/2×/ultrawide composition, accurate0/1/many counts and no simulated metadata. All28 unique pre-existing golden PNGs remain unchanged. |
| R2/AC2 | Local search, retained scroll/selection/disclosures/focus, all four workspace layouts, unsaved/missing/filtered/uncertain return and forward/reverse long-list keyboard cases pass. |
| R5/AC5, existing subset | Existing New/Add/Remove/Save/Cancel/exact-target Delete and replacement-discard guards remain; no Duplicate was added. |
| R6/AC6, unchanged-contract regression | Same-instance/revision fences, immutable capture/newer input, query-only uncertainty and deletion reconciliation remain; real compiled Projects fixtures ran without a schema change. |

Final configured `make test build`, package verification and actual-package helpers passed:1,497 aggregate coverage (1,214 fresh +283 cached installer) plus34 fresh helpers, five controlled fixture ignores/no other skips. Full Flutter869 passed. Stable1,265-core-input SHA256 `f00306ef6fb418e4c3a53b12261599cc290ede874d12822e5a647e58fdf54164`; rebuilt99-file/26-asset package tree SHA256 `d168fdf131402ee099b516ea752fb510c8b86f2eece1c7aca6b4f0e0e048f725`. [Separate layout evidence](../evidence/projects-layout/README.md) and [fresh independent closeout](../evidence/projects-layout/REVIEW-CLOSEOUT.md) bind outcomes, cache limitations and remediated M1/M2 findings. Initial failed gates/repros remain preserved; later passes do not explain the unchanged service fixture's earlier timeout or historical installer failures.

The New/no-dialog literal reviewer probe is preserved as failed historical evidence. The corrected requirement mapping changes only its post-Back action to Return, preserving every assertion; removing the New replacement safety prompt is neither required nor approved. Native/installed acceptance and publication/provenance/rights remain separate. No commit, branch/remote mutation, application launch, installation, live migration or runtime workers were authorized or performed.
