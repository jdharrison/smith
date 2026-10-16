#!/usr/bin/env python3
"""Capture synthetic GTK fixtures using the existing isolated Dagger UI recipe.

From next/: python3 installer/scripts/capture-ui.py --output installer/tests/fixtures/ui-redesign/run-01
Uses the module-managed Python SDK, not a host SDK/GTK/package installation.
The tiny export function is added to an in-memory Dagger source directory only;
shared next/dagger files are never edited. No recipient GUI or host effects run.
"""

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time
import urllib.error
import urllib.request

EXCLUDE = ["target", "build", "dist", ".git", "__pycache__"]
CAPTURES = "tests/fixtures/ui-redesign"
WRAPPER = '''import dagger
from dagger import function, object_type
from .main import SmithNext

@object_type
class SmithUiCapture:
    @function
    async def capture(self, source: dagger.Directory, service: dagger.Directory, scale: int = 1) -> dagger.Directory:
        checked = (SmithNext()._installer_ui_tests(source, service)
            .with_env_variable("SMITH_UI_CAPTURE_DIR", "/home/smith/ui-captures")
            .with_env_variable("GDK_SCALE", str(scale))
            .with_env_variable("GDK_DPI_SCALE", "1")
            .with_exec(["mkdir", "-p", "/home/smith/ui-captures"])
            .with_exec(["sh", "scripts/test-ui.sh"], expect=dagger.ReturnType.ANY))
        output = await checked.stdout()
        errors = await checked.stderr()
        code = await checked.exit_code()
        environment = await checked.with_exec(["sh", "-c",
            "id; cat /etc/os-release; rustc --version; cargo --version; "
            "pkg-config --modversion gtk+-3.0 pango pangocairo cairo gdk-pixbuf-2.0 fontconfig; "
            "dpkg-query -W xvfb xauth"]).stdout()
        return (checked.directory("/home/smith/ui-captures")
            .with_new_file("test-output.txt", output)
            .with_new_file("test-stderr.txt", errors)
            .with_new_file("exit-code.txt", str(code))
            .with_new_file("environment.txt", environment))
'''


def source_hashes(root):
    result = {}
    for component in ("installer", "service"):
        for path in sorted((root / component).rglob("*")):
            relative = path.relative_to(root)
            if (any(part in EXCLUDE for part in relative.parts)
                    or str(relative).startswith("installer/" + CAPTURES + "/")):
                continue
            if path.is_file():
                result[str(relative)] = hashlib.sha256(path.read_bytes()).hexdigest()
    result["dagger/src/smith_next/main.py"] = hashlib.sha256(
        (root / "dagger/src/smith_next/main.py").read_bytes()).hexdigest()
    return result


def graphql(document):
    port = int(os.environ["DAGGER_SESSION_PORT"])
    credentials = base64.b64encode((os.environ["DAGGER_SESSION_TOKEN"] + ":").encode()).decode()
    request = urllib.request.Request(
        f"http://127.0.0.1:{port}/query",
        data=json.dumps({"query": document}).encode(),
        headers={"Content-Type": "application/json", "Authorization": "Basic " + credentials},
    )
    try:
        with urllib.request.urlopen(request, timeout=900) as response:
            result = json.load(response)
    except urllib.error.HTTPError as error:
        result = json.load(error)
    if "errors" in result:
        raise RuntimeError("\n".join(e["message"] for e in result["errors"]))
    return result["data"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--gdk-scale", type=int, choices=(1, 2), default=1)
    args = parser.parse_args()
    output = args.output.resolve()
    root = Path(__file__).resolve().parents[2]
    if output.exists():
        parser.error("choose a new output directory (no stale/mixed captures)")
    if not output.is_relative_to(root / "installer/tests/fixtures/ui-redesign"):
        parser.error("output must be under installer/tests/fixtures/ui-redesign")
    if "DAGGER_SESSION_PORT" not in os.environ:
        environment = dict(os.environ, DAGGER_NO_NAG="1", DAGGER_CLOUD_TOKEN="", DAGGER_PROGRESS="plain")
        subprocess.run(["dagger", "run", "python3", str(Path(__file__).resolve()),
                        "--output", str(output), "--gdk-scale", str(args.gdk_scale)],
                       cwd=root, env=environment, check=True)
        return
    before = source_hashes(root)
    started = time.time()
    quote = json.dumps
    config = json.loads((root / "dagger/dagger.json").read_text())
    config["name"] = "smith-ui-capture"
    # The SDK runs in Dagger's normal module runtime, with its matching schema.
    # This wrapper calls the original recipe verbatim, not a parallel pipeline.
    graphql('{ host { directory(path: ' + quote(str(root / "dagger"))
            + ', exclude: ["__pycache__"]) { withNewFile(path: "dagger.json", contents: '
            + quote(json.dumps(config)) + ') { withNewFile(path: "src/smith_next/__init__.py", contents: '
            + quote(WRAPPER) + ') { asModule { serve } } } } } }')
    ids = {}
    for component in ("installer", "service"):
        excludes = EXCLUDE + ([CAPTURES] if component == "installer" else [])
        result = graphql('{ host { directory(path: ' + quote(str(root / component))
                         + ', exclude: ' + quote(excludes) + ') { id } } }')
        ids[component] = result["host"]["directory"]["id"]
    graphql('{ smithUiCapture { capture(source: ' + quote(ids["installer"])
            + ', service: ' + quote(ids["service"]) + ', scale: ' + str(args.gdk_scale)
            + ') { export(path: ' + quote(str(output)) + ') } } }')
    stable = before == source_hashes(root)
    (output / "source-sha256.json").write_text(json.dumps(before, indent=2) + "\n")
    evidence = {
        "kind": "implemented-source self-check; synthetic GTK, not installed-host QA or approval",
        "command": ["python3", "installer/scripts/capture-ui.py", "--output", str(output.relative_to(root)),
                    "--gdk-scale", str(args.gdk_scale)],
        "recipe": "SmithNext._installer_ui_tests(...).with_exec(['sh', 'scripts/test-ui.sh'])",
        "recipe_sha256": before["dagger/src/smith_next/main.py"],
        "gdk_scale": args.gdk_scale,
        "base_revision": subprocess.check_output(["git", "--no-pager", "rev-parse", "HEAD"], cwd=root, text=True).strip(),
        "branch": subprocess.check_output(["git", "--no-pager", "branch", "--show-current"], cwd=root, text=True).strip(),
        "dagger_version": subprocess.check_output(["dagger", "version"], cwd=root, text=True).strip(),
        "started_unix": started,
        "elapsed_seconds": round(time.time() - started, 2),
        "source_inputs_stable": stable,
        "exit_code": int((output / "exit-code.txt").read_text()),
        "capture_count": len(list(output.glob("*.png"))),
        "limitations": ["Fake sessions, no real manager/model/installation", "Client-area drawing, not a compositor screenshot",
                        "No fractional display scaling", "No paired build/export or independent assurance"],
    }
    (output / "run.json").write_text(json.dumps(evidence, indent=2) + "\n")
    if not stable:
        raise SystemExit("Source inputs changed during capture: export is not current validation")
    if evidence["exit_code"]:
        raise SystemExit(f"UI gate failed with exit {evidence['exit_code']}; partial captures/logs retained")
    print(f"PASS: {evidence['capture_count']} synthetic PNG captures; source hashes stable")


if __name__ == "__main__":
    main()
