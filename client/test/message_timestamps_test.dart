import 'dart:async';
import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/app_v2.dart';
import 'package:smith_client/cloud_routing.dart';
import 'package:smith_client/message_timestamps.dart';
import 'package:smith_client/workspace_controller.dart';

import 'app_fakes.dart';

const accepted = 1792069392000;
const completed = accepted + 15000;
const other = '22222222222222222222222222222222';
Map<String, dynamic> timing({int revision = 1}) => {
  'conversation_id': testConversation,
  'history_revision': revision,
  'turns': [
    {'user_at_ms': accepted, 'assistant_at_ms': completed},
  ],
};

class TimedApi extends FakeAppApi {
  TimedApi() {
    inventoryObserved = true;
    summaries[testConversation]!['history_revision'] = 1;
    histories[testConversation] = [
      {'role': 'user', 'content': 'question'},
      {'role': 'assistant', 'content': 'answer'},
    ];
  }
  Map<String, dynamic> times = timing();
  AppFailure? timingFailure;
  String? timingInstance;
  Completer<AppReply>? timingGate;
  final timingRequested = Completer<void>();

  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    if (op != 'conversation_timestamps') return super.call(op, args);
    calls.add(op);
    arguments.add(Map.of(args));
    if (!timingRequested.isCompleted) timingRequested.complete();
    if (timingFailure != null) throw timingFailure!;
    if (timingGate != null) return timingGate!.future;
    return AppReply(timingInstance ?? instance, times);
  }
}

class TimingExecutor implements AppExecutor {
  final requests = <Map<String, dynamic>>[];
  Map<String, dynamic> result = timing();
  @override
  Future<String> exchange(List<int> frame) async {
    final request = jsonDecode(utf8.decode(frame)) as Map<String, dynamic>;
    requests.add(request);
    return jsonEncode({
      'schema_version': 3,
      'service': 'smith-next',
      'protocol': 'app',
      'pid': 123,
      'instance_id': testInstance,
      'call_id': request['call_id'],
      'ok': true,
      'result': result,
      'error': null,
    });
  }

  @override
  void dispose() {}
}

Future<WorkspaceController> workspace(TimedApi api) async {
  final w = WorkspaceController(
    api,
    timestampsEnabled: true,
    pollInterval: const Duration(days: 1),
  );
  addTearDown(w.dispose);
  await w.synchronize();
  expect(w.stale, false, reason: w.error);
  return w;
}

void main() {
  test('strict metadata accepts unknown, bounded and backwards wall times', () {
    final value = timing()
      ..['turns'] = [
        {'user_at_ms': null, 'assistant_at_ms': null},
        {'user_at_ms': 253402300799999, 'assistant_at_ms': 1},
      ];
    final parsed = ConversationTimestamps(value);
    expect(parsed.conversationId, testConversation);
    expect(parsed.historyRevision, 1);
    expect(parsed.messageTimes, [null, null, 253402300799999, 1]);
    expect(() => parsed.messageTimes.add(3), throwsUnsupportedError);
    validateV3('conversation_timestamps', value);
    validateV3Arguments('conversation_timestamps', {
      'conversation_id': testConversation,
    });
    expect(
      () => validateV2('conversation_timestamps', value),
      throwsA(isA<AppFailure>()),
    );
  });

  test(
    'strict metadata rejects missing, extra, invalid and excessive fields',
    () {
      final invalidValues = <Map<String, dynamic>>[
        timing()..remove('turns'),
        timing()..['extra'] = true,
        timing()..['conversation_id'] = 'invalid',
        timing()..['history_revision'] = -1,
        timing()..['history_revision'] = 9007199254740992,
        timing()..['turns'] = null,
        timing()..['turns'] = 'invalid',
        timing()..['turns'] = [null],
        timing()
          ..['turns'] = [
            {'user_at_ms': null},
          ],
        timing()
          ..['turns'] = [
            {'user_at_ms': null, 'assistant_at_ms': null, 'extra': 0},
          ],
        timing()
          ..['turns'] = List.generate(
            17,
            (_) => {'user_at_ms': null, 'assistant_at_ms': null},
          ),
        for (final key in ['user_at_ms', 'assistant_at_ms'])
          for (final value in [0, -1, 253402300800000, '123', 1.5, true])
            timing()
              ..['turns'] = [
                {'user_at_ms': null, 'assistant_at_ms': null, key: value},
              ],
      ];
      for (final value in invalidValues) {
        expect(
          () => validateV3('conversation_timestamps', value),
          throwsA(isA<AppFailure>()),
          reason: '$value',
        );
      }
      for (final args in <Map<String, dynamic>>[
        {},
        {'conversation_id': null},
        {'conversation_id': 'invalid'},
        {'conversation_id': testConversation, 'extra': 0},
      ]) {
        expect(
          () => validateV3Arguments('conversation_timestamps', args),
          throwsA(isA<AppFailure>()),
        );
      }
    },
  );

  test(
    'native adapter sends exact schema3 query and validates its response',
    () async {
      final executor = TimingExecutor();
      final api = NativeAppApi(executor: executor);
      addTearDown(api.dispose);
      final reply = await api.call('conversation_timestamps', {
        'conversation_id': testConversation,
      });
      expect(ConversationTimestamps(reply.result).messageTimes, [
        accepted,
        completed,
      ]);
      expect(executor.requests.single['schema_version'], 3);
      expect(executor.requests.single['op'], 'conversation_timestamps');
      expect(executor.requests.single['args'], {
        'conversation_id': testConversation,
      });
      executor.result = timing()..remove('history_revision');
      await expectLater(
        api.call('conversation_timestamps', {
          'conversation_id': testConversation,
        }),
        throwsA(isA<AppFailure>()),
      );
      final count = executor.requests.length;
      await expectLater(
        api.call('conversation_timestamps', {
          'conversation_id': testConversation,
          'extra': true,
        }),
        throwsA(isA<AppFailure>()),
      );
      expect(executor.requests.length, count);
    },
  );

  test('times match saved history, remain stable on reload and are shared across panes', () async {
    final api = TimedApi();
    final w = await workspace(api);
    final c = w.selected;
    expect(c.historyTimestamps, [accepted, completed]);
    expect(c.messageTimestampAt(0), accepted);
    expect(c.messageTimestampAt(1), completed);
    expect(c.messageTimestampAt(-1), isNull);
    expect(c.messageTimestampAt(2), isNull);
    expect(c.history.first.keys, ['role', 'content']);
    final pane = w.createPane();
    await w.selectFor(pane, testConversation);
    expect(pane.selected, same(c));
    expect(pane.selected.historyTimestamps, [accepted, completed]);
    await w.select(testConversation);
    expect(c.historyTimestamps, [accepted, completed]);
    expect(api.calls.where((op) => op.startsWith('submit')), isEmpty);
  });

  test('old and new pairs keep nullable metadata aligned, never inferred from text', () async {
    final api = TimedApi();
    api.histories[testConversation]!.addAll(
      api.histories[testConversation]!.map(Map<String, dynamic>.of).toList(),
    );
    api.times['turns'] = [
      {'user_at_ms': null, 'assistant_at_ms': null},
      {'user_at_ms': accepted, 'assistant_at_ms': completed},
    ];
    final w = await workspace(api);
    expect(w.selected.historyTimestamps, [null, null, accepted, completed]);
    expect(w.selected.messageTimestampAt(0), isNull);
    expect(w.selected.messageTimestampAt(2), accepted);
  });

  for (final mismatch in [
    'instance',
    'conversation',
    'revision',
    'count',
    'malformed',
  ]) {
    test(
      '$mismatch mismatch discards metadata without hiding saved history',
      () async {
        final api = TimedApi();
        final w = await workspace(api);
        expect(w.selected.historyTimestamps, [accepted, completed]);
        switch (mismatch) {
          case 'instance':
            api.timingInstance = other;
          case 'conversation':
            api.times['conversation_id'] = other;
          case 'revision':
            api.times['history_revision'] = 2;
          case 'count':
            api.times['turns'] = [];
          case 'malformed':
            api.times.remove('history_revision');
        }
        await w.select(testConversation);
        expect(w.selected.history, hasLength(2));
        expect(w.selected.historyTimestamps, isEmpty);
        expect(w.selected.messageTimestampAt(0), isNull);
        w.selected.edit('next');
        expect(w.selected.canSend, true);
        expect(api.calls.where((op) => op.startsWith('submit')), isEmpty);
      },
    );
  }

  for (final failure in ['OP', 'TRANSPORT', 'STATE_UNAVAILABLE']) {
    test(
      'optional $failure leaves old and newly synchronized chat usable',
      () async {
        final api = TimedApi()
          ..timingFailure = AppFailure(failure, 'Synthetic timestamp failure');
        final w = await workspace(api);
        expect(w.selected.historyTimestamps, isEmpty);
        expect(w.selected.history, hasLength(2));
        w.selected.edit('new message');
        expect(w.selected.canSend, true);
        await w.selected.send();
        expect(w.selected.pendingId, isNull);
        expect(w.selected.provisionalAnswers, isEmpty);
        expect(w.selected.history, hasLength(4));
        expect(w.selected.historyTimestamps, isEmpty);
        expect(api.calls.where((op) => op == 'submit'), hasLength(1));
        expect(w.selected.canSubmit, true);
      },
    );
  }

  test('new saved snapshot replaces old timing instead of reusing equal-sized metadata', () async {
    final api = TimedApi();
    final w = await workspace(api);
    api.summaries[testConversation]!['history_revision'] = 2;
    api.histories[testConversation] = [
      {'role': 'user', 'content': 'replacement'},
      {'role': 'assistant', 'content': 'new reply'},
    ];
    api.timingFailure = const AppFailure(
      'TRANSPORT',
      'Synthetic missing metadata',
    );
    await w.synchronize();
    expect(w.selected.history.first['content'], 'replacement');
    expect(w.selected.historyTimestamps, isEmpty);
  });

  test(
    'late response cannot attach to a changed client history snapshot',
    () async {
      final api = TimedApi()..timingGate = Completer<AppReply>();
      final w = WorkspaceController(api, timestampsEnabled: true);
      addTearDown(w.dispose);
      final read = w.synchronize();
      await api.timingRequested.future;
      w.selected.history = List.of(w.selected.history);
      api.timingGate!.complete(AppReply(testInstance, timing()));
      await read;
      expect(w.selected.history, hasLength(2));
      expect(w.selected.historyTimestamps, isEmpty);
    },
  );

  test(
    'empty conversations have no fabricated times or metadata request',
    () async {
      final api = TimedApi()..histories[testConversation] = [];
      final w = await workspace(api);
      expect(w.selected.historyTimestamps, isEmpty);
      expect(api.calls, isNot(contains('conversation_timestamps')));
    },
  );
}
