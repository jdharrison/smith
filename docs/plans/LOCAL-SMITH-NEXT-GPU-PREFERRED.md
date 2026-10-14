# LOCAL-SMITH-NEXT-GPU-PREFERRED

## Status, intent and authority

**Active plan — first bounded source slice implemented, later milestones deferred.** User request on 2026-10-13/14: retain native CPU inference, prioritize useful GPU acceleration when available, and broaden the curated local model catalog. Scope is the native `next/` product on prepared Ubuntu 24.04 LTS x86_64; legacy clients/providers and concurrent multi-chat work are unchanged.

The user approved local source implementation and tests. That approval excludes host GPU probing, driver installation, permissions/groups/device changes, model weights, benchmarks, real inference, installed upgrade, service action, commits and remote mutation. Current branch is the preserved `feature/autonomy` bootstrap exception, HEAD `04b72f5` plus substantial pre-existing uncommitted work. Earlier host approvals are not reused.

Risk class: **elevated**, covering native driver trust, subprocess capabilities, backend selection/fallback, persistent preferences, packaging and operator reporting. Permission expansion would require separately scoped critical review.

## Implemented baseline and evidence

### 2026-10-14 first implementation checkpoint

**Implemented source:** paired SHA-pinned llama.cpp `b11429` CPU/Vulkan subtrees; CPU fixed `--device none --gpu-layers 0 --fit off`; Vulkan fixed `--device Vulkan0 --split-mode none --gpu-layers all --fit off`; strict single `offloaded X/Y` evidence requiring `X == Y > 0`; exactly one confirmed-cleanup startup/evidence fallback that returns the same held lease and retains the original deadline; no fallback after the Vulkan runner is accepted or completion dispatch can occur. Automatic partial offload, device discovery/classification, stable selection, preference/status schema and UI remain deferred.

**Verified artifact contract:** CPU archive size17693462/SHA256 `f6d25dde8f51133143d1453da4fd5f73b145127177612a283bf7995957af3392`; Vulkan size31636673/SHA256 `632c4e98feba2b94407a2130e3133e0c3aefb0ea1ab41337e926d8bfafdd0b74`. Selected ELF files use `$ORIGIN`; Vulkan adds `libggml-vulkan.so` size44315144 and host dependency `libvulkan.so.1`. This inspection and fake-runner validation are not live acceleration evidence.

- The service owns a lazy private CPU llama.cpp child; no external Ollama production dependency. See `next/service/src/runner.rs`, `docs/ARCHITECTURE.md#current-next-bundled-native-runner`, and the native-runner plan.
- `next/dagger/src/smith_next/main.py` packages separate SHA-pinned CPU and Vulkan llama.cpp `b11429` trees; `next/dagger/runtime.py` and package allowlists constrain exact files. Ordinary build validation executes only CPU `--version`, not the Vulkan backend.
- Existing limits: one runner/slot, held model FD, verified release/runtime, inherited lease, private Unix socket/peer checks, cleared environment, bounded threads, context4096/output1024, one180s cooperative operation budget, idle600s and joined bounded cleanup. Preserve these invariants; filesystem/driver calls are not hard real-time guarantees.
- `docs/HOST_PROFILE.md:7–30` records Ryzen9 7950X3D,61GiB reported RAM and AMD Navi31 family. Exact GPU model, usable VRAM, current driver/backend compatibility and performance are **unknown**. Earlier GRE/16GiB guesses must not drive sizing.
- Installer owns model/preferences; chat is minimal text-only and observational. No setup/install/model/acceleration controls belong in chat. The compiled catalog now has seven revision-pinned Apache-2.0 text models with Qwen3-14B retained as default.

## Requirements and acceptance criteria

All ACs below are blocking for the claimed first GPU slice unless explicitly marked deferred.

| Requirement | Acceptance / expected evidence |
|---|---|
| REQ1 GPU-preferred, CPU retained | AC1: compatible dedicated GPU is selected in Auto when a safe plan exists; CPU-only never initializes a GPU; absent/unsupported/unusable GPU selects independent CPU. Fake selection matrix and approved real GPU + CPU evidence. |
| REQ2 Honest device suitability | AC2: dedicated/integrated/unknown identity and memory are distinguished; shared memory is not called dedicated VRAM. Unknown values stay unknown. Detection does not claim speed or successful inference. Inventory fixtures and pinned backend verification. |
| REQ3 Full/partial offload | AC3: full offload preferred when within a reviewed budget; one conservative partial plan may be used when sizing controls are verified. Report actual offloaded layers, not desired values. Memory/plan fixtures and approved allocation/offload evidence. |
| REQ4 No replay fallback | AC4: at most one GPU-to-CPU preparation fallback before generation dispatch; confirmed cleanup, same lease/deadline. No fallback for integrity/identity/cancel/uncertain cleanup or after dispatch. Failure injection, call counts and lifecycle traces. |
| REQ5 Authoritative preference and diagnostics | AC5: installer alone exposes confirmed revision-checked preference changes; chat displays actual CPU/GPU/mixed/unknown mode compactly. Preference, planned and actual mode stay distinct; stale/last-run status is labeled. Persistence and widget tests. |
| REQ6 Maintain existing safety | AC6: held-model/runtime verification, request deduplication, restart interruption, private IPC, cancellation, context/history/draft guarantees and final-content validation remain. Existing and new aggregate regressions pass. |
| REQ7 Independent packaging paths | AC7: prepared no-GPU/no-Vulkan host can run the CPU executable; GPU code/dependencies are not loaded by that path. Verified manifests, dependency inspection, negative package tests and no-driver Ubuntu gates. |
| REQ8 No implicit host effects | AC8: opening/Refresh/Retry observes only; no inference/benchmark, driver/model download, permission repair or automatic install/start. Effect-call tests and separate live approval records. |
| REQ9 Optional measured integrated support | AC9: no automatic claim that integrated graphics beats or loses to CPU. Initial Auto excludes integrated/unknown types without approved workload-specific suitability evidence; explicit device choice is available after capability verification. Automatic benchmark ranking is deferred. |

## Execution policy

### Preferences (installer-owned, service-authoritative)

Proposed settings:

- **Auto — GPU preferred, CPU fallback allowed**: default for new installations and documented interpretation of an absent preference in legacy state; reading absence does not itself rewrite state.
- **CPU only**: independent CPU runtime, no Vulkan discovery/initialization required for inference.
- **Selected GPU, CPU fallback allowed**: optional explicit choice of one verified compatible device, including integrated graphics. This is a resource choice, not a guarantee of faster execution.

Use revision-checked service writes with exact displayed effect and explicit confirmation. Preserve saved model/history/IDs and preferences on installer reruns. Reject malformed/unsupported settings; do not silently reinterpret corruption. Re-resolve stable device identity each launch; an enumeration index alone is not persistent identity. A disappeared/ambiguous explicitly selected device falls back to CPU with a reason, never a different GPU. Preference changes during active work are rejected; apply at the next idle request boundary after the owned idle runner is stopped with confirmed cleanup. Never interrupt/resend accepted generation to apply a preference.

Proposed scope is one selected device and one backend, not multi-GPU splitting. For multiple eligible dedicated GPUs, choose greatest conservatively usable budget, tie-break by stable identity; expose chosen device/reason. Do not persist automatic selection as operator intent.

### Initial GPU backend

**Proposed first backend: Vulkan.** It is an AMD-relevant native path without requiring a ROCm stack as the baseline. This is a dependency/support choice, not a performance claim or confirmed compatibility with this host.

Retain separately verified CPU and Vulkan runtime subtrees/executables; missing GPU loader/driver must not break CPU. The concrete layout is frozen only after package review. Prefer the same reviewed llama.cpp revision for both; if `b11429` lacks necessary verified controls, review an explicit paired version update rather than guessing flags or mixing incompatible server contracts.

HIP/ROCm, CUDA, other operating systems, multi-GPU and runtime-downloaded backend plugins are excluded from the first slice. This does not claim other vendors are supported merely because Vulkan is cross-vendor. Advertise only tested device/backend combinations.

### Discovery and initialization

Service owns bounded discovery using a verified packaged probe/backend contract. No shell/PATH utility fallback or ambient executable/plugin search. A device observation may initialize a driver, but must not load model weights or dispatch inference. Installer artifact-only review/dry-run remains artifact-only; runtime discovery is lazy after authorized runtime access. App Retry observes cached service capability state, not an inference benchmark.

Retain cleared environment. Document any service-derived loader/ICD variables; reject ambient `LD_*`, `VK_*`, `LLAMA_ARG_*` and arbitrary HIP/ROCm selectors. Define reviewed system loader/ICD search policy: drivers are trusted host native code, not safe untrusted input or a sandbox. Do not indiscriminately bundle driver ICDs or assume `env_clear` prevents home-directory driver/plugin lookup. Pinned API/loader behavior is a P0 evidence blocker.

No sudo, group/device permission changes, `/dev/kfd` access, GPU container passthrough, new sockets/ports/egress or automatic driver repair. Existing allowed render-device access may be used only under a documented capability boundary. Unavailable access returns a reason and CPU eligibility, not an attempted repair. Discovery subprocesses have output caps, timeout/cancel and joined cleanup; loader/driver syscall latency limits must be honestly documented.

### Resource budget and actual mode

Sizing covers offloaded weights, KV cache, compute/staging buffers and reserve for the desktop/other workloads. GGUF file size alone is not required VRAM. Shared-memory graphics also consumes system memory; avoid counting shared capacity twice.

P0 must specify conservative numerical GPU/RAM headroom and verify candidate sizing controls before coding the automatic planner. No invented VRAM requirement or speed prediction. Unknown/insufficient budget selects CPU in Auto. Full offload if the reviewed plan fits; otherwise at most one validated partial-offload plan. No OOM-driven layer search, loops or silent model/context/output/reasoning changes. Competing host allocations can invalidate estimates; an allocation failure is not proof the GPU is unsupported.

Actual modes:

- **CPU**: zero confirmed GPU offload using CPU runtime.
- **GPU**: all model layers confirmed offloaded; this does not imply zero CPU work.
- **GPU + CPU**: partial model-layer offload confirmed.
- **Unknown**: evidence unavailable/incomplete; do not infer mode from device presence, flags, health or process startup.

Pinned structured backend evidence is preferred. If log parsing is necessary, it must be revision-bound, bounded, strict and separately tested; never forward raw environment/driver logs to UI. Zero actual offload after a requested GPU plan must not be reported as GPU success.

### Fallback state machine

1. Capture admitted model/config revision and one operation deadline; acquire/retain existing runner lease.
2. Verify runtime/model and observe capability. Select CPU or one GPU plan.
3. GPU preparation may perform allocation/loading and non-generating readiness/context work. Record dispatch fencing before any generation request bytes are written.
4. On classified compatibility/allocation/preparation failure **before** that fence: stop/join the GPU child, confirm cleanup, preserve the lease and original deadline; at most one independent CPU preparation is allowed if fallback policy/resources permit.
5. On dispatch begun, lost acknowledgement, device loss during generation, timeout or uncertain result: fail/interrupted/unknown under existing request identity; **never automatically replay on CPU**. Reconnect/Retry observes and polls only.
6. Cancellation, expired total budget, bad runtime/model identity/integrity, ambiguous peer or unconfirmed cleanup block fallback. No overlapping runner, forced socket cleanup or deadline reset.

Fallback is within one admitted operation, not a second submit and not a durable replay after service restart. It must leave no duplicate history. CPU failure ends the operation with truthful diagnostics. A later explicitly initiated new request may use CPU after a recorded GPU failure; no automated resubmission/backoff loop or indefinite GPU retry quarantine is introduced.

## Reporting and user experience

Chat remains text-only and minimal. One compact status may say `GPU · Vulkan`, `GPU + CPU · Vulkan` or `CPU · GPU unavailable`; detailed reasons are secondary/read-only, not a setup form. Before evidence exists use `Preparing · GPU preferred`; when unloaded show `Last run · …` or `Not loaded`, never a current execution claim. On disconnection label diagnostics stale.

Report bounded structured fields: configured preference/revision; capability observation and freshness; selected backend/device type and sanitized identity; requested/actual offload; current/preparing/last-run mode; classified fallback reason. No serials, sensitive paths, keys, prompts or raw logs. Do not create persisted telemetry beyond necessary existing operation diagnostics without schema/retention review.

Installer shows capability separate from saved preference and a service-backed check's actual mode. A successful CPU fallback is `Verified on CPU; GPU unavailable`, not `GPU verified`. Missing drivers receive actionable external guidance but no installation action inside chat. Optional comparative benchmarking requires separate explicit consent and no history/config mutation; defer it from initial delivery.

## Contract, compatibility and packaging gates

New preferences/reporting require a reviewed versioned durable schema and app/library contract. Current strict parsers do not justify simply adding arbitrary fields to schema1. P0 must choose version negotiation/compatibility handling across service, client, installer and any direct fixtures; preserve valid legacy model/history/jobs/tombstones, interrupt old active work without replay and reject unsupported state without reset.

Pair build artifacts and helper identity checks remain mandatory. Hash-pin source/artifacts, build flags, GPU libraries, licenses and host ABI requirements. Inspect CPU/GPU ELF dependencies/RUNPATH and establish approved native loader search paths. Update runtime manifests, `runtime.py`, client copy script, package allowlists, publisher validation and tests together. No unknown-file wildcard and no automatic relaxation of128MiB/file512MiB/tree limits to fit a GPU artifact; exceeding them is a separate reviewed scope decision.

Migration does not download models, alter unit/permissions, start work, replace a live runtime implicitly or discard data. Use the installer maintenance protocol and existing explicit upgrade/backup boundaries; its concurrent finalization work is not overwritten. Record exact candidate hashes and coordinated publication order before any installed upgrade. CPU rollback restores the prior compatible paired release/preferences from a documented backup after explicit approval, not an unreviewed binary swap.

## Independent inputs, disagreements and remaining blockers

Architecture: `c616cfb6-a266-487d-a0ad-a85fe41eba1a`. Independent producer/designer analysis: `6b41cca7-b329-48cf-b368-5c2d0068e8e3` (one fresh context, not two independent reviews).

| Concern / disagreement | Reconciliation / owner / expiry |
|---|---|
| GPU available vs GPU actually helpful | Auto prioritizes suitable dedicated devices; integrated/unknown default CPU unless validated or explicitly selected. Product owner confirms policy before implementation. |
| Vulkan vs HIP | Propose Vulkan-first; backend engineer verifies pinned artifact/API and host dependency contract at P0. HIP deferred, no speed claim. |
| Fallback vs no retry | Only one pre-generation preparation fallback, with dispatch fencing, confirmed cleanup and original deadline. Architecture/assurance must verify at P1. |
| Memory fit/performance unknown | No hard-coded VRAM/speed assumptions; backend engineer specifies measurable headroom/sizing at P0. Real QA separately approves evidence. |
| Frozen wire vs extra settings/status | Contract/migration owner resolves version compatibility at P0. Unresolved choice blocks implementation, not silently-added fields. |
| Minimal chat vs visible mode | Compact read-only execution status; installer owns configuration. Designer verifies enlarged-text and stale-state flows at P1. |
| Discovery vs inference consent | Observe only, no automatic warm-up/benchmark/generation. Driver initialization trust/effects documented separately. |

These are review obligations, not claims that supporting APIs, hardware or release evidence already exist. No majority vote removes a blocker.

## Prioritized tasks and traceability

| Task / priority | Dependencies and scope | Definition of done / evidence | REQ / AC |
|---|---|---|---|
| TASK1 P0 backend/host contract | Read-only research; exact pinned candidate, loader paths, structured discovery/offload, device identity, numerical memory reserve; host checks only after bounded approval | Verified API/dependency design; unknowns resolved or scope reduced, no guessed args |1–3,7–9 /1–3,7–9 |
| TASK2 P0 state/compatibility design | TASK1; service app/preferences/provider contracts, installer/client strict parsers | Versioned migration/reporting/dispatch fence frozen; architecture acceptance |4–6 /4–6 |
| TASK3 P1 runner policy | TASK1–2; `next/service/src/runner.rs`, providers/app and tests | CPU/GPU selection, lease/cleanup, budgets, no replay and status tested with fake devices/children |1–4,6,8–9 /1–4,6,8–9 |
| TASK4 P1 paired packaging | TASK1–2; Dagger/runtime.py/package.py, client build script, installer publisher validation | Independent CPU/GPU artifacts, exact allowlists/dependencies/licenses; no-Vulkan CPU gate |7–8 /7–8 |
| TASK5 P1 installer/status UI | TASK2–4; installer preference/check flows and client observational status | Revision/consent-safe choices and accurate current/last/stale mode; no setup in chat |5,8–9 /5,8–9 |
| TASK6 P1 self-check and assurance | TASK3–5; canonical docs and evidence | `make test build`, failures/negative probes, hashes, fresh adversarial review; no unreviewed host effects |1–9 /1–9 |
| TASK7 P1 separately approved live QA | TASK6 + exact human authorization for candidate/model/device/effects | GPU and independent CPU complete inference, actual offload/headroom/resource evidence, cleanup and graphical responsiveness; no fabricated performance |1–3,6–7 /1–3,6–7 |
| TASK8 P2 comparative benchmark/HIP | Separate future plan and explicit consent | Representative latency/prompt/decode throughput, peak RAM/VRAM, desktop impact; no automatic config/history changes |9 / deferred automatic ranking |

Suggested milestones: M0 resolve contracts/approval; M1 service+packaging isolated candidate; M2 installer/client integration and source assurance; M3 separately approved installed GPU/CPU QA. Separate commits/PRs only if explicitly authorized, with one coherent compatibility plan.

## Stage gates and validation

**Plan → develop:** user approves bounded implementation; P0 API/memory/trust/schema blockers resolved; relevant canonical SCOPE/HOST_PROFILE/ARCHITECTURE/DESIGN/PROCESS updated as target, not implementation guarantees.

**Develop → self_check:** isolated fake-device tests cover no GPU, missing Vulkan loader/driver, permissions denied, dedicated/integrated/unknown, reordered indices, disappeared device, full/partial/zero offload, OOM, deadline/cancel/uncertain cleanup and environment/plugin injection. Assert at most one fallback, generation dispatch count≤1, no CPU replay after lost response, no history duplication and no overlapping runners.

**Self_check → assurance:** run `DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make test build` from `next/` on the implemented snapshot. Require existing Rust/Flutter/GTK/package/native runtime gates plus new CPU-independent/backend gates; no GPU devices passed to ordinary container validation. Run strict migration tests, model-FD/runtime-integrity/peer/lease/teardown regressions, idle preference transitions and enlarged-text/stale diagnostics tests. Capture exit status, source/artifact digests and exact commands. A mock GPU cannot prove real offload.

**Assurance → release consideration:** fresh architecture/adversarial findings closed, separately approved real GPU/CPU QA and clean prepared-host evidence, dependency/license/provenance review, candidate-bound human signoff. Benchmark tables must distinguish prompt processing, time to first visible final answer, decode throughput, complete-answer latency and memory; current reasoning/final-only behavior can delay visible output. No numerical performance promise before measurement.

No release/deployment when required evidence is missing, required tests fail or a blocking concern remains. Optional benchmarking/HIP work is deferred to the future backend task owner, due before claiming comparative performance or those backends' support; it is not required to advertise only verified Vulkan capability.

## Review checklist and implementation handoff

- Confirm CPU remains functional without GPU dependencies and Auto policy matches operator intent.
- Confirm actual mode/offload evidence, classified fallback and stale labels; no setup controls in chat.
- Confirm version/migration, runtime/device trust and memory plans are explicit and reviewable.
- Confirm no dispatch replay, concurrency/deadline/cleanup regressions or implicit permissions/driver effects.
- Preserve concurrent installer work; coordinate exact paired artifacts and rollback.
- Resolve P0, obtain scoped implementation authorization, implement disjoint tasks, record self-check, obtain fresh assurance, then stop before live QA/upgrade until separately approved.

**Validation of this specification:** repository/source inspection and independent analyses; `git diff --check` exited0 and a local documentation check confirmed REQ1–9/AC1–9 are present. Fresh assurance `21d16b70-0e3f-4e5b-af21-ef52b2df717c` returned **scoped specification PASS**, finding no blocking contradiction in the spec or canonical proposal sections. This is not backend verification or implementation authorization. No GPU or implementation test was run. This file is an implementation handoff proposal, not a completed capability.
