# LOCAL-SMITH-JOB-RUNNER

## Authority and scope

**LOCAL_PLAN_APPROVED for a minimal reviewable source slice:** after the planning-only proposal, the operator requested “the most minimal version ... to go along with projects and agents” and explicitly asked not to pursue deep tests. This approves focused local source/docs edits and bounded isolated checks, not live container/daemon access, provisioning, installation, provider/model calls, migration, commits, remote operations or accepted risk. Approval expires at completion or material scope drift. Baseline: `feature/autonomy`, HEAD `14d960975e76f906efb5e447ff3dece0e8e4385b`, plus the existing dirty/untracked work, which must be preserved.

## Reconciled first slice

Independent producer, architect and designer analyses were obtained in the planning conversation. Reconciliation chooses a small retained Dev form (saved project, saved agent, exact job prompt, explicit Start, current-job facts), not the reference kanban. The architecture concern about exactly-once execution is retained: one automatic dispatch attempt, observation-only uncertainty, no replay. Actual model inference is deferred. The public OpenCode server contract documents health, session creation, `noReply` message submission and message read-back; executable compatibility and no-provider-call evidence are not yet established.

- REQ/AC-1: explicit start admits one service-owned job with frozen saved project/agent/global context and task text. Metadata CRUD/navigation stay inert. One active job; no scheduler.
- REQ/AC-2: a separately provisioned local worker image starts with no network/published ports/host mounts/credentials/devices, non-root and resource limits. A private stdio control wrapper uses container-internal loopback OpenCode HTTP. Actual health/version/session readiness gates prompt injection.
- REQ/AC-3: persist before dispatch, submit once with a correlated message ID, verify exact read-back. Show **receipt only, no inference/coding**. Ambiguity/restart never resends. Bound shutdown; preserve cleanup uncertainty instead of claiming quiescence.
- REQ/AC-4: minimal Dev inputs/action/result alongside existing Projects/Agents. No board/archive/review, clone/Git, model/provider gateway, installer integration or prompting framework.

## Implementation boundaries and tasks

1. Service job domain and additive App3 operations: `dev_jobs_state`, `dev_job_start`, `dev_job_get`. Start binds instance, saved project/agent registry revisions, IDs, request ID and exact prompt. Summaries contain request/project/agent identity, status/message, nullable session ID and created time, not prompt contents. Separate private bounded `dev-jobs.json` ledger under the existing data namespace (maximum32, no eviction/replay); app disk9 and Core2 remain unchanged. Corrupt Dev state disables Dev, never resets it or disables Chat. Preserve unfinished records on restart, with no automatic container recreation or prompt replay.
2. Native container adapter and internal `dev-worker` entry point, plus a small optional image recipe. Use explicitly addressed rootless local Docker, no ambient context/rootful fallback; no runtime pull/build/start-daemon. Pin OpenCode1.19.3 and verified upstream archive checksum. Runtime image lookup freezes the exact local image ID. No host Docker/OpenCode/image action is performed by this source task.
3. Thin client controller/view, shared per desktop and retained through page hiding. Loading/unknown/failure/receipt are factual, not semantic task completion. No prompt or work identity in workspace preferences. Explicit start discloses container lifecycle and receipt-only behavior.
4. Focused formatting/compile/analyzer/smoke checks and fresh independent source assurance. No deep matrix, golden refresh, full build/package or live-host checks in this iteration. Add only small high-value tests for changed boundaries. Missing required broader evidence remains pending and blocks acceptance/release; user preference for a quick review is not an accepted-risk waiver.

## Wire contract

- `dev_jobs_state {}` → `{jobs:[summary]}`.
- `dev_job_get {request_id}` → summary, or explicit unknown error (never resend).
- `dev_job_start {request_id,instance_id,project_id,agent_id,expected_projects_revision,expected_agents_revision,prompt}` → summary.
- Summary: `{request_id,project_id,project_key,agent_id,agent_name,status,message,session_id,created_ms}`. Status: `initializing|ready|dispatching|recorded|failed|interrupted|unknown|cleanup_unknown`. `recorded` means verified prompt receipt and stopped owned runtime, not inference/task completion.

## Gates, evidence and rollback

Plan → scoped source → focused self-check → fresh independent assurance → operator review. Record actual checks/results below, including failures/skips. No claim of real container/OpenCode compatibility, zero-provider behavior, graphical acceptance or release until their separately scoped checks pass. Live validation needs exact daemon/image/account/context/cleanup approval. No installed files are touched. Source rollback preserves unrelated work and any future job ledger; never delete state or downgrade paired artifacts as a shortcut.

## Current evidence

**Implemented preview source; focused self-check PASS; bounded source findings closed. No overall functional/installed/release acceptance.**

Implemented touchpoints: `service/src/app_dev_jobs.rs` and additive `app.rs`/`app_v3.rs`/current-settings helper; `dev_runtime.rs`/`dev_worker.rs` and private binary/module wiring; optional `service/dev-worker/` recipe; `client/lib/dev_jobs.dart`, `dev_jobs_controller.dart`, `ui/dev_jobs_view.dart` and additive API/main/retained-shell integration. Three existing isolated Application fixtures gained only the new owner initializer. Canonical Scope/Architecture/Process/Design/Host contracts and the observed-source report were updated without claiming runtime acceptance.

### Focused checks — 2026-10-18

Commands ran on the actual dirty working tree, not a clean reconstruction; dependency/build caches were used. No dependencies were downloaded and no Docker/daemon/provider/installed-state action ran. Scope is deliberately limited per the operator's request.

| Working directory / exact check | Result |
|---|---|
| `service/`: `rustfmt --edition 2021 --check --config skip_children=true src/app.rs src/app_v3.rs src/app_instructions.rs src/app_dev_jobs.rs src/dev_runtime.rs src/dev_worker.rs src/lib.rs src/main.rs src/app_v2_tests.rs src/app_projects_tests.rs src/app_timestamps_tests.rs` | PASS |
| `service/`: `cargo check --offline --locked` | PASS |
| `service/`: `cargo clippy --offline --locked --lib --bin smith-service -- -D warnings` | PASS, no warnings |
| `service/`: `cargo test --offline --locked --lib dev_ -- --test-threads=1` | PASS:7 tests,0 failed/ignored,175 filtered out |
| `client/`: `dart format lib/dev_jobs.dart lib/dev_jobs_controller.dart lib/ui/dev_jobs_view.dart lib/app_api.dart lib/main.dart lib/ui/smith_workspace.dart test/dev_jobs_test.dart` | PASS:7 files,0 changes on final run |
| `client/`: `flutter analyze --no-pub lib/dev_jobs.dart lib/dev_jobs_controller.dart lib/ui/dev_jobs_view.dart lib/app_api.dart lib/main.dart lib/ui/smith_workspace.dart test/dev_jobs_test.dart` | PASS: no issues |
| `client/`: `flutter test --no-pub test/dev_jobs_test.dart` | PASS:4 tests |

REQ/AC-1,3: fake-runtime tests cover frozen composition/replay, dispatch publication fencing and verified receipt/stopped semantics. REQ/AC-2: pure framing/receipt tests and prior-runtime/restart/corruption checks, not actual Docker execution. REQ/AC-4: strict wire/native capture, read-only load/lost-ack recovery and multiline form tests. No test count is presented as real harness integration.

The final tracked-file `git diff --check` over this slice's integration/docs paths also passed. The optional image build context is restricted by `service/.dockerignore` to its recipe and native service payload, excluding retained diagnostics and caches. It was not built or live-validated.

Core source SHA256 binding from the focused-check working tree (not a whole-candidate provenance claim):

```text
78f4a9788c5141296cc5cdbced3d3ad451c1d4be14346366dd5b4db1b9c956d6  service/src/app_dev_jobs.rs
bb72430a1cc205525b7075b9d2d09d3fc0f9008defd1a33207391f0661a21ab4  service/src/dev_runtime.rs
de75fec9530f16b1d0fbe5b69303096558b3cbb6a70f610b1d02342b0d90530f  service/src/dev_worker.rs
668d31a792c04bd65db1a266ab5493483a5c34aa79196a5603f9162778b4c90e  client/lib/dev_jobs.dart
d504a376219348b9f034428639bec3380c20d37765caed8f572c4be8548757a0  client/lib/dev_jobs_controller.dart
07b77ff1c341913822357825da2acf6cc4676a9aa078784ad26503be058350e8  client/lib/ui/dev_jobs_view.dart
```

### Independent source review and preserved failures

Fresh context reviewed the scoped source without running commands. Initial review requested changes: (1) buffered Stdin could stall descriptor-polled framing; (2) APP_UNAVAILABLE could clear an already admitted identity; (3) restart reconciliation could trust absence/stopped despite a pending daemon start. Fixed with an owned unbuffered duplicated input descriptor plus open-pipe test, explicit pre-admission refusal allowlist plus helper-error regression, and conservative restart cleanup-unknown blocking without probes/writes. Follow-up source review found all three resolved and no remaining concrete high/medium blocker in those fixes. This is source-review closure, not external approval or functional-runtime PASS.

Earlier focused Rust test attempts were blocked during compilation by the new owner missing in three existing Application fixtures. Parent added those isolated initializers; the final7-test invocation above then executed successfully. Earlier attempts did not run tests and remain blocked historical attempts. Pre-fix client runs passed3 tests; final4-test result includes the admission regression.

### Diagnostic-only follow-up

The separately approved [safe prerequisite diagnostics fix](LOCAL-SMITH-DEV-DIAGNOSTICS.md) adds payload-free classifications and fixed messages without changing the receipt-only runtime, uncertainty rules or schemas. Its checks/review bind a later source snapshot; the results above remain historical. It does not rewrite the original failed job or authorize runtime preparation/retry.

### Remaining limits

Full compiled client/service integration, real pinned-container/provider-side-effect checks, root/component/package gates, native graphical/host acceptance and candidate release provenance are deliberately not established by this iteration. The Ubuntu base is tagged rather than digest-pinned; the image recipe was not built. Stopped containers and private prompts remain retained; capacity is32 with no eviction. Restart/creation/cleanup ambiguity requires separately approved operator reconciliation and this slice has no recovery UI. Unsent Dev drafts are volatile. No live rollout, model/provider selection, installation, commit or accepted-risk decision is included.
