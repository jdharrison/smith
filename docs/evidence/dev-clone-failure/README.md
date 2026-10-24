# Public advertisement transport diagnostic

## Initial scope and authority

Explicit operator task: investigate job `01ceb621783ba4156ffa82a8b83e8b96` read-only, with an optional isolated local diagnostic here. No production edits, host service/job/model/Docker effects, clone, private repository, credentials, redirects or authentication. Parent owns installed/image/source binding. Existing failures are retained; no retry or reconciliation is authorized by this evidence.

## Initial metadata-only results — implemented source

Two fixed credential-free public metadata GETs were executed, both to `https://github.com/jdharrison/smith/info/refs?service=git-upload-pack`. No POST or clone was executed. Builds used existing locked dependencies offline in this isolated directory.

1. `cargo build --offline --locked --manifest-path docs/evidence/dev-clone-failure/Cargo.toml` with generated instrumented transport: exit0. The corresponding executable: exit0, 285ms; DNS one answer, public-address approval, TCP, Rustls TLS, request write, HTTP/1.1 200, expected content type, chunked body and final EOF all passed. Raw body 1821 bytes; Git service marker valid. See `build.log`, `transport-run.log`, `source-binding.json`, and preserved `instrumented-main.rs`.
2. Same build command after replacing the harness with `src/main.rs` using `production-dev-fetch.rs`: exit0. The latter is a byte-identical copy of the complete production module, compiled **without cfg(test)**. Executable: exit0, 253ms; `FetchSession::new` and `request(0, true, empty_body, 20s_deadline, cancel=false)` passed, including real DNS/TLS/HTTP, advertisement parsing and Base `main` pin selection. Sanitized advertisement 1694 bytes, one pin, selected `refs/heads/main`. See `full-build.log`, `full-fetch-session-run.log`, and `full-source-binding.json`.

Source SHA256: `417d583e5ec847a512e3c959d061e70283b29a974fc404aa8b3bd8b0809175d6` (`service/src/dev_fetch.rs`). Standalone stubs reproduce only the Repository/Checkout fields and error variants used by the module; they do not substitute any transport/parser implementation. No object IDs, response headers, credentials or private host inventory are printed.

Detected stack: Rust 2021 service, Tokio current-thread runtime, tokio-rustls 0.26.6 / Rustls / webpki-roots, strict custom HTTP/1 framing parser, Git smart HTTP v0 advertisement. Versions are from the successful locked build, not a claim about installed executable provenance.

## Failure-path finding

The reported `RepositoryUnavailable` does **not** establish a transport failure:

- `service/src/dev_runtime_local.rs:236-246`: host completes `fetch.request(...)` and sends the advertisement response before receiving another worker frame. A host fetch error returns immediately via `?`.
- `service/src/dev_runtime_local.rs:409-415`: a received `Frame::Failed { kind: "local_worker_failed" }` before checkout completion is explicitly converted to `LocalError::RepositoryUnavailable`, regardless of the worker's underlying cause.
- `service/src/dev_worker_local.rs:614-619`: child stdout/stderr are discarded. `execute` maps non-success to the fixed repository error (around lines696-698), and `run` emits only the generic failure (around lines1137-1151).

**Conditional conclusion, high confidence:** if the parent-reported sequence is the worker's actual stdout (`repository_request` then `failed/local_worker_failed`), and the installed host has the inspected control flow, the host did not terminate that request with a fetch error. It returned to its receive loop and consumed the worker failure. This is stronger than timing or lack of stderr. Parent must bind installed source; the diagnostic was not run inside the service process and is not a historical network trace.

## Minimal recommendation — proposal, not performed

P0: reconcile the parent-owned service/worker binding before any new live job. The observed sanitized production advertisement retains side-band capabilities but **omits `shallow`**. A stale worker requesting `--depth=1` is incompatible with that advertisement: a Git client requiring shallow support can exit after the advertisement without emitting a POST. This fits the reported frame sequence. Do not weaken the full-clone relay or reintroduce shallow behavior merely to accommodate stale worker bytes.

**Initial certainty limit (subsequent local reproduction below strengthens the mechanism):** the exact historical worker exit reason is not recovered by this diagnostic. At this initial stage, stale shallow-worker incompatibility was a strong source/protocol inference, not an observed Git stderr result. No DNS/TLS/body-framing/runtime defect was reproduced; there is no evidence-backed production transport patch to recommend. A transient historical failure or different installed-source behavior cannot be excluded solely by these standalone passes.

P1 only if failure persists after exact binding: add separately approved, payload-free internal stage/failure classification that distinguishes host DNS/TCP/TLS/HTTP/ad parsing from worker Git exit and preserves fixed public-facing messages. Do not expose raw stderr, headers, URLs beyond frozen public destinations, or credentials.

## Validation contract / release safety

These are narrow metadata-transport passes, not clone/image/OpenCode/model/installed acceptance. Before any eventual shipping or functional claim, require exact artifact/source binding, applicable offline regression gates and fresh independent review. Any new service/image rollout or smoke job needs its own current explicit approval; never replay the failed ID. Failed required checks, compiler/runtime errors, unresolved critical warnings or missing required verification remain blocking. No release or accepted risk is approved here.

## Subsequent offline native-Git reproduction

An isolated native-Git fixture was performed as local diagnostic validation of the reported failure, with static/synthetic advertisement, the old flags, bounded stderr capture and zero external network. These local fixture Git invocations and a deliberately rejected loopback control POST are not a live clone or new operator approval. No production, Docker, model, job or service mutation was performed.

Command: `python3 docs/evidence/dev-clone-failure/shallow_repro.py` (repository root). Actual result: **exit0, repro_pass=true**. Its Cargo build uses `--offline --locked --bin shallow_repro` and passes. Git reports `git version 2.43.0`.

`repro-dev-fetch.rs` contains the byte-identical current production module followed only by a diagnostic visibility adapter calling its private `parse_advertisement`. Source SHA256 remains `417d583e5ec847a512e3c959d061e70283b29a974fc404aa8b3bd8b0809175d6`. This is the same sanitizer used by `FetchSession::request`. The static v0 advertisement contains valid synthetic HEAD/main records and the `shallow` capability. Production sanitization succeeds and removes `shallow` (306 input bytes,298 output bytes).

| Native Git case | Result | Observed HTTP sequence |
|---|---|---|
| Old flags: `--depth=1 --single-branch --no-tags` | exit128; `fatal: Server does not support shallow clients`, then `fatal: the remote end hung up unexpectedly` | One advertisement GET; **zero pack POSTs** |
| Same invocation with only `--depth=1` removed | Reaches pack negotiation; exit128 because fixture deliberately returns HTTP403 | One advertisement GET, then **one pack POST** |

Both invocations retain `--no-recurse-submodules --template= --branch main`; Git uses an isolated temporary HOME/config, disabled credentials/redirects/hooks, protocol v0 and HTTP-only protocol allowlist. Only an ephemeral127.0.0.1 server is contacted. No pack is served, object acquisition succeeds or external destination is contacted. Temporary fixture clone directories are cleaned by the scoped temporary-directory context; evidence is retained. Exact safe stderr and request sequences: `shallow-repro-result.json`; build output: `shallow-repro-build.log`; source: `shallow_repro.py`, `src/bin/shallow_repro.rs`, `repro-dev-fetch.rs`; input/output: `synthetic-advertisement.bin`, `sanitized-advertisement.bin`.

**Updated conclusion:** incompatibility between the current sanitizer and the old depth-one worker is now experimentally demonstrated, including the exact native-Git error and absence of a pack POST. The one-variable control isolates `--depth=1` as the blocker for this fixture. Parent-reported artifact binding (running service2500f2 without depth flag; dist/worker493616 retaining old flags) and the failed job's advertisement-then-worker-failure sequence are consistent with this precise mechanism. The historical worker stderr itself was discarded and is not retroactively recovered; this is a local reproduction, not an execution of the retained worker image. No transport fix is supported by these results. Correct the artifact mismatch under separate exact approval, rather than restoring shallow support or trusting the stale dist payload.

## Parent read-only artifact and failed-job binding

Read-only inspection of the active `smith-next.service` and selected helper found the same SHA256 `2500f2f114342895d4a0e8c7da3247b2ccde7c47685716dd3c32450fed32e708`; the running executable contains the worker entrypoint but not the `--depth=1` literal. The staged `service/dist/smith-service` is SHA256 `493616d4172236d160e2789697d53b3f72c30eac2bba804a42f5625e47d61288` and differs from the running service. Read-only Docker copy of the executable from the stopped failed-job container returned that same old493616 digest and contains `--depth=1`.

The retained job ledger identifies image `sha256:9b87e922785dcef603cc537749d1f59b2cdc4abfcabc0f596c01188cbb64763a`, failed_stage=cloning, may_have_sent=false, no session/repositories/reply and cleanup_required=false. Exact-request Docker inspection agrees: exited, not running, no Docker state error, started18:43:38.215Z and finished18:43:38.641Z. Bounded stdout inspection found exactly one repository advertisement request then `local_worker_failed`, no stderr. No private task/context/credentials were printed or retained by these discovery checks. The failed record/container were not altered or replayed.

**Historical proposal, subsequently approved and completed:** preserve the old image/payload, stage a verified full-clone worker executable matching the current service, and rebuild only the existing local-worker image. The operator answered “yep”; [exact image-update scope and outcome](../../plans/LOCAL-SMITH-DEV-WORKER-IMAGE-UPDATE.md) records the completed update to image `sha256:3bb245491a8879160b2e61af480afc292ab03ee951c34278df3a139677b51d49`, matching the unchanged running service. Preparation and independent post-check passed; no new job or inference was run. No service/helper swap, restart, new job, inference, image pruning or historical-record mutation is implied. The old service-only updater must not be reused. Future functional acceptance still requires a separately approved fresh job and actual image/OpenCode/model evidence.

## Agent instructions

Use the logs and control-flow finding to correct the premise that the first advertisement necessarily failed. Complete parent-owned artifact binding. Preserve the failed job and cleanup evidence. Make no transport change without reproducing a transport defect. Obtain exact approval before any worker rebuild, service change or fresh live run.
