"""
smith Whisper sidecar — local STT for the Flutter client.

Listens on 127.0.0.1:4790 (loopback only).
Transcribes a WAV path already on this machine (no upload).

  GET  /v1/health
  POST /v1/transcribe  {"v":1,"path":"C:/.../smith_clip_1.wav"}

Requires: pip install -r requirements.txt
Run:     python whisper_server.py
"""

from __future__ import annotations

import argparse
import os
import tempfile
import threading
from pathlib import Path
from typing import Any

from contextlib import asynccontextmanager

import uvicorn
from fastapi import FastAPI, HTTPException
from fastapi.responses import JSONResponse
from pydantic import BaseModel, Field

DEFAULT_HOST = "127.0.0.1"
DEFAULT_PORT = 4790

_model = None
_model_name = "base.en"
_device = "cpu"
_compute = "int8"
_model_lock = threading.Lock()
_load_error: str | None = None


def load_model(name: str, device: str, compute: str) -> None:
    global _model, _model_name, _device, _compute, _load_error
    _model_name = name
    _device = device
    _compute = compute
    try:
        from faster_whisper import WhisperModel

        _model = WhisperModel(name, device=device, compute_type=compute)
        _load_error = None
        print(f"[smith-whisper] loaded model={name} device={device} compute={compute}")
    except Exception as e:  # noqa: BLE001
        _model = None
        _load_error = str(e)
        print(f"[smith-whisper] model load failed: {e}")


@asynccontextmanager
async def lifespan(_app: FastAPI):
    name = os.environ.get("SMITH_WHISPER_MODEL", _model_name)
    device = os.environ.get("SMITH_WHISPER_DEVICE", _device)
    compute = os.environ.get("SMITH_WHISPER_COMPUTE", _compute)
    load_model(name, device, compute)
    yield


app = FastAPI(title="smith-whisper", version="0.1.0", lifespan=lifespan)


class TranscribeRequest(BaseModel):
    v: int = 1
    path: str = Field(..., min_length=1)


def _allowed_roots() -> list[Path]:
    roots = [
        Path(tempfile.gettempdir()).resolve(),
        Path.home().resolve(),
    ]
    extra = os.environ.get("SMITH_WHISPER_ALLOW_ROOT")
    if extra:
        roots.append(Path(extra).expanduser().resolve())
    return roots


def _assert_local_wav(path: str) -> Path:
    raw = Path(path).expanduser()
    try:
        resolved = raw.resolve(strict=True)
    except FileNotFoundError as e:
        raise HTTPException(status_code=400, detail=f"file not found: {path}") from e

    if resolved.suffix.lower() not in {".wav", ".wave"}:
        raise HTTPException(status_code=400, detail="only .wav files are accepted")

    allowed = False
    for root in _allowed_roots():
        try:
            resolved.relative_to(root)
            allowed = True
            break
        except ValueError:
            continue
    if not allowed:
        raise HTTPException(
            status_code=403,
            detail="path not under allowed roots (temp, home, or SMITH_WHISPER_ALLOW_ROOT)",
        )
    return resolved


@app.get("/v1/health")
def health() -> dict[str, Any]:
    return {
        "ok": _model is not None,
        "service": "smith-whisper",
        "model": _model_name,
        "device": _device,
        "error": _load_error,
    }


@app.post("/v1/transcribe")
def transcribe(body: TranscribeRequest) -> JSONResponse:
    if _model is None:
        raise HTTPException(
            status_code=503,
            detail=_load_error or "whisper model not loaded",
        )

    path = _assert_local_wav(body.path)

    try:
        with _model_lock:
            segments, info = _model.transcribe(
                str(path),
                language="en",
                beam_size=1,
                vad_filter=True,
            )
            parts: list[str] = []
            for seg in segments:
                t = (seg.text or "").strip()
                if t:
                    parts.append(t)
        text = " ".join(parts).strip()
    except HTTPException:
        raise
    except Exception as e:  # noqa: BLE001
        raise HTTPException(status_code=500, detail=str(e)) from e

    return JSONResponse(
        {
            "v": 1,
            "text": text,
            "language": getattr(info, "language", "en"),
            "duration": getattr(info, "duration", None),
        }
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="smith Whisper sidecar")
    parser.add_argument("--host", default=DEFAULT_HOST)
    parser.add_argument("--port", type=int, default=DEFAULT_PORT)
    parser.add_argument(
        "--model",
        default=os.environ.get("SMITH_WHISPER_MODEL", "base.en"),
    )
    parser.add_argument(
        "--device",
        default=os.environ.get("SMITH_WHISPER_DEVICE", "cpu"),
    )
    parser.add_argument(
        "--compute",
        default=os.environ.get("SMITH_WHISPER_COMPUTE", "int8"),
    )
    args = parser.parse_args()

    os.environ["SMITH_WHISPER_MODEL"] = args.model
    os.environ["SMITH_WHISPER_DEVICE"] = args.device
    os.environ["SMITH_WHISPER_COMPUTE"] = args.compute

    uvicorn.run(app, host=args.host, port=args.port, log_level="info")


if __name__ == "__main__":
    main()
