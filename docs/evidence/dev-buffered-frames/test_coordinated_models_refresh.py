"""Offline protocol/retention fixtures only; no host services, models or Docker."""
import contextlib
import copy
import importlib.util
import os
from pathlib import Path
import signal
import tempfile
import unittest
from unittest.mock import Mock, patch

SCRIPT = Path(__file__).with_name('coordinated-models-refresh.py')
spec = importlib.util.spec_from_file_location('models_refresh', SCRIPT)
F = importlib.util.module_from_spec(spec)
with patch('subprocess.run', side_effect=AssertionError('No host execution')):
    spec.loader.exec_module(F)
M = F.M
C = F.C
ID = '1' * 32
INSTANCE = '2' * 32


def app():
    jobs = []
    for index in range(32):
        jobs.append({'operation': {'request_id': format(100 + index, '032x'), 'kind': 'submit_routed',
                     'status': 'succeeded', 'phase': 'complete', 'config_revision': 7,
                     'result': {'synthetic': True}, 'error': None},
                     'fingerprint': 'a' * 64, 'version': 3, 'conversation_id': 'conversation',
                     'history_revision': 1, 'task': {'Routed': {'input': 'synthetic'}}, 'accepted_ms': index + 1})
    return {'schema_version': 10, 'revision': 100, 'config': {'revision': 7, 'model': 'fixture'},
            'jobs': jobs, 'expired_ids': [format(i, '032x') for i in range(15)],
            'conversations': [{'conversation_id': 'conversation', 'history': [{'content': 'synthetic'}]}],
            'instructions': {'snapshots': {j['operation']['request_id']: {'synthetic': True} for j in jobs},
                             'grants': [{'synthetic': True}], 'settings': {'unchanged': True}},
            'projects': {'unchanged': True}, 'agents': {'unchanged': True}, 'cloud': {'unchanged': True}}


def after_app(before, status='succeeded'):
    after = copy.deepcopy(before)
    old = after['jobs'].pop(0)['operation']['request_id']
    after['expired_ids'].append(old)
    after['instructions']['snapshots'].pop(old)
    op = {'request_id': ID, 'kind': 'models', 'status': status, 'phase': 'complete',
          'config_revision': 7, 'result': {'models': []} if status == 'succeeded' else None,
          'error': None if status == 'succeeded' else {'code': 'SYNTHETIC'}}
    after['jobs'].append({'operation': op, 'fingerprint': F.request_fingerprint(ID), 'version': 2,
                         'conversation_id': None, 'history_revision': None, 'task': 'Models', 'accepted_ms': 1000})
    after['revision'] += 2
    return after, dict(op, conversation_id=None, config_revision='7')


class RefreshTests(unittest.TestCase):
    def setUp(self):
        self.host = patch('subprocess.run', side_effect=AssertionError('Host execution forbidden'))
        self.host.start()
        self.addCleanup(self.host.stop)
        self.before = app()
        self.pins = patch.multiple(F, OLDEST_SHA=M.sha(M.encoded(self.before['jobs'][0])),
                                   APP_SHA=M.sha(M.encoded(self.before)))
        self.pins.start()
        self.addCleanup(self.pins.stop)

    def test_import_pins_executed_coordinators_without_running_modes(self):
        self.assertEqual(M.sha(M.read(F.HERE / 'coordinated-resume.py', source=True)), F.RESUME_SHA)
        self.assertEqual(M.sha(M.read(F.HERE / 'coordinated-maintenance.py', source=True)), C.ORIGINAL)

    def test_precise_retention_and_companion_prune_preserve_all_other_fields(self):
        original = copy.deepcopy(self.before)
        after, operation = after_app(self.before)
        F.inventory_delta(self.before, after, ID, operation)
        self.assertEqual(self.before, original)
        self.assertEqual(after['conversations'], original['conversations'])
        self.assertEqual(after['jobs'][:-1], original['jobs'][1:])
        self.assertEqual(len(after['expired_ids']), len(original['expired_ids']) + 1)

    def test_failed_inventory_retains_exact_single_eviction_without_success_claim(self):
        after, operation = after_app(self.before, 'failed')
        F.inventory_delta(self.before, after, ID, operation)

    def test_capacity_active_count_schema_oldest_snapshot_and_revision_drift_block(self):
        for kind in ('capacity', 'active', 'count', 'schema', 'oldest', 'snapshot', 'revision'):
            before = copy.deepcopy(self.before)
            if kind == 'capacity':
                before['expired_ids'] = ['synthetic'] * 1024
            elif kind == 'active':
                before['jobs'][1]['operation']['status'] = 'running'
            elif kind == 'count':
                before['jobs'].pop()
            elif kind == 'schema':
                before['schema_version'] = 9
            elif kind == 'oldest':
                before['jobs'][0], before['jobs'][1] = before['jobs'][1], before['jobs'][0]
            elif kind == 'snapshot':
                before['instructions']['snapshots'].pop(before['jobs'][0]['operation']['request_id'])
            else:
                before['revision'] = 9007199254740991
            with self.subTest(kind=kind), self.assertRaises(M.Refused):
                F.retention_precondition(before)

    def test_unapproved_deltas_and_foreign_receipts_fail_closed(self):
        for kind in ('conversation', 'config', 'other-job', 'expiry', 'other-snapshot', 'old-snapshot',
                     'grants', 'projects', 'agents', 'cloud', 'revision', 'new-field', 'fingerprint',
                     'task', 'wire', 'result', 'status'):
            after, operation = after_app(self.before)
            if kind == 'conversation':
                after['conversations'][0]['history'] = []
            elif kind == 'config':
                after['config']['model'] = 'foreign'
            elif kind == 'other-job':
                after['jobs'][0]['operation']['result'] = None
            elif kind == 'expiry':
                after['expired_ids'][-1] = 'foreign'
            elif kind == 'other-snapshot':
                after['instructions']['snapshots'].pop(after['jobs'][0]['operation']['request_id'])
            elif kind == 'old-snapshot':
                old = self.before['jobs'][0]['operation']['request_id']
                after['instructions']['snapshots'][old] = self.before['instructions']['snapshots'][old]
            elif kind in ('grants', 'projects', 'agents', 'cloud'):
                if kind == 'grants':
                    after['instructions']['grants'] = []
                else:
                    after[kind] = {}
            elif kind == 'revision':
                after['revision'] += 1
            elif kind in ('new-field', 'fingerprint', 'task'):
                after['jobs'][-1][kind] = 'foreign'
            elif kind == 'wire':
                operation['request_id'] = 'f' * 32
            elif kind == 'result':
                operation['result'] = {'foreign': True}
            else:
                after['jobs'][-1]['operation']['status'] = 'running'
            with self.subTest(kind=kind), self.assertRaises(M.Refused):
                F.inventory_delta(self.before, after, ID, operation)

    def test_only_first_terminal_receipt_not_selected_alternative(self):
        after, operation = after_app(self.before)
        after['jobs'][0] = copy.deepcopy(self.before['jobs'][0])
        with self.assertRaises(M.Refused):
            F.inventory_delta(self.before, after, ID, operation)

    @contextlib.contextmanager
    def protocol_fixture(self, behavior='success'):
        with tempfile.TemporaryDirectory() as tmp, contextlib.ExitStack() as stack:
            base = Path(tmp)
            ns = base / 'smith-next'
            ns.mkdir()
            M.write(ns / M.STATES[0], M.encoded(self.before))
            roots = {'--data-root': base, '--config-root': base / 'config', '--runtime-root': base / 'runtime'}
            for name, suffix in (('BACKUP', 'backup'), ('REQUEST', 'request'), ('PROGRESS', 'progress'),
                                 ('RESULT', 'result'), ('REVIEW', 'review')):
                stack.enter_context(patch.object(F, name, base / suffix))
            stack.enter_context(patch.object(F, 'review'))
            stack.enter_context(patch.object(F, 'immutable_binding', return_value=({}, b'original', b'updated')))
            stack.enter_context(patch.object(F, 'identity'))
            stack.enter_context(patch.object(F, 'live_check', return_value=roots))
            stack.enter_context(patch.object(C, 'roots_of', return_value=roots))
            stack.enter_context(patch.object(M.uuid, 'uuid4', return_value=Mock(hex=ID)))
            H = Mock()
            H.HELD_LOCKS = []
            calls = []
            after, operation = after_app(self.before, 'failed' if behavior == 'failed' else 'succeeded')
            polls = [0]
            def exchange(socket, request, pid):
                op, args = request['op'], request['args']
                calls.append((op, copy.deepcopy(args)))
                if op == 'models':
                    # Durable originals and identity MUST precede the sole IPC.
                    self.assertEqual(M.read(F.BACKUP), M.encoded(self.before))
                    self.assertEqual(M.decode(M.read(F.REQUEST))['request_id'], args['request_id'])
                    self.assertEqual(M.decode(M.read(F.PROGRESS))['phase'], 'identity-durable-dispatch-attempted')
                    M.publish(ns / M.STATES[0], after)
                    if behavior == 'cancel-models':
                        os.kill(os.getpid(), signal.SIGTERM)
                        raise TimeoutError('synthetic uncertainty')
                    if behavior == 'lost-ack':
                        raise TimeoutError('synthetic uncertainty')
                    result = {'request_id': ID, 'kind': 'models'}
                elif op == 'poll':
                    self.assertEqual(args, {'request_id': ID, 'instance_id': INSTANCE})
                    polls[0] += 1
                    if behavior == 'lost-poll' and polls[0] == 1:
                        raise TimeoutError('synthetic lost poll')
                    result = operation
                elif op == 'state':
                    if behavior == 'cancel-state':
                        os.kill(os.getpid(), signal.SIGTERM)
                    result = {'config': {'revision': '7', 'model': 'fixture'}}
                elif op == 'dev_jobs_state_v2':
                    result = {'model': {'name': 'fixture', 'sha256': 'a' * 64,
                                       'config_revision': '7', 'ready': any(c[0] == 'models' for c in calls)}}
                    if behavior == 'model-drift' and any(c[0] == 'models' for c in calls):
                        result['model']['sha256'] = 'b' * 64
                else:
                    raise AssertionError('Unapproved protocol operation')
                return {'instance_id': 'f' * 32 if behavior == 'foreign' and op == 'poll' else INSTANCE,
                        'result': result}
            H.exchange.side_effect = exchange
            yield H, calls, base

    def test_backup_precedes_one_models_and_query_only_poll_frozen_config_success(self):
        with self.protocol_fixture() as (H, calls, base):
            result = F.run(H, True)
            self.assertEqual(result['status'], 'succeeded')
            self.assertEqual([c[0] for c in calls].count('models'), 1)
            self.assertEqual([c[0] for c in calls].count('poll'), 1)
            self.assertTrue(result['conversations_unchanged'])
            self.assertFalse(result['inference_run'])
            self.assertFalse(result['dev_job_submitted'])
            self.assertFalse(result['timestamp_failure_waived'])
            self.assertEqual(M.read(F.BACKUP), M.encoded(self.before))
            with self.assertRaises(M.Refused):
                F.run(H, True)
            with self.assertRaises(M.Refused):
                F.run(H, False)
            self.assertEqual([c[0] for c in calls].count('models'), 1)

    def test_lost_acceptance_observes_same_id_without_resend(self):
        with self.protocol_fixture('lost-ack') as (H, calls, base):
            self.assertEqual(F.run(H, True)['status'], 'succeeded')
            self.assertEqual([c[0] for c in calls].count('models'), 1)
            self.assertEqual([c[1]['request_id'] for c in calls if c[0] in ('models', 'poll')], [ID, ID])

    def test_lost_poll_followup_observe_uses_original_identity_only(self):
        with self.protocol_fixture('lost-poll') as (H, calls, base):
            with self.assertRaises(TimeoutError):
                F.run(H, True)
            self.assertFalse(F.RESULT.exists())
            self.assertEqual(F.run(H, False)['status'], 'succeeded')
            self.assertEqual([c[0] for c in calls].count('models'), 1)
            self.assertEqual(M.decode(M.read(F.REQUEST))['request_id'], ID)

    def test_cancelled_dispatch_preserves_identity_and_forbids_automatic_poll(self):
        with self.protocol_fixture('cancel-models') as (H, calls, base):
            with self.assertRaises(M.Refused):
                F.run(H, True)
            self.assertTrue(F.BACKUP.exists() and F.REQUEST.exists())
            self.assertEqual([c[0] for c in calls].count('poll'), 0)
            self.assertEqual(F.run(H, False)['status'], 'succeeded')
            self.assertEqual([c[0] for c in calls].count('models'), 1)

    def test_cancel_before_backup_never_dispatches_models(self):
        with self.protocol_fixture('cancel-state') as (H, calls, base):
            with self.assertRaises(M.Refused):
                F.run(H, True)
            self.assertFalse(F.BACKUP.exists() or F.REQUEST.exists())
            self.assertNotIn('models', [c[0] for c in calls])

    def test_terminal_failure_not_success_or_resend(self):
        with self.protocol_fixture('failed') as (H, calls, base):
            self.assertEqual(F.run(H, True)['status'], 'terminal-not-ready')
            self.assertEqual([c[0] for c in calls].count('models'), 1)

    def test_foreign_instance_or_model_drift_blocks_acceptance(self):
        for behavior in ('foreign', 'model-drift'):
            with self.subTest(behavior=behavior), self.protocol_fixture(behavior) as (H, calls, base):
                with self.assertRaises(M.Refused):
                    F.run(H, True)
                self.assertFalse(F.RESULT.exists())
                self.assertEqual([c[0] for c in calls].count('models'), 1)

    def test_backup_failure_prevents_identity_and_models(self):
        with self.protocol_fixture() as (H, calls, base):
            raw = M.write
            def write(path, data, mode=0o600):
                if path == F.BACKUP:
                    raise OSError('synthetic fsync failure')
                return raw(path, data, mode)
            with patch.object(M, 'write', side_effect=write), self.assertRaises(OSError):
                F.run(H, True)
            self.assertFalse(F.REQUEST.exists())
            self.assertNotIn('models', [c[0] for c in calls])

    def test_observe_without_original_identity_never_generates_request(self):
        with self.protocol_fixture() as (H, calls, base), patch.object(M.uuid, 'uuid4', side_effect=AssertionError('No IDs')):
            with self.assertRaises(FileNotFoundError):
                F.run(H, False)
            self.assertEqual(calls, [])

    def test_review_missing_never_writes_or_self_approves(self):
        with tempfile.TemporaryDirectory() as tmp, patch.object(F, 'REVIEW', Path(tmp) / 'review'):
            with self.assertRaises(FileNotFoundError):
                F.review()
            self.assertEqual(list(Path(tmp).iterdir()), [])

    def test_review_exact_identity_and_companion_prune_scope_no_implicit_approval(self):
        receipt = {'script_sha256': M.sha(M.read(SCRIPT, source=True)),
                   'tests_sha256': M.sha(M.read(F.TESTS, source=True)),
                   'executed_resume_sha256': F.RESUME_SHA, 'original_module_sha256': C.ORIGINAL,
                   'binding_sha256': C.BINDING, 'partial_evidence_sha256': M.sha(M.encoded(F.PARTIAL)),
                   'copies_proof_sha256': F.COPIES_SHA, 'app_before_sha256': F.APP_SHA,
                   'oldest_receipt_sha256': F.OLDEST_SHA, 'scope': F.SCOPE,
                   'verdict': 'pass', 'independent_review': True}
        with tempfile.TemporaryDirectory() as tmp, patch.object(F, 'REVIEW', Path(tmp) / 'review'):
            M.write(F.REVIEW, M.encoded(receipt))
            F.review()
            for field in ('script_sha256', 'partial_evidence_sha256', 'copies_proof_sha256',
                          'oldest_receipt_sha256', 'scope', 'independent_review'):
                bad = dict(receipt, **{field: False if field == 'independent_review' else 'foreign'})
                M.publish(F.REVIEW, bad)
                with self.subTest(field=field), self.assertRaises(M.Refused):
                    F.review()
            self.assertEqual(len(list(Path(tmp).iterdir())), 1)

    def test_poll_deadline_retains_original_identity_without_resend(self):
        with self.protocol_fixture() as (H, calls, base), patch('time.monotonic', side_effect=[0, 16]):
            with self.assertRaises(M.Refused):
                F.run(H, True)
            self.assertTrue(F.REQUEST.exists() and F.BACKUP.exists())
            self.assertFalse(F.RESULT.exists())
            self.assertEqual([c[0] for c in calls].count('models'), 1)
            self.assertEqual([c[0] for c in calls].count('poll'), 0)

    def test_saved_identity_or_backup_drift_refuses_observation(self):
        with self.protocol_fixture('lost-poll') as (H, calls, base):
            with self.assertRaises(TimeoutError):
                F.run(H, True)
            original = M.read(F.REQUEST)
            saved = M.decode(original)
            saved['request_id'] = self.before['jobs'][0]['operation']['request_id']
            M.publish(F.REQUEST, saved)
            with self.assertRaises(M.Refused):
                F.run(H, False)
            M.publish(F.REQUEST, M.decode(original))
            M.publish(F.BACKUP, {'foreign': True})
            with self.assertRaises(M.Refused):
                F.run(H, False)
            self.assertEqual([c[0] for c in calls].count('models'), 1)

    def test_docker_mutation_is_refused(self):
        H = Mock()
        with F.guards(H), patch.object(C, 'owned_command', side_effect=AssertionError('No dispatch')):
            for args in (['image', 'tag', 'x', 'y'], ['container', 'rm', 'x'], ['build', '.']):
                with self.assertRaises(M.Refused):
                    M.docker(args)

    def test_unknown_cli_mode_refused_before_source_or_host_checks(self):
        with patch.object(F.sys, 'argv', ['refresh', '--start']), patch.object(C, 'load') as load:
            with self.assertRaises(M.Refused):
                F.main()
            load.assert_not_called()


if __name__ == '__main__':
    unittest.main()
