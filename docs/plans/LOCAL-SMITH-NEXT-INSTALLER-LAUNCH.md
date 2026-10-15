# Explicit post-install desktop launch

## Scope and approval

User explicitly approved adding Launch Smith after successful GUI Install/Upgrade. Local source/tests only in existing dirty feature/autonomy worktree; preserve existing edits. No live app launch, installation, service/model mutation, commits or publication. CLI unchanged. Independent architect and designer/producer reviews completed before implementation.

## Reconciled plan

Show Launch Smith beside Close only on verified successful Installation/Upgrade terminal reports after installer worker joins, with no cancellation/fault. No automatic/default activation. Keep Manage models available in result body when footer has Launch/Close. Other results retain current behavior. Guard handler and rendering; synchronous pending reservation prevents duplicates. Successful dispatch reports Launch requested, not window/service readiness, and prevents another dispatch in that completion context. Failed dispatch shows sanitized bounded inline error and allows explicit retry without changing installation success/exit status.

Resolve the installed client using existing artifact selector/release verifier, not package payload, PATH, shell wrapper or desktop-file execution. Reuse exact owned current selector grammar releases/<digest>, owned safe directories/files and manifest checks. Launch resolved release's regular 0755 nonempty smith_client with no args and release working directory. Use installer-compatible HOME/XDG_DATA_HOME roots captured for the session. Resolution is read-only and never invokes manager/provider operations. Full verification belongs on background task, not GTK thread.

Native launcher uses a detached session, null stdio and an owned background child waiter; report spawn outcome immediately, never block GTK on desktop lifetime. Child survives installer Close; no kill-on-drop/parent-death signaling. Close during pending validation/spawn waits for dispatch outcome, not desktop lifetime; prevent new management flow until known. No automatic retries. Fresh path validation follows trusted single-operator model, not an atomic defense against malicious same-UID replacement or checksum authenticity guarantee.

## Traceability and validation

| Requirement | Evidence |
|---|---|
| R1 only verified Install/Upgrade joined completion | Fake GTK eligibility matrix, zero auto launch |
| R2 installed-only safe resolution | Temporary-root selector/manifest/permissions/target negative fixtures, no writes |
| R3 explicit single dispatch | Injected launcher double-click/key/callback guards |
| R4 errors preserve successful result | Fake failure/retry/exit-code assertions |
| R5 Close doesn't stop desktop | Isolated fixture child/session/reaping tests, pending close tests |
| R6 usable completion layout | Existing narrow GTK footer/result tests |

No real installed client execution in tests. Use isolated GTK tooling/fakes; run formatting, Clippy, installer/UI tests/build where available. Update ARCHITECTURE/DESIGN/PROCESS/HOST_PROFILE and validation record for this explicit-only extension. Fresh independent assurance required. No source evidence substitutes for operator graphical QA; missing gates remain reported, never waived.

## Evidence

Implemented in installer `src/artifact.rs`, `src/launch.rs`, `src/ui.rs`, `src/lib.rs`, with finite synthetic desktop `tests/fixtures/desktop.rs`. Source and retained log hashes are in [launch evidence](LOCAL-SMITH-NEXT-INSTALLER-LAUNCH-EVIDENCE.json).

PASS: locked formatting; warning-denied Clippy all targets with GUI; GUI build; complete serial native suite (259 passed, 2 ignored finite fixture entry points); isolated `make installer/ui-test` (9 GUI tests); isolated `make installer/test` (native+GTK gates and interpreter-free CLI release/runtime checks). Parent independently reran `cargo fmt --all --check`, `cargo clippy --locked --all-targets --features gui -- -D warnings`, `cargo test --locked launch:: -- --test-threads=1`, and final `DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make installer/test`, all exit 0. Final gate log: `next/installer/build/launch-final-installer-test.log`. Earlier log paths/hashes are retained in the evidence file.

Failure retained: initial parallel native suite failed existing maintenance test `captured_fake_oldvoice_runtime_stop_then_reload_and_stop_failure_fence` with “Artifacts or destinations changed after confirmation.” Its focused rerun, complete serial suite and isolated gates passed without changing/weakening that test. No claim of a root-cause fix for this historical failure.

Fresh independent source review found no actionable launch implementation blocker. Its evidence/documentation closure requests were addressed by the canonical updates, source/log hash record and parent final isolated gate. Final independent review verified all five current source/fixture hashes against its prior review, all four retained log hashes, and the final isolated log's 259 native / 9 GUI passing tests and successful runtime-gate summary. Both evidence/documentation findings are closed: PASS for this bounded source assurance scope, with no outstanding actionable findings. This is not release/deployment authorization. No normal-host launch/install/service/model action, commit or publication occurred. No full recipient package export was rebuilt for this slice; installed graphical readiness and broader release acceptance remain unverified.
