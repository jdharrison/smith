"""Ubuntu recipient installer; stdlib only, with separately consented effects.

Package installer.py and unchanged install.py at zip root, with __main__.py:
    import installer
    raise SystemExit(installer.main())
Payload is beside the executable, never inside its code archive. Builder tooling,
OS/Ollama installation, upgrades, enablement and lingering are not provided.
"""

import argparse
import http.client
import json
import os
import platform
import re
import selectors
import socket
import stat
import subprocess
import sys
import threading
import time
import uuid
from pathlib import Path

import install as artifact

CORE_LIMIT = 16 * 1024
APP_LIMIT = 4 * 1024 * 1024
ID = re.compile(r"[0-9a-f]{32}\Z")
DIGEST = re.compile(r"[0-9a-fA-F]{64}\Z")
RELOAD_MESSAGE = "Exact installed unit requires a separately approved reload-only repair before Start"


class Stopped(Exception):
    """Fail closed; effects already accepted are not rolled back."""


def require(condition, message):
    if not condition:
        raise Stopped(message)


def valid_model(name):
    return (isinstance(name, str) and 0 < len(name) <= 256
            and re.fullmatch(r"[A-Za-z0-9/:_.-]+", name) is not None
            and ':' in name and all(name.rsplit(':', 1))
            and 'cloud' not in name.lower()
            and not name.startswith(('http:', 'https:')))


def identifier(value):
    return isinstance(value, str) and ID.fullmatch(value) is not None


def text(value, limit=1024):
    return isinstance(value, str) and len(value.encode('utf-8')) <= limit


def display(value):
    # Provider/model output is data, not terminal control or instructions.
    return ''.join(c for c in str(value) if c.isprintable()
                   and c not in '\u202a\u202b\u202c\u202d\u202e\u2066\u2067\u2068\u2069')


def decode(data):
    def pairs(items):
        result = {}
        for key, value in items:
            require(key not in result, 'Duplicate JSON field')
            result[key] = value
        return result
    try:
        value = json.loads(data, object_pairs_hook=pairs,
                           parse_constant=lambda _: (_ for _ in ()).throw(Stopped('Invalid JSON number')))
        stack = [(value, 0)]
        while stack:
            item, depth = stack.pop()
            require(depth <= 64, 'JSON nesting bound exceeded')
            if isinstance(item, dict):
                stack.extend((v, depth + 1) for v in item.values())
            elif isinstance(item, list):
                stack.extend((v, depth + 1) for v in item)
        return value
    except (ValueError, UnicodeError, RecursionError) as exc:
        raise Stopped('Malformed bounded JSON response') from exc


def fields(value, names):
    require(isinstance(value, dict) and set(value) == set(names), 'Unexpected response fields')


def validate_helper(path):
    path = Path(path)
    require(path.is_absolute() and str(path) == os.path.normpath(str(path))
            and path.resolve(strict=True) == path, 'Helper must be a canonical absolute path')
    uid = os.geteuid()
    for parent in path.parents:
        info = parent.lstat()
        require(stat.S_ISDIR(info.st_mode) and info.st_uid in (0, uid)
                and not info.st_mode & 0o022, 'Unsafe helper ancestor; use an owner-controlled bundle root')
    before = path.lstat()
    require(stat.S_ISREG(before.st_mode) and before.st_uid == uid
            and before.st_nlink == 1 and stat.S_IMODE(before.st_mode) == 0o755,
            'Helper must be owned, single-link native executable mode 0755')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        info = os.fstat(fd)
        require(info == before, 'Helper changed during validation')
        header = os.read(fd, 64)
        require(len(header) == 64 and header[:7] == b'\x7fELF\x02\x01\x01'
                and int.from_bytes(header[18:20], 'little') == 62
                and int.from_bytes(header[16:18], 'little') in (2, 3),
                'Helper must be native Linux x86_64 ELF; runtime libraries must be separately provisioned')
    finally:
        os.close(fd)
    return str(path)


def cleanup(child):
    if child.poll() is not None:
        return
    child.terminate()
    try:
        child.wait(timeout=11)
    except subprocess.TimeoutExpired:
        child.kill()
        try:
            child.wait(timeout=1)
        except subprocess.TimeoutExpired as exc:
            raise Stopped('Helper cleanup unknown; do not reuse or retry') from exc


def process_json(path, args, request=None, limit=CORE_LIMIT, timeout=45,
                 popen=subprocess.Popen, clock=time.monotonic):
    """Incrementally bound both pipes and stdin; never communicate unbounded output."""
    command = [validate_helper(path), *args]
    payload = b'' if request is None else json.dumps(request, separators=(',', ':')).encode() + b'\n'
    require(len(payload) <= limit, 'Helper input exceeds frame bound')
    child = popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                  stderr=subprocess.PIPE, shell=False, start_new_session=True)
    out, err = bytearray(), bytearray()
    deadline = clock() + timeout
    try:
        assert child.stdin is not None and child.stdout is not None and child.stderr is not None
        with selectors.DefaultSelector() as selector:
            for stream, label in ((child.stdout, 'out'), (child.stderr, 'err')):
                os.set_blocking(stream.fileno(), False)
                selector.register(stream, selectors.EVENT_READ, label)
            if payload:
                os.set_blocking(child.stdin.fileno(), False)
                selector.register(child.stdin, selectors.EVENT_WRITE, 'in')
            else:
                child.stdin.close()
            offset = 0
            while selector.get_map():
                remaining = deadline - clock()
                require(remaining > 0, 'Helper deadline exceeded; action outcome unknown')
                for key, _ in selector.select(min(remaining, 0.1)):
                    if key.data == 'in':
                        offset += os.write(key.fd, payload[offset:offset + 8192])
                        if offset == len(payload):
                            selector.unregister(key.fileobj)
                            child.stdin.close()
                    else:
                        chunk = os.read(key.fd, 8192)
                        if not chunk:
                            selector.unregister(key.fileobj)
                            continue
                        buffer = out if key.data == 'out' else err
                        buffer.extend(chunk)
                        require(len(buffer) <= (limit if key.data == 'out' else CORE_LIMIT),
                                'Helper output exceeds bound; outcome unknown')
            remaining = deadline - clock()
            require(remaining > 0, 'Helper deadline exceeded')
            code = child.wait(timeout=remaining)
        value = decode(bytes(out))
        require(isinstance(value, dict), 'Expected helper JSON envelope')
        require(code == 0 or value.get('ok') is False, 'Helper exited unsuccessfully')
        return value
    finally:
        try:
            cleanup(child)
        finally:
            for stream in (child.stdin, child.stdout, child.stderr):
                if stream is not None:
                    stream.close()


def core_reply(value):
    required = {'schema_version', 'service', 'ok', 'binary_state', 'unit_state',
                'runtime_state', 'bridge_state', 'setup_complete', 'message', 'error', 'effects'}
    require(isinstance(value, dict) and required <= set(value)
            and set(value) <= required | {'approval_token'}, 'Invalid Core envelope')
    require(type(value['schema_version']) is int and value['schema_version'] == 1
            and value['service'] == 'smith-next' and type(value['ok']) is bool,
            'Core identity/schema mismatch')
    for key, allowed in (
        ('binary_state', {'installed', 'missing', 'conflict', 'unknown'}),
        ('unit_state', {'installed', 'missing', 'conflict', 'unknown'}),
        ('runtime_state', {'running', 'stopped', 'conflict', 'unknown', 'unavailable'}),
        ('bridge_state', {'ready', 'conflict', 'unknown', 'unavailable'}),
    ):
        require(value[key] in allowed, 'Invalid Core state')
    require(value['setup_complete'] is None or type(value['setup_complete']) is bool,
            'Invalid Core setup state')
    require(text(value['message']) and (value['error'] is None or text(value['error'])),
            'Invalid Core diagnostic')
    fields(value['effects'], {'binary_path', 'unit_path', 'state_path', 'socket_path', 'description'})
    require(all(text(v, CORE_LIMIT) for v in value['effects'].values()), 'Invalid Core effects')
    if 'approval_token' in value:
        require(isinstance(value['approval_token'], str)
                and re.fullmatch(r'[0-9]{1,20}\.[0-9a-f]{64}', value['approval_token']),
                'Invalid Core approval binding')
    require(value['ok'], 'Native preflight/action failed; inspect prerequisites and current facts')
    return value


class Native:
    def __init__(self, path, runner=process_json, clock=time.monotonic, sleep=time.sleep):
        self.path, self.runner, self.clock, self.sleep = path, runner, clock, sleep
        self.instance = None

    def core(self, *args):
        return core_reply(self.runner(self.path, list(args)))

    def app(self, op, args):
        shapes = {'state': set(), 'models': {'request_id'},
                  'configure': {'model', 'voice'}, 'submit': {'request_id', 'text', 'output'},
                  'poll': {'request_id', 'instance_id'}}
        require(op in shapes and isinstance(args, dict) and set(args) == shapes[op],
                'Unsupported app operation/arguments')
        for key in ('request_id', 'instance_id'):
            if key in args:
                require(identifier(args[key]), 'Invalid operation identity')
        if op == 'configure':
            require(valid_model(args['model']) and args['voice'] is None, 'Invalid text preference')
        if op == 'submit':
            require(text(args['text'], CORE_LIMIT) and args['text'].strip()
                    and args['output'] == 'chat', 'Invalid text submission')
        call = uuid.uuid4().hex
        request = {'schema_version': 1, 'service': 'smith-next', 'protocol': 'app',
                   'call_id': call, 'op': op, 'args': args}
        value = self.runner(self.path, ['app'], request=request, limit=APP_LIMIT)
        fields(value, {'schema_version', 'service', 'protocol', 'pid', 'instance_id',
                       'call_id', 'ok', 'result', 'error'})
        require(type(value['schema_version']) is int and value['schema_version'] == 1
                and value['service'] == 'smith-next' and value['protocol'] == 'app'
                and value['call_id'] == call and identifier(value['instance_id'])
                and type(value['pid']) is int and value['pid'] > 0
                and type(value['ok']) is bool, 'App envelope identity mismatch')
        require(self.instance is None or value['instance_id'] == self.instance,
                'Service instance changed; synchronize without resending')
        self.instance = value['instance_id']
        if not value['ok']:
            fields(value['error'], {'code', 'message'})
            require(value['result'] is None and text(value['error']['code'], 256)
                    and text(value['error']['message']), 'Invalid app error')
            raise Stopped('App operation failed: ' + display(value['error']['code'])
                          + '; no automatic resend')
        require(value['error'] is None and isinstance(value['result'], dict), 'Invalid app result')
        return value['result']

    def models(self):
        request_id = uuid.uuid4().hex
        admission = self.app('models', {'request_id': request_id})
        fields(admission, {'request_id', 'status', 'phase', 'config_revision'})
        require(admission['request_id'] == request_id and identifier(admission['request_id'])
                and admission['status'] in ('running', 'succeeded')
                and type(admission['config_revision']) is int and admission['config_revision'] >= 0,
                'Invalid inventory admission; do not resend')
        deadline = self.clock() + 45
        operation = self.app('poll', {'request_id': request_id, 'instance_id': self.instance})
        for _ in range(180):
            fields(operation, {'request_id', 'kind', 'status', 'phase', 'config_revision', 'result', 'error'})
            require(operation['request_id'] == request_id and identifier(operation['request_id'])
                    and operation['kind'] == 'models'
                    and type(operation['config_revision']) is int and operation['config_revision'] >= 0,
                    'Inventory operation mismatch')
            status = operation['status']
            require(status in ('running', 'succeeded', 'failed', 'interrupted'), 'Invalid inventory status')
            if status == 'succeeded':
                require(operation['error'] is None and operation['phase'] == 'complete',
                        'Invalid inventory completion')
                inventory = operation['result']
                fields(inventory, {'models', 'voices', 'providers'})
                require(inventory['voices'] == [], 'Unexpected active voice inventory')
                fields(inventory['providers'], {'llm', 'stt', 'tts'})
                for name, observation in inventory['providers'].items():
                    fields(observation, {'ready', 'error', 'model'} if name == 'stt' else {'ready', 'error'})
                    require(type(observation['ready']) is bool, 'Invalid provider observation')
                    error = observation['error']
                    if error is not None:
                        fields(error, {'code', 'message'})
                        require(text(error['code'], 256)
                                and re.fullmatch(r'[A-Z][A-Z0-9_]{0,255}', error['code'])
                                and text(error['message']) and error['message'].strip(),
                                'Invalid provider error')
                    require((observation['ready'] and error is None)
                            or (not observation['ready'] and error is not None),
                            'Provider readiness/error mismatch')
                    if name != 'llm':
                        require(observation['ready'] is False and isinstance(error, dict)
                                and error['code'] == 'RETIRED',
                                'Unexpected active speech provider')
                    if name == 'stt':
                        require(observation['model'] is None, 'Unexpected active speech model')
                require(inventory['providers']['llm']['ready'] is True
                        and inventory['providers']['llm']['error'] is None,
                        'Ollama inventory unavailable; install/start Ollama separately')
                return checked_models(inventory['models'])
            require(status == 'running' and operation['phase'] == 'inventory'
                    and operation['result'] is None and operation['error'] is None,
                    'Inventory failed/interrupted; no automatic retry')
            require(self.clock() < deadline, 'Inventory observation timed out; operation may continue')
            self.sleep(min(0.25, max(0, deadline - self.clock())))
            operation = self.app('poll', {'request_id': request_id, 'instance_id': self.instance})
        raise Stopped('Inventory poll limit exceeded; no resend')


def checked_models(models):
    require(isinstance(models, list) and len(models) <= 256, 'Invalid model inventory bound')
    names = set()
    for item in models:
        fields(item, {'name', 'digest'})
        require(valid_model(item['name']) and isinstance(item['digest'], str)
                and DIGEST.fullmatch(item['digest']) and item['name'] not in names,
                'Invalid/ambiguous eligible model inventory')
        names.add(item['name'])
    return names


def remote_metadata(value):
    if isinstance(value, dict):
        return any((('remote' in k.lower() or 'cloud' in k.lower())
                    and v is not None and v is not False and v != '')
                   or remote_metadata(v) for k, v in value.items())
    if isinstance(value, list):
        return any(remote_metadata(v) for v in value)
    return isinstance(value, str) and (value.lower() in ('cloud', 'remote')
                                      or value.startswith(('http://', 'https://')))


class Ollama:
    """Fixed numeric endpoint. http.client neither honors proxies nor follows redirects."""
    def __init__(self, connection=http.client.HTTPConnection, clock=time.monotonic):
        self.connection, self.clock = connection, clock
        self.deadlines = {}

    def open(self, path, body, deadline, idle):
        connection = self.connection('127.0.0.1', 11434, timeout=2)
        try:
            connection.connect()
            remaining = deadline - self.clock()
            require(remaining > 0, 'Ollama overall deadline exceeded')
            captured_socket = connection.sock

            def expire():
                # A socket idle timeout alone does not bound drip-fed headers or
                # chunk framing inside http.client. Interrupt only this socket.
                try:
                    captured_socket.shutdown(socket.SHUT_RDWR)
                except OSError:
                    pass

            timer = threading.Timer(remaining, expire)
            timer.daemon = True
            self.deadlines[connection] = timer
            timer.start()
            self.budget(connection, deadline, idle)
            encoded = None if body is None else json.dumps(body, separators=(',', ':')).encode()
            connection.request('GET' if body is None else 'POST', path, body=encoded,
                               headers={'Content-Type': 'application/json', 'Connection': 'close'})
            self.budget(connection, deadline, idle)
            response = connection.getresponse()
            require(response.status == 200, 'Ollama HTTP failure/redirect; no retry')
            return connection, response
        except BaseException:
            self.close(connection)
            raise

    def close(self, connection):
        timer = self.deadlines.pop(connection, None)
        if timer is not None:
            timer.cancel()
        connection.close()

    def budget(self, connection, deadline, idle):
        remaining = deadline - self.clock()
        require(remaining > 0, 'Ollama overall deadline exceeded; daemon/cache outcome unknown')
        if connection.sock is not None:
            connection.sock.settimeout(min(idle, remaining))

    def body(self, connection, response, limit, deadline, idle):
        data = bytearray()
        while True:
            self.budget(connection, deadline, idle)
            chunk = response.read1(min(8192, limit + 1 - len(data)))
            if not chunk:
                return decode(bytes(data))
            data.extend(chunk)
            require(len(data) <= limit, 'Ollama body bound exceeded')

    def tags(self):
        deadline = self.clock() + 10
        connection, response = self.open('/api/tags', None, deadline, 2)
        try:
            value = self.body(connection, response, 256 * 1024, deadline, 2)
            require(isinstance(value, dict) and isinstance(value.get('models'), list)
                    and len(value['models']) <= 256, 'Invalid fresh Ollama inventory')
            names = set()
            for item in value['models']:
                require(isinstance(item, dict) and isinstance(item.get('name'), str)
                        and isinstance(item.get('digest'), str), 'Invalid tag metadata')
                require(item['name'] not in names, 'Duplicate installed tag identity')
                names.add(item['name'])
                require(item.get('model', item['name']) == item['name'], 'Conflicting installed tag identity')
            # Identity ambiguity blocks the entire inventory, including entries
            # that a later local/format filter would otherwise hide.
            eligible = []
            for item in value['models']:
                if not valid_model(item['name']) or remote_metadata(item):
                    continue
                require(DIGEST.fullmatch(item['digest']), 'Invalid tag digest')
                details = item.get('details')
                if not isinstance(details, dict) or details.get('format') not in ('gguf', 'safetensors'):
                    continue
                eligible.append({'name': item['name'], 'digest': item['digest']})
            return checked_models(eligible)
        finally:
            try:
                response.close()
            finally:
                self.close(connection)

    def pull(self, model, output):
        require(valid_model(model), 'An exact eligible local model tag is required')
        deadline = self.clock() + 900
        connection, response = self.open('/api/pull', {'model': model, 'stream': True}, deadline, 30)
        pending = bytearray()
        total, frames, success = 0, 0, False
        try:
            while not success:
                self.budget(connection, deadline, 30)
                chunk = response.read1(4096)
                require(chunk, 'Pull ended without success; daemon/cache outcome unknown')
                total += len(chunk)
                require(total <= 16 * 1024 * 1024, 'Pull total progress bound exceeded')
                pending.extend(chunk)
                while b'\n' in pending:
                    frame, _, rest = pending.partition(b'\n')
                    pending = bytearray(rest)
                    frames += 1
                    require(len(frame) <= CORE_LIMIT and frames <= 65536, 'Pull progress bound exceeded')
                    event = decode(frame)
                    require(isinstance(event, dict) and 'error' not in event
                            and text(event.get('status'), 1024), 'Pull error/invalid progress')
                    for key in ('total', 'completed'):
                        if key in event:
                            require(type(event[key]) is int and 0 <= event[key] <= 2**64 - 1,
                                    'Invalid pull byte count')
                    counts = ''
                    if 'completed' in event and 'total' in event:
                        counts = f" (reported layer bytes {event['completed']}/{event['total']})"
                    print('Ollama: ' + display(event['status']) + counts, file=output)
                    if event['status'] == 'success':
                        success = True
                        require(not pending.strip(), 'Unexpected data after pull success')
                        break
                require(len(pending) <= CORE_LIMIT, 'Pull frame bound exceeded')
        finally:
            try:
                response.close()
            finally:
                self.close(connection)
        require(model in self.tags(), 'Pull success did not freshly verify the exact eligible local tag')

    def inference(self, model):
        require(valid_model(model) and model in self.tags(), 'Exact local model no longer available')
        deadline = self.clock() + 60
        connection, response = self.open('/api/generate', {
            'model': model, 'prompt': 'Reply with OK.', 'stream': False,
            'options': {'num_predict': 8, 'temperature': 0}, 'keep_alive': 0,
        }, deadline, 30)
        try:
            result = self.body(connection, response, 64 * 1024, deadline, 30)
            require(isinstance(result, dict), 'Inference response not verified')
            metadata = {key: value for key, value in result.items() if key != 'response'}
            tool_calls = result.get('tool_calls')
            require(result.get('model') == model and result.get('done') is True and 'error' not in result
                    and not remote_metadata(metadata)
                    and (tool_calls is None or (isinstance(tool_calls, list) and not tool_calls))
                    and text(result.get('response'), 4096)
                    and result['response'].strip(), 'Inference response not verified')
            return result['response']
        finally:
            try:
                response.close()
            finally:
                self.close(connection)


def prerequisites():
    require(sys.version_info >= (3, 12), 'Python 3.12 or newer required; provision it separately')
    require(platform.system() == 'Linux' and platform.machine() == 'x86_64',
            'Supported target is Ubuntu x86_64 only')
    require(os.geteuid() != 0, 'Run as the trusted unprivileged desktop user, not sudo/root')
    info = platform.freedesktop_os_release()
    require(info.get('ID') == 'ubuntu', 'Ubuntu required; no OS installation is performed')


class Installer:
    def __init__(self, bundle, model=None, pull_model=False, verify_inference=False,
                 publisher=artifact.Installer, native=Native, ollama=Ollama,
                 preflight=prerequisites, confirm=input, output=sys.stdout):
        self.bundle = Path(bundle)
        self.model, self.pull_model, self.verify_inference = model, pull_model, verify_inference
        self.publisher, self.native, self.ollama = publisher, native, ollama
        self.preflight, self.confirm, self.output = preflight, confirm, output
        self.effects = []

    def say(self, message):
        print(message, file=self.output)

    def consent(self, message, exact='y'):
        return self.confirm(message).strip() == exact

    def action(self, helper, action):
        prepared = helper.core('prepare', '--action', action)
        require('approval_token' in prepared, 'Native preparation supplied no approval')
        self.say('Prepared native effects: ' + display(json.dumps(prepared['effects'])))
        if not self.consent('Confirm ' + action + ' only? [y/N] '):
            raise Stopped('Declined ' + action + '; prior artifact/lifecycle effects remain')
        # Native apply rebinds paths, source, facts and the 60-second token.
        self.effects.append(action + ' accepted; outcome requires fresh inspection')
        helper.core('apply', '--action', action, '--approval', prepared['approval_token'])
        return helper.core('inspect')

    def run(self, dry_run=False):
        try:
            self.preflight()
            require(self.model is None or valid_model(self.model), 'Use --model with an exact eligible name:tag')
            require(not (self.pull_model or self.verify_inference) or self.model is not None,
                    '--pull-model/--verify-inference require --model EXACT')
            publisher = self.publisher(str(self.bundle / 'payload/client'),
                                       str(self.bundle / 'payload/service/smith-service'))
            if dry_run:
                return publisher.run(dry_run=True, output=self.output)
            self.say('Smith recipient installation: artifacts, native user-unit registration/start and saved model preference are separate confirmations. No upgrades, OS/Ollama installation/start, login enablement or lingering. Ubuntu runtime libraries and a usable user manager must already exist. No model weights are bundled.')
            accepted = False

            def publish_consent(prompt):
                nonlocal accepted
                accepted = self.consent(prompt)
                return 'y' if accepted else 'n'

            result = publisher.run(confirm=publish_consent, output=self.output)
            if result != 0 or not accepted:
                return result
            self.effects.append('Artifact publication completed; artifacts remain installed')
            helper = self.native(str(self.bundle / 'payload/service/smith-service'))
            facts = helper.core('inspect')
            require(facts['binary_state'] == 'installed', 'Installed binary mismatch; no implicit upgrade')
            require(facts['runtime_state'] != 'unavailable',
                    'User manager unavailable; establish a user session/manager separately')
            if facts['unit_state'] == 'missing' or facts['message'] == RELOAD_MESSAGE:
                facts = self.action(helper, 'install-unit')
            if facts['runtime_state'] == 'stopped':
                facts = self.action(helper, 'start')
            require(facts['unit_state'] == 'installed' and facts['runtime_state'] == 'running'
                    and facts['bridge_state'] == 'ready', 'Verified managed service unavailable; inspect/repair separately')
            names = helper.models()
            self.say('Observed installed local tags (inventory, not inference): ' + ', '.join(sorted(names)))
            if self.model is None:
                self.say('No model selected. Rerun with --model EXACT or explicitly Save an observed model in Smith. No default was invented.')
                return 0
            provider = None
            if self.model not in names:
                require(self.pull_model, 'Selected model missing; --pull-model can offer a separate download confirmation')
                warning = ('Download exactly ' + self.model + ' through already-running Ollama at 127.0.0.1:11434? '
                           'The external daemon may use internet and persistent disk; sizes and cancellation/cache outcomes may be unknown. '
                           'No retry or cache deletion. Type the exact model to consent: ')
                if not self.consent(warning, self.model):
                    raise Stopped('Model download declined; prior artifacts/lifecycle effects remain')
                provider = self.ollama()
                self.effects.append('Download of ' + self.model + ' accepted; daemon/cache outcome may be unknown')
                provider.pull(self.model, self.output)
                names = helper.models()
                require(self.model in names, 'Fresh app inventory did not observe selected model after pull')
            if not self.consent('Save exact model ' + self.model + ' with voice=null as a persistent preference (not inference verification)? [y/N] '):
                raise Stopped('Saved preference declined; prior effects remain')
            self.effects.append('Saved model preference accepted; outcome requires synchronization')
            config = helper.app('configure', {'model': self.model, 'voice': None})
            fields(config, {'model', 'voice', 'revision'})
            require(config['model'] == self.model and config['voice'] is None
                    and type(config['revision']) is int and config['revision'] >= 0,
                    'Saved preference response mismatch')
            self.say('Exact observed model preference saved; inference is not yet verified.')
            if self.verify_inference:
                if not self.consent('Run a direct minimal inference with ' + self.model + '? Uses provider compute, no tools; not a saved chat turn. [y/N] '):
                    raise Stopped('Inference declined; saved preference and prior effects remain')
                provider = provider or self.ollama()
                self.effects.append('Direct minimal inference accepted; not a persisted chat turn')
                self.say('Direct inference verified: ' + display(provider.inference(self.model)))
            return 0
        except (Stopped, artifact.Unsafe, OSError, ValueError, TypeError, KeyError,
                http.client.HTTPException, subprocess.SubprocessError, EOFError, KeyboardInterrupt) as exc:
            self.say('Installer stopped: ' + display(exc))
            self.say('No automatic retry/rollback. Missing runtimes, user manager and Ollama must be provisioned separately; no sudo/OS/provider start is performed.')
            return 1
        finally:
            if self.effects:
                self.say('Accepted/completed or uncertain effects (not rolled back):')
                for effect in self.effects:
                    self.say('  ' + effect)
                self.say('If download observation ended, daemon continuation and partial cache may be unknown; no cache was deleted.')


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bundle', metavar='ROOT', help='payload root (default: invoked installer executable parent)')
    parser.add_argument('--dry-run', action='store_true', help='artifact-only read-only inspection; no helpers/network/locks/writes')
    parser.add_argument('--model', metavar='EXACT', help='exact observed eligible local name:tag; no default')
    parser.add_argument('--pull-model', action='store_true', help='offer separate exact-model download consent only if missing')
    parser.add_argument('--verify-inference', action='store_true', help='offer separately confirmed direct minimal inference, not persisted chat')
    args = parser.parse_args(argv)
    bundle = os.path.abspath(args.bundle) if args.bundle else str(Path(os.path.abspath(sys.argv[0])).parent)
    return Installer(bundle, args.model, args.pull_model, args.verify_inference).run(args.dry_run)


if __name__ == '__main__':
    raise SystemExit(main())
