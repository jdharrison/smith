# Optional cloud providers — source/package evidence

## Scope and disposition

**Implemented and validated for bounded local source/package scope, not live operation or deployment.** Work record: [LOCAL-SMITH-NEXT-CLOUD-PROVIDERS](../../plans/LOCAL-SMITH-NEXT-CLOUD-PROVIDERS.md). Base/HEAD: `7f6a483ae82c4bcf647268f035f1e7cc80409d4b` on existing `feature/autonomy`, plus the inventoried uncommitted worktree. Concurrent queue/scaffold/timestamp work was preserved; this combined snapshot does not attribute those changes to the provider task. Current app schema is 3, disk schema 4 and Core schema 2.

- [Public API/model contract](../../plans/LOCAL-SMITH-NEXT-CLOUD-API-CONTRACT.md): three reviewed IDs; documentation research is not account access or live API evidence.
- [Service-security review](../../plans/LOCAL-SMITH-NEXT-CLOUD-PROVIDERS-SERVICE-REVIEW.md) and [installer/Flutter integration review](../../plans/LOCAL-SMITH-NEXT-CLOUD-PROVIDERS-INTEGRATION-REVIEW.md): independent findings, fixes, source identities and scoped closure.
- [Validation ledger](../../../next/VALIDATION.md): final source outcome and retained historical failures.

## Final gate

Working directory: `next/`.

```sh
env DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make test build
python3 dagger/package.py --root installer/dist --verify
```

Both exited 0 without timeout. Aggregate duration was 480.725 seconds. Service 247, installer 279, isolated GTK 9, Flutter 456 and package 41 tests passed; zero failures. Flutter had no skips and its analyzer was clean. Three service and two installer ordinary-harness ignores are explicit subprocess/test entry points, including the Rust fixture invoked by Dart. They are not omitted acceptance scenarios.

All 1,267 inventoried files matched before and after both commands. Inventory digest: `a4c131f90932cb7455acfd842373e93dd89d78e67b1d4b5e62efc1cb47da95db`. The later validation-ledger addition and review/evidence documentation do not retroactively change the tested manifest; no production edit is attributed to those documentation follow-ups.

Retained, project-relative metadata:

- [final.json](final.json): exact commands, time bounds, exits, counts, log digests and binary/package identities; copied unchanged from the final runner record.
- [source-manifest.json](source-manifest.json) and [source-manifest.sha256](source-manifest.sha256): tested inputs and their identities.
- [artifacts.json](artifacts.json): all 97 package files, including `manifest.json` and `SHA256SUMS`; package verification checks the 95 manifest-listed payload files. Artifact inventory SHA256: `6cc0713bc6afcba2ec13554ed2a5adb514b438d31fc0a8dbf7c1dfba52f8c34f`.

Raw logs and duplicate before/after manifests remain locally under `next/service/build/cloud-reviewed-final-validation-20261015/` (ignored build output): `combined.log`, `package-verify.log`, `test-summary.txt`, `combined-{before,after}.{json,sha256}` and `package-verify-{before,after}.{json,sha256}`. The recorded paths in `final.json` remain relative to the repository root, not this directory. Log hashes allow checking retained bytes; owner-writable hashes are not an adversarial integrity anchor or a signature.

Built recipient package: `next/installer/dist/`, including `smith-installer-ui`, `smith-installer` and the paired client/service payload. Building/exporting is not installing or launching it.

## Historical failures and supersession

These local records remain distinct and were not deleted, rewritten as passes, or used as final evidence:

| Local evidence directory under `next/service/build/` | Disposition |
|---|---|
| `cloud-combined-validation-20261015/` | Initial aggregate PASS before independent remediation; superseded for final-source acceptance. |
| `cloud-final-validation-20261015/` | Failed during concurrent timestamp edits; missing timestamp method and changing source. |
| `cloud-settled-validation-20261015/` | Aggregate failed two obsolete installer disk3 expectations after disk4 integration; separate build/package PASS was not aggregate acceptance. |
| `cloud-final-integrated-validation-20261015/` | Stable-source HTTP fixture failure: the 100-ms total budget did not establish that dispatch had begun. |
| `cloud-http-deadline-remediation-20261015/` | Test-only correction: separate gated pre-send expiry and observed slow-response expiry, one fixed deadline, no retry. Six predetermined CPU-loaded HTTP runs and full service gates passed; independently reviewed. Production budgets remained unchanged. |
| `cloud-reviewed-final-validation-20261015/` | Final stable aggregate/build/package PASS reported above. |

Earlier component failures and focused fixes remain in the reviewer records and component build logs. Passing follow-up does not erase unknown root causes of unrelated historical launch-fixture failures. An initial coordinator post-gate digest script incorrectly expected `next/VALIDATION.md` to be included as a changed manifest entry; the runner excludes that narrative ledger, so the observed zero mismatches was correct. The corrected verification requires zero source/artifact mismatches; no production or artifact change was made to satisfy it.

## Approval and remaining boundaries

No real credentials, official API requests, normal-host keyring access, model weights/inference, installed-state migration, service-manager changes, commit, branch creation, remote mutation or deployment occurred. Tests use temporary/private fake capabilities, TLS peers, session-bus protocol peers and compiled Rust/Dart fixtures.

Still unverified and separately authorized: real unlocked GNOME Keyring/login-session behavior, account/model access and charges, native graphical/accessibility acceptance, installed migration/restore and maintenance, clean-clone/provenance/license/SBOM release evidence, publication and deployment. A JSON backup is not a keyring backup. Binary-only downgrade cannot read disk4. API keys retained by replacement/uninstall are not automatically deleted or revoked. Source/component/aggregate PASS and independent recommendations do not approve external effects.
