# LOCAL-SMITH-DEV-JOBS-LIST

## Authority and intent

**LOCAL_PLAN_APPROVED for bounded UI/controller source implementation and isolated validation:** the operator requested Dev become a list of pipeline jobs/statuses, with the start prompt in its own modal and clicked-job details in a modal. This replaces the single inline form/current-result presentation only. The operator also noted jobs cannot be stopped; discovery confirms there is no stop/cancel/reconcile API. This slice must not pretend to solve that separate capability gap or clear retained uncertainty.

Repository Smith, existing `feature/autonomy` bootstrap branch, HEAD `14d960975e76f906efb5e447ff3dece0e8e4385b`, with existing dirty/untracked work preserved. Owner: developer/planner. Approval expires on completion or material drift. Scope: Dev client/controller/tests and affected canonical documentation. No service/wire/disk schema change, added execution slot, scheduler, cancellation/reconciliation, runtime/provider/network change, installed upgrade, Docker action, job submission, commit, publication or deployment is authorized. New and historical data remain service-owned.

## Independent analyses and reconciliation

- **Producer:** all v2-returned jobs must remain visible, new jobs require explicit origin-bound consent, and detail selection cannot release admission blockers. `used` includes legacy records not returned as v2 rows. No fabricated prompt titles/legacy rows. A list is not a solution for missing Stop.
- **Architect:** expose immutable retained jobs and keyed/coalesced detail state without a shared selected ID. Preserve operational `current`/`detail` compatibility, but derive polling/blocking from all authoritative rows, not browsing. Scope correlation to request/project/instance/generation; resolve pending original GET first, then consistent state, before retiring its identity. Preserve sticky disappearance fencing and both actual `UNKNOWN_REQUEST`/legacy fixture spellings. Historical reads never reset the60x2s poll budget.
- **Designer:** compact Pipeline jobs + New job, responsive labelled rows and scrollable accessible modals; start uses Edit -> Review -> awaiting acknowledgement in one modal route. Retain volatile drafts outside the route. Close means detach, never cancel/stop. Detail shows coherent frozen metadata/reply and distinct cleanup certainty; no invented original prompt or progress.
- **Planner:** preserve the producer's unresolved Stop concern explicitly. Implement the requested presentation and client observation corrections, not backend authority. Responsive labelled rows may reflow rather than introduce a horizontally scrolling data table. Opening New job may prepare a draft while blocked, but Review/Start remain disabled with factual reasons. A pending admission is a banner, never a fabricated saved row. Historical receipt records remain capacity-only in this v2 view. Existing source-only form/no-browser descriptions are superseded only for the new local-runner presentation.

## Implementation contract

### Controller handoff

Keep `refresh/capture/start`, `current/detail`, admission/uncertainty fields and original-request guard. Add:

- `List<DevJob> get jobs`: immutable, newest-created first with stable request-ID tie-break; immediate acknowledged upsert; failed refresh preserves stale rows.
- `bool get refreshing`, `bool get jobsStale`.
- `DevJobDetailState detailState(String requestId)` with immutable `data`, `loading`, `stale`, `error`.
- `Future<void> loadDetail(String requestId, {bool force = false})`: read-only, known-ID only, cache/coalesce per captured identity/instance/generation; errors stay keyed except proven contradictions/disappearance fence admission. Browsing never assigns operational current/detail, resolves pending or establishes freshness.

Retain all known/acknowledged request identities; a verified list omitting one or an unknown GET for a known record is a sticky protocol block, never eviction. Pending recovery is original GET then state on one consistent observed instance and membership/project binding; unknown does not establish non-admission. Model/project revision/readiness, shared admission, combined32-record capacity and no-resend rules remain intact.

Operational observation follows the active row even when the compatible `current` is a terminal resolved request. Refresh active details and invalidate/reload cached details on summary changes; modal browsing creates no timers. Preserve60 automatic observations every2seconds; force/history reads never reset/rearm the exhausted budget. Reject late cross-instance/disposed/generation results; mutable phase fields need not match across legitimate state/GET races.

### View handoff

- Main page: small Pipeline jobs heading, New job, existing read-only Refresh/Resolve, factual load/empty/stale/capacity/uncertainty messages. List all current v2 rows keyed by request ID, with project key, full request identity, text phase, created UTC, reply availability and cleanup verification. Never show unavailable as empty.
- New job modal: per-view retained project and exact TextEditingValue, authoritative current LOCAL model/name/digest, prompt limits/IME, Edit -> Review in the same route, unchanged immutable context/repository/network/local inference/retention disclosures, explicit Start job. Close available including pending; clearly not cancellation. Retain drafts on refusal/unknown; accepted acknowledgement clears only unchanged submitted typing and closes only its still-open originating route. Closing/reopening or hiding/disposing cannot pop another route or silently dispatch.
- Job detail modal: request-keyed loading/error/stale/refresh, coherent saved summary/message/session, frozen model/digest, failed stage, repository commit evidence and selectable inert reply. Empty evidence means no recorded checkout evidence, not no configured repositories. Close always reachable; no Stop/Delete/Retry/Re-run/clear controls. Active/unknown details explicitly explain that stop is not available and retained uncertain cleanup requires separate reconciliation.
- Preserve shell/navigation/Projects/Agents/Chat/theme, preferences format and session-only behavior. Drafts/selections/cache are volatile, never preferences. Keyboard row activation, newline/IME safety, wrapping48px actions, narrow/enlarged/short modals and source-pane visibility guards are blocking.

## Traceability / validation

| ID | Acceptance | Required evidence |
|---|---|---|
| LIST-1 | All returned jobs retained/listed, stable order/upsert/status updates; used includes invisible legacy capacity. | Immutable multi-job controller + list/capacity/empty/stale widget fixtures. |
| LIST-2 | Only reviewed valid explicit Start submits once, preserving exact draft, revisions, consent and original-request uncertainty. | Adapt existing start/IME/drift/newer-typing/hidden-origin tests to modal flow; duplicate/closed-route cases. |
| LIST-3 | Click exact row -> correct coherent details; late A cannot replace B; history selection never changes admission/current/polling. | Keyed cache/coalescing/cross-project/cross-instance/disposal tests and modal races. |
| LIST-4 | Active/unknown/cleanup-unknown block globally; resolve original only; no fake Stop/retry/reset; bounded polling unchanged. | Negative operation traces, missing non-current IDs, actual unknown spelling, budget and live-detail updates. |
| LIST-5 | Accessible responsive rows/modals; no neighboring regressions. | Narrow/enlarged/keyboard/focus/scroll/hidden-pane tests, scoped/full analyzer, Linux client build and selected navigation regressions. |

Run formatting and focused controller/widget/domain tests first, then full client analyzer, selected navigation/restore regressions and Linux build. Preserve exact failures/skips and stable source identities; obtain fresh independent adversarial/source review. Full configured compiled integration/root/package, installed/native graphical/IME acceptance remain separate required gates for their claims—not inferred from focused widgets. No live service or Docker is a fixture fallback.

## Evidence

**Implemented source; focused tests/analyzer/Linux build PASS; selected combined regression gate FAIL/BLOCKED.** No overall acceptance/release PASS, stop capability, reconciliation, installation or live execution is claimed.

### Implemented touchpoints

- `client/lib/ui/dev_jobs_view.dart`: retained keyed jobs/status rows, per-view Edit/Review/Start and read-only detail routes, retained TextEditingValue/project drafts, exact-route/visibility ownership, coherent metadata/reply/cleanup and responsive48px controls.
- `client/lib/dev_jobs_controller.dart`: immutable summary list/upserts, keyed coalesced detail snapshots, instance/generation/disposal guards, pending GET/state consistency, all-known disappearance fence and active-row bounded polling independent of browsing/compatibility selection.
- `client/lib/dev_jobs.dart`: client-only `DevJobCorrelationFailure` preserves proven wrong-request GET evidence through the existing native validator, retaining wire code `PROTOCOL`; no service/wire/schema change.
- `client/test/dev_jobs_test.dart`: existing17 domain/controller/widget cases preserved and modal entry flows adapted. New `dev_jobs_controller_list_test.dart` and `dev_jobs_list_view_test.dart` add41 controller and22 list/modal cases, respectively.

### Parent validation on final source

All commands below ran from `client/` with existing dependencies and `--no-pub`; no live service/Docker/provider fallback or graphical launch ran. Scoped source hashes matched the final review identities after execution. Repeated counts belong to overlapping invocations, not additive unique coverage.

| Exact command / check | Actual outcome |
|---|---|
| `flutter test --no-pub test/dev_jobs_test.dart test/dev_jobs_controller_list_test.dart test/dev_jobs_list_view_test.dart --reporter expanded` | Final80 passed,0 failed/skipped; exit0 |
| `dart format --output=none --set-exit-if-changed lib/dev_jobs.dart lib/dev_jobs_controller.dart lib/ui/dev_jobs_view.dart test/dev_jobs_test.dart test/dev_jobs_controller_list_test.dart test/dev_jobs_list_view_test.dart` | PASS;6 files,0 changed; exit0 |
| `flutter analyze --no-pub` | PASS; no issues; exit0 |
| `flutter build linux --no-pub` | PASS; Linux source bundle built, not installed; exit0 |
| `flutter test --no-pub test/dev_jobs_test.dart test/dev_jobs_controller_list_test.dart test/dev_jobs_list_view_test.dart test/dev_navigation_test.dart test/workspace_restore_test.dart test/workspace_preferences_test.dart --reporter expanded` | **FAIL:**152 passed/13 failed; exit1 |
| `flutter test --no-pub test/workspace_restore_test.dart test/workspace_preferences_test.dart --reporter expanded` | **FAIL:**72 passed/6 failed; failed test process retained even though following hash command returned0 |
| `flutter test --no-pub test/workspace_preferences_test.dart --reporter expanded` | PASS;57 tests; exit0 |

Initial implementation attempts had analyzer/style and widget-harness failures, fixed before the initial73-pass parent focused run. That initial run and successful analysis/build did not reveal the two later adversarial findings below. Final80-test, full-analyzer and build results bind the revised source, not the earlier snapshot.

### Independent findings, fixes and limits

Fresh independent UI/modal assurance found no scoped blocker in route retirement, close/reopen/late acknowledgements, exact/newer typing, IME and review drift, coherent details, no fake Stop, keyboard/focus and compact/text2 assertions. It inspected widget definitions, not native graphics or screenshots.

Fresh controller assurance initially found two medium-priority defects: native validation could flatten wrong-request GET evidence into a generic keyed error instead of sticky fencing, and an active original GET followed by clean terminal state could strand admission behind stale compatibility current with no timer. The implementer reproduced five new regression failures, added seven production-path/terminal-transition cases and fixed both. The actual NativeAppApi/synthetic AppExecutor path now retains the correlation subtype; admission relies on authoritative all-row state rather than stale current. Re-review independently closed both at the final hashes; it did not duplicate parent test execution.

A separate read-only triage ran `CI=true FLUTTER_SUPPRESS_ANALYTICS=true flutter --no-version-check test --no-pub --reporter expanded --concurrency=1 --timeout 30s test/dev_navigation_test.dart`: **0 passed/7 failed**, exit1. Four first failures expect the old placeholder; two assume hidden Agents links; one assumes a separate compact Dev control. Source corroboration: the unchanged shell hash matches prior navigation-hiding evidence. The remaining six combined failures are restore expectations around old normalization/read allowlists; the isolated two-file run preserves those failures. These checks do not establish a new list/modal defect, but early exits/stale fixtures prevent claiming the selected gate green. No executable pre-task control was run; do not present source corroboration as a before/after reproduction. These unrelated fixtures were not rewritten or assertions weakened in this slice.

### Final source binding

```text
30c37019e90f57ef2a4b7f8939ffad9b96a8b042eefe2581523534ec7dfadeae  client/lib/dev_jobs.dart
4f8d16fb27197739d38fe1d7fde3dbac682cabc3bdd71a238677b848cdfa2539  client/lib/dev_jobs_controller.dart
d690be3ddcad126c35fb01481f8ebedc1593aee5e6ca65f0a226441e9f4d7045  client/lib/ui/dev_jobs_view.dart
37e6be4362ef96b802b8cb58634f33593851f992f4c396915dad877b4fa6e7a4  client/test/dev_jobs_test.dart
1890238170f67a9e6d980908a91ae13b231cc135ea763d00836363154a59a0da  client/test/dev_jobs_controller_list_test.dart
df2668c2fc6c5ff208b45b3af21340db3139f3e83e5efaa5c35e886d409ae899  client/test/dev_jobs_list_view_test.dart
```

The six-file binding is not whole-candidate/clean-source provenance. Full configured compiled integration/root/package and native installed graphical/IME/accessibility evidence remain missing; the selected combined failure remains blocking, not accepted risk. No service, Docker, model, installed files, historical ledger or preferences were changed. Later deployment must preserve the verified paired client/helper/service upgrade, not a standalone binary swap.
