import 'dart:async';
import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/agents.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/cloud_routing.dart';

const instance = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
const request = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';
Map<String, dynamic> record([String id = 'developer']) => {
  'agent_id': id,
  'name': id,
  'description': '',
  'prompt': ' exact\n\ttext ',
};
Map<String, dynamic> snapshot() => {
  'revision': 1,
  'agents': [record(), record('assurance')],
};
Map<String, dynamic> arguments() => {
  'request_id': request,
  'instance_id': instance,
  'expected_revision': 1,
  ...record(),
};
Map<String, dynamic> receipt() => {
  'request_id': request,
  'agent_id': 'developer',
  'revision': 2,
};

class AgentExecutor implements AppExecutor {
  final requests = <Map<String, dynamic>>[];
  Map<String, dynamic> result = snapshot();
  Map<String, dynamic>? error;
  Completer<void>? gate;
  @override
  Future<String> exchange(List<int> frame) async {
    final request = jsonDecode(utf8.decode(frame)) as Map<String, dynamic>;
    requests.add(request);
    await gate?.future;
    return jsonEncode({
      'schema_version': 3,
      'service': 'smith-next',
      'protocol': 'app',
      'call_id': request['call_id'],
      'pid': 1,
      'instance_id': instance,
      'ok': error == null,
      'result': error == null ? result : null,
      'error': error,
    });
  }

  @override
  void dispose() {}
}

void main() {
  test('custom IDs, registry bounds, duplicate names and legacy built-ins', () {
    for (final id in [request, '0' * 32, 'f' * 32]) {
      expect(agentWireId(id), true);
      expect(AgentRecord.parse(record(id)).isBuiltIn, false);
    }
    for (final id in [
      request.toUpperCase(),
      'a' * 31,
      'a' * 33,
      '$request\n',
      'g' * 32,
    ]) {
      expect(agentWireId(id), false);
    }
    final records = [
      record(),
      record('assurance'),
      for (var i = 0; i < 30; i++)
        record(i.toRadixString(16).padLeft(32, '0'))..['name'] = 'Same',
    ];
    expect(
      AgentSnapshot(1, records.map(AgentRecord.parse).toList()).agents.length,
      maxAgents,
    );
    expect(
      () => AgentSnapshot.parse({
        'revision': 1,
        'agents': [...records, record(request)],
      }),
      throwsA(isA<AppFailure>()),
    );
    expect(
      () => AgentSnapshot.parse({
        'revision': 1,
        'agents': [...records, record(request)]
          ..removeAt(2)
          ..add(record(request)),
      }),
      throwsA(isA<AppFailure>()),
    );
    expect(
      () => AgentSnapshot.parse({
        'revision': 1,
        'agents': [
          record(),
          record('assurance'),
          record(request),
          record(request),
        ],
      }),
      throwsA(isA<AppFailure>()),
    );
    expect(
      () => AgentSnapshot.parse({
        'revision': 1,
        'agents': [record(request), record('assurance'), record()],
      }),
      throwsA(isA<AppFailure>()),
    );
    final legacy = record()..['description'] = '😀' * 256;
    expect(AgentRecord.parse(legacy).isBuiltIn, true);
    validateAgentArguments('agent_update_v2', {...arguments(), ...legacy});
    expect(
      () => AgentRecord.parse({...legacy, 'agent_id': request}),
      throwsA(isA<AppFailure>()),
    );
    expect(
      () => AgentRecord.parse({...legacy, 'description': '😀' * 257}),
      throwsA(isA<AppFailure>()),
    );
    expect(
      () => AgentRecord.parse(record(request)..['description'] = '😀' * 41),
      throwsA(isA<AppFailure>()),
    );
  });

  test(
    'create is strict, captured before queueing, and rejects builtin receipts',
    () async {
      final executor = AgentExecutor()..gate = Completer<void>();
      final api = NativeAppApi(executor: executor);
      addTearDown(api.dispose);
      final first = api.call('agents_list_v2', {});
      final args = arguments()..remove('agent_id');
      final creation = api.call('agent_create_v1', args);
      args['name'] = 'later';
      executor.gate!.complete();
      await first;
      executor.result = receipt()..['agent_id'] = request;
      await creation;
      expect(executor.requests.last['args'], arguments()..remove('agent_id'));
      final createArgs = arguments()..remove('agent_id');
      for (final bad in [
        {...createArgs, 'agent_id': request},
        {...createArgs, 'description': '😀' * 41},
        for (final key in createArgs.keys) {...createArgs}..remove(key),
      ]) {
        expect(
          () => validateV3Arguments('agent_create_v1', bad),
          throwsA(isA<AppFailure>()),
        );
      }
      for (final bad in [
        receipt(),
        receipt()..['agent_id'] = 'assurance',
        receipt()
          ..['agent_id'] = request
          ..['revision'] = 3,
        receipt()
          ..['agent_id'] = request
          ..['request_id'] = instance,
      ]) {
        executor.result = bad;
        await expectLater(
          api.call('agent_create_v1', createArgs),
          throwsA(isA<AppFailure>()),
        );
      }
      final count = executor.requests.length;
      for (final op in ['agents_list_v1', 'agent_update_v1']) {
        await expectLater(api.call(op, {}), throwsA(isA<AppFailure>()));
      }
      expect(executor.requests.length, count);
    },
  );

  test('duplicate decoded agent keys are rejected before JSON collapse', () {
    final frame = jsonEncode({
      'schema_version': 3,
      'service': 'smith-next',
      'protocol': 'app',
      'call_id': request,
      'pid': 1,
      'instance_id': instance,
      'ok': true,
      'result': snapshot(),
      'error': null,
    });
    for (final duplicate in [
      '"agent_id":"developer","agent_id":"developer"',
      r'"agent_id":"developer","\u0061gent_id":"developer"',
    ]) {
      expect(
        () => AppReply.parse(
          frame.replaceFirst('"agent_id":"developer"', duplicate),
          request,
          schemaVersion: 3,
        ),
        throwsA(isA<AppFailure>().having((e) => e.code, 'code', 'PROTOCOL')),
      );
    }
  });

  test(
    'invalid outgoing arguments never cross the executor boundary',
    () async {
      final executor = AgentExecutor();
      final api = NativeAppApi(executor: executor);
      addTearDown(api.dispose);
      for (final args in [
        arguments()..['agent_id'] = request.toUpperCase(),
        arguments()..['instance_id'] = 'developer',
        arguments()..['request_id'] = '$request\n',
        arguments()..['description'] = 'a\u2028b',
        arguments()..['prompt'] = '\ud800',
        arguments()..['expected_revision'] = 0,
        arguments()..['extra'] = true,
      ]) {
        await expectLater(
          api.call('agent_update_v2', args),
          throwsA(isA<AppFailure>()),
        );
      }
      expect(executor.requests, isEmpty);
    },
  );
  test('immutable records have all-field value equality and fresh JSON', () {
    final a = AgentRecord.parse(record());
    final b = AgentRecord.parse(record());
    expect(a, b);
    expect(a.hashCode, b.hashCode);
    for (final key in ['name', 'description', 'prompt']) {
      expect(a, isNot(AgentRecord.parse(record()..[key] = 'different')));
    }
    a.toJson()['name'] = 'changed';
    expect(a.name, 'developer');
    final list = AgentSnapshot.parse(snapshot()).agents;
    expect(() => list.clear(), throwsUnsupportedError);
  });

  test('scalar and exact UTF8 boundaries, no silent normalization', () {
    expect(agentNameError('😀' * 40), isNull);
    expect(agentNameError('😀' * 41), isNotNull);
    expect(agentDescriptionError('😀' * 40), isNull);
    expect(agentDescriptionError('😀' * 41), isNotNull);
    expect(agentPromptError('😀' * 1024), isNull);
    expect(agentPromptError('${'😀' * 1024}x'), isNotNull);
    expect(agentDescriptionError(''), isNull);
    expect(agentPromptError(''), isNull);
    expect(agentPromptError(' \n\t '), isNull);
    for (final value in [
      '',
      ' a',
      'a ',
      '\t',
      '\ud800',
      '\udfff',
      'a\u0085b',
    ]) {
      expect(agentNameError(value), isNotNull, reason: value);
    }
    for (final value in [
      ' a',
      'a ',
      'a\nb',
      'a\tb',
      'a\u2028b',
      'a\u2029b',
      '\ud800',
    ]) {
      expect(agentDescriptionError(value), isNotNull, reason: value);
    }
    for (var code = 0; code <= 0x9f; code++) {
      if (code >= 0x20 && code < 0x7f) continue;
      final value = 'a${String.fromCharCode(code)}b';
      expect(agentNameError(value), isNotNull);
      expect(agentDescriptionError(value), isNotNull);
      expect(
        agentPromptError(value),
        code == 9 || code == 10 ? isNull : isNotNull,
      );
    }
    expect(agentPromptError('\ud800'), isNotNull);
    expect(AgentRecord.parse(record()).prompt, ' exact\n\ttext ');
  });

  test('registry requires built-ins first and unique canonical IDs', () {
    for (final agents in [
      [],
      [record()],
      [record(), record()],
      [record('assurance'), record()],
      [record(), record('other')],
      [record(), record('assurance'), record()],
    ]) {
      expect(
        () => AgentSnapshot.parse({'revision': 1, 'agents': agents}),
        throwsA(isA<AppFailure>()),
      );
    }
    for (final revision in [null, 0, -1, 1.0, 9007199254740992, '1']) {
      expect(
        () => AgentSnapshot.parse(snapshot()..['revision'] = revision),
        throwsA(isA<AppFailure>()),
      );
    }
    for (final id in [
      request.toUpperCase(),
      'Developer',
      'developer\n',
      '',
      null,
    ]) {
      expect(
        () => AgentRecord.parse(record()..['agent_id'] = id),
        throwsA(isA<AppFailure>()),
      );
    }
  });

  test('all keys required, unknown keys and positional shapes rejected', () {
    for (final entry in record().keys) {
      expect(
        () => AgentRecord.parse(record()..remove(entry)),
        throwsA(isA<AppFailure>()),
      );
    }
    expect(
      () => AgentRecord.parse(record()..['icon'] = 'code'),
      throwsA(isA<AppFailure>()),
    );
    expect(
      () => AgentRecord.parse(record().values.toList()),
      throwsA(isA<AppFailure>()),
    );
    for (final key in arguments().keys) {
      expect(
        () => validateV3Arguments('agent_update_v2', arguments()..remove(key)),
        throwsA(isA<AppFailure>()),
      );
    }
    expect(
      () => validateV3Arguments('agents_list_v2', {'extra': true}),
      throwsA(isA<AppFailure>()),
    );
    expect(
      () => validateV3('agent_receipt', receipt()..['deleted'] = false),
      throwsA(isA<AppFailure>()),
    );
  });

  test(
    'native dispatch exact App3 operations and receipt correlation',
    () async {
      final executor = AgentExecutor();
      final api = NativeAppApi(executor: executor);
      addTearDown(api.dispose);
      await api.call('agents_list_v2', {});
      executor.result = receipt();
      await api.call('agent_update_v2', arguments());
      await api.call('agent_receipt', {'request_id': request});
      expect(executor.requests.map((r) => r['op']), [
        'agents_list_v2',
        'agent_update_v2',
        'agent_receipt',
      ]);
      expect(executor.requests[1]['args'], arguments());
      expect(executor.requests.every((r) => r['schema_version'] == 3), true);
      for (final wrong in [
        receipt()..['request_id'] = instance,
        receipt()..['agent_id'] = 'assurance',
        receipt()..['revision'] = 3,
      ]) {
        executor.result = wrong;
        await expectLater(
          api.call('agent_update_v2', arguments()),
          throwsA(isA<AppFailure>()),
        );
      }
      executor.result = receipt()..['request_id'] = instance;
      await expectLater(
        api.call('agent_receipt', {'request_id': request}),
        throwsA(isA<AppFailure>()),
      );
    },
  );

  test(
    'queued API arguments are captured before caller modification',
    () async {
      final executor = AgentExecutor()..gate = Completer<void>();
      final api = NativeAppApi(executor: executor);
      addTearDown(api.dispose);
      final first = api.call('agents_list_v2', {});
      final args = arguments();
      final second = api.call('agent_update_v2', args);
      args['prompt'] = 'later';
      executor.gate!.complete();
      await first;
      executor.result = receipt();
      await second;
      expect(executor.requests.last['args'], arguments());
    },
  );

  test('unsupported peers do not trigger fallback or extra calls', () async {
    final executor = AgentExecutor()
      ..error = {'code': 'UPGRADE_REQUIRED', 'message': 'Upgrade pair'};
    final api = NativeAppApi(executor: executor);
    addTearDown(api.dispose);
    await expectLater(
      api.call('agents_list_v2', {}),
      throwsA(isA<AppFailure>()),
    );
    expect(executor.requests.length, 1);
    await expectLater(
      api.call('agent_create_v1', {}),
      throwsA(isA<AppFailure>()),
    );
    expect(executor.requests.length, 1);
  });
}
