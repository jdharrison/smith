# LOCAL-SMITH-KEYRING-PEER-COST

## Approved local intent

The operator requests implementation after the read-only source/web scan of CI run38996557862. Base `becc14ed00da326e9ef8585d1fa309e893be48b2`, existing `feature/autonomy`. Scope: test-only fake-peer cost repair, regressions, isolated measurements/gates and evidence. No commit, push, CI rerun, merge, installed/keyring/provider/model/GUI or release effect is authorized. Approval expires on completion, revocation or material unreviewed scope/base drift.

The hosted service test `actual_session_negotiation_matches_independent_peer_with_leading_zero_shared_secret` failed with KEYRING_TIMEOUT at its final unwrap; the Dagger/asyncio traceback propagated Rust exit101. Historical logs do not distinguish its two-second outer deadline from its one-second save-effect deadline or identify a precise CPU stage.

## Evidence and reconciled design

The private peer searched ascending exponents2 through99,999, recomputing `public.modpow(private, prime)` each time until the shared secret needs128-byte leading-zero padding. This synchronous search occurs within async OpenSession on a current-thread runtime. Tokio1.53.3's upstream timeout documentation confirms non-yielding work is not preempted; source also rechecks elapsed time afterward. These facts identify a concrete variable-cost fixture, not an upstream library defect or proof of the historical failing stage.

Fresh isolated locked/offline Rust1.98 test-profile measurements compared exact first candidate/shared results on16 deterministic cases. All16 matched;96 timed pairs completed without retries. The longest measured scan (546 candidates) used about404ms CPU before versus8ms after under normal conditions; under fixed synthetic contention, about1,141ms wall before versus17ms after. Finite measurements are not a worst-case bound or hosted CPU profile. Raw evidence remains ignored under `target/keyring-fixture-fix/measurement-20261016-becc14e/`.

Independent architecture context `63a09cb7-cb34-4a67-9c08-dd2d600c3249` supports the minimal recurrence: initialize S2=public² modp, then Sn+1=Sn×public modp. Induction preserves every residue and first match. Keep ascending2..100000, canonical byte predicate and exhaustion failure. Do not substitute a fixed production private value, inject the session key, add fallback, use spawn_blocking or increase deadlines.

## Acceptance and implementation

| ID | Requirement / task | Required evidence |
|---|---|---|
| K1 | Pure test-local incremental search in `service/src/cloud_keyring_tests.rs` only | Same first private/shared result as original modpow oracle on bounded deterministic vectors; bound/order/exhaustion unchanged |
| K2 | Keep actual encrypted negotiation and independent derivation | Existing random-client end-to-end test,128-byte padding, peer HMAC/HKDF, returned public key, encrypted save/read, owner/UID/call/target assertions unchanged |
| K3 | Preserve all budgets and production behavior | No production-file/profile/dependency/workflow or OsRng changes; two-second test wrapper, one-second save guard, production three-second cap and100/150ms stalled tests unchanged |
| K4 | Validate under load and broadly | Fixed-case recurrence/oracle regressions outside timed protocol wrapper; predefined constrained private-session runs; all keyring tests, locked service fmt/Clippy/tests and once-only CI-equivalent aggregate/package gate |
| K5 | Independent assurance and honest binding | Actual hashes/counts/logs, preserved failures, cache and historical-attribution limits; fresh review before commit/remote approval |

The mathematical regression uses2, p−2 and inverse-two `(p+1)/2`; inverse-two's first padded hit is private8 with127-byte shared value, while private2–7 remain128-byte. This covers progression, modular reduction, first-match selection and padding independently of timing. The expensive reference oracle runs only on short fixed cases, outside the timed private-bus flow.

## Safety and outcome

Use only private socket-pair/synthetic credentials; no user session bus or keyring. Keep production randomness and all security/timeout checks. Do not retry failed samples until green or treat cached steps as newly executed. This optimization still has a finite variable-length synchronous search; measurements do not establish hard-preemption or worst-case host guarantees. Preserve those existing limits rather than broadening this repair.

Validation and final review are retained in `docs/evidence/keyring-peer-cost/` and root `VALIDATION.md`. Hosted CI remains failed on the old revision until a separately authorized updated run passes. Rollback is a scoped test-file reversal, never reset or live-state change.

## Local implementation outcome

**Implemented K1–K5 with fresh independent source assurance; not hosted or release acceptance.** The only code change is the incremental peer helper plus two pure regressions. The original random-client private-session test, peer derivation, production files/randomness and all budgets remain unchanged. Candidate test-file SHA256 `bd51d10ff5bc2a6657df7b06aa5d85d8dcaf445a76ec7ac2b36fa095ae565888`.

Both new regressions and all20 keyring tests passed. The predeclared20 normal/20 constrained original-session repetitions passed (constrained maximum208.116ms); separately instrumented10+10 sessions passed. Two scratch setup/binding failures are preserved, with no test retry or replaced sample. Finite samples do not establish worst-case timing or the historical timeout stage.

The once-only root aggregate/build/verifier passed:265 service +283 installer +9 GTK +668 Flutter +41 actual-package =1,266 current stdout passes, five controlled fixture ignores and zero failures/skips. Separate actual-package pipeline helpers passed34/34. Formatting/locked warning-denied Clippy and unchanged installer/runtime validation passed. All1,230 core inputs stayed stable. Existing dependency/build caches, configured Bookworm service tests, helper-after-build ordering and unproven single overall recorder deadline are explicitly qualified in the [evidence](../evidence/keyring-peer-cost/README.md). Original command/log/artifact records remain preserved.

No runtime/policy contract changes; no canonical runtime-policy amendment is required. Local documentation/evidence closure is outside core build inputs. No commit, push, hosted rerun, merge, tag, release, installation or deployment occurred under this repair. Fresh-context [independent review](../evidence/keyring-peer-cost/REVIEW.md) found no source-repair blockers, verified the evidence and passed all20 keyring tests on its own finite run. Its pass-with-risk verdict retains finite timing/environment limits, not accepted-risk authority; the historical supplementary plan hash is preserved and final documentation rebound in the review. Human authorization for any next external effect remains a separate gate.
