from pathlib import Path
import sys
import types
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import engines
from api import Failure


class AdapterTests(unittest.TestCase):
    def test_missing_files_prevent_library_import_or_download(self):
        for cls in (engines.WhisperEngine, engines.KokoroEngine):
            with patch.object(Path, "is_dir", return_value=False):
                with self.assertRaises(Failure) as raised:
                    cls()
            self.assertEqual(raised.exception.code, "MODEL_MISSING")

    def test_explicit_local_whisper_cpu_int8(self):
        constructor = Mock()
        module = types.SimpleNamespace(WhisperModel=constructor)
        with patch.object(engines, "required") as required, patch.dict(sys.modules, {"faster_whisper": module}):
            engines.WhisperEngine()
        required.assert_called_once_with(engines.STT_ROOT,
                                         ("model.bin", "config.json", "tokenizer.json", "preprocessor_config.json"))
        constructor.assert_called_once_with(str(engines.STT_ROOT), device="cpu", compute_type="int8",
                                            cpu_threads=2, num_workers=1, local_files_only=True)

    def test_actual_voices_and_cpu_session(self):
        model = Mock()
        model.get_voices.return_value = ["af_real", "bf_real", "not_english", "am_real"]
        kokoro = Mock()
        kokoro.from_session.return_value = model
        ort = types.SimpleNamespace(SessionOptions=Mock(), InferenceSession=Mock())
        with patch.object(engines, "required"), patch.dict(sys.modules, {
                "onnxruntime": ort, "kokoro_onnx": types.SimpleNamespace(Kokoro=kokoro)}):
            adapter = engines.KokoroEngine()
        self.assertEqual(adapter.voices, ("af_real", "bf_real", "am_real"))
        self.assertEqual(ort.InferenceSession.call_args.kwargs["providers"], ["CPUExecutionProvider"])
        self.assertEqual(kokoro.from_session.call_args.args[1], str(engines.TTS_ROOT / "voices-v1.0.bin"))
        self.assertEqual(ort.SessionOptions.return_value.intra_op_num_threads, 2)

    def test_no_voice_fallback(self):
        model = Mock()
        model.get_voices.return_value = []
        kokoro = Mock()
        kokoro.from_session.return_value = model
        with patch.object(engines, "required"), patch.dict(sys.modules, {
                "onnxruntime": types.SimpleNamespace(SessionOptions=Mock(), InferenceSession=Mock()),
                "kokoro_onnx": types.SimpleNamespace(Kokoro=kokoro)}):
            with self.assertRaises(Failure) as raised:
                engines.KokoroEngine()
        self.assertEqual(raised.exception.code, "VOICES_MISSING")

    def test_actual_synthesis_and_language(self):
        adapter = engines.KokoroEngine.__new__(engines.KokoroEngine)
        adapter.engine = Mock()
        adapter.engine.create.return_value = ("fake-samples", 24000)
        with patch.object(engines, "wav_output", return_value=b"fake-wav") as output:
            result = adapter.speak("text", "bf_real")
        self.assertEqual(result, b"fake-wav")
        adapter.engine.create.assert_called_once_with("text", voice="bf_real", speed=1.0, lang="en-gb")
        output.assert_called_once_with("fake-samples", 24000)


if __name__ == "__main__":
    unittest.main()
