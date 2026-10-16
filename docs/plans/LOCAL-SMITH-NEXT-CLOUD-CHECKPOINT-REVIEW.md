# Cloud checkpoint — fresh independent integration assurance

## Validation scope

- Fresh assurance context for the accumulated tracked **and untracked** work relative to `7f6a483ae82c4bcf647268f035f1e7cc80409d4b` on `feature/autonomy`, not merely the last saved-key amendment. Review date: **2026-10-16 UTC**; source/export comparison completed at approximately **08:14 UTC**.
- Risk-based integration inspection of immutable cloud attempts, destination consent, saved-key eligibility, client follow-ups/exit holds, disk4 timestamps, Boot/entrance, shared header/scroll, session-only New Window, and Make/Dagger/component wiring. Supplement existing feature assurance rather than repeat exhaustive HTTP, cryptography, keyring or catalog review.
- Independently verify the latest retained aggregate against current inputs, raw logs/statuses and actual exports. This reviewer ran read-only identity/evidence/whitespace checks, **not new test/build runs or artifact execution**.
- Authority is the explicit review request and [checkpoint plan](LOCAL-SMITH-NEXT-CLOUD-CHECKPOINT.md). **Only this document is written.** No staging, commit, implementation edit, cleanup, push, deployment, live API, normal-session keyring, installed service/state, model or GUI operation. The parallel privacy/artifact audit is separately owned and is not certified here.

Read in order: `AGENTS.md`, `docs/SCOPE.md`, relevant Architecture/Process/Design/Host Profile sections, `roles/assurance.md`, checkpoint plan and relevant feature plans/source/tests/evidence. Normative target policy is distinguished below from inspected implemented behavior. Assurance recommends or blocks; it does not approve its own work, accept risk or authorize an external effect.

## Findings, by severity

### Critical

**None identified in this integration review.** This is not a secret-free/artifact-publication certification; that is the separate audit.

### High

**No open source-integration or current aggregate-binding blocker identified.** The required latest aggregate really completed on the current component inputs; it is not a collection of substitute component passes. Historical failures remain failed historical executions, not accepted risks or proof of a fixed runner.

### Medium

**None identified in the inspected integration seams.** No concrete route rebinding after admission, new-consent bypass, eligibility-to-authentication confusion, timestamp/context contamination, or window-to-service ownership violation was found.

### Low / documentary qualifications

Older queue, catalog, schema-fix and New Window records retain then-current pending/FAIL/NO-GO statements. Those must be read with their dates and the newer `next/VALIDATION.md` saved-key section and final saved-key review closure. The latest **668-client/1,262-total** aggregate supplies current configured source/build evidence for accumulated work, including New Window; it does not retroactively change any old outcome or establish native graphical acceptance. This report supplies the checkpoint-level integration assessment missing from the narrower saved-key review. No implementation or historical evidence rewrite is requested.

## Integration assessment — implemented behavior

### 1. Saved-key attempts do not become ambient cloud authority

`next/service/src/app_v3.rs` keeps eligibility in `ready`: supported exact model plus the active exact credential generation, independent of cached observation/validation. `route_catalog` uses that predicate while returning factual advisory provider/check metadata. `next/client/lib/cloud_routing.dart:99–233` still validates provider identity, required nullable fields, generation equality and exact-model verification; availability is not authenticated account access. The installer shares the service catalog and retains its strict provider-domain parsing; the latest paired gates cover the changed semantics.

Execution remains separate: `next/service/src/app_v3_worker.rs:46–186` checks ownership/deadlines, freshly reads the admitted reference, rechecks provider identity/revision, publishes the dispatch marker, and checks permission again before one HTTP attempt. A key-read failure exits before HTTP. No cached successful probe or validation substitutes for retrieval, and no inaccessible saved key triggers another generation or local/provider fallback. Cloud success does not manufacture an explicit-validation record.

This agrees with the saved-key review and current canonical policy. It is **source/synthetic evidence**, not proof that a normal-session key exists, decrypts, authenticates, has billing or can access a particular listed model.

### 2. Immutable accepted attempts versus explicitly rebound unsent follow-ups

`next/service/src/app_v3.rs:828–943,1372–1550` snapshots exact route, credential/provider revision or local digest, endpoint/policy, bounded context/hash and limits. Submission rechecks preparation token/fingerprint, context, consent requirement and deadline before atomically retaining consent/route/job. Replay resolves the retained identity rather than making a new inference. The worker consumes the captured task, not a subsequently selected route.

`next/client/lib/workspace_cloud.dart:71–127` fences cloud preparation/confirmation against instance, config, conversation/history, route revision, draft and exit hold. `workspace_controller.dart:1068–1283` reserves immutable text/identity and distinguishes definitive pre-admission rejection from uncertain admission or later observation failure. Accepted/request route metadata is separate from the future-send selection.

The important queue qualification remains: **only accepted attempts are immutable across explicit Resume**. Route selection pauses retained local follow-ups; revision-checked Resume may rebind undispatched logical entries. It does not rewrite an accepted predecessor. For an unapproved destination, the queue pump cannot supply consent: it pauses for **Review queued cloud send**. An already-approved destination can advance after deliberate Resume without redundant first-transfer confirmation, under the retained per-chat/generation contract.

Evidence inspected: `workspace_controller.dart:1746–2021`, `workspace_cloud.dart:208–229`, and the actual compiled-boundary assertions in `next/client/test/rust_cloud_integration_test.dart:489–701`. The scenario verifies accepted-route stability, may-have-sent reattach, stale/expired preparation, explicit queued rebinding, first queued transfer review, new-ID retry with original queued text, preservation of a newer composer draft and lost-ack observation without resubmission. These tests are included in the source-bound aggregate, not newly executed by this reviewer.

### 3. Queue uncertainty, history ordering and close remain independent of presentation

The conversation-owned queue still advances only after the exact successful predecessor and matching synchronized saved pair/revision. Foreign history, service/config drift and unresolved outcomes pause it. `workspace_controller.dart:1710–1743` treats terminal `REMOTE_OUTCOME_UNKNOWN` as an unknown barrier, not a failed head eligible for replay/Skip. Retry/Skip/Remove/Resume retain revision and state checks. Four outstanding service jobs, one per chat and one worker remain the service boundary, not an inference-per-pane/window design.

The shell owns `QueueExitGuard`; the identity-bound workspace hold prevents undispatched work while Close/Quit is decided. `next/client/linux/runner/my_application.cc` accepts only an explicit boolean true before destroying the window; missing/error responses keep it open. Dart disposal never releases a confirmed exit hold into resumed dispatch. This is a normal-close source contract, not crash protection or cancellation of accepted service work. New windows have independent process-local drafts/queues/holds; shared saved history and service admission remain authoritative. Cross-window queues are not globally synchronized or exclusively ordered.

### 4. Disk4 timing does not alter provider context or queue authority

`next/service/src/app_v2.rs` strictly migrates disk1/2/3 into disk4, retaining configuration/history/routing/receipts and leaving historical timing unknown. `app_timestamps.rs` bounds and aligns a separate sidecar; reservation accounts for maximal timing before spending inference work. Routed completion appends/trims provenance, history and timing in the same durable candidate (`app_v3_worker.rs:318–373`); ordinary completion also appends timing. Timing is not inserted into role/content `Message`, request context or the consent hash.

`next/client/lib/workspace_controller.dart:520–569` clears previous timing on detail replacement and attaches optional metadata only for the same instance, conversation, history revision, history object and pair count. Missing/invalid/failed timing leaves readable history without fabricated observation dates. `message_timestamps.dart` and `ui/message_header.dart` retain strict nullable ranges, event-local timezone/offset and wrapping labels. Transcript order, successful-predecessor checks and delivery certainty do not depend on wall-clock differences.

**Compatibility target/constraint:** app3/Core2 are unchanged, but old binaries cannot read disk4. A future approved rollout/rollback requires compatible paired binaries, a pre-migration state snapshot and tested restore preserving exact keyring-generation relationships. No installed migration or restore was performed here; binary-only downgrade is not a rollback procedure.

### 5. Boot, entrance, shared header and reading anchors do not acquire service authority

`next/client/lib/boot.dart:391–527` owns cosmetic intro/hold/outro separately from `BootCoordinator`; the fully visible hold starts after intro completion, and repair/confirmation interrupts decoration. `main.dart:250–303,334–338` retains one shell/controller/Boot/exit owner, lazy conversation mounting and then preference hydration. `ui/workspace_entrance.dart` uses one finite paint-only clock after hydration; recovery/reduced motion settles it, and Close/Quit remains outside its interaction gate.

The shared `SmithAppHeader` is controller-free; the split owner supplies the single header above the canvas. Neither header duplication nor per-pane observation/worker ownership is introduced. `chat_panel.dart:50–124,149–189` retains desired reading position separately from render clamping; infinite desired-bottom anchors are clamped before application and never used as initial native offsets. Deferred callbacks check mount/controller identity. Timestamp wrapping or new queue/status content may change extents without authorizing Send, stealing composer ownership or intentionally forcing a reader who scrolled away to the bottom.

Inspected feature records retain the loader delayed-first-opaque-frame remediation, scroll regressions and entrance fixture correction. The latest full suite covers their combined current source; historical fake captures and widget passes remain **not installed graphical/IME/accessibility acceptance**.

### 6. New Window is a fresh client process, not a second lifecycle manager

`next/client/lib/window_launcher.dart` uses the current resolved executable, the sole fixed `--session-only-window` argument, detached mode and no shell. Production injection is only in `main`; embedded/test constructors cannot implicitly launch a host process. `main.dart:168–181` omits preference-store construction for the exact flag, so a child cannot later adopt the primary's preference lock/storage. Ordinary startup is unchanged. Pending launch is single-flight, failures are sanitized and explicit retry is required; disposal fences late UI feedback (`main.dart:347–363`). Spawn success does not claim visibility or readiness.

The existing native runner forwards argv to Dart and uses `G_APPLICATION_NON_UNIQUE` (`my_application.cc:136–154,206–215`), consistent with independent desktop processes rather than remote activation of the first process. Every production child receives the same launcher and ordinary Boot/exit path. No parent-child shutdown ownership, cloned drafts/queue, monitor placement, preference adoption or automatic model/service effect was added. `test/new_window_test.dart` exercises the injected composition, not an actual native descendant window; that limitation remains explicit.

### 7. Build/test graph covers the combined inputs without invoking installer/run

`next/Makefile:35–81` keeps aggregate test/build separate from the explicitly requested `installer/run` GUI convenience target. Test/build do not depend on that launch target or install targets. `next/dagger/src/smith_next/main.py:25–90` supplies the compiled service and fake-injected cloud fixture to the client test environment; configured cloud tests cannot silently use the standalone no-fixture skip path. Existing real socket/migration and timestamp fixtures remain complementary: the injected cloud application bridge is not production socket/keyring/TLS evidence by itself.

Installer formatting is now package-scoped; the aggregate's independent service formatter still covers service source. Locked warning-denied Clippy, native installer/isolated GTK/runtime checks, full Flutter checks, actual-ELF package tests, paired builds and post-export package verification completed in the retained aggregate. This is not hermetic or clean-clone provenance; dependency acquisition/cache behavior remains as documented.

## Required Checks Evidence

**D** = `docs/evidence/saved-key-attempts/`; **F** = `next/service/build/saved-key-attempts/final-validation/`. Commands shown for the retained gates ran from `next/`. Reviewer identity/evidence checks ran from the repository root using read-only Python and Git; no raw log or new script was written.

| Required check / command or scenario | Expected | Actual / result status | Evidence |
|---|---|---|---|
| Base/branch: `git --no-pager rev-parse HEAD`; `git --no-pager branch --show-current` | Approved unchanged base/branch | **PASS**, exact base above and `feature/autonomy`; exit 0 | Reviewer command output; D/F metadata |
| Complete current component inventory reconstruction | No changed, missing or extra selected component inputs | **PASS**, independently enumerated **1,291 entries**, exact records and digest `ce939c47c54b6afdb41c8b79002cd989a63e7b38ebc81933e928fea7d2cb25c3`; exit 0 | D `source-manifest.json`; reviewer reconstruction |
| Before/after aggregate and verification snapshots | Each command bound to the same complete source list | **PASS**, all four entry lists exactly equal current list, not merely equal counts | F `combined-before.json`, `combined-after.json`, `package-verify-before.json`, `package-verify-after.json` |
| Retained `env DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make test build` | Full configured graph and builds complete without timeout | **PASS, exit 0**, 07:27:03–07:33:03 UTC, not killed; **263 service + 281 installer + 9 GTK + 668 Flutter + 41 package = 1,262 passed**, zero failed | Independently hash-checked F `combined.log`, `combined-status.json`; D/F `final.json`, `component-results.json` |
| Raw count/fixture reconciliation | No omitted failed branch or hidden client/package skips | **PASS**, checked all **8 service / 12 installer / 1 GTK** primary suite summaries against raw lines, Flutter `+668: All tests passed!`, package `Ran 41 tests`/`OK`, and both compiled-fixture environment settings | Same hash-bound aggregate log; mandatory fixture source/wiring |
| Configured quality/runtime gates | Format, analyzer, Clippy and native-runtime branches succeed | **PASS as retained aggregate branches**, client 81 formatted files/zero changes and no analyzer issues; Rust quality and isolated native runtime completed | Component scripts, Dagger graph and retained aggregate; no reviewer runtime execution |
| Retained `python3 dagger/package.py --root installer/dist --verify` | Newly exported recipient verified | **PASS, exit 0**, 07:33:31 UTC; raw output verifies **95 files**, 97 recipient files total | Hash-matched F `package-verify.log`/status; D/F final record |
| Current actual exports and pairing | Exact path/content/size/kind/mode match, no stale copy substituted | **PASS**, independently enumerated **189 exports**, exact artifact records/digest; **91** standalone/client-payload pairs byte-equal; all **four** service copies byte-equal | Actual `next/{service,client,installer}/dist/`; D/F `artifacts.json` |
| Durable/raw evidence copy identity | Retained summaries correspond to original local evidence | **PASS**, all four final JSON copies byte-identical; both command log hashes match; initial failed report copy also exact | D/F final/source/component/artifact JSON; D `initial-client-failure.json` and original report |
| Historical evidence preservation | Old failures/evidence not overwritten to obtain PASS | **PASS for the recorded saved-key preservation set**, all **181** prior file hashes still match | F `prior-evidence-manifest.json`; independent rehash |
| Whitespace: `git --no-pager diff --check 7f6a483ae82c4bcf647268f035f1e7cc80409d4b` | No tracked-diff whitespace errors | **PASS, exit 0** before this report; untracked code additionally covered by retained component formatters | Reviewer command; complete current source binding |
| No staged work: `git --no-pager diff --cached --stat` | No accidental staging by review | Empty at inspection, **exit 0** | Reviewer command; no staging/commit tool invoked |

### Evidence identities and reconstruction method

SHA-256 of files, distinct from the canonical inventory digests:

| Record | SHA-256 |
|---|---|
| D/F `final.json` | `0451c35bd5bd54d69ac92e9d7109ec13f725c7c6bb9a3c9c52f035e312a1e5e0` |
| D/F `source-manifest.json` | `e21eafc2dd93e3fb52ef15df94b4285f777908a0a0c94ff312220021c22fb963` |
| D/F `component-results.json` | `fa468516649af12365bbb29f313182f4ae918152e0ea88805b185f79b894a194` |
| D/F `artifacts.json` | `3977bc642c2ab20d0297eaff70900ab99194bee070d7af3a77bd4b5d00c83c58` |
| F `combined.log` | `83994c2f5d29d0ad6f6c3e39b5cc768ea9fdba318be4cd325e4cd37d2db1d874` |
| F `package-verify.log` | `4f6e997052bf8ed0fe4e97c0e068bea00d34f1891dc7ee807e21581cdbb6da2e` |

Source reconstruction traversed `next/client`, `next/service`, `next/installer`, `next/dagger`, plus `next/Makefile`, `next/dagger.toml`, `next/dagger.lock`, applying the manifest's explicit generated/cache directory and file exclusions. Records contain project-relative path, SHA-256, byte length, file/symlink kind and actual mode, sorted by path; digest encoding is compact sorted-key JSON (`separators=(',', ':')`). An initial enumeration omitted the two top-level Dagger files; adding those recorded roots yielded exact equality, not evidence of source drift. Root canonical docs/plans, this review and later ledger/evidence additions are not compiled component inputs and are not claimed to be covered by that digest.

Artifact enumeration of the three actual export roots exactly matches all 189 retained entries and digest **`60e1f232d7519a411bd0754279e2a29d65b96050a5d3b72e5f319e1b801e5da6`**. The four service copies have SHA-256 **`5cfe584b8e597771319b3d4a3da9572bd30df9059d89f4f68e6e38fd5b2c4dcb`**. The complete recipient is `next/installer/dist/`, not a stale developer helper or installed binary. Recipient manifest SHA-256 retained in that exact artifact list is **`f2c46e2c42a595e3da9a48518dce428653a3dad258c88a6b1e12a72323c73e52`**.

No additional numbered tests are claimed from this review. Package41 completed inside the single retained aggregate; explicit verification is a different check, not a second package-test run. Three service/two installer top-level ignores are controlled child/fixture entrypoints; Flutter and package report **zero skips**. The concurrent reporter does not enumerate every compiled cloud case separately. Source endpoint equality and owner-writable hashes are drift evidence, not continuous monitoring, signed attestation or proof against transient edit-and-revert.

## Evidence Gaps and approval boundaries

1. **No required current configured source/test/build/package-binding gap found for this bounded integration assessment.** Existing feature reviews remain evidence for their own scope: [service security](LOCAL-SMITH-NEXT-CLOUD-PROVIDERS-SERVICE-REVIEW.md), [installer/client integration](LOCAL-SMITH-NEXT-CLOUD-PROVIDERS-INTEGRATION-REVIEW.md), [saved-key final closure](LOCAL-SMITH-NEXT-SAVED-KEY-ATTEMPTS-REVIEW.md), loader/scroll/New Window records and client queue validation. This review does not recertify all cryptographic/vendor contracts or every unchanged subsystem.
2. **Local commit prerequisites are still pending, not waived:** separate privacy/artifact audit (CK2), exact selected/staged file review and tree binding, hook/effect inspection, and immediate branch/base/approval revalidation (CK1/CK3/CK4). Post-commit CK5 cannot exist before a commit. This reviewer has neither staged nor approved a future tree. A later executable/test/build-input change invalidates reliance on the current gate until relevant configured gates and review are rebound.
3. Raw logs and original preservation records remain under ignored build roots. Durable JSON contains their identities, not the whole raw transcript. Availability of those local originals was verified here; a future clone cannot reconstruct all raw executions from the commit alone. Preserve originals and explicitly map any audit-produced sanitized derivatives; do not rewrite a historical hash/outcome silently.
4. Earlier runner lock/timing failures and the initial obsolete entrance-fixture failures remain recorded. The saved-key initial failed client report is preserved byte-for-byte; the latest gate is a later successful execution. No causal runner fix, harmless-flakiness judgment or accepted risk follows. New Window's previous three entrance failures/missing compiled-fixture gate are superseded **only as current configured source evidence** by the matching final aggregate, not erased as historical executions.
5. **Release/host acceptance remains unavailable and outside authority:** normal-session GNOME discovery/key access, live vendor entitlement/authentication/charges and exact-model compatibility; real native window/descendant lifetime, monitor/focus/IME/accessibility acceptance; installed login/logout/reboot/lifecycle, upgrade/migration/restore; clean-clone/reproducible release provenance and redistribution/license/SBOM clearance. No such check was run here. These are blockers to claiming those guarantees or proceeding to release/deployment, not reasons to perform unauthorized live checks during a local checkpoint review.
6. Static catalog additions are implemented source choices, not discovered account capabilities. Model discovery/generated-catalog lifecycle remains **proposal-only**. No passing source gate grants broader network, credential, lifecycle, publication or accepted-risk authority.

## Verdict and required remediation

- **Verdict: PASS for the bounded whole-checkpoint source-integration assurance and current configured evidence binding.** The inspected implemented seams meet the reviewed source contract; no concrete integration remediation is required before the separately owned checkpoint checks. This is not whole-product/native-host acceptance or approval to commit a still-unbound staged tree.
- **Release Gate Decision: NO-GO** for release, publication, installed upgrade or deployment. No residual risk is accepted and no external approval is granted.
- **Required follow-through before any local commit:** finish the independent privacy audit; resolve any blockers it raises; preserve original evidence/lineage; verify the final selected/staged bytes and review identities; inspect hooks and recheck the exact local approval/branch/base/tree immediately before the action. If executable/test/build inputs change, rerun the appropriate stable configured gates and obtain fresh affected-scope assurance. No commit, push or implementation change is performed by this review.
