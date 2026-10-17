import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/text_chat.dart';
import 'package:smith_client/chat_panel.dart';

import 'app_fakes.dart';

Future<void> tick() async {
  for (var i = 0; i < 10; i++) {
    await Future<void>.delayed(Duration.zero);
  }
}

void main() {
  test('M3 expired unknown acknowledgement requires explicit local resolution, never resend', () async {
    final api = LegacyFakeAppApi();
    final c = ChatController(api);
    await c.synchronize();

    await c.refreshModels();
    api.unknownAdmission = true;
    c.edit('uncertain work');
    await c.send();
    final oldId = c.pendingId!;
    c.abandonExpiredRequest(oldId);
    expect(c.pendingId, oldId);
    api.expiredPoll = true;
    await c.synchronize();

    expect(c.pendingId, oldId);
    expect(c.canAbandonExpiredRequest, isTrue);
    expect(c.error, contains('REQUEST_EXPIRED'));
    await c.send();
    await c.configure('local:exact');
    expect(api.calls.where((v) => v == 'submit').length, 1);
    expect(api.calls, isNot(contains('configure')));
    c.abandonExpiredRequest('ffffffffffffffffffffffffffffffff');
    expect(c.pendingId, oldId);
    c.abandonExpiredRequest(oldId);
    expect(c.pendingId, isNull);
    expect(c.canAbandonExpiredRequest, isFalse);
    expect(c.phase, 'Ready');
    expect(c.draft, 'uncertain work');
    expect(c.error, contains('outcome may be unknown'));
    expect(c.history.last['content'], 'useful answer');
    expect(api.calls.where((v) => v == 'submit').length, 1);
    api.unknownAdmission = false;
    api.expiredPoll = false;
    await c.synchronize();
    await c.refreshModels();
    expect(c.models, ['local:exact']);
    c.edit('deliberately new work');
    await c.send();
    final submissions = [
      for (var i = 0; i < api.calls.length; i++)
        if (api.calls[i] == 'submit') api.arguments[i],
    ];
    expect(submissions.length, 2);
    expect(submissions.last['request_id'], isNot(oldId));
    expect(submissions.last['text'], 'deliberately new work');
    c.dispose();
  });

  testWidgets(
    'M3 expired resolution requires confirmation and never resubmits',
    (tester) async {
      final api = LegacyFakeAppApi();
      final c = ChatController(api);
      await c.synchronize();

      await c.refreshModels();
      api.unknownAdmission = true;
      c.edit('uncertain draft');
      await c.send();
      api.expiredPoll = true;
      await c.synchronize();

      final id = c.pendingId;
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(body: ChatPanel(controller: c)),
        ),
      );
      await tester.tap(find.text('Resolve expired request'));
      await tester.pumpAndSettle();
      expect(find.textContaining('outcome may be unknown'), findsOneWidget);
      await tester.tap(find.text('Keep pending'));
      await tester.pumpAndSettle();
      expect(c.pendingId, id);
      await tester.tap(find.text('Resolve expired request'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Forget request'));
      await tester.pumpAndSettle();
      expect(c.pendingId, isNull);
      expect(find.text('Resolve expired request'), findsNothing);
      expect(api.calls.where((v) => v == 'submit').length, 1);
      expect(api.calls, isNot(contains('cancel')));
      await tester.pumpWidget(const SizedBox());
      c.dispose();
    },
  );

  test(
    'unknown admission is queried, never resent; reopen reads saved history',
    () async {
      final api = LegacyFakeAppApi();
      final c = ChatController(api);
      await c.synchronize();

      await c.refreshModels();
      api.unknownAdmission = true;
      c.edit('hello');
      await c.send();
      final id = c.pendingId;
      expect(id, matches(RegExp(r'^[0-9a-f]{32}$')));
      await c.send();
      expect(api.calls.where((v) => v == 'submit').length, 1);
      await c.synchronize();

      expect(c.pendingId, isNull);
      expect(api.arguments[api.calls.lastIndexOf('poll')]['request_id'], id);
      c.dispose();
      final reopened = ChatController(api);
      await reopened.synchronize();
      expect(reopened.history.last['content'], 'useful answer');
      reopened.dispose();
    },
  );
  test('restart does not resend unknown work', () async {
    final api = LegacyFakeAppApi();
    final c = ChatController(api);
    await c.synchronize();

    await c.refreshModels();
    api.unknownAdmission = true;
    c.edit('hello');
    await c.send();
    api.instance = 'ffffffffffffffffffffffffffffffff';
    await c.synchronize();

    expect(c.pendingId, isNull);
    expect(api.calls.where((v) => v == 'submit').length, 1);
    c.dispose();
  });
  test('poll deadline is finite and retains query-only operation', () async {
    final api = LegacyFakeAppApi();
    final c = ChatController(
      api,
      operationBudget: const Duration(milliseconds: 5),
      pollInterval: const Duration(milliseconds: 10),
    );
    await c.synchronize();

    await c.refreshModels();
    api.pollGate = Completer<AppReply>();
    c.edit('hello');
    final work = c.send();
    await tick();
    final job = {
      ...api.jobs.values.last,
      'status': 'running',
      'phase': 'generation',
    };
    api.pollGate!.complete(AppReply(testInstance, job));
    await work;
    expect(c.pendingId, isNotNull);
    expect(c.error, contains('Polling budget'));
    c.dispose();
  });
}
