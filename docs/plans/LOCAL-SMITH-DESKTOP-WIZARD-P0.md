# LOCAL-SMITH-DESKTOP-WIZARD-P0

## Approved intent / bounded authority

Operator approved implementing Flutter-first guided boot/setup/install/resolve using structured Smith background commands. Local worktree implementation, cached offline builds/tests, inert local HTTP/process fixtures and candidate assembly are permitted. No live model pulls, provider/service changes, normal-path installs, privileged actions, real app launches, commits/pushes/publication or accepted-risk decisions. Preserve all dirty work on feature/autonomy. Ubuntu 24.04 LTS x86_64 only.

## Independent analyses and reconciliation

Independent producer/architect/designer agree on deterministic wizard, namespaced structured commands, read-only inspection, installed-model selection, explicit model-download confirmation, immutable chat-only launch, preserved Home state, combined desktop backend discovery. Missing Ollama runtime must be clearly guided: no audited pinned runtime artifact/install contract exists, so automatic OS installation/start is deferred. This is a partial first-run setup, not turnkey empty-host installation. Install in this increment means operator-confirmed model pull through already-running local Ollama, not bare legacy smith install.

Scope narrowly supersedes previous provisioning exclusion for confirmed model pull only. Chat-only serve has no provisioning routes/tools. Ollama may independently use internet and write provider-managed storage; the request to its fixed loopback API does not confine daemon egress. Tag/size/digest metadata is provider-reported, not artifact authenticity; transfer/storage sizes are unknown until reported. Cancellation stops local observation/request but may not stop daemon work or remove partial data. No automatic retries, rollback/cache deletion, provider start, cloud fallback, drivers, manager mutations, persistent configuration, or headless implementation.

## Reconciled command contract (v1)

- `smith setup inspect`: exactly one stdout JSON object `{schema_version:1, command:"inspect", status:"ready"|"unavailable"|"error", provider:"ollama", endpoint:"http://127.0.0.1:11434", models:[{name,digest,size}], error:null|{code,message}}`. Inspect validates Ubuntu24.04/x86_64 and fixed-loopback `/api/tags`; positive ready means inventory observed, NOT inference verified. Lists only eligible exact local tags, rejects ambiguous/cloud metadata. No network beyond loopback, inference or mutation. stderr diagnostics bounded/sanitized. Unsupported platform returns error, not download/install.
- `smith setup pull --model <exact-name:tag>`: NDJSON stdout envelopes `{schema_version:1, command:"pull", operation_id, seq, event, ...}`. For valid requests the first event `approval_required` contains exact model and fixed endpoint; a fully validated sequence-zero terminal `error` may instead report preflight failure without consent (terminal-only success/cancelled is rejected). The approval request waits at most 60 seconds for one bounded stdin JSON line `{schema_version:1, operation_id, model, approved:true}`. Decline/EOF/mismatch/expiry performs no POST and emits one terminal event. Approval is trusted-local-operator consent correlation, not authentication. Progress events contain provider stage and per-layer `completed`/`total` when known; terminal `status:"success"|"error"|"cancelled"`, stable code/message. Exact terminal success requires provider success plus fresh eligible exact tag verification.
- Fixed Ollama HTTP client no proxy, no redirects, bounded tags 256KiB, progress/input frame 16KiB, two-second connect/probe; finite pull overall/idle deadline (15 minutes / 30 seconds). Input validation requires explicit local tag, rejects URLs/cloud/controls. No shell/Ollama CLI/legacy install helpers. Cancellation/EOF after approval drops local request and emits truthful terminal when possible; daemon outcome/cache may remain unknown. One active mutation per wizard, no polls spawning installs. Cross-process daemon scheduling is not claimed.

## Paired desktop contract

Halo payload includes backend at `smith_backend/smith`, alongside full Flutter lib/data. Discover ONLY from actual desktop executable directory, validated through existing absolute native executable checks before running setup command. No cwd/PATH/recursive scan. Invalid discovered candidate never silently selects another binary; explicit advanced path is allowed. JSON schema provides behavioral compatibility, not package authentication. Installer/assembly remains install-only and preserves full bundle, checksum/provenance; standalone backend candidate retained for secondary/headless-future compatibility, not primary product.

## Requirements / traceability

| ID | Requirement | Evidence |
|---|---|---|
| WIZ-1 | Detect backend -> local runtime -> installed/download model -> review/explicit Start -> Home. | widget/controller fixtures; stable Home/draft/no resend. |
| WIZ-2 | Structured bounded inspect/pull subprocess protocol; checks never mutate. | JSON/NDJSON schema/error/stdin/output/timeout tests; exact argv. |
| WIZ-3 | Action-specific approval/decline/expiry bound to exact tag/operation; no pull before consent. | fake transport mutation counters; changed selection/cancel tests. |
| WIZ-4 | Fixed-loopback no proxy/redirect, eligible local exact model; success reverified. | mock tags/pull negative fixtures/cloud/metadata tests. |
| WIZ-5 | Bounded progress/cancellation/process cleanup, unknown daemon/cache outcome truthful, no auto retry. | split/malformed/oversized/idle/early EOF/error streams; fake child tests. |
| WIZ-6 | Bundled backend discovery and private full-bundle install, advanced fallback explicit. | package mocks/checksums/paired layout + resolver tests. |
| WIZ-7 | Missing Ollama guided only, no OS install/start/managers; confirmed download internet/disk disclosure. | UI negative/confirmation tests; docs. |
| WIZ-8 | Source-bound validation, independent review; real integration/release gated. | retained commands/logs/identity and review. |

## Disjoint write scopes

1. Backend: src/main.rs, src/commands/mod.rs, new src/commands/setup.rs only (extract safe narrow provider functions only if necessary with coordinator consent). No legacy behavior changes.
2. Client: client/lib and client/test only, wizard adapter/controller and boot shell; existing desktop ownership controller remains authoritative.
3. Packaging: scripts/package-chat.sh, scripts/smoke-prebuilt.sh, scripts/install-prebuilt.sh only if needed for additive bundled backend; no live install or binary execution.
4. Coordinator: canonical docs, README/client README/release guide, plan/evidence and combined validation.

## Implementation and review status

Structured backend commands, guided Flutter wizard and additive paired desktop packaging are implemented. Self-check reported 127 Rust and, after error-presentation follow-up, 136 Flutter tests. Fresh independent static assurance and focused follow-up re-review passed with no blocking source finding. The follow-up accepts only validated terminal-only preflight errors without consent, preserves/redacts structured failure details, and bounds detection/advanced resolution. No live setup operation has been performed.

Coordinator final source-bound validation and candidate assembly are retained under `target/desktop-wizard-validation/20261006T041901Z/`. This location is not itself a pass: inspect source inventories, raw logs/results, manifest and candidate digests. Any real provider/runtime or release gate below remains separate. Earlier source identities/builds are historical and do not bind this implementation.

## Gates

Rust fmt/locked offline tests/build/strict Clippy; Flutter offline enforced-lock deps/analyzer/tests/Linux release; shell syntax/ShellCheck/mock prebuilt smoke; candidate source identity/checksums; fresh independent assurance/adversarial review. Only inert fixtures/fake processes, no live providers/models/managers. Actual Ollama compatibility/download/cancellation, desktop integration, denied-egress inference, clean-host installation, source reconstruction/license/SBOM, release workflow remediation and publication approval remain pending no-ship gates. Supplied icon still needs original workspace asset.
