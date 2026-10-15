# LOCAL-SMITH-NEXT-GPU-STARTUP-FIX

## Intent and authority

**Approved local source task; implemented, source gates passed, independent source assurance PASS.** The operator approved “go ahead” in the current thread after the proposal to correct Vulkan logging, add complete-record regressions and preserve a bounded startup-fallback cause. This is a bounded amendment to [GPU-preferred](LOCAL-SMITH-NEXT-GPU-PREFERRED.md), not completion of that plan's deferred milestones.

- Repository: Smith; existing bootstrap branch `feature/autonomy`; inspected base `2cc71a94fe924fbb9d324f29332e9f24c8c5991c` plus pre-existing local changes, especially UI/multi-chat. Preserve all unrelated work; no new branch or commit.
- Risk: elevated subprocess/evidence/fallback implementation. Source authority expires at completion of this source task; changed scope requires fresh approval.
- In scope: Vulkan logging arguments, bounded startup framing/admission, provider-owned sanitized fallback-cause retention, synthetic tests, relevant canonical docs and source evidence.
- Excluded: installed artifact/state changes, service actions, further host GPU probes, model loading/inference/benchmark/download, driver/device/permission changes, new dependencies, UI/wire/storage schema changes, device selection/partial offload, commits and remote effects. Ordinary configured locked build/test tooling is permitted; it must not access GPU devices or model weights.

## Established diagnosis and evidence boundary

**Observed in the preceding separately authorized non-model investigation:** the installed paired service/runtime matched its release manifest; all 26 Vulkan runtime files passed manifest hashes. The packaged `llama-server --list-devices` with a cleared environment enumerated `Vulkan0` as RX 7900 GRE (RADV NAVI31), 16368 MiB reported device memory and approximately 14 GiB free at observation. System Vulkan reported Mesa 25.2.8; existing render-node access was available. These are transient capability observations, not model-fit, acceleration or performance evidence. The then-running model child used `--device none --gpu-layers 0`.

**Verified pinned contract:** upstream llama.cpp `b11429` emits the model offload record through library INFO, mapped by `common_log_default_callback` to verbosity 4 while the default threshold is 3. `common_init()` enables timestamps and severity prefixes. The installed device-only probe at verbosity 4 produced decorated backend records; adding `--no-log-prefix --no-log-timestamps` produced plain records. The old Smith launch did not override these defaults but required an exact unprefixed `load_tensors: offloaded X/Y layers to GPU` record. This establishes a default-contract defect, not that the previous real model startup had no other failure.

Pinned references: [model emitter](https://github.com/ggml-org/llama.cpp/blob/b11429/src/llama-model.cpp), [library log macro](https://github.com/ggml-org/llama.cpp/blob/b11429/src/llama-impl.h), [callback and output](https://github.com/ggml-org/llama.cpp/blob/b11429/common/log.cpp), [verbosity levels](https://github.com/ggml-org/llama.cpp/blob/b11429/common/log.h), [initialization](https://github.com/ggml-org/llama.cpp/blob/b11429/common/common.cpp), [arguments](https://github.com/ggml-org/llama.cpp/blob/b11429/common/arg.cpp).

## Independent inputs and reconciliation

- Architect session `1d3e700f-2cd8-4e9c-9dd1-d977e0cb8fe6`: Vulkan-only explicit logging; CPU unchanged; complete newline-terminated records, sticky raw-byte cap, explicit bounded admission window, cancellation/deadline/liveness checks; provider-owned fixed diagnostics without raw stderr or wire changes.
- Producer session `c6b7f0eb-3c0d-4026-b676-5ea1d052f2d3`: realistic logger fixture must fail with old flags; preserve strict evidence, safe fallback/no replay, sanitized reason through CPU success/failure, and honest source-only evidence.
- Designer session `9669b591-a317-470b-b0b2-f83a8d467e0a`: an internal accessor suffices only if explicitly described as internal retention. `None` is not GPU verification; idle history is not current execution status. Actual-mode UI and operator-accessible logs remain deferred.

| Gap/disagreement | Reconciled decision |
|---|---|
| Producer asks inspectable cause; no existing operator diagnostic surface | Add a nonblocking, read-only Rust accessor for one last-startup cause; explicitly no UI/log-access claim. Broader GPU plan REQ5/AC5 remains deferred. |
| Architect prefers typed reasons; existing launch APIs use `AppError` | Keep the existing internal error type and code-based fallback eligibility; generate only finite fixed diagnostic messages, never forward child output. No new wire fields. |
| A complete line is not a flush barrier for an open pipe | Keep a candidate provisional through the fixed at-most-one-second post-health observation window, then assess a locked snapshot. Incomplete records cannot succeed; future stderr beyond that boundary is not covered. |
| Upstream may consult configuration despite cleared environment | Explicitly normalize supported Vulkan logging controls, including disabling JSONL. This is not host-config isolation or a sandbox claim. CPU args and host configuration remain untouched. |
| Higher verbosity may exceed the existing evidence cap | Preserve the 64 KiB raw startup-prefix cap and fail closed on overflow. Do not silently enlarge buffers or discard earlier evidence. |

No blocking disagreement remains for this bounded source design. These analyses are not final assurance or human approval of deployment.

## Implementation contract and traceability

| ID | Requirement / implementation | Required evidence |
|---|---|---|
| AC1 | Vulkan explicitly uses verbosity 4, no prefix/timestamps/JSONL; CPU args unchanged | Exact argument tests; fake models pinned suppression and decoration instead of unconditional canonical output |
| AC2 | Complete LF-terminated records only; exact one positive full-offload record; all raw bytes bounded | Every-byte split tests; unterminated/partial/zero/malformed/duplicate/decorated/invalid/overflow negatives; delayed/fragmented startup integration |
| AC3 | Fixed post-health observation window within original deadline; cancellation/liveness/capture failures fail closed | No premature acceptance; duplicates/overflow arriving during window rejected; timeout/cancel tests |
| AC4 | Existing one-fallback/confirmed-cleanup/same-lease/no-replay invariants unchanged | Existing fake lifecycle matrix plus fresh success/failure/CPU independence assertions |
| AC5 | Last-startup cause survives CPU success/failure, reuse and idle; resets at fresh preparation; busy accessor cannot block | Finite literal-only errors; accessor lifecycle and hostile-output tests; unchanged wire/state contracts |
| AC6 | Source checks and independent review; documentation accurately distinguishes source, host capability and real acceleration | Formatting, Clippy, locked service tests/build; relevant dependent checks; fresh assurance; snapshot-bound results |

The fallback accessor returns no retained cause before preparation or after a fresh preparation with no eligible fallback; this never establishes GPU success. It does not probe hardware, retry, infer or mutate state. Original CPU result/error remains authoritative. Retention is one service-process-memory value, lost on provider teardown/restart, not durable telemetry. Unknown cleanup/integrity/identity/cancellation/deadline errors remain non-fallback errors.

## Validation and rollout boundary

Start with focused parser/runner tests, then the configured Dagger service gate (`make service/test` from `next/`) and locked build. Run relevant dependent installer checks where practical without installed-host effects. Retain exact results and failures; do not replace a failed gate with earlier history. Obtain independent assurance/adversarial review from a fresh context after self-check.

No migration or rollback is needed for source-only edits. Future deployment requires a freshly built compatible paired release, bound human approval and the existing maintenance/backup flow; do not replace a live runner or revert unrelated work. Real model offload, successful GPU inference and measured performance remain separately approved and unverified.

## Subsequent paired-candidate build approval

**Approved local artifact preparation, results pending:** after source assurance PASS, the operator answered “yep” to building the corrected paired installer next, with upgrade and actual GPU inference explicitly deferred. This authorizes the existing `make installer/build` and `make package/test` workflows, local generated outputs under `next/installer/dist`, non-executing package/checksum validation and candidate-bound evidence. It does not authorize installer launch, deployment, service operations, installed-state migration, GPU probing/inference, model weights, driver/permission changes, commits or remote publication. Approval ends at completion of this candidate preparation; later effects require fresh exact approval.

The candidate uses the current working tree on the recorded `feature/autonomy` base, including existing UI/multi-chat work; it is not a GPU-only release or a clean-source production release. No unrelated source changes are made. Preserve the earlier source review and independent product/architecture/design constraints above. Record exact build inputs before/after, verify the reviewed GPU service sources are unchanged, bind package/service/runtime hashes, and obtain independent artifact verification. Source drift or failed checks must be reported, not silently attributed to a stable validated candidate. Broader graphical/client acceptance and previous aggregate failures remain separate from the bounded packaging claim.

## Results

**Implemented:** Vulkan logging normalization; complete-record parser; bounded post-health admission and capture failure/EOF handling; fixed-message provider-owned fallback history with a nonblocking accessor. CPU launch, selected device, model/runtime identity, fallback eligibility, cleanup, original deadline, and no-replay policy are unchanged. Canonical architecture/process/design and the separate host capability observation have been updated.

**Self-check before final gate:** 13 parser and 3 capture unit tests passed; 40 runner integrations passed, with two repeated 5-test Vulkan and 4-test diagnostic groups. Independent review `1ec9c7f2-3b3f-4219-8b5e-4ad75b298147` found no blocking implementation issue, but required retained final gate evidence and suggested a permanent continuous-output/non-sliding-window test. That regression was added and passed. The review's optional 90-second wrapper timed out during an accidentally repeated test after its first ten checks passed; that repetition is not counted as passing.

**Retained gate failure, not hidden by earlier passes:** the subsequent full isolated service gate passed formatting/Clippy and all other tests, but the new idle-retention test observed `RUNNER_BUSY` instead of its expected model-integrity error. [Initial evidence](LOCAL-SMITH-NEXT-GPU-STARTUP-FIX-EVIDENCE.json) retains the stable source snapshot and failed output. A fixture-only correction now observes actual lease availability after explicit idle cleanup, with a five-second bound, before asserting the unrelated corruption error. Persistent contention still fails; production never retries. Source inspection and a synthetic fork/exec lock probe support transient cross-test inherited-descriptor contention, but the failed run did not identify its lock holder, so this is not definitive historical attribution. The exact integrity-error and no-new-launch assertions were preserved. Focused rerun passed, then all 41 runner tests passed.

**Dependent gate:** isolated `make installer/test` passed 252 native and 9 GTK tests, one controlled ignored probe, formatting/Clippy and no-Python CLI checks. [Installer evidence](LOCAL-SMITH-NEXT-GPU-STARTUP-FIX-INSTALLER-EVIDENCE.json) retains unchanged before/after input hashes. The only later service change is test-fixture code, not a dependency of the installer build.

**Final source gates PASS:** `DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make service/test` passed formatting, warning-denied Clippy and 169 tests (one existing controlled probe ignored), including all 41 runner tests. `cargo build --release --locked --offline` passed. [Final evidence](LOCAL-SMITH-NEXT-GPU-STARTUP-FIX-FINAL-EVIDENCE.json) retains exact commands, sanitized log hashes and unchanged before/after source snapshots. The permanent continuous-output window regression and fixture-only lease precondition are included in that final gate. The earlier failed gate remains retained separately.

**Final independent assurance PASS, source scope only:** [the fresh-context review](LOCAL-SMITH-NEXT-GPU-STARTUP-FIX-REVIEW.md) verified final source/log identities and closed all source-scope findings. Its final evidence-recipe wording correction changes no snapshots, logs or results; the prior JSON identity is retained in the correction note. The intermediate failure and optional reviewer timeout remain accurately recorded. No deployment, installed upgrade, real GPU inference, release or accepted-risk decision is authorized by this source recommendation.
