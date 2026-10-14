# LOCAL-SMITH-NEXT-INSTALLER-UI

## Bounded intent / authority

User requests a very rudimentary visual installer that consolidates choices into a standard Install → Complete flow. Implement source and isolated validation; operator performs live functional QA. Existing `feature/autonomy`, base `04b72f5f5093d66294cb7e525dac0f4312dd0cbc`, uncommitted native/text work preserved. No commits, installed-client/service/state changes, real manager/provider/model operations, normal graphical-session launch, upgrades, OS/Ollama/GPU provisioning, autorun/linger, cloud/voice, or remote effects. Prior host-upgrade approval is consumed. The native migration's canceled final integrated assurance is not a PASS.

Independent analyses: architect `cc13f1a1-3834-4e84-9c6a-5dd7e8b551f4`, producer `125e682d-e1d7-4b94-bc42-1ce2cf886e6b`, designer `09777565-4914-40f2-93ce-156309ca46a1`. Elevated approval/concurrency/operator-flow work. One scoped independent source review after self-check; live graphical and installed-host QA remains operator-owned.

## Reconciliation

- One native GTK3/Rust window: review/choices → installing → complete or stopped. No app onboarding, provider dashboard or multistage Core/LLM/Services wizard.
- Fixed complete paired desktop/service artifacts, existing menu/icon/command integration. Optional Register service and Start now, both off initially; Start requires registration selected. Unselected work does not stop an existing service.
- Artifact-only is a valid complete recipe. Registration and Start are optional verified recipes, never unconditional full CLI orchestration/model inventory.
- Installation complete means selected verified installation effects, not chat/inference readiness. Model selection remains available in the existing app; no GUI model/provider/download/inference work in this first slice.
- GTK3 reuses the Flutter recipient runtime family. Optional `gui` Cargo feature and sibling `smith-installer-ui` ELF leave `smith-installer` headless contracts and minimal no-Python/no-display gate unchanged. Binding dependency is an explicit legacy GTK3 choice, not cross-platform support or universal ABI.
- **Preserved disagreement:** a literal prompt-free single-click batch authorization would conflict with current separate effect consent. This slice consolidates presentation only. Install approves the exact displayed artifact review. Selected registration/reload and Start use fresh inline effect-specific confirmations with original Core token/expiry. No callback that returns blanket yes, no silent reprepare/retry.

## Shared boundary / write sets

Publisher owner: `src/artifact.rs` and publisher tests only. Add opaque prepared-artifact review/publication API; do not alter filesystem/manifest checks or CLI semantics. Proposed `PreparedArtifacts` exposes `description()` and `review_id()`; private captured plan/trace stay publisher-owned. `Installer::prepare_review()` is read-only; `install_reviewed(prepared, approved_review_id, output)` reuses exactly the existing post-consent/under-lock/final-verification/no-clobber/retained-selector machinery and reports partial effects. Original CLI remains a wrapper over the same implementation.

Wizard engine owner: new `src/wizard.rs`/tests, `src/consent.rs` cancellation request, `src/lib.rs` module export; only minimal crate-visible `setup::usable` visibility if required. Own the publisher/Core objects inside one worker; expose bounded typed commands/events with unique pending-request IDs, worker-owned original Core token, choices frozen at Install and an explicit terminal report. No stdout scraping, process-wrapped CLI or duplicated filesystem rules. Create runtime roots/Controller only after successful artifact acceptance and only for requested lifecycle work. No model/provider factory.

GUI owner: new `src/ui.rs` only, native GTK3 shell using wizard engine. Parent owns Cargo feature/bin/dependency/lock and build/package/Make/docs. No client/service executable source/test changes.

Engine contract for GUI coordination: `Choices { register_service:bool, start_service:bool }`; `Session::spawn(bundle)` starts read-only review worker; `Session::send(Command)` is nonblocking/bounded; `try_event()`/`is_finished()`/`join()` retain ownership. Commands: Install{request_id,choices}, Approve{request_id}, Decline{request_id}, RefreshReview, Cancel. Events: Review{request_id,description}, Stage{message}, Confirmation{request_id,title,effects}, Finished{report}. Artifact/UI review request IDs are session-unique; Core tokens never leave worker. Any protocol/schema adjustments must be coordinated before the shell implements them.

## Concurrency / cancellation

GTK stays on the main thread. Exactly one owned worker constructs its own publisher/Controller; `Rc` publisher is never moved across threads. Bounded channels/capped owned reporting prevent unbounded queues and blocking progress delivery. Closing/canceling pending consent invalidates it. `consent::request_cancel()` invokes the composed installer/Core cancellation path, not only a GUI-local flag. Cancellation is terminal for that process; do not clear production flags to retry. Keep the responsive window/event loop alive in Stopping until worker returns and is joined; never detach an effect-capable worker. Filesystem/fsync may not be interruptible; accepted manager/service effects remain and are not rolled back. A normal close during work confirms stopping, with Keep installing as default.

## Traceability / gates

| Requirement | Implementation | Required isolated evidence |
|---|---|---|
| UI1 conventional minimal window | GTK shell and native UI export | GUI-enabled locked build/Clippy, headless GTK widget smoke, operator QA instructions |
| UI2 no effects before Install | opaque artifact review and engine | private-root snapshots; no manager/provider calls on review/close/decline |
| UI3 choices bound to selected effects | artifact/register/start recipes | default artifact-only, register-only, start dependency, no model/provider/launch tests |
| UI4 exact separate approvals | typed pending IDs and original token | changed review/stale or wrong request/decline/expiry tests; no automatic reprepare |
| UI5 owned responsive worker/cancel | bounded channels and composed cancellation | duplicate-command/backpressure/channel-loss/close-cancel tests; worker joined |
| UI6 truthful complete/partial result | structured terminal report | accepted/completed/skipped/uncertain failure tests; no inferred chat/inference readiness |
| UI7 preserve native/package boundaries | CLI/GUI sibling ELF, Dagger metadata | original CLI/publisher regressions, package allowlist/ELF/modes/pairing, no-Python CLI gate |

Run native baseline and GUI-enabled formatting/warning-denied Clippy/locked tests/build; Dagger installer/package/GUI gates and current aggregate build/tests. GTK tests use isolated Xvfb and injected sessions, not normal display/host managers/providers. Add GUI ELF explicitly to the package allowlist/manifest/checksums without weakening reviewed text-payload exclusions. Bind current source/artifacts and scoped independent review in validation history. Report manual graphical/live installation/model/inference/clean-host/provenance/MSRV checks not performed; no release/deployment approval implied.
