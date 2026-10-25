# LOCAL-SMITH-DEV-DIAGNOSTICS

## Authority and bounded intent

**LOCAL_PLAN_APPROVED:** the operator approved a scoped local fix for safe prerequisite/error reporting with isolated tests, explicitly excluding installation, Docker/daemon contact and retry of the failed job. After a clarification about unsupported spending figures, the operator instructed this work to continue. Approval covers this diagnostic source/docs change and bounded offline checks only; it expires at completion or material scope drift. No commits, branches, remote effects, network, image preparation, live runtime checks, inference, deployment, historical-ledger edits or accepted risk are authorized.

Owner: developer. Risk: elevated error/recovery presentation at the existing runtime/privacy boundary, without changing authority or recovery rules. Repository: Smith; existing `feature/autonomy` bootstrap branch; HEAD `14d960975e76f906efb5e447ff3dece0e8e4385b`. Preserve all existing dirty/untracked work. Baseline SHA256: `service/src/app_dev_jobs.rs` = `78f4a9788c5141296cc5cdbced3d3ad451c1d4be14346366dd5b4db1b9c956d6`; `service/src/dev_runtime.rs` = `bb72430a1cc205525b7075b9d2d09d3fc0f9008defd1a33207391f0661a21ab4`.

## Independent analyses and planner reconciliation

Read-only producer, architect and designer analyses were obtained in independent contexts before implementation. The developer owns this planner reconciliation, implementation and self-check, not final assurance.

- Producer: distinguish established absence from inability to inspect; preserve historical generic records, observation-only reads, privacy and uncertainty. Specific copy must reach the existing summary without operational effects.
- Architect: replace arbitrary diagnostic text with a payload-free enum and exhaustive static renderer. Carry typed provenance through private adapter helpers; contextual daemon/image errors must preserve more specific prerequisite errors. Preserve all security predicates, command ordering, deadlines, guard placement, dispatch/cleanup flag calculations and schema.
- Designer: use the existing result/live-region presentation; report failure of a check without claiming continuous host health, successful cleanup, no execution or safe retry. No dynamic paths/output/prompts or repair controls.
- Reconciliation: retain the existing generic no-replay fallback and short static messages for positively classified prerequisites. A missing CLI/socket is distinct from an inspection error. Image/daemon command failures mean verification failed, never prove an absent image/stopped daemon. No new preflight endpoint, extra command, UI control or query-time probe. String-returning worker/event errors convert only to the generic diagnostic; they are never matched or exposed. Specific diagnostics do not override unknown/cleanup-unknown status. Broader readiness/recovery UX and runtime provisioning are deferred. No blocking disagreement remains in this scope.

## Requirements, implementation and expected evidence

| Requirement / acceptance | Task | Isolated evidence |
|---|---|---|
| DIAG-1: explain known CLI, private runtime/socket, rootless-check and image-validation failures truthfully | Closed diagnostic enum, classify at existing checks, preserve prerequisite provenance through operation context | Exact static-message/bounds tests; synthetic metadata-error and daemon/image response cases |
| DIAG-2: never disclose raw errors, paths, prompts, stderr or inspected metadata | Payload-free failure type, generic conversion for unsupported string errors | Private-text/lookalike canaries; summary field-count and static diagnostic assertions |
| DIAG-3: preserve dispatch/cleanup ambiguity and existing execution boundaries | Replace message publication only; retain flag/status formulas and runtime security/command sequence | Fake-runtime failure flag matrix, zero additional stop probes; existing receipt/publication/restart/admission tests |
| DIAG-4: preserve wire/disk compatibility and historical evidence | Existing message field only; no migration or backfill; no client changes | New summary/ledger load, unchanged historical failed-record bytes after reopen/read; no runtime calls |
| DIAG-5: document narrow behavior and validate without live effects | Affected Architecture/Design detailed contracts and evidence update | Scoped formatting, offline check/Clippy, focused `dev_` tests, fresh independent source assurance |

Focused checks from `service/`: scoped `rustfmt --check`; `cargo check --offline --locked`; warning-denied `cargo clippy --offline --locked --lib --bin smith-service -- -D warnings`; `cargo test --offline --locked --lib dev_ -- --test-threads=1`. Use existing local caches, synthetic data and fake runtimes only. Missing offline capability is a blocked check, not authority to download or try live services. Full suites, real-image/provider behavior, installed/graphical acceptance and release gates remain pending as in the original [runner record](LOCAL-SMITH-JOB-RUNNER.md).

## Rollback and evidence

Source-only change: no installed migration or host rollback follows. Any later source rollback must preserve unrelated work and retained job evidence. The original failed request's exact diagnostic was discarded and must not be reconstructed from later observations.

## Implemented source and self-check — 2026-10-18

**Implemented source; initial focused self-check PASS.** Final snapshot reconciliation and bounded assurance are recorded below. `service/src/dev_runtime.rs` replaces raw failure text with a payload-free diagnostic enum/static messages and private typed propagation. It classifies only at existing checks, preserves specific prerequisites during daemon/image context mapping, and uses a generic fallback for unsupported string errors. Pure daemon/image response helpers retain their previous predicates. `service/src/app_dev_jobs.rs` publishes only the static message and adds fake-runtime regressions. No client, wire/disk schema, worker entry, Docker command/flag, admission, dispatch or cleanup decision was intentionally changed. Architecture/Design entry and detailed contracts, worker notes, runner-plan link and observed-source assessment document the narrow follow-up.

Checks ran against the actual dirty worktree with existing caches and no dependency downloads, not a clean reconstruction. Rust/cargo1.98.0. Observation time: `2026-10-18T21:49:38Z`. No live Docker/daemon/provider/installed-state action or job retry ran.

| Working directory / exact check | Expected | Actual |
|---|---|---|
| `service/`: `rustfmt --edition 2021 --config skip_children=true src/dev_runtime.rs src/app_dev_jobs.rs` | Format touched Rust | PASS |
| `service/`: `rustfmt --edition 2021 --check --config skip_children=true src/dev_runtime.rs src/app_dev_jobs.rs` | No formatting differences | PASS |
| `service/`: `cargo check --offline --locked` | Compile using local dependencies | PASS |
| `service/`: `cargo clippy --offline --locked --lib --bin smith-service -- -D warnings` | No production warnings | PASS |
| `service/`: `cargo test --offline --locked --lib dev_ -- --test-threads=1` | Focused synthetic/fake-runtime regressions pass | PASS:14 passed,0 failed/ignored,178 filtered out |
| Repository: `git diff --check` scoped to the11 touched source/docs/evidence paths | No tracked-diff whitespace errors | PASS; untracked files are not covered by Git's tracked diff |

DIAG-1/2: four runtime tests cover exact allowlisted bounded/control-free messages, metadata-error absence versus inaccessibility, untrusted/lookalike string fallback, operation-context provenance, malformed JSON and rootless/image response validation. Security predicates are retained for source comparison; these are not live filesystem/Docker enforcement tests. DIAG-2/3: app tests cover private-error summary exclusion and all8 combinations of persisted/failure delivery and cleanup ambiguity, unchanged nine-field summaries, disk reload and no error-path stop probes. DIAG-4: historical generic terminal record reopen preserves bytes and makes zero runtime calls. Existing admission, restart/corruption, prior-cleanup blocking, publication fencing, verified receipt/stopped-runtime and worker framing/receipt tests remain selected and pass. No new read-query probing code was added.

No formatting/build/lint/test failures occurred in this follow-up's check invocation; the original runner's historical failures and broader missing gates remain in its record. No Flutter or full component/root/build/package, real-image/provider, native graphical or installed-host acceptance was run or inferred.

Checked source SHA256 (not whole-candidate provenance):

```text
5676ae70c40566ee181c4041426d8f00c13ed12594d0a417534ced793edb94af  service/src/app_dev_jobs.rs
770926315d655160ed047ad8222555a6c941f0bcd5036776f002c9b5705e7749  service/src/dev_runtime.rs
```

### Concurrent-source reconciliation and second self-check

Fresh source assurance found no blocking diagnostics implementation defect, but flagged a test-inventory mismatch: the initial14-test result no longer matched the visible15-test source. Inspection confirmed concurrent non-diagnostic work had changed the ledger agent-ID validation to `agents::valid_agent_id` and added `custom_agent_ledger_reopens_without_runtime_effects_and_rejects_noncanonical_ids`. Those edits were not authored by this diagnostics task and were preserved, not reverted or claimed as part of it. The first14-test result and hash above remain valid historical evidence for their earlier snapshot, not the later one. Other concurrent source work also changed the filtered-out count.

`cargo test --offline --locked --lib dev_ -- --list` then listed15 tests (nine app, four runtime, two worker). The added concurrent test was inspected to confirm it uses a private temporary ledger/fake runtime, not live Docker. Scoped formatting, `cargo check --offline --locked`, warning-denied production Clippy, and `cargo test --offline --locked --lib dev_ -- --test-threads=1` were rerun against the later snapshot: **all PASS;15 passed,0 failed/ignored,185 filtered out**. SHA256s were captured immediately before and after that check group and matched:

```text
28fe809ed57530c743444ddf745622e45404b71336605bfc23e200bc2fe966bd  service/src/app_dev_jobs.rs
770926315d655160ed047ad8222555a6c941f0bcd5036776f002c9b5705e7749  service/src/dev_runtime.rs
```

This binding is limited to the two scoped source files and actual dirty worktree; it is not a stable whole-repository or clean-candidate provenance claim. No diagnostics code change was needed to reconcile the inventory. No live operations or downloads were introduced by the rerun.

### Independent assurance

Initial fresh read-only review found no critical/high or blocking diagnostics implementation defect. It confirmed payload-free static copy, no raw-string promotion, no inferred daemon/image absence, unchanged supplied flag/status formulas, gated success, and no historical/query probing or backfill. It withheld source-bound assurance because of the now-reconciled test inventory mismatch; that evidence finding and both snapshots are retained above. Follow-up read-only review confirmed that the preserved historical result, current15-test listing, fresh passing check group and matching two-file hashes resolve the inventory blocker. **Bounded diagnostics source/evidence assurance PASS; no blocking diagnostics code fix remains.** The reviewer did not execute commands/recompute hashes, no baseline source snapshot is available, and two-file binding is not whole-input/clean-candidate provenance. Concurrent custom-agent functionality is expressly outside this diagnostics assurance. Overall runtime/installed/graphical/release acceptance remains unestablished. No external action or accepted-risk approval follows from self-check or source assurance.
