# LOCAL-SMITH-FLUTTER-BOOT-P0

## Approved intent and authority

The operator approved Flutter-first boot/readiness plus explicit backend start as the MVP focus. Flutter is Smith's primary interface; CLI is secondary query/status/configuration and existing commands remain compatible. Future headless operation is a separate deployment/lifecycle capability, not synonymous with CLI. Ubuntu 24.04 LTS x86_64 only.

Authority: local scoped implementation, cached offline builds/tests, and fake/inert lifecycle fixtures. No live installed applications, actual model/provider work, managers, installs/downloads, normal-path deployment, commits, pushes, tags, publication, or accepted risk. Preserve all existing dirty work on feature/autonomy. This supersedes the earlier slice's read-only client lifecycle restriction only for the bounded actions below; its chat-only isolation and release blockers remain.

## Independent analyses and reconciliation

Producer, architect, and designer independently inspected policy and current code. Agreed: truthful boot states, positive profile/provider/model readiness, explicit Start, preserved conversation/draft, no existing-listener interference, bounded ownership/cleanup, no fake progress. Architect identifies success-on-existing-listener and health alone as insufficient ownership; designer requires persistent Home and external-process protection; producer requires unknown availability to block sending.

Resolution: add an opt-in chat-only desktop session identifier, refuse occupied-port adoption in that mode, and watch the launched parent's stdin pipe for EOF. Health exposes the per-launch identifier. Flutter retains the exact Process handle, keeps stdin open, and uses the identifier plus child liveness to correlate readiness; no HTTP shutdown or port/PID-based kill. This is correlation for one trusted operator, not authentication against hostile same-UID processes. Parent pipe EOF initiates shutdown; bounded client stop escalates TERM/KILL only on its captured child. No claim of crash-proof instantaneous termination or provider-work cancellation.

Automatic checks may attach to a strictly compatible externally managed listener; do not send/reset any conversation during attachment. Exact external model is displayed; no substitution for an explicitly configured model. Starts only via explicit button. Manual absolute executable and exact local-model fields are sufficient; no broad scanning or PATH/shell fallback. Packaged sibling discovery is optional only with a clearly documented paired-root contract. Backend file checks are behavioral/local safety, not artifact authenticity.

## Traceability

| ID | Requirement | Implementation / evidence |
|---|---|---|
| BOOT-1 | Flutter primary Launch -> factual Boot/Setup -> Home/Chat, CLI secondary; headless deferred. | App shell/widgets; canonical docs. |
| BOOT-2 | Positive Smith chat-only/non-echo identity, Ollama provider, exact model and affirmative availability; unknown/mismatch blocks send. | Readiness matrix/client tests. |
| BOOT-3 | Explicit immutable start arguments from validated absolute native executable; no shell, PATH, install/pull/provider or manager calls. | Injectable launcher/path tests, exact argv. |
| BOOT-4 | Owned desktop-session correlation, occupied-port rejection, parent EOF, bounded startup/stop, duplicate/stale-event guards, external never stopped. | Rust inert router/parent tests and Flutter fake lifecycle tests. |
| BOOT-5 | Pulse/recovery never remounts Home or clears draft/transcript; no resend or hidden reset. | Widget navigation/disconnect tests. |
| BOOT-6 | Accessible factual states, reduced motion, no fabricated percentages or artificial delay. | Widget tests; manual desktop gate remains pending. |
| BOOT-7 | Source-bound offline checks, independent assurance, honest candidate status. | Retained logs/review; release and real-host gates remain pending. |

## Write scopes and method

- Backend: src/main.rs and src/commands/offline_chat.rs only; desktop launch option and tests, preserve legacy serve/service behavior.
- Client: client/lib and client/test only; lifecycle adapters/controller, boot shell, strict readiness, persistent conversation.
- Coordinator: affected canonical docs, README/release guide, this record and retained validation evidence.

Dependency/model provisioning, Ollama startup, user-manager integration, headless implementation, durable history, voice/tools/Work, and release-workflow changes are deferred. The supplied icon cannot be integrated until its original asset is available in the workspace; do not invent it or read arbitrary host paths looking for it.

## Implementation and independent review

Implemented the backend desktop-session/parent-pipe contract and Flutter boot coordinator/shell with manual configuration, explicit Start, positive readiness, captured-child cleanup, and stable Home. Initial self-check reported 110 Rust and 98 Flutter tests. Fresh assurance found two blockers: checks could restore readiness during shutdown, and the startup timeout omitted asynchronous preflight. Remediation gates checks/response application during cleanup, invalidates observations on entry/completion, and shares the 15-second deadline across resolver/probe/launch/readiness. Fake regressions were added; subsequent self-check reported 108 Flutter tests. Independent focused static re-review passed the remediated local source; it does not verify real desktop/process integration or authorize release.

Final coordinator validation is retained under `target/flutter-boot-validation/20261006T025455Z/`, including source-before/after inventories, exact commands/results and raw logs. Inspect those results rather than treating this planned location or agent-reported counts as a pass. Earlier full-suite noisy-probe failures remain historical; later passes do not establish a resolved root cause. This work changes source identity and supersedes earlier chat-only build identity; no published artifact is implied.

## Validation and stop conditions

Flutter cached locked dependency resolution, analyzer, unit/widget tests and Linux release build; Rust fmt/locked offline build/tests/strict Clippy for changed backend; script syntax/ShellCheck and packaging smoke when relevant. Fakes and temporary local test listeners only, no live services/models/apps. Independent fresh assurance/adversarial review required. Existing unrelated full-suite OpenCode noisy-probe failure remains reported and must not be hidden or counted as a pass. Real desktop/process integration, denied-egress contextual inference, clean-host install, source reconstruction/license/SBOM and publication workflow remain separate no-ship gates.
