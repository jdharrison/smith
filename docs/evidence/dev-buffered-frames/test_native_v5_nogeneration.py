"""Offline source/receipt/ownership regressions; never launch a native runtime."""
import copy
import importlib.util
from pathlib import Path
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location('native_v5_fixture',
    Path(__file__).with_name('native-v5-nogeneration.py'))
N = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(N)


class ReviewTests(unittest.TestCase):
    def fixture(self):
        prepared = {'tool_sha256': 'a' * 64}
        receipt = {'verdict': 'pass', 'independent_review': True, 'reviewer': 'offline-fixture',
            'expires_unix': N.time.time() + 3600, 'scope': N.REVIEW_SCOPE,
            'source_snapshot': N.Q.SOURCE_SNAPSHOT, 'candidate_sha256': N.Q.CANDIDATE_SHA,
            'driver_sha256': N.Q.sha(b'driver'), 'tests_sha256': N.Q.sha(b'tests'),
            'harness_sha256': N.Q.sha(b'harness'), 'harness_tests_sha256': N.Q.sha(b'harness-tests'),
            'prepared_sha256': N.Q.sha(b'prepared'), 'tool_sha256': prepared['tool_sha256']}
        return prepared, receipt

    def validate(self, receipt, owner_change=None, detached=True):
        prepared, _ = self.fixture()
        owner = {'pid': 123, 'sid': 123, 'start': '42',
                 'driver_sha256': N.Q.sha(b'driver'), 'scope': N.REVIEW_SCOPE}
        if owner_change:
            owner.update(owner_change)
        def read(path, *args, **kwargs):
            if path.name == 'review.json':
                return N.M.encoded(receipt)
            if path.name == 'supervisor-owner.json':
                return N.M.encoded(owner)
            return {'prepared.json': b'prepared', N.TESTS.name: b'tests',
                N.Q.TESTS.name: b'harness-tests', Path(N.__file__).name: b'driver',
                Path(N.Q.__file__).name: b'harness'}[path.name]
        with mock.patch.object(N.M, 'read', side_effect=read), \
             mock.patch.object(N.os, 'getpid', return_value=123), \
             mock.patch.object(N.os, 'getsid', return_value=123 if detached else 456), \
             mock.patch.object(N.Q, 'process_stat', return_value={'start': '42'}), \
             mock.patch.object(N.M, 'write') as write, \
             mock.patch.object(N.subprocess, 'Popen') as popen:
            N.reviewed_probe(prepared)
            write.assert_not_called()
            popen.assert_not_called()

    def test_exact_mock_review_and_owned_session(self):
        self.validate(self.fixture()[1])

    def test_every_binding_required_and_exact(self):
        _, receipt = self.fixture()
        for key in receipt:
            changed = copy.deepcopy(receipt)
            del changed[key]
            with self.subTest(missing=key), self.assertRaises(RuntimeError):
                self.validate(changed)
        for key in ('source_snapshot', 'candidate_sha256', 'driver_sha256', 'tests_sha256',
                    'harness_sha256', 'harness_tests_sha256', 'prepared_sha256', 'tool_sha256', 'scope'):
            with self.subTest(drift=key), self.assertRaises(RuntimeError):
                self.validate({**receipt, key: 'drift'})

    def test_expired_overlong_or_nonindependent_review_refused(self):
        _, receipt = self.fixture()
        for key, value in (('expires_unix', 0), ('expires_unix', N.time.time() + 90000),
                           ('expires_unix', True), ('independent_review', False),
                           ('verdict', 'no-go'), ('reviewer', '')):
            with self.subTest(field=key, value=value), self.assertRaises(RuntimeError):
                self.validate({**receipt, key: value})

    def test_foreground_or_owner_identity_drift_refused(self):
        receipt = self.fixture()[1]
        with self.assertRaises(RuntimeError):
            self.validate(receipt, detached=False)
        for change in ({'pid': 456}, {'sid': 456}, {'start': '43'}, {'driver_sha256': 'drift'}):
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                self.validate(receipt, owner_change=change)

    def test_missing_review_stops_probe_before_effect_or_tool_dispatch(self):
        prepared = N.M.encoded(self.fixture()[0])
        def read(path, *args, **kwargs):
            if path.name == 'prepared.json':
                return prepared
            raise FileNotFoundError('offline fixture')
        with mock.patch.object(N.M, 'read', side_effect=read), \
             mock.patch.object(N.M, 'write') as write, \
             mock.patch.object(N.subprocess, 'Popen') as popen, \
             mock.patch.object(N.C, 'owned_command') as command, \
             mock.patch.object(N, 'tool_hash') as tool:
            with self.assertRaises(FileNotFoundError):
                N.probe()
            write.assert_not_called()
            popen.assert_not_called()
            command.assert_not_called()
            tool.assert_not_called()

    def test_endpoint_allowlist_never_includes_generation(self):
        self.assertEqual(N.D.ALLOWED, {('GET', '/health'), ('GET', '/props'),
                                      ('POST', '/apply-template'), ('POST', '/tokenize')})


if __name__ == '__main__':
    unittest.main()
