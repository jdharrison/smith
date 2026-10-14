# Smith developer pipeline and native recipient installer

## Current bundled native runner

**Implemented source:** [native-runner plan](../../docs/plans/LOCAL-SMITH-NEXT-NATIVE-RUNNER.md) supersedes external Ollama setup/provider descriptions from earlier stages below for production `next/` only. Smith bundles separate CPU and Vulkan llama.cpp runtimes in the paired client release; no external Ollama is required. Weights remain separate. List choices with `smith-installer --models`; use `--model EXACT --download-model` (`--pull-model` alias) with separate exact-name download, preference and optional inference consent. The focused visual installer supplies model provisioning/service-backed checks/removal; Qwen3-14B is fresh-only preselection, preserving existing saved selection. Neither installer is persistently installed: retain the full package.

Runtime/model verification, private Unix IPC, one runner lease, bounded context and service-owned cancellation/cleanup are described in canonical Architecture. Production loads the held verified model FD; app-owner teardown cancels/joins native inference, while desktop close only detaches. No implicit upgrades, retries, cloud/tools, bundled weights or real-inference/real-GPU/clean-host/release guarantee. The service attempts fixed full offload on `Vulkan0` with classified confirmed-cleanup CPU fallback; no driver discovery/provisioning or acceleration claim. Unused installer Ollama code remains legacy library/test coverage, not a production fallback. See [validation history](../VALIDATION.md) for observed checks.

Dagger owns developer build/test/package orchestration. The client, service and installer remain native Flutter/Rust runtimes; recipients need neither Dagger nor Python. Python here is only the Dagger SDK and builder-only package metadata tooling.

Run from `next/`, using the local `dagger.toml` default entrypoint:

```sh
dagger call test-service --source service
dagger call test-client --source client --service service
dagger call test-installer --source installer --service service
dagger call test-installer-ui --source installer --service service
dagger call test-package --source dagger --installer installer --service service
dagger call test --service service --client client --installer installer --tooling dagger

dagger call -o service/dist/smith-service build-service --source service
dagger call -o client/dist build-client --source client --service service
dagger call -o installer/build/smith-installer build-installer-binary --source installer --service service
dagger call -o installer/build/smith-installer-ui build-installer-ui --source installer --service service
dagger call -o installer/dist build-installer --installer installer --client client --service service --tooling dagger
```

Equivalent shortcuts: `make test`, `make build`, `make service/test`, `make client/test`, `make installer/test`, `make installer/ui-test`, `make package/test`, `make installer/binary`, `make installer/ui`, `make installer/build`. Formatting/lint belong to `/test`; no separate public check target. Build success does not imply tests passed. No archived voice/Python installer source or speech image dependency enters the pipeline.

## Tooling boundaries

- `../installer/`: independent Rust crate for descriptor-relative artifact publication and recipient orchestration. It directly reuses the unchanged service library's inspect/prepare/confirm/apply and app identity checks, with the **packaged service binary** as Controller source, not the installer executable. CLI model provisioning uses curated HTTPS download with separate consent; separate CPU/Vulkan runners and CPU fallback are bundled, with no OS/GPU provisioning or implicit upgrade.
- `package.py`: builder-only assembly/verification of both CLI/GTK3 installer ELFs, complete paired client/service payload, manifests and checksums. A reviewed explicit Flutter/native path contract rejects arbitrary model/voice/reference inputs before exclusion declarations. It contains no host publication or lifecycle implementation and creates no Python entrypoint.
- `tests/test_package.py`: builder metadata tests with actual compiled CLI/GUI ELFs, CLI help/dry-run and no-display GUI help/version/preflight under private roots. Publisher, consent, lifecycle and provider tests live in `../installer/` and use injected seams, not real managers/providers/downloads.
- `INSTALLER.md`: recipient instructions embedded in each exported package.

Native publication stays outside the build container. Make developer install targets build the tool independently, then invoke `installer/build/smith-installer --artifacts-only` with explicit client and/or service inputs. Service-only installation does not build the full Flutter package. Dagger exports artifacts, not arbitrary writes into the recipient's home. No Docker socket, user credentials, broad home or installed-service bindings are added to worker containers.

## Runtime and build gates

Configured native quality/wizard tests run on **actual Ubuntu 24.04**, as UID/GID 1000, with the Rust toolchain copied from the build image. `test-installer-ui` adds optional-feature GTK3 formatting/Clippy and isolated Xvfb widget/session smoke; `test-installer` includes it. A separate minimal Ubuntu CLI gate executes help/version and service-only dry-run with Python absent, no GTK/display and PATH unavailable, checking no namespace/lock writes. UI execution/results and scoped review remain pending; configuration is not a passing result or real graphical/installed-host evidence.

Service/CLI/GUI release builders use Rust1.98.0-bookworm; GUI builds enable `gui` with GTK3 development libraries. Client uses Ubuntu24.04 and Flutter3.47.2. Toolchain preparation runs as root; source builds/tests run as UID/GID1000. Package metadata tests/assembly use Ubuntu24.04 Python3.12 **only in the builder**, plus GTK3 runtime libraries for GUI help/version. Those calls skip GTK initialization/display discovery, not shared-library loading. The declared installer MSRV1.83 is not an actual1.83 compile result. Native means a Linux x86_64 ELF with runtime library requirements, not static linking or universal Linux ABI.

## Reproducibility and privacy

The configured module engineVersion is v0.21.0. Exact executed toolchain/engine results belong in [VALIDATION](../VALIDATION.md), not inferred from configuration. Dagger engine/cache writes and image/toolchain/dependency downloads are developer effects. Tags/apt inputs are not digest-pinned; this is not a hermetic, clean-cache or signed release-provenance claim. No cloud account/token is required; consider source/log privacy before enabling trace upload. Package checksums detect accidental changes, not authenticity without an independently trusted digest.

The full package contains both installer ELFs; `make installer/ui` is only a binary export. Preferred operator QA from `next/` is `./installer/dist/smith-installer-ui`, after `make installer/build` if needed. This is separate from isolated Xvfb testing and is not permission for agent host effects. The GUI has fixed paired artifacts, optional separately confirmed Register/Start, and separately consented model download/check/removal work, without automatic app launch. CLI/Make behavior stays unchanged.

The native migration's final integrated assurance was cancelled, not a PASS. Live upgrade, provider/download/inference, graphical QA and publication remain separate gates. See [UI plan](../../docs/plans/LOCAL-SMITH-NEXT-INSTALLER-UI.md), [native migration plan](../../docs/plans/LOCAL-SMITH-NEXT-NATIVE-INSTALLER.md) and [recipient instructions](INSTALLER.md). The archives are excluded historical references, not optional runtime fallbacks.
