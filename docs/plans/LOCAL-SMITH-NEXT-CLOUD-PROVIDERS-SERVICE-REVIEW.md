# Cloud providers — independent service-security review

## Scope and authority

- **Independent assurance/adversarial review, 2026-10-15; remediation verification completed at approximately 21:44 UTC.** Review only of service security, not installer/client UX, packaging, deployment or global feature acceptance.
- Base/HEAD: `7f6a483ae82c4bcf647268f035f1e7cc80409d4b`; branch: `feature/autonomy`. Target is the current uncommitted service source identified below, including untracked files. The remediation intentionally supersedes the original reviewed snapshot.
- Read `AGENTS.md`, `docs/SCOPE.md`, applicable architecture/process/design/host policy, `roles/assurance.md`, the approved [provider plan](LOCAL-SMITH-NEXT-CLOUD-PROVIDERS.md) and [API contract](LOCAL-SMITH-NEXT-CLOUD-API-CONTRACT.md). The latter now specifies the actual single-connection Hyper/Rustls transport rather than the original Reqwest implementation.
- This record is the reviewer's **only repository write**. No production code edits, full compiler suite, real session bus/keyring, official vendor API, installed state, service-manager action, commit or publication. Focused existing test binaries used private synthetic fixtures. Concurrent queue/scaffold/timestamp work was preserved.

**Current assessment (final evidence closure, 2026-10-15T22:51:08Z): scoped service-source assurance PASS.** The three original service concerns and test-only deadline correction are closed, and the final stable source/build/package evidence is verified below. Earlier pending/FAIL decisions are retained as historical and explicitly superseded for this source scope. This is not human release, deployment or live-host approval.

## Original review retained

The original review returned **FAIL / NO-GO**, with two medium correctness/security findings and a blocking credential-protocol evidence gap. It inspected the original `next/service/build/cloud-providers-validation.log`: 219 passing tests, zero reported failures, two top-level ignored subprocess fixtures, and successful compiler stage endings. It did not establish that log's binding to the concurrent worktree.

Original log SHA-256: `15c64890ce53f9f8dd852fcd7b3f3bae4e8ffbbdf34bf7279087e54f19c984e0`.

Original scoped source digest: `d0ee73fbe96a403dcf34ea7e10f75cd388b17b77c8378561ccacad361fac51a6`. Construction was SHA-256 over project-relative path, NUL, file bytes, NUL, in this order: `next/service/src/app.rs`, `app_v2.rs`, `app_v3.rs`, `app_v3_worker.rs`, `app_transport.rs`, `cloud.rs`, `cloud_keyring.rs`, `cloud_http.rs`, then `next/service/Cargo.toml` and `Cargo.lock` (all abbreviated source names share the first source directory). This is historical identity, not the remediated identity.

### F1 — medium: stale interrupted validation overwrote newer results — CLOSED

**Original implemented defect:** the old `app_v3.rs:568–575` scanned all retained interrupted validation receipts on every open. An older interrupted A could overwrite a later successful/failed B for the same credential generation and receive a new completion timestamp. `app.rs` invoked reconciliation even when no pending operation had just been interrupted, allowing an unpersisted in-memory metadata change.

**Original reproduction:** interrupt validation A by restart; explicitly complete validation B for the same generation; restart with both receipts retained. The old implementation selected A rather than retaining B. Repeated restart also restamped A.

**Remediated implemented behavior:** `next/service/src/app.rs:564–581` reconciles inside the pending-job interruption branch, in the same recovery transaction that increments revision and persists. `next/service/src/app_v3.rs:568–578` receives that single newly interrupted job and still requires the exact credential reference. Historical terminal receipts are no longer rescanned.

**Evidence:** `next/service/tests/cloud.rs:794–870` constructs actual interrupted validation and verifies three repeated opens preserve validation, completion time and durable bytes, with no additional keyring reads or HTTP calls. Separate regressions cover newer success and newer failure. All three passed in the retained final suite and the reviewer's direct focused execution.

### F2 — medium: header limit enforced only after buffering/parsing — CLOSED

**Original implemented defect:** old `cloud_http.rs:131–143` awaited a complete Reqwest response before measuring normalized headers. The specified 16-KiB raw ingress bound did not constrain incomplete headers or whitespace removed by parsing; Hyper's larger default buffer remained applicable.

**Original reproduction:** supply an unterminated header exceeding 16 KiB and stall; the Smith check did not execute at the boundary. Oversized whitespace-heavy raw headers could also be underestimated after normalization.

**Remediated implemented behavior:** `next/service/src/cloud_http.rs:150–238` uses a single TCP/TLS connection and Hyper HTTP/1 send, with normal Rustls certificate/hostname verification, no high-level retry/pool/proxy/redirect/decompression layer. Both parser limits are set to `HEADER_LIMIT`. `HeaderBounded` at lines 256–359 bounds each plaintext read to the remaining allowance, counts raw bytes before normalization, and shares the allowance across informational and final response heads. It rejects an incomplete head when the allowance is exhausted and rejects protocol switching. The existing absolute operation deadline remains; connect/TLS is additionally clipped to the original dispatch deadline, followed by another permission/cancellation check before send.

**Evidence:** `next/service/src/cloud_http_tests.rs:230–295` covers limit−1/limit/limit+1 with whitespace/status lines, an incomplete slow header rejected before delimiter/EOF, and aggregate 103-plus-final-head limits. The private TLS suite also retains certificate/hostname rejection, proxy-environment isolation, redirects, body/encoding bounds, slow drip/disconnect and cancellation. Reviewer execution: 11 passed, one helper ignored at top level (explicitly invoked by the passing proxy parent test).

**Boundary:** this is a response-head cap at the plaintext HTTP read/parser boundary, not a promise that TLS/TCP internal encrypted buffering never exceeds 16 KiB. Body bytes remain incrementally limited to 1 MiB. No OS egress sandbox is claimed.

### F3 — blocking evidence gap: encrypted session negotiation bypassed — CLOSED for synthetic protocol evidence

**Original gap:** old `cloud_keyring_tests.rs:138–165` constructed a `Session` with a fixed AES key instead of exercising production authentication, owner lookup and `OpenSession`. The original review did not assert a confirmed cryptographic defect; it could not accept item-exchange tests as negotiation evidence.

**Remediated implemented behavior:** `next/service/src/cloud_keyring.rs:71–174` retains the fixed same-user Unix socket checks, factors the authenticated connected-stream path into `Session::open_stream`, checks peer UID, forces EXTERNAL, checks the unique owner and its UID, negotiates the encrypted session, validates the peer public value, derives the padded shared-secret HKDF key, and rechecks owner/UID after negotiation. Later calls remain pinned and use `NoAutoStart`; no plaintext, anonymous/cookie authentication, activation, unlock or Prompt fallback was added.

**Evidence:** `next/service/src/cloud_keyring_tests.rs:120–203,368–395,528–725` supplies a private bus-protocol peer and calls the same `open_stream` path. The peer independently computes DH and HKDF using its own HMAC construction, deliberately exercises a leading-zero shared secret, then decrypts/reads the synthetic item. Negative cases cover malformed/rejected public values/session paths, absent or malformed owner names, wrong/changing UID/owner, wrong socket peer UID, rejected EXTERNAL authentication and a stalled OpenSession under the original budget. Assertions prohibit fallback/item access after failed negotiation. All 12 keyring tests passed in the final retained suite and reviewer execution.

**Remaining boundary:** fixtures inject an already connected private socket. They do not exercise discovery of the actual normal-session socket or real GNOME Keyring integration. Those remain separately authorized live-host gates, not missing synthetic-negotiation coverage.

## Additional scoped checks and concurrent changes

- The secret-save boundary remains key-free on disk: `app_v3.rs:1196–1339` removes `api_key` before public fingerprinting, resolves retained bindings without comparing replacement key bytes, consumes preparation with durable admission and retains the secret only in the volatile worker slot. `app_v3_worker.rs:249–281,375–390` takes that slot once and activates the new reference only on successful publication. No new retry/orphan activation path was found.
- Immutable routing, exact generation checks, ownership/history fences, marker-before-send and post-marker permission checks remain at `app_v3.rs:1437–1569` and `app_v3_worker.rs:28–191`. Fixed vendor endpoints and test-only endpoint/trust injection remain separate. Final-only parsing and duplicate-key rejection remain in `cloud_http.rs`; 10 parser/request tests passed in reviewer execution.
- Current disk schema is **4**, not the original cloud plan's disk3: the concurrent [timestamp plan](LOCAL-SMITH-NEXT-MESSAGE-TIMESTAMPS.md:14) explicitly records disk4 with strict disk1/2/3 migration and unchanged application schema 3. Inspected the migration seam at `app_v2.rs:190–310`, bounded sidecar in `app_timestamps.rs`, and routed completion integration. Timestamp data is separate from provider `Message`/context hashes; this is not a complete timestamp-feature assurance review.
- The remediation log preserves an initial failed run: `save_is_local_validation_is_exact_synthetic_and_no_secret_is_durable` expected schema 3 but observed 4 (`cloud-providers-review-remediation.log:277–293`). Current `tests/cloud.rs:407–410` expects 4 while retaining secret/reasoning/token exclusion assertions. The subsequent final source-bound suite passes. This earlier failure is superseded by changed-source evidence, not silently treated as a passing run.
- Informational documentation follow-up identified at the initial remediation review: the cloud-validation section described the older socket-pair fixtures as not proving encrypted-session negotiation. **Resolved in the 22:19 UTC follow-up below:** `docs/PROCESS.md:112` now distinguishes shared production-path protocol fixtures from still-unverified real GNOME Keyring/socket discovery.

## Required Checks Evidence

Retained log: `next/service/build/cloud-providers-review-remediation.log`, SHA-256 `7430032f0eb73ae6256e0c12665eb41c39eab4b43de3598b3d817e702dead365`. Its **final** source-bound section begins at line 608, dated `2026-10-15T21:38:48.765835+00:00`. Earlier sections contain different manifests; do not use the first `Final source digests` entry as the final binding.

| Check | Command/scenario | Expected | Actual / status | Evidence |
|---|---|---|---|---|
| Base/branch | `git --no-pager rev-parse HEAD`; `git --no-pager branch --show-current` | Approved base and branch | Match; exit 0 | Reviewer tool output |
| Current source binding | Hash all 33 paths from the last final manifest | No missing/changed files | `CURRENT_HASH_MISMATCHES []`; exit 0 | Reviewer Python verification; manifest below for scoped inputs |
| Format | `cargo fmt --all --check` | Exit 0 | Exit 0 | Retained final log lines 611–613; not rerun by reviewer |
| Warning-denied lint | `cargo clippy --locked --all-targets -- -D warnings` | Exit 0 | Exit 0 | Retained final log lines 637–640; not rerun by reviewer |
| Component tests | `cargo test --locked --all-targets --no-fail-fast` | All executed tests pass | Exit 0; 246 passed, 0 failed, 3 top-level helpers ignored | Retained final log lines 642–943 |
| Release build | `cargo build --release --locked` | Exit 0 | Exit 0 | Retained final log lines 945–948; not rerun by reviewer |
| Whitespace | `git --no-pager diff --check -- next/service` | No errors | Exit 0 | Reviewer execution from repository root |
| HTTP remediation | Existing unit binary, `cloud::http::tests --test-threads=1` | Limit/TLS/no-retry fixtures pass | Exit 0; 11 passed, 1 helper ignored | Reviewer execution; 1.41 s |
| Keyring remediation | Existing unit binary, `cloud::keyring::tests --test-threads=1` | Negotiation and negative fixtures pass | Exit 0; 12 passed | Reviewer execution; 0.67 s |
| Recovery remediation | Existing cloud binary, `interrupted_validation --test-threads=1` | All three regressions pass | Exit 0; 3 passed | Reviewer execution; 0.27 s |
| Final-only parser | Existing cloud_protocol binary, `--test-threads=1` | Exact requests/parser negatives pass | Exit 0; 10 passed | Reviewer execution; 0.02 s |
| Final aggregate source/package/cross-language gates | Parent's combined runner and exact source/artifact manifest | Complete passing evidence matching reviewed inputs | **Pending / not assessed here** | No global PASS inferred from component results |

The three component-suite top-level ignored entries are the proxy subprocess helper, captured-manager subprocess helper and explicitly launched Dart cloud fixture. Their names do not establish that the external Dart gate passed; the combined owner must supply that evidence.

### Exact reviewer validation commands

Working directory `smith/next/service`, one bounded command (30-second tool timeout), no compiler invocation:

```sh
target/debug/deps/smith_service-7c11b7ac37ef8548 cloud::http::tests --test-threads=1 && target/debug/deps/smith_service-7c11b7ac37ef8548 cloud::keyring::tests --test-threads=1 && target/debug/deps/cloud-5262b1b2d0cad470 interrupted_validation --test-threads=1 && target/debug/deps/cloud_protocol-d2d8df0c7e4b49c7 --test-threads=1
```

Working directory `smith`, verification commands:

```sh
git --no-pager rev-parse HEAD
git --no-pager branch --show-current
git --no-pager diff --check -- next/service
git --no-pager diff --stat 7f6a483ae82c4bcf647268f035f1e7cc80409d4b -- next/service
git --no-pager diff 7f6a483ae82c4bcf647268f035f1e7cc80409d4b -- next/service/src/app.rs
sha256sum next/service/target/debug/deps/smith_service-7c11b7ac37ef8548 next/service/target/debug/deps/cloud-5262b1b2d0cad470 next/service/target/debug/deps/cloud_protocol-d2d8df0c7e4b49c7
date -u '+%Y-%m-%dT%H:%M:%SZ'
```

The final-manifest verification was run with this exact read-only command (an earlier exploratory command selected the first manifest; the final verification deliberately selects the last):

```sh
python3 -c 'import pathlib,json,hashlib; r=pathlib.Path("next/service"); p=r/"build/cloud-providers-review-remediation.log"; t=p.read_text(); lines=t.splitlines(); d=json.loads([l.removeprefix("Final source digests: ") for l in lines if l.startswith("Final source digests: ")][-1]); print("CURRENT_HASH_MISMATCHES",[k for k,v in d.items() if not (r/k).is_file() or hashlib.sha256((r/k).read_bytes()).hexdigest()!=v]); print("LOG_SHA256",hashlib.sha256(p.read_bytes()).hexdigest()); print("LOGGED_FILE_COUNT",len(d)); print("\n".join(l for l in lines[642:] if "Running " in l)); print("SCOPED_HASHES"); print("\n".join(k+" "+v for k,v in d.items() if k.startswith(("src/app", "src/cloud", "tests/cloud")) or k in ("Cargo.toml","Cargo.lock","src/lib.rs","src/transport.rs")))'
```

Read-only `cat`, targeted `sed` and `rg -n` calls inspected the policy, contracts, production paths, fixture implementations and log sections cited above; they are inspection, not test executions. The editor directory-list tool verified `smith/docs/plans` before creating this record.

### Executed binary identities

Paths relative to `next/service/`; SHA-256 sampled after focused execution:

```text
target/debug/deps/smith_service-7c11b7ac37ef8548  81b784c9b73dac9f0dfd01a1f41bd2b33430b14415dc86b6ff3fb7e7a60a7db6
target/debug/deps/cloud-5262b1b2d0cad470          fa37ba3d5aecd08446822d1d05d5acb0d9c84a3c29d30db9b5d350f4354804e9
target/debug/deps/cloud_protocol-d2d8df0c7e4b49c7 e58fc25d654942b4bf0d68a14683491feaec230df8d46e49528c83a7203191bf
```

These are existing test artifacts, not a reviewer clean rebuild or signed provenance. Source binding also relies on the retained final compiler/test log and independently matched current-source manifest.

## Reviewed service input identities

SHA-256; paths relative to `next/service/`. These hashes match the last final manifest. Timestamp inputs are recorded because the shared persistence/publication paths now include them; listing them does not extend this review into a global timestamp-feature approval.

```text
Cargo.lock                    28c0a55661c6d63024e484307075fde42a600ff863c132b596816b102316692e
Cargo.toml                    8f423111abe8c293b6eb997085802e7810415074c66fa1b0eaebb7d310bdda9a
src/app.rs                    54d26429433ee8440d686053fd4672e977934ab424230d5c96ae0c6f5876c3a6
src/app_timestamps.rs         e4386e813887345c5aade50a264351c99d2018fdc633030027ffbd8e608ffc49
src/app_timestamps_tests.rs   b17659925d290d0f89b2bbc657031718a5de62257d66416390a8c5677f0ff572
src/app_transport.rs          797c258df171d2caada66edf859c3cf8ffcccfb0c7268e7d065497fcfd798324
src/app_v2.rs                 094845f730f583c9d8d190e7fea6dcd36565b7561dc380d8a45d0c4fdf998be6
src/app_v2_tests.rs           ce63a255238e57799af5bd3b26c412c717f4aa542ebaec8527c44198c26f34c6
src/app_v3.rs                 f6fd85dc9bc0413101d11b2a961919d02987c9862334cf37c73757f38770f9e2
src/app_v3_tests.rs           3f01841c23845c9b772d14dd956db4a30673092e03beb9bf7ff7ef3d73aaf24c
src/app_v3_worker.rs          c09100f0c4729834dfc640367711de6e71d315966290de365cae438c4d6af0b6
src/cloud.rs                  1c3de134d5f5c81f9684653685a39fff2e3fe845558ede9e5fd14476ebe76110
src/cloud_http.rs             bd672c1ccccd3181ae4dc29f00abd3238b83f861f8c5d027f34a543a83ae0fd0
src/cloud_http_tests.rs       79ac80e23e3c086d909cdf17e9952eb87a4c396f18ac146d7e45fe17ec36772d
src/cloud_keyring.rs          bf14f1524781323efa2f327d15b84a603a361bc0d77510ee3404fa2fb67985d4
src/cloud_keyring_tests.rs    9da515506dcc079a4b847ff2fcbc40e2a225e41770f991eff8cde875ffdd0973
src/lib.rs                    8c23116addafc46110e860c39baf4ebbbd04966ee58a7a24e45b84eb0f73bfcd
src/transport.rs              8626ad11aca4b5472389e8f89856f73e71752598e1716a4dcdef74c6f67749b5
tests/cloud.rs                642282645edbd576acacc9311e2d69b522f137916e453e497e9339f7e72e9326
tests/cloud_protocol.rs       d18bd79ba79d66b525df54512ee7bcef04f8283455a008c3b4a5accf79656f4b
```

## Focused source-binding follow-up — 2026-10-15T22:19:35Z

**Scope:** one bounded read-only comparison against this review's own manifest, plus inspection of the corrected documentation. No compiler/test execution, production edit, normal bus/API access, or polling/waiting for the final runner. Only this review record was updated. Base/branch remain `7f6a483ae82c4bcf647268f035f1e7cc80409d4b` / `feature/autonomy`.

**Actual result:** all **20/20 recorded input hashes are unchanged**, with zero missing files and zero mismatches. This includes `app.rs`, `app_v2.rs`, `app_v3.rs`, `app_v3_worker.rs`, both transports, `cloud.rs`, HTTP/keyring implementations and tests, cloud integration/parser tests, Cargo files and the recorded timestamp sidecar inputs. There are **no cloud/recovery/consent source deltas** against the last reviewed service manifest to inspect. The existing F1/F2/F3 closures remain bound to those exact bytes; unchanged hashes are not a new test run.

**Disk4 integration status:** the previously inspected disk4 migration/publication/capacity seams are also byte-identical (`app_v2.rs`, `app_timestamps.rs`, `app.rs`, `app_v3_worker.rs`). Consequently the prior scoped assessment remains applicable: timing metadata is a bounded sidecar rather than provider message/context content; migration preserves cloud references and accepted work; restart interrupts rather than replays; successful history/timing publication is atomic and reserved before dispatch. This follow-up does not broaden assurance to unrelated timestamp functionality or claim installed migration validation. `docs/PROCESS.md:108–110,205` now explicitly identifies application schema3 and disk1/2/3 → disk4 compatibility requirements.

**Documentation follow-up resolved:** inspected `docs/PROCESS.md:112`; it now describes EXTERNAL authentication, independent peer derivation/leading-zero padding, malformed/owner/UID/auth/stall fixtures and the remaining installed-login-bus/real-GNOME-Keyring limitation accurately. The Hyper boundary remains documented in `LOCAL-SMITH-NEXT-CLOUD-API-CONTRACT.md:31` and `LOCAL-SMITH-NEXT-CLOUD-PROVIDERS.md:57,305`.

Exact commands executed from repository root for this follow-up (all exited 0):

```sh
python3 -c 'import pathlib,re,hashlib; p=pathlib.Path("docs/plans/LOCAL-SMITH-NEXT-CLOUD-PROVIDERS-SERVICE-REVIEW.md"); t=p.read_text(); section=t.split("## Reviewed service input identities",1)[1].split("## Evidence Gaps",1)[0]; rows=re.findall(r"^([\w./-]+)\s+([0-9a-f]{64})$",section,re.M); print("REVIEWED_INPUT_COUNT",len(rows)); [(print(("UNCHANGED" if (pathlib.Path("next/service")/n).is_file() and hashlib.sha256((pathlib.Path("next/service")/n).read_bytes()).hexdigest()==h else "CHANGED"),n,hashlib.sha256((pathlib.Path("next/service")/n).read_bytes()).hexdigest() if (pathlib.Path("next/service")/n).is_file() else "MISSING")) for n,h in rows]'
rg -n 'negotiation|open_stream|GNOME|Hyper|disk4|schema 4' docs/PROCESS.md docs/plans/LOCAL-SMITH-NEXT-CLOUD-API-CONTRACT.md docs/plans/LOCAL-SMITH-NEXT-CLOUD-PROVIDERS.md
git --no-pager rev-parse HEAD
git --no-pager branch --show-current
date -u '+%Y-%m-%dT%H:%M:%SZ'
```

**No new source blocker identified.** The parent-announced run at `next/service/build/cloud-final-integrated-validation-20261015` was not polled or treated as completed evidence. Final evidence-link/source/artifact closure is explicitly deferred to the requested later follow-up; the aggregate acceptance and release decision below remain unchanged.

## Test-only deadline correction review — 2026-10-15, after loaded validation

**Decision: GO to execute the next final source gate.** No blocker in this test-only correction was identified. This is permission-to-proceed advice for the already authorized isolated validation, **not** a global source PASS, release approval or waiver of the failed aggregate. Only this review document was edited by the reviewer.

### Source identity and retained failure

Compared all 20 inputs in this review's original manifest and all 33 inputs in the previous service evidence manifest. **Only `src/cloud_http_tests.rs` changed.** All 32 other service inputs, including production `cloud_http.rs`, `cloud.rs`, worker/recovery/consent code, dependencies and disk4 integration, are byte-identical. Independently matched all 33 current files to `evidence.json`'s `source_after`; `source_before` and `source_after` are identical. Here `source_before` means before loaded validation, not before the test correction.

The current test-file SHA-256 is **`4c52b289494e952eb288be4e0e1aff44cde4a9cab2af9f9f6ea0ab136b9b17db`**, superseding only its earlier `79ac80e23e3c086d909cdf17e9952eb87a4c396f18ac146d7e45fe17ec36772d` entry. The earlier manifest and follow-up are retained as historical observations.

Read the original failed aggregate at `next/service/build/cloud-final-integrated-validation-20261015/combined.log:6180–6190`: `slow_drip_and_disconnect_are_bounded_never_retried` failed at old line 309, `assertion failed: !e.not_sent`. Its SHA-256 independently remains `fe6754d98848ed51e593179a8bf28151c85655a9de6487bcec5ad5cb61624ccc`, matching the remediation evidence. It remains a **failed aggregate**, not retrospectively a passing run.

**Root-cause qualification:** the old 100-ms end-to-end test assumed it had reached HTTP transmission without a stage barrier. The assertion failure proves that the adapter reported `not_sent=true`; inspection of the unchanged adapter associates that with not reaching its attempted-send marker. Together these support the test's unproven post-send assumption as the immediate defect. The original failure log does **not** identify whether runtime setup, root-store work, TCP/TLS or scheduler delay consumed the budget; CPU contention is a plausible contributor, not independently proven causation. This review does not claim production timed out late or misclassified an actual transmitted request.

### Inspected invariants — no production deadline weakening

- `cloud_http_tests.rs:9–15,170–196` defines a three-second **test** budget, separate two-second setup observation envelope and one-second assertion scheduling allowance. `request_until` passes the same absolute `Instant` as operation and dispatch deadline. Neither notification nor response progress refreshes it. Production deadlines and transport code are unchanged.
- `cloud_http_tests.rs:94–109,358–399` gates the TLS handshake after actual TCP accept. The caller observes accept within two seconds and keeps the gate closed until the original deadline returns. It then asserts zero captured HTTP, zero response bytes, `not_sent=true`, no queued second connection, and completion in `[deadline, deadline + 1 second)`. The broad `REMOTE_OUTCOME_UNKNOWN` error code is checked separately from the truthful `not_sent` flag; no claim of HTTP delivery is inferred from the code alone.
- `cloud_http_tests.rs:125–151,401–454` notifies only after capturing the complete request, requires that observation within two seconds, and checks an actual POST. It asserts `REMOTE_OUTCOME_UNKNOWN`, `not_sent=false`, one connection count, response progress (at least 96 bytes) without completion, and the same absolute deadline interval. The counter alone records TLS completion, not an independent POST parser count; captured request plus the single-send production path provides the HTTP evidence.
- At 32-byte chunks separated by 20 ms, the complete framed response requires at least **6.26 seconds** after response start. The code computes this minimum and asserts it exceeds the three-second budget plus one-second scheduling allowance (`409–410`). Thus the allowance cannot make a full slow-body drain pass; it is an observation tolerance, not a deadline given to production. Timing assertions are not an exhaustive proof against every hypothetical sub-second timer regression; source inspection establishes that this change introduced no sliding budget.
- The truncated-body/disconnect case (`455–472`) now checks the failure category, `not_sent=false`, captured POST, one count and no queued second connection. `Server::assert_no_second_connection` (`198–203`) checks the retained listener for queued retries after completion. The unchanged production single-connection/no-retry implementation supports this observation; the fixture is not an OS-level egress proof.
- Header ingress/1xx/whitespace limits, certificate/hostname verification, no proxy/redirect, cancellation and body bounds remain covered by the same HTTP suite. No assertion accepting fallback, replay, a full slow response or expired-authority send was added.

### Evidence verified and reviewer execution

Evidence directory: `next/service/build/cloud-http-deadline-remediation-20261015/`.

| Check | Expected | Actual / status | Source |
|---|---|---|---|
| Six numbered HTTP suite runs | Each passes, truthful pre/post-send observations | Each exit 0; 12 passed, 0 failed, 1 helper ignored | `loaded-validation.log:4–153`; numbered runs 1–6 |
| Load configuration | Explicit bounded synthetic load | Two workers, shared CPU affinity `[0,1]`, four test threads recorded; worker exit codes 0 and completed hash counts retained | Log header and `evidence.json`; reviewer did not independently recreate the load |
| Phase observations | Pre-send zero HTTP; post-send actual progress; original deadline | Recorded elapsed 3000–3001 ms; captured post-send requests within 1–5 ms; incomplete response progress | Actual log observations, not solely JSON verdict |
| Full service suite | All executed tests pass | Exit 0; 247 passed, 0 failed, 3 helpers ignored | `loaded-validation.log:166–468`; sum of eight suite result lines |
| Format / strict Clippy / release build | Exit 0 | Each exit 0 | Log lines 155–164,470–473; not rerun by reviewer |
| Source binding | No unexplained production changes or validation-time drift | 33 current matches; no before/after differences; only test file differs from previous 33-file manifest | Independent Python hash comparison |
| Reviewer focused execution | Revised actual HTTP fixtures pass | Exit 0; 12 passed, 0 failed, 1 helper ignored; 3.06 s | Existing unit binary, command below |
| Reviewer whitespace check | Exit 0 | Exit 0 | `git --no-pager diff --check -- next/service` |

Reviewer-observed phase output: `pre-send expiry: accepted_ms=0 elapsed_ms=3001 http_calls=0 not_sent=true`; `post-send expiry: captured_ms=1 elapsed_ms=3001 response_bytes=4576 http_calls=1 not_sent=false`. The ignored proxy helper is invoked explicitly by its passing parent test. These focused results do not replace the combined runner.

Evidence SHA-256 values independently computed:

```text
loaded-validation.log  f9c3098b83af7554a1bec80c49b3751b7f9727b15d4d383865f4934aceef397a
evidence.json          62eba3f510ae0155914b1c2eec63c407f49ccbb9feff0e96f695bbfd90084f0b
```

Exact focused command, working directory `smith/next/service`, 15-second tool timeout; **no compiler invocation**:

```sh
target/debug/deps/smith_service-7c11b7ac37ef8548 cloud::http::tests --test-threads=4 --nocapture
```

The executed binary's SHA-256 sampled afterward was `57ecf184e7d142ccaec64496544ea5d5f9ba6d4fa65582210c7ed1de50e31900`; it supersedes the earlier unit-test artifact hash, not the production source hashes.

Exact evidence/source comparison command, working directory `smith`:

```sh
python3 -c 'import pathlib,json,hashlib; r=pathlib.Path("next/service"); e=json.loads((r/"build/cloud-http-deadline-remediation-20261015/evidence.json").read_text()); print("SOURCE_COUNT",len(e["source_after"])); print("CURRENT_MISMATCHES",[n for n,h in e["source_after"].items() if hashlib.sha256((r/n).read_bytes()).hexdigest()!=h]); print("BEFORE_AFTER_DIFFERENCES",[n for n,h in e["source_before"].items() if e["source_after"].get(n)!=h]); old=(r/"build/cloud-providers-review-remediation.log").read_text(); d=json.loads([l.removeprefix("Final source digests: ") for l in old.splitlines() if l.startswith("Final source digests: ")][-1]); print("CHANGES_FROM_PREVIOUS_33",[n for n,h in d.items() if e["source_after"].get(n)!=h]); [(print(str(p),hashlib.sha256(p.read_bytes()).hexdigest())) for p in [r/"build/cloud-final-integrated-validation-20261015/combined.log",r/"build/cloud-http-deadline-remediation-20261015/loaded-validation.log",r/"build/cloud-http-deadline-remediation-20261015/evidence.json"]]'
```

Read-only `cat`, `nl -ba`/`sed` and `rg -n` inspected the revised test, evidence JSON, numbered suite outcomes and original assertion failure. No normal bus/vendor API, production edits or broad tests were performed by this reviewer.

**Remaining action:** execute and retain a new stable combined gate with this exact test hash and unchanged production inputs, then request aggregate evidence closure. No new service-code remediation is requested. The old failed combined run and the pending new aggregate keep the overall acceptance/release decision below at NO-GO; this does not block launching the authorized final source gate.

## Evidence Gaps

1. **Historical gap, now superseded by final evidence closure below:** combined service/installer/client builds/tests, Rust–Dart fixture execution, package checks and source/artifact binding were pending. The parallel runner was intentionally not duplicated or presumed successful. The subsequent retained final gate has now been inspected and bound to the reviewed service inputs.
2. No real GNOME Keyring/session discovery, vendor account/model/billing, installed migration/restore, normal-host lifecycle or deployment evidence. These require separate approval and are not claimed by synthetic tests.
3. This is scoped static/source assurance plus focused synthetic execution, not a new full review of concurrent client, installer, timestamp UI or scaffold changes. Changed service hashes after this record require delta review and renewed evidence binding.

## Findings by severity and verdict

- **Critical/high:** no new concrete scoped implementation finding identified.
- **Medium:** original F1 and F2 closed by source inspection and passing targeted regressions.
- **Evidence blocker F3:** closed for shared production-path synthetic negotiation; real-host validation remains separate.
- **Informational:** the stale fixture description in `docs/PROCESS.md` was corrected by its owner and independently verified resolved in the 22:19 UTC follow-up.

**Scoped remediation conclusion:** all three original concerns are resolved at the recorded service input identities. No additional production-code remediation is requested by this re-review.

**Historical acceptance verdict before final evidence closure: FAIL / pending required aggregate evidence**, not a claim that the remediated component tests failed. Per assurance policy, missing required final verification could not be called a global pass or pass-with-risk. This pending-source decision is superseded only by the scoped final closure below.

**Historical Release Gate Decision: NO-GO.** No release, deployment, live access or accepted-risk approval was granted. The then-required next action was final aggregate log/source/artifact binding and follow-up review; that source-evidence action is now completed below, without changing the external approval boundary.

## Final evidence closure — 2026-10-15T22:51:08Z

### Scope and disposition

**Verdict: PASS for scoped service-source assurance against the final isolated source/build/package snapshot.** No remaining service-security source blocker was identified. F1 (validation recovery), F2 (raw-header ingress), F3 (encrypted-session protocol evidence), the documentation clarification and the test-only deadline correction were reviewed before this gate; their production/input identities are unchanged. This section supersedes the earlier pending-aggregate paragraphs and source NO-GO recommendations, **not** the preserved historical failures or live/release limitations.

The reviewer made no production edits and reran no build/test suite. Only this review was updated. The final gate was executed by the parent runner; this closure independently inspected its retained commands, statuses, raw logs, source inventories and actual artifact bytes.

### Evidence binding verified

Canonical retained evidence: [index](../evidence/cloud-providers/README.md), [final command record](../evidence/cloud-providers/final.json), [tested source manifest](../evidence/cloud-providers/source-manifest.json), [source SHA list](../evidence/cloud-providers/source-manifest.sha256), [artifact inventory](../evidence/cloud-providers/artifacts.json).

Base/HEAD and branch remain `7f6a483ae82c4bcf647268f035f1e7cc80409d4b` / `feature/autonomy` plus the inventoried uncommitted worktree.

- Independently reconstructed the runner's inventory from current files, including paths, bytes, modes, kinds, exclusions and added/removed paths: **1,267 entries, exact equality** with the retained manifest. Recomputed compact sorted-JSON inventory digest: **`a4c131f90932cb7455acfd842373e93dd89d78e67b1d4b5e62efc1cb47da95db`**.
- Compared the four actual `combined-before`, `combined-after`, `package-verify-before`, `package-verify-after` entry arrays: all equal that manifest. Verified the retained SHA-list text is generated from precisely those entries. This is before/after equality, not continuous monitoring or a signed adversarial integrity anchor.
- The manifest includes **51 service entries**. All current inventoried service bytes match. All **33 service source/test/Cargo inputs** from the last remediation evidence match the final inventory, including `cloud_http_tests.rs` SHA-256 `4c52b289494e952eb288be4e0e1aff44cde4a9cab2af9f9f6ea0ab136b9b17db`. Cloud transport, parser, keyring, save/consent, recovery, worker and disk4 integration have no further source delta.
- Verified byte-for-byte retained copies: `final.json` equals the final runner's `evidence.json`; `source-manifest.json`/`.sha256` equal `combined-after.json`/`.sha256`; retained `artifacts.json` equals the runner's copy.
- **Inventory-scope clarification:** the runner inventories `next/service`, `next/installer`, `next/client`, `next/dagger`, plus `next/Makefile`, `next/dagger.toml`, and `next/dagger.lock`, excluding the explicitly listed generated/cache names. `next/VALIDATION.md` is **not an inventoried entry**. Its later narrative addition therefore is not a current manifest mismatch and is not claimed tested. Canonical docs/reviews/evidence outside this scope also do not retroactively alter the tested inventory. All 1,267 actually inventoried entries currently match; there is no hidden allowance for a production difference.

Metadata/log SHA-256 values independently verified:

```text
docs/evidence/cloud-providers/final.json
  eadee426167a05ce4cf8f074b2066ffe868b2cbc112437bf893c14006696e5e6
docs/evidence/cloud-providers/source-manifest.json
  34b392844875115df3814f20545bb5b9a30acd8a49424f03c2c42c27cc3dba51
docs/evidence/cloud-providers/source-manifest.sha256
  7dcd1c87d82dd76d8c4450e5f46339f25c7224b31019561df50dad61c8d0aac5
docs/evidence/cloud-providers/artifacts.json
  147bf5d8104c604a59a810bf1550073ccd155268b8a9daf3ad0e37647637af64
next/service/build/cloud-reviewed-final-validation-20261015/combined.log
  1198c41199858154d3b64f55775b5f77622b8e8914d5eaa71a037d9b57bfd4b7
next/service/build/cloud-reviewed-final-validation-20261015/package-verify.log
  4f6e997052bf8ed0fe4e97c0e068bea00d34f1891dc7ee807e21581cdbb6da2e
```

### Required Checks Evidence — final gate

Working directory for both parent commands: `next/`.

| Check / exact command | Expected | Verified actual status | Evidence |
|---|---|---|---|
| `env DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make test build` | Successful complete source/build gates on unchanged inputs | Exit 0, `killed:false`, 480.725 s; 22:38:27–22:46:27 UTC | Retained final record, original `combined-status.json`, hash-matched raw combined log |
| `python3 dagger/package.py --root installer/dist --verify` | Paired package accepted | Exit 0, `killed:false`, 0.407 s; 95 files verified | Final record, original `package-verify-status.json`, raw package log |
| Service suites, Dagger vertex 34 | All executed tests pass | **247 passed, 0 failed, 3 helper ignores** | Independently totaled raw result lines at 4227–10574 |
| Installer suites, vertex 66 | All executed tests pass | **279 passed, 0 failed, 2 helper ignores** | Independently totaled raw result lines at 4744–9386 |
| Isolated GTK, vertex 60 | GTK fixtures pass | **9 passed, 0 failed** | Raw combined log line 10642 |
| Client analyzer/tests | Clean analyzer, passing suite without skipped acceptance cases | `No issues found`; **456 passed**, no reported skip counters | Raw log lines 6699,10412; compiled fixture wiring at 93–96,1180–1193 |
| Package tests | Passing package test suite | **41 tests**, successful gate | Raw log line 9322 and following successful result |
| Current source identity | Exact manifest equality, including reviewed correction | 1,267 entries match; all 33 reviewed service inputs match | Independent current-tree/hash reconstruction |
| Actual built artifact identity | Recorded package/binary hashes match bytes | All **97 artifact inventory entries** and all five separately recorded binaries match | Independently hashed actual files; no execution |

The package verifier's 95-file count is distinct from the 97-entry complete artifact inventory, which includes package metadata/checksums. Recomputed artifact inventory digest: `6cc0713bc6afcba2ec13554ed2a5adb514b438d31fc0a8dbf7c1dfba52f8c34f`. The service binary and client-bundled service both match `c870ae568e1d9447fc728ced4c1842b8792ecee3d42a42b816edcc3acb05100c`.

The cloud fixture is ignored only in the ordinary Rust harness and is wired into Flutter by `SMITH_TEST_CLOUD_SERVICE`; the combined log records that setup. Inspected `next/client/test/rust_cloud_integration_test.dart:136–245` to verify that it exercises the compiled Rust schema3 path, actual local/fake-cloud routing, frozen consent, running-request reattach and expiry rather than substituting Dart-only responses. The 456-test gate has no reported skips. This closes the pending aggregate evidence boundary for this service review; it is not a fresh comprehensive installer/Flutter UX review.

The final raw service log includes the three recovery regressions, raw-header cases, encrypted negotiation cases, gated TLS expiry and corrected slow-drip test passing. The earlier failed aggregate remains retained with its original hash and outcome; it was superseded by this changed-test snapshot, not erased or relabeled.

### Reviewer verification method and limits

Read-only commands executed from repository root included:

```sh
git --no-pager rev-parse HEAD
git --no-pager branch --show-current
git --no-pager diff --check -- next/service
sed -n '1,180p' next/service/build/cloud-reviewed-final-validation-20261015/validate.py
cat next/service/build/cloud-reviewed-final-validation-20261015/package-verify.log
sed -n '130,245p' next/client/test/rust_cloud_integration_test.dart
sed -n '245,330p' next/client/test/rust_cloud_integration_test.dart
date -u '+%Y-%m-%dT%H:%M:%SZ'
```

Read-only Python inspection independently parsed the retained JSON, hashed current source/log/artifact bytes, compared copied records and four inventory entry arrays, rebuilt the inventory using `validate.py`'s exact traversal/exclusions without invoking its write/run functions, and summed raw test result lines by Dagger vertex after stripping ANSI formatting. The actual reconstructed inventory result was `CURRENT_RECONSTRUCTED 1267 a4c131f90932cb7455acfd842373e93dd89d78e67b1d4b5e62efc1cb47da95db EXACT_MATCH True`; final reviewed input comparison returned `REVIEWED_33_VS_FINAL []`. Targeted `rg` and `sed` inspected raw failures/successes and fixture wiring; no broad suite, package verifier or binaries were rerun in this evidence-only closure.

**Evidence limitations retained:** owner-writable metadata/hashes are not signatures; Dagger may reuse cache; this is not clean-clone/hermetic provenance. No real GNOME Keyring/login-session, vendor account/model/billing, installed migration/restore, normal-host operation or deployment is validated or authorized. Timestamp UI and unrelated feature assurance remain outside this exclusive service review. Later changes to reviewed inputs require renewed binding/delta review.

### Final decision

- **Scoped service-source assurance: PASS.** Final isolated aggregate/build/package evidence is now bound to the previously reviewed implementation and test-only remediation. No outstanding scoped source blocker or further production remediation request.
- **Source gate recommendation: GO** for recording completion of this bounded source-validation task; earlier pending aggregate paragraphs are superseded as described above.
- **Release/live gate: NO-GO without separately bound human approval and required release/live evidence.** No human approval, risk acceptance, release, deployment, credential use or installed-state migration is granted by this review.
