"""Short-runtime v4 review candidate; executed v1-v3 artifacts remain immutable.

Import and --plan are offline (no Docker, service, model copy/load or writes).
--launch requires two real independent reviews and fresh explicit operator consent.
It returns after detaching a persistent supervisor; NEVER timeout-kill that owner.
No retries, restart, deployment, retag, reconciliation, deletion or cloud operation.
Uncertain writes terminate forward progress; all state remains private for diagnosis.
"""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import select
import signal
import socket
import stat
import struct
import subprocess
import sys
import threading
import time
import types

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
DIAGNOSTIC_SHA = '841006a40b40d51399a4bff5ab0257e0f6e6a6daa26060cd831c9a7421c9b761'
BASELINE_SHA = 'e05dc1eb9019090a6c76c4f2c11599fbc325463f567802d8de25c88c96e04529'
WORKER_PROOF_SHA = '21f04bb53b87b8e840b690f1206b6fa4b0439bb63eeda4030f7059a9d3137722'
HISTORICAL_LOG_PROOF_SHA = '17473fcf08f0d5fb1733600a814a2e284335121fe8911e5697a0671c56d40e01'
RETAINED_CONTAINERS_SHA = 'a1d84425437ffde6e39272a28773efaa30621622862ffd836bd3eba104067afb'
NATIVE_PROOF_SHA = '7e4bbeec49dc8cc5e7c10264ed9332fe3c51a293e34d07c6228ac9a0c46076c2'
NATIVE_DRIVER_SHA = '17ce634db2a732e84036f5ed91b4190fd8a91ff4bdfb69075749fc6fe4bb003a'
CANDIDATE_SHA = '2ce2b26842f4cfeeccd29b084dbaf2e2fc4b254ed3b4825e1782f9dcbf0d4204'
SOURCE_SNAPSHOT = 'e612499c356f2654e3d035aca40aace3b74b3efaba8315901271af421d0f1876'
DIAGNOSTIC_CHECKS_SHA = '7474a862cfbf22d09e54f7a0fbc89e43b43de2f083ec883fd8006db95c5f3dc7'
CALLER_CHECKS_SHA = '6dd6327f303ae98763d775811f2a45343e7199e3766380f635ab36bb68955dea'
CHECKS_SHA = '9df41ea76528b3f43447f6d9b47f76a5ba0a4ec1f089198cd5893f4a444456de'
RUNNER_SHA = '79968bd6fe82fc8332c198f873dab346e12f10a7c42be0e14f270785257df367'
IMAGE = 'sha256:fae115562506916e3b1d1d65743476d7a26350b0723720884a4ee1e8f3325e82'
TAG = 'smith-dev-worker-local:1.19.3'
MODEL = 'qwen2.5-1.5b-instruct:q4_k_m'
MODEL_SHA = '6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e'
MODEL_SIZE = 1117320736
URL = 'https://github.com/octocat/Hello-World'
PROMPT = ('Read /workspace/repos/r0/README once. Answer in at most 20 words what it says. '
          'If it does not identify a programming language, say unknown. Do not run commands.')
WORKER_FILES = tuple('service/src/' + name + '.rs' for name in (
    'main', 'dev_worker_local', 'dev_worker', 'dev_local', 'dev_fetch', 'dev_read',
    'dev_completion', 'cloud', 'filesystem', 'lib', 'transport')) + (
    'service/Cargo.toml', 'service/Cargo.lock', 'service/dev-worker/guard.mjs')
FRAME_LIMIT = 4 * 1024 * 1024
BUDGET = 420  # Includes service startup and public clone; production inner limits unchanged.


def require(ok, code='binding-refused'):
    if not ok:
        raise RuntimeError(code)


class AppRefusal(RuntimeError):
    def __init__(self, operation, error):
        super().__init__('app-operation-refused')
        self.operation = operation if isinstance(operation, str) and operation in APP_OPERATIONS else 'unknown'
        code = error.get('code') if isinstance(error, dict) else None
        self.code = code if isinstance(code, str) and code in APP_ERROR_CODES else 'UNRECOGNIZED_APP_ERROR'


def refusal_fields(error):
    if isinstance(error, AppRefusal):
        return {'failed_operation': error.operation, 'error_code': error.code}
    return {'error_code': 'HARNESS_REFUSED_OR_INTERRUPTED'}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def load_support():
    path = HERE / 'native-tool-schema-diagnostic.py'
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        data = stream.read(131073)
    require(len(data) <= 131072 and sha(data) == DIAGNOSTIC_SHA)
    module = types.ModuleType('qa_pinned_diagnostic_definitions')
    module.__file__ = str(path)
    exec(compile(data, '<pinned-diagnostic-definitions>', 'exec'), module.__dict__)
    return module


D = load_support()
C, M = D.C, D.M
PRIVATE = M.PRIVATE
CANDIDATE = ROOT / '.smith-inspection-diag-v3/candidate/smith-service'
TESTS = HERE / 'test_pipeline_qa_v4.py'
SOCKET_LIMIT = 107
NATIVE_SOCKET_SUFFIX_BYTES = len(b'/runner-') + 32 + len(b'/llama.sock')
PREVIOUS_RUN_ID = '438264a28b4248ecbbfe88405172b06e'
PREVIOUS_RESULT_SHA = '7c88af6dac91d46f49316a64d282791801f2f1ee9207ec64e17c0119678bae1a'
PREVIOUS_DIAGNOSIS_SHA = '3086f1fa45a1319113e0edb4a89013226690f73be54cfa66f8d7636a5b9a7cd6'
DIAGNOSTIC_PHASES = frozenset(('broker_request', 'broker_validate_request', 'broker_response',
    'broker_validate_response', 'broker_read_evidence', 'provider_validate_request',
    'provider_frozen_preflight', 'runner_validate_request', 'props', 'semantic_probe_render',
    'semantic_probe_verify', 'actual_template', 'tokenize', 'context', 'native_completion',
    'validate_response', 'limit'))
DIAGNOSTIC_CODES = frozenset(('START', 'OK', 'OTHER', 'LIMIT', 'PROVIDER_PROTOCOL', 'LLM_PROTOCOL',
    'LLM_UNAVAILABLE', 'MODEL_UNAVAILABLE', 'LOCAL_MODEL_UNAVAILABLE', 'RUNNER_BUSY', 'RUNNER_UNSAFE',
    'RUNNER_CANCELLED', 'PROVIDER_TIMEOUT', 'PROVIDER_HTTP', 'PROVIDER_BODY_LIMIT', 'RUNTIME_UNSAFE',
    'MODEL_STORE_UNSAFE', 'UNSUPPORTED', 'RUNTIME_MISSING', 'CONTEXT_LIMIT', 'MODEL_TRUNCATED',
    'MODEL_RESULT_LIMIT', 'GPU_PREPARATION', 'INTERRUPTED', 'LOCAL_LIMIT', 'MODEL_PROTOCOL',
    'MODEL_LIMIT', 'READ_DENIED', 'REPOSITORY_REJECTED', 'REPOSITORY_UNAVAILABLE',
    'RUNTIME_UNAVAILABLE', 'PUBLICATION', 'PROTOCOL', 'SHUTDOWN', 'DEADLINE', 'PREREQUISITE'))
APP_ERROR_CODES = frozenset({
    'INVALID_REQUEST', 'STALE_REVISION', 'INSTANCE_MISMATCH', 'MODEL_UNAVAILABLE',
    'BUSY', 'STATE_UNAVAILABLE', 'UPGRADE_REQUIRED', 'REQUEST_UNKNOWN',
    'REQUEST_EXPIRED', 'UNKNOWN_REQUEST', 'LOCAL_PROTOCOL', 'PROVIDER_PROTOCOL',
})
APP_OPERATIONS = frozenset({
    'state', 'models', 'poll', 'configure_checked', 'projects_list_v3',
    'project_create_v3', 'project_receipt', 'dev_jobs_state_v2',
    'dev_job_start_v2', 'dev_job_get_v2',
})


def run_id(value):
    require(isinstance(value, str) and re.fullmatch('[0-9a-f]{32}', value), 'invalid-run-id')
    return value


def ids(value):
    run_id(value)
    return {kind: sha(('isolated-pipeline:' + value + ':' + kind).encode())[:32]
            for kind in ('models', 'project', 'job')}


def specification(value):
    return {'harness_revision': 4, 'run_id': run_id(value), 'ids': ids(value), 'candidate_sha256': CANDIDATE_SHA,
            'source_snapshot': SOURCE_SNAPSHOT, 'baseline_sha256': BASELINE_SHA,
            'previous_run_id': PREVIOUS_RUN_ID, 'previous_result_sha256': PREVIOUS_RESULT_SHA,
                        'runtime_leaf_policy': 'qa-sha256-full-run-id-16hex-with-exclusive-owner-journal',
                        'native_socket_limit': SOCKET_LIMIT,
                        'retained_containers_sha256': RETAINED_CONTAINERS_SHA,
                        'native_validation_sha256': NATIVE_PROOF_SHA,
                        'native_validation_policy': 'positive-startup-semantic-context-and-preservation-required-before-launch',
            'worker_proof_sha256': WORKER_PROOF_SHA, 'worker_image': IMAGE,
            'worker_policy': 'base-worker-equivalence-with-host-only-diagnostic-delta',
                        'candidate_features': ['inspection-diagnostics'],
                        'diagnostic_checks_sha256': DIAGNOSTIC_CHECKS_SHA,
            'historical_log_proof_sha256': HISTORICAL_LOG_PROOF_SHA,
            'historical_log_policy': 'exact-pre-existing-error-equivalence-only',
            'model_sha256': MODEL_SHA, 'repository': URL, 'prompt_sha256': sha(PROMPT.encode()),
            'wall_seconds': BUDGET, 'runs': 1, 'model_requests': 'production-bounded-one-job',
            'read_policy': 'production-read-only', 'installed_writes': False,
            'timestamp_gate': 'NO-GO-unwaived'}


def candidate_bytes():
    # Cargo's output is a builder input (may be hardlinked to deps and group
    # writable), not a recipient artifact. Capture/recheck the FD and exact hash;
    # publish a distinct single-link 0755 private copy, never chmod the source.
    fd = os.open(CANDIDATE, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        before = os.fstat(stream.fileno())
        require(stat.S_ISREG(before.st_mode) and before.st_uid == os.geteuid()
                and before.st_size <= 128 * 1024 * 1024, 'candidate-source-refused')
        data = stream.read(128 * 1024 * 1024 + 1)
        require(M.file_identity(before) == M.file_identity(os.fstat(stream.fileno()))
                == M.file_identity(CANDIDATE.lstat()) and len(data) == before.st_size
                and sha(data) == CANDIDATE_SHA, 'candidate-drift')
    return data


def source_snapshot():
    # Stable review binding includes both workspaces and their Rust tests; the
    # executable remains separately byte-bound, not inferred from this manifest.
    paths = {ROOT / area / leaf for area in ('service', 'installer') for leaf in ('Cargo.toml', 'Cargo.lock')}
    for area in ('service', 'installer'):
        paths.update((ROOT / area / 'src').rglob('*.rs'))
        paths.update((ROOT / area / 'tests').rglob('*.rs'))
    paths.add(ROOT / 'service/dev-worker/guard.mjs')
    inputs = {str(path.relative_to(ROOT)): sha(M.read(path, source=True)) for path in sorted(paths)}
    return sha(M.encoded(inputs)), inputs


def static_check():
    raw = M.read(PRIVATE / 'pipeline-qa-baseline.json')
    require(sha(raw) == BASELINE_SHA, 'baseline-drift')
    before = M.decode(raw)
    previous = PRIVATE / ('pipeline-qa-' + PREVIOUS_RUN_ID)
    require(sha(M.read(previous / 'result.json')) == PREVIOUS_RESULT_SHA
            and sha(M.read(previous / 'diagnosis.json')) == PREVIOUS_DIAGNOSIS_SHA,
            'first-attempt-evidence-drift')
    require(sha(M.read(HERE / 'pipeline-qa.py', source=True)) ==
            'cb9dda2880c41e292d0fcf64365356860f9bdfe77887c7442fffb697451505e1'
            and sha(M.read(HERE / 'test_pipeline_qa.py', source=True)) ==
            '1e8bb8a7bcea7d2596df5c9566a87b76c674a6ae545ecb0d9b4cd6adf6d76a1a',
            'executed-v1-artifact-drift')
    initial = PRIVATE / 'pipeline-qa-dc7b0308a0ea44cab7ffaea004e50a69'
    require(sha(M.read(initial / 'result.json')) ==
            'efbaed8135d1d65d17e2e29ceb38c46bd430233b26feb75b2edf61763de8bfcb'
            and sha(M.read(initial / 'diagnosis.json')) ==
            '6d34d074ea946d875843c03709d56c11bb171aa88200bbc838d1ef1c852aa616',
            'initial-attempt-evidence-drift')
    require(sha(M.read(HERE / 'pipeline-qa-v2.py', source=True)) ==
            '2faba82285ec56af02dd53e63b6631996d02211fd612104acedf0280569a0db9'
            and sha(M.read(HERE / 'test_pipeline_qa_v2.py', source=True)) ==
            'a32b5afb1d70cd1d6966af6afc22a8bc06e0b48d4542e4c6646e86f1d3c0eb8f',
            'executed-v2-artifact-drift')
    v3 = PRIVATE / 'pipeline-qa-159361a5dbe74646a23f2878d51de89a'
    require(sha(M.read(v3 / 'result.json')) ==
            '60b0814415f72adfe5d02b6c783a1712c96ad44801f7e398995941b48db1ebc2'
            and sha(M.read(v3 / 'diagnosis.json')) ==
            '4d263458f974958219c7a16bebb9f6a40ad196fe9ccc176c0984e9519446925a',
            'v3-evidence-drift')
    require(sha(M.read(HERE / 'pipeline-qa-v3.py', source=True)) ==
            '682cfb5b2c883dc43514bbd499c5afc2b42925e6a6a58c936989c2e0afe7ad60'
            and sha(M.read(HERE / 'test_pipeline_qa_v3.py', source=True)) ==
            '7b0eac1e0a75252eb0b5495e2e7b8894e527f5710396379268b4ce1e9a9c8316',
            'executed-v3-artifact-drift')
    historical_log_binding()
    require(sha(M.read(PRIVATE / 'pipeline-qa-v4-retained-containers.json')) == RETAINED_CONTAINERS_SHA,
            'retained-container-proof-drift')
    require(source_snapshot()[0] == SOURCE_SNAPSHOT, 'source-snapshot-drift')
    candidate_bytes()
    require(sha(M.read(ROOT / 'service/src/runner.rs', source=True)) == RUNNER_SHA, 'runner-drift')
    require(sha(M.read(HERE / 'native-tool-fix-final-checks.log', source=True)) == CHECKS_SHA)
    require(sha(M.read(HERE / 'native-tool-fix-caller-checks.log', source=True)) == CALLER_CHECKS_SHA)
    require(sha(M.read(HERE / 'inspection-diag-v3-checks.log', source=True)) == DIAGNOSTIC_CHECKS_SHA)
    proof = M.read(HERE / 'coordinated-candidate-v2-aligned-result.json', source=True)
    require(sha(proof) == WORKER_PROOF_SHA, 'worker-proof-drift')
    old = M.decode(proof)['source_sha256']
    for name in WORKER_FILES:
        current = M.read(ROOT / name, source=True)
        if name in ('service/src/lib.rs', 'service/Cargo.toml'):
            archived = M.read(ROOT / '.smith-inspection-diag-v3/before' / Path(name).name, source=True)
            require(sha(archived) == old[name], 'worker-base-source-drift')
            addition = (b'mod inspection_diagnostics;\n' if name.endswith('lib.rs') else
                        b'[features]\ndefault = []\ninspection-diagnostics = []\n\n')
            require(current.count(addition) == 1 and current.replace(addition, b'', 1) == archived,
                    'worker-diagnostic-delta-drift')
        else:
            require(sha(current) == old[name], 'worker-source-drift')
    require(before['service_sha256'] == D.SERVICE_SHA and before['image_id'] == IMAGE
            and before['model_marker']['sha256'] == MODEL_SHA)
    return before


def environment():
    # No Docker context, model/provider keys, Git credentials or proxy inheritance.
    return {**{k: os.environ[k] for k in ('HOME', 'XDG_DATA_HOME', 'XDG_CONFIG_HOME',
             'XDG_RUNTIME_DIR', 'DBUS_SESSION_BUS_ADDRESS') if k in os.environ},
            'PATH': '/usr/bin:/bin', 'LANG': 'C', 'LC_ALL': 'C'}


def command(argv, limit=8 * 1024 * 1024, merge_stderr=False):
    return C.owned_command(argv, environment(), timeout=15, limit=limit, merge_stderr=merge_stderr)


def prop(name):
    return command(['/usr/bin/systemctl', '--user', 'show', 'smith-next.service',
                    '--property=' + name, '--value'], 16384).decode().strip()


def docker(*args):
    return command(['/usr/bin/docker', '--config', str(PRIVATE / 'docker-config'),
                    '--host', 'unix:///run/docker.sock', *args], merge_stderr=args[0] == 'logs')


def process_stat(pid):
    # /proc comm may contain spaces and parentheses.
    parts = (Path('/proc') / str(pid) / 'stat').read_text().rsplit(')', 1)[1].split()
    return {'state': parts[0], 'pgrp': int(parts[2]), 'sid': int(parts[3]), 'start': parts[19]}


def live_check(before):
    roots = {k: Path(v) for k, v in before['roots'].items()}
    H, _ = M.support()
    require({str(path): list(identity) for path, identity in H.root_identities(roots).items()}
                == before['root_ids'], 'root-drift')
    C.environment({'before': {'roots': before['roots']}})
    require(int(prop('MainPID')) == before['pid'] and prop('ActiveState') == before['active'] == 'active'
            and prop('SubState') == before['substate'] == 'running'
            and prop('UnitFileState') == before['enabled'], 'managed-service-drift')
    proc = Path('/proc') / str(before['pid'])
    require(process_stat(before['pid'])['start'] == before['process_start']
            and sha((proc / 'exe').read_bytes()) == D.SERVICE_SHA
            and sha((proc / 'cmdline').read_bytes()) == before['cmdline_sha256'])
    group = prop('ControlGroup')
    require(group.startswith('/') and group != '/' and '..' not in Path(group).parts)
    group_path = Path('/sys/fs/cgroup') / group.lstrip('/')
    require(str(group_path) == before['cgroup'], 'cgroup-drift')
    require(D.group_quiet(group_path) == {before['pid']},
            'installed-not-quiescent')
    namespace = roots['--data-root'] / 'smith-next'
    D.idle(namespace)
    require(D.hashes(namespace) == before['states'], 'installed-state-drift')
    require(os.readlink(namespace / 'client/current') == before['selector'])
    release, _ = D.manifest_check(namespace)
    require(M.decode(M.read(namespace / 'models' / (MODEL_SHA + '.json'), 4096)) == before['model_marker'])
    unit = roots['--config-root'] / 'systemd/user/smith-next.service'
    enabled = unit.parent / 'default.target.wants/smith-next.service'
    require(os.readlink(enabled) == before['unit_link']
            and M.identity(enabled.lstat()) == before['unit_link_identity']
            and not unit.is_symlink() and sha(M.read(unit)) == before['unit_sha256'], 'unit-drift')
    require(prop('DropInPaths') == '' and prop('Transient') == 'no'
            and prop('FragmentPath') == str(unit), 'unit-overrides-refused')
    C.effective_start(prop('ExecStart'), C.arguments(roots))
    inspected = docker('image', 'inspect', TAG)
    info = M.decode(inspected)
    require(len(info) == 1 and sha(M.encoded(info[0])) == before['image_inspect_sha256']
            and info[0]['Id'] == IMAGE, 'image-drift')
    return roots, namespace, release


def historical_log_binding():
    raw = M.read(PRIVATE / 'pipeline-qa-historical-log-diagnostic.json')
    require(sha(raw) == HISTORICAL_LOG_PROOF_SHA, 'historical-log-proof-drift')
    proof = M.decode(raw)
    require(proof['baseline_sha256'] == BASELINE_SHA
            and proof['request_id'] == '9a8d5ecb22fa4b85629e69f7a1e624b1'
            and proof['classification'] == 'pre-existing-local-driver-option-error-not-deletion'
            and proof['repeat_exact'] is True, 'historical-log-proof-refused')
    return proof


def retained_containers(query=docker, exclude_request=None, cleanup=False, observe_logs=None):
    listing = query('ps', '-aq', '--no-trunc', '--filter', 'label=org.smith.dev.request').decode().split()
    require(len(listing) <= 128 and all(re.fullmatch('[0-9a-f]{64}', v) for v in listing))
    proof = {}
    historical = historical_log_binding()
    observe = observe_logs or (lambda identifier: docker_query('logs', identifier, cleanup=cleanup, observe=True))
    for identifier in sorted(listing):
        inspected = query('inspect', identifier)
        info = M.decode(inspected)[0]
        if exclude_request is not None and info['Config']['Labels'].get('org.smith.dev.request') == exclude_request:
            continue
        require(info['Id'] == identifier and info['State']['Running'] is False,
                'retained-container-not-stopped')
        observation = observe(identifier)
        if identifier == historical['container_id']:
            require(info['Config']['Labels']['org.smith.dev.request'] == historical['request_id']
                    and sha(inspected) == historical['inspect_sha256']
                    and observation == historical['log_observation'], 'historical-log-observation-drift')
        else:
            require(observation['exit_code'] == 0, 'retained-log-new-failure')
        proof[identifier] = {'inspect_sha256': sha(inspected), 'log_observation': observation}
    require(historical['container_id'] in proof, 'historical-container-missing')
    return proof


def native_validation():
    # A review receipt cannot waive an unsuccessful prerequisite. This snapshot is
    # deliberately blocked until a separately reviewed correction/rebinding.
    raw = M.read(PRIVATE / 'inspection-diag-v4/native-proof.json')
    require(sha(raw) == NATIVE_PROOF_SHA
            and sha(M.read(HERE / 'native-v4-nogeneration.py', source=True)) == NATIVE_DRIVER_SHA,
            'native-validation-binding-drift')
    proof = M.decode(raw)
    require(proof.get('status') == 'pass' and proof.get('generation_requests') == 0
            and proof.get('source_snapshot') == SOURCE_SNAPSHOT
            and proof.get('candidate_sha256') == CANDIDATE_SHA
            and proof.get('retained_containers_sha256') == RETAINED_CONTAINERS_SHA
            and proof.get('retained_container_count') == 9
            and all(proof.get(k) is True for k in ('startup_verified', 'semantic_probe_verified',
                'actual_context_verified', 'kernel_socket_absent', 'native_cleanup_verified',
                'stderr_capture_ok', 'installed_states_exact', 'reused_profile_states_exact',
                'retained_containers_exact')), 'native-validation-not-pass')


def review(value, before):
    spec = specification(value)
    digest = sha(M.encoded(spec))
    source = M.decode(M.read(PRIVATE / 'pipeline-qa-v4-source-review.json', 16384))
    harness = M.decode(M.read(PRIVATE / 'pipeline-qa-v4-harness-review.json', 16384))
    for receipt in (source, harness):
        require(receipt.get('verdict') == 'pass' and receipt.get('independent_review') is True
                and isinstance(receipt.get('reviewer'), str) and bool(receipt['reviewer'])
                and type(receipt.get('expires_unix')) in (int, float)
                and time.time() < receipt['expires_unix'] <= time.time() + 86400,
                'review-missing-stale-or-refused')
    require(source.get('source_snapshot') == SOURCE_SNAPSHOT
            and source.get('candidate_sha256') == CANDIDATE_SHA, 'source-review-drift')
    require(harness.get('script_sha256') == sha(M.read(Path(__file__), source=True))
            and harness.get('tests_sha256') == sha(M.read(TESTS, source=True))
            and harness.get('specification_sha256') == digest
            and harness.get('baseline_sha256') == BASELINE_SHA
            and harness.get('retained_containers_sha256') == RETAINED_CONTAINERS_SHA
            and harness.get('native_validation_sha256') == NATIVE_PROOF_SHA
            and harness.get('worker_source_equivalence_accepted') is True
            and harness.get('historical_log_unavailability_accepted') is True,
            'harness-review-drift')
    native_validation()
    require(before['states'] == static_check()['states'])
    return {'source_review_sha256': sha(M.encoded(source)),
            'harness_review_sha256': sha(M.encoded(harness)), 'specification_sha256': digest,
            'retained_containers_sha256': harness['retained_containers_sha256']}


def session_members(sid):
    """Unknown enumeration is not absence. Only vanished proc entries are benign."""
    members = {}
    for entry in Path('/proc').iterdir():
        if not entry.name.isdecimal():
            continue
        try:
            facts = process_stat(int(entry.name))
        except FileNotFoundError:
            continue
        if facts['sid'] == sid and facts['state'] not in ('Z', 'X'):
            members[int(entry.name)] = facts
    return members


def end_session(child, status, scan=session_members, sleep=time.sleep):
    """Retain exclusion on survivors/errors, even after the cooperative budget.

    The unreaped leader pins session identity. Native creates a process GROUP,
    not a session; Docker containers are separately checked after this function.
    """
    began = time.monotonic()
    while True:
        try:
            members = scan(child.pid)
            if not members:
                child.wait()  # Only now reap the session leader.
                return
            for pid, facts in members.items():
                try:
                    if process_stat(pid) == facts:
                        os.kill(pid, signal.SIGTERM if time.monotonic() - began < 5 else signal.SIGKILL)
                except ProcessLookupError:
                    pass
        except Exception:
            try:
                status('cleanup-supervised-unknown')
            except BaseException:
                pass  # Broken evidence storage is never permission to abandon supervision.
        sleep(0.2)


def isolated_roots(roots, value):
    value = run_id(value)
    return {k: v / ('qa-' + sha(value.encode())[:16] if k == '--runtime-root'
                    else 'pipeline-qa-' + value) for k, v in roots.items()}


def socket_budget(copies):
    parent = copies['--runtime-root'] / 'smith-next'
    lengths = {'core': len(os.fsencode(parent / 'service.sock')),
               'app': len(os.fsencode(parent / 'app.sock')),
               'native': len(os.fsencode(parent)) + NATIVE_SOCKET_SUFFIX_BYTES}
    require(all(n <= SOCKET_LIMIT for n in lengths.values()), 'nested-unix-socket-budget-refused')
    return lengths


def runtime_binding(copies, value, purpose='pipeline'):
    return {'harness_revision': 4, 'run_id': run_id(value), 'purpose': purpose,
            'roots': {k: str(v) for k, v in copies.items()},
            'runtime_identity': M.identity(copies['--runtime-root'].lstat()),
            'socket_bytes': socket_budget(copies)}


def claim_runtime(copies, value, purpose='pipeline'):
    socket_budget(copies)  # Before the first mkdir/copy, including non-pipeline validation.
    make_directory(copies['--runtime-root'])  # O_EXCL collision fence; never reuse or replace.
    C.mutate(M.write, copies['--runtime-root'] / 'qa-owner.json',
             M.encoded(runtime_binding(copies, value, purpose)))


def validate_runtime_binding(copies, value, purpose='pipeline'):
    M.safe_directory(copies['--runtime-root'], private=True)
    require(M.decode(M.read(copies['--runtime-root'] / 'qa-owner.json', 16384)) ==
            runtime_binding(copies, value, purpose), 'runtime-full-id-or-identity-conflict')


def make_directory(path):
    C.checkpoint()
    path.mkdir(mode=0o700)
    M.sync_dir(path.parent)
    C.checkpoint()


def copy_file(source, destination, expected, mode, limit=128 * 1024 * 1024):
    C.checkpoint()
    data = candidate_bytes() if source == CANDIDATE else M.read(source, limit)
    require(sha(data) == expected, 'copy-source-drift')
    M.write(destination, data, mode)
    C.checkpoint()


def copy_model(source, destination):
    C.checkpoint()
    fd = os.open(source, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        meta = os.fstat(fd)
        require(stat.S_ISREG(meta.st_mode) and meta.st_uid == os.geteuid()
                and stat.S_IMODE(meta.st_mode) == 0o600 and meta.st_nlink == 1 and meta.st_size == MODEL_SIZE)
        out = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        with os.fdopen(out, 'wb') as stream:
            digest = hashlib.sha256()
            until = time.monotonic() + 90
            while True:
                C.checkpoint()
                require(time.monotonic() < until, 'model-copy-deadline')
                chunk = os.read(fd, 1024 * 1024)
                if not chunk:
                    break
                digest.update(chunk)
                stream.write(chunk)
            stream.flush()
            os.fsync(stream.fileno())
        M.sync_dir(destination.parent)
        require(digest.hexdigest() == MODEL_SHA
                and M.file_identity(os.fstat(fd)) == M.file_identity(meta), 'model-copy-integrity')
        C.checkpoint()
    finally:
        os.close(fd)


def prepare_copy(roots, namespace, release, value):
    copies = isolated_roots(roots, value)
    claim_runtime(copies, value)
    for key in ('--data-root', '--config-root'):
        make_directory(copies[key])
    ns = copies['--data-root'] / 'smith-next'
    make_directory(ns)
    for leaf in ('bin', 'models', 'client'):
        make_directory(ns / leaf)
    make_directory(ns / 'client/releases')
    staging = ns / 'client/releases/qa-staging'
    make_directory(staging)
    manifest = M.decode(M.read(release / '.smith-next-manifest.json'))
    require(manifest['schema'] == 1 and len(manifest['entries']) <= 4096)
    for name, entry in sorted(manifest['entries'].items(), key=lambda item: (len(Path(item[0]).parts), item[0])):
        C.checkpoint()
        relative = Path(name)
        require(not relative.is_absolute() and '..' not in relative.parts and name != '.')
        target = staging / name
        if entry['type'] == 'directory':
            make_directory(target)
            # Runtime parent directories retain packaged canonical modes.
            C.mutate(os.chmod, target, entry['mode'])
        else:
            require(entry['type'] == 'file' and entry['mode'] in (0o644, 0o755))
            if name == 'smith_backend/smith-service':
                entry['sha256'] = CANDIDATE_SHA
                entry['mode'] = 0o755
                copy_file(CANDIDATE, target, CANDIDATE_SHA, 0o755)
            else:
                copy_file(release / name, target, entry['sha256'], entry['mode'])
    raw = M.encoded(manifest)
    C.mutate(M.write, staging / '.smith-next-manifest.json', raw)
    destination = staging.with_name(sha(raw))
    C.checkpoint()
    os.rename(staging, destination)
    M.sync_dir(destination.parent)
    C.checkpoint()
    os.symlink('releases/' + destination.name, ns / 'client/current')
    M.sync_dir(ns / 'client')
    C.checkpoint()
    copy_file(CANDIDATE, ns / 'bin/smith-service', CANDIDATE_SHA, 0o755)
    copy_model(namespace / 'models' / (MODEL_SHA + '.gguf'), ns / 'models' / (MODEL_SHA + '.gguf'))
    copy_file(namespace / 'models' / (MODEL_SHA + '.json'), ns / 'models' / (MODEL_SHA + '.json'),
              sha(M.read(namespace / 'models' / (MODEL_SHA + '.json'))), 0o600)
    return copies


class App:
    def __init__(self, path, pid, guard):
        self.path, self.pid, self.guard = path, pid, guard
        self.instance = None
        self.sequence = 0

    def call(self, op, args):
        self.guard()
        self.sequence += 1
        call_id = f'{self.sequence:032x}'
        message = {'schema_version': 3, 'service': 'smith-next', 'protocol': 'app',
                   'call_id': call_id, 'op': op, 'args': args}
        meta = self.path.lstat()
        require(stat.S_ISSOCK(meta.st_mode) and meta.st_uid == os.geteuid()
                and stat.S_IMODE(meta.st_mode) == 0o600)
        deadline = time.monotonic() + 2
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as stream:
            stream.settimeout(2)
            stream.connect(str(self.path))
            peer, uid, _ = struct.unpack('3i', stream.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
            require(peer == self.pid and uid == os.geteuid()
                    and M.identity(meta) == M.identity(self.path.lstat()), 'candidate-peer-conflict')
            self.guard()
            require(time.monotonic() < deadline, 'app-frame-deadline')
            stream.settimeout(max(0.001, deadline - time.monotonic()))
            stream.sendall(M.encoded(message))
            data = bytearray()
            while not data.endswith(b'\n'):
                self.guard()
                require(len(data) <= FRAME_LIMIT and time.monotonic() < deadline, 'app-frame-deadline')
                stream.settimeout(max(0.001, deadline - time.monotonic()))
                part = stream.recv(min(8192, FRAME_LIMIT + 1 - len(data)))
                require(bool(part), 'uncertain-app-response')
                data.extend(part)
            require(len(data) <= FRAME_LIMIT and data.count(b'\n') == 1)
        response = M.decode(data)
        require(response['schema_version'] == 3 and response['service'] == 'smith-next'
                and response['protocol'] == 'app' and response['pid'] == self.pid
                and response['call_id'] == call_id and re.fullmatch('[0-9a-f]{32}', response['instance_id'])
                and (self.instance is None or self.instance == response['instance_id'])
                and type(response['ok']) is bool
                and response['ok'] == (response['error'] is None)
                and response['ok'] == (response['result'] is not None), 'candidate-response-conflict')
        self.instance = response['instance_id']
        self.guard()
        if not response['ok']:
            raise AppRefusal(op, response['error'])
        return response['result']


def project_args(value, instance, revision):
    return {'request_id': ids(value)['project'], 'instance_id': instance,
            'expected_revision': revision, 'name': 'Isolated pipeline QA', 'key': 'pipeline-qa',
            'description': '', 'prompt': '', 'agent': 'opencode',
            'repositories': [{'name': 'r0', 'url': URL, 'read_only': True,
                'branch': {'base': None, 'work': None,
                           'types': dict.fromkeys(('feature', 'fix', 'release', 'hotfix'))}}]}


def exercise(app, value, attempted, pause=time.sleep):
    """Exactly one dispatch per mutating operation. A lost ack aborts, never resends."""
    identity = ids(value)
    state = app.call('state', {})
    attempted('models')
    app.call('models', {'request_id': identity['models']})
    while True:
        result = app.call('poll', {'request_id': identity['models'], 'instance_id': app.instance})
        if result['status'] not in ('queued', 'running'):
            break
        pause(0.2)
    if result['status'] != 'succeeded':
        raise AppRefusal('models', result.get('error'))
    attempted('configure')
    configured = app.call('configure_checked', {'model': MODEL, 'voice': None,
                                               'expected_revision': state['config']['revision']})
    require(configured['model'] == MODEL and configured['voice'] is None)
    projects = app.call('projects_list_v3', {})
    attempted('project')
    receipt = app.call('project_create_v3', project_args(value, app.instance, projects['revision']))
    ready = app.call('dev_jobs_state_v2', {})
    require(ready['jobs'] == [] and ready['model']['ready'] is True
            and ready['model']['name'] == MODEL and ready['model']['sha256'] == MODEL_SHA
            and ready['model']['config_revision'] == str(configured['revision']), 'model-not-ready')
    attempted('job')
    app.call('dev_job_start_v2', {'request_id': identity['job'], 'instance_id': app.instance,
        'project_id': receipt['project_id'], 'expected_projects_revision': receipt['revision'],
        'expected_config_revision': ready['model']['config_revision'], 'prompt': PROMPT})
    stages = []
    while True:
        result = app.call('dev_job_get_v2', {'request_id': identity['job']})
        status = result['job']['status']
        if not stages or stages[-1] != status:
            stages.append(status)
        if status not in ('preparing', 'cloning', 'ready', 'running', 'captured'):
            break
        pause(0.2)
    require(result['job']['request_id'] == identity['job'])
    return result, stages


def verify_frames(raw, value, result):
    require(len(raw) <= 8 * 1024 * 1024 and raw.endswith(b'\n'), 'worker-log-bound')
    frames = [M.decode(line) for line in raw.splitlines()]
    request = ids(value)['job']
    require(frames and all(frame.get('request_id') == request for frame in frames), 'worker-correlation')
    kinds = [frame.get('type') for frame in frames]
    require('failed' not in kinds and kinds.count('ready') == 1 and kinds.count('reply') == 1
            and kinds.count('model_request') >= 2 and kinds.count('read_request') == 1
            and kinds.count('read_result') == 1, 'worker-stages-incomplete')
    require(kinds.index('ready') < kinds.index('model_request') < kinds.index('read_request')
            < kinds.index('read_result') < len(kinds) - 1 and kinds[-1] == 'reply', 'worker-stage-order')
    read = frames[kinds.index('read_request')]
    readback = frames[kinds.index('read_result')]
    require(read['arguments']['filePath'] == '/workspace/repos/r0/README'
            and readback['call_id'] == read['call_id'] and isinstance(readback['text'], str)
            and bool(readback['text']), 'read-evidence-conflict')
    require(frames[-1]['text'] == result['reply']
            and frames[-1]['session_id'] == result['job']['session_id'], 'reply-evidence-conflict')
    return {kind: kinds.count(kind) for kind in sorted(set(kinds))}


def parse_diagnostics(raw):
    require(len(raw) <= 2 * 1024 * 1024, 'diagnostic-capture-bound')
    events, other = [], 0
    for line in raw.decode(errors='replace').splitlines():
        if not line.startswith('SMITH_INSPECTION_DIAG'):
            other += 1  # Never retain or echo non-diagnostic text in the safe result.
            continue
        match = re.fullmatch(r'SMITH_INSPECTION_DIAG phase=([a-z_]+) code=([A-Z_]+)', line)
        require(match is not None and match[1] in DIAGNOSTIC_PHASES
                and match[2] in DIAGNOSTIC_CODES, 'diagnostic-format-refused')
        require(len(events) < 257 and not any(e['phase'] == 'limit' for e in events),
                'diagnostic-event-bound')
        require((match[1] == 'limit') == (match[2] == 'LIMIT'), 'diagnostic-limit-shape')
        events.append({'phase': match[1], 'code': match[2]})
    return {'events': events, 'non_diagnostic_lines': other,
            'first_error': next((e for e in events if e['code'] not in ('START', 'OK', 'LIMIT')), None),
            'limit_reached': bool(events and events[-1]['phase'] == 'limit')}


def capture(stream, destination):
    # Drain even beyond retention cap, never block child on an undrained pipe.
    with os.fdopen(os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600), 'wb') as out:
        retained = 0
        while True:
            data = stream.read(8192)
            if not data:
                break
            take = min(len(data), max(0, 2 * 1024 * 1024 - retained))
            out.write(data[:take])
            retained += take
        out.flush()
        os.fsync(out.fileno())
    stream.close()


def docker_query(*args, cleanup=False, observe=False):
    # Only read-only Docker operations. Cleanup observation remains possible after
    # cancellation without resetting the task latch or permitting forward work.
    require(args and args[0] in ('ps', 'inspect', 'logs'), 'cleanup-query-not-allowed')
    if observe:
        require(len(args) == 2 and args[0] == 'logs' and re.fullmatch('[0-9a-f]{64}', args[1]),
                'log-observation-command-refused')
    def check():
        if not cleanup:
            C.checkpoint()
    check()
    argv = ['/usr/bin/docker', '--config', str(PRIVATE / 'docker-config'),
            '--host', 'unix:///run/docker.sock', *args]
    child = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, start_new_session=True, env=environment())
    channels = {child.stdout: bytearray(), child.stderr: bytearray()}
    pending = set(channels)
    until = time.monotonic() + 15
    try:
        while pending:
            check()
            require(time.monotonic() < until, 'cleanup-query-deadline')
            for stream in select.select(list(pending), [], [], 0.1)[0]:
                part = os.read(stream.fileno(), 8192)
                if not part:
                    pending.remove(stream)
                else:
                    channels[stream].extend(part)
                    require(sum(map(len, channels.values())) <= 8 * 1024 * 1024, 'cleanup-query-size')
        while os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT) is None:
            check()
            require(time.monotonic() < until, 'cleanup-query-deadline')
            time.sleep(0.05)
        require(not session_members(child.pid), 'cleanup-query-descendants')
        code = os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
        require(code.si_code == os.CLD_EXITED, 'cleanup-query-signalled')
        out, err = (bytes(channels[stream]) for stream in (child.stdout, child.stderr))
        if observe:
            result = {'exit_code': code.si_status, 'stdout_bytes': len(out), 'stderr_bytes': len(err),
                      'stdout_sha256': sha(out), 'stderr_sha256': sha(err)}
        else:
            require(code.si_status == 0, 'cleanup-query-failed')
            result = out + err if args[0] == 'logs' else out
    finally:
        end_session(child, lambda phase: None)  # Only reap; retain ownership on unknown descendants.
        child.stdout.close()
        child.stderr.close()
    check()  # A late cancellation after the final join is still sticky.
    return result


def cleanup_docker(*args):
    return docker_query(*args, cleanup=True)


def owned_container_quiet(value, allow_absent=False):
    request = ids(value)['job']
    found = cleanup_docker('ps', '-aq', '--no-trunc', '--filter', 'label=org.smith.dev.request=' + request).decode().split()
    require(len(found) <= 1)
    if not found:
        # A killed create CLI can leave a daemon-side operation uncertain.
        # Absence is sufficient only if no Dev dispatch was ever attempted.
        return allow_absent
    require(re.fullmatch('[0-9a-f]{64}', found[0]))
    info = M.decode(cleanup_docker('inspect', found[0]))[0]
    labels = info['Config']['Labels']
    require(info['Id'] == found[0] and info['Name'] == '/smith-dev-' + request and info['Image'] == IMAGE
            and labels['org.smith.dev.request'] == request
            and re.fullmatch('[0-9a-f]{32}', labels['org.smith.dev.owner']))
    return info['State']['Running'] is False


def supervise(value, directory):
    os.umask(0o077)
    child = lease = preserved = None
    drains = []
    outcome = {'status': 'incomplete', 'timestamp_gate': 'NO-GO-unwaived', 'installed_writes': False}
    with C.cancellation_owner():
        def status(phase):
            M.publish(directory / 'progress.json', {'phase': phase, 'run_id': value,
                      'supervisor_pid': os.getpid(), 'session': os.getsid(0)})
        try:
            status('revalidating')
            before = static_check()
            binding = review(value, before)
            require(M.decode(M.read(directory / 'review-binding.json')) == binding
                    and M.decode(M.read(directory / 'specification.json')) == specification(value)
                    and M.decode(M.read(directory / 'launch-authorization.json')) == {
                        'run_id': value, 'consent': 'one-isolated-public-clone-local-generation'})
            roots, namespace, release = live_check(before)
            socket_budget(isolated_roots(roots, value))
            lockpath = roots['--runtime-root'] / 'smith-next/runner.lock'
            meta = lockpath.lstat()
            require(stat.S_ISREG(meta.st_mode) and stat.S_IMODE(meta.st_mode) == 0o600
                    and meta.st_uid == os.geteuid() and meta.st_nlink == 1)
            lease = os.open(lockpath, os.O_RDWR | os.O_NOFOLLOW | os.O_NONBLOCK)
            fcntl.flock(lease, fcntl.LOCK_EX | fcntl.LOCK_NB)
            require(M.identity(os.fstat(lease)) == M.identity(lockpath.lstat()))
            live_check(before)
            preserved = retained_containers()
            require(sha(M.encoded(preserved)) == binding['retained_containers_sha256'],
                    'retained-container-review-drift')
            C.mutate(M.write, directory / 'retained-containers-before.json', M.encoded(preserved))
            for name, digest in before['states'].items():
                if digest is not None:
                    copy_file(namespace / name, directory / name, digest, 0o600)
            C.mutate(M.write, directory / 'baseline.json', M.encoded(before))
            require(not any(os.path.lexists(path) for path in isolated_roots(roots, value).values()))
            # Reserve enough space for complete packaged tree and a single-link model copy.
            needed = MODEL_SIZE + sum(p.stat().st_size for p in release.rglob('*') if p.is_file()) + 512 * 1024 * 1024
            available = os.statvfs(roots['--data-root'])
            require(available.f_bavail * available.f_frsize > needed, 'copy-space-insufficient')
            status('isolated-copy')
            copies = prepare_copy(roots, namespace, release, value)
            validate_runtime_binding(copies, value)
            live_check(before)
            review(value, before)
            argv = [str(copies['--data-root'] / 'smith-next/bin/smith-service'), 'serve',
                    *[part for key in ('--data-root', '--config-root', '--runtime-root')
                      for part in (key, str(copies[key]))]]
            socket_path = copies['--runtime-root'] / 'smith-next/app.sock'
            require(len(os.fsencode(socket_path)) < 108)
            status('candidate-dispatch-attempted')
            C.checkpoint()
            child = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE, env={'HOME': str(directory), 'PATH': '/usr/bin:/bin',
                        'LANG': 'C', 'LC_ALL': 'C', 'XDG_DATA_HOME': str(copies['--data-root']),
                        'XDG_CONFIG_HOME': str(copies['--config-root']),
                        'XDG_RUNTIME_DIR': str(copies['--runtime-root'])}, cwd=directory,
                    pass_fds=(lease,), start_new_session=True)
            M.write(directory / 'candidate-owner.json', M.encoded({'pid': child.pid, 'sid': child.pid,
                    'start': process_stat(child.pid)['start'], 'candidate_sha256': CANDIDATE_SHA}))
            for stream, name in ((child.stdout, 'candidate-stdout.log'), (child.stderr, 'candidate-stderr.log')):
                thread = threading.Thread(target=capture, args=(stream, directory / name), daemon=True)
                thread.start()
                drains.append(thread)
            until = time.monotonic() + BUDGET
            def guard():
                C.checkpoint()
                validate_runtime_binding(copies, value)
                require(time.monotonic() < until, 'qa-budget-exhausted')
                require(os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT) is None,
                        'candidate-exited')
                # State/cgroup checks before every API operation, without touching installed API.
                require(D.hashes(namespace) == before['states'], 'installed-state-drift')
            startup = time.monotonic() + 15
            while not socket_path.is_socket():
                guard()
                require(time.monotonic() < startup, 'candidate-startup-deadline')
                time.sleep(0.1)
            require(sha((Path('/proc') / str(child.pid) / 'exe').read_bytes()) == CANDIDATE_SHA)
            app = App(socket_path, child.pid, guard)
            def attempted(kind):
                guard()
                live_check(before)
                review(value, before)
                guard()
                # O_EXCL durable publication makes any subsequent invocation fail closed.
                M.write(directory / (kind + '-attempted.json'), M.encoded({'ids': ids(value), 'kind': kind}))
                guard()
                status(kind + '-dispatch-attempted')
            result, stages = exercise(app, value, attempted)
            C.mutate(M.write, directory / 'job-result.json', M.encoded(result))
            summary = result['job']
            outcome.update(status=summary['status'], stages=stages,
                           cleanup_verified=summary['cleanup_verified'], reply_available=summary['reply_available'])
            require(summary['status'] == 'completed' and summary['cleanup_verified'] is True
                    and summary['reply_available'] is True and isinstance(result['reply'], str)
                    and bool(result['reply'].strip()) and result['failed_stage'] is None,
                    'pipeline-not-completed')
            require(result['model'] == {'name': MODEL, 'sha256': MODEL_SHA}
                    and len(result['repositories']) == 1
                    and result['repositories'][0]['url'] == URL
                    and re.fullmatch('[0-9a-f]{40}', result['repositories'][0]['commit']),
                    'frozen-job-binding-conflict')
            guard()
            request = ids(value)['job']
            container = M.decode(docker('inspect', 'smith-dev-' + request))[0]
            require(container['Image'] == IMAGE and container['State']['Running'] is False
                    and container['Config']['Labels']['org.smith.dev.request'] == request,
                    'qa-container-conflict')
            raw = docker('logs', container['Id'])
            C.mutate(M.write, directory / 'qa-worker-frames.log', raw)
            outcome['frame_kinds'] = verify_frames(raw, value, result)
            require(len(result['reply'].split()) <= 20, 'reply-word-budget')
            status('reply-verified')
        except BaseException as error:
            outcome['status'] = 'incomplete-preserved'
            outcome.update(refusal_fields(error))
            # No raw messages, bodies, prompts, replies or paths enter diagnostics.
        finally:
            def cleanup_status(phase):
                try:
                    status(phase)
                except BaseException:
                    pass
            if child is not None:
                cleanup_status('candidate-session-cleanup')
                end_session(child, cleanup_status)
                for thread in drains:
                    while thread.is_alive():
                        thread.join(0.2)
                try:
                    diagnostic = parse_diagnostics(M.read(directory / 'candidate-stderr.log', 2 * 1024 * 1024))
                    M.write(directory / 'inspection-diagnostics.json', M.encoded(diagnostic))
                    outcome['diagnostic_event_count'] = len(diagnostic['events'])
                    outcome['diagnostic_first_error'] = diagnostic['first_error']
                    outcome['diagnostic_limit_reached'] = diagnostic['limit_reached']
                except BaseException:
                    outcome['diagnostic_capture'] = 'unavailable-or-refused'
                # Daemon-owned containers are not covered by session absence.
                while True:
                    try:
                        if owned_container_quiet(value, allow_absent=not
                                os.path.lexists(directory / 'job-attempted.json')):
                            break
                    except BaseException:
                        pass
                    cleanup_status('container-cleanup-supervised-unknown')
                    time.sleep(1)
            if lease is not None:
                # Cancellation must inhibit effects, not inhibit final read-only preservation proof.
                try:
                    require(D.hashes(namespace) == before['states'], 'preservation-conflict')
                    outcome['installed_states_byte_exact'] = True
                    if preserved is not None:
                        require(retained_containers(cleanup_docker, ids(value)['job'], cleanup=True) == preserved,
                                'retained-container-drift')
                        outcome['retained_container_inspect_and_log_observations_exact'] = True
                        outcome['historical_log_unavailable_unchanged'] = True
                except BaseException:
                    outcome['status'] = 'preservation-not-proven'
                os.close(lease)  # No LOCK_UN: inherited descriptors share this owned description.
            M.write(directory / 'result.json', M.encoded(outcome))
            status('finished-preserved')


def launch(value, consent):
    require(consent == 'one-isolated-public-clone-local-generation', 'explicit-consent-required')
    before = static_check()
    socket_budget(isolated_roots({k: Path(v) for k, v in before['roots'].items()}, value))
    binding = review(value, before)
    live_check(before)
    preserved = retained_containers()
    require(sha(M.encoded(preserved)) == binding['retained_containers_sha256'],
            'retained-container-review-drift')
    directory = PRIVATE / ('pipeline-qa-' + run_id(value))
    M.safe_directory(PRIVATE, private=True)
    make_directory(directory)  # Existence permanently fences re-entry, including uncertain launch.
    C.mutate(M.write, directory / 'specification.json', M.encoded(specification(value)))
    C.mutate(M.write, directory / 'review-binding.json', M.encoded(binding))
    C.mutate(M.write, directory / 'launch-authorization.json', M.encoded({'run_id': value, 'consent': consent}))
    gate_r, gate_w = os.pipe()
    logfd = os.open(directory / 'supervisor.log', os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    try:
        C.checkpoint()
        child = subprocess.Popen([sys.executable, '-B', str(Path(__file__).resolve()),
                                  '--supervise', value, '--gate-fd', str(gate_r)], stdin=subprocess.DEVNULL,
                                 stdout=logfd, stderr=logfd, start_new_session=True,
                                 env=environment(), close_fds=True, pass_fds=(gate_r,))
        M.write(directory / 'supervisor-owner.json', M.encoded({'pid': child.pid, 'sid': child.pid,
                    'start': process_stat(child.pid)['start'], 'run_id': value}))
        C.checkpoint()
        os.write(gate_w, b'1')  # No child effects before durable PID/session receipt.
    finally:
        os.close(logfd)
        os.close(gate_r)
        os.close(gate_w)
    print(json.dumps({'status': 'detached-supervision-launched', 'supervisor_pid': child.pid,
                      'run_id': value, 'poll': '--status ' + value}, sort_keys=True))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument('--plan', metavar='RUN_ID')
    modes.add_argument('--launch', metavar='RUN_ID')
    modes.add_argument('--preflight', action='store_true', help='read-only live bindings; no locks or writes')
    modes.add_argument('--status', metavar='RUN_ID')
    modes.add_argument('--supervise', metavar='RUN_ID', help=argparse.SUPPRESS)
    parser.add_argument('--consent')
    parser.add_argument('--gate-fd', type=int, help=argparse.SUPPRESS)
    args = parser.parse_args()
    require(os.geteuid() != 0 and not sys.flags.optimize)
    os.umask(0o077)
    if args.plan:
        static_check()
        spec = specification(args.plan)
        print(json.dumps({'status': 'prepared-not-executed', 'specification': spec,
                          'specification_sha256': sha(M.encoded(spec)),
                          'reviews_required': ['source', 'harness-with-worker-equivalence']}, sort_keys=True))
    elif args.preflight:
        with C.cancellation_owner():
            before = static_check()
            live_check(before)
            preserved = retained_containers()
            print(json.dumps({'status': 'read-only-preflight-pass', 'retained_containers': len(preserved),
                              'preservation_sha256': sha(M.encoded(preserved)),
                                                            'historical_log_unavailable': 1,
                                                            'historical_log_proof_sha256': HISTORICAL_LOG_PROOF_SHA}, sort_keys=True))
    elif args.launch:
        with C.cancellation_owner():
            launch(args.launch, args.consent)
    elif args.status:
        directory = PRIVATE / ('pipeline-qa-' + run_id(args.status))
        # Only allowlisted coordinator fields, never service responses/logs.
        progress = M.decode(M.read(directory / 'progress.json', 4096))
        safe = {k: progress[k] for k in ('phase', 'run_id', 'supervisor_pid', 'session')}
        if (directory / 'result.json').exists():
            result = M.decode(M.read(directory / 'result.json', 4096))
            safe['result'] = {k: result[k] for k in ('status', 'stages', 'failed_operation', 'error_code', 'cleanup_verified', 'reply_available',
                                'diagnostic_event_count', 'diagnostic_first_error', 'diagnostic_limit_reached', 'diagnostic_capture',
                    'installed_states_byte_exact', 'retained_container_inspect_and_log_observations_exact',
                                        'historical_log_unavailable_unchanged', 'frame_kinds',
                    'timestamp_gate') if k in result}
        print(json.dumps(safe, sort_keys=True))
    else:
        value = run_id(args.supervise)
        directory = PRIVATE / ('pipeline-qa-' + value)
        M.safe_directory(directory, private=True)
        require(args.gate_fd is not None and os.getsid(0) == os.getpid(), 'owned-launch-required')
        require(select.select([args.gate_fd], [], [], 15)[0] and os.read(args.gate_fd, 1) == b'1',
                'launcher-not-committed')
        os.close(args.gate_fd)
        owner = M.decode(M.read(directory / 'supervisor-owner.json'))
        require(owner == {'pid': os.getpid(), 'sid': os.getpid(), 'run_id': value,
                         'start': process_stat(os.getpid())['start']}, 'supervisor-owner-conflict')
        M.write(directory / 'supervisor-entered.json', M.encoded(owner))
        supervise(value, directory)


if __name__ == '__main__':
    try:
        main()
    except BaseException:
        print('{"status":"refused-or-incomplete","detail":"private-evidence-only-no-replay"}', file=sys.stderr)
        sys.exit(1)
