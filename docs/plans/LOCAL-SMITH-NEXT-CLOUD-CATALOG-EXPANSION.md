# LOCAL-SMITH-NEXT-CLOUD-CATALOG-EXPANSION

## Bounded approval and intent

**Approved local source task:** the operator explicitly requested “just add a bunch that are 70-80% likely to be valid and add it to filter. worst case it fails on send.” This supersedes the proposed provider Models API integration: expand the static text-model catalog and add local picker filtering, not discovery. Published model identities are available from the preceding official-document research; no guessed identifiers or real-account tests are necessary. The operator accepts that listed models can be unavailable to their account and fail on an explicitly submitted request. This does not waive safety checks or accept unrelated quality-gate failures.

Repository `smith`, existing `feature/autonomy`, base `7f6a483ae82c4bcf647268f035f1e7cc80409d4b`, target this bounded uncommitted delta plus preserved cloud, keyring, queue, timestamp, Boot, header/logo and scroll work. Approval expires at task completion, revocation or material scope/base change. Elevated model-routing/operator-flow work requires architecture input, tests and fresh assurance. No commits, branch changes, installed upgrade/restart, host keyring/credentials, real API calls/charges, model downloads, new endpoints, publication or deployment are authorized.

## Independent inputs and reconciliation

- **Architecture assessment** `da722719-c67f-490e-86ac-fb3e8acb5779`: the 15 exact IDs below fit existing eight-per-provider/272-total limits and existing request/parser rules. Official model pages confirm compatible text APIs, OpenAI low reasoning, and listed contexts. Preserve per-provider validation defaults, strict response identity and original budgets. Synchronize Rust and Dart catalogs; repair installer/global-index assumptions. No transport, credential, schema or parser change is needed.
- **Producer/designer assessment** `4f0fc3da-f3f0-4952-8175-84f99b4b8670`: a compact trigger opening a private searchable dialog is the least-risk extension to the current non-searchable DropdownButton. Search must not change route/draft/queue or issue API requests; preserve selected-route summary, availability reasons and not-individually-verified labels. Revalidate activation against the originating controller/current catalog; same-route selection is a no-op. Cover keyboard, narrow/enlarged layout and stale dialogs.
- **Planner resolution:** use a bounded static catalog, no model-list API, no arbitrary custom IDs, no live checks. Adopt the private searchable picker without modifying ChatPanel or concurrent header/Boot work. No blocking disagreement remains. API/account failures remain factual existing send failures, never silent fallback/retry. Existing provider-generation validation requirements stay unchanged; a separate successful check of every model is not required.

## Exact catalog

Keep providers contiguous and existing first choices unchanged. OpenAI entries have context 1,050,000; Anthropic entries 1,000,000. Smith's own 64-KiB context/8192 generation/180-second limits remain unchanged.

| Provider | Ordered request IDs |
|---|---|
| OpenAI (7) | `gpt-6-astra`, `gpt-6-sol`, `gpt-6.1-sol`, `gpt-6-luna`, `gpt-5.6-sol`, `gpt-5.6-terra`, `gpt-5.6-luna` |
| Anthropic (8) | `claude-sonnet-5-5`, `claude-haiku-5-5`, `claude-opus-5-5`, `claude-fable-5-1`, `claude-opus-5`, `claude-opus-4-8`, `claude-opus-4-7`, `claude-sonnet-5` |

Official evidence: OpenAI per-model pages at `https://developers.openai.com/api/docs/models/<exact-id>.md`; [Claude overview](https://platform.claude.com/docs/en/models/overview.md) and corresponding `/docs/en/models/<line-version>/overview.md` pages. All selected Claude IDs are dateless pinned snapshots; no dated response-ID alias allowance is added. Account entitlement is untested. Omitted Claude thinking retains each model's own default, which is not universally adaptive.

## Requirements and acceptance traceability

| ID | Implementation | Blocking evidence |
|---|---|---|
| CAT-R1: useful expanded catalog including GPT-6 Sol | Rust catalog and exact Dart allowlist, 7/8 ordered entries; installer consumes shared catalog | Exact ordered identities/contexts/uniqueness/count tests; request/final-text fixtures across every entry; real Rust-to-Dart catalog parsing |
| CAT-R2: truthful availability, failures and immutable routing | Preserve generation validation, exact model response matching, consent, accepted route/context, one-attempt errors and no fallback | Existing/new catalog readiness, unverified-choice, unknown-ID/wrong-provider and routing/failure regressions |
| CAT-R3: searchable picker, no effects from search | Case-insensitive trimmed substring over label/model/provider/group; LOCAL/CLOUD order; clear/no-results/retained selection | Widget tests for open/type/clear/cancel/current no-op, unchanged draft/queue/revisions and zero picker-induced calls |
| CAT-R4: explicit safe selection | Only currently enabled exact route in originating active controller; preserve unavailable/historical selection and queued-send review | Stale/disposed/disabled selection, future-only routing, keyboard Enter/Escape and focus tests |
| CAT-R5: usable bounded presentation | Compact trigger, viewport-bounded scrollable dialog, readable reasons and not-verified labels | 400x400 / 2x text widget layout and reachability evidence; no overflow |
| CAT-R6: consistent documentation and delivery | Update three-model freeze/canonical descriptions, retain independent assurance and stable package evidence | Focused tests, configured component/aggregate/build/package gates, source/artifact hashes and validation ledger |

## Ownership and exclusions

- Backend implementer: `next/service/src/cloud.rs`, affected `next/service/tests/**` catalog/compatibility fixtures and `next/installer/tests/providers.rs` assumptions. No HTTP/keyring/worker/installer production behavior edits without escalation.
- Client implementer: `next/client/lib/cloud_routing.dart`, `lib/ui/cloud_selector.dart` and relevant cloud picker/routing/protocol/integration tests only. No Boot/header/ChatPanel/controller/queue production redesign.
- Parent: this plan, canonical/API-contract/as-built documentation, validation ledger, evidence, integration and fresh review coordination.

No new dependencies, wire/disk schema, context/token/deadline changes, remote model discovery, key handling, automatic inference, retries, fallback, endpoint/trust override or installation. Rebuild paired binaries: old strict clients/installers may reject new catalog IDs despite unchanged schema. Do not claim mixed-version compatibility or migrate installed state.

## Validation and evidence

Retain baseline input identities and scoped prechange copies under ignored `next/service/build/cloud-catalog-expansion/`; preserve prior task evidence. Run focused service catalog/protocol and client picker/routing/parser tests first, then configured `next/` gates with cloud tracing disabled: component tests and stable `make test build`, explicit package tests/verification as applicable. Record source stability and actual skips/failures, not only counts. No normal-host bus, keyring, vendor API, model or manager effect is a validation shortcut.

The preceding schema task records unrelated aggregate runner lock/timing failures. Do not fix them here, erase them or retry indefinitely. An observed new aggregate PASS establishes only that execution, not their root-cause resolution. If aggregate fails, retain failure and distinguish separate component/build/package results. Fresh independent assurance must inspect the actual scoped delta and evidence, not approve publication or accepted risk. Installed upgrade remains separately scoped.

## Outcome

**Implemented source and local candidate:** seven OpenAI/eight Claude entries and the private searchable picker are complete. Production changes are limited to Rust `MODELS`, Dart `cloudModels` and `cloud_selector.dart`; affected service/installer/client fixtures were updated without weakening their behavioral assertions. Focused backend61/client54 checks passed; a later test-only remediation run passed74. A new real split-pane unsent-chat regression closed independent reviewer L1.

**Final separate component/build/package PASS:** service258 (three controlled fixture ignores), installer279 plus GTK9 (two controlled fixture ignores), Flutter538 (zero skips), formatting/Clippy/analyzer, paired build and95-file explicit verification all passed on stable1,289 inputs. The41 package tests passed on that same source in the second aggregate and were reused rather than rerun. Four service copies and91 client/payload pairs match. [Retained evidence](../evidence/cloud-catalog-expansion/README.md) binds the source, commands and artifacts; [independent review](LOCAL-SMITH-NEXT-CLOUD-CATALOG-EXPANSION-REVIEW.md) distinguishes scoped source findings from broad gate acceptance.

**Aggregate acceptance remains unmet:** the first aggregate caught two obsolete picker-type test casts, repaired with actual picker interactions and retained historical recovery/no-effect assertions. The second failed unchanged runner/provider bounded-lease verification with `RUNNER_BUSY` at `tests/runner.rs:2130`; Flutter/GTK were incomplete/canceled. No third aggregate, runner fix or accepted-risk decision was made. Separate passes do not establish a causal fix or whole-product acceptance. No real-provider, keyring, installed upgrade or deployment result is claimed.
