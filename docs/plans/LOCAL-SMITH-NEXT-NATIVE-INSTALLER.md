# LOCAL-SMITH-NEXT-NATIVE-INSTALLER

## Intent and authority

User explicitly requests full native Rust installer migration; unreleased product must not retain a recipient Python dependency for a short-term shortcut. Implement source and isolated validation only. No installed app/service changes, real manager/provider/model operations, GUI, model download, OS/GPU/Ollama provisioning, upgrade/autorun/linger, commits or remote effects. Existing feature/autonomy/checkpoint04b72f5 with uncommitted work. Prior host approval consumed.

Independent architecture2ea056f6-99f2-48af-adca-2b0d65fb3456; producer/designer492a8945-c8f6-4c5e-a16e-40eff62e2dd6. Full runtime migration, not a Rust launcher around Python. Builder-only Python Dagger/package tooling is allowed; one active installer implementation.

## Reconciled native architecture

Independent crate `next/installer`, own Cargo workspace/lock, Rust2021/MSRV1.83. Reuse existing serde/json/libc/sha2/blockingreqwest families and path-depend on `../service` library. Do not edit client or service/Core implementation. Direct `Controller` with source **packaged payload/service/smith-service**, never installer current_exe. Reuse exact inspect/prepare/confirm/apply and Controller::app identity/manager/socket authority. Create Roots/backend lazily after artifact consent; dry-run/artifact-only require only their relevant existing HOME/data roots and never inspect a manager/provider.

Publication is a native port of the descriptor-relative Python publisher, not substitution with weaker service filesystem routines. Preserve no-follow traversal, owned/mode/single-link files, private namespaces, normalized restricted paths, bounded trees/files, paired complete bundle checks, post-consent/under-lock revalidation, bounded cooperative lock, no-clobber files/release directories, fsync, verified immutable releases, current selector exchange/retained displaced objects and truthful partial effects. No global transaction or same-UID filesystem CAS. Final service-only and client dependency verification required. Canonical release manifest remains byte-identical to Python `json.dumps({schema:1,entries:...},sort_keys=True,separators=(',',':')) + newline`; release identities do not change merely because the implementation language changes.

## Frozen runtime interfaces / write ownership

Crate owner writes Cargo.toml/lock, lib/main/cli/consent/setup/model modules, scripts and setup/CLI/provider tests. Publisher owner exclusively writes src/artifact.rs, src/fs.rs, src/manifest.rs and publisher tests. Parent owns Dagger/Make/build-only package.py/docs/reference migration. No overlapping writes; crate owner declares sibling modules/interfaces below but publisher implements its internals. If the contract cannot be met, coordinate before edits.

Publisher API:
- `artifact::Installer::new(client:Option<PathBuf>,service:Option<PathBuf>) -> crate::Result<Self>`; construction does not mutate or contact manager.
- `Installer::with_roots(client,service,home:Option<PathBuf>,data:PathBuf) -> crate::Result<Self>` for explicit validated roots/library fixtures.
- `Installer::run(&mut self,dry_run:bool,assume_yes:bool,confirm:&mut dyn FnMut(&str)->crate::Result<String>) -> crate::Result<artifact::Outcome>`.
- Outcome enum Installed/Declined/DryRun. Print source/digests/destinations/effects before confirmation. Initial no returns Declined and no writes; confirmation EOF/timeout errors propagate. Print completed/partial effects on failures. assume_yes is exposed only for explicit developer artifact mode, never recipient effects.
- manifest::{digest,manifest_bytes} and internal tree/entry structs may be exposed for tests, but callers do not weaken publisher checks.

CLI:
- native ELF `smith-installer` full recipient mode; `--bundle ROOT` explicit package root, omitted default is parent of actual canonical running executable, not cwd/argv alias/PATH lookup.
- Preserve --dry-run --model EXACT --pull-model --verify-inference. Pull/test require model and each separate consent.
- --artifacts-only plus --client BUNDLE and/or --service BINARY serves existing Make artifact targets. Reject mixed recipient arguments. Optional --yes only in explicit artifact mode, binds displayed plan and bypasses no safety checks. No global recipient --yes.
- Help/version finite and no operational discovery. Exit0 help/dryrun/success/initialdecline;2 argument errors;1 semantic/EOF/interrupt/timeout/laterdecline/partialfailure.
- Consent at most4096bytes/60seconds, bounded actual stdin read including drip input; EOF/interrupt do not approve. Preserve native prepare token original60s expiry, never auto reprepare/retry. Integrate cooperative cancellation with existing Core signal/manager cleanup; do not abandon a thread still able to apply effects. Direct library removes wrapper-helper subprocess but retains library manager/socket budgets; no false 45s total filesystem deadline or external inference cancellation claim.

Recipient orchestration: artifact publication -> separately consented unit registration/reload -> separately consented Start -> one async installed-model inventory -> optional exact-model pull -> fresh verification -> separate preference save -> optional fixed minimal `/api/generate` test. Missing prerequisites guided, not provisioned. No complete-setup flag write or service-domain replay/state reconciliation inside installer. Direct calls still validate current identity and instance/request correlation; inventory is not inference. Model download consent requires exact selected model. No default/best model invented.

Fixed numeric loopback11434 only, no proxy/redirect/endpoint env/retries/cloud/tools. Existing strict installer metadata validation retained (duplicate/conflicting raw model identities before filtering, including excluded rows). Tags2sconnect/idle10soverall256KiB256models; pull30sidle900soverall16KiBframe16MiBprogress65536frames; inference60soverall30sidle64KiBbody4KiBnonblank text. Reject malformed/oversized/slow-drip metadata/frames including headers/chunk framing; generated text inert, not remote-metadata heuristic. Cancellation discloses possible daemon/cache continuation, never deletes cache. Tests library fake managers/providers or controlled temporary network seams only, no production endpoint override.

## Archive and cutover

Preserved7 exact Python runtime/test/package sources in excluded `reference/python-installer-v1/`; manifest SHA256 `dfe5f852f6c57dd3b0b2c2531d96ab44acbeec4b378b9bfed7ec2bac9f256489`. Historical evidence is not rewritten. After native parity/test/review passes, remove active dagger/install.py, installer.py, test_install.py and test_setup.py. Convert package.py/tests to builder-only native assembly/verification with no runtime imports, zipapp or Python entrypoint. No maintained fallback installer. Existing voice archive stays unchanged outside inputs.

## Dagger / package

Installer builder takes explicit sibling installer/service trees, uses existing Rust1.98Bookworm toolchain/root preparation then UID1000 source build/tests. Separate native-binary export to ignored installer/build/smith-installer for component artifact installs, and full recipient export installer/dist from Dagger native code plus complete paired client/service payload. No Python on recipient, no home/daemon/manager/provider mounts. No optional installer CLI source migration in app; its instruction-only handoff stays unchanged.

Recipient package still smith-installer + payload/client + payload/service + manifest/checksums/README. Builder-only Python may generate/verify metadata, but must not recreate host publication/lifecycle logic. Preserve reviewed15Flutter/native allowed paths and derived directories; exclude arbitrary model/voice/reference files/dirs before declarations. Validate actual ELF architecture/interpreter/library dependencies and execution under Ubuntu24.04 without Python/build-tool invocation. Native means no interpreter, not statically linked or universal Linux ABI. Existing app/service ABI and unit bytes unchanged.

## Implemented source and review handoff

The native crate, both CLI modes, Make/Dagger exports, builder-only metadata assembly and Python cutover are implemented in source. The old runtime/test sources are retained only in the excluded archive; there is no active interpreter fallback. Current operator/build documentation follows the native contract. Service/client executable source and tests are unchanged; the client README's installer-description wording is updated only.

Focused independent publisher review `ea115b26-7d91-44f9-8991-e303dbe1a000` closed the tree-fact/ancestor-fact collision after separate fact maps and original/ctime/subdirectory/under-lock probes. Focused workflow/model review `ad9c3baa-3f57-4ebb-87a8-4b952d1c4c8c` closed blocked progress/error output after bounded `ProgressOutput`/FD reporting, and independently closed the test-only observer race after bounded condition-variable observation and a deterministic delayed-observer regression. Native CLI capture also replaces unbounded post-exit `read_to_end` on nonblocking pipes with a capped available-byte drain and retained-writer regression. These scoped reviews do not substitute for integrated package/source assurance.

Candidate-bound integrated commands, source/artifact/ABI identities, archive verification, findings and verdict belong in `LOCAL-SMITH-NEXT-NATIVE-INSTALLER-EVIDENCE.json` and `next/VALIDATION.md`. Evidence must distinguish fresh checks from cached Dagger reuse and the tested Rust1.98 toolchain from the untested declared MSRV1.83. The source gate does not approve installation, real manager/provider/model/GUI actions, release or publication; no prior host approval is reusable.

## Traceability and required evidence

| Requirement | Task | Evidence |
|---|---|---|
| R1 no recipient interpreter | crate/CLI, pipeline | ELF/readelf/dependency checks; directly executed packaged help/dryrun with private roots/traces no interpreter |
| R2 all CLI modes | crate/Make | args/exits/mixed-mode/default executable-location; component recipe expansion |
| R3 consent/cancel | crate/setup/model | initialdecline/dryrun no writes/manager/network; each later effect separate; bounded stdin and Core expiry tests |
| R4 publisher parity | publisher | port/mapping of43 baseline invariants incl races, state/input changes, lock/FD/limits/partialfailure/retry |
| R5 model parity | crate/model | old46 wrapper invariants mapped; strict typed contracts, metadata ambiguity, no proxy/redirect/retry, slow/oversize/cancel/postpull |
| R6 single native cutover | parent | no active Python installer/runtime source/zipapp routes or packaged scripts; docs/manifest updated |
| R7 service unchanged | parent/review | source hashes/compiled Core/app regressions unchanged; voice compatibility and text UI unaffected |

Run native formatting/Clippy warnings denied/locked tests/release build from installer root, then make installer/test make test make installer/build make build plus package builder tests and direct private-package CLI execution. Map every prior test to replacement or obsolete interpreter mechanism; counts alone insufficient. Fresh independent architecture/adversarial assurance and current source/artifact evidence. MSRV metadata or declared bound is not actual1.83compile evidence; report toolchain used. Cold/warm cache and interpreter/library ABI observations retained. Actual host installs/managers/providers/GPU/inference/graphics/provenance/license/SBOM/publication remain separately gated; do not silently deploy new candidate.
