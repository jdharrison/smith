# LOCAL-SMITH-AGENTS-PROFILE-LAYOUT

## Active approval

**LOCAL_PLAN_APPROVED — existing-profile UI only.** The operator narrowed the original full Agents request to “trim and remove all the extra crap, fix the anchor, and ui/ux layout”; execution and agent/project/container composition come later. After the assistant confirmed a UI-only cleanup preserving existing fields, persistence, validation and unsaved-change protections, the operator said “continue”. This supersedes the proposed registry/runtime expansion, not the existing service contracts.

Baseline: existing `feature/autonomy`, HEAD `14d960975e76f906efb5e447ff3dece0e8e4385b`, plus pre-existing modified/untracked source. Preserve all of it. Selected pre-edit UI/test copies and validation logs are under `target/agents-profile-layout/`. No commits, branch changes, remote actions, installation, live services/models, state migration, execution or publication are authorized. Approval expires on completion or material scope drift.

## Reconciliation

Independent planning inputs: producer `16e9d8d7-c5d1-4cf3-bf4c-308e52f2be1d`, architect `2e06cc96-cbc4-484e-ab2d-63951baea67e`, designer `b3a62d6a-ffd4-46f7-a202-3f325e73821a`. They established that profiles are metadata only and no active OpenCode runtime exists. The operator resolved that blocker by expressly excluding execution and narrowing to existing-profile presentation. New CRUD, model/harness settings, registry/storage changes and per-agent draft collections are not part of this slice. Existing guarded replacement and retained per-pane editor behavior remain.

## Scope and traceability

| Task | Requirement / acceptance | Evidence |
|---|---|---|
| T1 | R1/A1: compact list, clear selected row, existing profile icon/name/description; remove repeated boilerplate and duplicate Refresh without removing recovery access | Widget tests and reviewed real-font goldens |
| T2 | R2/A2: top-left editor anchor beside list, compact responsive name/description fields, clear saved profile identity and prompt, Save/Cancel; no unsupported tabs or controls | Geometry assertions, compact/enlarged/high-DPI fixtures |
| T3 | R3/A3: preserve exact validators, prompts above1000 characters within4096 bytes, immutable ID, save/cancel/review/unknown-outcome behavior, navigation/draft/focus/scroll retention | Existing and new profile/controller/navigation tests |
| T4 | R4/A4: update Design contract and retain actual validation evidence; obtain fresh independent assurance | Analyzer, focused/full client tests, Linux build where available, independent review |

Production scope: `client/lib/ui/agents_view.dart`. Reuse existing compiled icons and theme; no new icon dependency or asset contract. Tests/goldens, this plan, affected Design documentation and evidence may change. Shell/sidebar/routes/preferences, Projects, shared theme/fonts, API/controllers/models and Rust service are unchanged. No enabled/model/provider/timeout/iteration/harness/Test/history/Add/Delete controls are fabricated. Profile fields and limits remain name40 scalars, description256 scalars, prompt4096 UTF-8 bytes. No1000-character cap is introduced.

The existing editor is centered in an800px maximum-width box. This slice interprets the requested anchor correction as top-left alignment with a consistent gutter beside the list, using the Projects reading-width bound, not a shell/navigation rewrite. Both list and editor remain independently scrollable and retained through compact/wide changes. Essential loading, validation, stale-instance/revision and query-only recovery states remain reachable.

## Gates and exclusions

Run scoped tests first, then formatting/analyzer, full client tests including compiled fixtures when available, Linux build and fresh assurance. Preserve existing failure evidence; do not fix unrelated baseline bugs or label missing/skipped checks PASS. Broader existing root/package failures and installed acceptance remain separate. Native graphical/IME/high-DPI host acceptance is not implied by widgets or goldens. No persistence change or live rollback is involved; no release is authorized.
