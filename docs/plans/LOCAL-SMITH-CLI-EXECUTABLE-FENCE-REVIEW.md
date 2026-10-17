# Independent assurance: CLI executable-I/O fence

## Scope and initial disposition

- Fresh-context static review of the local test-only change in `installer/tests/cli.rs` against base `bd663a34e6433fc61deaf36b190b0c11af21da97` and requirements E1–E5 in [the bounded plan](LOCAL-SMITH-CLI-EXECUTABLE-FENCE.md).
- Reviewed writer/spawn exclusion, missing process-creation paths, lock ordering/reentrancy, retained test behavior, namespace isolation and regression cleanup.
- Authority for this review is **read-only inspection plus writing this document only**. No code edits, tests/builds, network access, staging, commits, CI reruns or host lifecycle operations were performed.
- **Initial code verdict (historical):** no blocking static defect found; validation was pending and acceptance was no-go for missing evidence. **Final scoped verdict after evidence reconciliation: PASS for the local test-only repair.** The missing local verification blocker is closed below; hosted CI, release and installed-host acceptance are not granted.

## Candidate and evidence identity

Implemented source inspected at HEAD/base `bd663a34e6433fc61deaf36b190b0c11af21da97`, with an uncommitted CLI test diff and an untracked plan. Initial `git status --short` listed only those two paths. Final reconciliation confirms the same HEAD and exact CLI source hash. The plan subsequently gained an outcome section; documentation/ledger/evidence updates outside the core inputs are not executable source drift. This verdict does not cover later executable source changes.

| Input | SHA-256 at inspection |
|---|---|
| `installer/tests/cli.rs` | `0ba1d779ce64b70a35a5f7fc753b9c0d997df5fe21a33eff6e0613f692f51361` |
| `docs/plans/LOCAL-SMITH-CLI-EXECUTABLE-FENCE.md` | `9226cc325c763b0e307f82f265bd6f7304a63dfbc3f7d56718a9bd5b57b3d5b8` |

Policy read: `AGENTS.md`, `docs/SCOPE.md`, relevant architecture/trust-boundary, process/evidence/approval/gate, design/evidence, and host/builder sections, plus `roles/assurance.md`. Configured gate references inspected: `Makefile` and `.github/workflows/ci.yml`.

The operator's run `38994173360` and architecture context `da4ab02d-5053-4858-9485-a656dbbc5e43` are **reported context**, recorded in the request/plan, not independently retrieved in this review. The exact hosted writable-descriptor holder remains unknown. The inheritance mechanism is technically consistent with Linux fork/exec behavior; neither this static review nor an isolated reproduction establishes the historical holder.

## Checks performed and static findings

### E1: writer/spawn exclusion — implemented, statically consistent

`installer/tests/cli.rs:18–33` defines one `Mutex<()>`, a lexical helper, and shared copy/spawn wrappers. The copy guard encloses both `fs::copy` and executable permission setting; `fs::copy`'s owned descriptors close before that guarded action returns. The spawn guard encloses the synchronous `Command::spawn()` operation, including its normal Unix pre-exec error/exec acknowledgement, rather than merely command construction.

All seven original launch sites now use `spawn_fixture`: fixture `run` (304), known-child capture (566), help/version (704), symlink/argv0 (810), relative explicit bundle (846), service-only (862), and artifact read-only/decline (895). The new writer regression also uses it (403). The only raw `command.spawn()` is inside the wrapper (32). Full-file inspection found no direct fork/exec, `Command::output`/`status`, or `pre_exec` bypass. Thread creation, including the scoped interleaving worker, is not process creation.

The real installer copy is made only through `copy_executable` (239), with the regression's extra independent copy also using that helper (362). The regression's intentional writable open is held under the same mutex (393–409). Unfenced payload writes create small inert files that are not the CLI inode executed by these tests. In-process artifact publications similarly do not execute their payloads; `installer/src/artifact.rs` has no production process launch in this path. They do not require broadening this CLI-executable fence.

This is process-local cooperative protection, not a cross-process file lock or a guarantee against arbitrary external writers. Independent Cargo test processes do not inherit each other's private writable descriptors. No production concurrency or capability change is introduced by this diff.

### E2: lock scope, deadlock and constructor reentrancy — no blocking static defect

The helper is non-reentrant, but neither guarded production-fixture action calls the other wrapper, constructs a `Fixture`, initializes the namespace, invokes consent, or waits for a child. `Fixture::new` enters namespace initialization before taking the copy lock. Both new regressions construct their fixtures before manually taking the fence. Thus no `OnceLock`/mutex cycle or recursive lock acquisition was found.

`spawn_fixture` returns before stdin writes, `wait`, output capture or child lifetime. Rust evaluates `spawn_fixture(...).map(wait)` by returning from the helper first, so the regression also waits outside the lock. Spawn errors return normally and release the guard; panics release it but poison the mutex, causing subsequent participating tests to fail rather than silently bypass protection. No post-fork callback was added that could attempt to reacquire an inherited locked mutex.

A stalled copy or pre-exec spawn can delay all participants: there is no new lock-acquisition/pre-exec deadline. The retained 30-second watchdog starts in `wait`, not before spawn. This is a limit of the narrow fence, not evidence of a newly introduced lock cycle; no claim of whole-suite bounded termination is made.

### E3: retained behavior and isolation — implemented; runtime evidence verified below

Base/current static enumeration found **15 original tests retained, 17 current tests, exactly two additions, no removed tests**. The diff preserves original assertions, the small native `/usr/bin/true` payloads, executable modes and helper equality, the 30-second watchdog, and output caps/drain semantics. It does not add retries, runtime sleeps, hardlinks, deadlines, endpoint overrides or whole-test serialization.

The two new fixture names are predeclared in `FIXTURE_NAMES` (137–138); 13 leaves remain within the `AtomicU64` claim mask. Precreation preserves shared namespace link counts. Fixture drop still removes contents but retains each leaf until process-exit namespace cleanup. The consent/construction/cleanup interleaving regression (662–693) is unchanged and remains capable of detecting a fixture-lifetime lock. No constructor is called while a manual fence is held.

### E4: guard regressions and cleanup — implemented with limitations

The copy test holds the actual shared mutex, starts a worker using the production copy helper, checks noncompletion while held, releases, joins on the normal/bypass paths and compares bytes. The spawn test holds an actual writable descriptor to the actual copied CLI under the same mutex, invokes the production spawn wrapper, closes the writer **before** releasing the fence, then checks successful version output. An immediate bypass is expected to return `ETXTBSY`, which is reported after releasing the parent resources and joining. Neither test intentionally leaves its guard/writer held during an assertion or join on the expected success/bypass branches.

The established pattern is independently visible in `service/tests/debug_service.rs:15–39,950–987`: same lexical mutex/copy/spawn design and writer-held spawn regression. That is design precedent, not proof that this candidate passes.

## Findings by severity

### Critical / high / medium

No blocking code defect established by static inspection. Missing local validation was initially an acceptance blocker; the final reconciliation below closes that evidence gap. No new critical/high/medium finding arose from the supplied evidence.

### Low — L1: finite negative observations are scheduler-dependent

Locations: `installer/tests/cli.rs:360–366,400–407`.

The attempt notification occurs immediately before calling the helper, not after attempting mutex acquisition. A worker can be descheduled after notification; the 100ms no-result observation alone therefore does not prove it blocked at the fence. The copy result also includes copy duration; the spawn result includes `wait`. These are useful regression guards, but not scheduler-independent proofs. **Required follow-up:** retain both expected-red private-clone bypass results and the separate controlled inheritance proof before calling E4 validated. If a bypass unexpectedly passes, investigate the guard rather than rerunning until red. No source change is requested on static evidence alone.

### Low — L2: exceptional regression paths do not always join workers

Locations: `installer/tests/cli.rs:369–379,411–422`.

The parent releases its fence/writer before interpreting result-channel failures, preventing the obvious held-lock cleanup deadlock. However, a post-release five-second timeout/disconnect can panic before `thread.join()`. Dropping that join handle detaches the worker; `Fixture::drop` can then remove files while a delayed worker still copies/spawns/waits. The spawn worker's existing child watchdog is 30 seconds, longer than the result timeout. Normal and immediate expected-red bypass paths join correctly; unconditional cleanup under severe scheduling/I/O delay is not guaranteed. **Recommendation:** record any such failure as a cleanup/worker-lifetime limitation, not an ETXTBSY diagnosis. If observed, require worker-lifetime-safe cleanup before acceptance. This is not a reason to widen the executable lock or change the retained child watchdog.

## Final evidence reconciliation

The initial validation-pending assessment is superseded **only for the bounded local repair** by inspection of [README](../evidence/cli-executable-fence/README.md), [results](../evidence/cli-executable-fence/results.json), [source manifest](../evidence/cli-executable-fence/source-manifest.json), [CI-equivalent record](../evidence/cli-executable-fence/ci-equivalent.json) and [inheritance proof](../evidence/cli-executable-fence/inheritance-proof.json). Execution records are dated 2026-10-17 UTC. This reviewer inspected existing evidence with read-only Python/Git commands; no tests, builds, proof or package verifier were rerun.

### Independent identity and log checks

- Rehashed every referenced native/configured/aggregate/package raw log against its machine-record digest; all matched. Verified retained negative/fixed test-binary hashes and checked recorded test-summary text against the logs.
- Rehashed all 1,068 selected root input records (hash/mode/size), then all **1,230 core inputs** and five supplemental control inputs. Core selection exactly matches the stated Git-tracked component/root-file selection. Recomputed compact sorted-key JSON digests using the recorded contract: core **`a1ad3faedd7c20afdc50be0e8edff9c322ac6703134323c6324603a69a5e2c3f`**, supplemental `0fd0ea3808c588f92e86dc044ae42b7e402ae309496cf2ff71efdc091fc10132`; both match. Before/after records agree and current core files still match. The CLI remains `0ba1d779ce64b70a35a5f7fc753b9c0d997df5fe21a33eff6e0613f692f51361`.
- Verified all **1,052 installer/service component files** in the fixed private snapshot against the selected source. The negative component snapshot differs only at `installer/tests/cli.rs`: exactly the sole lock-acquisition statement was removed from common `executable_io`. Negative CLI SHA-256 is `6c19603610e8377ceb9b52e9106165e7778b843ec17f5fff8d1b451273380396`. Supplemental workflow/tooling files are root gate inputs, not required contents of those component snapshots.
- Recomputed the artifact inventory digest **`c933074d8390d095c6b64658de0ddc19477eaadd3202b8c00330def16d5cdf04`** and checked all 118 listed filesystem entries against the exact current export inventory, including modes and all 97 regular-file hashes/sizes. The manifest lists 95 payload files; manifest/checksums account for the other two regular files. Standalone and bundled service bytes agree. Inventory paths are already project-relative, not relative to `export_root`.
- Confirmed Dagger CLI bytes match the recorded executable digest. This check does not independently attest archive origin or clean-build provenance; the retained record identifies verified official v0.21.9 and explicitly disclaims hermetic/clean-cache execution.

Public evidence hashes at closure:

| Record | SHA-256 |
|---|---|
| `results.json` | `16b2c6de9bf0e8368b846422f148562f924b3cfd64e6fe22c04dbe7020bce1d0` |
| `source-manifest.json` | `c9d5142069a91ecc4ab423ed1135405a1f678ff3cadf57461f13075eb7c1d4cd` |
| `ci-equivalent.json` | `0a10984affdbf22a30737f8a98ce641ca40aa8674ceb1251965794fbba296d1c` |
| `inheritance-proof.json` | `4bb18f12f697f1e1e1b9ee9abe50af92bc712ec5c682d2a071e29d8e7004f4d6` |
| `README.md` | `2f12755ad1750bb4e7b3235cb87413f5e50d2b18cffd916f79854291468d9b6e` |

The CI-equivalent record's prior-evidence bindings match the preserved results/source/proof files. The source-manifest digest alone is not substituted for the recomputed core-input digest.

### Controlled inheritance proof

Read `target/cli-executable-fence/inheritance-proof.py` without executing it. Its SHA-256 matches the public proof's `166fc140b9ce17e6e239594b794c9dd2e207564aaa01b0371498190a2108d8fd`. Public and ignored raw proof JSON are byte-identical. The actual source CLI binary hash matches recorded copied-CLI SHA-256 `7312801228db5f8a33c3ceab1ba28aca53870bf1577ce8e26b7d486e1ac6fb29` (binary identity, not the Rust source-file hash).

The script copies the actual CLI into a private temporary root, opens it writable with `O_CLOEXEC`, checks `FD_CLOEXEC`, forks a controlled holder and waits for readiness. After the parent closes its writer, execution of that CLI records errno26. A gate releases the holder to exec unrelated `/usr/bin/sleep`; ready-pipe EOF signals closure at exec. A nonblocking wait then confirms the holder has not exited, and CLI `--version` succeeds with exit0. The script bounds waits and cleans its descriptors/holder/private root. This is fresh controlled evidence of the inheritance mechanism, not a reproduction of the hosted scheduling/descriptor owner or a proof about every kernel interleaving.

## Required Checks Evidence

Exact commands, working directories, timestamps, time bounds and log hashes remain in the linked machine records. Expected failures below are successful negative-control evidence, **not failures of the unchanged candidate**.

| Check | Command/scenario | Expected outcome | Actual outcome / exit status | Evidence source |
|---|---|---|---|---|
| Base, source, scope, whitespace | Read-only Git diff/HEAD/status, input hashing, `git diff --check bd663a3 -- installer/tests/cli.rs` | Same reviewed source; no weakening | Same HEAD/CLI hash; all core identities match; diff check exit0 | Reviewer inspection and manifests |
| Controlled inheritance | Parent-owned private Linux proof script above | ETXTBSY after parent writer closes; success after holder exec, before holder exit | errno26, CLOEXEC true, holder still running, actual copied CLI exit0; PASS | `inheritance-proof.json` and hash-verified script |
| Copy bypass | Common-helper lock removed in private negative snapshot; targeted copy guard | Nonzero escape assertion, no hang | Exit101, 0 passed/1 failed/16 filtered; explicit copy-escaped assertion | `results.json`, `negative-regression-1.log` |
| Spawn bypass | **Same common-helper mutation**, separate targeted spawn guard | Nonzero writer-held escape, errno26, no hang | Exit101, 0 passed/1 failed/16 filtered; `ExecutableFileBusy`, code26 | `results.json`, `negative-regression-2.log` |
| Fixed guards | Unchanged fixed snapshot, each targeted guard | One passing test each | Both exit0, one pass each; no ignore | `fixed-regression-1.log`, `fixed-regression-2.log` |
| Fixed-count parallel CLI | Five predefined invocations of fixed CLI test binary, no serial/thread-count flag or test filter | All17 pass each, no retries/ignores | Five exit0 results, each17 passed/0 failed/0 ignored; both guards and consent/construction/cleanup interleaving explicitly pass | `fixed-cli-parallel-1.log` through `-5.log`; execution policy in `results.json` |
| Direct native quality | `cargo fmt --package smith-installer --check`; `cargo clippy --locked --offline --all-targets -- -D warnings`; `cargo test --locked --offline --all-targets` | Clean formatting/Clippy and native suite | All exit0; native283 passed/0 failed/two controlled ignores | `fmt.log`, `clippy.log`, `native-suite.log` |
| Configured installer, isolated GTK and package | `make installer/test package/test` using recorded Dagger executable | Native quality, embedded isolated UI gate, actual-native package and no-Python checks pass | Exit0;283 native +9 GTK +41 package =333; two native fixture ignores, zero GTK/package skips | `configured-gates.log`; `Makefile`, `dagger/src/smith_next/main.py:153–161` |
| Independent-workspace/aggregate/build | Once-only `make DAGGER=target/installer-pipeline/smith-dagger-0.21.9/dagger test installer/build` | Configured root aggregate and paired installer export succeed | Exit0,159.146s, no timeout; cache-aware1,264 coverage with five controlled ignores | `ci-equivalent.json`, `ci-equivalent-make.log` |
| Explicit exported package verification | `python3 dagger/package.py --root installer/dist --verify` | Native strict package passes | Exit0;95 manifest-listed files; exact97-file export inventory verified independently above | `ci-equivalent-package-verify.log` |

### Counts, cache and invariant coverage

Independently summed native log summaries: **283 passed, zero failed, two ignored**. Configured Rust totals are **292 passed** including nine GTK tests; package log separately says `Ran 41 tests` and `OK`, giving333. No GTK ignore/package skip is hidden in that sum. The two installer ignores are the existing `launch_parent_fixture` and `progress_output_probe_child` subprocess fixtures, not either new regression.

Aggregate service vertex22 reports **263 passed/zero failed/three controlled ignores**; Flutter vertex84 reports **668 passed**. Aggregate vertices62 (installer quality/tests),109 (isolated UI),98 (actual-native package) are explicitly cached successful steps. Their exact333 passes are bound to the hash-verified earlier configured execution on unchanged selected inputs. Thus **263 +283 +9 +668 +41 =1,264**, consisting of **931 reported passes in the later run plus333 reused passes**, not1,264 freshly executed tests or a sum inflated by the five CLI repetitions. Three existing service fixture ignores plus two installer fixture ignores make five. Existing evidence finalization's ANSI-parser correction is disclosed, not a test retry.

Read-only inspection of the configured scripts confirms service fmt/all-target locked Clippy/tests, installer fmt/all-target locked Clippy/tests, and isolated Xvfb GUI-feature Clippy/tests remain enabled. `installer/test` includes isolated `test_installer_ui`; a separate standalone `make installer/ui-test` invocation was not necessary and is not claimed. The no-Python runtime path checks interpreter absence, runs actual release CLI help/version/artifact dry-run as UID1000 with `PATH=/unavailable`, and checks no publication; retained configured log vertices67–70 finish successfully. Package tests receive actual CLI/GTK binaries. None of these gates was replaced with header fixtures or skipped.

E1–E3 remain supported by unchanged static source and the passing real-copy/launch and interleaving paths. E4 now has both fixed passes, both expected-red targets and the independent controlled mechanism proof. One common-helper mutation exercising two separate targeted tests is the actual negative-control topology: the initial proposal for separately copy-only/spawn-only bypass clones was not executed and is not claimed. Removing the common lock defeats each direction as intended without other component-source changes. That is sufficient for this bounded E4 review; it does not prove every conceivable bypass is detectable. E5 is satisfied by bound quality, repeated default-parallel and configured evidence plus this independent reconciliation. No observed deadlock, cleanup failure, source weakening or candidate test failure appears in the required executions.

## Evidence Gaps and remaining limits

- **No required local verification remains missing for this repair.** This reviewer verified retained artifacts/logs rather than rerunning the producer's commands. Hashes bind inspected bytes, not independently attested execution or publication authenticity.
- **L1 remains a test-method limitation**, not a mathematically closed scheduler proof. Its requested evidence follow-up is complete: both targeted bypasses fail and the controlled inheritance proof passes. No mandatory old-test rewrite or whole-kernel proof is added.
- **L2 remains an unmodified exceptional-cleanup limitation**, not a fixed bug. No such timeout/detached-worker cleanup failure was observed in the supplied checks. Normal/expected-red joins and release ordering are verified; arbitrary stalled-worker or panic cleanup is not guaranteed. This is recorded as nonblocking for the narrow local repair, not a human accepted-risk waiver for release or a claim of unconditional bounded termination.
- The exact hosted holder in run `38994173360` is still unknown; the run was not fetched, rerun or superseded by this review. Local results neither alter its failed status nor grant CI/network/external-effect permission.
- Raw logs, private snapshots, proof script and local exported binaries are ignored outputs and are not all Git-recoverable. Public machine records retain their identities; do not claim clean-cache/hermetic builds, independent provenance, installed-host/normal-session/GPU/model/provider acceptance or release readiness from these checks.

## Final decision

**Verdict: PASS for the implemented, bounded local CLI executable-I/O fence repair on the exact source identified above.** Initial missing-evidence acceptance-pending status is closed. E1–E5 have proportionate static, adversarial and configured local evidence; no blocking code finding or required local check remains unresolved. L1/L2 remain truthfully recorded limitations, not falsely closed as fixed issues.

**Local source assurance recommendation: go** for human consideration of this test-only repair. This is independent assurance, not approval of its own work or any external action.

**Release Gate Decision: no-go** for hosted CI, publication, merge, release, deployment or installed-host acceptance under this local-only review. Those distinct gates require their own current evidence and explicit human scope. No commit, stage, push, network access, CI rerun, build/test rerun or host operation was performed by this reviewer; only this review document was updated.

**Required remediation for this local repair: none.** Any executable source drift or newly observed timeout/cleanup failure requires reassessment; future external actions remain outside this verdict's authority.
