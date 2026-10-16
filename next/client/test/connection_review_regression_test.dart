import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/main.dart';
import 'package:smith_client/ui/smith_workspace.dart';
import 'package:smith_client/workspace_controller.dart';

import 'app_fakes.dart';
import 'connection_polling_test.dart' show ObservedApi, flush;
import 'widget_test.dart' show FakeControlApi, profile;

void main() {
  for (final newChat in [false, true]) {
    testWidgets('queued Send captures text and reserves once (new: $newChat)', (
      tester,
    ) async {
      final api = ObservedApi()..inventoryObserved = true;
      final w = WorkspaceController(api);
      await w.synchronize();
      if (newChat) w.newChat();
      final c = w.selected;
      c.edit('approved text');
      final revision = c.draftRevision;
      w.startConnectionObservation();
      api.stateGate = Completer<void>();
      await tester.pump(const Duration(seconds: 30));
      final send = c.send();
      expect(c.canSend, false);
      expect(c.busy, true);
      c.edit('later draft');
      await c.send();
      await c.send();
      expect(c.localSends.values.single.text, 'approved text');
      api.stateGate!.complete();
      await flush(tester);
      await send;
      final submissions = api.calls
          .where((op) => op.startsWith('submit'))
          .toList();
      expect(submissions, [newChat ? 'submit_new' : 'submit']);
      final args = api.arguments[api.calls.indexOf(submissions.single)];
      expect(args['text'], 'approved text');
      expect(args['expected_config_revision'], 1);
      expect(args['conversation_id'], newChat ? null : testConversation);
      expect(c.draft, 'later draft');
      expect(c.draftRevision, greaterThan(revision));
      expect(api.maxActive, 1);
      w.dispose();
    });
  }

  for (final change in ['config', 'history', 'instance', 'archive', 'delete']) {
    testWidgets(
      'queued Send rejects changed $change authority without rebinding',
      (tester) async {
        final api = ObservedApi()..inventoryObserved = true;
        final w = WorkspaceController(api);
        await w.synchronize();
        final c = w.selected;
        c.edit('original');
        w.startConnectionObservation();
        api.stateGate = Completer<void>();
        await tester.pump(const Duration(seconds: 30));
        final send = c.send();
        c.edit('keep newer');
        switch (change) {
          case 'config':
            api.config['revision'] = 2;
          case 'history':
            api.summaries[testConversation]!['history_revision'] = 1;
          case 'instance':
            api.instance = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
          case 'archive':
            api.summaries[testConversation]!['archived'] = true;
          case 'delete':
            api.summaries.remove(testConversation);
        }
        api.stateGate!.complete();
        await flush(tester);
        await send;
        expect(api.calls.where((op) => op.startsWith('submit')), isEmpty);
        expect(c.localSends.values.single.text, 'original');
        expect(c.localSends.values.single.status, contains('Not sent'));
        expect(c.localSends.values.single.active, false);
        expect(c.draft, 'keep newer');
        expect(c.busy, false);
        w.dispose();
      },
    );
  }

  for (final manualFirst in [false, true]) {
    testWidgets(
      'post-Boot Retry and heartbeat serialize (manual first: $manualFirst)',
      (tester) async {
        final api = ObservedApi()..inventoryObserved = true;
        final control = FakeControlApi(profile());
        await tester.pumpWidget(SmithApp(controlApi: control, appApi: api));
        await flush(tester);
        await tester.pumpAndSettle();
        final coreCalls = List<String>.of(control.calls);
        final bootStates = api.states;
        final retry = tester
            .widget<SmithWorkspace>(find.byType(SmithWorkspace))
            .onRetry!;
        api.stateGate = Completer<void>();

        if (manualFirst) {
          retry();
          await flush(tester);
          await tester.pump(const Duration(seconds: 30));
        } else {
          await tester.pump(const Duration(seconds: 30));
          retry();
          await flush(tester);
        }
        expect(api.states, bootStates + 1);
        expect(api.maxActive, 1);
        expect(find.text('SYSTEM ONLINE'), findsOneWidget);
        expect(find.text('CHECKING SYSTEM'), findsNothing);
        expect(control.calls, coreCalls);
        api.stateFailure = true;
        api.stateGate!.complete();
        await flush(tester);

        expect(api.states, bootStates + 2);
        expect(find.text('SYSTEM UNAVAILABLE'), findsOneWidget);
        expect(control.calls, coreCalls);
        api.stateFailure = false;
        retry();
        await flush(tester);
        expect(find.text('SYSTEM ONLINE'), findsOneWidget);
        expect(control.calls, coreCalls);
        expect(
          api.calls,
          contains('models'),
        ); // Explicit manual inventory allowed.
        expect(api.maxActive, 1);
        await tester.pumpWidget(const SizedBox.shrink());
      },
    );
  }

  for (final held in ['detail', 'poll']) {
    testWidgets('dispose fences held $held and queued user actions', (
      tester,
    ) async {
      final api = ObservedApi()
        ..inventoryObserved = true
        ..holdChats = true;
      final w = WorkspaceController(api, pollInterval: const Duration(days: 1));
      await w.synchronize();
      w.startConnectionObservation();
      final c = w.selected;
      Future<void> active;
      if (held == 'detail') {
        api.detailGate = Completer<void>();
        active = w.synchronize();
      } else {
        c.edit('accepted');
        api.pollGate = Completer<AppReply>();
        active = c.send();
      }
      await flush(tester);
      final request = c.pendingId;
      w.newChat();
      final queued = w.selected;
      queued.edit('never dispatch');
      final sending = queued.send();
      expect(queued.busy, true);
      final mutation = w.mutate(
        'conversation_rename',
        testConversation,
        0,
        0,
        title: 'Never',
      );
      var notifications = 0;
      w.addListener(() => notifications++);
      final before = api.calls.length;
      final history = List<Map<String, dynamic>>.of(c.history);
      w.dispose();
      if (held == 'detail') {
        api.detailGate!.complete();
      } else {
        api.pollGate!.complete(
          AppReply(testInstance, {
            ...api.jobs[request]!,
            'status': 'succeeded',
            'phase': 'complete',
            'result': {
              'assistant_text': 'late answer',
              'audio': null,
              'speech_error': null,
            },
          }),
        );
      }
      await flush(tester);
      await Future.wait([active, sending, mutation]);
      await tester.pump(const Duration(minutes: 2));
      expect(api.calls.length, before + (held == 'detail' ? 1 : 0));
      expect(api.calls, isNot(contains('submit_new')));
      expect(api.calls, isNot(contains('conversation_rename')));
      expect(api.disposedCount, 1);
      expect(notifications, 0);
      expect(c.history, history);
      expect(c.provisionalAnswers, isEmpty);
      expect(queued.busy, false);
      expect(tester.takeException(), isNull);
    });
  }
}
