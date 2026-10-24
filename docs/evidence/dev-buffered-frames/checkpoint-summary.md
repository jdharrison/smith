# Local Rust checkpoint validation

Scope: bounded, offline source validation only. No source edits, index changes, staging, commit, release, deployment, live host fixtures, network checks or Docker operations were performed. Existing dirty/untracked work was preserved. A checkpoint commit was not created because the explicit no-index-change restriction was retained.

Each command had a 120-second timeout. Cargo subprocesses used CARGO_NET_OFFLINE=true, RUSTUP_OFFLINE=1 and CARGO_TERM_COLOR=never. Full combined stdout/stderr is retained in the logs below; actual home paths were replaced with <HOME> before saving. Each log records cwd, exact command, exit status, timeout status and elapsed time.

| Working directory | Exact command | Actual result | Evidence |
|---|---|---|---|
| service | `cargo fmt --all -- --check` | PASS, exit 0 | `checkpoint-service-fmt.log` |
| installer | `cargo fmt --all -- --check` | PASS, exit 0 | `checkpoint-installer-fmt.log` |
| service | `cargo check --locked --offline` | PASS, exit 0 | `checkpoint-service-check.log` |
| installer | `cargo check --locked --offline` | PASS, exit 0 | `checkpoint-installer-check.log` |
| service | `cargo test --locked --offline --lib retained_jobs::tests:: -- --skip app_retained_jobs` | PASS, exit 0; 20 passed, 0 failed, 0 ignored, 477 filtered out | `checkpoint-service-retained-tests.log` |
| service | `cargo test --locked --offline --lib app_dev_local::tests::` | Exit 0, but ZERO tests matched; not counted as coverage | `checkpoint-service-dev-local-tests.log` |
| service | `cargo test --locked --offline --lib local_dev::tests::` | PASS, exit 0; 56 passed, 0 failed, 0 ignored, 441 filtered out | `checkpoint-service-local-dev-tests.log` |

The initial Dev filter used the source filename rather than its compiled module name. Source inspection identified `app::local_dev`; the corrected run exercised 56 tests. The zero-match attempt remains preserved rather than presented as a test pass.

Observed result: both workspaces passed formatting and locked offline compilation; 76 focused service tests passed. Retained-ledger tests use temporary storage, and Dev local-domain fixtures inject fake runtimes/providers. Fixture cleanup is confined to their temporary test data, not live application state.

Limits: default-feature checks only; no installer GUI-feature validation, installer tests, full service suite, Clippy, release build, root/package gates, compiled Rust/Dart checks, independent assurance, or installed/native acceptance was run. Historical failures were not repaired, rerun outside this scope, or waived. These results do not establish full component, functional, installed or release acceptance and authorize no external effect.
