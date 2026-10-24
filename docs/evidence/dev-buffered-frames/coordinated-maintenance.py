"""One-shot coordinated maintenance. Import is inert; modes require fresh review.

No substitute installer, manager action, job submission, retry, rollback or window
management. Private bindings and original bytes never enter repository evidence.
"""
import ast
import copy
import hashlib
import ctypes
import contextlib
import select
import signal
import types
import json
import os
from pathlib import Path
import re

import stat
import subprocess
import sys

import time
import uuid

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
EVIDENCE = Path(__file__).resolve().parent
PRIVATE = Path.home() / '.local/state/smith-qa-backups/dev-coordinated-20261019'
BUNDLE = ROOT / '.smith-dev-coordinated-candidate-v2'
DRIVER = EVIDENCE / 'coordinated-driver/smith-coordinated-driver'
# Parent-supplied driver pin; never discover/accept a replacement hash automatically.
DRIVER_SHA = '37d1708b7221d68ede0e0142171512cc80983292f209b515ea93cd86b6f7b3f8'
V2_RESULT = '21f04bb53b87b8e840b690f1206b6fa4b0439bb63eeda4030f7059a9d3137722'
V2_VALIDATION = 'fbccbb7e026976008b9993fc56575e120e61ecae504cf232a2855d5efecbfa1d'
AOT = '96a1fa187d543784ed1dad84762d58d6ea75852c71fb5f29bdcf44ac80d78f25'
OLD = 'b7790a585d568373c0c3207e34ea4bbbeec080f863ff9ecec0a08b60a25c9d10'
NEW = '9bb0228c61bf762c7899f15f7105924d666b6e534102a33c6ecea8779fee77ec'
GUARD = 'b8c512c1b80382783bec41a76204815a9ddf3c234cee6c8ad6e721295e2489f0'
CLIENT = '49ba82e101bd54799948d4104cd6faa892d54ed1b2bc3511c4ac28e029abc9b5'
PACKAGE = 'ddb1e8077249da1b5d9bdd229519f7894d8443e4eeb775318c3429da3e01436a'
SELECTED = '9f670a310ffc1f883c5fdb6b93ebcebdf93b035de8928b8ba98e789b0c9343fb'
UNIT = 'd1deeb0d1525212edbc56bbb5a8eb72bec327612e97f7b30aba045cf95c49ab3'
LEDGER = '1cd468a8b41cdf2bf5360b912ffe652a4e09610564f0b1657261de4e63ac2b38'
REQUEST = 'ae8149524f0cd9e3d9f95a8205d7ee4a'
IMAGE = 'sha256:e799b3750e8f8e6dd3f7256f040e625996a2301d7566a934c1d14ea706e94df7'
CONTAINER = '54e3c8bf4b9367b8bd53482f31352d7310844aa3c8eaa8f50dc428bb9fec4319'
OWNER = '7e6eb0a86d2a83204921713873753713'
RUNTIME_TAG = 'smith-dev-worker-local:1.19.3'
BACKUP_TAG = 'smith-dev-worker-local:before-coordinated-20261019'
CANDIDATE_TAG = 'smith-dev-worker-local:coordinated-verified-20261019'
STATES = ('app-state.json', 'dev-jobs.json', 'dev-local-jobs.json', 'state.json')
PREP = EVIDENCE / 'coordinated-prepared.json'
PROGRESS = EVIDENCE / 'coordinated-progress.json'
HOOK = EVIDENCE / 'coordinated-reconciliation.json'
ENV = {'PATH': '/usr/bin:/bin', 'HOME': '/nonexistent', 'LANG': 'C', 'DOCKER_BUILDKIT': '0'}
SUPPORT = {
    'restore_service.py': 'd609474fbbcdf63c74e6129f278b4cd1e3893d6bd3d3b843fc81b2a3044bff51',
    'reconcile_logging_failure.py': '7ed1d0c8419ae5060dba7e062725b2039b9b6f872516d6b820db8c2f1c772e1c',
}


class Refused(RuntimeError):
    pass


def require(ok, reason='Binding refused'):
    if not ok:
        raise Refused(reason)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def identity(m):
    return [m.st_dev, m.st_ino, m.st_uid, m.st_mode]


def file_identity(m):
    # Reading can update atime. It is not evidence of content/identity drift.
    return [*identity(m), m.st_nlink, m.st_size, m.st_mtime_ns, m.st_ctime_ns]


def read(path, limit=128 * 1024 * 1024, source=False):
    m = path.lstat()
    require(stat.S_ISREG(m.st_mode) and m.st_uid == os.geteuid()
            and (source or not m.st_mode & 0o022) and m.st_nlink == 1 and m.st_size <= limit)
    with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), 'rb') as f:
        before = os.fstat(f.fileno())
        require(file_identity(before) == file_identity(m))
        data = f.read(limit + 1)
        require(file_identity(os.fstat(f.fileno())) == file_identity(before)
                and file_identity(path.lstat()) == file_identity(m) and len(data) == m.st_size)
    return data


def unique(pairs):
    result = {}
    for k, v in pairs:
        require(k not in result, 'Duplicate JSON member')
        result[k] = v
    return result


def decode(data):
    return json.loads(data, object_pairs_hook=unique)


def encoded(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False) + '\n').encode()


def sync_dir(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def write(path, data, mode=0o600):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, mode)
    with os.fdopen(fd, 'wb') as f:
        os.fchmod(f.fileno(), mode)
        f.write(data)
        f.flush()
        os.fsync(f.fileno())
    sync_dir(path.parent)


def publish(path, value):
    staged = path.with_name(path.name + '.staging')
    write(staged, encoded(value))
    os.replace(staged, path)
    sync_dir(path.parent)


def safe_directory(path, private=False):
    require(path.is_absolute() and '..' not in path.parts)
    for p in [*reversed(path.parents), path]:
        m = p.lstat()
        require(stat.S_ISDIR(m.st_mode) and m.st_uid in (0, os.geteuid()) and not m.st_mode & 0o022)
    if private:
        m = path.lstat()
        require(m.st_uid == os.geteuid() and stat.S_IMODE(m.st_mode) == 0o700)


def pinned_source(path, expected):
    # Source checkout files need not have recipient artifact modes. Never execute
    # through their pathname: only independently digest-bound captured bytes.
    data = read(path, source=True)
    require(sha(data) == expected, 'Reviewed source drift')
    return data


def support():
    path = ROOT / 'docs/evidence/dev-rootful-qa/restore_service.py'
    data = pinned_source(path, SUPPORT[path.name])
    H = types.ModuleType('coordinated_pinned_restore')
    H.__file__ = str(path)
    exec(compile(data, '<pinned-restore-support>', 'exec'), H.__dict__)
    path = path.with_name('reconcile_logging_failure.py')
    data = pinned_source(path, SUPPORT[path.name])
    tree = ast.parse(data)
    nodes = [node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == 'stopped_proof']
    require(len(nodes) == 1)
    R = types.ModuleType('coordinated_pinned_stopped_proof')
    R.__dict__.update(H=H, Path=Path, os=os)
    # Extract exactly the pinned proof; do not import its old installer/actions or
    # re-open its group-writable support path through importlib after hashing.
    exec(compile(ast.Module(body=nodes, type_ignores=[]), '<pinned-stopped-proof>', 'exec'), R.__dict__)
    return H, R


def driver_pin():
    require(isinstance(DRIVER_SHA, str) and re.fullmatch('[0-9a-f]{64}', DRIVER_SHA),
            'Rebuilt driver pin pending parent review; all modes disabled')
    return DRIVER_SHA


def production_paths():
    # Exact membership contract used by the verified v2 preparation, not v1's
    # mutable docs/plans/tests/evidence map. Membership additions also block.
    paths = set()
    for folder in ('service/src', 'service/dev-worker', 'service/scripts',
                   'client/lib', 'client/assets', 'client/linux', 'client/scripts'):
        for p in (ROOT / folder).rglob('*'):
            if (p.is_file() and not {'ephemeral', '__pycache__'} & set(p.parts)
                    and not p.name.endswith(('_tests.rs', '.test.mjs')) and p.name != 'tests.rs'):
                paths.add(p.relative_to(ROOT).as_posix())
    paths.update(('service/Cargo.toml', 'service/Cargo.lock', 'service/.dockerignore',
                  'client/pubspec.yaml', 'client/pubspec.lock', 'client/.metadata', 'client/analysis_options.yaml'))
    return paths


def v2_records(result, validation):
    require(result['status'] == 'prepared_verified_candidate_only_not_installed_or_accepted'
            and result['package'] == '.smith-dev-coordinated-candidate-v2'
            and result['service_sha256'] == NEW and result['guard_sha256'] == GUARD
            and result['client_manifest_sha256'] == CLIENT and result['package_manifest_sha256'] == PACKAGE
            and result['flutter_libapp_sha256'] == AOT and len(result['candidate_inventory']) == 97)
    require(result['validation_record'] == 'docs/evidence/dev-buffered-frames/coordinated-candidate-v2-validation.json'
            and result['validation_record_sha256'] == V2_VALIDATION
            and validation['status'] == 'aligned_navigation_and_dev_tests_analysis_pass_current_aot_freshness_verified')
    require(len(result['source_sha256']) == 132 and result['source_sha256'] == validation['source_sha256']
            and result['flutter_build_inventory'] == validation['flutter_build_inventory']
            and validation['flutter_build_inventory']['lib/libapp.so']['sha256'] == AOT
            and validation['expectation_alignment']['all_other_bytes_unchanged'] is True)
    require(set(validation['checks']) == {'tests', 'analyze'}
            and all(c['exit_code'] == 0 for c in validation['checks'].values())
            and validation['retained_current_linux_build_check']['exit_code'] == 0)
    return result['source_sha256']


def source_check():
    driver_pin()
    data = pinned_source(EVIDENCE / 'coordinated-candidate-v2-aligned-result.json', V2_RESULT)
    result = decode(data)
    validation = decode(pinned_source(EVIDENCE / 'coordinated-candidate-v2-validation.json', V2_VALIDATION))
    sources = v2_records(result, validation)
    require(production_paths() == set(sources), 'Production input membership drift')
    for name, expected in sources.items():
        pinned_source(ROOT / name, expected)
    for check in [*validation['checks'].values(), validation['retained_current_linux_build_check']]:
        relative = Path(check['log'])
        require(not relative.is_absolute() and '..' not in relative.parts)
        pinned_source(ROOT / relative, check['log_sha256'])
    safe_directory(BUNDLE, private=True)
    manifest_bytes = read(BUNDLE / 'manifest.json')
    require(sha(manifest_bytes) == PACKAGE)
    manifest = decode(manifest_bytes)
    require(manifest['files'] == result['candidate_inventory'])
    require(sha(read(BUNDLE / 'SHA256SUMS')) == result['checksums_sha256'])
    require(sha(read(BUNDLE / 'payload/client/lib/libapp.so')) == AOT)
    require(manifest['service_sha256'] == NEW and manifest['client_manifest_sha256'] == CLIENT
            and len(manifest['files']) == 97)
    for name, entry in manifest['files'].items():
        require(not Path(name).is_absolute() and '..' not in Path(name).parts)
        p = BUNDLE / name
        safe_directory(p.parent)
        require(sha(read(p)) == entry['sha256'] and stat.S_IMODE(p.lstat().st_mode) == entry['mode'])
    require(sha(read(BUNDLE / 'payload/service/smith-service')) == NEW
            and sha(read(BUNDLE / 'payload/client/smith_backend/smith-service')) == NEW
            and sha(pinned_source(ROOT / 'service/dev-worker/guard.mjs', GUARD)) == GUARD)
    observed = set()
    for folder, dirs, files in os.walk(BUNDLE, onerror=lambda e: (_ for _ in ()).throw(e)):
        for name in dirs:
            safe_directory(Path(folder) / name)
        for name in files:
            p = Path(folder) / name
            read(p)
            observed.add(p.relative_to(BUNDLE).as_posix())
    require(observed == set(manifest['files']) | {'manifest.json', 'SHA256SUMS'}, 'Candidate inventory membership drift')
    require(sha(read(DRIVER)) == DRIVER_SHA and stat.S_IMODE(DRIVER.lstat().st_mode) == 0o755)
    return {'script': sha(read(Path(__file__).resolve())), 'driver': DRIVER_SHA,
            'candidate_result': sha(data), 'validation': V2_VALIDATION}


def target(ledger, expected='unknown'):
    found = [r for r in ledger['records'] if r['summary']['request_id'] == REQUEST]
    require(len(found) == 1 and len(ledger['records']) == 7)
    r = found[0]
    s = r['summary']
    require(s['status'] == expected and s['cleanup_verified'] is True
            and r['cleanup_required'] is False and r['may_have_sent'] is True
            and r['failed_stage'] == 'running' and r['image_id'] == IMAGE
            and s['reply_available'] is False and r['reply'] is None and r['message_id'] is None
            and isinstance(s['session_id'], str) and bool(re.fullmatch(r'[A-Za-z0-9_-]{1,256}', s['session_id']))
            and r['repositories'] and len(r['repositories']) == len(r['job']['repositories']))
    for checkout, frozen in zip(r['repositories'], r['job']['repositories']):
        require(checkout['url'] == frozen['url'] and re.fullmatch('[0-9a-f]{40}', checkout['commit'])
                and isinstance(checkout['ref'], str) and bool(checkout['ref']))
    return r


def updated_ledger(original):
    ledger = decode(original)
    require(ledger['schema_version'] == 1)
    target(ledger)
    updated = copy.deepcopy(ledger)
    updated['schema_version'] = 3
    target(updated)['summary']['status'] = 'reconciled'
    assert_record_diff(ledger, updated)
    payload = encoded(updated)
    require(len(payload) <= 4 * 1024 * 1024)
    return payload


def assert_record_diff(before, after):
    target(before)
    target(after, 'reconciled')
    require(after['schema_version'] == 3)
    reverse = copy.deepcopy(after)
    reverse['schema_version'] = before['schema_version']
    target(reverse, 'reconciled')['summary']['status'] = 'unknown'
    require(reverse == before, 'Unapproved record delta')


def idle(namespace):
    app = decode(read(namespace / STATES[0]))
    require(all(j['operation']['status'] not in ('queued', 'running') for j in app['jobs']))
    for name in STATES[1:3]:
        ledger = decode(read(namespace / name))
        for r in ledger['records']:
            if name == STATES[2] and r['summary']['request_id'] == REQUEST:
                continue
            if name == STATES[1]:
                # Receipt summaries predate the local runner's cleanup proof field.
                require(r['summary']['status'] in ('failed', 'recorded')
                        and r['cleanup_required'] is False)
            else:
                require(r['summary']['status'] in ('failed', 'completed', 'reconciled')
                        and r['cleanup_required'] is False and r['summary']['cleanup_verified'] is True)
    return app


def state_hashes(namespace):
    return {name: sha(read(namespace / name)) if os.path.lexists(namespace / name) else None for name in STATES}


def pair(namespace, binary, selected):
    require(os.readlink(namespace / 'client/current') == 'releases/' + selected)
    release = namespace / 'client/releases' / selected
    safe_directory(release)
    data = read(release / '.smith-next-manifest.json')
    require(sha(data) == selected)
    manifest = decode(data)
    require(manifest['schema'] == 1 and data == encoded(manifest))
    actual_names = set()
    for directory, dirs, files in os.walk(release, onerror=lambda e: (_ for _ in ()).throw(e)):
        for name in dirs + files:
            p = Path(directory) / name
            relative = p.relative_to(release).as_posix()
            if relative == '.smith-next-manifest.json':
                continue
            actual_names.add(relative)
            entry = manifest['entries'][relative]
            m = p.lstat()
            require(m.st_uid == os.geteuid() and stat.S_IMODE(m.st_mode) == entry['mode'] and not m.st_mode & 0o022)
            if entry['type'] == 'directory':
                require(stat.S_ISDIR(m.st_mode))
            else:
                require(entry['type'] == 'file' and sha(read(p)) == entry['sha256'])
    require(actual_names == set(manifest['entries']))
    require(sha(read(namespace / 'bin/smith-service')) == binary
            and sha(read(release / 'smith_backend/smith-service')) == binary)
    return release


def unit_facts(H, roots):
    unit = Path(H.property_value('FragmentPath'))
    require(unit == roots['--config-root'] / 'systemd/user/smith-next.service' and sha(read(unit)) == UNIT)
    require(H.property_value('NeedDaemonReload') == 'no' and H.property_value('DropInPaths') == ''
            and H.property_value('UnitFileState') == 'enabled' and H.property_value('Job') in ('', '0'))
    link = unit.parent / 'default.target.wants/smith-next.service'
    require(link.lstat().st_uid == os.geteuid() and link.is_symlink() and os.readlink(link) == str(unit))
    return {'unit': str(unit), 'link': os.readlink(link), 'link_identity': identity(link.lstat()),
            'effective_start': H.property_value('ExecStart')}


def capture(H):
    require(H.property_value('ActiveState') == 'active' and H.property_value('SubState') == 'running')
    pid = int(H.property_value('MainPID'))
    require(pid == 321258)
    proc = Path('/proc') / str(pid)
    require(sha((proc / 'exe').read_bytes()) == OLD)
    args = [os.fsdecode(x) for x in (proc / 'cmdline').read_bytes().split(b'\0') if x]
    require(len(args) == 8 and args[1] == 'serve' and args[2::2] == ['--data-root', '--config-root', '--runtime-root'])
    roots = dict(zip(args[2::2], map(Path, args[3::2])))
    namespace = roots['--data-root'] / 'smith-next'
    require(args[0] == str(namespace / 'bin/smith-service') and (proc / 'exe').resolve() == Path(args[0]))
    root_ids = {str(p): list(v) for p, v in H.root_identities(roots).items()}
    unit = unit_facts(H, roots)
    require('ExecStart=' + ' '.join(args) in read(Path(unit['unit'])).decode().splitlines())
    require(f"path={args[0]} ;" in unit['effective_start'] and f"argv[]={' '.join(args)} ;" in unit['effective_start'])
    group = H.property_value('ControlGroup')
    require(group.startswith('/') and '..' not in Path(group).parts and group != '/')
    require((proc / 'cgroup').read_text().splitlines() == ['0::' + group])
    cgroup = Path('/sys/fs/cgroup') / group.lstrip('/')
    require(cgroup.is_dir() and Path('/sys/fs/cgroup/cgroup.controllers').is_file())
    require(pid in [int(v) for v in (cgroup / 'cgroup.procs').read_text().split()])
    release = pair(namespace, OLD, SELECTED)
    idle(namespace)
    hashes = state_hashes(namespace)
    require(hashes[STATES[2]] == LEDGER)
    record = target(decode(read(namespace / STATES[2])))
    target_proof = {'session_id': record['summary']['session_id'], 'repositories': record['repositories']}
    # Correlated peer proof is observation only, not a substitute for stopped cgroup/runner proof.
    H.exchange(roots['--runtime-root'] / 'smith-next/app.sock', H.app_request(), pid)
    return {'roots': {k: str(v) for k, v in roots.items()}, 'root_ids': root_ids, 'pid': pid,
            'cmdline_sha256': sha((proc / 'cmdline').read_bytes()), 'proc_start': (proc / 'stat').read_text().rsplit(')', 1)[1].split()[19],
            'group': str(cgroup), 'group_identity': identity(cgroup.lstat()), 'unit': unit,
            'states': hashes, 'release': str(release), 'target_proof': target_proof}


def docker(args, timeout=10):
    config = PRIVATE / 'docker-config'
    safe_directory(config, private=True)
    require(not any(config.iterdir()), 'Docker config must remain empty')
    result = subprocess.run(['/usr/bin/docker', '--config', str(config), '--host', 'unix:///run/docker.sock', *args],
                            env=ENV, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            timeout=timeout)
    require(result.returncode == 0, 'Fixed local Docker operation failed; no retry')
    require(len(result.stdout) <= 16 * 1024 * 1024 and not result.stderr, 'Unexpected Docker output')
    return result.stdout


def image(name):
    values = decode(docker(['image', 'inspect', name]))
    require(len(values) == 1)
    return values[0]


def log_proof(data, expected=None):
    require(data and data.endswith(b'\n'))
    frames = [decode(line) for line in data.splitlines()]
    require(len(frames) == 5)
    require(all(f['request_id'] == REQUEST for f in frames))
    require([f['type'] for f in frames] == ['repository_request', 'repository_request', 'repositories', 'ready', 'failed'])
    require(frames[0]['advertisement'] is True and frames[1]['advertisement'] is False
            and frames[0]['sequence'] == 1 and frames[1]['sequence'] == 2)
    if expected is not None:
        require(frames[0]['repository'] == 0 and frames[1]['repository'] == 0
                and frames[2]['repositories'] == expected['repositories']
                and frames[3]['session_id'] == expected['session_id']
                and frames[3]['policy'] == 'smith-local-inspection-v1'
                and frames[3]['guard_sha256'] == '40504069bc2548e7d1da3c9a47cecda3ee1be77e2be3a60bbdaac737e25ef310'
                and frames[4]['kind'] == 'local_worker_failed')
    return frames


def containers(expected):
    ids = docker(['container', 'ls', '--all', '--no-trunc', '--filter', 'label=org.smith.dev.request=' + REQUEST, '--format', '{{.ID}}']).decode().splitlines()
    require(ids == [CONTAINER])
    values = decode(docker(['container', 'inspect', ids[0]]))
    require(len(values) == 1)
    c = values[0]
    labels, s = c['Config']['Labels'], c['State']
    require(c['Id'] == ids[0] and c['Name'] == '/smith-dev-' + REQUEST and c['Image'] == IMAGE)
    require(labels['org.smith.dev.request'] == REQUEST and labels['org.smith.dev.kind'] == 'smith-receipt-worker-v1'
            and labels['org.smith.dev.owner'] == OWNER)
    require(s['Status'] == 'exited' and s['Running'] is False and s['Pid'] == 0
            and s['Paused'] is False and s['Restarting'] is False and s['Dead'] is False
            and c['RestartCount'] == 0 and c['HostConfig']['RestartPolicy']['Name'] == 'no')
    logs = docker(['container', 'logs', ids[0]])
    log_proof(logs, expected)
    return {'inspect_sha256': sha(encoded(c)), 'id': ids[0], 'owner': labels['org.smith.dev.owner'],
            'logs_sha256': sha(logs)}


def recipe():
    return (f'FROM {BACKUP_TAG}\nUSER 0\n'
            'COPY --chown=0:0 smith-service /usr/local/bin/smith-service\n'
            'COPY --chown=0:0 guard.mjs /etc/smith-opencode/guard.mjs\n'
            'RUN chmod 0555 /usr/local/bin/smith-service && chmod 0444 /etc/smith-opencode/guard.mjs \\\n'
            f' && printf "{NEW}  /usr/local/bin/smith-service\\n{GUARD}  /etc/smith-opencode/guard.mjs\\n" | sha256sum --check --strict \\\n'
            ' && stat -c "%u:%g:%a" /usr/local/bin/smith-service | grep -Fx "0:0:555" \\\n'
            ' && stat -c "%u:%g:%a" /etc/smith-opencode/guard.mjs | grep -Fx "0:0:444" \\\n'
            ' && /usr/local/bin/smith-service --help >/dev/null\nUSER 1000:1000\n').encode()


def image_config(old, new):
    require(old['Config']['User'] == '1000:1000'
            and old['Config']['Entrypoint'] == ['/usr/local/bin/smith-service', 'dev-local-worker'])
    for key in ('User', 'WorkingDir', 'Env', 'Entrypoint', 'Cmd', 'Volumes', 'OnBuild', 'Shell'):
        require(new['Config'].get(key) == old['Config'].get(key), 'Image configuration drift')
    require(not new['Config'].get('Volumes') and not new['Config'].get('OnBuild') and not new['Config'].get('Shell'))


def phase(value, expected, **changes):
    require(value['phase'] == expected, 'Phase replay or mismatch')
    result = dict(value, **changes)
    require(result['invocation_id'] == value['invocation_id'])
    return result


def progress_change(value, expected, **changes):
    require(decode(read(PROGRESS)) == value, 'Progress ownership drift')
    updated = phase(value, expected, **changes)
    publish(PROGRESS, updated)
    return updated


PREPARE_PHASES = ('support', 'preconditions', 'source-check', 'capture', 'leases', 'capture-under-lease',
                  'docker-preflight', 'artifact-snapshot', 'build-context', 'image-preservation',
                  'offline-build', 'candidate-validation', 'final-binding')


def record_prepare_failure(phase_name, error):
    # Exception messages, keys, tracebacks and subprocess output may contain
    # prompts or roots. Persist only allowlisted phase/code values, privately.
    if isinstance(error, KeyError):
        code = 'state-field-missing'
    elif isinstance(error, Refused):
        code = 'binding-refused'
    elif isinstance(error, AssertionError):
        code = 'support-proof-refused'
    elif isinstance(error, subprocess.TimeoutExpired):
        code = 'command-timeout'
    elif isinstance(error, OSError):
        code = 'filesystem-error'
    elif isinstance(error, (KeyboardInterrupt, SystemExit)):
        code = 'interrupted'
    else:
        code = 'unexpected-error'
    try:
        require(phase_name in PREPARE_PHASES)
        safe_directory(PRIVATE, private=True)
        write(PRIVATE / ('prepare-refusal-' + uuid.uuid4().hex + '.json'), encoded({
            'mode': 'prepare', 'status': 'refused', 'phase': phase_name, 'code': code,
            'automatic_retry': False}))
    except BaseException:
        # Preserve the original refusal if diagnostics cannot be retained. Never
        # fall back to repository evidence, stderr details or permission repair.
        pass


def prepare(H):
    stage = {'phase': 'preconditions'}
    try:
        _prepare(H, stage)
    except BaseException as error:
        record_prepare_failure(stage['phase'], error)
        raise


def _prepare(H, stage):
    require(not any(os.path.lexists(p) for p in (PREP, PROGRESS, HOOK, PRIVATE / 'bindings.json')))
    stage['phase'] = 'source-check'
    sources = source_check()
    stage['phase'] = 'capture'
    before = capture(H)
    roots = {k: Path(v) for k, v in before['roots'].items()}
    stage['phase'] = 'leases'
    H.acquire_lock(roots['--data-root'] / '.smith-next-install.lock')
    H.acquire_lock(roots['--data-root'], directory=True)
    H.validate_locks()
    stage['phase'] = 'capture-under-lease'
    require(capture(H) == before)
    stage['phase'] = 'docker-preflight'
    (PRIVATE / 'docker-config').mkdir(mode=0o700)
    sync_dir(PRIVATE)
    container = containers(before['target_proof'])
    old_image = image(RUNTIME_TAG)
    require(old_image['Id'] == IMAGE)
    image_config(old_image, old_image)
    # Do not interpret daemon failure as tag absence.
    tags = docker(['image', 'ls', '--format', '{{.Repository}}:{{.Tag}}']).decode().splitlines()
    require(BACKUP_TAG not in tags and CANDIDATE_TAG not in tags)
    stage['phase'] = 'artifact-snapshot'
    artifacts = PRIVATE / 'artifacts-before'
    artifacts.mkdir(mode=0o700)
    namespace = roots['--data-root'] / 'smith-next'
    metadata = {}
    for name, path in {'service': namespace / 'bin/smith-service', 'unit': Path(before['unit']['unit']),
                       'manifest': Path(before['release']) / '.smith-next-manifest.json',
                       'helper': Path(before['release']) / 'smith_backend/smith-service'}.items():
        payload = read(path)
        metadata[name] = {'identity': identity(path.lstat()), 'sha256': sha(payload)}
        write(artifacts / name, payload)
    write(artifacts / 'metadata.json', encoded({'artifacts': metadata, 'unit': before['unit']}))
    stage['phase'] = 'build-context'
    context = PRIVATE / 'build-context'
    context.mkdir(mode=0o700)
    write(context / 'smith-service', read(BUNDLE / 'payload/service/smith-service'))
    write(context / 'guard.mjs', pinned_source(ROOT / 'service/dev-worker/guard.mjs', GUARD))
    write(context / 'Dockerfile', recipe())
    write(PRIVATE / 'prepare-attempt.json', encoded({'sources': sources, 'before': before, 'container': container, 'old_image': old_image}))
    stage['phase'] = 'image-preservation'
    docker(['image', 'tag', IMAGE, BACKUP_TAG])
    require(image(BACKUP_TAG)['Id'] == IMAGE)
    # Build stdout/stderr can contain local paths; keep both exclusively private.
    stage['phase'] = 'offline-build'
    with (PRIVATE / 'build.log').open('xb') as log:
        os.fchmod(log.fileno(), 0o600)
        result = subprocess.run(['/usr/bin/docker', '--config', str(PRIVATE / 'docker-config'), '--host', 'unix:///run/docker.sock',
                                 'build', '--pull=false', '--network=none', '--rm=false', '-t', CANDIDATE_TAG, str(context)],
                                env=ENV, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT, timeout=240)
        log.flush()
        os.fsync(log.fileno())
    require(result.returncode == 0, 'Offline build failed; no retry')
    stage['phase'] = 'candidate-validation'
    new_image = image(CANDIDATE_TAG)
    image_config(old_image, new_image)
    require(new_image['Id'] != IMAGE and image(RUNTIME_TAG)['Id'] == IMAGE and image(BACKUP_TAG)['Id'] == IMAGE)
    require(source_check() == sources and capture(H) == before and containers(before['target_proof']) == container)
    stage['phase'] = 'final-binding'
    binding = {'before': before, 'sources': sources, 'container': container, 'old_image': old_image,
               'new_image': new_image, 'recipe_sha256': sha(recipe())}
    write(PRIVATE / 'bindings.json', encoded(binding))
    write(PREP, encoded({'status': 'prepared', 'candidate_sha256': NEW, 'worker_image': new_image['Id'],
                         'bindings_sha256': sha(encoded(binding)), 'sources': sources, 'job_submitted': False}))


def load_binding():
    safe_directory(PRIVATE, private=True)
    require(stat.S_IMODE((PRIVATE / 'bindings.json').lstat().st_mode) == 0o600)
    binding = decode(read(PRIVATE / 'bindings.json'))
    prepared = decode(read(PREP))
    require(prepared['status'] == 'prepared' and prepared['bindings_sha256'] == sha(encoded(binding))
            and source_check() == binding['sources'])
    return binding


def lock_line_proof(text, pid, device, inode):
    matches = []
    for line in text.splitlines():
        if not line.startswith('lock:'):
            continue
        fields = line.split()
        if len(fields) == 9 and fields[2:5] == ['FLOCK', 'ADVISORY', 'WRITE']:
            parts = fields[6].split(':')
            if len(parts) == 3:
                matches.append((int(fields[5]), int(parts[0], 16), int(parts[1], 16), int(parts[2]), fields[7:]))
    return (pid, os.major(device), os.minor(device), inode, ['0', 'EOF']) in matches


def parent_proof(roots, progress, proc_root=Path('/proc')):
    pid = os.getppid()
    proc = proc_root / str(pid)
    require(progress['phase'] in ('driver-running', 'hook-running', 'image-publish-attempted', 'ledger-publish-attempted')
            and progress['driver_pid'] == pid)
    require(sha((proc / 'exe').read_bytes()) == DRIVER_SHA and (proc / 'exe').resolve() == DRIVER)
    require((proc / 'cmdline').read_bytes().split(b'\0') == [os.fsencode(DRIVER), b'--apply-approved-update', b''])
    expected = [roots['--data-root'] / '.smith-next-install.lock', roots['--data-root'],
                roots['--runtime-root'] / 'smith-next/runner.lock']
    facts = []
    for path in expected:
        m = path.lstat()
        require(m.st_uid == os.geteuid())
        if path == roots['--data-root']:
            require(stat.S_ISDIR(m.st_mode) and not m.st_mode & 0o022)
        else:
            require(stat.S_ISREG(m.st_mode) and m.st_nlink == 1 and stat.S_IMODE(m.st_mode) == 0o600)
        found = []
        # Enumeration/read errors are fatal, never interpreted as missing/empty.
        for fd in (proc / 'fd').iterdir():
            try:
                held = fd.stat()
            except FileNotFoundError:
                raise Refused('Parent descriptors changed during proof') from None
            if identity(held) == identity(m):
                info = (proc / 'fdinfo' / fd.name).read_text()
                if lock_line_proof(info, pid, m.st_dev, m.st_ino):
                    require(os.readlink(fd) == str(path) and path.lstat() == m)
                    found.append(fd.name)
        require(len(found) == 1, 'Missing exact parent-held flock')
        facts.append([str(path), identity(m), found[0]])
    return facts


def stopped_check(H, R, binding, progress, expected_image=IMAGE):
    before = binding['before']
    roots = {k: Path(v) for k, v in before['roots'].items()}
    require({str(p): list(v) for p, v in H.root_identities(roots).items()} == before['root_ids'])
    locks = parent_proof(roots, progress)
    require(unit_facts(H, roots) == before['unit'])
    group = Path(before['group'])
    try:
        group_meta = group.lstat()
    except FileNotFoundError:
        pass
    else:
        require(identity(group_meta) == before['group_identity'])
    R.stopped_proof(group, [roots['--runtime-root'] / 'smith-next' / n for n in ('service.sock', 'app.sock')])
    namespace = roots['--data-root'] / 'smith-next'
    pair(namespace, NEW, CLIENT)
    idle(namespace)
    require(state_hashes(namespace) == before['states'] and containers(before['target_proof']) == binding['container'])
    require(image(RUNTIME_TAG)['Id'] == expected_image and image(BACKUP_TAG)['Id'] == IMAGE
            and image(CANDIDATE_TAG)['Id'] == binding['new_image']['Id'])
    require(source_check() == binding['sources'])
    return roots, namespace, locks


def validate_readers(H, R, namespace, original, updated):
    # The old executable is preserved, never executed against schema3. The new
    # reader checks both variants. Fixed short private paths respect AF_UNIX limits.
    for index, (payload, status) in enumerate(((original, 'unknown'), (updated, 'reconciled'))):
        base = PRIVATE / ('v' + str(index))
        base.mkdir(mode=0o700)
        data, config, runtime = [base / n for n in ('d', 'c', 'r')]
        for p in (data, config, runtime, data / 'smith-next', data / 'smith-next/bin'):
            p.mkdir(mode=0o700)
        require(len(os.fsencode(runtime / 'smith-next/service.sock')) < 108)
        exe = data / 'smith-next/bin/smith-service'
        write(exe, read(BUNDLE / 'payload/service/smith-service'), 0o700)
        ns = data / 'smith-next'
        for name in STATES:
            if name == STATES[2]:
                write(ns / name, payload)
            elif os.path.lexists(namespace / name):
                write(ns / name, read(namespace / name))
        idle(ns)
        before = state_hashes(ns)
        child = subprocess.Popen([str(exe), 'serve', '--data-root', str(data), '--config-root', str(config),
                                  '--runtime-root', str(runtime)],
                                 env={'PATH': '/nonexistent', 'HOME': str(base), 'XDG_DATA_HOME': str(data),
                                      'XDG_CONFIG_HOME': str(config), 'XDG_RUNTIME_DIR': str(runtime)},
                                 stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                 start_new_session=False)  # Remain in the driver's owned hook group.
        try:
            H.wait_app(runtime, child)
            reply = H.exchange(runtime / 'smith-next/app.sock', H.app_request(), child.pid)
            found = [j for j in reply['result']['jobs'] if j['request_id'] == REQUEST]
            require(len(found) == 1 and found[0]['status'] == status)
            require(state_hashes(ns) == before, 'Isolated reader rewrote state')
        finally:
            if child.poll() is None:
                child.terminate()
            try:
                child.wait(timeout=2)
            except subprocess.TimeoutExpired:
                child.kill()  # Only this captured isolated fixture child, never a live PID.
                child.wait(timeout=1)
                raise Refused('Fixture child exceeded cleanup budget') from None


def stopped_hook(H, R):
    binding = load_binding()
    require(not os.path.lexists(HOOK))
    progress = decode(read(PROGRESS))
    roots, namespace, locks = stopped_check(H, R, binding, progress)
    progress = progress_change(progress, 'driver-running', phase='hook-running')
    snapshot = PRIVATE / 'before-transition'
    snapshot.mkdir(mode=0o700)
    for name, expected in binding['before']['states'].items():
        if expected is not None:
            data = read(namespace / name)
            require(sha(data) == expected)
            write(snapshot / name, data)
    write(snapshot / 'original-binding.json', encoded(binding))
    for p in (PRIVATE / 'artifacts-before').iterdir():
        write(snapshot / ('artifact-' + p.name), read(p))
    write(snapshot / 'locks.json', encoded(locks))
    sync_dir(PRIVATE)
    original = read(snapshot / STATES[2])
    updated = updated_ledger(original)
    write(snapshot / 'updated-ledger.json', updated)
    validate_readers(H, R, namespace, original, updated)
    require(stopped_check(H, R, binding, progress)[2] == locks)
    progress = progress_change(progress, 'hook-running', phase='image-publish-attempted')
    docker(['image', 'tag', binding['new_image']['Id'], RUNTIME_TAG])
    require(image(RUNTIME_TAG)['Id'] == binding['new_image']['Id'])
    require(stopped_check(H, R, binding, progress, expected_image=binding['new_image']['Id'])[2] == locks)
    progress = progress_change(progress, 'image-publish-attempted', phase='ledger-publish-attempted')
    require(state_hashes(namespace) == binding['before']['states'])
    staged = namespace / 'dev-local-jobs.json.coordinated-staging'
    write(staged, updated)
    require(parent_proof(roots, progress) == locks)
    require(read(namespace / STATES[2]) == original)
    os.replace(staged, namespace / STATES[2])
    sync_dir(namespace)
    require(read(namespace / STATES[2]) == updated)
    hashes = state_hashes(namespace)
    require(all(hashes[n] == binding['before']['states'][n] for n in STATES if n != STATES[2]))
    require(parent_proof(roots, progress) == locks)
    R.stopped_proof(Path(binding['before']['group']),
                    [roots['--runtime-root'] / 'smith-next' / n for n in ('service.sock', 'app.sock')])
    progress = progress_change(progress, 'ledger-publish-attempted', phase='reconciled')
    write(HOOK, encoded({'status': 'reconciled', 'request_id': REQUEST, 'candidate_sha256': NEW,
                         'original_ledger_sha256': sha(original), 'updated_ledger_sha256': sha(updated),
                         'snapshot_retained': True, 'worker_image': binding['new_image']['Id']}))


@contextlib.contextmanager
def cancellation_signals():
    cancelled = [False]
    def cancel(signum, frame):
        cancelled[0] = True
    previous = {sig: signal.signal(sig, cancel) for sig in (signal.SIGINT, signal.SIGTERM)}
    previous_mask = signal.pthread_sigmask(signal.SIG_UNBLOCK, set(previous))
    try:
        yield cancelled
    finally:
        signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)
        for sig, handler in previous.items():
            signal.signal(sig, handler)


def parent_death_guard(parent):
    # Linux-only child setup, before releasing the exec gate. The parent identity
    # recheck closes the death-before-prctl race. SIGTERM survives ordinary exec.
    libc = ctypes.CDLL(None, use_errno=True)
    require(libc.prctl(1, int(signal.SIGTERM), 0, 0, 0) == 0, 'Parent-death guard refused')
    require(os.getppid() == parent, 'Launching parent disappeared')


def cancel_join(pid, group_ready):
    # pid is our unreaped fork child; it cannot be recycled. Before the session
    # handshake only that child may be signalled, never the caller's group.
    while True:
        try:
            finished, status = os.waitpid(pid, os.WNOHANG)
            break
        except InterruptedError:
            continue
    if finished == pid:
        return status
    try:
        if group_ready:
            os.killpg(pid, signal.SIGTERM)
        else:
            os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        pass  # Exit raced cancellation; still join the exact child below.
    # Do not abandon the native operation on a second outer deadline, or kill it
    # before it reaps its separate hook group and unwinds its held leases. Only
    # the rebuilt driver's cooperative cancellation path may do that cleanup.
    while True:
        try:
            return os.waitpid(pid, 0)[1]
        except InterruptedError:
            continue


def wait_owned(pid, deadline, cancelled):
    while True:
        finished, status = os.waitpid(pid, os.WNOHANG)
        if finished == pid:
            return status
        require(not cancelled[0], 'Native operation cancelled; partial outcome retained')
        require(time.monotonic() < deadline, 'Native supervision deadline exceeded; cancelling and joining')
        time.sleep(0.05)


def supervise_driver(on_started, timeout=75):
    gate_read, gate_write = os.pipe()
    ready_read, ready_write = os.pipe()
    pid, status, group_ready = None, None, False
    cancelled_and_joined = False
    owned_fds = {gate_read, gate_write, ready_read, ready_write}
    def close_fd(fd):
        if fd in owned_fds:
            os.close(fd)
            owned_fds.remove(fd)
    parent = os.getpid()
    with cancellation_signals() as cancelled:
        try:
            pid = os.fork()
            if pid == 0:
                try:
                    os.close(gate_write)
                    os.close(ready_read)
                    for sig in (signal.SIGINT, signal.SIGTERM):
                        signal.signal(sig, signal.SIG_DFL)
                    parent_death_guard(parent)
                    signal.pthread_sigmask(signal.SIG_UNBLOCK, {signal.SIGINT, signal.SIGTERM})
                    os.setsid()
                    os.write(ready_write, b'1')
                    os.close(ready_write)
                    require(os.read(gate_read, 1) == b'1')
                    os.close(gate_read)
                    require(os.getppid() == parent)
                    null = os.open(os.devnull, os.O_RDONLY)
                    os.dup2(null, 0)
                    os.close(null)
                    log = os.open(PRIVATE / 'native-driver.log', os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
                    os.dup2(log, 1)
                    os.dup2(log, 2)
                    os.close(log)
                    allowed = ('HOME', 'XDG_DATA_HOME', 'XDG_CONFIG_HOME', 'XDG_RUNTIME_DIR', 'DBUS_SESSION_BUS_ADDRESS')
                    env = {k: os.environ[k] for k in allowed if k in os.environ}
                    env.update(PATH='/usr/bin:/bin', LANG='C')
                    os.execve(DRIVER, [str(DRIVER), '--apply-approved-update'], env)
                except BaseException:
                    os._exit(125)
            close_fd(gate_read)
            close_fd(ready_write)
            require(select.select([ready_read], [], [], 5)[0] and os.read(ready_read, 1) == b'1',
                    'Owned driver session handshake failed')
            require(os.getpgid(pid) == pid, 'Driver is not in its owned session')
            group_ready = True
            require(not cancelled[0], 'Cancelled before driver dispatch')
            on_started(pid)  # Durably bind PID before native effects become possible.
            require(not cancelled[0], 'Cancelled before driver dispatch')
            os.write(gate_write, b'1')
            close_fd(gate_write)
            status = wait_owned(pid, time.monotonic() + timeout, cancelled)
            require(not cancelled[0] and os.waitstatus_to_exitcode(status) == 0,
                    'Native operation incomplete; partial outcome retained; no retry')
        finally:
            # Every exceptional path after fork (including callback, timeout,
            # KeyboardInterrupt and signal cancellation) joins before returning.
            try:
                if pid is not None and pid > 0 and status is None:
                    status = cancel_join(pid, group_ready)
                    cancelled_and_joined = True
                if pid is not None and pid > 0:
                    write(PRIVATE / 'native-supervision.json', encoded({
                        'status': 'cancelled-joined' if cancelled_and_joined else 'joined',
                        'driver_pid': pid, 'wait_status': status, 'owned_session': group_ready,
                        'partial_effects_retained': True, 'rollback_attempted': False}))
            finally:
                for fd in list(owned_fds):
                    close_fd(fd)


def app_request(op, args):
    return {'schema_version': 3, 'service': 'smith-next', 'protocol': 'app',
            'call_id': uuid.uuid4().hex, 'op': op, 'args': args}


def inventory_delta(before, after, request_id, operation):
    require(len(before['jobs']) < 32 and len(after['jobs']) == len(before['jobs']) + 1)
    require(after['jobs'][:-1] == before['jobs'])
    job = after['jobs'][-1]
    op = job['operation']
    require(job['version'] == 2 and job['task'] == 'Models'
            and job['conversation_id'] is None and job['history_revision'] is None)
    require(op['request_id'] == request_id and op['kind'] == 'models' and op['status'] == 'succeeded'
            and op['phase'] == 'complete' and op['error'] is None
            and op['result'] == operation['result'] and str(op['config_revision']) == operation['config_revision']
                        and op['config_revision'] == before['config']['revision'])
    reverse = copy.deepcopy(after)
    reverse['jobs'].pop()
    require(reverse['revision'] == before['revision'] + 2)
    reverse['revision'] = before['revision']
    require(reverse == before, 'Unexpected app-state change during metadata inventory')


def metadata_inventory(H, roots, pid):
    namespace = roots['--data-root'] / 'smith-next'
    socket = roots['--runtime-root'] / 'smith-next/app.sock'
    idle(namespace)
    baseline = state_hashes(namespace)
    before_bytes = read(namespace / STATES[0])
    require(sha(before_bytes) == baseline[STATES[0]], 'App state changed before metadata snapshot')
    before = decode(before_bytes)
    require(len(before['jobs']) < 32, 'Inventory would evict retained app metadata')
    observed = H.exchange(socket, app_request('state', {}), pid)
    dev = H.exchange(socket, H.app_request(), pid)
    require(dev['instance_id'] == observed['instance_id'])
    selected = {k: dev['result']['model'][k] for k in ('name', 'sha256', 'config_revision')}
    require(selected['name'] and selected['sha256'], 'Selected model identity unavailable')
    request_id = uuid.uuid4().hex
    require(request_id not in before.get('expired_ids', [])
            and all(j['operation']['request_id'] != request_id for j in before['jobs']))
    receipt = PRIVATE / 'metadata-inventory.json'
    write(PRIVATE / 'metadata-app-before.json', before_bytes)
    identity_receipt = {'request_id': request_id, 'instance_id': observed['instance_id'], 'pid': pid,
                        'status': 'dispatch-attempted', 'app_before_sha256': sha(before_bytes)}
    # Persist original identity BEFORE the single models call. Any ambiguity is
    # observation-only: this ID must be polled, never resent or replaced.
    write(receipt, encoded(identity_receipt))
    require(state_hashes(namespace) == baseline, 'State changed before metadata dispatch')
    accepted = H.exchange(socket, app_request('models', {'request_id': request_id}), pid)
    require(accepted['instance_id'] == observed['instance_id'] and accepted['result']['request_id'] == request_id
            and accepted['result']['kind'] == 'models')
    deadline = time.monotonic() + 12
    while True:
        require(time.monotonic() < deadline, 'Metadata poll deadline; retain original request; do not resend')
        reply = H.exchange(socket, app_request('poll', {'request_id': request_id, 'instance_id': observed['instance_id']}), pid)
        require(reply['instance_id'] == observed['instance_id'] and reply['result']['request_id'] == request_id
                and reply['result']['kind'] == 'models')
        operation = reply['result']
        if operation['status'] not in ('queued', 'running'):
            break
        time.sleep(0.05)
    require(operation['status'] == 'succeeded' and operation['error'] is None)
    after_state = H.exchange(socket, app_request('state', {}), pid)
    after_dev = H.exchange(socket, H.app_request(), pid)
    require(after_state['instance_id'] == observed['instance_id'] == after_dev['instance_id']
            and after_state['result']['config'] == observed['result']['config'])
    require(all(after_dev['result']['model'][k] == v for k, v in selected.items())
            and after_dev['result']['model']['ready'] is True, 'Selected model readiness not established')
    after_bytes = read(namespace / STATES[0])
    inventory_delta(before, decode(after_bytes), request_id, operation)
    require(all(state_hashes(namespace)[n] == baseline[n] for n in STATES if n != STATES[0]))
    result = dict(identity_receipt, status='succeeded', app_after_sha256=sha(after_bytes),
                  app_delta='one-models-receipt-and-two-revisions', model_ready=True,
                  inference_run=False, model_downloaded=False, config_changed=False)
    publish(receipt, result)
    return result


def apply(H, inventory=False):
    binding = load_binding()
    home = Path.home()
    environment_roots = {'--data-root': os.environ.get('XDG_DATA_HOME', str(home / '.local/share')),
                         '--config-root': os.environ.get('XDG_CONFIG_HOME', str(home / '.config')),
                         '--runtime-root': os.environ.get('XDG_RUNTIME_DIR')}
    require(environment_roots == binding['before']['roots'], 'Native environment roots differ from captured service')
    require(not os.path.lexists(HOOK) and not os.path.lexists(PROGRESS))
    require(capture(H) == binding['before'] and containers(binding['before']['target_proof']) == binding['container'])
    require(image(RUNTIME_TAG)['Id'] == IMAGE and image(BACKUP_TAG)['Id'] == IMAGE
            and image(CANDIDATE_TAG)['Id'] == binding['new_image']['Id'])
    progress = {'invocation_id': uuid.uuid4().hex, 'phase': 'launching', 'driver_pid': None}
    write(PROGRESS, encoded(progress))
    def started(pid):
        progress_change(progress, 'launching', phase='driver-running', driver_pid=pid)
    supervise_driver(started)
    hook = decode(read(HOOK))
    require(hook['status'] == 'reconciled' and hook['request_id'] == REQUEST and hook['candidate_sha256'] == NEW)
    before = binding['before']
    roots = {k: Path(v) for k, v in before['roots'].items()}
    namespace = roots['--data-root'] / 'smith-next'
    pair(namespace, NEW, CLIENT)
    require(unit_facts(H, roots) == before['unit']
            and {str(p): list(v) for p, v in H.root_identities(roots).items()} == before['root_ids'])
    require(H.property_value('ActiveState') == 'active' and H.property_value('SubState') == 'running')
    new_pid = int(H.property_value('MainPID'))
    require(new_pid > 0 and new_pid != before['pid'] and sha((Path('/proc') / str(new_pid) / 'exe').read_bytes()) == NEW)
    require(sha((Path('/proc') / str(new_pid) / 'cmdline').read_bytes()) == before['cmdline_sha256'])
    H.exchange(roots['--runtime-root'] / 'smith-next/service.sock',
               {'schema_version': 1, 'service': 'smith-next', 'command': 'get_state'}, new_pid)
    reply = H.exchange(roots['--runtime-root'] / 'smith-next/app.sock', H.app_request(), new_pid)
    require(len([j for j in reply['result']['jobs'] if j['request_id'] == REQUEST and j['status'] == 'reconciled']) == 1)
    original = decode(read(PRIVATE / 'before-transition' / STATES[2]))
    assert_record_diff(original, decode(read(namespace / STATES[2])))
    require(sha(read(namespace / STATES[2])) == hook['updated_ledger_sha256'])
    require(all(state_hashes(namespace)[n] == before['states'][n] for n in STATES if n != STATES[2]))
    require(image(RUNTIME_TAG)['Id'] == binding['new_image']['Id'] and containers(before['target_proof']) == binding['container'])
    require(source_check() == binding['sources'])
    inventory_receipt = metadata_inventory(H, roots, new_pid) if inventory else None
    write(EVIDENCE / 'coordinated-result.json', encoded({'status': 'installed-reconciled-core-app-ready',
          'candidate_sha256': NEW, 'client_manifest_sha256': CLIENT, 'worker_image': binding['new_image']['Id'],
          'request_id': REQUEST, 'dev_job_submitted': False, 'inventory_run': inventory,
          'model_readiness_verified': inventory_receipt is not None,
          'metadata_request_id': inventory_receipt['request_id'] if inventory_receipt else None,
          'app_state_bytes_unchanged': not inventory,
          'app_delta': inventory_receipt['app_delta'] if inventory_receipt else None,
          'other_state_bytes_unchanged': True, 'unchanged_state_files': ['dev-jobs.json', 'state.json'],
                    'snapshot_retained': True}))


def parse_mode(args):
    require(bool(args) and args[0] in ('prepare', 'stopped-hook', 'apply'), 'Explicit mode required')
    require(len(args) == 1 or (args == ['apply', '--inventory-models']), 'Unapproved option')
    return args[0], len(args) == 2


def main():
    require(not sys.flags.optimize, 'Optimized Python would disable audited helper assertions')
    mode, inventory = parse_mode(sys.argv[1:])
    require(os.geteuid() != 0)
    driver_pin()
    os.umask(0o077)
    safe_directory(PRIVATE, private=True)  # Parent-created only: never create/repair this root.
    try:
        H, R = support()
    except BaseException as error:
        if mode == 'prepare':
            record_prepare_failure('support', error)
        raise
    try:
        if mode == 'prepare':
            prepare(H)
        elif mode == 'stopped-hook':
            stopped_hook(H, R)
        else:
            apply(H, inventory=inventory)
    finally:
        for fd, _, _ in H.HELD_LOCKS:
            os.close(fd)
    print('Coordinated phase complete; no job submitted.')


if __name__ == '__main__':
    try:
        main()
    except BaseException:
        # Never print arbitrary exception text, subprocess output, roots or prompt data.
        print('Coordinated maintenance incomplete; no automatic retry, Start or rollback. Inspect private evidence.', file=sys.stderr)
        sys.exit(1)
