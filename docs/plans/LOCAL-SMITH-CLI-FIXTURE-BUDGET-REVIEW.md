# CLI fixture budget — independent scoped review

- **Scope:** static assurance and final retained-evidence verification of the test-only repair against base `766d63b481702c21f5fee6ca9c2dc64c30a4d80d`; requirements F1–F5 in [the approved plan](LOCAL-SMITH-CLI-FIXTURE-BUDGET.md).
- **Authority:** user-requested local review only. No implementation edits, tests, hosted reruns, staging, commits, pushes or live-host effects performed.
- **Evidence boundary:** implemented source and retained final logs/hashes inspected independently; tests were executed by the parent, not rerun by this reviewer. Historical diagnostic numbers remain attributed plan evidence. This final review supersedes the earlier verification-pending verdict.

## Checks performed and findings

`git rev-parse HEAD` resolved to the stated base. `git diff 766d63b --stat` showed only `installer/tests/cli.rs` (9 insertions, 2 deletions). SHA256 of `git diff 766d63b -- installer/tests/cli.rs`: `1a2444b8df304ed1a5b16dc47d539a9a24f596c8c9907441d087cf5e7b5b5771`. The plan was untracked at review time.

**Critical/high/medium/low implementation findings: none. No static code blockers identified.**

1. **Actual installer preserved (F1).** `Fixture::new` still copies `CARGO_BIN_EXE_smith-installer` to the executed package entrypoint with mode0755 (`installer/tests/cli.rs:219–221`). Commands execute that path, not `/usr/bin/true`. The help/version test still checks the actual installer's ELF64/little-endian/x86_64 identity. The retained-writer regression also executes the Cargo-built installer directly.
2. **All15 CLI test bodies reviewed (F2/F3).** Two parsing tests, two capture/drain tests and pure root/platform preflight do not depend on changed payloads. The remaining ten tests retain their publication, ancestor-drift/interleaving, help/version, exit/consent, partial-effects, read-only, executable-root/bundle-selection and artifact-only assertions. None requires installer-sized inert bytes or executes those bytes as a client/service. Three separate writes retain0755 modes and identical paired helper bytes; no hardlink substitution. Namespace ownership/cleanup, parallelism, environment isolation,30s child deadline, output caps and all original assertions are unchanged. Smaller payloads deliberately remove incidental large-file/hash-cost stress, not a stated acceptance assertion; this is not a large-payload performance guarantee.
3. **Builder prerequisite is appropriate (F2).** The configured quality builder is Ubuntu24.04 (`dagger/src/smith_next/main.py:110–128`); its standard coreutils `/usr/bin/true` is a genuine native executable, not a fabricated ELF header or script. Reading it as inert bytes adds a builder fixture prerequisite, not a shipped/recipient dependency or process execution. Missing/non-ELF/>1MiB content fails explicitly with no fallback. The four-byte magic guard is not full ELF/architecture validation; the trusted supported builder supplies that provenance. The configured Ubuntu builder's complete native suite now passes with these guards enabled.
4. **Separate actual-ELF coverage unchanged (F4).** Dagger still supplies compiled CLI/GUI binaries to `dagger/tests/test_package.py`, whose native help/dry-run/alias/decline cases remain configured. Its synthetic service/runtime fixtures are not actual product execution coverage. Separately, `test_native_runtime` supplies the real built installer and service to a no-Python Ubuntu gate, and CI still builds/verifies the complete paired package (`.github/workflows/ci.yml:74–82`). This repair neither replaces those binaries nor weakens their checks. Retained logs now confirm the configured actual-entrypoint package tests and real installer/service no-Python runtime gate passed. A fresh complete distributable build/export was not part of these selected gates.
5. **Historical cause stated honestly (F5).** The plan distinguishes the reported hosted14-pass/1-timeout outcome from the parent's controlled large-versus-small payload comparison. That comparison supports a reproducible payload-cost trigger, not proof of the historical hosted CPU, scheduler or SHA backend cause. No hosted recovery/acceptance claim is justified yet.

## Required Checks Evidence

Final evidence: [README](../evidence/cli-fixture-fix/README.md), [results](../evidence/cli-fixture-fix/results.json), [source manifest](../evidence/cli-fixture-fix/source-manifest.json). Raw logs and execution metadata remain under `target/cli-fixture-fix/`.

| Check / command or scenario | Expected | Actual / status | Evidence source |
|---|---|---|---|
| Base-bound diff and all15 test bodies | Only inert payload source changes; invariants retained | PASS; same diff digest as initial review, no production/workflow changes | Git diff/status; `installer/tests/cli.rs` |
| Formerly failing exact Rust test, reported four same-CPU competitors | Original assertions finish within unchanged30s budget | PASS, exit0;0.3245s recorded wall time; log reports1 pass/14 filtered,0.27s test time | `constrained-cli.log`, `constrained-result.json`, compiler-artifact records |
| Direct locked/offline all-target tests | Native suite including all15 CLI tests passes | PASS, exit0;281 passed,0 failed,two existing controlled fixture ignores | `native-tests.log`, `native-validation.json` |
| Direct formatting / locked offline all-target Clippy `-D warnings` | Both exit0 | PASS; empty format log and successful Clippy completion; exit0 metadata | `format.log`, `clippy.log`, `native-validation.json` |
| Configured `make installer/test package/test` with retained official Dagger0.21.9 | Native quality, isolated GTK, no-Python runtime and actual-entrypoint package gates pass | PASS, exit0;176.867s metadata;281 native +9 GTK +41 package =331;two native ignores,zero GTK/package skips | `configured-gates.log`, `configured-gates.json`; unchanged Dagger definitions |

### Independent binding checks

- Recomputed all six raw-log SHA256 values in `results.json`: all match. Parsed Rust summaries independently: direct281 passed/0 failed/2 ignored; configured290 passed/0 failed/2 ignored including9 GTK. Package log explicitly reports41 tests and `OK`; native15-test CLI summary is present in both runs. Direct/constrained passes are not added again to331.
- Recomputed compact sorted-key manifest digest: `ba7d0348f3ec7808086029bd4cf881260e850c2e552ae20276c7bd7b42023ac1`; all1,062 listed current file hashes match. The retained configured input map equals that manifest and records unchanged before/after inputs. All1,052 direct snapshot records also match both the retained scratch source and current files. Changed test digest matches `becb112caaebdafb3b08f53de2c4889df4f9b2aaca78f494f446a5f741dd53d8`.
- Configured log names the retained Dagger executable and client/server0.21.9. Its retained archive SHA256 is `33eea0b08d6be444bada18e64b2216459100d6070abcb4b5345cee40c9fff982`, matching `scripts/install-dagger-ci.sh`; archive binary bytes match the used executable. No fresh engine or hosted execution was inferred.
- Logs show unchanged `scripts/test.sh`, isolated `scripts/test-ui.sh`, no-Python checks, real service artifact dry-run and actual compiled package-entrypoint tests. No validation-disable/workflow change exists. Additional tracked changes are outcome-only `VALIDATION.md`; plan/review/evidence documentation is outside the selected core-input manifest.

## Evidence Gaps and decision

The constrained launcher was executed directly from the parent's terminal; no separate launcher script is retained. Its log verifies the exact test passed; four-competitor topology and outer wall/CPU measurements are recorded parent metadata, not independently reconstructed observations. Compiler-artifact records and matching scratch/current source bind the actual test implementation. This is adequate corroborating local evidence alongside complete configured gates, not proof of historical hosted CPU conditions or a newly reproducible archived contention harness.

Raw logs are ignored local artifacts, not guaranteed Git-recoverable evidence. No full service/Flutter aggregate, fresh complete distributable export, installation or hosted rerun is claimed or required for this bounded test-only closure. Hosted run38982647442 remains failed until a separately authorized push/rerun actually passes; exact historical slowdown remains unknown.

**Assurance verdict: PASS for F1–F5, scoped local fixture repair and selected validation. No implementation blockers or required remediation remain.**

**Release Gate Decision: no-go for publication/deployment (outside this review and authorization); go for local repair closure only.** This is an independent recommendation, not human approval, and grants no commit, push, hosted rerun, merge, release or deployment authority.
