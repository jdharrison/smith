# Smith history and source relocation

The root promotion makes the former `next/` product the only supported active implementation. It intentionally retires the old root orchestration/chat CLI, legacy Flutter client and obsolete installers/scripts rather than retaining compatibility shims. This is a **source-support withdrawal**, not an uninstall or migration of existing legacy installations. Broader orchestration remains target/deferred policy, not an implemented capability of the promoted text product.

## Immutable pre-cutover checkpoint

**Full commit:** [`168865b0422b787f08ab33e89d052cd4adbd11f7`](https://github.com/jdharrison/smith/tree/168865b0422b787f08ab33e89d052cd4adbd11f7).

This is the root-promotion plan's pre-cutover baseline, not evidence that a later cutover passed validation. Historical commands, hashes, paths, manifests, outcomes and limitations describe their original revision. Keep them verbatim when retained locally; do not globally replace `next/` inside evidence.

Git-backed retention replaces copied historical source/reference trees. There is no supported `legacy/` alternate product, copied archive, new Git history ref or release tag required by this index. Preserve checkpoint ancestry through reviewed integration; do not rewrite history or delete the bootstrap branch without separately bound approval.

## Path and capability map

Paths below are repository-relative. Links identify the **old** bytes at the full checkpoint; destinations describe the promoted layout, not runtime renames.

| Checkpoint location | Promoted location or disposition |
|---|---|
| [`next/client/`](https://github.com/jdharrison/smith/tree/168865b0422b787f08ab33e89d052cd4adbd11f7/next/client) | `client/`: current Flutter text desktop, live fixtures, goldens, assets and notices |
| [`next/service/`](https://github.com/jdharrison/smith/tree/168865b0422b787f08ab33e89d052cd4adbd11f7/next/service) | `service/`: independent Rust service workspace/lockfile |
| [`next/installer/`](https://github.com/jdharrison/smith/tree/168865b0422b787f08ab33e89d052cd4adbd11f7/next/installer) | `installer/`: independent native CLI/GTK workspace/lockfile; sibling service dependency retained |
| [`next/dagger/`](https://github.com/jdharrison/smith/tree/168865b0422b787f08ab33e89d052cd4adbd11f7/next/dagger) | `dagger/`: builder/package tooling; `dagger/INSTALLER.md` remains the recipient guide |
| [`next/Makefile`](https://github.com/jdharrison/smith/blob/168865b0422b787f08ab33e89d052cd4adbd11f7/next/Makefile), [`dagger.toml`](https://github.com/jdharrison/smith/blob/168865b0422b787f08ab33e89d052cd4adbd11f7/next/dagger.toml), [`dagger.lock`](https://github.com/jdharrison/smith/blob/168865b0422b787f08ab33e89d052cd4adbd11f7/next/dagger.lock) | Same basenames at root; active source paths rebased |
| [`next/README.md`](https://github.com/jdharrison/smith/blob/168865b0422b787f08ab33e89d052cd4adbd11f7/next/README.md), old [`README.md`](https://github.com/jdharrison/smith/blob/168865b0422b787f08ab33e89d052cd4adbd11f7/README.md) | Replaced by one current [root README](../README.md), not concatenated historical instructions |
| [`next/VALIDATION.md`](https://github.com/jdharrison/smith/blob/168865b0422b787f08ab33e89d052cd4adbd11f7/next/VALIDATION.md) | Root `VALIDATION.md`; old results remain historical, fresh cutover evidence required |
| Legacy [`src/`](https://github.com/jdharrison/smith/tree/168865b0422b787f08ab33e89d052cd4adbd11f7/src), [`client/`](https://github.com/jdharrison/smith/tree/168865b0422b787f08ab33e89d052cd4adbd11f7/client), [`Cargo.toml`](https://github.com/jdharrison/smith/blob/168865b0422b787f08ab33e89d052cd4adbd11f7/Cargo.toml), [`Cargo.lock`](https://github.com/jdharrison/smith/blob/168865b0422b787f08ab33e89d052cd4adbd11f7/Cargo.lock) | Retired tracked implementation: old `smith` orchestration/chat/setup commands and legacy desktop; no root Cargo package/workspace or compatibility shim |
| Baseline [`scripts/`](https://github.com/jdharrison/smith/tree/168865b0422b787f08ab33e89d052cd4adbd11f7/scripts) | Retire the nine legacy helpers listed below; retain/rebase separately reviewed current pipeline helpers/tests |
| [`reference/voice-chat-v1/`](https://github.com/jdharrison/smith/tree/168865b0422b787f08ab33e89d052cd4adbd11f7/reference/voice-chat-v1) | Git-backed historical voice snapshot, not an active speech capability or build/package input |
| [`reference/python-installer-v1/`](https://github.com/jdharrison/smith/tree/168865b0422b787f08ab33e89d052cd4adbd11f7/reference/python-installer-v1) | Git-backed historical Python installer, not a supported fallback or recipient dependency |
| [`docs/`](https://github.com/jdharrison/smith/tree/168865b0422b787f08ab33e89d052cd4adbd11f7/docs) | Keep canonical policy, active contracts and necessary current evidence; retrieve retired/superseded guides, plans and reports from the checkpoint |
| Tracked diagnostic `output.log` | Retired historical diagnostics, not product input; do not publish raw content or private values |

The nine retired baseline scripts are `bump-version.sh`, `check-docker.sh`, `install-prebuilt.sh`, `install.sh`, `list-agents.sh`, `package-chat.sh`, `smoke-prebuilt.sh`, `smoke-user-service.sh` and `validate.sh`. This is **not** a blanket retirement of `scripts/`: concurrent installer-pipeline additions and their tests must be preserved and rebased with their consumers.

`AGENTS.md`, legal/security files, roles, toolchain configuration, current policy and active GitHub controls remain in the working tree with narrowly scoped alignment. Development role prompts are not implemented runtime orchestration. Live migration fixtures, golden images, fonts/icons/notices and intentionally tracked build inputs are not historical-cull candidates.

## Retrieve and verify without changing the checkout

For example, inspect a historical document or identify its exact Git blob:

```sh
git --no-pager show 168865b0422b787f08ab33e89d052cd4adbd11f7:next/README.md
git --no-pager rev-parse 168865b0422b787f08ab33e89d052cd4adbd11f7:next/README.md
git --no-pager ls-tree -r 168865b0422b787f08ab33e89d052cd4adbd11f7 reference/
git --no-pager merge-base --is-ancestor 168865b0422b787f08ab33e89d052cd4adbd11f7 HEAD
```

These are local read-only queries; ancestry success is exit status 0. Full reconstruction requires the cutover's reviewed per-file disposition manifest: original path, destination/retirement, Git mode, content hash, containing checkpoint and reason. This compact index is not that manifest and does not attest that every retired hash or later addition has been verified. The baseline voice/Python manifests describe 46 and seven payload files respectively; verify those exact bytes against their manifests as part of final cutover assurance.

The baseline cannot preserve files first created by later concurrent work. Bind such files to their actual later approved checkpoint in the final disposition/evidence record; do not pretend `168865b` contains them. Local Git retrieval needs the checkpoint objects; remote links need repository access. A link or checksum is not a backup or permission to disclose private historical content.

## Compatibility and evidence limits

Source relocation preserves `smith-next.service`, the `smith-next` launcher and XDG namespaces, desktop IDs, keyring identity fields, executable/wire identities and deterministic ownership-recognition bytes. It neither migrates legacy `smith.service` nor changes installed data/services. Existing inert voice-bearing state remains compatibility data, not permission to execute speech.

Ignored/untracked caches, raw logs, binaries, model weights and build outputs are **not Git-backed** and cannot be recovered from this checkpoint. Their approved preservation/clearance is separate local work; this index makes no purge or backup claim.

Old test counts and PASS records do not validate the relocated tree. Source/fake/compiled/build evidence is not release, deployment, real-key/provider, clean-host, graphical or model/GPU acceptance. Current disk4 rollback needs separately approved pre-migration state and compatible paired binaries with exact credential-generation references, not a historical checkout alone. Never restore an unsafe legacy publisher as an incidental source rollback.

See the [root-promotion plan](plans/LOCAL-SMITH-ROOT-PROMOTION.md), [current validation](../VALIDATION.md) and [approval boundaries](PROCESS.md#approval-boundaries). This history index grants no commit, push, tag, release, deployment, branch deletion or history-rewrite authority.
