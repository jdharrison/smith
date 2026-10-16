import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/chat_panel.dart';
import 'package:smith_client/workspace_controller.dart';

import 'app_fakes.dart';

class DelayedAdmissionApi extends FakeAppApi {
  final admission = Completer<void>();
  AppFailure? rejection;
  bool queued = false;

  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    if (op == 'submit' || op == 'submit_new') {
      await admission.future;
      if (rejection != null) throw rejection!;
      final reply = await super.call(op, args);
      if (queued) {
        jobs[args['request_id']]!['status'] = 'queued';
        jobs[args['request_id']]!['phase'] = 'queued';
        return AppReply(instance, brief(jobs[args['request_id']]!));
      }
      return reply;
    }
    return super.call(op, args);
  }
}

Future<WorkspaceController> ready(FakeAppApi api) async {
  api.inventoryObserved = true;
  final workspace = WorkspaceController(
    api,
    pollInterval: const Duration(days: 1),
  );
  await workspace.synchronize();
  return workspace;
}

void complete(FakeAppApi api, ConversationController chat) {
  final request = chat.pendingId!;
  api.jobs[request] = {
    ...api.jobs[request]!,
    'status': 'succeeded',
    'phase': 'complete',
    'result': {
      'assistant_text': 'Finished answer',
      'audio': null,
      'speech_error': null,
    },
  };
  api.histories[chat.id!] = [
    {'role': 'user', 'content': 'Original message'},
    {'role': 'assistant', 'content': 'Finished answer'},
  ];
  api.summaries[chat.id!]!['history_revision']++;
}

void main() {
  testWidgets('reconciling B after failed A does not scroll back to A', (
    tester,
  ) async {
    final api = DelayedAdmissionApi()..holdChats = true;
    for (var i = 0; i < 12; i++) {
      api.history.addAll([
        {'role': 'user', 'content': 'Earlier question $i'},
        {'role': 'assistant', 'content': 'Earlier answer $i'},
      ]);
    }
    final workspace = await ready(api);
    final chat = workspace.selected;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(body: ChatPanel(controller: chat)),
      ),
    );
    await tester.pumpAndSettle();
    final scroll = tester
        .widget<SingleChildScrollView>(
          find.byKey(const ValueKey('conversation-history')),
        )
        .controller!;
    api.admission.complete();
    chat.edit('Failed A');
    await chat.send();
    final failedRequest = chat.pendingId!;
    api.jobs[failedRequest]!.addAll({
      'status': 'failed',
      'phase': 'complete',
      'error': {'code': 'GENERATION', 'message': 'Generation failed'},
    });
    await workspace.synchronize();
    await tester.pumpAndSettle();
    expect(chat.localSends[failedRequest]!.active, false);
    scroll.jumpTo(0);
    chat.edit('Successful B');
    await chat.send();
    await tester.pumpAndSettle();
    expect(scroll.offset, 0);
    final successfulRequest = chat.pendingId!;
    expect(chat.localSends.keys, [failedRequest, successfulRequest]);
    scroll.jumpTo(0);
    api.jobs[successfulRequest]!.addAll({
      'status': 'succeeded',
      'phase': 'complete',
      'result': {
        'assistant_text': 'B answer',
        'audio': null,
        'speech_error': null,
      },
    });
    api.history.addAll([
      {'role': 'user', 'content': 'Successful B'},
      {'role': 'assistant', 'content': 'B answer'},
    ]);
    api.summaries[chat.id!]!['history_revision']++;
    await workspace.synchronize();
    await tester.pumpAndSettle();
    expect(chat.localSends.keys, [failedRequest]);
    expect(find.text('B answer'), findsOneWidget);
    expect(scroll.position.maxScrollExtent, greaterThan(0));
    expect(scroll.offset, 0);
    chat.edit('Newer draft');
    await tester.pumpAndSettle();
    expect(scroll.offset, 0);
    await tester.pumpWidget(const SizedBox.shrink());
    workspace.dispose();
  });
  test(
    'optimistic send is immediate even behind serialized reconciliation',
    () async {
      final api = DelayedAdmissionApi()..holdChats = true;
      final workspace = await ready(api);
      addTearDown(workspace.dispose);
      final first = workspace.selected..edit('First');
      final firstSend = first.send();
      await Future<void>.delayed(Duration.zero);
      workspace.newChat();
      final second = workspace.selected..edit('Second');
      final secondSend = second.send();
      expect(second.draft, isEmpty);
      expect(second.localSends.values.single.status, 'Sending…');
      expect(second.canSend, false);
      second.edit('Later');
      await second.send();
      api.admission.complete();
      await Future.wait([firstSend, secondSend]);
      expect(first.localSends.values.single.text, 'First');
      expect(second.localSends.values.single.text, 'Second');
      expect(second.draft, 'Later');
      expect(workspace.selected, same(second));
      expect(second.id, isNotNull);
      expect(api.calls.where((op) => op.startsWith('submit')), hasLength(2));
    },
  );

  testWidgets('send and admission updates preserve a locked reading position', (
    tester,
  ) async {
    final api = DelayedAdmissionApi()..holdChats = true;
    for (var i = 0; i < 12; i++) {
      api.history.addAll([
        {'role': 'user', 'content': 'Earlier question $i'},
        {'role': 'assistant', 'content': 'Earlier answer $i'},
      ]);
    }
    final workspace = await ready(api);
    final chat = workspace.selected;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(body: ChatPanel(controller: chat)),
      ),
    );
    await tester.pumpAndSettle();
    final scroll = tester
        .widget<SingleChildScrollView>(
          find.byKey(const ValueKey('conversation-history')),
        )
        .controller!;
    scroll.jumpTo(0);
    chat.edit('Latest message');
    final sending = chat.send();
    await tester.pumpAndSettle();
    expect(scroll.offset, 0);
    api.admission.complete();
    await sending;
    await tester.pumpAndSettle();
    expect(scroll.offset, 0);
    chat.edit('Another draft');
    await tester.pumpAndSettle();
    expect(scroll.offset, 0);
    await tester.pumpWidget(const SizedBox.shrink());
    workspace.dispose();
  });
  testWidgets(
    'Send clears immediately; queued/thinking becomes authoritative answer',
    (tester) async {
      final api = DelayedAdmissionApi()
        ..holdChats = true
        ..queued = true;
      final workspace = await ready(api);
      final chat = workspace.selected;
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(body: ChatPanel(controller: chat)),
        ),
      );
      chat.edit('Original message');
      final sending = chat.send();
      expect(chat.draft, isEmpty);
      expect(chat.localSends.values.single.text, 'Original message');
      await tester.pumpAndSettle();
      expect(find.text('Sending…'), findsOneWidget);
      expect(find.text('Original message'), findsOneWidget);
      chat.edit('Newer draft');
      api.admission.complete();
      await sending;
      await tester.pumpAndSettle();
      expect(find.text('Queued…'), findsOneWidget);
      final request = chat.pendingId!;
      api.jobs[request]!['status'] = 'running';
      await workspace.synchronize();
      await tester.pumpAndSettle();
      expect(find.text('Thinking…'), findsOneWidget);
      complete(api, chat);
      await workspace.synchronize();
      await tester.pumpAndSettle();
      expect(find.text('Thinking…'), findsNothing);
      expect(find.text('Finished answer'), findsOneWidget);
      expect(find.text('Original message'), findsOneWidget);
      expect(chat.localSends, isEmpty);
      expect(chat.draft, 'Newer draft');
      expect(api.calls.where((op) => op == 'submit'), hasLength(1));
      await tester.pumpWidget(const SizedBox.shrink());
      workspace.dispose();
    },
  );

  test(
    'definitive rejection preserves text without active status or resend',
    () async {
      final api = DelayedAdmissionApi()
        ..rejection = const AppFailure('CAPACITY', 'Queue full');
      final workspace = await ready(api);
      addTearDown(workspace.dispose);
      final chat = workspace.selected..edit('Original message');
      final sending = chat.send();
      chat.edit('Newer draft');
      api.admission.complete();
      await sending;
      expect(chat.pendingId, isNull);
      expect(chat.localSends.values.single.active, false);
      expect(chat.localSends.values.single.status, contains('Not sent'));
      expect(chat.draft, 'Newer draft');
      await workspace.synchronize();
      expect(chat.localSends.values.single.text, 'Original message');
      expect(api.calls.where((op) => op.startsWith('submit')), isEmpty);
    },
  );

  test('unknown new-chat admission remains query-only and promotes without duplicates', () async {
    final api = DelayedAdmissionApi()..unknownAdmission = true;
    final workspace = await ready(api);
    addTearDown(workspace.dispose);
    workspace.newChat();
    final chat = workspace.selected..edit('Original message');
    final sending = chat.send();
    api.admission.complete();
    await sending;
    expect(chat.localSends.values.single.active, false);
    expect(chat.localSends.values.single.status, contains('Outcome unknown'));
    expect(chat.pendingId, isNotNull);
    chat.edit('Newer draft');
    await workspace.select(testConversation);
    expect(workspace.selected.localSends, isEmpty);
    await workspace.synchronize();
    expect(chat.id, isNotNull);
    expect(workspace.selected.id, testConversation);
    expect(chat.localSends, isEmpty);
    expect(chat.draft, 'Newer draft');
    expect(api.calls.where((op) => op == 'submit_new'), hasLength(1));
  });

  test('verified answer retains optimistic prompt on detail failure then reconciles', () async {
    final api = DelayedAdmissionApi()..holdChats = true;
    final workspace = await ready(api);
    addTearDown(workspace.dispose);
    final chat = workspace.selected..edit('Original message');
    final sending = chat.send();
    api.admission.complete();
    await sending;
    complete(api, chat);
    api.detailFailure = true;
    await workspace.synchronize();
    expect(chat.localSends.values.single.text, 'Original message');
    expect(chat.localSends.values.single.active, false);
    expect(chat.provisionalAnswers.values.single, 'Finished answer');
    api.detailFailure = false;
    await workspace.synchronize();
    expect(chat.localSends, isEmpty);
    expect(chat.provisionalAnswers, isEmpty);
  });

  test(
    'terminal failure stops progress and leaves newer draft untouched',
    () async {
      final api = DelayedAdmissionApi()..holdChats = true;
      final workspace = await ready(api);
      addTearDown(workspace.dispose);
      final chat = workspace.selected..edit('Original message');
      final sending = chat.send();
      api.admission.complete();
      await sending;
      chat.edit('Newer draft');
      api.jobs[chat.pendingId!]!.addAll({
        'status': 'failed',
        'phase': 'complete',
        'error': {'code': 'GENERATION', 'message': 'Generation failed'},
      });
      await workspace.synchronize();
      expect(chat.pendingId, isNull);
      expect(chat.localSends.values.single.active, false);
      expect(chat.localSends.values.single.status, contains('Failed'));
      expect(chat.draft, 'Newer draft');
    },
  );
}
