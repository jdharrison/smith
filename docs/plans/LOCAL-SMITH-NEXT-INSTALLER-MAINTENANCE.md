# LOCAL-SMITH-NEXT-INSTALLER-MAINTENANCE

## Intent and approval

2026-10-13 user explicitly approved implementing the existing-installation flow Install / Upgrade / Manage models / Uninstall after QA found no actionable existing-install path. Local source + isolated synthetic/fake manager/GTK validation only. No host service/install/state changes, weights, live GUI/user manager, GPU/login/linger, commits or remote effects. Prior deployment approval consumed; runtime operator effect-specific confirmations are separate. Preserve feature/autonomy base04b72f5 and extensive dirty work.

## Reconciliation

Read-only role reconciliation588e9b3a-6556-4ba1-88f9-575368e41576 examined existing control/manager/artifact/filesystem/UI and reported no Stop/unregister support, candidate-bound ownership preventing old-service maintenance, distinct publisher/Core locks and absent quiescence/enablement facts. This is combined role analysis, not independent triangulation; fresh focused assurance follows source self-check.

Adopt explicit maintenance rather than force-overwrite. Ownership inspection verifies installed standalone == helper in manifest-verified selected release and exact deterministic unit/menu/command; never executes old code, never treats filenames/comments alone as ownership. Existing Install/Make candidate mismatch and no-clobber guards remain. No new provenance receipt prerequisite excluding old voice installations. Historical retained releases may contain voice files and are not reinterpreted with current package allowlist.

## Bound contract / traceability

R1 Four focused actions: Install (current reviewed publisher), Upgrade, Manage models, Uninstall. Initial window gives clear main-page blocked reasons, no effects on opening. Upgrade/Uninstall do not require candidate-equal installed bytes; uninstall does not depend on valid candidate payload. No automatic installed helper execution or model download.

R2 Existing ownership: safe roots/no-follow/single-link/modes; exact selected release/relative current; installed standalone/helper equality; exact generated launcher/shim; exact deterministic unit and FragmentPath, no on-disk or manager drop-ins/transient/foreign/unknown enablement. Independently inspect current executable for running unit against captured installed bytes, not candidate. Missing/corrupt app data is preserved; lifecycle authority does not parse/reset it. Incomplete/foreign/modified ambiguous objects block conservatively, with reason.

R3 Source service library adds narrow maintenance Stop/unregister with original action/identity/path/expiry-bound prepared consent. Fixed bounded SystemManager stop, extra read-only maintenance enablement/cgroup facts, verified stopped/PID0/group-quiescent and socket-absent before replacement/removal. No rawPID kill, detached fallback, stale-socket deletion, enabling/disabling or lingering. Unit removal retained under fresh no-clobber name before reload; verify registration absent.

R4 Locks: installer installation lock -> Core data-directory lock (held across Stop/publication/unregister) -> runner lease after stopped. Service exposes typed maintenance lease and narrowly validated locked apply path, preventing recursive flock deadlock. Revalidate lock paths and reviewed objects after consent/under locks. Lifetime guard cannot be forged by executable parameters. Participating operations serialized; no hostile same-UID CAS/atomic global transaction claim.

R5 Upgrade: read-only capture old/candidate; separate explicit upgrade/artifact confirmation, separate Stop if needed; preserve old binary/unit/selector/launcher metadata/content before replacement (states/models untouched in place). Stage verified candidate pair; atomically exchange standalone with retained displacement, select candidate via existing reviewed publisher discipline, verify pair/launchers. Separate registration/reload as applicable and Start consent with fresh managed identity/bridges. Declined Start reports artifacts upgraded/service stopped, not model readiness. Failed stop/quiescence prevents publication; failure preserves accepted/uncertain stages/backups, no rollback/retry/reprepare. Persist compact maintenance evidence, no crash replay.

R6 Uninstall: separately confirmed exact removal review, Stop if needed, original-token unregister/reload confirmation; only after absent registration remove active menu/shim/current/standalone via held-dir no-clobber preservation and moved-identity checks. Keep ALL app/Core state/preferences/history/models/releases/backups/lockfiles/stages and unrelated objects; no purge/recursive delete. Explain retained disk usage. Scope-specific completed/partial/uncertain reporting.

R7 GUI retains single owned worker, bounded typed channels, original request IDs/deadlines, frozen operation, joined cancel/close, sticky cancellation requiring reopen. Expose direct Upgrade path from old-install conflict. No recipient yes/force; artifact-only CLI unchanged. GUI and optional explicit CLI maintenance use same native orchestration.

## Implementation API handoff

Service new maintenance module with ExistingController<M> roots/manager/captured binary_sha256; inspect/prepare/apply Stop or Unregister independently of candidate. MaintenanceLease retains existing Core data-directory flock; locked apply accepts only lease bound to same roots. Existing Controller can perform candidate registration/start under that lease with original token checks. Manager optional maintenance methods default refusal for old injected fixtures; SystemManager implements fixed commands/strict facts/quiescence. No wire Action changes needed for desktop.

Installer new maintenance module: Operation::{Upgrade,Uninstall}, Prompt::confirm(title,&[String],deadline:Instant)->Result<bool>, Prompt::progress(message)->Result<()>; Outcome{complete,message,details}; Native::new(bundle:PathBuf)->Result<Self>, execute(operation,&mut dyn Prompt)->Result<Outcome>. Typed generic manager/roots injection for isolated tests. Maintenance module owns locks/review/preservation/controller/publication; normal publisher remains guarded. No GUI dependence.

Wizard adds explicit command Upgrade/Uninstall routes without needing artifact review success. Worker calls maintenance with its exact-ID consent/progress adapter and cancellation; final Report scopes Upgrade/Uninstall. UI offers four clearly labeled actions and progresses one at a time; Upgrade/Uninstall are operable for independently owned old installation and refuse with visible reasons otherwise. Native safety remains authoritative; no hidden bypass.

## Write scopes / gates

Service author: next/service/src/{maintenance.rs,manager.rs,control.rs,lib.rs} and service tests only. Installer maintenance author: next/installer/src/{maintenance.rs,artifact.rs,fs.rs,lib.rs,cli.rs} and maintenance/CLI/artifact tests only. GUI author: next/installer/src/{wizard.rs,ui.rs} and wizard/model/GUI tests only. Parent coordinates frozen APIs, integration, canonical docs and evidence; no overlapping writes.

Focused locked fmt/Clippy/tests; oldvoice-pair mismatch adoption, foreign/link/drop-in/enablement/launcher rejection, no old-helper execution, manager stop/remainingchild/sockets failure, consent drift/expiry/replay, lock contention, staged backup/exchange/race/partial failures, uninstall ordered effects and preservation sentinels, declined Start and scope reporting, GTK actions/blocked reasons/cancel. Aggregate make test build after stable integration and one fresh focused assurance/adversarial review. Real installed upgrade/uninstall/GUI/model inference and clean-host/release/SBOM remain separate evidence/approval gates.

## Final source validation and handoff

Parent final `make test build` with DAGGER_NO_NAG=1/no cloud token/plain progress (cwd next) exited0 and exported the paired service/client/full installer.101 service/238 installer/5 actual isolated Xvfb GTK/69 Flutter/22 package tests, locked fmt/Clippy/no-Python CLI gates. Service/Flutter same-input passes reused cache; not clean-clone/hermetic proof. Initial GTK failure was one stale old-upgrade-wording assertion, corrected without production changes.

Reviewer2de5830d-079f-47b3-b7d8-7b5e56fbe6dc independently inspected the critical paths and ran57 offline tests PASS with no blocking finding, then confirmed source continuity, final candidate hashes/pair equality/checksums and canonical updates. Focused source/fake PASS only; live/release NO-GO without separate approval/evidence. [Evidence](LOCAL-SMITH-NEXT-INSTALLER-MAINTENANCE-EVIDENCE.json) and next/VALIDATION.md bind the final candidate.

Implementation provides GUI maintenance, not additional CLI flags. Backup of exact old artifact/unit/selector metadata/content occurs after accepted artifact review and before separately consented Stop; live state/models are kept untouched in place, not claimed snapshotted. Uninstall preserves inactive objects rather than purging disk contents. No actual installed-host/service/state/GUI/weights action or remote mutation occurred. Operator-owned QA uses the full package and Upgrade for old differing installations, then explicitly Manage models.
