# LOCAL-SMITH-NEXT-SETUP-WIZARD

## Intent and bounded implementation

User requests a substantially simpler Matrix-dark setup wizard: **Core → LLM → Services**, with early speech capture, local-model selection, login startup, and consideration of a unified Compose runtime. This record reconciles independent architecture and producer/designer analyses for the first source slice. Base checkpoint `04b72f5`; uncommitted worktree, existing branch unchanged. No commits, publication, installed binary replacement, host unit/container changes, provider calls/downloads or graphical launch.

**Implemented baseline:** native Rust core, private verified Unix bridge, separately consented install/register/start/debug completion, Flutter loader/setup/home. Operator reports normal-path installation and reopen-to-Home working; this is operator-reported evidence, not independently captured host integration. The debug setup flag does not certify LLM/services setup.

**First implementation scope:** presentation-only wizard and theme; preserve existing lifecycle APIs and routing. Native runtime stays unchanged. Planned provider pages are navigable and clearly unavailable, with no fake selection, completion or provisioning. Existing Core completion may return to Core Home; Home exposes setup navigation for viewing remaining stages. No automatic step completion or mutation on navigation.

## Reconciliation and traceability

Independent architecture analysis: `eb29c10c-0ad4-452b-9f39-de30a6b347d2`. Independent producer/designer analysis: `0c176864-8ade-4fd7-8b3d-b148462f1c70`.

| Requirement | Task | Acceptance/evidence |
|---|---|---|
| R1 three clear stages | T1 wizard/navigation | A1 Core/LLM/Services shown, planned stages explicit, no fake readiness |
| R2 minimal factual Core flow | T2 concise primary action and progressive details | A2 opening/refresh/navigation inspection-only; existing prepare/confirm/apply unchanged; decline/no duplicate effects |
| R3 green/black dark theme | T3 global theme, compact cards | A3 readable neutral text, labeled states, focus, narrow/text-scale tests |
| R4 returning user/repair | T4 retain BootController routing | A4 completed Core returns Home; outages preserve Home repair; no full-onboarding claim |
| R5 first-class speech/search direction | T5 Services page with STT/TTS/search | A5 clear planned status, no unsupported toggles or provisioning |
| R6 authority/lifecycle preserved | T6 regression/assurance | A6 no stop/enable/download/share mutations, tests/build pass, fresh review |

Producer requires meaningful configuration rather than polish alone; designer requires honest unsupported states. Resolved by a bounded UI-first milestone, not claiming complete product setup. Subsequent real adapter slices remain required.

## Deferred target decisions (not implemented)

- **Automatic setup:** read-only inspection and attaching to verified existing state are automatic. New install/register/start/enable/download effects retain explicit consent. A batched setup action or persisted standing permission needs its own effect-bound approval contract, not reuse of a 60-second action token. Only prompt for effects actually needed. Exact effects must be accessible before confirming.
- **Login startup:** offer an explicit option in a later lifecycle slice; distinguish enabled-at-login from running and from pre-login availability. No lingering or implicit desktop launch. App search integration already exists in the artifact installer.
- **LLM:** local Ollama first; read-only exact installed-tag inventory, explicit selection/persistence, separately consented download/test. Cloud later. Validate actual provider inference separately from inventory. Containerized AMD GPU support needs explicit devices/driver validation, no silent CPU substitution.
- **Services:** STT, TTS, search are first-class setup requirements. Whether all must be healthy before text chat remains a product decision; this UI slice does not silently impose or waive that gate. Speech capture uses shared editable text draft, review-before-send; future trusted-speech submission is opt-in and not tool authority.
- **Compose:** prefer one distribution/UX with multiple isolated services, not one privilege domain. Native core + managed provider containers is the least disruptive candidate; a containerized core requires a distinct identity/supervision migration. Never mount Docker socket, broad home, host D-Bus or all `/tmp` into model-facing services. Explicit per-project mounts and access mode; separate model storage; constrained processes/resources; minimal ports; deliberate egress. Search is networked even when self-hosted. Provisioning internet access differs from steady-state inference. Rootless/rootful support and GPU compatibility need evidence.
- Existing sidecar path-based audio APIs require bounded upload/download or a narrow spool; do not solve cross-container paths by broad host mounts.
- Current artifact installer blocks differing service binaries. An approved upgrade plan is required before deploying changed service behavior; do not replace the running installation as a test shortcut.

These directions supersede no executable authority rules. Canonical policy remains unchanged except presentation documentation for this slice.

## Validation and approval

Run client tests (format/analyzer/widget), aggregate `make test` and `make build` through existing Dagger pipeline. Add narrow/large-text, planned-step/navigation/no-mutation, concise confirmation and returning Home tests. Obtain independent assurance/adversarial review. No real app launch, model/image downloads outside developer build inputs, Compose execution, normal-home installation, unit mutation or remote action. Retain command results and scope limits in `next/VALIDATION.md`. Local source changes are not deployed until independently reviewed and separately approved.
