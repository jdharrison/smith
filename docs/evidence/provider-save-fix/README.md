# Provider Save MIME correction — source and isolated GNOME evidence

Work record: [LOCAL-SMITH-NEXT-PROVIDER-SAVE-FIX](../../plans/LOCAL-SMITH-NEXT-PROVIDER-SAVE-FIX.md). Base/HEAD `7f6a483ae82c4bcf647268f035f1e7cc80409d4b`, existing `feature/autonomy`, plus preserved uncommitted work. [Independent review](../../plans/LOCAL-SMITH-NEXT-PROVIDER-SAVE-FIX-REVIEW.md). [Validation ledger](../../../next/VALIDATION.md).

## Defect and correction

Both vendors share one local Save path. GNOME returns a secret with MIME `text/plain`; Smith previously required the exact `text/plain; charset=utf-8` spelling during encrypted read-back. The new [expected-red regression log](expected-red.log) records `KEYRING_CONFLICT; creates=1` against unchanged production, proving failure after publication in the fixture, not a failed vendor API request.

The correction accepts exactly GNOME's plain-text form and the prior UTF-8 form. The peer no longer echoes input MIME; it independently normalizes it. Fifteen focused keyring tests passed, including both supported forms and seven unsupported MIME cases. Encryption/session, bounded printable-ASCII bytes, read-back equality, attributes, ownership, timestamps, deadlines, no activation/unlock/Prompt, no retry and publication uncertainty remain unchanged. No past inactive item is adopted, removed or retried. This is not proof that every cause of the operator's installed Save failure is fixed.

## Stable combined gate

From `next/`:

```sh
env DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make test build
python3 dagger/package.py --root installer/dist --verify
```

Both exited 0, without timeout or rerun. Service **250**, installer **279**, isolated GTK **9**, Flutter **456** and package **41** tests passed; zero failures, zero Flutter skips and clean analyzer. Three service and two installer ordinary-harness ignores are controlled subprocess/fixture entry points, not skipped product acceptance.

All **1,267 inventoried inputs** matched before and after both commands. Only `next/service/src/cloud_keyring.rs` and `cloud_keyring_tests.rs` differ from the [previous cloud aggregate](../cloud-providers/README.md). Later validation-ledger/canonical/review documentation does not retroactively change those recorded inputs.

- [final.json](final.json): copied command/time/count/log/binary evidence, with paths relative to repository root.
- [source-manifest.json](source-manifest.json): input identities; digest `ad0cdd12d92cdf163b02fce7ab5d70e4ba8fa5160584fa5fe5df5bb6368e235c`.
- [artifacts.json](artifacts.json): all **97 package files**, including manifest/checksums; verification checks **95 manifest-listed files**. Inventory digest `3794c8684a179e6292b2cf7db37abce9b8886e19e138554d8316235fdcd3b3cb`.
- Standalone/client-bundled/installer-payload service bytes all match SHA256 `e5860b9e3751eddf1cefa5eb3b8464d31c1b3847c9d7c5c6fda6c1fc847169a1`.

Raw final logs, before/after lists and duplicate metadata remain in ignored `next/service/build/provider-save-fix/final-validation/`. Hashes detect stale mixing/accidental changes, not adversarial owner rewriting or release authenticity.

## Real GNOME, private container

[Sanitized integration log](gnome-integration.log) and [summary](gnome-summary.json) record a generated probe linked to the freshly built real `smith_service` library. It ran as UID1000 in a disposable Ubuntu24.04 container with GNOME Keyring `46.1-2ubuntu0.2`, D-Bus `1.14.10-4ubuntu4.1` and a private prestarted unlocked default collection. The adapter used its fixed `/run/user/1000/bus`, encrypted session and unchanged three-second budget. The harness prepared the synthetic collection, not Smith.

Exactly one Save and one explicit read succeeded per provider; synthetic canary equality was exact. The collection started empty and ended with two items. Runtime network was `none`, there were no mounts, capabilities were dropped, no-new-privileges was enabled and the container was removed. No normal-host bus/keyring or real credentials were accessed and no vendor HTTP was possible. Build-time package acquisition is not runtime offline/hermetic provenance.

The generated probe, harness, Dockerfile, runner, isolation metadata and compilation command are preserved locally under ignored `next/service/build/provider-save-fix/gnome-integration/`, with artifact hashes in the retained summary. Its repository-wide before/after comparison changed only because the independent review document appeared concurrently; service source/manifest/lock files were unchanged. This real-daemon fixture establishes compatibility with that tested version, not installed-user discovery, every keyring version, all timestamp conditions or live provider access. The existing `Created == Modified` concern is unchanged and has not been accepted as risk.

## Delivery boundary

The verified new recipient package is `next/installer/dist/`. It was not installed or launched. The installed service must be updated through a separately approved/operator-owned paired upgrade before this change can affect normal Save. No automatic retry, orphan cleanup, state migration, host lifecycle change, commit or publication occurred. OpenAI/Anthropic credits and model access are relevant to explicit API validation, not credential Save. Actual account/API billing, graphical acceptance and deployment remain separately authorized checks.
