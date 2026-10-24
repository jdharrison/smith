import 'dart:async';
import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/agents_controller.dart';
import 'package:smith_client/app_api.dart';

import 'agents_protocol_test.dart' show instance, record;

const custom = 'dddddddddddddddddddddddddddddddd';
const created = 'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee';
const restarted = 'cccccccccccccccccccccccccccccccc';
typedef Response = FutureOr<AppReply> Function(Map<String, dynamic> args);

class AgentsApi implements AppApi {
  String instanceId = instance;
  int revision = 1;
  int disposed = 0;
  final agents = [record(), record('assurance'), record(custom)];
  final calls = <String>[];
  final arguments = <Map<String, dynamic>>[];
  final receipts = <String, Map<String, dynamic>>{};
  final responses = <String, List<Response>>{};
  void next(String op, Response response) =>
      responses.putIfAbsent(op, () => []).add(response);
  AppReply list() => AppReply(instanceId, {
    'revision': revision,
    'agents': jsonDecode(jsonEncode(agents)),
  });
  AppReply commit(Map<String, dynamic> args) {
    final id = args['agent_id'] ?? created;
    final index = agents.indexWhere((a) => a['agent_id'] == id);
    final updated = <String, dynamic>{
      for (final key in ['agent_id', 'name', 'description', 'prompt'])
        key: key == 'agent_id' ? id : args[key],
    };
    if (index < 0) {
      agents.add(updated);
    } else {
      agents[index] = updated;
    }
    final receipt = <String, dynamic>{
      'request_id': args['request_id'],
      'agent_id': id,
      'revision': ++revision,
    };
    receipts[args['request_id']] = receipt;
    return AppReply(instanceId, receipt);
  }

  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    calls.add(op);
    arguments.add(Map.of(args));
    final scripted = responses[op];
    if (scripted != null && scripted.isNotEmpty) {
      return scripted.removeAt(0)(args);
    }
    switch (op) {
      case 'agents_list_v2':
        return list();
      case 'agent_create_v1':
      case 'agent_update_v2':
        return commit(args);
      case 'agent_receipt':
        final receipt = receipts[args['request_id']];
        if (receipt == null) {
          throw const AppFailure('REQUEST_UNKNOWN', 'Unknown');
        }
        return AppReply(instanceId, receipt);
      default:
        throw StateError('Unexpected operation: $op');
    }
  }

  @override
  void dispose() => disposed++;
}

Future<void> save(
  AgentsController c, {
  String? origin,
  int? revision,
  String prompt = ' exact\n\t ',
}) => c.update(
  custom,
  'Changed',
  'Description',
  prompt,
  expectedInstanceId: origin ?? c.instanceId!,
  expectedRevision: revision ?? c.revision!,
);

void main() {
  late AgentsApi api;
  late AgentsController c;
  setUp(() {
    api = AgentsApi();
    c = AgentsController(api);
  });
  tearDown(() => c.dispose());

  Future<void> create({String? requestId, String prompt = ' exact\n '}) =>
      c.create(
        'Custom',
        'Description',
        prompt,
        expectedInstanceId: c.instanceId!,
        expectedRevision: c.revision!,
        requestId: requestId,
      );

  test('create uses supplied ID, reconciles and retains lastReceipt across dispatch', () async {
    await c.refresh();
    await create(requestId: restarted);
    expect(c.lastReceipt!.requestId, restarted);
    expect(c.lastReceipt!.agentId, created);
    expect(c.agents.last.agentId, created);
    expect(api.arguments[1].containsKey('agent_id'), false);
    final receipt = c.lastReceipt;
    final gate = Completer<AppReply>();
    api.next('agent_update_v2', (_) => gate.future);
    final action = save(c);
    expect(identical(c.lastReceipt, receipt), true);
    gate.complete(api.commit(api.arguments.last));
    await action;
    expect(c.lastReceipt!.agentId, custom);
    expect(c.lastReceipt!.requestId, isNot(restarted));
  });

  test(
    'cap32 blocks create but not edits; built-ins lock current metadata only',
    () async {
      for (var i = 0; i < 29; i++) {
        api.agents.add(record(i.toRadixString(16).padLeft(32, '0')));
      }
      api.agents[0]['name'] = 'Legacy saved name';
      api.agents[0]['description'] = '😀' * 256;
      await c.refresh();
      await create();
      expect(api.calls.length, 1);
      for (final fields in [
        ['Changed', '😀' * 256],
        ['Legacy saved name', 'changed'],
      ]) {
        await c.update(
          'developer',
          fields[0],
          fields[1],
          '',
          expectedInstanceId: c.instanceId!,
          expectedRevision: c.revision!,
        );
        expect(api.calls.length, 1);
      }
      await c.update(
        'developer',
        'Legacy saved name',
        '😀' * 256,
        'edited prompt',
        expectedInstanceId: c.instanceId!,
        expectedRevision: c.revision!,
        requestId: restarted,
      );
      expect(c.error, isNull);
      expect(c.lastReceipt!.requestId, restarted);
      expect(c.agents.first.prompt, 'edited prompt');
    },
  );

  test('lost create ack and restart query only original receipt, never duplicate matching profile', () async {
    await c.refresh();
    api.next('agent_create_v1', (args) {
      expect(() => args['name'] = 'tamper', throwsUnsupportedError);
      api.commit(args);
      throw const AppFailure('TRANSPORT', 'lost', unknown: true);
    });
    await create(requestId: restarted);
    expect(c.lastReceipt, isNull);
    expect(c.uncertain, true);
    api.instanceId = 'ffffffffffffffffffffffffffffffff';
    await c.refresh();
    api.next(
      'agent_receipt',
      (_) => throw const AppFailure('REQUEST_UNKNOWN', 'unknown'),
    );
    await c.resolvePending();
    await create();
    expect(c.uncertain, true);
    expect(c.lastReceipt, isNull);
    await c.resolvePending();
    expect(c.lastReceipt!.requestId, restarted);
    expect(c.lastReceipt!.agentId, created);
    expect(c.uncertain, false);
    expect(api.calls.where((op) => op == 'agent_create_v1').length, 1);
    expect(api.agents.where((a) => a['agent_id'] == created).length, 1);
    for (var i = 0; i < api.calls.length; i++) {
      if (api.calls[i] == 'agent_receipt') {
        expect(api.arguments[i], {'request_id': restarted});
      }
    }
  });

  for (final bad in ['builtin', 'missing', 'fields']) {
    test('create $bad acknowledgement cannot unblock duplication', () async {
      await c.refresh();
      api.next('agent_create_v1', (args) {
        final reply = api.commit(args);
        if (bad == 'builtin') {
          return AppReply(instance, {...reply.result, 'agent_id': 'developer'});
        }
        if (bad == 'missing') api.agents.removeLast();
        if (bad == 'fields') api.agents.last['prompt'] = 'mismatched';
        return reply;
      });
      await create(requestId: restarted);
      expect(c.uncertain, true);
      expect(c.canMutate, false);
      expect(c.lastReceipt == null, bad == 'builtin');
      await create();
      expect(api.calls.where((op) => op == 'agent_create_v1').length, 1);
      await c.resolvePending();
      expect(c.uncertain, bad != 'builtin');
    });
  }

  test(
    'create immutable capture and newer caller typing survives ack and restart',
    () async {
      await c.refresh();
      var draft = 'original';
      final gate = Completer<AppReply>();
      api.next('agent_create_v1', (_) => gate.future);
      final action = create(requestId: restarted, prompt: draft);
      draft = 'newer typing';
      api.instanceId = 'ffffffffffffffffffffffffffffffff';
      gate.complete(api.commit(api.arguments.last));
      await action;
      expect(c.agents.last.prompt, 'original');
      expect(draft, 'newer typing');
      expect(c.instanceId, api.instanceId);
      final count = api.calls.length;
      await c.update(
        created,
        'Custom',
        'Description',
        draft,
        expectedInstanceId: instance,
        expectedRevision: 2,
      );
      expect(api.calls.length, count);
      expect(draft, 'newer typing');
    },
  );

  test(
    'lazy shared controller only lists, updates and reads authoritative list',
    () async {
      expect(api.calls, isEmpty);
      expect(c.canMutate || c.loaded, false);
      final a = c.refresh();
      final b = c.refresh();
      expect(identical(a, b), true);
      await a;
      expect(c.agents.map((a) => a.agentId), [
        'developer',
        'assurance',
        custom,
      ]);
      await save(c);
      expect(api.calls, [
        'agents_list_v2',
        'agent_update_v2',
        'agents_list_v2',
      ]);
      expect(api.arguments[1], {
        'request_id': api.arguments[1]['request_id'],
        'instance_id': instance,
        'expected_revision': 1,
        'agent_id': custom,
        'name': 'Changed',
        'description': 'Description',
        'prompt': ' exact\n\t ',
      });
      expect(c.agents.last.prompt, ' exact\n\t ');
      expect(c.revision, 2);
      expect(c.canMutate, true);
      expect(() => c.agents.clear(), throwsUnsupportedError);
      expect(api.disposed, 0);
    },
  );

  test(
    'same revision restart and stale revision cannot dispatch old editor',
    () async {
      await c.refresh();
      api.instanceId = restarted;
      await c.refresh();
      await save(c, origin: instance, revision: 1);
      expect(api.calls, ['agents_list_v2', 'agents_list_v2']);
      expect(c.error, contains('restarted'));
      api.revision = 2;
      await c.refresh();
      await save(c, origin: restarted, revision: 1);
      expect(api.calls.where((op) => op == 'agent_update_v2'), isEmpty);
      expect(c.error, contains('changed'));
    },
  );

  test('lost acknowledgement resolves original receipt across restart without replay', () async {
    await c.refresh();
    api.next('agent_update_v2', (args) {
      expect(() => args['prompt'] = 'tamper', throwsUnsupportedError);
      api.commit(args);
      throw const AppFailure('TRANSPORT', 'lost', unknown: true);
    });
    await save(c);
    final original = api.arguments[1]['request_id'];
    expect(c.uncertain, true);
    expect(c.canMutate, false);
    await save(c);
    api.instanceId = restarted;
    await c.refresh();
    expect(c.uncertain, true);
    await c.resolvePending();
    expect(api.calls, [
      'agents_list_v2',
      'agent_update_v2',
      'agents_list_v2',
      'agent_receipt',
      'agents_list_v2',
    ]);
    expect(api.arguments[3], {'request_id': original});
    expect(c.instanceId, restarted);
    expect(c.uncertain, false);
    expect(c.canMutate, true);
  });

  for (final code in ['REQUEST_EXPIRED', 'REQUEST_UNKNOWN']) {
    test(
      '$code preserves pending identity while refresh observes newer state',
      () async {
        await c.refresh();
        api.next('agent_update_v2', (args) {
          api.commit(args);
          throw const AppFailure('TRANSPORT', 'lost', unknown: true);
        });
        await save(c);
        final id = api.arguments[1]['request_id'];
        for (var i = 0; i < 2; i++) {
          api.next(
            'agent_receipt',
            (_) => throw AppFailure(code, 'unavailable'),
          );
          await c.resolvePending();
          expect(c.uncertain, true);
          expect(c.canMutate, false);
          expect(c.loaded, true);
        }
        api.revision = 4;
        api.agents[2]['prompt'] = 'newer saved value';
        await c.refresh();
        await save(c);
        expect(c.agents.last.prompt, 'newer saved value');
        expect(c.revision, 4);
        expect(api.calls.where((op) => op == 'agent_update_v2').length, 1);
        expect(
          [
            for (var i = 0; i < api.calls.length; i++)
              if (api.calls[i] == 'agent_receipt') api.arguments[i],
          ],
          [
            {'request_id': id},
            {'request_id': id},
          ],
        );
      },
    );
  }

  test(
    'historical receipt never replaces newer state and regression is rejected',
    () async {
      await c.refresh();
      api.next('agent_update_v2', (args) {
        api.commit(args);
        throw const AppFailure('TRANSPORT', 'lost', unknown: true);
      });
      await save(c);
      api.revision = 5;
      api.agents[2]['name'] = 'Newer';
      await c.refresh();
      await c.resolvePending();
      expect(c.revision, 5);
      expect(c.agents.last.name, 'Newer');
      api.revision = 4;
      await c.refresh();
      expect(c.revision, 5);
      expect(c.canMutate, false);
      expect(c.error, isNotNull);
    },
  );

  test('acknowledged write with unavailable list remains query-only', () async {
    await c.refresh();
    api.next(
      'agents_list_v2',
      (_) => throw const AppFailure('TRANSPORT', 'lost'),
    );
    await save(c);
    expect(c.uncertain, true);
    await c.resolvePending();
    expect(c.canMutate, true);
    expect(api.calls, [
      'agents_list_v2',
      'agent_update_v2',
      'agents_list_v2',
      'agent_receipt',
      'agents_list_v2',
    ]);
  });

  for (final field in ['request_id', 'agent_id', 'revision']) {
    test(
      'mismatched $field receipt is uncertain and cannot update current data',
      () async {
        await c.refresh();
        api.next('agent_update_v2', (args) {
          final reply = api.commit(args);
          return AppReply(instance, {
            ...reply.result,
            field: switch (field) {
              'request_id' => restarted,
              'agent_id' => 'assurance',
              _ => 3,
            },
          });
        });
        await save(c);
        expect(c.uncertain, true);
        expect(c.revision, 1);
        await c.resolvePending();
        expect(c.uncertain, false);
        expect(c.revision, 2);
      },
    );
  }

  for (final code in ['OP', 'UPGRADE_REQUIRED', 'STALE', 'INSTANCE_MISMATCH']) {
    test('$code definitive rejection does not replay or fallback', () async {
      await c.refresh();
      api.next('agent_update_v2', (_) => throw AppFailure(code, 'rejected'));
      await save(c);
      await c.resolvePending();
      expect(c.uncertain, false);
      expect(c.canMutate, false);
      expect(api.calls, ['agents_list_v2', 'agent_update_v2']);
    });
  }

  test('invalid input and unavailable list dispatch no mutation', () async {
    await c.refresh();
    await save(c, prompt: '\ud800');
    expect(api.calls, ['agents_list_v2']);
    api.next(
      'agents_list_v2',
      (_) => throw const AppFailure('UPGRADE_REQUIRED', 'old peer'),
    );
    await c.refresh();
    await save(c);
    expect(api.calls, ['agents_list_v2', 'agents_list_v2']);
  });

  test(
    'busy coalescing and disposal fence late results without owning API',
    () async {
      await c.refresh();
      final gate = Completer<AppReply>();
      api.next('agent_update_v2', (_) => gate.future);
      final action = save(c);
      expect(c.busy, true);
      expect(identical(c.refresh(), c.resolvePending()), true);
      await save(c);
      expect(api.calls.length, 2);
      c.dispose();
      gate.complete(api.commit(api.arguments.last));
      await action;
      expect(c.revision, 1);
      expect(api.calls.length, 2);
      expect(api.disposed, 0);
    },
  );

  test(
    'safe integer revision exhaustion permits reads but blocks writes',
    () async {
      api.revision = 9007199254740991;
      await c.refresh();
      expect(c.loaded, true);
      expect(c.canMutate, false);
      await save(c);
      await c.refresh();
      expect(api.calls, ['agents_list_v2', 'agents_list_v2']);
    },
  );

  test(
    'receipt/list regression leaves the original pending attempt unresolved',
    () async {
      await c.refresh();
      api.next(
        'agents_list_v2',
        (_) => AppReply(instance, {
          'revision': 1,
          'agents': [record(), record('assurance')],
        }),
      );
      await save(c);
      expect(c.uncertain, true);
      expect(c.revision, 1);
      final id = api.arguments[1]['request_id'];
      await c.resolvePending();
      expect(api.arguments[3], {'request_id': id});
      expect(c.revision, 2);
      expect(c.canMutate, true);
    },
  );

  test('disposal inside loading notification makes no API call', () async {
    c.addListener(() {
      if (c.loading) c.dispose();
    });
    await c.refresh();
    expect(api.calls, isEmpty);
    expect(api.disposed, 0);
  });
}
