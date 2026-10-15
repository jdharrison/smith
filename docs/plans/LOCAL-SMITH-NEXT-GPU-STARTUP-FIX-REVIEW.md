# GPU startup fix — independent source assurance

## Validation scope

- **Implemented source review:** [GPU startup fix](LOCAL-SMITH-NEXT-GPU-STARTUP-FIX.md), AC1–AC6, on `feature/autonomy`, base/HEAD `2cc71a94fe924fbb9d324f29332e9f24c8c5991c`. Implementation scope is `next/service/src/{acceleration.rs,runner.rs,providers.rs}` and `next/service/tests/runner.rs`; unrelated user/UI/multi-chat/installer work is not approved or comprehensively reviewed here.
- Final closure verifies retained gate logs, snapshot identities, the two subsequent test-only additions, and GPU-specific documentation amendments. The three production files are unchanged from the initial independent review.
- Method follows `AGENTS.md`, canonical scope/architecture/process/design/host policy and `roles/assurance.md`. Earlier adversarial tests used only extracted source and fake runners. Closure ran read-only source/hash/log checks, not full gates or the earlier timed-out wrapper.
- The only closure write is this review. No installed artifacts/state, service actions, GPU probes, model loading/inference, driver changes, commits or remote effects were performed or authorized.

## Snapshot binding

SHA-256 of the four-file `git diff` against the base above: `eae525e39f1d2283b348617ca0fe45cb45f564c4de489269f2cf08046ba148e3`.

| Reviewed file | SHA-256 |
|---|---|
| `next/service/src/acceleration.rs` | `d9ee167d6d60555f0ba16ed0fca49de0916522a62644771b30df7ec227df7abe` |
| `next/service/src/runner.rs` | `34ccf8da4da7edc262f8df8d4673b815b74de6ea3cd7d2ed659346d9aded05c7` |
| `next/service/src/providers.rs` | `978f9885735e8c261c8e50eea801c37681a0b3e74442ff082560767b91d952fc` |
| `next/service/tests/runner.rs` | `b0661d713bd63c5bd5ae3eaa80db9e347ae55e9c9d1251b6e41a07026d07c931` |

| Retained evidence | Verified JSON SHA-256 |
|---|---|
| [Final service/build](LOCAL-SMITH-NEXT-GPU-STARTUP-FIX-FINAL-EVIDENCE.json) | `8b5ed379f252bfeb878851e4a35ff4c08f5988ac1ef2c9c7941a728647ce074a` |
| [Dependent installer](LOCAL-SMITH-NEXT-GPU-STARTUP-FIX-INSTALLER-EVIDENCE.json) | `227bb7f793fb4d9aaf2d5846fca9ccc51d45ff2a90373c1b078374ba3ac4e0f8` |
| [Intermediate failed gate](LOCAL-SMITH-NEXT-GPU-STARTUP-FIX-EVIDENCE.json) | `3d9a3fa10766b7aa17b9bb392b9c3263e342c57b4c73ec68b5a8990f69cc4ba1` |

All 23 final service before/after hashes agree and match current files. Both 1003-entry installer endpoint arrays agree; individual content hashes, lengths, modes and types were checked. Their only current difference is the later `next/service/tests/runner.rs` lease-fixture correction, not installer production dependencies. All named log hashes match their files. Scoped Make/Dagger/service gate definitions remain unchanged from the base.

## Required Checks Evidence

Commands below are recorded parent/dependent-worker runs, independently verified from retained evidence; they were not rerun during closure. Both Make gates used `DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain` from `next/`.

| Check | Command / scenario | Expected | Actual | Exit / result | Evidence |
|---|---|---|---|---|---|
| AC1–AC5 source invariants | Exact-source parser/capture/admission harness; fake-runner logger, startup/fallback, lease, CPU arguments and diagnostic lifecycle cases | Strict bounded admission; unchanged CPU; cleaned single fallback/no replay; finite internal nonblocking history | Earlier independent harness: 24 passed (13 parser, 3 capture, 8 adversarial); 12 distinct fake-runner tests passed | PASS for completed tests; interrupted repetition excluded | Initial independent review session `1ec9c7f2-3b3f-4219-8b5e-4ad75b298147`; supplemental results retained below |
| Service formatting/lint/tests | `make service/test`: `cargo fmt --all --check`; locked all-targets Clippy with `-D warnings`; locked all-targets tests | All required checks pass | 169 passed, 0 failed, 1 controlled ignored probe; all 41 runner tests pass, including both corrected fixtures | 0; no timeout | [Final evidence](LOCAL-SMITH-NEXT-GPU-STARTUP-FIX-FINAL-EVIDENCE.json), [service log](LOCAL-SMITH-NEXT-GPU-STARTUP-FIX-service-gate-final.log), [script](../../next/service/scripts/test.sh) |
| Locked release build | `cargo build --release --locked --offline` from `next/service/` | Successful locked build | Release profile completed; incremental/cached, reported 0.02s | 0; no timeout | [Final evidence](LOCAL-SMITH-NEXT-GPU-STARTUP-FIX-FINAL-EVIDENCE.json), [build log](LOCAL-SMITH-NEXT-GPU-STARTUP-FIX-release-build-final.log) |
| Dependent installer | `make installer/test` | Native/GTK formatting, warning-denied Clippy, locked tests and no-Python CLI checks pass | 252 native + 9 isolated GTK passed; 0 failed; 1 controlled ignored probe; 12 suite summaries matched log indices and totals | 0; no timeout | [Installer evidence](LOCAL-SMITH-NEXT-GPU-STARTUP-FIX-INSTALLER-EVIDENCE.json), [installer log](LOCAL-SMITH-NEXT-GPU-STARTUP-FIX-installer-gate.log) |
| AC6 evidence/current-source binding | Python SHA-256/JSON checks, scoped diff, in-memory reversal of the new test blocks and recipe-only correction | Reproduce identities; explain subsequent differences; retain failure history | Final identities/logs match; only two intended test additions since initial review; installer recipe correction independently verified below | PASS for binding checks | Snapshot binding above; three evidence manifests |
| Documentation and whitespace | GPU-specific canonical amendments, plan and `next/VALIDATION.md`; scoped `git diff --check` | Accurate source-only claims, no whitespace errors | Consistent with implementation and approval limits; whitespace check clean | 0 / PASS | [Architecture](../ARCHITECTURE.md#current-next-bundled-native-runner), [Process](../PROCESS.md#current-gpu-preferred-source-validation-gates), [Design](../DESIGN.md#gpu-execution-experience), [Host Profile](../HOST_PROFILE.md#current-bundled-cpuvulkan-runtime-boundary), [assessment](../SMITH_LINUX_ARCHITECTURE_REPORT.md#gpu-startup-logging-correction--bounded-source-assessment), [GPU plan amendment](LOCAL-SMITH-NEXT-GPU-PREFERRED.md#bounded-startup-correction-amendment), [validation](../../next/VALIDATION.md#gpu-startup-logging-and-bounded-evidence-correction) |

Formatting/Clippy success is supported by successful execution of the inspected `set -eu` scripts, not merely the manifests' narrative. Repeated service summaries in stdout/Dagger trace were counted once. Ignored tests are controlled subprocess fixtures: service `manager::tests::captured_helper_fixture` and installer `model::tests::progress_output_probe_child`.

The earlier independent harness exercised the exact parser/capture/admission source without a real runtime. Fake-runner checks covered realistic pinned logging, Vulkan admission/fallback, diagnostic lifecycle, carried lease and exact CPU arguments. The optional 90-second wrapper **timed out during an accidentally repeated fallback test**, after its first ten tests passed. That test had already passed in the `vulkan_` selection; the interrupted repetition is not a pass. Two remaining short tests passed separately, giving 12 distinct completed tests. Ephemeral fixtures were cleaned and no live synthetic processes remained. These supplemental session results are not a retained standalone harness or a substitute for the final recorded gates.

## Findings and remediation assessment

### Critical / high

None identified in the bounded source diff. The reviewed implementation meets the bounded target contract: explicit `b11429` verbosity 4/plain logging, unchanged CPU flags, complete records, sticky raw 64 KiB cap, fixed at-most-one-second post-health admission under the original deadline, fail-closed cancellation/liveness/capture handling, confirmed-cleanup single fallback under the same lease, no replay, and finite literal-only fallback history behind a nonblocking accessor. This does not complete the broader GPU-preferred target.

### Medium — closed

**M1: required final gate evidence was unavailable.** Closed by the verified final service/build and dependent-installer records above. No required source gate remains missing or failing at the reviewed final snapshot.

The intermediate service failure remains a failure, not overwritten by prior passes: 168 passed, 1 failed, 1 ignored; Make exit 2, no timeout. The [retained log](LOCAL-SMITH-NEXT-GPU-STARTUP-FIX-service-gate.log), lines 337–340, records `RUNNER_BUSY` instead of `MODEL_UNAVAILABLE`. In `next/service/tests/runner.rs:2315–2341`, the fixture now polls its actual lease with nonblocking `flock`, for at most five seconds after explicit idle/cleanup, tolerates only `WouldBlock`, and explicitly releases its probe lock. Persistent contention still fails. It does not retry production work; the exact corruption error, diagnostic reset and no-new-launch assertions at lines 2343–2365 remain intact.

In-memory removal of this block/import reproduces the intermediate test SHA `b4c4d9815d927404db047cb7ec4f73973df1e3494978260b04459376c1a0ee30`. Removing the continuous-output addition as well reproduces the initially reviewed SHA `132635d6bd2de7df40f595a67935cbd422cc310338901b19c28a4a57d43231aa`. No other test delta or production correction was hidden. Source and the builder-reported synthetic fork probe support inherited-FD interference, but the failed run's lock holder is unknown; definitive historical attribution is **not** established.

### Low — closed

**L1 — closed: missing permanent non-sliding-window regression.** `next/service/tests/runner.rs:164–174,603–649,1270–1285` now emits complete heartbeat records every 50 ms for six seconds, below the cap, requires at least two captured heartbeats, retains the 900 ms minimum observation assertion, adds a scheduling-tolerant upper bound below four seconds, and verifies stop/cleanup. It detects continuous-output window extension; source inspection establishes the exact one-second limit. The final gate passes this regression.

**L2 — closed: evidence-recipe wording.** `LOCAL-SMITH-NEXT-GPU-STARTUP-FIX-INSTALLER-EVIDENCE.json:58–62` now explicitly specifies hashing the files array in **stored order**, sorting object keys only. Its nested `recipe_wording_correction` retains the prior JSON SHA-256 `8db9bef801d66873377a012de6aecb6b282d859dfd09b30e559bf876ab616f83` and identifies the documentation-only change. Independently removing that note and restoring only the former recipe wording in memory reproduces the exact prior JSON hash. Thus no snapshot entries, content hashes, command results or logs changed. The corrected `json.dumps(files, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()` recipe reproduces both endpoint and component digests. The updated JSON hash is bound above; all 23 final service inputs, the reviewed four-file diff and all retained log hashes were rechecked unchanged. No new tests were needed or executed, and no evidence file was edited by this reviewer.

## Evidence Gaps

- No outstanding required **source-scope** verification gap. The wrapper timeout above remains a limitation, not a successful repetition; closure did not rerun it or any full gate.
- Endpoint snapshots are not atomic/continuous proof against transient edits. Owner-controlled checksums bind retained bytes and detect stale mixing, not independent provenance. The release build is incremental/cached, not a clean-build or reproducibility result.
- No real GPU/model allocation, full offload, inference, performance, installed-service, driver, live graphical or deployment validation was performed. No fresh paired recipient package, licensing/SBOM or release-provenance assessment is supplied. These are outside this approval and remain separate gates, not inferred from synthetic success or earlier device-only observations.
- Final stderr admission covers observed bytes, not future output; health is not a flush barrier. Internal fallback history is neither current execution mode nor operator-visible diagnostics. Canonical amendments preserve these limitations. Unrelated worktree findings and historical aggregate failures remain outside this review.

## Scoped recommendation

- **Verdict: pass — bounded source scope only.** M1, L1 and L2 are closed; no open finding or production-source blocker remains at the bound snapshot. This supersedes the initial `pass-with-risk` recommendation after the independently verified documentation-only correction; validation limitations and approval boundaries remain unchanged.
- **Release Gate Decision: no-go.** This source recommendation grants no merge, release, deployment, installed-host or real-model/GPU approval.
- **Required remediation:** none outstanding for this bounded source scope. Preserve the failed intermediate run and timeout qualification. Any later external action needs its own exact candidate-bound validation, review and explicit human authorization.
