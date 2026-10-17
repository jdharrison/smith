# Runner parked-lease repair evidence

## Implemented scope and binding

The [approved plan](../../plans/LOCAL-SMITH-RUNNER-PARKED-LEASE.md) repairs final parked-slot release, not runner scheduling or GPU provisioning. Base `c1339f3670c70582db05a158b46821353662b87b`, branch `feature/autonomy`. `Providers::chat` can drop its verified-model owner while the runner retains the return slot. Closing the final returned file alone can leave a fork/dup-shared open-file-description locked; private `ParkedLease` explicitly unlocks its populated slot on final destruction. Active `Runner.lock` remains a raw file. Same-live-model fallback keeps its continuous lease; the actual unconfirmed-child-cleanup early return is unchanged.

Frozen source SHA256:

| File | SHA256 |
|---|---|
| `service/src/runner.rs` | `3bd158de9566d0b84f8d296b6472bb3c3efc3d3b2a1631fdf77aaf83f7dc05c8` |
| `service/src/runner_lease_tests.rs` | `6a504792ce009209c5ff58aecd0111b934f4b47ed0d6a27a8e9539c3f4281a19` |

No timeout increase, retry, global serialization, validation disablement, reference-count heuristic or generic active-file unlock destructor was added. The exact historical CI holder/schedule/errno remains unknown; the kernel mechanism and source ownership defect are independently established, not an exact reproduction of hosted run39002802336.

## Targeted results

[Retained targeted results](targeted-results.json) bind exact commands, environments, timestamps, outcomes and raw log/source/artifact hashes. Seven new ownership tests and eight existing private runner tests passed. New coverage includes actual private `Runner::stop`/Drop with an already-cleaned/no-child state and retained duplicate descriptor, active exclusion, live-model handback/reconsumption, simulated unknown-cleanup ownership, and populated/empty poisoned slots. Independent fresh-open probes distinguish contention from other errors and explicitly unlock successful acquisitions.

The unknown-cleanup test is **an ownership-state simulation, not execution of a real child-exit timeout**. Static review of the unchanged actual early return and existing fake-runner cleanup/fallback gates provide separate evidence.

The original failing fake-runner integration test passed all ten predeclared samples: three normal, three single-CPU, four single-CPU with four competitors each. All16 competitors were terminated/reaped. A no-op parked destructor negative control produced five expected failures/two passes; a premature handback unlock control produced one expected failure/six passes. These deliberate controls are not required candidate-test failures.

Preserved harness failures: initial compilation failed because bubblewrap hid the `cc` alternatives target; corrected compiler exposure passed without warnings. Initial full runner invocation passed two/failed39 because owned sandbox ancestry was mode0775. After changing only owned harness roots to0700, one separately authorized, separately recorded invocation of the original candidate binary passed **41/41** in22.7s, without recompilation or retry-until-green. Original failed records remain unchanged. Candidate binary SHA256 `36a8e2ce100f299033c0ca6f69374fd678de411dffe572725815496b2c6ecd6c`.

## Full isolated local CI-equivalent results

[CI results](ci-results.json), [commands](commands.json) and [core manifest](core-source-manifest.jsonl) bind a retained Git archive of the base plus **exactly the two source overlays above**, not the changing live worktree. The snapshot has its own empty Git boundary, not a copy of the live Git metadata. Core input count1231; controls77; total1308 source files. Core manifest SHA256 `3f2e19c5867be87b6d03c9b26c305bad74340995018491eb6caeaf883446fdac`. Snapshot inputs were unchanged before/after.

Once-only sequence: verified official Dagger v0.21.9/module discovery; snapshot-root `make DAGGER=<verified CLI> test installer/build`; `python3 -B dagger/package.py --root installer/dist --verify`; then `SMITH_TEST_PACKAGE=<snapshot>/installer/dist python3 -B -m unittest discover -s scripts/tests -p 'test_*.py' -v`.

| Gate | Fresh passes |
|---|---:|
| Service |272|
| Installer |283|
| GTK |9|
| Flutter |668|
| Actual native package |41|
| Aggregate |1273|
| Post-build actual-package helpers |34|
| Combined |1307|

Zero cached test passes; five existing controlled fixture ignores; zero skips/failures. Formatting, locked warning-denied Clippy, builds, compiled Rust↔Dart cloud fixtures and interpreter-free Ubuntu runtime passed. Package95 payload entries/97 files with manifest/checksums, paired helpers identical. Package remains in the snapshot; live `installer/dist` was untouched. Dependencies/build vertices used caches: no cold/hermetic/provenance claim. Service tests use the configured Bookworm container; other component gates use Ubuntu24.04. No supplemental Ubuntu service run is claimed.

Preserved setup errors before any CI gate: copy tool refused overwriting an archived source destination; whole-archive1247 count was initially confused with core filtering; recorder compared tar0664 against extraction-filter0644. Corrected recorder labels/mode comparison and owned-snapshot overwrite are additive records; zero CI attempts preceded recovery, one aggregate gate followed. None is hidden as a passed test.

## Preservation, limits and approval

Concurrent Projects work changed unrelated live files during validation. The isolated candidate explicitly excludes those changes; these results **do not accept the current mixed worktree**. Live index checksum also changed during CI; attribution is unresolved. HEAD/branch/refs and both repaired sources remained unchanged. No index restoration or staging was performed by this repair; stable snapshot evidence is not proof of live-index preservation.

Existing raw-file early-error windows before slot attachment or before runner construction remain outside this repair, neither fixed nor risk-waived. Synthetic fixtures do not prove actual GPU/CPU inference, installed behavior, graphical behavior, live provider/keyring access, hosted CI, clean-source provenance or release readiness. Local rollback is a scoped reversal of this repair only, not a reset or installed rollback.

[Fresh independent assurance](REVIEW.md) passed the bounded isolated repair with no blocking findings and independently verified L1–L7 plus source/log/count/artifact bindings. Source acceptance is distinct from human approval for commit/push/hosted rerun/install/release. None of those effects is included or performed here.

## Raw evidence integrity

Raw logs/snapshots stay ignored under `target/runner-parked-lease/targeted-c1339f3/` and `target/runner-parked-lease/ci-c1339f3/`; they are not guaranteed present in another checkout. Retained checksum indexes bind57 original targeted files, three supplemental entries and40 CI evidence records. The retention check verified all indexed records plus seven command-log hashes and both current/snapshot source overlays. [Targeted index](raw-targeted-evidence-checksums.txt), [supplemental index](raw-corrected-target-evidence-checksums.txt), [CI index](raw-ci-evidence-checksums.json). Exact retained bindings are evidence anchors, not package authenticity or publication approval.
