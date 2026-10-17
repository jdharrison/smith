# Installer pipeline self-check — 2026-10-16

**Implemented local workflow/tooling and self-check evidence, not hosted GitHub execution, clean-source provenance, release or deployment approval.** Work is bounded by [the pipeline plan](../../plans/LOCAL-SMITH-INSTALLER-PIPELINE.md), base `168865b`, existing `feature/autonomy`.

## Results

- Official checksum-verified Dagger CLI/server **v0.21.9** successfully discovered the module with explicit `--mod dagger` and completed `make -C next test installer/build` using the verified executable override. The bounded 20-minute command exited **0**.
- **1,262 tests passed:** 263 service, 281 native installer, nine isolated GTK, 668 Flutter and 41 actual-native-ELF package tests. Three service/two installer controlled fixture entry points remain ignored by their ordinary harness; Flutter/package skips were zero. Formatting (81 files, zero changes), analyzer, warning-denied Clippy, locked build/tests and interpreter-free CLI execution passed.
- The exported full native recipient package verified **95 manifest-listed files**, including matching standalone/bundled helpers and current embedded recipient instructions. The separate final helper/tooling suite passed **30/30, zero skips**, including an actual rebuilt-package archive/mode/checksum round trip with explicitly **synthetic isolated Git source identity**. That fixture is not real release provenance.
- Legacy root Ubuntu Rust formatting, locked Clippy/build/tests passed; **127 distinct tests** (the nested single-test subprocess is not an additional distinct test).
- Actionlint **v1.7.12**, ShellCheck, shell syntax and tracked-diff whitespace checks passed.

[Machine-readable results](results.json) retain exact commands, results, local log locations/digests, source/tooling/package identities and limitations. [Build-source manifest](build-source-manifest.json) records **1,230 component/tooling/configuration inputs**, unchanged across the aggregate. [Sanitized aggregate excerpts](aggregate-excerpts.log) retain gate summaries; full logs remain under ignored `target/installer-pipeline/`, not copied wholesale into distributable artifacts or documentation.

The checkout correction, pin-comment fix and regression tests followed the aggregate and were separately tested/linted. A later developer README wording-only change does not alter executable tooling or the recipient README/payload; the source manifest remains the actual gate snapshot, not a retroactive hash rewrite. Final workflow/helper hashes are in `results.json`.

## Failures and independent review

Preserve these findings rather than treating the final pass as if no earlier failure occurred:

1. Official Dagger v0.21.0 returned `No functions found` when relying on the retained beta workspace registration. Explicit module loading then failed in the upstream Python SDK's Go runtime with an OpenTelemetry import cycle. The official v0.21.9 patch plus explicit module selection passed actual discovery and aggregate gates. The queried v1.0.0-beta.14 GitHub release was unavailable (404); this does not establish that the previously installed binary is a fork. Compatibility workspace files are retained, **without claiming their image pins are enforced by the official CLI**.
2. Initial actionlint/ShellCheck findings were corrected without disabling checks. Test-authoring fixture failures were repaired before the passing tooling suite.
3. Fresh assurance reproduced pinned checkout v4.2.2's default annotated-tag fallback replacing the local tag annotation. Both candidate checkouts now explicitly select the full event tag ref while retaining exact expected-commit/HEAD/main-history checks. A real-Git regression reproduces the old failure and verifies the corrected path; a static regression covers both workflow checkouts. This is not a hosted Actions simulation.

Fresh reviewer disposition is retained in [REVIEW.md](REVIEW.md). The reviewer may recommend local acceptance but cannot approve publication or accept release risk.

## Effects and unresolved gates

No commits, user-repository ref mutation, push, merge, GitHub artifact upload, release publication, installed Smith upgrade, real model download/inference or vendor/keyring operation was performed. Fixture repositories use temporary local commits/tags/remotes only. Dagger dependency/image downloads and builder engine/cache changes occurred: initial v0.21.0 startup automatically replaced the previous beta build-engine container. Later validation used a separately named v0.21.9 engine, preserving other engines and Smith services. That dedicated validation engine was stopped after the checks; its container/cache is retained for inspection.

The existing local lightweight `release/v0.1.0` points at an older commit; remote/publication status is unknown. Do not move/reuse a released version. Resolve the intended version/revision and matching manifests/lockfiles before any approved tag action.

**Release remains NO-GO:** hosted workflow/settings behavior, clean-source reconstruction/provenance, effective image locking/hermeticity, real prepared-recipient graphical/lifecycle/model/compatibility/restore acceptance, redistribution rights (including supplied icons), licenses/SBOM and exact human publication approval remain separate gates. Actions upload is itself external disclosure; repository-dependent visibility and 14-day retention need approved activation. No green local check establishes that authority.
