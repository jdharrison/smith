"""T3 isolated stdlib tests. No real native helper, manager, provider or download."""

from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipapp
from pathlib import Path
from typing import Any
from unittest import mock

# Also works when Dagger mounts the two library modules directly in /src.
SOURCE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SOURCE))
SPEC = importlib.util.spec_from_file_location('smith_recipient_installer', SOURCE / 'installer.py')
assert SPEC is not None and SPEC.loader is not None
setup = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(setup)
MODEL = 'tiny:exact'
INSTANCE = 'a' * 32


def core(unit='installed', runtime='running', bridge='ready', message='Observed') -> dict[str, Any]:
    return {'schema_version': 1, 'service': 'smith-next', 'ok': True, 'binary_state': 'installed',
            'unit_state': unit, 'runtime_state': runtime, 'bridge_state': bridge, 'setup_complete': False,
            'message': message, 'error': None, 'effects': {'binary_path': '/managed/bin',
            'unit_path': '/managed/unit', 'state_path': '/managed/state', 'socket_path': '/managed/socket',
            'description': 'Exact isolated effects'}}


def operation(request, status='succeeded') -> dict[str, Any]:
    return {'request_id': request, 'kind': 'models', 'status': status,
            'phase': 'complete' if status == 'succeeded' else 'inventory', 'config_revision': 0,
            'result': inventory() if status == 'succeeded' else None, 'error': None}


def inventory() -> dict[str, Any]:
    return {'models': [{'name': MODEL, 'digest': 'b' * 64}], 'voices': [], 'providers': {
        'llm': {'ready': True, 'error': None},
        'stt': {'ready': False, 'model': None,
                'error': {'code': 'RETIRED', 'message': 'Speech capability is retired; use text chat'}},
        'tts': {'ready': False,
                'error': {'code': 'RETIRED', 'message': 'Speech capability is retired; use text chat'}}}}


def envelope(request, result, instance=INSTANCE):
    return {'schema_version': 1, 'service': 'smith-next', 'protocol': 'app', 'pid': 123,
            'instance_id': instance, 'call_id': request['call_id'], 'ok': True, 'result': result, 'error': None}


class FakePublisher:
    def __init__(self, client, service):
        self.client, self.service = client, service
        self.published = False
        self.calls = []

    def run(self, dry_run=False, confirm=None, output=None):
        self.calls.append(dry_run)
        if not dry_run:
            assert confirm is not None
            if confirm('Publish exact artifacts? ') == 'y':
                self.published = True
        return 0


class FakeNative:
    def __init__(self):
        self.calls = []
        self.facts = core()
        self.tags = [{MODEL}]
        self.configured = None

    def core(self, *args):
        self.calls.append(args)
        if args[0] == 'prepare':
            result = dict(self.facts)
            result['approval_token'] = '123.' + 'c' * 64
            return result
        if args[0] == 'apply':
            self.facts = core(runtime='stopped', bridge='unavailable') if args[2] == 'install-unit' else core()
        return self.facts

    def models(self):
        self.calls.append(('models',))
        return self.tags.pop(0) if len(self.tags) > 1 else self.tags[0]

    def app(self, op, args):
        self.calls.append((op, args))
        self.configured = args
        return {'model': args['model'], 'voice': args['voice'], 'revision': 1}


class FakeProvider:
    def __init__(self):
        self.calls = []

    def pull(self, model, output):
        self.calls.append(('pull', model))

    def inference(self, model):
        self.calls.append(('inference', model))
        return 'OK'


class OrchestrationTests(unittest.TestCase):
    def setUp(self):
        self.output = io.StringIO()
        self.publisher = FakePublisher('', '')
        self.native = FakeNative()
        self.provider = FakeProvider()
        self.answers = iter(['y'] * 12)
        self.prompts = []

    def confirm(self, prompt):
        self.prompts.append(prompt)
        return next(self.answers)

    def installer(self, **kwargs):
        return setup.Installer('/isolated/bundle', publisher=lambda client, service: self.publisher,
                               native=lambda path: self.native, ollama=lambda: self.provider,
                               preflight=lambda: None, output=self.output, confirm=self.confirm, **kwargs)

    def test_artifact_decline_does_not_construct_helpers_or_provider(self):
        self.answers = iter(['n'])
        instance = self.installer(model=MODEL, pull_model=True)
        instance.native = mock.Mock(side_effect=AssertionError('native effect'))
        instance.ollama = mock.Mock(side_effect=AssertionError('provider effect'))
        self.assertEqual(instance.run(), 0)
        self.assertFalse(self.publisher.published)
        self.assertFalse(instance.effects)

    def test_dry_run_artifact_only_no_consent_helper_or_network(self):
        instance = self.installer(model=MODEL, pull_model=True, verify_inference=True)
        instance.confirm = mock.Mock(side_effect=AssertionError('prompt'))
        instance.native = mock.Mock(side_effect=AssertionError('native'))
        instance.ollama = mock.Mock(side_effect=AssertionError('network'))
        self.assertEqual(instance.run(dry_run=True), 0)
        self.assertEqual(self.publisher.calls, [True])
        self.assertFalse(instance.effects)

    def test_missing_prerequisite_before_artifacts(self):
        instance = self.installer()
        instance.preflight = mock.Mock(side_effect=setup.Stopped('Python/runtime missing'))
        self.assertEqual(instance.run(), 1)
        self.assertFalse(self.publisher.calls)
        self.assertIn('provisioned separately', self.output.getvalue())

    def test_register_and_start_separate_prepare_confirmation_apply(self):
        self.native.facts = core(unit='missing', runtime='stopped', bridge='unavailable')
        self.assertEqual(self.installer(model=MODEL).run(), 0)
        self.assertEqual(self.native.calls[:7], [
            ('inspect',), ('prepare', '--action', 'install-unit'),
            ('apply', '--action', 'install-unit', '--approval', '123.' + 'c' * 64), ('inspect',),
            ('prepare', '--action', 'start'),
            ('apply', '--action', 'start', '--approval', '123.' + 'c' * 64), ('inspect',)])
        self.assertEqual(self.native.configured, {'model': MODEL, 'voice': None})
        self.assertEqual(len(self.prompts), 4)
        self.assertNotIn('complete-setup', repr(self.native.calls))

    def test_reload_only_repair_uses_native_install_unit(self):
        self.native.facts = core(runtime='stopped', bridge='unavailable', message=setup.RELOAD_MESSAGE)
        self.assertEqual(self.installer().run(), 0)
        self.assertIn(('prepare', '--action', 'install-unit'), self.native.calls)

    def test_later_decline_reports_artifacts_without_start(self):
        self.native.facts = core(unit='missing', runtime='stopped', bridge='unavailable')
        self.answers = iter(['y', 'n'])
        self.assertEqual(self.installer().run(), 1)
        self.assertTrue(self.publisher.published)
        self.assertFalse(any(call[0] == 'apply' for call in self.native.calls))
        self.assertIn('artifacts remain installed', self.output.getvalue())

    def test_missing_manager_and_conflicting_identity_block(self):
        for facts in (core(runtime='unavailable', bridge='unavailable'),
                      dict(core(), binary_state='conflict'), core(runtime='conflict', bridge='conflict')):
            with self.subTest(facts=facts):
                self.native.facts = facts
                self.native.calls.clear()
                self.assertEqual(self.installer().run(), 1)
                self.assertEqual(self.native.calls, [('inspect',)])

    def test_configure_decline_no_preference_write(self):
        self.answers = iter(['y', 'n'])
        self.assertEqual(self.installer(model=MODEL).run(), 1)
        self.assertIsNone(self.native.configured)
        self.assertFalse(self.provider.calls)

    def test_missing_model_no_implicit_download(self):
        self.native.tags = [set()]
        self.assertEqual(self.installer(model=MODEL).run(), 1)
        self.assertFalse(self.provider.calls)
        self.assertIsNone(self.native.configured)

    def test_pull_flag_not_consent_and_exact_model_bound(self):
        for answer in ('y', 'other:tag', 'n'):
            with self.subTest(answer=answer):
                self.native.tags = [set()]
                self.answers = iter(['y', answer])
                self.assertEqual(self.installer(model=MODEL, pull_model=True).run(), 1)
                self.assertFalse(self.provider.calls)
                self.assertIn('external daemon', self.prompts[-1])

    def test_pull_fresh_inventory_configure_and_separate_inference(self):
        self.native.tags = [set(), {MODEL}]
        self.answers = iter(['y', MODEL, 'y', 'y'])
        self.assertEqual(self.installer(model=MODEL, pull_model=True, verify_inference=True).run(), 0)
        self.assertEqual(self.provider.calls, [('pull', MODEL), ('inference', MODEL)])
        self.assertEqual(self.native.calls.count(('models',)), 2)
        self.assertNotIn('submit', repr(self.native.calls))
        self.assertEqual(len(self.prompts), 4)

    def test_postpull_app_inventory_must_match(self):
        self.native.tags = [set(), set()]
        self.answers = iter(['y', MODEL])
        self.assertEqual(self.installer(model=MODEL, pull_model=True).run(), 1)
        self.assertIsNone(self.native.configured)

    def test_pull_cancellation_reports_unknown_daemon_cache_no_resend(self):
        self.native.tags = [set()]
        self.answers = iter(['y', MODEL])
        self.provider.pull = mock.Mock(side_effect=KeyboardInterrupt())
        self.assertEqual(self.installer(model=MODEL, pull_model=True).run(), 1)
        self.provider.pull.assert_called_once_with(MODEL, self.output)
        self.assertIsNone(self.native.configured)
        self.assertIn('partial cache may be unknown; no cache was deleted', self.output.getvalue())

    def test_inference_decline_preserves_saved_preference(self):
        self.answers = iter(['y', 'y', 'n'])
        self.assertEqual(self.installer(model=MODEL, verify_inference=True).run(), 1)
        self.assertIsNotNone(self.native.configured)
        self.assertFalse(self.provider.calls)
        self.assertIn('saved preference', self.output.getvalue())

    def test_no_default_model_and_flags_require_selection(self):
        self.assertEqual(self.installer().run(), 0)
        self.assertIsNone(self.native.configured)
        self.assertIn('No default', self.output.getvalue())
        for kwargs in ({'pull_model': True}, {'verify_inference': True}, {'model': 'tiny'}):
            self.publisher.calls.clear()
            self.assertEqual(self.installer(**kwargs).run(), 1)
            self.assertFalse(self.publisher.calls)


class ProtocolTests(unittest.TestCase):
    def runner(self, path, args, request=None, **kwargs):
        assert request is not None
        self.requests.append(request)
        if request['op'] == 'models':
            self.request_id = request['args']['request_id']
            result = {key: operation(self.request_id)[key]
                      for key in ('request_id', 'status', 'phase', 'config_revision')}
        elif request['op'] == 'poll':
            result = operation(self.request_id, self.statuses.pop(0))
        else:
            result = {}
        return envelope(request, result)

    def setUp(self):
        self.requests = []
        self.statuses = ['running', 'succeeded']
        self.native = setup.Native('/fake/helper', runner=self.runner, sleep=lambda _: None)

    def test_async_models_poll_and_all_ids_lowerhex(self):
        self.assertEqual(self.native.models(), {MODEL})
        self.assertEqual([r['op'] for r in self.requests], ['models', 'poll', 'poll'])
        for request in self.requests:
            self.assertTrue(setup.identifier(request['call_id']))
            self.assertEqual(request['schema_version'], 1)
            self.assertEqual(request['protocol'], 'app')
            for value in request['args'].values():
                self.assertTrue(setup.identifier(value))

    def test_paired_production_inventory_wire_fixture_reaches_configure(self):
        requests = []
        def runner(path, args, request=None, **kwargs):
            if args == ['inspect']:
                return core()
            assert args == ['app'] and request is not None
            requests.append(request)
            if request['op'] == 'models':
                job = operation(request['args']['request_id'], 'running')
                result = {key: job[key] for key in ('request_id', 'status', 'phase', 'config_revision')}
            elif request['op'] == 'poll':
                result = operation(request['args']['request_id'])
                # Exact serialized shape of providers::Observations after the
                # text service's retire_speech; independent of the generic fake.
                result['result']['providers'] = json.loads('''{
                    "llm":{"ready":true,"error":null},
                    "stt":{"ready":false,"error":{"code":"RETIRED",
                        "message":"Speech capability is retired; use text chat"},"model":null},
                    "tts":{"ready":false,"error":{"code":"RETIRED",
                        "message":"Speech capability is retired; use text chat"}}
                }''')
            else:
                assert request['op'] == 'configure'
                result = {'model': request['args']['model'], 'voice': request['args']['voice'], 'revision': 1}
            return envelope(request, result)
        native = setup.Native('/fake/paired-helper', runner=runner)
        publisher = FakePublisher('', '')
        instance = setup.Installer('/fake/bundle', model=MODEL, publisher=lambda *_: publisher,
            native=lambda _: native, preflight=lambda: None, confirm=lambda _: 'y', output=io.StringIO())
        self.assertEqual(instance.run(), 0)
        self.assertEqual([request['op'] for request in requests], ['models', 'poll', 'configure'])
        self.assertEqual(requests[-1]['args'], {'model': MODEL, 'voice': None})

    def test_inventory_provider_specific_schema_and_retired_facts_fail_closed(self):
        for name, mutate in (
            ('stt', lambda v: v.pop('model')),
            ('stt', lambda v: v.update(model='speech-model')),
            ('stt', lambda v: v.update(error=None)),
            ('stt', lambda v: v.update(ready=True, error=None)),
            ('tts', lambda v: v.update(model=None)),
            ('tts', lambda v: v['error'].update(code='NOT_OBSERVED')),
            ('tts', lambda v: v['error'].update(message='')),
            ('stt', lambda v: v['error'].update(extra=1)),
            ('llm', lambda v: v.update(model=None)),
            ('llm', lambda v: v.update(ready='true')),
            ('llm', lambda v: v.update(error={'code': 'FAILURE', 'message': 'Not ready'})),
        ):
            with self.subTest(provider=name, mutate=mutate):
                def runner(path, args, request=None, mutate=mutate, name=name, **kwargs):
                    assert request is not None
                    value = self.runner(path, args, request=request, **kwargs)
                    if request['op'] == 'poll' and value['result']['result'] is not None:
                        mutate(value['result']['result']['providers'][name])
                    return value
                self.statuses = ['succeeded']
                native = setup.Native('/fake/helper', runner=runner)
                self.assertRaises(setup.Stopped, native.models)

    def test_app_five_operation_subset_only_and_strict_args(self):
        for op, args in [('speak', {}), ('transcribe', {}), ('state', {'extra': 1}),
                         ('poll', {'request_id': 'A' * 32, 'instance_id': INSTANCE}),
                         ('configure', {'model': MODEL, 'voice': 'voice'}),
                         ('submit', {'request_id': INSTANCE, 'text': 'x', 'output': 'speech'})]:
            with self.subTest(op=op), self.assertRaises(setup.Stopped):
                self.native.app(op, args)
        self.assertFalse(self.requests)

    def test_wrong_instance_call_schema_unknown_field_are_rejected(self):
        for mutate in (lambda v: v.update(call_id='d' * 32),
                       lambda v: v.update(instance_id='A' * 32),
                       lambda v: v.update(schema_version=True),
                       lambda v: v.update(extra=1), lambda v: v.update(service='foreign')):
            def runner(path, args, request=None, mutate=mutate, **kwargs):
                value = envelope(request, {})
                mutate(value)
                return value
            with self.subTest(mutate=mutate), self.assertRaises(setup.Stopped):
                setup.Native('/fake', runner=runner).app('state', {})

    def test_changed_instance_no_resend(self):
        def runner(path, args, request=None, **kwargs):
            return envelope(request, {}, instance=INSTANCE if not self.requests else 'c' * 32)
        native = setup.Native('/fake', runner=runner)
        native.app('state', {})
        self.requests.append(1)
        with self.assertRaisesRegex(setup.Stopped, 'instance changed'):
            native.app('state', {})

    def test_inventory_timeout_finite_no_readmission(self):
        self.statuses = ['running'] * 181
        self.assertRaises(setup.Stopped, self.native.models)
        self.assertEqual(sum(r['op'] == 'models' for r in self.requests), 1)

    def test_core_schema_identity_token_and_unknown_fields(self):
        self.assertEqual(setup.core_reply(core()), core())
        for changes in ({'service': 'foreign'}, {'schema_version': True}, {'extra': 1},
                        {'approval_token': 'not-a-binding'}, {'runtime_state': 'enabled'}, {'ok': False}):
            with self.subTest(changes=changes), self.assertRaises(setup.Stopped):
                setup.core_reply(dict(core(), **changes))

    def test_duplicate_fields_and_nonfinite_json_blocked(self):
        for payload in (b'{"ok":true,"ok":false}', b'{"value":NaN}', b'{'):
            with self.subTest(payload=payload), self.assertRaises(setup.Stopped):
                setup.decode(payload)

    def test_ambiguous_cloud_invalid_models_blocked(self):
        for models in ([{'name': MODEL, 'digest': 'x'}], inventory()['models'] * 2,
                       [{'name': 'tiny:cloud', 'digest': 'b' * 64}]):
            self.assertRaises(setup.Stopped, setup.checked_models, models)


class Sock:
    def __init__(self):
        self.timeouts = []
        self.shutdown = mock.Mock()

    def settimeout(self, timeout):
        self.timeouts.append(timeout)


class Response:
    def __init__(self, chunks, status=200):
        self.chunks = list(chunks)
        self.status = status
        self.closed = False

    def read1(self, n):
        if not self.chunks:
            return b''
        chunk = self.chunks.pop(0)
        if isinstance(chunk, BaseException):
            raise chunk
        if len(chunk) > n:
            self.chunks.insert(0, chunk[n:])
        return chunk[:n]

    def close(self):
        self.closed = True


class Connection:
    def __init__(self, response):
        self.response, self.sock = response, Sock()
        self.calls, self.closed = [], False

    def connect(self):
        self.calls.append('connect')

    def request(self, *args, **kwargs):
        self.calls.append((args, kwargs))

    def getresponse(self):
        return self.response

    def close(self):
        self.closed = True


def tags_bytes(model=MODEL, **changes):
    tag = {'name': model, 'model': model, 'digest': 'b' * 64, 'details': {'format': 'gguf'}}
    tag.update(changes)
    return json.dumps({'models': [tag]}).encode()


class HttpTests(unittest.TestCase):
    def provider(self, responses, clock=lambda: 0):
        self.connections = []
        self.factory_calls = []
        responses = iter(responses)

        def factory(*args, **kwargs):
            self.factory_calls.append((args, kwargs))
            connection = Connection(next(responses))
            self.connections.append(connection)
            return connection
        return setup.Ollama(connection=factory, clock=clock)

    def test_pull_bounded_chunks_fresh_tags_fixed_numeric_endpoint(self):
        provider = self.provider([Response([b'{"status":"down', b'loading","completed":2,"total":3}\n',
                                            b'{"status":"success"}\n']), Response([tags_bytes()])])
        with mock.patch.dict(os.environ, {'HTTP_PROXY': 'http://invalid:9', 'OLLAMA_HOST': 'http://invalid:9'}):
            provider.pull(MODEL, io.StringIO())
        self.assertEqual(self.factory_calls, [(('127.0.0.1', 11434), {'timeout': 2})] * 2)
        request = self.connections[0].calls[1]
        self.assertEqual(request[0], ('POST', '/api/pull'))
        self.assertEqual(json.loads(request[1]['body']), {'model': MODEL, 'stream': True})
        self.assertTrue(all(c.closed and c.response.closed for c in self.connections))
        self.assertTrue(all(t <= 30 for c in self.connections for t in c.sock.timeouts))

    def test_pull_success_requires_exact_fresh_eligible_tag(self):
        for payload in (tags_bytes('other:tag'), tags_bytes(remote_host='server'),
                        tags_bytes(details={'format': 'unknown'})):
            provider = self.provider([Response([b'{"status":"success"}\n']), Response([payload])])
            self.assertRaises(setup.Stopped, provider.pull, MODEL, io.StringIO())
            self.assertEqual(len(self.connections), 2)

    def test_tags_check_all_duplicate_names_before_eligibility_filtering(self):
        local = json.loads(tags_bytes())['models'][0]
        for changes in ({}, {'digest': 'c' * 64}, {'remote_host': 'server'}, {'cloud': True},
                        {'details': {'format': 'unknown'}}, {'model': 'other:tag'}):
            conflicting = dict(local, **changes)
            for entries in ([local, conflicting], [conflicting, local]):
                with self.subTest(changes=changes, entries=entries):
                    payload = json.dumps({'models': entries}).encode()
                    provider = self.provider([Response([payload])])
                    self.assertRaises(setup.Stopped, provider.tags)
                    self.assertEqual(len(self.connections), 1)
                    self.assertTrue(self.connections[0].closed and self.connections[0].response.closed)

    def test_tags_conflicting_model_identity_rejected_even_if_ineligible(self):
        for changes in ({'model': 'other:tag'}, {'model': None},
                        {'model': 'other:tag', 'remote_host': 'server'},
                        {'model': 'other:tag', 'details': {'format': 'unknown'}}):
            with self.subTest(changes=changes):
                tag = json.loads(tags_bytes())['models'][0]
                tag.update(changes)
                payload = json.dumps({'models': [tag]}).encode()
                provider = self.provider([Response([payload])])
                self.assertRaises(setup.Stopped, provider.tags)

    def test_ambiguous_inventory_blocks_inference_and_postpull_verification(self):
        entries = [json.loads(tags_bytes())['models'][0], json.loads(tags_bytes(remote_host='server'))['models'][0]]
        payload = json.dumps({'models': entries}).encode()
        provider = self.provider([Response([payload])])
        self.assertRaises(setup.Stopped, provider.inference, MODEL)
        self.assertEqual(self.connections[0].calls[1][0], ('GET', '/api/tags'))
        self.assertEqual(len(self.connections), 1)
        provider = self.provider([Response([b'{"status":"success"}\n']), Response([payload])])
        self.assertRaises(setup.Stopped, provider.pull, MODEL, io.StringIO())
        self.assertEqual(len(self.connections), 2)
        self.assertTrue(all(c.closed for c in self.connections))

    def test_redirect_error_eof_oversize_idle_no_retry(self):
        for response in (Response([], status=302), Response([b'{"error":"no"}\n']),
                         Response([]), Response([b'x' * (setup.CORE_LIMIT + 1)]),
                         Response([TimeoutError('idle')]), Response([b'{"status":"download","total":-1}\n'])):
            with self.subTest(response=response):
                provider = self.provider([response])
                with self.assertRaises((setup.Stopped, OSError)):
                    provider.pull(MODEL, io.StringIO())
                self.assertEqual(len(self.connections), 1)
                self.assertTrue(self.connections[0].closed)

    def test_watchdog_bounds_dripfed_http_and_is_cancelled(self):
        provider = self.provider([Response([b'{"status":"success"}\n']), Response([tags_bytes()])])
        with mock.patch.object(setup.threading, 'Timer') as timer_factory:
            provider.pull(MODEL, io.StringIO())
            self.assertEqual(timer_factory.call_args_list[0].args[0], 900)
            self.assertEqual(timer_factory.call_args_list[1].args[0], 10)
            timer_factory.return_value.start.assert_called()
            timer_factory.return_value.cancel.assert_called()
            sock = self.connections[0].sock
            timer_factory.call_args_list[0].args[1]()
            sock.shutdown.assert_called_once_with(setup.socket.SHUT_RDWR)
        self.assertFalse(provider.deadlines)

    def test_overall_deadline_and_cancellation_cleanup(self):
        ticks = iter([0, 0, 0, 901])
        provider = self.provider([Response([b'{"status":"success"}\n'])], clock=lambda: next(ticks))
        self.assertRaises(setup.Stopped, provider.pull, MODEL, io.StringIO())
        self.assertTrue(self.connections[0].closed)
        provider = self.provider([Response([KeyboardInterrupt()])])
        self.assertRaises(KeyboardInterrupt, provider.pull, MODEL, io.StringIO())
        self.assertTrue(self.connections[0].closed)
        self.assertTrue(self.connections[0].response.closed)

    def test_inference_fixed_minimal_no_tools_no_persisted_chat(self):
        result = json.dumps({'model': MODEL, 'done': True, 'response': 'OK'}).encode()
        provider = self.provider([Response([tags_bytes()]), Response([result])])
        self.assertEqual(provider.inference(MODEL), 'OK')
        request = self.connections[1].calls[1]
        self.assertEqual(request[0], ('POST', '/api/generate'))
        self.assertEqual(json.loads(request[1]['body']), {'model': MODEL, 'prompt': 'Reply with OK.',
                         'stream': False, 'options': {'num_predict': 8, 'temperature': 0}, 'keep_alive': 0})
        self.assertTrue(all(c.closed for c in self.connections))

    def test_inference_rejects_remote_metadata_but_not_generated_text(self):
        for changes in ({'remote_host': 'server'}, {'cloud': True},
                        {'details': {'remote_host': 'server'}}, {'metadata': [{'cloud': True}]},
                        {'remote_host': 0}, {'cloud': {}}, {'metadata': 'https://remote.example'}):
            with self.subTest(changes=changes):
                result = dict(model=MODEL, done=True, response='OK', **changes)
                provider = self.provider([Response([tags_bytes()]), Response([json.dumps(result).encode()])])
                self.assertRaises(setup.Stopped, provider.inference, MODEL)
                self.assertTrue(all(c.closed for c in self.connections))
        for generated in ('cloud', 'remote', 'http://example.test', 'https://example.test'):
            with self.subTest(generated=generated):
                result = {'model': MODEL, 'done': True, 'response': generated, 'cloud': False, 'remote_host': None}
                provider = self.provider([Response([tags_bytes()]), Response([json.dumps(result).encode()])])
                self.assertEqual(provider.inference(MODEL), generated)

    def test_inference_tool_calls_strict_type_absent_null_or_empty_list_only(self):
        for tool_calls in ({}, False, 0, '', [{}], {'name': 'tool'}):
            with self.subTest(tool_calls=tool_calls):
                result = {'model': MODEL, 'done': True, 'response': 'OK', 'tool_calls': tool_calls}
                provider = self.provider([Response([tags_bytes()]), Response([json.dumps(result).encode()])])
                self.assertRaises(setup.Stopped, provider.inference, MODEL)
                self.assertTrue(all(c.closed for c in self.connections))
        for tool_calls in (None, []):
            with self.subTest(tool_calls=tool_calls):
                result = {'model': MODEL, 'done': True, 'response': 'OK', 'tool_calls': tool_calls}
                provider = self.provider([Response([tags_bytes()]), Response([json.dumps(result).encode()])])
                self.assertEqual(provider.inference(MODEL), 'OK')

    def test_inference_wrong_model_or_tools_or_unbounded_text(self):
        for changes in ({'model': 'other:tag'}, {'tool_calls': [{}]}, {'response': 'x' * 4097}, {'done': False}):
            result: dict[str, Any] = {'model': MODEL, 'done': True, 'response': 'OK'}
            result.update(changes)
            provider = self.provider([Response([tags_bytes()]), Response([json.dumps(result).encode()])])
            self.assertRaises(setup.Stopped, provider.inference, MODEL)


class ProcessTests(unittest.TestCase):
    def run_fake(self, code, **kwargs):
        # Only Python fake children are launched; no executable payload runs.
        with mock.patch.object(setup, 'validate_helper', return_value=sys.executable):
            return setup.process_json('/fake/helper', ['-c', code], **kwargs)

    def test_json_stdin_not_argv_and_pipe_bounds(self):
        request = {'prompt': 'private test text'}
        value = self.run_fake('import json,sys; print(json.dumps({"input":json.load(sys.stdin),"args":sys.argv}))',
                              request=request, limit=setup.APP_LIMIT)
        self.assertEqual(value['input'], request)
        self.assertNotIn('private test text', repr(value['args']))
        for code in ('print("x" * 17000)', 'import sys; sys.stderr.write("x" * 17000)'):
            self.assertRaises(setup.Stopped, self.run_fake, code)
        self.assertRaises(setup.Stopped, self.run_fake, 'print("{}")', request={'x': 'x' * setup.APP_LIMIT},
                          limit=setup.APP_LIMIT)

    def test_timeout_and_nonjson_failure_cleanup(self):
        self.assertRaises(setup.Stopped, self.run_fake, 'import time; time.sleep(10)', timeout=0.02)
        self.assertRaises(setup.Stopped, self.run_fake, 'print("not json")')
        self.assertRaises(setup.Stopped, self.run_fake, 'import sys; print("{}"); sys.exit(1)')

    def test_term11_kill1_and_unknown_cleanup(self):
        child = mock.Mock()
        child.poll.return_value = None
        child.wait.side_effect = [subprocess.TimeoutExpired('fake', 11), 0]
        setup.cleanup(child)
        self.assertEqual(child.method_calls, [mock.call.poll(), mock.call.terminate(),
                         mock.call.wait(timeout=11), mock.call.kill(), mock.call.wait(timeout=1)])
        child.wait.side_effect = [subprocess.TimeoutExpired('fake', 11), subprocess.TimeoutExpired('fake', 1)]
        self.assertRaisesRegex(setup.Stopped, 'cleanup unknown', setup.cleanup, child)

    def test_native_canonical_elf_owned_singlelink_mode_ancestors(self):
        with tempfile.TemporaryDirectory(dir=SOURCE / 'tests') as temp:
            path = Path(temp) / 'helper'
            header = bytearray(64)
            header[:7] = b'\x7fELF\x02\x01\x01'
            header[16:20] = b'\x03\x00\x3e\x00'
            path.write_bytes(header)
            path.chmod(0o755)
            # The checkout may have group-writable host ancestors. Model safe
            # ancestry above this private fixture without modifying host modes.
            original_lstat = Path.lstat
            def fixture_lstat(candidate):
                info = original_lstat(candidate)
                if candidate in path.parents and candidate != Path(temp):
                    values = list(info)
                    values[0] = int(values[0]) & ~0o022
                    values[4] = os.geteuid()
                    return os.stat_result(values)
                return info
            patcher = mock.patch.object(Path, 'lstat', fixture_lstat)
            patcher.start()
            self.addCleanup(patcher.stop)
            self.assertEqual(setup.validate_helper(str(path)), str(path))
            path.chmod(0o775)
            self.assertRaises(setup.Stopped, setup.validate_helper, str(path))
            path.chmod(0o755)
            link = Path(temp) / 'linked'
            os.link(path, link)
            self.assertRaises(setup.Stopped, setup.validate_helper, str(path))
            link.unlink()
            link.symlink_to(path)
            self.assertRaises(setup.Stopped, setup.validate_helper, str(link))
            path.write_bytes(b'#!/bin/sh\n')
            self.assertRaises(setup.Stopped, setup.validate_helper, str(path))
            Path(temp).chmod(0o777)
            self.assertRaises(setup.Stopped, setup.validate_helper, str(path))


class PackagingTests(unittest.TestCase):
    def test_recipient_preflight_guides_python_ubuntu_and_unprivileged_user(self):
        with mock.patch.object(setup.sys, 'version_info', (3, 11)):
            self.assertRaisesRegex(setup.Stopped, 'Python 3.12', setup.prerequisites)
        with mock.patch.object(setup.os, 'geteuid', return_value=0):
            self.assertRaisesRegex(setup.Stopped, 'not sudo/root', setup.prerequisites)
        with mock.patch.object(setup.platform, 'freedesktop_os_release', return_value={'ID': 'other'}):
            self.assertRaisesRegex(setup.Stopped, 'Ubuntu required', setup.prerequisites)

    def test_library_main_bundle_defaults_argv0_parent_not_zip_file(self):
        with mock.patch.object(setup, 'Installer') as installer, mock.patch.object(sys, 'argv', ['/bundle/smith-installer']):
            installer.return_value.run.return_value = 0
            self.assertEqual(setup.main(['--model', MODEL, '--dry-run']), 0)
            installer.assert_called_once_with('/bundle', MODEL, False, False)
            installer.return_value.run.assert_called_once_with(True)
        with mock.patch.object(setup, 'Installer') as installer:
            setup.main(['--bundle', '/explicit', '--model', MODEL, '--pull-model', '--verify-inference'])
            installer.assert_called_once_with('/explicit', MODEL, True, True)

    def test_no_global_yes_bypass(self):
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            setup.main(['--yes'])

    def test_packaged_entrypoint_imports_sibling_install_and_resolves_external_payload(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            code = root / 'code'
            code.mkdir()
            for name in ('installer.py', 'install.py'):
                (code / name).write_bytes((SOURCE / name).read_bytes())
            target = root / 'smith-installer'
            (code / '__main__.py').write_text('import installer\nraise SystemExit(installer.main())\n')
            zipapp.create_archive(code, target=target, interpreter='/usr/bin/env python3')
            result = subprocess.run([sys.executable, str(target), '--help'], capture_output=True,
                                    timeout=5, check=True)
            self.assertIn(b'--bundle', result.stdout)
            self.assertIn(b'--verify-inference', result.stdout)
            self.assertTrue(target.stat().st_mode & 0o111)
            # Fake orchestration seam inside test archive, not a production environment switch.
            (code / '__main__.py').write_text('''import json, installer
class Fake:
    def __init__(self, bundle, *args): self.bundle = bundle
    def run(self, dry_run):
        print(json.dumps({"bundle": self.bundle, "client": self.bundle + "/payload/client", "dry_run": dry_run}))
        return 0
installer.Installer = Fake
raise SystemExit(installer.main())
''')
            zipapp.create_archive(code, target=target, interpreter='/usr/bin/env python3')
            for args in ([], ['--bundle', '/explicit', '--dry-run']):
                result = subprocess.run([sys.executable, str(target), *args], capture_output=True,
                                        timeout=5, check=True)
                value = json.loads(result.stdout)
                expected = '/explicit' if args else str(root)
                self.assertEqual(value['bundle'], expected)
                self.assertEqual(value['client'], expected + '/payload/client')
                self.assertNotIn('.py', value['bundle'])


@unittest.skipIf(os.geteuid() == 0, 'Artifact publication tests require non-root')
class RealPublisherIsolationTests(unittest.TestCase):
    def test_actual_publisher_dryrun_decline_snapshot_no_effects(self):
        # Reuse the unchanged publisher's complete temporary artifact fixture.
        spec = importlib.util.spec_from_file_location('fixture_install_tests', SOURCE / 'tests/test_install.py')
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        fixture = module.InstallerTests('test_dry_run_and_decline_are_read_only')
        fixture.setUp()
        try:
            bundle = fixture.root / 'recipient'
            (bundle / 'payload/service').mkdir(parents=True)
            for directory in (bundle, bundle / 'payload', bundle / 'payload/service'):
                directory.chmod(0o755)
            shutil.copytree(fixture.bundle, bundle / 'payload/client')
            shutil.copyfile(fixture.service, bundle / 'payload/service/smith-service')
            (bundle / 'payload/service/smith-service').chmod(0o755)
            publisher = lambda client, service: setup.artifact.Installer(client, service, fixture.env)
            before = fixture.snapshot()
            for dry in (True, False):
                log = io.StringIO()
                instance = setup.Installer(bundle, publisher=publisher, preflight=lambda: None,
                    native=mock.Mock(side_effect=AssertionError('native helper')), confirm=lambda _: 'n',
                    output=log)
                self.assertEqual(instance.run(dry), 0, log.getvalue())
                self.assertEqual(fixture.snapshot(), before)
            native = FakeNative()
            native.facts = core(unit='missing', runtime='stopped', bridge='unavailable')
            answers = iter(['y', 'n'])
            instance = setup.Installer(bundle, publisher=publisher, preflight=lambda: None,
                native=lambda _: native, confirm=lambda _: next(answers), output=log)
            self.assertEqual(instance.run(), 1)
            self.assertNotEqual(fixture.snapshot(), before)
            installed = fixture.data / 'smith-next/bin/smith-service'
            self.assertEqual(installed.read_bytes(), fixture.service.read_bytes())
            self.assertIn('artifacts remain installed', log.getvalue())
            self.assertFalse(any(call[0] == 'apply' for call in native.calls))
            # A differing next candidate is still blocked, even after prior consent.
            for path in (bundle / 'payload/service/smith-service',
                         bundle / 'payload/client/smith_backend/smith-service'):
                path.write_bytes(b'differing-candidate')
            before = fixture.snapshot()
            instance.native = mock.Mock(side_effect=AssertionError('must not launch helper'))
            self.assertEqual(instance.run(), 1)
            self.assertEqual(fixture.snapshot(), before)
        finally:
            fixture.doCleanups()


if __name__ == '__main__':
    unittest.main()
