# Installer CLI fixture budget repair

## Implemented scope

Under [LOCAL-SMITH-CLI-FIXTURE-BUDGET](../../plans/LOCAL-SMITH-CLI-FIXTURE-BUDGET.md), only `installer/tests/cli.rs` changes executable test behavior. `Fixture::new()` still copies and executes the actual Cargo-built installer. Its three non-executed client/service payload files now use the supported Ubuntu builder's genuine `/usr/bin/true` ELF, with ELF magic and≤1MiB guards. The copies remain independent, equal and0755. Existing assertions, namespace isolation, consent/integrity checks,30s watchdog and output bounds are unchanged. No production, workflow, dependency, profile or recipient behavior changed; validation remains enabled.

## Diagnosis and limits

Hosted run [38982647442](https://github.com/jdharrison/smith/actions/runs/38982647442) at `766d63b` failed the30s CLI fixture watchdog (14 passed/one failed), not a component compilation or workflow timeout. Historical hosted scheduling/backend details were not recorded.

Diagnostic evidence remains ignored in `target/ci-diagnosis/`. Unchanged exact test passed locally in6.98s; software-SHA control passed in8.73s, so missing acceleration alone was not established as the cause. Under a fixed synthetic CPU-contention comparison, the same executed installer with three16.6MB payloads timed out in30.03s; three small genuine ELF payloads completed in0.15s and satisfied every original outcome check. Published service bytes in the timed-out comparison prove consent consumption/publication progress, not an input hang. This establishes a reproducible avoidable fixture-cost trigger, **not the precise historical hosted cause**. Scratch unsafe-ancestor preflight failures were retained separately, not reported as performance observations.

## Executed fixed-source checks

[Results](results.json) and [source manifest](source-manifest.json) bind commands, exits, input/log hashes and scope.

| Check | Actual result |
|---|---|
| Formerly failing actual Rust test under four same-CPU synthetic competitors | PASS,0.3245s, unchanged30s watchdog; all original assertions executed |
| Direct locked/offline all-target native installer tests | 281 passed/two controlled fixture ignores; all15 CLI tests passed |
| Rust formatting / locked all-target warning-denied Clippy | PASS |
| Configured `make installer/test package/test`, official verified Dagger v0.21.9 | PASS,176.867s;281 native +nine isolated GTK +41 actual-ELF package tests =331 passed |
| Interpreter-free Ubuntu CLI/runtime gate | PASS within configured installer gate |
| Selected input stability | All1,062 inventoried installer/service/Dagger inputs unchanged before/after |

The direct281 and constrained one-test results are additional checks, not added again to the configured331 total. GTK/package have zero skips; two native ignores are existing controlled fixture entrypoints. Full service/Flutter aggregate was not rerun for this test-only repair. Raw logs are ignored under `target/cli-fixture-fix/`; they are not guaranteed Git-recoverable artifacts.

Input manifest SHA256 `ba7d0348f3ec7808086029bd4cf881260e850c2e552ae20276c7bd7b42023ac1`; changed test SHA256 `becb112caaebdafb3b08f53de2c4889df4f9b2aaca78f494f446a5f741dd53d8`. Fresh [independent review](../../plans/LOCAL-SMITH-CLI-FIXTURE-BUDGET-REVIEW.md) records its own scoped verdict and final evidence closure; self-check is not human approval.

## Boundaries

The existing hosted CI run remains failed until a separately approved push/rerun actually passes. No installer validation was disabled or relaxed. No commit, push, hosted rerun, PR mutation, merge, tag/release/upload, installation, user-manager, normal keyring/provider/model or host GUI effect was performed by this fix. Earlier source/release limitations and historical failures remain intact.
