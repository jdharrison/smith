"""Backup-first one-shot models metadata refresh; import is inert.

No lifecycle, image publication, direct application write, Dev mutation or
inference. The approved NORMAL oldest-app-receipt eviction includes that job's
instruction snapshot prune and one expiry marker, never a conversation/message.
An exact independent scope review is required before --execute-reviewed. After
any attempted dispatch, only --observe may be used; it never sends models.
Run eventual live execution in a detached session with durable PID/private log
(as for the prior continuation), not under a terminal kill deadline. On observer
cancellation accepted service work remains service-owned; retain the original
request receipt and use query-only observation, never resend/replacement IDs.
"""
import contextlib
import copy
import hashlib
import os
from pathlib import Path
import re
import sys
import time
import types

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
RESUME_SHA = '4a37f8d87c520e92808c5fd8c3a3cd40dd9a6257cc3c7968162ea8d6393b0135'
COPIES_SHA = '04239dd14f9fac7fe2c9d91c64e9442529079ad105076f34ca773f2747ca4365'
APP_SHA = 'fe823204974daab7302aa9533a6bd372b5e9327a11eef3a1fdf314c1a61797af'
LEDGER_SHA = '5e49b67db3926c9e179b949c657c6542d05c4634c6b8f33acecde511bca6a76e'
OLDEST_SHA = '31bcee8e9db8120d38e34c8bc420cc46fd5c6931deab07622674a1a805238f85'
PID = 575956
PROCESS_START = '2822746'
SCOPE = 'one-models-request-oldest-app-receipt-expiry-and-associated-instruction-snapshot-only'
PARTIAL = {
    'resume-launch.json': '8054aa0a24a461d2393b85184f81347fc86ee8bd9488123044c25424a88785da',
    'resume-progress.json': 'e1eced68c534a457fd58a3ac54f1f064732331bb9b2c518967f8a65313f6adb5',
    'resume-refusal.json': 'e4ec064e64700a7b8f97cc323b6057a070491cd8b963091606f7ad184a0f9ece',
    'resume-review.json': 'b78e773787afb04c17e529479bb53ed0fe965067be8992bc8427434290e6c424',
}


def pinned_resume():
    path = HERE / 'coordinated-resume.py'
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        data = stream.read(128 * 1024 + 1)
    if len(data) > 128 * 1024 or hashlib.sha256(data).hexdigest() != RESUME_SHA:
        raise RuntimeError('Executed continuation drift')
    module = types.ModuleType('models_refresh_pinned_resume')
    module.__file__ = str(path)
    exec(compile(data, '<pinned-executed-resume>', 'exec'), module.__dict__)
    return module


C = pinned_resume()
M = C.M
PRIVATE = C.PRIVATE
BACKUP = PRIVATE / 'inventory-refresh-app-before.json'
REQUEST = PRIVATE / 'inventory-refresh-request.json'
PROGRESS = PRIVATE / 'inventory-refresh-progress.json'
RESULT = PRIVATE / 'inventory-refresh-result.json'
REVIEW = PRIVATE / 'inventory-refresh-review.json'
TESTS = HERE / 'test_coordinated_models_refresh.py'


def number(value):
    return type(value) is int and 0 <= value <= 9007199254740991


def retention_precondition(before):
    M.require(before['schema_version'] == 10 and len(before['jobs']) == 32)
    M.require(number(before['revision']) and before['revision'] <= 9007199254740989)
    M.require(len(before['expired_ids']) < 1024)
    M.require(all(j['operation']['status'] in ('succeeded', 'failed', 'interrupted') for j in before['jobs']))
    oldest = before['jobs'][0]
    M.require(M.sha(M.encoded(oldest)) == OLDEST_SHA, 'Oldest retained receipt drift')
    old_id = oldest['operation']['request_id']
    M.require(old_id not in before['expired_ids'])
    M.require(oldest['version'] == 3 and oldest['operation']['kind'] == 'submit_routed'
              and oldest['operation']['status'] == 'succeeded' and oldest['conversation_id'] is not None)
    M.require(old_id in before['instructions']['snapshots'], 'Expected companion snapshot unavailable')
    return old_id


def request_fingerprint(request_id):
    # app_v3 delegates models to v2; app_v2::request_fingerprint uses v2:models.
    data = {'kind': 'v2:models', 'args': {'request_id': request_id}}
    return M.sha(M.encoded(data).removesuffix(b'\n'))


def inventory_delta(before, after, request_id, operation):
    old_id = retention_precondition(before)
    M.require(len(after['jobs']) == 32 and after['jobs'][:-1] == before['jobs'][1:])
    M.require(after['expired_ids'] == before['expired_ids'] + [old_id])
    job = after['jobs'][-1]
    M.require(set(job) == {'operation', 'fingerprint', 'version', 'conversation_id',
                           'history_revision', 'task', 'accepted_ms'})
    M.require(job['version'] == 2 and job['task'] == 'Models'
              and job['conversation_id'] is None and job['history_revision'] is None
              and number(job['accepted_ms']) and job['accepted_ms'] > 0
              and job['fingerprint'] == request_fingerprint(request_id))
    op = job['operation']
    M.require(set(op) == {'request_id', 'kind', 'status', 'phase', 'config_revision', 'result', 'error'})
    M.require(op['request_id'] == request_id and op['kind'] == 'models'
              and op['status'] in ('succeeded', 'failed') and op['phase'] == 'complete'
              and op['config_revision'] == before['config']['revision'])
    wire = dict(op, conversation_id=None, config_revision=str(op['config_revision']))
    M.require(operation == wire, 'Foreign or mismatched poll receipt')
    M.require((op['status'] == 'succeeded' and op['error'] is None and isinstance(op['result'], dict))
              or (op['status'] == 'failed' and op['result'] is None and isinstance(op['error'], dict)))
    snapshots = copy.deepcopy(before['instructions']['snapshots'])
    snapshots.pop(old_id)
    M.require(after['instructions']['snapshots'] == snapshots)
    reverse = copy.deepcopy(after)
    reverse['jobs'] = [copy.deepcopy(before['jobs'][0]), *reverse['jobs'][:-1]]
    reverse['expired_ids'].pop()
    reverse['instructions']['snapshots'][old_id] = copy.deepcopy(before['instructions']['snapshots'][old_id])
    M.require(reverse['revision'] == before['revision'] + 2)
    reverse['revision'] = before['revision']
    M.require(reverse == before, 'Unapproved app-state delta')


def immutable_binding():
    binding = C.load()
    M.pinned_source(C.COPIES, COPIES_SHA)
    for name, digest in PARTIAL.items():
        M.pinned_source(PRIVATE / name, digest)
    M.require(not os.path.lexists(C.RESULT))
    M.require(not any(os.path.lexists(PRIVATE / n) for n in ('metadata-inventory.json', 'metadata-app-before.json')))
    original = M.read(C.SNAPSHOT / M.STATES[2])
    updated = M.read(C.SNAPSHOT / 'updated-ledger.json')
    M.require(M.sha(updated) == LEDGER_SHA)
    M.assert_record_diff(M.decode(original), M.decode(updated))
    M.require(M.state_hashes(C.SNAPSHOT) == binding['before']['states'])
    M.require(not C.group_members(574586), 'Prior continuation still active')
    return binding, original, updated


def identity(H):
    M.require(H.property_value('MainPID') == str(PID))
    proc = Path('/proc') / str(PID)
    M.require(proc.joinpath('stat').read_text().rsplit(')', 1)[1].split()[19] == PROCESS_START)


def peer(H, roots, op, args, instance=None):
    C.checkpoint()
    M.require(op in ('state', 'dev_jobs_state_v2', 'models', 'poll'))
    reply = H.exchange(roots['--runtime-root'] / 'smith-next/app.sock', M.app_request(op, args), PID)
    C.checkpoint()
    if instance is not None:
        M.require(reply['instance_id'] == instance, 'Service instance changed')
    return reply


def live_check(H, binding, original, updated, app_sha):
    identity(H)
    M.require(immutable_binding()[0] == binding)
    roots, pid = C.running(H, binding, original, updated, app_sha)
    M.require(pid == PID)
    C.image_binding(binding, True)
    C.artifacts(binding)
    return roots


def review():
    expected = {'script_sha256': M.sha(M.read(Path(__file__), source=True)),
                'tests_sha256': M.sha(M.read(TESTS, source=True)), 'executed_resume_sha256': RESUME_SHA,
                'original_module_sha256': C.ORIGINAL, 'binding_sha256': C.BINDING,
                'partial_evidence_sha256': M.sha(M.encoded(PARTIAL)),
                'copies_proof_sha256': COPIES_SHA,
                'app_before_sha256': APP_SHA, 'oldest_receipt_sha256': OLDEST_SHA,
                'scope': SCOPE, 'verdict': 'pass', 'independent_review': True}
    M.require(M.decode(M.read(REVIEW)) == expected, 'Fresh exact independent review required')


def phase(name):
    M.publish(PROGRESS, {'phase': name, 'automatic_retry': False, 'dev_job_submitted': False})


def snapshot_and_request(H, binding, original, updated):
    roots = live_check(H, binding, original, updated, APP_SHA)
    ns = roots['--data-root'] / 'smith-next'
    data = M.read(ns / M.STATES[0])
    M.require(M.sha(data) == APP_SHA)
    before = M.decode(data)
    retention_precondition(before)
    state = peer(H, roots, 'state', {})
    dev = peer(H, roots, 'dev_jobs_state_v2', {}, state['instance_id'])
    selected = {k: dev['result']['model'][k] for k in ('name', 'sha256', 'config_revision')}
    M.require(selected['name'] and selected['sha256'])
    request_id = M.uuid.uuid4().hex
    M.require(request_id not in before['expired_ids']
              and all(j['operation']['request_id'] != request_id for j in before['jobs']))
    # Private exact bytes include the evicted receipt and companion snapshot.
    # write() fsyncs file and parent before request identity or IPC can follow.
    M.write(BACKUP, data)
    M.require(M.read(BACKUP) == data)
    saved = {'request_id': request_id, 'instance_id': state['instance_id'], 'pid': PID,
             'process_start': PROCESS_START, 'app_before_sha256': APP_SHA,
             'binding_sha256': C.BINDING, 'partial_evidence_sha256': M.sha(M.encoded(PARTIAL)),
             'oldest_receipt_sha256': OLDEST_SHA, 'selected_model': selected,
             'wire_config_sha256': M.sha(M.encoded(state['result']['config']))}
    M.write(REQUEST, M.encoded(saved))
    phase('identity-durable-dispatch-attempted')
    # No service lease can serialize unrelated UI writers. Revalidate immediately
    # before dispatch; any concurrent writer invalidates acceptance, not replay.
    live_check(H, binding, original, updated, APP_SHA)
    C.checkpoint()
    try:
        accepted = peer(H, roots, 'models', {'request_id': request_id}, saved['instance_id'])
        M.require(accepted['result']['request_id'] == request_id and accepted['result']['kind'] == 'models')
    except Exception:
        C.checkpoint()  # Cancellation prohibits even further observation here.
        phase('dispatch-uncertain-query-only')
    return saved


def saved_request():
    saved = M.decode(M.read(REQUEST))
    M.require(saved['pid'] == PID and saved['process_start'] == PROCESS_START
              and saved['app_before_sha256'] == APP_SHA and saved['binding_sha256'] == C.BINDING
              and saved['oldest_receipt_sha256'] == OLDEST_SHA
              and saved['partial_evidence_sha256'] == M.sha(M.encoded(PARTIAL)))
    M.require(bool(re.fullmatch('[0-9a-f]{32}', saved['request_id']))
              and bool(re.fullmatch('[0-9a-f]{32}', saved['instance_id'])))
    backup = M.read(BACKUP)
    M.require(M.sha(backup) == APP_SHA)
    before = M.decode(backup)
    retention_precondition(before)
    M.require(saved['request_id'] not in before['expired_ids']
              and all(j['operation']['request_id'] != saved['request_id'] for j in before['jobs']))
    return saved, before


def observe(H, binding, original, updated):
    saved, before = saved_request()
    identity(H)
    roots = C.roots_of(binding)
    deadline = time.monotonic() + 15
    while True:
        C.checkpoint()
        M.require(time.monotonic() < deadline, 'Observation deadline; keep original ID; no resend')
        identity(H)
        reply = peer(H, roots, 'poll', {'request_id': saved['request_id'],
                     'instance_id': saved['instance_id']}, saved['instance_id'])
        operation = reply['result']
        M.require(operation['request_id'] == saved['request_id'] and operation['kind'] == 'models')
        if operation['status'] not in ('queued', 'running'):
            break
        time.sleep(0.1)
    data = M.read(roots['--data-root'] / 'smith-next' / M.STATES[0])
    inventory_delta(before, M.decode(data), saved['request_id'], operation)
    live_check(H, binding, original, updated, M.sha(data))
    state = peer(H, roots, 'state', {}, saved['instance_id'])
    dev = peer(H, roots, 'dev_jobs_state_v2', {}, saved['instance_id'])
    M.require(M.sha(M.encoded(state['result']['config'])) == saved['wire_config_sha256'])
    M.require(all(dev['result']['model'][k] == v for k, v in saved['selected_model'].items()))
    M.require(M.read(roots['--data-root'] / 'smith-next' / M.STATES[0]) == data)
    ready = dev['result']['model']['ready'] is True
    result = {'status': 'succeeded' if operation['status'] == 'succeeded' and ready else 'terminal-not-ready',
              'request_id': saved['request_id'], 'app_before_sha256': APP_SHA,
              'app_after_sha256': M.sha(data), 'oldest_receipt_sha256': OLDEST_SHA,
              'one_receipt_evicted': True, 'expiry_marker_appended': True,
              'associated_instruction_snapshot_pruned': True, 'conversations_unchanged': True,
              'other_app_records_unchanged': True, 'dev_ledgers_unchanged': True,
              'model_ready': ready, 'config_changed': False, 'inference_run': False,
              'dev_job_submitted': False, 'release_accepted': False, 'timestamp_failure_waived': False}
    M.publish(RESULT, result)
    phase('verified-' + result['status'])
    return result


@contextlib.contextmanager
def guards(H):
    with C.cancellation_owner(), C.live_guards(H):
        docker = M.docker
        def readonly_docker(args, timeout=10):
            M.require(args[:2] in (['image', 'inspect'], ['container', 'ls'],
                                  ['container', 'inspect'], ['container', 'logs']))
            return docker(args, timeout=timeout)
        M.docker = readonly_docker
        try:
            yield
        finally:
            M.docker = docker


def run(H, execute):
    with guards(H):
        review()  # Never create/approve a receipt here, in either mode.
        binding, original, updated = immutable_binding()
        if execute:
            M.require(not any(os.path.lexists(p) for p in (BACKUP, REQUEST, PROGRESS, RESULT)), 'No replay')
            M.write(PROGRESS, M.encoded({'phase': 'claimed', 'automatic_retry': False,
                                       'dev_job_submitted': False}))
            snapshot_and_request(H, binding, original, updated)
        else:
            M.require(not os.path.lexists(RESULT), 'Already observed; no extra protocol calls')
        return observe(H, binding, original, updated)


def main():
    M.require(not sys.flags.optimize and os.geteuid() != 0)
    M.require(sys.argv[1:] in (['--execute-reviewed'], ['--observe']), 'Explicit narrow mode required')
    os.umask(0o077)
    binding = C.load()
    env = C.environment(binding)
    H, _ = M.support()
    def property_value(name):
        data = C.owned_command(['/usr/bin/systemctl', '--user', 'show', 'smith-next.service',
                               '--property=' + name, '--value'], env, timeout=10, merge_stderr=True, limit=16384)
        return data.decode().strip()
    H.property_value = property_value
    result = run(H, sys.argv[1:] == ['--execute-reviewed'])
    print('Models metadata observation: ' + result['status'] + '; no inference or Dev execution.')


if __name__ == '__main__':
    try:
        main()
    except BaseException:
        print('Refresh refused or uncertain. Keep private evidence; query original ID only; never resend.', file=sys.stderr)
        sys.exit(1)
