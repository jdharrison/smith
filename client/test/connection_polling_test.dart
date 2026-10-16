import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/main.dart';
import 'package:smith_client/workspace_controller.dart';

import 'app_fakes.dart';
import 'widget_test.dart' show FakeControlApi, profile;

class ObservedApi extends FakeAppApi {
  Completer<void>? stateGate;
  Completer<void>? detailGate;
  String? unavailableReceipt;
  int activeCalls = 0, maxActive = 0, states = 0;
  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    activeCalls++;
    if (activeCalls > maxActive) maxActive = activeCalls;
    try {
      if (op == 'poll' &&
          args['request_id'] == unavailableReceipt &&
          unavailableReceipt != null) {
        throw const AppFailure('TRANSPORT', 'Receipt unavailable');
      }
      if (op == 'conversation_get') await detailGate?.future;
      if (op == 'state') {
        states++;
        await stateGate?.future;
      }
      return await super.call(op, args);
    } finally {
      activeCalls--;
    }
  }
}

// Stopwatch's production monotonic semantics driven by the widget fake clock.
class TestStopwatch implements Stopwatch {
  TestStopwatch(this.now);
  final DateTime Function() now;
  Duration accumulated = Duration.zero;
  DateTime? started;
  @override
  bool get isRunning => started != null;
  @override
  Duration get elapsed =>
      accumulated +
      (started == null ? Duration.zero : now().difference(started!));
  @override
  void start() => started ??= now();
  @override
  void stop() {
    accumulated = elapsed;
    started = null;
  }

  @override
  void reset() {
    accumulated = Duration.zero;
    if (isRunning) started = now();
  }

  @override
  int get elapsedMicroseconds => elapsed.inMicroseconds;
  @override
  int get elapsedMilliseconds => elapsed.inMilliseconds;
  @override
  int get elapsedTicks => elapsedMicroseconds;
  @override
  int get frequency => 1000000;
}

Future<void> flush(WidgetTester tester) async {
  for (var i = 0; i < 20; i++) {
    await tester.pump();
  }
}

void main() {
  testWidgets(
    'one-shot starts idempotently; completion-based cadence, no overlap or healthy flicker',
    (tester) async {
      final api = ObservedApi()..inventoryObserved = true;
      final w = WorkspaceController(api);
      await w.synchronize();
      final c = w.selected;
      c.edit('keep draft');
      final offline = <bool>[];
      w.addListener(() => offline.add(w.stale || !w.connected));
      await tester.pump(const Duration(minutes: 1));
      expect(api.states, 1);
      w.startConnectionObservation();
      w.startConnectionObservation();
      await tester.pump(const Duration(seconds: 29));
      expect(api.states, 1);
      api.stateGate = Completer<void>();
      await tester.pump(const Duration(seconds: 1));
      expect(api.states, 2);
      final manual = w.synchronize();
      final sameManual = w.synchronize();
      expect(identical(manual, sameManual), true);
      await tester.pump(const Duration(minutes: 2));
      expect(api.states, 2);
      expect(api.maxActive, 1);
      api.stateGate!.complete();
      await flush(tester);
      await manual;
      expect(api.states, 3);
      await tester.pump(const Duration(seconds: 29));
      expect(api.states, 3);
      await tester.pump(const Duration(seconds: 1));
      await flush(tester);
      expect(api.states, 4);
      expect(offline, everyElement(false));
      expect(w.selected, same(c));
      expect(c.draft, 'keep draft');
      expect(api.calls.toSet(), {'state', 'conversation_get'});
      expect(api.maxActive, 1);
      w.dispose();
      await tester.pump(const Duration(minutes: 2));
      expect(api.states, 4);
      expect(api.disposedCount, 1);
    },
  );

  testWidgets(
    'outage gates actions; recovery and instance change are read-only',
    (tester) async {
      final api = ObservedApi()..inventoryObserved = true;
      final w = WorkspaceController(api);
      await w.synchronize();
      w.selected.edit('unsent');
      final c = w.selected;
      expect(c.canSend, true);
      w.startConnectionObservation();
      api.stateFailure = true;
      await tester.pump(const Duration(seconds: 30));
      await flush(tester);
      expect(w.stale, true);
      expect(c.canSend, false);
      await c.send();
      await expectLater(
        w.mutate('conversation_archive', c.id!, 0, 0),
        throwsA(isA<AppFailure>()),
      );
      api.stateFailure = false;
      await tester.pump(const Duration(seconds: 30));
      await flush(tester);
      expect(w.stale, false);
      expect(c.canSend, true);
      api.instance = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
      api.inventoryObserved = false;
      await tester.pump(const Duration(seconds: 30));
      await flush(tester);
      expect(w.stale, false);
      expect(w.inventoryObserved, false);
      expect(w.models, isEmpty);
      expect(c.canSend, false);
      expect(c.draft, 'unsent');
      expect(w.selected, same(c));
      expect(api.calls.toSet(), {'state', 'conversation_get'});
      w.dispose();
    },
  );

  testWidgets(
    'automatic snapshots preserve exhausted chat, mutation and global budgets and identities',
    (tester) async {
      final api = ObservedApi()
        ..inventoryObserved = true
        ..holdChats = true;
      final w = WorkspaceController(
        api,
        operationBudget: const Duration(seconds: 5),
        queueBudget: Duration.zero,
        pollInterval: const Duration(days: 1),
        observationClock: () => TestStopwatch(tester.binding.clock.now),
      );
      await w.synchronize();
      w.selected.edit('once');
      await w.selected.send();
      final request = w.selected.pendingId;
      api.unknownAdmission = true;
      await expectLater(
        w.mutate('conversation_rename', w.selected.id!, 0, 0, title: 'Renamed'),
        throwsA(isA<AppFailure>()),
      );
      final mutation = w.mutations.values.single;
      api.unavailableReceipt = mutation.request;
      api.jobs[testInstance] = {
        'request_id': testInstance,
        'conversation_id': null,
        'kind': 'models',
        'status': 'running',
        'phase': 'inventory',
        'config_revision': 1,
        'result': null,
        'error': null,
      };
      await w.synchronize();
      final polls = api.calls.where((op) => op == 'poll').length;
      w.startConnectionObservation();
      await tester.pump(const Duration(seconds: 30));
      await flush(tester);
      await tester.pump(const Duration(seconds: 30));
      await flush(tester);
      expect(api.calls.where((op) => op == 'poll'), hasLength(polls));
      expect(w.selected.pendingId, request);
      expect(w.selected.uncertain, true);
      expect(w.selected.observation.elapsed, const Duration(seconds: 60));
      expect(w.mutations.values.single, same(mutation));
      expect(mutation.clock.elapsed, const Duration(seconds: 60));
      expect(api.calls.where((op) => op == 'submit'), hasLength(1));
      expect(api.calls, isNot(contains('models')));
      await w.synchronize();
      expect(api.calls.where((op) => op == 'poll').length, greaterThan(polls));
      w.dispose();
    },
  );

  testWidgets(
    'operation observation and heartbeat serialize; restart interrupts without replay',
    (tester) async {
      final api = ObservedApi()
        ..inventoryObserved = true
        ..holdChats = true;
      final w = WorkspaceController(api, pollInterval: const Duration(days: 1));
      await w.synchronize();
      w.startConnectionObservation();
      w.selected.edit('once');
      api.pollGate = Completer<AppReply>();
      final sending = w.selected.send();
      await flush(tester);
      final request = w.selected.pendingId!;
      await tester.pump(const Duration(seconds: 30));
      expect(api.states, 1);
      expect(api.maxActive, 1);
      api.pollGate!.complete(
        AppReply(testInstance, Map.of(api.jobs[request]!)),
      );
      await sending;
      api.pollGate = null;
      await flush(tester);
      expect(api.states, 2);
      api.instance = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';
      api.inventoryObserved = false;
      api.jobs[request] = {
        ...api.jobs[request]!,
        'status': 'interrupted',
        'phase': 'complete',
        'error': {'code': 'INTERRUPTED', 'message': 'Service restarted'},
      };
      await tester.pump(const Duration(seconds: 30));
      await flush(tester);
      expect(w.selected.pendingId, isNull);
      expect(w.selected.error, contains('INTERRUPTED'));
      expect(w.inventoryObserved, false);
      expect(api.calls.where((op) => op == 'submit'), hasLength(1));
      expect(
        api.calls.toSet().difference({
          'state',
          'conversation_get',
          'poll',
          'submit',
        }),
        isEmpty,
      );
      expect(api.maxActive, 1);
      w.dispose();
    },
  );

  testWidgets(
    'expired request remains explicit recovery only across automatic checks',
    (tester) async {
      final api = ObservedApi()
        ..inventoryObserved = true
        ..holdChats = true;
      final w = WorkspaceController(api, pollInterval: const Duration(days: 1));
      await w.synchronize();
      w.selected.edit('once');
      api.expiredPoll = true;
      await w.selected.send();
      await w.synchronize();
      final request = w.selected.pendingId;
      expect(w.selected.expired, request);
      final polls = api.calls.where((op) => op == 'poll').length;
      w.startConnectionObservation();
      await tester.pump(const Duration(seconds: 30));
      await flush(tester);
      expect(w.selected.expired, request);
      expect(w.selected.pendingId, request);
      expect(api.calls.where((op) => op == 'poll'), hasLength(polls));
      w.dispose();
    },
  );

  testWidgets(
    'dispose during held observation discards reply and cancels rearm',
    (tester) async {
      final api = ObservedApi()..inventoryObserved = true;
      final w = WorkspaceController(api);
      await w.synchronize();
      w.startConnectionObservation();
      api.stateGate = Completer<void>();
      await tester.pump(const Duration(seconds: 30));
      final calls = api.calls.length;
      w.dispose();
      api.stateGate!.complete();
      await flush(tester);
      await tester.pump(const Duration(minutes: 2));
      expect(api.states, 2);
      expect(api.calls.length, calls + 1);
      expect(api.disposedCount, 1);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'Boot starts heartbeat only after ready; Settings retains polling and draft',
    (tester) async {
      final api = ObservedApi()..inventoryObserved = true;
      final control = FakeControlApi(profile());
      await tester.pumpWidget(SmithApp(controlApi: control, appApi: api));
      await flush(tester);
      await tester.pumpAndSettle();
      expect(find.text('SYSTEM ONLINE'), findsOneWidget);
      final states = api.states;
      await tester.enterText(find.byType(TextField).first, 'retain me');
      await tester.tap(find.byTooltip('Settings').first);
      await flush(tester);
      expect(find.textContaining('Automatic read-only checks'), findsOneWidget);
      api.stateFailure = true;
      await tester.pump(const Duration(seconds: 30));
      await flush(tester);
      expect(api.states, states + 1);
      expect(find.text('SYSTEM UNAVAILABLE'), findsWidgets);
      api.stateFailure = false;
      await tester.pump(const Duration(seconds: 30));
      await flush(tester);
      expect(find.text('SYSTEM ONLINE'), findsWidgets);
      await tester.tap(find.byTooltip('Return to Chat').first);
      await flush(tester);
      expect(find.text('retain me'), findsOneWidget);
      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pump(const Duration(minutes: 1));
      expect(api.disposedCount, 1);
    },
  );
}
