# LOCAL-SMITH-DEV-ROOTFUL-DOCKER

## Authority and bounded intent

**LOCAL_PLAN_APPROVED for source implementation and isolated validation:** the operator explicitly accepted host Smith service access to the existing rootful Docker daemon after the root-equivalent host authority was explained: “i think its okay for the host smith service to have rootful since its an autonomy substrate.” This accepts that specific authority boundary, not a blanket deferral of safety checks or accepted-risk waiver for unrelated issues. Docker is operator-provided; installer provisioning and Dagger integration are excluded.

Repository Smith; existing `feature/autonomy` bootstrap branch, HEAD `14d960975e76f906efb5e447ff3dece0e8e4385b`, with concurrent dirty/untracked work preserved. Owner: developer/planner. Risk: elevated host authority and endpoint/cleanup boundary. Approval expires on completion or material scope drift. No commit, daemon setup/start/configuration, group/permission change, sudo, image build/pull, container run, repository fetch, inference, installed service replacement/restart, publication or deployment is authorized by this source record. Discovery observed a root-owned native `/run/docker.sock` with0660, safe root-owned0755 `/` and `/run`, and interactive-account access; this is not service-account/live-run acceptance.

## Independent analysis and reconciliation

- **Producer:** Docker remains an operator prerequisite. Limit the change to current local-runner endpoint selection, truthful errors and preservation of existing execution/recovery restrictions. No new Dagger or installer work.
- **Architect:** use an immutable adapter mode selected at construction. Current local runner selects only `unix:///run/docker.sock`; legacy receipt-only runtime retains its private rootless endpoint and verifier. The same adapter must perform create/attach/inspect/stop/drop, with no environment/context/failure fallback. Rootful socket/ancestor checks and positive supported daemon-security fixtures are required. Old uncertain jobs remain observation-only; do not probe/reconcile them through a different daemon.
- **Designer:** add fixed rootful-specific diagnostics rather than changing old strings. Keep strict new/old message decoding, historical outcomes and cleanup uncertainty; no new setup/retry UI.
- **Planner reconciliation:** implement only this endpoint/diagnostic slice. Preserve the service's ordinary non-root identity and private runtime-root checks. For rootful mode require real root-owned, non-writable-by-group/other `/` and `/run`, plus a non-symlink root-owned Unix socket0660 (no special/world bits). Accept an order-independent exact supported security set: AppArmor (`name=apparmor` or `name=apparmor,profile=default`), `name=seccomp,profile=builtin`, `name=cgroupns`; reject malformed/duplicate/unknown/rootless data. This binds the observed discovery security features without interpreting absent rootless evidence alone as success. These reported features do not prove container confinement or resource enforcement. No analysis grants live execution or release approval.

## Requirements and acceptance

| ID | Requirement | Required isolated evidence |
|---|---|---|
| ROOTFUL-1 | Current local runner exclusively selects the fixed rootful endpoint; legacy receipt runtime remains rootless. No configurable remote host, ambient context, mode fallback or sudo. | Mode/argv/environment fixtures; legacy verifier regressions; command construction remains shared by every lifecycle operation. |
| ROOTFUL-2 | Validate CLI, private runtime root, selected socket/ancestors and supported daemon security before creation; retain image freezing and no pull/setup. | Synthetic metadata safety/absence/access and daemon positive/negative cases; exact fixed diagnostics. |
| ROOTFUL-3 | Keep worker network/mount/user/resource/tool/provider restrictions and exact-owned cleanup unchanged. Retained reads never probe; uncertainty blocks admission/replay. | Existing Dev/lifecycle/restart/publication tests; source review of the unchanged lifecycle/guard binding. |
| ROOTFUL-4 | New rootful diagnostics persist/reopen; old rootless/generic messages and ledgers remain valid and unchanged. | Message allowlist/round-trip and zero-effect reopen fixtures. |
| ROOTFUL-5 | Document root-equivalent host authority and operator prerequisites without implying runtime acceptance. | Canonical Scope/Architecture/Host/Design plus worker preparation guidance; fresh independent assurance. |

This does not make the current local runner repository-free: it still requires at least one public HTTPS repository and verified read evidence for a final reply. Earlier branch-ref/read-evidence fixes remain intact. Existing old `recorded` jobs remain receipt-only.

## Validation and external boundary

Run focused offline Dev/runtime and local-ledger tests, formatting, locked build and warning-denied production Clippy on stable scoped source; obtain fresh independent architecture/adversarial assurance. Preserve exact results/failures and source identities. Missing broader component/root/package/compiled integration/live/image/model evidence remains pending, not silently waived or relabelled PASS. All host execution, candidate rollout and live functional checks need separately bound approval; source work changes no installed service or daemon.

## Evidence

**Implemented source; focused self-check and bounded independent source review PASS. No functional/installed/release acceptance.**

`service/src/dev_runtime.rs` adds immutable adapter modes, fixed rootful socket selection, factored synthetic metadata predicates, supported security verification and five fixed diagnostics. `dev_runtime_local.rs` selects rootful only for the new local runner; the old receipt runtime retains rootless verification. `app_dev_local.rs` preserves historical messages and admits the new diagnostics; tests round-trip all old/new prerequisite messages without runtime/provider calls or disk rewrite, and preserve dispatch/cleanup uncertainty. Container restrictions, guard ownership, cleanup routing, no-probe reads and no replay remain unchanged.

Integration inspection also found the earlier approved branch-ref fix had not reached domain checkout publication. The domain and its fake runtime now share `checkout_reference`; an additional regression covers bare/qualified main/nested refs, absent Base and double-prefix rejection. This closes the original branch finding through acquisition, worker evidence and ledger publication rather than weakening validation. Historical actually produced checkouts still retain exact URL/ref/OID validation; no live ledger was opened or rewritten.

### Parent validation — current dirty working tree

Commands ran from `service/`, offline with existing dependency/build caches. All returned exit0, with no failures/skips/ignored cases in these test invocations. Four scoped source hashes matched before and after the final combined command group. This is bounded identity, not whole-candidate provenance.

| Exact command | Actual result |
|---|---|
| `cargo test --offline --locked --lib dev_ -- --test-threads=1` | 94 passed,0 failed/ignored,212 filtered out |
| `cargo test --offline --locked --lib app::local_dev::tests -- --test-threads=1` | 25 passed,0 failed/ignored,281 filtered out; overlaps the preceding group by one test |
| `cargo clippy --offline --locked --lib --bin smith-service -- -D warnings` | PASS, no warnings/errors |
| `cargo build --offline --locked` | PASS; source debug build only, no installed-service replacement |
| `rustfmt --edition 2021 --check --config skip_children=true src/dev_runtime.rs src/dev_runtime_local.rs src/app_dev_local.rs src/app_dev_local_tests.rs` | PASS |

Earlier implementation checks also passed9 runtime predicate tests and3 local framing tests. The final Dev invocation above supersedes their scoped source check without implying additional unique test coverage. Exact fixed diagnostics, all socket permission combinations, unsafe owners/types, supported security permutations, malformed/duplicate/unknown/rootless data and hostile command environment/context entries are exercised without spawning Docker. Lifecycle argv fixtures exercise shared construction, not a real daemon or end-to-end fake Docker executable; timeout/cancellation/ownership uncertainty retain their existing fixtures and source review.

Scoped documentation `git diff --check` and a Python trailing-whitespace check passed; checks include untracked records separately because Git diff does not cover them. No Flutter code changed or Flutter checks reran for this follow-up.

### Fresh independent assurance

A fresh read-only architecture/adversarial reviewer inspected the four Rust files, immediate integration and affected docs. It found no concrete scoped blocker in immutable endpoint selection, env isolation, socket/security validation, cleanup adapter binding, diagnostic preservation, historical uncertainty or checkout normalization. The review's stable hashes match the parent check group:

```text
68a182036f0a3c8cff0539ba627f1a912253b9d64e8eb3a341c1ff1cd1f88bc1  service/src/dev_runtime.rs
45900efc648caaa156b233b262d5a0dbab8ab635d8cd7d21e80edb2c07943255  service/src/dev_runtime_local.rs
eb2f83dd17b32be3abaa1ab6bddbe85ae0e195695fa03a088fa336ed93382595  service/src/app_dev_local.rs
278a2eec30c3a998db576052621639a53f95eb9f4c8ddea5c0c42c83d91bd0d1  service/src/app_dev_local_tests.rs
```

The reviewer did not duplicate parent executable checks. New strict message readers preserve historical data, but older binaries may reject new diagnostic strings despite the unchanged ledger version; later rollout/restore requires compatible paired artifacts and preserved state, not binary-only downgrade. Formal functional/installed/release assurance remains no-go pending its separate required evidence.

No daemon setup/access, image build/pull, container creation, clone, inference, installed service change or restart ran for this source task. Broader component/root/package/compiled integration, actual service-process Docker access, pinned image/Git/OpenCode/model/resource and live acceptance remain unverified. The operator's host-authority acceptance does not satisfy those gates.
