# Saved-key attempts — independent assurance/adversarial review

## Validation scope

- Fresh independent source/evidence review of `LOCAL-SMITH-NEXT-SAVED-KEY-ATTEMPTS`, repository `smith`, existing dirty `feature/autonomy`, HEAD/base `7f6a483ae82c4bcf647268f035f1e7cc80409d4b`. Read `AGENTS.md` → `docs/SCOPE.md` → relevant Architecture/Process/Design/Host Profile → `roles/assurance.md` → approved plan, source, tests and retained evidence. No builder reasoning transcript used.
- The operator explicitly supersedes **only eligibility**: an active saved exact credential generation permits a supported-model Send/Validate attempt without current-instance observation or previous successful validation. Actual credential retrieval/identity, consent, deadlines, uncertainty, reservations and no-replay remain requirements. Discovery/model scanning, persisted catalog/cache and upgrade/uninstall lifecycle proposals remain analysis-only and excluded.
- Reviewed the actual **14-file scoped delta**, not the broad HEAD diff: five service/installer files and nine Dart files. Independently matched baseline hashes, regenerated both supplied patches, and recovered the two omitted client test originals from earlier retained evidence as described below. The production slice is four files: `app_v3.rs`, installer `providers.rs`, Dart `cloud_routing.dart` and `ui/cloud_selector.dart`.
- Reviewer actions were file/image inspection and bounded read-only local Git/Python comparisons. No tests, builds, package-verifier execution, artifact execution, application/container/manager calls, normal-session bus/keyring/credential access, network/vendor calls, installation, cleanup, commit or branch mutation. Only this new report was written; its parent and absence were verified. Initial evidence cutoff: **2026-10-16 07:11:30 UTC**. Later remediation/results require renewed binding, not inference from this report; the H1 follow-up below supplies that bounded remediation binding.

## Findings, severity ranked

### High — H1: required full client gate fails on three obsolete unavailable-cloud fixtures — CLOSED

**Disposition update:** the test-only fixture correction and its targeted source-bound evidence are independently verified in the H1 follow-up below. The initial finding, reproduction and remediation criteria are retained here as history. Closing H1 does not convert the original full-client execution into a PASS or close G1.

**Actual failing required verification, localized to test fixtures; no production defect demonstrated.** The parent's completed `make client/test` exited **2**, with **663 passed / 3 failed**, on a stable 1,291-input snapshot. The failures are:

1. `selected-route entrance recovery healthy selected local route still animates when an unselected optional destination is unavailable`;
2. `selected-route entrance recovery selected openai route becoming unavailable mid-entrance settles permanently`;
3. `selected-route entrance recovery healthy selected openai route still animates when an unselected optional destination is unavailable`.

`next/client/test/workspace_entrance_test.dart:228–247` defines `_AvailabilityCloudFake`, which changes a model's `available` to false and supplies an error, but leaves the provider and route's saved generation intact. Its base catalog still reports configured credentials and successful exact-model validation (`cloud_routing_test.dart:18–57`). This is no longer a valid cloud catalog under the approved contract. `RouteCatalog` correctly rejects it at `cloud_routing.dart:219–224`, rather than turning an adverse observation into an eligibility gate.

The two optional-destination cases fail during `readyCloud`/catalog synchronization (`workspace_entrance_test.dart:1251–1261`); the selected-cloud transition injects the invalid catalog at `:1214–1217`. Raw diagnostics report `PROTOCOL: Invalid v2 application data` through the shared validator; that wording is not evidence of a schema fallback.

**Reproduction/evidence:** the retained execution was `env DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make client/test` from `next/`, 07:05:48–07:07:08 UTC. See `next/service/build/saved-key-attempts/client-test-status.json`, `client-test.log:1315–1407` and its final failure list. Source inventory before/after is `7ff716129095837394ed5a102eca56d126fbce6c94ee3f68b604fdba4d5955e0`; log SHA-256 independently matches the status record. The reviewer did not rerun this command.

**Required remediation:** adapt the fixture to a valid unavailable identity/catalog state, such as a provider with no active generation or an absent historical route, while retaining the entrance-settling, healthy-other-route, no-remount, focus, no-effects and no-fallback assertions. Preserve the local unavailable-route case. Add/retain the distinct assertion that a configured cloud route with adverse cached facts stays eligible. Do **not** relax the new iff predicate, restore observation/validation gates, delete/skip these tests, or alter Boot/controller behavior merely to make the suite green. Independently review the bounded test delta and its new source-bound results. The original failed execution must remain retained.

### High — G1: final combined/package acceptance is incomplete; historical aggregate failures remain unaccepted

TRY-R5 and `docs/PROCESS.md:106–116` require final stable configured component/combined/build/package evidence and paired artifact binding. Direct service/installer and focused client passes do not satisfy those gates. At cutoff, the only completed parent status in this task's evidence root is the **failed** full client gate above; no final saved-key aggregate/build/package result or bound rebuilt recipient package was supplied/verified.

`next/VALIDATION.md:3–23` also retains the preceding catalog aggregate's `RUNNER_BUSY` bounded-lease failure and the schema task's teardown lock-reacquisition and Vulkan full-window timing failures. Those are distinct historical failed executions, not accepted risks, fixes, or proof of harmless flakiness. The current direct 263-service-test pass, including the runner suite, does not explain or erase them. This amendment changes neither runner production nor runner tests.

**Required disposition:** supply actual final command results, raw logs, source inventories and paired artifacts for fresh review; retain every failed/skipped/incomplete outcome and its exact source identity. Separate any new component/package PASS from broad aggregate disposition. Do not inherit the preceding task's package PASS or claim a running command succeeded. This report requests no unrelated runner repair, indefinite aggregate retries, risk acceptance or live-host action.

**Critical: none identified. Medium/low: none identified in the reviewed slice.** H1 was a concrete test-integration blocker; its fixture remediation is now closed by the bounded follow-up below. G1 remains an acceptance/evidence blocker. No concrete security or execution-boundary defect was identified in the four-file production delta; that is not an overall assurance pass.

## Exact delta and adversarial assessment

### TRY-R1 — eligibility and strict DTOs

**Implemented, source-limited:** `next/service/src/app_v3.rs:686–724` retains supported provider/model checks, configured `CredentialRef`, exact generation matching and the unchanged local inventory/digest requirements. Its sole production hunk removes only the cached observation match and prior-successful-validation check. `route_catalog` still obtains advisory provider facts separately and computes each model's eligibility from `ready` (`:1035–1075`). No probe, keyring call or HTTP is introduced into these reads.

Both catalog readers enforce **cloud `available` iff the provider generation is non-null**, in addition to exact route/provider-generation equality:

- Dart: `next/client/lib/cloud_routing.dart:219–224`;
- installer: `next/installer/src/providers.rs:254–274`.

This is an iff relationship, not merely permission for `available=true` in more states. A configured choice falsely disabled by an obsolete cached gate is rejected, as is an available choice without a generation. Both readers still validate required nullable fields, exact keys/types, provider order, bounded IDs/revisions/labels/errors, supported provider/model membership, grouping/duplicates and generation-correlated validation. Local availability remains inventory-dependent. Validation success flags still require success for the **exact selected model**, with its generation checked in the provider view; availability does not set or erase those facts (`cloud_routing.dart:74–169,176–228`; installer `providers.rs:171–277,356–449`).

New matrices cover both providers × six observation states × four validation states, including unobserved/locked/missing/conflict/unavailable and null/failed/interrupted/succeeded validation. Negative cases retain missing/stale generations, wrong/unknown models/provider, malformed DTOs, inconsistent errors/verification and forbidden extra fields. H1 is an unadapted consumer of the old fixture contract, not justification to weaken these checks.

### TRY-R2/R3 — fresh credential boundary, one attempt and recovery

**Implemented boundary unchanged:** the initial inventory independently matches current worker, keyring, HTTP, cloud adapter, storage/domain, controller and queue inputs. In `next/service/src/app_v3_worker.rs`:

- `credential_identity` checks the exact admitted reference and provider revision against owned running work (`:62–76`).
- `key` freshly reads that identity under the original operation bound and three-second keyring bound, sanitizes errors, respects cancellation, and updates only factual observation metadata (`:78–105`).
- `cloud_completion` reads the key, rechecks identity, durably publishes the dispatch marker, rechecks original permission/deadlines after publication, and calls the HTTP adapter once (`:125–175`). A failed credential read returns before dispatch/HTTP; it does not substitute a current replacement, unlock, fall back locally or retry.
- Normal routed success publishes chat/provenance/timing; only explicit `Action::Validate` writes a validation record (`:375–415`). Chat success does not invent model verification.

The new `service/tests/cloud.rs` cases exercise actual `Application` admission/worker execution with synthetic capabilities, not a prevalidated status-only fixture:

- `saved_keys_after_restart_allow_send_and_validation_without_probe` (`:451–517`) saves without validation, reopens the application, checks `not_observed` plus null validation and zero reads/HTTP, then performs Send or Validate for each vendor. Each actual attempt reads exactly one `CredentialRef` including provider, generation and save-request ID, and produces one HTTP call.
- `saved_routes_ignore_cached_observations_and_validation_without_changing_facts` (`:519–611`) covers all 48 observation/validation/provider combinations; catalog and both kinds of preparation leave disk, observations, validation, key-read count and HTTP count unchanged.
- `key_read_failures_stop_send_and_validation_before_http_then_repair_without_probe` (`:1128–1232`) covers both providers × six read failures × Send/Validate. Each failure is sanitized, retains the exact route, reports `not_sent` and makes **zero HTTP calls**. Clearing the injected failure and making a new explicit request succeeds with a second read of the same complete identity, one total HTTP call, no extra save and no intermediate probe.
- `failed_validation_keeps_routes_attemptable_and_auth_quota_fail_once_per_attempt` (`:1234–1289`) returns synthetic 401/402 through the real service error path: `CLOUD_AUTH`/`CLOUD_QUOTA`, `received`, one read/post per explicit Validate/Send, preserved key, no automatic retry/fallback, no chat-induced validation change. This is readable-key/provider-rejection simulation, not live invalid-key/account verification. Successful repair is separately exercised by the read-failure matrix and compiled 503 recovery below; this auth/quota case itself does not assert a later success after repairing billing/authentication.

The production Secret Service identity/encryption/NoAutoStart/no-Prompt/no-fallback and fixed HTTPS/no-proxy/no-redirect/no-retry implementations are byte-unchanged. Existing private-keyring and actual-transport negative tests remain unchanged and are included in the retained service suite. This is a focused boundary assessment, not a fresh exhaustive audit of all surrounding code or live keyring/provider acceptance.

### Consent, deadlines, uncertain reservations and replay

The retained source still binds preparations/admission to request, instance, route, generation, provider/configuration/history revisions and context hash. `route_prepare` retains 60-second admission and 600-second dispatch bounds; `provider_prepare` retains its 60-second effect-start bound. `submit_routed` rechecks token, preparation fingerprint, context and exact consent requirement before committing destination consent and immutable route (`app_v3.rs:1112–1186,1215–1269,1372–1549`). None of these paths changed outside `ready`.

Existing tests for first-transfer binding/replay/legacy rejection, same-chat destination consent, replacement requiring fresh consent, monotonic expiry, immutable queued routes, durable-marker failure/expiry, interrupted work and unknown remote delivery remain. In the restart unknown-delivery test, **only** the obsolete post-restart availability assertion changes; interrupted status, `REMOTE_OUTCOME_UNKNOWN`, call count and empty completed history remain (`service/tests/cloud.rs:1339–1384`).

Installer `Manager::perform` still refuses new effects while an earlier operation is pending, uses a correlated expiring confirmation, sets the reservation before the one apply, and retains uncertainty except for enumerated definitive pre-admission rejections (`installer/src/providers.rs:658–812`). GTK Validate was already gated by saved generation, exact model and no pending operation, not by observation (`installer/src/ui.rs:950–988`, unchanged).

Dart tests explicitly distinguish terminal adverse/failed probes from unknown outcomes: `cloud_retry_test.dart:177–213` keeps the reservation, observes the original ID, makes no automatic reissue, and blocks Send **even though `routeReady` is true**. Lost acknowledgments recover only by polling that identity. Completed adverse metadata does not block a genuinely new explicit attempt. The eligibility amendment does not grant permission to replay an uncertain request.

### TRY-R4 — advisory, not blocking, presentation

`next/client/lib/ui/cloud_selector.dart:45–75` shows saved/unobserved/adverse observations and null/failed/interrupted validation as distinct factual advisories; it keeps exact past-model verification and possible previous billing visible. The changed rendering splits advisory lines within the existing scrollable results/current-selection areas. It changes no selection/filter/consent authority. Rows still use current-origin plus `choice.available`, with rechecks after the dialog returns (`:123–160,325–421`); no implicit Retry, probe, validation or Send is added.

Installer status/guidance now distinguishes saved reference, last observed access, past exact-model check and attempt eligibility, without requiring preliminary observation (`providers.rs:114–143,1082 onward`). Unconfigured choices remain disabled; active-generation choices remain eligible despite an adverse cached fact. Existing operation reservations still take precedence.

Inspected retained real-font 400×400/2x advisory and disabled captures. Long advisories scroll and Cancel remains visible; this does not mean every status/model is simultaneously visible. Parser/controller/picker tests assert no-effect selection/filter/dismissal, enabled adverse-state choices, preserved draft/history, stale callback fences and reachable non-ellipsized statuses. These are synthetic Flutter/widget observations, not installed graphical, IME or assistive-technology acceptance.

### TRY-R5 — actual cold integration, not prevalidated substitutes

`next/service/tests/debug_service.rs:1217–1263,1310 onward` keeps the same `dart_cloud::dart_cloud_fixture` entry point. Its test-only `SMITH_DART_CLOUD_SAVED_ONLY=1` branch genuinely omits validation for both vendors. Default mode still saves/validates both. Private restart drops/reopens `Application` with the same fake key store; generations/validation persist, the instance changes and volatile observation clears. The added bootstrap self-test checks **pre-counter-reset** counts: saved-only has two keys and zero reads/posts; default has two validations. The separate retained interface check also exercises default and saved-only restart behavior.

The retained focused Flutter run executes **six compiled Rust↔Dart cases**, not six skips: five new saved-only cases plus the unchanged original routed local/cloud/consent/expiry/reattach/queue/unknown-ack journey. `rust_cloud_integration_test.dart:224–487` checks cold Send for both providers, cold Validate independently per provider, and a real service-worker synthetic 503 validation/send failure followed by a new explicit successful send without probe or auto-retry. Validation leaves history empty and verifies only its exact model; later chat success leaves the failed validation record unchanged.

The new cold-Send test explicitly drains the serialized synchronization workflow before the second route-changing gesture (`:296–304`). Its earlier `canSend` failure is retained in `focused-final.log`; the final test still asserts eligibility, exact consent/route, two submissions, one fresh read/post per send and absence of provider probe/prepare/apply. The extra synchronization is observation, not hidden validation or an assertion removal. This qualifies what that test establishes; it is not proof that every intermediate UI scheduling state must be Send-ready.

Native installer integration uses the real service library through `Domain`, saves both providers, actually reopens the application before cold validation, and verifies zero preliminary reads/posts. Its second roundtrip covers locked read failure with `not sent`/zero posts, external synthetic repair without probe, and one authentication rejection (`installer/tests/providers.rs:1718–1913`). No credential-management method was added to production Flutter; the Dart Validate calls are fixture-only protocol helpers.

## Existing-test preservation and baseline completeness

Independently regenerated the entire five-file backend patch and seven-file supplied client patch in memory; both match the retained patch bytes exactly. All twelve supplied prechange copies match the initial inventory. The two other changed tests were not represented by full prechange copies in the saved-key bundle, but their originals were recovered without writing files:

- `next/client/build/cloud-catalog-expansion/before/test/cloud_protocol_test.dart` exactly matches initial SHA-256 `30f00655b97b27277d84be56bbf45e862ed8667af17f6e2baad5a7910236165d`. Comparison shows additive advisory/malformed-data cases and optional executor fixture data; the original schema3/no-fallback, full-u64/forbidden-field, preparation/provenance negatives remain unchanged.
- The prior catalog `client-scoped.patch` contains the complete new visual test (`@@ -0,0 +1,121 @@`). Reconstructing its added lines in memory yields exactly initial SHA-256 `4bf1e5506a5f1f6e5403e8524ac15b7f98befe863398acf094e6f565c6cf9ad9`. Its current delta changes capture output location, replaces the superseded locked-means-disabled fixture with advisory/enabled assertions, then adds a genuinely unconfigured disabled state. Original unverified/status reachability, Cancel and no-route/draft/call-effect assertions remain.

Thus all **14** scoped files have an independently checkable baseline-relative comparison; no missing-original finding remains. No scoped test skip/ignore, timeout increase, relaxed malformed-data predicate, removed consent/no-replay assertion or meaningful non-policy test weakening was identified. Explicit old-policy cases appropriately change: saved-but-unvalidated/failed/locked/unobserved states become attempt-eligible, while truthful observations and exact-model flags remain. H1 identifies three additional old fixture assumptions still needing adaptation, rather than silently accepting their failure.

Against the full initial 1,289-input inventory, the only changed existing inputs are these 14 files plus concurrent `next/client/lib/main.dart`; new inputs are `next/client/lib/window_launcher.dart` and `next/client/test/new_window_test.dart`. They were preserved and included in the current 1,291-input gate snapshot, **not independently approved as part of this amendment**. There are no missing initial inputs or mode-only changes. In particular worker/keyring/HTTP/cloud adapter, service storage/domain, installer UI and client controllers/queue match the initial manifest. Root canonical documentation and this report are outside that component-input inventory.

## Required Checks Evidence

**B** = `next/service/build/saved-key-attempts/backend/`; **C** = `next/client/build/saved-key-attempts/`; **P** = `next/service/build/saved-key-attempts/`. These are retained developer/parent executions independently inspected, **not reviewer-executed tests**. Backend log hashes match records; passing backend before/after inventories are identical and match current service/installer inputs. Final client focused/analyzer inventories are stable and match current inputs. The earlier scoped formatter snapshot differs only in concurrent `new_window_test.dart`, not any of the nine formatted files; the full failed client gate subsequently reports formatting and analyzer success on its complete snapshot.

Commands below are from `next/` unless stated otherwise. Expected outcome is successful, stable, source-bound verification unless the row explicitly describes expected red.

| Check / command or scenario | Expected | Actual / exit | Evidence |
|---|---|---|---|
| Reviewer read-only baseline hashing, exact patch regeneration, original-test recovery and full source comparison | Exact bounded delta; no boundary expansion | Verified, exit 0; all 14 current hashes match supplied records | P `initial.json`, `baseline/`; B `backend.patch`, `summary.json`; C `source-delta.json`, `client-scoped.patch`; prior catalog copies/patch |
| `cargo fmt --manifest-path service/Cargo.toml --all --check` | Format clean | Retained exit **0** | B `service-format.{json,log}` |
| `cargo clippy --manifest-path service/Cargo.toml --all-targets --locked --offline -- -D warnings` | No warnings/errors | Retained exit **0** | B `service-clippy.{json,log}` |
| `cargo test --manifest-path service/Cargo.toml --locked --offline --test cloud` | Both-provider eligibility, cold execution and negative regressions pass | **34 passed / 0 failed/ignored**, exit **0** | B `service-cloud.{json,log}` |
| `cargo test --manifest-path service/Cargo.toml --all-targets --locked --offline` | Full host service suite passes | **263 passed / 0 failed / 3 controlled fixture ignores**, exit **0**, 06:50:40–06:51:32 UTC | B `service-tests.{json,log}` |
| `cargo fmt --manifest-path installer/Cargo.toml --package smith-installer --check` | Format clean | Retained exit **0** | B `installer-format.{json,log}` |
| `cargo clippy --manifest-path installer/Cargo.toml --all-targets --locked --offline -- -D warnings` | No warnings/errors | Retained exit **0** | B `installer-clippy.{json,log}` |
| `cargo test --manifest-path installer/Cargo.toml --locked --offline --test providers` | Parser, reservation, consent and actual-domain integration pass | **20 passed / 0 failed/ignored**, exit **0**, 06:50:00–06:50:03 UTC | B `installer-providers.{json,log}` |
| `python3 service/build/saved-key-attempts/backend/fixture_contract.py` | Default and saved-only private restart contract hold | Both modes **PASS**, exit **0** | B `fixture-interface.{json,log}`; fixture source and bootstrap test |
| `cargo test --manifest-path ../service/Cargo.toml --locked --offline --test debug_service --no-run --message-format=json`, from `next/client/` | Compile actual service fixture | Retained exit **0**; binary independently rehashed | C `fixture-compile.{json,log}` |
| `flutter test --no-pub --reporter expanded` on `test/cloud_catalog_expansion_test.dart`, `cloud_protocol_test.dart`, `cloud_routing_test.dart`, `cloud_retry_test.dart`, `cloud_picker_test.dart`, `cloud_selector_widget_test.dart`, `cloud_picker_visual_test.dart`, `rust_cloud_integration_test.dart`, from `next/client/` | All focused cases and six compiled journeys run, not skip | **164 passed / 0 failed/skipped**, exit **0**, 06:58:21–06:58:28 UTC | C `focused-verified.{json,log}` and before/after manifests; exact argv in record |
| `flutter analyze --no-pub`, from `next/client/` | Analyzer clean | No issues, exit **0**, 07:00:05–07:00:09 UTC | C `analyze-current.{json,log}` |
| `dart format --output=none --set-exit-if-changed` on the nine owned Dart files, from `next/client/` | No formatting changes | Retained exit **0** | C `format-settled.{json,log}`; exact nine-path argv in record |
| `git --no-pager diff --check -- service installer`; scoped client whitespace record | No whitespace diagnostics | Retained exit **0** for backend check; client scoped record reports clean | B `whitespace.{json,log}`; C `whitespace.json`. Tracked Git whitespace alone does not cover untracked files; formatters are separate evidence |
| Parent `env DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make client/test` | Complete configured client suite passes | **FAIL, exit 2**, **663 passed / 3 failed**, 07:05:48–07:07:08 UTC; formatter/analyzer passed | P `client-test-status.json`, `client-test.log`, matching 1,291-input before/after inventories; **H1** |
| Final configured `make service/test installer/test client/test`, combined `make test build`, `make package/test` and `python3 dagger/package.py --root installer/dist --verify`, as applicable | Complete stable final evidence and rebuilt paired package | **Not verified for the final saved-key deliverable**; full client currently failed; no aggregate/build/package PASS inferred | Parent-owned final records/ledger follow-up; **G1** |

The service ignores are the existing isolated proxy-environment child, captured-helper child and explicit Dart cloud fixture entrypoints. They are not three skipped product regressions; their parent/subprocess or Dart invocation is separate. The focused Dart evidence explicitly supplies and runs the compiled cloud fixture. This does not turn focused checks into complete aggregate or installed-host verification.

**Earlier failed executions retained:** the service and installer regression-before runs both exit 101 for the intended old eligibility/parser rejection. Their failing diagnostics identify `NOT_OBSERVED` and `Catalog validation/readiness mismatch`, respectively. They are expected-red demonstrations, not current green checks. Client `format` exits 65; `focused-initial` records 73 passed/85 failed; `focused-corrected` 155 passed/3 failed; `focused-final` 163 passed/1 failed. The final focused pass follows typed/decoded fixture corrections, advisory layout correction and the explicit synchronization qualification above. None erases the new full-client failure, earlier runner failures or the original failed logs. Hash consistency is retained evidence binding, not signed provenance or independent authentication of chronology.

## Source and evidence identities

SHA-256 independently recomputed at review. These bind the uncommitted source/evidence target, **not a released or deployed artifact**.

- Initial manifest file `P initial.json`: `4d4e11c526b1e403cbe8b8c18e2479d3c62069f227198c780ae9cf4dfc03cf01`.
- Initial **1,289-entry inventory**: `205bfa82434e1a4ecbd23f2f9c5455c7d61274c1b96c888b9e3a948453724b03`.
- Current/full-client **1,291-entry inventory**: `7ff716129095837394ed5a102eca56d126fbce6c94ee3f68b604fdba4d5955e0`. Inventory encoding is compact sorted-key JSON of path-sorted entries including SHA-256, size, kind and mode, with P `validate.py`'s traversal/exclusions; that script was read, not executed by the reviewer.
- **14-file scoped current mapping**: `72d75b254b0ec6f55e27150463729444f021d008d5aa1848eda7b77568e0f4a5`, SHA-256 of `json.dumps(rows, sort_keys=True, separators=(',', ':')).encode()`, where rows are the path-sorted `{"path": path, "sha256": digest}` entries below.

| Scoped current input | SHA-256 |
|---|---|
| `next/service/src/app_v3.rs` | `a8c5dbae65d9c7a76c39b5a4d0f3f80a43dcda1af3d82b59f36b938e410d9b82` |
| `next/service/tests/cloud.rs` | `643456ea3446f3bcb65c9e315fd93fa66b05a4e4094802d7c357de178168158b` |
| `next/service/tests/debug_service.rs` | `e238f9ae5ede99dd1dca006a9eb5d6d9be8ee2c7d7cca6f8d768cc4db3365ffd` |
| `next/installer/src/providers.rs` | `cf3a0c339c9a1247df37c4b6fdb903ab4dcc2948b573ccf2c6c9c695fed14e36` |
| `next/installer/tests/providers.rs` | `d05c0d06650a103f640cc388fd22a30467f3b64a820dccdd5a8ee718254d66de` |
| `next/client/lib/cloud_routing.dart` | `cffcd7874a78cf7b1af8e97fc12178609d75c090ef6537b8e7c2ddfe335b6b6d` |
| `next/client/lib/ui/cloud_selector.dart` | `33976a675b4401624d24a7425647db11b3d80ace5c0aac38881a26db3c39af3e` |
| `next/client/test/cloud_catalog_expansion_test.dart` | `24ee05090a23d717a5fe41a8c0829c6ad4bc33dd5922b0bf8243bc7711d574bb` |
| `next/client/test/cloud_retry_test.dart` | `a70cf3b72a7956a6764aa4b89364e403c4af69b3de748b7cdec6d4b9698dfdfc` |
| `next/client/test/cloud_routing_test.dart` | `a64b5eecf77b016fad0d4f524f8a42c832c132dac5c1e7053165cb0e8b3bcee9` |
| `next/client/test/cloud_picker_test.dart` | `16acb6ba1ceb091d894da5eb24f1d5bfe38a42c0bee3c97c271b58f03ac7bf85` |
| `next/client/test/cloud_protocol_test.dart` | `454972fa7c86e07489ea0ffffbc1df5fac77f1c4c4f715992cbd381de9956cd5` |
| `next/client/test/cloud_picker_visual_test.dart` | `057cef540c8d35f403fcbf25bd509a649521f2af23b8986a41755fc72996ebc4` |
| `next/client/test/rust_cloud_integration_test.dart` | `3d3565f99aa01c7f6b536adf2dcef38bac47a231b97bd4f2ac4ff0f457d13deb` |

Additional anchors:

| Evidence/boundary | SHA-256 |
|---|---|
| Plan `LOCAL-SMITH-NEXT-SAVED-KEY-ATTEMPTS.md` | `a37da082076949074fe86075007ff0b05d7b59fce249e21cf344cb6dc916ffb1` |
| B `backend.patch` | `7b33bc52726e73ced8dc151bc11d4a8cda8900514ad513f94168aee8f78e1b9d` |
| C `client-scoped.patch` (seven-file patch; recovered originals supplement it) | `424694c340c7eb7be1c112605b8264cda4dd628f89e4d06dea21ad8cad1c5cbe` |
| Prior catalog `next/client/build/cloud-catalog-expansion/client-scoped.patch` | `dd2b81f8dadfb5ed8660937fa8f8fd7cc8cafe391d9b737c464280be710db7dc` |
| B `summary.json` | `90283597cc573520e75f70495ad589d2dab0b3bcfc2985e8db0022f6a864e572` |
| C `summary.json` | `5a650e4ea0ae52b984bbbba4a959fc48daac751b4e572fb556395c838d6da91f` |
| B `service-tests.log` | `3e29aa4244163ef4de3064e3542053de9db01a37d474fc2dfd85db2784b9d02b` |
| B `installer-providers.log` | `a9a689b8bd8e5ef7db80de0c3be0cad59f388283494f35b81217684c7cbfa4fe` |
| C `focused-verified.log` | `c299723f3adb61ff00af2bc0c0289df312ba58634014b13282f06f5fe9f502d7` |
| Compiled `next/service/target/debug/deps/debug_service-0a4be2ddda80d80f` | `128742c2dd134fe0b8b943374532d2f7c1e74efbf367b308154f77e3dbc4286b` |
| P `client-test-status.json` | `5abc56965f37d2cb6f5eecb1f0f57f70aaf71703cb9759d9aa176127b78e2d30` |
| P `client-test.log` — failed required gate | `6ca1b1b8317c340dd539888a3e415f8b91d20d0cc9be2e9cbcaab47a32d0b702` |
| Unchanged `next/client/test/workspace_entrance_test.dart` — H1 fixture | `650a580e0d22b3ecacab051abf8ab773820e1c5c925491fd16c5b0fff83bdf15` |
| Unchanged `next/service/src/app_v3_worker.rs` | `c09100f0c4729834dfc640367711de6e71d315966290de365cae438c4d6af0b6` |
| Unchanged `next/service/src/cloud_keyring.rs` | `f6ac5c6ea536992ad7bf252d92ef06b4ab8085b6393dc2e1592c53f5628c20aa` |
| Unchanged `next/service/src/cloud_http.rs` | `bd672c1ccccd3181ae4dc29f00abd3238b83f861f8c5d027f34a543a83ae0fd0` |

## Evidence Gaps and pending final gates

1. **H1 test-only remediation CLOSED:** the follow-up below independently verifies the exact fixture correction, assertion preservation and passing targeted entrance/visual/cloud checks. No source/test remediation was made by this reviewer. The initial 663-pass/3-failure full-client execution remains retained; final full-client verification belongs to G1 and is not replaced by targeted passes.
2. **G1 remains open:** configured service/installer/GTK/runtime, full client, combined test/build, package tests/export verification and exact paired source/artifact/manifest equality require final binding. Current exports are not claimed to contain this amendment. Any additional source changes, including concurrent window work or test remediation, must be separately inventoried/reviewed; unchanged HEAD alone is insufficient.
3. Historical aggregate runner failures remain unresolved/unaccepted and retained under the prior schema/catalog evidence and ledger. A later genuine PASS establishes that execution's outcome, not causal repair or harmlessness of previous failures. No risk waiver is inferred from the eligibility approval.
4. The evidence establishes synthetic/private-domain behavior only. Normal unlocked GNOME session access, real keys/account/model entitlement, auth/quota repair, requests/charges, native graphical/IME/accessibility acceptance, installed migration/restore and paired upgrade remain separately scoped, unverified and unauthorized here. The existing storage compatibility and exact keyring-generation restore boundary remains; binary-only downgrade is not a rollback guarantee.
5. Clean-clone provenance, redistribution/license/SBOM and release/deployment approval are not supplied by this dirty-worktree source review. No external effect or accepted-risk decision is granted.

## Verdict and required remediation

**Source-limited conclusion:** the inspected four-file production delta implements the operator-approved saved-generation eligibility contract without an identified weakening of actual credential, identity, consent, deadline, reservation, uncertainty or replay boundaries. TRY-R1–R4 have substantial source/synthetic evidence, including actual cold worker and cross-language execution. All 14 scoped baseline-relative changes were reviewed, with no meaningful non-policy test weakening identified. **The deliverable is not yet acceptance-ready:** the initial H1 full-client execution remains a retained failed execution, its test-only remediation is closed below, and TRY-R5/G1 final verification is incomplete.

- **Formal assurance verdict: FAIL.** The initial full-client failure remains retained and final full-client/combined/artifact verification is missing; targeted H1 closure is not a complete required-gate PASS. `pass` or `pass-with-risk` would violate the assurance role.
- **Release Gate Decision: NO-GO.** This is not human approval, product-wide target-policy compliance, installed repair, risk acceptance or permission to deploy.
- **Required remediation/follow-through:** H1's fixture correction and targeted evidence are independently closed by the follow-up below; bind and independently review the final source/component/full-client/build/package outcomes for G1. Retain all historical/global runner failures and separate their disposition. Do not change production eligibility back, weaken safety checks, or repair unrelated runner/lifecycle/model-discovery work under this review.

## H1 test-only remediation follow-up — CLOSED, G1 unchanged

### Scope and independent checks

Reviewed the supplied one-test-file remediation without new tests/builds, artifact execution or host/runtime calls. Only this review was updated. **R** = `next/client/build/saved-key-attempts/remediation/`; **P** retains its definition above. The parent's new `final-validation/` run was neither inspected nor awaited; no outcome is presumed. HEAD/branch remain `7f6a483ae82c4bcf647268f035f1e7cc80409d4b` / `feature/autonomy`.

Read the actual baseline/current test, complete R `workspace-entrance.patch`, command records, source inventories and raw red/passing logs, plus the unchanged base catalog producer. Independently regenerated the diff in memory: it exactly matches the retained patch, **one file, +153/-9 lines**, with two additional parameterized cases. The baseline copy `P baseline/workspace_entrance_test.dart` hashes to the original reviewed/failed-gate value `650a580e0d22b3ecacab051abf8ab773820e1c5c925491fd16c5b0fff83bdf15`; current test hashes to `b1f99807a7470a8083716c4d33bbcade162cd483d3e044b8ce0ab59eb529ff5b`.

All ten before/after inventories for the five passing targeted/quality commands agree on **210 inputs**, and every recorded hash matches the current files. The expected-red inventory is exactly the original reviewed 210-input snapshot. Red to remediated source changes **only `next/client/test/workspace_entrance_test.dart`**. Independently comparing the prior full-client snapshot's 1,291 existing inputs to current bytes/modes likewise finds only this test changed; all **31 inventoried golden files** remain unchanged. The previously reviewed 14-file source mapping and production boundary hashes therefore remain unchanged. No production, controller, Boot, header, queue, parser or golden remediation is present.

### Fixture contract and existing-test preservation

**H1 CLOSED at the source/targeted-evidence level.** `_AvailabilityCloudFake` now models a credential-generation replacement (`next/client/test/workspace_entrance_test.dart:228–277`), not a configured route becoming ineligible because of cached observations:

- `replaceCredential` retains the provider/model and chooses a distinct opaque generation. Catalog projection increments that provider's revision, updates **every choice for that provider** to the new generation, leaves them `available=true` with no disabling reason, and clears old-generation validation/verification.
- The base fake creates a fresh catalog for each query (`cloud_routing_test.dart:135–137`), so the projection's `revision += 1` does not accumulate on repeated reads. The new OpenAI and Anthropic cases explicitly require a single revision increment, null validation, exact generation across all choices, old identity unavailable, replacement eligible/unverified, unchanged other-provider/local facts, identical repeated replies, only three catalog reads and zero accepted submissions/jobs (`workspace_entrance_test.dart:1118–1188`).
- The helper's local-unavailability branch retains the previous false/error behavior, now explicitly restricted to local routes. Local recovery still removes that fixture condition, synchronizes and restores readiness; its entrance and no-effect assertions remain.
- The selected-cloud transition still becomes not-ready mid-entrance and settles immediately, without changing the selected historical route. Added assertions retain history, last accepted route and route revision. A second synchronization must not silently retarget to the replacement. Only explicit `selectRoute(replacement)` restores readiness, increments the route revision once and makes no API call; last accepted routing/history remain untouched (`:1287–1378`). This is deliberate test-side future-route selection, not automatic credential substitution or consent to send.
- The two healthy-other-route cases retain an unavailable **historical Anthropic identity** alongside an eligible/unverified active replacement, while the selected local/OpenAI route keeps its original entrance behavior (`:1381–1437`). The original timeline/gate, focus, same-widget/editor identity, no-remount, no-extra-call and `_expectCloudReadOnly` assertions remain. Historical first-frame recovery and hidden-route cases are unchanged.

No original test was deleted/skipped; no timeout, golden baseline, malformed-data rule, no-replay or no-fallback assertion was weakened. The two new parameter cases strengthen the synthetic replacement contract. Existing adverse-observation eligibility and uncertainty-reservation regressions are unchanged and are rerun by the retained cloud suite. No new critical/high/medium/low source finding was identified in this remediation.

### Required Checks Evidence — H1 remediation only

These are retained implementer executions independently inspected, not new reviewer test runs. All commands are from `next/client/`, with UTC times on 2026-10-16. Log hashes, exit statuses, source digests and before/after equality were independently checked rather than accepted from the summary alone.

| Check / command or scenario | Expected | Actual / exit | Evidence under R |
|---|---|---|---|
| Pre-edit `flutter test --no-pub --reporter expanded test/workspace_entrance_test.dart --plain-name 'selected-route entrance recovery'` | Reproduce the same three obsolete-fixture failures on original source | **4 passed / 3 failed**, exit **1**, 07:19:34–07:19:39; same three failure identities as the original parent gate | `selected-route-before.{json,log}`, matching original-source manifests |
| Same selected-route command after remediation | Original seven cases plus two replacement-contract cases pass | **9 passed / 0 failed/skipped**, exit **0**, 07:21:24–07:21:28 | `selected-route-after.{json,log}` and inventories |
| `flutter test --no-pub --reporter expanded test/workspace_entrance_test.dart test/workspace_entrance_visual_test.dart` | Full entrance regressions and unchanged visual baselines pass | **31 passed / 0 failed/skipped**: 28 entrance plus 3 visual cases; exit **0**, 07:22:00–07:22:08 | `workspace-entrance.{json,log}` and inventories; golden hashes unchanged |
| `flutter test --no-pub --reporter expanded` on the same eight cloud files listed in the initial Required Checks Evidence | Eligibility, strict DTOs, consent, uncertain reservations and compiled cloud journeys remain intact | **164 passed / 0 failed/skipped**, including the six compiled fixture cases; exit **0**, 07:22:00–07:22:07 | `cloud-focused.{json,log}` with exact eight-path argv, inventories and unchanged fixture binding |
| `flutter analyze --no-pub` | Analyzer clean | No issues, exit **0**, 07:22:01–07:22:04 | `analyze.{json,log}` and inventories |
| `dart format --output=none --set-exit-if-changed test/workspace_entrance_test.dart` | No formatting changes | One file / zero changes, exit **0**, 07:22:04–07:22:05 | `format-check.{json,log}` and inventories |
| Reviewer read-only patch/hash/source and preserved-evidence comparisons | Exact bounded delta, no production/golden drift, original failure retained | Verified, exit **0** | Actual baseline/current test, patch, all above inventories, prior reviewed inventories and original P failure log/status |

The red run binds the original 210-input digest `a13fea9b4490dafc7bd587d3444ff5ad76c1ebfc8f742fa10e6c4b0c12ba88d1`. All passing remediation checks bind `589f1602a9c95025ef1df5aeb6406d972cc489c1c595e98b85307930dafc7e47`, using SHA-256 of `json.dumps({project_relative_path: sha256}, sort_keys=True).encode()` with default separators. These inventories include the client/service source/test inputs described by R `check.py`; they are not complete package provenance. That script was inspected, not executed by the reviewer.

### Superseding test/evidence identities

Independently recomputed SHA-256. Only the H1 test identity is superseded; the original production and failed-execution identities above remain historical anchors.

| Input/evidence | SHA-256 |
|---|---|
| Before-copy `P baseline/workspace_entrance_test.dart` | `650a580e0d22b3ecacab051abf8ab773820e1c5c925491fd16c5b0fff83bdf15` |
| Current `next/client/test/workspace_entrance_test.dart` | `b1f99807a7470a8083716c4d33bbcade162cd483d3e044b8ce0ab59eb529ff5b` |
| R `workspace-entrance.patch` | `9c906cecb1a5d768cad10645febb4219bc341b61abb2bfa8b24956bc3f09b58f` |
| R `summary.json` | `15c9990a914c4b4f991cea71c3c4aef13e3bafc8d11a55b49b767da73836d38e` |
| R `workspace-entrance-source-after.json` — manifest file, distinct from its canonical inventory digest | `2e0f9b6bd8a5dbbd7e504b32b194f58c5b27dc917dd29a3514a9b89a03e6da2d` |
| R `selected-route-before.log` | `646af31fae488656b5e404658852b43069001e77cf1d83cfffd31ba6bd4c8b51` |
| R `selected-route-after.log` | `3ee6b5c1bc3969882f21c587ab29309f6c9854c017aa5bf964958d57c24899df` |
| R `workspace-entrance.log` | `c622ea045ec413b8bf6e9532df9734811c8da3d9d57d1789bf9bf6a0a4baf385` |
| R `cloud-focused.log` | `8846035bb9fb480e10670adc36acf1db13753d88c9c7d080ff9ba5dc2c1729fc` |
| R `analyze.log` | `3efe87c2882fe46588a738f133e71f662adcc2c47b3bd940d4495c49757acb86` |
| R `format-check.log` | `d0bf0ba77c96cfb6c29907d8a9213fd763d633606445180a0907c09d84ffcf1d` |

The compiled cloud fixture still hashes to `128742c2dd134fe0b8b943374532d2f7c1e74efbf367b308154f77e3dbc4286b`. Original P `client-test.log` and `client-test-status.json` independently remain exactly `6ca1b1b8317c340dd539888a3e415f8b91d20d0cc9be2e9cbcaab47a32d0b702` and `5abc56965f37d2cb6f5eecb1f0f57f70aaf71703cb9759d9aa176127b78e2d30`. Their **663 passed / 3 failed, exit 2** outcome is not rewritten or counted as green. The pre-follow-up review file itself was `aec423a6c7ec5663f47300f17c0f65a9f9858ce8a9a5fefc2190e6c0faa5d2f6`.

### Evidence Gaps and final disposition

- **H1 source/targeted remediation is CLOSED.** No further H1 fixture or production change is requested on the inspected evidence. This is a correction of an invalid test catalog, not permission to reintroduce an observation gate or weaken generation/consent boundaries.
- **G1 remains OPEN and unchanged.** Final full-client/component/combined/build/package outcomes and paired artifacts still require independent binding. The new parent `final-validation/` execution was not inspected or awaited here. Targeted passing tests do not establish its outcome or supply a full-client/aggregate PASS.
- The initial failed gate, expected-red reproduction and historical/global runner failures remain retained, unresolved as broader-gate evidence and unaccepted as risk. No host, live-provider, installed graphical, migration, release or deployment acceptance is supplied.

**Source-limited follow-up verdict:** H1's test-only remediation is independently verified, with existing assertions preserved/strengthened and no identified production delta or meaningful test weakening.

- **Formal assurance verdict: FAIL — G1 final required verification remains incomplete; the original failed executions are retained.**
- **Release Gate Decision: NO-GO.**
- **Required follow-through:** independently bind and review the parent-owned final source/full-client/component/build/package results and any new source delta, keeping historical failures and their separate dispositions intact. This H1 closure grants no risk acceptance, human approval or external-action authority.

## Final configured aggregate/package closure — G1 CLOSED

### Closure scope and method

**The final saved-key source/test/build/package gate is independently verified PASS. H1 remains CLOSED and G1 is now CLOSED for this bounded local deliverable.** This append-only closure supersedes the earlier pending-G1/evidence-limited FAIL disposition for the current saved-key deliverable, not the initial failed execution, earlier review history, unrelated runner failures or lack of human release/deployment approval.

- **F** = `next/service/build/saved-key-attempts/final-validation/`; **D** = `docs/evidence/saved-key-attempts/`. Inspected raw statuses/logs, bookkeeping definitions and retained correction history, before/after/source inventories, durable copies/index, actual exports, package manifest/checksums, and the current plan/ledger/as-built description.
- Reviewer work was read-only local source/evidence inspection, independent inventory reconstruction and hashing, raw-suite reconciliation and package-content comparisons. No new tests/builds, package-verifier invocation, artifact execution, host/container/manager/keyring calls, real credentials, network/vendor calls, installation, cleanup, Git mutation or publication. **Only this review was appended.**
- The implementation target remains the previously reviewed 14-file delta plus the separately reviewed H1 test correction, with unchanged HEAD/base `7f6a483ae82c4bcf647268f035f1e7cc80409d4b` on dirty `feature/autonomy`. Concurrent New Window work is preserved and included in the tested snapshot, not independently approved by this saved-key review.
- The reviewed final source inventory is **1,291 inputs**, SHA-256 **`ce939c47c54b6afdb41c8b79002cd989a63e7b38ebc81933e928fea7d2cb25c3`**. Artifact comparison cutoff was **2026-10-16 07:50:45 UTC**. Root canonical documents, this review, the ledger and durable evidence additions are outside the component-input inventory; their later documentation updates are not represented as compiled inputs.

### Final findings, severity ranked

- **Critical / high / medium / low scoped findings: none open.** No current source, raw/durable-copy, log/status, source-inventory, export, manifest or checksum binding mismatch was found.
- **H1 remains CLOSED.** The corrected generation-replacement test still matches `b1f99807a7470a8083716c4d33bbcade162cd483d3e044b8ce0ab59eb529ff5b`. Its preserved assertions and two added cases were already independently reviewed; the final full client suite now passes as well.
- **G1 CLOSED.** The complete configured aggregate, build and explicit recipient verification have actual passing source/artifact-bound evidence. These are not substituted focused/component results or an inference from a running command.
- **Historical failures are not waived.** The initial full-client **663 passed / 3 failed, exit 2** execution and expected-red checks remain failed historical executions. Earlier unrelated runner lock/timing failures remain documented and unaccepted; current runner production/tests match the pre-amendment source. This successful aggregate establishes this execution's result, not a causal runner repair, proof of harmless flakiness or accepted risk.

### Required Checks Evidence — final actual results

The two top-level commands below are retained validator executions, **not reviewer-executed gates**. Both ran once from `next/`, exited **0**, were not killed/timed out, and had identical 1,291-input before/after inventories. Required configured component checks completed inside this aggregate; no standalone component fallback, separate build retry or standalone package-test rerun is claimed.

| Check / command or scenario | Expected | Independently verified actual | Evidence |
|---|---|---|---|
| `env DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make test build` | Full configured test graph and paired builds complete on stable final source | **PASS, exit 0**, 07:27:03–07:33:03 UTC; **263 service / 281 native installer / 9 isolated GTK / 668 Flutter / 41 package tests passed**, zero failed; three service and two installer controlled fixture ignores; zero Flutter/package skips | F `combined-status.json`, `combined.log`; F/D `component-results.json`, `final.json`; matching source snapshots |
| `python3 dagger/package.py --root installer/dist --verify` | Verify the newly exported recipient package | **PASS, exit 0**, 07:33:31 UTC; **95 manifest-listed files**, **97 recipient files total** | F `package-verify-status.json`, hash-matched `package-verify.log`; actual `next/installer/dist/` |
| Configured formatter, analyzer, warning-denied Clippy and native runtime branches | All configured quality/runtime branches complete successfully | Client **81 files / zero formatting changes**, analyzer **no issues**; Rust quality scripts and isolated GTK/runtime branches completed in successful aggregate. Native runtime is the configured Ubuntu actual-ELF/no-Python/private-root gate, not normal-host installation | Raw aggregate, unchanged component scripts, `Makefile`, Dagger definitions and component records |
| Primary suite/count reconciliation | No failed/omitted component hidden by duplicate trace output | Independently summed **8 service + 12 native-installer + 1 GTK Rust suite summaries**; raw Flutter `+668: All tests passed!`; raw package `Ran 41 tests` followed by `OK`; **1,262 numbered passes**, five controlled fixture ignores | `combined.log:5632–11243`, component trace nodes 28/63/61/114/128; F/D `component-results.json` |
| Source/current-target and raw/durable evidence binding | Current files and all command/source copies agree | Exact source entries/digests/SHA lists and all final document copies verified; original 14-file mapping and H1 test unchanged | Independent reconstruction; F snapshots; D exact copies; prior scoped manifests |
| Export/manifest/checksum binding, read-only | Actual current package/export contents agree with retained records | **189 exports**, four identical service copies, **91 identical client pairs**, 95 manifest members and exact 96-record checksum file verified | Actual exports, F/D `artifacts.json`, recipient `manifest.json` / `SHA256SUMS` |

**Package41 attribution:** those tests fully completed **inside this single aggregate** on its final source snapshot. They were not rerun standalone and are not added again from earlier focused/package evidence. The explicit package verification is a separate check of the actual newly exported recipient tree. The raw aggregate records its full-test completion at line 11243, then proceeds through build/export; native package verification appears during the configured build and again after export. The separate retained verification log is the additional explicitly requested command, not evidence of another aggregate or package-test run.

The five Rust ignores are the unchanged isolated proxy-environment child, captured-helper child, Dart cloud fixture, installer launch-parent child and progress-output probe child entrypoints. Their parent/subprocess or compiled integration paths are separate. The full Dart suite has **no reported skips**, with both `SMITH_TEST_SERVICE=/opt/smith-service` and `SMITH_TEST_CLOUD_SERVICE=/opt/smith-cloud-fixture` configured. This includes the existing socket gates and six compiled cloud cases; the concurrent reporter does not name every case individually. No live keyring/vendor acceptance is inferred.

### Final source, evidence and history binding

1. **Current source reconstructed independently:** traversed the actual component/Dagger roots using the recorded exclusions, symlink treatment, byte counts and modes. All **1,291 entries exactly match** F/D `source-manifest.json`; its compact sorted-entry JSON digest is `ce939c47c54b6afdb41c8b79002cd989a63e7b38ebc81933e928fea7d2cb25c3`. F `initial`, `combined-before`, `combined-after`, `package-verify-before`, `package-verify-after` and `final-source` each contain those same entries, and every corresponding SHA-list file agrees.
2. **No unreviewed saved-key production drift:** all original 14 scoped file hashes still match the reviewed records/mapping digest `72d75b254b0ec6f55e27150463729444f021d008d5aa1848eda7b77568e0f4a5`. Including H1's test gives the **15-file scoped mapping digest `cb4af120d0cbeffa5fc3509ee02f3800bb5fd7e3a5b3952859d8e6127cb83350`**, using the path-sorted `{"path": path, "sha256": digest}` compact sorted-key JSON encoding defined earlier. The only input differing from the first failed full-client snapshot is `next/client/test/workspace_entrance_test.dart`. Worker/keyring/HTTP, runner production/tests, controller and queue identities remain equal to the initial manifest. No production safety fix or unrelated runner remediation was smuggled into the final gate.
3. **Raw/durable copies:** D `final.json`, `component-results.json`, `source-manifest.json` and `artifacts.json` are byte-identical to F originals. D `initial-client-failure.json` exactly equals the retained original failed report at `next/service/build/saved-key-attempts/final.json`. Independently verified each final command's raw-log digest and all referenced source/component/artifact/preservation record hashes.
4. **Prior evidence preserved:** independently rehashed all **181** entries in F `prior-evidence-manifest.json`; none is changed/missing. The original failed client log/status retain SHA-256 `6ca1b1b8317c340dd539888a3e415f8b91d20d0cc9be2e9cbcaab47a32d0b702` and `5abc56965f37d2cb6f5eecb1f0f57f70aaf71703cb9759d9aa176127b78e2d30`. The failed outcome is neither overwritten nor counted as green.
5. **Collector correction is bounded and retained:** the first evidence collector's `str.strip()` dropped the first Git porcelain status-column space. Independently confirmed the original after-capture equals the exact capture with only its first space removed, and exact before/after captures match. The original capture/report are retained. Comparing corrected and pre-correction final JSON changes only `git_status_after`, `git_status_unchanged` and the two correction-note/reference fields—not commands, outcomes, source or artifacts. This is an evidence-capture correction, not a Git/source mutation or removal of a failed gate.
6. **Documentation disposition:** the current plan Outcome, `next/VALIDATION.md` saved-key section, observed architecture report and D index distinguish the current configured PASS, H1's test-only repair, the initial failed client gate, historical runner failures, preserved concurrent New Window work and absence of installed/live-provider acceptance. Their final-review pointer is fulfilled by this append. No other document was edited by the reviewer.

Source endpoint equality and owner-writable hashes provide local drift/evidence binding, not continuous monitoring, signed provenance, hermetic builds or a clean-clone reproducibility guarantee.

### Actual exported package binding

Independently enumerated the three export roots: **one standalone service file + 91 standalone client files + 97 recipient files = 189**. Exact path sets, hashes, sizes, kinds and modes match the retained artifact list; artifact inventory digest is **`60e1f232d7519a411bd0754279e2a29d65b96050a5d3b72e5f319e1b801e5da6`**.

- All four service copies agree on bytes/kind/mode and SHA-256 **`5cfe584b8e597771319b3d4a3da9572bd30df9059d89f4f68e6e38fd5b2c4dcb`**: `next/service/dist/smith-service`, `next/client/dist/smith_backend/smith-service`, `next/installer/dist/payload/service/smith-service`, and `next/installer/dist/payload/client/smith_backend/smith-service`.
- All **91** standalone client relative files match packaged client peers in bytes, size, kind and mode. The complete rebuilt recipient candidate is **`next/installer/dist/`**; no stale developer helper or installed binary is substituted for it.
- The recipient manifest's **95-member file/mode map** exactly matches the actual recipient files excluding `manifest.json` and `SHA256SUMS`. Manifest SHA-256 is **`f2c46e2c42a595e3da9a48518dce428653a3dad258c88a6b1e12a72323c73e52`**. The checksum text exactly equals the 95 sorted file records followed by the manifest record: **96 records**, checksum-file SHA-256 **`4f2204ffd98a10d27f2cc273bf149427d89c0a1aa6717e74a3111e923a260ec0`**.
- Reconstructed the packaged-client release-manifest digest including directory modes and the required trailing newline: **`611ad7668c9443f27c5dd9168f71e4f34cf91b5a51ae71b214c11dbaceade043`**, matching `client_manifest_sha256`. No unsafe symlink/mode was found in that tree. Recipient metadata retains native/GTK, Ubuntu24.04 x86_64, CPU+Vulkan runner and no-bundled-model/voice declarations.
- Independently read executable headers/modes: standalone service, desktop client and both installer entrypoints are executable Linux x86_64 ELF, not interpreter wrappers. This is file/header inspection, not executing them or claiming installed native runtime/graphical acceptance.

### Final evidence anchors

SHA-256 independently recomputed. D's four final JSON copies below exactly match F; file hashes are distinct from canonical inventory digests.

| Evidence/artifact | SHA-256 |
|---|---|
| F/D `final.json` | `0451c35bd5bd54d69ac92e9d7109ec13f725c7c6bb9a3c9c52f035e312a1e5e0` |
| F/D `source-manifest.json` | `e21eafc2dd93e3fb52ef15df94b4285f777908a0a0c94ff312220021c22fb963` |
| F/D `component-results.json` | `fa468516649af12365bbb29f313182f4ae918152e0ea88805b185f79b894a194` |
| F/D `artifacts.json` | `3977bc642c2ab20d0297eaff70900ab99194bee070d7af3a77bd4b5d00c83c58` |
| F `combined-status.json` | `82f5667e5ac9e35312b649b25925400feaab3f59be0b8f32a485279ae6706a2f` |
| F `combined.log` | `83994c2f5d29d0ad6f6c3e39b5cc768ea9fdba318be4cd325e4cd37d2db1d874` |
| F `package-verify-status.json` | `d49cca6b19757cb59ef2fc590133b529476e526af640b9af14fb5785803e24d8` |
| F `package-verify.log` | `4f6e997052bf8ed0fe4e97c0e068bea00d34f1891dc7ee807e21581cdbb6da2e` |
| F `preserved-evidence.json` | `27dd2b9cb1a48866994cc7b69c7eb2139d416297a5fa691d260f54e0a8c5c6aa` |
| D `README.md` | `f94efafa3fc0d2371aec3bfd3b17f3f22c883b7c92a5e9989df7397d28ff3a3e` |
| Exported desktop `next/client/dist/smith_client` | `b12601aa3bf99ee6bbc66c857282b0d8c111d78929b69f83ea9db1b45dba551c` |
| Recipient `smith-installer` | `4162d4924b227cdf7a4f2f9845ae9ac1338e87694bd36a8317e5935f7854ea4c` |
| Recipient `smith-installer-ui` | `4163bf4d8519900de4630b70765a18d19a81ca5b26d4f1ae158990fe95333f39` |

The pre-closure review file hashes to `54f973d938e0889b241173256bb071390a89b3d00867dd3d83d6d379bac513aa`. Its earlier source findings, FAIL dispositions, remediation history and initial failed-execution anchors remain intact above this appended closure.

### Evidence Gaps and final scoped verdict

**No required final source/configured-test/build/package binding gap remains for this approved saved-key local deliverable.** TRY-R1–R4's reviewed implementation/synthetic boundary evidence is unchanged, and TRY-R5/G1 now has verified aggregate and paired-artifact evidence. This is not an exhaustive whole-product assurance review or independent semantic approval of concurrent New Window/other accumulated work.

Still outside this approval/evidence scope: real keys and normal-session GNOME access, vendor authentication/account/model entitlement/charges, live auth/quota repair, installed graphical/IME/accessibility acceptance, installed upgrade/migration/restore and lifecycle operation, clean-clone/release provenance and redistribution/license/SBOM clearance. Older strict clients/installers still require the rebuilt paired components; no mixed-version compatibility or binary-only storage rollback guarantee is added. None of these limits grants or implies accepted risk, and no installed upgrade or real-key/API test is authorized.

- **Final scoped assurance verdict: PASS — the approved saved-key source/test/build/package deliverable and its evidence binding meet the reviewed contract; H1 and G1 are CLOSED.** This supersedes only the earlier current-task pending-verification verdict, not failed historical executions or broader unverified guarantees.
- **Release Gate Decision: NO-GO for release, publication, installed upgrade or deployment.** No human approval or accepted-risk decision is granted by this review; separately bound approval and applicable remaining release/host gates are required before external effects.
- **Required remediation:** none identified within this bounded saved-key source/artifact scope. Retain all evidence and historical failure dispositions; rebind review/checks for any new source, package or requested external effect. Only this review was appended, with no other source/docs edits or validation reruns by the reviewer.
