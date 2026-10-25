# Smith shared identity and global instructions

## Approval and bounded objective

**LOCAL_PLAN_APPROVED:** the operator's “yes please” approves the preceding proposed first slice: service-owned shared Smith identity/context, compact product knowledge, global instructions/personality settings, minimal inspector, and local regression validation. Owner: current implementation session; approval expires at completion of this slice or material scope drift. No commit, branch operation, push, PR, installation, deployment, live state migration, live manager/keyring/GPU/model/vendor use, or automatic self-editing is included. No new external endpoint, permission, filesystem discovery, or runtime agent is authorized.

Repository: Smith; existing `feature/autonomy`; base HEAD `14d960975e76f906efb5e447ff3dece0e8e4385b`. Existing tracked/untracked work is supporting input, not disposable or owned by this change. Preserve it. The evidence directory retains content-free baseline identities; HEAD alone does not identify the worktree. This record does not retroactively approve unrelated work or an installed rollout.

Risk: **elevated** (prompt authority, protocol/storage migration, context export, concurrency/recovery). Independent producer, architect, designer analyses and planner reconciliation were performed in the preceding discovery session. Their findings are summarized below; they are not runtime guarantees or approval for external effects.

## Requirements, reconciliation, and decisions

- REQ01: Smith is the product identity; vessel/scope and actual provider/model are separate facts. Current vessel is `chat::global::assistant`, a bounded conversation surface, not all-chat memory. Named personas and executable Dev vessels are deferred; example address segments do not grant authority.
- REQ02: always include small compiled/versioned product knowledge about the app, installer, supported Ubuntu target, limits, recovery, and a source-maintenance map. No arbitrary repository/host scan. Implemented, target, and unknown facts remain distinct. Compiled knowledge is not live host observation.
- REQ03: enforced permissions and safety boundaries remain code-owned; shared identity/safety guidance precedes operator preferences and vessel/task/turn. Ordinary personality defaults may be overridden. Retrieved text/model output cannot grant capabilities, approvals, or persistent policy changes.
- REQ04: one service-owned, persisted global configuration: instructions, personality, additional guardrails, and explicit permission to share personal text with cloud providers. Preview/Apply are local and inert; read-only core/product context and revision are inspectable. No workspace-preference storage or automatic model writeback.
- REQ05: no cross-chat/private-project/history/credential/host-inventory inclusion. **Never silently omit personal global instructions on cloud sends.** When nonempty personal text is local-only, block cloud preparation/send with an actionable error; inspector may show locally previewed text but must mark export ineligible. Enabling sharing is not send consent; new effective bundle revisions require renewed destination-bound export consent.
- REQ06: immutable effective context/version/hash captured at every chat admission, including legacy local paths. Accepted work is unchanged. Reuse config revision to fence all sends/preparations and existing queue review. **MVP concurrency resolution:** reject global settings updates while any accepted service work is queued/running, matching current configure behavior. Preserve the editor draft and show BUSY; no hidden waits or resubmission. Unsent client queues still require review after applied revision changes; stale windows must fail closed at service admission.
- REQ07: installer/model/provider health checks remain exact, isolated synthetic prompts, without personal instructions or saved history. This resolves the “every prompt” concern by purpose: every conversational prompt is Smith-grounded; diagnostics are not conversations.
- REQ08/09: automatic learning/memory extraction, self-editing, runtime Dev agents, custom persona/vessel switching and new tool access remain deferred. Owner: operator/product; revisit only under a separately approved plan. No current completion claim for them.

The prior architect proposed omission of cloud-disabled personal text as a possible simplification. This is explicitly rejected in favor of REQ05: additional guardrails must not disappear on a route change. Earlier producer/designer preference for editable changes during accepted work is deferred in favor of visible BUSY rejection; snapshots remain required. The public vessel grammar is not broadened by this slice.

## Service/client contract

App3/Core2 stay unchanged; strict durable schema advances from disk6 to disk7. Add `instructions_get`, `instructions_update`, `instructions_receipt`, with strict required keys. `ConfigRevision` retains existing full-u64 integer/canonical-decimal-string wire encoding.

Settings exact shape: `{instructions:string, personality:string, guardrails:string, cloud_enabled:bool}`. Combined personal text is bounded to 1024 UTF-8 bytes; empty is valid. Preserve exact text; LF/tab permitted, other control characters rejected. Core/product/combined preamble have explicit bounds and count inside existing context limits.

- `instructions_get({route: Route|null})` is read-only with no keyring/provider/manager access. Return `config_revision`, `settings`, `renderer_version`, `core:{version,text}`, `product:{version,text}`, `vessel:string|null`, `local:{text,sha256}`, `cloud:{text,sha256,bundle_sha256,personal_included,eligible}`, `limits:{max_personal_bytes,max_preamble_bytes}`. Text is the exact preamble only, not a promise about future history or tokenizer fit. Null route previews the shared vessel-free prefix. All personal fields are included in both previews; `eligible:false` blocks their cloud dispatch.
- `instructions_update({request_id,instance_id,expected_config_revision,settings})` is full replacement. Return `{request_id,config_revision,changed}`. A changed update increments config revision once; identical settings do not. Record bounded content-free receipts/fingerprints; exact replay returns its original receipt without reapplying. Reject fresh updates during accepted work. No inference/worker wake/network effect.
- `instructions_receipt({request_id})` returns the original update receipt, including across restart; never replays. Unknown/expired outcomes remain uncertain, not permission to resubmit.
- Dedicated ledger: 32 retained receipts, 1024 non-reusable expired IDs, atomically capacity-bounded, following existing Projects patterns. Keep old personal text out of receipt payloads.
- Bound context digest to instructions/renderer/vessel plus conversation messages. Include preamble in context bytes/accounting; user/assistant message counts retain their meaning. Per-conversation provider/generation export grants additionally bind global cloud-bundle hash; preserve model-change behavior where the effective global bundle is unchanged.
- Strict disk1–6 migration preserves current state and old receipt meanings, interrupts unfinished work without replay, initializes empty personal settings/export grants, and fences activation of newly introduced/changed compiled context through configuration revision. Corrupt/exhausted/unpublishable state remains preserved. Old binaries reject disk7; source changes never migrate installed data.

Contract errors follow existing fixed wire error categories; new local-only export refusal must be explicit and secret-free. Client unknown-apply recovery queries original receipt plus authoritative current settings; never retries a write automatically.

## Traceability and write scopes

| Task | Requirements / acceptance | Write scope | Required evidence |
|---|---|---|---|
| TASK01 shared foundation | REQ01–03, AC01: every chat adapter gets exactly one versioned Smith preamble; fixed diagnostics unchanged | service context module/resources, admissions, native/cloud adapters/providers | exact request fixtures, token fitting/overflow, legacy/routed/diagnostic coverage |
| TASK02 storage/API/privacy | REQ04–07, AC02: strict revisioned persistence; no omission/export without consent; no replay | service durable migration, instruction ops/receipts, routing consent, directly affected service/installer compatibility tests | stale/cross-instance/duplicate/unknown/expired/BUSY/capacity/restart/publication/migration/consent tests |
| TASK03 client contract/state | REQ04–06, AC03: strict model/controller, persistent applied revision and retained dirty editor, queue fencing | new Dart instruction model/controller; AppApi validation/allowlist; workspace queue/routing integration | protocol/controller fakes, no-effect call counts, config drift/unknown apply/compiled Rust↔Dart fixture |
| TASK04 settings/inspector | REQ01/04–06, AC04: clear Smith identity, global editor, review/Apply and inspector, reachable recovery | new Dart editor, shell/main integration, focused UI tests | navigation/dirty preservation, loading/error/unknown/conflict/BUSY states, 400×400/2× text/keyboard fixtures |
| TASK05 canonical docs | REQ01–09, AC05: as-built contracts and limits are accurately documented | affected Architecture/Design/Process/Scope/Host details and assessment, cloud contract, maintenance guide | link/consistency checks; no target represented as enforced behavior |
| TASK06 self-check/assurance | all, AC06: stable-source, independently reviewed evidence | focused test additions, evidence, root validation | actual commands/exits, failures/skips retained; fresh assurance/adversarial review |

Implementation write ownership: service + directly affected installer test files belong to the service implementer; Dart API/model/controller/queue contract to the client implementer; UI shell/editor, canonical docs, integration and evidence to the parent unless explicitly reassigned. Do not overlap writes or revert unrelated input. Escalate necessary capability/scope changes.

## Validation and compatibility

Use existing isolated fixtures/temp roots, never normal host manager/session/model/API state. Start with focused unit/protocol/controller/widget tests and format/analyzer/Clippy; then service/installer/client suites, real compiled Rust↔Dart fixtures, and root `make test build`/package verification where the approved environment supports them. No silent fixture skips or mock-only claims of installed acceptance. Missing tools/caches/capabilities are reported as blocked, not bypassed with live effects.

Preserve intermediate failures and exact source/input identities. Required feature cases include private-canary isolation, malicious operator/turn text granting no tool authority, fixed diagnostics, instruction-counted budgets, oldest-pair-only trimming, stale preparation caches, new global-bundle consent, local-only cloud blocking, queued revision review, strict migration/corruption preservation, full-u64 echoes, lost acknowledgements with query-only recovery, and no inference from settings edits.

Self-check is followed by fresh independent assurance from requirements/base/diff/evidence, not author reasoning. Failing or missing required gates block release/installed acceptance; no agent accepts residual risk.

Rollback: preserve the original dirty source; no reset/stash/clean or blanket revert. Restoring older instruction content is a new revision, not revision reuse. Any later rollout needs separate named host approval, pre-migration state plus compatible paired artifacts/keyring references, and tested restore; binary-only downgrade is not rollback.
