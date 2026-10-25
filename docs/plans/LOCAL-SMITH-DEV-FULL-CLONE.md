# LOCAL-SMITH-DEV-FULL-CLONE

## Authority and core intent

**LOCAL_PLAN_APPROVED for bounded source implementation and isolated validation:** the operator explicitly requested full Git clone/fetch for repository code references instead of custom shallow acquisition. This supersedes depth-one acquisition in the local-runner plan, not the other execution boundaries. Repository Smith, existing `feature/autonomy` bootstrap branch; preserve all concurrent dirty/untracked work. Approval expires on completion or material drift. No commit, live clone, Docker/image operation, service restart/replacement, inference, reconciliation, release or deployment is included.

## Independent analyses and planner reconciliation

- Producer: ordinary full reachable history, branches and tags; selected Base/HEAD checkout stays pinned. Keep existing budgets and fail rather than silently becoming shallow. Full acquisition does not grant the model `.git` access or Git execution.
- Architect: remove depth/single-branch/no-tags flags and the shallow-discovery exchange, retaining the existing credential-free frozen-public-HTTPS relay and network-isolated worker. Ordinary initial clone uses advertised wants, flush/done, then NAK/sideband pack. Native Git verifies objects; retain pin verification and transport limits.
- Designer: retain the job workflow; disclose full-history/normal branches/tags and bounded failure in existing Review. Label the selected ref Checkout ref. No new controls or progress system.
- Planner: architecture notes two HTTP requests/repository could support sixteen repositories, but producer/designer reject an incidental capacity expansion. Preserve the current ten-repository execution ceiling, thirty-two HTTP requests and all byte/time/workspace/read limits. Remove shallow-only grammar, not destination or request validation. Ordinary tags mean standard clone behavior, not a mirror of hidden/unreachable objects, submodules or LFS. No blocking disagreement remains for this bounded source change.

## Traceability and implementation

| Requirement / acceptance | Task | Required evidence |
|---|---|---|
| FULL-1: full non-shallow reachable history, remote branches and normal tags; exact Base/HEAD checkout | Remove shallow clone flags and discovery state; accept bounded advertised wants and ordinary pack response | Actual isolated native Git clone/upload-pack through FetchSession: root/linear/merge histories, divergent branches, tags, default/exact refs; no `.git/shallow` |
| FULL-2: unchanged destination/credential/network, pin, sealing and resource boundaries | Retain existing relay, sanitize capabilities and validate initial clone grammar; preserve ten-repository preflight | Malformed/unadvertised/shallow/filter requests, phase, cancellation/deadline/size/security and checkout regressions |
| FULL-3: truthful existing consent with no new workflow | Update Review paragraph/ref label and matching canonical details | Focused Dev widgets, formatting/analyzer and source review |
| FULL-4: stable source evidence and independent review | Self-check then fresh assurance | Focused Rust/Flutter checks, locked build/production Clippy, exact failures and source identities; no overall acceptance if required evidence is missing |

## Gates and exclusions

Run focused isolated native-Git/service checks first, followed by relevant broader service checks/build/Clippy, client Dev tests/analyzer and formatting. Save bounded secret-safe logs under `docs/evidence/dev-full-clone/`. Obtain fresh independent assurance after self-check. Failing required checks, errors, critical warnings and missing verification block the corresponding completion/release claim. Installed/image/model/graphical acceptance and full root/package provenance remain separate gates; no live-resource fallback. Existing failed/uncertain jobs are preserved and never replayed. No schema or ledger migration is intended.

## Evidence

**Implemented source; scoped self-check and fresh independent assurance PASS. Not installed/live functional acceptance.**

Backend changes are limited to `service/src/dev_fetch.rs`, `dev_fetch_tests.rs` and `dev_worker_local.rs`: remove shallow/single-branch/no-tags clone options, shallow discovery state and response parsing; accept bounded ordinary advertised wants including repeated shared-tip wants, require the checkout pin, and retain NAK/sideband pack verification. Native Git handles object integrity. Existing transport/credential/sealing/resource restrictions and ten-repository ceiling remain. UI changes only the Review acquisition disclosure and Checkout ref label, with corresponding disclosure/narrow-layout assertions.

### Actual source checks

Commands ran offline with existing dependencies; native Git fixtures use temporary local repositories and loopback, never production DNS/TLS. Counts overlap and are not additive unique coverage.

| Command (component working directory) | Actual outcome |
|---|---|
| `cargo test --offline --locked dev_fetch -- --nocapture` (`service/`) | 46 passed; exit0 |
| `cargo test --offline --locked dev_worker_local` (`service/`) | 10 passed; exit0 |
| `cargo test --offline --locked --lib dev_ -- --nocapture` (`service/`) | 105 passed,0 failed/ignored; exit0 |
| `cargo test --offline --locked --lib app::local_dev::tests -- --test-threads=1` (`service/`) | 25 passed,0 failed/ignored; exit0 |
| `cargo build --offline --locked` (`service/`) | PASS; source build only |
| `cargo clippy --offline --locked --lib --bins -- -D warnings` (`service/`) | PASS; no warnings/errors |
| `rustfmt --edition 2021 --check` on three backend files | PASS |
| `flutter test --no-pub test/dev_jobs_test.dart test/dev_jobs_controller_list_test.dart test/dev_jobs_list_view_test.dart --reporter expanded` (`client/`) | 80 passed; exit0 |
| `flutter analyze --no-pub` (`client/`) | PASS; no issues |
| `flutter build linux --no-pub` (`client/`) | PASS; not installed |
| `dart format --output=none --set-exit-if-changed lib/ui/dev_jobs_view.dart test/dev_jobs_list_view_test.dart test/dev_jobs_test.dart` (`client/`) | PASS;0 changed |

Native Git2.43 fixtures cover ten successful full-clone variants and corrupt-pack rejection: complete reachable root/linear/merge parent graphs, divergent remote branches, annotated/lightweight tags, default/exact/nested refs, pinned detached HEAD, absence of `.git/shallow`, and exactly advertisement plus POST. Negative grammar/phase, limits, cancellation and sealing fixtures remain. Initial backend fixture failures were corrected and rerun; exact attempts/log hashes are retained in `docs/evidence/dev-full-clone/backend-final.json`. Initial client run was79 passed/1 failed because an existing assertion still expected Base ref; updating that assertion to Checkout ref yielded80 passed. The original failed log is retained, not overwritten.

### Independent assurance and stable scoped identity

A fresh independent reviewer found no backend blocker, independently reran105 Dev tests, the executable Git fixture, build/Clippy/format and verified the backend evidence log hashes. Follow-up read-only UI/docs assurance found no scoped blocker and independently reran80 client tests, full analyzer, formatting, Linux build and25 local-domain tests. Review is not human approval or a release decision.

```text
417d583e5ec847a512e3c959d061e70283b29a974fc404aa8b3bd8b0809175d6  service/src/dev_fetch.rs
317cdc1cc58ea597ddcbf5695339377689f25c8a61c9eb5e30a83972ea00e6dd  service/src/dev_fetch_tests.rs
60e3a31709d4e3fed3ec701e915efcd9eb6398cf8d06b9aebbd42a95dcd92786  service/src/dev_worker_local.rs
e054b24ced7c4bbddce0d5d21f957920ae5920f80cc185b9e53bce501ebdfaeb  client/lib/ui/dev_jobs_view.dart
a34bc1e187d5e562355c39a93544825ca9c8b83ddd146718f80bfbee86f1616a  client/test/dev_jobs_list_view_test.dart
448f3a8586143f1dcf201837cf5c05fbf03acf2313484fe3bc0c5a45ddf8ec01  client/test/dev_jobs_test.dart
```

This is scoped dirty-source evidence, not whole-candidate provenance. Historical navigation/restore fixture failures recorded in the jobs-list plan were not changed, rerun or waived. Broader root/package/compiled integration, production DNS/TLS, actual image/OpenCode/model, installed/native graphical and live functional acceptance remain unestablished. No external clone, Docker/image action, inference, service restart/replacement, historical job/ledger reconciliation or remote mutation ran. Independent role analyses above remain planning recommendations, not runtime acceptance or external-action approval.
