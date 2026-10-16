# Local cloud/desktop checkpoint evidence

## Scope and approval

Implemented local checkpoint preparation under [the approved plan](../../plans/LOCAL-SMITH-NEXT-CLOUD-CHECKPOINT.md), based on `7f6a483ae82c4bcf647268f035f1e7cc80409d4b` on existing `feature/autonomy`. The operator explicitly requested cleanup and a local commit. No push, publication, deployment, installed-service change, real credential access or vendor request is authorized by this checkpoint.

## Validation and independent assurance

- Reused the latest configured `make test build` and explicit package verification: **1,262 passed**, zero failed; 263 service, 281 installer, nine GTK, 668 Flutter, 41 package. Three service/two installer fixture entry points are controlled ignores; Flutter/package have zero skips. Commands were not rerun for documentation-only checkpoint preparation.
- [Original final evidence](../saved-key-attempts/final.json), [source manifest](../saved-key-attempts/source-manifest.json), [component results](../saved-key-attempts/component-results.json) and [artifacts](../saved-key-attempts/artifacts.json) retain original identities and failures.
- [Fresh independent integration review](../../plans/LOCAL-SMITH-NEXT-CLOUD-CHECKPOINT-REVIEW.md) verified all 1,291 current recorded inputs against source inventory SHA256 `ce939c47c54b6afdb41c8b79002cd989a63e7b38ebc81933e928fea7d2cb25c3`, raw gate results and all 189 exports, four service copies and 91 client/payload pairs. Verdict: bounded source-integration PASS, no concrete blocker.
- Independent privacy/artifact audit (fresh session `6c0ab495-e2f2-4acf-abf4-e7b9418ecd4a`) scanned complete contents of 175 then-current candidates: 52 modified tracked files and 123 nonignored untracked files, 6,170,948 bytes. No confirmed real secrets, private operator paths, compiled binaries, model weights, symlinks or accidental generated artifacts were found. All 13 binary candidates were intentional PNG evidence/goldens; PNG chunks had no text metadata. Three representative images were visually inspected; remaining images received structural/provenance checks only. Pattern scanning is not proof against every secret encoding.
- The privacy audit's initial path concern was independently withdrawn: `client/lib/home/desktop_boot_shell.dart` is a tracked repository-relative source reference, not an absolute user-home path. No source reference or historical evidence was redacted. The scroll plan already labels its temporary log ephemeral.
- Historical `next/client/MESSAGE_QUEUE*` evidence is preserved. Its manifests are not current-source validation, and concurrent-attempt scratch-log names were reused, as already qualified in its validation record. Historical failures and hashes are not rewritten as PASS.

## Exact checkpoint binding

The initial inventory, reviewed explicit NUL-delimited staging list, final staged-blob inventory and approval/tree binding remain under ignored `next/service/build/cloud-checkpoint/`. Final staging must preserve all initial candidates and admit only reviewed checkpoint documentation additions. This document deliberately does not claim its own future tree/commit hash; the external binding records that identity immediately before the normal local commit.

No implementation or selected build/test input changed during checkpoint preparation. Repository and staged whitespace checks plus independent exact staged-tree verification are required immediately before commit. Final commit/tree equality and worktree status are checked afterward.

## Limitations

This is a local source checkpoint, not a release. Real provider entitlement/billing, installed keyring/session behavior, native graphical/IME/multi-monitor acceptance, installed schema migration/restore and clean-source licensing/SBOM/provenance remain separate concerns. The latest aggregate pass does not identify or fix historical runner timing/lease failures. Automatic model discovery and persisted catalog upgrade/uninstall lifecycle remain proposal-only, not implemented.
