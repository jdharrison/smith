# LOCAL-SMITH-OFFLINE-CHAT-SLICE-P0

## Intent and authority

**Approved local implementation scope:** the operator requested wrapping up `feature/autonomy` as an easy-install, offline ChatGPT-style Smith vertical slice, expandable later. This authorizes scoped worktree edits, local builds/tests, and local candidate artifacts. It does not authorize commits, pushes, merge, tags/publication, normal-path installation, live manager/service mutation, dependency/model provisioning, or accepted risk.

Branch: existing `feature/autonomy` bootstrap exception. Base: `825b8afe7aed14a44efda0c58fc74e65b8fb02b6`; candidate identity must additionally include the dirty source snapshot. Risk: critical (capability reduction, network and release installation paths).

## Independent analysis and reconciliation

Producer, architect, and designer assessments independently inspected implementation and policy. They agree on an explicit text-chat profile, Home/Chat with secondary Settings, separate backend/client artifacts, network-free prebuilt installation, and truthful offline claims. Existing orchestration remains available separately; hiding its UI is not an authorization boundary.

Preserved concerns:

- Real-model offline inference and clean-host desktop installation cannot be established by mock tests; these remain ship blockers until approved validation.
- Behavioral OpenCode verification is not cryptographic provenance; the new profile avoids OpenCode entirely.
- Models/Ollama and Ubuntu runtime libraries are prerequisites, not bundled or automatically downloaded.
- Conversation history is bounded in-memory, one default session, and is cleared on backend restart. Persistent history/streaming/voice are deferred.
- User-service registration currently selects legacy `serve`; the new profile is started explicitly. No silent unit/default migration is allowed in this slice.

## Reconciled requirements and acceptance

| ID | Requirement | Implementation/evidence |
|---|---|---|
| CHAT-1 | Explicit `serve --chat-only` immutable process profile, conflicting with echo; loopback only. | CLI parsing tests, restricted router tests; legacy serve retained. |
| CHAT-2 | Direct local Ollama with explicit/preprovisioned model; no tools, intent dispatch, OpenCode, warm services, pulls, search, Docker, cloud fallback, ambient proxy, or redirects. | Dedicated module and mock-provider negative tests, route exclusions. |
| CHAT-3 | Bounded input/history/response/time/concurrency; contextual turns, explicit reset, visible failures. | Admission/session tests, client state tests. |
| CHAT-4 | Home/Chat/Settings text-first client; no Work/pipeline/task injection, service mutations, remote fonts, required voice, or implied saved history. | Flutter analyzer/tests/Linux build; manual desktop remains pending. |
| CHAT-5 | Separate versioned backend and full Halo bundle artifacts with network-free install-only scripts, checksums and provenance. | Packaging/static checks, isolated-root install tests, artifact inventories and digests. |
| CHAT-6 | Exact source/evidence identity; honest local candidate rather than published/verified offline release. | Source hashes/snapshot, raw build/test logs, independent review. |
| CHAT-7 | Actual contextual inference without external networking and recipient installation without build tools. | Explicitly approved real-model/clean-host validation: pending, blocks publication. |

## Scope staging

Initial target: Ubuntu 24.04 LTS x86_64 text conversation using loopback Ollama and a separately provisioned model. Home/Chat-first delivery is a staged subset of Smith's longer-term workstation orchestration scope. Voice, Work, integrations, durable chat history, uploads, and token streaming are not part of this slice's product promise.

## Write scopes

1. Backend: `src/main.rs`, `src/commands/serve.rs`, new dedicated offline-chat module and its module declaration/tests. Do not alter service registration or legacy chat dispatch.
2. Client: `client/lib/`, `client/test/`, `client/README.md` only; preserve existing work/voice implementations separately where practical.
3. Packaging: new local packaging/prebuilt installer scripts and release guide, packaging tests; existing source installer remains unchanged.
4. Coordinator: canonical docs, work record, integrated validation/evidence and synthesis.

## Independent review remediation

Fresh static assurance identified two blocking capability gaps: assistant Markdown could implicitly load remote/local images, and health-check-then-chat on the shared legacy endpoint permitted listener replacement before dispatch. Both are remediated in source: assistant output is inert selectable text; Halo uses only `/v1/chat-only` and validates the chat-only reply schema; legacy routing rejects that endpoint before reset/dispatch. Regression tests cover resource-loading tripwires and replacement after health. Backend reset-on-failure details remain visible. Fresh re-review and final candidate-bound integrated validation follow; this record does not self-approve release.

## Validation and stop conditions

Rust fmt/locked build/tests/strict Clippy; Flutter cached dependency resolution/analyzer/tests/Linux build; shell syntax/ShellCheck; isolated prebuilt install/rollback/negative paths; paired local artifact checksums/provenance; fresh independent assurance/adversarial review. Use no live models, Docker, managers, or installed applications for automated checks. Missing tools/cache or unverified integration are reported, not bypassed with downloads or host changes.

No publication, live install or service migration occurs under this record. Prepare exact next approvals and effects for the operator.
