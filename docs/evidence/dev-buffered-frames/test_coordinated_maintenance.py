"""Offline-only adversarial fixtures. Never call a coordinator CLI mode or host tool."""
import ast
import copy
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch, Mock

SCRIPT = Path(__file__).with_name('coordinated-maintenance.py')
spec = importlib.util.spec_from_file_location('coordinator', SCRIPT)
C = importlib.util.module_from_spec(spec)
with patch('subprocess.run', side_effect=AssertionError('Host execution forbidden during import')):
    spec.loader.exec_module(C)


def ledger():
    checkout = {'url': 'https://example.invalid/repo.git', 'ref': 'refs/heads/main', 'commit': 'a' * 40}
    record = {'summary': {'request_id': C.REQUEST, 'status': 'unknown', 'cleanup_verified': True,
                          'reply_available': False, 'session_id': 'session_fixture', 'message': 'Original uncertainty'},
              'cleanup_required': False, 'may_have_sent': True, 'failed_stage': 'running', 'image_id': C.IMAGE,
              'reply': None, 'message_id': None, 'repositories': [checkout],
              'job': {'repositories': [{'url': checkout['url'], 'ref': 'main'}], 'prompt': 'fixture only'},
              'fingerprint': 'unchanged'}
    other = [dict(copy.deepcopy(record), summary=dict(record['summary'], request_id=str(i), status='failed')) for i in range(6)]
    for r in other:
        r['may_have_sent'] = False
    return {'schema_version': 1, 'records': [record, *other]}


def receipt_record(status='failed', cleanup_required=False):
    # Exact receipt ledger shape from app_dev_jobs.rs, not a local-runner Summary.
    return {'summary': {'request_id': C.REQUEST, 'project_id': '1' * 32,
                        'project_key': 'fixture', 'agent_id': 'developer', 'agent_name': 'Developer',
                        'status': status, 'message': 'Fixture receipt',
                        'session_id': 'fixture_session' if status == 'recorded' else None, 'created_ms': 1},
            'fingerprint': 'a' * 64, 'prompt': 'fixture only', 'may_have_sent': status == 'recorded',
            'cleanup_required': cleanup_required}


def frames():
    return [dict(type='repository_request', request_id=C.REQUEST, advertisement=True, sequence=1),
            dict(type='repository_request', request_id=C.REQUEST, advertisement=False, sequence=2),
            dict(type='repositories', request_id=C.REQUEST), dict(type='ready', request_id=C.REQUEST),
            dict(type='failed', request_id=C.REQUEST)]


class OfflineTests(unittest.TestCase):
    def idle_fixture(self, receipts, local_records):
        with tempfile.TemporaryDirectory() as temp:
            ns = Path(temp)
            payloads = {C.STATES[0]: {'jobs': []}, C.STATES[1]: {'records': receipts},
                        C.STATES[2]: {'schema_version': 1, 'records': local_records}}
            for name, value in payloads.items():
                C.write(ns / name, C.encoded(value))
            before = {name: (ns / name).read_bytes() for name in payloads}
            result = C.idle(ns)
            self.assertEqual({name: (ns / name).read_bytes() for name in payloads}, before)
            return result

    def test_idle_accepts_exact_legacy_failed_and_recorded_without_cleanup_verified(self):
        for status in ('failed', 'recorded'):
            record = receipt_record(status)
            self.assertNotIn('cleanup_verified', record['summary'])
            self.assertEqual(self.idle_fixture([record], ledger()['records']), {'jobs': []})

    def test_idle_receipt_unknown_active_and_local_only_statuses_refused(self):
        for status in ('unknown', 'cleanup_unknown', 'initializing', 'ready', 'dispatching',
                       'interrupted', 'completed', 'reconciled'):
            with self.subTest(status=status), self.assertRaises(C.Refused):
                self.idle_fixture([receipt_record(status)], ledger()['records'])

    def test_idle_receipt_requires_explicit_false_cleanup(self):
        for value in (True, None, 0):
            with self.subTest(cleanup_required=value), self.assertRaises(C.Refused):
                self.idle_fixture([receipt_record('failed', value)], ledger()['records'])
        record = receipt_record()
        del record['cleanup_required']
        with self.assertRaises(KeyError):
            self.idle_fixture([record], ledger()['records'])

    def test_idle_local_statuses_and_positive_cleanup_proof_remain_strict(self):
        original = ledger()['records'][1]
        for status in ('failed', 'completed', 'reconciled'):
            record = copy.deepcopy(original)
            record['summary']['status'] = status
            self.idle_fixture([], [record])
        for status in ('recorded', 'unknown', 'cleanup_unknown', 'running', 'ready', 'cloning', 'preparing'):
            record = copy.deepcopy(original)
            record['summary']['status'] = status
            with self.subTest(status=status), self.assertRaises(C.Refused):
                self.idle_fixture([], [record])
        for value in (False, None, 1):
            record = copy.deepcopy(original)
            record['summary']['cleanup_verified'] = value
            with self.subTest(cleanup_verified=value), self.assertRaises(C.Refused):
                self.idle_fixture([], [record])
        record = copy.deepcopy(original)
        del record['summary']['cleanup_verified']
        with self.assertRaises(KeyError):
            self.idle_fixture([], [record])
        record = copy.deepcopy(original)
        record['cleanup_required'] = True
        with self.assertRaises(C.Refused):
            self.idle_fixture([], [record])

    def test_idle_target_exception_is_local_only(self):
        self.idle_fixture([], [ledger()['records'][0]])
        # Same original request ID must not exempt an unsafe receipt ledger row.
        with self.assertRaises(C.Refused):
            self.idle_fixture([receipt_record('unknown')], [])

    def test_prepare_capture_refusal_retains_private_phase_code_without_effects(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            private = base / 'private'
            private.mkdir(mode=0o700)
            error = KeyError('sensitive root and prompt must not be copied')
            H = Mock()
            with patch.multiple(C, PRIVATE=private, PREP=base / 'prep', PROGRESS=base / 'progress', HOOK=base / 'hook'), \
                 patch.object(C, 'safe_directory') as directory, patch.object(C, 'source_check', return_value={}) as source, \
                 patch.object(C, 'capture', side_effect=error) as capture, patch.object(C, 'docker') as docker, \
                 patch('subprocess.run', side_effect=AssertionError('Host execution forbidden')):
                with self.assertRaises(KeyError) as caught:
                    C.prepare(H)
                self.assertIs(caught.exception, error)
                source.assert_called_once()
                capture.assert_called_once_with(H)
                directory.assert_called_once_with(private, private=True)
                H.acquire_lock.assert_not_called()
                docker.assert_not_called()
                self.assertFalse(any(p.exists() for p in (C.PREP, C.PROGRESS, C.HOOK)))
            files = list(private.iterdir())
            self.assertEqual(len(files), 1)
            self.assertTrue(files[0].name.startswith('prepare-refusal-'))
            self.assertEqual(files[0].stat().st_mode & 0o777, 0o600)
            self.assertEqual(C.decode(C.read(files[0])), {'mode': 'prepare', 'status': 'refused',
                             'phase': 'capture', 'code': 'state-field-missing', 'automatic_retry': False})
            self.assertNotIn(str(error), files[0].read_text())
            self.assertNotIn(str(base), files[0].read_text())

    def test_prepare_diagnostic_failure_never_masks_refusal_or_falls_back(self):
        error = C.Refused('private details must not be emitted')
        with patch.object(C, '_prepare', side_effect=error), patch.object(C, 'safe_directory'), \
             patch.object(C, 'write', side_effect=PermissionError('diagnostic unavailable')) as write:
            with self.assertRaises(C.Refused) as caught:
                C.prepare(Mock())
            self.assertIs(caught.exception, error)
            self.assertEqual(write.call_count, 1)
            self.assertEqual(write.call_args.args[0].parent, C.PRIVATE)
        with patch.object(C, 'safe_directory', side_effect=C.Refused('unsafe private root')), \
             patch.object(C, 'write') as write:
            C.record_prepare_failure('capture', error)
            write.assert_not_called()

    def test_prepare_diagnostic_codes_are_allowlisted_not_exception_text(self):
        cases = [(C.Refused('private'), 'binding-refused'), (AssertionError('private'), 'support-proof-refused'),
                 (C.subprocess.TimeoutExpired('private', 1), 'command-timeout'),
                 (PermissionError('private'), 'filesystem-error'), (KeyboardInterrupt(), 'interrupted'),
                 (ValueError('private'), 'unexpected-error')]
        with patch.object(C, 'safe_directory'), patch.object(C, 'write') as write:
            for error, code in cases:
                C.record_prepare_failure('source-check', error)
                data = C.decode(write.call_args.args[1])
                self.assertEqual(data['code'], code)
                self.assertEqual(data['phase'], 'source-check')
                self.assertNotIn('private', data.values())
            count = write.call_count
            C.record_prepare_failure('untrusted private path', ValueError('private'))
            self.assertEqual(write.call_count, count)

    def test_exact_two_field_diff_and_original_immutable(self):
        before = ledger()
        original = C.encoded(before)
        after = C.decode(C.updated_ledger(original))
        C.assert_record_diff(before, after)
        self.assertEqual(C.encoded(before), original)
        self.assertEqual(after['records'][1:], before['records'][1:])
        self.assertTrue(after['records'][0]['may_have_sent'])
        self.assertEqual(after['records'][0]['summary']['message'], 'Original uncertainty')

    def test_arbitrary_extra_delta_rejected(self):
        before = ledger()
        after = C.decode(C.updated_ledger(C.encoded(before)))
        mutations = [lambda x: x['records'][0].update(may_have_sent=False),
                     lambda x: x['records'][0]['summary'].update(message='rewritten'),
                     lambda x: x['records'][1].update(fingerprint='changed'),
                     lambda x: x.update(extra=True),
                     lambda x: x['records'].reverse()]
        for mutate in mutations:
            with self.subTest(mutate=mutate):
                variant = copy.deepcopy(after)
                mutate(variant)
                with self.assertRaises(C.Refused):
                    C.assert_record_diff(before, variant)

    def test_bad_original_state_refused(self):
        variants = [lambda r: r.update(cleanup_required=True),
                    lambda r: r.update(may_have_sent=False),
                    lambda r: r.update(failed_stage='cloning'),
                    lambda r: r.update(reply='invented'),
                    lambda r: r['summary'].update(session_id=None),
                    lambda r: r.update(repositories=[]),
                    lambda r: r['repositories'][0].update(commit='bad')]
        for mutate in variants:
            before = ledger()
            mutate(before['records'][0])
            with self.assertRaises(C.Refused):
                C.updated_ledger(C.encoded(before))

    def test_stable_reads_allow_atime_only_not_content_metadata_drift(self):
        with tempfile.TemporaryDirectory() as temp:
            p = Path(temp) / 'input'
            C.write(p, b'fixture')
            m = p.lstat()
            fields = ('st_dev', 'st_ino', 'st_uid', 'st_mode', 'st_nlink', 'st_size', 'st_mtime_ns', 'st_ctime_ns')
            changed = C.types.SimpleNamespace(**{name: getattr(m, name) for name in fields}, st_atime_ns=m.st_atime_ns + 1)
            with patch('os.fstat', side_effect=[m, changed]):
                self.assertEqual(C.read(p), b'fixture')
            changed.st_mtime_ns += 1
            with patch('os.fstat', side_effect=[m, changed]):
                with self.assertRaises(C.Refused):
                    C.read(p)

    def test_pinned_checkout_bytes_do_not_relax_artifact_safety(self):
        with tempfile.TemporaryDirectory() as temp:
            p = Path(temp) / 'checkout.py'
            p.write_bytes(b'# inert fixture\n')
            p.chmod(0o664)
            with self.assertRaises(C.Refused):
                C.read(p)
            self.assertEqual(C.pinned_source(p, C.sha(b'# inert fixture\n')), b'# inert fixture\n')
            with self.assertRaises(C.Refused):
                C.pinned_source(p, '0' * 64)

    def test_duplicate_json_and_schema_replay_refused(self):
        with self.assertRaises(C.Refused):
            C.decode(b'{"a":1,"a":2}')
        value = ledger()
        value['schema_version'] = 3
        with self.assertRaises(C.Refused):
            C.updated_ledger(C.encoded(value))

    def test_frame_sequence_no_inference_or_read_or_reply(self):
        good = b''.join(C.encoded(f) for f in frames())
        self.assertEqual(len(C.log_proof(good)), 5)
        for kind in ('model_request', 'read_request', 'reply', 'prompt', 'unknown'):
            bad = frames()
            bad[3]['type'] = kind
            with self.assertRaises(C.Refused):
                C.log_proof(b''.join(C.encoded(f) for f in bad))
        for bad in (good + C.encoded(frames()[0]), good[:-1], good.replace(C.REQUEST.encode(), b'foreign')):
            with self.assertRaises(C.Refused):
                C.log_proof(bad)

    def test_lock_proof_requires_flock_write_owner_inode_device_full_range(self):
        device = os.makedev(0, 34)
        good = 'pos:\t0\nlock:\t1: FLOCK ADVISORY WRITE 42 00:22:987 0 EOF\n'
        self.assertTrue(C.lock_line_proof(good, 42, device, 987))
        for bad in (good.replace('FLOCK', 'POSIX'), good.replace('WRITE', 'READ'),
                    good.replace('42 ', '41 '), good.replace('987', '986'), good.replace('00:22', '00:23'),
                    good.replace('EOF', '100'), good.replace('0 EOF', '1 EOF'), ''):
            self.assertFalse(C.lock_line_proof(bad, 42, device, 987))

    def test_parent_descriptor_proof_reads_only_exact_fake_proc_source(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            roots = {'--data-root': base / 'd', '--runtime-root': base / 'r'}
            proc_root = base / 'proc'
            proc = proc_root / '42'
            for p in (*roots.values(), proc_root, proc, proc / 'fd', proc / 'fdinfo', roots['--runtime-root'] / 'smith-next'):
                p.mkdir(mode=0o700)
            driver = base / 'driver'
            C.write(driver, b'fake driver')
            (proc / 'exe').symlink_to(driver)
            C.write(proc / 'cmdline', os.fsencode(driver) + b'\0--apply-approved-update\0')
            paths = [roots['--data-root'] / '.smith-next-install.lock', roots['--data-root'],
                     roots['--runtime-root'] / 'smith-next/runner.lock']
            for index, p in enumerate(paths):
                if index != 1:
                    C.write(p, b'')
                (proc / 'fd' / str(index)).symlink_to(p)
                m = p.lstat()
                line = f'lock:\t1: FLOCK ADVISORY WRITE 42 {os.major(m.st_dev):x}:{os.minor(m.st_dev):x}:{m.st_ino} 0 EOF\n'
                C.write(proc / 'fdinfo' / str(index), line.encode())
            progress = {'phase': 'driver-running', 'driver_pid': 42}
            with patch.object(C, 'DRIVER', driver), patch.object(C, 'DRIVER_SHA', C.sha(b'fake driver')), patch('os.getppid', return_value=42):
                self.assertEqual(len(C.parent_proof(roots, progress, proc_root)), 3)
                for field, value in [('phase', 'launching'), ('driver_pid', 43)]:
                    with self.assertRaises(C.Refused):
                        C.parent_proof(roots, dict(progress, **{field: value}), proc_root)
                info = proc / 'fdinfo/2'
                original = info.read_bytes()
                info.write_bytes(original.replace(b'WRITE', b'READ'))
                with self.assertRaises(C.Refused):
                    C.parent_proof(roots, progress, proc_root)
                info.write_bytes(original)
                (proc / 'fd/2').unlink()
                with self.assertRaises(C.Refused):
                    C.parent_proof(roots, progress, proc_root)
                (proc / 'fd/2').symlink_to(paths[2])
                info.unlink()
                with self.assertRaises(FileNotFoundError):
                    C.parent_proof(roots, progress, proc_root)

    def test_native_reader_fixture_uses_only_owned_serve_children(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            private, bundle, ns = [base / n for n in ('p', 'bundle', 'ns')]
            for p in (private, bundle, ns, bundle / 'payload', bundle / 'payload/service'):
                p.mkdir(mode=0o700)
            C.write(bundle / 'payload/service/smith-service', b'fake executable')
            C.write(ns / C.STATES[0], C.encoded({'jobs': []}))
            C.write(ns / C.STATES[1], C.encoded({'records': []}))
            original = C.encoded(ledger())
            C.write(ns / C.STATES[2], original)
            updated = C.updated_ledger(original)
            H = Mock()
            H.exchange.side_effect = [{'result': {'jobs': [{'request_id': C.REQUEST, 'status': status}]}}
                                      for status in ('unknown', 'reconciled')]
            children = []
            def spawn(argv, **kwargs):
                self.assertEqual(argv[1], 'serve')
                self.assertEqual(kwargs['env']['PATH'], '/nonexistent')
                self.assertNotIn('DBUS_SESSION_BUS_ADDRESS', kwargs['env'])
                self.assertFalse(kwargs['start_new_session'])
                child = Mock(pid=100 + len(children))
                child.poll.return_value = None
                children.append(child)
                return child
            with patch.multiple(C, PRIVATE=private, BUNDLE=bundle), patch('subprocess.Popen', side_effect=spawn), \
                 patch('subprocess.run', side_effect=AssertionError('No Docker or provider command')):
                C.validate_readers(H, Mock(), ns, original, updated)
            self.assertEqual(len(children), 2)
            for child in children:
                child.terminate.assert_called_once()
                child.wait.assert_called_once_with(timeout=2)
            self.assertEqual((ns / C.STATES[2]).read_bytes(), original)

    def test_captured_cgroup_enumeration_errors_and_descendants_fail_closed(self):
        _, R = C.support()
        properties = {'ActiveState': 'inactive', 'SubState': 'dead', 'MainPID': '0',
                      'Job': '0', 'NeedDaemonReload': 'no', 'DropInPaths': ''}
        with tempfile.TemporaryDirectory() as temp:
            group = Path(temp) / 'group'
            group.mkdir()
            child = group / 'child'
            child.mkdir()
            (group / 'cgroup.procs').write_text('')
            (child / 'cgroup.procs').write_text('')
            sockets = [Path(temp) / n for n in ('service.sock', 'app.sock')]
            with patch.object(R.H, 'property_value', side_effect=properties.__getitem__):
                R.stopped_proof(group, sockets)
                (child / 'cgroup.procs').write_text('99\n')
                with self.assertRaises(AssertionError):
                    R.stopped_proof(group, sockets)
                (child / 'cgroup.procs').write_text('')
                def enumeration_error(*args, **kwargs):
                    kwargs['onerror'](PermissionError('fixture enumeration refused'))
                with patch.object(R.os, 'walk', side_effect=enumeration_error):
                    with self.assertRaises(PermissionError):
                        R.stopped_proof(group, sockets)
                sockets[0].write_text('not absent')
                with self.assertRaises(AssertionError):
                    R.stopped_proof(group, sockets)

    def test_original_log_bindings_match_frozen_session_and_checkouts(self):
        values = frames()
        frozen = {'session_id': 'fixture_session', 'repositories': ledger()['records'][0]['repositories']}
        values[0]['repository'] = values[1]['repository'] = 0
        values[2]['repositories'] = frozen['repositories']
        values[3].update(session_id=frozen['session_id'], policy='smith-local-inspection-v1',
                         guard_sha256='40504069bc2548e7d1da3c9a47cecda3ee1be77e2be3a60bbdaac737e25ef310')
        values[4]['kind'] = 'local_worker_failed'
        payload = b''.join(C.encoded(v) for v in values)
        C.log_proof(payload, frozen)
        with self.assertRaises(C.Refused):
            C.log_proof(payload, dict(frozen, session_id='foreign'))
        with self.assertRaises(C.Refused):
            C.log_proof(payload, dict(frozen, repositories=[]))
        for index, key, value in ((0, 'advertisement', False), (1, 'advertisement', True),
                                  (0, 'repository', 1), (1, 'repository', 1), (2, 'repositories', []),
                                  (3, 'session_id', 'foreign'), (3, 'policy', 'foreign'),
                                  (3, 'guard_sha256', C.GUARD), (4, 'kind', 'foreign'),
                                  (4, 'request_id', 'foreign')):
            invalid = copy.deepcopy(values)
            invalid[index][key] = value
            with self.subTest(frame=index, key=key), self.assertRaises(C.Refused):
                C.log_proof(b''.join(C.encoded(f) for f in invalid), frozen)

    def test_retained_sequences_accept_one_two_and_reject_zero_one(self):
        actual = frames()
        self.assertEqual([f['sequence'] for f in actual[:2]], [1, 2])
        C.log_proof(b''.join(C.encoded(f) for f in actual))
        for pair in ((0, 1), (1, 1), (2, 3), (1, 3)):
            invalid = copy.deepcopy(actual)
            invalid[0]['sequence'], invalid[1]['sequence'] = pair
            with self.subTest(sequence=pair), self.assertRaises(C.Refused):
                C.log_proof(b''.join(C.encoded(f) for f in invalid))

    def test_container_and_owner_are_bound_to_full_ids_not_prefixes(self):
        container = {'Id': C.CONTAINER, 'Name': '/smith-dev-' + C.REQUEST, 'Image': C.IMAGE,
                     'Config': {'Labels': {'org.smith.dev.request': C.REQUEST,
                                          'org.smith.dev.kind': 'smith-receipt-worker-v1', 'org.smith.dev.owner': C.OWNER}},
                     'State': {'Status': 'exited', 'Running': False, 'Pid': 0,
                               'Paused': False, 'Restarting': False, 'Dead': False},
                     'RestartCount': 0, 'HostConfig': {'RestartPolicy': {'Name': 'no'}}}
        with patch.object(C, 'docker', side_effect=[(C.CONTAINER + '\n').encode(), C.encoded([container]), b'fixture']), \
             patch.object(C, 'log_proof') as logs:
            result = C.containers({'frozen': 'fixture'})
            self.assertEqual((result['id'], result['owner']), (C.CONTAINER, C.OWNER))
            logs.assert_called_once_with(b'fixture', {'frozen': 'fixture'})
        foreign = C.CONTAINER[:-1] + ('0' if C.CONTAINER[-1] != '0' else '1')
        with patch.object(C, 'docker', return_value=(foreign + '\n').encode()) as docker:
            with self.assertRaises(C.Refused):
                C.containers({})
            self.assertEqual(docker.call_count, 1)
        container['Config']['Labels']['org.smith.dev.owner'] = C.OWNER[:-1] + '0'
        with patch.object(C, 'docker', side_effect=[(C.CONTAINER + '\n').encode(), C.encoded([container])]):
            with self.assertRaises(C.Refused):
                C.containers({})

    def test_readonly_docker_allowlist_cannot_dispatch_mutations(self):
        result = Mock(returncode=0, stdout=b'fixture', stderr=b'')
        with patch.object(C.os.path, 'lexists', return_value=False), patch.object(C.subprocess, 'run', return_value=result) as run:
            self.assertEqual(readonly_docker(['image', 'inspect', C.RUNTIME_TAG]), b'fixture')
            self.assertEqual(run.call_args.args[0], ['/usr/bin/docker', '--config', '/nonexistent/smith-docker-config',
                                                     '--host', 'unix:///run/docker.sock', 'image', 'inspect', C.RUNTIME_TAG])
            self.assertEqual(run.call_args.kwargs['env'], C.ENV)
            for args in (['image', 'tag', C.IMAGE, C.BACKUP_TAG], ['build', '.'], ['run', C.IMAGE],
                         ['container', 'start', C.CONTAINER], ['container', 'inspect', 'foreign']):
                with self.assertRaises(C.Refused):
                    readonly_docker(args)
            self.assertEqual(run.call_count, 1)

    def test_phase_replay_and_invocation_substitution(self):
        p = {'phase': 'driver-running', 'invocation_id': 'fixture'}
        self.assertEqual(C.phase(p, 'driver-running', phase='hook-running')['phase'], 'hook-running')
        with self.assertRaises(C.Refused):
            C.phase(p, 'hook-running', phase='image-publish-attempted')
        with self.assertRaises(C.Refused):
            C.phase(p, 'driver-running', invocation_id='foreign')

    def test_recipe_only_offline_binary_guard_help(self):
        recipe = C.recipe().decode()
        self.assertTrue(recipe.startswith('FROM ' + C.BACKUP_TAG + '\nUSER 0\n'))
        self.assertEqual(recipe.count('COPY --chown=0:0'), 2)
        self.assertIn(C.NEW, recipe)
        self.assertIn(C.GUARD, recipe)
        self.assertIn('chmod 0444', recipe)
        self.assertIn('smith-service --help', recipe)
        self.assertTrue(recipe.endswith('USER 1000:1000\n'))
        self.assertIn('stat -c "%u:%g:%a" /usr/local/bin/smith-service | grep -Fx "0:0:555"', recipe)
        self.assertIn('stat -c "%u:%g:%a" /etc/smith-opencode/guard.mjs | grep -Fx "0:0:444"', recipe)
        self.assertNotIn('$(', recipe)
        self.assertNotIn('`', recipe)
        for forbidden in ('curl ', 'apt-get', 'git ', 'opencode ', 'dev-local-worker', 'models'):
            self.assertNotIn(forbidden, recipe)

    def test_image_config_refuses_any_runtime_configuration_delta(self):
        config = {'User': '1000:1000', 'Entrypoint': ['/usr/local/bin/smith-service', 'dev-local-worker'],
                  'WorkingDir': '/workspace', 'Env': ['FIXED=1'], 'Cmd': None}
        old = {'Config': config}
        C.image_config(old, copy.deepcopy(old))
        for field, value in [('User', '0'), ('Env', ['OTHER=1']), ('Volumes', {'/host': {}}),
                             ('Shell', ['/bin/bash']), ('OnBuild', ['RUN bad']), ('WorkingDir', '/')]:
            new = copy.deepcopy(old)
            new['Config'][field] = value
            with self.assertRaises(C.Refused):
                C.image_config(old, new)

    def run_fake_hook(self, failure=None):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            private, evidence, namespace = [base / n for n in ('private', 'evidence', 'namespace')]
            for p in (private, evidence, namespace, private / 'artifacts-before'):
                p.mkdir(mode=0o700)
            for name, value in {C.STATES[0]: {'jobs': []}, C.STATES[1]: {'records': []}, C.STATES[2]: ledger()}.items():
                C.write(namespace / name, C.encoded(value))
            binding = {'before': {'states': C.state_hashes(namespace), 'group': str(base / 'fake-group')},
                       'new_image': {'Id': 'sha256:new'}}
            progress = evidence / 'progress.json'
            hook = evidence / 'hook.json'
            C.write(progress, C.encoded({'phase': 'driver-running', 'invocation_id': 'fixture', 'driver_pid': 42}))
            original = (namespace / C.STATES[2]).read_bytes()
            events = []
            def stopped(*args, **kwargs):
                events.append('stopped-proof')
                if failure == 'second-proof' and len(events) == 2:
                    raise C.Refused('drift')
                return {'--runtime-root': base}, namespace, []
            def tag(args):
                events.append('tag-attempt')
                self.assertEqual(C.decode(C.read(progress))['phase'], 'image-publish-attempted')
                if failure == 'tag':
                    raise C.Refused('failed')
                return b''
            def publish_ledger(path, data, mode=0o600):
                if path.name.endswith('coordinated-staging'):
                    events.append('ledger-attempt')
                    self.assertEqual(C.decode(C.read(progress))['phase'], 'ledger-publish-attempted')
                    if failure == 'ledger':
                        raise C.Refused('failed')
                return real_write(path, data, mode)
            real_write = C.write
            with patch.multiple(C, PRIVATE=private, PROGRESS=progress, HOOK=hook), \
                 patch.object(C, 'load_binding', return_value=binding), \
                 patch.object(C, 'stopped_check', side_effect=stopped), \
                 patch.object(C, 'validate_readers') as readers, \
                 patch.object(C, 'parent_proof', return_value=[]), \
                 patch.object(C, 'docker', side_effect=tag), \
                 patch.object(C, 'image', return_value={'Id': 'sha256:new'}), \
                 patch.object(C, 'write', side_effect=publish_ledger), \
                 patch('subprocess.run', side_effect=AssertionError('Host tool forbidden')), \
                 patch('subprocess.Popen', side_effect=AssertionError('Native child forbidden')):
                if failure:
                    with self.assertRaises(C.Refused):
                        C.stopped_hook(Mock(), Mock())
                    self.assertFalse(hook.exists())
                    self.assertEqual((namespace / C.STATES[2]).read_bytes(), original)
                else:
                    C.stopped_hook(Mock(), Mock())
                    result = C.decode(C.read(hook))
                    self.assertEqual((result['status'], result['request_id'], result['candidate_sha256']),
                                     ('reconciled', C.REQUEST, C.NEW))
                    C.assert_record_diff(C.decode(original), C.decode(C.read(namespace / C.STATES[2])))
                    self.assertEqual(C.decode(C.read(progress))['phase'], 'reconciled')
                self.assertEqual(readers.call_count, 1)
                self.assertEqual((private / 'before-transition' / C.STATES[2]).read_bytes(), original)
                return events

    def test_hook_success_stops_rechecks_then_two_publications(self):
        self.assertEqual(self.run_fake_hook(), ['stopped-proof', 'stopped-proof', 'tag-attempt', 'stopped-proof', 'ledger-attempt'])

    def test_hook_drift_before_effects(self):
        self.assertEqual(self.run_fake_hook('second-proof'), ['stopped-proof', 'stopped-proof'])

    def test_image_failure_leaves_no_ledger_or_success_receipt(self):
        self.assertEqual(self.run_fake_hook('tag').count('tag-attempt'), 1)

    def test_ledger_failure_preserves_partial_image_effect_no_rollback(self):
        self.assertEqual(self.run_fake_hook('ledger'), ['stopped-proof', 'stopped-proof', 'tag-attempt', 'stopped-proof', 'ledger-attempt'])

    def test_v2_actual_record_keys_and_production_only_membership(self):
        result = C.decode(C.pinned_source(SCRIPT.parent / 'coordinated-candidate-v2-aligned-result.json', C.V2_RESULT))
        validation = C.decode(C.pinned_source(SCRIPT.parent / 'coordinated-candidate-v2-validation.json', C.V2_VALIDATION))
        sources = C.v2_records(result, validation)
        self.assertEqual(len(sources), 132)
        self.assertEqual(C.production_paths(), set(sources))
        self.assertFalse(any(n.startswith(('docs/', 'client/test/')) for n in sources))
        validation['historical_document_references_sha256_not_freshness_inputs'] = {'historical': 'not a gate'}
        self.assertEqual(C.v2_records(result, validation), sources)
        validation['source_sha256'] = dict(sources, **{'service/src/lib.rs': '0' * 64})
        with self.assertRaises(C.Refused):
            C.v2_records(result, validation)

    def test_missing_rebuilt_driver_pin_disables_modes_and_inventory_is_explicit(self):
        with patch.object(C, 'DRIVER_SHA', None):
            with self.assertRaises(C.Refused):
                C.driver_pin()
        self.assertEqual(C.parse_mode(['apply']), ('apply', False))
        self.assertEqual(C.parse_mode(['apply', '--inventory-models']), ('apply', True))
        for args in ([], ['prepare', '--inventory-models'], ['stopped-hook', '--inventory-models'],
                     ['apply', '--yes'], ['apply', '--inventory-models', '--again']):
            with self.assertRaises(C.Refused):
                C.parse_mode(args)

    def fake_supervisor(self, failure=None):
        events = []
        cancelled = [failure == 'pre-cancel']
        def started(pid):
            events.append(('bound', pid))
            if failure == 'binding':
                raise C.Refused('fake binding failure')
        def wait(*args):
            if failure == 'timeout':
                raise C.Refused('fake deadline')
            if failure == 'interrupt':
                raise KeyboardInterrupt()
            if failure == 'cancel':
                cancelled[0] = True
                raise C.Refused('fake cancellation')
            return 0
        def join(pid, group):
            events.append(('cancel-joined', pid, group))
            return 15
        with patch.object(C.os, 'pipe', side_effect=[(10, 11), (12, 13)]), \
             patch.object(C.os, 'fork', return_value=42), patch.object(C.os, 'getpgid', return_value=42), \
             patch.object(C.os, 'close'), patch.object(C.os, 'read', return_value=b'1'), \
             patch.object(C.os, 'write', side_effect=lambda *a: events.append(('gate',))), \
             patch.object(C.select, 'select', return_value=([] if failure == 'handshake' else [12], [], [])), \
             patch.object(C, 'cancellation_signals', return_value=C.contextlib.nullcontext(cancelled)), \
             patch.object(C, 'wait_owned', side_effect=wait), patch.object(C, 'cancel_join', side_effect=join), \
             patch.object(C, 'write', side_effect=lambda p, b: events.append(('receipt', C.decode(b)))):
            if failure:
                with self.assertRaises((C.Refused, KeyboardInterrupt)):
                    C.supervise_driver(started)
            else:
                C.supervise_driver(started)
        return events

    def test_native_outer_timeout_interrupt_and_cancel_join_before_returning(self):
        for failure in ('timeout', 'interrupt', 'cancel', 'binding', 'pre-cancel', 'handshake'):
            with self.subTest(failure=failure):
                events = self.fake_supervisor(failure)
                joins = [e for e in events if e[0] == 'cancel-joined']
                self.assertEqual(joins, [('cancel-joined', 42, failure != 'handshake')])
                self.assertEqual(events[-1][0], 'receipt')
                self.assertEqual(events[-1][1]['status'], 'cancelled-joined')
                self.assertTrue(events[-1][1]['partial_effects_retained'])
                if failure in ('binding', 'pre-cancel', 'handshake'):
                    self.assertNotIn(('gate',), events)

    def test_native_success_is_joined_before_post_checks(self):
        events = self.fake_supervisor()
        self.assertEqual([e[0] for e in events], ['bound', 'gate', 'receipt'])
        self.assertEqual(events[-1][1]['status'], 'joined')

    def test_cancel_signals_only_owned_child_or_verified_owned_group_and_joins(self):
        for ready in (False, True):
            with patch.object(C.os, 'waitpid', side_effect=[(0, 0), InterruptedError(), (42, 15)]) as wait, \
                 patch.object(C.os, 'killpg') as group, patch.object(C.os, 'kill') as child:
                self.assertEqual(C.cancel_join(42, ready), 15)
                (group if ready else child).assert_called_once_with(42, C.signal.SIGTERM)
                (child if ready else group).assert_not_called()
                self.assertTrue(all(call.args[0] == 42 for call in wait.call_args_list))
        with patch.object(C.os, 'waitpid', return_value=(42, 0)), \
             patch.object(C.os, 'killpg') as group, patch.object(C.os, 'kill') as child:
            C.cancel_join(42, True)
            group.assert_not_called()
            child.assert_not_called()

    def test_wait_deadline_and_cancellation_are_not_silent_detach(self):
        with patch.object(C.os, 'waitpid', return_value=(0, 0)), patch.object(C.time, 'monotonic', return_value=80):
            with self.assertRaises(C.Refused):
                C.wait_owned(42, 75, [False])
            with self.assertRaises(C.Refused):
                C.wait_owned(42, 100, [True])

    def test_parent_death_signal_has_parent_identity_race_check(self):
        libc = Mock()
        libc.prctl.return_value = 0
        with patch.object(C.ctypes, 'CDLL', return_value=libc), patch.object(C.os, 'getppid', return_value=42):
            C.parent_death_guard(42)
            libc.prctl.assert_called_once_with(1, int(C.signal.SIGTERM), 0, 0, 0)
            with self.assertRaises(C.Refused):
                C.parent_death_guard(43)

    def fake_inventory(self, failure=None):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            private, data, runtime = [base / n for n in ('p', 'd', 'r')]
            ns = data / 'smith-next'
            for p in (private, data, runtime, ns):
                p.mkdir(mode=0o700)
            before = {'jobs': [], 'revision': 7, 'config': {'model': 'fixture', 'revision': 3}, 'sentinel': 'preserved'}
            C.write(ns / C.STATES[0], C.encoded(before))
            C.write(ns / C.STATES[1], C.encoded({'records': []}))
            C.write(ns / C.STATES[2], C.updated_ledger(C.encoded(ledger())))
            H = Mock()
            H.app_request.side_effect = lambda: C.app_request('dev_jobs_state_v2', {})
            calls, dispatched = [], []
            poll_count = [0]
            def exchange(socket, req, pid):
                calls.append(req)
                op = req['op']
                self.assertIn(op, ('state', 'dev_jobs_state_v2', 'models', 'poll'))
                self.assertEqual(pid, 42)
                result = {}
                if op == 'state':
                    result = {'config': {'model': 'fixture', 'revision': '3'}}
                elif op == 'dev_jobs_state_v2':
                    result = {'model': {'name': 'fixture', 'sha256': 'a' * 64,
                                         'config_revision': '3', 'ready': bool(dispatched)}}
                elif op == 'models':
                    request_id = req['args']['request_id']
                    dispatched.append(request_id)
                    operation = {'request_id': request_id, 'kind': 'models', 'status': 'succeeded',
                                 'phase': 'complete', 'error': None, 'result': {'models': []}, 'config_revision': 3}
                    job = {'operation': operation, 'version': 2, 'task': 'Models',
                           'conversation_id': None, 'history_revision': None}
                    after = dict(copy.deepcopy(before), revision=9, jobs=[job])
                    if failure == 'app-drift':
                        after['config']['model'] = 'foreign'
                    (ns / C.STATES[0]).write_bytes(C.encoded(after))
                    if failure == 'ambiguous-models':
                        raise C.Refused('ambiguous metadata dispatch')
                    result = dict(operation, config_revision='3')
                elif op == 'poll':
                    self.assertEqual(req['args'], {'request_id': dispatched[0], 'instance_id': 'instance_fixture'})
                    poll_count[0] += 1
                    result = {'request_id': dispatched[0], 'kind': 'models', 'status': 'succeeded',
                              'phase': 'complete', 'error': None, 'result': {'models': []}, 'config_revision': '3'}
                    if poll_count[0] == 1 and failure != 'foreign-poll':
                        result['status'] = 'running'
                return {'instance_id': 'foreign' if op == 'poll' and failure == 'foreign-poll' else 'instance_fixture',
                        'result': result}
            H.exchange.side_effect = exchange
            clock = patch.object(C.time, 'monotonic', side_effect=[0, 13]) if failure == 'poll-timeout' else C.contextlib.nullcontext()
            with patch.object(C, 'PRIVATE', private), patch.object(C.time, 'sleep'), clock, \
                 patch('subprocess.run', side_effect=AssertionError('No host tools')), \
                 patch('subprocess.Popen', side_effect=AssertionError('No process launch')):
                if failure:
                    with self.assertRaises(C.Refused):
                        C.metadata_inventory(H, {'--data-root': data, '--runtime-root': runtime}, 42)
                else:
                    receipt = C.metadata_inventory(H, {'--data-root': data, '--runtime-root': runtime}, 42)
                    self.assertTrue(receipt['model_ready'])
                    self.assertEqual(receipt['app_delta'], 'one-models-receipt-and-two-revisions')
                    self.assertNotEqual(receipt['app_before_sha256'], receipt['app_after_sha256'])
                saved = C.decode(C.read(private / 'metadata-inventory.json'))
                self.assertEqual(saved['request_id'], dispatched[0])
                self.assertEqual(saved['status'], 'dispatch-attempted' if failure else 'succeeded')
                self.assertEqual(C.decode(C.read(private / 'metadata-app-before.json')), before)
                self.assertEqual(len(dispatched), 1)
                return [req['op'] for req in calls]

    def test_metadata_inventory_one_dispatch_original_id_polls_no_inference(self):
        calls = self.fake_inventory()
        self.assertEqual(calls.count('models'), 1)
        self.assertEqual(calls.count('poll'), 2)

    def test_metadata_ambiguity_foreign_poll_and_config_drift_no_resend(self):
        for failure in ('ambiguous-models', 'foreign-poll', 'app-drift', 'poll-timeout'):
            with self.subTest(failure=failure):
                self.assertEqual(self.fake_inventory(failure).count('models'), 1)

    def test_apply_supervision_failure_never_reaches_post_checks_or_inventory(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            roots = {'--data-root': str(base / 'd'), '--config-root': str(base / 'c'), '--runtime-root': str(base / 'r')}
            before = {'roots': roots, 'target_proof': {}}
            binding = {'before': before, 'container': 'fixture', 'new_image': {'Id': 'sha256:new'}}
            H = Mock()
            with patch.dict(C.os.environ, {'XDG_DATA_HOME': roots['--data-root'],
                                          'XDG_CONFIG_HOME': roots['--config-root'], 'XDG_RUNTIME_DIR': roots['--runtime-root']}), \
                 patch.multiple(C, PROGRESS=base / 'progress', HOOK=base / 'hook'), \
                 patch.object(C, 'load_binding', return_value=binding), patch.object(C, 'capture', return_value=before), \
                 patch.object(C, 'containers', return_value='fixture'), \
                 patch.object(C, 'image', side_effect=lambda name: {'Id': 'sha256:new' if name == C.CANDIDATE_TAG else C.IMAGE}), \
                 patch.object(C, 'supervise_driver', side_effect=C.Refused('cancelled and joined')) as supervise, \
                 patch.object(C, 'metadata_inventory') as inventory, patch.object(C, 'docker') as docker:
                with self.assertRaises(C.Refused):
                    C.apply(H, inventory=True)
                supervise.assert_called_once()
                inventory.assert_not_called()
                H.exchange.assert_not_called()
                H.manager.assert_not_called()
                docker.assert_not_called()

    def test_cancellation_handler_latches_and_restores_both_signals(self):
        originals = [object(), object()]
        with patch.object(C.signal, 'signal', side_effect=[*originals, None, None]) as register, \
             patch.object(C.signal, 'pthread_sigmask', side_effect=[{C.signal.SIGTERM}, None]) as mask:
            with C.cancellation_signals() as cancelled:
                handler = register.call_args_list[0].args[1]
                handler(C.signal.SIGTERM, None)
                self.assertTrue(cancelled[0])
                handler(C.signal.SIGINT, None)
            self.assertEqual(register.call_args_list[-2].args, (C.signal.SIGINT, originals[0]))
            self.assertEqual(register.call_args_list[-1].args, (C.signal.SIGTERM, originals[1]))
            self.assertEqual(mask.call_args_list[0].args, (C.signal.SIG_UNBLOCK, {C.signal.SIGINT, C.signal.SIGTERM}))
            self.assertEqual(mask.call_args_list[-1].args, (C.signal.SIG_SETMASK, {C.signal.SIGTERM}))

    def test_source_has_fixed_native_path_and_no_manager_bypass(self):
        source = SCRIPT.read_text()
        tree = ast.parse(source)
        self.assertIn("'--apply-approved-update'", source)
        self.assertNotIn('update_worker_image.py', source)
        self.assertNotIn('H.manager(', source)
        self.assertNotIn('R.main(', source)
        self.assertNotIn('H.main(', source)
        self.assertNotIn('dev_job_start', source)
        self.assertIn("'service.sock', 'app.sock'", source)
        self.assertIn('source_check() == binding[', source)
        self.assertTrue(any(isinstance(n, ast.FunctionDef) and n.name == 'main' for n in tree.body))


def readonly_docker(args, timeout=10):
    # Explicit opt-in observer below only; ordinary unittest discovery never runs
    # live Docker. Do not replace the operational coordinator's private config.
    allowed = {
        ('container', 'ls', '--all', '--no-trunc', '--filter', 'label=org.smith.dev.request=' + C.REQUEST, '--format', '{{.ID}}'),
        ('container', 'inspect', C.CONTAINER), ('container', 'logs', C.CONTAINER),
        ('image', 'inspect', C.RUNTIME_TAG), ('image', 'inspect', C.IMAGE),
        ('image', 'ls', '--format', '{{.Repository}}:{{.Tag}}'),
    }
    C.require(tuple(args) in allowed, 'Read-only Docker command not allowlisted')
    C.require(not C.os.path.lexists('/nonexistent/smith-docker-config'))
    result = C.subprocess.run(['/usr/bin/docker', '--config', '/nonexistent/smith-docker-config',
                               '--host', 'unix:///run/docker.sock', *args], env=C.ENV,
                              stdin=C.subprocess.DEVNULL, stdout=C.subprocess.PIPE, stderr=C.subprocess.PIPE,
                              timeout=timeout)
    C.require(result.returncode == 0 and not result.stderr and len(result.stdout) <= 16 * 1024 * 1024,
              'Read-only Docker observation failed')
    return result.stdout


def readonly_preflight():
    """Explicitly authorized observation, not a CLI mode or an ordinary test.

    Print only fixed check names and public artifact identities. Never emit raw
    capture/inspect/log data or write a result, snapshot, config or diagnostic.
    """
    checks = []
    phase = 'private-root-and-absence'
    def forbidden(*args, **kwargs):
        raise C.Refused('Mutation forbidden in read-only preflight')
    try:
        C.safe_directory(C.PRIVATE, private=True)
        private_before = C.file_identity(C.PRIVATE.lstat())
        C.require(not any(C.PRIVATE.iterdir()))
        C.require(not any(C.os.path.lexists(p) for p in (C.PREP, C.PROGRESS, C.HOOK)))
        checks.append(phase)
        H, _ = C.support()
        with patch.object(C, 'docker', side_effect=readonly_docker) as docker, \
             patch.object(C, 'write', side_effect=forbidden), patch.object(C, 'publish', side_effect=forbidden), \
             patch.object(C, 'prepare', side_effect=forbidden), patch.object(C, 'apply', side_effect=forbidden), \
             patch.object(C, 'stopped_hook', side_effect=forbidden), patch.object(Path, 'mkdir', side_effect=forbidden), \
             patch.object(H, 'acquire_lock', side_effect=forbidden), patch.object(H, 'manager', side_effect=forbidden):
            phase = 'source-candidate-driver'
            sources = C.source_check()
            checks.append(phase)
            phase = 'capture-pair-process-unit-roots-ledgers-peer'
            before = C.capture(H)
            checks.append(phase)
            phase = 'container-full-id-owner-state-and-five-original-frames'
            container = C.containers(before['target_proof'])
            checks.append(phase)
            phase = 'retained-image-config-and-reserved-tags'
            old = C.image(C.RUNTIME_TAG)
            C.require(old['Id'] == C.IMAGE)
            C.image_config(old, old)
            tags = C.docker(['image', 'ls', '--format', '{{.Repository}}:{{.Tag}}']).decode().splitlines()
            C.require(C.BACKUP_TAG not in tags and C.CANDIDATE_TAG not in tags)
            checks.append(phase)
            phase = 'preservation-inputs-and-build-inputs-readable'
            ns = Path(before['roots']['--data-root']) / 'smith-next'
            artifacts = ((ns / 'bin/smith-service', C.OLD), (Path(before['unit']['unit']), C.UNIT),
                         (Path(before['release']) / '.smith-next-manifest.json', C.SELECTED),
                         (Path(before['release']) / 'smith_backend/smith-service', C.OLD),
                         (C.BUNDLE / 'payload/service/smith-service', C.NEW))
            for path, expected in artifacts:
                C.require(C.sha(C.read(path)) == expected)
            C.pinned_source(C.ROOT / 'service/dev-worker/guard.mjs', C.GUARD)
            C.require(b'$(' not in C.recipe() and b'`' not in C.recipe())
            checks.append(phase)
            phase = 'final-readonly-rechecks'
            C.require(C.source_check() == sources and C.capture(H) == before)
            C.require(C.containers(before['target_proof']) == container and C.image(C.RUNTIME_TAG)['Id'] == C.IMAGE)
            C.require(C.docker(['image', 'ls', '--format', '{{.Repository}}:{{.Tag}}']).decode().splitlines() == tags)
            C.require(C.file_identity(C.PRIVATE.lstat()) == private_before and not any(C.PRIVATE.iterdir()))
            C.require(not any(C.os.path.lexists(p) for p in (C.PREP, C.PROGRESS, C.HOOK)))
            checks.append(phase)
            report = {'status': 'PASS', 'checks': checks, 'docker_readonly_calls': docker.call_count,
                      'request_id': C.REQUEST, 'container_id': container['id'], 'owner': container['owner'],
                      'original_frame_types': ['repository_request', 'repository_request', 'repositories', 'ready', 'failed'],
                      'original_repository_sequences': [1, 2], 'model_read_reply_frames': 0,
                      'private_directory_unchanged_empty': True, 'prep_progress_hook_absent': True,
                      'all_current_readonly_prepare_predicates_passed': True,
                      'deferred_effect_dependent_proofs': ['held-leases', 'image-build-and-new-image-validation',
                                                          'stopped-hook-quiescence-and-native-readers', 'publication-and-restart'],
                      'coordinator_mode_run': False, 'mutation_executed': False}
        print(C.encoded(report).decode(), end='')
        return report
    except BaseException:
        print(C.encoded({'status': 'FAIL', 'phase': phase, 'checks_completed': checks,
                         'details': 'withheld; no mutation or automatic retry'}).decode(), end='')
        raise SystemExit(1) from None


if __name__ == '__main__':
    unittest.main(verbosity=2)
