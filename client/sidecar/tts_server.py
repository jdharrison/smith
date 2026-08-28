"""
smith Kokoro TTS sidecar — local speech for the Flutter client.

Listens on 127.0.0.1:4791 (loopback only).
Synthesizes text to a WAV under the system temp dir.

  GET  /v1/health
  GET  /v1/voices
  POST /v1/speak  {"v":1,"text":"Hello","voice":"am_onyx","speed":1.0}

Requires: .venv-tts + pip install -r requirements-tts.txt
Models:   models/kokoro-v1.0.onnx + models/voices-v1.0.bin
Run:      python tts_server.py

License: kokoro-onnx MIT; Kokoro weights Apache-2.0.
"""

from __future__ import annotations

import argparse
import os
import tempfile
import threading
import uuid
from contextlib import asynccontextmanager
from pathlib import Path
from typing import Any

import uvicorn
from fastapi import FastAPI, HTTPException
from fastapi.responses import JSONResponse
from pydantic import BaseModel, Field

DEFAULT_HOST = "127.0.0.1"
DEFAULT_PORT = 4791
DEFAULT_VOICE = "am_onyx"

_SIDECAR_DIR = Path(__file__).resolve().parent
_MODELS_DIR = _SIDECAR_DIR / "models"

_kokoro = None
_voices: list[str] = []
_default_voice = DEFAULT_VOICE
_load_error: str | None = None
_lock = threading.Lock()


def _model_paths() -> tuple[Path, Path]:
    onnx = Path(os.environ.get("SMITH_TTS_ONNX", str(_MODELS_DIR / "kokoro-v1.0.onnx")))
    voices = Path(
        os.environ.get("SMITH_TTS_VOICES_BIN", str(_MODELS_DIR / "voices-v1.0.bin"))
    )
    return onnx, voices


def load_kokoro() -> None:
    global _kokoro, _voices, _default_voice, _load_error
    onnx, voices_bin = _model_paths()
    if not onnx.is_file() or not voices_bin.is_file():
        _kokoro = None
        _voices = []
        _load_error = (
            f"models missing — expected {onnx.name} and {voices_bin.name} under "
            f"{_MODELS_DIR} (run `smith install`)"
        )
        print(f"[smith-tts] {_load_error}")
        return
    try:
        from kokoro_onnx import Kokoro

        k = Kokoro(str(onnx), str(voices_bin))
        names: list[str] = []
        try:
            raw = getattr(k, "voices", None)
            if isinstance(raw, dict):
                names = sorted(raw.keys())
            elif raw is not None:
                names = sorted(str(x) for x in raw)
        except Exception:  # noqa: BLE001
            names = []
        if not names:
            # Known v1.0 English male/female presets as fallback listing.
            names = [
                "af_sarah",
                "af_bella",
                "af_nicole",
                "am_adam",
                "am_onyx",
                "am_michael",
                "bm_george",
                "bm_lewis",
            ]
        env_voice = os.environ.get("SMITH_TTS_VOICE", DEFAULT_VOICE).strip()
        _default_voice = env_voice if env_voice in names else (
            DEFAULT_VOICE if DEFAULT_VOICE in names else names[0]
        )
        _voices = names
        _kokoro = k
        _load_error = None
        print(
            f"[smith-tts] loaded kokoro onnx={onnx.name} voices={len(names)} "
            f"default={_default_voice}"
        )
    except Exception as e:  # noqa: BLE001
        _kokoro = None
        _voices = []
        _load_error = str(e)
        print(f"[smith-tts] model load failed: {e}")
        if "espeak" in str(e).lower():
            print(
                "[smith-tts] hint: install espeak-ng and ensure it is on PATH "
                "(Windows: github.com/espeak-ng/espeak-ng/releases)"
            )


@asynccontextmanager
async def lifespan(_app: FastAPI):
    load_kokoro()
    yield


app = FastAPI(title="smith-tts", version="0.1.0", lifespan=lifespan)


class SpeakRequest(BaseModel):
    v: int = 1
    text: str = Field(..., min_length=1)
    voice: str | None = None
    speed: float = Field(default=1.0, ge=0.5, le=2.0)


@app.get("/v1/health")
def health() -> dict[str, Any]:
    return {
        "ok": _kokoro is not None,
        "service": "smith-tts",
        "backend": "kokoro",
        "voice": _default_voice,
        "voices": _voices,
        "error": _load_error,
    }


@app.get("/v1/voices")
def voices() -> dict[str, Any]:
    return {"voices": _voices, "default": _default_voice}


@app.post("/v1/speak")
def speak(body: SpeakRequest) -> JSONResponse:
    if _kokoro is None:
        raise HTTPException(
            status_code=503,
            detail=_load_error or "kokoro model not loaded",
        )

    text = body.text.strip()
    if not text:
        raise HTTPException(status_code=400, detail="text is required")

    voice = (body.voice or _default_voice).strip()
    if _voices and voice not in _voices:
        raise HTTPException(
            status_code=400,
            detail=f"unknown voice '{voice}'; try GET /v1/voices",
        )

    out = Path(tempfile.gettempdir()) / f"smith_tts_{uuid.uuid4().hex}.wav"
    sample_rate = 24000
    try:
        import soundfile as sf

        with _lock:
            samples, sample_rate = _kokoro.create(
                text, voice=voice, speed=float(body.speed)
            )
            sf.write(str(out), samples, sample_rate)
    except HTTPException:
        raise
    except Exception as e:  # noqa: BLE001
        if out.exists():
            try:
                out.unlink()
            except OSError:
                pass
        detail = str(e)
        if "espeak" in detail.lower():
            detail += (
                " — install espeak-ng (Windows release) and re-run smith install / tts"
            )
        raise HTTPException(status_code=500, detail=detail) from e

    return JSONResponse(
        {
            "v": 1,
            "path": str(out.resolve()),
            "voice": voice,
            "sample_rate": int(sample_rate),
        }
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="smith Kokoro TTS sidecar")
    parser.add_argument("--host", default=DEFAULT_HOST)
    parser.add_argument("--port", type=int, default=DEFAULT_PORT)
    parser.add_argument(
        "--voice",
        default=os.environ.get("SMITH_TTS_VOICE", DEFAULT_VOICE),
    )
    args = parser.parse_args()
    os.environ["SMITH_TTS_VOICE"] = args.voice
    uvicorn.run(app, host=args.host, port=args.port, log_level="info")


if __name__ == "__main__":
    main()
