"""Bounded, raw-byte speech API. No HTTP request can name a filesystem path."""
import asyncio
import io
import json
import struct
import threading
import wave

STT_BYTES = 2 * 1024 * 1024
TTS_BYTES = 2_880_044
TEXT_BYTES = 4096
JSON_BYTES = 32 * 1024  # permits escaped UTF-8 text without unbounded JSON parsing
READ_SECONDS = 10


class Failure(Exception):
    def __init__(self, code, status=400):
        self.code = code
        self.status = status


def wav_input(data):
    """Require a complete RIFF PCM16 WAV, including trustworthy frame bounds."""
    try:
        if len(data) < 44 or data[:4] != b"RIFF" or data[8:12] != b"WAVE":
            raise ValueError
        if struct.unpack_from("<I", data, 4)[0] + 8 != len(data):
            raise ValueError
        # wave accepts truncated payloads; check every RIFF chunk ourselves.
        offset, fmt, pcm = 12, None, None
        while offset < len(data):
            if offset + 8 > len(data):
                raise ValueError
            name, size = struct.unpack_from("<4sI", data, offset)
            offset += 8
            end = offset + size
            if end + (size % 2) > len(data):
                raise ValueError
            if name == b"fmt ":
                if fmt is not None or size < 16:
                    raise ValueError
                fmt = struct.unpack_from("<HHIIHH", data, offset)
            elif name == b"data":
                if pcm is not None:
                    raise ValueError
                pcm = data[offset:end]
            offset = end + (size % 2)
        if fmt != (1, 1, 16000, 32000, 2, 16) or pcm is None:
            raise ValueError
        if not pcm or len(pcm) % 2 or len(pcm) > 16000 * 2 * 60:
            raise ValueError
        return pcm, len(pcm) * 1000 // 32000
    except (ValueError, struct.error):
        raise Failure("INVALID_AUDIO") from None


def wav_output(samples, rate):
    # Imported only for real TTS, so fake API tests have no native dependencies.
    import numpy as np

    samples = np.asarray(samples)
    if rate != 24000 or samples.ndim != 1 or not 0 < samples.size <= 24000 * 60:
        raise Failure("OUTPUT_TOO_LARGE", 502)
    if not np.isfinite(samples).all():
        raise Failure("INFERENCE_FAILED", 502)
    pcm = np.rint(np.clip(samples, -1, 1) * 32767).astype("<i2").tobytes()
    output = io.BytesIO()
    with wave.open(output, "wb") as writer:
        writer.setnchannels(1)
        writer.setsampwidth(2)
        writer.setframerate(24000)
        writer.writeframes(pcm)
    return output.getvalue()


def strict_json(data):
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError
            result[key] = value
        return result

    try:
        value = json.loads(data.decode("utf-8"), object_pairs_hook=pairs,
                           parse_constant=lambda _: (_ for _ in ()).throw(ValueError()))
        if not isinstance(value, dict) or set(value) != {"schema_version", "text", "voice"}:
            raise ValueError
        if type(value["schema_version"]) is not int or value["schema_version"] != 1:
            raise ValueError
        text, voice = value["text"], value["voice"]
        if not isinstance(text, str) or not text.strip() or len(text.encode("utf-8")) > TEXT_BYTES:
            raise ValueError
        if not isinstance(voice, str) or not voice or len(voice.encode("utf-8")) > 128:
            raise ValueError
        return text, voice
    except (ValueError, UnicodeError, RecursionError):
        raise Failure("INVALID_REQUEST") from None


class SpeechApp:
    def __init__(self, kind, loader):
        if kind not in ("stt", "tts"):
            raise ValueError("invalid service")
        self.kind = kind
        self.service = "smith-next-" + kind
        self.loader = loader
        self.engine = None
        self.error_code = "MODEL_NOT_READY"
        self.lock = threading.Lock()

    def load(self):
        try:
            self.engine = self.loader()
            self.error_code = None
        except Failure as error:
            self.error_code = error.code
        except Exception:
            self.error_code = "MODEL_LOAD_FAILED"

    def health(self):
        return dict(schema_version=1, service=self.service, ready=self.engine is not None,
                    model=self.engine.model if self.engine else None,
                    voices=list(self.engine.voices) if self.engine else [],
                    error_code=self.error_code)

    async def body(self, receive, cap):
        chunks, total = [], 0
        async with asyncio.timeout(READ_SECONDS):
            while True:
                event = await receive()
                if event["type"] == "http.disconnect":
                    raise Failure("CLIENT_DISCONNECTED")
                if event["type"] != "http.request":
                    raise Failure("INVALID_REQUEST")
                chunk = event.get("body", b"")
                total += len(chunk)
                if total > cap:
                    raise Failure("REQUEST_TOO_LARGE", 413)
                chunks.append(chunk)
                if not event.get("more_body", False):
                    return b"".join(chunks)

    def infer(self, argument):
        try:
            if self.kind == "stt":
                pcm, duration = argument
                text, language = self.engine.transcribe(pcm)
                if (not isinstance(text, str) or len(text.encode("utf-8")) > 64 * 1024
                        or not isinstance(language, str) or len(language) > 32):
                    raise Failure("INFERENCE_FAILED", 502)
                return dict(schema_version=1, service=self.service, text=text,
                            language=language, duration_ms=duration)
            result = self.engine.speak(*argument)
            validate_output(result)
            return result
        finally:
            # Remain busy even if the HTTP caller disconnects/cancels mid-inference.
            self.lock.release()

    async def __call__(self, scope, receive, send):
        if scope["type"] == "lifespan":
            while True:
                event = await receive()
                if event["type"] == "lifespan.startup":
                    await asyncio.to_thread(self.load)
                    await send({"type": "lifespan.startup.complete"})
                elif event["type"] == "lifespan.shutdown":
                    await send({"type": "lifespan.shutdown.complete"})
                    return
            return
        if scope["type"] != "http":
            return
        owned = False
        try:
            method, path = scope["method"], scope["path"]
            if path == "/v1/health" and method == "GET":
                await self.reply(send, 200, self.health())
                return
            if path != "/v1/" + ("transcribe" if self.kind == "stt" else "speak"):
                raise Failure("NOT_FOUND", 404)
            if method != "POST":
                raise Failure("METHOD_NOT_ALLOWED", 405)
            if self.engine is None:
                raise Failure(self.error_code or "MODEL_NOT_READY", 503)
            if not self.lock.acquire(blocking=False):
                raise Failure("BUSY", 409)
            owned = True
            headers = scope.get("headers", [])
            types = [v.decode("latin1").lower() for k, v in headers if k.lower() == b"content-type"]
            expected = "audio/wav" if self.kind == "stt" else "application/json"
            if len(types) != 1 or types[0].split(";", 1)[0].strip() != expected:
                raise Failure("UNSUPPORTED_MEDIA_TYPE", 415)
            if any(k.lower() == b"content-encoding" and v.lower() != b"identity" for k, v in headers):
                raise Failure("UNSUPPORTED_MEDIA_TYPE", 415)
            data = await self.body(receive, STT_BYTES if self.kind == "stt" else JSON_BYTES)
            argument = wav_input(data) if self.kind == "stt" else strict_json(data)
            if self.kind == "tts" and argument[1] not in self.engine.voices:
                raise Failure("UNKNOWN_VOICE")
            # No await between ownership transfer and scheduling the worker.
            task = asyncio.create_task(asyncio.to_thread(self.infer, argument))
            task.add_done_callback(consume_exception)
            owned = False
            result = await asyncio.shield(task)
            await self.reply(send, 200, result)
        except Failure as error:
            await self.reply(send, error.status, self.error(error.code))
        except TimeoutError:
            await self.reply(send, 408, self.error("REQUEST_TIMEOUT"))
        except Exception:
            await self.reply(send, 500, self.error("INFERENCE_FAILED"))
        finally:
            if owned:
                self.lock.release()

    def error(self, code):
        return dict(schema_version=1, service=self.service,
                    error={"code": code, "message": "Speech request could not be completed."})

    async def reply(self, send, status, result):
        audio = isinstance(result, bytes)
        body = result if audio else json.dumps(result, ensure_ascii=True).encode("ascii")
        headers = [(b"content-type", b"audio/wav" if audio else b"application/json"),
                   (b"content-length", str(len(body)).encode("ascii")),
                   (b"cache-control", b"no-store"), (b"x-content-type-options", b"nosniff")]
        if status != 200:
            headers.append((b"connection", b"close"))
        await send({"type": "http.response.start", "status": status, "headers": headers})
        await send({"type": "http.response.body", "body": body})


def consume_exception(task):
    if not task.cancelled():
        task.exception()


def validate_output(data):
    try:
        if not isinstance(data, bytes) or len(data) > TTS_BYTES:
            raise Failure("OUTPUT_TOO_LARGE", 502)
        with wave.open(io.BytesIO(data), "rb") as reader:
            frames = reader.getnframes()
            if (reader.getnchannels(), reader.getsampwidth(), reader.getframerate(), reader.getcomptype()) != (1, 2, 24000, "NONE"):
                raise ValueError
            if not 0 < frames <= 24000 * 60 or len(reader.readframes(frames)) != frames * 2:
                raise ValueError
    except (ValueError, wave.Error, EOFError):
        raise Failure("INFERENCE_FAILED", 502) from None
