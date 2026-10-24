# Projects editor layout — independent assurance review

- **Scope:** approved `LOCAL-SMITH-PROJECTS-EDITOR-LAYOUT` section0 only: existing-field Projects interior, responsive navigation/state, and two exact local glyph additions. No implementation ownership or risk acceptance.
- **Baseline:** independently verified the actual 1,459-file pre-layout manifest/archive, not `git diff HEAD`; preserved existing Projects/structured-repository work and historical evidence.
- **Method:** source/delta and test-assertion inspection, independently executed headless regressions/goldens/package-policy checks, and ignored adversarial scratch fixtures. No production files, source tests, or goldens were edited.
- **Status:** **provisional source/UX review — FAIL / NO-GO.** One reproducible medium-severity acceptance blocker remains. Required aggregate/build/package gates also failed or did not complete; this is not final parent closeout or native/installed acceptance.

## Source and authority binding

Read in the required order: `AGENTS.md`, `docs/SCOPE.md`, relevant Architecture/Process/Design/Host Profile requirements and detailed Projects contracts, `roles/assurance.md`, then the approved layout plan and implementation/tests. Section0 controls over the deferred full-field draft.

| Identity | Reviewed value |
|---|---|
| Work / approved slice | `LOCAL-SMITH-PROJECTS-EDITOR-LAYOUT` / section0 existing-field layout only |
| Branch | `feature/autonomy` |
| HEAD, not the layout baseline | `14d960975e76f906efb5e447ff3dece0e8e4385b` |
| Baseline files | 1,459 |
| Canonical baseline SHA256 | `740cc41d9ba4bc8265d5ea69a34fe5bf5ac8e11596ef4223357232a6756a44c3` |
| Independently selected review-core files | 1,262, including tracked and untracked inputs |
| Before and after review-core SHA256 | `5f1f8b7070ad71fd424c23b873ecd5eeffe4e19eee68c601a3470932f19b0f98` |
| Changes during review | None in that core manifest |

The canonical baseline digest is SHA256 of the manifest's `files` array serialized with sorted keys and compact JSON separators, not the pretty-printed JSON file's raw digest. The archive has exactly the same 1,459 unique regular-file names, sizes, SHA256 contents, and modes; no absolute/traversing/link/special entries were accepted or extracted. Raw manifest/archive digests and every observed delta are in [baseline verification](review/baseline-verification.json).

The 11 modified baseline paths are the view, icon-name wrapper, asset README, two adjusted Projects tests, package allowlist/test, Design, mechanical Process asset count, source assessment, and layout plan. Actual additions are two SVGs, two layout test files, and eight Projects-only PNGs. Five tracked paths omitted from the selected baseline inventory (`.gitattributes`, `.gitignore`, `.smith/project.yaml`, `LICENSE`, `SECURITY.md`) were separately verified byte-identical to HEAD; they are not attributed as layout additions.

Production service/model/controller/API/preferences/shell/header/sidebar/navigation/theme/font bytes and modes match the actual baseline. The only package-code change is adding the two exact UI asset strings. All **127** manifest-listed files under the two historical evidence directories remain unchanged; none of their prior test passes is used as current acceptance evidence.

[Before manifest](review/before-core-manifest.json), [after manifest](review/after-core-manifest.json), and [stability result](review/source-stability.json) bind the independent checks. The validation agent uses a differently shaped 1,265-file manifest/digest. Independent comparison found **1,261 shared files with zero content/size/mode conflicts**; its four additional root build-policy files also match the current checkout. The reviewer-only entry is `AGENTS.md`. See [parent gate observation/binding](review/parent-gates-observed.json); the different digests are not presented as the same manifest.

## Findings by severity

### Critical

None identified in the bounded source/UX review. This is not a claim about unexecuted native or installed behavior.

### High

No high-severity layout-source defect identified. Required gate failures below independently block acceptance; they have not been waived or classified as harmless/flaky.

### Medium — M1: Compact Back strands a retained editor that has no saved-card route

**Locations:** `client/lib/ui/projects_view.dart:169–178`, `:188–191`, `:727–738`, `:998–1003`, `:1077–1081`.

**Requirement:** plan section0 (lines18–23), R2/AC2, and `docs/DESIGN.md:74–78`: compact Back is navigation, not discard; preserved drafts and recovery must remain usable.

**Reproduction (independently observed at 400×400, normal and 2× text):**

1. Open Projects in a compact pane with an empty registry. Choose **New Project**.
2. Enter a project Name, add a repository URL, open Branch types and enter a type pattern.
3. Choose **Back to projects**. The editor and controllers remain mounted offstage, but there is no saved card for this unsaved form and no return/resume action.
4. Choose **New Project**, the only apparent route back. It opens **Discard unsaved project changes?** rather than reopening the retained form.
5. Choose **Keep editing**. The dialog closes, but the list stays visible; the preserved editor is still unreachable through compact navigation. Discarding instead replaces the draft. Widening the pane reveals the original input, demonstrating that it was retained rather than immediately erased.

**Root cause:** the resume fast path in `_select` only handles a **non-null saved project ID** equal to `_original.projectId`. Back unconditionally sets `_editorOpen = false`; the list has only saved cards and a create/replacement action. New forms have no saved ID. Dirty missing-target forms have no saved card either. Under unresolved create uncertainty, New is additionally disabled by `registry.canMutate`, so it cannot be used even as an attempted return route. No mutation fence needs to be loosened to fix this presentation problem.

**Related confirmed cases:** a dirty saved target removed externally, and an uncertain create with expired receipt/currently absent target, are also stranded after Back. All three cases reproduce at both text scales. This is **one navigation-state defect**, not three separately counted findings. No unintended API write or replay was observed; input persists in memory until an explicit discard/replacement or view lifetime ends.

**Evidence:**

- [Requirement repro record](review/scratch-probes-v3.json) / [raw sanitized log](review/scratch-probes-v3.log): the two normal/2× unsaved-New resume assertions fail; three other independent keyboard/bookmark checks pass.
- [Observational confirmation record](review/defect-confirmation.json) / [log](review/defect-confirmation.log): six scenarios confirm retained-but-inaccessible input, Keep editing staying on the list, blocked uncertainty, and widening as a workaround. These six passing observations **reproduce the defect**; they are not six acceptance passes.
- Ignored repros: `target/projects-layout/assurance/independent_layout_probe_test.dart` and `target/projects-layout/assurance/defect_confirmation_test.dart`. Their identities and ignored status are recorded in [scratch identities](review/scratch-identities.json).

Run from the repository root:

```sh
python3 -B target/projects-layout/assurance/review_check.py run reproduce-m1 --concurrency=1 --reporter=expanded --plain-name 'unsaved New Back resume' ../target/projects-layout/assurance/independent_layout_probe_test.dart
python3 -B target/projects-layout/assurance/review_check.py run confirm-m1 --concurrency=1 --reporter=expanded ../target/projects-layout/assurance/defect_confirmation_test.dart
```

The wrapper derives the cached Flutter SDK from `client/.dart_tool/package_config.json`, runs from `client/`, and records the exact Dart/snapshot command, environment overrides, timestamps, exit code and sanitized log. The first command is expected to fail on the reviewed core; the second confirms the observed failure mode.

**Required remediation:** provide a compact-list route to resume the currently retained editor, including an unsaved, missing-target, or write-blocked editor, without `_load`, implicit discard, selection substitution, automatic save, or replay. Preserve the current origin fences, controller/selection/disclosure identities and scroll bookmarks. Add source regression coverage for those return paths and rerun fresh assurance on the changed core. The existing saved-card Back test (`client/test/projects_layout_test.dart:354–464`) and replacement-discard test (`:466–495`) do not cover this gap.

### Low

None additionally reported.

## Source/UX observations that were verified

These are **implemented-source / headless-fixture observations**, not target-policy guarantees or installed acceptance:

- Search derives matches from saved project **Name** only, trimming/lowercasing the query. Filtering invokes local state only; it neither calls the API nor replaces selection. Tests preserve a filtered-out editor, draft, text selection and disclosure. Loading, empty, unavailable and no-match states are distinct.
- Cards use the existing component/theme machinery, display actual saved Names, and derive 0/1/many repository counts with selected semantics and right-aligned count/branch glyphs. No ID/description/prompt/read-only/Duplicate/dropdown/schema controls were introduced.
- Editor ordering is local heading/actions → Basic Information/Name → Agent/OpenCode → Repositories; Add is in the repository heading. Field pairs use local available width/text scale independently and retain `EditableText` state through paired/stacked reflow. Work remains an exact optional branch; equal Base/Work is valid; all four independent optional type patterns remain a disclosure.
- Per-view list/editor scroll controllers and mounted slots retain saved-editor state through Back/return, filtering/clamping/reflow, destination changes, hidden panes and all four workspace layouts. Independent scratch filtering restored a temporarily clamped list bookmark. M1 limits the claim that all retained editors can be reached again.
- Corrected independent **actual Tab traversal** reached all 17 expected controls/fields at 400×400 normal/2×, checked focused geometry and ≥48px targets, and excluded the hidden list search from focus. The existing four-pane regression checks hidden/destination focus fences and independent scroll/selection/disclosure state.
- `_registryChanged`, discard, Refresh/Resolve, Save, submission reconciliation, revision review and exact-target Delete method bodies are byte-identical to baseline. [Seven critical-section hashes](review/critical-safety-sections.json) independently bind that inspection. Existing tests actually executed captured old Save/delete/review callbacks, same-revision instance changes, nested-only dirty edits, newer input after immutable submissions, query-only unknown/expired receipt recovery and dirty deletion reconciliation across restart.
- No production mutation eligibility, stale-instance/revision fence, controller or model change was found. No new repository/Git/provider/agent/container/lifecycle operation appears in the scoped production delta. Synthetic API-count assertions passed; this is not a live egress/host-effect audit.
- All 15 imported SVG bytes remain unchanged. The new SVGs contain only local outline `svg/path/circle` geometry and safe fixed attributes, with no scripts, XML entities, external references, fonts or embedded raster content. Wrapper names are exactly 15→17; packaging is exactly 24→26, not a wildcard. Source/README digests agree. [Static checks](review/static-checks.json) and 12 selected package-policy tests cover missing/empty/tampered/oversized/symlink/unknown-icon/font/opaque/raster inputs.
- Adjusted existing tests retain their safety and API assertions. Changes are chooser→Back interaction, required heading text, a bounded Tab search reflecting relocated actions, and pane-local scrolling. The previous exact two-Tab ordering assertion is relaxed to reachability; no mutation/origin/uncertainty assertion was removed. Controller/protocol and other existing safety tests are byte-unchanged.

## Required Checks Evidence

Exact argument vectors, working directories, environment overrides, times, results and log SHA256 values are retained in the linked machine records. Local Flutter commands use direct cached Dart/`flutter_tools.snapshot` with `--suppress-analytics --no-version-check test --no-pub`, `CI`, `DART_SUPPRESS_ANALYTICS`, and `FLUTTER_ALREADY_LOCKED`; no pub update, app/GUI launch, installation, live manager/provider/model operation, commit or remote mutation was performed by this review.

| Check / command or scenario | Expected | Actual / exit | Evidence |
|---|---|---|---|
| `python3 -B target/projects-layout/assurance/review_check.py before` — baseline canonical hash/archive/current-delta check | Exact 1,459-file archive/manifest; frozen paths unchanged | PASS; archive content/modes/names match; frozen and historical inputs unchanged | [baseline verification](review/baseline-verification.json) |
| `review_check.py run focused …` — Projects layout/view/navigation/controller/protocol (five files; full argv retained) | No failing tests; stale/current callbacks, immutable captures, uncertainty and deletion preserved | **149 passed**, exit 0 | [command](review/focused.json), [log](review/focused.log) |
| `review_check.py run visual …` — Projects + existing UI/entrance/Boot golden entrypoints | New layout matches; unchanged outer fixtures still match without updates | **30 test cases passed**, exit 0; all 8 new PNGs and all manifest-listed existing PNGs exercised by their entrypoints | [command](review/visual.json), [log](review/visual.log) |
| `review_check.py run neighboring …` — API decoder, shell, entrance, compact/split workspace, restore, queue/UI assurance (eight files) | No neighboring behavior regressions | **102 passed**, exit 0 | [command](review/neighboring.json), [log](review/neighboring.log) |
| `python3 -B target/projects-layout/assurance/static_checks.py` and its exact selected `unittest` argv | Exact asset sets/local-safe SVGs and fail-closed package policy | PASS static assertions; **12 synthetic policy cases passed**, exit 0; not an actual ELF/package acceptance claim | [static result](review/static-checks.json), [package command](review/package-policy.json), [log](review/package-policy.log) |
| Corrected ignored adversarial probes, `scratch-probes-v3` | Unsaved New/Back resume; actual keyboard reachability/focus; filter bookmarks | **2 failed / 3 passed**, exit 1: M1 at both text scales; keyboard and bookmark checks pass | [command](review/scratch-probes-v3.json), [log](review/scratch-probes-v3.log) |
| Six observational M1 confirmations | Confirm whether draft is erased vs retained but inaccessible; test missing and uncertainty variants | All six reproduce the accessibility/navigation defect; exit 0 is **not acceptance** | [command](review/defect-confirmation.json), [log](review/defect-confirmation.log) |
| `review_check.py after` and `finalize_review.py` | Same reviewed core; critical bodies and parent shared source hashes match | PASS: no core changes; seven critical sections unchanged; zero shared-core conflicts | [stability](review/source-stability.json), [critical hashes](review/critical-safety-sections.json), [parent comparison](review/parent-gates-observed.json) |
| Validation-agent `make DAGGER=target/installer-pipeline/smith-dagger-0.21.9/dagger test build` | Full stable-source test/build/export success | **FAIL**, exit 2; inner Cargo/Dagger exit 101. `fixture_spawn_waits_until_executable_writer_is_closed` times out at `service/tests/debug_service.rs:976`; Make stops before full build/export. Not independently rerun or waived here. | [observed record/digests](review/parent-gates-observed.json); parent [root record](validation/root-gates.json), [failure log](validation/root-gates.log) |
| Validation-agent `python3 -B dagger/package.py --root installer/dist --verify` | Current 26-asset package verifies | **FAIL**, exit 1: existing export lacks `git-branch.svg`; artifact binding lists both new glyphs missing and only 24 assets. The stopped build did not produce a reviewed replacement. | [observed binding](review/parent-gates-observed.json); parent [record](validation/package-verify.json), [log](validation/package-verify.log) |
| Validation-agent helper discovery with `SMITH_TEST_PACKAGE=installer/dist` | All helpers, including actual-package round trip, pass | **FAIL**, exit 1: 34 run, actual-package round-trip error against the stale export. Not skipped or treated as pass. | [observed binding](review/parent-gates-observed.json); parent [record](validation/pipeline-helpers.json), [log](validation/pipeline-helpers.log) |
| Validation-agent compiled `rust_projects_integration_test.dart` with its configured debug fixture | Seven compiled Projects cases execute/pass, no fixture skips | Observed **7 passed**, exit 0; log digest/source overlap verified. This is the separate agent's current execution, **not reviewer-run evidence** and not aggregate completion. | [observed binding](review/parent-gates-observed.json); parent [record](validation/compiled-projects.json), [log](validation/compiled-projects.log) |

Independent repository Flutter total: **281 passing test executions** across the three independent runs, plus 12 selected synthetic Python policy cases. This does not include scratch observations, parent executions, or historical passes, and does not cancel the two required UX failures.

**Inventory precision:** the supplied baseline has **28 unique pre-existing golden PNGs** (4 Boot, 16 UI-redesign, 8 workspace-entrance), not 31 unique PNGs. All existing golden test entrypoints were run and all 28 bytes/modes match baseline. No claim of three additional, unobserved image passes is made. New Projects PNGs are eight. The test-case count differs from PNG count because some cases capture multiple images and some entrypoint cases do not capture an image.

**Retained harness failures:** the initial scratch run included offscreen programmatic starting focus (not actual Tab traversal); the second scratch attempt could not identify an icon-only button using a text-only helper. These are reviewer fixture issues, not asserted product keyboard findings. Both raw logs remain [initial](review/scratch-probes.log) / [second](review/scratch-traversal-v2.log). The corrected fixture establishes visible initial focus and identifies icon-only controls, then uses only real Tab events for subsequent visibility; both keyboard cases pass in the unchanged `scratch-probes-v3` core. M1 failed before and after these scratch-only corrections.

## Evidence Gaps and approval limits

1. **Required aggregate/build/export closure is absent, not pending-PASS.** The concurrently produced gate records were available by the end of review and contain actual failures. Root full Flutter/analyzer/Linux build/package-export success cannot be inferred from partial Dagger vertices, independent focused tests, old exports or historical ledgers. The executable-writer timeout root cause was not investigated here and is not labelled flaky or layout-caused; that fixture/source is unchanged from baseline. Parent owns remediation/reruns, any needed scope escalation, and final stable evidence/artifact rebinding.
2. **M1 is unresolved.** No production fix, source-test alteration, golden regeneration, acceptance waiver or risk acceptance was performed by assurance. A changed core invalidates this source-bound review until follow-up evidence/review is rebound.
3. **Native UX remains unverified:** real Ubuntu window/IME and composition, accessibility-service traversal, fractional scaling/font-weight fidelity, window-edge/deactivation and native scroll/focus timing. The fixtures load the bundled Regular face; they do not attest production 400/500/600 weight rendering. The font assets and declarations are unchanged.
4. No installed migration/restore, live manager/keyring/provider/model, repository access, worker/container operation, clean-host build, release provenance/SBOM/publication or redistribution-rights acceptance was attempted. SVG geometry/provenance consistency does not independently establish authorship or clear the pre-existing imported-icon rights gate.
5. Canonical documentation, validation ledgers and final parent evidence may continue changing outside the frozen reviewed core. This report is a timestamped provisional source/UX assessment, not final whole-tree approval. [Machine summary](review/review-summary.json), [toolchain identity](review/review-toolchain.json) and [parent gate observation](review/parent-gates-observed.json) delimit the claims.

## Recommendation and required remediation

- **Verdict: fail.** The slice does not yet meet the compact return/recovery acceptance requirement, and mandatory aggregate/build/package verification is not passing.
- **Release Gate Decision: no-go.** No merge/release/deployment/host/publication authority is granted by this review.
- **Required remediation:** fix M1 within the existing presentation-only boundary; add source regressions for unsaved/missing/uncertain editor resume; preserve write/origin/replay safeguards; resolve the required gate failures and build a fresh 26-asset export; then rerun and rebind parent full gates and fresh independent review to the exact final source/artifacts. Only the operator can authorize any additional effects or accepted risk.
