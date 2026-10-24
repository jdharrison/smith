#!/usr/bin/env python3
"""One-shot Ubuntu transport QA. SOURCE ONLY until separately reviewed/approved.

API (parent stages all inputs; check never contacts Docker or writes files):
  python3 -B live-transport-qa.py check
  python3 -B live-transport-qa.py run --review FILE --review-sha256 SHA256 \
      --approval FILE --approval-sha256 SHA256

Fixed layout: .smith-artifact-functional-qa/dagger (verified 0.21.9),
run-01/{module/,request.json,engine.json,docker/config.json,home/,config/,cache/}.
All run-01 directories/files must be private and owned by the operator. Parent
copies the COMPLETE pipeline tree (no symlinks/special files) and stages an exact
canonical request from expected_request(binding, baseline). engine.json must be
CONFIG below; Docker config is {} (no inherited credentials/helpers/proxies).

The retained pinned live-sdk-source-main.go.txt (334-346, 386-445) establishes
that call codegen changes the engine-side ContextDir; only init/develop export
generated files to the host. The entire private host module remains hash-frozen,
with NO generated-SDK exclusions. Unexpected host writes fail the check; owned-
engine cleanup is attempted and UNKNOWN cleanup state and markers are retained.
Editable SDK/package dependency compatibility remains unverified, not repaired
or retried automatically. Fresh pre-effect review must bind this private leaf's
new destination hash; receipts for the previous staging destination are invalid.

check emits binding only (hashes/fixed constants). Parent independently reviews
those bytes, authenticates the reviewer/operator OUTSIDE this script and supplies
hash-pinned JSON receipts: {binding: <check output>, issued_at: unix_seconds,
expires_at: unix_seconds, decision: "approved", actor: <nonempty identifier>,
role: "independent-review" | "operator"}. Review additionally requires
independent:true; operator additionally requires review_sha256:<review file hash>.
Both receipts must be <=30 minutes old and unexpired at the effect boundary.
These are parent-attested records, NOT signatures or a same-UID security boundary.
This author must not supply the independent receipt. No receipt-generation API.

run retains exclusive consumed/pending markers, private captures and result.json;
never resumes, retries, adopts, removes or prunes. Do not use an outer timeout
shorter than 960 seconds (900 total + scheduling margin); SIGINT/SIGTERM request
bounded cancellation. SIGKILL/host loss cannot be cleaned up by Python: retained
markers mean UNKNOWN, never permission to restart. Kernel/daemon I/O is not a
hard-real-time guarantee. Raw results stay private; stdout is fixed status only.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import signal
import stat
import subprocess
import time
import uuid

ROOT = Path(__file__).resolve().parents[3]
NAME = "smith-artifact-qa-20261020-01"
VOLUME = NAME + "-data"
IMAGE = "registry.dagger.io/engine:v0.21.9"
LABEL = "smith.artifact.qa.owner"
CONFIG = {"logLevel": "error", "security": {"insecureRootCapabilities": False}}
SCOPE = {"engine": NAME, "volume": VOLUME, "image": IMAGE, "seconds": 900,
         "cleanup_seconds": 60, "cpus": 4, "memory": 4294967296, "pids": 1024,
         "privileged": True, "cgroupns": "private", "attempts": 1,
         "operation": "execute-policy-denied-only", "endpoint": "unix:///run/docker.sock",
         "no_setup_model_provider_auth_installed_or_remote_mutation": True}


class Refuse(Exception):
    pass


def need(condition):
    if not condition:
        raise Refuse()


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()


def pairs(items):
    value = {}
    for key, item in items:
        need(key not in value)
        value[key] = item
    return value


def decode(raw):
    def invalid(_):
        raise Refuse()
    return json.loads(raw.decode("utf-8", "strict"), object_pairs_hook=pairs,
                      parse_constant=invalid)


def read(path, limit=4 * 1024 * 1024):
    need(path.is_file() and not path.is_symlink() and path.stat().st_size <= limit)
    return path.read_bytes()


def safe(path, private=False):
    for item in (path, *path.parents):
        meta = item.lstat()
        need(not stat.S_ISLNK(meta.st_mode) and not meta.st_mode & 0o022)
        need(meta.st_uid in {0, os.getuid()})
    if private:
        meta = path.stat()
        need(meta.st_uid == os.getuid() and not meta.st_mode & 0o077)


def tree(path, private=False):
    safe(path, private)
    result = {}
    for item in sorted(path.rglob("*")):
        safe(item, private)
        need(item.is_file() or item.is_dir())
        if item.is_file():
            result[item.relative_to(path).as_posix()] = sha(read(item))
    need(bool(result))
    return result


def durable(path, value):
    # Exclusive file plus directory fsync: existence always fences another run.
    with path.open("xb") as stream:
        os.chmod(path, 0o600)
        stream.write(encoded(value))
        stream.flush()
        os.fsync(stream.fileno())
    fd = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def expected_request(binding, baseline):
    return encoded({"schema_version": 1, "run_id": "20261020010000000000000000000001",
                    "phase": "execute", "attempt": 1,
                    "contract_sha256": sha(encoded(SCOPE)),
                    "executable_sha256": binding["cli"],
                    "module_sha256": binding["module"],
                    "input": encoded({"schema_version": 1, "baseline": {
                        "path": "clamp.py", "content": baseline.decode("utf-8"),
                        "sha256": sha(baseline)}}).decode()})


class Inputs:
    def __init__(self, root=ROOT):
        self.root = root
        self.base = root / ".smith-artifact-functional-qa"
        self.work = self.base / "run-01"
        self.cli = self.base / "dagger"
        self.source = root / "pipeline"
        self.module = self.work / "module"
        self.installed = Path.home() / ".local/bin/dagger"

    def binding(self):
        safe(self.cli)
        need(os.access(self.cli, os.X_OK))
        safe(self.work, True)
        for name in ("home", "config", "cache", "docker"):
            safe(self.work / name, True)
        for name in ("request.json", "engine.json", "docker/config.json"):
            safe(self.work / name, True)
        need(decode(read(self.work / "engine.json")) == CONFIG)
        need(decode(read(self.work / "docker/config.json")) == {})
        # No pre-populated CLI credentials, configuration or alternate runtime.
        need(all(not list((self.work / name).iterdir()) for name in ("home", "config", "cache")))
        need({p.name for p in (self.work / "docker").iterdir()} == {"config.json"})
        original = tree(self.source)
        need(original == tree(self.module, True))
        need(decode(read(self.module / "dagger.json"))["engineVersion"] == "v0.21.9")
        preflight_raw = read(self.root / "docs/evidence/artifact-pipeline/functional-qa-readonly-preflight.json")
        preflight = decode(preflight_raw)
        installed = sha(read(self.installed, 128 * 1024 * 1024))
        need(installed == preflight["installed_dagger"]["sha256"])
        binding = {"scope": SCOPE, "script": sha(read(Path(__file__))),
                   "tests": sha(read(Path(__file__).with_name("test_live_transport_qa.py"))),
                   "source": sha(encoded(original)), "module": sha(encoded(original)),
                   "cli": sha(read(self.cli, 128 * 1024 * 1024)), "installed_cli": installed,
                   "engine_config": sha(read(self.work / "engine.json")),
                   "docker_config": sha(read(self.work / "docker/config.json")),
                   "preflight": sha(preflight_raw),
                   "destination": sha(str(self.work.absolute()).encode())}
        request = read(self.work / "request.json", 32768)
        need(request == expected_request(binding, read(self.source / "fixtures/clamp/clamp.py")))
        binding["request"] = sha(request)
        return binding

    def preserve_snapshot(self, binding):
        files = tree(self.module, True)
        need(sha(encoded(files)) == binding["module"])
        durable(self.work / "module-manifest.json", files)

    def unchanged(self, binding):
        # Runtime may populate HOME/XDG, so do not reapply the initial empty check.
        need(sha(encoded(tree(self.source))) == binding["source"])
        need(sha(encoded(tree(self.module, True))) == binding["module"])
        for path, key, limit in ((self.cli, "cli", 128 * 1024 * 1024),
                                 (self.installed, "installed_cli", 128 * 1024 * 1024),
                                 (self.work / "request.json", "request", 32768),
                                 (self.work / "engine.json", "engine_config", 4096),
                                 (self.work / "docker/config.json", "docker_config", 4096),
                                 (Path(__file__), "script", 4 * 1024 * 1024)):
            need(sha(read(path, limit)) == binding[key])


def receipt(path, digest, role, binding, now, review=None):
    safe(path, True)
    raw = read(path, 65536)
    need(sha(raw) == digest)
    value = decode(raw)
    need(value["binding"] == binding and value["role"] == role)
    need(value["decision"] == "approved" and type(value["actor"]) is str and bool(value["actor"].strip()))
    need(type(value["issued_at"]) in (int, float) and type(value["expires_at"]) in (int, float))
    need(now - 1800 <= value["issued_at"] <= now < value["expires_at"] <= value["issued_at"] + 1800)
    if role == "independent-review":
        need(value.get("independent") is True and value.get("actor") != "live-transport-qa-builder")
    else:
        need(value.get("review_sha256") == review)
    return value


class Commands:
    """Concurrent capped capture; every child owns a new process group."""
    def __init__(self, work, env, cancelled):
        self.work, self.env, self.cancelled = work, env, cancelled
        self.index, self.unknown = 0, False

    def __call__(self, argv, deadline, cleanup=False, cap=1024 * 1024):
        need(time.monotonic() < deadline and (cleanup or not self.cancelled()))
        self.index += 1
        out = self.work / ("capture-%03d.out" % self.index)
        err = self.work / ("capture-%03d.err" % self.index)
        buffers = [bytearray(), bytearray()]
        proc = None
        files = []
        try:
            for path in (out, err):
                try:
                    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
                    files.append(os.fdopen(fd, "wb"))
                except OSError:
                    if not cleanup:
                        raise
                    files.append(open(os.devnull, "wb"))  # Cleanup cannot depend on log storage.
            proc = subprocess.Popen(argv, cwd=self.work, env=self.env, stdin=subprocess.DEVNULL,
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
            with selectors.DefaultSelector() as selector:
                for index, pipe in enumerate((proc.stdout, proc.stderr)):
                    os.set_blocking(pipe.fileno(), False)
                    selector.register(pipe, selectors.EVENT_READ, index)
                while selector.get_map() or proc.poll() is None:
                    need(time.monotonic() < deadline and (cleanup or not self.cancelled()))
                    for key, _ in selector.select(0.05):
                        chunk = os.read(key.fd, 65536)
                        if not chunk:
                            selector.unregister(key.fileobj)
                            continue
                        index = key.data
                        remaining = cap - len(buffers[index])
                        try:
                            files[index].write(chunk[:remaining])
                        except OSError:
                            if not cleanup:
                                raise
                        buffers[index].extend(chunk[:remaining])
                        need(len(chunk) <= remaining)
            return proc.wait(timeout=0.1), bytes(buffers[0]), bytes(buffers[1])
        finally:
            # Also terminate descendants that closed their inherited pipes.
            if proc is not None:
                for sig, seconds in ((signal.SIGTERM, 0.5), (signal.SIGKILL, 1.5)):
                    try:
                        os.killpg(proc.pid, sig)
                    except ProcessLookupError:
                        break
                    until = time.monotonic() + seconds
                    while time.monotonic() < until:
                        proc.poll()
                        try:
                            os.killpg(proc.pid, 0)
                        except ProcessLookupError:
                            break
                        time.sleep(0.02)
                try:
                    proc.wait(timeout=0.1)
                    os.killpg(proc.pid, 0)
                    self.unknown = True
                except ProcessLookupError:
                    pass
                except subprocess.TimeoutExpired:
                    self.unknown = True
                for pipe in (proc.stdout, proc.stderr):
                    pipe.close()
            for stream in files:
                try:
                    stream.close()
                except OSError:
                    if not cleanup:
                        raise


def cgroup(pid):
    need(Path("/sys/fs/cgroup/cgroup.controllers").is_file() and type(pid) is int and pid > 0)
    rows = Path(f"/proc/{pid}/cgroup").read_text().splitlines()
    need(len(rows) == 1 and rows[0].startswith("0::/"))
    relative = rows[0][4:]
    need(relative and ".." not in relative.split("/"))
    path = Path("/sys/fs/cgroup") / relative
    quota, period = (path / "cpu.max").read_text().split()
    need(quota != "max" and 0 < int(quota) <= 4 * int(period))
    need((path / "memory.max").read_text().strip() == "4294967296")
    need((path / "pids.max").read_text().strip() == "1024")
    return path


def empty_cgroup(path):
    if not path.exists():
        return True
    # cgroup.events populated includes all descendants, supplemented by procs.
    need(dict(line.split() for line in (path / "cgroup.events").read_text().splitlines())["populated"] == "0")
    for directory in (path, *path.rglob("*")):
        if directory.is_dir():
            need(not (directory / "cgroup.procs").read_text().strip())
    return True


class Supervisor:
    def __init__(self, inputs, command=None, probe=cgroup, empty=empty_cgroup):
        self.inputs, self.work = inputs, inputs.work
        self.cancelled = False
        self.env = {"HOME": str(self.work / "home"), "XDG_CONFIG_HOME": str(self.work / "config"),
                    "XDG_CACHE_HOME": str(self.work / "cache"), "PATH": "/usr/bin:/bin",
                    "DOCKER_HOST": SCOPE["endpoint"], "DOCKER_CONFIG": str(self.work / "docker")}
        self.command = command or Commands(self.work, self.env, lambda: self.cancelled)
        self.probe, self.empty = probe, empty
        self.cid, self.image, self.cg, self.pid = None, None, None, None
        self.nonce = uuid.uuid4().hex
        self.end = time.monotonic() + 900
        self.work_end = self.end - 60

    def docker(self, *args, cleanup=False, long=False):
        deadline = self.end - 3 if cleanup else self.work_end
        if not long:
            deadline = min(deadline, time.monotonic() + (20 if cleanup else 30))
        rc, out, _ = self.command(["/usr/bin/docker", "--host", SCOPE["endpoint"],
                                   "--config", str(self.work / "docker"), *args], deadline, cleanup=cleanup)
        need(rc == 0 and (cleanup or not getattr(self.command, "unknown", False)))
        return out

    def inspect_owned(self, cleanup=False):
        values = decode(self.docker("container", "inspect", self.cid, cleanup=cleanup))
        need(type(values) is list and len(values) == 1)
        value = values[0]
        need(value["Id"] == self.cid and value["Image"] == self.image)
        need(value["Name"] == "/" + NAME and value["Config"]["Labels"].get(LABEL) == self.nonce)
        return value

    def execution_config(self, value):
        host = value["HostConfig"]
        need(host["Privileged"] is True and host["CgroupnsMode"] == "private")
        need(host["NanoCpus"] == 4000000000 and host["Memory"] == 4294967296)
        need(host["MemorySwap"] == 4294967296 and host["PidsLimit"] == 1024)
        need(not host.get("PortBindings") and not host.get("Binds"))
        need(host["RestartPolicy"]["Name"] == "no" and host["NetworkMode"] == "default")
        mounts = value["Mounts"]
        need(len(mounts) == 2)
        by_dest = {m["Destination"]: m for m in mounts}
        data, config = by_dest["/var/lib/dagger"], by_dest["/etc/dagger/engine.json"]
        need(data["Type"] == "volume" and data["Name"] == VOLUME and data["RW"] is True)
        need(config["Type"] == "bind" and config["Source"] == str(self.work / "engine.json") and config["RW"] is False)
        return value

    def inspect(self):
        return self.execution_config(self.inspect_owned())

    def capture_cid(self):
        value = read(self.work / "engine.cid", 128).decode("ascii").strip()
        need(re.fullmatch(r"[0-9a-f]{64}", value) is not None)
        self.cid = value
        durable(self.work / "owned.json", {"cid": value, "image": self.image, "nonce": self.nonce})

    def run(self, binding, authorize):
        status, cleanup = "refused", "not_started"
        marked = False
        def revalidate():
            need(not self.cancelled and time.monotonic() < self.work_end)
            self.inputs.unchanged(binding)
            authorize()
            need(not self.cancelled and time.monotonic() < self.work_end)
        try:
            revalidate()
            durable(self.work / "consumed.json", {"binding": binding, "nonce": self.nonce})
            marked = True
            durable(self.work / "pending.json", {"nonce": self.nonce})
            self.inputs.preserve_snapshot(binding)
            need(not (self.work / "engine.cid").exists() and not (self.work / "owned.json").exists())
            names = self.docker("container", "ls", "-a", "--format", "{{.Names}}").decode().splitlines()
            volumes = self.docker("volume", "ls", "--format", "{{.Name}}").decode().splitlines()
            need(NAME not in names and VOLUME not in volumes)
            revalidate()
            self.docker("pull", IMAGE, long=True)
            images = decode(self.docker("image", "inspect", IMAGE))
            need(len(images) == 1 and re.fullmatch(r"sha256:[0-9a-f]{64}", images[0]["Id"]) is not None)
            self.image = images[0]["Id"]
            revalidate()
            self.docker("volume", "create", "--label", LABEL + "=" + self.nonce, VOLUME)
            volume = decode(self.docker("volume", "inspect", VOLUME))
            need(len(volume) == 1 and volume[0]["Name"] == VOLUME and volume[0]["Labels"].get(LABEL) == self.nonce)
            need(volume[0]["Driver"] == "local" and not volume[0].get("Options"))
            revalidate()
            self.docker("create", "--name", NAME, "--cidfile", str(self.work / "engine.cid"),
                        "--label", LABEL + "=" + self.nonce, "--privileged", "--cpus", "4",
                        "--memory", "4g", "--memory-swap", "4g", "--pids-limit", "1024",
                        "--cgroupns", "private", "--restart", "no", "--log-driver", "local",
                        "--log-opt", "max-size=1m", "--log-opt", "max-file=1", "--log-opt", "compress=false",
                        "--mount", "type=volume,src=" + VOLUME + ",dst=/var/lib/dagger",
                        "--mount", "type=bind,src=" + str(self.work / "engine.json") + ",dst=/etc/dagger/engine.json,readonly",
                        self.image)
            self.capture_cid()  # fsynced BEFORE start; never infer ownership from name
            self.inspect()
            revalidate()
            self.docker("start", self.cid)
            live = self.inspect()
            need(live["State"]["Running"] is True)
            self.cg = self.probe(live["State"]["Pid"])
            self.pid = live["State"]["Pid"]
            durable(self.work / "running.json", {"pid": live["State"]["Pid"], "cgroup": str(self.cg)})
            need(self.inspect()["State"]["Pid"] == live["State"]["Pid"])
            self.env["_EXPERIMENTAL_DAGGER_RUNNER_HOST"] = "container+docker://" + self.cid
            revalidate()
            durable(self.work / "dispatch.json", {"request": binding["request"]})
            rc, raw, _ = self.command([str(self.inputs.cli), "--mod", str(self.inputs.module),
                                       "call", "run-fixture", "--request", str(self.work / "request.json")],
                                      self.work_end, cap=65536)
            # Raw JSON envelope ONLY; quoted JSON strings/heuristics are rejected.
            need(rc == 0 and decode(raw) == {"schema_version": 1, "request_sha256": binding["request"],
                                            "outcome": "failed", "error": "policy_denied"})
            need(type(decode(raw)["schema_version"]) is int)
            self.inputs.unchanged(binding)
            need(not self.cancelled)
            status = "policy_denied"
        except Exception:
            status = "failed"
        finally:
            if marked:
                cleanup = "unknown"
                try:
                    # A failed create may still have produced a CID; NEVER start it.
                    if self.cid is None and (self.work / "engine.cid").exists():
                        self.capture_cid()
                    if self.cid is not None:
                        before = self.inspect_owned(cleanup=True)
                        configuration_ok = True
                        try:
                            self.execution_config(before)
                        except Exception:
                            configuration_ok, status = False, "failed"
                        proof = self.cg is not None
                        try:
                            if before["State"]["Running"]:
                                current = self.probe(before["State"]["Pid"])
                                if self.cg is None:
                                    self.cg, self.pid = current, before["State"]["Pid"]
                                proof = current == self.cg and before["State"]["Pid"] == self.pid
                        except Exception:
                            proof = False
                        # Ownership authorizes shutdown; config/proof drift must not block it.
                        self.docker("stop", "--time", "10", self.cid, cleanup=True)
                        stopped = self.inspect_owned(cleanup=True)
                        try:
                            self.execution_config(stopped)
                        except Exception:
                            configuration_ok, status = False, "failed"
                        need(stopped["State"]["Running"] is False and stopped["State"]["Pid"] == 0)
                        need(configuration_ok and proof and self.cg is not None and self.empty(self.cg))
                        need(not getattr(self.command, "unknown", False))
                        cleanup = "verified_stopped"
                    self.inputs.unchanged(binding)
                    need(time.monotonic() < self.end)
                except Exception:
                    cleanup = "unknown"
                if self.cancelled:
                    status = "failed"
                try:
                    durable(self.work / "result.json", {"status": status, "cleanup": cleanup,
                                                         "binding": binding})
                except Exception:
                    cleanup = "unknown"
        return {"status": status, "cleanup": cleanup}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("mode", choices=("check", "run"))
    for name in ("review", "review-sha256", "approval", "approval-sha256"):
        parser.add_argument("--" + name)
    args = parser.parse_args()
    os.umask(0o077)
    try:
        inputs = Inputs()
        binding = inputs.binding()
        if args.mode == "check":
            print(encoded(binding).decode())
            return 0
        need(all((args.review, args.review_sha256, args.approval, args.approval_sha256)))
        def authorize():
            now = time.time()
            reviewer = receipt(Path(args.review), args.review_sha256, "independent-review", binding, now)
            operator = receipt(Path(args.approval), args.approval_sha256, "operator", binding, now, args.review_sha256)
            need(reviewer["actor"] != operator["actor"])
        authorize()
        supervisor = Supervisor(inputs)
        def cancel(_sig, _frame):
            supervisor.cancelled = True
        for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            signal.signal(sig, cancel)
        result = supervisor.run(binding, authorize)
        print(encoded(result).decode())
        return 0 if result == {"status": "policy_denied", "cleanup": "verified_stopped"} else 1
    except Exception:
        print('{"status":"refused"}')
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
