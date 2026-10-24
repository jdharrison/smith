# Smith validation

This page summarizes recorded results, not a guarantee that every later worktree, installation, or release has passed. [Detailed validation history](docs/VALIDATION_DETAILS.md) preserves exact outcomes, failures, corrections, evidence links, and limits.

## Hosted integration

[PR #7](https://github.com/jdharrison/smith/pull/7) passed both hosted CI jobs on `14d9609`: pipeline source/archive checks and native Ubuntu installer checks/package. [Run39007625887](https://github.com/jdharrison/smith/actions/runs/39007625887) succeeded before the PR merged into `main` as `0d5f58b`.

This validates that revision, not the later feature checkpoint. Merge is not release or deployment approval.

## Local feature checkpoint

**Implemented source, broader acceptance pending:** Projects and Agents configuration, Dev local runners/jobs and the retained read/write foundation have separate source-bound checks and historical failures. Configuration finalization remains deferred. See the [Dev follow-up record](docs/plans/LOCAL-SMITH-DEV-BUFFERED-FRAMES.md), [retained-runtime contract](docs/ARCHITECTURE_DETAILS.md#retained-readwrite-job-foundation--source-only), and [checkpoint Rust checks](docs/evidence/dev-buffered-frames/checkpoint-summary.md).

Focused checkpoint checks are not full component, installed, or release acceptance. The documented broader timestamp-fixture failure remains unresolved and unwaived. Earlier navigation, migration and combined-gate failures retain their original snapshot-specific outcomes in the detailed history and linked plans.

## Earlier repairs

- **Runner parked lease:** isolated candidate passed1273 aggregate tests plus34 helpers;41/41 runner integration tests and ten predefined repetitions of the original failing test passed. Independent review found no bounded-source blocker. Unknown-cleanup coverage includes a state simulation, not an actual child-exit timeout. [Evidence](docs/evidence/runner-parked-lease/README.md).
- **Keyring fixture cost:**1266 aggregate passes plus34 helpers; production cryptography and deadlines unchanged. [Evidence](docs/evidence/keyring-peer-cost/README.md).
- **Installer executable fence:**1264 aggregate coverage, partly cached; intentional negative controls reproduced unsafe behavior. [Evidence](docs/evidence/cli-executable-fence/README.md).
- **Root promotion:**1262 aggregate passes plus34 helpers; exact history/disposition and artifact bindings retained. [Evidence](docs/evidence/root-promotion/README.md).

These counts belong to separate snapshots; do not add them together. Dependencies/build caches were used. Configured service tests used Bookworm; other component/runtime gates included Ubuntu. Historical failed runs remain failed.

## Remaining gates

Source tests and synthetic fixtures do not establish native graphical/IME/accessibility behavior, real model/GPU performance, live provider/keyring acceptance, clean-host readiness, installed migration/restore, or clean-source provenance.

License/icon rights, SBOM, publication, and deployment require separate evidence and human approval. Current disk10 needs compatible paired artifacts and approved pre-migration state for rollback—not binary-only downgrade. Follow [Process](docs/PROCESS.md) for required evidence and approval boundaries.
