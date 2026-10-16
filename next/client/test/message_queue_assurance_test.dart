import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';

import 'app_fakes.dart';
import 'message_queue_controller_test.dart'
    show QueueApi, ready, start, enqueue, settle;

void main() {
  test('assurance: reviewed foreign success must not permanently disable Retry of failed original', () async {
    final api = QueueApi();
    final w = await ready(api);
    final c = w.selected;
    await start(c);
    final original = c.pendingId!;
    enqueue(c, 'local successor');
    api.complete(original, success: false);
    await w.synchronize();

    final foreign = appId();
    await api.call('submit', {
      'request_id': foreign,
      'conversation_id': c.id,
      'text': 'external message',
      'output': 'chat',
      'expected_config_revision': 1,
      'expected_history_revision': 0,
    });
    api.complete(foreign);
    await w.synchronize();
    final q = c.messageQueue;
    expect(q.paused, true);
    expect(q.canResume, true);
    expect(q.entries.first.text, 'A');
    await q.resume(q.revision);
    await settle();

    expect(
      q.paused,
      false,
      reason: 'Explicit review/rebinding must not immediately relatch against the same acknowledged foreign request',
    );
    final originalRow = q.entries.first;
    expect(originalRow.canRetry, true);
    await q.retry(originalRow.id, originalRow.actionRevision);
    await settle();
    expect(api.sends.last['text'], 'A');
    expect(api.sends.last['request_id'], isNot(original));
    expect(api.sends.last['expected_history_revision'], 1);
  });

  test('assurance: enqueue during reattached terminal observation uses predecessor base history', () async {
    final api = QueueApi();
    final request = appId();
    await api.call('submit', {
      'request_id': request,
      'conversation_id': testConversation,
      'text': 'external predecessor',
      'output': 'chat',
      'expected_config_revision': 1,
      'expected_history_revision': 0,
    });
    final w = await ready(api);
    final c = w.selected;
    expect(c.pendingId, request);
    api.complete(request);
    api.pollGate = Completer<AppReply>();
    final observation = w.synchronize();
    await settle();
    expect(c.summary!['history_revision'], 1);
    expect(c.pendingId, request);
    enqueue(c, 'B');
    final terminalGate = api.pollGate!;
    api.pollGate = null;
    terminalGate.complete(AppReply(api.instance, Map.of(api.jobs[request]!)));
    await observation;
    await settle();

    expect(
      api.sends.map((s) => s['text']),
      ['external predecessor', 'B'],
      reason: 'The predecessor succeeded and its exact saved pair is synchronized; no extra history increment is due',
    );
    expect(api.sends.last['expected_history_revision'], 1);
  });

  test('assurance: superseded pending identity resolves before newer running work and never replays', () async {
    final api = QueueApi();
    final w = await ready(api);
    final c = w.selected;
    await start(c);
    final original = c.pendingId!;
    enqueue(c, 'B');
    api.complete(original, success: false);
    final foreign = appId();
    await api.call('submit', {
      'request_id': foreign,
      'conversation_id': c.id,
      'text': 'external message',
      'output': 'chat',
      'expected_config_revision': 1,
      'expected_history_revision': 0,
    });
    await w.synchronize();
    await settle();
    expect(c.pendingId, foreign);
    expect(c.messageQueue.paused, true);
    expect(c.messageQueue.entries.first.text, 'A');
    expect(c.messageQueue.entries.first.canRetry, false);
    expect(c.messageQueue.entries.first.canSkip, false);
    expect(api.sends, hasLength(2));
    expect(
      api.arguments.where((a) => a['request_id'] == original).length,
      greaterThanOrEqualTo(2),
    );
  });
}
