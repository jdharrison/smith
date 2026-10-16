# LOCAL-SMITH-NEXT-SAVED-KEY-ATTEMPTS

## Approval, bounded intent and supersession

**Approved local source change:** the operator explicitly requests that a saved key be sufficient to attempt Send and Validate, without “Credential availability has not been observed in this service instance” or mandatory previous model validation/inference. Invalid or inaccessible credentials should fail during the explicit attempt. This narrowly supersedes the cloud plan's cached-observation and successful-validation eligibility requirements; it does not weaken actual credential retrieval, identity, consent or execution safeguards.

Repository `smith`, existing `feature/autonomy`, base `7f6a483ae82c4bcf647268f035f1e7cc80409d4b`, resulting bounded uncommitted delta plus all preserved current cloud/catalog/keyring, Boot/header/scroll, queue and timestamp work. Approval expires at task completion/revocation/material scope or base change. Credential-related work requires architecture input, relevant tests and fresh assurance. No installed upgrade/restart, real key/bus/keyring/API access, paid inference, model download, commit/branch/publication or cleanup is authorized. The separately discussed discovery/persisted-catalog/upgrade-uninstall lifecycle remains analysis-only and excluded.

## Root cause and independent reconciliation

**Implemented baseline:** `app_v3.rs::ready()` checks active model and credential generation, then rejects absent/adverse current-instance keyring observations or no successful validation. Fresh application instances reset volatile observations while keeping durable credential references. Thus restart can disable a previously working route without any provider rejection. Validate itself already accepts a saved generation; old catalog parsers can prevent the corrected response from reaching otherwise-correct controls.

- **Independent architect** `08a8b9f7-eb0d-45e5-8d15-938fbd400306`: no architectural blocker to removing only the two preflight gates. Worker `key()` freshly retrieves the exact credential and checks provider revision, bounded access, ownership and cancellation before durable dispatch marking and HTTP. Preserve that path, all consent/deadlines/storage/no-replay and keyring/HTTP implementation. Service-only changes are insufficient: Flutter and installer catalog readers enforce the old relationship.
- **Independent producer/designer** `2612e754-e681-451a-a1ec-3ae5a3d530b8`: eligibility, cached observations and past model-test facts must remain separate. Keep saved-but-unobserved/adverse/failed/interrupted states eligible, with advisory copy. No generation remains unconfigured. A completed adverse probe must not block a new attempt, but an unresolved operation reservation/uncertain acknowledgment must not be discarded or automatically reissued.
- **Planner reconciliation:** no blocking disagreement. Change the shared eligibility contract consistently, keep observation/error/validation facts truthful, and add no implicit startup/selection/Send probe. Existing optional Retry observations remain optional. A new explicit attempt is distinct from replay of prior accepted or uncertain work.

## Contract

For a supported cloud route, an active saved `CredentialRef` with matching exact generation is sufficient for preparation/admission, subject to unchanged busy/capacity/revision/context/consent checks. In `route_catalog`, cloud `available` means **eligible to attempt**, not guaranteed key accessibility, authentication, billing or model availability. Provider observation and validation remain independently factual. No active generation stays disabled; stale generations and unsupported/wrong-provider models stay rejected. Local readiness remains unchanged.

Actual Send and Validate each freshly read their admitted exact credential at execution. Read failure terminates before HTTP with `not_sent`; a readable invalid key can produce the actual sanitized provider error. There is no automatic unlock, retry, credential substitution or model/provider fallback. A normal chat success does not invent an explicit validation record. `validated_model` continues to identify only the exact model/generation with a successful explicit validation.

This changes application-schema3 field semantics without changing shapes, disk4 or Core2. Old strict catalog readers may reject the new combinations: rebuild/test paired service/client/installer; no version fallback or mixed-version guarantee.

## Requirements and evidence

| ID | Task | Blocking evidence |
|---|---|---|
| TRY-R1: saved identity permits explicit attempts | Remove only observation/validation preconditions in service cloud eligibility; synchronize both DTO readers | Both-provider matrices over unobserved/available/adverse observation and null/failed/interrupted/succeeded validation; configured vs missing/stale generation negatives |
| TRY-R2: restart and recovery do not require preliminary work | Keep durable references, independent advisory facts and optional probe behavior | Saved-but-unvalidated cold-instance Send/Validate; failure then external repair/fresh explicit attempt without intermediate probe; no startup/catalog side effects |
| TRY-R3: real execution boundary remains | No worker/keyring/HTTP/consent/storage/queue changes | Exact-generation read and call counts, keyring failures with zero HTTP/not_sent, synthetic auth/quota errors, no automatic extra attempt, existing stale/consent/unknown-delivery regressions |
| TRY-R4: truthful client and installer experience | Adapt parser relationships and limited advisory copy, no UI redesign | Eligible choices/Send/Validate without clearing cached errors; exact verification flags; malformed DTO rejection unchanged; narrow picker/status tests |
| TRY-R5: actual integration and delivery | Synthetic compiled service-domain installer and Rust-Dart cold-credential journeys; docs/evidence | Focused checks, stable configured component/build/package evidence, retained aggregate outcomes and fresh assurance/source/artifact binding |

## Ownership and implementation limits

- Backend/installer slice: `next/service/src/app_v3.rs`, optional cloud diagnostic wording in `cloud.rs`, cloud tests and private `debug_service` fixture; installer `providers.rs` parser/status/guidance and related tests, narrowly scoped GUI tests/copy only if needed. No worker, HTTP, keyring or lifecycle changes.
- Client slice: `next/client/lib/cloud_routing.dart`, `lib/ui/cloud_selector.dart` advisory presentation and related parser/retry/picker/routing/compiled integration tests. No controller/queue/Boot/header/ChatPanel behavior edits.
- Parent: documentation/API contract amendments, evidence, validation ledger, integration and fresh assurance. Record fixture-interface coordination before client/server divergence.

## Validation and boundaries

Preserve initial source inventory and scoped prechange copies under ignored `next/service/build/saved-key-attempts/`. Use only synthetic credentials/private roots/fake HTTP; never normal-host keyring or provider access as a shortcut. Start with focused service/installer and client tests, then stable configured component/aggregate/build/package gates with cloud tracing disabled. Distinguish controlled fixture ignores from skipped required integration.

Known unrelated aggregate runner lock/timing failures remain unaccepted. Do not fix them here or retry the aggregate indefinitely; retain any failed execution and distinguish separate component/package results. Fresh independent assurance binds the actual scoped delta and artifacts, not a previous task's PASS. No live repair or installed restart is authorized or claimed.

## Outcome

**Implemented:** the four production-file changes are limited to service cloud eligibility, installer catalog parsing/advisory copy, Dart catalog parsing and picker advisories. Worker/keyring/HTTP/storage/controller/queue/lifecycle and model catalog behavior are unchanged. Backend263 full host tests plus20 focused installer tests and client164 focused tests passed, including six compiled integration cases; these overlap final counts and are not additive.

The initial full-client gate passed663/failed3 because entrance fixtures disabled active cloud generations. Test-only reconciliation now uses coherent generation replacement, preserves local outage/recovery/no-effect assertions and adds two replacement cases. H1 was independently closed after targeted28 entrance/three unchanged-golden visual/164 cloud passes; production was not changed for this remediation.

**Final configured gates PASS:** one `make test build` plus one explicit package verification on stable1,291 inputs passed263 service,281 native installer,nine GTK,668 Flutter and41 package tests; zero failures, five controlled fixture ignores, zero Flutter/package skips. Formatting/analyzer/Clippy/runtime gates passed;95 manifest-listed recipient files verified,97 total. All four service copies and91 client/payload pairs agree. [Retained evidence](../evidence/saved-key-attempts/README.md) binds source/commands/artifacts; [independent review](LOCAL-SMITH-NEXT-SAVED-KEY-ATTEMPTS-REVIEW.md) supplies scoped findings and final binding. Prior runner failures remain historical evidence, not causally fixed or accepted by this successful run. No installed-host remediation, real key validity or vendor availability is established; the rebuilt local package has not been deployed.
