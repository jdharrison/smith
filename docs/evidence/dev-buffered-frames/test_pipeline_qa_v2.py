"""Fixtures plus isolated candidate metadata regression; no model/Docker/clone calls."""
import copy
import fcntl
import importlib.util
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location('isolated_pipeline_qa_v2', Path(__file__).with_name('pipeline-qa-v2.py'))
Q = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(Q)
RUN = 'a' * 32


class FakeApp:
    instance = 'b' * 32

    def __init__(self, fail=None):
        self.calls = []
        self.fail = fail

    def call(self, op, args):
        self.calls.append((op, copy.deepcopy(args)))
        if op == self.fail:
            raise RuntimeError('lost-ack')
        return {
            'state': {'config': {'revision': '0'}},
            'models': {}, 'poll': {'status': 'succeeded'},
            'configure_checked': {'model': Q.MODEL, 'voice': None, 'revision': '1'},
            'projects_list_v3': {'revision': 1, 'projects': []},
            'project_create_v3': {'project_id': 'c' * 32, 'revision': 2},
            'dev_jobs_state_v2': {'jobs': [], 'model': {'name': Q.MODEL, 'sha256': Q.MODEL_SHA,
                                                       'ready': True, 'config_revision': '1'}},
            'dev_job_start_v2': {},
            'dev_job_get_v2': {'job': {'request_id': Q.ids(RUN)['job'], 'status': 'completed'}},
        }[op]


def frames():
    common = {'request_id': Q.ids(RUN)['job']}
    return [{**common, **frame} for frame in (
        {'type': 'ready'}, {'type': 'model_request'},
        {'type': 'read_request', 'call_id': 'read1',
         'arguments': {'filePath': '/workspace/repos/r0/README'}},
        {'type': 'read_result', 'call_id': 'read1', 'text': 'Hello World!'},
        {'type': 'model_request'}, {'type': 'reply', 'text': 'Hello World!', 'session_id': 'ses_fixture'})]


RESULT = {'reply': 'Hello World!', 'job': {'session_id': 'ses_fixture'}}


def encoded_frames(value):
    return b''.join(Q.M.encoded(frame) for frame in value)


class ContractTests(unittest.TestCase):
    def test_actual_private_socket_schema_and_pid_guard(self):
        for wrong_pid in (False, True):
            with tempfile.TemporaryDirectory() as temporary:
                path = Path(temporary, 'app.sock')
                listener = Q.socket.socket(Q.socket.AF_UNIX, Q.socket.SOCK_STREAM)
                listener.bind(str(path)); path.chmod(0o600); listener.listen(1); listener.settimeout(3)
                requests, errors = [], []
                def server():
                    try:
                        with listener.accept()[0] as stream:
                            stream.settimeout(3)
                            data = bytearray()
                            while not data.endswith(b'\n'):
                                part = stream.recv(8192)
                                if not part:
                                    return
                                data.extend(part)
                            request = Q.M.decode(data)
                            requests.append(request)
                            stream.sendall(Q.M.encoded({'schema_version': 3, 'service': 'smith-next',
                                'protocol': 'app', 'pid': os.getpid(), 'instance_id': 'b' * 32,
                                'call_id': request['call_id'], 'ok': True, 'result': {'fixture': True}, 'error': None}))
                    except Exception as error:
                        errors.append(type(error).__name__)
                thread = Q.threading.Thread(target=server)
                thread.start()
                try:
                    app = Q.App(path, os.getpid() + int(wrong_pid), lambda: None)
                    if wrong_pid:
                        with self.assertRaises(RuntimeError):
                            app.call('state', {})
                    else:
                        self.assertEqual(app.call('state', {}), {'fixture': True})
                        self.assertEqual(requests[0]['schema_version'], 3)
                        self.assertEqual(requests[0]['op'], 'state')
                finally:
                    thread.join(4)
                    listener.close()
                self.assertFalse(thread.is_alive())
                self.assertEqual(errors, [])
                self.assertEqual(len(requests), 0 if wrong_pid else 1)

    def test_source_rebind_includes_installer_and_rejects_caller_drift(self):
        digest, inputs = Q.source_snapshot()
        self.assertEqual(digest, Q.SOURCE_SNAPSHOT)
        self.assertIn('installer/src/catalog.rs', inputs)
        read = Q.M.read
        def changed(path, *args, **kwargs):
            data = read(path, *args, **kwargs)
            return data + b'\n' if path == Q.ROOT / 'installer/src/catalog.rs' else data
        with mock.patch.object(Q.M, 'read', side_effect=changed):
            self.assertNotEqual(Q.source_snapshot()[0], Q.SOURCE_SNAPSHOT)

    def test_allowlisted_refusal_discards_message_and_unknown_code(self):
        for code in ('INVALID_REQUEST', 'MODEL_UNAVAILABLE', 'STATE_UNAVAILABLE'):
            error = Q.AppRefusal('project_create_v3', {'code': code, 'message': 'private-path prompt secret'})
            self.assertEqual(Q.refusal_fields(error), {'failed_operation': 'project_create_v3', 'error_code': code})
            self.assertNotIn('private-path', str(error))
        for error in ({'code': 'PRIVATE_VALUE', 'message': 'secret'}, {'code': []}, [], None):
            fields = Q.refusal_fields(Q.AppRefusal('untrusted private op', error))
            self.assertEqual(fields, {'failed_operation': 'unknown', 'error_code': 'UNRECOGNIZED_APP_ERROR'})
        self.assertEqual(Q.refusal_fields(RuntimeError('private exception message')),
                         {'error_code': 'HARNESS_REFUSED_OR_INTERRUPTED'})

    def test_allowlisted_code_survives_supervisor_result_without_raw_error(self):
        with tempfile.TemporaryDirectory() as temporary:
            error = Q.AppRefusal('project_create_v3', {'code': 'INVALID_REQUEST', 'message': 'private prompt secret'})
            with mock.patch.object(Q, 'static_check', side_effect=error), \
                 mock.patch.object(Q, 'live_check', side_effect=AssertionError('must not dispatch')):
                Q.supervise(RUN, Path(temporary))
            raw = Path(temporary, 'result.json').read_bytes()
            result = Q.M.decode(raw)
            self.assertEqual(result['status'], 'incomplete-preserved')
            self.assertEqual(result['error_code'], 'INVALID_REQUEST')
            self.assertEqual(result['failed_operation'], 'project_create_v3')
            self.assertNotIn(b'private prompt', raw)

    def test_v1_reviews_cannot_authorize_v2(self):
        self.assertEqual(Q.TESTS.name, 'test_pipeline_qa_v2.py')
        self.assertEqual(Q.specification(RUN)['harness_revision'], 2)
        names = []
        def missing(path, *args, **kwargs):
            names.append(path.name)
            raise FileNotFoundError
        with mock.patch.object(Q.M, 'read', side_effect=missing), self.assertRaises(FileNotFoundError):
            Q.review(RUN, {})
        self.assertEqual(names, ['pipeline-qa-v2-source-review.json'])

    def test_import_and_spec_have_no_effects(self):
        with mock.patch.object(Q, 'docker', side_effect=AssertionError), \
             mock.patch.object(Q.subprocess, 'Popen', side_effect=AssertionError), \
             mock.patch.object(Q.M, 'write', side_effect=AssertionError):
            value = Q.specification(RUN)
        self.assertFalse(value['installed_writes'])
        self.assertEqual(value['worker_image'], Q.IMAGE)
        self.assertEqual(value['timestamp_gate'], 'NO-GO-unwaived')

    def test_identity_deterministic_and_disjoint(self):
        first = Q.ids(RUN)
        self.assertEqual(first, Q.ids(RUN))
        self.assertEqual(len(set(first.values())), 3)
        self.assertTrue(set(first.values()).isdisjoint(Q.ids('d' * 32).values()))

    def test_invalid_identity_refused(self):
        for value in ('', 'a' * 31, 'A' * 32, '../bad', None):
            with self.subTest(value=value), self.assertRaises(RuntimeError):
                Q.ids(value)

    def test_no_credentials_or_proxy_in_environment(self):
        with mock.patch.dict(os.environ, {'OPENAI_API_KEY': 'secret', 'HTTPS_PROXY': 'bad',
                                         'DOCKER_HOST': 'bad', 'GIT_ASKPASS': 'bad'}):
            env = Q.environment()
        for key in ('OPENAI_API_KEY', 'HTTPS_PROXY', 'DOCKER_HOST', 'GIT_ASKPASS'):
            self.assertNotIn(key, env)

    def test_pipeline_exact_public_api_and_one_dispatch(self):
        app = FakeApp()
        attempts = []
        result, stages = Q.exercise(app, RUN, attempts.append)
        self.assertEqual(attempts, ['models', 'configure', 'project', 'job'])
        self.assertEqual(stages, ['completed'])
        names = [op for op, _ in app.calls]
        self.assertEqual(names.count('dev_job_start_v2'), 1)
        self.assertEqual(names.count('models'), 1)
        project = dict(app.calls)['project_create_v3']
        self.assertEqual(project['agent'], 'opencode')
        self.assertEqual(project['repositories'][0]['name'], 'r0')
        self.assertTrue(project['repositories'][0]['read_only'])
        self.assertEqual(project['repositories'][0]['url'], Q.URL)
        self.assertEqual(result['job']['request_id'], Q.ids(RUN)['job'])

    def test_each_uncertain_write_permanently_stops_forward_progress(self):
        for failing in ('models', 'configure_checked', 'project_create_v3', 'dev_job_start_v2'):
            with self.subTest(operation=failing):
                app = FakeApp(failing)
                with self.assertRaises(RuntimeError):
                    Q.exercise(app, RUN, lambda kind: None)
                names = [op for op, _ in app.calls]
                self.assertEqual(names[-1], failing)
                self.assertEqual(names.count(failing), 1)

    def test_inventory_failure_no_configuration(self):
        app = FakeApp()
        original = app.call
        app.call = lambda op, args: {'status': 'failed'} if op == 'poll' else original(op, args)
        with self.assertRaises(RuntimeError):
            Q.exercise(app, RUN, lambda kind: None)
        self.assertNotIn('configure_checked', [op for op, _ in app.calls])

    def test_readiness_failure_no_job(self):
        app = FakeApp()
        original = app.call
        def call(op, args):
            value = original(op, args)
            if op == 'dev_jobs_state_v2':
                value['model']['ready'] = False
            return value
        app.call = call
        with self.assertRaises(RuntimeError):
            Q.exercise(app, RUN, lambda kind: None)
        self.assertNotIn('dev_job_start_v2', [op for op, _ in app.calls])

    def test_attempt_fsync_failure_no_dispatch(self):
        app = FakeApp()
        with self.assertRaises(OSError):
            Q.exercise(app, RUN, mock.Mock(side_effect=OSError('fixture')))
        self.assertEqual([op for op, _ in app.calls], ['state'])

    def test_cancel_after_last_query_no_next_effect(self):
        app = FakeApp()
        with Q.C.cancellation_owner() as owner:
            original = app.call
            def call(op, args):
                owner.check()
                value = original(op, args)
                if op == 'poll':
                    owner.cancelled[0] = True
                owner.check()
                return value
            app.call = call
            with self.assertRaises(Exception):
                Q.exercise(app, RUN, lambda kind: owner.check())
        self.assertEqual([op for op, _ in app.calls][-1], 'poll')

    def test_worker_frames_require_real_read_and_matching_reply(self):
        kinds = Q.verify_frames(encoded_frames(frames()), RUN, RESULT)
        self.assertEqual(kinds['model_request'], 2)
        self.assertEqual(kinds['read_request'], 1)

    def test_lost_reordered_duplicated_or_foreign_frames_reject(self):
        cases = []
        missing = frames(); del missing[2]; cases.append(missing)
        reordered = frames(); reordered[2], reordered[3] = reordered[3], reordered[2]; cases.append(reordered)
        duplicate = frames(); duplicate.insert(2, copy.deepcopy(duplicate[2])); cases.append(duplicate)
        foreign = frames(); foreign[0]['request_id'] = 'e' * 32; cases.append(foreign)
        for value in cases:
            with self.subTest(types=[f['type'] for f in value]), self.assertRaises(RuntimeError):
                Q.verify_frames(encoded_frames(value), RUN, RESULT)

    def test_wrong_file_wrong_reply_or_failed_frame_reject(self):
        wrong = frames(); wrong[2]['arguments']['filePath'] = '/etc/passwd'
        reply = frames(); reply[-1]['text'] = 'different'
        failed = frames(); failed.insert(1, {'type': 'failed', 'request_id': Q.ids(RUN)['job']})
        for value in (wrong, reply, failed):
            with self.assertRaises(RuntimeError):
                Q.verify_frames(encoded_frames(value), RUN, RESULT)

    def test_duplicate_json_or_missing_newline_reject(self):
        for raw in (b'{"type":"ready","type":"reply"}\n', b'{}'):
            with self.assertRaises(Exception):
                Q.verify_frames(raw, RUN, RESULT)

    def test_missing_review_no_launch(self):
        with mock.patch.object(Q, 'static_check', return_value={}), \
             mock.patch.object(Q.M, 'read', side_effect=FileNotFoundError), \
             mock.patch.object(Q.subprocess, 'Popen') as popen:
            with self.assertRaises(FileNotFoundError):
                Q.launch(RUN, 'one-isolated-public-clone-local-generation')
            popen.assert_not_called()

    def test_unreadable_retained_log_blocks_before_any_launch_write(self):
        with mock.patch.object(Q, 'static_check', return_value={}), \
             mock.patch.object(Q, 'review', return_value={}), \
             mock.patch.object(Q, 'live_check'), \
             mock.patch.object(Q, 'retained_containers', side_effect=RuntimeError('unreadable-log')), \
             mock.patch.object(Q, 'make_directory') as mkdir, \
             mock.patch.object(Q.subprocess, 'Popen') as popen:
            with self.assertRaises(RuntimeError):
                Q.launch(RUN, 'one-isolated-public-clone-local-generation')
            mkdir.assert_not_called()
            popen.assert_not_called()

    def test_consent_missing_no_read_or_launch(self):
        with mock.patch.object(Q, 'static_check') as check:
            with self.assertRaises(RuntimeError):
                Q.launch(RUN, None)
            check.assert_not_called()

    def test_review_rejects_stale_missing_equivalence_and_binding_drift(self):
        before = {'states': {}}
        source = {'verdict': 'pass', 'independent_review': True, 'reviewer': 'offline-fixture',
                  'expires_unix': Q.time.time() + 3600, 'source_snapshot': Q.SOURCE_SNAPSHOT,
                  'candidate_sha256': Q.CANDIDATE_SHA}
        harness = {**source, 'script_sha256': Q.sha(b'script'), 'tests_sha256': Q.sha(b'tests'),
                   'specification_sha256': Q.sha(Q.M.encoded(Q.specification(RUN))),
                   'baseline_sha256': Q.BASELINE_SHA, 'worker_source_equivalence_accepted': True,
                   'historical_log_unavailability_accepted': True, 'retained_containers_sha256': 'f' * 64}
        def validate(s, h):
            def read(path, *args, **kwargs):
                if path.name == 'pipeline-qa-v2-source-review.json':
                    return Q.M.encoded(s)
                if path.name == 'pipeline-qa-v2-harness-review.json':
                    return Q.M.encoded(h)
                return b'tests' if path == Q.TESTS else b'script'
            with mock.patch.object(Q.M, 'read', side_effect=read), \
                 mock.patch.object(Q, 'static_check', return_value=before):
                return Q.review(RUN, before)
        self.assertIn('source_review_sha256', validate(source, harness))
        for key, bad in (('independent_review', False), ('expires_unix', 0),
                         ('worker_source_equivalence_accepted', False),
                         ('historical_log_unavailability_accepted', False), ('script_sha256', '0' * 64),
                         ('tests_sha256', '0' * 64), ('specification_sha256', '0' * 64),
                         ('baseline_sha256', '0' * 64), ('retained_containers_sha256', None)):
            changed = {**harness, key: bad}
            with self.subTest(field=key), self.assertRaises(RuntimeError):
                validate(source, changed)
        with self.assertRaises(RuntimeError):
            validate({**source, 'candidate_sha256': '0' * 64}, harness)

    def test_cleanup_query_cannot_mutate(self):
        with mock.patch.object(Q.subprocess, 'Popen') as popen:
            for operation in ('start', 'stop', 'rm', 'exec', 'run', 'tag', 'build', 'pull'):
                with self.assertRaises(RuntimeError):
                    Q.cleanup_docker(operation)
            popen.assert_not_called()

    def test_absent_container_is_not_quiescence_after_uncertain_dispatch(self):
        with mock.patch.object(Q, 'cleanup_docker', return_value=b''):
            self.assertFalse(Q.owned_container_quiet(RUN))
            self.assertTrue(Q.owned_container_quiet(RUN, allow_absent=True))

    def test_cleanup_observation_does_not_reset_sticky_cancellation(self):
        with Q.C.cancellation_owner() as owner, \
             mock.patch.object(Q, 'cleanup_docker', return_value=b''):
            owner.cancelled[0] = True
            self.assertFalse(Q.owned_container_quiet(RUN))
            self.assertTrue(owner.cancelled[0])
            with self.assertRaises(Exception):
                Q.C.checkpoint()

    def test_real_sticky_sigterm_inhibits_later_dispatch(self):
        with Q.C.cancellation_owner() as owner:
            os.kill(os.getpid(), signal.SIGTERM)
            with self.assertRaises(Exception):
                owner.perform(mock.Mock(side_effect=AssertionError('must not dispatch')))
            self.assertTrue(owner.cancelled[0])

    def test_exclusive_copy_never_replaces(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, target = Path(temporary, 'source'), Path(temporary, 'target')
            source.write_bytes(b'fixture'); source.chmod(0o600)
            target.write_bytes(b'keep')
            with self.assertRaises(FileExistsError):
                Q.copy_file(source, target, Q.sha(b'fixture'), 0o600)
            self.assertEqual(target.read_bytes(), b'keep')

    def test_cargo_hardlinked_input_copies_to_distinct_private_0755(self):
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary, 'cargo-artifact')
            source.write_bytes(b'compiled-fixture'); source.chmod(0o775)
            os.link(source, Path(temporary, 'deps-artifact'))
            target = Path(temporary, 'private-candidate')
            with mock.patch.object(Q, 'CANDIDATE', source), \
                 mock.patch.object(Q, 'CANDIDATE_SHA', Q.sha(b'compiled-fixture')):
                Q.copy_file(source, target, Q.CANDIDATE_SHA, 0o755)
            self.assertEqual(source.stat().st_nlink, 2)
            self.assertEqual(source.stat().st_mode & 0o777, 0o775)
            self.assertEqual(target.stat().st_nlink, 1)
            self.assertEqual(target.stat().st_mode & 0o777, 0o755)
            self.assertEqual(target.read_bytes(), b'compiled-fixture')

    def test_copy_rejects_wrong_hash_and_symlink(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, target = Path(temporary, 'source'), Path(temporary, 'target')
            source.write_bytes(b'fixture'); source.chmod(0o600)
            with self.assertRaises(RuntimeError):
                Q.copy_file(source, target, '0' * 64, 0o600)
            link = Path(temporary, 'link'); link.symlink_to(source)
            with self.assertRaises(Exception):
                Q.copy_file(link, target, Q.sha(b'fixture'), 0o600)
            self.assertFalse(target.exists())


class CandidateContractTests(unittest.TestCase):
    def test_exact_payload_on_isolated_candidate_without_model_or_docker(self):
        # Preserve this fixture; never use the installed socket/profile or the
        # first QA attempt. No models/configure/Dev operation is reachable here.
        baseline = Q.M.read(Q.PRIVATE / 'pipeline-qa-baseline.json')
        self.assertEqual(Q.sha(baseline), Q.BASELINE_SHA)
        before = Q.M.decode(baseline)
        Q.M.safe_directory(Q.PRIVATE, private=True)
        runtime_parent = Path(before['roots']['--runtime-root'])
        Q.M.safe_directory(runtime_parent, private=True)
        self.assertEqual(Q.source_snapshot()[0], Q.SOURCE_SNAPSHOT)
        base = Path(tempfile.mkdtemp(prefix='qa-v2-contract-', dir=Q.PRIVATE))
        runtime = Path(tempfile.mkdtemp(prefix='qa-v2-contract-', dir=runtime_parent))
        data, config = base / 'd', base / 'c'
        for path in (data, config, data / 'smith-next', data / 'smith-next/bin'):
            Q.make_directory(path)
        namespace = data / 'smith-next'
        exe = namespace / 'bin/smith-service'
        Q.copy_file(Q.CANDIDATE, exe, Q.CANDIDATE_SHA, 0o755)
        endpoint = runtime / 'smith-next/app.sock'
        self.assertLess(len(os.fsencode(endpoint)), 108)
        deadline = Q.time.monotonic() + 20
        permitted = frozenset(('state', 'projects_list_v3', 'project_create_v3', 'project_receipt'))
        operations, drains = [], []
        parent = os.getpid()
        child = subprocess.Popen([str(exe), 'serve', '--data-root', str(data),
                    '--config-root', str(config), '--runtime-root', str(runtime)],
            env={'HOME': str(base), 'PATH': '/nonexistent', 'XDG_DATA_HOME': str(data),
                 'XDG_CONFIG_HOME': str(config), 'XDG_RUNTIME_DIR': str(runtime)},
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            start_new_session=True, preexec_fn=lambda: Q.M.parent_death_guard(parent))
        try:
            Q.M.write(base / 'owner.json', Q.M.encoded({'pid': child.pid, 'sid': child.pid,
                         'runtime_fixture': str(runtime), 'candidate_sha256': Q.CANDIDATE_SHA}))
            for stream, name in ((child.stdout, 'stdout.log'), (child.stderr, 'stderr.log')):
                thread = Q.threading.Thread(target=Q.capture, args=(stream, base / name), daemon=True)
                thread.start(); drains.append(thread)
            def guard():
                self.assertLess(Q.time.monotonic(), deadline)
                self.assertIsNone(os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT))
            while not endpoint.is_socket():
                guard(); Q.time.sleep(0.02)
            self.assertEqual(Q.sha((Path('/proc') / str(child.pid) / 'exe').read_bytes()), Q.CANDIDATE_SHA)
            class MetadataApp(Q.App):
                def call(self, op, args):
                    Q.require(op in permitted, 'non-metadata-fixture-operation-forbidden')
                    operations.append(op)
                    return super().call(op, args)
            app = MetadataApp(endpoint, child.pid, guard)
            initial = app.call('state', {})
            self.assertIsNone(initial['config']['model'])
            registry = app.call('projects_list_v3', {})
            args = Q.project_args(RUN, app.instance, registry['revision'])
            disk_before = Q.M.read(namespace / 'app-state.json')
            bad = copy.deepcopy(args)
            bad['request_id'] = 'b' * 32
            bad['repositories'][0]['name'] = None
            with self.assertRaises(Q.AppRefusal) as refused:
                app.call('project_create_v3', bad)
            self.assertEqual(Q.refusal_fields(refused.exception),
                             {'failed_operation': 'project_create_v3', 'error_code': 'INVALID_REQUEST'})
            self.assertEqual(Q.M.read(namespace / 'app-state.json'), disk_before)
            receipt = app.call('project_create_v3', args)
            self.assertEqual(app.call('project_receipt', {'request_id': args['request_id']}), receipt)
            saved = app.call('projects_list_v3', {})
            project = next(p for p in saved['projects'] if p['project_id'] == receipt['project_id'])
            self.assertEqual(project['repositories'], args['repositories'])
            self.assertEqual(project['repositories'][0]['name'], 'r0')
            self.assertEqual(saved['revision'], receipt['revision'])
            disk = Q.M.decode(Q.M.read(namespace / 'app-state.json'))
            self.assertEqual(disk['config'], {**initial['config'], 'revision': int(initial['config']['revision'])})
            self.assertEqual(disk['jobs'], [])
            self.assertFalse((namespace / 'models').exists())
            self.assertFalse((namespace / 'client').exists())
            self.assertFalse((namespace / 'dev-jobs.json').exists())
            self.assertFalse((namespace / 'dev-local-jobs.json').exists())
            self.assertEqual(set(Q.session_members(child.pid)), {child.pid})
            proof = {'status': 'pass', 'candidate_sha256': Q.CANDIDATE_SHA,
                'source_snapshot': Q.SOURCE_SNAPSHOT, 'operations': operations,
                'legacy_null_refusal': Q.refusal_fields(refused.exception),
                'corrected_repository_id': 'r0', 'project_receipt_verified': True,
                'configuration_unchanged': True, 'model_files': 0, 'metadata_jobs': 0,
                'generation_requests': 0, 'docker_operations': 0,
                'app_state_sha256': Q.sha(Q.M.read(namespace / 'app-state.json'))}
        finally:
            Q.end_session(child, lambda phase: None)
            for thread in drains:
                while thread.is_alive():
                    thread.join(0.2)
        self.assertFalse(Q.session_members(child.pid))
        self.assertTrue(all(not os.path.lexists(runtime / 'smith-next' / name)
                            for name in ('service.sock', 'app.sock')))
        proof['cleanup_verified'] = True
        Q.M.write(base / 'contract-result.json', Q.M.encoded(proof))
        print('candidate-contract: PASS; private fixture=' + base.name
              + '; proof_sha256=' + Q.sha(Q.M.encoded(proof)))


class RetainedLogTests(unittest.TestCase):
    def setUp(self):
        self.old, self.target = 'a' * 64, 'b' * 64
        self.old_request = '9a8d5ecb22fa4b85629e69f7a1e624b1'
        self.target_request = '432f0382da5bea048bf1f09cb431f9ea'
        self.infos = {identifier: {'Id': identifier, 'State': {'Running': False},
            'Config': {'Labels': {'org.smith.dev.request': request}}}
            for identifier, request in ((self.old, self.old_request), (self.target, self.target_request))}
        self.error = {'exit_code': 1, 'stdout_bytes': 0, 'stderr_bytes': 83,
                      'stdout_sha256': Q.sha(b''), 'stderr_sha256': Q.sha(b'fixed historical error')}
        self.observations = {self.old: self.error, self.target: {**self.error, 'exit_code': 0}}
        self.historical = {'container_id': self.old, 'request_id': self.old_request,
                           'inspect_sha256': Q.sha(Q.M.encoded([self.infos[self.old]])),
                           'log_observation': copy.deepcopy(self.error)}
        self.listing = [self.old, self.target]

    def proof(self):
        def query(*args):
            if args[0] == 'ps':
                return ('\n'.join(self.listing) + '\n').encode()
            self.assertEqual(args[0], 'inspect')
            return Q.M.encoded([self.infos[args[1]]])
        with mock.patch.object(Q, 'historical_log_binding', return_value=self.historical):
            return Q.retained_containers(query=query, observe_logs=self.observations.__getitem__)

    def test_exact_historical_error_and_target_readable_are_preserved(self):
        proof = self.proof()
        self.assertEqual(proof[self.old]['log_observation'], self.error)
        self.assertEqual(proof[self.target]['log_observation']['exit_code'], 0)

    def test_same_error_on_target_never_gets_exception(self):
        self.observations[self.target] = copy.deepcopy(self.error)
        with self.assertRaises(RuntimeError):
            self.proof()

    def test_changed_error_or_newly_readable_history_requires_rebinding(self):
        for change in ({'stderr_sha256': '0' * 64}, {'exit_code': 0}, {'stdout_bytes': 1}):
            self.observations[self.old] = {**self.error, **change}
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                self.proof()

    def test_historical_inspect_drift_or_disappearance_rejected(self):
        self.infos[self.old]['extra'] = 'changed'
        with self.assertRaises(RuntimeError):
            self.proof()
        self.listing = [self.target]
        with self.assertRaises(RuntimeError):
            self.proof()

    def test_running_container_never_equates_to_retained_error(self):
        self.infos[self.old]['State']['Running'] = True
        with self.assertRaises(RuntimeError):
            self.proof()


class DockerObservationTests(unittest.TestCase):
    def fixture_spawn(self, program, collected):
        popen = subprocess.Popen
        def spawn(argv, **kwargs):
            self.assertEqual(argv[0], '/usr/bin/docker')
            child = popen([sys.executable, '-B', '-c', program], **kwargs)
            collected.append(child)
            return child
        return mock.patch.object(Q.subprocess, 'Popen', side_effect=spawn)

    def test_exact_nonzero_channel_observation_and_positive_join(self):
        children = []
        with self.fixture_spawn('import os; os.write(1,b"out"); os.write(2,b"error"); raise SystemExit(1)', children):
            result = Q.docker_query('logs', 'a' * 64, observe=True)
        self.assertEqual(result, {'exit_code': 1, 'stdout_bytes': 3, 'stderr_bytes': 5,
                                 'stdout_sha256': Q.sha(b'out'), 'stderr_sha256': Q.sha(b'error')})
        self.assertEqual(children[0].returncode, 1)

    def test_nonzero_without_observation_is_still_failure(self):
        children = []
        with self.fixture_spawn('raise SystemExit(1)', children), self.assertRaises(RuntimeError):
            Q.cleanup_docker('logs', 'a' * 64)
        self.assertEqual(children[0].returncode, 1)

    def test_late_cancellation_after_join_never_returns_forward_result(self):
        children = []
        end = Q.end_session
        with Q.C.cancellation_owner() as owner:
            def join(*args, **kwargs):
                end(*args, **kwargs)
                owner.cancelled[0] = True
            with self.fixture_spawn('pass', children), mock.patch.object(Q, 'end_session', side_effect=join):
                with self.assertRaises(Exception):
                    Q.docker_query('logs', 'a' * 64, observe=True)
            self.assertIsNotNone(children[0].returncode)
            self.assertTrue(owner.cancelled[0])

    def test_readonly_cleanup_observation_after_cancel_keeps_latch(self):
        children = []
        with Q.C.cancellation_owner() as owner:
            owner.cancelled[0] = True
            with self.fixture_spawn('pass', children):
                result = Q.docker_query('logs', 'a' * 64, cleanup=True, observe=True)
            self.assertEqual(result['exit_code'], 0)
            self.assertTrue(owner.cancelled[0])
            self.assertEqual(children[0].returncode, 0)

    def test_observation_refuses_arbitrary_flags_or_commands(self):
        with mock.patch.object(Q.subprocess, 'Popen') as popen:
            for args in (('inspect', 'a' * 64), ('logs', '--follow', 'a' * 64), ('logs', 'bad')):
                with self.assertRaises(RuntimeError):
                    Q.docker_query(*args, observe=True)
            popen.assert_not_called()


class OwnershipTests(unittest.TestCase):
    def test_finite_survivors_and_enumeration_errors_keep_lease(self):
        with tempfile.TemporaryFile() as first, tempfile.TemporaryFile() as unused:
            duplicate = os.open('/proc/self/fd/' + str(first.fileno()), os.O_RDWR)
            try:
                fcntl.flock(first, fcntl.LOCK_EX | fcntl.LOCK_NB)
                facts = {'sid': 123456789, 'state': 'S', 'pgrp': 123456789, 'start': '1'}
                answers = iter((OSError('proc enumeration'), {123456789: facts}, PermissionError('proc stat'), {}))
                child = mock.Mock(pid=123456789)
                def scan(sid):
                    child.wait.assert_not_called()
                    with self.assertRaises(BlockingIOError):
                        fcntl.flock(duplicate, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    answer = next(answers)
                    if isinstance(answer, Exception):
                        raise answer
                    return answer
                with mock.patch.object(Q, 'process_stat', return_value=facts), \
                     mock.patch.object(Q.os, 'kill') as kill:
                    Q.end_session(child, mock.Mock(), scan=scan, sleep=lambda seconds: None)
                    kill.assert_called_once()
                child.wait.assert_called_once()
                fcntl.flock(first, fcntl.LOCK_UN)
                fcntl.flock(duplicate, fcntl.LOCK_EX | fcntl.LOCK_NB)
            finally:
                os.close(duplicate)

    def test_evidence_failure_cannot_unwind_cleanup(self):
        child = mock.Mock(pid=123456789)
        answers = iter((OSError('proc'), {}))
        def scan(sid):
            answer = next(answers)
            if isinstance(answer, Exception):
                raise answer
            return answer
        Q.end_session(child, mock.Mock(side_effect=OSError('fsync')), scan=scan, sleep=lambda seconds: None)
        child.wait.assert_called_once()

    def test_child_inherited_lease_survives_parent_fd_close(self):
        with tempfile.TemporaryFile() as original:
            first = os.dup(original.fileno())
            other = os.open('/proc/self/fd/' + str(original.fileno()), os.O_RDWR)
            fcntl.flock(first, fcntl.LOCK_EX | fcntl.LOCK_NB)
            original.close()  # Only the explicit parent/child descriptions remain.
            child = subprocess.Popen([sys.executable, '-B', '-c',
                'import os,time; os.write(1,b"ready\\n"); time.sleep(10)'],
                start_new_session=True, pass_fds=(first,), stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
            try:
                self.assertTrue(Q.select.select([child.stdout], [], [], 3)[0])
                self.assertEqual(child.stdout.readline(), b'ready\n')
                os.close(first)
                with self.assertRaises(BlockingIOError):
                    fcntl.flock(other, fcntl.LOCK_EX | fcntl.LOCK_NB)
                Q.end_session(child, lambda phase: None)
                fcntl.flock(other, fcntl.LOCK_EX | fcntl.LOCK_NB)
            finally:
                if child.returncode is None:
                    Q.end_session(child, lambda phase: None)
                child.stdout.close()
                os.close(other)

    def test_real_secondary_process_group_is_joined_before_release(self):
        program = ('import os,time; p=os.fork(); '
                   'os.setpgrp() if p==0 else None; time.sleep(10)')
        child = subprocess.Popen([sys.executable, '-B', '-c', program], start_new_session=True,
                                 stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            # Wait positively for the nested process group rather than assume a sleep was sufficient.
            until = Q.time.monotonic() + 3
            while True:
                members = Q.session_members(child.pid)
                if len({p['pgrp'] for p in members.values()}) >= 2:
                    break
                self.assertLess(Q.time.monotonic(), until)
                Q.time.sleep(0.02)
            Q.end_session(child, lambda phase: None)
            self.assertEqual(Q.session_members(child.pid), {})
            self.assertIsNotNone(child.returncode)
        finally:
            if child.returncode is None:
                Q.end_session(child, lambda phase: None)


if __name__ == '__main__':
    unittest.main()
