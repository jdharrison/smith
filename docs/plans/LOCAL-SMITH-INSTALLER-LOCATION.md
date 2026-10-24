# LOCAL-SMITH-INSTALLER-LOCATION

## Approval and scope

**LOCAL_PLAN_APPROVED:** the operator's “yes” approves the preceding prevention proposal: local Make/builder tooling, tests and documentation; early ancestor validation, explicit safe creation of generated output leaves, post-export/pre-launch revalidation and isolated regressions. Repository `smith`, existing `feature/autonomy`, base `14d960975e76f906efb5e447ff3dece0e8e4385b` plus pre-existing dirty/untracked work. Baseline hashes/copies are retained in `target/installer-location/`; HEAD alone is not input identity. Owner: coordinating session. Expires on completion, revocation or material scope drift.

No existing source/host-directory chmod, native guard relaxation, installation, live upgrade/retry, service/manager/provider/model/keyring use, state migration, commit/branch/remote action, release or accepted risk. Only generated output leaves and isolated fixtures may be created. Preserve all unrelated work.

## Operator-approved follow-up permission correction

The operator's “yeah tighten all usages up please” explicitly authorizes this one local correction after the prevention fix: inspect the existing component/export directory chains (`client`, `service`, `installer` and their `dist`/`build` outputs) and remove unintended group/world write from unsafe current-user-owned directories only. Bound to the same repository/branch/base plus current dirty inputs. Inspection found only `client` and `service` require changes, each0775→0755; root/installer are0755 and existing dist leaves0700. Missing installer/build will remain absent. Apply only to held no-follow directory descriptors after owner/mode validation; no recursion, file-mode changes, HOME/installed roots, source-content changes, creation of missing exports or application/service effects. This narrow human approval supersedes the earlier no-existing-chmod exclusion only for these two named directories and expires when this correction completes; the build tooling still never repairs existing permissions automatically. Record before/after modes and rerun read-only location/package checks.

**Follow-up completed — 2026-10-18:** only client/ and service/ changed0775→0755 through held no-follow descriptors after both owner/mode checks passed. Other inspected root/installer/output identities and modes remained unchanged; absent installer/build was not created. Before/after evidence is `target/installer-location/permission-tightening.json`. Read-only client/dist, service/dist, installer/dist and installer parent checks all passed; the existing97-file package verification and whitespace check also passed (`permission-tightening-checks.log`). This one-time permission authority is consumed. No recursive/file-mode/installed/service/application effects occurred.

## Evidence and reconciliation

**Observed:** the reported `installer/0775` ancestor blocked a private `dist/0700`; the operator's manual permission tightening removed that error. Current shell umask is0002, which permits new0775 directories; what restored this particular existing directory's mode is unproven. Ordinary installer dry-run then reached its expected differing-installed-service refusal, not an upgrade result.

Independent producer `a0e19435-2166-48af-8142-097ba5b38706`, architect `5839d105-bfc6-4a86-a8e7-9d7e69d65c21` and designer `f5102dd9-51f4-4f9c-ac94-68b31657a9c1` agree on early builder-only detection, native-compatible ownership/mode/no-follow rules and no automatic repair of existing directories. Architecture concurrence includes held-FD traversal, single-leaf creation, EEXIST revalidation, checks before metadata writes/reads and Python `-B`.

**Ordering clarification resolved with producer/architect:** each affected export is gated before its own Dagger invocation; failed prerequisites block dependent publishers/GUI. Under parallel Make an unrelated safe build may continue. No global cancellation, transaction, rollback or prevention of external permission drift is promised.

## Traceability / implementation

| Requirement / blocking acceptance | Task / scope | Required evidence |
|---|---|---|
| R1/A1: reject unsafe ancestors even with safe output leaf | T1 parent: `dagger/package.py` |0775/0777 ancestors, ownership, symlinks, missing intermediate, root-owned sticky ancestor-only parity; unchanged source permissions |
| R2/A2: fail before affected export and before GUI; check after export | T2 parent: `Makefile`; test owner: `scripts/tests/test_build_locations.py` | Fake Dagger/GUI call order and sentinels, per-target parallel prerequisite failure, postexport/prelaunch drift |
| R3/A3: safe absent output under umask0002 | T1/T2; test owner: package regressions | Explicit mkdir0700 of one absent generated leaf; preserve existing safe0700/0755; EEXIST validate/fail, no recursive mkdir/chmod or caller-umask mutation |
| R4/A4: usable truthful failure and preserved native boundary | T3 parent: affected canonical/build docs | Offending component/mode/UID, owner-reviewed narrow remedy; phase-specific effects; runtime guard unchanged and no recipient Python dependency |
| R5/A5: source-bound self-check and independent assurance | T4 parent/reviewer | Focused package/Make tests, existing pipeline/archive tests, actual package read-only verification, source hashes/diff and fresh review; exact skips/failures retained |

Production behavior: `check_location(root, create=False)` walks existing directories using held no-follow directory descriptors, validates native-compatible restricted path spelling/ownership/modes, and requires a current-owned leaf. `create=True` may create only one missing final generated directory with0700, then verifies it; never changes existing permissions. Package assembly/verification call the strict gate first. CLI `--prepare-location` is explicit leaf creation; `--check-location` is read-only. Make checks installer package/component and service/client export directories before and after export, and package location again immediately before GUI launch. Export is scoped under restrictive umask, but post-export verification remains required because exporter behavior is not inferred from umask.

## Gates and remaining boundaries

Use finite synthetic fixtures, fake exporters/GUI/publisher sentinels and private roots only. Run focused regressions then existing package, pipeline/archive tests and whitespace checks. Verify the existing full package read-only if compatible; do not substitute real upgrade/manager use for a fixture. Preserve intermediate failures and distinguish real ELF checks from synthetic content tests. Bind final tracked/untracked inputs and obtain fresh independent assurance. Component/root/package reconstruction, real exporter/installed acceptance and release remain separate gates; no passing fixture grants rollout authority.

## Results

**Implemented / self-check — 2026-10-18:** all five Make export targets prepare/check their own location and revalidate after export; installer/run rechecks before GTK. Package assembly/verification use the strict gate first. New tests cover the exact recurrence, modes/ownership/no-follow/creation races, umask0002, ordering and drift. Native filesystem/maintenance sources match their baseline hashes.

Parent checks passed:54 package tests using the existing real CLI/GUI ELFs against private synthetic payload/roots;43 script/pipeline/Make tests with `SMITH_TEST_PACKAGE=installer/dist`, including the real existing97-file package's archive round-trip under synthetic source identity. Final runs had zero skips; the first script run's one unsupplied-package skip was retained and rerun with the artifact supplied. Existing full package verification passed read-only. Logs and the scoped source manifest are in `target/installer-location/`. Actual Dagger export/full reconstruction was not run; fake exporters test ordering/mode drift, not engine behavior. No installed acceptance is inferred.

Before the separately approved follow-up correction, read-only location scan found installer/dist safe and existing client/ and service/0775, correctly rejected by component export checks. They were not chmodded automatically by the prevention tooling. The recurring original directory's creator remains unproven. Fresh assurance `8f99cd47-af7c-4339-be63-3d193340c7a2` found one medium raw-spelling normalization gap. CLI/string spelling is now validated before Path construction, with dot/repeated/trailing-separator regressions; the full54+43 checks passed again without skips (`final-package-tests.log`, `final-pipeline-tests.log`). The same fresh reviewer independently reran13 location regressions, verified the corrected source/patch bindings and closed the finding: scoped PASS, no outstanding remediation. This does not approve release/live effects. No new installation, GUI launch, retry, service or migration outcome is claimed.
