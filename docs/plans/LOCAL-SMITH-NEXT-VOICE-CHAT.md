# LOCAL-SMITH-NEXT-VOICE-CHAT

## Intent / authority

User requests functional rough speech:speech, speech:chat, chat:speech and chat:chat with a model. Local worktree implementation and isolated validation; no installed binary replacement, live manager/container/provider operation, microphone capture, real model download, publication or commit by agent. Ubuntu desktop only. Base checkpoint `04b72f5` plus uncommitted next work. Preserve legacy files and schema-1 Core lifecycle contracts. Operator later performs separately reviewed normal-host setup.

## Reconciled architecture

Independent architect `4aae7a3c-21cf-49ca-9b2f-dc6e5fab2b5f`; producer/designer `b88fcff3-0cfb-4108-8fff-a3fd07bcba6b`. Native Rust remains authoritative; desktop captures/plays audio. Local Ollama numeric loopback 11434; new STT Whisper 4890 and TTS Kokoro 4891 distinct from legacy ports. No cloud, search execution, tools, provider start, model pull, autorun or container core migration. Search setup stays planned. Compose packages only voice providers with separate internal networks, read-only scoped model mounts, loopback ports, nonroot, dropped caps/no-new-privileges, bounded tmpfs/resources and no Docker socket/host audio/home mounts. No automatic weight downloads at build/start/inference.

**Durability reconciliation:** architecture initially suggested process-local history; designer identified conflict with durable-chat target. Implement a bounded atomically synced app-state file instead: persisted preferences and up to 16 completed turn pairs/64 KiB context, plus bounded operation identity/status metadata sufficient to avoid replay and mark unfinished work interrupted after restart. Do not promise full archival transcript, unlimited history, persisted audio or automatic operation resume. Audio is transient. New service instance on restart; client synchronizes history, never silently retries old IDs. Corrupt/unsupported state preserved and reported. App state separate from debug state.json; no reinterpretation of debug completion as full provider setup. If implementation cannot satisfy this gate, report blocker rather than silently downgrade.

## Frozen application contract

Separate private `app.sock` served by the same managed PID; 0700 namespace, 0600 socket, checked inode/UID/PID analogous to control socket. Existing `service.sock` limits unchanged. `smith-service app` accepts one bounded JSON line on stdin, returns one JSON line, verifies installed helper/unit/MainPID and app peer before request. No prompts/audio in argv. IDs are 32 lowercase hex characters (not hyphenated UUIDs).

Request: `{"schema_version":1,"service":"smith-next","protocol":"app","call_id":"<id>","op":"state","args":{}}`.
Reply: `{"schema_version":1,"service":"smith-next","protocol":"app","pid":123,"instance_id":"<id>","call_id":"<id>","ok":true,"result":{},"error":null}`. Error: ok false/result null/error `{code,message}`. Reject unsupported schema/enums/unknown request fields. JSON payload <=4 MiB excluding newline, buffered reads, finite exchange deadlines, no provider calls inline in socket handler. Provider worker single active/no queue; state/poll remain responsive.

Operations:
- `state {}` → `{config:{model:<string|null>,voice:<string|null>,revision:<int>},history:[{role:"user"|"assistant",content:<string>}],active:<operation|null>,providers:{...},retention:{...}}`; history in ordered complete pairs. Provider observations cached, not a provisioning check.
- `models {request_id}` → admitted operation; worker checks `/api/tags` and speech health; terminal result `{models:[{name,digest}],voices:[<string>],providers:{llm:{ready,error},stt:{ready,error,model},tts:{ready,error}}}`. Missing speech does not fail LLM inventory.
- `configure {model,voice}` → persisted config with new revision. Model exact observed installed tag; voice observed or null when TTS unavailable. No inference/start/download. Config changes blocked during work.
- `submit {request_id,text,output:"chat"|"speech"}` → admission with request_id/status/phase/config_revision. Poll terminal result `{assistant_text,audio:<audio|null>,speech_error:<error|null>}`. If generation succeeded but TTS failed/oversized input, retain text, succeed with speech_error, never regenerate.
- `poll {request_id,instance_id}` → operation `{request_id,kind,status,phase,config_revision,result,error}`. Status running/succeeded/failed/interrupted; phases inventory/transcription/generation/synthesis/complete. Instance mismatch fails, no silent resend.
- `transcribe {request_id,audio:{mime:"audio/wav",data_base64}}` → admission then terminal `{text,language,duration_ms}`.
- `speak {request_id,text}` → admission then terminal `{audio}` for deliberate replay without LLM generation.
Audio result `{mime:"audio/wav",sample_rate:24000,data_base64}`. Input WAV mono16kHz PCM16 <=60s and <=2MiB. Output WAV mono24kHz PCM16 <=60s and <=2,880,044bytes. Text16KiB, model result64KiB, TTS input4KiB (reject/no silent truncation). No raw audio persisted/logged. Single active + bounded retained terminal jobs, explicit capacity/expiry not unknown resurrection. Dedup identical request_id+fingerprint; changed input ID_CONFLICT; audio input fingerprint only. Persist accepted turn metadata before acknowledgement, completion + history coherently. No replay on restart. File constraints reuse existing safe filesystem helpers, state0600.

Provider HTTP: reqwest0.12 blocking/json no default features + base640.22; fixed numeric loopback destinations, no proxy/redirect, bounded bodies, 2s connect and finite inventory10/STT90/LLM120/TTS60 second budgets. Exact local tags, reject remote/cloud metadata, no substitution; text-only `/api/chat` stream false, no tool calls. Context/tokens bounded. Timeout does not guarantee external inference stopped; no automatic retry. Tests inject providers/transport at library-only seams, no ambient production fake endpoint override.

Sidecars:
- `GET /v1/health` → `{schema_version:1,service:"smith-next-stt"|"smith-next-tts",ready,model:<string|null>,voices:[...],error_code:<string|null>}`.
- STT `POST /v1/transcribe` raw bounded audio/wav → `{schema_version:1,service:"smith-next-stt",text,language,duration_ms}`.
- TTS `POST /v1/speak` strict `{schema_version:1,text,voice}` → raw bounded audio/wav.
- Whisper explicitly mounted CTranslate2 local model directory, local_files_only. Kokoro mounted ONNX+voice file, observed voices (no invented fallback). Missing weights factual not-ready, no network fetch. Audio bytes only, no paths. Busy returns immediately, bounded reads with no Content-Length trust, redacted errors/logs.

## UI contract

Keep Matrix theme and existing Core setup. Make local model/voice selection real in LLM/Services via explicit provider refresh/config save. Home hosts conversation/composer. Shared draft: Type/Microphone choices plus independent Text/Text+speech output. Record/stop/transcribe/edit/explicit Send; no automatic send. Draft revision guards late transcription; cancel preserves earlier draft. State visible recording/transcribing/generating/synthesizing/playing/unavailable. Request IDs retained through uncertain acknowledgements; query not resend. Speech-off stops/suppresses current/pending playback; no autoplay on reopening or switching speech on. Assistant text always selectable, TTS/playback failure cannot remove answer or repeat inference. Closing cancels only captured desktop audio/helper children and polling, not service work.

Native audio uses fixed `/usr/bin/parecord` and `/usr/bin/paplay` via Dart stdlib, no shell/PATH. Requires pulseaudio-utils and usable PulseAudio-compatible user session (including PipeWire Pulse). Private transient owned client files bounded duration/bytes, cleanup captured processes finite TERM/KILL, do not delete unknown paths. Explicit recording authorizes capture, explicit speech output/replay authorizes synthesis/playback. Missing audio utilities do not block chat:chat. No global shortcut/always listening yet.

## Tasks / disjoint ownership

| Task | Writer | Files | Evidence |
|---|---|---|---|
| T1 Rust application/domain/providers/persistence/bridge | backend | next/service/** | async responsiveness, durability/restart/dedup, bounds/identity/provider negative tests |
| T2 Flutter app API/composer/capture/playback/setup | frontend | next/client/** | all4 paths, edit/send/switch/cancel, reattach, missing speech, lifecycle regressions |
| T3 voice sidecars/Compose/model instructions | voice | next/sidecars/** | raw bytes API, bounded WAV, missing model/busy tests, manifest review |
| T4 Dagger/Make/docs/integration | parent | next/Makefile next/dagger/** docs/** next/README.md next/VALIDATION.md | make test/build includes voice tests, contracts interoperability, fresh review |

Acceptance: all4 mode wiring with fakes and compiled service/client where possible, no auto send/retry, text-only survives speech outage, generation+failed TTS retains text, history/preferences survive restart with unfinished jobs interrupted, no tool/cloud/download capability, controller approvals regressions remain. Actual microphone/providers/containers need separately approved operator smoke. A source pass is not real speech/inference evidence. Current installed-service upgrade block remains; do not weaken installer. Document exact pending deployment/model provisioning steps.
