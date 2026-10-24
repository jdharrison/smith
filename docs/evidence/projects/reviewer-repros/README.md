# Preserved independent Projects repros

These four Dart files are unchanged copies of the independent fake-API fixtures. They perform no normal-session service, repository, credential, provider, model or GUI operation. Flutter widget tests use the test engine, not a launched desktop application.

| File | SHA-256 |
|---|---|
| `assurance_recovery_test.dart` | `b395fb737e3452339f5f87dc625e293a50c1310cf46fc6c260df312145252579` |
| `assurance_delete_restart_test.dart` | `07aa62ba88bbc95197ebb905d07564b990e4de235b9ce6a1926e34e718fb6761` |
| `projects_view_test.dart` | `64040a17600bea783cf41fbe62fc9eedb2a0eaf410e7c4d450d3a5f94db0b93d` |
| `app_fakes.dart` | `ca9badbd3bda514fe9cac17b518d30020c3ebfadaca3e58ceeef5b12fa9a35aa` |

## Cases

`assurance_recovery_test.dart` contains:

1. `a pre-restart dirty editor must not dispatch into a refreshed instance without review`
2. `an old delete confirmation must not dispatch into a refreshed instance`
3. `unknown original receipt must not disable all authoritative list observation`

`assurance_delete_restart_test.dart` contains:

1. `confirmed delete after restart clears the clean original selection without replay`
2. `confirmed delete after restart clears the dirty original selection without replay`

The helper `projects_view_test.dart` is a frozen historical fixture, not a substitute for the current production test suite. Invoke only the two independent repro files when reproducing these five cases. Their helpers' own `main` functions are not invoked by imports.

## Reproduce without editing production

Use an isolated client mirror with the reviewed candidate's complete `client/lib/`, assets, pubspec/lock and compatible already-cached Flutter dependency configuration. Copy these four files into that mirror's `test/` directory. Do not put them over production tests, install a toolchain, or use a normal service/manager/provider as a substitute for a missing fixture. Verify all production library paths/bytes against the candidate before and after running; rerunning an old copied `lib/` does not test a fix.

The retained review mirror is `target/projects-assurance/client/`. Its package configuration resolves `smith_client` to the mirror (`rootUri: "../"`). From that mirror, the recorded cached Dart executable runs the cached `flutter_tools.snapshot` with:

```text
--suppress-analytics test --no-pub --reporter expanded --concurrency 1 test/assurance_recovery_test.dart test/assurance_delete_restart_test.dart
```

Environment overrides: `CI=true`, `FLUTTER_ALREADY_LOCKED=true`, `FLUTTER_SUPPRESS_ANALYTICS=true`, `DART_SUPPRESS_ANALYTICS=true`. Exact executable arguments, SDK hashes, times and successful output are in `../reviewer-records/final-repros.json`, `verification.json` and `final-repros.log`. Five cases passed at the final review, with no skips or mutation replay.

`../reviewer-records/verify_bundle.py` is the independent, single-use binding/check recorder; it refuses to overwrite records. Preserve earlier records rather than rerunning it in place to replace a failure. The separate `--seal` mode verifies that parent inputs are still unchanged and binds the completed review. To perform another review attempt, use a separately named recorder/output set and bind it to that candidate.

Hashes are drift evidence, not an authenticity signature. The source-limited verdict and authority boundaries are in `../REVIEW.md`.
