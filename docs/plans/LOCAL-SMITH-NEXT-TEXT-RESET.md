# LOCAL-SMITH-NEXT-TEXT-RESET

## Intent and authority

User explicitly approved executing the recommendation: preserve voice as excluded reference; trim active product to text chat; remove the wizard/completion-state gate; separate recipient installation from the application; provision models outside app bundles by default. Elevated persistence/operator migration and installer capability source work. Repository smith, existing feature/autonomy, checkpoint04b72f5 plus uncommitted candidate. Source changes and isolated validation only: no installed-service upgrade, real manager/provider/download/inference, GUI, model/container/audio changes, commits or remote effects. Previous one-time upgrade approval is consumed and not reused.

Independent architecture:89710b55-7b56-45f0-8f33-47bcde6d84ab; combined independent producer/designer:c4b13cf9-cb23-4d31-bb4a-435729c5da47. Blocking concerns reconciled below; no majority vote.

## Bound archive before trimming

Copied source byte-for-byte into `reference/voice-chat-v1/`, outside every explicit next component build input. **46 files verified before edits**; manifest SHA256 `93f57832bd27d0a20f06d4432dba9a8bf2595a1c3f00cda6f5e48cce5f3bbf2d`. `MANIFEST.json` binds source provenance and hashes; README will explain that it is an excluded reference, not a shipping/runnable whole-product release. Captures client lib/tests/dependencies/docs, service src/tests/dependencies/docs, speech sidecars/Compose, Make/Dagger voice wiring. Excludes models, host data, generated outputs, credentials and unrelated legacy root/client. No git stash/branch/commit. Remove active voice files only after archive verification; preserve minimal schema-v1 compatibility decoders and generic durable/domain/transport code.

## Frozen text contract and compatibility

Keep schema1 Core and app envelopes, IDs32lowerhex, verified managed binary/unit/MainPID/peer/inode, existing root/file safety and unit bytes unchanged. Preserve 4MiB app and4095byte Core bounds, one bounded async worker, durable admission/dedup/tombstones, restart interruption/no replay,16pairs64KiB bounded context and32jobs1024tombstones/CAPACITY. Not an unlimited archive.

Live app operations only:
- state{} => config/history/active/providers/retention existing shape; no provider call.
- models{request_id} => async Ollama-only inventory, terminal `{models:[{name,digest}],voices:[],providers:{llm:{ready,error},stt:<static retired>,tts:<static retired>}}`. No speech network requests.
- configure{model:<exact observed tag>,voice:null} => model/revision saved; compatibility voice field inert.
- submit{request_id,text,output:"chat"} => text generation only, terminal `{assistant_text,audio:null,speech_error:null}`.
- poll{request_id,instance_id} => existing operation/status/result/error.

Separate legacy durable decoding from current admission. Valid voice-bearing v1 config/revision/history/jobs/inventories/IDs/tombstones must remain readable and bounded; completed old results remain pollable without audio recreation. All old running jobs reconcile interrupted without replay. Existing completed audio has already-expired representation; no WAV/base64 encoder/backend remains active. New transcribe/speak/speech-output requests explicitly retired before dispatch. Retained duplicates resolve original identity/status with no execution; conflicting IDs and tombstones still reject reuse. Actual corrupted/unsafe/unsupported state preserved, not normalized/reset. Never change installed unit text for cosmetic cleanup.

Fixed Ollama numeric loopback11434; no proxy/redirect/retry/cloud/tool or runtime/model provisioning in application. Remove base64 and all STT/TTS/audio execution deps/functions; compatibility structs accept bounded legacy nullable audio metadata only if necessary to read valid old files, not an executable provider.

## Conversation-first UI

Mount one dark green/black conversation screen for every launch. No Core Home, Core→LLM→Services, install/register/start/finish actions, microphone/playback/voice control or wizard. Read-only Core facts still protect connection; readiness ignores setup_complete boolean but retains integrity/identity validation. State/history/draft remain visible on outage, labeled stale/unavailable not fresh. Retry reinspects/synchronizes/queries uncertain work, never resends or provisions. One bounded inventory observation (manual Retry may repeat) can establish selected model availability without inference or auto retry. If no model is selected, show compact installed-model chooser + explicit Save; provisioning/model downloads stay in installer. Repair only Retry/Open installer, exact missing prerequisite message. Open installer is explicit bounded launch of a canonical paired installer command; no PATH/shell fallback, no auto launch. If executable absent or cannot open terminal/session, show actionable installer invocation rather than claiming it launched.

Remove audio.dart/voice_chat.dart and voice-only tests from active source after preservation; extract text controller no-replay/expiry/durability tests and generic native executor guards. Core native management API may remain for shared installer/helper contracts/tests but is never exposed as app actions. Keep legacy flag storage/API compatibility; it neither grants nor denies app chat readiness.

## Functional recipient installer

A separate stdlib Python **smith-installer zipapp**, code-owned under next/dagger/, packaged with complete paired native payload and a checksum manifest. Output `next/installer/dist/` contains executable `smith-installer`, `payload/client/`, `payload/service/smith-service`, manifest/checksums and operator README. No Dagger/Docker/Cargo/Flutter/pip required by recipient; Python3.12 and Ubuntu runtime dependencies required. No weights included. Dagger prepares zipapp/code/payload in isolated builder container and exports artifacts only; native publication remains outside container.

Wrapper reuses artifact-only install.py unchanged for consented publication. Source install.py remains accessible for developer component installs. Wrapper uses exact native helper inspect/prepare/confirm/apply for Register/reload/Start, not Python unit recreation. It exposes explicit exact-model selection through already-running loopback Ollama and optionally a separately confirmed pull/test. No invented best/default model. `--model EXACT` selects; `--pull-model` only requests an additional effect-specific confirmation if missing; `--verify-inference` additionally offers a disclosed fixed minimal direct inference test, not a persisted chat turn. Opening/inspection isn't consent. No global --yes bypass of lifecycle/model effects. Dry-run is artifact-only read-only, no locks/writes/manager/provider calls. Missing Ollama/user-manager/runtime libraries yield guided prerequisites, not sudo/install/start/fallback. Configure is an explicit saved preference action after verified inventory; it is not inference verification.

Model pull: stdlib direct numeric-loopback HTTP (no ambient proxies/automatic redirects), exact local tag validation, bounded NDJSON frames/progress, finite connection/idle/overall deadlines, no retry, fresh exact eligible model verification after success. Disclose external Ollama internet/disk and uncertain cancellation/cache outcomes; no cache deletion. Use existing app models/poll/configure contract for observed model selection. Native helpers get bounded stdin/stdout/deadlines/TERM11s/KILL1s cleanup and no prompt in argv. Artifact mismatch still blocks: no implicit upgrades or standing approval. Cloud/offline-pack/general OS/GPU provisioning future separate work.

**Reconciled limitation:** installer handles Smith app/service/user-unit runner and models via existing Ollama, not installation of Ollama/OS dependencies on an empty host. State this clearly in commands/docs. No claim that install/readiness succeeds without prerequisites or real inference evidence.

## Tasks / disjoint write sets

| Req/AC | Task/writer | Write scope | Evidence |
|---|---|---|---|
| R1 conversation first / flag ignored | T1 frontend | next/client/** | firstuse/flagfalse/return/outage, no app mutations, text-only/draft/correlation/layout |
| R3 durable safety / retired compatibility | T2 backend | next/service/** | voice-bearing v1 fixtures preserve history/jobs/IDs, no speech calls/deps, Core unchanged, no replay |
| R2 independent installer / consent | T3 installer | next/dagger/installer.py, next/dagger/tests/test_setup.py (new), no install.py/main.py edits | decline/dryrun/missingprereq/pull/manager identity/packaged code tests |
| R4 archive exact excluded | T4 parent | reference/**, next/sidecars deletion after verified copy, next/Makefile, next/dagger/src/**, README/VALIDATION/docs |46file checksumverification, no archive inputs/defaultvoicegate, zipapp pairedpayloadbuild |

All writers read this record and policy first. Archive edits forbidden except parent metadata. Remove retired default service test target and Dagger sidecar arg/functions. New installer/build/test through Dagger; aggregate includes installer orchestration tests and packaged entrypoint. No unneeded repository restructuring/frameworks/dependencies.

## Gates / rollout

Native targeted tests first; then make test and make build (includes recipient bundle), same compiled Rust↔Dart temporary fixture and packaged-installer validation without real services. Negative auth/identity/unknown fields, input/response bounds, legacy corruption and idempotency required. Independent fresh assurance/adversarial review; retained current source/artifact evidence. Parent canonical SCOPE ARCHITECTURE DESIGN PROCESS HOST_PROFILE assessment and product reset reflect active text-only direction while historical voice evidence remains intact.

Actual install→chat→close→reopen/provider and upgrade smoke is a separately approved live gate. The installed voice candidate remains unchanged by this source task; no implicit deployment or model/voice-data deletion. Source passes don't establish inference/GPU/clean-host licensing/release readiness. Models outside default appbundle and optional offline pack are explicit future distribution policy.
