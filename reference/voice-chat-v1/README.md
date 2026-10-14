# Voice/chat v1 — excluded reference snapshot

Preserved before the approved text-only reset. This directory is **not an active product, build input, installed bundle or deployment**. It preserves the voice experiment without a Git stash/commit or deletion of host models/data.

`MANIFEST.json` records 46 original source files, their source-relative provenance and SHA-256 identities. Snapshot manifest SHA-256: `93f57832bd27d0a20f06d4432dba9a8bf2595a1c3f00cda6f5e48cce5f3bbf2d`.

Contents:
- `client/`: old combined wizard/text/voice UI, audio capture/playback and associated tests/dependency inputs.
- `service/`: old text/voice provider operations, bounded durable worker, identity/lifecycle code and tests/dependency inputs.
- `sidecars/`: bounded raw-WAV Whisper/Kokoro APIs, constrained Compose/Docker packaging, exact Python pins and fake tests.
- `tooling/`: old Make/Dagger voice pipeline wiring.

This is a source reference, not a complete independently runnable release: shared Linux runner/build scripts, the new installer, generated artifacts and model weights are not copied. It contains no installed user state, model recordings, secrets or broad host mounts. Historical validation describes source/fake scenarios, not actual speech quality.

Current `next/` build/test/package functions receive explicit component/tooling inputs and never this directory. Do not reactivate archived providers, mounts, dependencies or audio paths implicitly. Reintroduction requires a bounded design, model/license review, migration/consent tests and independently approved host/provider/audio validation. Existing voice-bearing schema-v1 data is decoded by a narrow compatibility layer in the active text service; archived execution is not used.
