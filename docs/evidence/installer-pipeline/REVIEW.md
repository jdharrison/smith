# Independent installer-pipeline assurance re-review — 2026-10-16

## Validation scope

- Re-review of the local installer-pipeline implementation against base `168865b0422b787f08ab33e89d052cd4adbd11f7`, the pipeline plan's REQ/AC-1–5, canonical policy and `roles/assurance.md`.
- Independently checked both prior findings, final workflow/helper identities, retained aggregate evidence, source snapshots and rebuilt package integrity; reran focused tests and lint.
- No aggregate rerun, engine start, code edits, real-repository ref mutation or remote operation. This review file is the only authored change. Concurrent root-promotion planning is excluded.

## Findings and disposition

### High — annotated-tag checkout: closed

Both checkout steps in `.github/workflows/ci.yml` now explicitly set `ref: ${{ inputs.release-candidate && github.ref || '' }}`. For candidates, the full event tag ref avoids the pinned checkout action's default event-commit fallback that replaced the annotated local ref. Noncandidate runs retain default checkout behavior. The source helper still requires annotation, canonical version, exact expected commit/HEAD, main-history membership, clean tracked source and component/lock version agreement; no safety requirement was relaxed.

The new real-Git regression independently passed: it recreates annotation loss on the default path and verifies intact annotation plus source acceptance on the explicit-tag path, including a release that is an ancestor rather than the tip of main. A separate static regression verifies the correction in both jobs. These are local Git/static checks, not execution of hosted GitHub Actions.

### Low — pin provenance comment: closed

`scripts/install-dagger-ci.sh` now consistently identifies v0.21.9 in its provenance comment, URL and version. Final source digest matches the retained result record. The reviewed archive pin remains `33eea0b08d6be444bada18e64b2216459100d6070abcb4b5345cee40c9fff982`.

### Remaining critical/high/medium/low findings

None established within the bounded local implementation scope. This is not a finding that all release or installed-host requirements are satisfied.

## Required Checks Evidence

Paths below are repository-relative. Retained evidence is self-check evidence, not signed provenance. Commands rerun by the reviewer are distinguished from inspected builder results.

| Check | Command/scenario; expected outcome | Actual outcome/status | Evidence |
|---|---|---|---|
| Final helper/tooling and checkout regressions | Reviewer reran `PYTHONDONTWRITEBYTECODE=1 SMITH_TEST_PACKAGE=next/installer/dist python3 -m unittest discover -s scripts/tests -p 'test_*.py' -v`; all tests pass, including actual package round trip | Exit 0; 30 passed, zero skips/failures | Review execution; retained earlier run in `target/installer-pipeline/final-helper-tests.log` and `final-helper-result.json` |
| Workflow and shell lint | Reviewer reran `target/installer-pipeline/actionlint .github/workflows/ci.yml .github/workflows/release.yml`, `shellcheck scripts/install-dagger-ci.sh`, `sh -n scripts/install-dagger-ci.sh`; no errors | All exit 0 | Review execution; configurations identified by `results.json` |
| Whitespace | Reviewer reran `git diff --check 168865b`; no errors | Exit 0 | Review execution |
| Aggregate native/current product gates | Inspect completed `make -C next DAGGER=<verified-v0.21.9-cli> test installer/build`; successful tests, export and verification | Recorded exit 0; full log ends with successful export, 95-file verification and Make completion. Independently parsed 263 service, 281 installer, nine GTK, 668 Flutter and 41 package passes: 1,262 total. Three service/two installer fixture ignores; Flutter/package skips zero | `results.json`, `aggregate-excerpts.log`, full `target/installer-pipeline/aggregate.log` |
| Executed engine identity | Inspect full aggregate log; reviewed CLI/server version on both calls | Both connections report client/server v0.21.9 | Full aggregate log |
| Aggregate source stability | Compare all manifest records against retained before/after snapshots; identities unchanged during gate | All 1,230 records match both snapshots | `build-source-manifest.json`; `target/installer-pipeline/inputs-before.json`, `inputs-after.json` |
| Final source and retained log binding | Independently recompute SHA-256 for every `final_tooling_sources` entry, aggregate/helper logs and four legacy logs | All recorded hashes match; four legacy checks record exit 0 | `results.json`; corresponding local logs |
| Rebuilt recipient package | Reviewer reran `PYTHONDONTWRITEBYTECODE=1 python3 next/dagger/package.py --root next/installer/dist --verify`; full package intact | Exit 0; 95 manifest files. Manifest/service digests match result record; standalone and bundled service bytes match; embedded README bytes equal `next/dagger/INSTALLER.md` | `next/installer/dist`; `results.json`; review execution |
| Legacy root gates | Inspect retained `cargo fmt --check`, locked Clippy/build/test and hash-bound logs; successful results | All recorded exit 0; no conflicting evidence found | `results.json`; `target/installer-pipeline/root-rust-validation-s50_n95c/` |

Verified aggregate log SHA-256: `eb1c16b03e9f28984fc9d909b56f955f21331661b92585c824723a062f9f0c7f`.

Verified build-source manifest SHA-256: `c587cd29a698fe7d13f91660679abe7820c52bbef93e09c847b8d158e31271ab`.

Verified rebuilt package manifest SHA-256: `94d90e5c0ba36219df39063cad753c1de4166fa4124d985806bed8d6644178d9`.

## Evidence boundaries and gaps

- The aggregate was completed before the checkout/comment/test corrections. Those final sources are hash-bound in `results.json` and were separately exercised by the reviewer's passing tests/lint. No aggregate rerun is represented or required for these workflow/comment/regression-only corrections.
- Comparing the 1,230 build inputs to the current worktree finds only the disclosed `next/dagger/README.md` change. Its reviewed changes are developer documentation, not executable logic or embedded recipient instructions. The original gate snapshot is preserved, not rewritten to claim testing of later bytes.
- The actual-package round trip uses synthetic isolated Git identity. It proves package/archive integrity and modes, not that a real release tag built this archive. Its new archive digest need not equal a prior fixture archive's digest; deterministic archive reproduction is not claimed.
- Hosted GitHub event, checkout/action execution, artifact retention/visibility and repository settings remain unexecuted/unverified. Failure-to-upload gates are supported by source inspection/static tests, not a hosted failure-injection run.
- Effective enforcement of the retained beta-workspace image lock remains unverified. v0.21.9 CLI/server execution is established; hermeticity, clean-cache reconstruction, immutable dependency resolution and signed/clean-source release provenance are not.
- Real prepared-recipient graphical/lifecycle/model/GPU/provider/compatibility/restore acceptance, redistribution rights including supplied icons, licenses/SBOM, intended version/tag availability and exact human approval remain release gates. The existing local old lightweight tag is not made suitable for reuse by this review.

These are explicit deferred release gates, not implemented guarantees or accepted residual risks. No required bounded local implementation check remains pending on the evidence inspected.

## Scoped verdict

**Verdict: pass — local implementation/self-check gate only.** The prior high and low findings are closed. The implementation meets the bounded local pipeline REQ/AC on inspected source and executed local evidence; end-to-end hosted and recipient behavior is not established. Local acceptance is a recommendation, not self-approval, release approval or authority for external effects.

**Release Gate Decision: no-go.** No tag push, workflow activation/artifact disclosure, GitHub Release publication or deployment is authorized.

**Required remediation:** none outstanding for the two findings or the bounded local implementation gate. Before external activation/publication, resolve the version/tag identity, complete the applicable release evidence and redistribution gates, and obtain exact human approval bound to revision, effects, destination/visibility/retention and expiry. Preserve these limitations in any handoff.
