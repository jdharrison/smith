"""One fresh approved smoke after verified image ownership correction; never resend."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
import uuid

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
EVIDENCE = ROOT / "docs/evidence/dev-rootful-qa"
SUPPORT = EVIDENCE / "restore_service.py"
assert hashlib.sha256(SUPPORT.read_bytes()).hexdigest() == "d609474fbbcdf63c74e6129f278b4cd1e3893d6bd3d3b843fc81b2a3044bff51"
spec = importlib.util.spec_from_file_location("smoke_read_support", SUPPORT)
H = importlib.util.module_from_spec(spec)
spec.loader.exec_module(H)
BINARY = "493616d4172236d160e2789697d53b3f72c30eac2bba804a42f5625e47d61288"
IMAGE = "sha256:9b87e922785dcef603cc537749d1f59b2cdc4abfcabc0f596c01188cbb64763a"
MODEL = "smollm2-1.7b-instruct:q4_k_m"
MODEL_DIGEST = "decd2598bc2c8ed08c19adc3c8fdd461ee19ed5708679d1c54ef54a5a30d4f33"
TASK = "What language is this project in? Cite repository-relative files and line numbers from your inspection."
ATTEMPT = EVIDENCE / "smoke-ownership-attempt.json"
RESULT = EVIDENCE / "smoke-ownership-result.json"


def sync_directory():
    directory = os.open(EVIDENCE, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(directory)
    finally:
        os.close(directory)


def save(path, value):
    staged = path.with_name(path.name + ".staging")
    with staged.open("xb") as output:
        output.write((json.dumps(value, indent=2) + "\n").encode())
        output.flush()
        os.fchmod(output.fileno(), 0o600)
        os.fsync(output.fileno())
    os.replace(staged, path)
    sync_directory()


def main():
    assert __debug__, "Assertions must remain enabled for this bounded QA script"
    assert not ATTEMPT.exists() and not RESULT.exists(), "Attempt already recorded; observe original only"
    ownership = json.loads((EVIDENCE / "image-ownership-verification.json").read_text())
    assert ownership["image_id"] == IMAGE and ownership["service_payload_sha256"] == BINARY
    assert ownership["recipe_sha256"] == H.digest(ROOT / "service/dev-worker/Dockerfile.local")
    assert any(m["path"] == "/usr/local/bin/opencode" and m["uid"] == "0" and m["mode"] == "555" for m in ownership["metadata"])
    assert H.property_value("ActiveState") == "active" and H.property_value("Job") in ["", "0"]
    pid = int(H.property_value("MainPID"))
    proc = Path("/proc") / str(pid)
    assert H.digest(proc / "exe") == BINARY
    args = [os.fsdecode(p) for p in (proc / "cmdline").read_bytes().split(b"\0") if p]
    assert len(args) == 8 and args[1] == "serve"
    assert args[2::2] == ["--data-root", "--config-root", "--runtime-root"]
    roots = dict(zip(args[2::2], map(Path, args[3::2])))
    namespace = roots["--data-root"] / "smith-next"
    assert H.digest(namespace / "client/current/smith_backend/smith-service") == BINARY
    assert H.digest(namespace / "bin/smith-service") == BINARY
    H.idle(namespace)
    app_socket = roots["--runtime-root"] / "smith-next/app.sock"
    docker_base = ["/usr/bin/docker", "--config", "/nonexistent/smith-docker-config", "--host", "unix:///run/docker.sock"]
    image = json.loads(subprocess.check_output(docker_base + ["image", "inspect", "smith-dev-worker-local:1.19.3"], text=True, timeout=10, stderr=subprocess.DEVNULL, env={"PATH": "/usr/bin:/bin", "HOME": "/nonexistent", "LANG": "C"}))
    assert len(image) == 1 and image[0]["Id"] == IMAGE
    assert image[0]["Config"]["User"] == "1000:1000" and image[0]["Config"].get("Volumes") in [None, {}]

    def call(op, arguments, version=3):
        assert H.property_value("MainPID") == str(pid)
        request = {"schema_version": version, "service": "smith-next", "protocol": "app", "call_id": uuid.uuid4().hex, "op": op, "args": arguments}
        return H.exchange(app_socket, request, pid, 4 * 1024 * 1024)

    # Metadata inventory only; no inference, download, config change or retry.
    inventory_id = uuid.uuid4().hex
    inventory = call("models", {"request_id": inventory_id}, 2)
    instance = inventory["instance_id"]
    operation = inventory["result"]
    until = time.monotonic() + 30
    while operation["status"] in ["queued", "running"]:
        assert time.monotonic() < until
        time.sleep(0.25)
        reply = call("poll", {"request_id": inventory_id, "instance_id": instance}, 2)
        assert reply["instance_id"] == instance
        operation = reply["result"]
    assert operation["status"] == "succeeded"
    projects_reply = call("projects_list_v3", {})
    assert projects_reply["instance_id"] == instance
    projects = projects_reply["result"]
    chosen = [p for p in projects["projects"] if p["key"] == "smith"]
    assert len(chosen) == 1
    project = chosen[0]
    assert len(project["repositories"]) == 1
    repository = project["repositories"][0]
    assert repository["url"] == "https://github.com/jdharrison/smith" and repository["branch"]["base"] == "main"
    state_reply = call("dev_jobs_state_v2", {})
    assert state_reply["instance_id"] == instance
    state = state_reply["result"]
    assert state["used"] < 32
    assert all(j["status"] in ["failed", "completed"] and j["cleanup_verified"] for j in state["jobs"])
    model = state["model"]
    assert model["ready"] is True and model["name"] == MODEL and model["sha256"] == MODEL_DIGEST

    request_id = uuid.uuid4().hex
    submitted = {"request_id": request_id, "instance_id": instance, "project_id": project["project_id"], "expected_projects_revision": projects["revision"], "expected_config_revision": model["config_revision"], "prompt": TASK}
    H.idle(namespace)
    attempt = {"request_id": request_id, "task": TASK, "repository": repository["url"], "base": "main", "model": MODEL, "model_sha256": MODEL_DIGEST, "service_sha256": BINARY, "image_id": IMAGE, "script_sha256": H.digest(Path(__file__)), "inventory_request_id": inventory_id, "phase": "admission-attempted", "submission_count": 1, "automatic_retry": False}
    with ATTEMPT.open("x") as output:
        json.dump(attempt, output, indent=2)
        output.write("\n")
        output.flush()
        os.fchmod(output.fileno(), 0o600)
        os.fsync(output.fileno())
    sync_directory()
    # Exactly one Start. Any failed acknowledgement leads to original-ID GET only.
    try:
        acknowledgement = call("dev_job_start_v2", submitted)
        assert acknowledgement["instance_id"] == instance
        assert acknowledgement["result"]["request_id"] == request_id
        assert acknowledgement["result"]["project_id"] == project["project_id"]
        attempt["phase"] = "admitted"
        save(ATTEMPT, attempt)
    except Exception:
        attempt["phase"] = "admission-unverified-observe-original-only"
        save(ATTEMPT, attempt)

    phases = []
    deadline = time.monotonic() + 315
    detail = None
    while time.monotonic() < deadline:
        reply = call("dev_job_get_v2", {"request_id": request_id})
        assert reply["instance_id"] == instance
        detail = reply["result"]
        job = detail["job"]
        assert job["request_id"] == request_id and job["project_id"] == project["project_id"]
        if not phases or phases[-1] != job["status"]:
            phases.append(job["status"])
            print("Observed phase:", job["status"], flush=True)
            attempt.update(phase=job["status"], observed_phases=phases)
            save(ATTEMPT, attempt)
        if job["status"] not in ["preparing", "cloning", "ready", "running"]:
            break
        time.sleep(1)
    assert detail is not None
    job = detail["job"]
    text = detail.get("reply")
    docker = ["/usr/bin/docker", "--config", "/nonexistent/smith-docker-config", "--host", "unix:///run/docker.sock", "container", "inspect", "smith-dev-" + request_id]
    inspected = subprocess.run(docker, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, timeout=10, env={"PATH": "/usr/bin:/bin", "HOME": "/nonexistent", "LANG": "C"})
    container = None
    if inspected.returncode == 0:
        c = json.loads(inspected.stdout)[0]
        assert c["Config"]["Labels"]["org.smith.dev.request"] == request_id
        assert c["Image"] == IMAGE
        container = {"id": c["Id"], "status": c["State"]["Status"], "running": c["State"]["Running"], "exit_code": c["State"]["ExitCode"], "started": c["State"]["StartedAt"] != "0001-01-01T00:00:00Z", "state_error_present": bool(c["State"].get("Error")), "compression_explicitly_disabled": c["HostConfig"]["LogConfig"]["Config"].get("compress") == "false"}
    result = {"request_id": request_id, "status": job["status"], "message": job["message"], "failed_stage": detail["failed_stage"], "observed_phases": phases, "reply_available": job["reply_available"], "cleanup_verified": job["cleanup_verified"], "reply_bytes": len(text.encode()) if text else 0, "reply_sha256": hashlib.sha256(text.encode()).hexdigest() if text else None, "recognized_language_mentions": [language for language in ["Rust", "Dart", "Flutter", "Python", "Shell", "TypeScript", "CMake"] if text and re.search(r"\b" + language + r"\b", text, re.IGNORECASE)], "repositories": detail["repositories"], "container": container, "submission_count": 1, "automatic_retry": False}
    result["image_id"] = IMAGE
    save(RESULT, result)
    print(json.dumps(result, indent=2), flush=True)
    # No assertion implies that arbitrary failed/unknown attempts are safe to bypass.
    return 0 if job["status"] == "completed" and job["reply_available"] and job["cleanup_verified"] and container and not container["running"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
