import 'dart:async';
import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/app_v2.dart';
import 'package:smith_client/workspace_controller.dart';

import 'app_fakes.dart';

const other = '22222222222222222222222222222222';
Future<WorkspaceController> ready(FakeAppApi api) async {
  api.inventoryObserved = true;
  final w = WorkspaceController(api, pollInterval: const Duration(days: 1));
  await w.synchronize();
  expect(w.stale, false, reason: w.error);
  return w;
}

void addOther(FakeAppApi api) {
  api.summaries[other] = api.summary(other, 'Other', 1);
  api.histories[other] = [];
}

void main() {
  test(
    'four outstanding chats are coalesced and fifth admission is disabled',
    () async {
      final api = FakeAppApi();
      final ids = [
        testConversation,
        other,
        '33333333333333333333333333333333',
        '44444444444444444444444444444444',
      ];
      for (var i = 0; i < ids.length; i++) {
        final id = ids[i];
        api.summaries[id] = api.summary(id, 'Chat $i', i);
        api.histories[id] = [];
        api.jobs[id] = {
          'request_id': id,
          'conversation_id': id,
          'kind': 'submit',
          'status': i == 0 ? 'running' : 'queued',
          'phase': i == 0 ? 'generation' : 'queued',
          'config_revision': 1,
          'result': null,
          'error': null,
        };
      }
      final w = await ready(api);
      addTearDown(w.dispose);
      expect(api.calls.where((op) => op == 'poll'), hasLength(4));
      expect(
        w.conversations.values.where((c) => c.pendingId != null),
        hasLength(4),
      );
      w.newChat();
      w.selected.edit('fifth');
      expect(w.selected.canSend, false);
      await w.selected.send();
      expect(api.calls.where((op) => op.startsWith('submit')), isEmpty);
    },
  );
  test('missing reattach inventory refreshes once only after active work becomes idle', () async {
    final api = FakeAppApi();
    api.jobs[testInstance] = {
      'request_id': testInstance,
      'conversation_id': testConversation,
      'kind': 'submit',
      'status': 'running',
      'phase': 'generation',
      'config_revision': 1,
      'result': null,
      'error': null,
    };
    final w = WorkspaceController(
      api,
      pollInterval: const Duration(milliseconds: 5),
    );
    addTearDown(w.dispose);
    await w.synchronize();
    await w.refreshModels();
    expect(api.calls, isNot(contains('models')));
    api.jobs[testInstance] = {
      ...api.jobs[testInstance]!,
      'status': 'succeeded',
      'phase': 'complete',
      'result': {'assistant_text': 'done', 'audio': null, 'speech_error': null},
    };
    await Future<void>.delayed(const Duration(milliseconds: 60));
    expect(api.calls.where((op) => op == 'models'), hasLength(1));
    expect(w.inventoryObserved, true);
    expect(api.calls.where((op) => op.startsWith('submit')), isEmpty);
  });
  test('New is one ephemeral draft; atomic promotion preserves presentation and newer draft', () async {
    final api = FakeAppApi()..holdChats = true;
    final w = await ready(api);
    addTearDown(w.dispose);
    w.selected.edit('saved chat draft');
    w.newChat();
    final blank = w.selected;
    blank.edit('first message');
    w.newChat();
    expect(w.selected, same(blank));
    expect(api.calls.where((s) => s.startsWith('submit')), isEmpty);
    await blank.send();
    expect(blank.id, isNotNull);
    expect(w.selected, same(blank));
    expect(w.blank, isNot(same(blank)));
    expect(api.calls.where((s) => s == 'submit_new'), hasLength(1));
    expect(api.calls.where((s) => s == 'submit'), isEmpty);
    blank.edit('newer draft');
    final request = blank.pendingId!;
    api.jobs[request] = {
      ...api.jobs[request]!,
      'status': 'succeeded',
      'phase': 'complete',
      'result': {'assistant_text': 'done', 'audio': null, 'speech_error': null},
    };
    api.histories[blank.id!] = [
      {'role': 'user', 'content': 'first message'},
      {'role': 'assistant', 'content': 'done'},
    ];
    await w.synchronize();
    expect(blank.draft, 'newer draft');
    expect(blank.pendingId, isNull);
    expect(blank.history.last['content'], 'done');
    expect(w.conversations[testConversation]!.draft, 'saved chat draft');
  });
  test(
    'Boot snapshot during A adopts current-instance inventory and can submit B',
    () async {
      final api = FakeAppApi()..holdChats = true;
      addOther(api);
      api.jobs[testInstance] = {
        'request_id': testInstance,
        'conversation_id': testConversation,
        'kind': 'submit',
        'status': 'running',
        'phase': 'generation',
        'config_revision': 1,
        'result': null,
        'error': null,
      };
      final w = await ready(api);
      addTearDown(w.dispose);
      expect(w.conversations[testConversation]!.pendingId, testInstance);
      await w.select(other);
      w.selected.edit('B');
      expect(w.selected.canSend, true);
      await w.selected.send();
      expect(api.calls.where((s) => s == 'submit'), hasLength(1));
      expect(w.conversations[testConversation]!.pendingId, testInstance);
      expect(w.selected.pendingId, isNotNull);
      expect(w.busy, false);
      expect(api.calls, isNot(contains('models')));
    },
  );
  test(
    'lost submit_new acknowledgement discovers origin without replay',
    () async {
      final api = FakeAppApi();
      final w = await ready(api);
      addTearDown(w.dispose);
      w.newChat();
      final c = w.selected;
      c.edit('new');
      api.unknownAdmission = true;
      await c.send();
      final id = c.pendingId;
      expect(c.id, isNull);
      c.edit('keep');
      await w.synchronize();
      expect(w.selected, same(c));
      expect(c.id, isNotNull);
      expect(c.pendingId, isNull);
      expect(c.draft, 'keep');
      expect(api.calls.where((s) => s == 'submit_new'), hasLength(1));
      expect(
        api.arguments.where((a) => a['request_id'] == id).length,
        greaterThan(1),
      );
    },
  );
  test('verified terminal result survives detail failure and cannot clear newer draft', () async {
    final api = FakeAppApi();
    final w = await ready(api);
    addTearDown(w.dispose);
    final c = w.selected;
    c.edit('A');
    api.pollGate = Completer<AppReply>();
    final sending = c.send();
    for (var i = 0; i < 10; i++) {
      await Future<void>.delayed(Duration.zero);
    }
    c.edit('newer');
    api.detailFailure = true;
    api.pollGate!.complete(AppReply(testInstance, api.jobs[c.pendingId]!));
    await sending;
    expect(c.provisionalAnswers.values, ['useful answer']);
    expect(c.pendingId, isNull);
    expect(c.draft, 'newer');
    expect(c.stale, true);
    expect(c.canSend, false);
    api.pollGate = null;
    api.detailFailure = false;
    await w.synchronize();
    expect(c.provisionalAnswers, isEmpty);
    expect(c.draft, 'newer');
  });
  test('terminal submit_new survives failed promotion detail', () async {
    final api = FakeAppApi();
    final w = await ready(api);
    addTearDown(w.dispose);
    w.newChat();
    final c = w.selected;
    c.edit('first');
    api.detailFailure = true;
    await c.send();
    expect(c.id, isNull);
    expect(c.provisionalAnswers.values, ['useful answer']);
    expect(c.pendingId, isNotNull);
    expect(c.draft, isEmpty);
    c.edit('next');
    api.detailFailure = false;
    await w.synchronize();
    expect(w.selected, same(c));
    expect(c.id, isNotNull);
    expect(c.draft, 'next');
  });
  test(
    'target-only archive/delete restrictions; rename during generation',
    () async {
      final api = FakeAppApi()..holdChats = true;
      addOther(api);
      final w = await ready(api);
      addTearDown(w.dispose);
      await w.select(testConversation);
      final a = w.selected;
      a.edit('A');
      await a.send();
      await expectLater(
        w.mutate('conversation_archive', a.id!, 0, 0),
        throwsA(isA<AppFailure>()),
      );
      await expectLater(
        w.mutate('conversation_delete', a.id!, 0, 0),
        throwsA(isA<AppFailure>()),
      );
      await w.mutate('conversation_rename', a.id!, 0, 0, title: 'Renamed');
      expect(a.summary!['title'], 'Renamed');
      expect(a.pendingId, isNotNull);
      await w.mutate('conversation_archive', other, 0, 0);
      expect(w.conversations[other]!.archived, true);
      await expectLater(
        w.mutate('conversation_rename', a.id!, 0, 0, title: 'Stale'),
        throwsA(isA<AppFailure>()),
      );
    },
  );
  test('archive/delete fallback preserves ephemeral draft; restore does not select', () async {
    final api = FakeAppApi();
    final w = await ready(api);
    addTearDown(w.dispose);
    w.newChat();
    final blank = w.blank;
    blank.edit('unsaved');
    await w.select(testConversation);
    await w.mutate('conversation_archive', testConversation, 0, 0);
    expect(w.selected, same(blank));
    expect(blank.draft, 'unsaved');
    await w.mutate('conversation_restore', testConversation, 1, 0);
    expect(w.selected, same(blank));
    await w.select(testConversation);
    await w.mutate('conversation_delete', testConversation, 2, 0);
    expect(w.selected, same(blank));
    expect(w.conversations, isEmpty);
    await w.synchronize();
    expect(w.stale, false, reason: w.error);
  });
  test(
    'lost lifecycle acknowledgement queries original receipt once',
    () async {
      final api = FakeAppApi();
      final w = await ready(api);
      addTearDown(w.dispose);
      api.unknownAdmission = true;
      await expectLater(
        w.mutate('conversation_archive', testConversation, 0, 0),
        throwsA(isA<AppFailure>()),
      );
      expect(w.mutations, isNotEmpty);
      await w.synchronize();
      expect(w.mutations, isEmpty);
      expect(w.selected, same(w.blank));
      expect(api.calls.where((s) => s == 'conversation_archive'), hasLength(1));
    },
  );
  test(
    'polling budget is finite and explicit Retry remains query-only',
    () async {
      final api = FakeAppApi()
        ..inventoryObserved = true
        ..holdChats = true;
      final w = WorkspaceController(
        api,
        pollInterval: const Duration(milliseconds: 5),
        operationBudget: const Duration(milliseconds: 10),
        queueBudget: Duration.zero,
      );
      addTearDown(w.dispose);
      await w.synchronize();
      w.selected.edit('work');
      await w.selected.send();
      await Future<void>.delayed(const Duration(milliseconds: 50));
      final count = api.calls.where((op) => op == 'poll').length;
      expect(w.selected.pendingId, isNotNull);
      expect(w.selected.uncertain, true);
      await Future<void>.delayed(const Duration(milliseconds: 30));
      expect(api.calls.where((op) => op == 'poll').length, count);
      await w.synchronize();
      expect(api.calls.where((op) => op == 'poll').length, greaterThan(count));
      expect(api.calls.where((op) => op == 'submit'), hasLength(1));
    },
  );
  test('stale global observation disables all lifecycle mutations', () async {
    final api = FakeAppApi();
    final w = await ready(api);
    addTearDown(w.dispose);
    api.stateFailure = true;
    await w.synchronize();
    await expectLater(
      w.mutate('conversation_rename', testConversation, 0, 0, title: 'No'),
      throwsA(isA<AppFailure>()),
    );
    expect(api.calls.where((op) => op == 'conversation_rename'), isEmpty);
  });
  test(
    'invalid title is rejected locally without a pending mutation',
    () async {
      final api = FakeAppApi();
      final w = await ready(api);
      addTearDown(w.dispose);
      await expectLater(
        w.mutate(
          'conversation_rename',
          testConversation,
          0,
          0,
          title: 'bad\u200e',
        ),
        throwsA(isA<AppFailure>()),
      );
      expect(w.mutations, isEmpty);
      expect(api.calls.where((op) => op == 'conversation_rename'), isEmpty);
    },
  );
  test(
    'shared API disposed exactly once, not on selection or promotion',
    () async {
      final api = FakeAppApi();
      final w = await ready(api);
      w.newChat();
      w.selected.edit('create');
      await w.selected.send();
      await w.select(testConversation);
      expect(api.disposedCount, 0);
      w.dispose();
      expect(api.disposedCount, 1);
    },
  );
  test('strict state and cross-conversation poll fail closed', () async {
    final api = FakeAppApi();
    final w = await ready(api);
    addTearDown(w.dispose);
    final bad = Map<String, dynamic>.from(api.state)..['extra'] = true;
    expect(() => validateV2('state', bad), throwsA(isA<AppFailure>()));
    api.pollGate = Completer<AppReply>();
    final c = w.selected;
    c.edit('A');
    final sending = c.send();
    for (var i = 0; i < 10; i++) {
      await Future<void>.delayed(Duration.zero);
    }
    api.pollGate!.complete(
      AppReply(testInstance, {
        ...api.jobs[c.pendingId]!,
        'conversation_id': other,
      }),
    );
    await sending;
    expect(c.pendingId, isNotNull);
    expect(c.provisionalAnswers, isEmpty);
    expect(c.uncertain, true);
  });
  test(
    'native arbiter serializes individual exchanges FIFO and survives failures',
    () async {
      final executor = _SerialExecutor();
      final api = NativeAppApi(executor: executor);
      addTearDown(api.dispose);
      final first = api.call('state', {});
      final second = api.call('state', {});
      final third = api.call('state', {});
      await Future<void>.delayed(Duration.zero);
      expect(executor.calls, 1);
      executor.gates[0].complete();
      await first;
      await Future<void>.delayed(Duration.zero);
      expect(executor.calls, 2);
      final rejected = expectLater(second, throwsA(isA<AppFailure>()));
      executor.gates[1].completeError(
        const AppFailure('TRANSPORT', 'fixture failure'),
      );
      await rejected;
      await Future<void>.delayed(Duration.zero);
      executor.gates[2].complete();
      await third;
      expect(executor.maxActive, 1);
    },
  );
}

class _SerialExecutor implements AppExecutor {
  int calls = 0, active = 0, maxActive = 0;
  final gates = <Completer<void>>[];
  @override
  Future<String> exchange(List<int> frame) async {
    calls++;
    active++;
    if (active > maxActive) maxActive = active;
    final req = jsonDecode(utf8.decode(frame));
    expect(req['schema_version'], 3);
    final gate = Completer<void>();
    gates.add(gate);
    try {
      await gate.future;
    } finally {
      active--;
    }
    return jsonEncode({
      'schema_version': 3,
      'service': 'smith-next',
      'protocol': 'app',
      'pid': 123,
      'instance_id': testInstance,
      'call_id': req['call_id'],
      'ok': true,
      'result': FakeAppApi().state,
      'error': null,
    });
  }

  @override
  void dispose() {}
}
