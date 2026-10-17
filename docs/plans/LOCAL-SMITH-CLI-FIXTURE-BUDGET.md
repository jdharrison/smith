# LOCAL-SMITH-CLI-FIXTURE-BUDGET

## Approved bounded intent

The operator requests “fiiiixx iittt” after diagnosis of PR7 / CI run38982647442 at `766d63b481702c21f5fee6ca9c2dc64c30a4d80d`. This approves a local fixture-only repair and isolated validation on existing `feature/autonomy`. No commit, push, hosted rerun, merge, tag, release, installation or live service/provider effect is authorized. Approval expires on completion, revocation or material scope/base drift.

The hosted installer CLI suite passed14/failed1: `accepted_artifacts_then_missing_runtime_roots_report_partial_effects_without_manager` hit the test harness's30s child watchdog before its outcome assertions. Hosted scheduling/CPU/backend details were not recorded; the exact historical slowdown is not proven.

## Diagnosis and reconciliation

Exact committed source was built and inspected in isolated scratch. The unchanged test passed locally in6.98s, primarily user CPU, with negligible input/lock/sync time. A software-SHA control also passed in8.73s and does not establish the hosted cause.

A controlled comparison kept the executed16.6MB test-built installer, private roots, consent, modes,30s external budget and synthetic CPU contention constant. Three installer-sized inert payloads timed out at30.03s with the same last prompt/empty stderr; a published service file proved consent consumption and publication progress. Replacing only those payload bytes with a genuine26,936-byte native ELF completed in0.15s, exit1 as expected, with all original partial-effects/no-manager checks satisfied. This establishes a reproducible payload-cost trigger, not a measurement of the historical hosted runner. Initial scratch preflight failures are retained separately, not counted as performance observations.

Independent diagnostic review `241aa4c0-b0b7-450d-b14b-c90ea0767d22` verified source identities and the comparison, and found no CLI test requiring installer-sized inert payloads. Its narrow recommendation is adopted: reduce fixture payload cost, not checks or budgets. Raw diagnostic evidence remains ignored under `target/ci-diagnosis/`.

## Requirements and implementation

| ID | Requirement / acceptance | Change and evidence |
|---|---|---|
| F1 | Execute the actual Cargo-built installer unchanged | Keep `CARGO_BIN_EXE_smith-installer` copy as running CLI; source diff and CLI suite |
| F2 | Avoid irrelevant repeated hashing of large inert payloads | Read the supported Ubuntu builder's small `/usr/bin/true` ELF for exactly three independent0755 client/service payload copies; assert ELF magic and≤1MiB size |
| F3 | Preserve semantics and safety coverage | Same helper bytes, independent files/modes, namespace isolation, assertions, consent,30s deadline, output limits, publication and manager/provider boundaries; no production/workflow changes |
| F4 | Validate the repair rather than retry until green | Formerly failing exact test under the fixed controlled contention, all15 CLI tests, formatting/warning-denied Clippy and configured native installer/isolated GTK/no-Python gates; actual-ELF package coverage retained |
| F5 | Honest evidence and independent assurance | Bind changed source/test results; preserve historical CI failure and diagnosed uncertainty; fresh reviewer checks no weakened test or replaced real installer |

This fixture uses a native system executable only as inert bytes, never as an executed client/service. It adds no recipient runtime dependency or shipped binary. No arbitrary endpoint, environment fallback, source/profile/dependency upgrade or concurrency change is included.

## Validation and rollback

Run finite commands with private temporary roots and the configured Ubuntu builder. Build/cache effects are local validation only; no normal account keyring, user manager, models or GUI session is used. Record all failures rather than hiding them through retries. Rollback is a scoped reversal of this test-only change, not repository reset or installed-state action.

[Executed results](../evidence/cli-fixture-fix/README.md) record the fixture-only implementation, constrained exact-test PASS0.3245s, direct281 native passes and formatting/Clippy, plus configured281 native/nine GTK/41 actual-package passes (331 total) including no-Python runtime. All1,062 selected inputs stayed stable; no full service/Flutter aggregate rerun is claimed. [Independent review](LOCAL-SMITH-CLI-FIXTURE-BUDGET-REVIEW.md) and root `VALIDATION.md` retain the final disposition. Hosted CI remains failed until a separately authorized push/rerun actually passes; local results are not GitHub acceptance. No commit, remote action or host install occurred.
