# Smith validation

## Root promotion — fresh local source/build/package validation PASS

The approved [root-promotion plan](docs/plans/LOCAL-SMITH-ROOT-PROMOTION.md) promotes the former `next/` product to the repository root and intentionally retires the old root source/CLI. Installed `smith-next` identities, application/Core/storage contracts, credentials, models and live services are unchanged.

The [exact disposition manifest](docs/evidence/root-promotion/disposition.json) binds all 1,721 baseline tracked entries: 1,231 promotions, 454 history-backed retirements, 34 retained files, and two README replacement dispositions. All retired bytes and modes were independently checked against checkpoint `168865b0422b787f08ab33e89d052cd4adbd11f7`; all 53 archived reference payloads verified. Concurrent installer-pipeline source and evidence were captured and preserved before migration. Local old-client/next leftovers and original modified sources are retained under ignored `target/root-promotion/`; they are not a Git backup or product input.

**Fresh root gate PASS:** `make test build` with official verified Dagger v0.21.9 passed **263 service +281 native installer +nine GTK +668 Flutter +41 actual-ELF package =1,262 tests**, zero failures; three service/two installer controlled fixture ignores and zero Flutter/package skips. Formatting81/zero changes, analyzer and warning-denied Clippy passed. Explicit package verification accepted95 manifest-listed files/97 total;34 helper tests passed with zero skips, including rebuilt-package archive round trip. All1,230 core inputs stayed unchanged (inventory SHA256 `0617ce1780c0f88d5f325c4ee9e104aaf61e38905a3d1a6544945fc586a82670`). Verified snapshot exports were copied into root `service/dist/`, `client/dist/`, `installer/dist/` after confirming destinations absent; content and modes, four service copies and paired payloads agree.

The first gate failed at module discovery before component execution: a snapshot nested under ignored `target/` without its own Git boundary selected the wrong workspace schema. An independent reversible diagnostic isolated that harness cause. A fresh snapshot with empty independent Git metadata exposed all13 expected functions and passed the second gate without product-source/Make changes. [Attempt1 evidence](docs/evidence/root-promotion/attempt-1-validation.json) remains unchanged. [Final evidence](docs/evidence/root-promotion/validation.json), [source manifest](docs/evidence/root-promotion/source-manifest.json), [artifact identities](docs/evidence/root-promotion/artifacts.json) and [promotion index](docs/evidence/root-promotion/README.md) bind the actual commands and outputs. Final independent assurance closure is recorded separately in [REVIEW](docs/evidence/root-promotion/REVIEW.md). Later documentation-only closure does not change core build inputs. No commit, push, PR, merge, tag, upload, installation or deployment was performed by this cutover.

## Most recent pre-promotion results

### Installer candidate pipeline

[Pipeline evidence and review](docs/evidence/installer-pipeline/README.md) retain the concurrent task's actual outcomes and prior failures. Official checksum-verified Dagger **v0.21.9** with explicit module selection passed configured `make test installer/build`: **263 service + 281 native installer + nine GTK + 668 Flutter + 41 package = 1,262 tests**, five controlled fixture ignores, no Flutter/package skips. The separate final helper suite passed30/30 including actual rebuilt-package round trip under synthetic Git identity. These commands ran **before relocation**, not in this task.

The pipeline now uses Ubuntu-only validation and annotated canonical release-tag candidate archives, not automatic GitHub Release publication. Root-promotion repairs its helper/test/workflow source paths and removes retired root-Cargo CI gates. Fresh tests for those repairs and actual hosted execution are distinct evidence. Retained compatibility files do not establish effective image-lock enforcement/hermeticity on the official Dagger CLI.

### Saved-key eligibility and desktop checkpoint

[Saved-key evidence](docs/evidence/saved-key-attempts/README.md) retains the earlier **1,262-test PASS**, original manifests, artifacts and initial client failures. [Checkpoint evidence](docs/evidence/cloud-checkpoint/README.md) and retained independent reviews distinguish privacy/source assurance from release approval. Observation/prior validation remains advisory; explicit dispatch retrieves the exact saved generation and preserves context consent, uncertainty and no retry/fallback.

Historical records retained locally preserve their original bytes, hashes, commands and `next/...` paths. Their relative links/log locations refer to the original checkpoint/layout, not a promise that retired documents or ignored raw outputs exist in the promoted checkout. Use [History](docs/HISTORY.md) and the disposition map for original paths.

## Earlier history and remaining boundaries

The full earlier validation ledger is [pinned at the pre-cutover checkpoint](https://github.com/jdharrison/smith/blob/168865b0422b787f08ab33e89d052cd4adbd11f7/next/VALIDATION.md). Earlier failed aggregates remain failed historical executions; later passes do not establish causes or fixes for runner timing/lease failures. Retired plans, reports and excluded voice/Python-installer snapshots remain recoverable through reachable Git objects, not copied archives.

Source/widget/fake/compiled-socket/build evidence is not native graphical/IME/multi-monitor acceptance, real provider/keyring/model/GPU proof, installed migration/restore, clean-host qualification, license/SBOM/provenance clearance, publication or deployment approval. Supplied-icon redistribution and other recorded release limitations remain outstanding. Current disk4 rollback requires separately approved pre-migration state, compatible paired binaries and exact keyring-generation references; binary-only downgrade is invalid. Automatic model discovery/catalog lifecycle remains unimplemented.
