# Independent assurance — installer redesign option A

## Validation scope

- Fresh independent review of the approved **presentation-only option A**: `next/installer/src/ui.rs`, new `src/appearance.rs`, installer assets, `scripts/capture-ui.py`, synthetic screenshots and retained native/package/runtime evidence.
- Preserve existing artifact/Core/model approvals, model-preparation navigation, worker ownership, cancellation, lifecycle and CLI behavior. No grouped approval or option B authority.
- Excluded unrelated multi-chat client/service implementation and `tests/multi_chat_compatibility.rs`. Reading package identities does not constitute assurance of that implementation.
- Read-only review and small local checks only; this review document is the sole write. No build, live GUI, manager, model, installation, commit, remote action or release approval.

**Claim type:** observed implementation and evidence assessment, not installed-host acceptance. Policy read: `AGENTS.md`, `docs/SCOPE.md`, relevant Architecture/Process/Design/Host Profile sections, `roles/assurance.md`, and `LOCAL-SMITH-NEXT-INSTALLER-REDESIGN.md`. The operator's supplied prior architect resource PASS is context, not a substitute for the checks below.

## Reviewed identity

Base verified by `git --no-pager rev-parse HEAD`: `2cc71a94fe924fbb9d324f29332e9f24c8c5991c`; concurrent uncommitted work remains outside this review.

| Object | SHA256 |
|---|---|
| `next/installer/src/ui.rs` | `3b8271c1a2a4a9b67eb719217a6fbb1ed6546053ff9ed76b7d54cad3658a62af` |
| `next/installer/src/appearance.rs` | `3600a3378b5c04139139cf0973231f1220e092f78b8dbdc5e3125f2a5a50727d` |
| `next/installer/scripts/capture-ui.py` | `25efe42f57a62b50507a1cc83e43310664039e30bfb744709f8cd7d44b2e6f08` |
| Follow-up-reviewed `LOCAL-SMITH-NEXT-INSTALLER-REDESIGN-EVIDENCE.json` | `d7a45e9f17dff125f9544aceec3c88f96e810f8c260ec4498b4c86fd9feb3755` |
| `standalone-installer-followup.log` | `4a6527ba6390872a85b80795de162a22113e241dffdf6368a3b91a113b732e12` |
| Exported `smith-installer-ui` | `f5b4e40bd6523a7c3f0961eb139f70cda191ed98100919df8dcf18ec1210f07c` |
| Exported `smith-installer` | `02004dbbea342926213369b6445023e4ba95b6682c75fbbf02f5b384021375ad` |
| Exported `manifest.json` | `781a7b14d4e7df75bf45f9884507097c00ce69009467cd5c74d2a9236b33f76d` |

The retained runs use observed UTC 2026-10-15 timestamps; the plan/operator date is 2026-10-14. No timestamp normalization or historical-evidence rewriting was performed. This updated verdict binds to the identities above and the retained standalone follow-up below. The initial review used evidence digest `0f35ae3489a860f2fc81bb8466e539b1f609874d9c6b744350c381d5b7e26824` and correctly failed the then-incomplete installer gate; that historical decision is superseded only for scoped source/package validation.

## Required Checks Evidence

`E` below means `docs/plans/LOCAL-SMITH-NEXT-INSTALLER-REDESIGN-EVIDENCE.json`; `L` means `next/installer/build/redesign-final-validation/`; `F` means `next/installer/tests/fixtures/ui-redesign/`. Parent/validation-agent commands are verified from retained evidence, **not rerun by this assurance reviewer**.

| Check | Command/scenario and expected outcome | Actual result/status | Evidence |
|---|---|---|---|
| Protected implementation scope | `git --no-pager diff HEAD --name-only -- next/installer/src next/installer/tests/cli.rs next/installer/Cargo.toml next/installer/Cargo.lock`; only tracked UI source changes | PASS: only `ui.rs`; new GUI-only appearance module separately reviewed. Worker, consent, CLI and CLI test unchanged against HEAD | Actual Git output; full UI diff |
| Current source binding | Python SHA256 comparison of installer and recipe entries against E.after and capture manifests; expect equality | PASS: initial 51 selected inputs and both capture installer snapshots matched; follow-up independently rehashed all 201 source-manifest entries with zero mismatches and verified before/after maps equal the original fresh-build snapshot | E.after; F/{gdk1-reviewed,gdk2-reviewed}/source-sha256.json |
| Resource integrity | `sha256sum --check SHA256SUMS` in installer assets; expect all pinned inputs valid | PASS, exit 0: 14 entries | `next/installer/assets/SHA256SUMS` |
| Focused GUI quality/runtime | Parent `make installer/ui-test`; locked fmt, warning-denied Clippy and isolated GTK tests all pass | PASS, recorded exit 0; 9 passed, 0 failed/ignored | E.runs/counts; L/isolated-ui.log |
| Capture matrix | Parent `python3 installer/scripts/capture-ui.py --output …/gdk1-reviewed` and same with `--gdk-scale 2`; expect successful stable-input GTK runs | PASS, recorded exit 0 each; 9 tests each; 141 PNGs each | F reviewed run.json, test-output.txt, source manifests |
| Screenshot integrity | Python comparison of every PNG hash/length/IHDR dimensions with JSON and logical dimensions × actual GDK scale; `sha256sum --check --quiet SHA256SUMS` in F | PASS, exit 0: all 282 PNGs and current evidence checksum index; no mismatches | Actual small checks; F/SHA256SUMS and per-PNG JSON |
| Initial full installer attempt (historical) | Parent `make installer/test package/test installer/build`; expect every required installer suite to complete/pass | **FAIL, recorded exit 2**, retained: CLI finite-budget failure; later default Rust suites not executed. Observed 154 passed, 1 failed, 1 intentionally ignored | E.failure/counts/historical_initial_outcome; L/focused-gates.log |
| Current complete installer regression gate | Validation agent independently ran `DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make installer/test` from `next/`; expect full native/GTK/quality/runtime pass | **PASS, recorded exit 0**, 103.51s: 252 native and 9 GTK passed, 0 failed, 1 intentional controlled signal-probe child ignored; all previously unreached suites completed. Original failing CLI test explicitly reports `ok`. No code or test-budget changes | E.followups[0]/final_scoped_validation; L/standalone-installer-followup.log |
| Native package tests/build | Parent separately ran `make package/test installer/build`; expect tests and paired export to pass | PASS, recorded exit 0: 41 package tests; fresh export. Still bound to identical source/artifacts; complete installer follow-up is separately evidenced | L/package-build.log; E.runs/package |
| Package verification/current bytes | Parent `python3 dagger/package.py --root installer/dist --verify`; independently hash all E.artifact_files | PASS, recorded exit 0 initially and at follow-up: 95 manifest entries; all 97 recorded package files independently rehashed with zero mismatches; paired service/helper identity recorded equal | L/export-verify.log, L/followup-package-verify.log; E.package/artifact_files/followups[0] |
| Retained evidence integrity | Python SHA256 comparison of E.logs against retained files | PASS initially for 70 files; follow-up rechecked all 72 recorded log/evidence-file hashes, including both new logs: zero mismatches | E.logs; L |
| CLI native runtime isolation | Parent Ubuntu UID1000 native help/version/artifact dry-run and absent-effect assertions; separate exported-ELF probes | PASS in retained evidence. CLI direct dependencies are libc, libgcc_s and loader only; no GTK. No new CLI behavior found | E.native_runtime_gate/elfs; L/focused-gates.log; L/release-elf-probes.json |
| Display-free GUI documentation/licenses | Five exported CLI/GUI help/version/license probes in isolated Ubuntu; GUI receives native GTK libraries but no display/package payload; expect exit 0 and full notices | PASS, all five recorded exit 0. Independently compared full license stdout byte-for-byte with heading + OFL + icon notice: exact match | L/probe.py, release-elf-probes.json and smith-installer-ui--licenses.stdout; ui.rs::dispatch |
| Capture helper credential handling | Static inspection; mocked loopback GraphQL request with synthetic noncredential; retained-output scan without printing credentials | PASS within stated scope: token goes in Authorization header, not query/body/metadata; no stdout/stderr token in mock; no matching Basic/Bearer credential header in scanned retained outputs. No ambient Dagger credentials available for exact-value comparison | capture-ui.py:67–82, 96–147; actual in-memory mock and output scan |
| Small source hygiene | Python AST parse of capture helper; root `git --no-pager diff --check -- next/installer/src/ui.rs` | PASS, exit 0 | Actual assurance checks |

The first documentation-probe wrapper failed on unsupported GraphQL `loadContainerFromID`; its corrected validation-only wrapper passed. Both retained logs match E. This was not an installer test retry or source fix.

## Implementation and adversarial observations

**Implemented, consistent with bounded option A:** the production diff changes presentation/resources/documentation dispatch, not effect commands or lifecycle sequencing. Artifact review becomes directly visible instead of an expander. `Event::Confirmation` still validates bounds, sets literal exact effects with `set_text`, stores the original request ID and focuses the effects. Approval consumes that pending request once; no automatic replies, grouped consent, default approval or new worker dispatch appears in the diff. “Review model” still invokes the existing preparation path, not an implicit model approval. The source-derived seven fresh-install effect approvals remain; no click-reduction claim is justified.

`finish()` retains report messages and incomplete-result completed/failed/uncertain rows. Scope-specific headings, worker-join gating and Close behavior remain. This change does not newly truncate confirmation text or suppress failure/uncertainty rows. Existing bounded report rendering and historical inference-result interpretation were not redesigned or represented as new typed guarantees.

Working now says “Working on this step. Cancelling does not undo earlier changes.” Confirmation says “Review the next action before approving it.” Added assertions cover both, including preconsent operation preparation. Working/finishing hide disabled pseudo-actions without exposing Close early. New synthetic assertions cover literal effects, default/focused repeat activation, single-use request dispatch, no effects from selection/draw/resize/scroll, joined results, and Close sending no consent. These support, but do not replace, native keyboard and full-worker regression gates.

`appearance.rs` is GUI-only. Three sealed CLOEXEC memfds and their Fontconfig registrations are retained process-wide, including partial registration failures; no disk/network fallback, system-font install, subprocess or client-payload lookup is introduced. Font loading and landing PNG decoding occur before `Session::spawn`. Embedded OFL/icon notices dispatch before GTK or bundle discovery. The required native-library loading is not claimed display/library independent. The developer-only icon generator pins four sources and is not a recipient build/runtime hook.

The capture helper restricts output to a new fixture directory, uses an in-memory Dagger module wrapper, records stability and real test exit status, and fails rather than claiming a passing partial export. Its session token is used only for loopback API authentication; it is not interpolated into exported scripts, run metadata or subprocess arguments. The helper relays API error messages and test streams, so this is **not** a universal redaction guarantee for arbitrary future server error content. No credential exposure was found in the inspected evidence.

## Actual image inspection

Viewed the PNGs themselves, not just README claims:

- GDK1 default landing; minimum 480×420/text2 landing last tile; minimum/text2 model first tile.
- Default exact confirmation and GDK2 minimum/text2 confirmation final tile (`scroll15`). Literal markup-like text and `END OF EXACT EFFECTS` remain visible; footer actions are fully in-window and reflow vertically. Default confirmation intentionally needs scrolling.
- Default numeric download progress: real-byte fixture count, 50%, elapsed time, corrected Working sentence, Cancel and no disabled primary action.
- Default installation success and Upgrade/Uninstall/Model removal success. Scope limitations, retained data/disk usage, no model readiness claim for upgrade, and Close are visible.
- Minimum/text2 partial failure tiles 00, 05 and 09: incomplete outcome, possibly-running service/accepted-work warning, failed model step and safe repair/no-inference guidance remain readable through scrolling; Close remains reachable.

The observed flat teal/cyan palette, real version, monospaced text and labelled outline icons match the requested visual direction. No exact-effect or uncertainty concealment was found in these samples. This is sampled synthetic rendering inspection, not inspection of every tile, pixel-perfect Flutter parity, compositor acceptance, or proof of real worker effect generation.

## Findings by severity

### Critical

None newly identified in the scoped implementation diff. No approval/lifecycle expansion found.

### High — historical H1, scoped validation blocker cleared

**H1 historical failure:** `accepted_artifacts_then_missing_runtime_roots_report_partial_effects_without_manager` failed at `next/installer/tests/cli.rs:385` with `CLI fixture exceeded its finite budget`; L/focused-gates.log records 14 CLI tests passing and 1 failing. The original failure remains evidence and has not been rewritten as a pass.

**Follow-up disposition:** E.followups[0] retains the validation agent's independent standalone `make installer/test` run, exit 0 in 103.51s, with 252 native and 9 GTK tests passing, plus fmt/Clippy and interpreter-free Ubuntu runtime gates. The previously failing test and all downstream suites now pass. This reviewer checked the raw log summaries, final success message, log hash, all 201 source identities and all 97 exports. Before/after source maps equal the original fresh-build map; no executable, test or test-budget edits occurred. The parent also reports an earlier standalone pass in approximately 96s; that is corroborative reported evidence, not a second raw log independently checked here.

H1 is cleared as a **current scoped source/package validation blocker**, not diagnosed or declared fixed. The timeout's root cause is **not determined**. No concurrency, cache, scheduling or other explanation is inferred. Its unexplained occurrence remains residual regression risk and must remain visible if it recurs. The current verdict rests on the complete retained standalone pass, not on substituting GUI/package-only results. No additional expensive test run was launched by this reviewer.

### Medium — acceptance/publication gaps, not demonstrated redesign defects

- Native desktop keyboard/focus/key-repeat, opened chooser popup, AT-SPI/screen-reader behavior and fractional display scaling remain unverified. Synthetic activation/geometry checks are not equivalent. Human visual acceptance is still pending under plan T5/T7.
- Supplied-icon redistribution rights are explicitly unresolved in `assets/ICONS-NOTICE.txt`. Embedding that notice or rasterizing SVGs does not grant rights. This independently blocks publication/redistribution, not the already approved local presentation work.

### Low

No additional actionable implementation defect found in the inspected scope. Small text duplication on successful upgrade is cosmetic and not grounds to broaden this approved review into another implementation pass.

## Evidence Gaps

- Full installer gate and previously unreached suites: now verified passing in the standalone follow-up; H1's unexplained initial timeout remains historical/residual risk, not missing current suite evidence.
- Whole-product aggregate validation is **not cleared**. The operator reports a separate aggregate runner-test failure in the parallel multi-chat records. That failure is outside this installer review and remains independently blocking for whole-product readiness; the installer standalone pass does not supersede it. No aggregate client/service correctness verdict or aggregate PASS is made here.
- Manual graphical/accessibility/fractional-scale and operator visual acceptance: pending, not silently waived or counted as passes. Do not claim complete AC5/operator acceptance.
- No live manager/model/download/inference/upgrade/uninstall validation or deployment. These require separate exact approval; their absence is not permission to run them during assurance.
- Resource-only historical syscall traces were removed at handoff per RESOURCE-VALIDATION.md. Its historical zero-write/network narrative was not independently replayed; current resource source, pinned bytes and integrated passing resource tests were inspected instead.
- Checksums detect stale/mixed artifacts, not adversarial rewriting. Existing cached Dagger inputs and a fresh export do not prove a clean-clone/hermetic build or release provenance.

## Documentation-only follow-up accounting

All **201** source-manifest entries, including `next/VALIDATION.md`, matched the original fresh-build snapshot when this follow-up review checked them. Both retained standalone before/after maps also match that snapshot. The recorded validation-document digest is `2f47b6e4fc3466ea39dfa950a3e1dbf1eb59a87a2fc135b8ea08ce949cce3eb0`.

The parent plans to update `next/VALIDATION.md` after handoff, as explicitly recorded in E.final_scoped_validation.documentation_exception. That future documentation-only change is **not** part of the tested 201-file identity and must not be described as byte-identical to it. An evidence-summary-only edit does not require a paired executable rebuild; retain the original snapshot and identify the documentation delta. No exception is granted for executable sources, assets, tests, Cargo inputs, scripts or package bytes. This reviewer neither made nor pre-reviewed that future edit. Only this review record was edited.

## Verdict and required remediation

**Scoped implementation assessment:** no new option-A implementation blocker identified; inspected source and synthetic presentation conform to the approved presentation-only direction. Current installer source/quality/runtime and package gates are verified passing on the unchanged build inputs and exports.

**Verdict: pass-with-risk — scoped installer source/package validation only.** H1's missing complete-gate evidence is now supplied; the unexplained initial timeout remains explicitly recorded residual risk. This is not a pass for complete AC5/manual operator acceptance, whole-product aggregate validation, publication or deployment. Their outstanding checks/failures are not waived or reclassified as successful.

**Release Gate Decision: no-go.**

Remaining requirements: preserve and investigate the initial timeout if it recurs; record the bounded validation-document follow-up without claiming unchanged full-source identity afterward; complete applicable manual/visual/accessibility checks before claiming that acceptance; independently resolve the separately recorded aggregate runner failure and complete coordinated whole-product validation; resolve icon rights before redistribution and obtain separately bound human approval for any external action. No implementation changes were requested or made by this review. No approval, release, deployment or accepted-risk decision is granted.
