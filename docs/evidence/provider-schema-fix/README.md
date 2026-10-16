# Provider Save GNOME schema correction — retained evidence

**Implemented source correction; isolated component/build/package PASS. Overall aggregate acceptance remains FAIL; no installed repair, release or deployment approval.** Work record: [bounded plan](../../plans/LOCAL-SMITH-NEXT-PROVIDER-SCHEMA-FIX.md). Final disposition: [validation ledger](../../../next/VALIDATION.md) and [independent review](../../plans/LOCAL-SMITH-NEXT-PROVIDER-SCHEMA-FIX-REVIEW.md).

## What changed and what passed

Only the returned-attribute predicate in `next/service/src/cloud_keyring.rs` changed in production: four exact Smith identity attributes remain required; only the exact optional `xdg:schema=org.freedesktop.Secret.Generic` pair is permitted in addition. Outgoing search/create identity remains four fields. Encryption, MIME, timestamps, ownership, deadlines, publication uncertainty, no retry and no orphan adoption/removal are unchanged.

The new schema regression reproduced `KEYRING_CONFLICT; provider=openai; schema=true; creates=1` against the unmodified predicate. All 18 private-keyring tests then passed, including both providers/MIME forms/four-five attributes, 32 invalid maps rejected before secret retrieval, and six invalid post-read mutations. The [expected-red log](expected-red.log) is a sanitized derivative of raw local evidence, replacing only the repository's absolute path with `<repository>`; original raw SHA256 is `828ad62e72ff39a04fe71745253938b67922a98b603ab1018b4bba53c2ee2956`.

[Final sequential command evidence](final.json), [source manifest](source-manifest.json) and [artifact manifest](artifacts.json) bind these separate commands, all exit 0:

- `env DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make service/test`: 253 passed, three controlled fixture ignores; all 18 keyring tests included.
- Same environment, `make installer/test`: 279 Rust plus nine isolated GTK passed, two controlled fixture ignores; interpreter-free Ubuntu CLI runtime gate passed.
- Same environment, `make build`: rebuilt standalone service, full paired client and recipient package.
- Same environment, `python3 dagger/package.py --root installer/dist --verify`: 95 manifest-listed files verified, 97 total recipient files.
- Same environment, `make package/test`: 41 passed using actual compiled installer ELFs.

Commands ran from `next/` on one stable **1,273-entry** input inventory, SHA256 `2f083f0e5556c35faf7b1f0696c55e27e86073e52790f5545709c071b20f85c0`. This includes concurrent Boot changes, preserved and inventoried but not authored or independently approved by the schema task. No final-current-snapshot Flutter test PASS is inferred from the build or earlier tests.

All four exported service copies match SHA256 `56e617c12fba1dba98b7d55c05b9c4efaaae0dbbda05432736cc50b51eeb9777`. All 91 standalone/packaged client files match in bytes and mode. Recipient inventory SHA256 `4530e813f628e08e57af87aa81c675b30877ee65410163b2cd091ed086e4bb3e`; manifest SHA256 `397f3dd8eeebd4228e4cf6810d5cd1f0ac453e170de8d5bee0809d5118b38d7c`. Use the complete `next/installer/dist/` candidate; the separate development helper under `next/installer/build/` was not rebuilt.

## Aggregate failures are retained, not superseded

1. [First aggregate attempt](aggregate-attempt-1.json): stable 1,267 inputs, exit 2; `application_teardown_cancels_active_native_inference_and_joins_cleanup`, `tests/runner.rs:2787`, `RUNNER_BUSY`. Service 252 passed/one failed; installer279, Flutter456 and package41 passed; GTK canceled; build not reached. The historical JSON's “explicit user instruction forbids continuing” describes the validation delegation's stop-on-failure instruction, not an additional human instruction or restriction on later approved package work.
2. [Second aggregate attempt and intervening isolation](aggregate-attempt-2.json): the original teardown test passed alone (one test) and the full runner suite passed41 before another aggregate. The aggregate then failed the separate `vulkan_startup_observes_full_window_and_accepts_only_complete_fragmented_evidence` full-window assertion at `tests/runner.rs:636`. Stable 1,272 inputs, exit2; service252/one failed, installer279, Flutter470 and package41 passed; GTK canceled; build not reached.

No runner source/test relaxation or causal fix was made. Separate component passes above establish those executions only, not that either aggregate failure is harmless or resolved. Concurrent client inputs changed between attempts, not during their recorded command snapshots. No third full aggregate was attempted and no global PASS/accepted-risk decision is claimed.

## Real GNOME interoperability and limits

[Final log](gnome-integration.log), [summary](gnome-summary.json) and [runtime isolation](gnome-isolation.json): disposable Ubuntu24.04 GNOME Keyring `46.1-2ubuntu0.2`, D-Bus `1.14.10-4ubuntu4.1`, UID1000. Two encrypted synthetic five-attribute bootstrap items were successfully read through the actual production adapter, one per provider. Two separate fresh normal adapter Save/read pairs also passed, exactly one creation each. Final count was four unique unlocked items with equal Created/Modified and exact canary equality. Normal adapter-created items returned four attributes in this final run; private-peer tests exercise immediate five-attribute Save read-back.

Runtime had network none, no mounts, all capabilities dropped, no-new-privileges and resource bounds. The fixture alone prepared its private bus/collection; the real adapter retained fixed-bus, no-activation/no-unlock and three-second checks. Container removal was recorded. Test-only dependency image acquisition is not runtime egress. Cached direct-rustc fixture linkage is not a clean locked release build or full transitive attestation.

[Baseline log](gnome-baseline.log) and [baseline summary](gnome-baseline-summary.json) preserve both pre-fix schema-read conflicts and the normal Anthropic unknown Save/read failure; status is `UNEXPECTED_BASELINE_RESULT`, not green. All observed baseline timestamps were equal. The earlier bounded wait for the not-yet-applied parent change was blocked and is not counted as a test PASS. Timing/version universality and the separate timestamp-integrity concern remain unverified.

## Evidence location and authority

Raw command logs/statuses/inventories, source copies, fixture code and compiler/container commands remain under ignored `next/service/build/provider-schema-fix/`, with later attempts in `continuation-run/` and `package-validation/`. JSON here retains exact local command/log/source/artifact hashes; old MIME/cloud evidence was not overwritten. Before/after hashes are owner-controlled consistency evidence, not signed provenance or continuous monitoring. Source snapshot hashes exclude generated outputs and root validation-ledger changes as specified in the manifest.

No real key values, host bus/keyring access, installed service/manager/state mutation, vendor API/billing, item adoption/deletion, model download, commit or publication occurred in this implementation. The prior separately approved metadata-only host observation is contextual diagnosis, not secret/API validation. Installed upgrades remain a separate explicitly approved action after required gates and reviews; package verification alone is not that authority.
