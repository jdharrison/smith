# Provider Save MIME correction — independent source/security review

## Validation scope

- Fresh assurance context for `LOCAL-SMITH-NEXT-PROVIDER-SAVE-FIX`, on `feature/autonomy`, HEAD/base `7f6a483ae82c4bcf647268f035f1e7cc80409d4b` plus preserved uncommitted user work. Review performed 2026-10-15, starting around 23:22 UTC.
- Exact follow-up: `next/service/src/cloud_keyring.rs` and `cloud_keyring_tests.rs`. Production read-only; the reviewer authored only this report. Focused private-fixture tests and formatting checks were permitted and executed.
- No normal-host bus/keyring, real credentials, vendor API, installation, service lifecycle, cleanup, commit, publication or risk acceptance. Broader cloud/queue/timestamp work was not edited or re-reviewed wholesale.

**Final scoped source verdict: PASS.** The final evidence closure below verifies the new stable source/build/package gate and bounded real-GNOME fixture against current inputs. No concrete blocker remains in this MIME correction. Earlier pending-gate statements are retained as review history and superseded by that closure, not erased. This is not normal-host compatibility, release approval or authorization of external effects.

## Exact original versus new semantics

Git reports these source files as untracked within the earlier cloud work, so an empty Git diff is not evidence of no change. The comparison anchor is `docs/evidence/cloud-providers/source-manifest.json`, supported by the preceding service and integration reviews.

Independently compared that prior manifest with the new running gate's `combined-before.json`: the only entry deltas are the two keyring files. Independently hashed 52 current service source/test/Cargo and installer-source entries against the prior manifest: only those same two files differ. This preserves, rather than overwrites, the broader uncommitted baseline.

For stronger byte-level verification, reconstructed the originals **in memory only**:

1. Production: remove the two-line GNOME comment and replace the new `matches!` predicate with `content_type != "text/plain; charset=utf-8"`. The resulting SHA-256 exactly equals the retained original production hash.
2. Tests: remove `returned_content_type`, restore the single-line `Some((path(SESSION_PATH), iv.to_vec(), encrypted, secret.3))` assignment, and remove the new round-trip helper/two positive tests/negative-MIME test block. The resulting SHA-256 exactly equals the retained original test hash.

Thus no other production or existing-test edits are hidden in these untracked files. No reconstructed file was written or compiled.

**Implemented delta:** accept exactly `text/plain` or the previously accepted literal `text/plain; charset=utf-8`. No case folding, whitespace trimming, MIME parser, wildcard or arbitrary charset acceptance. The old literal has a space and hyphenated `utf-8`; the specification's illustrative `charset=utf8` is not an additional accepted spelling.

### Upstream/specification verification

Independently fetched the plan's exact upstream URL:

- https://raw.githubusercontent.com/GNOME/gnome-keyring/46.2/daemon/dbus/gkd-secret-secret.c
- `gkd_secret_secret_append()` sets `const gchar *content_type = "text/plain"` and returns it in the secret tuple. `gkd_secret_secret_parse()` extracts the incoming content type but does not preserve it in the constructed secret. This establishes the literal independently of submitted MIME for this upstream version, not the operator's installed implementation.
- https://specifications.freedesktop.org/secret-service/latest/types.html defines the tuple as session, parameters, value and content_type; its example is `'text/plain; charset=utf8'`, not a mandatory return literal or round-trip equality requirement.
- https://specifications.freedesktop.org/secret-service/latest/ch02.html describes secret values as byte arrays and encryption during transfer. Text MIME does not select a plaintext transport/session.

The initially tried `ch14.html` URLs returned 404; the specification's index linked the successful `types.html` page above. No claim relies on those failed URLs.

## Security and adversarial findings

### Critical / high / medium / low

**No new scoped source finding at these severities.** The following implemented invariants were inspected, not inferred from MIME alone:

- `cloud_keyring.rs:244–265`: exact negotiated session, 16-byte IV, nonempty ciphertext capped at 4112 bytes, AES-CBC/PKCS7 decryption and `Secret::new` validation remain. `cloud.rs:84–98` still permits only 1–4096 bytes in `0x21..=0x7e`; no decoding, transcoding or byte normalization was introduced.
- `cloud_keyring.rs:200–227,238–265`: exact four identity attributes, unlocked state, `Created == Modified`, before/after timestamp comparison and repeated unique search remain. `save:341–347` still requires byte-exact readback and the returned created-item path to be the exact unique match.
- Fixed-path login-bus selection, socket and peer UID checks, EXTERNAL-only authentication, unique-owner/UID validation and pinning, encrypted DH session and no plaintext fallback are byte-unchanged. Calls still carry `NoAutoStart`.
- Cancellation/original deadline checks precede the single `CreateItem`, with `replace=false`. Three-second outer keyring bound remains. No Prompt invocation, unlock, activation, collection creation, deletion, adoption or retry was added.
- `attempted` is still published before `CreateItem`; any resulting save error maps to `CREDENTIAL_OUTCOME_UNKNOWN` in the production wrapper (`cloud_keyring.rs:373–390`). Raw `Session::save` negatives correctly assert `KEYRING_CONFLICT`; they do not claim the public wrapper would report certainty after publication.
- Service save permission, one-time volatile secret consumption, success-only activation path and installer original-token workflow are unchanged against the prior source manifest. Inspected `app_v3_worker.rs:249–276` and `providers.rs:753–800`: one secret-bearing apply, original pending identity retained on uncertain response, observation rather than automatic resend. MIME success does not confer vendor-call authority or resolve an earlier orphan.

### Independent negative-MIME fixtures

`Collection::create_item` asserts the client's unchanged outgoing UTF-8 MIME, independently decrypts the canary using the peer-derived key, then generates fresh IV/ciphertext. Return MIME comes solely from `State::returned_content_type`, defaulting to `text/plain`; it never echoes `secret.3`. Tests use fresh state/session per negative case, with otherwise valid identity/session/ciphertext. This isolates MIME rejection from unrelated malformed-session/property failures.

All seven values are separately exercised: empty string, `application/octet-stream`, `application/x-binary`, `text/html`, `text/plain; charset=iso-8859-1`, `text/pläin`, and `text/plain; charset=ütf-8`. Each asserts conflict after exactly one create, attempted publication and existing item, then conflict on a separate read and still exactly one create. Positive default-GNOME and explicit previous-literal cases both save/read byte-exactly without a second create. The test count is 15 functions; seven MIME inputs are cases within one function, not seven separate test functions.

### Known concern deliberately unchanged

The existing `Created != Modified` rejection may independently affect real GNOME creation/set-secret interoperability. The plan explicitly preserves it. This review neither accepts that residual risk nor requires an unrelated persistent-integrity redesign to pass the bounded MIME source correction. Synthetic success is not a claim that all real GNOME Save failures are fixed. Any separately observed timestamp failure must be reported accurately under its own scope.

## Required Checks Evidence

| Check / command or scenario | Expected | Actual / status | Evidence |
|---|---|---|---|
| Parent pre-fix `gnome_plain_text_round_trip` | Fail specifically after publication | Retained failure: `KEYRING_CONFLICT; creates=1`, 0 passed / 1 failed | `next/service/build/provider-save-fix/expected-red.log:6–18`; independently read and hashed; not rerun against edited production |
| Reviewer `cargo test --locked --offline --lib cloud::keyring::tests -- --test-threads=1` from `next/service` (30-second tool bound) | All private keyring tests pass without host bus/vendor access | Exit 0; **15 passed, 0 failed, 0 ignored**, 90 filtered; 0.78 s tests; no rebuild (test profile ready in 0.05 s) | Reviewer terminal execution; test binary `smith_service-7c11b7ac37ef8548`, SHA below |
| Reviewer `rustfmt --check --edition 2021 next/service/src/cloud_keyring.rs next/service/src/cloud_keyring_tests.rs` | No formatting changes needed | Exit 0, no diagnostic output | Reviewer terminal execution from repository root |
| Original-source reconstruction and prior/current manifest comparison | Exactly bounded MIME/test delta | Both reconstructed original hashes match; 52 current scoped entries checked, only two deltas | Prior source manifest; running gate `combined-before.json`; read-only Python SHA-256 comparison |
| GNOME literal and Secret Service contract | Independent support for normalized MIME, no plaintext-session inference | Verified source and specification as described above | Public URLs above, fetched by reviewer |
| New combined tests/build, Clippy, service/installer and paired-package evidence | Final successful source/artifact-bound gates | **PENDING / not accepted by this review**; combined status was `running` at inspection | `next/service/build/provider-save-fix/final-validation/combined-status.json`; parent-owned final closure required |

The retained pre-fix failure is a successful expected-red regression check, not a current failing required test. Its text alone does not cryptographically prove execution chronology; the exact original reconstruction independently supports the diagnosed failing predicate. The current focused run is reviewer-executed, not merely a parent narrative.

## Source and evidence identities

SHA-256:

| Input | Original | Reviewed current |
|---|---|---|
| `next/service/src/cloud_keyring.rs` | `bf14f1524781323efa2f327d15b84a603a361bc0d77510ee3404fa2fb67985d4` | `93b2be987cdd6883d8d500269c7ec8be5c5576d3a85375b9e184d938ffdd3279` |
| `next/service/src/cloud_keyring_tests.rs` | `9da515506dcc079a4b847ff2fcbc40e2a225e41770f991eff8cde875ffdd0973` | `e35cd783bbe4bae05f95b18dbb13572f89aae2be0fb5bada1dbc20fce5f036dd` |

Additional independently computed hashes:

- Prior `docs/evidence/cloud-providers/source-manifest.json`: `34b392844875115df3814f20545bb5b9a30acd8a49424f03c2c42c27cc3dba51`.
- Bounded plan: `01639f07cb43922426d0cc16f81be7b18f0037fd6e4a990449e5ab320bd7597a`.
- Expected-red log: `2a17b93e52e3bdd3c657ea9fc08f2fbea3f3f836b3a9f77b24763350aad63e04` (unchanged on repeat hash).
- Executed focused test binary: `3c158d43ad2ec9039b0e63764e4faf4dbb8a67e0670c1aea187205f8aa765325`.
- Current `cloud.rs`: `1c3de134d5f5c81f9684653685a39fff2e3fe845558ede9e5fd14476ebe76110`, unchanged against prior manifest.

Both reviewed keyring hashes match the running gate's before-inventory entries. That inventory records digest `ad0cdd12d92cdf163b02fce7ab5d70e4ba8fa5160584fa5fe5df5bb6368e235c`. This is a start snapshot, not verified before/after stability or final artifact binding. Owner-writable hashes are not signed provenance.

## Evidence Gaps and final gates

- No broad compiler/build execution by this reviewer, avoiding contention with parent validation. The prior cloud aggregate PASS belongs to the prior source identities and cannot automatically cover this correction. Final new aggregate, warning-denied Clippy, service/installer and paired-package verification must pass and be bound to unchanged reviewed inputs, with raw outcomes retained in the validation ledger.
- The separately running disposable private-GNOME task was not executed, observed or accepted here. Preserve its actual result and limitations separately; it must not be represented as normal-session/installed-host evidence.
- No real credential, normal session keyring, vendor account/model/billing, installed migration/restore, live service upgrade or deployment evidence. Rebuilding an installer does not repair an unchanged installed service. Existing uncertain items are not adopted, retried, cleaned or activated by this review.
- A later production/test change requires renewed delta review; the final parent closure must check exact source and artifact identities. Canonical documentation/validation outcome updates remain parent-owned, outside this report-only authorization.

## Decision

- **Verdict: PASS for the exact source/security correction and focused private-fixture checks, conditional on final gates.** Implemented MIME behavior meets FIX-R1/R2 and preserves the reviewed FIX-R3 boundaries. No source remediation requested in these two files.
- **Overall final acceptance: not granted while required final evidence is missing.** This conditional scoped PASS must not be recorded as a combined PASS or an accepted-risk decision.
- **Release Gate Decision: NO-GO.** Required follow-through is final successful source/build/package evidence, unchanged-input binding and accurate separate interop reporting. Release, deployment and normal-host effects still need their separately bound explicit human approvals. Assurance grants none.

## Final scoped evidence closure — 2026-10-15T23:34:58Z

### Disposition and review method

**Verdict: PASS for the bounded MIME source/security correction and its final isolated source/build/package gates.** Required final evidence is now verified. This section supersedes the earlier conditional verdict, pending combined-evidence row, start-snapshot qualification and pending private-GNOME paragraphs. The original expected-red failure and all original review observations remain preserved. No critical, high, medium or low source finding was added by this closure; no further scoped production remediation is requested.

Only this review was edited. The reviewer ran read-only Python inventory/hash/log checks and read retained source/harness/metadata; no broad tests, compiler, package verifier, container, keyring, vendor call or installed binary was rerun. The earlier reviewer-executed 15-test run remains valid evidence. HEAD and branch were rechecked and remain the recorded base / `feature/autonomy`.

Evidence root: `docs/evidence/provider-save-fix/`. Original runner records and raw logs: `next/service/build/provider-save-fix/final-validation/`. Original GNOME harness/probe/isolation records: `next/service/build/provider-save-fix/gnome-integration/`.

### Source and artifact binding independently verified

- Reconstructed the inventory from the current filesystem using the inspected runner's traversal, exclusions, symlink handling, byte counts and modes, without executing its snapshot/write/run functions. **All 1,267 entries exactly match** the retained source manifest; compact sorted-JSON digest is **`ad0cdd12d92cdf163b02fce7ab5d70e4ba8fa5160584fa5fe5df5bb6368e235c`**.
- All four actual `combined-before`, `combined-after`, `package-verify-before` and `package-verify-after` entry arrays equal that current reconstruction. Their SHA-list files also exactly match independently generated text. This is verified before/after equality, not continuous monitoring.
- Recompared the previous cloud source manifest: **exactly two entry deltas**, `next/service/src/cloud_keyring.rs` and `cloud_keyring_tests.rs`. Their current hashes still equal the two reviewed-current hashes above. The original-vs-new byte reconstruction and security conclusions therefore remain bound to the final source. No concurrent cloud/queue/timestamp work was reverted or silently exempted.
- Verified retained copies byte-for-byte: `final.json` equals runner `evidence.json`; `source-manifest.json` equals `combined-after.json`; `artifacts.json` equals runner `artifacts.json`; expected-red log equals its original copy and retains the original hash above.
- Independently hashed all **97 package entries**, checked their byte counts, and recomputed package inventory digest **`3794c8684a179e6292b2cf7db37abce9b8886e19e138554d8316235fdcd3b3cb`**. All match. This is distinct from the verifier's **95 manifest-listed payload files**.
- All **six separately recorded binary paths** match actual bytes. Standalone service, client-bundled service and installer service payload all equal **`e5860b9e3751eddf1cefa5eb3b8464d31c1b3847c9d7c5c6fda6c1fc847169a1`**. Client, installer CLI and installer UI binaries also match their respective final-record hashes.
- Canonical docs, this review and `next/VALIDATION.md` are outside the runner's inventoried component/build-input scope. Their subsequent outcome updates do not create a tested-source mismatch and are not claimed to have been tested by that gate.

### Required Checks Evidence — final gate

Commands below ran from `next/` in the parent runner, not in this reviewer closure. Original status records exactly equal the corresponding command records in `final.json`; log hashes were independently recomputed.

| Required check / scenario | Expected | Verified actual / result | Evidence |
|---|---|---|---|
| `env DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make test build` | Complete successful aggregate on stable inputs | **Exit 0**, `killed:false`, 566.6 s, 23:22:09–23:31:36 UTC | `combined-status.json`, hash-matched `combined.log` |
| Rust formatting, warning-denied Clippy, locked all-target tests | All pass | Successful service/installer test vertices invoke inspected `set -eu` scripts with formatting checks, `cargo clippy --all-targets --locked -- -D warnings`, then `cargo test --all-targets --locked` | Current inventoried `next/service/scripts/test.sh`, `next/installer/scripts/test.sh`; combined log vertices 25/76 |
| Service suites | No failed product tests | **250 passed, 0 failed, 3 helper ignores** | Independently summed raw result lines for vertex 25; MIME positives and negative test pass at log lines 5205, 5212, 5222 |
| Installer suites | No failed product tests | **279 passed, 0 failed, 2 helper ignores** | Independently summed raw result lines for vertex 76 |
| Isolated GTK | All tests pass | **9 passed, 0 failed** | Raw vertex 54 result |
| Client format/analyzer/tests and compiled cloud fixture wiring | Passing client gate, no missing acceptance skip | Clean analyzer; **456 passed**; no skip lines reported in raw log | `No issues found` line 9816; final test line 10598; `SMITH_TEST_CLOUD_SERVICE` wiring line 1966; inventoried client test script |
| Package tests | Suite passes | **41 tests, OK** | Raw vertex 128, lines 9853–9855 |
| `python3 dagger/package.py --root installer/dist --verify` | Paired recipient package accepted | **Exit 0**, `killed:false`, 0.409 s; 95 files verified | `package-verify-status.json`, hash-matched `package-verify.log` |
| Final source / artifact identity | Exact current identity and stable snapshots | 1,267 current entries, four matching snapshots; 97 package entries and six binary paths match | Independent read-only reconstruction/hash comparisons described above |
| Real GNOME isolated fixture | Production encrypted adapter saves/reads both synthetic keys without normal-host access | Retained **PASS**, probe exit 0; one successful Save and one explicit read per provider, exact canary equality | Retained integration log, summary, inspected original harness/probe/isolation/source snapshots |

Service/installer ordinary-harness ignores are the recorded controlled subprocess/fixture entry points; the compiled schema3 fixture is explicitly wired into the client gate. The counts are not obtained solely from `final.json`: raw result lines were independently totaled by Dagger vertex after removing ANSI formatting. No new test failure or missing required bounded-source gate remains.

### Real GNOME evidence: verified scope and limits

Inspected `Dockerfile`, `run.py`, `compile-command.json`, `probe.rs`, `harness.py`, `isolation.json`, original/copy summaries and logs, before/after source maps, empty successful compiler-output logs and cleanup output. Verified all six summary-listed GNOME artifacts against actual hashes, including the compiled library and executable; copied log and summary equal their originals.

- Ubuntu image is `ubuntu:24.04`; the actual fixture log reports **GNOME Keyring `46.1-2ubuntu0.2`**, D-Bus `1.14.10-4ubuntu4.1` and libgcrypt `1.10.3-2ubuntu0.2`. This observed package version, not the separately inspected upstream 46.2 source, defines the real-daemon result.
- Compilation uses actual `next/service/src/lib.rs` with cached Rust dependencies to produce a fresh `libsmith_service.rlib`; the generated probe links that library. It is not a fake `CredentialStore` or the packaged service executable. `Vendor::ALL` is traversed once, with one `SecretService.save` and one explicit `SecretService.read` per provider, the unchanged `KEYRING_BUDGET`, and synthetic non-vendor canaries. The production Save additionally performs its normal internal verification read.
- The harness, not Smith, starts a private D-Bus and GNOME daemon, unlocks the disposable collection and verifies an initially empty default collection and same-user service owner. The production adapter still uses its fixed `/run/user/1000/bus`, encrypted session and unchanged security checks. The harness's ambient bus variable is not a new production override.
- Retained container inspection records **UID `1000:1000`, network `none`, mounts `[]`, privileged `false`, capabilities dropped `ALL`, and `no-new-privileges`**, consistent with the create command. Image ID is `sha256:30009d688f5412eae6afdff1964258c5fd3ac67f5d34d73629169f95870f981e`. The inspected runtime setup has no host-bus/keyring mount or vendor call; build-time package acquisition is separate from runtime network isolation.
- Actual log: OpenAI Save OK (513 ms), explicit read OK/canary equal; Anthropic Save OK (208 ms), explicit read OK/canary equal; final item count two; observed items unlocked with `Created == Modified`; probe exit 0. The inspected probe has no retry loop. Production adapter checks, not MIME metadata alone, establish encrypted-session/readback validity in this fixture.
- The original repository before/after maps differ **only in this review document**, which appeared concurrently. Independently checked all **51 service entries** present in those maps against both snapshots and current files: all match. This explains `source_before_after_equal:false`; it is not a service-source mismatch.
- Cleanup output names the removed disposable container, consistent with runner cleanup and summary. This is retained execution evidence, not a fresh inspection of a still-running container or an independently signed audit trail.

This fixture establishes compatibility with the tested private daemon/package under the observed conditions only. It does not establish installed-user discovery, normal-host behavior, every keyring version, repeated trials or all creation/set-secret timing conditions. **The existing timestamp integrity check remains unchanged and its broader residual concern is not accepted as risk.** No real provider credential, account entitlement, model access or billable API request was tested.

### Final evidence identities

Independently computed SHA-256:

```text
docs/evidence/provider-save-fix/final.json
  ee04e521d82f24ad0d51240f76de053b33e240348ec14daf367492224cbb668a
docs/evidence/provider-save-fix/source-manifest.json
  728abc4551e485fd592a6c30bec5f684b3ee8c4765c22ac95e5731ae470f75d2
docs/evidence/provider-save-fix/artifacts.json
  f0e78e46b35402c3a5879cc010236c540c73dd556d3b99323cb0b325837159b8
docs/evidence/provider-save-fix/gnome-integration.log
  76d8aeef627b81f75484f88fc246df36b5b165c41b59252c2e8b4d84013e1d30
docs/evidence/provider-save-fix/gnome-summary.json
  e84e54efb6caad45022b5f300676d0ec9e1cffd267f35791ebae286d767e7a43
next/service/build/provider-save-fix/final-validation/combined.log
  d5a776368304570ef357c395a26f8e684223f120e75b9eda49f034ec474f1c50
next/service/build/provider-save-fix/final-validation/package-verify.log
  4f6e997052bf8ed0fe4e97c0e068bea00d34f1891dc7ee807e21581cdbb6da2e
next/service/build/provider-save-fix/gnome-integration/isolation.json
  0e42b5e4334eaef4bb2e6cbf5efcfe0d41536c9317c7106f4882b0f1f9eaca70
next/service/build/provider-save-fix/gnome-integration/compile-command.json
  dd20cdebe01082ce632fa2e054b2ac112f385d253990cfc801a1b561e613ab13
```

Expected-red and reviewed source hashes remain as recorded above. Hashes and owner-writable metadata provide consistency checks, not signatures or adversarial provenance. Cached Dagger/dependency inputs may be reused; this is not clean-clone/hermetic release evidence.

### Final decision and remaining boundaries

- **Verdict: PASS — scoped source/security and final isolated source/build/package assurance.** FIX-R1/R2 are satisfied; FIX-R3 boundaries remain byte-bound to the reviewed correction; final evidence is now sufficient to close the earlier pending source gate. Accurate private-GNOME reporting supports FIX-R4 without claiming installed-host repair.
- **Source gate recommendation: GO** for recording completion of this bounded correction. No remaining scoped source blocker or production remediation request.
- **Release Gate Decision: NO-GO without separately bound explicit human approval and required release/live evidence.** This source closure neither approves a release/deployment nor accepts residual risk. Normal-host keyring, graphical acceptance, actual vendor/API/model/billing, installed upgrade or migration/restore remain separate effects/checks. No previous uncertain item may be automatically retried, adopted, activated or removed. The normal running service is not changed by rebuilding the package.
- Later changes to reviewed inputs require renewed binding/delta review. Canonical docs/ledger and broader feature assurance remain separately owned; this closure does not broaden the original two-file correction or authorize host effects.
