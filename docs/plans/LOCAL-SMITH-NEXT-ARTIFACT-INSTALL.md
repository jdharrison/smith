# LOCAL-SMITH-NEXT-ARTIFACT-INSTALL

## Intent and authority

User requests `make install`, `make client/install`, and `make service/install` from `next/`: build with Dagger and install artifacts into accessible user locations. Source implementation and isolated temporary-root tests are approved scope; normal-host installation is not executed by the agent. Base checkpoint `04b72f5`, existing bootstrap branch unchanged. Elevated/critical filesystem/desktop-publication work; no commits, remote effects or accepted-risk approval.

## Reconciled proposal

Independent architecture and producer/designer inputs require build/install/lifecycle separation, complete paired artifacts, exact effect confirmation, no unknown overwrite, source/destination revalidation, safe atomic publication and preserved partial-failure evidence. Commands run from `next/`, not repository root.

Make invokes Dagger builds, then a native developer-only Python stdlib installer at `next/dagger/install.py`. Python/Dagger are not desktop runtime dependencies. The installer never invokes service control helpers or a service manager, and does not register/start/enable units, use sudo, change login/lingering, download providers, or touch model/config/conversation/legacy data.

Defaults:
- Client complete bundle: validated XDG data `smith-next/client/releases/<content-digest>`; managed relative `current` symlink selects an immutable verified release.
- Desktop entry: XDG data `applications/dev.smith.smith_client.desktop`, Name=smith, absolute installed Exec/Icon, existing GTK identity.
- User command: HOME `.local/bin/smith-next` (not legacy `smith`), exact managed wrapper to installed client; no PATH edits or automatic launch.
- Standalone service: XDG data `smith-next/bin/smith-service`, exactly the existing debug-boot installation location. Binary-only publication; registration/start remain separate desktop actions.

Data base root must already exist and be safe; explicit installation may create only named subordinate destinations. Match runtime safe ownership/modes/restricted absolute paths, UID-zero refusal, no-follow ancestors and regular-file single-link checks. Only the installer-owned verified relative current symlink is accepted. Content manifest covers paths/types/modes/bytes; existing releases are verified, not trusted by name. Unknown files/links and differing existing service binaries block before switching current. No implicit live service upgrade/restart. Aggregate install verifies standalone and bundled helper byte identity and preflights all destinations before writing.

Installer `--dry-run` is read-only and never builds or creates locks/directories. Interactive confirmation displays artifact identity, exact destinations and effects. `--yes` is explicit approval of the displayed plan, not a safety bypass. Revalidate after confirmation and under a bounded installation lock; publication is no-clobber for the service and unknown files. Stage on the destination filesystem, sync, preserve old releases and atomically switch current last. Multiple paths are not a global transaction; report completed effects and permit safe idempotent retry, never pretend rollback or delete unknown data.

## Reviewed concurrency refinement

Independent assurance reproduced late unconditional selector overwrite and missing target/dependency reverification. Remediation replaces selector overwrite with Linux atomic exchange, checks the displaced object's captured identity, and retains it on success or conflict; it never deletes unknown displaced data or attempts unsafe rollback. All target dependencies are verified immediately before selection and before success. A conflict may leave `current` selecting the new release and returns failure with exact retained-path/partial-effect reporting. This refines “atomic switch last” without claiming filesystem compare-and-swap against arbitrary same-UID writers. The cooperative lock serializes participating installers only. Cleanup of retained selectors/stages is separately scoped. Independent re-review is required before final source verdict.

## Installer tooling relocation

**User-requested source organization refinement:** move the installer and its tests from `next/install.py` and `next/tests/` to `next/dagger/install.py` and `next/dagger/tests/`, byte-for-byte. Make install targets invoke the relocated native script; Dagger installer tests receive only the `dagger` subtree and still copy only installer/test inputs. This is a routine relocation, not an authorization/capability/runtime change: confirmation, destinations, no-clobber publication and native host execution remain unchanged. No host home mounts or containerized installation are introduced. Update current command documentation; historical digest/evidence records remain unchanged and are explicitly historical. Validate 43 isolated tests, Make route expansion and aggregate `make test`; obtain independent scoped review. No real installation, runtime effects or commits.

## Traceability and gates

| Requirement/acceptance | Task | Evidence |
|---|---|---|
| R1/A1 build→confirmed artifact install, no lifecycle effects | T1 Make targets and native installer | Dry-run/decline/no-manager tests, Make route checks |
| R2/A2 complete menu/icon/command-accessible client and compatible helper | T2 release/launcher/shim publication | Bundle manifest and generated launcher tests, both component orders |
| R3/A3 safe roots, non-root, no unknown overwrites/races | T3 filesystem/approval revalidation | Symlink/hardlink/changed-input/concurrent/conflict tests |
| R4/A4 idempotency and truthful partial failures | T4 staging/publication reporting | Fault-injection/retry tests, unchanged existing service |
| R5/A5 all quality under /test | T5 Dagger installer suite | make test and make build; fresh assurance |

Plan → develop → self-check → fresh independent assurance/adversarial review. No normal-host installation, launcher launch/cache command, unit change or GUI effect is performed in validation. Menu/dock integration and real deployed app/service remain separately approved live-host gates. Do not silently replace legacy binaries/entries or restart work. Preserve code/user data and prior client releases for rollback; removal/uninstall/upgrade is separately scoped.
