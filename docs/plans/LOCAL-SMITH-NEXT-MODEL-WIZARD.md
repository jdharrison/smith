# LOCAL-SMITH-NEXT-MODEL-WIZARD

## Intent / authority

2026-10-13 user approved continuing the MVP vertical slice: Qwen3-14B option preselected for fresh installer setup, default model thinking, focused installer-owned model setup and rerun add/remove. Source and isolated fixtures only. No weights/live install/user manager/GPU/GUI/commits/remote mutation. Existing feature/autonomy and all uncommitted work preserved. Model removal implementation is authorized, not removal of real installed data. Prior host upgrade approval remains consumed.

## Research and reconciliation

Official HF metadata endpoint `https://huggingface.co/api/models/Qwen/Qwen3-14B-GGUF?blobs=true` returned revision530227a7d994db8eca5ab5ced2fb692b614357fd, Qwen3-14B-Q4_K_M.gguf size9001752960, LFS SHA256500a8806e85ee9c83f3ae08420295592451379b4f8cf2d0f41c15dffeb6b81f0, Apache-2.0/ungated. Metadata only, no weights. Its template enables thinking unless explicitly disabled. Pinned llama.cpp defaults were independently researched in `https://raw.githubusercontent.com/ggml-org/llama.cpp/b11429/common/common.h`: reasoning_budget_tokens=-1 disables budget enforcement, enable_reasoning=-1 selects automatic template behavior, and the DEEPSEEK response format separates reasoning_content. Smith omits forced-zero reasoning and retains only valid complete final content; this is source research, not real14B inference proof. No hardware throughput claims.

Read-only producer/architect/designer reconciliation c8518634-6266-4bc8-9130-658724e211e5 identified removal/start races, selected preference protection, 8GiB download cap and installer-direct inference not validating service. This single-context role reconciliation is not independent triangulation or assurance. Binding resolutions below; independent focused assurance remains required after tests.

## Bounded contract

- REQ1: Qwen3-14B Q4_K_M compiled catalog/default UI preselection. No download/preference/inference on opening. Existing saved model is preserved on rerun. Streaming model maximum16GiB only; artifact publisher bounds unchanged.
- REQ2: One focused step at a time: Install Smith (paired artifacts + separately confirmed ensure registration/start) -> Model (choose/install) -> service-backed check -> Complete. Model management entry bypasses artifact republication. Dark green/black, compact text, details collapsed, truthful byte versus indeterminate progress.
- REQ3: Separate original-ID/expiry consent for download, preference, inference and destructive exact-model removal. Typed GUI confirm can bind a displayed catalog name without a terminal. No blanket yes, auto retry/reprepare or automatic app launch. Existing CLI/Make modes remain.
- REQ4: Narrow service-owned verify_model operation, request/poll/durable correlation, exact model/SHA/config revision binding. Fixed greeting; no conversation history or appended chat turn. Tool/identity checks retained; reasoning-only/empty/truncated inference is not successful verification. Service cancellation/owner join preserved; stopping installer observation does not cancel accepted service work or resend it.
- REQ5: Optional configure_checked model/voice:null/expected_revision rejects stale configuration without changing existing configure schema. verify_model/remove_model args request_id/model/sha256/expected_revision. Results {model,sha256,verified:true,assistant_text} and {model,sha256,removed:true}. New bounded durable operation kinds accepted by service and observational desktop protocol, no provisioning controls added to desktop.
- REQ6: Removal service-mediated, admitted on same single worker, expected revision and selected-model rejection. No implicit runner/service stop. All loaded runners may conservatively block removal with RUNNER_BUSY (including idle-loaded, waits for normal idle expiry). Acquire installation lock then runner lease, descriptor-relative verified catalog marker/file only, marker removal synced before file removal; preserve orphan/partial data, no recursive cleanup, retry/purge/rollback or chat/history/config writes. Runtime lease acquired before production model verification/start so deletion cannot race loading. Direct installer inference must use same lease contract.
- REQ7: Model-default thinking (no forced reasoning-budget0), final content only remains visible/persisted. Keep existing ctx4096/max_tokens1024/180s budgets provisionally with clear CPU/timeout/truncation disclosure; installer service-check observer210s. Do not fabricate unmeasured adequate performance. Inspect pinned runner help/source default semantics before claiming default is enabled.

## Implementation ownership / gates

Service slice: runner/providers/app, service tests, desktop schema compatibility tests only. Installer slice: wizard/UI/setup/catalog/CLI and tests, no artifact/fs semantics changes beyond using established primitives. Parent: metadata research, canonical docs/evidence, orchestration/review. No concurrent same-file writers.

Focused locked fmt/Clippy/test checks, fake service check/removal/lock/path/state regressions, fake download size/progress/consent/rerun cases and GTK Xvfb focused-step tests; then aggregate make test build. Preserve filesystem/consent/peer/worker safeguards. One fresh focused reviewer closes any findings. Real model inference/download/cancel, hardware suitability, clean-host installation, licensing/SBOM/provenance and exact installed upgrade remain separate gates. Reports distinguish implemented/tests from target and do not claim live-ready without real evidence.

## Source validation / review closure

Final parent `make test build` (DAGGER_NO_NAG=1, no cloud token, plain progress; cwd next) exited0;90 service/214 installer/5 GTK/65 Flutter/22 package tests and all three exports. Actual Xvfb tests cover scope-specific completion and sticky-cancel management guards. Initial aggregate hit200-second tool budget after partial exports; final run is retained as successful candidate evidence.

Reviewer7bd1ec84-3eaa-4bc3-b439-bc1f887b4d1e identified medium scope-overclaim, sticky-cancel rerun and stale-doc findings. Scope-specific reports, shared-sticky/GTK regressions and updated canonical policy closed all three with targeted source/fixture PASS (reviewer independently reran31 tests;53 initial focused tests passed). No full release/live approval. Source/candidate hashes, commands and pinned default-semantics research are in [evidence](LOCAL-SMITH-NEXT-MODEL-WIZARD-EVIDENCE.json) and next/VALIDATION.md.

No weights or installed app/service/state/manager/GUI/GPU/login/remote effects. The installed differing-service guard remains intentional. Live candidate upgrade,9GB download, inference/performance and operator QA remain separately approved work.
