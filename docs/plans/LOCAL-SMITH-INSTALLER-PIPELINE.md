# LOCAL-SMITH-INSTALLER-PIPELINE

## Intent and authority

**Local implementation scope:** the operator requested a final autonomy-branch pipeline pass so a `release/v<version>` tag builds `smith-installer_v<version>` automatically. This record bounds that request to local workflow, packaging-helper, test and documentation changes. It grants no local commit, branch creation, push, tag, merge, artifact upload, GitHub Release publication, deployment or accepted-risk authority.

- Branch: existing `feature/autonomy` (unchanged).
- Base revision: `168865b0422b787f08ab33e89d052cd4adbd11f7`.
- Risk: **critical**, because the legacy automatic release path is being replaced.
- Supported package: complete current `next/` native installer/client/service, Ubuntu 24.04 LTS x86_64 only.
- User-requested example: `release/v0.1.0` → `smith-installer_v0.1.0.tar.gz`, containing directory `smith-installer_v0.1.0/`. Internal executable names remain unchanged.

## Independent analyses and reconciliation

Producer, architect and designer independently inspected the request, policy and implementation in fresh contexts before implementation. Findings are retained here as planning inputs, not approval or executed release evidence.

| Concern / disagreement | Resolution |
|---|---|
| Existing release workflow detects tags but builds the legacy root backend for multiple platforms and publishes automatically. | Replace it with build-only native installer candidates; remove automatic GitHub Release creation and write credentials. |
| `release/*` may mean branches or tags; main merge alone supplies no release identity. | PRs targeting `main` and pushes to `main` validate/build without retained distributable uploads. Only pushed tags under `release/*` request a named candidate; validate canonical stable `release/vMAJOR.MINOR.PATCH` before expensive work. Release branches alone do not trigger. |
| Building versus publication authority. | Tag workflow retains a verified Actions archive/evidence only after checks pass. Upload is still external disclosure; enabling/triggering it needs approval for destination/visibility/retention and redistribution clearance. Public GitHub Release creation remains a separate manual approved action, never inferred from a green job. |
| Existing local `release/v0.1.0` is lightweight and points to older `1c3e621`, not the current product. | Do not mutate it. Treat this as a real-release blocker, not a reason to weaken checks. Remote status is unknown. A human must resolve version availability; never retag a released version. |
| Tag/archive names alone do not bind source or component versions. | Require annotated tag, exact HEAD/expected commit, membership in fetched main history, clean tracked source, matching next installer/service Cargo and Flutter release versions. Ignore only Flutter numeric `+build` metadata for matching. Retain exact versions in build metadata. |
| Packaging a single executable would be unusable. | Reuse `make test installer/build`, package every verified `installer/dist/` file in a tar archive preserving modes, extract into fresh private staging and reverify before candidate acceptance. Keep build metadata outside the recipient root's strict allowlist. |
| Shipped instructions describe obsolete screens/lifecycle behavior. | Align embedded `next/dagger/INSTALLER.md` with current two-action GUI, separate Enable/Start, curated models, optional providers, explicit Launch and preserved data. |
| Source/build success versus recipient/release readiness. | Keep clean-source provenance, real installed/graphical/model/restore acceptance, supplied-icon rights, licenses/SBOM and exact human release approval as explicit outstanding publication gates. |

## Traceability and implementation plan

| REQ / blocking acceptance criterion | Task / files | Required evidence |
|---|---|---|
| REQ-1 / AC-1: canonical valid tag identifies exact annotated main-history commit; versions match; malformed/lightweight/off-main/mismatched/dirty cases refuse. | TASK-1: tested builder-only release helper under `scripts/`, isolated Git fixtures under `scripts/tests/`. | Positive and negative unit tests; no real repository tag mutation. |
| REQ-2 / AC-2: complete paired package is archived with requested stem, safe members/modes, checksum and source/build record; tampering/missing payload refuses. | TASK-2: release helper reuses existing `next/dagger/package.py`, verifies fresh archive extraction. | Synthetic archive round trip/negative tests, actual rebuilt package round trip separately labelled with exact artifact identity. |
| REQ-3 / AC-3: PR/main validate current product and legacy Ubuntu checks; tag builds only; required failure prevents distributable upload. | TASK-3: `.github/workflows/ci.yml`, `release.yml`, pinned checksum-verified CI Dagger setup. | Workflow lint/static invariants, helper tests, configured aggregate/package build, failure-path inspection. |
| REQ-4 / AC-4: no automatic tag/release/deploy, write token, user-session access, live models/providers or platform expansion. | TASK-4: read-only permissions, credential-free checkout, no secrets/cloud token, bounded jobs/ref concurrency, no raw-directory artifact upload. | Independent architecture and fresh adversarial workflow review. |
| REQ-5 / AC-5: operator can distinguish candidate/release, acquire/verify/extract/launch full package and understand prerequisites/partial outcomes. | TASK-5: affected canonical architecture/process/design, developer/recipient docs and current implementation assessment. | Document review; existing isolated runtime/package gates, fresh archive checks. Live recipient QA remains separately approved. |

## Validation and evidence

1. Run pipeline-helper unit tests, shell syntax, workflow lint/security inspection and `git diff --check`.
2. Verify the checksum-pinned Dagger CLI matches module `v0.21.9`; run the configured `next/` aggregate tests and full recipient package build with cloud tracing disabled. Bound execution time and retain failures rather than silently retrying.
3. Verify the rebuilt recipient package and fresh archive round trip. Bind results/digests to changed inputs; synthetic Git/version fixtures are not clean-clone provenance.
4. Obtain fresh independent assurance/adversarial review of the diff, tests and retained evidence; remediate concrete defects without weakening policy.
5. Record actual outcomes below and in `next/VALIDATION.md`; never label unexecuted GitHub runs, clean-host acceptance or publication as PASS.

## Rollback and external rollout

Changes are local configuration/tooling only. Before activation, inspect the exact merged commit and workflow permissions. If faulty, prepare a reviewed correction or disable the affected workflow through separately authorized repository administration; do not restore the unsafe automatic legacy publisher. A failed candidate leaves no release and must not cause tag movement or automatic retry/rollback. Once uploaded, Actions artifacts have explicit 14-day retention and repository-dependent visibility; publication and installation remain distinct effects.

**Human-owned, before any external rollout:** resolve the old version/tag identity, clear redistribution rights and other required release evidence, approve the exact push/tag/upload destinations and revision/effects/expiry, then independently approve any GitHub Release publication. No GitHub settings or remote refs have been inspected or changed in this task.

## Builder compatibility amendment

**Observed during implementation:** official Dagger v0.21.0 does not discover Smith's `dagger.toml` entrypoint (`No functions found`, exit 0); explicit `--mod dagger` reaches the Python SDK but fails with an upstream OpenTelemetry Go import cycle. The previously installed local v1.0.0-beta.14 is not available at the queried upstream GitHub release tag (404); this does not establish that the binary is a fork. Historical beta-based results remain unchanged. Logs: `target/installer-pipeline/dagger-module.log`, `dagger-explicit-module.log`, `dagger-sdk-debug.log`.

Independent architecture review recommends official checksum-pinned **v0.21.9**, matching module `engineVersion`, and explicit `--mod dagger` in every Make recipe, outside an overridable `DAGGER` executable variable. These are builder-only compatibility corrections, not recipient behavior changes. Require actual function discovery, engine identity, effective input/lock behavior and full configured gates before claiming resolution; no automatic fallback to the beta. Preserve existing `dagger.toml`/`dagger.lock` compatibility files and do not claim their digests are enforced by official Dagger without verification. Dagger v0.21.0 startup automatically replaced the previous cached beta build-engine container; no Smith application/service lifecycle action was run. Further engine checks use a separately named retained test engine.

## Results

**Implemented local workflow/tooling and self-check PASS; publication remains NO-GO.** [Evidence index](../evidence/installer-pipeline/README.md) and [machine-readable results](../evidence/installer-pipeline/results.json) retain the actual commands, source/artifact identities and limitations. Official checksum-verified v0.21.9 explicit module discovery and `make test installer/build` completed, exit0: 263 service,281 native installer,nine GTK,668 Flutter,41 actual-ELF package tests passed (1,262 total; five controlled fixture ignores). All1,230 inventoried component/tooling inputs remained unchanged across that gate. The95-file recipient package, matching helpers and current embedded instructions verified. Legacy root fmt/locked Clippy/build/tests passed (127 distinct tests).

Independent assurance reproduced pinned checkout's default annotation-stripping fallback. Both candidate checkout steps now select the full event tag ref, retaining event-commit/HEAD/main-history binding. Real-Git plus static both-checkout regressions cover the repair. Final separate helper/tooling tests passed30/30 with zero skips, including actual rebuilt-package archive verification under synthetic fixture identity. Actionlint1.7.12, ShellCheck, shell syntax and whitespace checks passed. Post-aggregate workflow/comment/test and developer README wording changes are separately recorded; executable component/tooling logic and recipient payload stayed unchanged. Fresh reviewer disposition is retained in the evidence directory, not inferred from these self-checks.

No local commit, user-repository ref mutation, remote action, release/deployment or accepted-risk approval occurred. Concurrent `LOCAL-SMITH-ROOT-PROMOTION.md` planning was preserved and is outside this change; any later approved path promotion must reconcile these workflow/helper/test paths. Actual GitHub execution/settings, version collision resolution, redistribution/license/SBOM, clean-source/recipient/restore evidence and exact human publication approval remain pending.
