# Exact recorded command ledger

All paths below are sanitized. Each linked JSON retains the complete allowlisted environment, executable identity, timestamps, timeout, exit, before/after core identities and log digest. Logs and intermediate failures are retained. Times are UTC. A `not_run` record has no exit code and is not a pass.

## rust-bootstrap

```sh
cargo test --manifest-path Cargo.toml --locked --offline --lib projects_bootstrap
```

- Working directory: `<repo>/target/default-project/validation/candidate/service`
- Started: `2026-10-17T23:21:57.941952+00:00`; finished: `2026-10-17T23:22:11.346534+00:00`
- Timeout: 240s; elapsed: 13.405s; exit: `0`; timed out: `False`; status: **pass**
- Live core stable during command: `True`.
- [Record](rust-bootstrap.json) · [Log](rust-bootstrap.log)

## rust-projects

```sh
cargo test --manifest-path Cargo.toml --locked --offline --lib projects_
```

- Working directory: `<repo>/target/default-project/validation/candidate/service`
- Started: `2026-10-17T23:22:11.348039+00:00`; finished: `2026-10-17T23:22:15.109260+00:00`
- Timeout: 240s; elapsed: 3.761s; exit: `101`; timed out: `False`; status: **fail**
- Live core stable during command: `True`.
- [Record](rust-projects.json) · [Log](rust-projects.log)

## rust-production-clippy

```sh
cargo clippy --manifest-path Cargo.toml --locked --offline --lib -- -D warnings
```

- Working directory: `<repo>/target/default-project/validation/candidate/service`
- Started: `2026-10-17T23:22:15.110857+00:00`; finished: `2026-10-17T23:22:23.643622+00:00`
- Timeout: 240s; elapsed: 8.533s; exit: `0`; timed out: `False`; status: **pass**
- Live core stable during command: `True`.
- [Record](rust-production-clippy.json) · [Log](rust-production-clippy.log)

## rust-all-target-clippy

```sh
cargo clippy --manifest-path Cargo.toml --locked --offline --all-targets -- -D warnings
```

- Working directory: `<repo>/target/default-project/validation/candidate/service`
- Started: `2026-10-17T23:22:23.645213+00:00`; finished: `2026-10-17T23:22:29.114122+00:00`
- Timeout: 240s; elapsed: 5.469s; exit: `101`; timed out: `False`; status: **fail**
- Live core stable during command: `True`.
- [Record](rust-all-target-clippy.json) · [Log](rust-all-target-clippy.log)

## rust-format

```sh
cargo fmt --manifest-path Cargo.toml --all -- --check
```

- Working directory: `<repo>/target/default-project/validation/candidate/service`
- Started: `2026-10-17T23:22:29.115727+00:00`; finished: `2026-10-17T23:22:29.730591+00:00`
- Timeout: 60s; elapsed: 0.615s; exit: `0`; timed out: `False`; status: **pass**
- Live core stable during command: `True`.
- [Record](rust-format.json) · [Log](rust-format.log)

## fixture-rebuild

```sh
cargo test --manifest-path Cargo.toml --locked --offline --test debug_service --no-run
```

- Working directory: `<repo>/target/default-project/validation/candidate/service`
- Started: `2026-10-17T23:22:29.732194+00:00`; finished: `2026-10-17T23:22:34.374675+00:00`
- Timeout: 240s; elapsed: 4.642s; exit: `0`; timed out: `False`; status: **pass**
- Live core stable during command: `True`.
- [Record](fixture-rebuild.json) · [Log](fixture-rebuild.log)

## fixture-control-disk5_projects_control_seeds_real_legacy_bytes_and_reopens_without_effects

```sh
cargo test --manifest-path Cargo.toml --locked --offline --test debug_service dart_cloud::disk5_projects_control_seeds_real_legacy_bytes_and_reopens_without_effects -- --exact
```

- Working directory: `<repo>/target/default-project/validation/candidate/service`
- Started: `2026-10-17T23:22:34.420145+00:00`; finished: `2026-10-17T23:22:34.901328+00:00`
- Timeout: 240s; elapsed: 0.481s; exit: `0`; timed out: `False`; status: **pass**
- Live core stable during command: `True`.
- [Record](fixture-control-disk5_projects_control_seeds_real_legacy_bytes_and_reopens_without_effects.json) · [Log](fixture-control-disk5_projects_control_seeds_real_legacy_bytes_and_reopens_without_effects.log)

## fixture-control-bootstrap_modes_keep_fake_keys_and_restart_resets_only_volatile_facts

```sh
cargo test --manifest-path Cargo.toml --locked --offline --test debug_service dart_cloud::bootstrap_modes_keep_fake_keys_and_restart_resets_only_volatile_facts -- --exact
```

- Working directory: `<repo>/target/default-project/validation/candidate/service`
- Started: `2026-10-17T23:22:34.902934+00:00`; finished: `2026-10-17T23:22:35.436719+00:00`
- Timeout: 240s; elapsed: 0.534s; exit: `0`; timed out: `False`; status: **pass**
- Live core stable during command: `True`.
- [Record](fixture-control-bootstrap_modes_keep_fake_keys_and_restart_resets_only_volatile_facts.json) · [Log](fixture-control-bootstrap_modes_keep_fake_keys_and_restart_resets_only_volatile_facts.log)

## negative-control-projects

```sh
cargo test --manifest-path Cargo.toml --locked --offline --lib projects_
```

- Working directory: `<repo>/target/default-project/validation/negative-control/service`
- Started: `2026-10-17T23:22:35.438350+00:00`; finished: `2026-10-17T23:22:40.589951+00:00`
- Timeout: 240s; elapsed: 5.152s; exit: `101`; timed out: `False`; status: **fail**
- Live core stable during command: `True`.
- [Record](negative-control-projects.json) · [Log](negative-control-projects.log)
- **INVALID baseline provenance:** shared target reused candidate artifacts. Not used for baseline attribution. See replacement isolated attempts.

## negative-control-all-target-clippy

```sh
cargo clippy --manifest-path Cargo.toml --locked --offline --all-targets -- -D warnings
```

- Working directory: `<repo>/target/default-project/validation/negative-control/service`
- Started: `2026-10-17T23:22:40.591724+00:00`; finished: `2026-10-17T23:22:44.067708+00:00`
- Timeout: 240s; elapsed: 3.476s; exit: `101`; timed out: `False`; status: **fail**
- Live core stable during command: `True`.
- [Record](negative-control-all-target-clippy.json) · [Log](negative-control-all-target-clippy.log)
- **INVALID baseline provenance:** shared target reused candidate artifacts. Not used for baseline attribution. See replacement isolated attempts.

## flutter-focused-projects

```sh
'<home>/snap/flutter/common/flutter/bin/cache/dart-sdk/bin/dart' '<home>/snap/flutter/common/flutter/bin/cache/flutter_tools.snapshot' --suppress-analytics --no-version-check test --no-pub --reporter expanded test/projects_view_test.dart test/projects_navigation_test.dart test/projects_layout_test.dart test/projects_layout_golden_test.dart test/projects_controller_test.dart test/projects_protocol_test.dart
```

- Working directory: `<repo>/target/default-project/validation/candidate/client`
- Started: `2026-10-17T23:22:49.385070+00:00`; finished: `2026-10-17T23:23:06.760470+00:00`
- Timeout: 240s; elapsed: 17.375s; exit: `1`; timed out: `False`; status: **fail**
- Live core stable during command: `True`.
- [Record](flutter-focused-projects.json) · [Log](flutter-focused-projects.log)

## flutter-assets

```sh
'<home>/snap/flutter/common/flutter/bin/cache/dart-sdk/bin/dart' '<home>/snap/flutter/common/flutter/bin/cache/flutter_tools.snapshot' --suppress-analytics --no-version-check test --no-pub --reporter expanded test/ui_foundation_assets_test.dart
```

- Working directory: `<repo>/target/default-project/validation/candidate/client`
- Started: `2026-10-17T23:23:06.762192+00:00`; finished: `2026-10-17T23:23:09.141196+00:00`
- Timeout: 120s; elapsed: 2.379s; exit: `0`; timed out: `False`; status: **pass**
- Live core stable during command: `True`.
- [Record](flutter-assets.json) · [Log](flutter-assets.log)

## flutter-compiled-projects

```sh
'<home>/snap/flutter/common/flutter/bin/cache/dart-sdk/bin/dart' '<home>/snap/flutter/common/flutter/bin/cache/flutter_tools.snapshot' --suppress-analytics --no-version-check test --no-pub --reporter expanded test/rust_projects_integration_test.dart
```

- Working directory: `<repo>/target/default-project/validation/candidate/client`
- Started: `2026-10-17T23:23:09.142822+00:00`; finished: `2026-10-17T23:23:12.522289+00:00`
- Timeout: 180s; elapsed: 3.379s; exit: `0`; timed out: `False`; status: **pass**
- Live core stable during command: `True`.
- [Record](flutter-compiled-projects.json) · [Log](flutter-compiled-projects.log)

## flutter-scoped-analyze

```sh
'<home>/snap/flutter/common/flutter/bin/cache/dart-sdk/bin/dart' '<home>/snap/flutter/common/flutter/bin/cache/flutter_tools.snapshot' --suppress-analytics --no-version-check analyze --no-pub lib/projects.dart lib/projects_controller.dart lib/ui/projects_view.dart test/rust_projects_integration_test.dart
```

- Working directory: `<repo>/target/default-project/validation/candidate/client`
- Started: `2026-10-17T23:23:12.523966+00:00`; finished: `2026-10-17T23:23:15.393205+00:00`
- Timeout: 180s; elapsed: 2.869s; exit: `0`; timed out: `False`; status: **pass**
- Live core stable during command: `True`.
- [Record](flutter-scoped-analyze.json) · [Log](flutter-scoped-analyze.log)

## flutter-format

```sh
'<home>/snap/flutter/common/flutter/bin/cache/dart-sdk/bin/dart' format --output=none --set-exit-if-changed lib test
```

- Working directory: `<repo>/target/default-project/validation/candidate/client`
- Started: `2026-10-17T23:23:15.394847+00:00`; finished: `2026-10-17T23:23:16.319368+00:00`
- Timeout: 90s; elapsed: 0.925s; exit: `0`; timed out: `False`; status: **pass**
- Live core stable during command: `True`.
- [Record](flutter-format.json) · [Log](flutter-format.log)

## dagger-version

```sh
'<repo>/target/installer-pipeline/smith-dagger-0.21.9/dagger' version
```

- Working directory: `<repo>`
- Started: `2026-10-17T23:23:20.415105+00:00`; finished: `2026-10-17T23:23:20.821470+00:00`
- Timeout: 15s; elapsed: 0.406s; exit: `0`; timed out: `False`; status: **pass**
- Live core stable during command: `True`.
- [Record](dagger-version.json) · [Log](dagger-version.log)

## engine-observation

```sh
docker inspect --format '{"running":{{json .State.Running}},"image":{{json .Config.Image}},"started_at":{{json .State.StartedAt}}}' smith-installer-pipeline-dagger-v0.21.9
```

- Working directory: `<repo>`
- Started: `2026-10-17T23:23:20.823210+00:00`; finished: `2026-10-17T23:23:21.222898+00:00`
- Timeout: 15s; elapsed: 0.4s; exit: `0`; timed out: `False`; status: **pass**
- Live core stable during command: `True`.
- [Record](engine-observation.json) · [Log](engine-observation.log)

## root-gates

```sh
make DAGGER=target/installer-pipeline/smith-dagger-0.21.9/dagger test build
```

- Working directory: `<repo>`
- Started: `2026-10-17T23:23:21.224530+00:00`; finished: `2026-10-17T23:23:52.920280+00:00`
- Timeout: 900s; elapsed: 31.696s; exit: `2`; timed out: `False`; status: **fail**
- Live core stable during command: `True`.
- [Record](root-gates.json) · [Log](root-gates.log)

## package-verify

```sh
python3 -B dagger/package.py --root installer/dist --verify
```

- Working directory: `<repo>`
- Started: `2026-10-17T23:23:52.922087+00:00`; finished: `2026-10-17T23:23:53.285328+00:00`
- Timeout: 60s; elapsed: 0.363s; exit: `None`; timed out: `False`; status: **not_run**
- Live core stable during command: `True`.
- [Record](package-verify.json) · [Log](package-verify.log)
- Not-run reason: Root rebuild did not succeed; stale exports are not current-candidate evidence

## pipeline-helpers

```sh
python3 -B -m unittest discover -s scripts/tests -p 'test_*.py' -v
```

- Working directory: `<repo>`
- Started: `2026-10-17T23:23:53.286910+00:00`; finished: `2026-10-17T23:23:53.649440+00:00`
- Timeout: 120s; elapsed: 0.363s; exit: `None`; timed out: `False`; status: **not_run**
- Live core stable during command: `True`.
- [Record](pipeline-helpers.json) · [Log](pipeline-helpers.log)
- Not-run reason: Root rebuild did not succeed; stale exports are not current-candidate evidence

## diff-check

```sh
git --no-pager diff --check
```

- Working directory: `<repo>`
- Started: `2026-10-17T23:23:53.651098+00:00`; finished: `2026-10-17T23:23:54.037350+00:00`
- Timeout: 15s; elapsed: 0.386s; exit: `0`; timed out: `False`; status: **pass**
- Live core stable during command: `True`.
- [Record](diff-check.json) · [Log](diff-check.log)

## negative-control-isolated-projects

```sh
cargo test --manifest-path Cargo.toml --locked --offline --lib projects_
```

- Working directory: `<repo>/target/default-project/validation/negative-control/service`
- Started: `2026-10-17T23:24:21.680118+00:00`; finished: `2026-10-17T23:24:39.609333+00:00`
- Timeout: 300s; elapsed: 17.929s; exit: `101`; timed out: `False`; status: **fail**
- Live core stable during command: `True`.
- [Record](negative-control-isolated-projects.json) · [Log](negative-control-isolated-projects.log)

## negative-control-isolated-all-target-clippy

```sh
cargo clippy --manifest-path Cargo.toml --locked --offline --all-targets -- -D warnings
```

- Working directory: `<repo>/target/default-project/validation/negative-control/service`
- Started: `2026-10-17T23:24:39.610876+00:00`; finished: `2026-10-17T23:24:48.569203+00:00`
- Timeout: 300s; elapsed: 8.958s; exit: `101`; timed out: `False`; status: **fail**
- Live core stable during command: `True`.
- [Record](negative-control-isolated-all-target-clippy.json) · [Log](negative-control-isolated-all-target-clippy.log)

## flutter-full-analyze

```sh
'<home>/snap/flutter/common/flutter/bin/cache/dart-sdk/bin/dart' '<home>/snap/flutter/common/flutter/bin/cache/flutter_tools.snapshot' --suppress-analytics --no-version-check analyze --no-pub
```

- Working directory: `<repo>/target/default-project/validation/candidate/client`
- Started: `2026-10-17T23:25:32.528458+00:00`; finished: `2026-10-17T23:25:36.156286+00:00`
- Timeout: 180s; elapsed: 3.628s; exit: `0`; timed out: `False`; status: **pass**
- Live core stable during command: `True`.
- [Record](flutter-full-analyze.json) · [Log](flutter-full-analyze.log)

## rust-toolchain-version

```sh
rustc --version --verbose
```

- Working directory: `<repo>`
- Started: `2026-10-17T23:25:36.157832+00:00`; finished: `2026-10-17T23:25:36.552796+00:00`
- Timeout: 15s; elapsed: 0.395s; exit: `0`; timed out: `False`; status: **pass**
- Live core stable during command: `True`.
- [Record](rust-toolchain-version.json) · [Log](rust-toolchain-version.log)

## cargo-toolchain-version

```sh
cargo --version
```

- Working directory: `<repo>`
- Started: `2026-10-17T23:25:36.554362+00:00`; finished: `2026-10-17T23:25:36.943278+00:00`
- Timeout: 15s; elapsed: 0.389s; exit: `0`; timed out: `False`; status: **pass**
- Live core stable during command: `True`.
- [Record](cargo-toolchain-version.json) · [Log](cargo-toolchain-version.log)

## engine-final-observation

```sh
docker inspect --format '{"running":{{json .State.Running}},"image":{{json .Config.Image}},"started_at":{{json .State.StartedAt}}}' smith-installer-pipeline-dagger-v0.21.9
```

- Working directory: `<repo>`
- Started: `2026-10-17T23:25:36.944845+00:00`; finished: `2026-10-17T23:25:37.343585+00:00`
- Timeout: 15s; elapsed: 0.399s; exit: `0`; timed out: `False`; status: **pass**
- Live core stable during command: `True`.
- [Record](engine-final-observation.json) · [Log](engine-final-observation.log)

