"""Single-process entry point; never log request bodies, paths or exceptions."""
import logging
import os
import sys
import warnings

# Apply before importing native libraries or model loaders.
os.environ["HF_HUB_OFFLINE"] = "1"
os.environ["TRANSFORMERS_OFFLINE"] = "1"
os.environ["HF_HUB_DISABLE_TELEMETRY"] = "1"
os.environ["DO_NOT_TRACK"] = "1"
os.environ["OMP_NUM_THREADS"] = "2"
logging.disable(logging.CRITICAL)
warnings.filterwarnings("ignore")


def main():
    # Native runtimes can bypass Python logging. Suppress their stdout/stderr,
    # retaining a separate descriptor solely for the fixed startup error.
    safe_stdout = os.fdopen(os.dup(1), "w")
    null = os.open(os.devnull, os.O_WRONLY)
    os.dup2(null, 1)
    os.dup2(null, 2)
    os.close(null)
    try:
        import uvicorn
        from api import SpeechApp
        from engines import KokoroEngine, WhisperEngine

        if len(sys.argv) != 2 or sys.argv[1] not in ("stt", "tts"):
            raise ValueError
        kind = sys.argv[1]
        app = SpeechApp(kind, WhisperEngine if kind == "stt" else KokoroEngine)
        uvicorn.run(app, host="0.0.0.0", port=4890 if kind == "stt" else 4891,
                    workers=1, loop="asyncio", http="h11", lifespan="on",
                    access_log=False, log_config=None, server_header=False,
                    timeout_keep_alive=2, h11_max_incomplete_event_size=16384)
    except Exception:
        print("Speech service could not start.", file=safe_stdout, flush=True)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
