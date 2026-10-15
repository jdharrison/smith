# LOCAL-SMITH-NEXT-INSTALLER-REDESIGN

## Status and intent

**Implemented bounded presentation slice — option A only; human visual acceptance pending.** On 2026-10-14 the operator requested the Flutter redesign's visual language in the installer, then selected: “lets keep as-is for now and just redesign the ui/flow a small amount to match new design ui/ux and assets”. This authorizes local presentation/assets/tests/builds, not changes to consent granularity. Preserve all current approval steps, model-selection preparation and operation semantics. Option B below is deferred and is not authorized. No live installation, GUI/service/model operation, commit or publication is authorized.

Scope: the native GTK `next/installer` GUI on Ubuntu, not a Flutter rewrite, CLI redesign or multi-chat implementation. Preserve the worker, filesystem/identity checks, cancellation, effect accounting and service ownership. Inspected base is `2cc71a94fe924fbb9d324f29332e9f24c8c5991c` on existing `feature/autonomy`, with concurrent uncommitted client/service/package/documentation work. No installer source was modified during planning. Styling/operator flow is elevated; changing approval granularity is critical and needs explicit policy approval plus architecture/adversarial review.

## Implemented baseline and source evidence

- `next/installer/src/ui.rs` uses GTK3, near-black/green CSS, a 720×560 default window and embedded narrow-layout tests. It has Landing, Inspecting, Review, Model, Working, Confirmation, Stopping, Finishing and Terminal internal phases.
- There is **no literal Confirm results step**. `finish()` displays a report after joining the worker; Terminal offers Close and, when allowed, Manage models. Close is not an approval. Automatic progress, checking and finishing require no acknowledgement clicks.
- `render()` creates `Confirm {pending.title}` for a pending effect. The separate model check in `wizard.rs` performs actual inference, not acknowledgement of a download. The wording and repeated explanation make these distinctions harder to see.
- Successful step rows are already filtered; routine diagnostics are retained but not attached to the normal page. Repeated report introduction/outcome/inference paragraphs still clutter completion. Partial results can contain many important effects and must not be reduced to an inaccurate Done.
- The normal fresh path with missing default model/no saved selection is nine activations before completion: choose Install/Upgrade; approve artifacts; Register; Enable; Start; Continue with exact model; Download; Save preference; Test model. Seven are approvals, two navigation/choice. Close is one additional activation. Optional chooser/detail interactions are excluded.
- Running/current-enabled Upgrade normally takes four activations before result (choose, artifacts, Stop, Start); running/registered Uninstall normally takes four (choose, artifacts, Stop, Unregister). Actual state changes these counts. Uninstall's prepared Unregister includes disable-and-verify, not an extra independent Disable page.
- Existing verified lifecycle facts, installed weights and unchanged saved selection already skip their unnecessary operations.
- `wizard.rs::action` consumes a fresh, original, action-bound Core token; later actions depend on earlier outcomes. They cannot all be preprepared against the initial state or approved by automatically replying to later dialogs.

Evidence: `next/installer/src/ui.rs` (`Widgets::new`, `event`, `finish`, `render`, `connect`), `next/installer/src/wizard.rs` (installation/model sequence and `action`), `next/installer/src/maintenance.rs`, and canonical Architecture/Design/Process. Click counts are source-derived, not executed live traces.

## Independent analysis and reconciliation

Planning contexts: producer `fc686d57-2e79-4bf9-9ba5-36b719d91da7`, architect `d80e81ce-9b2d-4364-be22-50eee6fa165f`, designer `6c4b2c5c-e02e-40b6-b190-6327548104b8`. These are independent planning inputs, not implementation assurance or approval.

| Issue | Reconciliation / owner | Gate |
|---|---|---|
| Same UI versus new toolkit | Use existing Flutter tokens/assets as reference; keep Rust/GTK. UI owner. | No Flutter/WebView migration. |
| Results appear to require confirmation | Show one concise terminal outcome; no acknowledgement beyond Close. Preserve scope and uncertain effects. UI owner. | Close sends no consent; worker must be joined. |
| Producer proposes removing model Continue; architect flags changed preparation contract | Do not treat this as styling. A model choice is not consent to a later unseen effect. Preparation/invalidation requires reviewed worker changes. Architect. | Explicit decision below; not automatic Approve. |
| Fewer pages versus fewer approvals | Three presentation surfaces can retain all current approvals. Material click reduction requires task-scoped consent policy, not cosmetic relabeling. Operator/architect. | Explicit approval of granularity before implementation. |
| Concise exact consent | Avoid duplicate prose. Current path keeps existing authorization-bearing facts visible; typed summaries must not infer effects by parsing English. Architect/designer. | New disclosure policy needs approval if moving consent-critical fields out of normal view. |
| Offline fonts | GTK CSS alone cannot load Flutter fonts. Use installer-owned licensed assets and verified process-local registration. Build owner. | No system font install or runtime download; native loading spike before visual acceptance. |
| Concurrent multi-chat | Installer files are separate, but service build inputs, exports and canonical docs are shared. Coordinator. | Stable source/ownership before combined gates; preserve v1 installer compatibility. |

## Target visual and interaction design

**Proposal:** three visible surfaces, with internal safety phases retained:

1. **Choose:** `> SMITH`, `INSTALLER`, real installer version from Cargo metadata; Install / Upgrade and Uninstall. No invented online/GPU status or extra navigation rail.
2. **Action:** one stable panel for checking, concise review/decision, model choice and progress. One title, only relevant facts, one primary action and at most one secondary. Advance through successful checking/progress automatically; never advance across an unapproved effect.
3. **Result:** one scope-specific heading, essential facts and safe next action. Close plus optional Manage models. No Confirm results, Continue after success, chronological success log or automatic app launch/close.

Use client background `#031514`, surface `#04201D`, raised `#092A27`, accent `#00E9C6`, focus `#00FFE0`, text `#E3F0EE`, secondary `#99B5B2`, corrected muted `#789290`, danger `#FF4D58`. JetBrains Mono, flat surfaces, thin outlines, 6px radii, 8/16/24 spacing, ≥48px hit targets. Keep native decorations. No glow/gradients, fake terminal transcript, chat sidebar or service polling for decoration.

Retain 720×560 default and existing 480×420 minimum initially; support enlarged text with a scrollable body and reflowing fixed footer. Bounded ~640px reading column at larger sizes. Fonts must be bundled with notices and used offline, not assumed installed. Reuse only necessary icons; existing supplied-icon redistribution rights remain unresolved. A label-first UI does not require importing the entire icon set.

### Copy and results

- Prefer action labels such as Install Smith, Enable at login, Start Smith, Download model, Use model, Test model and Remove model over generic Confirm/Continue.
- Display each actual effect once. Keep important target, interruption, download size/source, data-retention and destructive meaning explicit. Do not truncate consent, invent an expiry countdown or hide failures to achieve a word budget.
- Working shows a factual stage, elapsed time and real download bytes/percentage; other stages are indeterminate. Remove redundant disabled Installing buttons. Cancellation remains reachable and does not promise rollback or cancellation of accepted service work.
- Successful setup: `Setup complete` with only verified app/service/model facts. Upgrade: `Upgrade complete — service ready; models not checked.` Uninstall: `Smith uninstalled — your data and preserved files remain.` Model-only outcomes never claim installation complete.
- Failures/partial/unknown results show the specific cause, completed/uncertain effects, what remains and safe next action. No generic Done, automatic retry or cleanup. Sticky cancellation still requires reopening.
- Obtain any new concise outcome fields from typed worker state, not prose matching, if existing report data cannot support a truthful summary.

## Signoff decision: approval granularity

### A — approved presentation-only implementation

Apply the visual design, remove duplicated text and disabled pseudo-actions, show compact outcomes, and retain separate existing artifact/Core/model approvals. No new effects or policy changes. Fresh setup still has seven effect approvals; changing the number of visible pages cannot honestly promise fewer clicks. The model Continue gate stays until its preparation contract is separately redesigned.

### B — deferred proposal, not authorized

Authorize **one bounded task**, rather than asking for every internal step. Proposed initial scope:

- **Install & set up:** one displayed authorization for the exact app/service artifacts, registration, Enable at login and Start. No download, inference or destructive maintenance is implicitly included.
- **Model setup stays a separate decision:** exact model, download size if needed, saved-selection change and any explicitly included test must be disclosed. Combining those model decisions also requires explicit approval; it is not inherited from Install.
- Upgrade/Uninstall/model removal remain distinct operator-selected tasks with visible interruption/destruction/retention effects. Their existing separate approvals remain unless a separately reviewed task-consent contract explicitly covers them.

For fresh installation, grouping only artifacts/Register/Enable/Start would replace four approvals with one: **nine → six activations before completion**, assuming the same default-model path. Further model preparation/grouping can reduce this, but no three-click or one-click guarantee is made before its contract is approved and tested. Model choice itself must never approve later unseen effects.

**B is not implementation-ready authorization.** First define an immutable displayed task identity, target artifact/root/unit/model bindings, explicitly permitted action set, expiry and cancellation rules, and a typed task-consent object. Later fresh Core preparations must still validate current facts and original-token expiry. Changed targets, new effects, expired authorization, unknown outcomes or conflicts stop; no blanket yes, fabricated approval responses, replay, automatic reprepare of an expired action, or consent carried into another session. Define handling of legitimate intermediate state changes versus unauthorized drift. Preserve CLI behavior and all lower-level locks/identity checks. Update applicable Scope/Architecture/Design/Process text in the same approved policy change; do not quietly contradict the separate-consent rules.

Operator selected A. Implement only a small visual/copy/layout pass: theme/header/real version, bundled fonts and minimal icons, reduced repeated explanation, progress without disabled pseudo-actions, and cleaner scope-specific results. Keep raw exact consent and failure/uncertainty facts intact. No `wizard.rs`, service, CLI or approval contract changes. Resource loading uses process-local assets only. `ui.rs`/embedded tests and installer resources are owned by this work; the parallel `tests/multi_chat_compatibility.rs` and client/service changes remain untouched.

## Requirements, tasks and evidence

| Task | Requirement / acceptance | Write scope / dependency | Required evidence |
|---|---|---|---|
| P0 T1: freeze decision/ownership | R1/AC1 native visual parity; R2/AC2 explicit consent granularity | This plan, coordinator; confirm A/B and inspect concurrent changes | No invented approval; base, owners and protected files recorded. |
| P0 T2: resources/theme | R1/AC1 local real fonts, palette, real version | `next/installer/src/ui.rs`, proposed GUI-only resource module/assets; narrowly justified Cargo/build inputs | Offline font/weight/load and no persistent host writes; GUI help/version display-free; CLI stays GTK-independent; provenance/notices. |
| P0 T3: concise surfaces | R3/AC3 no redundant navigation/results prose; R4/AC4 truthful recovery | `ui.rs` and embedded GTK tests; typed display fields in `wizard.rs` only where needed | Three visible surfaces, exact effect/action counts, Close sends no consent, no effects on open/selection/resize, complete/partial/unknown scope correct. |
| Conditional P0 T4: task-consent architecture | R2/AC2 minimum clicks without inferred authority | Only after B approval; architect specifies worker/control compatibility and narrowly bounded files before editing | New contract plus policy reconciliation; no auto-Approve workaround; negative drift/expiry/replay/cancel tests. Blocking architecture objections preserved. |
| P0 T5: accessibility/visual QA | R5/AC5 legible minimum/enlarged layouts | Installer tests and synthetic captures | Readable exact effects; keyboard/focus, ≥48px targets, contrast, body/footer reflow, no held-Enter approval of next request, reduced motion. |
| P0 T6: integration/self-check | R6/AC6 stable paired candidate | Coordinated scripts/package/docs; do not edit multi-chat code | Locked fmt/Clippy/tests, isolated GTK, actual native package tests/build and fresh verification on unchanged inputs. |
| P0 T7: independent review | R2–R6/AC2–AC6 | Fresh assurance/adversarial context | Review actions/click trace, safety negatives, images and candidate hashes; then human visual acceptance. |

All functional requirements are P0. A decorative polish deferral needs a named owner and follow-up before acceptance; consent correctness, accessible controls and truthful outcomes are not cosmetic deferrals.

### Validation plan — not executed during planning

Use synthetic sessions/private roots, not the live installer or service. Capture Choose, review/each approval type, model choice, download/indeterminate progress, cancellation, successful scope-specific results and long partial/unknown failures. Minimum matrix: 720×560 and 480×420 at 100%/200% text; 1024×420 short window; 1440×900 and ultrawide bounded-width samples; reflow boundaries and 1×/2× plus available fractional-display environment. Label GTK logical/physical geometry and fonts. Visual inspection and keyboard/accessibility checks supplement assertions.

Required behavioral gates: no effects on open, request/target identity, stale/double/key-repeat rejection, original expiry, decline/partial preservation, no automatic retry/rollback, worker join, bounded progress/event ordering, curated models and unsupported selections, loaded/selected removal refusal, multi-chat BUSY refusal and installer v1 compatibility. In B, prove exact admitted action-set coverage and rejection of every unapproved later action or changed target.

After implementation and an agreed stable build window, from `next/`:

- `DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make installer/ui-test`
- `DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make installer/test`
- `DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make package/test installer/build`
- `python3 dagger/package.py --root installer/dist --verify`
- Final combined `make test build` only after parallel integration stabilizes.

Record actual commands/timeouts/results, source/asset/artifact hashes, screenshots and review in a focused evidence record plus `next/VALIDATION.md`. Failed/stale exports and historical passes are not current validation. No validation commands were run for this planning record.

## Implementation outcome and validation

**Implemented:** native GTK teal/cyan theme, real Cargo version, embedded JetBrains Mono 400/500/600, selected supplied PNG icon derivatives, direct exact artifact review, concise action-specific copy, wrapped model controls, adaptive footer/bounded reading column and reduced redundant progress/result presentation. All current consent, model preparation and worker/lifecycle behavior remain. GUI `--licenses` prints full embedded notices without display/package discovery; CLI and worker files remain unchanged.

Observed final source checks: 252 native tests plus 9 GTK tests passed; one intentional native probe test ignored. Formatting/warning-denied Clippy and the no-interpreter CLI runtime gate passed. Native package tests passed all 41; fresh paired installer build and verification passed with 95 manifest entries/97 files. Two screenshot runs at 1× and 2× display scale each passed all 9 GUI tests and retained 141 images (282 total), including 100%/200% text and scroll tiles. The [image index](../../next/installer/tests/fixtures/ui-redesign/README.md) identifies current reviewed captures and historical attempts.

The initial full installer run failed an unchanged CLI fixture's internal finite budget. Two subsequent standalone runs passed without code/budget changes; the first failure is preserved and its root cause remains undetermined. [Evidence](LOCAL-SMITH-NEXT-INSTALLER-REDESIGN-EVIDENCE.json) binds 201 before/after source entries and 97 exports. The subsequent validation-history edit is explicitly documentation-only. Fresh [assurance](LOCAL-SMITH-NEXT-INSTALLER-REDESIGN-REVIEW.md) returned **pass-with-risk for the scoped source/package slice**, with no new implementation blocker; this is not human risk acceptance or release approval.

Remaining: human visual acceptance, real desktop/keyboard/screen-reader/fractional-display checks and icon redistribution rights. The separate whole-product aggregate runner failure recorded by the multi-chat work remains unresolved and is not cleared by these installer results. No host installation, GUI/service/model operation, commit or publication occurred. T4/grouped consent remains deferred, not silently implemented.

## Stage gates and implementation handoff

1. **Plan → develop:** operator selects bounded A/B. B additionally requires architecture contract and normative policy reconciliation before coding. No implied permission to weaken consent.
2. **Develop → self_check:** implement only assigned installer files; preserve concurrent client/service/package/docs work. Recheck owners before shared edits.
3. **self_check → assurance:** required tests, images and stable package evidence pass; retain failures and gaps without substituting old exports.
4. **Assurance → human review:** fresh review verifies source, outcomes, click traces and visual evidence. Do not self-approve.
5. **Release/deploy:** separate exact approval, no failing/missing required evidence; icon rights and installed-host QA remain independent gates.

Rollback is limited to owned presentation/resources/approved consent-contract changes, not whole-worktree resets or reverting the parallel multi-chat storage migration. Rebuild the current compatible pair. No automatic installation, GUI/service/model operation, commit, push or release.
