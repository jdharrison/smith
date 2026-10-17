# Independent assurance — keyring private-peer cost repair

- **Scope:** fresh independent/adversarial review of the uncommitted test-only repair against K1–K5 in `docs/plans/LOCAL-SMITH-KEYRING-PEER-COST.md`, not implementation or historical-timeout reproduction.
- **Candidate:** repository `smith`, branch `feature/autonomy`, HEAD/base `becc14ed00da326e9ef8585d1fa309e893be48b2`; exact changed test-file SHA256 `bd51d10ff5bc2a6657df7b06aa5d85d8dcaf445a76ec7ac2b36fa095ae565888`.
- **Method:** inspect policy, plan, base/diff, actual source/logs/commands/snapshots/package; independently rehash and reconcile evidence; execute the existing uninstrumented keyring-only binary once. No broad gate rebuild/rerun or real keyring, user bus, provider, manager, host GUI or GPU access.
- **Authority:** this review writes only this tracked report and private validation files under the new ignored `target/keyring-fixture-fix/assurance-20261016-becc14e/`. It grants no commit, push, hosted rerun, merge, release, install, deployment or accepted-risk authority.

Review observations were recorded at UTC **2026-10-17 04:16–04:20** using the tool clock. The `20261016` directory suffix is a name, not a timestamp assertion. The source was not authored or repaired in this review context. Builder private reasoning was not used.

## Decision summary

**Implemented/as-built:** the repair meets the bounded local K1–K5 requirements at the identities below. No source-repair blocker, required local test failure, weakened assertion, disabled installer validation, new skip/retry or production change was found. Recommend retaining the test-only repair, subject to the requested documentation-status closure and short final rebind in this review session.

**Target boundary:** this is not complete Ubuntu recipient/production acceptance, hosted CI success, clean-source provenance or human approval. The configured service gate is Bookworm; other relevant component gates are Ubuntu24.04. Existing finite synchronous-search/timing limits remain, not waived guarantees.

Read authority: `AGENTS.md`; `docs/SCOPE.md`; relevant trust/keyring/build/resource sections of `docs/ARCHITECTURE.md`, evidence/gates/approval sections of `docs/PROCESS.md`, evidence/privacy/signoff sections of `docs/DESIGN.md`, builder/keyring/resource sections of `docs/HOST_PROFILE.md`; and `roles/assurance.md`. The source-only repair changes no runtime/policy contract requiring a canonical runtime-policy amendment.

## Exact source and evidence binding

These are **initial-review** hashes. Plan/`VALIDATION.md`/results review-status prose was still pending when read; subsequent status-only changes need the promised documentation/source rebind, not silent substitution of these identities.

| Object | SHA256 |
|---|---|
| Base `service/src/cloud_keyring_tests.rs` bytes | `b3c5816aa52f4b73e1ce2c55bd50bd8c7196f6ed9a136a6a96708f3c7981ec83` |
| Candidate `service/src/cloud_keyring_tests.rs` bytes | `bd51d10ff5bc2a6657df7b06aa5d85d8dcaf445a76ec7ac2b36fa095ae565888` |
| Exact test-file Git binary diff against base | `d839eba59bdea3254857fa6d6bd4b5f8f72ef2a17dc7badf22e93a87d8bf4794` |
| Unchanged production `service/src/cloud_keyring.rs` | `f6ac5c6ea536992ad7bf252d92ef06b4ab8085b6393dc2e1592c53f5628c20aa` |
| Unchanged production budgets/`bounded()` in `service/src/cloud.rs` | `7e7faba6ae9a841c3c157368c557dc8004d1f5dc2b0de7d316519b3a4f5d0aa0` |
| `docs/plans/LOCAL-SMITH-KEYRING-PEER-COST.md` | `36f075c1a332ec723dc1bdfb9fad59dfc4fa5fc953266f61470c7e22b4157cb1` |
| `VALIDATION.md` | `c64fc406a96657633bdcec1452261393a9bed73c673cce750b9fb7f8794737e0` |
| `docs/evidence/keyring-peer-cost/README.md` | `da199a9ac0e0e900cd13f2c766885344c28ed2939a48f9cc0da17c125261a474` |
| `docs/evidence/keyring-peer-cost/results.json` | `c7941ced72e4c4b4eb6b7f9a7b7eebcbb0325c954e3a439f2b49224151f06b3e` |
| `docs/evidence/keyring-peer-cost/core-source-manifest.jsonl` file bytes | `d395418651684c77be3d748a63b51834a7deea0a08ae71b96d0dcb2b3cf60a27` |
| Same core records, compact sorted-key JSON array | `2e054a19a5879d3d70ed72927015277916f2c6cad8bfd124e59357a94757a301` |
| `docs/evidence/keyring-peer-cost/workflow-helper-controls.jsonl` | `005539639f8de338acf0d9c8e79ea897829d3fb9df54b6bafbeefe55a4d7c73d` |

All **1,230 core paths/bytes/sizes/actual modes** independently match the retained before/after source manifests and current tracked input selection. There are no nonignored untracked component inputs. The only tracked code difference against base is the named test file; existing `VALIDATION.md` changes are documentation. No staged change was present. Production, manifests, dependencies, profiles, workflows, installer tests and scripts remain unchanged.

For raw paths below:

- `C` = `target/keyring-fixture-fix/ci-equivalent-20261016-becc14e/`
- `P` = `target/keyring-fixture-fix/private-session-20261016-becc14e/`
- `M` = `target/keyring-fixture-fix/measurement-20261016-becc14e/`
- `A` = `target/keyring-fixture-fix/assurance-20261016-becc14e/`

| Retained anchor | SHA256 |
|---|---|
| `C/results.json` (original, not rewritten) | `a132de81ea1d740d0811b5143473596114600c2e19a4356a56d84185c4319b0f` |
| `C/logs/make.log` | `61e33eb6cd3fea185b03da12db5374cd5a6049865285ac1861a8d7529c8890fd` |
| `C/logs/make.plain.log` | `98d999fb38b9164c7ab92d3c3c8b422226286f070c0669d858eaa61c98624849` |
| `C/logs/package.log` | `4f6e997052bf8ed0fe4e97c0e068bea00d34f1891dc7ee807e21581cdbb6da2e` |
| `C/logs/helper.log` | `5fe8f3f91689996c5b1f92cd6bae449fede499cf45869cdab25194e4903aa8f7` |
| `C/continuation-verification.json` | `e4461c03bd8ed363b06abf4769974c1eeba926944a40040d05a2a9fd8f28df0b` |
| `P/SHA256SUMS` (183 indexed files) | `01cc5d7ae87a973a15b1ca7eec2159f49950034159f009868679dbdb02bec776` |
| `M/SHA256SUMS` (22 indexed files) | `e3f522351febabb6d907a4000003e10df6c7241e7b99a17b05107222f3d2085d` |
| Original uninstrumented `P/binaries/original/service-tests` | `438b7c6e83151885dd616d81e448ecddc0b35a5f7402203d32bb0da7c464ad00` |
| Separate diagnostic `P/binaries/instrumented/service-tests` | `17dce2c7b29aef150c52f1aacc426894842b81f6db0e7f51edb31e73f24c3ed2` |
| Current package canonical inventory (117 entries) | `5c3ed9f62440134ab0e1e7f01b1e79114bacc08e12fba444f03ff14219def007` |
| Package `manifest.json` | `52804bfae4a71ff783cff3a0c299e7e9b491b2d79c9d05ea22c0eafe3e7d5102` |
| Both packaged service helpers | `5cfe584b8e597771319b3d4a3da9572bd30df9059d89f4f68e6e38fd5b2c4dcb` |

The published results' **21 raw-evidence references**, original CI results' **17 evidence references**, **35 original command records and their 35 log hashes**, and all **205 indexed scratch files** were independently verified. These are local integrity/binding checks, not adversarial authenticity or an independently retained external provenance anchor.

## Mathematical and adversarial source review

### K1: equivalence, first match, order, bound and exhaustion

In `service/src/cloud_keyring_tests.rs:205–279`, write `a = public`, `p = prime`. On entry to candidate `n=2`, `shared = a² mod p`. If the loop invariant is `shared = a^n mod p`, multiplication/reduction yields `(shared × a) mod p = a^(n+1) mod p`. Induction gives precisely the original `modpow` residue at **every evaluated candidate**, not merely the successful samples.

Both paths convert that residue using the same `BigUint::to_bytes_be()` and apply the unchanged `!padded || bytes.len() < 128` predicate. Both evaluate ascending **2 through 99,999**, hence at most **99,998 predicates**, returning the first identical `(private, canonical shared bytes)` tuple. `padded=false` still returns private2. If no candidate qualifies, neither returns a key or fallback: the old `unwrap()` panics and the new helper explicitly panics. The new loop computes one unused residue for100,000 after a rejected99,999, but does not evaluate/select it; this does not broaden the candidate bound. Exhaustion is established by source/proof, not by a claimed exhaustive runtime sample.

The two new standalone `#[test]` functions compare against the original `modpow` oracle for `2`, `p−2`, inverse-two, and both padding modes. The inverse-two test independently checks rejection of2–7 and selection of8 with127-byte shared bytes, covering modular reduction/progression and first-match behavior. Both functions remain outside `run()`, `bounded()`, socket setup and the timed protocol flow. No expensive reference oracle was inserted into `OpenSession`.

### K2–K3: independence, randomness, security and budgets

The complete candidate file was independently reconstructed from the base using only the approved helper/regression insertion and old-search substitution; it equals the current file exactly. Byte-identical protected function bodies include `peer_prime`, `peer_hmac`, `open_pair`, `pair`, `run`, the original leading-zero session test and both stall tests.

- The production client still draws its private exponent from `OsRng` in `cloud_keyring.rs:101–109`; production IV randomness is also unchanged. No fixed client key/exponent, test seed, injected derived key or production fallback was introduced.
- The fixture receives only the public value. Its independent prime/HMAC/HKDF computation,128-byte left padding, public reply `2^private mod p`, and actual independently encrypted create/read exchange remain intact. It does not obtain the client's private exponent or call the production derivation.
- The original test at `cloud_keyring_tests.rs:853–885` still traverses `pair()` → private `UnixStream::pair()` → production `Session::open_stream()`, EXTERNAL authentication, owner/UID checks and encrypted negotiation. It never calls fixed-path `Session::open()` or the real user bus.
- Key equality, leading-zero, unique-owner/call/target, encrypted item, MIME/attributes, drift/rejection, no-auto-start and no-Prompt/unlock assertions are not deleted or relaxed. Installer validation is unchanged, not bypassed to obtain a green result.
- Two-second outer test wrapper, one-second original save-effect guard, three-second production cap,100ms private-D-Bus stall and150ms OpenSession stall remain unchanged. No `spawn_blocking`, timeout increase, retry, ignore, fallback or harness scheduling change was added to source.
- `cloud.rs:325–366` remains a current-thread cooperative runtime with a final elapsed-deadline check. This change reduces fixture work; it does not add hard preemption or establish a worst-case wall-clock guarantee.

## Required Checks Evidence

All statuses here refer to the bounded local repair, not release approval. Previously executed gates were audited, **not rerun** by the reviewer.

| Check | Command / scenario and expected outcome | Actual result / status | Evidence |
|---|---|---|---|
| K1 pure regression and range/exhaustion | Two new exact regressions; original modpow tuple, first hit/order/exclusive bound and fail-on-exhaustion preserved | **PASS**: both regressions pass in targeted/full logs and fresh reviewer run; range/exhaustion proven above | Current source; `P/samples-original.jsonl`; `C/logs/make.log` vertex22; `A/keyring-run.log` |
| K2 actual original encrypted session | Original random-client test, independent private peer, padding and encrypted save/read unchanged | **PASS**: named test passes in uninstrumented targeted/full logs and reviewer run; protected body matches base | `P/sources-original.json`, original build/binary binding; `A/static-bindings.json` |
| K3 production/security/budgets | Base/current comparison; no production/profile/dependency/workflow/assertion/installer weakening | **PASS**: exact approved file reconstruction; protected bodies and production bytes match; no other code diff | Git base/diff, core manifest, `A/static-bindings.json` |
| K4 normal/constrained original repetitions | Fixed20 normal +20 constrained original exact tests, four same-CPU competitors, no replaced sample | **PASS**:40/40; constrained maximum208.115762ms process wall; all20 keyring and both exact regressions also pass | `P/PLAN.md`, sample commands/affinity/readiness, sample-level logs, `P/SHA256SUMS` |
| K4 separate timing diagnostics | Fixed10 normal +10 constrained, separately instrumented binary; preserve assertions and budgets, emit metadata only | **PASS**:20/20; constrained maxima search44,682µs, pair135,057µs, save29,121µs, whole166,399µs | `P/instrumentation.patch`, build/source identities and actual per-sample metadata/logs |
| K4 cleanup/preserved failures | Preserve setup failures and no test retries; owned competitors stopped/reaped | **PASS**:120 started/120 reaped; ambiguous anchor and rejected cached diagnostic binary preserved; no failed test/sample replacement found | `P/events.jsonl`, phase/build records, samples and rejected binary |
| K4 service quality/full tests | `sh service/scripts/test.sh` in configured Dagger service container: fmt, locked all-target Clippy `-D warnings`, locked all-target tests | **PASS**, original make exit0:265 passed/3 controlled ignores/0 failures; test vertex22 is DONE, not CACHED | `service/scripts/test.sh`; source-bound `dagger/src/smith_next/main.py`; actual `C/logs/make.log` |
| K4 installer quality/full tests | Corresponding installer fmt/locked warning-denied Clippy/locked all-target tests; no weakened validation | **PASS**:283 passed/2 controlled ignores/0 failures; vertex61 DONE, not CACHED | `installer/scripts/test.sh`; actual vertex61 compiler/test stdout |
| K4 GTK | GUI-feature fmt/Clippy and serial GTK tests in isolated Xvfb | **PASS**:9/0; vertex72 DONE, not CACHED | `installer/scripts/test-ui.sh`, configured Ubuntu container and stdout |
| K4 Flutter/compiled Rust↔Dart | Format/analyze/full unfiltered `flutter test --no-pub`; required compiled service/cloud fixtures, no fixture skip | **PASS**:81 formatted files unchanged, analyzer clean, final `+668: All tests passed!`, no failure/skip counters; vertex125 DONE, not CACHED | Actual fixture build/injection/environment, registered tests/skip conditions, `client/scripts/test.sh`, `A/static-bindings.json`, stdout |
| K4 actual native package tests | Python package tests must receive real native CLI/GUI, not substitute header fixtures or skip | **PASS**:41 named cases and OK; vertex103 DONE, not CACHED; both `SMITH_TEST_INSTALLER` variables present | `dagger/src/smith_next/main.py`, actual package test names/outputs and supplied-native-file vertices |
| K4 once-only aggregate/build and export | `make DAGGER=target/installer-pipeline/smith-dagger-0.21.9/dagger test installer/build`; finite1200s command | **PASS**, exit0 in153.715s; all1,266 passes have current non-CACHED test stdout. Linux build, interpreter-free Ubuntu CLI checks and full package complete | `C/commands/make.json`, preflight, raw/plain make logs; unchanged source-bound gate scripts |
| K4 explicit package verification | `python3 -B dagger/package.py --root installer/dist --verify`; finite60s | **PASS**, exit0 in0.386s;95 manifest payload entries/97 regular files,20 directories, paired helper equality | `C/commands/package.json`, log and independently rehashed complete package inventory |
| Separate helper controls | `SMITH_TEST_PACKAGE=installer/dist python3 -B -m unittest discover -s scripts/tests -p 'test_*.py' -v`; finite180s; real-package roundtrip required | **PASS**, exit0 in25.294s;34 cases including actual-package roundtrip, no skips | `C/commands/helper.json`, actual log, package/archive identity. One multiline unittest description was reconciled, not miscounted as a missing case |
| Pure cost evidence | Original/reference16 oracle cases +96 paired timed samples; equality and fixed schedule, not a historical reproduction | **PASS**:112 distinct sample records/equality results,16+48+48 schedule,0 failures/timeouts; longest measured scan546 candidates | `M/PLAN.md`, pure example/source, samples/build/profile/environment and indexed hashes |
| K5 independent integrity/privacy/claims | Recompute current source/package/log hashes; inspect failures, cache/freshness, privacy and scope qualifications | **PASS** for local scope, with low binding note below; final documentation-status rebind still requested | `A/audit.json`, `A/static-bindings.json`, this report and exact reviewed evidence hashes |
| Fresh reviewer execution | `python3 -B A/run-keyring.py` → original binary `cloud::keyring::tests:: --test-threads=1`,10s external limit, private roots/minimal environment | **PASS**, exit0,20 passed/0 failed/0 ignored,1.725148s;90 unrelated library tests intentionally filtered. One invocation, no retry | `A/keyring-run.json` and `A/keyring-run.log`; exact binary/source hashes above |
| Local hygiene | `git --no-pager diff --check`; refreshed diagnostics for changed test file | **PASS**, diff check exit0; diagnostics refreshed with no errors/warnings | Reviewer command/tool results |

Rust stdout was reconciled by actual harness: service `108+1+35+34+12+23+11+41=265`; installer `105+0+36+8+17+20+4+11+2+20+35+25=283`; GTK9. Explicit Rust `... ok` lines agree with these totals. Package41 named cases, Flutter's final668 result and helper34 outcomes were inspected independently of the published counts. Compacted Flutter progress is suite evidence, not a separate retained line-per-case proof; required fixture execution is additionally bound through the unchanged complete test command, compiled fixture injection and false skip conditions.

The five existing ignores are service `proxy_environment_child`, `captured_helper_fixture`, `dart_cloud_fixture`; installer `launch_parent_fixture`, `progress_output_probe_child`. Their unchanged source documents controlled subprocess/fixture entrypoints and explicit invocation paths. They are not newly ignored requirements. No current required local test failure or skip is waived.

The83 targeted passing executions are repetitions, not83 new unique tests; diagnostic20 are not original-test passes. The reviewer's additional20 executions are also not added to the1,266 aggregate or recast as unique coverage. The before/after timing example has measurement counters and omits the final unused multiplication on exhaustion; successful sampled search paths match the reviewed algorithms. It is not a rebuilt historical hosted run or a session-cost guarantee.

## Findings by severity

### Critical

None found in the bounded source repair.

### High

None found in the bounded source repair.

### Medium

No new source defect found. Existing timing, environment and release-evidence limits are retained below, not silently accepted as production guarantees.

### Low — L1: supplementary plan snapshot is not a current-plan digest

**Reproduction:** run `sha256sum docs/plans/LOCAL-SMITH-KEYRING-PEER-COST.md` and compare with the plan record at `workflow-helper-controls.jsonl:13`. The run-time record is `9fe989a9174af96c0ac885ae0328f679815e1f0bf0b363e433893bf067421099`; the reviewed current plan is `36f075c1a332ec723dc1bdfb9fad59dfc4fa5fc953266f61470c7e22b4157cb1`. It is not merely an identical byte prefix plus an appended outcome. Other supplementary controls match; all1,230 core inputs match exactly.

**Impact:** the supplementary manifest remains valid historical gate-input evidence, but its plan row must not be presented as binding the final plan. It does not invalidate the code/test evidence or the active user-approved K1–K5 reviewed here. Current plan/evidence hashes are explicitly rebound in this report. Preserve original records rather than rewriting them to hide the difference. Complete the requested final documentation/source rebind after status-only prose updates; no source remediation or heavy gate rerun is indicated by this note.

## Evidence Gaps and retained limits

- **None of the required bounded local K1–K4 build/runtime/test evidence is missing or failing.** K5 fresh assurance is supplied by this report at the exact initial-review snapshot. Parent-owned status prose and the requested short final rebind remain a documentation closure step, not permission for an external effect.
- **Historical attribution remains unknown:** the supplied historical hosted failure is not reproduced. The timeout could not be localized to a precise CPU stage or conclusively distinguished between the two-second wrapper and one-second save guard. This review made no new GitHub query and does not report an updated hosted pass. The historical failed gate remains failed until separately authorized and successfully rerun on an updated revision; this local source recommendation does not waive it.
- **No worst-case timing/preemption guarantee:** the finite, synchronous, variable-length search still runs on a cooperative current-thread runtime. Original random sessions and deterministic cost samples are useful finite evidence, not coverage of all random tails, hosted scheduling/CPU quotas or hard deadlines. Instrumented stages overlap; maxima are not summed into a fictitious single sample. Uninstrumented208.116ms is process wall, not an internal stage.
- **Cache and environment:** existing dependency/build/Dagger caches were allowed and used. The five test vertices themselves completed with current stdout, but this is not cold/hermetic reconstruction, enforced-image-lock proof or clean-source provenance. The actual service quality/full-test container is Rust1.98 Bookworm, not an additional Ubuntu service-quality run. Installer/GTK/Flutter/package use configured Ubuntu24.04. Canonical Ubuntu quality/recipient target language must not be mistaken for an extra execution that did not occur.
- **Order/deadline qualifications:** helpers ran after build with the real package, not hosted `needs` order. Recorder source/metadata establish finite make1200s/package60s/helper180s command limits; no single overarching original-session deadline can be reconstructed. The continuation is identity/log reconciliation only, not another gate run.
- **Artifact correction:** the old `prior_ci_artifact_inventory_equal_on_shared_fields=false` record is preserved. Independently removing the prior `installer/dist/` path prefix and omitting its root entry gives equality of all117 file/directory entries on type/mode/size/SHA256. No artifact was silently replaced to correct that comparison.
- **Retention/privacy:** reviewed published evidence and310 retained text files were scanned for literal operator/repository paths, unredacted named homes/runtime UIDs, private-key headers, probable provider keys and available ambient secret values, then checked against source. Two pattern hits were generic redaction-regex literals, not private paths. No actual credential/private-key/private-path leakage was found in the inspected retained evidence. This is not a proof about every byte of all ignored caches/binaries. No cryptographic values are emitted by the diagnostic timers; synthetic fixed oracle vectors are not live credentials. Ignored raw logs remain local-only and are not a Git backup, public retention or authenticity guarantee.
- **Release/deployment prerequisites remain outside this repair:** updated hosted gates where required, clean-source provenance/effective locking, licenses/icon/SBOM/redistribution, appropriate compatibility/restore and separately approved installed/real-manager/keyring/provider/model/graphical acceptance, plus exact human effect approval. Local hashes/tests cannot supply these missing external outcomes or authority.

## Reviewer validation record

Private review artifacts are under `A` only:

| Artifact | SHA256 |
|---|---|
| `audit.py` | `bcfea380eed9165548303a7b5666e7b87939a6137082811104ae11ea4d61a223` |
| `audit.json` | `1abd79f5ca1b3b42f14e06af854c92be435cd0243f2017e04da686448a34d7c5` |
| `static-bindings.py` | `18190783aa19d11c7ddf4547a6430156a4fe397526dca9bdf8b38c1df4c07bf0` |
| `static-bindings.json` | `871a80374291416e18f13a8ce8df8f6c1525a334f9cad077257c845b694a5a1a` |
| `run-keyring.py` | `47d5b42d94282f68c59c4563e43b04681a741fa653e54b2a231e46eeb8cd94ce` |
| `keyring-run.json` | `0d2ef39c68eb05bde91943dcfe82f755172aaced13a11006d4663b20703f4cb9` |
| `keyring-run.log` | `8bb7fe8a3529da9046d7d1aff11023b72c3c403650102e4ef8918b6d4fbe1712` |

Final read/hash audits exited0. Initial read-only inspection/audit attempts exposed reviewer parser assumptions: a source manifest is an array; plan equality is not a prefix requirement; diagnostic sanitization deliberately extracts an interleaved test-name prefix into sample metadata; the reconcile command record omits the optional `status` field. Those assumptions were corrected after reading the actual records/sanitizer; the first audit script/output are retained as `audit-initial.py`/`audit-initial.json`. These were not test failures, retries of test samples or changes to source/evidence. The single fresh20-test execution passed on its first invocation.

## Final recommendation

- **Verdict: pass-with-risk — bounded local source repair K1–K5 at the reviewed identities.** The risk label records finite timing/evidence limits, not agent acceptance of risk or a waiver of any required failed check.
- **Source-repair recommendation:** retain the recurrence and two pure regressions; no code remediation identified. Finish parent-owned review-status prose and the requested short final documentation/source rebind before treating the documentation snapshot as closed.
- **Release Gate Decision: no-go.** No release, publication, merge, installation or deployment approval is supplied; broader required release/installed/hosted evidence and exact human approval remain separate.
- **Required remediation:** documentation-status closure/rebind for this local record; separately satisfy the relevant missing release/deployment gates and obtain exact human authority before any such effect. Do not increase budgets, disable installer checks, retry failed required tests until green or infer hosted success from these local results.

## Final documentation/source rebind — 2026-10-17 04:25:48 UTC

**Implemented closure, independently verified:** this append completes the status-only documentation closure and final rebind requested above. The original review's first **25,937 bytes** remain unchanged, SHA256 `19eb5e4014379367d95f1c59d28a7c484106fe306fac284c1727712e234bfc90`. Initial hash tables, findings and private review records are preserved, not rewritten; their pending-closure wording is historical and resolved by this append.

Exact reverse-edit/hash comparison reconstructed each initial reviewed document byte-for-byte: plan and `VALIDATION.md` changes are assurance-status prose; README changes add assurance status and explicitly label the supplementary plan row historical; `results.json` changes only `independent_assurance`. All other results, criteria, counts, commands, limits and approval boundaries are unchanged. The final text accurately describes source assurance, not hosted success, accepted risk or external approval.

### Final bound identities

Repository/branch/base remain `smith` / `feature/autonomy` / `becc14ed00da326e9ef8585d1fa309e893be48b2`.

| Final object | SHA256 |
|---|---|
| `docs/plans/LOCAL-SMITH-KEYRING-PEER-COST.md` | `3495981d7c6676369824f348ffbedda589b25984f1fd46908d25434209133be6` |
| `VALIDATION.md` | `2ce1bd3eebeb9e8e786e973bb2c1f66f04a34c95ade6cba770f4247144ae938e` |
| `docs/evidence/keyring-peer-cost/README.md` | `9a585ba7c6f1ff761cf50e7d6eaefaf1703416b0497e5a78325071b7637f9b82` |
| `docs/evidence/keyring-peer-cost/results.json` | `ec6638665fb53f3ef224672b01cdddc5fcc6799fac4b18188fab735037eee0fe` |
| `docs/evidence/keyring-peer-cost/core-source-manifest.jsonl` file bytes | `d395418651684c77be3d748a63b51834a7deea0a08ae71b96d0dcb2b3cf60a27` |
| Same1,230 core records, compact sorted-key JSON array | `2e054a19a5879d3d70ed72927015277916f2c6cad8bfd124e59357a94757a301` |
| `docs/evidence/keyring-peer-cost/workflow-helper-controls.jsonl` | `005539639f8de338acf0d9c8e79ea897829d3fb9df54b6bafbeefe55a4d7c73d` |
| Candidate `service/src/cloud_keyring_tests.rs` | `bd51d10ff5bc2a6657df7b06aa5d85d8dcaf445a76ec7ac2b36fa095ae565888` |
| Exact test-file Git binary diff against base | `d839eba59bdea3254857fa6d6bd4b5f8f72ef2a17dc7badf22e93a87d8bf4794` |
| Production `service/src/cloud_keyring.rs` | `f6ac5c6ea536992ad7bf252d92ef06b4ab8085b6393dc2e1592c53f5628c20aa` |
| Production `service/src/cloud.rs` | `7e7faba6ae9a841c3c157368c557dc8004d1f5dc2b0de7d316519b3a4f5d0aa0` |
| Current package canonical inventory,117 entries | `5c3ed9f62440134ab0e1e7f01b1e79114bacc08e12fba444f03ff14219def007` |
| `A/final-rebind.py` | `b71d40199864cc075f0fc2430b30f97003a5e44aba50a31ba64d4105efc0c72d` |
| `A/final-rebind.json` | `9aa351f8cd7e102b11729a7ba0ce628db0ccdb9286a9508e02186ed031d5f853` |

The read/hash/status rebind exited0. All **1,230 core paths/bytes/sizes/modes**, the core input selection and before/after source bindings still match. All **1,300 tracked files other than the reviewed `VALIDATION.md` closure** match the original gate snapshot. No staged change or additional code/source drift was found. The21 raw-evidence references,17 original CI evidence references,35 command records and their logs,205 indexed scratch files, other supplementary controls, prior reviewer artifact hashes and current package inventory still match. `git diff --check` passed. No gate, test, build, package verifier or external action was rerun for this closure.

**L1 disposition: closed for documentation binding.** README now explicitly distinguishes the historical gate-input plan row from final documentation, and the exact final plan/docs/results are bound above. The historical plan hash and controls file remain unchanged by design; closure does not assert equality with the final plan or erase the original finding. No source remediation, risk waiver or newly required local verification follows from this documentation-only update.

- **Verdict: pass-with-risk, unchanged, for the now-closed bounded local K1–K5 source/documentation snapshot.** No new required evidence gap, source drift or required test failure was found; all previously stated timing/environment/provenance limits remain.
- **Release Gate Decision: no-go, unchanged.** Hosted/installed/release prerequisites and exact human effect approval remain separate and unsatisfied by this report.
- **Required remediation:** none remaining for this local documentation rebind. Before any broader effect, satisfy its separately required evidence and obtain exact human approval. Any subsequent reviewed-input change requires a new binding/review as appropriate. The completed whole-file review digest is reported outside this file to avoid self-reference.
