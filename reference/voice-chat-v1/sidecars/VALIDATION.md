# T3 source self-check evidence

**Implemented/source-only**, 2026-10-09. Scope of edits: `next/sidecars/**`. Approved plan: `docs/plans/LOCAL-SMITH-NEXT-VOICE-CHAT.md`, T3. Parent owns canonical docs/integration and independent assurance; this is not self-approval.

| Check | Scenario/command | Observed result |
|---|---|---|
| Dependency-free API/loader/packaging tests | `PYTHONDONTWRITEBYTECODE=1 python3.12 -m unittest discover -s next/sidecars/tests` | PASS: 30 discovered, 27 passed, 3 optional NumPy tests skipped |
| Full fake/numeric suite | Same discovery command using existing Python 3.12 TTS venv | PASS: all 30, including synthetic PCM encoding and mocked transcription conversion |
| Installed dependency health | `python -m pip check` in each existing STT/TTS Python 3.12 venv | PASS: no broken requirements in either |
| Metadata-derived runtime closure | Parse all `requirements.txt` entries with packaging; compare installed versions and evaluate dependency specifiers/markers with no extras | PASS: 31 exact pins, 41 active transitive constraints satisfied |
| Compose static semantics | Parse YAML with installed PyYAML; assert service/network sets, numeric loopback ports, independent internal networks, one read-only bind per service, required env interpolation/no host path creation, nonroot/read-only/capdrop/security/resource/restart constraints | PASS; no Docker invocation |
| Python syntax | `ast.parse` each sidecar Python source | PASS: 7 files |
| Fixed startup failure | Subprocess invocation of `serve.py invalid`, also covered in the suite | Expected exit 1; stdout exactly `Speech service could not start.` plus newline, empty stderr |
| Whitespace | `git --no-pager diff --check -- next/sidecars` | PASS, but subtree is newly untracked and this command is not comprehensive formatting evidence |

API fixtures cover health schema, missing/corrupt model redaction, startup lifecycle, exact STT/TTS contracts, streamed caps despite dishonest Content-Length, media/path rejection, malformed/truncated WAV, UTF-8/strict JSON/unknown voice, input/output boundary durations and sizes, fixed errors, concurrent BUSY with responsive health, cancellation retaining admission until worker exits, body timeout/disconnect, local-only CPU/int8 construction, real-file CPU ONNX adapter calls, observed voices/no fallback and stdout safety.

## Explicitly not performed / remaining gates

- No normal-host servers/containers/providers, microphone capture, playback, weight downloads or real model inference.
- No image build, Compose up/config against real host model directories, clean combined dependency install or fresh dependency-registry resolution.
- No model/license/checksum verification, real-weight initialization, speech quality, CPU memory suitability or network-denied actual inference evidence.
- No independent fresh-context assurance/adversarial review performed by this writer; parent T4 must obtain it and integrate the quality command.
- No deployment, service/unit mutation, publication, remote effects or commit.

The source tests do not establish production-readiness or approve operator provisioning. Exact model formats, filenames, narrow required mounts, manual operator steps and packaging limitations are in `README.md` here.
