"""Exit status only: never emit model/HTTP details into Docker health logs."""
import json
import sys
import urllib.request


def main():
    try:
        kind = sys.argv[1]
        if kind not in ("stt", "tts"):
            return 1
        port = 4890 if kind == "stt" else 4891
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        with opener.open(f"http://127.0.0.1:{port}/v1/health", timeout=2) as response:
            value = json.loads(response.read(4097))
        return 0 if (value.get("schema_version") == 1
                     and value.get("service") == "smith-next-" + kind
                     and value.get("ready") is True and value.get("error_code") is None) else 1
    except Exception:
        return 1


if __name__ == "__main__":
    sys.exit(main())
