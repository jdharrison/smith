# LOCAL-SMITH-DEV-LOGGING-FIX

## Authority, cause and scope

**LOCAL_PLAN_APPROVED for a source-only defect fix and isolated regression validation:** the operator answered “yep” to adding explicit `compress=false` in both worker adapters and regression coverage. No standalone installed binary replacement, package/image build or rollout, live job/retry, container deletion or ledger reconciliation is included. Approval expires on completion or material drift. Preserve existing dirty/untracked work; no commit or remote mutation.

Repository Smith, `feature/autonomy` bootstrap branch, HEAD `14d960975e76f906efb5e447ff3dece0e8e4385b`. Owner: developer/planner. Observed failing request `9a8d5ecb22fa4b85629e69f7a1e624b1`: the Docker daemon reports `failed to initialize logging driver: compression cannot be enabled when max file count is 1`. Its container is created/not running with no start timestamp; the retained ledger has failed_stage=cloning, may_have_sent=false, reply_available=false and cleanup_unknown. The default-compressed `local` driver plus explicit max-file=1 is the concrete startup defect, not a clone/model failure. The original record/container remain preserved; present observation is not automatic reconciliation authority.

## Reconciled implementation

Existing independent producer/designer analysis in [the rootful plan](LOCAL-SMITH-DEV-ROOTFUL-DOCKER.md) still governs operator prerequisites, fixed diagnostics and conservative recovery; no UI or task-flow change is proposed. Obtain a narrow independent architecture check for the logging delta, then use one pure shared create-argument builder in `service/src/dev_runtime.rs` consumed by both receipt and local adapter creation. It must emit Docker `container create`, the existing local driver/1m/one-file limits and explicit `--log-opt=compress=false` before the unchanged caller options/image/entrypoint. This removes duplicate logging settings without broad lifecycle refactoring.

- LOG-1: both adapters disable compression explicitly, retaining one1MiB-bounded local-driver log file; no increased count/disk allocation or unbounded driver.
- LOG-2: production command construction has isolated exact-argv regression coverage across rootless/rootful adapter modes and both worker entrypoints; image/arguments remain after options. No Docker contact is a test fallback.
- LOG-3: endpoint/env isolation, ownership, worker restrictions, budgets, receipt/final-reply rules and cleanup uncertainty remain unchanged. No retry, replay, history rewrite or new diagnostic surface.
- LOG-4: focused formatting, locked build, warning-denied production Clippy and Dev/lifecycle tests plus fresh independent source assurance bind the final scoped source. Broader/live/installed acceptance remains separate and unverified. Any later deployment uses the manifest-verified paired client/helper/service path, never the earlier service-only QA updater.

Update the matching Architecture/Host logging contract and worker guidance. Source changes alone do not unblock the retained cleanup_unknown job; that needs a separately approved factual reconciliation, not deletion or fabricated completion.

## Evidence

**Implemented source; focused self-check PASS; fresh independent source review found no scoped blocker. No functional PASS, live acceptance, installed update or reconciliation.**

A narrow independent architect review found no blocker in explicit compression-off with unchanged size/count, shared pure construction and unchanged lifecycle/authority. Implementation adds `create_arguments(options)` in `service/src/dev_runtime.rs`; both production create paths call it and no longer duplicate their logging flags. Remaining options, ownership labels, tmpfs differences, frozen image IDs, entrypoints and request IDs are unchanged. The new regression uses that production builder plus fixed-endpoint command configuration across rootless/rootful modes and receipt/local worker entrypoints, asserting exact logging prefix, one driver/three unique option positions, unchanged caller arguments and options-before-image ordering. Existing command tests cover clean environment behavior separately.

### Parent checks — final dirty source

Commands ran from `service/`, offline with existing caches. All returned exit0; no failures/ignores/skips occurred in these test invocations. Before/after hashes of both scoped files matched across the final combined check group.

| Exact command | Actual result |
|---|---|
| `cargo test --offline --locked --lib dev_runtime::tests::both_worker_commands_disable_compression_with_one_bounded_log_file -- --test-threads=1` | 1 passed,0 failed/ignored,306 filtered out |
| `cargo test --offline --locked --lib dev_ -- --test-threads=1` | 95 passed,0 failed/ignored,212 filtered out |
| `cargo test --offline --locked --lib app::local_dev::tests -- --test-threads=1` | 25 passed,0 failed/ignored,282 filtered out; overlaps preceding group by one test |
| `cargo clippy --offline --locked --lib --bin smith-service -- -D warnings` | PASS; no warnings/errors |
| `cargo build --offline --locked` | PASS; source debug build only |
| `rustfmt --edition 2021 --check --config skip_children=true src/dev_runtime.rs src/dev_runtime_local.rs` | PASS |

Scoped documentation/record whitespace checks passed on the final run. The first check found a trailing space in the earlier restoration evidence paragraph; that retained record was corrected and the eight-file check then passed. This was a documentation-format failure, not a runtime or Rust-test failure.

Fresh independent read-only assurance reconstructed each predecessor in memory and matched both prior rootful-plan hashes, confirming only the shared builder, caller substitutions and regression were added. It found no concrete blocker in production argv ordering, temporary-string lifetimes, retained limits or unchanged safety/recovery behavior. Review did not duplicate tests or grant external approval. After reading the parent results and independently recomputing the unchanged hashes, assurance confirmed bounded source-only closeout PASS; deployment/live acceptance remained no-go pending their separate evidence/approval. Its initial/final hashes match the parent check group:

```text
20bc9e91e12e06225485faecdf9af89dccef1f15dca07a45cf7527b4326f1d8b  service/src/dev_runtime.rs
8dddb9d8c12ceba0061a10a9031e809a0c4722623d1ad0217948401e3ed6cca6  service/src/dev_runtime_local.rs
```

This binding covers two untracked files, not the whole dirty candidate. No Flutter code changed or Flutter checks ran. Broader/root/package/compiled integration and actual Docker startup remain unverified; command fixtures do not establish live log/resource enforcement or OpenCode/model compatibility. No Docker/daemon contact, image build, installed replacement/restart, retry or ledger/container mutation ran during this fix. The original cleanup_unknown record still requires separately approved reconciliation; do not fabricate cleanup certainty or replay its request.
