"""Local-only adapters. Model locations are fixed container paths, not API data."""
from pathlib import Path

from api import Failure, wav_output

STT_ROOT = Path("/models/whisper")
TTS_ROOT = Path("/models/kokoro")


def required(root, names):
    if not root.is_dir() or any(not (root / name).is_file() for name in names):
        raise Failure("MODEL_MISSING", 503)


class WhisperEngine:
    model = "whisper-ctranslate2"
    voices = ()

    def __init__(self):
        required(STT_ROOT, ("model.bin", "config.json", "tokenizer.json", "preprocessor_config.json"))
        from faster_whisper import WhisperModel

        self.engine = WhisperModel(str(STT_ROOT), device="cpu", compute_type="int8",
                                   cpu_threads=2, num_workers=1, local_files_only=True)

    def transcribe(self, pcm):
        import numpy as np

        audio = np.frombuffer(pcm, dtype="<i2").astype(np.float32) / 32768.0
        segments, info = self.engine.transcribe(audio, beam_size=1, best_of=1,
                                               temperature=0, vad_filter=False,
                                               condition_on_previous_text=False,
                                               max_new_tokens=448, log_progress=False)
        texts, size = [], 0
        for segment in segments:
            size += len(segment.text.encode("utf-8"))
            if size > 64 * 1024:
                raise Failure("OUTPUT_TOO_LARGE", 502)
            texts.append(segment.text)
        return "".join(texts).strip(), info.language


class KokoroEngine:
    model = "kokoro-onnx"

    def __init__(self):
        required(TTS_ROOT, ("kokoro-v1.0.onnx", "voices-v1.0.bin"))
        import onnxruntime as ort
        from kokoro_onnx import Kokoro

        options = ort.SessionOptions()
        options.intra_op_num_threads = 2
        options.inter_op_num_threads = 1
        options.log_severity_level = 4
        session = ort.InferenceSession(str(TTS_ROOT / "kokoro-v1.0.onnx"),
                                       sess_options=options, providers=["CPUExecutionProvider"])
        self.engine = Kokoro.from_session(session, str(TTS_ROOT / "voices-v1.0.bin"))
        # Only advertise English voices supported by this explicit language contract.
        self.voices = tuple(v for v in self.engine.get_voices()
                            if isinstance(v, str) and v.startswith(("af_", "am_", "bf_", "bm_"))
                            and len(v.encode("utf-8")) <= 128)
        if not self.voices:
            raise Failure("VOICES_MISSING", 503)

    def speak(self, text, voice):
        samples, rate = self.engine.create(text, voice=voice, speed=1.0,
                                          lang="en-gb" if voice.startswith("b") else "en-us")
        return wav_output(samples, rate)
