# LOCAL-SMITH-DEV-WORKER-IMAGE-UPDATE

## Exact operator authority

The operator answered **“yep”** to: rebuild the stale local worker image using the executable matching the current running service, preserve the old image and failed job, leave the service untouched and do not submit another job automatically. This is bounded local image-preparation approval, not functional acceptance, release, service rollout, new inference or reconciliation. Repository Smith, existing `feature/autonomy`, base `14d960975e76f906efb5e447ff3dece0e8e4385b`; preserve dirty work. Approval expires on completion or material drift.

## Bound artifacts and effects

- Existing local rootful Docker only: `unix:///run/docker.sock`, isolated empty Docker config, no ambient credentials/context.
- Running service/selected helper SHA256 must remain `2500f2f114342895d4a0e8c7da3247b2ccde7c47685716dd3c32450fed32e708`. Copy the verified running executable into the allowlisted worker build payload, not a newly compiled or service-only installed replacement.
- Preserve old payload `493616d4172236d160e2789697d53b3f72c30eac2bba804a42f5625e47d61288` and image `sha256:9b87e922785dcef603cc537749d1f59b2cdc4abfcabc0f596c01188cbb64763a` before replacing `smith-dev-worker-local:1.19.3`.
- Preserve recipe SHA256 `0009f1538f11bfd2145796ab1b2a6b893016ac7ee4614fb8dd90ba657f8f47df`, guard `40504069bc2548e7d1da3c9a47cecda3ee1be77e2be3a60bbdaac737e25ef310`, ignore file `6cab522c6a0c3c730943c2628d18b84e4efbe83e91138bf151371c7942101000`.
- Use the existing allowlisted `service/` context and preserve the original recipe unchanged. Derive a minimal preparation recipe FROM the verified retained old-image tag, COPY only the matching service executable, then run image-build checks for exact service/guard hashes and root-owned immutable executable modes. This reuses the actual existing Git/OpenCode/base/guard bytes without assuming cross-builder cache compatibility. Build with `--pull=false --network=none`; no package/archive download, network fallback or daemon configuration. Create a temporary candidate tag before replacing the runtime tag.
- Verify final image user1000:1000, fixed entrypoint, no volumes, unchanged environment/workdir, matching payload/guard and root-owned0555 executables. Verification runs only hash/stat tools during image build, never the worker entrypoint, Git clone, OpenCode or model. No service/helper/selector/unit changes, restart, ledger mutation, old-container change, image deletion/pruning, new Dev job or inference.

## Evidence and review

Diagnosis and controlled old-flags reproduction: `docs/evidence/dev-clone-failure/README.md`. Full-clone source checks and fresh assurance: `docs/plans/LOCAL-SMITH-DEV-FULL-CLONE.md`. Obtain fresh read-only script review before host effects; immediately recheck artifact/service/image identity. Save secret-safe bounded build and identity evidence under `docs/evidence/dev-clone-failure/`; retain payload/config scratch privately under `.smith-dev-worker-full-clone-update/`. On failure, preserve partial evidence and old image; no retry with broader network or automatic rollback affecting installed state. No working pipeline claim without separately approved live functional evidence.

## Outcome

Initial fresh read-only review blocked the original offline rebuild strategy: forcing the legacy builder could not reuse prior BuildKit package-install intermediates. No script or mutation ran. Revised preparation derives directly from the verified preserved local worker image and replaces only its stale executable; it does not rerun apt/curl or depend on layer-cache reuse. Fresh revised-script review passed before effects, bound to script SHA256 `c36e1003f6cfcf9765a0c147eefd85b9d154f95eab46033ee32414c266aa6aae`. The exact hash-gated `python3 -B` invocation completed once, exit0.

**Image preparation PASS:** `smith-dev-worker-local:1.19.3` now resolves to `sha256:3bb245491a8879160b2e61af480afc292ab03ee951c34278df3a139677b51d49`. Build-time service/guard hash checks passed; service and OpenCode retain root ownership/mode0555. Runtime image user1000:1000, entrypoint, environment, workdir and no-volumes configuration match the retained image. Existing toolchain layers remain an exact prefix; only the executable replacement and hash/stat verification layers were added. Build network was none, pull=false, with no downloads.

Running service PID126935 stayed active and unchanged; executable, selected helper and staged payload all match2500f2. Old image retained as `smith-dev-worker-local:before-full-clone-20261019`; old payload preserved privately in the approved scratch directory. No service restart/swap, job submission, inference or ledger-write command ran. Old failed job `01ceb621783ba4156ffa82a8b83e8b96` remains retained and is not replayed.

Fresh independent read-only post-effect assurance verified script/recipe/log hashes, final/backup image tags, inherited layers/configuration, hash/mode build checks, unchanged service PID/helper and retained failed-job identity/status/count. No mismatch found. No pre-effect ledger digest was taken, so this is not byte-for-byte ledger preservation evidence. Exact result and build log: `docs/evidence/dev-clone-failure/worker-image-update.json` and `worker-image-build.log`.

This completes only the approved image preparation. Real clone/OpenCode/model/reply/cleanup functional acceptance remains untested and requires a fresh explicitly authorized job, not replay of an old ID.
