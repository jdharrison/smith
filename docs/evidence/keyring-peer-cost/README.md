# Keyring fake-peer search cost repair

**Implemented local test-only repair; source gates PASS. Independent assurance is recorded separately in [REVIEW](REVIEW.md). This is not hosted CI or installed acceptance.**

Work record: [LOCAL-SMITH-KEYRING-PEER-COST](../../plans/LOCAL-SMITH-KEYRING-PEER-COST.md). Base `becc14ed00da326e9ef8585d1fa309e893be48b2`, branch `feature/autonomy`. Actual command timestamps are UTC2026-10-17; the scratch directory's20261016 suffix is only a name.

## Failure and bounded implementation

[Hosted run38996557862](https://github.com/jdharrison/smith/actions/runs/38996557862) failed in `actual_session_negotiation_matches_independent_peer_with_leading_zero_shared_secret` with `KEYRING_TIMEOUT`. The Dagger/asyncio exit101 traceback propagated that Rust failure; it was not a second diagnosis. The historical log does **not** identify which stage expired or distinguish the two-second test wrapper from the one-second save-effect deadline.

The private synthetic peer previously restarted `public.modpow(private, prime)` for each ascending candidate. `service/src/cloud_keyring_tests.rs` now starts with `public² mod p` and advances with `shared × public mod p`. This preserves every residue, candidate range2..100000, canonical byte-length predicate, first selected candidate and exhaustion failure. Only this test file changes. It is compiled under `cfg(test)`.

Two pure regressions compare the selected private/shared tuple against the original `modpow` oracle on short deterministic vectors and verify that inverse-two first needs padding at candidate8, not2–7. The expensive reference computation stays outside the timed protocol flow. Original random production client, independent peer HMAC/HKDF,128-byte padding, encrypted save/read, identity assertions, two-second wrapper, one-second save guard, three-second production cap and100/150ms stall tests are unchanged. No retry, skip, fallback, fixed production key, timeout increase or disabled validation was added.

Research: [Tokio1.53.3 timeout](https://docs.rs/tokio/1.53.3/tokio/time/fn.timeout.html), [current-thread runtime](https://tokio.rs/tokio/topics/bridging), [num-bigint0.4.8 modpow](https://docs.rs/num-bigint/0.4.8/num_bigint/struct.BigUint.html#method.modpow), [Secret Service encrypted sessions](https://specifications.freedesktop.org/secret-service-spec/latest/ch07s03.html). Non-yielding CPU work is not preempted by Tokio's timeout; Smith's `bounded()` also rechecks elapsed time after runtime teardown. No upstream defect or exact historical CPU attribution is established.

## Targeted results

- Pure-search diagnosis:16 oracle cases plus96 timed pairs, all112 exact-result comparisons equal, zero failures/timeouts. Longest measured scan546 candidates: normal median search wall404ms→8ms; four same-CPU competitors1,141ms→17ms. These are search-only measurements, not session timing.
- Uninstrumented candidate: both new regressions, original exact session test and all20 keyring tests passed. Predeclared20 normal +20 single-CPU/four-competitor repetitions of the original random-client session test all passed; observed maximum208.116ms under contention.
- Separately bound scratch instrumentation:10 normal +10 constrained sessions passed. Constrained maxima: peer search44.682ms, pair/auth/session135.057ms, save29.121ms, whole test166.399ms. Stages overlap; maxima need not come from the same sample. No private/shared/derived-key values were retained.
- Targeted totals:64 invocations/83 passing test executions, of which63 are uninstrumented and20 diagnostic. Repeated tests are not additional unique aggregate coverage.120/120 owned competitors were stopped and reaped.
- Two scratch setup/binding failures remain preserved: an ambiguous insertion anchor and an unchanged cached executable rejected before instrumented testing. A fresh separate diagnostic target supplied the instrumented binary. No failed test was retried or sample replaced.

## Full local source gates

The actual root commands, with verified official Dagger v0.21.9 and existing isolated engine/cache:

```sh
make DAGGER=target/installer-pipeline/smith-dagger-0.21.9/dagger test installer/build
python3 -B dagger/package.py --root installer/dist --verify
SMITH_TEST_PACKAGE=installer/dist python3 -B -m unittest discover -s scripts/tests -p 'test_*.py' -v
```

The recorder resolves `SMITH_TEST_PACKAGE` to the verified repository-relative package's absolute path; sanitized exact argv/environment, command times, finite per-command budgets and log hashes are in [results.json](results.json).

| Suite | Passed | Failed | Controlled ignores | Skipped |
|---|---:|---:|---:|---:|
| Service |265|0|3|0|
| Native installer |283|0|2|0|
| Isolated GTK |9|0|0|0|
| Flutter |668|0|0|0|
| Actual native package |41|0|0|0|
| **Aggregate** |**1,266**|**0**|**5**|**0**|
| Separate pipeline-helper suite |34|0|0|0|

All1,266 aggregate passes have current non-CACHED test stdout; dependency/build vertices still use cache. No clean-cache/hermetic/provenance claim. Service formatting, locked all-target warning-denied Clippy/tests, installer quality, Xvfb GTK, real compiled Rust↔Dart cloud fixtures, Flutter format/analyzer/Linux build and interpreter-free Ubuntu CLI checks passed. Controlled ignores are subprocess/fixture entrypoints, not newly disabled tests. The configured service gate uses Rust1.98 **Bookworm**; Ubuntu24.04 installer/GTK/Flutter/package gates are distinct. The34 helper cases ran after build to consume the real package, not in hosted job order.

All1,230 core inputs stayed byte/mode-stable through the gates. Exported `installer/dist` verified95 manifest payload entries/97 regular files. Paired service helpers match. The output is unchanged from the prior generated package: an initial comparison's differing path prefixes falsely reported inequality; normalized comparison matches all117 file/directory entries. Original evidence is preserved, not rewritten. No single overall original-session deadline is established retrospectively; finite make1200s/package60s/helper180s budgets and successful exits are recorded.

## Binding and retention

- [results.json](results.json): compact commands, source/artifact identities, counts, measurements, raw-log SHA256s, preserved failures and limitations.
- [core-source-manifest.jsonl](core-source-manifest.jsonl): all1,230 core path/hash/mode/size records; canonical array SHA256 `2e054a19a5879d3d70ed72927015277916f2c6cad8bfd124e59357a94757a301`.
- [workflow-helper-controls.jsonl](workflow-helper-controls.jsonl): historical supplementary gate-input snapshot. Its plan row binds the pre-closeout plan, not the final documentation; other policy/workflow/helper controls are unchanged. [REVIEW](REVIEW.md) binds the current plan and final status-only closure without rewriting this historical record.
- Candidate test-file SHA256 `bd51d10ff5bc2a6657df7b06aa5d85d8dcaf445a76ec7ac2b36fa095ae565888`.
- Sanitized raw logs, scripts, snapshots, sample-level records and binaries remain ignored under `target/keyring-fixture-fix/{measurement,private-session,ci-equivalent}-20261016-becc14e/`. Retention self-check verified183 private-session and22 pure-measurement hash-index entries, CI logs/evidence and current source/package bytes. Ignored evidence is local-only, not a Git backup or publicly retained log.

Fresh independent assurance recommends the bounded K1–K5 source repair with no code blockers and a pass-with-risk verdict limited to finite evidence; it grants no accepted-risk or external authority. Its separate20-test keyring execution passed, without adding repeated coverage to the aggregate. The final report rebinds the status-only documentation closure.

The optimization still has a finite variable-length synchronous search; these samples do not prove worst-case timing, hosted scheduling or the historical timeout stage. No real user bus/keyring, vendor/API credentials, model/GPU inference, host GUI/user manager or installation was used. No commit, push, hosted rerun, merge, release or deployment is authorized by these local results. Human approval remains required for the next external effect.
