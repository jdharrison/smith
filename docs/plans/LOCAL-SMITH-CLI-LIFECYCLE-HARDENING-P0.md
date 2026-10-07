# LOCAL-SMITH-CLI-LIFECYCLE-HARDENING-P0

## Status and authorization

- Status: **approved local implementation and QA** from the operator request to address the observed CLI lifecycle failures and harden them.
- Branch: `feature/autonomy` (existing bootstrap exception; do not rename or force-push).
- Base/HEAD at discovery: `825b8afe7aed14a44efda0c58fc74e65b8fb02b6`.
- Risk class: **Critical** — user-service restart policy, startup ordering, executable discovery, status truthfulness, and release behavior.
- Allowed effects: repository-local edits, builds, tests, mechanical Rust formatting needed to clear the required repository gate, hermetic sandbox smoke, and retained local evidence. Rustfmt followed the crate module tree from `src/main.rs`; the resulting additional Rust-file changes are formatting-only and were followed by full locked tests and strict Clippy.
- Excluded effects: local commits, normal-path installation, installed-binary replacement, live unit rewrite/reload/start/stop/restart, enablement change, login/logout, lingering change, push, PR mutation, merge, tag, release, deployment, destructive cleanup, and accepted-risk decisions.

This successor record is linked to `LOCAL-SMITH-USER-SERVICE-P0.md`. It does not rewrite or retroactively broaden that plan's evidence or approval scope.

## Triggering observations

Read-only inspection on 2026-09-11 found an installed legacy `smith.service` that was registered and enabled but in `activating/auto-restart`, with no Smith bridge listener and 7,971 recorded restarts. Its installed unit and executable differ from the current source policy. The journal showed each attempt performing warm/model work and then exiting because host-runtime OpenCode was unavailable in the user-manager environment. The error guidance was Windows-specific despite Ubuntu being the sole supported target.

Repository-local CLI QA also found:

- `cargo build --locked`: pass.
- shell syntax, ShellCheck, and `git diff --check`: pass.
- `smith version`: pass and reports `0.4.0`.
- `smith --version`: parser failure because the standard version flag is disabled.
- repository-wide `cargo fmt --check`: initially failed on inherited formatting drift. The operator directed failures to be addressed; the crate was mechanically normalized and the gate now passes.

Source changes cannot contain or remediate the currently loaded legacy unit. Any live containment or rollout remains separately approval-gated.

## Requirements

- **REQ-CL-001 — Permanent-failure containment:** a missing/unrunnable host OpenCode prerequisite is detected before warm service/model/sidecar work and exits with a stable configuration-failure status that the rendered unit does not restart automatically.
- **REQ-CL-002 — Generic restart fuse:** retryable or unclassified failures remain eligible for `Restart=on-failure` but are bounded by a deterministic start-rate limit. No start-limit action may reboot, log out, broaden privilege, or mutate lingering.
- **REQ-CL-003 — Ubuntu discovery:** host OpenCode resolution works in a minimal non-interactive Ubuntu user-service environment using an explicit absolute override, `PATH`, or documented stable per-user locations; candidates must be executable and pass a bounded version probe. Ubuntu errors must not contain Windows-only guidance.
- **REQ-CL-004 — Truthful status:** status preserves separate registration/load, active/substate, enablement, linger, and bridge observations and adds manager result, restart count, and last process-exit evidence without inferring unknown values or mutating the manager.
- **REQ-CL-005 — CLI identity:** `smith version`, `smith --version`, and `smith -V` report the Cargo package version successfully.
- **REQ-CL-006 — Existing safety invariants:** preserve stable executable validation, unit/install/start/stop/restart/autostart separation, no `enable --now`, no unmanaged fallback, no lingering mutation, loopback identity checks, pre-readiness reconciliation, SIGTERM handling, and separate optional client/sidecars.
- **REQ-CL-007 — Evidence honesty:** local and sandbox evidence is bound to the dirty worktree and never presented as proof that the installed legacy service was fixed. Mechanical repository formatting used to clear the required gate must remain distinguishable from lifecycle behavior changes.

## Reconciled design

1. Keep bridge port reservation and persisted-work reconciliation before readiness.
2. For host runtime, run a static OpenCode preflight immediately after reconciliation and before warm service/model/sidecar work.
3. Classify static host-runtime configuration failure with exit status `78` (`EX_CONFIG`).
4. Render `RestartPreventExitStatus=78` in `[Service]`.
5. Render `StartLimitIntervalSec=300` and `StartLimitBurst=5` in `[Unit]`; retain `RestartSec=15`, configure no disruptive `StartLimitAction`, and remove the unnecessary `After=default.target` self-ordering relationship.
6. Resolve OpenCode to a canonical absolute executable on Ubuntu from:
   - explicit absolute `OPENCODE_BIN` (invalid explicit configuration fails closed);
   - direct search of `PATH` entries;
   - `$HOME/.opencode/bin/opencode`;
   - `$HOME/.local/bin/opencode`.
7. Do not add a general environment/credential file in this change. That requires a separate allowlist, ownership, secret-reference, and migration design.
8. Query only non-sensitive manager properties required for diagnosis: `Result`, `NRestarts`, `ExecMainCode`, and `ExecMainStatus`. Failed queries remain unknown.
9. Restore Clap's standard version flag while retaining the existing `version` subcommand.
10. Do not add a combined retry/reset operation in this change; explicit start remains separate, and Smith must not automatically clear the manager's safety fuse.

## Acceptance criteria

- **AC-CL-001:** resolver tests prove explicit override precedence/fail-closed behavior, service-like `PATH` lookup, stable Ubuntu user paths, executable validation, canonical absolute return values, and Linux-only guidance. (`REQ-CL-001`, `REQ-CL-003`)
- **AC-CL-002:** startup tests or deterministic helper tests prove failed host OpenCode preflight occurs before warm application/sidecars/readiness and maps to exit `78`; existing listener/reconciliation behavior remains unchanged. (`REQ-CL-001`, `REQ-CL-006`)
- **AC-CL-003:** unit rendering tests and sandbox artifacts prove correct section placement for `StartLimitIntervalSec=300`, `StartLimitBurst=5`, and `RestartPreventExitStatus=78`, with existing hardening and lifecycle separation intact. (`REQ-CL-001`, `REQ-CL-002`, `REQ-CL-006`)
- **AC-CL-004:** fake-manager status tests prove result/restart/exit fields are truthful for a restart loop and remain unknown after failed/malformed queries. (`REQ-CL-004`)
- **AC-CL-005:** parser/process checks prove `version`, `--version`, and `-V` agree on `0.4.0`. (`REQ-CL-005`)
- **AC-CL-006:** repository-wide formatting, locked build/tests/Clippy, shell syntax, ShellCheck, sandbox smoke, and `git diff --check` pass. (`REQ-CL-006`, `REQ-CL-007`)
- **AC-CL-007:** final reporting distinguishes implemented source, sandbox evidence, and unchanged observed live state; no live manager mutation occurs. (`REQ-CL-007`)

## Implementation tasks

1. **TASK-CL-001:** implement injectable Ubuntu OpenCode resolution and diagnostics in `src/brain/host_runtime.rs`. (`AC-CL-001`)
2. **TASK-CL-002:** add static host-runtime preflight and exit classification before warm work in `src/commands/serve.rs`. (`AC-CL-002`)
3. **TASK-CL-003:** render bounded restart policy and extend manager observations in `src/service/os_register.rs`, propagating status through `src/service/mod.rs` and `src/commands/service.rs`. (`AC-CL-003`, `AC-CL-004`)
4. **TASK-CL-004:** restore standard version flags and tests in `src/main.rs`; extend installer/smoke assertions. (`AC-CL-005`)
5. **TASK-CL-005:** update affected canonical architecture, process, design, host-profile/as-built documentation without claiming live remediation. (`AC-CL-006`, `AC-CL-007`)
6. **TASK-CL-006:** run self-check, then obtain fresh independent assurance/adversarial review. (`AC-CL-001`–`AC-CL-007`)

## Validation contract

```bash
cargo build --locked
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
bash -n scripts/install.sh scripts/smoke-user-service.sh
shellcheck scripts/install.sh scripts/smoke-user-service.sh
bash scripts/smoke-user-service.sh --sandbox
git diff --check
```

Also run process-level help/version checks and offline verification of the sandbox-rendered unit. Record the Rust files changed only by mechanical repository formatting so they can be reviewed separately from lifecycle behavior.

## 2026-09-12 remediation and self-check

- Review-blocking identity, classification, timeout-cleanup, startup-order, malformed-manager, early fake-manager, replayability, evidence-package, and stale-report findings were addressed in the dirty worktree.
- OpenCode preflight now requires a native Ubuntu x86_64 executable plus bounded bare-semantic-version and OpenCode-specific help behavior; permanent configuration/behavior failures and retryable operational failures are typed separately. This is behavioral impostor resistance rather than cryptographic package provenance. Linux cleanup kills and confirms the probe process group after success, nonzero, timeout, and wait failures, with forking-descendant regression tests.
- Production startup now uses the deterministic sequence covered by tests: task reconciliation, job reconciliation, classified host preflight, warm services, voice sidecars, brain readiness, and bridge readiness. Permanent preflight failure stops after reconciliation with exit 78; transient failure stops at the same point with exit 1.
- Successful manager queries now validate all requested properties. Missing, duplicate, malformed, unknown, textual `ExecMainCode`, and out-of-range status values remain unknown.
- The sandbox activates deny-by-default fake managers before installer code or binaries run and records complete command results in fresh, checksummed, owner-read-only run-specific packages. It retains the exact rendered unit and a fixture rewritten only to a retained copy of the tested binary, then verifies that fixture after deleting the sandbox. Local checksums detect stale mixing and accidental changes, not adversarial owner rewriting; review must retain the candidate package digest externally.
- Passing and failed historical packages remain under `target/smoke-user-service/runs/`; no historical run is implicitly “latest.” Each re-review must name one source-identical candidate whose manifest binds `feature/autonomy`, HEAD/base `825b8afe7aed14a44efda0c58fc74e65b8fb02b6`, dirty-diff and source-input hashes, timestamps, environment assumptions, and command records.
- Self-check result before each review candidate must include formatting, locked build/tests, strict Clippy, shell syntax, ShellCheck, source installer, all version forms, fake-manager lifecycle, malformed-output handling, pre/post-cleanup systemd parsing, and `git diff --check`. Cargo was locked but not network- or cache-hermetic, as the evidence manifest states.
- This is developer self-check only. Fresh independent assurance and adversarial review remain required. The installed legacy unit remains unchanged and outside this approval scope.

## Independent planning record

Independent producer, architect, and CLI operator-flow analyses were obtained before implementation. They agreed that root-cause discovery, preflight ordering, permanent-failure restart suppression, a generic rate-limit fuse, status diagnostics, version flags, tests, and documentation are P0. This plan deliberately defers broader degraded-bridge semantics, automatic retry/reset commands, arbitrary service environment files, credentials, and live rollout.
