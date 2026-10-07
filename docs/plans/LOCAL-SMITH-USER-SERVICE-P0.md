# LOCAL-SMITH-USER-SERVICE-P0

## Objective

Harden Smith for its supported Ubuntu LTS x86_64 workstation so the Rust supervisor can be installed to a stable unprivileged path, managed truthfully as a `systemd --user` service, recovered safely after restart, and exercised through non-destructive sandbox smoke QA. Keep the Flutter Halo a separate observational Linux desktop application.

## Work record

- Branch: `feature/autonomy` (approved bootstrap exception)
- Base revision: `825b8afe7aed14a44efda0c58fc74e65b8fb02b6`
- Risk class: **Critical** — systemd lifecycle, filesystem roots, persistence/recovery, and process state
- Tracking mode: approved local plan; Jira integration is not required
- Independent planning: producer, architect, designer, and planner analyses completed before implementation

## Dirty worktree declaration

The worktree already contains an uncommitted Flutter Halo redesign in `client/`, new client tests, and `docs/DESIGN.md`. Those changes are in scope for compatible observational-startup completion and validation, but must not be reset, stashed, broadly reformatted, or overwritten. Service hardening must remain surgically separate from unrelated client work.

## Requirements

- **REQ-001 Stable installation:** install an Ubuntu x86_64 Smith binary to an explicit stable user-owned root without registering or starting services, sidecars, Docker, or lingering.
- **REQ-002 Separated lifecycle:** expose distinct unit install, start, stop, restart, enable-at-login, disable-at-login, status, and explicit uninstall operations. No operation may silently combine runtime and enablement.
- **REQ-003 Truthful state:** report unit registration/load, running state/substate, enabled-at-login, linger state, and Smith bridge identity/health separately.
- **REQ-004 Safe service definition:** render a deterministic hardened user unit around a canonical absolute executable that is executable and not a transient build/development path. Respect `XDG_CONFIG_HOME`.
- **REQ-005 No authority expansion:** never mutate lingering, never register a privileged system service, never fall back to an unmanaged detached process, and never claim pre-login boot when `Linger=no` or unknown.
- **REQ-006 Restart recovery:** before bridge readiness, idempotently mark persisted queued/running tasks and jobs as interrupted/recovery-required while preserving identifiers and data; do not alter awaiting-signoff or terminal records.
- **REQ-007 Smith identity:** distinguish a healthy Smith bridge from an unrelated listener on the configured loopback port; fail when a non-Smith process owns the port.
- **REQ-008 Observational desktop launch:** initial Flutter startup and local session reset perform reads/local state changes only. Chat requests, voice sidecar startup, recording, and settings mutations require explicit operator actions.
- **REQ-009 Separate desktop artifact:** keep Halo independently packaged and launched; do not embed it in the supervisor or make it an installer/service dependency.
- **REQ-010 Reproducible QA and docs:** provide fake-manager sandbox smoke coverage and update canonical documentation to distinguish implemented, target, and observed states.

## Acceptance criteria

- **AC-001:** `scripts/install.sh` rejects unsupported hosts, uses `cargo install --locked --path ... --root ...`, verifies the resulting binary and `smith --help`, and has no service, Docker, sidecar, or linger side effects. (`REQ-001`, `REQ-005`)
- **AC-002:** unit rendering and path tests prove stable absolute executable validation, safe quoting, `XDG_CONFIG_HOME` support, deterministic hardened directives, and rejection of transient/debug/release/temp paths. (`REQ-004`)
- **AC-003:** CLI/service tests and sandbox transcript prove install only stages/reloads; start/stop/restart only affect runtime; enable/disable only affect login policy; no `enable --now`; uninstall is explicit. (`REQ-002`, `REQ-005`)
- **AC-004:** status output independently exposes registration/load, active/substate, enabled-at-login, linger yes/no/unknown, and bridge identity/health without mutating the host. (`REQ-003`, `REQ-007`)
- **AC-005:** command failures include command context, exit status, and sanitized stderr; registration failure cannot spawn an unmanaged process or produce an unconditional readiness claim. (`REQ-002`, `REQ-005`)
- **AC-006:** restart-reconciliation tests show queued/running task and job records become interrupted exactly once with actionable recovery text; all other states remain unchanged. (`REQ-006`)
- **AC-007:** serve tests or deterministic helper tests show valid existing Smith identity returns safely, while a non-Smith port occupant fails; reconciliation occurs only for the new supervisor before readiness. (`REQ-006`, `REQ-007`)
- **AC-008:** Flutter tests prove initial startup produces bridge reads only, session reset sends no hidden prompt, and voice sidecars are requested only after explicit microphone action while an online bridge permits that attempt. (`REQ-008`)
- **AC-009:** Linux client metadata has a stable Halo identity and normal single-instance behavior, with no supervisor bundling or installer coupling. (`REQ-009`)
- **AC-010:** the sandbox smoke harness uses temporary HOME/XDG/install/config roots and fake `systemctl`/`loginctl`, records commands, asserts lifecycle separation/no lingering mutation/no unmanaged spawn, and never contacts the live user manager. (`REQ-010`)
- **AC-011:** required canonical docs and READMEs distinguish installed, running now, enabled at login, optional manual lingering/pre-login behavior, sandbox smoke evidence, later approved host smoke, restart recovery, and the separate client. (`REQ-003`, `REQ-006`, `REQ-009`, `REQ-010`)

All acceptance criteria are blocking for this P0 work record. Missing or unverifiable evidence is a failure, not a deferral.

## Scope exclusions

- No support, packaging, or validation for non-Ubuntu or non-x86_64 hosts.
- No privileged/system-wide service, multi-user, multi-tenant, LAN/public listener, or new port behavior.
- No automatic `loginctl enable-linger` or `disable-linger`; pre-login boot remains an operator-approved manual host decision.
- No stronger systemd filesystem/network sandboxing until Docker and optional voice dependencies are compatibility-tested.
- No bundling, auto-launching, or service dependency between the Flutter client and Rust supervisor.
- No pixel-perfect/golden visual claim without a checked-in immutable reference or later manual approval.
- No unrelated cleanup, broad formatting, dependency upgrades, remote mutation, commit, push, PR, merge, release, or deployment.

## No-live-host-mutation boundary

This implementation turn authorizes local repository edits, builds, tests, and sandboxed smoke execution only. It does **not** authorize installing to normal user/system paths, invoking the live `systemctl --user` manager for mutation, starting/stopping/restarting a live Smith service, logging out, rebooting, changing lingering, removing live registrations, or destructive cleanup. Any later real-host smoke requires exact human approval bound to the commands and target account.

## Implementation sequence and traceability

1. **TASK-001:** refactor unit rendering, path validation, checked manager commands, lifecycle separation, and observed status. (`REQ-002`–`REQ-005`; `AC-002`–`AC-005`)
2. **TASK-002:** update CLI/service configuration/install semantics and truthful output. (`REQ-001`–`REQ-005`; `AC-001`, `AC-003`–`AC-005`)
3. **TASK-003:** add persisted task/job interrupted states and pre-readiness reconciliation; harden bridge identity handling. (`REQ-006`, `REQ-007`; `AC-006`, `AC-007`)
4. **TASK-004:** remove implicit Flutter startup mutations and stabilize separate Linux Halo packaging. (`REQ-008`, `REQ-009`; `AC-008`, `AC-009`)
5. **TASK-005:** replace installer, add isolated fake-manager smoke harness, and update required documentation. (`REQ-001`, `REQ-010`; `AC-001`, `AC-010`, `AC-011`)
6. **TASK-006:** run the complete self-check, preserve evidence, and obtain fresh independent assurance/adversarial review. (all requirements and criteria)

## Validation contract

Run from the repository root unless noted:

```bash
cargo fmt --check
cargo build --locked
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings

bash -n scripts/install.sh scripts/smoke-user-service.sh
bash scripts/smoke-user-service.sh --sandbox

cd client
flutter pub get
flutter analyze
flutter test
flutter build linux

cd ..
git diff --check
```

Required evidence includes command, exit status, relevant output, tested revision/worktree identity, sandbox fake-manager transcript assertions, developer self-check, and fresh independent assurance/adversarial findings. A failed Rust build/test/Clippy check, failed Flutter analyze/test/build, failed sandbox invariant, unresolved critical warning, or missing required evidence is blocking.

## Developer self-check evidence — 2026-09-05

- Base/HEAD during validation: `825b8afe7aed14a44efda0c58fc74e65b8fb02b6`; implementation remains an uncommitted dirty worktree.
- `cargo build --locked`: pass.
- `cargo test --locked`: pass, 77 tests.
- `cargo clippy --locked --all-targets -- -D warnings`: pass.
- `bash -n scripts/install.sh scripts/smoke-user-service.sh`: pass.
- `shellcheck scripts/install.sh scripts/smoke-user-service.sh`: pass.
- `bash scripts/smoke-user-service.sh --sandbox`: pass using the binary-only installer, a temporary run beneath the stable per-user QA root, fake managers, suppressed ambient probes, deterministic negative installer cases, and retained evidence at `target/smoke-user-service/{transcript.log,manager.log}`.
- `flutter pub get`: pass; newer incompatible package versions were informational only and no dependency update was made.
- `flutter analyze`: pass.
- `flutter test`: pass, 32 tests.
- `flutter build linux`: pass; output executable `build/linux/x64/release/bundle/smith_halo`.
- `desktop-file-validate` for source and bundled `dev.smith.Halo.desktop`: pass.
- `git diff --check`: pass.
- Scoped `rustfmt --check` over changed Rust files passes except `src/commands/chat.rs`, whose broader formatting drift predates this work and was not rewritten for its single required enum match arm. Repository-wide `cargo fmt --check` remains blocked by inherited formatting drift in multiple untouched files; no broad unrelated formatting change was made.
- The previously questioned fake-manager observation test passed five consecutive focused executions after manager observations were dependency-injected; full Rust tests subsequently passed again.
- No normal-path install, live user-manager mutation, service runtime action, login/logout, reboot, lingering mutation, commit, or remote effect was performed.

A generated file/digest manifest for the final dirty worktree and retained command artifacts is stored at `target/smoke-user-service/validation-manifest.txt`. Independent review findings remain recommendations only and cannot authorize live effects.

## Rollback and compatibility

All implementation changes remain uncommitted local work until separately approved. Rollback is file-scoped review/reversion of this plan’s edits only; existing dirty Flutter work must be preserved. Service-file format changes require an explicit later `service install` and approved live-host smoke; no migration may silently alter runtime, login enablement, or lingering.
