# LOCAL-SMITH-NEXT-MINIMAL-INSTALLER

## Intent and authority

Explicit user task, 2026-10-13: radically simplify the native GTK installer. Initial view exposes only large **INSTALL / UPGRADE** and **UNINSTALL** choices. Subsequent screens show one current action with a short title/explanation. Routine artifact, lifecycle and progress diagnostics move behind collapsed Logs. Smith installer model choices are restricted to the compiled curated catalog; unsupported saved selections remain preserved but inert. Local source/tests/build only on `feature/autonomy`; no host install, upgrade, uninstall, service/model action, weights, commit or remote mutation.

## Reconciled contract

1. Opening is read-only and visually presents exactly two operation choices. The existing bounded artifact preview may complete in the background, but it authorizes no effect.
2. INSTALL / UPGRADE routes to one focused setup screen based on available review facts. It must not combine install and upgrade authorization or bypass maintenance ownership checks.
3. Normal install defaults remain register service + start now. Their existing separate exact confirmations remain; login/linger remains unchanged. Remove these choices and generated recipe from the opening screen.
4. Manage models is not a third landing action. It remains reachable from the setup/installed path and appropriate result screens without weakening service compatibility or artifact-independence checks.
5. Routine details, paths, stage history and successful low-level steps are hidden by default under bounded selectable Logs. Exact effects before consent, blocking reason, failure/uncertainty, preservation state and safe next action remain directly visible.
6. One primary and at most one secondary current-action control is visible in the fixed footer. No row of unrelated buttons or below-the-fold action list.
7. The selectable model list equals `smith_service::runner::CATALOG`. Unknown saved selections are shown only as unsupported read-only status, never appended to the chooser or submitted. No free-form model/path/import capability.
8. Preserve request IDs, expiry, separate approvals, critical event ordering, numeric progress coalescing, cancellation/join, sticky reopen guard, no retry/resume/rollback/cleanup and scope-specific verified completion.

## Implementation and evidence

Primary implementation is `next/installer/src/ui.rs` and embedded GTK tests. Change `wizard.rs` only if a typed presentation seam is required; no maintenance, service, artifact filesystem, network or catalog-authority changes. Update canonical Architecture/Design/Process and `next/VALIDATION.md`.

Required tests: exactly two landing actions; no opening effect command; lifecycle/artifact/model controls hidden on landing; one-operation visibility; install/upgrade/uninstall/model routing; exact confirmations remain visible; failure/uncertainty visible without Logs; Logs bounded/collapsed and report-message-first; unsupported saved model inert; only catalog entries submitted; narrow/minimum-size GTK flow; existing cancellation/progress/maintenance regressions. Run locked installer formatting/Clippy/tests, isolated Xvfb, package build, then fresh independent assurance. Source evidence does not prove live user-manager, model transfer/inference or installed visual QA.
