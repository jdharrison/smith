"""Narrow continuation of the stopped, partially applied e06 coordinated upgrade.

Import is inert. --validate-copies runs only isolated candidate readers; it never
contacts Docker or the user manager. Live mode requires a fresh independent
review receipt, not created here. No Upgrade, Stop, rollback, replay or old reader.
Private resume-review.json must bind this script, the tests and copies proof,
PREP binding, and original module with verdict=pass and independent_review=true.
That review is NOT deployment approval: the operator must separately authorize
this exact reviewed continuation immediately before invoking live mode.
"""
import contextlib
import fcntl
import os
from pathlib import Path
import re
import select
import signal
import stat
import subprocess
import sys
import time
import types

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ORIGINAL = 'e06a6074007ddebc7e2252102defdd4fdca37064f9b75bb95cc8b4db7d2fa92c'
BINDING = '98738547a002ecd764d2aa8a3556c1abb88e14431c3fef15fd967042c9c55472'
NEW_IMAGE = 'sha256:fae115562506916e3b1d1d65743476d7a26350b0723720884a4ee1e8f3325e82'
SUPERVISION = 'cd7e3a0ce4cb9e66fb5b9be4cd43ef21af053256d14b71821d7daa19a160a39d'
DRIVER_LOG = '0c883a44df69885f75e40c4e877fcb2db90334ec44e87482cedb07e7384f3a4a'
HOOK_LOG = '9856868996127a1f4aed3e796ba2ea6bb484e9a3777e1cda28e5c0c77b5c8366'
ARTIFACT_METADATA = '6665546a6f99b43a88149425020819a00c48d7ceb676a193585f29279694be47'
DRIVER_PID = 552827


def original_module():
    path = HERE / 'coordinated-maintenance.py'
    # Bootstrap without importing through a mutable pathname after verification.
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        meta = os.fstat(stream.fileno())
        if not stat.S_ISREG(meta.st_mode) or meta.st_size > 128 * 1024:
            raise RuntimeError('Original source refused')
        data = stream.read(128 * 1024 + 1)
    import hashlib
    if hashlib.sha256(data).hexdigest() != ORIGINAL:
        raise RuntimeError('Original source drift')
    module = types.ModuleType('resume_pinned_original')
    module.__file__ = str(path)
    exec(compile(data, '<e06-pinned-coordinator>', 'exec'), module.__dict__)
    return module


M = original_module()
PRIVATE = M.PRIVATE
COPIES = PRIVATE / 'resume-copies.json'
PROGRESS = PRIVATE / 'resume-progress.json'
SAFE_PROGRESS = HERE / 'coordinated-resume-progress.json'
RESULT = HERE / 'coordinated-resume-result.json'
SNAPSHOT = PRIVATE / 'before-transition'
TESTS = HERE / 'test_coordinated_resume.py'


OWNER = None


class CancellationOwner:
    def __init__(self, cancelled):
        self.cancelled = cancelled
        self.write_refusal = M.write

    def check(self):
        M.require(not self.cancelled[0], 'Task cancelled; partial effects retained; no later dispatch')

    def perform(self, function, *args, **kwargs):
        self.check()
        result = function(*args, **kwargs)
        self.check()
        return result


@contextlib.contextmanager
def cancellation_owner():
    global OWNER
    if OWNER is not None:
        # Owned commands share the task latch, never replace/reset its handlers.
        yield OWNER
        return
    with M.cancellation_signals() as cancelled:
        OWNER = CancellationOwner(cancelled)
        try:
            yield OWNER
        finally:
            OWNER = None


def checkpoint():
    if OWNER is not None:
        OWNER.check()


def mutate(function, *args, **kwargs):
    checkpoint()
    result = function(*args, **kwargs)
    checkpoint()
    return result


@contextlib.contextmanager
def live_guards(H):
    # Patch only the captured support module, never global os/subprocess APIs.
    # A write already in progress finishes its fsync before observing the latch.
    saved = []
    def guard(module, name, replacement=None):
        function = getattr(module, name)
        saved.append((module, name, function))
        setattr(module, name, replacement or (lambda *a, **kw: OWNER.perform(function, *a, **kw)))
    try:
        for name in ('write', 'publish'):
            guard(M, name)
        for name in ('exchange', 'acquire_lock', 'property_value'):
            guard(H, name)
        guard(M, 'docker', owned_docker)
        yield
    finally:
        for module, name, function in reversed(saved):
            setattr(module, name, function)


def roots_of(binding):
    return {k: Path(v) for k, v in binding['before']['roots'].items()}


def arguments(roots):
    return [str(roots['--data-root'] / 'smith-next/bin/smith-service'), 'serve',
            *[part for k in ('--data-root', '--config-root', '--runtime-root') for part in (k, str(roots[k]))]]


def effective_start(text, expected):
    # Parse one command, not a substring. Unknown fields/command prefixes and
    # ignore_errors are refused; only execution-result fields are volatile.
    match = re.fullmatch(r'\{ path=([^;{}]+) ; argv\[\]=([^;{}]+) ; ignore_errors=no ; '
                         r'start_time=\[[^\]\n]*\] ; stop_time=\[[^\]\n]*\] ; '
                         r'pid=[0-9]+ ; code=(?:\(null\)|[A-Za-z]+) ; status=[0-9]+(?:/[0-9]+)? \}', text)
    M.require(match is not None, 'Effective command shape refused')
    M.require(all(not re.search(r'[\s;{}\\"\']', x) for x in expected), 'Ambiguous command encoding')
    M.require(match[1] == expected[0] and match[2].split(' ') == expected,
              'Effective command configuration drift')
    return expected


def environment(binding):
    home = Path.home()
    observed = {'--data-root': os.environ.get('XDG_DATA_HOME', str(home / '.local/share')),
                '--config-root': os.environ.get('XDG_CONFIG_HOME', str(home / '.config')),
                '--runtime-root': os.environ.get('XDG_RUNTIME_DIR')}
    M.require(observed == binding['before']['roots'], 'Inherited root drift')
    M.require(os.environ.get('HOME') == str(home) and os.geteuid() != 0)
    M.safe_directory(home)
    M.require(bool(os.environ.get('DBUS_SESSION_BUS_ADDRESS')), 'Inherited user manager missing')
    env = {k: os.environ[k] for k in ('HOME', 'XDG_DATA_HOME', 'XDG_CONFIG_HOME',
                                     'XDG_RUNTIME_DIR', 'DBUS_SESSION_BUS_ADDRESS') if k in os.environ}
    env.update(PATH='/usr/bin:/bin', LANG='C')
    return env


def unit_check(H, binding):
    roots = roots_of(binding)
    expected = binding['before']['unit']
    facts = M.unit_facts(H, roots)
    M.require({k: facts[k] for k in ('unit', 'link', 'link_identity')}
              == {k: expected[k] for k in ('unit', 'link', 'link_identity')})
    M.require(H.property_value('LoadState') == 'loaded' and H.property_value('Transient') == 'no')
    M.require(H.property_value('SourcePath') == '')
    args = arguments(roots)
    M.require('ExecStart=' + ' '.join(args) in M.read(Path(facts['unit'])).decode().splitlines())
    effective_start(expected['effective_start'], args)
    effective_start(facts['effective_start'], args)


def joined_driver():
    proof = M.decode(M.pinned_source(PRIVATE / 'native-supervision.json', SUPERVISION))
    M.require(proof == {'driver_pid': DRIVER_PID, 'owned_session': True,
                       'partial_effects_retained': True, 'rollback_attempted': False,
                       'status': 'joined', 'wait_status': 256})
    M.pinned_source(PRIVATE / 'native-driver.log', DRIVER_LOG)
    M.pinned_source(PRIVATE / ('driver-hook-' + str(DRIVER_PID) + '.log'), HOOK_LOG)
    M.require(M.decode(M.read(M.PROGRESS)) == {
        'driver_pid': DRIVER_PID, 'invocation_id': 'e34790ec5f844641b95420e72abaf97e', 'phase': 'driver-running'})
    M.require(not os.path.lexists(Path('/proc') / str(DRIVER_PID)))
    # No surviving hook/driver group descendants, even after leader exit.
    for path in Path('/proc').iterdir():
        if not path.name.isdecimal():
            continue
        try:
            fields = (path / 'stat').read_text().rsplit(')', 1)[1].split()
        except FileNotFoundError:
            continue  # A process that disappeared cannot be a survivor.
        M.require(int(fields[2]) != DRIVER_PID and int(fields[3]) != DRIVER_PID,
                  'Native owned group/session survived')
        if int(fields[1]) == DRIVER_PID:
            raise M.Refused('Native child survived')
        try:
            cmd = (path / 'cmdline').read_bytes().split(b'\0')
        except FileNotFoundError:
            continue
        M.require(os.fsencode(M.DRIVER) not in cmd and
                  not (os.fsencode(HERE / 'coordinated-maintenance.py') in cmd
                       and b'stopped-hook' in cmd), 'Native operation survived')


def load():
    binding = M.load_binding()
    M.require(M.sha(M.encoded(binding)) == BINDING and binding['new_image']['Id'] == NEW_IMAGE)
    M.require(not os.path.lexists(M.HOOK), 'Original hook already published')
    joined_driver()
    return binding


def stopped(H, R, binding):
    roots = roots_of(binding)
    M.require({str(p): list(v) for p, v in H.root_identities(roots).items()} == binding['before']['root_ids'])
    unit_check(H, binding)
    group = Path(binding['before']['group'])
    try:
        meta = group.lstat()
    except FileNotFoundError:
        pass
    else:
        M.require(M.identity(meta) == binding['before']['group_identity'])
    M.require(Path('/sys/fs/cgroup/cgroup.controllers').is_file())
    R.stopped_proof(group, [roots['--runtime-root'] / 'smith-next' / n for n in ('service.sock', 'app.sock')])


def artifacts(binding):
    expected = {'manifest': M.SELECTED, 'helper': M.OLD, 'unit': M.UNIT,
                'metadata.json': ARTIFACT_METADATA, 'service': M.OLD}
    base = PRIVATE / 'artifacts-before'
    M.safe_directory(base, private=True)
    M.require({p.name for p in base.iterdir()} == set(expected))
    for name, digest in expected.items():
        M.require(M.sha(M.read(base / name)) == digest)
    return expected


def image_binding(binding, published):
    # Tagging changes RepoTags and Docker's LastTagTime, not image content.
    # Bind every other inspect field and the exact expected tag membership.
    for key, alias in (('old_image', M.BACKUP_TAG), ('new_image', M.CANDIDATE_TAG)):
        expected = M.copy.deepcopy(binding[key])
        actual = M.image(alias)
        tags = set(expected.pop('RepoTags'))
        if key == 'old_image':
            tags.add(M.BACKUP_TAG)
            if published:
                tags.remove(M.RUNTIME_TAG)
        elif published:
            tags.add(M.RUNTIME_TAG)
        M.require(set(actual.pop('RepoTags')) == tags)
        old_meta = expected.pop('Metadata')
        new_meta = actual.pop('Metadata')
        M.require(set(old_meta) == set(new_meta) == {'LastTagTime'})
        M.require(isinstance(new_meta['LastTagTime'], str) and bool(re.fullmatch(
            r'[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9:.]+Z', new_meta['LastTagTime'])))
        M.require(actual == expected, 'Immutable image inspect drift')


def check(H, R, binding, ledger_sha=None, runtime_image=None, held=False):
    M.require(load() == binding)
    environment(binding)
    stopped(H, R, binding)
    if held:
        H.validate_locks()
        M.require(len(H.HELD_LOCKS) == 3)
        expected = [(roots_of(binding)['--data-root'] / '.smith-next-install.lock', False),
                    (roots_of(binding)['--data-root'], True),
                    (roots_of(binding)['--runtime-root'] / 'smith-next/runner.lock', False)]
        M.require([(p, d) for _, p, d in H.HELD_LOCKS] == expected)
        for fd, path, _ in H.HELD_LOCKS:
            meta = os.fstat(fd)
            M.require(M.lock_line_proof(Path('/proc/self/fdinfo', str(fd)).read_text(),
                                       os.getpid(), meta.st_dev, meta.st_ino))
    ns = roots_of(binding)['--data-root'] / 'smith-next'
    M.pair(ns, M.NEW, M.CLIENT)
    M.require(stat.S_IMODE((ns / 'bin/smith-service').lstat().st_mode) == 0o755)
    M.idle(ns)
    expected_states = dict(binding['before']['states'])
    if ledger_sha is not None:
        expected_states[M.STATES[2]] = ledger_sha
    M.require(M.state_hashes(ns) == expected_states, 'Original state drift')
    if ledger_sha is not None:
        M.assert_record_diff(M.decode(M.read(SNAPSHOT / M.STATES[2])), M.decode(M.read(ns / M.STATES[2])))
    M.require(M.containers(binding['before']['target_proof']) == binding['container'])
    M.require(M.image(M.RUNTIME_TAG)['Id'] == (runtime_image or M.IMAGE)
              and M.image(M.BACKUP_TAG)['Id'] == M.IMAGE
              and M.image(M.CANDIDATE_TAG)['Id'] == NEW_IMAGE)
    image_binding(binding, runtime_image == NEW_IMAGE)
    M.image_config(binding['old_image'], binding['new_image'])
    artifacts(binding)
    return ns


@contextlib.contextmanager
def leases(H, R, binding):
    M.require(not H.HELD_LOCKS)
    # Positive quiescence and joined evidence BEFORE any lock helper (which can
    # otherwise create a missing file). These locks must already exist.
    stopped(H, R, binding)
    joined_driver()
    roots = roots_of(binding)
    specs = [(roots['--data-root'] / '.smith-next-install.lock', False),
             (roots['--data-root'], True),
             (roots['--runtime-root'] / 'smith-next/runner.lock', False)]
    for path, directory in specs:
        M.require(os.path.lexists(path))
        if not directory:
            M.require(stat.S_IMODE(H.safe_file(path).st_mode) == 0o600)
    try:
        for path, directory in specs:
            H.acquire_lock(path, directory=directory)
        H.validate_locks()
        yield
    finally:
        for fd, _, _ in reversed(H.HELD_LOCKS):
            fcntl.flock(fd, fcntl.LOCK_UN)
            os.close(fd)
        H.HELD_LOCKS.clear()


def fixture_sources(binding):
    ns = roots_of(binding)['--data-root'] / 'smith-next'
    M.require(M.state_hashes(ns) == binding['before']['states'])
    original = M.read(ns / M.STATES[2])
    return ns, original, M.updated_ledger(original)


def copies_proof(binding):
    proof = M.decode(M.read(COPIES))
    M.require(proof['binding_sha256'] == BINDING and proof['candidate_sha256'] == M.NEW
              and proof['original_module_sha256'] == ORIGINAL and proof['status'] == 'validated')
    _, original, updated = fixture_sources(binding)
    M.require(proof['ledger_sha256'] == [M.sha(original), M.sha(updated)])
    for index, expected in enumerate(proof['fixtures']):
        base = PRIVATE / ('r' + str(index))
        M.safe_directory(base, private=True)
        states = dict(binding['before']['states'])
        states[M.STATES[2]] = [M.sha(original), M.sha(updated)][index]
        M.require(expected['states'] == states and M.state_hashes(base / 'd/smith-next') == states)
        exe = base / 'd/smith-next/bin/smith-service'
        M.require(M.sha(M.read(exe)) == M.NEW and stat.S_IMODE(exe.lstat().st_mode) == 0o755)
        M.require(not any(os.path.lexists(base / 'r/smith-next' / n) for n in ('service.sock', 'app.sock')))
    M.require(len(proof['fixtures']) == 2)
    return proof


def validate_copies(H, binding):
    # Explicitly isolated: no check(), Docker, systemctl, locks or live lifecycle.
    if os.path.lexists(COPIES):
        return copies_proof(binding)
    ns, original, updated = fixture_sources(binding)
    proof = {'binding_sha256': BINDING, 'candidate_sha256': M.NEW, 'original_module_sha256': ORIGINAL,
             'ledger_sha256': [M.sha(original), M.sha(updated)], 'fixtures': [], 'status': 'validated'}
    for index, (payload, status) in enumerate(((original, 'unknown'), (updated, 'reconciled'))):
        base = PRIVATE / ('r' + str(index))
        base.mkdir(mode=0o700)  # Never reuse a failed/partial fixture silently.
        M.sync_dir(PRIVATE)
        data, config, runtime = [base / n for n in ('d', 'c', 'r')]
        for p in (data, config, runtime, data / 'smith-next', data / 'smith-next/bin'):
            p.mkdir(mode=0o700)
        M.require(len(os.fsencode(runtime / 'smith-next/service.sock')) < 108)
        exe = data / 'smith-next/bin/smith-service'
        M.write(exe, M.read(M.BUNDLE / 'payload/service/smith-service'), 0o755)
        M.require(M.sha(M.read(exe)) == M.NEW)
        fixture = data / 'smith-next'
        for name in M.STATES:
            if name == M.STATES[2]:
                M.write(fixture / name, payload)
            elif os.path.lexists(ns / name):
                M.write(fixture / name, M.read(ns / name))
        before = M.state_hashes(fixture)
        parent = os.getpid()
        child = subprocess.Popen([str(exe), 'serve', '--data-root', str(data), '--config-root', str(config),
                                  '--runtime-root', str(runtime)],
                                 env={'HOME': str(base), 'PATH': '/nonexistent', 'XDG_DATA_HOME': str(data),
                                      'XDG_CONFIG_HOME': str(config), 'XDG_RUNTIME_DIR': str(runtime)},
                                 stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                 start_new_session=True, preexec_fn=lambda: M.parent_death_guard(parent))
        try:
            H.wait_app(runtime, child)
            core = H.exchange(runtime / 'smith-next/service.sock',
                              {'schema_version': 1, 'service': 'smith-next', 'command': 'get_state'}, child.pid)
            reply = H.exchange(runtime / 'smith-next/app.sock', H.app_request(), child.pid)
            M.require(core['ok'] is True)
            jobs = reply['result']['jobs']
            M.require(len(jobs) == 7 and len([j for j in jobs if j['request_id'] == M.REQUEST
                                            and j['status'] == status]) == 1)
            M.require(M.state_hashes(fixture) == before, 'Fixture state changed')
        finally:
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)  # Owned isolated reader only.
                child.wait(timeout=2)
                raise M.Refused('Fixture cleanup exceeded budget') from None
        M.require(M.state_hashes(fixture) == before)
        proof['fixtures'].append({'states': before})
        M.sync_dir(base)
    M.require(M.state_hashes(ns) == binding['before']['states'])
    M.write(COPIES, M.encoded(proof))
    return copies_proof(binding)


def review(binding):
    receipt = M.decode(M.read(PRIVATE / 'resume-review.json'))
    M.require(receipt == {'script_sha256': M.sha(M.read(Path(__file__), source=True)),
                          'tests_sha256': M.sha(M.read(TESTS, source=True)),
                          'copies_proof_sha256': M.sha(M.read(COPIES)),
                          'binding_sha256': BINDING, 'original_module_sha256': ORIGINAL,
                          'verdict': 'pass', 'independent_review': True}, 'Fresh independent review required')
    copies_proof(binding)


def snapshot(binding, ns, locks):
    mutate(SNAPSHOT.mkdir, mode=0o700)
    M.sync_dir(PRIVATE)
    for name, expected in binding['before']['states'].items():
        if expected is not None:
            payload = M.read(ns / name)
            M.require(M.sha(payload) == expected)
            M.write(SNAPSHOT / name, payload)
    M.write(SNAPSHOT / 'state-presence.json', M.encoded(binding['before']['states']))
    M.write(SNAPSHOT / 'original-binding.json', M.encoded(binding))
    for name in artifacts(binding):
        M.write(SNAPSHOT / ('artifact-' + name), M.read(PRIVATE / 'artifacts-before' / name))
    original = M.read(SNAPSHOT / M.STATES[2])
    updated = M.updated_ledger(original)
    M.write(SNAPSHOT / 'original-target.json', M.encoded(M.target(M.decode(original))))
    M.write(SNAPSHOT / 'updated-ledger.json', updated)
    M.write(SNAPSHOT / 'locks.json', M.encoded(locks))
    M.sync_dir(SNAPSHOT)
    M.sync_dir(PRIVATE)
    return original, updated


def phase(value, name):
    M.require(M.decode(M.read(PROGRESS)) == value, 'Continuation phase ownership drift')
    value = dict(value, phase=name)
    M.publish(PROGRESS, value)
    M.publish(SAFE_PROGRESS, value)
    return value


def observation(value, prepare, roots):
    required = {'schema_version', 'service', 'ok', 'binary_state', 'unit_state', 'enablement_state',
                'runtime_state', 'bridge_state', 'setup_complete', 'message', 'error', 'effects'}
    M.require(set(value) == required | ({'approval_token'} if prepare else set()))
    M.require(value['schema_version'] == 2 and value['service'] == 'smith-next'
              and value['ok'] is True and value['error'] is None
              and value['binary_state'] == 'installed' and value['unit_state'] == 'installed'
              and value['enablement_state'] == 'enabled')
    effects = value['effects']
    M.require(set(effects) == {'binary_path', 'unit_path', 'state_path', 'socket_path', 'description'})
    M.require(effects['binary_path'] == arguments(roots)[0]
              and effects['unit_path'] == str(roots['--config-root'] / 'systemd/user/smith-next.service')
              and effects['state_path'] == str(roots['--data-root'] / 'smith-next/state.json')
              and effects['socket_path'] == str(roots['--runtime-root'] / 'smith-next/service.sock'))
    if prepare:
        token = value['approval_token']
        M.require(isinstance(token, str) and re.fullmatch(r'[0-9]{1,20}\.[0-9a-f]{64}', token))
        M.require(value['runtime_state'] == 'stopped')
        return token
    M.require(value['runtime_state'] == 'running' and value['bridge_state'] == 'ready')


def group_members(pid):
    members = []
    for path in Path('/proc').iterdir():
        if not path.name.isdecimal():
            continue
        try:
            fields = (path / 'stat').read_text().rsplit(')', 1)[1].split()
        except FileNotFoundError:
            continue
        if int(fields[2]) == pid and fields[0] != 'Z':
            members.append(int(path.name))
    return members


def cancel_core(pid, group_ready):
    # Keep the leader unreaped to pin its owned group identity. Never signal the
    # managed service, which systemd starts in another cgroup. The deadline only
    # escalates TERM to KILL: it is NOT permission to release publication leases.
    # Unknown /proc/wait/signal facts retain supervision indefinitely. Only a
    # positively exited leader AND no effect-capable group members permit join
    # and return. Before the session handshake the closed exec gate guarantees
    # that the captured child cannot yet have launched descendants.
    if OWNER is not None:
        OWNER.cancelled[0] = True
    deadline = time.monotonic() + 5
    sent = None
    while True:
        sig = signal.SIGKILL if time.monotonic() >= deadline else signal.SIGTERM
        if sent != sig:
            try:
                (os.killpg if group_ready else os.kill)(pid, sig)
                sent = sig
            except ProcessLookupError:
                sent = sig  # Still require positive wait/group evidence below.
            except Exception:
                time.sleep(0.05)
                continue  # Failed signalling must not unwind the held leases.
        try:
            exited = os.waitid(os.P_PID, pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
            absent = exited is not None and (not group_ready or not group_members(pid))
            if absent:
                return os.waitpid(pid, 0)[1]
        except Exception:
            # Includes enumeration/decoding errors and interrupted/failed wait.
            # Do not reap the leader or interpret unknown as group quiescence.
            pass
        time.sleep(0.05)


def owned_command(args, env, timeout=35, merge_stderr=False, limit=256 * 1024):
    # Same gate/session/parent-death/cooperative cancellation protocol as e06.
    # Only bounded JSON is retained in memory. No token, response or stderr log.
    out_r, out_w = os.pipe()
    gate_r, gate_w = os.pipe()
    ready_r, ready_w = os.pipe()
    fds = {out_r, out_w, gate_r, gate_w, ready_r, ready_w}
    pid, status, group_ready = None, None, False
    parent = os.getpid()
    with cancellation_owner() as owner:
        try:
            owner.check()
            pid = os.fork()
            if pid == 0:
                try:
                    for fd in (out_r, gate_w, ready_r):
                        os.close(fd)
                    for sig in (signal.SIGINT, signal.SIGTERM):
                        signal.signal(sig, signal.SIG_DFL)
                    M.parent_death_guard(parent)
                    os.setsid()
                    os.write(ready_w, b'1')
                    os.close(ready_w)
                    M.require(os.read(gate_r, 1) == b'1' and os.getppid() == parent)
                    os.close(gate_r)
                    null = os.open(os.devnull, os.O_RDWR)
                    os.dup2(null, 0)
                    os.dup2(null, 2)
                    os.dup2(out_w, 1)
                    if merge_stderr:
                        os.dup2(out_w, 2)
                    os.close(null)
                    os.close(out_w)
                    os.execve(args[0], args, env)
                except BaseException:
                    os._exit(125)
            for fd in (out_w, gate_r, ready_w):
                os.close(fd)
                fds.remove(fd)
            M.require(select.select([ready_r], [], [], 5)[0] and os.read(ready_r, 1) == b'1')
            M.require(os.getpgid(pid) == pid)
            group_ready = True
            owner.check()
            os.write(gate_w, b'1')
            os.close(gate_w)
            fds.remove(gate_w)
            deadline = time.monotonic() + timeout
            data = bytearray()
            eof = False
            exited = None
            while not eof or exited is None:
                owner.check()
                M.require(time.monotonic() < deadline, 'Owned command deadline')
                if not eof and select.select([out_r], [], [], 0.05)[0]:
                    block = os.read(out_r, 8192)
                    eof = not block
                    data.extend(block)
                    M.require(len(data) <= limit, 'Owned command response bound')
                if exited is None:
                    exited = os.waitid(os.P_PID, pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
                if eof and exited is None:
                    time.sleep(0.01)
            M.require(not group_members(pid), 'Owned Core descendants survived')
            status = os.waitpid(pid, 0)[1]
            owner.check()  # Includes cancellation delivered during the final join.
            M.require(os.waitstatus_to_exitcode(status) == 0, 'Owned command incomplete; no retry')
            return bytes(data)
        finally:
            try:
                if pid is not None and pid > 0 and status is None:
                    cancel_core(pid, group_ready)
            finally:
                for fd in fds:
                    os.close(fd)


def owned_core(args, env, timeout=35):
    with cancellation_owner() as owner:
        data = owned_command(args, env, timeout=timeout)
        owner.check()
        M.require(data.endswith(b'\n') and data.count(b'\n') == 1)
        result = M.decode(data)
        owner.check()  # JSON decoding must not discard a late task cancellation.
        return result


def owned_docker(args, timeout=10):
    checkpoint()
    M.require(args[:2] in (['image', 'inspect'], ['image', 'tag'], ['container', 'ls'],
                          ['container', 'inspect'], ['container', 'logs']))
    if args[:2] == ['image', 'tag']:
        M.require(args == ['image', 'tag', NEW_IMAGE, M.RUNTIME_TAG])
    config = PRIVATE / 'docker-config'
    M.safe_directory(config, private=True)
    M.require(not any(config.iterdir()))
    result = owned_command(['/usr/bin/docker', '--config', str(config), '--host',
                            'unix:///run/docker.sock', *args], M.ENV, timeout=timeout,
                           merge_stderr=True, limit=16 * 1024 * 1024)
    checkpoint()
    if args[:2] == ['image', 'tag']:
        M.require(result == b'', 'Unexpected tag output; partial effect retained')
    return result


def core_command(H, R, binding, updated_sha, args):
    # A fresh binding check for EACH Core command; no held recursive Core lease.
    M.require(not H.HELD_LOCKS)
    check(H, R, binding, updated_sha, NEW_IMAGE)
    env = environment(binding)
    exe = M.BUNDLE / 'payload/service/smith-service'
    M.require(M.sha(M.read(exe)) == M.NEW and stat.S_IMODE(exe.lstat().st_mode) == 0o755)
    checkpoint()
    result = owned_core([str(exe), *args], env)
    checkpoint()
    return result


def running(H, binding, original, updated, app_sha=None):
    M.require(load() == binding)
    roots = roots_of(binding)
    unit_check(H, binding)
    M.require({str(p): list(v) for p, v in H.root_identities(roots).items()} == binding['before']['root_ids'])
    M.require(H.property_value('ActiveState') == 'active' and H.property_value('SubState') == 'running')
    pid = int(H.property_value('MainPID'))
    M.require(pid > 0 and pid != binding['before']['pid'])
    proc = Path('/proc') / str(pid)
    ns = roots['--data-root'] / 'smith-next'
    M.require((proc / 'exe').resolve() == ns / 'bin/smith-service'
              and M.sha((proc / 'exe').read_bytes()) == M.NEW
              and (proc / 'cmdline').read_bytes().split(b'\0') == [os.fsencode(a) for a in arguments(roots)] + [b''])
    M.pair(ns, M.NEW, M.CLIENT)
    H.exchange(roots['--runtime-root'] / 'smith-next/service.sock',
               {'schema_version': 1, 'service': 'smith-next', 'command': 'get_state'}, pid)
    reply = H.exchange(roots['--runtime-root'] / 'smith-next/app.sock', H.app_request(), pid)
    M.require(len(reply['result']['jobs']) == 7 and len([j for j in reply['result']['jobs']
                  if j['request_id'] == M.REQUEST and j['status'] == 'reconciled']) == 1)
    M.require(M.read(ns / M.STATES[2]) == updated)
    M.assert_record_diff(M.decode(original), M.decode(updated))
    expected_states = dict(binding['before']['states'])
    expected_states[M.STATES[2]] = M.sha(updated)
    if app_sha is not None:
        expected_states[M.STATES[0]] = app_sha
    M.require(M.state_hashes(ns) == expected_states)
    M.require(M.containers(binding['before']['target_proof']) == binding['container']
              and M.image(M.RUNTIME_TAG)['Id'] == NEW_IMAGE)
    return roots, pid


def continuation(H, R, binding):
    with cancellation_owner() as owner, live_guards(H):
        owner.check()
        _continuation(H, R, binding)
        owner.check()


def replace_ledger(staged, destination):
    # Complete rename + directory durability even if TERM arrives between them.
    os.replace(staged, destination)
    M.sync_dir(destination.parent)


def _continuation(H, R, binding):
    checkpoint()
    review(binding)
    M.require(not any(os.path.lexists(p) for p in (PROGRESS, SAFE_PROGRESS, RESULT, SNAPSHOT)), 'No replay')
    check(H, R, binding)
    value = {'phase': 'claimed', 'invocation_id': M.uuid.uuid4().hex, 'automatic_retry': False,
             'rollback_attempted': False, 'dev_job_submitted': False}
    M.write(PROGRESS, M.encoded(value))
    M.write(SAFE_PROGRESS, M.encoded(value))
    try:
        with leases(H, R, binding):
            ns = check(H, R, binding, held=True)
            locks = [[str(p), M.identity(os.fstat(fd))] for fd, p, _ in H.HELD_LOCKS]
            value = phase(value, 'snapshot-attempted')
            original, updated = snapshot(binding, ns, locks)
            value = phase(value, 'snapshot-durable')
            check(H, R, binding, held=True)
            staged = ns / 'dev-local-jobs.json.resume-staging'
            M.write(staged, updated)
            value = phase(value, 'ledger-publish-attempted')
            check(H, R, binding, held=True)
            M.require(M.read(ns / M.STATES[2]) == original)
            mutate(replace_ledger, staged, ns / M.STATES[2])
            digest = M.sha(updated)
            check(H, R, binding, digest, held=True)
            value = phase(value, 'ledger-confirmed-image-publish-attempted')
            check(H, R, binding, digest, held=True)
            checkpoint()
            M.docker(['image', 'tag', NEW_IMAGE, M.RUNTIME_TAG])
            checkpoint()
            check(H, R, binding, digest, NEW_IMAGE, held=True)
            value = phase(value, 'ledger-and-image-confirmed')
        M.require(not H.HELD_LOCKS)
        value = phase(value, 'start-prepare-attempted')
        checkpoint()
        prepared = core_command(H, R, binding, digest, ['prepare', '--action', 'start'])
        checkpoint()
        token = observation(prepared, True, roots_of(binding))
        del prepared
        value = phase(value, 'start-apply-attempted')
        checkpoint()
        applied = core_command(H, R, binding, digest, ['apply', '--action', 'start', '--approval', token])
        checkpoint()
        del token
        observation(applied, False, roots_of(binding))
        del applied
        roots, pid = running(H, binding, original, updated)
        value = phase(value, 'started-verified-inventory-dispatch')
        checkpoint()
        inventory = M.metadata_inventory(H, roots, pid)
        checkpoint()
        # Inventory verifies its sole app-state delta, frozen model and all other
        # state hashes. Recheck the exact ledger and paired identity afterwards.
        ns = roots['--data-root'] / 'smith-next'
        M.require(M.read(ns / M.STATES[2]) == updated and inventory['model_ready'] is True)
        final_roots, final_pid = running(H, binding, original, updated, inventory['app_after_sha256'])
        M.require(final_roots == roots and final_pid == pid)
        value = phase(value, 'complete')
        M.write(RESULT, M.encoded({'status': 'core-app-ready-reconciled-inventory-refreshed',
                'candidate_sha256': M.NEW, 'client_manifest_sha256': M.CLIENT, 'worker_image': NEW_IMAGE,
                'original_ledger_sha256': M.sha(original), 'updated_ledger_sha256': digest,
                'request_id': M.REQUEST, 'inventory_request_id': inventory['request_id'],
                'model_ready': True, 'dev_job_submitted': False, 'release_accepted': False,
                'timestamp_failure_waived': False, 'rollback_attempted': False}))
    except BaseException:
        # Do not mask the last attempted phase or leak exception/response text.
        try:
            retained = M.decode(M.read(PROGRESS))
            M.require(retained['invocation_id'] == value['invocation_id'])
            # Evidence-only exception to the cancellation fence: no host/state
            # publication or dispatch, and all owned children/leases unwound.
            OWNER.write_refusal(PRIVATE / 'resume-refusal.json', M.encoded({
                'status': 'partial-or-refused', 'last_phase': retained['phase'],
                'cancelled': OWNER.cancelled[0], 'automatic_retry': False, 'rollback_attempted': False}))
        except BaseException:
            pass  # Never mask the original failure or resume after diagnostic failure.
        raise


def main():
    M.require(not sys.flags.optimize and os.geteuid() != 0, 'Audited assertions and non-root required')
    os.umask(0o077)
    M.require(sys.argv[1:] in (['--validate-copies'], ['--apply-reviewed-continuation']))
    binding = M.load_binding()  # Source/artifact checks only; no host tools.
    M.require(M.sha(M.encoded(binding)) == BINDING and binding['new_image']['Id'] == NEW_IMAGE)
    H, R = M.support()
    if sys.argv[1:] == ['--validate-copies']:
        validate_copies(H, binding)
        print('Isolated candidate readers validated; no live effects.')
    else:
        env = environment(binding)
        def property_value(name):
            result = subprocess.run(['/usr/bin/systemctl', '--user', 'show', 'smith-next.service',
                                     '--property=' + name, '--value'], env=env, stdin=subprocess.DEVNULL,
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10)
            M.require(result.returncode == 0 and not result.stderr and len(result.stdout) < 16384)
            return result.stdout.decode().strip()
        H.property_value = property_value
        continuation(H, R, binding)
        print('Continuation complete; QA only, not release acceptance.')


if __name__ == '__main__':
    try:
        main()
    except BaseException:
        print('Continuation refused or partial; preserve private phases; no automatic retry.', file=sys.stderr)
        sys.exit(1)
