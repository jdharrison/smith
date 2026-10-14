# LOCAL-SMITH-NEXT-SCAFFOLD

## Scope and authority

User-approved local barebones initialization following the product reset: `next/client` (Flutter, Linux only), `next/service` (Rust), and `next/dagger` (developer-only orchestration). The former `next/smith-*` README-only mocks are renamed/consolidated; CLI implementation is deferred. Base checkpoint: `04b72f5`, existing `feature/autonomy` bootstrap branch retained. No commit, push, branch rename, publication, host installation, or service mutation is authorized here.

Risk: elevated developer pipeline/dependencies and client shell; no new application runtime capabilities. Existing root Rust and Flutter implementations remain untouched. Flutter dependency resolution/builds may access package registries and write local build/cache data. Dagger engine execution, installation, image pulls, and real-host graphical launch are not performed by this scope.

## Independent inputs and reconciliation

Independent producer, architect, and designer read-only reviews agree on an inert shell and explicit development disclosure. Architect flags Dagger images/source access as reviewable despite dev-only status. Designer requires no fake readiness or controls and narrow/large-text tests. Producer requires Dagger unavailability to remain a reported validation gap.

Naming disagreement resolved by the user's latest agreed concise client/service layout. Use a standalone service Cargo package/library with an explicit local workspace boundary; component-local `target/` and lockfile avoid changing the legacy root package. A multi-package Rust workspace can be added when the terminal client exists.

## Traceability

| Requirement / acceptance | Task | Evidence |
|---|---|---|
| R1 / A1: static, truthful Linux desktop shell; no runtime calls | T1: Flutter scaffold with one screen and widget tests | Analyzer, tests at compact/large text sizes, Linux build |
| R2 / A2: reusable Rust library and finite scaffold binary; no dependencies/listener | T2: standalone Cargo package, native scripts | fmt, strict Clippy, locked tests/build; source review |
| R3 / A3: developer-only component checks/builds, no publication | T3: Python Dagger module with explicit source inputs | Syntax/config review; engine execution deferred because CLI absent |
| R4 / A4: isolated source/output and unchanged legacy paths | T4: concise directories, ignore rules, documentation | Git diff/status, artifact exclusion review |

SDK choice: Python for small pipeline functions, separate from product-language choices. Dagger engine/SDK target v0.21.0 (stable release rather than 1.0 beta); official function/template references consulted. Container tags and Flutter release are version-selected, not digest-pinned; no hermetic/reproducible-release claim. Module load/runtime verification and image availability remain unverified until Dagger can be run under a separately bounded development-tool scope.

## Gates and exclusions

Plan → local development → self-check → fresh-context assurance. Record actual results separately from proposed commands. Dagger execution and graphical launch gaps block claiming integrated pipeline/runtime validation, not creation of the source scaffold. No release is in scope.

Defer API/listeners, setup/chat, providers, credentials, persistence, backend ownership, Rust UI/FFI, CLI, packaging, release preparation/publication, and old-source cutover/removal. Model services, systemd/autostart, external providers and existing data remain untouched.

Rollback is limited to reverting this scaffold's sources/ignore changes while preserving user changes and generated caches; never remove legacy implementation or user data. No automatic destructive cleanup.
