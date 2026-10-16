# Smith Delivery Process

## Status

**Normative target process.** Follow the stricter documented authority boundary when runtime/repository enforcement is incomplete. Configured checks, source-limited review and historical passes are not implemented guarantees, hosted CI execution or human approval. Root `VALIDATION.md` records actual source/artifact-bound outcomes, failures, skips and outstanding gates.

## Work record and branch policy

Every change needs bounded objective, owner, requirements/acceptance criteria, risk class, exclusions and evidence expectations. Use Jira when its optional integration is enabled; otherwise an approved local task/plan. GitHub is optional for local work; when used it owns commits, PRs/checks/reviews, merge/tag/release records.

| Mode | Ordinary branch |
|---|---|
| Jira feature/defect | `feature/<KEY>-<slug>` / `fix/<KEY>-<slug>` |
| Local feature/defect | `feature/<slug>` / `fix/<slug>` |
| Approved emergency | `hotfix/<slug>` or `hotfix/<KEY>-<slug>` |
| Approved release preparation | `release/v<version>` |

Ordinary work starts from remote `main`; no `develop` exists. `feature/autonomy` remains the one-time bootstrap exception. Do not rename/rebase/force-push it merely to conform. Its approved root-promotion implementation does not include commits, push/PR, merge, tag or branch deletion unless separately authorized. Prefer history-preserving normal merge for the bootstrap cutover; retain local/remote branch and checkpoint ancestry unless named deletion is separately approved.

## Canonical lifecycle

Bounded intent → independent producer/architect/designer analyses → planner reconciliation/traceability → resolve blockers → scoped local implementation → developer self-check → fresh independent assurance/adversarial review → human signoff → exact approved external action → separately approved merge/release/deploy/completion where required. No earlier stage substitutes for a later gate. Current chat does not implement this general runtime orchestration; this is the required development method and future product target.

## Triangulation protocol

Producer defines scenarios/blocking acceptance; architect evaluates fit/trust/operations/rollback; designer evaluates visibility/recovery/accessibility. Planner retains `REQ/AC → task → expected evidence`, assumptions/owner/expiry, disagreements and resolutions/blockers. No voting away scope/safety/architecture disagreement. Developer implements the reconciled plan, not new authority. Fresh-context assurance receives requirements/base/diff/evidence/policy, not builder private reasoning. Adversarial review attempts reproducible authorization/injection/path/network/secret/replay/concurrency/cancel/recovery violations. Release judge recommends from evidence; only human accepts risk/approves effects. Authoring session/role cannot finally approve its own consequential work.

## Evidence contract

Record work ID, requirements/risk/exclusions; repo/branch/base/target/diff/interface identity; exact command/scenario, environment, timestamp, exit/result, artifacts/digests; self-check, independent review/adversarial findings/risks; compatibility/rollback; exact approval binding/outcome and external results. Missing, unverifiable, stale or narrative-only required evidence is failure, not PASS. Preserve intermediate failures, controlled fixture ignores and skips; counts alone do not bind a candidate. Independently retain consequential artifact/evidence digests outside owner-writable checksum packages. Never retain secrets/raw private transcripts or sensitive paths.

## Required gates

Select by impact and bind to a **stable final source snapshot**. Formatting, compiler/runtime errors, failing required tests, unresolved critical warnings and missing required evidence block the relevant acceptance/release. Independent review cannot waive them without explicit human accepted-risk authority, and must not call an unverified gate passed.

| Area | Required evidence |
|---|---|
| Rust service/installer | Formatting, warning-denied Clippy, locked build/tests; both independent workspaces. |
| Flutter | Dependencies, formatting/analyzer, unit/widget/compiled-integration tests, Linux build; graphical evidence separately. |
| Protocol/storage/authority | Strict compatibility/corrupt preservation, negative authorization, duplicate/expiry/retry/cancel/restart and publication-failure tests. |
| Package/build tooling | Actual native ELFs, paired helpers, exact allowlist/manifests/modes/checksums, runtime dependencies/no-Python CLI and archive round trip. |
| Host/network integration | Exact candidate/account/root/manager/socket/cgroup/egress/driver checks in separately approved environment, never inferred from mocks. |
| Release | Clean-source reconstruction/build binding, license/icon/SBOM/provenance, compatibility/restore, independent reviews and exact human approval. |

### Root development and component gates

**Implemented tooling:** run Make from repository root. `client/`, `service/`, `installer/`, `dagger/` are explicit inputs; service and installer keep separate lockfiles/workspaces and sibling dependency. No root `cargo test` product gate, retired sidecar/reference input, broad repository/home mounts or implicit source discovery. Make stays thin; logic belongs in component scripts/Dagger. `make test` combines quality/tests; no separate public check target. `make build` exports service, paired client and complete recipient package. Namespaced `client/test`, `service/test`, `installer/test`, `installer/ui-test`, `package/test`, component builds and `installer/binary` retain semantics. `installer/run` builds/verifies then intentionally launches host GTK, **not a validation gate**. `client/run`, install targets and GUI launches are operator effects, not routine source-test shortcuts.

Make explicitly selects `dagger --mod dagger`; reviewed `DAGGER` executable override may select the checksum-verified official **v0.21.9** CLI matching module engine version. Retained `dagger.toml`/`dagger.lock` compatibility files do not establish effective image-lock enforcement/hermeticity. Actual module discovery, CLI/server identity, dependencies/downloads/cache/engine effects and task results must be recorded. No automatic beta fallback; cloud trace/token sharing is not implicit. Dagger is neither recipient dependency nor app lifecycle manager.

Native quality tests run on actual Ubuntu24.04 UID/GID1000, Rust1.98; release builder uses Rust1.98 Bookworm. Toolchain preparation as container root does not mean tests run root. Isolated Xvfb GTK tests cover optional `gui` feature; separate Ubuntu non-root CLI runtime executes actual release ELF help/version/artifact dry-run with Python absent and no useful PATH/build tools. Declared MSRV is not a minimum-toolchain compile result. Builder package tests consume actual CLI/GUI via `SMITH_TEST_INSTALLER`/`SMITH_TEST_INSTALLER_UI` and synthetic payloads, never silently substitute header fixtures/skips. GUI display-free help still needs shared libraries. ELF/loader/dependencies, helper equality, exact UI/font/icon/notice/runtime paths and rejected unknown payloads are required. Current UI asset contract is24 exact nonempty paths, not a permissive wildcard.

### Multi-chat validation and migration boundary

Require disk1/2/3→4 preservation, inert old voice fields/results, full-u64 configuration echoes, corrupt-byte preservation, exact local v1/v2 projections/receipts and correlated errors. Real compiled Rust↔Dart gates must exercise domain/transport, not only Dart fakes. Cover isolated context, atomic first submit/title, four-job FIFO/one-per-chat/one worker, queue expiry, completion/state/frame reservations, terminal-only eviction/tombstone exhaustion, interrupted restart without replay, publication fencing, target/revision-bound lifecycle and logical-delete residue. Old receipts cannot regress metadata; poll/admission uncertainty retains accepted identity. Cover per-chat drafts/reading, blank promotion/hidden panes, keyboard/IME/focus/narrow enlarged text and provisional/expired-request recovery. Source tests do not migrate installed data; old binaries reject current state. Live rollout needs approved pre-migration snapshot and tested paired-binary restore, with exact keyring references; logical deletion keeps backups.

### Next cloud-provider validation and compatibility

**Implemented definitions, not an automatic current PASS:** app3/disk4/Core2, fixed catalogs/transport in [API contract](plans/LOCAL-SMITH-NEXT-CLOUD-API-CONTRACT.md). Require v1/v2 exact local compatibility, v3 envelopes/delegation/nullables, `UPGRADE_REQUIRED` rather than hidden/misrouted cloud state, provider-aware installer local management after cloud use, unknown historical provenance and no mutation/version fallback. Paired catalog-aware components are mandatory even when schema numbers are unchanged.

Use synthetic capabilities/private session fixtures/TLS servers, not real secrets or normal bus/vendors. Cover exact7/8 model ordering/labels/contexts/uniqueness/unknown rejection/every-entry request/final/error fixtures; bounded raw16KiB headers/1MiB bodies/64KiB final text/context;8192 total including reasoning;180s original deadline including keyring/connect≤10s. Test certificates/hostnames, proxy suppression, redirect credential isolation, slow drip/encoding/overflow, one attempt/no replay/fallback, expiry and pre-send publication failure. No endpoint/trust production override. Opening/cached reads/selection/Save/Retry/migration/replay cause zero vendor calls; probes read keyring only; Validate is one fixed no-history request.

Keyring gates must exercise production EXTERNAL authentication/encrypted negotiation with independent peer derivation, leading-zero padding, UID/owner drift, malformed/stall/rejection, encrypted item exchange, no-auto-start/unlock/Prompt, locked/missing/ambiguous/identity/timestamp drift, exact MIME and optional GNOME schema allowlist before/after read. Canary secrets must be absent from disk/fingerprints/errors/events/output/context. Save uncertainty preserves original identity/inactive items, no retry/adoption/cleanup. Both-provider cold saved-only/adverse-observation attempts require no preliminary probe/validation, but fresh exact-generation retrieval; inaccessible key yields zero HTTP/`not_sent`, invalid provider key one attempt. Successful checks mark only exact model/generation. Metadata advisories cannot weaken consent or unknown-operation reservations.

Installer gates cover masked/hidden input/terminal restoration, independent keys/save/check consent, optional skip/failure preservation, narrow `--providers` mode and service-domain/GTK round trips. Flutter gates cover same-chat route changes, unavailable historical route, immutable accepted context, new-generation/retained-destination consent including intervening history, stale dialog/origin fencing, local filter no effects and400×400 enlarged-text recovery. Queue Resume must not silently approve export. **Compiled Rust↔Dart must exercise schema3 routed local and injected fake-cloud consent/admission/migration/reattach**, saved-only attempts and timestamps, not just older/local socket fixtures. Explicitly report `SMITH_TEST_SERVICE` fixture skips; skipped integration is not acceptance. Concurrent changes require routed-queue/consent regressions on one stable snapshot.

### Saved message timestamp validation and migration

Disk4 sidecar must preserve undated history, strict nullable bounds/alignment, admission/completion semantics, atomic trim/delete/reservations, replay/restart/operation-eviction stability and failed/interrupted no-pair behavior. Role/content/provider/context-hash and v1/v2 results stay unchanged. Installer compatibility must test disk4, not stale disk3 assertions. Client tests bind instance/conversation/history revision/count, omit unavailable/invalid metadata without replay, render event-local seconds/offset and preserve focus/reading at narrow/2x text. Run real Rust↔Dart timestamp fixture. Source fixtures do not authorize installed migration/restore or claim model-only latency.

### Client-side message-queue validation

Require exact fake-API counts/order, immutable attempts, predecessor success **plus saved history**, unknown acceptance/no replay, failed-head explicit safe Retry/Skip, foreign-history/config/instance changes, stale revision/origin actions, hidden/shared-pane/blank identity, UTF-8/count/long-wait bounds and close-hold/disposal races. Test Queue/Enter/IME, truthful local-versus-accepted states, blocked dependency preservation, inactive dialogs and400×400 enlarged-text close/recovery. Normal native Close/Quit needs explicit true Dart reply; missing reply stays open. Forced death may lose drafts; accepted work detaches. No live vendor/keyring/manager/model needed; real GTK close/IME/provider timing remains separate.

### Desktop presentation, persistence and windows

Require analyzer/tests/Linux build plus current real-font goldens, responsive keyboard/focus/recovery and stale-menu capability fences. Four retained panes share one controller/API/observation/queue, no duplicate worker. Test scroll anchors through resizing/hiding, hidden work, selection repairs and current callbacks (not shallow screenshots). Safe workspace preferences need strict schema/4KiB/path/mode/no-follow/flock/atomic-save/preservation and session-only fallback cases; restoration never resumes work. New-window tests must verify resolved executable/exact flag/no shell, production-only launcher and **no preference-store construction/access** for the new process, with independent local queues/close guards. Mock spawn is not real native lifetime/multi-monitor proof. Loader/entrance clocks/reduced motion cannot change readiness/action authority. Current local feature reviews remain under `client/` and retained evidence; native window-edge, IME, display/accessibility QA is separate.

### Current next-generation login-boot validation

Require strict Core2 independent facts, exact enabled+owned wants link, deterministic install stanza, fixed separate Enable/Start/no `--now`/`loginctl`, original expiring tokens and no detached fallback. Fake managers/private roots and real compiled Rust↔Dart Core/app fixture must cover root refusal, unsafe roots, managed bytes/unit/MainPID/socket/peer drift, duplicate locks, helper cleanup and pending-reload repair. Boot tests: serialized15s observation, separate sync bounds, no chat controls before sync, exact enabled+safely-stopped Start only, separate confirmation, no automatic effect/replay and accessible/reduced-motion repair. Installed login/logout/reboot/user-manager behavior needs separately approved QA; fake-manager evidence is explicitly simulated.

### Next-generation artifact install validation

Root Make install targets build before confirming native `--artifacts-only` publication; direct dry-run does not build or write. Only artifact mode accepts `--yes`, with no safety bypass or recipient/model flag mix. Require byte-identical canonical release manifests, full bundle/menu/icon/shim and paired helper; held-FD/no-follow/mode/owner/link/bounds, no-files/no-locks decline, post-consent/under-lock revalidation, verified cooperative lock, concurrent no-clobber/sync and final dependency checks including service-only/selected client. Selector exchange retains displacement and may leave current changed on conflict: truthful partial failure, no rollback/global CAS/cleanup claims. Normal-home publication/launch/manager is separately approved, never substituted for fixtures.

### Current bundled runner validation

Require independent SHA-pinned CPU/Vulkan archives, exact members/licenses/ELF dependencies/`$ORIGIN`, CPU without Vulkan loading, held-model FD/path replacement, per1MiB hashing cancel/deadline checks, lease/socket/peer/process ownership, app-owner teardown fence/cancel/join, context/final-output/token bounds and idle cleanup. Startup fakes must model pinned verbosity/decorations and exact CPU/Vulkan flags; cover full/partial/zero/missing/malformed/duplicate/every-byte/unterminated/overflow records, fixed post-health window/failure/cancel/deadline, same-lease/original-deadline single cleaned startup fallback and no post-acceptance replay. Sanitized fallback reason must remain nonblocking/in-memory/not current-mode wire state. No GPU passthrough/host probe/weights/live inference for source gates. Real CPU/GPU allocation/offload/performance and recipient compatibility remain separate.

### Focused installer/model wizard validation

Require lazy packaged-service Controller construction after artifact acceptance, original60s Core tokens/no reprepare,4096-byte60s slow-drip/EOF/interrupt/decline consent and incomplete/backpressured-output preservation. No total filesystem/fsync/accepted-work cancellation guarantee. GUI requires two landing actions/read-only opening/Checking state/focused controls, no normal Logs/Details surface but exact effects/blockers/uncertainty/preservation/next action visible, unchanged separate register/Enable/Start defaults. Managed model/provider paths must not depend on install-preview success/expiry or bypass their own identity/expiry checks.

Test frozen recipes/opaque artifact review/owner/request binding, bounded critical FIFO, latest-only byte telemetry/stale-boundary invalidation/default strict CLI sink, capped report beginning with outcome, joined sticky cancellation/reopen and scope-specific completion. Curated chooser must equal catalog; unsupported saved model inert, no free-form/import. Fake exact download/preference/check/remove tests cover TLS destinations/size/SHA/staging/marker-last/deadlines and async runtime factory ownership, no retry/resume/deletion. No-history verify and selected/loaded removal refusal, lock order/partial orphan preservation remain. Appearance tests cover sealed memfd/font/PNG failures before worker, notices/licenses without display and unchanged GTK-free CLI. Current fixture captures are not real transfer/graphics/redistribution acceptance.

### Existing-installation maintenance validation

Require ownership independent of candidate mismatch/old voice contents, no old-helper execution, candidate-independent uninstall, ordinary Install no-overwrite; exact unit/launcher/drop-in/enablement/cgroup/runtime facts and foreign/unknown refusal. Test original consent expiry/drift/replay, installation→Core→runner leases/root/path drift, captured process/group descendants/sockets quiescence, Stop/reload/Enable/Start failures, exact static migration/current-enabled no-churn/current-disabled Enable and disable-before-unregister. Verify backup/exchange/displacement/partial preservation and unchanged state/model/release sentinels. GUI decline/cancel/expired-preview paths must retain request/consent/outcome truth without purge/retry/rollback. No real host maintenance authority follows.

### Explicit installer launch validation

Only joined verified Install/Upgrade exposes Launch, never render/completion/Close/model-only/Uninstall/failure. Require selector/manifest/executable drift refusal read-only/no managers, production installed-client resolution, pending/duplicate/Close/error-versus-install-result handling. Finite synthetic executable tests cover argv0/cwd/null stdio/direct exec/setsid/immediate reporting/background reaping and independence from parent lifetime. Real installed desktop launch/readiness is separate approved graphical QA.

## User-service rollout and evidence

Sandbox source gates precede separately approved real-host validation; **never bypass missing fake capability with the live manager**. Source tests use temporary roots, injected fake managers/providers, private socket/TLS/keyring fixtures and captured finite children; no normal home/session bus/GPU/model/API or ambient service access. Builder dependency/cache downloads are explicit approved development effects, not model/API authority.

For real rollout, bind exact candidate hashes, account, paths, rendered unit, commands/effects and expiry. Preserve pre-migration state, compatible paired binaries and keyring references; test restore, not binary-only rollback. Record unit load/active/substate/enablement/MainPID, exact link/bridge/socket/executable identity, cgroup quiescence, loopback/private transport, login policy/linger observation, interruption/no replay, sanitized diagnostics/timestamps and rollback. Test login separately from pre-login/logouts/reboot. Smith never mutates lingering; any manual host policy change requires its own human scope/rollback. Historical one-time installation approvals are consumed, not standing authority.

## Approval boundaries

| Action | Required authorization |
|---|---|
| Discovery/proposal | `DISCOVERY_READ_ONLY` or explicit user task; no mutation. |
| Scoped local edits/branch/install/test | `LOCAL_PLAN_APPROVED`; commits require explicit inclusion. Live-host installation still needs exact named effects. |
| Named push/draft PR; enabled Jira review/link/comment mutation | `REVIEW_REQUEST_APPROVED` bound to work record/repository. |
| Named PR/set merge | `MERGE_APPROVED`, with fresh reviews/checks/base/head. |
| Release branch/tag/GitHub Release exact version/commit | `RELEASE_APPROVED`. |
| Exact artifact to named environment | `DEPLOY_APPROVED`. |
| Ticket completion, destructive operation or accepted risk | Separate explicit human scope; merge/general local approval is insufficient. |

Bind work ID/repository/branch/base/target/effect/destinations/expiry; stale/mismatched/general prior approval or model instruction is invalid. Recheck evidence, review, revision and scope immediately before effects. Agents cannot self-approve. Push/PR, merge, tag/upload/release and deployment are distinct; workflow-triggered disclosure must be included in push/tag authority.

## Optional Jira and GitHub synchronization

When enabled, include Jira key in branch/commit where compatible/PR; draft PR links criteria, dependencies/merge order, evidence/risks/rollback, builder/reviewer and pending signoff. Jira gets concise links/milestones, never secrets/raw transcripts; completion requires approved cross-repo conditions. When disabled retain equivalent local records; no invented links or mandatory ticket. Server protections/labels do not prove reviews/status checks are configured: inspect and enforce documented gates manually without unapproved settings changes/admin bypass.

## Release and deployment

Only approved release preparation; exact version/commit, clean-source required gates and compatibility. New releases use a new **annotated `release/vMAJOR.MINOR.PATCH`** tag; never move/reuse a released version. Package publication, GitHub Release and deployment are separate effects. Publish only the exact reviewed archive, not a rebuild/overwrite. Root retirement does not restore the obsolete automatic multi-platform publisher. Legacy commands/release candidates are unsupported; prior source/host observations remain historical, not promoted-product acceptance.

### Native installer candidate automation

**Implemented workflow/helper configuration; required current-root gate policy:** `.github/workflows/ci.yml` targets PRs to `main` and pushes to `main` on Ubuntu24.04 x86_64. Its accepted gate set is pipeline-helper/workflow invariants, native aggregate tests/full installer build and package verification. Pre-promotion source paths and retired root-Cargo checks are not valid current-product gates; reconcile any remaining workflow/helper references before accepting the cutover. Use root `make test installer/build` and `python3 -B dagger/package.py --root installer/dist --verify`. `.github/workflows/release.yml` delegates pushed `release/*` **tags** to the same reusable CI with candidate mode; release branches/main merge alone do not trigger candidates. Both candidate checkouts must explicitly select full event tag ref to preserve annotation.

Before expensive build, `scripts/prepare-installer-release.py` validates stable canonical version, annotated tag, exact expected event commit==HEAD, membership in fetched `origin/main`, clean tracked source, matching installer/service manifests **and both lockfiles**, and Flutter version (numeric `+build` retained but excluded only from release comparison). Recheck source before acceptance. This is identity validation, not human approval, remote ref immutability or proof a supplied package was built from that source.

After gates: verify full paired `installer/dist/`, create single-root `smith-installer_v<version>.tar.gz` preserving modes, bounded safe member-table inspection/manual extraction into fresh private staging, content/mode equality and package reverify. Reject traversal, duplicates, links/devices, unsafe modes, missing parents and size/count overflow. External archive `.sha256`, `build-record.json` (tag object/commit/tree/component versions/archive/package identities/run URL), validation log and Dagger version stay **outside** strict recipient payload. Actions artifact `smith-installer_v<version>` retains them14days; main/PR never uploads distributables. Failed gates must block candidate upload.

Pinned actions, read-only `contents`, credential-free full checkout, bounded jobs/ref concurrency and checksum-pinned Dagger remain. Neither workflow creates tags/GitHub Releases/deployments. **Upload is remote disclosure:** approve exact source/tag push, destination/visibility/retention and redistribution rights before activation. Hosted execution/settings/retention and recipient acceptance are not established by static/helper/synthetic-Git tests. Checksums supplied beside artifacts are not authenticity; approval/provenance must independently anchor digest. Effective retained lock enforcement/hermetic clean-cache builds, clean-source provenance, real manager/graphical/model/provider/restore evidence, supplied-icon rights/licenses/SBOM and exact publication approval remain required. Existing old lightweight `release/v0.1.0` is not eligible; remote/version availability must be resolved by human, never retagged as shortcut.

Current source-limited pipeline outcomes/review: [evidence](evidence/installer-pipeline/README.md), [independent review](evidence/installer-pipeline/REVIEW.md). They are not GitHub-run, cutover or release acceptance.

## Root promotion and history retention

The [approved local promotion record](plans/LOCAL-SMITH-ROOT-PROMOTION.md) defines source-layout/support retirement, not installed identity migration. Current policy is direct; superseded stages/plans/reference bytes use immutable Git links at `168865b0422b787f08ab33e89d052cd4adbd11f7` or verified later pre-cutover checkpoints. [History](HISTORY.md) records relocation/retrieval; the machine-readable move/retirement ledger must bind every tracked disposition/hash/mode/source commit, active contract/evidence closure and verified ancestor retrieval. No copied archive, history rewrite/retag/force-push or blanket cleanup. Untracked/ignored work/logs/exports are not in Git; preserve them unless exact disposable-cache clearing is separately approved, and never call them checkpoint-recoverable.

Final promoted-root self-check, after concurrency settles:

```sh
make help
env DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make test build
python3 -B dagger/package.py --root installer/dist --verify
python3 -B -m unittest discover -s scripts/tests -p 'test_*.py' -v
git --no-pager diff --check
```

Also lint workflows/shell, check active doc links, source/package identities, history retrieval and unchanged runtime `next` literals; actual-package helper fixture must be enabled/recorded rather than silently skipped. Retain failures and controlled ignores. Prior1,262-test passes are **historical**, not validation of relocated paths. Obtain fresh full-cutover/full-branch independent assurance after remediation, then stop at unapproved commit/push/PR/merge/deletion boundaries. Integration readiness is not release/deployment readiness.

## Current implementation deltas

Broad runtime orchestration/approval automation remains target/deferred; process rules are manually binding on agents, not advertised as service-enforced delivery authorization. Host/session/provider/model/graphics/provenance and publication gaps remain explicit. Superseded pipeline/legacy evidence is accessible through [pinned prior Process](https://github.com/jdharrison/smith/blob/168865b0422b787f08ab33e89d052cd4adbd11f7/docs/PROCESS.md), not current release authority.
