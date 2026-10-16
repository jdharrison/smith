import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/message_queue.dart';
import 'package:smith_client/workspace_controller.dart';

import 'app_fakes.dart';

class QueueApi extends FakeAppApi {
  QueueApi() {
    inventoryObserved = true;
    holdChats = true;
  }
  final sends = <Map<String, dynamic>>[];
  String? rejection;
  bool postCommitError = false;
  Completer<void>? detailGate;
  Completer<void>? detailEntered;
  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    if (op == 'conversation_get' && detailGate != null) {
      if (!(detailEntered?.isCompleted ?? true)) detailEntered!.complete();
      await detailGate!.future;
    }
    if (op == 'submit' || op == 'submit_new') {
      sends.add(Map.of(args));
      if (rejection != null) throw AppFailure(rejection!, 'Not admitted');
      final result = await super.call(op, args);
      if (postCommitError) {
        throw const AppFailure('STATE_UNAVAILABLE', 'Publication failed');
      }
      return result;
    }
    return super.call(op, args);
  }

  void complete(String request, {bool success = true, bool save = true}) {
    final job = jobs[request]!;
    job['status'] = success ? 'succeeded' : 'failed';
    job['phase'] = 'complete';
    job['result'] = success
        ? {
            'assistant_text': 'answer $request',
            'audio': null,
            'speech_error': null,
          }
        : null;
    job['error'] = success ? null : {'code': 'MODEL', 'message': 'Failed'};
    if (success && save) savePair(request);
  }

  void savePair(String request) {
    final id = jobs[request]!['conversation_id'] as String;
    final text = sends.singleWhere((s) => s['request_id'] == request)['text'];
    histories[id]!.addAll([
      {'role': 'user', 'content': text},
      {'role': 'assistant', 'content': 'answer $request'},
    ]);
    summaries[id]!['history_revision']++;
  }
}

class QueueClock extends Stopwatch {
  Duration age = Duration.zero;
  @override
  Duration get elapsed => age;
  @override
  void reset() {
    age = Duration.zero;
  }
}

Future<WorkspaceController> ready(QueueApi api) async {
  final w = WorkspaceController(api, pollInterval: const Duration(days: 1));
  await w.synchronize();
  addTearDown(w.dispose);
  return w;
}

Future<void> settle() async {
  for (var i = 0; i < 20; i++) {
    await Future<void>.delayed(Duration.zero);
  }
}

void enqueue(ConversationController c, String text) {
  c.edit(text);
  expect(c.messageQueue.canEnqueue, true);
  c.messageQueue.enqueue();
}

Future<void> start(ConversationController c) async {
  c.edit('A');
  await c.send();
}

void main() {
  test('Resume reviews only the current foreign baseline, preserving failed A for Retry', () async {
    final api = QueueApi();
    final w = await ready(api);
    final c = w.selected;
    await start(c);
    final original = c.pendingId!;
    enqueue(c, 'B');
    api.complete(original, success: false);
    await w.synchronize();
    final q = c.messageQueue;
    final logical = q.entries.first.id;
    Future<void> foreign(bool success) async {
      final request = appId();
      await api.call('submit', {
        'request_id': request,
        'conversation_id': c.id,
        'text': 'foreign',
        'output': 'chat',
        'expected_config_revision': 1,
        'expected_history_revision': api.summaries[c.id]!['history_revision'],
      });
      api.complete(request, success: success);
      await w.synchronize();
    }

    await foreign(true);
    expect(q.paused, true);
    await q.resume(q.revision);
    await settle();
    expect(q.paused, false);
    expect(q.entries.first.id, logical);
    expect(q.entries.first.text, 'A');
    // A new failed foreign operation changes identity, but not history.
    await foreign(false);
    expect(q.paused, true);
    expect(q.entries.first.canRetry, false);
    await q.resume(q.revision);
    final a = q.entries.first;
    await q.retry(a.id, a.actionRevision);
    await settle();
    expect(api.sends.last['text'], 'A');
    expect(api.sends.last['request_id'], isNot(original));
    expect(api.sends.last['expected_history_revision'], 1);
    expect(q.entries.first.id, logical);
  });

  test(
    'reattached base revision does not permit a mismatched completed pair',
    () async {
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
      api.complete(request);
      api.pollGate = Completer<AppReply>();
      final observation = w.synchronize();
      await settle();
      expect(c.summary!['history_revision'], 1);
      enqueue(c, 'B');
      api.histories[testConversation]!.last['content'] = 'different answer';
      final gate = api.pollGate!;
      api.pollGate = null;
      gate.complete(AppReply(api.instance, Map.of(api.jobs[request]!)));
      await observation;
      await settle();
      expect(api.sends, hasLength(1));
      expect(c.messageQueue.paused, true);
    },
  );

  test('queue receipt provenance and bounded Skip notice never fabricate unsent history', () async {
    final api = QueueApi();
    final w = await ready(api);
    final c = w.selected;
    await start(c);
    expect(c.localSends[c.pendingId]!.fromQueue, false);
    enqueue(c, 'B');
    enqueue(c, 'C');
    api.complete(c.pendingId!);
    await w.synchronize();
    await settle();
    expect(c.localSends[c.pendingId]!.fromQueue, true);
    api.complete(c.pendingId!, success: false);
    await w.synchronize();
    final q = c.messageQueue;
    q.pause();
    final b = q.entries.first;
    q.skip(b.id, b.actionRevision);
    expect(c.error, contains('skipped'));
    final neverSent = q.entries.first;
    q.skip(neverSent.id, neverSent.actionRevision);
    expect(c.error, contains('without sending'));
    expect(c.localSends.values.any((receipt) => receipt.text == 'C'), false);
    expect(c.history.any((turn) => turn['content'] == 'C'), false);
    expect(q.entries, isEmpty);
    await w.synchronize();
    expect(c.error, contains('without sending'));
  });

  test('external draft-only removal restores blank selection without retaining a queue', () async {
    final api = QueueApi();
    final w = await ready(api);
    final old = w.selected;
    old.edit('ordinary draft');
    api.summaries.remove(old.id);
    api.histories.remove(old.id);
    await w.synchronize();
    expect(w.selected, same(w.blank));
    expect(w.conversations.containsKey(old.id), false);
    expect(old.canSubmit, false);
  });

  test(
    'synchronous close listener fences the last undispatched send boundary',
    () async {
      final api = QueueApi();
      final w = await ready(api);
      final c = w.selected;
      QueueExitHold? hold;
      c.addListener(() {
        if (c.pendingId != null && !w.queueExitPending) {
          hold = w.acquireQueueExitHold();
        }
      });
      await start(c);
      expect(hold, isNotNull);
      expect(hold!.messages, 1);
      expect(api.sends, isEmpty);
      expect(c.pendingId, isNull);
    },
  );

  test(
    'immediate terminal acknowledgement still wakes a newly queued tail',
    () async {
      final api = QueueApi()..holdChats = false;
      final w = await ready(api);
      final c = w.selected;
      c.edit('A');
      final sending = c.send();
      enqueue(c, 'B');
      enqueue(c, 'C');
      await sending;
      await settle();
      expect(api.sends.map((s) => s['text']), ['A', 'B', 'C']);
      expect(c.messageQueue.entries, isEmpty);
    },
  );

  for (final success in [true, false]) {
    test(
      'reattached predecessor ${success ? 'success gates history' : 'failure requires safe Skip'}',
      () async {
        final api = QueueApi();
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
        final c = w.selected;
        enqueue(c, 'B');
        final q = c.messageQueue;
        expect(q.entries.first.text, isEmpty);
        expect(q.entries.first.canSkip, false);
        api.jobs[testInstance] = {
          ...api.jobs[testInstance]!,
          'status': success ? 'succeeded' : 'failed',
          'phase': 'complete',
          'result': success
              ? {
                  'assistant_text': 'external answer',
                  'audio': null,
                  'speech_error': null,
                }
              : null,
          'error': success ? null : {'code': 'MODEL', 'message': 'failed'},
        };
        if (success) {
          api.summaries[testConversation]!['history_revision'] = 1;
          api.histories[testConversation] = [
            {'role': 'user', 'content': 'external user'},
            {'role': 'assistant', 'content': 'external answer'},
          ];
        }
        await w.synchronize();
        await settle();
        if (!success) {
          expect(api.sends, isEmpty);
          final predecessor = q.entries.first;
          expect(predecessor.canRetry, false);
          expect(predecessor.canSkip, true);
          q.skip(predecessor.id, predecessor.actionRevision);
          await settle();
        }
        expect(api.sends.single['text'], 'B');
        expect(api.sends.single['expected_history_revision'], success ? 1 : 0);
      },
    );
  }

  test('local wait has no queue or operation deadline; dispatch starts a fresh budget', () async {
    final api = QueueApi()..rejection = 'BUSY';
    final clocks = <QueueClock>[];
    final w = WorkspaceController(
      api,
      pollInterval: const Duration(days: 1),
      queueBudget: const Duration(milliseconds: 1),
      operationBudget: const Duration(milliseconds: 1),
      observationClock: () {
        final clock = QueueClock();
        clocks.add(clock);
        return clock;
      },
    );
    addTearDown(w.dispose);
    await w.synchronize();
    final c = w.selected;
    await start(c);
    enqueue(c, 'B');
    c.messageQueue.pause();
    for (final clock in clocks) {
      clock.age = const Duration(days: 30);
    }
    await w.synchronize();
    expect(c.messageQueue.waitingCount, 1);
    expect(api.sends, hasLength(1));
    api.rejection = null;
    final a = c.messageQueue.entries.first;
    c.messageQueue.skip(a.id, a.actionRevision);
    await c.messageQueue.resume(c.messageQueue.revision);
    await settle();
    expect(api.sends.map((s) => s['text']), ['A', 'B']);
    expect(c.uncertain, false);
    expect(c.observation.elapsed, Duration.zero);
  });

  test(
    'bounded fair pump admits other chats without waiting for generation',
    () async {
      final api = QueueApi();
      final w = await ready(api);
      final chats = [w.selected];
      for (var i = 0; i < 3; i++) {
        chats.add(w.createPane().selected);
      }
      for (final c in chats) {
        await start(c);
        enqueue(c, 'tail ${chats.indexOf(c)}');
      }
      for (final c in chats) {
        api.complete(c.pendingId!);
      }
      await w.synchronize();
      await settle();
      expect(api.sends, hasLength(8));
      expect(api.sends.skip(4).map((s) => s['text']).toSet(), {
        'tail 0',
        'tail 1',
        'tail 2',
        'tail 3',
      });
      expect(chats.every((c) => c.pendingId != null), true);
    },
  );

  test(
    'ordinary rejected Send without queued successors stays reusable',
    () async {
      final api = QueueApi()..rejection = 'BUSY';
      final w = await ready(api);
      final c = w.selected;
      await start(c);
      expect(c.messageQueue.entries, isEmpty);
      c.edit('new immediate message');
      expect(c.canSend, true);
      api.rejection = null;
      await c.send();
      expect(api.sends.map((s) => s['text']), ['A', 'new immediate message']);
      c.edit(' ');
      final hold = w.acquireQueueExitHold();
      expect(
        hold.messages,
        1,
        reason: 'Retained failed receipt A is counted once',
      );
      expect(hold.drafts, 1);
    },
  );

  test(
    'A B C dispatch once, after exact success and fresh saved detail',
    () async {
      final api = QueueApi();
      final w = await ready(api);
      final c = w.selected;
      await start(c);
      enqueue(c, 'B');
      enqueue(c, 'C');
      c.edit('newer draft');
      await c.send(); // Direct send never enqueues implicitly.
      expect(api.sends.map((s) => s['text']), ['A']);
      api.complete(c.pendingId!);
      await w.synchronize();
      await settle();
      expect(api.sends.map((s) => s['text']), ['A', 'B']);
      expect(api.sends.last['expected_history_revision'], 1);
      api.complete(c.pendingId!);
      await w.synchronize();
      await settle();
      expect(api.sends.map((s) => s['text']), ['A', 'B', 'C']);
      expect(api.sends.last['expected_history_revision'], 2);
      expect(c.draft, 'newer draft');
      expect(c.messageQueue.waitingCount, 0);
    },
  );

  test(
    'success alone or unavailable detail cannot release successor',
    () async {
      final api = QueueApi();
      final w = await ready(api);
      final c = w.selected;
      await start(c);
      enqueue(c, 'B');
      final request = c.pendingId!;
      api.complete(request);
      api.detailFailure = true;
      await w.synchronize();
      await settle();
      expect(api.sends, hasLength(1));
      api.detailFailure = false;
      await w.synchronize();
      await settle();
      expect(api.sends.map((s) => s['text']), ['A', 'B']);
    },
  );

  test('mismatched saved history latches pause and Retry observation does not resume', () async {
    final api = QueueApi();
    final w = await ready(api);
    final c = w.selected;
    await start(c);
    enqueue(c, 'B');
    api.complete(c.pendingId!, save: false);
    await w.synchronize();
    await settle();
    expect(api.sends, hasLength(1));
    expect(c.messageQueue.paused, true);
    await w.synchronize();
    expect(api.sends, hasLength(1));
  });

  for (final rejection in [null, 'BUSY', 'STALE_REVISION']) {
    test(
      'failed original A retries A, not B; fresh identity and stale actions ($rejection)',
      () async {
        final api = QueueApi()..rejection = rejection;
        final w = await ready(api);
        final c = w.selected;
        await start(c);
        enqueue(c, 'B');
        enqueue(c, 'C');
        if (rejection == null) {
          api.complete(c.pendingId!, success: false);
          await w.synchronize();
        }
        final q = c.messageQueue;
        final old = q.entries.first;
        expect(old.text, 'A');
        expect(old.canRetry, true);
        final original = api.sends.first['request_id'];
        api.rejection = null;
        await q.retry(old.id, old.actionRevision);
        await q.retry(old.id, old.actionRevision);
        q.skip(old.id, old.actionRevision);
        await settle();
        expect(api.sends.map((s) => s['text']), ['A', 'A']);
        expect(c.pendingId, isNot(original));
        expect(q.entries.first.id, old.id);
        api.complete(c.pendingId!);
        await w.synchronize();
        await settle();
        expect(api.sends.map((s) => s['text']), ['A', 'A', 'B']);
        expect(c.localSends.containsKey(original), false);
      },
    );
  }

  test('explicit failed-head Skip continues, Remove pauses the tail', () async {
    final api = QueueApi()..rejection = 'BUSY';
    final w = await ready(api);
    final c = w.selected;
    await start(c);
    enqueue(c, 'B');
    enqueue(c, 'C');
    final q = c.messageQueue;
    final a = q.entries.first;
    api.rejection = null;
    q.skip(a.id, a.actionRevision);
    q.skip(a.id, a.actionRevision);
    await settle();
    expect(api.sends.map((s) => s['text']), ['A', 'B']);
    api.complete(c.pendingId!, success: false);
    await w.synchronize();
    final b = q.entries.first;
    q.remove(b.id, b.actionRevision);
    await settle();
    expect(q.paused, true);
    expect(api.sends, hasLength(2));
    final rev = q.revision;
    await q.resume(rev - 1);
    expect(q.paused, true);
    await q.resume(rev);
    await settle();
    expect(api.sends.map((s) => s['text']), ['A', 'B', 'C']);
  });

  test('unknown post-commit error retains identity; forget and Clear do not skip it', () async {
    final api = QueueApi()..postCommitError = true;
    final w = await ready(api);
    final c = w.selected;
    await start(c);
    final request = c.pendingId;
    enqueue(c, 'B');
    final q = c.messageQueue;
    final a = q.entries.first;
    expect(a.state, QueuedMessageState.unknown);
    expect(a.canRetry || a.canSkip || a.canRemove, false);
    await q.retry(a.id, a.actionRevision);
    q.skip(a.id, a.actionRevision);
    q.remove(a.id, a.actionRevision);
    q.clear(q.revision);
    expect(q.entries.single.id, a.id);
    expect(c.pendingId, request);
    api.expiredPoll = true;
    await w.synchronize();
    c.abandonExpiredRequest(request!);
    c.edit('never resend');
    await c.send();
    await q.resume(q.revision);
    await settle();
    expect(api.sends, hasLength(1));
    expect(q.entries.single.state, QueuedMessageState.unknown);
  });

  test(
    'Pause Remove Clear cannot act as Skip of running predecessor',
    () async {
      final api = QueueApi();
      final w = await ready(api);
      final c = w.selected;
      await start(c);
      enqueue(c, 'B');
      final q = c.messageQueue;
      final a = q.entries.first;
      q.remove(a.id, a.actionRevision);
      q.skip(a.id, a.actionRevision);
      q.pause();
      q.clear(q.revision);
      expect(q.entries.single.text, 'A');
      expect(c.pendingId, isNotNull);
      expect(q.canResume, false);
      expect(api.sends, hasLength(1));
    },
  );

  test(
    'eight waiting plus predecessor, UTF8 bound, drafts and local long wait',
    () async {
      final api = QueueApi();
      final w = await ready(api);
      final c = w.selected;
      await start(c);
      c.messageQueue.pause();
      c.edit('😀' * 4097);
      expect(c.messageQueue.canEnqueue, false);
      c.messageQueue.enqueue();
      expect(c.draft, '😀' * 4097);
      enqueue(c, '😀' * 4096);
      for (var i = 1; i < 8; i++) {
        enqueue(c, 'B$i');
      }
      c.edit('ninth');
      expect(c.messageQueue.canEnqueue, false);
      c.messageQueue.enqueue();
      expect(c.draft, 'ninth');
      expect(c.messageQueue.entries, hasLength(9));
      expect(c.messageQueue.waitingCount, 8);
      api.complete(c.pendingId!, success: false);
      await w.synchronize();
      expect(c.messageQueue.entries, hasLength(9));
      expect(c.messageQueue.waitingCount, 8);
      expect(c.messageQueue.entries.first.canSkip, true);
    },
  );

  test(
    'desktop limit deduplicates shared panes and includes hidden blanks',
    () async {
      final api = QueueApi();
      final w = await ready(api);
      final chats = [w.selected];
      for (var i = 0; i < 3; i++) {
        chats.add(w.createPane().selected);
      }
      for (final c in chats) {
        await start(c);
        c.messageQueue.pause();
        for (var i = 0; i < 8; i++) {
          enqueue(c, 'tail $i');
        }
      }
      expect(w.all.fold<int>(0, (n, c) => n + c.messageQueue.waitingCount), 32);
      w.setVisiblePanes({w.panes.first});
      final hold = w.acquireQueueExitHold();
      expect(hold.messages, 32);
      expect(hold.drafts, 0);
      w.releaseQueueExitHold(hold);
      final shared = w.panes[1];
      await w.selectFor(shared, chats.first.id!);
      final again = w.acquireQueueExitHold();
      expect(again.messages, 32);
    },
  );

  test('New chat promotion retains the exact controller and FIFO', () async {
    final api = QueueApi();
    final w = await ready(api);
    w.newChat();
    final c = w.selected;
    c.edit('A');
    final sending = c.send();
    enqueue(c, 'B');
    await sending;
    expect(w.selected, same(c));
    expect(w.conversations[c.id], same(c));
    expect(w.blank, isNot(same(c)));
    api.complete(c.pendingId!);
    await w.synchronize();
    await settle();
    expect(api.sends.map((s) => s['text']), ['A', 'B']);
    expect(api.sends.last['conversation_id'], c.id);
  });

  test(
    'config and restart changes pause; synchronization never resumes',
    () async {
      final api = QueueApi();
      final w = await ready(api);
      final c = w.selected;
      await start(c);
      enqueue(c, 'B');
      api.config = {...api.config, 'revision': 2};
      api.complete(c.pendingId!);
      await w.synchronize();
      await settle();
      expect(c.messageQueue.paused, true);
      expect(api.sends, hasLength(1));
      api.instance = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
      await w.synchronize();
      await settle();
      expect(api.sends, hasLength(1));
      await c.messageQueue.resume(c.messageQueue.revision);
      await settle();
      expect(api.sends, hasLength(2));
      expect(api.sends.last['expected_config_revision'], 2);
    },
  );

  test('exit hold counts drafts/failures, blocks send/retry/enqueue and is identity bound', () async {
    final api = QueueApi()..rejection = 'BUSY';
    final w = await ready(api);
    final c = w.selected;
    await start(c);
    enqueue(c, 'B');
    c.edit('draft');
    w.createPane().blank.edit('hidden draft');
    final q = c.messageQueue;
    final a = q.entries.first;
    final hold = w.acquireQueueExitHold();
    expect(hold.messages, 2);
    expect(hold.drafts, 2);
    expect(w.acquireQueueExitHold(), same(hold));
    expect(q.canEnqueue, false);
    await q.retry(a.id, a.actionRevision);
    await c.send();
    w.releaseQueueExitHold(QueueExitHold(messages: 2, drafts: 2));
    expect(w.queueExitPending, true);
    w.releaseQueueExitHold(hold);
    expect(w.queueExitPending, false);
    expect(q.entries.first.state, QueuedMessageState.failed);
    expect(api.sends, hasLength(1));
  });

  test('hold acquired during fresh dispatch detail prevents submission after await', () async {
    final api = QueueApi()..rejection = 'BUSY';
    final w = await ready(api);
    final c = w.selected;
    await start(c);
    enqueue(c, 'B');
    api.rejection = null;
    api.detailGate = Completer<void>();
    api.detailEntered = Completer<void>();
    final a = c.messageQueue.entries.first;
    c.messageQueue.skip(a.id, a.actionRevision);
    await api.detailEntered!.future;
    final hold = w.acquireQueueExitHold();
    api.detailGate!.complete();
    await settle();
    expect(api.sends, hasLength(1));
    c.messageQueue.pause();
    w.releaseQueueExitHold(hold);
    await settle();
    expect(api.sends, hasLength(1));
    expect(c.messageQueue.paused, true);
  });

  test('hold fences immediate reservation queued behind observation', () async {
    final api = QueueApi();
    final w = await ready(api);
    final c = w.selected;
    api.detailGate = Completer<void>();
    api.detailEntered = Completer<void>();
    final observation = w.synchronize();
    await api.detailEntered!.future;
    c.edit('A');
    final send = c.send();
    final hold = w.acquireQueueExitHold();
    expect(hold.messages, 1);
    api.detailGate!.complete();
    await observation;
    await send;
    expect(api.sends, isEmpty);
    w.releaseQueueExitHold(hold);
    await settle();
    expect(api.sends, isEmpty);
  });

  test(
    'archive/delete blocked; external deletion retains reachable text',
    () async {
      final api = QueueApi();
      final w = await ready(api);
      final c = w.selected;
      await start(c);
      enqueue(c, 'B');
      for (final op in ['conversation_archive', 'conversation_delete']) {
        await expectLater(
          w.mutate(op, c.id!, 0, 0),
          throwsA(isA<AppFailure>()),
        );
      }
      api.summaries.remove(c.id);
      api.histories.remove(c.id);
      api.jobs.clear();
      await w.synchronize();
      expect(w.selected, same(c));
      expect(w.conversations[c.id], same(c));
      expect(c.messageQueue.entries.map((e) => e.text), ['A', 'B']);
      expect(c.messageQueue.paused, true);
      expect(c.canSend, false);
    },
  );

  test('disposal fences queue work awaiting detail without cancelling accepted work', () async {
    final api = QueueApi()..rejection = 'BUSY';
    final w = WorkspaceController(api, pollInterval: const Duration(days: 1));
    await w.synchronize();
    final c = w.selected;
    await start(c);
    enqueue(c, 'B');
    api.detailGate = Completer<void>();
    api.detailEntered = Completer<void>();
    final a = c.messageQueue.entries.first;
    c.messageQueue.skip(a.id, a.actionRevision);
    await api.detailEntered!.future;
    w.dispose();
    api.detailGate!.complete();
    await settle();
    expect(api.sends, hasLength(1));
    expect(api.disposedCount, 1);
    expect(
      api.calls.where((op) => op.contains('cancel') || op.contains('stop')),
      isEmpty,
    );
  });
}
