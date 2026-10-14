"""Optional installed-library checks: numeric fixtures only, never weights/inference."""
import importlib.util
import io
from pathlib import Path
import sys
import unittest
from unittest.mock import Mock
import wave

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from api import Failure, wav_output
from engines import WhisperEngine


@unittest.skipUnless(importlib.util.find_spec("numpy"), "optional runtime NumPy unavailable")
class NumericTests(unittest.TestCase):
    def test_pcm_encoding(self):
        import numpy as np
        result = wav_output(np.array([-2., -1., 0., 1., 2.]), 24000)
        with wave.open(io.BytesIO(result)) as reader:
            samples = np.frombuffer(reader.readframes(5), dtype="<i2")
        self.assertEqual(samples.tolist(), [-32767, -32767, 0, 32767, 32767])

    def test_numeric_output_rejection(self):
        import numpy as np
        for data, rate in [(np.zeros((2, 2)), 24000), (np.zeros(1), 16000),
                           (np.array([np.nan]), 24000), (np.array([np.inf]), 24000),
                           (np.array([]), 24000), (np.zeros(24000 * 60 + 1), 24000)]:
            with self.assertRaises(Failure):
                wav_output(data, rate)

    def test_whisper_pcm_conversion_with_mock_inference(self):
        import numpy as np
        engine = WhisperEngine.__new__(WhisperEngine)
        engine.engine = Mock()
        engine.engine.transcribe.return_value = ([Mock(text=" hello ")], Mock(language="en"))
        result = engine.transcribe(np.array([-32768, 0, 32767], dtype="<i2").tobytes())
        self.assertEqual(result, ("hello", "en"))
        actual = engine.engine.transcribe.call_args.args[0]
        np.testing.assert_allclose(actual, [-1, 0, 32767 / 32768])
        self.assertEqual(actual.dtype, np.float32)


if __name__ == "__main__":
    unittest.main()
