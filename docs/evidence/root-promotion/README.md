# Root promotion evidence

## Approval and scope

The operator approved [LOCAL-SMITH-ROOT-PROMOTION](../../plans/LOCAL-SMITH-ROOT-PROMOTION.md), including legacy root CLI/source retirement, unchanged installed identities and Git-backed historical retention. Follow-up approval permits intact preservation of leftover directories and clearing identified disposable caches. This task performs local source edits and isolated rebuild validation only; no commit, remote action, live installation/service/keyring/provider/model effect or release is approved.

## Exact pre-cutover inventory and preservation

[disposition.json](disposition.json) records all **1,721** tracked entries at `168865b0422b787f08ab33e89d052cd4adbd11f7`: **1,231 promotions / 454 retirements / 34 retained / two README replacement dispositions**, with baseline blob/mode/hash, current pre-cutover identities and recovery paths. It also records 24 modified/new inputs from concurrent pipeline/planning work and all53 archived payload checks. No newly created source is claimed to exist in the old checkpoint.

Independent pre-move assurance session `a5d89b50-b1cb-41b7-9736-c284c34ba5fe` verified every baseline/retired/promotion identity, all53 reference payloads, destination collisions, captured concurrent work and index/base/branch. **PASS for pre-move only**, manifest SHA256 `950c2f930441b3b71d85c4e02bce95d4b44e558a8d58b0eb9e3c680475eabc0f`. The manifest's pre-cutover meaning is immutable; it does not claim final-source validation.

Tracked files were moved individually. The old entire client namespace and all next-layout leftovers were preserved at ignored `target/root-promotion/preserved-legacy-client/` and `preserved-next-outputs/`; original concurrent modifications were copied into `pre-cutover/`. Thus stale local caches do not overlay the replacement client. No cache deletion was necessary. None of these ignored originals is claimed to be retained by Git. The migration's source operation log remains in ignored scratch; Git index/refs were untouched.

Historical records remain original evidence, not newly valid guarantees. [History](../../HISTORY.md) gives immutable retrieval links. Retained pre-cutover evidence may contain original relative links and `next/...` paths; read them in their checkpoint context. New root validation must be independently bound.

## Fresh validation and artifacts

[Final validation](validation.json), [source manifest](source-manifest.json), [component results](component-results.json) and [artifact inventory](artifacts.json) record a fresh promoted-root **PASS**: **263 service /281 native installer /nine GTK /668 Flutter /41 actual-ELF package tests** (1,262 aggregate), zero failed; three service/two installer controlled fixture ignores and zero Flutter/package skips. Formatting81/zero changes, analyzer and warning-denied Clippy passed. The final helper suite passed34 with zero skips, including the actual rebuilt-package archive round trip. Actionlint/ShellCheck/shell syntax and root-path negative fixture checks passed in the scoped pipeline work.

The initial discovery failure is retained verbatim in [attempt1](attempt-1-validation.json), [its source manifest](attempt-1-source-manifest.json), [component status](attempt-1-component-results.json) and [artifact status](attempt-1-artifacts.json). An independent reversible diagnostic established that an ignored nested snapshot without an independent Git boundary selected a different workspace schema; adding/removing an empty scratch Git boundary changed discovery with no source-byte changes. Attempt2 used a fresh independent snapshot and asserted all13 Smith functions before running the configured gate. No product or Make workaround was added, and the failed initial command is not relabelled PASS. Diagnostic/raw logs remain ignored under `target/root-promotion/diagnostic-discovery/` and `validation-v2/`.

All **1,230 core inputs** stayed stable in snapshot and live original, inventory SHA256 `0617ce1780c0f88d5f325c4ee9e104aaf61e38905a3d1a6544945fc586a82670`. The complete source snapshot includes1,288 files; generated evidence/doc closure is distinguished from core inputs. Later changes close current documentation/navigation only. Verified exports were copied to originally absent root `service/dist/`, `client/dist/`, `installer/dist/`; bytes/modes match snapshot exports, all four service copies and client/package pairs match. Recipient verification accepted95 manifest-listed files/97 total. Service ELF SHA256 `5cfe584b8e597771319b3d4a3da9572bd30df9059d89f4f68e6e38fd5b2c4dcb`; package manifest SHA256 `52804bfae4a71ff783cff3a0c299e7e9b491b2d79c9d05ea22c0eafe3e7d5102`.

[Fresh full-cutover assurance](REVIEW.md) records source review and final evidence closure separately. Source/build acceptance is not source-clean Git provenance, actual hosted CI, installed acceptance or a release. The final candidate remains uncommitted and unstaged; exact commit/remote approval is still required.

## External and installed effects

No commit, push, PR, merge, branch deletion, tag, artifact upload, GitHub Release, app launch, service/user-manager operation, model download, real credential/provider request, installed migration or deployment is part of this cutover. Existing release/host/legal limitations remain; no accepted-risk decision is made.
