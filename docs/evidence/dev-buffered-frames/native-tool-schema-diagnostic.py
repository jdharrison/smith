"""One approved no-generation native schema probe; no production writes.

--self-test is offline. --probe must be launched in a durable detached session:
prepare BASE (0700), a private log and PID/session receipt before dispatch, as in
prior diagnostics. Do not timeout-kill the supervisor: cleanup retains the shared
runner lease until the exact owned process group is positively absent and joined.
No retry/fallback, service stop, model refresh, Dev operation or history cleanup.
Only health/props/template/tokenize are callable. The latter use synthetic data.
CPU intentionally isolates template semantics; it does not certify Vulkan startup.
"""
import contextlib
import ctypes
import fcntl
import hashlib
import http.client
import json
import os
from pathlib import Path
import re
import signal
import socket
import stat
import struct
import subprocess
import sys
import threading
import time
import types
import uuid

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
RESUME_SHA = '4a37f8d87c520e92808c5fd8c3a3cd40dd9a6257cc3c7968162ea8d6393b0135'
SERVICE_SHA = '3ce3c3e44cf9584850d8d1c15f7b15e7bf97f37408c894b7f6ea362f65e498b8'
MANIFEST_SHA = '5b3a8571fa3a77e604a78b2a6872d87b6ec6793d398c58efce417ce7fd525ef2'
MODEL_SHA = '6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e'
MODEL_SIZE = 1117320736
EXE_SHA = '5faf410bbbc00a1b94b09ee01623699f255365628dfa73b1ee552d9e52a2231d'
IMPL_SHA = '0cb1fd100e7f018ac9ac0c5a3fc12aeef4cbe7614bb7ca8c686ec7d01c30810f'
COMMON_SHA = '9329f7b28a8c82ad02764b66ce038a2383c28043e2158f134a35c01ddb64cc7d'
EXPECTED_CAPS = ('supports_tools', 'supports_tool_calls', 'supports_tool_responses')
ALLOWED = {('GET', '/health'), ('GET', '/props'), ('POST', '/apply-template'), ('POST', '/tokenize')}
STATES = ('app-state.json', 'dev-jobs.json', 'dev-local-jobs.json', 'state.json')


def support():
    path = HERE / 'coordinated-resume.py'
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        data = stream.read(131073)
    if len(data) > 131072 or hashlib.sha256(data).hexdigest() != RESUME_SHA:
        raise RuntimeError('Cleanup support drift')
    module = types.ModuleType('pinned_native_diagnostic_cleanup')
    module.__file__ = str(path)
    exec(compile(data, '<pinned-cleanup>', 'exec'), module.__dict__)
    return module


C = support()
M = C.M
BASE = M.PRIVATE / 'native-tool-schema-diagnostic'


def check():
    C.checkpoint()


def safe_names(value):
    return sorted(k for k in value if isinstance(k, str) and re.fullmatch('[A-Za-z0-9_-]{1,80}', k))[:64]


def caps_summary(props):
    caps = props.get('chat_template_caps')
    actual = {k: v if type(v) is bool else {'json_type': type(v).__name__}
              for k, v in caps.items() if k in safe_names(caps)} if isinstance(caps, dict) else None
    expected = {k: caps.get(k) if isinstance(caps, dict) and type(caps.get(k)) is bool else None
                for k in EXPECTED_CAPS}
    return {'top_level_keys': safe_names(props), 'actual_chat_template_caps': actual,
            'smith_expected_values': expected, 'smith_guard_accepts': all(v is True for v in expected.values())}


def redact(text, key, paths):
    text = text.replace(key, '<redacted>') if key else text
    for path in sorted(paths, key=len, reverse=True):
        text = text.replace(path, '<private>')
    if re.search('authorization|api[_ -]?key.*[:=]', text, re.I):
        return '<authentication diagnostic redacted>'
    return re.sub(r'(?<![A-Za-z0-9])/(?:[^\s\"\'<>])+', '<path>', text)[:2000]


def group_quiet(root):
    assert root.is_dir()
    pids = set()
    def fail(error):
        raise error
    for directory, _, _ in os.walk(root, onerror=fail):
        pids.update(int(v) for v in Path(directory, 'cgroup.procs').read_text().split())
    return pids


def hashes(namespace):
    return {name: M.sha(M.read(namespace / name)) if os.path.lexists(namespace / name) else None for name in STATES}


def idle(namespace):
    app = M.decode(M.read(namespace / STATES[0]))
    M.require(app['config']['model'] == 'qwen2.5-1.5b-instruct:q4_k_m')
    M.require(all(j['operation']['status'] not in ('queued', 'running') for j in app['jobs']))
    for name in STATES[1:3]:
        data = M.decode(M.read(namespace / name))
        M.require(all(r['summary']['status'] not in ('preparing', 'cloning', 'ready', 'running', 'dispatching')
                      and r['cleanup_required'] is False for r in data['records']))


def manifest_check(namespace):
    target = os.readlink(namespace / 'client/current')
    M.require(target == 'releases/' + MANIFEST_SHA)
    release = namespace / 'client' / target
    M.safe_directory(release)
    raw = M.read(release / '.smith-next-manifest.json')
    M.require(M.sha(raw) == MANIFEST_SHA)
    manifest = M.decode(raw)
    for backend, count in (('cpu', 25), ('vulkan', 26)):
        prefix = 'smith_backend/runtime/' + backend + '/'
        entries = {name: entry for name, entry in manifest['entries'].items() if name.startswith(prefix)}
        directory = release / prefix
        M.safe_directory(directory)
        M.require(len(entries) == count and {p.name for p in directory.iterdir()} == {n[len(prefix):] for n in entries})
        for name, entry in entries.items():
            check()
            path = release / name
            M.require(entry['type'] == 'file' and stat.S_IMODE(path.lstat().st_mode) == entry['mode']
                      and M.sha(M.read(path)) == entry['sha256'])
    cpu = release / 'smith_backend/runtime/cpu'
    for name, digest in (('llama-server', EXE_SHA), ('libllama-server-impl.so', IMPL_SHA),
                         ('libllama-common.so.0', COMMON_SHA)):
        M.require(M.sha(M.read(cpu / name)) == digest)
    M.require(M.sha(M.read(namespace / 'bin/smith-service')) == SERVICE_SHA
              and M.sha(M.read(release / 'smith_backend/smith-service')) == SERVICE_SHA)
    return release, cpu


class LocalHTTP(http.client.HTTPConnection):
    def __init__(self, path, pid):
        super().__init__('localhost', timeout=2)
        self.path, self.pid = path, pid

    def connect(self):
        meta = self.path.lstat()
        M.require(stat.S_ISSOCK(meta.st_mode) and meta.st_uid == os.geteuid() and not meta.st_mode & 0o077)
        stream = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            stream.settimeout(2)
            stream.connect(str(self.path))
            pid, uid, _ = struct.unpack('3i', stream.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
            M.require(pid == self.pid and uid == os.geteuid())
            M.require((meta.st_dev, meta.st_ino) == (self.path.lstat().st_dev, self.path.lstat().st_ino))
            self.sock = stream
        except BaseException:
            stream.close()
            raise


def http(path, pid, key, method, route, body=None):
    M.require((method, route) in ALLOWED, 'Generation/other endpoint forbidden')
    check()
    data = json.dumps(body, separators=(',', ':')).encode() if body is not None else None
    M.require(data is None or len(data) <= 128 * 1024)
    connection = LocalHTTP(path, pid)
    try:
        connection.request(method, route, body=data,
                           headers={'Authorization': 'Bearer ' + key, 'Content-Type': 'application/json',
                                    'Connection': 'close'})
        response = connection.getresponse()
        payload = response.read(1024 * 1024 + 1)
        M.require(len(payload) <= 1024 * 1024)
        status = response.status
        value = M.decode(payload)
        M.require(isinstance(value, dict))
    finally:
        connection.close()
    check()
    return status, value


def template_messages(roundtrip=False):
    messages = [{'role': 'system', 'content': 'Diagnostic placeholder only. No generation.'},
                {'role': 'user', 'content': 'Inspect a synthetic file.'}]
    if roundtrip:
        messages += [{'role': 'assistant', 'content': None, 'tool_calls': [
            {'id': 'diagnostic_read_1', 'type': 'function', 'function': {
                'name': 'read', 'arguments': '{"filePath":"/workspace/repos/r0/README.md","offset":1,"limit":2}'}}]},
            {'role': 'tool', 'tool_call_id': 'diagnostic_read_1', 'content': 'Synthetic diagnostic line.'}]
    return {'messages': messages, 'tools': [{'type': 'function', 'function': {
        'name': 'read', 'description': 'Read a bounded synthetic checkout range.',
        'parameters': {'type': 'object', 'properties': {'filePath': {'type': 'string'},
            'offset': {'type': 'integer', 'minimum': 1, 'maximum': 20000},
            'limit': {'type': 'integer', 'minimum': 1, 'maximum': 32}},
            'required': ['filePath'], 'additionalProperties': False}}}],
        'add_generation_prompt': True, 'tool_choice': 'auto', 'parallel_tool_calls': False}


def template_probe(path, pid, key, roundtrip):
    status, value = http(path, pid, key, 'POST', '/apply-template', template_messages(roundtrip))
    result = {'variant': 'synthetic-tool-roundtrip' if roundtrip else 'synthetic-tools',
              'apply_template_status': status, 'apply_template_keys': safe_names(value)}
    if status == 200 and isinstance(value.get('prompt'), str):
        prompt = value['prompt']
        result.update(rendered_bytes=len(prompt.encode()), rendered_sha256=M.sha(prompt.encode()))
        code, tokens = http(path, pid, key, 'POST', '/tokenize', {'content': prompt, 'add_special': True})
        valid = isinstance(tokens.get('tokens'), list) and all(type(t) is int and t >= 0 for t in tokens['tokens'])
        result.update(tokenize_status=code, tokenize_keys=safe_names(tokens), token_schema_valid=valid,
                      token_count=len(tokens['tokens']) if valid else None)
    return result


def probe():
    M.require(os.geteuid() != 0 and not sys.flags.optimize)
    os.umask(0o077)
    M.safe_directory(BASE, private=True)  # Created only by finite owned launcher.
    M.require(not any(os.path.lexists(BASE / n) for n in ('progress.json', 'result.json', 'native.log')))
    result = {'status': 'incomplete', 'generation_requests': 0, 'warmup_disabled': True,
              'backend': 'cpu', 'service_stopped': False, 'production_writes': False}
    lock = model = child = stderr_thread = None
    before = namespace = runtime_dir = group = service_proc = service_start = None
    stderr_errors = []
    with C.cancellation_owner() as owner:
        def prop(name):
            env = {k: os.environ[k] for k in ('HOME', 'XDG_DATA_HOME', 'XDG_CONFIG_HOME',
                   'XDG_RUNTIME_DIR', 'DBUS_SESSION_BUS_ADDRESS') if k in os.environ}
            env.update(PATH='/usr/bin:/bin', LANG='C')
            data = C.owned_command(['/usr/bin/systemctl', '--user', 'show', 'smith-next.service',
                                    '--property=' + name, '--value'], env, timeout=10, limit=16384, merge_stderr=True)
            return data.decode().strip()
        def phase(name):
            check()
            M.publish(BASE / 'progress.json', {'phase': name, 'generation_requests': 0})
        try:
            M.write(BASE / 'progress.json', M.encoded({'phase': 'static-revalidation', 'generation_requests': 0}))
            service_pid = int(prop('MainPID'))
            M.require(service_pid == 617055 and prop('ActiveState') == 'active' and prop('SubState') == 'running')
            service_proc = Path('/proc') / str(service_pid)
            M.require(M.sha((service_proc / 'exe').read_bytes()) == SERVICE_SHA)
            service_start = (service_proc / 'stat').read_text().rsplit(')', 1)[1].split()[19]
            args = [os.fsdecode(x) for x in (service_proc / 'cmdline').read_bytes().split(b'\0') if x]
            M.require(len(args) == 8 and args[1] == 'serve'
                      and args[2::2] == ['--data-root', '--config-root', '--runtime-root'])
            roots = dict(zip(args[2::2], map(Path, args[3::2])))
            H, _ = M.support()
            root_ids = H.root_identities(roots)
            C.environment({'before': {'roots': {k: str(v) for k, v in roots.items()}}})
            namespace = roots['--data-root'] / 'smith-next'
            idle(namespace)
            before = hashes(namespace)
            raw_group = Path(prop('ControlGroup'))
            M.require(raw_group.is_absolute() and raw_group != Path('/') and '..' not in raw_group.parts)
            group = Path('/sys/fs/cgroup') / str(raw_group).lstrip('/')
            M.require(group_quiet(group) == {service_pid})
            release, cpu = manifest_check(namespace)
            lockpath = roots['--runtime-root'] / 'smith-next/runner.lock'
            meta = lockpath.lstat()
            M.require(stat.S_ISREG(meta.st_mode) and meta.st_uid == os.geteuid()
                      and stat.S_IMODE(meta.st_mode) == 0o600 and meta.st_nlink == 1)
            lock = os.open(lockpath, os.O_RDWR | os.O_NOFOLLOW | os.O_NONBLOCK)
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            M.require(M.identity(os.fstat(lock)) == M.identity(lockpath.lstat()))
            phase('shared-runner-lease-held')
            idle(namespace)
            M.require(hashes(namespace) == before and group_quiet(group) == {service_pid})
            marker = M.decode(M.read(namespace / 'models' / (MODEL_SHA + '.json'), 4096))
            M.require(marker['name'] == 'qwen2.5-1.5b-instruct:q4_k_m' and marker['sha256'] == MODEL_SHA
                      and marker['file'] == MODEL_SHA + '.gguf' and marker['size'] == MODEL_SIZE)
            modelpath = namespace / 'models' / (MODEL_SHA + '.gguf')
            model = os.open(modelpath, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
            model_meta = os.fstat(model)
            M.require(stat.S_ISREG(model_meta.st_mode) and model_meta.st_uid == os.geteuid()
                      and stat.S_IMODE(model_meta.st_mode) == 0o600 and model_meta.st_nlink == 1
                      and model_meta.st_size == MODEL_SIZE)
            digest = hashlib.sha256()
            deadline = time.monotonic() + 45
            while True:
                check()
                M.require(time.monotonic() < deadline, 'Model hashing deadline')
                chunk = os.read(model, 1024 * 1024)
                if not chunk:
                    break
                digest.update(chunk)
            M.require(digest.hexdigest() == MODEL_SHA and M.file_identity(os.fstat(model)) == M.file_identity(model_meta))
            available = next(int(line.split()[1]) * 1024 for line in Path('/proc/meminfo').read_text().splitlines()
                             if line.startswith('MemAvailable:'))
            M.require(available > MODEL_SIZE + 1024 * 1024 * 1024)
            check()
            runtime_dir = roots['--runtime-root'] / 'smith-next' / ('diag-tools-' + uuid.uuid4().hex[:12])
            runtime_dir.mkdir(mode=0o700)
            M.sync_dir(runtime_dir.parent)
            endpoint = runtime_dir / 'llama.sock'
            M.require(len(os.fsencode(endpoint)) < 108)
            key = uuid.uuid4().hex
            command = [str(cpu / 'llama-server'), '--model', '/proc/self/fd/' + str(model),
                       '--host', str(endpoint), '--offline', '--no-webui', '--no-slots', '--no-agent',
                       '--no-mmproj-auto', '--no-cache-idle-slots', '--cache-ram', '0', '--ctx-checkpoints', '0',
                       '--ctx-size', '4096', '--parallel', '1', '--threads', '2', '--threads-http', '2',
                       '--timeout', '15', '--alias', MODEL_SHA, '--device', 'none', '--gpu-layers', '0',
                       '--fit', 'off', '--jinja', '--no-warmup']
            phase('isolated-cpu-no-warmup-launch')
            paths = [str(Path.home()), str(runtime_dir), str(release)]
            log = os.fdopen(os.open(BASE / 'native.log', os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600), 'wb')
            parent = os.getpid()
            def setup():
                libc = ctypes.CDLL(None, use_errno=True)
                if libc.prctl(1, signal.SIGKILL, 0, 0, 0) != 0 or os.getppid() != parent:
                    os._exit(125)
            child = subprocess.Popen(command, cwd=cpu, env={'LLAMA_API_KEY': key}, stdin=subprocess.DEVNULL,
                                     stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, pass_fds=(lock, model),
                                     start_new_session=True, preexec_fn=setup)
            M.write(BASE / 'native-child.json', M.encoded({'pid': child.pid, 'group': child.pid,
                    'runtime_directory_name': runtime_dir.name, 'executable_sha256': EXE_SHA,
                    'model_sha256': MODEL_SHA, 'warmup_disabled': True, 'generation_requests': 0}))
            def drain():
                total = 0
                try:
                    discarding = False
                    while True:
                        raw = child.stderr.readline(65536)
                        if not raw:
                            break
                        if discarding or (len(raw) == 65536 and not raw.endswith(b'\n')):
                            discarding = not raw.endswith(b'\n')
                            continue  # Never retain partial oversized lines or split secrets.
                        data = (redact(raw.decode(errors='replace').rstrip(), key, paths) + '\n').encode()
                        if total + len(data) <= 128 * 1024:
                            log.write(data)
                            total += len(data)
                    log.flush()
                    os.fsync(log.fileno())
                except BaseException:
                    stderr_errors.append('capture-failed')
                finally:
                    child.stderr.close()
                    log.close()
            stderr_thread = threading.Thread(target=drain, name='diagnostic-log-drain', daemon=True)
            stderr_thread.start()
            phase('waiting-health')
            deadline = time.monotonic() + 60
            while True:
                check()
                M.require(time.monotonic() < deadline, 'Native startup deadline')
                M.require(os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT) is None,
                          'Native child exited before health')
                if endpoint.is_socket():
                    try:
                        code, _ = http(endpoint, child.pid, key, 'GET', '/health')
                    except (socket.timeout, ConnectionRefusedError, FileNotFoundError):
                        code = None  # Only bounded startup transport races; never peer/identity refusal.
                    if code == 200:
                        break
                time.sleep(0.1)
            phase('no-generation-endpoint-probes')
            code, props = http(endpoint, child.pid, key, 'GET', '/props')
            result['props_status'] = code
            result['transport'] = 'private-peer-verified-af-unix'
            result['props'] = caps_summary(props)
            result['template_probes'] = [template_probe(endpoint, child.pid, key, flag) for flag in (False, True)]
            M.require(hashes(namespace) == before and H.root_identities(roots) == root_ids)
            M.require((service_proc / 'stat').read_text().rsplit(')', 1)[1].split()[19] == service_start)
            M.require(M.file_identity(os.fstat(model)) == M.file_identity(model_meta))
            M.require(os.readlink(namespace / 'client/current') == 'releases/' + MANIFEST_SHA)
            result.update(status='endpoint-probes-complete', app_and_dev_state_exact=True,
                          service_identity_and_roots_exact=True, model_sha256=MODEL_SHA,
                          runtime_executable_sha256=EXE_SHA, runtime_implementation_sha256=IMPL_SHA,
                          runtime_common_sha256=COMMON_SHA, installed_manifest_sha256=MANIFEST_SHA,
                          runtime_revision='b11429/d81235049')
        except BaseException as error:
            result['failure_code'] = type(error).__name__
        finally:
            result['operator_cancelled_before_cleanup'] = owner.cancelled[0]
            if child is not None:
                # This may intentionally wait without an outer kill deadline.
                status = C.cancel_core(child.pid, True)
                child.returncode = os.waitstatus_to_exitcode(status)
                if stderr_thread is not None:
                    while stderr_thread.is_alive():
                        stderr_thread.join(0.1)
                # A stale filesystem socket may remain as private evidence;
                # the kernel endpoint must be gone before exclusion is released.
                while True:
                    try:
                        present = any(line.split() and line.split()[-1] == str(runtime_dir / 'llama.sock')
                                      for line in Path('/proc/net/unix').read_text().splitlines()[1:])
                        if not present:
                            break
                    except Exception:
                        pass  # Unknown socket cleanup retains the lease, never unwinds it.
                    time.sleep(0.05)
                result.update(owned_group_absent=True, exact_child_joined=True,
                              kernel_socket_absent=True, child_exit_code=child.returncode,
                              stderr_capture_ok=not stderr_errors)
            if model is not None:
                os.close(model)
            # Release inherited exclusion only AFTER positive child/group cleanup.
            if lock is not None:
                fcntl.flock(lock, fcntl.LOCK_UN)
                os.close(lock)
                result['runner_lease_released_after_join'] = True
            if namespace is not None and before is not None:
                after = hashes(namespace)
                result['app_and_dev_state_exact_after_cleanup'] = after == before
                result['state_sha256_before'] = before
                result['state_sha256_after'] = after
            if group is not None:
                result['service_cgroup_only_service_after_cleanup'] = group_quiet(group) == {617055}
            if result['status'] == 'endpoint-probes-complete' and (not result.get('stderr_capture_ok')
                    or not result.get('app_and_dev_state_exact_after_cleanup')
                    or not result.get('service_cgroup_only_service_after_cleanup')):
                result['status'] = 'endpoint-probes-complete-preservation-check-failed'
            M.write(BASE / 'result.json', M.encoded(result))
    print(json.dumps(result, sort_keys=True))
    if result['status'] != 'endpoint-probes-complete':
        raise SystemExit(1)


def self_test():
    import unittest
    from unittest.mock import patch
    class Tests(unittest.TestCase):
        def test_current_guard_requires_all_three_exact_booleans(self):
            for caps in ({}, {'tools': True, 'tool_calls': True, 'tool_responses': True},
                         {'supports_tools': True, 'supports_tool_calls': True, 'supports_tool_responses': 1}):
                self.assertFalse(caps_summary({'chat_template_caps': caps})['smith_guard_accepts'])
            self.assertTrue(caps_summary({'chat_template_caps': dict.fromkeys(EXPECTED_CAPS, True)})['smith_guard_accepts'])
        def test_generation_endpoint_refused_before_connect(self):
            with patch.object(LocalHTTP, 'connect', side_effect=AssertionError('No native access')):
                with self.assertRaises(M.Refused):
                    http(Path('/unused'), 1, 'synthetic-key', 'POST', '/v1/chat/completions', {})
        def test_templates_are_synthetic_and_no_execution(self):
            self.assertEqual(len(template_messages()['messages']), 2)
            self.assertEqual(len(template_messages(True)['messages']), 4)
            self.assertEqual(template_messages()['tools'][0]['function']['name'], 'read')
        def test_redaction(self):
            self.assertNotIn('synthetic-key', redact('key synthetic-key', 'synthetic-key', []))
            self.assertNotIn('/private/example', redact('load /private/example/model', '', ['/private/example']))
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(Tests)
    M.require(unittest.TextTestRunner(verbosity=1).run(suite).wasSuccessful())


if __name__ == '__main__':
    try:
        M.require(sys.argv[1:] in (['--self-test'], ['--probe']))
        self_test() if sys.argv[1:] == ['--self-test'] else probe()
    except BaseException:
        print('Native diagnostic refused/incomplete; retain private evidence and supervision.', file=sys.stderr)
        sys.exit(1)
