# Retained independent adversarial repros

These are reviewer-owned copies, not replacements for production tests. Original failures and successful closures are distinct immutable records. Source-copy equality is checked by `../verify_bundle.py` against the original scratch artifacts.

## Runnable retained Dart cases

From repository root, the bounded recorder uses the existing local Flutter SDK, does not resolve packages, and writes only a new reviewer record:

```sh
python3 -B docs/evidence/project-repositories/reviewer/run_retained_repros.py
```

It runs the **unchanged** `duplicate_reply_test.dart` (three expected-rejection cases) and `../parser-followup-1/parser_adversarial_test.dart` (seven independent malformed/Unicode/string/depth/frame-boundary/generation cases). The first three originally failed because malformed duplicate-key responses were accepted. The original failed log/result are `../records/duplicates-original.{log,json}`; the prior source-bound passing closure is `../parser-followup-1/unchanged-repro.{log,json}`. A passing current-source rerun does not relabel the old run.

Exact earlier commands, source hashes, timestamps, exits and log digests remain in the copied JSON records. The copied earlier recorder scripts are historical command evidence: they retain their original scratch-layout assumptions and should not be invoked from their relocated documentation paths.

## Representative native and UI cases

- `reviewer_native.rs`: three reviewer-only tests for duplicate-member disk5/disk6 preservation, null migration/unrelated saves/shared-ID/legacy erasure refusal, and ASCII/total-pattern boundaries. It was appended only to an isolated copy of `service/src/app_projects_tests.rs`; no production test was modified. `../records/native-extra-0.log` contains three new tests plus seven pre-existing tests selected by the broad `assurance_` filter, not ten newly authored tests. The actual disk5 seed/reopen check is separately recorded in `native-extra-1.log`.
- `reviewer_test.dart`: nine widget cases covering each of eight fields independently through restart and newer-input recovery, plus null-preserving unrelated save and dirty-delete recovery. This copy deliberately retains the original relative helper import (`../../../client/test/projects_view_test.dart`). It is source evidence, not runnable at this relocated path: recreate the original `target/project-repositories/assurance/` layout in a new isolated reproduction checkout before running it. The passing result is `../records/reviewer.{log,json}`. Two earlier reviewer setup failures (wrong button type, then missing widget pump) remain separately preserved in the original scratch evidence indexed by the parent bundle; neither was a product fix or weakened product assertion.

No retained fixture authorizes a real repository operation, installed migration, live provider/keyring access, application launch, publication or deployment. Archive/helper tests use synthetic Git identities and mutations only in private disposable fixture repositories; the real source checkout was not mutated by those tests.
