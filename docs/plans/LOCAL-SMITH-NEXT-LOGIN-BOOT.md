# LOCAL-SMITH-NEXT-LOGIN-BOOT

## Intent and authority

Explicit user approval, 2026-10-13: make the exact owned `smith-next.service` available after workstation restart and user login through persistent user-manager enablement, while keeping lingering disabled/unchanged and `Restart=no`. Add a minimal normal Smith Loader/Boot state—not repair branding—with one restrained vector animation and a few-word status while required dependencies become ready. If the exact enabled owned service remains safely stopped after bounded observation, offer separately confirmed **Start Smith**. Local source/tests/build only; no live manager/login/reboot/install/model effects, commits or remote mutation.

## Reconciled requirements

1. Current root cause: the unit is static and Start-only. Add deterministic `[Install]\nWantedBy=default.target`, retaining `Type=simple`, `Restart=no`, `UMask=0077`, bounded stop, unprivileged user service and exact paths.
2. Enabled-at-login, running-now, bridge-ready and lingering are independent facts. No pre-login guarantee; with linger unchanged, expected availability begins after user login.
3. Add strict control schema 2 with independent enablement facts and exact deterministic default-target link verification. Reject runtime-only, linked, alias, masked, foreign, malformed or unknown states.
4. Add separate expiring/single-use `Enable` action and fixed manager `enable smith-next.service`, never `--now`; Enable must not Start. Start must not Enable and requires exact current enabled installation.
5. Fresh install recipe is artifacts → register/reload → separately confirm Enable → separately confirm Start → fresh Core/app readiness. Decline/failure is partial/incomplete, never rollback.
6. Maintenance recognizes only byte-exact legacy static and current installable units. Upgrade migrates static to current, preserves current enabled state without churn, and explicitly enables current disabled state. Uninstall stops/quiesces, disables and verifies before preserving/removing unit and active integration. Data/models/releases/backups remain.
7. App Boot uses finite serialized read-only observations for normal login-start races, then synchronizes conversation. It performs no automatic Start, enable, install, model action, inference or resend.
8. Boot surface: centered single CustomPainter vector loop, short status (`Starting Smith`, `Connecting to Smith`, `Syncing conversation`), reduced-motion static equivalent, accessible live status, no sidebar/chat controls during Boot.
9. Only a fresh exact installed+enabled safely stopped state exposes **Start Smith**. Start uses existing prepare → visible exact confirmation → apply original token → fresh bounded readiness observation. Conflicts/unknown/disabled/static/missing never expose Start.
10. Optional future dependencies may reuse typed Boot states, but optional voice/pairing must not block text chat and action-required dependencies stop animation rather than waiting forever.
11. Preserve cancellation/helper cleanup, no unmanaged fallback, no stale retry/reprepare, query-only recovery, history/draft/provisional answers, model catalog, network/filesystem boundaries and installer minimalism.

## Implementation ownership

- Service lifecycle/schema/maintenance: `next/service/src/{filesystem,manager,control,maintenance,main}.rs` and service tests.
- Installer orchestration/UI/maintenance: `next/installer/src/{setup,wizard,ui,maintenance}.rs` and installer tests.
- Client Boot: `next/client/lib/{control_api,native_control,main}.dart`, a focused boot component if useful, and client tests.
- Canonical docs/evidence: parent coordination.

## Validation

Focused locked Rust fmt/Clippy/tests; manager denylist and exact call ordering; legacy-static/current-enabled migration; disable-before-unregister preservation; strict schema-2 Rust↔Dart fixture; Flutter fake-clock Loader/Start/conflict/reduced-motion/360×800@2× tests; existing history/no-resend tests; GTK installer lifecycle confirmation tests. Then `make test build` and full installer package build, fresh architecture/adversarial assurance. Real user-manager enable/start, logout/login, reboot and uninstall require a new exact host approval and are not part of source validation.
