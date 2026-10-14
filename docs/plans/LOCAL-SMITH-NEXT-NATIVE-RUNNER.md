# LOCAL-SMITH-NEXT-NATIVE-RUNNER

## Intent and authority

User explicitly approved replacing the external Ollama dependency with a bundled native llama.cpp runner owned by the Smith service ("awesome do it please" after the bundled-runner proposal). Local-first; no cloud fallback. Source implementation and isolated validation only: no installed app/service/state change, no real user-manager actions, no GUI launch, **no model-weight download on this workstation** (including inside local builder containers), no commits/remote effects. Prior host approval is consumed. Branch `feature/autonomy`, base `04b72f5`, extensive uncommitted `next/` work preserved.

Elevated: new child-process capability, new local IPC endpoint, network egress for explicit model download, new persistent model storage, provider replacement. Requires architecture review, focused tests and one scoped independent review.

## Verified facts (2026-10-13)

- llama.cpp `v0.6.0` stable points to binary assets `b11429` (GitHub-marked prerelease). Pinned asset `llama-b11429-bin-ubuntu-x64.tar.gz`, 17,693,462 bytes, SHA256 `f6d25dde8f51133143d1453da4fd5f73b145127177612a283bf7995957af3392`, MIT license.
- In clean `ubuntu:24.04`, `llama-server --version` reports `0.6.0-dev (build 11429, commit d81235049)` with an empty environment from a private directory of 21 regular files (~40 MB): `llama-server`, `libllama-server-impl.so`, `libllama-common.so.0`, `libmtmd.so.0`, `libllama.so.0`, `libggml.so.0`, `libggml-base.so.0` and all `libggml-cpu-*.so` variants. RUNPATH is `$ORIGIN`. Host must supply glibc/libstdc++/libgcc/libssl3; `libgomp.so.1` is missing on base Ubuntu and is bundled from Ubuntu 24.04 `libgomp1` (GCC runtime library exception).
- `--host` accepts a UNIX socket path ending `.sock`; `--api-key`/`LLAMA_API_KEY`; `--offline`; `--no-webui`; default no built-in tools; many `LLAMA_ARG_*` environment overrides exist, so the child environment must be cleared.
- Curated Apache-2.0 GGUF models (HF metadata only, no download): `Qwen/Qwen2.5-1.5B-Instruct-GGUF@91cad51170dc346986eccefdc2dd33a9da36ead9` `qwen2.5-1.5b-instruct-q4_k_m.gguf` 1,117,320,736 bytes SHA256 `6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e`; `Qwen/Qwen3-4B-GGUF@bc640142c66e1fdd12af0bd68f40445458f3869b` `Qwen3-4B-Q4_K_M.gguf` 2,497,280,256 bytes SHA256 `7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5`. Revision-pinned `resolve` URLs 302 to `*.hf.co` CDN hosts.

## Architecture

> **Historical baseline:** the CPU-only packaging/runtime details in this section are superseded for current `next/` source by [LOCAL-SMITH-NEXT-GPU-PREFERRED](LOCAL-SMITH-NEXT-GPU-PREFERRED.md). They remain the evidence record for the initial native-runner slice.

Smith app → Smith service → private `llama-server` child → local GGUF file. CPU backend first; Vulkan/ROCm acceleration was deferred at this stage.

- **Runtime location:** runner files ship in the verified immutable client release at `smith_backend/runtime/` (published by the existing native publisher). The service resolves `XDG_DATA/smith-next/client/current` → `releases/<64-hex>/smith_backend/runtime/llama-server` with no-follow ownership/mode/single-link checks. Developer `serve` uses the same resolution.
- **Models:** `XDG_DATA/smith-next/models/<sha256>.gguf` (0600 regular, owner, single link) plus completion marker `<sha256>.json` written last. Canonical compact sorted JSON + newline: `{"file":"<sha256>.gguf","license":...,"name":...,"schema_version":1,"sha256":...,"size":N,"source":...}`. Names use the existing `name:tag` alphabet. The service lists at most 64 valid records, checks file facts (size/owner/mode/link) on inventory and full SHA256 once per runner start.
- **Process ownership:** service spawns the child lazily on chat for the selected model, `env_clear`, cwd = runtime dir, stdin/stdout/stderr null, `PR_SET_PDEATHSIG(SIGKILL)` in its own process group, args fixed: `--model <file> --host <XDG_RUNTIME/smith-next/runner-random/llama.sock> --offline --no-webui --ctx-size 4096 --parallel 1 --threads <bounded> --timeout 300` with a fresh random API key passed via `LLAMA_API_KEY`. Socket directory is the existing private 0700 runtime namespace; stale runner socket owned by the service may be removed only after its own child exited. Bounded readiness (`/health`, 120 s load budget), chat via minimal bounded HTTP/1.1 over `UnixStream` to `/v1/chat/completions` (`stream:false`, `max_tokens` 1024), validated content-only response; reasoning/tool fields ignored/rejected; no retry. Model switch or crash → stop/restart on next request. Idle stop after 10 minutes. Service shutdown/Drop: SIGTERM, bounded wait, SIGKILL; unit cgroup is the backstop.
- **Provider wire:** `Inventory` schema, app ops and client contract unchanged: models = installed records (name, sha256 digest); `providers.llm.ready` = verified runtime present (error codes `RUNTIME_MISSING`/`RUNTIME_UNSAFE`). The Ollama HTTP provider and fixed port 11434 are removed from the service.
- **Installer:** curated catalog compiled in; `--model NAME` selects, `--download-model` offers one exact-name-typed consent to download that catalog entry over HTTPS (redirects ≤3, https only, hosts `huggingface.co` or `*.hf.co`, no proxy env, bounded idle/total time), streamed to a private staging file in the models dir with incremental SHA256 + exact size, fsync, no-clobber rename, then marker. Interrupted/failed downloads leave only a staging file reported for cleanup; no automatic retry. `--verify-inference` starts a private runner instance via the service library with a separate socket, one bounded completion, then stops it. Separate preference consent remains. GUI model step is deferred to the installer-wizard redesign; GUI stays model-free in this slice.
- **Packaging:** Dagger client build downloads only the pinned runner archive (SHA-verified) and Ubuntu `libgomp1`, copies the exact 25 runtime/library/license files (materialized, no symlinks) into the bundle; `package.py` allowlists exactly those paths; publisher limits unchanged (runtime ≈ 41 MB).

## Architecture review amendments (binding)

Architect `893fac23-e721-4c74-8204-64dabed991cf` returned no-go until these were adopted; they supersede conflicting text above.

1. **Context fit:** service trims oldest complete turn pairs (in the request only, durable history never rewritten) until a conservative budget fits ctx 4096 with `max_tokens` 1024: tokens counted via runner `/tokenize` on the rendered messages (or a conservative byte-token upper-bound fallback). A single unfittable prompt returns `CONTEXT_LIMIT`.
2. **One deadline:** one end-to-end service budget ≤180 s from dispatch covering verify, load and generation (client budget is 210 s). On expiry the runner is stopped, never reused mid-generation.
3. **Reasoning/tools:** start with `--reasoning-budget 0` and `--alias <sha256>`; ignore `reasoning_content`; reject any `tool_calls`; response `model` must equal the alias.
4. **Single runner:** 0600 `flock` at `XDG_RUNTIME/smith-next/runner.lock` held for each runner's lifetime by service and installer verify; contention → `RUNNER_BUSY`, no wait/retry. Bounded `--threads`/`--threads-http`.
5. **Socket:** fresh 0700 `runtime/smith-next/runner-<random>/llama.sock` per runner, removed after child exit. Leftover `runner-*` dirs removed only under `runner.lock` after lstat type/UID/inode checks and ECONNREFUSED probe. Each connection checks `SO_PEERCRED` pid == child pid. API key only in env (documented as weak against same-UID processes).
6. **Child cleanup:** runner spawned/owned only by the service app worker thread (installer: its orchestration thread); reuse the existing parent-death guard pattern with **SIGKILL** + own process group; stop = TERM group, ≤2 s, KILL, ≤1 s (fits `TimeoutStopSec=5`); idle stop via the same worker's `recv_timeout`.
7. **Streaming publication:** new reviewed installer primitive (no `MAX_FILE`/`MAX_TOTAL` change): installation lock + `statvfs` free-space check, staging via held dir fd `O_CREAT|O_EXCL|O_NOFOLLOW` 0600, incremental SHA256/count, exact size+hash, fsync, no-clobber `renameat2(NOREPLACE)`, dir fsync, marker no-clobber, dir fsync. Model files mode 0600. Service hashes via held no-follow fd with dev/ino/size/mtime unchanged before/after.
8. **Catalog trust:** curated catalog lives in the service crate (`runner::CATALOG`); `list_models` accepts only exact catalog matches (name, sha256, size); duplicate names fail inventory; bounded directory scan, dotfiles skipped; `smith-next/models` (0700) added to `Roots::validate`.
9. **Egress:** TLS only in the installer (service gains no TLS/egress). Every hop https:443, no userinfo/IP literal, host exactly `huggingface.co` or label-boundary `*.hf.co`, ≤3 redirects, no proxies, exact byte count regardless of Content-Length.

Non-blocking adopted: resolve `current` once per start; verify the runtime files against the release manifest at runner start; `--no-slots`, no metrics; env contains only `LLAMA_API_KEY`; Ollama-era saved model → `MODEL_UNAVAILABLE`, reselect, no migration; service-only install → `RUNTIME_MISSING`; package llama.cpp MIT and libgomp GCC runtime-exception license texts; bounded stderr capture.

## Write ownership

- Service agent: `next/service/src/{runner.rs,providers.rs,app.rs,filesystem.rs,lib.rs}`, service tests, `next/service/README.md`, `Cargo.*` if needed.
- Installer agent: `next/installer/src/{model.rs,catalog.rs,setup.rs,cli.rs,lib.rs}`, installer tests/README; no publisher/fs semantics changes beyond reuse; wizard/ui unchanged except text if compile requires.
- Parent: Dagger/client build script/package allowlist/client strings/docs/validation.

## Frozen service library contract (used by installer)

```rust
pub mod runner {
  pub const MODEL_LIMIT: usize = 64;
  pub struct CatalogEntry { pub name: &'static str, pub sha256: &'static str, pub size: u64, pub license: &'static str, pub source: &'static str, pub url: &'static str }
  pub const CATALOG: &[CatalogEntry];                           // default first: qwen2.5-1.5b-instruct:q4_k_m, then qwen3-4b:q4_k_m
  pub fn lock_path(roots: &Roots) -> PathBuf;                   // runtime/smith-next/runner.lock
  pub fn runner_parent(roots: &Roots) -> PathBuf;               // runtime/smith-next (per-runner dirs created inside)
  pub struct ModelRecord { pub name: String, pub sha256: String, pub size: u64, pub license: String, pub source: String }
  pub fn models_dir(roots: &Roots) -> PathBuf;                 // data/smith-next/models
  pub fn runtime_executable(roots: &Roots) -> Result<PathBuf>; // verified current release runtime/llama-server
  pub fn list_models(dir: &Path) -> Result<Vec<ModelRecord>>;   // marker+file facts validated, sorted, bounded
  pub fn marker_bytes(record: &ModelRecord) -> Vec<u8>;         // canonical JSON + newline
  pub fn verify_model(dir: &Path, record: &ModelRecord) -> Result<PathBuf>; // full SHA256, returns gguf path
  pub struct RunnerConfig { pub executable: PathBuf, pub runtime_parent: PathBuf, pub lock: PathBuf, pub context: u32, pub threads: u32 }
  pub struct Runner;  // owns child; Drop stops it
  impl Runner {
    pub fn start(config: &RunnerConfig, model_file: &Path, deadline: Instant) -> Result<Runner>;
    pub fn chat(&mut self, messages: &[crate::app::Message], max_tokens: u32, deadline: Instant) -> crate::app::AppResult<String>;
    pub fn stop(self) -> Result<()>;
  }
}
```

## Review handoff — implementation paused at user request

**Implemented in source:** service-owned private CPU runner and Unix-socket provider; curated model records/integrity checks; cancellation/idle/crash cleanup; native streaming model download with no-clobber publication; `--models` and `--download-model` CLI options; pinned Dagger runtime assembly and package allowlist; client text updated away from Ollama. The old installer Ollama transport remains unused by production (legacy library tests only).

**Final checks observed:** offline locked service and installer Cargo tests PASS; service/installer warning-denied Clippy passed during implementation; seven synthetic streaming publication tests PASS; builder metadata tests PASS with six compiled-entrypoint cases skipped in the local Python run; Dagger `build-runner` PASS and real pinned binary `--version` under an empty Ubuntu environment PASS.

**Superseded handoff:** aggregate test/build, exports, GUI/Flutter gates, documentation and scoped three-finding closure are now complete; see safety closure below and `next/VALIDATION.md`. Model license/provenance review and real-model inference remain pending. No model weights downloaded and no installed-host action. The GUI model step remains deferred, as scoped above. The final aggregate rebuilt `installer/dist`; it is an isolated development candidate, not release/deployment approval. User requested minimal wrap-up and source review before further work.

**Review entry points:** `next/service/src/runner.rs`, `next/service/src/providers.rs`, `next/installer/src/catalog.rs`, `next/dagger/runtime.py`. Existing app/Core wire envelopes remain unchanged; CPU-only, no automatic retries, cloud fallback, GPU permission changes or upgrades.

## Downloader runtime-boundary repair — 2026-10-13

User model-download QA exposed a no-reactor panic. Same failure was reproduced with a real reqwest request to a private synthetic HTTP fixture, no model weights or production endpoint; traceback identified timeout timer construction by `.send()` before runtime entry. Narrow source correction defers request/body future construction via `FnOnce` factory inside the owned runtime, after checking cancellation/deadline. Existing policies/budgets/publication semantics are unchanged; no retry, new threads or runtime leak. Scope is installer source+isolated tests/package refresh only, no installed-host effects or weights.4 focused unit/8 publication tests independently passed; parent installer/test+build passed with240 native/5 GTK/no-Python gates. Source/artifact evidence and baseline failure retained in next/VALIDATION.md. Real transfer/inference remains operator QA, not claimed validated.

## Validation

Fake runner (re-executed test binary or equivalent) over a real UNIX socket for lifecycle/protocol/bounds/crash/model-switch/idle/env-clearing tests; fake HTTPS/transport seam for download tests (production endpoint/host policy not overridable by executable flags/env). Real `llama-server --version`/`--help` runtime smoke in Ubuntu 24.04 builder with the packaged runtime directory and empty environment; ELF/RUNPATH/library inspection. Existing service/client/installer/package gates. **Not performed:** real model inference (requires weights), GPU acceleration, installed-host upgrade/QA. Real-inference smoke is an opt-in operator action after this slice.

## Safety review closure — 2026-10-13

Fresh reviewer `092f336f-2fec-4d2a-bda1-ac0003731980` initially returned FAIL for model-path substitution, active-inference teardown and unbounded hashing. Parent retained verified model descriptors across exec (`VerifiedModel`, `verify_model_held`, `launch_verified`/`start_verified`), connected atomic owner cancellation to native waits with joined worker cleanup, and added bounded-read verification deadline/cancel checks. Service and installer production use held APIs.25 offline runner tests passed, including three focused regressions. Reviewer resumed with **scoped static PASS: all three closed**; did not rerun tests, not full release assurance. Individual filesystem syscalls and same-UID inode writes remain explicit limits. Current aggregate/artifact evidence is maintained in `next/VALIDATION.md`.

**Final integration:** `DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make test build` PASS; refreshed full package.80 service/204 installer/5 GTK/64 Flutter/22 package tests, with source/artifact hashes in [native-runner evidence](LOCAL-SMITH-NEXT-NATIVE-RUNNER-EVIDENCE.json). Prior aggregate fixture failure and tool timeout are retained in validation history. No installed-host effects or weights.
