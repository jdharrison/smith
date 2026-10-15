# Multi-chat implementation and independent review

Work record: [LOCAL-SMITH-NEXT-MULTI-CHAT](LOCAL-SMITH-NEXT-MULTI-CHAT.md). Base `2cc71a94fe924fbb9d324f29332e9f24c8c5991c`, branch `feature/autonomy`, uncommitted candidate. Final commands completed 2026-10-15 UTC. This record is a coordinator synthesis of independent reviews, not human acceptance.

## Outcome

**Implemented source; scoped independent backend/client reviews PASS. Aggregate acceptance BLOCKED.** No release/deployment approval or accepted-risk decision follows. Concurrent installer appearance work was preserved.

- `make service/test installer/test client/test`: PASS in the final bounded run, including formatting/lint and component tests.
- `make test build`: FAIL in tests; paired build not reached. Existing `provider_prefers_full_vulkan_and_falls_back_once_only_before_completion_dispatch` at `next/service/tests/runner.rs:1676` expected 17 launches, observed 16 (runner suite: 30 passed, 1 failed).
- Separate `make build`: PASS; exported package verification: PASS.
- Source changed during both windows: installer UI/capture work. No stable combined-source PASS is claimed. No unrelated test was weakened to pass.

Full command timestamps, exits, input hashes, log hashes and artifact identities are in [evidence JSON](LOCAL-SMITH-NEXT-MULTI-CHAT-EVIDENCE.json). Raw logs remain local ignored outputs; their digests and structured results are retained, not embedded logs containing local environment details.

Earlier failures remain evidence: first combined attempt hit the installer CLI finite-budget fixture timeout; second and final aggregate attempts hit the unchanged runner launch-count assertion. Earlier successful checks do not supersede these failures.

## Independent backend review

Reviewer context `801e9b3b-8ce9-4229-9843-960cb6c89fc6`: final scoped PASS, all findings closed.

| Finding | Closure |
|---|---|
| Accepted jobs could exhaust encoded-byte/revision headroom | Phase-aware completion reservations; four jobs complete at exact limits |
| Invalid origin/deletion receipt relationships accepted | Duplicate origins, existing-target delete receipts and retained origin/job target contradictions rejected; expired origins permitted |
| High-u64 valid legacy configuration could not migrate | Lossless v2-only decimal-string representation above safe integer; maximum mutation refuses without change |
| Recognizable trailing frame lost v2 correlation | Version/call correlation preserved |

Independent final command: `cargo test --offline --locked --lib app::v2::tests -- --nocapture`: **23 passed**, plus scoped whitespace check. Reviewed SHA256:

- `next/service/src/app_v2.rs`: `37426f92a76049e8c6fb50d03f6e884c72261e88ea37e09126c9abc65f5bc026`
- `next/service/src/app_v2_tests.rs`: `8da7ea3531844293f12e2f69d17d5218e93b9f76af449f8aa0a6e281958a84c8`

## Independent client review

Reviewer context `2e24cdde-9a2c-4514-9d8e-afe2d27300dc`: final scoped PASS, all four findings closed.

1. Accepted identity now survives a subsequent poll failure, retaining query-only recovery.
2. Migrated inert legacy voice results parse without enabling voice execution.
3. Latest failed/interrupted outcomes survive reattachment without overriding newer outcomes.
4. Historical lifecycle receipts cannot regress newer metadata or resurrect deleted chats.

Independent evidence: original probes plus permanent regressions **26 passed**; additional probes **2 passed**; actual Rust migration/socket fixtures **4 passed**; full client suite **223 passed, no skips**. High-u64 representation also verified. Reviewed SHA256:

- `next/client/lib/app_api.dart`: `9a6ad13a0b79dd0bc9346765900b5fa759bfdbd09ae524fac4853f487da95568`
- `next/client/lib/app_v2.dart`: `244be3f11c1466d49cf0eb8c653ff09cad16929eba1dfaf94da47011396b4ccf`
- `next/client/lib/workspace_controller.dart`: `f155aa4f3dc394ce9f72fa04efdb1545a39bc0c5f0bc1bbec533c73d63b890d6`
- `next/client/test/assurance_regression_test.dart`: `99f7a897f3ccb1c64ddadc65c1588c58e4c677d68d4a8ff016ceed6af3265f9d`
- `next/client/test/rust_socket_integration_test.dart`: `0aec031d284b0f85db0275312132cd374fac26afcf0a989cb14a910a1b37307a`

See also [client self-check](../../next/client/MULTI_CHAT_VALIDATION.md). Installer compatibility is tested by two real-service/fake-provider tests in `next/installer/tests/multi_chat_compatibility.rs`, preserving exact v1 DTOs and deleted legacy-conversation semantics.

## Acceptance and exclusions

REQ/AC 001–005 have implementation and focused regression evidence (migration/context, lifecycle, FIFO, protocol/recovery, UI/keyboard respectively). REQ/AC 006 has independent review and recorded identities, but stable aggregate validation remains unresolved. Ready for human source review, **not final acceptance or deployment**.

Four outstanding chat jobs mean one generation plus up to three FIFO queued jobs, not parallel inference. New chat persists only on first accepted Send. Unsent drafts/scroll are process-local; context is isolated and configuration shared. Delete is logical current-store deletion, not secure erasure. Historical single-chat goldens are not current multi-chat screenshots.

No installation, live-state migration, service/manager action, model download, real inference, commit or publication occurred. Installed graphical QA remains separate. Any live rollout requires separately approved paired client/service artifacts and pre-migration state backup/restore; old binaries cannot read v2 state. Next gate: resolve the existing runner fixture failure and rerun aggregate validation after concurrent installer work stabilizes, without treating repeated retries as remediation.
