# Installer CLI executable-I/O fence

## Implemented repair and scope

The operator-reported [CI run38994173360](https://github.com/jdharrison/smith/actions/runs/38994173360) at `bd663a34e6433fc61deaf36b190b0c11af21da97` failed two CLI launches with errno26 (`ETXTBSY`) before installer logic ran:13 tests passed/two failed in0.46s. This is distinct from the prior30s fixture-cost failure. The small inert native payload repair remains unchanged.

Under [the bounded plan](../../plans/LOCAL-SMITH-CLI-EXECUTABLE-FENCE.md), only `installer/tests/cli.rs` changes executable test behavior. One shared mutex now covers copying/setting permissions on the actual Cargo-built fixture executable and all seven original process-spawn sites through exec acknowledgement. It releases before stdin, consent, child lifetime, capture, waiting, publication or cleanup. All original15 tests/assertions, namespace interleaving, independent copies, small payloads,30s watchdog and output caps remain; two regressions are added. No retries, production changes, hardlinks, sleeps, workflow changes or disabled gates.

## Diagnosis and regression evidence

A fork can inherit another thread's writable copy descriptor. CLOEXEC closes it at exec, not fork; even after the original writer closes, the sibling's copy can block exec with ETXTBSY. The [fresh controlled Linux proof](inheritance-proof.json) observed errno26 while the holder retained a CLOEXEC writer. Exec of an unrelated bounded program closed that writer; the copied actual installer then executed successfully **while the holder was still running**. The exact descriptor holder in historical hosted CI remains unobserved.

[Selected-gate results](results.json) and [source identities](source-manifest.json) record:

- Private negative-control source bypassed the common fence only; targeted copy regression failed as expected (escaped copy), and targeted spawn regression failed as expected with errno26. These are retained expected failures, not a passing negative suite.
- Both fixed regressions passed separately.
- Five predefined default-parallel runs each passed all17 CLI tests; no failed run was retried.
- Locked/offline formatting, warning-denied Clippy and all-target native tests passed:283 native/two existing controlled fixture ignores.
- Configured `make installer/test package/test` passed:283 native +nine isolated GTK +41 actual-native-ELF package tests =333, with no GTK/package skips and the no-Python runtime check intact.

The guard tests use finite channel observations; these are not scheduler-independent proofs. Negative controls and the separate inheritance proof complement them. Exceptional worker-timeout cleanup limitations are recorded by the independent reviewer; no such cleanup failure occurred in the executed checks.

## Local CI-equivalent gate and package

[ci-equivalent.json](ci-equivalent.json) binds a once-only root `make test installer/build` (verified official Dagger v0.21.9, privacy settings, existing named builder engine), followed by explicit package verification. Both exited0. Aggregate coverage is **1,264 passed**:263 service +283 installer +nine GTK +668 Flutter +41 package, five controlled fixture ignores and zero Flutter/package skips.

This is cache-using configured execution, not1,264 freshly executed tests: service/Flutter contribute931 reported passes in that run; unchanged installer/GTK/package steps reuse the exact333-pass execution bound above. Counts are not double-counted across repeated/direct/regression commands. No retries or clean-cache/hermeticity claim.

All1,230 core inputs (hash/mode/size) stayed unchanged before/after; core SHA256 `a1ad3faedd7c20afdc50be0e8edff9c322ac6703134323c6324603a69a5e2c3f`. Re-exported root `installer/dist/` verifies95 manifest-listed files/97 total; standalone/bundled service identities agree. Artifact inventory SHA256 `c933074d8390d095c6b64658de0ddc19477eaadd3202b8c00330def16d5cdf04`. Inputs, commands, log/artifact hashes and cache qualifications are in the machine record. Raw logs/proof script remain ignored under `target/cli-executable-fence/`; not all are Git-recoverable.

## Assurance and boundaries

[Fresh independent assurance](../../plans/LOCAL-SMITH-CLI-EXECUTABLE-FENCE-REVIEW.md) records static findings and final evidence closure separately from human approval. Hosted CI still requires a separately authorized push/run; local results do not change the old failed status or identify its precise holder. No commit, push, CI rerun, merge, release, installation, normal user-manager/keyring/provider/model or GUI-session action was performed. Existing release/host/provenance limitations remain.
