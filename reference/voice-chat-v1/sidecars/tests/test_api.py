import asyncio
import io
import json
from pathlib import Path
import struct
import sys
import threading
import unittest
from unittest.mock import patch
import wave

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import api
from api import Failure, SpeechApp


def wav(rate=16000, channels=1, width=2, frames=160, payload=None):
    out = io.BytesIO()
    with wave.open(out, "wb") as writer:
        writer.setnchannels(channels)
        writer.setsampwidth(width)
        writer.setframerate(rate)
        writer.writeframes(payload if payload is not None else b"\0" * frames * channels * width)
    return out.getvalue()


class Engine:
    model = "test-local-model"
    voices = ("af_test",)

    def transcribe(self, pcm):
        return "hello", "en"

    def speak(self, text, voice):
        return wav(rate=24000)


async def request(app, path, data=b"", method="POST", content_type=None, chunks=None, headers=()):
    chunks = list(chunks if chunks is not None else [data])
    messages = []
    if content_type is None:
        content_type = "audio/wav" if app.kind == "stt" else "application/json"
    scope = {"type": "http", "method": method, "path": path,
             "headers": [(b"content-type", content_type.encode()), *headers]}

    async def receive():
        chunk = chunks.pop(0)
        return {"type": "http.request", "body": chunk, "more_body": bool(chunks)}

    async def send(event):
        messages.append(event)

    await app(scope, receive, send)
    start, body = messages
    value = body["body"]
    if dict(start["headers"])[b"content-type"] == b"application/json":
        value = json.loads(value)
    return start["status"], value


def speak(text="hello", voice="af_test", **extra):
    return json.dumps(dict(schema_version=1, text=text, voice=voice, **extra)).encode()


class APITests(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        self.stt = SpeechApp("stt", Engine)
        self.tts = SpeechApp("tts", Engine)
        self.stt.load()
        self.tts.load()

    async def test_health(self):
        status, value = await request(self.stt, "/v1/health", method="GET")
        self.assertEqual(status, 200)
        self.assertEqual(value, dict(schema_version=1, service="smith-next-stt", ready=True,
                                     model="test-local-model", voices=["af_test"], error_code=None))

    async def test_missing_model(self):
        def missing():
            raise Failure("MODEL_MISSING")
        app = SpeechApp("tts", missing)
        app.load()
        status, value = await request(app, "/v1/health", method="GET")
        self.assertEqual(status, 200)
        self.assertEqual(value["error_code"], "MODEL_MISSING")
        self.assertFalse(value["ready"])
        self.assertIsNone(value["model"])
        self.assertEqual(value["voices"], [])
        status, value = await request(app, "/v1/speak", speak())
        self.assertEqual(status, 503)

    async def test_load_failure_redacted(self):
        def broken():
            raise RuntimeError("private model location and secret")
        app = SpeechApp("stt", broken)
        app.load()
        _, value = await request(app, "/v1/health", method="GET")
        self.assertEqual(value["error_code"], "MODEL_LOAD_FAILED")
        self.assertNotIn("private", str(value))

    async def test_lifespan_loads_once(self):
        events = iter([{"type": "lifespan.startup"}, {"type": "lifespan.shutdown"}])
        replies = []
        async def receive():
            return next(events)
        async def send(event):
            replies.append(event["type"])
        await SpeechApp("stt", Engine)({"type": "lifespan"}, receive, send)
        self.assertEqual(replies, ["lifespan.startup.complete", "lifespan.shutdown.complete"])

    async def test_transcription_contract(self):
        status, value = await request(self.stt, "/v1/transcribe", wav())
        self.assertEqual(status, 200)
        self.assertEqual(value, dict(schema_version=1, service="smith-next-stt", text="hello",
                                     language="en", duration_ms=10))

    async def test_transcription_at_duration_limit(self):
        status, value = await request(self.stt, "/v1/transcribe", wav(frames=16000 * 60))
        self.assertEqual(status, 200)
        self.assertEqual(value["duration_ms"], 60000)

    async def test_invalid_audio(self):
        for data in [b"/models/file.wav", b"", wav(rate=24000), wav(channels=2),
                     wav(width=1), wav(frames=0), wav(frames=16000 * 60 + 1),
                     wav()[:-1], wav() + b"trailing", wav(payload=b"\0")]:
            with self.subTest(size=len(data)):
                status, value = await request(self.stt, "/v1/transcribe", data)
                self.assertEqual(status, 400)
                self.assertEqual(value["error"]["code"], "INVALID_AUDIO")

    async def test_lied_chunk_lengths_and_stream_cap(self):
        status, value = await request(self.stt, "/v1/transcribe",
                                     chunks=[b"a" * 100, b"a" * api.STT_BYTES],
                                     headers=[(b"content-length", b"1")])
        self.assertEqual(status, 413)
        self.assertEqual(value["error"]["code"], "REQUEST_TOO_LARGE")
        self.assertFalse(self.stt.lock.locked())

    async def test_streamed_valid_body(self):
        data = wav()
        status, _ = await request(self.stt, "/v1/transcribe", chunks=[data[:7], data[7:]])
        self.assertEqual(status, 200)

    async def test_json_stream_cap(self):
        status, _ = await request(self.tts, "/v1/speak", b" " * (api.JSON_BYTES + 1))
        self.assertEqual(status, 413)

    async def test_speak_contract(self):
        status, value = await request(self.tts, "/v1/speak", speak())
        self.assertEqual(status, 200)
        with wave.open(io.BytesIO(value)) as reader:
            self.assertEqual((reader.getframerate(), reader.getnchannels(), reader.getsampwidth()), (24000, 1, 2))

    async def test_strict_json(self):
        invalid = [speak(path="ignored"), speak(text=""), speak(text=" "), speak(text="x" * 4097),
                   speak(text="é" * 2049), speak(text=None), speak(voice=3), b"[]", b"{}", b"\xff",
                   b'{"schema_version":true,"text":"hello","voice":"af_test"}',
                   b'{"schema_version":2,"text":"hello","voice":"af_test"}',
                   b'{"schema_version":1,"text":"a","text":"b","voice":"af_test"}',
                   b'{"schema_version":1,"text":NaN,"voice":"af_test"}',
                   b'{"schema_version":1,"text":"\\ud800","voice":"af_test"}']
        for data in invalid:
            with self.subTest(data=data[:80]):
                status, value = await request(self.tts, "/v1/speak", data)
                self.assertEqual(status, 400)
                self.assertEqual(value["error"]["code"], "INVALID_REQUEST")

    async def test_text_limit(self):
        status, _ = await request(self.tts, "/v1/speak", speak(text="x" * 4096))
        self.assertEqual(status, 200)

    async def test_unknown_voice(self):
        status, value = await request(self.tts, "/v1/speak", speak(voice="not-real"))
        self.assertEqual(status, 400)
        self.assertEqual(value["error"]["code"], "UNKNOWN_VOICE")

    async def test_output_bounds_and_format(self):
        for data, code in [(wav(rate=24000, frames=24000 * 60 + 1), "OUTPUT_TOO_LARGE"),
                           (wav(rate=16000), "INFERENCE_FAILED"),
                           (wav(rate=24000)[:-1], "INFERENCE_FAILED")]:
            self.tts.engine.speak = lambda *args: data
            status, value = await request(self.tts, "/v1/speak", speak())
            self.assertEqual(status, 502)
            self.assertEqual(value["error"]["code"], code)

    async def test_output_at_limit(self):
        self.tts.engine.speak = lambda *args: wav(rate=24000, frames=24000 * 60)
        status, value = await request(self.tts, "/v1/speak", speak())
        self.assertEqual(status, 200)
        self.assertEqual(len(value), api.TTS_BYTES)

    async def test_redacted_inference_error(self):
        def fail(*args):
            raise RuntimeError("sensitive text and local model path")
        self.stt.engine.transcribe = fail
        status, value = await request(self.stt, "/v1/transcribe", wav())
        self.assertEqual(status, 500)
        self.assertEqual(value["error"], dict(code="INFERENCE_FAILED", message="Speech request could not be completed."))
        self.assertFalse(self.stt.lock.locked())

    async def test_busy_and_health_responsive(self):
        started, release = threading.Event(), threading.Event()
        def blocked(*args):
            started.set()
            release.wait(5)
            return "hello", "en"
        self.stt.engine.transcribe = blocked
        pending = asyncio.create_task(request(self.stt, "/v1/transcribe", wav()))
        try:
            for _ in range(100):
                if started.is_set():
                    break
                await asyncio.sleep(.01)
            self.assertTrue(started.is_set())
            status, value = await asyncio.wait_for(request(self.stt, "/v1/transcribe", wav()), .5)
            self.assertEqual(status, 409)
            self.assertEqual(value["error"]["code"], "BUSY")
            status, _ = await asyncio.wait_for(request(self.stt, "/v1/health", method="GET"), .5)
            self.assertEqual(status, 200)
            pending.cancel()
            with self.assertRaises(asyncio.CancelledError):
                await pending
            self.assertTrue(self.stt.lock.locked())
        finally:
            release.set()
            for _ in range(100):
                if not self.stt.lock.locked():
                    break
                await asyncio.sleep(.01)
        self.assertFalse(self.stt.lock.locked())

    async def test_read_timeout_and_disconnect(self):
        async def stall():
            await asyncio.sleep(1)
        events = []
        async def send(event):
            events.append(event)
        scope = dict(type="http", method="POST", path="/v1/transcribe", headers=[(b"content-type", b"audio/wav")])
        with patch.object(api, "READ_SECONDS", .01):
            await self.stt(scope, stall, send)
        self.assertEqual(events[0]["status"], 408)
        self.assertFalse(self.stt.lock.locked())
        async def disconnected():
            return dict(type="http.disconnect")
        events.clear()
        await self.stt(scope, disconnected, send)
        self.assertEqual(events[0]["status"], 400)
        self.assertFalse(self.stt.lock.locked())

    async def test_routes_and_media(self):
        for path, method, mime, status in [("/upload", "POST", "audio/wav", 404),
                                            ("/v1/transcribe", "GET", "audio/wav", 405),
                                            ("/v1/transcribe", "POST", "multipart/form-data", 415),
                                            ("/v1/transcribe", "POST", "application/json", 415)]:
            actual, _ = await request(self.stt, path, wav(), method=method, content_type=mime)
            self.assertEqual(actual, status)
        status, _ = await request(self.stt, "/v1/transcribe", wav(), headers=[(b"content-encoding", b"gzip")])
        self.assertEqual(status, 415)


class PackagingTests(unittest.TestCase):
    def test_stdout_safe_startup_failure(self):
        import os
        import subprocess
        script = Path(__file__).resolve().parents[1] / "serve.py"
        env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
        result = subprocess.run([sys.executable, str(script), "invalid"],
                                capture_output=True, timeout=5, env=env)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, b"Speech service could not start.\n")
        self.assertEqual(result.stderr, b"")

    def test_security_manifest(self):
        text = (Path(__file__).resolve().parents[1] / "compose.yaml").read_text()
        for line in ['create_host_path: false', 'read_only: true', 'cap_drop: [ALL]',
                     'security_opt: ["no-new-privileges:true"]', 'restart: "no"', 'internal: true',
                     'user: "10001:10001"', 'pids_limit: 64', 'driver: "none"']:
            self.assertGreaterEqual(text.count(line), 2)
        self.assertIn('127.0.0.1:4890:4890', text)
        self.assertIn('127.0.0.1:4891:4891', text)
        self.assertNotIn('docker.sock', text)
        self.assertNotIn('/dev/snd', text)
        self.assertNotIn('network_mode:', text)
        self.assertIn('${SMITH_STT_MODEL_DIR:?', text)
        self.assertIn('${SMITH_TTS_MODEL_DIR:?', text)


if __name__ == "__main__":
    unittest.main()
