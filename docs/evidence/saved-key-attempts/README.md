# Saved-key explicit cloud attempts — retained evidence

**Implemented source; final configured aggregate/build/package gates PASS. Not installed-host repair, live-provider verification, release or deployment approval.** See the [bounded plan](../../plans/LOCAL-SMITH-NEXT-SAVED-KEY-ATTEMPTS.md), [independent review](../../plans/LOCAL-SMITH-NEXT-SAVED-KEY-ATTEMPTS-REVIEW.md) and [validation ledger](../../../next/VALIDATION.md).

## Change and boundary

The service cloud eligibility predicate now requires a supported model plus a configured exact active credential generation, not a current-instance keyring observation or successful previous validation. Flutter and installer catalog parsers use the same rule. Provider observations/errors and validation facts stay truthful and advisory; only the exact successfully checked model retains the individual-verification flag. Local readiness is unchanged.

Send and Validate still retrieve/recheck the admitted exact credential at execution. Real keyring failure stops before HTTP; readable invalid credentials can produce a sanitized provider failure. Existing consent, revisions, deadlines, storage/dispatch marking, unknown-operation reservations, no automatic retry/fallback, encrypted keyring and fixed HTTP endpoints remain unchanged. No inactive keyring item is adopted. Discovery and generated-catalog/upgrade-uninstall lifecycle remain analysis-only, not implemented here.

## Final aggregate and package evidence

[Final commands/results](final.json), [component results](component-results.json), [source manifest](source-manifest.json) and [export identities](artifacts.json) retain exact statuses, timestamps, log hashes and source/artifact binding.

From `next/`, run once after test-only fixture reconciliation:

```sh
env DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain make test build
python3 dagger/package.py --root installer/dist --verify
```

Both exited **0** on identical **1,291 inputs**. No standalone component fallback or separate build retry was required.

| Component | Passed | Controlled ignores / skips |
|---|---:|---|
| Service | 263 | Three fixture entry points |
| Native installer | 281 | Two fixture entry points |
| Isolated GTK | 9 | None |
| Flutter | 668 | Zero skipped |
| Package tests | 41 | Zero skipped |

Total **1,262 passed**, zero failed, five controlled fixture ignores. Parent/compiled integration gates execute their corresponding private fixtures; ignores are not live-provider evidence. Formatting checked81 files with zero changes; analyzer had zero issues; configured warning-denied Clippy and interpreter-free native-runtime gates passed. Package41 completed inside this aggregate on the final source and was not rerun standalone. Explicit recipient verification accepted **95 manifest-listed files**, 97 recipient files total.

- Source inventory SHA256: `ce939c47c54b6afdb41c8b79002cd989a63e7b38ebc81933e928fea7d2cb25c3`.
- All **189 exports** inventoried: one service, 91 client, 97 recipient files.
- All **four service copies** match SHA256 `5cfe584b8e597771319b3d4a3da9572bd30df9059d89f4f68e6e38fd5b2c4dcb`.
- All **91 standalone/client-payload pairs** match.
- Recipient manifest SHA256: `f2c46e2c42a595e3da9a48518dce428653a3dad258c88a6b1e12a72323c73e52`.
- Artifact inventory SHA256: `60e1f232d7519a411bd0754279e2a29d65b96050a5d3b72e5f319e1b801e5da6`.

Complete rebuilt candidate: `next/installer/dist/`. These identities establish local snapshot/export consistency, not signed provenance, clean-clone/hermetic reproducibility or installed compatibility. Concurrent New Window work is preserved and included in the final snapshot, not authored or independently approved by this change.

## Regression evidence and retained failure

Backend focused checks include both-provider observation/validation eligibility matrices, saved-only cold Send/Validate, exact fresh-reference reads, keyring failures with zero HTTP/not_sent, auth/quota one-attempt failures, explicit repair without probing, and preserved unsupported/stale-generation/consent/uncertainty negatives. The private Rust fixture keeps its default path and adds only test-only `SMITH_DART_CLOUD_SAVED_ONLY=1`; restart preserves fake keys while clearing volatile observations. Client focused164 includes six actual compiled Rust-Dart cases, no required preliminary probe/test, strict DTO negatives and advisory enlarged-layout checks. These counts overlap the aggregate and must not be added as unique tests.

[Initial client failure](initial-client-failure.json) retains **663 passed/three failed**, exit2. Its `_AvailabilityCloudFake` disabled configured active cloud choices, contradicting the deliberately changed strict DTO contract. H1 remediation changes only the entrance fixture/tests to model actual credential-generation replacement: new active choices stay attemptable/unverified while historical generations remain unavailable without fallback. Existing local outage behavior and entrance/history/focus/no-effect/no-replay assertions are preserved/strengthened. Two replacement cases were added. Remediation passed28 entrance, three unchanged-golden visual and164 focused cloud tests before the final aggregate668 client PASS. No production or golden change was used to repair H1.

Earlier expected-red gates, temporary scoped failures, source copies and exact patches remain under:

- `next/service/build/saved-key-attempts/backend/`
- `next/client/build/saved-key-attempts/`, with fixture reconciliation in `remediation/`
- `next/service/build/saved-key-attempts/final-validation/`, raw final logs/statuses/inventories

The initial snapshot and all181 prior evidence files were preserved by the final validator. The initial validation worker's note that assurance was parent-owned is not a final review disposition; use the linked independent report. Historical unrelated aggregate runner failures from earlier tasks also remain retained. This passing execution does not identify their causes or claim an unrelated runner fix; no runner production/test changes were made here.

## Authority and privacy

HEAD remains `7f6a483ae82c4bcf647268f035f1e7cc80409d4b`, branch `feature/autonomy`, with preserved dirty/untracked work. Source snapshots include untracked component inputs and exclude generated/cache roots as documented; subsequent root-document/ledger/evidence edits are not retroactive changes to tested inputs. Before/after hashes are owner-controlled drift evidence, not continuous monitoring or signed attestation.

No normal-host credential/keyring/service/manager/state access or mutation, authenticated vendor request, paid inference, model download, commit, branch mutation, release or publication occurred. The running installation remains unchanged. Paired updated components are necessary for the new schema-3 availability semantics; no old-client fallback or binary-only rollback guarantee is supplied. Real account/model acceptance, installed upgrade/migration/restore, graphical/IME acceptance and release/deployment remain separately authorized work.
