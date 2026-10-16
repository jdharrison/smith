# LOCAL-SMITH-NEXT-PROVIDER-SAVE-FIX

## Bounded continuation

Source-only defect remediation within the operator-approved [cloud-provider implementation](LOCAL-SMITH-NEXT-CLOUD-PROVIDERS.md), following the operator's report that both OpenAI and Anthropic Save return to Providers as Not configured. Existing branch `feature/autonomy`, base `7f6a483ae82c4bcf647268f035f1e7cc80409d4b`, plus preserved concurrent/uncommitted work. This does not extend approval to real credentials, normal-host keyring/service access, installation, vendor HTTP, API charges, cleanup, commits or publication. Approval expires with this bounded follow-up/session or material scope/base drift.

## Evidence and independent inputs

- Independent credential-architecture inspection found a concrete compatibility defect: `cloud_keyring.rs` requires `text/plain; charset=utf-8`, whereas GNOME Keyring 46.2 `gkd_secret_secret_append()` returns `text/plain` independently of the submitted content type. The coordinator independently confirmed the literal at upstream https://raw.githubusercontent.com/GNOME/gnome-keyring/46.2/daemon/dbus/gkd-secret-secret.c (2026-10-15).
- Save publishes an item and reads it back before activating its generation. The mismatch can fail that read, conservatively preserve publication uncertainty and leave Smith unconfigured. This fits both providers but is not evidence from the operator's installed keyring.
- Independent installer/UX inspection confirmed Save outcomes and current catalog share one status area; returning there does not imply success. It did not identify an automatic-approval/decline defect. Broader per-action status retention is separate from this minimal compatibility correction.
- Requirements: confirmed Save must be able to verify an ASCII API key returned with GNOME's documented plain-text media type; successful metadata normalization must not be interpreted as secret/identity mutation. Credits are irrelevant to Save.

## Reconciled implementation and acceptance

| Requirement | Task | Blocking evidence |
|---|---|---|
| FIX-R1: support GNOME read-back MIME without accepting arbitrary media | Accept exactly `text/plain` and the existing `text/plain; charset=utf-8` | Explicit regression fails before the production change, passes after; negative MIME cases |
| FIX-R2: fixtures model server normalization rather than echo input | Private peer returns `text/plain` by default regardless of submitted MIME; configurable negative/legacy fixtures | Encrypted create/read and negotiated-session suites |
| FIX-R3: preserve all existing safety boundaries | No encryption, identity/attribute, timestamp, deadline, prompt, replacement, activation, storage or retry change | Existing private keyring, service and installer tests; source review |
| FIX-R4: be truthful about installed behavior and prior effects | Update canonical credential/host notes and validation ledger | Independent review and exact source/build evidence; no live claims |

A separate architecture concern is the existing `Created == Modified` check across GNOME's creation/set-secret steps. Do not remove or redesign it as part of a MIME fix: changing persistent integrity semantics requires its own bounded design/evidence. No claim of complete GNOME integration is made from synthetic peers. No previous orphan item is adopted, retried, deleted or activated.

## Validation and delivery

Start with the named private-peer regression against unchanged production code, preserve the failure, apply the narrow allowlist correction, then run the private keyring suite, warning-denied Clippy, service tests and installer tests. Rebuild/verify the paired recipient package when source gates pass. Isolated build dependencies and test-only GNOME fixtures are permitted; never mount/access the normal host bus or keyring. A disposable real-GNOME test would strengthen compatibility evidence but does not authorize normal-host effects.

Obtain fresh independent review of the final narrow diff and retained results. Source self-check is not deployment/release approval. The running installed service is unchanged until a separately approved/operator-owned upgrade; rebuilding only an installer does not repair an old running service binary.

## Outcome

The new GNOME-normalized private-peer regression failed against unchanged production with `KEYRING_CONFLICT; creates=1`, then passed after the exact two-form MIME allowlist correction. All 15 private keyring tests passed. The final stable aggregate passed 250 service, 279 installer, 9 GTK, 456 Flutter and 41 package tests, plus paired build/verification. A disposable Ubuntu 24.04 GNOME Keyring46.1 test with the real adapter saved/read both synthetic provider keys once, with no host mounts or runtime network. [Retained evidence](../evidence/provider-save-fix/README.md) and [independent review](LOCAL-SMITH-NEXT-PROVIDER-SAVE-FIX-REVIEW.md) bind the narrow correction. This closes the identified source defect, not installed-host diagnosis/deployment or the separate timestamp concern; actual API inference and credits were not exercised.
