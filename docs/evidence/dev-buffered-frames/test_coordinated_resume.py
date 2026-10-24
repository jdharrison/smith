"""Pure adversarial tests: no Docker, user manager, model or installed lifecycle."""
import copy
import contextlib
import importlib.util
import os
import signal
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import Mock, patch

SCRIPT = Path(__file__).with_name('coordinated-resume.py')
spec = importlib.util.spec_from_file_location('resume', SCRIPT)
C = importlib.util.module_from_spec(spec)
with patch('subprocess.run', side_effect=AssertionError('No host tools')):
    spec.loader.exec_module(C)
M = C.M


def ledger():
    checkout = {'url': 'https://example.invalid/repo.git', 'ref': 'refs/heads/main', 'commit': 'a' * 40}
    record = {'summary': {'request_id': M.REQUEST, 'status': 'unknown', 'cleanup_verified': True,
                          'reply_available': False, 'session_id': 'session_fixture', 'message': 'Original uncertainty'},
              'cleanup_required': False, 'may_have_sent': True, 'failed_stage': 'running', 'image_id': M.IMAGE,
              'reply': None, 'message_id': None, 'repositories': [checkout],
              'job': {'repositories': [{'url': checkout['url'], 'ref': 'main'}], 'prompt': 'synthetic'},
              'error': {'code': 'original'}, 'fingerprint': 'unchanged'}
    others = [dict(copy.deepcopy(record), summary=dict(record['summary'], request_id=str(i), status='failed'))
              for i in range(6)]
    return {'schema_version': 1, 'records': [record, *others]}


def roots():
    return {k: Path('/fixture') / v for k, v in (('--data-root', 'd'), ('--config-root', 'c'),
                                               ('--runtime-root', 'r'))}


def exec_text(args, **changes):
    values = {'path': args[0], 'argv[]': ' '.join(args), 'ignore_errors': 'no',
              'start_time': '[Mon 2026-10-19 14:08:50 PDT]', 'stop_time': '[n/a]',
              'pid': '321258', 'code': '(null)', 'status': '0/0'}
    values.update(changes)
    return '{ ' + ' ; '.join(k + '=' + v for k, v in values.items()) + ' }'


def observation(prepare=True):
    r = roots()
    value = {'schema_version': 2, 'service': 'smith-next', 'ok': True, 'binary_state': 'installed',
             'unit_state': 'installed', 'enablement_state': 'enabled', 'runtime_state': 'stopped',
             'bridge_state': 'unavailable', 'setup_complete': None, 'message': 'synthetic', 'error': None,
             'effects': {'binary_path': C.arguments(r)[0],
                         'unit_path': str(r['--config-root'] / 'systemd/user/smith-next.service'),
                         'state_path': str(r['--data-root'] / 'smith-next/state.json'),
                         'socket_path': str(r['--runtime-root'] / 'smith-next/service.sock'),
                         'description': 'synthetic'}}
    if prepare:
        value['approval_token'] = '1.' + 'a' * 64
    else:
        value.update(runtime_state='running', bridge_state='ready')
    return value


class ResumeTests(unittest.TestCase):
    def setUp(self):
        self.host = patch('subprocess.run', side_effect=AssertionError('No host execution'))
        self.host.start()
        self.addCleanup(self.host.stop)

    def test_import_and_original_pin(self):
        self.assertEqual(M.sha(M.read(C.HERE / 'coordinated-maintenance.py', source=True)), C.ORIGINAL)

    def test_exec_volatile_results_only(self):
        args = C.arguments(roots())
        self.assertEqual(C.effective_start(exec_text(args), args), args)
        self.assertEqual(C.effective_start(exec_text(args, pid='0', start_time='[n/a]',
                         stop_time='[Mon 2026-10-19 16:00:00 PDT]', code='exited', status='0'), args), args)

    def test_exec_config_and_dangerous_flags_refused(self):
        args = C.arguments(roots())
        for changes in ({'ignore_errors': 'yes'}, {'path': '/other'}, {'argv[]': ' '.join(args) + ' --unknown'},
                        {'argv[]': ' '.join(args).replace(' serve ', ' inspect ')},
                        {'argv[]': '-' + ' '.join(args)}, {'argv[]': ' '.join(args).replace('/fixture/d', '/other')},
                        {'flags': 'ignore-failure'}, {'pid': '-1'}):
            with self.subTest(changes=changes), self.assertRaises(M.Refused):
                C.effective_start(exec_text(args, **changes), args)

    def test_exec_multiple_or_malformed_commands_refused(self):
        args = C.arguments(roots())
        for text in (exec_text(args) * 2, '+' + exec_text(args), exec_text(args) + '\n',
                     exec_text(args).replace('ignore_errors=no ; ', ''), exec_text(args).replace(' ; ', ';')):
            with self.subTest(text=text), self.assertRaises(M.Refused):
                C.effective_start(text, args)

    def test_unit_still_checks_link_reload_dropins_transient_source(self):
        H = Mock()
        args = C.arguments(roots())
        expected = {'unit': '/unit', 'link': '/unit', 'link_identity': [1, 2], 'effective_start': exec_text(args)}
        binding = {'before': {'roots': {k: str(v) for k, v in roots().items()}, 'unit': expected}}
        facts = dict(expected, effective_start=exec_text(args, pid='0'))
        values = {'LoadState': 'loaded', 'Transient': 'no', 'SourcePath': ''}
        H.property_value.side_effect = values.__getitem__
        with patch.object(M, 'unit_facts', return_value=facts), patch.object(M, 'read', return_value=(
                'ExecStart=' + ' '.join(args) + '\n').encode()):
            C.unit_check(H, binding)
            for key, bad in (('LoadState', 'not-found'), ('Transient', 'yes'), ('SourcePath', '/foreign')):
                old = values[key]
                values[key] = bad
                with self.assertRaises(M.Refused):
                    C.unit_check(H, binding)
                values[key] = old
            facts['link_identity'] = [1, 3]
            with self.assertRaises(M.Refused):
                C.unit_check(H, binding)

    def test_ledger_exact_two_field_delta(self):
        original = ledger()
        before = copy.deepcopy(original)
        updated = M.decode(M.updated_ledger(M.encoded(original)))
        M.assert_record_diff(original, updated)
        self.assertEqual(original, before)
        self.assertEqual(len(updated['records']), 7)
        self.assertEqual(updated['schema_version'], 3)
        self.assertEqual(updated['records'][0]['summary']['status'], 'reconciled')
        self.assertEqual(updated['records'][1:], before['records'][1:])

    def test_record_hidden_markers_or_other_deltas_refused(self):
        original = ledger()
        for field, value in (('deleted', True), ('may_have_sent', False), ('error', None),
                             ('reply', 'invented'), ('repositories', []), ('session_id', 'other')):
            updated = M.decode(M.updated_ledger(M.encoded(original)))
            record = updated['records'][0]
            if field == 'session_id':
                record['summary'][field] = value
            else:
                record[field] = value
            with self.subTest(field=field), self.assertRaises(M.Refused):
                M.assert_record_diff(original, updated)
        updated = M.decode(M.updated_ledger(M.encoded(original)))
        updated['records'][1]['job']['prompt'] = 'changed'
        with self.assertRaises(M.Refused):
            M.assert_record_diff(original, updated)

    def test_pending_other_work_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            ns = Path(tmp)
            for name, value in ((M.STATES[0], {'jobs': [{'operation': {'status': 'queued'}}]}),
                                (M.STATES[1], {'records': []}), (M.STATES[2], ledger())):
                M.write(ns / name, M.encoded(value))
            with self.assertRaises(M.Refused):
                M.idle(ns)

    def test_schema_count_and_target_drift_refused(self):
        for change in ('schema', 'count', 'target'):
            before = ledger()
            if change == 'schema':
                before['schema_version'] = 3
            elif change == 'count':
                before['records'].pop()
            else:
                before['records'][0]['summary']['status'] = 'failed'
            with self.subTest(change=change), self.assertRaises(M.Refused):
                M.updated_ledger(M.encoded(before))

    def test_cli_exact_real_schema(self):
        token = C.observation(observation(), True, roots())
        self.assertEqual(token, '1.' + 'a' * 64)
        self.assertIsNone(C.observation(observation(False), False, roots()))

    def test_cli_bad_observations_refused(self):
        for key, bad in (('schema_version', 1), ('ok', False), ('error', 'failed'),
                         ('unit_state', 'conflict'), ('enablement_state', 'disabled'),
                         ('runtime_state', 'running'), ('approval_token', 'invalid'), ('extra', 1)):
            value = observation()
            value[key] = bad
            with self.subTest(key=key), self.assertRaises(M.Refused):
                C.observation(value, True, roots())
        value = observation()
        value['effects']['binary_path'] = '/other'
        with self.assertRaises(M.Refused):
            C.observation(value, True, roots())

    def test_image_binding_all_immutable_fields_and_exact_tags(self):
        old = {'Id': M.IMAGE, 'Config': {'User': '1000:1000'}, 'RepoTags': [M.RUNTIME_TAG],
               'Metadata': {'LastTagTime': '2026-10-19T19:57:27Z'}}
        new = dict(copy.deepcopy(old), Id=C.NEW_IMAGE, RepoTags=[M.CANDIDATE_TAG])
        binding = {'old_image': old, 'new_image': new}
        for published in (False, True):
            actual_old = copy.deepcopy(old)
            actual_old['RepoTags'] = [M.BACKUP_TAG] + ([] if published else [M.RUNTIME_TAG])
            actual_new = copy.deepcopy(new)
            actual_new['RepoTags'] += [M.RUNTIME_TAG] if published else []
            actual_new['Metadata']['LastTagTime'] = '2026-10-19T23:12:01.123Z'
            with patch.object(M, 'image', side_effect=lambda name: copy.deepcopy(
                    actual_old if name == M.BACKUP_TAG else actual_new)):
                C.image_binding(binding, published)
                actual_new['Config']['User'] = '0'
                with self.assertRaises(M.Refused):
                    C.image_binding(binding, published)
                actual_new['Config']['User'] = '1000:1000'
                actual_new['RepoTags'].append('unexpected:tag')
                with self.assertRaises(M.Refused):
                    C.image_binding(binding, published)
        self.assertEqual(binding['old_image'], old)

    def test_lease_order_and_reverse_release(self):
        H, R = Mock(), Mock()
        H.HELD_LOCKS = []
        binding = {'before': {'roots': {k: str(v) for k, v in roots().items()}}}
        events = []
        def acquire(path, directory=False):
            events.append(('acquire', path, directory))
            H.HELD_LOCKS.append((100 + len(H.HELD_LOCKS), path, directory))
        H.acquire_lock.side_effect = acquire
        H.safe_file.return_value.st_mode = stat.S_IFREG | 0o600
        with patch.object(C, 'stopped', side_effect=lambda *a: events.append(('stopped',))), \
             patch.object(C, 'joined_driver', side_effect=lambda: events.append(('joined',))), \
             patch('os.path.lexists', return_value=True), \
             patch('fcntl.flock', side_effect=lambda fd, _: events.append(('unlock', fd))), \
             patch('os.close', side_effect=lambda fd: events.append(('close', fd))):
            with C.leases(H, R, binding):
                self.assertEqual([p for _, p, _ in H.HELD_LOCKS], [roots()['--data-root'] / '.smith-next-install.lock',
                                 roots()['--data-root'], roots()['--runtime-root'] / 'smith-next/runner.lock'])
            self.assertEqual(events[:2], [('stopped',), ('joined',)])
            self.assertEqual([x[1] for x in events if x[0] == 'unlock'], [102, 101, 100])
            self.assertEqual(H.HELD_LOCKS, [])

    def test_quiescence_failure_prevents_any_lock(self):
        H = Mock()
        H.HELD_LOCKS = []
        with patch.object(C, 'stopped', side_effect=M.Refused('active')):
            with self.assertRaises(M.Refused):
                with C.leases(H, Mock(), {}):
                    self.fail('must not enter')
        H.acquire_lock.assert_not_called()

    def test_missing_lock_refused_without_creating_it(self):
        H = Mock()
        H.HELD_LOCKS = []
        binding = {'before': {'roots': {k: str(v) for k, v in roots().items()}}}
        with patch.object(C, 'stopped'), patch.object(C, 'joined_driver'), patch('os.path.lexists', return_value=False):
            with self.assertRaises(M.Refused):
                with C.leases(H, Mock(), binding):
                    self.fail('must not enter')
        H.acquire_lock.assert_not_called()

    def test_core_rechecks_binding_before_each_command(self):
        H = Mock()
        H.HELD_LOCKS = []
        with patch.object(C, 'check', side_effect=M.Refused('drift')), patch.object(C, 'owned_core') as owned:
            for args in (['prepare', '--action', 'start'], ['apply', '--action', 'start', '--approval', 'synthetic']):
                with self.assertRaises(M.Refused):
                    C.core_command(H, Mock(), {}, 'digest', args)
            owned.assert_not_called()

    def test_core_refuses_recursive_lease(self):
        H = Mock()
        H.HELD_LOCKS = [(1, Path('/fixture'), True)]
        with patch.object(C, 'owned_core') as owned, self.assertRaises(M.Refused):
            C.core_command(H, Mock(), {}, 'digest', ['prepare', '--action', 'start'])
        owned.assert_not_called()

    def test_review_required_and_bound_without_self_approval(self):
        with tempfile.TemporaryDirectory() as tmp, patch.object(C, 'PRIVATE', Path(tmp)):
            with self.assertRaises(FileNotFoundError):
                C.review({})
            self.assertEqual(list(Path(tmp).iterdir()), [])

    def test_review_binds_checkout_source_hashes_not_recipient_modes(self):
        with tempfile.TemporaryDirectory() as tmp:
            private = Path(tmp)
            copies = private / 'copies'
            M.write(copies, b'synthetic-proof')
            receipt = {'script_sha256': M.sha(M.read(SCRIPT, source=True)),
                       'tests_sha256': M.sha(M.read(C.TESTS, source=True)),
                       'copies_proof_sha256': M.sha(M.read(copies)),
                       'binding_sha256': C.BINDING, 'original_module_sha256': C.ORIGINAL,
                       'verdict': 'pass', 'independent_review': True}
            M.write(private / 'resume-review.json', M.encoded(receipt))
            with patch.object(C, 'PRIVATE', private), patch.object(C, 'COPIES', copies), \
                 patch.object(C, 'copies_proof') as proof:
                C.review({})
                proof.assert_called_once_with({})
                receipt['script_sha256'] = '0' * 64
                M.publish(private / 'resume-review.json', receipt)
                with self.assertRaises(M.Refused):
                    C.review({})

    def test_progress_retains_attempted_phase_and_refuses_replay(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            with patch.object(C, 'PROGRESS', base / 'private'), patch.object(C, 'SAFE_PROGRESS', base / 'safe'):
                value = {'phase': 'claimed', 'invocation_id': 'fixture'}
                M.write(C.PROGRESS, M.encoded(value))
                updated = C.phase(value, 'ledger-publish-attempted')
                self.assertEqual(M.decode(M.read(C.PROGRESS)), updated)
                self.assertEqual(M.decode(M.read(C.SAFE_PROGRESS)), updated)
                with self.assertRaises(M.Refused):
                    C.phase(value, 'retry')

    def test_snapshot_original_bytes_absence_and_target_retained(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            ns = base / 'ns'
            private = base / 'private'
            ns.mkdir()
            private.mkdir()
            artifacts = private / 'artifacts-before'
            artifacts.mkdir()
            M.write(artifacts / 'unit', b'synthetic-unit')
            for name, payload in ((M.STATES[0], b'app'), (M.STATES[1], b'dev'),
                                  (M.STATES[2], M.encoded(ledger()))):
                M.write(ns / name, payload)
            binding = {'before': {'states': M.state_hashes(ns)}}
            with patch.object(C, 'PRIVATE', private), patch.object(C, 'SNAPSHOT', private / 'before-transition'), \
                 patch.object(C, 'artifacts', return_value={'unit': 'synthetic'}):
                original, updated = C.snapshot(binding, ns, [])
                self.assertEqual(M.read(ns / M.STATES[2]), original)
                self.assertEqual(M.read(C.SNAPSHOT / M.STATES[2]), original)
                self.assertEqual(M.decode(M.read(C.SNAPSHOT / 'original-target.json')), M.target(ledger()))
                self.assertIsNone(M.decode(M.read(C.SNAPSHOT / 'state-presence.json'))['state.json'])
                self.assertFalse((C.SNAPSHOT / 'state.json').exists())
                M.assert_record_diff(M.decode(original), M.decode(updated))
                with self.assertRaises(FileExistsError):
                    C.snapshot(binding, ns, [])

    def test_validate_existing_proof_does_not_repeat_readers(self):
        with patch('os.path.lexists', return_value=True), patch.object(C, 'copies_proof', return_value={'saved': True}), \
             patch('subprocess.Popen', side_effect=AssertionError('No duplicate fixture')):
            self.assertEqual(C.validate_copies(Mock(), {}), {'saved': True})

    def test_reader_partial_directory_not_reused(self):
        with tempfile.TemporaryDirectory() as tmp:
            private = Path(tmp)
            (private / 'r0').mkdir()
            with patch.object(C, 'PRIVATE', private), patch.object(C, 'COPIES', private / 'proof'), \
                 patch.object(C, 'fixture_sources', return_value=(Path('/fixture'), b'original', b'updated')), \
                 patch('subprocess.Popen', side_effect=AssertionError('No spawn')):
                with self.assertRaises(FileExistsError):
                    C.validate_copies(Mock(), {})

    def simulate_continuation(self, failure=None, cancellation=None):
        with tempfile.TemporaryDirectory() as tmp, contextlib.ExitStack() as stack:
            base = Path(tmp)
            private = base / 'private'
            private.mkdir()
            ns = base / 'smith-next'
            ns.mkdir()
            r = {'--data-root': base, '--config-root': base / 'config', '--runtime-root': base / 'runtime'}
            original = M.encoded(ledger())
            updated = M.updated_ledger(original)
            M.write(ns / M.STATES[2], original)
            H, R = Mock(), Mock()
            H.HELD_LOCKS = []
            artifacts = private / 'artifacts-before'
            artifacts.mkdir()
            M.write(artifacts / 'unit', b'synthetic-unit')
            binding = {'before': {'states': M.state_hashes(ns)}}
            events = []
            previous_handler = signal.getsignal(signal.SIGTERM)
            sent = []
            def cancel_at(name):
                if cancellation != name or sent:
                    return
                sent.append(name)
                events.append('cancel-' + name)
                self.assertIsNotNone(C.OWNER)
                if name in ('snapshot-write', 'ledger-rename', 'tag-join'):
                    self.assertTrue(H.HELD_LOCKS)
                    contender = os.open(base / 'test.lock', os.O_RDWR)
                    try:
                        with self.assertRaises(BlockingIOError):
                            C.fcntl.flock(contender, C.fcntl.LOCK_EX | C.fcntl.LOCK_NB)
                    finally:
                        os.close(contender)
                os.kill(os.getpid(), signal.SIGTERM)
                self.assertTrue(C.OWNER.cancelled[0])
            real_fsync, real_replace = os.fsync, os.replace
            def fsync(fd):
                real_fsync(fd)
                if os.readlink('/proc/self/fd/' + str(fd)) == str(C.SNAPSHOT / M.STATES[2]):
                    cancel_at('snapshot-write')
                if sent == ['ledger-rename'] and os.readlink('/proc/self/fd/' + str(fd)) == str(ns):
                    self.assertTrue(H.HELD_LOCKS)
                    events.append('renamed-ledger-fsynced')
            def replace(source, destination):
                real_replace(source, destination)
                if destination == ns / M.STATES[2]:
                    cancel_at('ledger-rename')
            @contextlib.contextmanager
            def leases(*args):
                events.append('locks-acquired')
                fd = os.open(base / 'test.lock', os.O_RDWR | os.O_CREAT, 0o600)
                C.fcntl.flock(fd, C.fcntl.LOCK_EX | C.fcntl.LOCK_NB)
                H.HELD_LOCKS = [(fd, base / 'test.lock', False)]
                try:
                    yield
                finally:
                    C.fcntl.flock(fd, C.fcntl.LOCK_UN)
                    os.close(fd)
                    H.HELD_LOCKS = []
                    events.append('locks-released')
            def docker(args):
                self.assertEqual(M.read(ns / M.STATES[2]), updated)
                self.assertTrue(H.HELD_LOCKS)
                events.append('image-tag')
                cancel_at('tag-join')
                if cancellation == 'tag-join':
                    events.append('tag-child-joined')
                if failure == 'image':
                    raise M.Refused('synthetic image failure')
            def core(*args):
                self.assertFalse(H.HELD_LOCKS)
                command = args[-1]
                events.append(command[0])
                if failure == command[0]:
                    raise M.Refused('synthetic Core failure')
                cancel_at('late-' + command[0])
                return {'synthetic': True}
            def observe(value, prepare, roots):
                return 'synthetic-token' if prepare else None
            for name, value in {'PRIVATE': private, 'PROGRESS': private / 'progress',
                                'SAFE_PROGRESS': base / 'safe-progress', 'RESULT': base / 'result',
                                'SNAPSHOT': private / 'snapshot'}.items():
                stack.enter_context(patch.object(C, name, value))
            stack.enter_context(patch.object(C, 'review'))
            stack.enter_context(patch.object(C, 'check', return_value=ns))
            stack.enter_context(patch.object(C, 'leases', side_effect=leases))
            stack.enter_context(patch.object(C, 'artifacts', return_value={'unit': 'synthetic'}))
            stack.enter_context(patch('os.fsync', side_effect=fsync))
            stack.enter_context(patch('os.replace', side_effect=replace))
            stack.enter_context(patch.object(C, 'roots_of', return_value=r))
            stack.enter_context(patch.object(C, 'core_command', side_effect=core))
            stack.enter_context(patch.object(C, 'observation', side_effect=observe))
            stack.enter_context(patch.object(C, 'running', return_value=(r, 123)))
            stack.enter_context(patch.object(C, 'unit_check'))
            stack.enter_context(patch.object(C, 'load', return_value={}))
            stack.enter_context(patch.object(C, 'owned_docker', side_effect=docker))
            stack.enter_context(patch.object(M, 'pair'))
            inventory = stack.enter_context(patch.object(M, 'metadata_inventory',
                            return_value={'request_id': 'original-inventory-id', 'model_ready': True,
                                          'app_after_sha256': 'synthetic-app-digest'}))
            if failure or cancellation:
                with self.assertRaises(M.Refused):
                    C.continuation(H, R, binding)
                receipt = M.decode(M.read(private / 'resume-refusal.json'))
                expected_phase = {'image': 'ledger-confirmed-image-publish-attempted',
                    'prepare': 'start-prepare-attempted', 'apply': 'start-apply-attempted',
                    'snapshot-write': 'snapshot-attempted', 'ledger-rename': 'ledger-publish-attempted',
                    'tag-join': 'ledger-confirmed-image-publish-attempted',
                    'late-prepare': 'start-prepare-attempted', 'late-apply': 'start-apply-attempted'}
                self.assertEqual(receipt['last_phase'], expected_phase[failure or cancellation])
                self.assertEqual(receipt['cancelled'], bool(cancellation))
                self.assertFalse(C.RESULT.exists())
                inventory.assert_not_called()
            else:
                C.continuation(H, R, binding)
                result = M.decode(M.read(C.RESULT))
                self.assertFalse(result['release_accepted'])
                self.assertFalse(result['timestamp_failure_waived'])
                self.assertEqual(result['inventory_request_id'], 'original-inventory-id')
                inventory.assert_called_once()
            self.assertEqual(M.read(ns / M.STATES[2]), original if cancellation == 'snapshot-write' else updated)
            self.assertEqual(events.count('image-tag'), 0 if cancellation in ('snapshot-write', 'ledger-rename') else 1)
            self.assertIs(signal.getsignal(signal.SIGTERM), previous_handler)
            self.assertIsNone(C.OWNER)
            with self.assertRaises(M.Refused):
                C.continuation(H, R, binding)
            return events

    def test_continuation_order_no_start_under_leases_and_one_inventory(self):
        self.assertEqual(self.simulate_continuation(),
                         ['locks-acquired', 'image-tag', 'locks-released', 'prepare', 'apply'])

    def test_image_partial_failure_never_starts_or_rolls_back(self):
        self.assertEqual(self.simulate_continuation('image'), ['locks-acquired', 'image-tag', 'locks-released'])

    def test_prepare_failure_no_reprepare_apply_or_inventory(self):
        self.assertEqual(self.simulate_continuation('prepare'),
                         ['locks-acquired', 'image-tag', 'locks-released', 'prepare'])

    def test_apply_failure_no_retry_or_inventory(self):
        self.assertEqual(self.simulate_continuation('apply'),
                         ['locks-acquired', 'image-tag', 'locks-released', 'prepare', 'apply'])

    def test_sigterm_during_actual_snapshot_fsync_stops_before_ledger_or_tag(self):
        self.assertEqual(self.simulate_continuation(cancellation='snapshot-write'),
                         ['locks-acquired', 'cancel-snapshot-write', 'locks-released'])

    def test_sigterm_after_actual_rename_finishes_fsync_before_lease_release(self):
        self.assertEqual(self.simulate_continuation(cancellation='ledger-rename'),
                         ['locks-acquired', 'cancel-ledger-rename', 'renamed-ledger-fsynced', 'locks-released'])

    def test_cancelled_tag_completion_retains_lease_through_join_and_never_starts(self):
        self.assertEqual(self.simulate_continuation(cancellation='tag-join'),
                         ['locks-acquired', 'image-tag', 'cancel-tag-join', 'tag-child-joined', 'locks-released'])

    def test_cancelled_late_prepare_return_prevents_apply(self):
        self.assertEqual(self.simulate_continuation(cancellation='late-prepare'),
                         ['locks-acquired', 'image-tag', 'locks-released', 'prepare', 'cancel-late-prepare'])

    def test_cancelled_late_apply_return_prevents_inventory(self):
        self.assertEqual(self.simulate_continuation(cancellation='late-apply'),
                         ['locks-acquired', 'image-tag', 'locks-released', 'prepare', 'apply', 'cancel-late-apply'])

    def late_owned_completion(self, where):
        real_decode = M.decode
        def waitpid(*args):
            if where == 'join':
                os.kill(os.getpid(), signal.SIGTERM)
            return 654321, 0
        def decode(data):
            if where == 'json':
                os.kill(os.getpid(), signal.SIGTERM)
            return real_decode(data)
        with C.cancellation_owner() as owner, \
             patch('os.fork', return_value=654321) as fork, patch('os.getpgid', return_value=654321), \
             patch('select.select', side_effect=lambda read, *args: (read, [], [])), \
             patch('os.read', side_effect=[b'1', b'{"ok":true}\n', b'']), patch('os.write'), \
             patch('os.waitid', return_value=Mock()), patch('os.waitpid', side_effect=waitpid), \
             patch.object(C, 'group_members', return_value=[]), patch.object(M, 'decode', side_effect=decode), \
             patch.object(C, 'cancel_core') as cancel:
            with self.assertRaises(M.Refused):
                C.owned_core(['/synthetic', 'prepare'], {})
            self.assertTrue(owner.cancelled[0])
            self.assertIs(C.OWNER, owner)
            with self.assertRaises(M.Refused):
                C.owned_core(['/synthetic', 'apply'], {})
            self.assertEqual(fork.call_count, 1)
            cancel.assert_not_called()  # The first child was already joined.

    def test_sigterm_in_final_core_wait_stays_sticky_before_next_dispatch(self):
        self.late_owned_completion('join')

    def test_sigterm_in_core_json_decode_stays_sticky_before_next_dispatch(self):
        self.late_owned_completion('json')

    def test_real_owned_cli_and_descendant_cancel_join_before_unlock(self):
        # Finite private Python fixture, never Docker/Core/systemctl. Deliver real
        # SIGTERM while its owned process group is alive under an actual flock.
        with tempfile.TemporaryDirectory() as tmp:
            lock = Path(tmp) / 'lock'
            fd = os.open(lock, os.O_RDWR | os.O_CREAT, 0o600)
            C.fcntl.flock(fd, C.fcntl.LOCK_EX | C.fcntl.LOCK_NB)
            real_read, real_cancel = os.read, C.cancel_core
            captured = []
            events = []
            def read(pipe, size):
                data = real_read(pipe, size)
                if data.startswith(b'READY '):
                    captured.extend(int(v) for v in data.split()[1:])
                    os.kill(os.getpid(), signal.SIGTERM)
                return data
            def cancel(pid, ready):
                contender = os.open(lock, os.O_RDWR)
                try:
                    with self.assertRaises(BlockingIOError):
                        C.fcntl.flock(contender, C.fcntl.LOCK_EX | C.fcntl.LOCK_NB)
                finally:
                    os.close(contender)
                events.append('cancel-under-lease')
                result = real_cancel(pid, ready)
                self.assertEqual(C.group_members(pid), [])
                events.append('joined')
                return result
            code = ('import os,time; child=os.fork(); '
                    'print("READY",os.getpid(),child,flush=True) if child else None; time.sleep(20)')
            try:
                with C.cancellation_owner() as owner, patch('os.read', side_effect=read), \
                     patch.object(C, 'cancel_core', side_effect=cancel):
                    with self.assertRaises(M.Refused):
                        C.owned_command([os.sys.executable, '-c', code], {'PATH': '/nonexistent'}, timeout=5)
                    self.assertTrue(owner.cancelled[0])
                    self.assertEqual(len(captured), 2)
            finally:
                C.fcntl.flock(fd, C.fcntl.LOCK_UN)
                os.close(fd)
                events.append('unlocked')
            self.assertEqual(events, ['cancel-under-lease', 'joined', 'unlocked'])

    def retained_supervision(self, outcomes, wait_failures=(), signal_failures=(), join_failures=()):
        # Finite simulated failures and survivors, with a real cooperating flock.
        # The fake clock exceeds BOTH old cleanup deadlines, without real waits.
        with tempfile.TemporaryDirectory() as tmp:
            lock = Path(tmp) / 'publication.lock'
            fd = os.open(lock, os.O_RDWR | os.O_CREAT, 0o600)
            C.fcntl.flock(fd, C.fcntl.LOCK_EX | C.fcntl.LOCK_NB)
            scans, events, clock = [], [], [0]
            proof_absent = [False]
            remaining = iter(outcomes)
            wait_errors, signal_errors, join_errors = map(iter, (wait_failures, signal_failures, join_failures))
            def held():
                contender = os.open(lock, os.O_RDWR)
                try:
                    try:
                        C.fcntl.flock(contender, C.fcntl.LOCK_EX | C.fcntl.LOCK_NB)
                    except BlockingIOError:
                        return True
                    C.fcntl.flock(contender, C.fcntl.LOCK_UN)
                    return False
                finally:
                    os.close(contender)
            def advance(seconds):
                events.append(('sleep', clock[0], held()))
                clock[0] += 10
            def members(pid):
                outcome = next(remaining, [])
                proof_absent[0] = not isinstance(outcome, Exception) and not outcome
                scans.append((clock[0], held(), 'unknown' if isinstance(outcome, Exception) else list(outcome)))
                if isinstance(outcome, Exception):
                    raise outcome
                return outcome
            def send(pid, sig):
                events.append(('signal', clock[0], held(), sig))
                error = next(signal_errors, None)
                if error is not None:
                    raise error
            def waitid(*args):
                events.append(('wait', clock[0], held()))
                error = next(wait_errors, None)
                if error is not None:
                    raise error
                return Mock()
            def join(pid, flags):
                events.append(('join', clock[0], held(), proof_absent[0]))
                error = next(join_errors, None)
                if error is not None:
                    raise error
                return pid, 256
            try:
                with C.cancellation_owner() as owner, patch('os.killpg', side_effect=send), \
                     patch('os.waitid', side_effect=waitid), patch('os.waitpid', side_effect=join), \
                     patch.object(C, 'group_members', side_effect=members), \
                     patch('time.monotonic', side_effect=lambda: clock[0]), patch('time.sleep', side_effect=advance):
                    self.assertEqual(C.cancel_core(654321, True), 256)
                    self.assertTrue(owner.cancelled[0])
                    with self.assertRaises(M.Refused):
                        C.checkpoint()
                    with patch('os.fork') as dispatch:
                        with self.assertRaises(M.Refused):
                            C.owned_core(['/synthetic', 'apply'], {})
                        dispatch.assert_not_called()
                    self.assertTrue(held())
            finally:
                C.fcntl.flock(fd, C.fcntl.LOCK_UN)
                os.close(fd)
                events.append(('unlocked', clock[0], False))
            self.assertTrue(all(scan[1] for scan in scans))
            self.assertTrue(all(event[2] for event in events[:-1]))
            self.assertTrue(all(event[3] for event in events if event[0] == 'join'))
            self.assertEqual(scans[-1][2], [])
            self.assertEqual(events[-2][0], 'join')
            return scans, events

    def test_cancel_proc_failures_after_expiry_retain_lease_until_positive_absence(self):
        outcomes = [OSError('synthetic proc failure') for _ in range(4)] + [[654322], []]
        scans, events = self.retained_supervision(outcomes)
        self.assertEqual([scan[2] for scan in scans], ['unknown'] * 4 + [[654322], []])
        self.assertGreater(scans[-1][0], 7)
        self.assertEqual([event[3] for event in events if event[0] == 'signal'],
                         [signal.SIGTERM, signal.SIGKILL])

    def test_cancel_survivors_beyond_kill_deadline_retain_lease_and_unreaped_leader(self):
        scans, events = self.retained_supervision([[654322]] * 5 + [[]])
        self.assertEqual([scan[2] for scan in scans], [[654322]] * 5 + [[]])
        self.assertEqual(len([event for event in events if event[0] == 'join']), 1)
        self.assertGreater(scans[-1][0], 7)

    def test_cancel_wait_errors_and_interrupted_join_do_not_unwind_lease(self):
        scans, events = self.retained_supervision([[654322], [], []],
            wait_failures=[OSError('synthetic wait failure'), InterruptedError()],
            join_failures=[InterruptedError()])
        self.assertEqual([scan[2] for scan in scans], [[654322], [], []])
        self.assertEqual(len([event for event in events if event[0] == 'join']), 2)

    def test_cancel_signal_failure_retains_lease_until_supervision_recovers(self):
        scans, events = self.retained_supervision([[654322], []],
            signal_failures=[PermissionError('synthetic signal failure')])
        self.assertEqual([scan[2] for scan in scans], [[654322], []])
        self.assertGreater(scans[-1][0], 7)

    def test_task_owner_inhibits_later_write_and_inventory_protocol_dispatch(self):
        H = Mock()
        sent = []
        def exchange(*args):
            sent.append(args)
            os.kill(os.getpid(), signal.SIGTERM)
            return {'synthetic': True}
        H.exchange.side_effect = exchange
        with tempfile.TemporaryDirectory() as tmp, C.cancellation_owner() as owner, C.live_guards(H):
            with self.assertRaises(M.Refused):
                H.exchange('synthetic-state-query')
            self.assertTrue(owner.cancelled[0])
            with self.assertRaises(M.Refused):
                H.exchange('synthetic-models-dispatch')
            with self.assertRaises(M.Refused):
                M.write(Path(tmp) / 'must-not-exist', b'synthetic')
            self.assertEqual(len(sent), 1)
            self.assertEqual(list(Path(tmp).iterdir()), [])

    def test_owned_docker_fixed_tag_command_uses_supervised_group(self):
        with tempfile.TemporaryDirectory() as tmp:
            private = Path(tmp)
            (private / 'docker-config').mkdir(mode=0o700)
            with patch.object(C, 'PRIVATE', private), patch.object(M, 'safe_directory'), \
                 patch.object(C, 'owned_command', return_value=b'') as command, C.cancellation_owner():
                C.owned_docker(['image', 'tag', C.NEW_IMAGE, M.RUNTIME_TAG])
                self.assertEqual(command.call_args.args[0], ['/usr/bin/docker', '--config',
                    str(private / 'docker-config'), '--host', 'unix:///run/docker.sock',
                    'image', 'tag', C.NEW_IMAGE, M.RUNTIME_TAG])
                self.assertTrue(command.call_args.kwargs['merge_stderr'])
                for args in (['image', 'tag', M.IMAGE, M.RUNTIME_TAG], ['build', '.']):
                    with self.assertRaises(M.Refused):
                        C.owned_docker(args)
                self.assertEqual(command.call_count, 1)

    def test_owned_core_output_in_memory_only_and_joined(self):
        # Mock the fork parent branch completely: do not execute any child/tool.
        with patch('os.fork', return_value=654321), patch('os.getpgid', return_value=654321), \
             patch('select.select', side_effect=lambda read, *args: (read, [], [])), \
             patch('os.read', side_effect=[b'1', b'{"ok":true}\n', b'']), \
             patch('os.write'), patch('os.waitid', return_value=Mock()), \
             patch('os.waitpid', return_value=(654321, 0)), patch.object(C, 'group_members', return_value=[]), \
             patch.object(C, 'cancel_core') as cancel:
            self.assertEqual(C.owned_core(['/synthetic', 'prepare'], {}), {'ok': True})
            cancel.assert_not_called()

    def test_owned_core_handshake_failure_cancels_and_joins(self):
        with patch('os.fork', return_value=654321), patch('select.select', return_value=([], [], [])), \
             patch.object(C, 'cancel_core') as cancel:
            with self.assertRaises(M.Refused):
                C.owned_core(['/synthetic'], {})
            cancel.assert_called_once_with(654321, False)

    def test_owned_core_timeout_cancels_owned_group_and_joins(self):
        with patch('os.fork', return_value=654321), patch('os.getpgid', return_value=654321), \
             patch('select.select', side_effect=lambda read, *args: (read, [], [])), \
             patch('os.read', return_value=b'1'), patch('os.write'), \
             patch.object(C, 'cancel_core') as cancel:
            with self.assertRaises(M.Refused):
                C.owned_core(['/synthetic'], {}, timeout=0)
            cancel.assert_called_once_with(654321, True)

    def test_main_unknown_modes_refused_before_binding(self):
        with patch.object(C.sys, 'argv', ['resume', '--start']), patch.object(M, 'load_binding') as load:
            with self.assertRaises(M.Refused):
                C.main()
            load.assert_not_called()


if __name__ == '__main__':
    unittest.main()
