# Smith validation

This page summarizes recorded results, not a guarantee that every later worktree, installation, or release has passed. [Detailed validation history](docs/VALIDATION_DETAILS.md) preserves exact outcomes, failures, corrections, evidence links, and limits.

## Hosted integration

[PR #7](https://github.com/jdharrison/smith/pull/7) passed both hosted CI jobs on `14d9609`: pipeline source/archive checks and native Ubuntu installer checks/package. [Run39007625887](https://github.com/jdharrison/smith/actions/runs/39007625887) succeeded before the PR merged into `main` as `0d5f58b`.

This validates that revision, not the uncommitted Projects work or subsequent documentation edits. Merge is not release or deployment approval.

## Earlier repairs

- **Runner parked lease:** isolated candidate passed1273 aggregate tests plus34 helpers;41/41 runner integration tests and ten predefined repetitions of the original failing test passed. Independent review found no bounded-source blocker. Unknown-cleanup coverage includes a state simulation, not an actual child-exit timeout. [Evidence](docs/evidence/runner-parked-lease/README.md).
- **Keyring fixture cost:**1266 aggregate passes plus34 helpers; production cryptography and deadlines unchanged. [Evidence](docs/evidence/keyring-peer-cost/README.md).
- **Installer executable fence:**1264 aggregate coverage, partly cached; intentional negative controls reproduced unsafe behavior. [Evidence](docs/evidence/cli-executable-fence/README.md).
- **Root promotion:**1262 aggregate passes plus34 helpers; exact history/disposition and artifact bindings retained. [Evidence](docs/evidence/root-promotion/README.md).

These counts belong to separate snapshots; do not add them together. Dependencies/build caches were used. Configured service tests used Bookworm; other component/runtime gates included Ubuntu. Historical failed runs remain failed.

## Remaining gates

Source tests and synthetic fixtures do not establish native graphical/IME/accessibility behavior, real model/GPU performance, live provider/keyring acceptance, clean-host readiness, installed migration/restore, or clean-source provenance.

License/icon rights, SBOM, publication, and deployment require separate evidence and human approval. Disk4 rollback requires approved pre-migration state, compatible paired binaries, and exact keyring-generation relationships—not binary-only downgrade.

This documentation cleanup reruns no application tests or builds. Follow [Process](docs/PROCESS.md) for the required evidence and approval boundaries.
