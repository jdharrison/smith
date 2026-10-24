# LOCAL-SMITH-PROJECT-REPOSITORIES

## Approved intent and local boundary

The operator explicitly approved proceeding after agreeing to structured repository objects with `name`, `url`, and `branch.{base,work,types.{feature,fix,release,hotfix}}`. Base is the main/stable line; Work is the shared working line. Both may be `main`. Type values such as `feature/*` are configuration, not a workflow executor. This iteration stays on Projects before any Dev page.

Approval covers scoped local implementation, documentation, isolated tests/builds and independent review. No commit/branch operation/push/PR/merge/release/installation/live migration/application launch, repository access, Git invocation for repository management, credentials/provider/model probe, agent/container execution or external mutation is included. Builder/test containers remain the existing validation tooling, not a runtime feature. Stop on material scope drift or revocation.

Risk: elevated protocol/persistence/editor change; architecture review, compatible migration, negative tests and fresh assurance are required. Preserve the existing uncommitted Projects foundation and separate documentation cleanup. HEAD at discovery: `14d960975e76f906efb5e447ff3dece0e8e4385b`, existing `feature/autonomy` checkout; this is a shared working-tree extension, not changes based on HEAD alone. Pre-iteration selected source/policy snapshot: `target/project-repositories/baseline-source.tar.gz`; 1,357 manifest entries, canonical SHA256 `41bea2ed038625478da27b4cc5a8b276eeec2ef5b51cb9d3b4334b2b416c215c`. Do not overwrite earlier Projects evidence or reclassify it as evidence for this extension.

## Independent inputs and planner reconciliation

- Producer `3e0c5144-e5cb-4b0e-a2b3-c6118a380999`: named editor, independently optional settings, no invented legacy knowledge, nested immutable recovery and inertness; AC1–AC8 below.
- Architect `db08c955-7ff4-41f0-a403-35500eaf6db9`: frozen disk5→6 conversion, domain-specific versioned operations, shared replay namespace, strict bounds and no old-client destructive downgrade.
- Designer `94d57160-e8d8-420a-8dd3-c2382212f47d`: repository groups, visible Base/Work, collapsed Branch types, unknowns preserved, reachable errors and existing origin/uncertainty protections.
- Reconciled disagreement: choose individually nullable branch/type leaves over the architect's initial all-or-nothing nullable branch object, preserving useful partial configuration. All objects/keys stay required.
- Reconciled disagreement: allow nullable names consistently on all writes, rather than adding legacy provenance rules. Clearly label Name (optional); do not block unrelated edits or invent URL-derived labels.
- Reconciled disagreement: choose exact case-sensitive uniqueness of non-null names, rather than producer's duplicate-label alternative, for unambiguous labels. Multiple null names remain valid.
- All three independent contexts confirmed these resolutions without a remaining design blocker. No scope authority or implementation acceptance is delegated to them.

## Exact data and validation contract

A project still contains exactly `{project_id,name,repositories,agent}`; stable project identity and fixed `opencode` metadata are unchanged. Repositories are ordered, 0–16 entries; at most32 projects. Each repository contains exactly:

```json
{
  "name": "backend",
  "url": "git@example.invalid:team/backend.git",
  "branch": {
    "base": "main",
    "work": "dev",
    "types": {
      "feature": "feature/*",
      "fix": "fix/*",
      "release": "release/*",
      "hotfix": "hotfix/*"
    }
  }
}
```

All keys/objects must be present. Only repository `name`, `base`, `work` and the four type leaves may be null. `branch:null`, `types:null`, missing/unknown keys, positional object arrays and string-list input are invalid. Empty strings are invalid stored values, not null aliases; an explicitly cleared optional UI field serializes null. No automatic defaults or presets: examples are hints only. Base/Work and type patterns may repeat.

Non-null repository names reuse the existing trimmed, control-free ≤40-Unicode-scalar project-name predicate. Preserve the credential-free locator grammar and ≤2048-byte bound unchanged. Non-null names and URLs are independently unique within a project by exact case-sensitive equality; do not normalize remote identity or case.

Non-null branch names are a conservative pure subset: 1–255 ASCII bytes; nonempty slash-separated components, each starting alphanumeric with remaining alphanumeric/`._-`; no `..`, dot-ended or case-insensitive `.lock`-ended components; reject exact `HEAD` and `refs/` prefix. This does not promise every Git-valid ref. No Git subprocess is needed.

Non-null type patterns are exactly `<safe-branch-prefix>/*`, ≤255 ASCII bytes in total. Validate the prefix under the branch-name rules. There is one final `*`, no other wildcard syntax. Store only; no matching, expansion, generation, branch selection, PR direction or publication authority follows.

## Persistence, compatibility and recovery

Disk6 becomes the writable format; App3/Core2 remain. Freeze and strictly validate disk5 with its old string-list registry rules before conversion. Disk1–4 continue preserving existing histories/configuration/cloud references/provenance/timing/jobs/results/receipts/IDs and initialize an empty registry directly in disk6. A disk5 URL becomes its exact original URL plus null name and all six null branch/type leaves; never derive names or branches. Preserve order, project IDs, registry revision, every receipt/fingerprint (including already-null fingerprints) and expired ID. Conversion is not a Projects mutation. Existing application migration revision/ownership/publication/interruption semantics remain.

Strict disk6 decoding, duplicate-key rejection, capacity preflight and atomic publication protect unsupported/corrupt/unpublishable bytes. Older binaries reject disk6; any installed rollout/restore needs separately approved compatible paired artifacts and pre-migration state/keyring relationships, not binary-only downgrade.

Introduce App3-only `projects_list_v2`, `project_create_v2`, `project_update_v2`, `project_delete_v2` with the existing respective argument/receipt fields and structured repositories. Keep `project_receipt({request_id})` unchanged. New clients never fall back to old operations. Legacy `projects_list`, `project_create`, `project_update`, `project_delete` unconditionally return `UPGRADE_REQUIRED` before legacy replay/admission—even on empty registry, empty-array updates, deletes or historical mutation requests. This prevents old clients erasing unseen settings.

Keep one Projects replay namespace,32 receipts/1024 non-evictable tombstones. New fingerprints bind the new operation name and all arguments. New calls cannot reuse old IDs; retained old receipts remain queryable without replay. Exact retained v2 replay precedes fresh instance/revision admission; conflicting inputs reject, scrubbed/expired IDs stay expired. Deletes scrub old metadata-derived fingerprints and remain metadata-only. Existing app/chat projections, worker/resource limits and presentation-preference schema are unchanged.

One shared controller retains original pending identity; lost/malformed/post-publication results are query-only. Resolve queries original receipt plus `projects_list_v2`; unknown/expired outcomes block writes but permit list observation. Nested fields are immutable in captured requests. Dirty editors/open confirmations retain captured instance and revision; no silent refresh/restart rebind. Confirmed delete plus authoritative absence clears the original selection, including dirty editors after restart, without choosing another project.

## UX and implementation touchpoints

Replace URL-only rows with compact repository groups: optional Name, URL, Base branch, Work branch and a collapsed Branch types section with four optional pattern fields. Use one project-level Save/Cancel and draft-only Add/Remove. All nested edits participate in dirty detection, immutable capture, successful reconciliation and current-origin review. Keep existing fixed OpenCode information, navigation, mounted chat/drafts/queues and per-view editor lifetime. No project data/editor text enters workspace preferences.

Show unknown migrated values as Not configured/blank optional fields. Presentation headings may use row position but never persist invented names. Keep errors/recovery outside hidden panels; reveal invalid type fields on Save. Stack at narrow/enlarged sizes, use labelled keyboard-accessible controls and reachable 48px critical targets. No other metadata, model/prompt/primary-repository/subdirectory features or Dev destination in this slice.

Disjoint implementation write sets:
- T1 service owner: `service/src/app_projects.rs`, `app_v2.rs`, `app_v3.rs`, native tests/current-schema fixtures and test-only compiled fixture support; installer compatibility assertions as necessary.
- T2 client domain owner: `client/lib/projects.dart`, `projects_controller.dart`, `app_api.dart`, `cloud_routing.dart`, Projects protocol/controller tests.
- T3 UI owner: `client/lib/ui/projects_view.dart`, Projects view/navigation tests; consume agreed immutable domain API.
- T4 parent: compiled Dart Projects integration, canonical overview/DETAILS docs, plan and evidence; no other feature changes.

## Acceptance traceability and gates

| REQ / AC | Task | Required evidence |
|---|---|---|
| R1 named ordered objects; AC1 CRUD/cancel/row identity | T1–T3 | strict models, widget add/edit/remove/cancel and compiled restart round trip |
| R2 optional knowledge; AC2 independent populate/clear, equal base/work | T1–T3 | every nullable leaf, unchanged unknowns, no inferred defaults |
| R3 naming metadata; AC3 pure branch/pattern grammar | T1–T3 | shared positive/negative/boundary vectors; no subprocess/network path |
| R4 lossless migration; AC4 disk1–5→6 and corrupt preservation | T1/T4 | rich disk5 fixture, actual compiled migration, receipts/IDs/order preserved |
| R5 recovery; AC5 stale nested-only edits | T2/T3 | same-revision restart, captured callback, all-field review and newer input preserved |
| R5 recovery; AC6 uncertainty/deletion/replay | T1–T4 | original-ID queries/no resend, old-op refusal including empty update/delete, old receipt lookup |
| R6 strict bounds; AC7 capacity/duplicates/accessibility | T1–T3 | maximal registry/state/frame bounds, exact shape/uniqueness, compact/enlarged keyboard tests |
| R5–R7 compatibility; AC8 inertness/chat preservation | T1–T4 | unchanged chat projections/preferences, zero synthetic inference/HTTP/keyring calls, independent source review |

All ACs block source acceptance until evidenced. Start with focused Rust/Dart/widget tests, then formatting/analyzer/locked warning-denied Clippy/tests for both Rust workspaces and Flutter. Run real compiled Rust↔Dart cases (not skipped/fake-only). Final configured root `make test build`, package verifier and real-package helper tests must bind stable final core inputs, including untracked files. Use the verified official Dagger CLI/isolated engine, not the PATH beta. Preserve intermediate failures and distinguish cached, skipped, cancelled and fresh outcomes.

Fresh independent assurance receives the final source/diff/plan and evidence, attempts compatibility/migration/recovery violations and records findings/closure. Source/widget/build passes do not constitute native graphical acceptance, live installation/migration/restore, real provider/model/GPU proof, clean-host/release provenance or publication approval. Stop before those effects.

## Outcome

Implemented T1–T4 and the reconciled R1–R7/AC1–AC8 contract. The editor, nested immutable models, strict disk6 migration and versioned service operations remain configuration only. Current records preserve independently unset leaves through unrelated saves and all existing origin/uncertainty/deletion safeguards.

Fresh independent assurance found raw Dart JSON decoding could collapse duplicate nested repository keys before strict validation. Root-cause remediation adds decoded member-name uniqueness preflight across all app reply object scopes, escaped-key handling and a64-container depth bound before whole-document decoding. Existing frame/correlation and mutation-uncertainty semantics remain. Architecture follow-up `db08c955-7ff4-41f0-a403-35500eaf6db9` found no architectural blocker or supported existing payload requiring deeper nesting. Durable23-case parser tests and unchanged3-case independent repro passed; additional independent parser/neighbor checks also passed. No dependency, runtime tool or capability was added.

The first parent compiled migration assertion failed because it incorrectly treated restart-cleared model inventory as durable state. The corrected test explicitly asserts inventory reset and compares the unchanged durable projection. The failed source/log/record are retained under `target/project-repositories/focused-attempt-1/`. Seven actual compiled Rust↔Dart cases passed on final source, including real synthetic disk5 migration/old receipts/shared IDs/legacy guards/branch-only query-only recovery and zero inference/HTTP/keyring calls.

Root attempt1 failed before component execution because the existing checksum-pinned Dagger builder was stopped (prior exit255, no recorded OOM; cause unknown). Only the same existing builder was started with unchanged image/configuration. Its immediate unsuitable buildctl probe failed; the bounded official CLI/module probe then passed. Root attempt2 passed all four recorder children: `make test build`, package verification, real-package helpers and diff check. Counts:295 service +283 installer +9 GTK +834 Flutter +41 package =**1462 aggregate executions**, plus34 helpers; five controlled fixture ignores/no other skips or final failures. Format90/zero changes, analyzer, locked warning-denied Clippy/builds, Linux build and no-Python Ubuntu runtime passed. All1243 core inputs stayed stable at SHA256 `ed6dc46ef3938f7753ffb9a7a0c76e6be1a2ea88ed0ff760835610fa7cd53481`; test steps were fresh while dependency/build caches were used. Service quality is configured Bookworm, not supplemental Ubuntu service proof.

Developer self-check records also preserve intermediate compile/style/schema failures and two initial installer artifact-guard failures with no established cause; later complete native and root gates passed without unrelated installer repairs. Reruns are not claimed to explain those failures. Independent reviewer setup errors and initial duplicate-key repro failures remain distinct from passing final checks.

[Extension evidence](../evidence/project-repositories/README.md), exact results/source/artifact manifests and [fresh independent assurance](../evidence/project-repositories/REVIEW.md) record closure and boundaries. The prior Projects evidence remains historical/unchanged. No Git mutation, Smith installation/launch, live state migration, real provider/model/keyring probe, runtime agent/container/Dev execution or external publication was performed. The sole container start was the existing approved validation builder. Installed/native graphical/restore, clean-host/release provenance and external effects remain separate human-authorized stages.
