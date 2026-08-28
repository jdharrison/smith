# smith client

Flutter desktop UI for smith (Windows / macOS / Linux). Voice-first chat against
`smith serve` on `127.0.0.1:4783`.

## Tabs

**Chat | Tasks | Settings** — no Chat|Dev session toggle. Repo ship/pipeline work is a
task kind from intent, not a chat mode.

- **Chat** — messages + hold-to-talk
- **Tasks** — host task board (status, kind, result); tap for detail
- **Settings** — TTS backend/voice/rate, Whisper + bridge status; CLI still owns
  projects/agents/pipelines

## Async flow

Every user message creates a board task. Fast replies complete in the HTTP response.
Search / files / code / pipeline return an ack immediately; the client polls
`GET /v1/tasks` (~2s) while work is running and **injects an assistant row** (with TTS
if unmuted) when the task completes or fails.

## Run

```bash
# Terminal 1 — after `smith install` (Whisper + Kokoro TTS)
smith serve          # or: smith serve --echo  — Whisper :4790 + TTS :4791

# Terminal 2 — UI
cd client
flutter run -d windows
```

`smith install` creates `client/sidecar/.venv` (Whisper) and `.venv-tts` (Kokoro),
and downloads Kokoro ONNX models under `models/` when missing. Manual sidecar setup
is only a fallback — see [sidecar/README.md](sidecar/README.md).

## Voice path

Hold mic → record WAV → release → Whisper (`127.0.0.1:4790`) transcribes → typewriter
into the chat bubble → send to smith. WinRT `speech_to_text` is not used.

After each voice-turn reply (and on task-result inject), TTS prefers **Kokoro** on
`127.0.0.1:4791` (Apache-2.0 weights / MIT runner). If that sidecar is down, falls back
to OS TTS (Windows: SAPI via PowerShell; macOS/Linux: `flutter_tts`). Mute/unmute from
the app bar volume icon. Tune voice/rate under **Settings**.

Windows: allow microphone access under **Settings → Privacy → Microphone**.
If Kokoro fails mentioning espeak, install [espeak-ng](https://github.com/espeak-ng/espeak-ng/releases).

Windows build note: `flutter_tts` needs **NuGet** on `PATH` (CppWinRT). Install once:

```bash
winget install --id Microsoft.NuGet -e
```

Then open a new terminal (or refresh PATH) before `flutter run -d windows`.
