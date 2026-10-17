# Independent assurance — runner parked-lease repair

- **Scope:** fresh-context assurance of `service/src/runner.rs`, new `service/src/runner_lease_tests.rs`, the approved L1–L7 plan, and related canonical documentation and retained evidence only.
- **Candidate:** `feature/autonomy`, base/HEAD `c1339f3670c70582db05a158b46821353662b87b`; isolated base archive plus exactly two approved source overlays, **not** the concurrent Projects work or live mixed worktree.
- **Method:** independent production-diff and ownership analysis; read-only hash, archive, snapshot, artifact and raw-log verification. No build, format, pipeline rerun, live inference or external action was performed by assurance.
- **Result:** required bounded source evidence verified; no blocking implementation or documentation finding. Source acceptance does not approve external effects or waive residual risks.

## Authority and review boundary

Read `AGENTS.md`, `docs/SCOPE.md`, relevant Architecture/Process/Design/Host Profile sections, `roles/assurance.md`, then `docs/plans/LOCAL-SMITH-RUNNER-PARKED-LEASE.md`. Reviewed the complete production diff against the named base and the complete new test module. Inspected the repair-specific edits to `docs/ARCHITECTURE.md`, `docs/SMITH_LINUX_ARCHITECTURE_REPORT.md` and `VALIDATION.md` and the current evidence package. No builder private reasoning was used.

Only this new tracked report was written. No staging, index restoration, unrelated source edits, commits, push, hosted execution, installation, service/manager/keyring/model/API/GPU activity or publication was performed. Read-only Git queries and Python checks used `-B` where applicable. No existing raw evidence record was modified.

## Production assessment — implemented bounded contract

The diff introduces private `ParkedLease(Option<File>)`, changes the existing shared slot types and field access, removes the model-only destructor, and attaches the private test module. It does not introduce reference-count heuristics, synchronization policy, new dependencies, retries, sleeps, deadlines or runtime capabilities.

The populated parked value now explicitly issues `LOCK_UN` on final destruction without acquiring its surrounding mutex. This fixes the owner-lifetime hole: terminal cleanup can return the file after the temporary model owner has gone, and final slot destruction still unlocks even while another descriptor retains the OFD. Poisoning cannot suppress that value destructor. An empty slot has no descriptor to unlock.

`Runner.lock` remains `Option<File>`, not a generic unlock-on-drop guard. Consumption takes the same file out of the shared slot; confirmed handback puts it back without unlocking. A surviving model therefore retains continuous exclusion across handback and reconsumption. The new tests probe independently opened contenders, retain an OFD duplicate through release assertions, check exact `WouldBlock`/`EWOULDBLOCK`, and explicitly unlock successful probes.

Independently extracted and compared the production cleanup section from child handling through directory cleanup, before lease handback: it is byte-identical to the base. In particular, failed TERM/KILL exit confirmation restores `self.child` and returns before taking `self.lock` or `lease_return`. Child inheritance of the raw lock descriptor, cleanup signaling/waits, fallback error classification and launch/request budgets are unchanged. This is static preservation evidence, **not execution of an actual child-exit timeout**. Directory-cleanup refusal is a different existing integration scenario.

The known raw-file windows in `verify_model_leased` before slot attachment and in launch after consumption but before complete `Runner` construction remain. This repair neither fixes them nor accepts/waives their risk. No exact historical hosted holder, errno or scheduling attribution is established.

## Required Checks Evidence

All results below concern the frozen candidate and retained executions; assurance independently verified the records rather than rerunning the pipeline.

| Check | Command/scenario and expected outcome | Actual outcome / status | Evidence |
|---|---|---|---|
| L1 terminal final release | Private actual `Runner::stop` and Drop with no child/already-cleaned state, model owner dropped, duplicate retained; contender must acquire | Both assertions pass within seven-test ownership run; exit 0 | `targeted-results.json`; raw `candidate-ownership.log`; test module lines 143–170 |
| L2 active exclusion | Drop model while file is raw active ownership and slot empty; contender must remain busy | Pass; no active-file unlock destructor added | Same ownership log; test lines 173–190; production diff |
| L3 continuous live-model lease | Handback/reconsume same descriptor, probe exclusion until final model destruction; existing startup fallback/no-replay regression | Ownership test passes; original integration test passes all ten exact samples | Test lines 193–217; raw `normal-1..3.log`, `single-cpu-1..3.log`, `contended-1..4.log`; corrected full target |
| L4 unknown cleanup | Empty-slot/raw-file/duplicate ownership simulation must retain exclusion, followed by explicit positive unlock; real cleanup early return must be unchanged | Pass, exact busy error checked; cleanup section independently compared equal; existing cleanup-refusal integration passes | Test lines 219–241; base/current source comparison; `corrected-full-target.log` |
| L5 poisoned slots | Destroy populated poisoned slot: release; destroy empty poisoned slot: preserve raw exclusion | Both pass; intentional caught poison panics are expected, not failures | Test lines 243–275; `candidate-ownership.log` |
| L6 targeted gates | Seven ownership tests, eight existing private tests, ten fixed exact samples and corrected complete target | 7/7, 8/8, 10/10, 41/41; successful executions exit 0 | `targeted-results.json`; bound raw logs and artifact records |
| Discriminating controls | No-op destructor must fail final release; premature handback unlock must fail continuous exclusion | A: five expected failures/two passes; B: one expected failure/six passes; both exit 101 as intended | Raw `negative-a.patch`, `negative-b.patch`, ownership logs and source inventories |
| L6 aggregate quality/build | Snapshot-root `make DAGGER=<verified CLI> test installer/build`; formatting, locked warning-denied Clippy, tests/builds must pass | Exit 0; 272 service +283 installer +9 GTK +668 Flutter +41 package =1273 fresh passes; five controlled ignores, zero skips/failures | `commands.json`; `ci-results.json`; raw `execution2-aggregate-build.log`; pinned component quality scripts |
| L6 package/helpers | `python3 -B dagger/package.py --root installer/dist --verify`, then actual-package `python3 -B -m unittest discover -s scripts/tests -p 'test_*.py' -v` | Both exit 0; 95 payload entries/97 inventory files; 34 helpers pass, no skips | Bound package-verify/helper logs; artifact inventory; actual-package roundtrip digest |
| L7 documentation/evidence | Canonical ownership contract and factual assessment, preserved failures, privacy, independent assurance and exact bindings | Verified; this report supplies fresh assurance | Architecture/report/VALIDATION repair edits; plan; current README and JSON/checksum records |

### Independent evidence reconstruction

Read-only commands included `git --no-pager diff <base> -- service/src/runner.rs`, the corresponding scoped documentation diff, `git --no-pager show <base>:service/src/runner.rs`, branch/HEAD queries, `sha256sum`, and Python `hashlib`, `tarfile`, JSON and raw-log checks. An independently generated `git archive` stream matches retained `base.tar` SHA256 `dc83806dbeb5247340e675f7bf33bb9309c5c8c0a88603150298bd0f1bed4f4e`.

Verified:

- All **57 original targeted +3 supplemental +40 CI indexed records** match retained checksums. All **seven** command-log hashes match; retained `commands.json` equals raw `execution2-results.json`.
- All targeted run log hashes and parsed test summaries agree. All 16 competitors are recorded reaped and absent at the final owned-wrapper check. The fixed schedule and separate negative-control patches are retained.
- The original missing-`cc` compile exit101 and full-target **2 pass/39 fail** remain present. Their logs show linker resolution and unsafe-ancestor failures. Corrections are harness-only. The separately recorded corrected full run uses the unchanged binary and reports **41 pass/0 fail**, 22.69s, exit0; it does not overwrite or erase those failures.
- All **1231 core** manifest content hashes and modes match the retained snapshot. All **1308 total** input hashes match the retained before record; before/after records are identical. Against the 1307-file archive, the sole changed existing file is `service/src/runner.rs`; the sole additional declared input is `service/src/runner_lease_tests.rs`. Targeted candidate's 52 service source inputs also match the CI snapshot and its source before/after inventories agree.
- The isolated Git boundary has no index or actual refs. Its object files are disclosed; it is not copied live repository metadata. `target/runner-parked-lease/` raw records are ignored according to `git check-ignore`.
- Raw Rust summary lines independently sum to272/283/9; Flutter's final `+668` has no skip count; native package reports41 and helpers34. Five exact ignored-entrypoint bindings match. ANSI stripping is needed when comparing displayed Dagger status text; an initial assurance parser assertion without that normalization was corrected read-only, not a candidate failure.
- Every cited test/fixture/native-runtime Dagger execution vertex reports `DONE`, not `CACHED`; dependency/build caches remain disclosed. The pinned fail-fast quality scripts contain formatting, warning-denied locked Clippy and locked tests. Compiled service/cloud fixture environments are supplied to Flutter. Configured service container is Bookworm; no supplemental Ubuntu service execution is inferred.
- All97 retained package inventory file hashes match current isolated artifacts; paired service copies agree at `65dcd05ffbec9f8c536be35687f57451eec4dfd2417bf87fac66831f02b9ca0f`. Candidate unit and integration executable hashes match their artifact record. Roundtrip digest is `6687d03a1d2c38e7574fb1573458f861f59a29116fb132184ba9747d2c0af91f`, not clean-source provenance.
- Retained tracked evidence/plan were scanned for home/root private paths, credential-like tokens and private keys, with no matches; manual document review found no raw private transcript/secret disclosure. This is a bounded privacy inspection, not a universal secret-detection guarantee.

## Reviewed identities (SHA256)

| Input | SHA256 |
|---|---|
| `service/src/runner.rs` | `3bd158de9566d0b84f8d296b6472bb3c3efc3d3b2a1631fdf77aaf83f7dc05c8` |
| `service/src/runner_lease_tests.rs` | `6a504792ce009209c5ff58aecd0111b934f4b47ed0d6a27a8e9539c3f4281a19` |
| `core-source-manifest.jsonl` | `3f2e19c5867be87b6d03c9b26c305bad74340995018491eb6caeaf883446fdac` |
| `README.md` | `57ec9024bde768748101fb76cbfcf70b1e517f2397fe38a25dce451ee8897b56` |
| `ci-results.json` | `9d34c9f62ad18d8ea629280dcef71772fed5465b02560b31a3ee204e0d657540` |
| `commands.json` | `f96058baf98388f034cc81c9e28511615dd08be5c559e819b7588a9c21e77af2` |
| `targeted-results.json` | `ee9a5c92c86d35ef837ab2f4b40e35fd03d2785206731326cf2bbacbca2275ec` |
| `raw-targeted-evidence-checksums.txt` | `2a7384402458b0e7819a8fc559ae9316bc1c64316f2e099056b9d8836f0a7276` |
| `raw-corrected-target-evidence-checksums.txt` | `76dd88c48336e25f5831c797af1be0ca2d133f32c6c1c8d4913cb071053436c8` |
| `raw-ci-evidence-checksums.json` | `4938fb123e78fcc0477b522105e070d47e3578203b51e7cae57fa9d962d957d9` |
| Plan | `727bc46d6105bb25037518c4fb962198b09340aad528cb86af20c71951b38f3d` |
| `docs/ARCHITECTURE.md` | `b4459abb4083470c7af8aaa3dbcd3ae76e899f3691bcd69164d9709b74c1c1f0` |
| `docs/SMITH_LINUX_ARCHITECTURE_REPORT.md` | `0ef1180e4fecda212bdb9a9154cdda4cd8664a5ed34a48116a065d981d965411` |
| `VALIDATION.md` | `9ee91df592f93a6300dd7a8bbe7a1a2df1ecadad14d0a6b62384a28796a0e67c` |

Evidence filenames without a repository prefix in this table are relative to this report's directory. Targeted unit binary SHA256: `7a09e03d1d44f2e4a94a9c5c94a518ccfcdf282b61594d7c719c7f4b75d335c3`; corrected integration binary: `36a8e2ce100f299033c0ca6f69374fd678de411dffe572725815496b2c6ecd6c`.

## Findings by severity

- **Critical:** none in reviewed scope.
- **High:** none in reviewed scope.
- **Medium:** none requiring bounded-source remediation. Live-index byte preservation remains **unresolved**, not silently passed: recorded before `dadf73f26466e47f5e9680ad21811c6bada9c596553f2bc578b2a2d0a3a534cb`, after `76a28643e868216e37d5c6af900a20697bda8eb6145bb81217be3e5de0d8eaa8`. Stable snapshot results do not establish attribution or authorize restore/staging. Live Projects work is excluded.
- **Low:** no required prose correction found. Raw `commands.json` deliberately retains `blocked_identity_drift`; `ci-results.json` and README correctly distinguish successful source gates from unresolved live-index preservation. Do not rewrite that raw outcome to plain PASS. Pending-review prose is accurate at the reviewed revision and may be updated by the parent only with the requested brief final rebind.

## Evidence Gaps and limitations

**No missing required L1–L7 bounded-source evidence identified.** Assurance did not rerun already-built tests or the pipeline; execution claims above are independently checked retained evidence. These owner-writable hashes are local binding/integrity checks, not independent authenticity, archival availability in another checkout or clean-source provenance.

Actual unkillable-child/exit-timeout execution, real llama.cpp/model/CPU/GPU inference, installed/graphical behavior, live cloud/keyring/user-manager integration, cold/hermetic builds and hosted CI remain unverified and outside this approval. The unknown-cleanup ownership simulation and negative controls are not substitutes for those claims, nor are they represented as such. The historical CI failure remains failed. No acceptance of concurrent Projects changes, release readiness, external-action approval or residual-risk waiver follows from this review.

The implementation meets the **bounded implemented ownership contract**, not all deferred target policy or broader runner recovery guarantees. Pre-child residuals remain explicit. Any change to either frozen source requires renewed source/evidence review; pending-review-only documentation updates require the requested brief rebind.

## Decision

**Verdict: pass — bounded isolated source repair only.** L1–L7 are supported by the verified evidence, including required corrected successful gates; original harness failures and deliberate failing controls remain preserved and explained rather than waived.

**Release Gate Decision: no-go.** This assurance does not supply hosted/release/installed/provenance evidence or human approval for commit, push, hosted rerun, merge, installation or publication.

**Required remediation:** no source or evidence correction required for this bounded verdict. Parent should update only pending-review prose, then obtain brief final document/hash rebind before claiming the final reviewed package. Retain the unresolved live-index caveat, explicit pre-child residuals, original failures, simulation distinctions and all external-action boundaries. Human approval remains a separate gate.

## Closure — final pending-review prose rebind

Rechecked the final plan Outcome, root `VALIDATION.md` repair section and evidence README after the parent's pending-review updates. Independently reversed only the identified assurance-status/link sentences in memory and recovered each historical SHA256 recorded above, proving that these three files contain exactly those prose changes and no additional changes relative to the reviewed versions. The new statements accurately describe the bounded pass, preserve every limitation and grant no external-action or residual-risk approval. **No new blocker or required prose correction.** Historical bindings above remain intact; the final bindings below supersede only those three document identities.

| Final input | SHA256 |
|---|---|
| `docs/plans/LOCAL-SMITH-RUNNER-PARKED-LEASE.md` | `0ccf4acefe405c975c251bad41260014191073949013263d3fe470c461d230b3` |
| `VALIDATION.md` | `b25017cce3beecd9d570998a644f04bc1ddc0895d15967f8dc51ca74601cd047` |
| `docs/evidence/runner-parked-lease/README.md` | `901c1e7cf7ffc42ae0a61430c6a166a56f11307598077b9eb7779492ea266b1c` |
| `service/src/runner.rs` — unchanged | `3bd158de9566d0b84f8d296b6472bb3c3efc3d3b2a1631fdf77aaf83f7dc05c8` |
| `service/src/runner_lease_tests.rs` — unchanged | `6a504792ce009209c5ff58aecd0111b934f4b47ed0d6a27a8e9539c3f4281a19` |

Also rehashed Architecture, the as-built report, `ci-results.json`, `commands.json`, `targeted-results.json`, the core source manifest and all three raw checksum indexes: every identity remains equal to the historical review table. Reverified all57 original targeted, three supplemental and40 CI indexed raw records against those unchanged indexes, with no mismatches. Branch/HEAD remain `feature/autonomy` / `c1339f3670c70582db05a158b46821353662b87b`.

This closure used read-only checks and appended only this report; no test/build/format/pipeline rerun, staging, restoration, unrelated edit or external effect. The requested final document/source rebind is complete. **Final verdict: pass for the bounded isolated source repair; Release Gate Decision: no-go.** Unresolved live-index attribution, explicit pre-child residuals, simulation/actual-timeout distinctions and separate human external-action approval remain unchanged.
