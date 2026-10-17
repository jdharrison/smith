# LOCAL-SMITH-RUNNER-PARKED-LEASE

## Approval and scope

**LOCAL_PLAN_APPROVED:** the operator's “go ahead” following the read-only diagnosis approves this bounded local implementation, synthetic tests, documentation, build/package validation and fresh independent review. Repository `smith`, existing bootstrap branch `feature/autonomy`, base `c1339f3670c70582db05a158b46821353662b87b`. Approval ends on completion, revocation or material unreviewed scope/base drift. No commit, push, hosted rerun, PR mutation, merge, release, installation, live service/keyring/provider/model/GPU/GUI or accepted-risk effect is included.

**Elevated:** production runner lease ownership changes; architecture review and negative exclusion tests are mandatory. No new capability, schema, endpoint, permission, dependency, timeout, retry or scheduling policy.

## Diagnosis and uncertainty

GitHub run39002802336 attempt1 on the base passed the keyring regressions and108 service library tests, then failed `provider_prefers_full_vulkan_and_falls_back_once_only_before_completion_dispatch` at `service/tests/runner.rs:2130`: RUNNER_BUSY during model lease acquisition; runner target40 passed/1 failed. The source/archive job passed; downstream package verification was skipped. This is a synthetic runner test, not GPU inference or a builder GPU requirement.

`Providers::chat` drops its local `VerifiedModel` after successful preparation. Its `Runner` retains the return-slot Arc. Terminal shutdown can return a raw lease to that final slot, whose ordinary destruction closes without explicit LOCK_UN. A concurrent fork can retain the shared open-file-description until exec and extend exclusion beyond the parent's close. The unleased shutdown branch already explicitly unlocks for this reason. Adjacent test code documents an inherited-lease observation workaround.

An empty anonymous-memfd/dup/fresh-open diagnostic confirmed the kernel mechanism: close-only with a duplicate remains busy (errno11); explicit unlock permits acquisition while the duplicate remains open. This is not actual-runner or historical CI reproduction. The hosted log lacks scenario/index, flock errno, exact holder and predecessor cleanup outcome; do not claim a proven historical schedule.

## Independent analyses and reconciliation

- Architect `f8bd63ed-6861-489a-8402-df9fe156d6d7`: private typed parked-slot final destructor, existing Arc/Mutex transfer, remove model-only destructor; retain raw active file and unknown-cleanup early return. No reference-count heuristics. Poisoned parked/empty slots need coverage. No architecture blocker to this bounded scope.
- Producer `0d34c914-4f8f-4f55-8322-f61d646b8d3f`: terminal stop/drop must release despite a retained OFD duplicate; active ownership and live-model fallback must remain excluded; preserve removal/teardown/no-replay/deadline regressions.
- Designer `1e752608-f903-431a-b87b-f81b6e4e532a`: invisible correctness repair, no UI/messages/consent changes; preserve factual busy and unknown outcomes, no automatic recovery/replay. Update architecture and as-built assessment.
- Reconciled AC4: deterministic unknown-cleanup **ownership-state simulation** plus static proof that the real cleanup-failure branch remains unchanged is adequate. Do not claim execution of an actual child-exit timeout. Existing directory-cleanup-refusal integration test is separate evidence, not an unkillable-child test.
- Proposed pre-child guard expansion is deliberately deferred: verification-before-slot-attachment and launch-after-consumption/before-construction raw-file error windows remain existing residuals. They need separate discriminating fault coverage and are not repaired or risk-waived here. No generic unlock-on-drop is added to active files.

## Implementation contract

Introduce private `ParkedLease(Option<File>)`. Its destructor explicitly unlocks only a file it actually owns, without locking the surrounding mutex or panicking. Store it in existing shared slots: `VerifiedModel.lease` and `Runner.lease_return` become Arc/Mutex of this type. Preserve take → raw active file → return. Remove `VerifiedModel::drop`; the final parked value owns release even when the model owner is gone or the mutex is poisoned.

A populated slot must mean no child started or cleanup confirmed. Empty-slot destruction does nothing. `Runner.lock` stays `Option<File>` with ordinary file destruction. Preserve the real unconfirmed-child-exit return before handback, process signaling/waits, cleanup refusal, fallback classification, no replay, original budgets, one-runner policy and all existing tests. No sleeps/retries/extra waits/serialization to mask the failure.

## Traceability and validation

| ID | Blocking requirement | Evidence |
|---|---|---|
| L1 | Final populated-slot destruction explicitly releases after model owner is gone | Separate actual private `Runner::stop`/drop ownership tests with no-child/already-cleaned runner and retained OFD duplicate; independent fresh-open contender succeeds before duplicate closes; destructor-disabled negative control fails |
| L2 | Dropping model during active ownership never unlocks | Empty-slot/raw-runner test; independent contender remains busy; unsafe unconditional-unlock negative control rejected |
| L3 | Live-model cleaned handback retains one continuous lease | Handback, re-consumption and final owner release with independent lock probes and retained duplicate; existing real fake-runner GPU→CPU/original-deadline/no-post-acceptance-replay test |
| L4 | Unknown cleanup remains exclusion, no fallback | Raw active-file/empty-slot/retained-duplicate state simulation; exact WouldBlock check, positive release control; unchanged real early-return/source proof; existing cleanup-refusal integration gate |
| L5 | Poisoning cannot suppress final parked release or unlock empty active slot | Populated and empty poisoned-slot destruction tests |
| L6 | Stable source/build/package gates, no scope drift | Focused ownership tests, baseline negative controls, full runner target, predeclared repeated runner target under bounded CPU constraints, locked formatting/Clippy/full service; configured aggregate/build/package and pipeline helpers |
| L7 | Canonical contract/evidence and fresh independent assurance | `docs/ARCHITECTURE.md`, as-built report, `VALIDATION.md`, compact evidence and reviewed source/artifact hashes; stop before commit/remote effects |

Tests use private roots/regular files, retained duplicate descriptors sharing the original OFD and independently opened contenders. Explicitly unlock successful probe acquisitions before closing to avoid creating new inherited-description flakes. Unit ownership tests are not actual model inference, child-timeout execution or full fallback protocol tests. Negative controls and any failed setup/test remain recorded; no retry-until-green.

One build/test pipeline at a time. Existing approved Dagger engine/cache and checksum-verified official v0.21.9 CLI are allowed; disclose cached versus fresh test results and configured Bookworm service container versus Ubuntu installer/client/package gates. No clean-cache, hermetic, hosted, installed or provenance guarantee. Finite command timeouts; bounded synthetic subprocesses only. Record exact commands, outcomes/counts/ignores/skips, source inventories, sanitized log hashes and package identities under `docs/evidence/runner-parked-lease/`; raw diagnostics stay ignored under `target/runner-parked-lease/`.

## Outcome

**Implemented; isolated local source/build/package gates PASS; fresh independent assurance PASS for this bounded candidate.** The two frozen runner sources satisfy the bounded ownership repair. Seven new ownership tests, eight existing private tests, ten predeclared exact integration samples, both discriminating negative controls and the separately recorded corrected41-test runner target completed. The once-only pinned-base-plus-two-overlays CI-equivalent passed1273 fresh aggregate tests plus34 actual-package helpers, five controlled ignores and zero skips. Harness/setup failures and corrections remain preserved in the [evidence](../evidence/runner-parked-lease/README.md). [Fresh independent assurance](../evidence/runner-parked-lease/REVIEW.md) verified L1–L7, source/evidence/artifact bindings and preserved limits with no blocking findings; it grants no external-action or residual-risk approval.

Concurrent Projects changes are excluded from the pinned candidate; no acceptance of the live mixed worktree is claimed. Live index bytes changed with unresolved attribution; no restore/staging was performed. Unknown cleanup remains an ownership-state simulation plus unchanged-branch review, not actual child-timeout execution. The existing pre-child raw-file residuals remain neither repaired nor risk-waived.

Existing hosted failure remains failed until separately authorized updated CI succeeds. No changes to the approved source snapshot may be silently carried past final review. Local rollback is a scoped reversal of this repair, not a reset, cache purge or installed-state rollback. No commit, push, hosted rerun, merge, release or installation occurred.
