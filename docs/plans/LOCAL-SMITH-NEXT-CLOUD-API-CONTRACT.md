# LOCAL-SMITH-NEXT-CLOUD-API-CONTRACT

- Freeze the optional OpenAI/Anthropic text adapter subset for [TASK-001](https://github.com/jdharrison/smith/blob/168865b0422b787f08ab33e89d052cd4adbd11f7/docs/plans/LOCAL-SMITH-NEXT-CLOUD-PROVIDERS.md#external-api-contract-freeze-and-network-policy); do not implement another provider framework.
- Preserve explicit destination consent, service-owned final-text history, immutable accepted routes, one worker and no replay/fallback.
- This document alone is owned by this bounded task. No source, other documentation, credentials, keyring, installed state or live inference is changed or accessed.

## Status and evidence

**Target contract, originally frozen 2026-10-15 and expanded by the approved [static-catalog follow-up](https://github.com/jdharrison/smith/blob/168865b0422b787f08ab33e89d052cd4adbd11f7/docs/plans/LOCAL-SMITH-NEXT-CLOUD-CATALOG-EXPANSION.md); not an account-availability guarantee.** The parent work record binds local-source approval and records prior architecture reconciliation. This document supplies its API prerequisite, not independent assurance or authorization for external effects. Canonical policy and parent integration updates remain main-agent owned. Follow [AGENTS](../../AGENTS.md) and [Process](../PROCESS.md#approval-boundaries).

Root-promotion navigation update: parent/follow-up records are historical and linked to their immutable checkpoint; request/catalog semantics and dated research below are unchanged. The current operational boundary is in canonical Scope/Architecture.

Public documentation research only. No authenticated request, account availability check, paid request or live API validation occurred. Published model support is not evidence that this operator's account can use it, that billing is configured, or that a request will finish within Smith's deadline.

### Official source ledger

The original ledger observations below are dated **2026-10-15**. Additional model-page checks belong to the static-catalog follow-up, not retroactively to this original ledger. “Parent verified” explicitly distinguishes supplied research from this task's fetches. Provider documentation is external evidence, not authority to weaken Smith policy.

| Ref | Official URL | Evidence used |
|---|---|---|
| O1 | https://developers.openai.com/api/docs/models/gpt-6-astra | Fetched: exact ID, Responses support, context/output limits, reasoning efforts; no dated snapshot listed in retrieved page |
| O2 | https://developers.openai.com/api/reference/resources/responses/methods/create | Fetched: endpoint/auth example, request fields, status/model/error/incomplete/output schemas, phase, cache options |
| O3 | https://developers.openai.com/api/docs/guides/text.md | Parent verified: role/content input array; traverse output items, not just output[0] |
| O4 | https://developers.openai.com/api/docs/guides/your-data.md | Fetched: storage, abuse monitoring, caching and retention exceptions |
| O5 | https://developers.openai.com/api/docs/guides/error-codes.md | Fetched: authentication/access, rate/quota/billing and server errors |
| A1 | https://platform.claude.com/docs/en/about-claude/models/overview | Fetched: exact Sonnet/Haiku IDs, 1M context/128K output, adaptive thinking |
| A2 | https://platform.claude.com/docs/en/api/messages/create | Parent verified: endpoint, headers, max_tokens/messages/model/stream, text/thinking/redacted blocks and stop reasons |
| A3 | https://platform.claude.com/docs/en/api/errors | Fetched: HTTP/error envelope, SDK automatic retries, current adaptive-thinking compatibility |
| A4 | https://platform.claude.com/docs/en/build-with-claude/zero-data-retention | Fetched (renders “API and data retention”): organization arrangements, feature eligibility and legal/safety exceptions |

## Detected Stack

Parent plan's base-source assessment identifies Rust 2021 service (`serde`/`serde_json`, declared Rust 1.83 minimum), Flutter/Dart clients and native Rust installer. Use the remediated single-connection Hyper/Rustls/Tokio transport and injectable transport/keyring seams. The initial Reqwest direction was superseded to enforce the raw-header limit during reading, not after parsing; this document does not claim a fresh source/build audit. Production HTTP belongs in the service, never Flutter or installer. See [concrete seams](https://github.com/jdharrison/smith/blob/168865b0422b787f08ab33e89d052cd4adbd11f7/docs/plans/LOCAL-SMITH-NEXT-CLOUD-PROVIDERS.md#detected-stack-and-concrete-seams).

## Frozen catalog and identity

Exactly fifteen static entries: seven OpenAI, then eight Anthropic, within existing per-provider limits. The provider plus exact request model ID is the catalog key; labels are presentation only. The operator approved broader plausible choices without a real-account check; published support is not entitlement or successful inference. There is no discovery, arbitrary custom ID, substitution or automatic retry. Existing first-per-provider validation choices remain Astra and Sonnet 5.5. Deprecated GPT-4.1 mini/GPT-5 mini, Pro variants needing different parameters, and unverified illustrative IDs remain excluded.

| Provider | Display label | Request model ID | Exact accepted response.model set | Published ceiling (not Smith budget) |
|---|---|---|---|---|
| `openai` | GPT-6 Astra | `gpt-6-astra` | `{gpt-6-astra}` | 1,050,000 context / 128,000 output |
| `openai` | GPT-6 Sol | `gpt-6-sol` | `{gpt-6-sol}` | 1,050,000 context / 128,000 output |
| `openai` | GPT-6.1 Sol | `gpt-6.1-sol` | `{gpt-6.1-sol}` | 1,050,000 context / 128,000 output |
| `openai` | GPT-6 Luna | `gpt-6-luna` | `{gpt-6-luna}` | 1,050,000 context / 128,000 output |
| `openai` | GPT-5.6 Sol | `gpt-5.6-sol` | `{gpt-5.6-sol}` | 1,050,000 context / 128,000 output |
| `openai` | GPT-5.6 Terra | `gpt-5.6-terra` | `{gpt-5.6-terra}` | 1,050,000 context / 128,000 output |
| `openai` | GPT-5.6 Luna | `gpt-5.6-luna` | `{gpt-5.6-luna}` | 1,050,000 context / 128,000 output |
| `anthropic` | Claude Sonnet 5.5 | `claude-sonnet-5-5` | `{claude-sonnet-5-5}` | 1M context / 128K synchronous output |
| `anthropic` | Claude Haiku 5.5 | `claude-haiku-5-5` | `{claude-haiku-5-5}` | 1M context / 128K synchronous output |
| `anthropic` | Claude Opus 5.5 | `claude-opus-5-5` | `{claude-opus-5-5}` | 1M context / 128K synchronous output |
| `anthropic` | Claude Fable 5.1 | `claude-fable-5-1` | `{claude-fable-5-1}` | 1M context / 128K synchronous output |
| `anthropic` | Claude Opus 5 | `claude-opus-5` | `{claude-opus-5}` | 1M context / 128K synchronous output |
| `anthropic` | Claude Opus 4.8 | `claude-opus-4-8` | `{claude-opus-4-8}` | 1M context / 128K synchronous output |
| `anthropic` | Claude Opus 4.7 | `claude-opus-4-7` | `{claude-opus-4-7}` | 1M context / 128K synchronous output |
| `anthropic` | Claude Sonnet 5 | `claude-sonnet-5` | `{claude-sonnet-5}` | 1M context / 128K synchronous output |

Attempt eligibility is separately amended by the [saved-key contract](https://github.com/jdharrison/smith/blob/168865b0422b787f08ab33e89d052cd4adbd11f7/docs/plans/LOCAL-SMITH-NEXT-SAVED-KEY-ATTEMPTS.md): configured active generation plus exact supported route suffices, with no mandatory cached observation or previous successful validation. Observation/check/individual-model verification facts remain independently truthful; execution still retrieves the admitted exact credential before HTTP. Request/response shapes, identities, endpoints and budgets below are unchanged. Paired updated clients/installers are required for the new schema-3 availability semantics.

Expansion evidence: architecture assessment in the [follow-up plan](https://github.com/jdharrison/smith/blob/168865b0422b787f08ab33e89d052cd4adbd11f7/docs/plans/LOCAL-SMITH-NEXT-CLOUD-CATALOG-EXPANSION.md) checked each exact OpenAI `/api/docs/models/<id>.md` page and the [Claude overview](https://platform.claude.com/docs/en/models/overview.md) plus corresponding individual current/legacy model pages. OpenAI's selected entries support Responses and low reasoning; all selected Claude IDs are pinned dateless snapshots. These are public-document checks, not authenticated availability tests.

The Astra page describes a context window, not a guarantee of 1,050,000 input tokens **plus** output. Reserve output and framing inside the context budget.

Aliases can resolve differently over time. Accept a returned model only by byte-exact membership in that request entry's compiled allowlist. No prefix matching, date-suffix regex, trimming, case folding, cross-entry acceptance or learning IDs from a response. No dated IDs have been independently established here: the initial sets are singletons. An unlisted dated resolution fails closed as identity mismatch even if plausible. Adding a documented exact snapshot requires a reviewed catalog/contract update and fixtures; this availability tradeoff is deliberate.

## Common transport, bounds and retention

**Target:** one foreground POST per explicitly authorized validation or dispatched chat. Only HTTPS, standard certificate/hostname validation, port 443 and the two fixed URLs below. No custom URL/header/trust override, ambient proxies, redirect following, cookie jar, model listing, token-count API, tools/search, uploads, remote IDs/chaining, polling, background mode, retry (including library/protocol retries), or provider/model fallback. A 3xx is failure; never attach credentials to its destination. Fixed application URLs are not an OS egress sandbox.

| Bound | Frozen value / enforcement |
|---|---|
| Overall | 180 seconds from worker dispatch, monotonic; includes secret access, DNS, connect/TLS, write, read and parse; cancellation closes pending transport |
| Secret access | At most 3 seconds, clipped to remaining overall deadline; no unlock/prompt or environment/plaintext fallback |
| Connect | At most 10 seconds including DNS/TLS, clipped to original deadline; never reset after progress |
| Headers | At most 16 KiB raw response-head bytes, enforced while reading, including status lines, whitespace and all informational response heads |
| Body | At most 1 MiB (1,048,576 bytes), success **and error** responses, incrementally bounded independent of Content-Length; disable automatic decompression or also cap decoded bytes |
| Final text | At most 64 KiB (65,536 UTF-8 bytes), checked during accumulation; no silent clipping |
| Input | At most 64 KiB combined UTF-8 context including the current user message (itself at most 16 KiB); at most 16 complete historical pairs; no cross-chat data |
| Output | **8192 total output tokens including reasoning**, not 8192 visible tokens plus reasoning |

**Implementation-owned budget decision within the approved feature scope:** the verified current models include reasoning in their output budget, so the coordinator originally replaced the provisional 1024-token cloud cap with 8192 for the first three entries; the same 8192 total cap, including any reasoning, is retained for all fifteen. This is not a user-specified token count or live billing authorization. The parent plan records the same amendment; the native runner's existing cap is unchanged. Reasoning can consume the entire budget: empty/reasoning-only and truncated results still fail, never retry with a larger cap.

Use conservative request-only accounting: count each UTF-8 input byte as one estimated token, add 256 estimated framing tokens per message and 4096 fixed overhead, then reserve 8192 output tokens inside the published context window. Include any service-owned instruction text within the 64-KiB context bound and accounting. This is a conservative Smith admission heuristic, not a provider-tokenizer guarantee. Serialize to a bounded request buffer of at most 1 MiB, accounting for JSON escaping. Remove oldest **complete** pairs deterministically if needed; never trim the current user text or alter durable history. Reject if the remaining required input cannot fit. Do not retry a vendor context rejection.

Only validated final text crosses the adapter into service history/results. Treat all provider output as inert, untrusted text. Never log, persist, forward or replay raw bodies, thinking/reasoning summaries, signatures, encrypted reasoning, annotations, remote IDs, or provider error messages. Discard skipped content in bounded transient memory; do not claim secure erasure. Diagnostic messages come from fixed Smith categories, not remote text.

**External retention facts, not Smith guarantees:**

- OpenAI `store:false` disables generated-response storage for later API retrieval; omitted `store` defaults true (O2). It is not ZDR. O4 describes default abuse-monitoring retention up to 30 days with legal/harm exceptions; separately approved organization/project controls have limitations. Prompt caching can retain encrypted KV application state, with documented 24-hour expiration and newer minimum-TTL semantics independent of `store`. Do not send deprecated `prompt_cache_retention:in_memory` as a supposed Astra privacy control. This minimal request leaves cache configuration omitted and makes no no-cache claim.
- Anthropic Messages has no verified per-request `store:false` equivalent in this contract: **do not invent one**. A4 describes organization-level ZDR arrangements and Messages/adaptive-thinking eligibility, but eligibility is not proof of the operator's arrangement. It also describes legal holds and flagged inputs/outputs retained up to two years, and separate Covered Model requirements. Do not promise zero retention or extrapolate a universal deletion deadline from an eligibility table.
- API data training and storage are distinct: O4 says no training unless explicitly opted in; A4 says no training of retained data without express permission. These descriptions do not establish this account's settings. Show provider processing/retention and possible charges before authorized export. Local deletion cannot delete provider records. Rates, quotas and access vary; no free-tier, cost ceiling, universal availability or latency guarantee is asserted.

## OpenAI Responses adapter

POST **`https://api.openai.com/v1/responses`**. Use JSON content type and service-injected Bearer authentication; no credentials appear in fixtures. No additional organization/project headers or API-version header are needed by this subset (O2).

Frozen request fields: `model`, `input` role/content array, `store:false`, `stream:false`, `background:false`, `max_output_tokens:8192`, `reasoning:{effort:"low"}`. Astra explicitly supports low/medium/high/xhigh/max (O1); freeze **low**, not unsupported none/minimal. O2 defines the cap as visible plus reasoning tokens. Omit tools, sampling (`temperature`, `top_p`, `top_k`), reasoning summaries/include, metadata, conversations, previous_response_id, server prompts and automatic compaction. Plain text is the default format; default input truncation is disabled. Historical assistant final text uses the easy-input message form with `phase:"final_answer"`; current/history user text uses `input_text` content. No remote output item is replayed.

Success requires HTTP **200**, valid JSON object, `object:"response"`, exact allowed `model`, `status:"completed"`, explicitly null `error` and `incomplete_details`, and a valid `output` array. Reject missing/mistyped required fields. Reject non-null conversation/previous_response_id, `background:true` or `store:true` if returned.

Traverse **every** output item, not output[0] and not an SDK-only top-level output_text shortcut:

1. `reasoning`: discard all content/summary/encrypted fields. If status is supplied, require completed; never expose it.
2. `message`: require assistant role, completed status and content array. Scan all content even in commentary for refusal/unsupported types. Accept only output_text with string text; any refusal rejects the whole result. Discard commentary-phase text. Collect text from phase final_answer or absent/null phase; reject unknown phase.
3. Reject all other item types, including every tool/function/search/program/compaction item, even alongside valid text. Ignore unrelated additive metadata fields rather than treating them as output.
4. Concatenate eligible text blocks in wire order without invented separators; require non-whitespace final text within the byte cap. Return only after the entire envelope passes.

There is **no Chat Completions finish_reason/stop requirement** here. Responses terminal status is authoritative: incomplete (including max_output_tokens/content_filter/max_messages/steered), failed, cancelled, queued, in_progress, missing or unknown status never succeeds. Do not poll/continue. Classify explicit refusal/content filtering as rejected, token exhaustion as truncated, other incomplete statuses as incomplete; never publish partial text.

## Anthropic Messages adapter

POST **`https://api.anthropic.com/v1/messages`**. JSON content type; service-injected `x-api-key`; fixed `anthropic-version` value **`2023-06-01`**; no beta headers (A2).

Frozen request fields: `model`, `max_tokens:8192`, `messages`, `stream:false`. Send chronological user/assistant text content blocks and end in the current user message, never assistant prefill. If service instructions exist, use top-level `system`, not a user-controlled system role. Omit `thinking` to retain each model's default (adaptive for some models; off by default for Opus 4.7/4.8, per the [thinking configuration guide](https://platform.claude.com/docs/en/build-with-claude/thinking.md)), and omit `temperature`, `top_p`, `top_k`, output_config, stop_sequences, tools/tool_choice, cache_control, metadata, containers and remote IDs. Do not send legacy thinking enabled/budget_tokens or disabled. The total output budget includes thinking; no separate extra reasoning allowance.

Success requires HTTP **200**, JSON `type:"message"`, `role:"assistant"`, exact allowed `model`, content array, **`stop_reason:"end_turn"`**, and explicitly null `stop_sequence`. Anthropic has no required Responses-style status field.

Scan all content blocks: concatenate `text` blocks' string `text` in wire order without separators; discard `thinking` and `redacted_thinking` entirely, including signatures/data. Reject all other block types (including tool_use/server_tool_use/tool results), even mixed with text. Ignore additive metadata, never fetch citations. Require non-whitespace final text within 64 KiB. Reject every other/missing/unknown stop reason: max_tokens/model_context_window_exceeded = truncated, refusal = rejected, tool_use = unsupported, pause_turn/stop_sequence/other = incomplete or unsupported. Never resume a pause, replay thinking, or publish partial output. Final-only reconstructed history deliberately forgoes providers' reasoning-continuity optimizations; no tool loop is supported.

## HTTP, parser and delivery errors

HTTP status is checked before interpreting success content. Every failure returns a fixed, credential-free Smith error category and no assistant text. Parse bounded JSON error envelopes only for allowlisted machine codes; never string-match or echo human error messages. OpenAI supplies `error.code/type/message`; Anthropic supplies `type:"error"`, `error.type/message` and potentially request_id. Unknown codes, HTML/proxy errors and malformed JSON remain generic failures, not success.

| Condition | Adapter handling |
|---|---|
| 401 | Authentication rejected; do not delete key or assert which credential defect caused it |
| 403 | Access denied (permissions/region/account policy may differ); no substitution |
| 402 | Billing-related rejection where returned (documented by A3) |
| 400/404/409/413/422 or other 4xx | Request/resource rejection; 413 size; no parameter mutation or retry |
| 429 | Rate/quota limited; OpenAI exact credit_balance_exhausted/organization_spend_limit_exceeded/project_spend_limit_exceeded/organization_usage_limit_exceeded may refine billing/quota classification; do not assume every 429 is transient |
| 500/503/504/529 and other 5xx | Provider failure/timeout/overload; no automatic retry |
| 3xx or unexpected 2xx (including 202/204) | Unsupported response; no redirect, polling or follow-up |
| TLS/DNS/connect failure proven before request transmission | Not sent; fixed transport error |
| Disconnect/timeout/cancellation after transmission may have begun | Remote outcome unknown; never claim no charge or no processing and never replay |
| 200 with wrong identity, invalid schema, unsupported output, refusal, truncation, empty text or oversized body | No usable completion; discard all text, retain local failure and possible remote-processing uncertainty |

Retry-After and vendor retry recommendations do not authorize retries. A transport error after attempted send must not be relabeled “not sent” without evidence. Adapter categories must map to the parent's existing wire/delivery vocabulary rather than invent new wire enums here. Never persist raw headers/remote request IDs; parent operation identity supplies local correlation.

## Synthetic fixture examples

These are **hand-authored parser/request fixtures, not captured API responses**. Response examples contain the relevant subset, not every vendor metadata field. Keys/headers are deliberately absent.

OpenAI request:

```json
{"model":"gpt-6-astra","input":[{"role":"user","content":[{"type":"input_text","text":"Say hello."}]}],"store":false,"stream":false,"background":false,"max_output_tokens":8192,"reasoning":{"effort":"low"}}
```

OpenAI 200 response: expected final text `Hello.` despite reasoning preceding the message and two text blocks:

```json
{"object":"response","model":"gpt-6-astra","status":"completed","error":null,"incomplete_details":null,"output":[{"id":"rs_fixture","type":"reasoning","summary":[],"encrypted_content":"synthetic-discard-canary"},{"id":"msg_fixture","type":"message","role":"assistant","status":"completed","phase":"final_answer","content":[{"type":"output_text","text":"Hello","annotations":[]},{"type":"output_text","text":".","annotations":[]}]}]}
```

Anthropic request (repeat unchanged apart from exact model ID for Haiku):

```json
{"model":"claude-sonnet-5-5","max_tokens":8192,"messages":[{"role":"user","content":[{"type":"text","text":"Say hello."}]}],"stream":false}
```

Anthropic 200 response: expected final text `Hello.`; both non-text blocks must disappear from service results/history/logs:

```json
{"id":"msg_fixture","type":"message","role":"assistant","model":"claude-sonnet-5-5","content":[{"type":"thinking","thinking":"synthetic-discard-canary","signature":"synthetic-signature"},{"type":"redacted_thinking","data":"synthetic-redacted-canary"},{"type":"text","text":"Hello."}],"stop_reason":"end_turn","stop_sequence":null}
```

Error fixtures: HTTP 429 OpenAI and HTTP 529 Anthropic respectively; neither returns text, retries or leaks `message`:

```json
{"error":{"type":"insufficient_quota","code":"credit_balance_exhausted","message":"synthetic-error-canary"}}
```

```json
{"type":"error","error":{"type":"overloaded_error","message":"synthetic-error-canary"},"request_id":"req_fixture"}
```

## Validation Contract

**Target blocking evidence for adapter integration:**

- Capture exact request JSON/endpoint/header names with fake secrets for all three catalog entries; prove omitted forbidden fields, correct role history, no cross-chat history and exactly one POST. No real credentials or public API transport.
- Parameterize success fixtures across the exact catalog. Test reasoning-before-text, multiple messages/blocks, commentary exclusion, absent/final_answer phase, and final-only reconstructed follow-up history.
- Mutate fixtures for every rejected status/stop reason; refusal mixed with text; tool mixed with text; unknown discriminants; missing/null/wrong-type model/status/role/content/text; wrong exact model and plausible dated/prefix/case variants; reasoning-only, whitespace-only, malformed JSON/UTF-8 and duplicate security-relevant JSON keys. All must fail closed with no partial text. Parsers must reject duplicate security-relevant keys rather than silently selecting one.
- Boundary tests at limit and limit+1 for final UTF-8 text, current user, retained context, escaped request body, response/error body and headers; deterministic complete-pair trimming; oversized chunked/decompressed bodies and slow drip. No truncation by byte slicing.
- Inject secret/connect/read timeouts and cancellation under the same monotonic deadline; prove no post-deadline send or restarted budget. Test proxy environment, 3xx cross-host credential leakage, TLS invalid certificate/hostname, HTTP2/library retry behavior and exact send counts.
- Canary assertions across successful/failed history, DTOs, logs and errors for secrets, provider error text, commentary, thinking, encrypted/signature/redacted data. Observe-only UI/catalog/keyring checks produce zero HTTP calls. Unknown delivery never resumes after restart.
- Parent service/component/aggregate gates must exit zero and retain revision-bound logs plus fresh independent assurance. This documentation task did not run adapter tests or claim their existence/pass. Additional non-blocking confidence: future separately approved account/model integration and latency/cost measurements; absence is not live-validation evidence.

## Prioritized handoff and Release Safety Policy

1. **P0:** main reconciles the 8192 cloud-only amendment and canonical-document deltas; backend implements exact catalog, serialization, bounded transport and fail-closed parsing at the parent's service seams.
2. **P0:** add/run isolated fixtures and boundary/canary tests before exposing routes; maintain the parent's consent, credential-generation and delivery-state contracts.
3. **P1:** main runs combined service/installer/client gates and fresh assurance; no release/deployment or account verification is implied.

Failing build/tests, compiler/runtime errors, unresolved critical warnings or missing required verification evidence are blocking: **no ship**. This document neither approves itself nor accepts alias/retention/billing risk on the operator's behalf. Unknown dated aliases intentionally fail until reviewed. Live validation, installed-state changes and external publication require fresh exact human approval.

## Agent Instructions

- Implement only the frozen three-entry text subset under the parent approval and your assigned source ownership.
- Preserve the original deadline and no-retry/no-fallback/no-tool/no-remote-state boundaries on every failure path.
- Return only fully validated final text; discard reasoning and raw external diagnostics.
- Record synthetic evidence truthfully, reconcile parent policy wording through its owner, and obtain independent review before any release claim.
