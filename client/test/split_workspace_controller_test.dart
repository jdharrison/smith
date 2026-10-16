import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/workspace_controller.dart';

import 'app_fakes.dart';

const other = '22222222222222222222222222222222';

class SplitApi extends FakeAppApi {
  int activeCalls = 0;
  int maxActive = 0;
  Completer<void>? stateGate;
  String? failDetail;
  bool rejectNew = false;

  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    activeCalls++;
    if (activeCalls > maxActive) maxActive = activeCalls;
    try {
      if (op == 'state' && stateGate != null) await stateGate!.future;
      if (op == 'conversation_get' && args['conversation_id'] == failDetail) {
        throw const AppFailure('TRANSPORT', 'Selected detail unavailable');
      }
      if (op == 'submit_new' && rejectNew) {
        throw const AppFailure('CAPACITY', 'No capacity');
      }
      return await super.call(op, args);
    } finally {
      activeCalls--;
    }
  }
}

Future<WorkspaceController> ready(SplitApi api) async {
  api.inventoryObserved = true;
  api.summaries[other] = api.summary(other, 'Other', 1);
  api.histories[other] = [];
  final w = WorkspaceController(api, pollInterval: const Duration(days: 1));
  addTearDown(w.dispose);
  await w.synchronize();
  await w.select(testConversation);
  return w;
}

void main() {
  test(
    'stable bounded slots preserve primary aliases and independent blanks',
    () async {
      final api = SplitApi();
      final w = await ready(api);
      final list = w.panes;
      final primary = list.single;
      final second = w.createPane();
      expect(w.panes, same(list));
      expect(w.selected, same(primary.selected));
      expect(w.blank, same(primary.blank));
      expect(second.selected, same(second.blank));
      expect(second.blank, isNot(same(w.blank)));
      expect(second.blank.canSubmit, true);
      w.newChat();
      w.blank.edit('primary draft');
      second.blank.edit('second draft');
      await w.selectFor(second, testConversation);
      expect(w.selected, same(w.blank));
      w.newChatFor(second);
      expect(second.selected.draft, 'second draft');
      expect(w.blank.draft, 'primary draft');
      w.createPane();
      w.createPane();
      expect(w.createPane, throwsStateError);
      expect(() => list.clear(), throwsUnsupportedError);
      expect(api.calls.where((op) => op.startsWith('submit')), isEmpty);
    },
  );

  test(
    'saved controllers share drafts and reserve a send once across views',
    () async {
      final api = SplitApi()..holdChats = true;
      final w = await ready(api);
      final second = w.createPane();
      await w.selectFor(second, testConversation);
      expect(second.selected, same(w.selected));
      w.selected.edit('shared');
      expect(second.selected.draft, 'shared');
      api.stateGate = Completer<void>();
      final snapshot = w.synchronize();
      await Future<void>.delayed(Duration.zero);
      final first = w.selected.send();
      final duplicate = second.selected.send();
      second.selected.edit('newer');
      api.stateGate!.complete();
      await Future.wait([snapshot, first, duplicate]);
      expect(api.calls.where((op) => op == 'submit'), hasLength(1));
      expect(w.selected.draft, 'newer');
      expect(api.maxActive, 1);
    },
  );

  for (final unknown in [false, true]) {
    test(
      'secondary blank promotes in place without replay (unknown: $unknown)',
      () async {
        final api = SplitApi()..holdChats = true;
        final w = await ready(api);
        final second = w.createPane();
        final original = second.blank;
        final primaryBlank = w.blank;
        original.edit('second first message');
        api.unknownAdmission = unknown;
        await original.send();
        original.edit('newer draft');
        if (unknown) {
          expect(original.uncertain, true);
          expect(original.id, isNull);
          expect(w.outstanding, true);
        }
        w.setVisiblePanes({w.panes.first});
        await w.synchronize();
        expect(original.id, isNotNull);
        expect(second.selected, same(original));
        expect(w.conversations[original.id], same(original));
        expect(second.blank, isNot(same(original)));
        expect(w.blank, same(primaryBlank));
        expect(original.draft, 'newer draft');
        expect(original.uncertain, false);
        expect(w.all.where((c) => identical(c, original)), hasLength(1));
        expect(api.calls.where((op) => op == 'submit_new'), hasLength(1));
        expect(api.maxActive, 1);
      },
    );
  }

  test(
    'four independent blanks promote and still enforce four outstanding',
    () async {
      final api = SplitApi()..holdChats = true;
      final w = await ready(api);
      w.newChat();
      for (var i = 1; i < 4; i++) {
        w.createPane();
      }
      final originals = w.panes.map((p) => p.blank).toList();
      for (var i = 0; i < 4; i++) {
        originals[i].edit('message $i');
      }
      await Future.wait(originals.map((c) => c.send()));
      expect(originals.map((c) => c.id).toSet(), hasLength(4));
      expect(originals.every((c) => c.id != null && c.pendingId != null), true);
      expect(w.all.where((c) => c.pendingId != null), hasLength(4));
      expect(w.panes.every((p) => !p.blank.canSubmit), true);
      expect(api.maxActive, 1);
    },
  );

  test(
    'rejected secondary blank stays reusable without affecting primary',
    () async {
      final api = SplitApi()..rejectNew = true;
      final w = await ready(api);
      final second = w.createPane();
      final blank = second.blank;
      blank.edit('rejected text');
      await blank.send();
      expect(second.blank, same(blank));
      expect(blank.id, isNull);
      expect(blank.pendingId, isNull);
      expect(blank.uncertain, false);
      expect(blank.localSends.values.single.text, 'rejected text');
      expect(blank.localSends.values.single.active, false);
      expect(w.selected.id, testConversation);
      blank.edit('retry manually');
      expect(blank.canSend, true);
    },
  );

  test(
    'visible snapshots deduplicate shared chats and omit hidden detail',
    () async {
      final api = SplitApi();
      final w = await ready(api);
      final second = w.createPane();
      await w.selectFor(second, testConversation);
      w.setVisiblePanes({w.panes.first, second});
      api.calls.clear();
      await w.synchronize();
      expect(api.calls.where((op) => op == 'conversation_get'), hasLength(1));
      await w.selectFor(second, other);
      api.calls.clear();
      await w.synchronize();
      expect(api.calls.where((op) => op == 'conversation_get'), hasLength(2));
      w.setVisiblePanes({second});
      api.calls.clear();
      await w.synchronize();
      expect(api.calls.where((op) => op == 'conversation_get'), hasLength(1));
      expect(w.selected.id, testConversation);
      expect(api.maxActive, 1);
    },
  );

  test(
    'secondary terminal answer survives detail failure and later reconciles',
    () async {
      final api = SplitApi();
      final w = await ready(api);
      final second = w.createPane();
      await w.selectFor(second, other);
      w.setVisiblePanes({w.panes.first, second});
      api.jobs[testInstance] = {
        'request_id': testInstance,
        'conversation_id': other,
        'kind': 'submit',
        'status': 'succeeded',
        'phase': 'complete',
        'config_revision': 1,
        'result': {
          'assistant_text': 'retained answer',
          'audio': null,
          'speech_error': null,
        },
        'error': null,
      };
      api.failDetail = other;
      await w.synchronize();
      expect(
        second.selected.provisionalAnswers[testInstance],
        'retained answer',
      );
      expect(second.selected.detailStale, true);
      expect(second.selected.error, contains('detail unavailable'));
      expect(w.selected.provisionalAnswers, isEmpty);
      api.failDetail = null;
      api.histories[other] = [
        {'role': 'user', 'content': 'original question'},
        {'role': 'assistant', 'content': 'retained answer'},
      ];
      api.summaries[other]!['history_revision'] = 1;
      await w.synchronize();
      expect(second.selected.provisionalAnswers, isEmpty);
      expect(second.selected.history.last['content'], 'retained answer');
      expect(w.stale, false);
      expect(api.calls.where((op) => op.startsWith('submit')), isEmpty);
    },
  );

  for (final operation in [
    'archive',
    'delete',
    'index removal',
    'index archive',
  ]) {
    test(
      '$operation repairs visible and hidden selections to their own blanks',
      () async {
        final api = SplitApi();
        final w = await ready(api);
        final second = w.createPane();
        await w.selectFor(second, testConversation);
        final old = w.selected;
        w.blank.edit('first blank');
        second.blank.edit('second blank');
        old.edit('saved draft');
        w.setVisiblePanes({w.panes.first});
        if (operation == 'index removal') {
          api.summaries.remove(testConversation);
          await w.synchronize();
        } else if (operation == 'index archive') {
          api.summaries[testConversation]!['archived'] = true;
          await w.synchronize();
        } else {
          await w.mutate('conversation_$operation', testConversation, 0, 0);
        }
        expect(w.selected, same(w.blank));
        expect(second.selected, same(second.blank));
        expect(w.selected.draft, 'first blank');
        expect(second.selected.draft, 'second blank');
        if (operation.contains('archive')) {
          expect(old.draft, 'saved draft');
          expect(w.conversations[testConversation], same(old));
        } else {
          expect(w.conversations.containsKey(testConversation), false);
          expect(old.canSubmit, false);
        }
        expect(api.disposedCount, 0);
      },
    );
  }

  test('queued selection cannot override newer New chat navigation', () async {
    final api = SplitApi();
    final w = await ready(api);
    final second = w.createPane();
    api.stateGate = Completer<void>();
    final snapshot = w.synchronize();
    await Future<void>.delayed(Duration.zero);
    final selection = w.selectFor(second, other);
    w.newChatFor(second);
    api.stateGate!.complete();
    await Future.wait([snapshot, selection]);
    expect(second.selected, same(second.blank));
    expect(api.maxActive, 1);
  });

  test(
    'rename is shared and uncertain archive repairs all panes without replay',
    () async {
      final api = SplitApi();
      final w = await ready(api);
      final second = w.createPane();
      await w.selectFor(second, testConversation);
      final saved = second.selected;
      await w.mutate(
        'conversation_rename',
        testConversation,
        0,
        0,
        title: 'Renamed',
      );
      expect(w.selected.summary!['title'], 'Renamed');
      expect(second.selected, same(w.selected));
      api.unknownAdmission = true;
      await expectLater(
        w.mutate('conversation_archive', testConversation, 1, 0),
        throwsA(isA<AppFailure>()),
      );
      expect(second.selected, same(saved));
      expect(w.mutations.containsKey(testConversation), true);
      w.setVisiblePanes({w.panes.first});
      await w.synchronize();
      expect(w.mutations, isEmpty);
      expect(w.selected, same(w.blank));
      expect(second.selected, same(second.blank));
      expect(
        api.calls.where((op) => op == 'conversation_archive'),
        hasLength(1),
      );
      api.unknownAdmission = false;
      await w.mutate('conversation_restore', testConversation, 2, 0);
      expect(w.selected, same(w.blank));
      expect(second.selected, same(second.blank));
      expect(saved.archived, false);
    },
  );

  test('foreign panes are rejected and shared API is disposed once', () {
    final api = SplitApi();
    final w = WorkspaceController(api);
    final foreign = WorkspaceController(SplitApi());
    expect(() => w.newChatFor(foreign.panes.first), throwsArgumentError);
    expect(() => w.selectFor(foreign.panes.first, other), throwsArgumentError);
    expect(() => w.setVisiblePanes({foreign.panes.first}), throwsArgumentError);
    w.createPane();
    w.createPane();
    w.createPane();
    w.setVisiblePanes({});
    w.dispose();
    foreign.dispose();
    expect(api.disposedCount, 1);
  });
}
