# LOCAL-SMITH-CLI-EXECUTABLE-FENCE

## Bounded local repair

Continue diagnosis/repair of PR7 installer fixtures after the operator reports a new failure in run38994173360 at `bd663a34e6433fc61deaf36b190b0c11af21da97`: two calls to `Command::spawn()` return errno26 (`ETXTBSY`) before installer logic runs; CLI suite13 passed/two failed in0.46s. This is distinct from the previous fixed30s payload-cost timeout. The prior small inert payload change remains.

Scope: local `installer/tests/cli.rs` fixture synchronization and regression tests, isolated validation and evidence on existing `feature/autonomy`. No production, workflow, dependency, deadline, installed-host, remote/ref, commit, push or CI-rerun action is included. Preserve unrelated work; approval/intent expires on task completion, revocation or material scope/base drift. Explicit permission remains required before external effects.

## Diagnosis and architecture reconciliation

Each fixture copies the actual built installer while other tests may create processes. A child can inherit a concurrent copy's writable descriptor and keep it until exec/exit; `CLOEXEC` acts at exec, not fork. Unique paths prevent same-name collisions but not this inheritance window. Closing the original thread's descriptor does not close inherited copies. This is a concrete Linux race and a strong explanation for the observed errno; the exact descriptor holder in hosted CI was not captured.

Independent architecture context `da4ab02d-5053-4858-9485-a656dbbc5e43` recommends the established `service/tests/debug_service.rs` executable-I/O fence pattern. Retained `service/TEXT-RESET-L1-INVESTIGATION.json` contains earlier controlled inheritance evidence, not a fresh observation of this hosted failure. Reject hardlinks/shared source inode mutation, temp-rename-only fixes, spawn retries, sleeps, timeout changes and whole-test serialization.

## Requirements and implementation

| ID | Contract | Implementation / evidence |
|---|---|---|
| E1 | Prevent executable writers overlapping participating child creation | One module-local mutex shared by executable copying and every process spawn through exec acknowledgement; all seven launch sites use one wrapper |
| E2 | Keep meaningful concurrency and test behavior | Lock covers only copy/permission and spawn windows; release before input, consent, child lifetime, capture, publication and cleanup; retain existing construction/cleanup interleaving test |
| E3 | Preserve real binaries and isolation | Actual Cargo-built installer remains an independent copy; small genuine inert payloads/modes/helper equality unchanged; no hardlinks or new source/endpoint overrides |
| E4 | Deterministic regression, not retries | Verify both copy and spawn are blocked while the fence/writer is held; close/release before assertions/joins; bypass fence only in disposable negative-control source and require regression failures |
| E5 | Validate and independently review | Default-parallel CLI suite with a predefined bounded repeat count; locked native fmt/Clippy/tests; configured installer/isolated GTK/no-Python/package gates; stable source/evidence and fresh assurance |

Keep all original assertions,30s child watchdog, output bounds and namespace lifecycle. Test-only short critical-section serialization is not a service/application concurrency change. No canonical runtime policy change is intended.

## Execution and evidence

Use private roots, no ordinary manager/keyring/provider/model/GUI session. Record controlled negative results as expected failures, never overwrite prior CI/diagnostic evidence or retry until green. Tests, source identities, counts/ignores/skips and independent review belong in the evidence index/root validation ledger. Hosted CI stays failed until a separately authorized updated run actually passes. A local passing suite does not establish the exact historical holder or release readiness.

Rollback is a scoped reversal of this fixture-only repair, not history reset or live-host action. No commit/push/merge or user-service action is granted by this record.

## Local outcome

[Evidence](../evidence/cli-executable-fence/README.md) records a fresh controlled inherited-writer errno26 reproduction and success after unrelated exec closed the writer, both expected-red guard bypasses, both fixed guard passes and five predefined17-test parallel CLI passes. Configured selected gates passed283 native/nine GTK/41 actual-package tests (333 total), with original two native fixture ignores and no GTK/package skips. The subsequent once-only CI-equivalent `make test installer/build` and package verifier passed with1,264 aggregate coverage;333 cached installer/GTK/package results are explicitly bound to their prior execution, not claimed freshly run. All1,230 core inputs stayed stable. No gate, assertion, original watchdog or production behavior was weakened. Final independent closure is in [review](LOCAL-SMITH-CLI-EXECUTABLE-FENCE-REVIEW.md); hosted CI/external effects remain unperformed.
