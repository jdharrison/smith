# smith Whisper + Kokoro TTS sidecars
#
# Whisper STT — 127.0.0.1:4790
# Kokoro TTS  — 127.0.0.1:4791
#
# Prefer `smith install` (venvs + Kokoro model download) and `smith serve`
# (starts both). Manual steps below are fallbacks.

## Whisper STT (`:4790`)

Prefer **`smith install`** — creates `.venv`, installs `requirements.txt` when
missing or outdated.

Manual:

```bash
cd client/sidecar
python -m venv .venv
# Windows: .venv\Scripts\activate
pip install -r requirements.txt
python whisper_server.py
```

| Method | Path | Body |
|--------|------|------|
| GET | `/v1/health` | — |
| POST | `/v1/transcribe` | `{"v":1,"path":"<absolute wav path>"}` |

## Kokoro TTS (`:4791`)

Local neural TTS (kokoro-onnx **MIT**, Kokoro weights **Apache-2.0**). Separate
venv `.venv-tts` and ONNX files under `models/`.

Prefer **`smith install`** (downloads `kokoro-v1.0.onnx` + `voices-v1.0.bin`).

Manual:

```bash
cd client/sidecar
python -m venv .venv-tts
# Windows: .venv-tts\Scripts\activate
pip install -r requirements-tts.txt
# place models/kokoro-v1.0.onnx and models/voices-v1.0.bin
# from https://github.com/thewh1teagle/kokoro-onnx/releases (model-files-v1.0)
python tts_server.py
```

If load/speak fails mentioning **espeak**, install [espeak-ng](https://github.com/espeak-ng/espeak-ng/releases)
and ensure it is on `PATH`.

| Method | Path | Body |
|--------|------|------|
| GET | `/v1/health` | — |
| GET | `/v1/voices` | — |
| POST | `/v1/speak` | `{"v":1,"text":"Hello","voice":"am_onyx","speed":1.0}` |

Default voice: `am_onyx` (override with `SMITH_TTS_VOICE`). Skip with `SMITH_TTS=0`.

## Client

Flutter records → Whisper; replies → Kokoro when online, else OS SAPI / flutter_tts.
`smith serve` starts Whisper + TTS. Flutter: `flutter run -d windows`.
