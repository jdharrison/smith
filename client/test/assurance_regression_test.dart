import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/app_v2.dart';
import 'package:smith_client/workspace_controller.dart';

import 'app_fakes.dart';

const _newer = '22222222222222222222222222222222';
Map<String, dynamic> terminal(
  String kind, {
  dynamic result,
  dynamic error,
  String id = testInstance,
  String status = 'succeeded',
  dynamic revision = 1,
}) => {
  'request_id': id,
  'conversation_id': testConversation,
  'kind': kind,
  'status': status,
  'phase': 'complete',
  'config_revision': revision,
  'result': result,
  'error': error,
};
Map<String, dynamic> answer({dynamic speechError}) => {
  'assistant_text': 'saved answer',
  'audio': null,
  'speech_error': speechError,
};
const expiredAudio = {'code': 'AUDIO_EXPIRED', 'message': 'Audio expired'};
const interrupted = {'code': 'INTERRUPTED', 'message': 'Service restarted'};

class PollFault extends FakeAppApi {
  bool fault = true;
  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    if (op == 'poll' && fault) {
      throw const AppFailure(
        'STATE_UNAVAILABLE',
        'Persistence failed; state preserved',
      );
    }
    return super.call(op, args);
  }
}

class AdmissionRejected extends FakeAppApi {
  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    if (op == 'submit' || op == 'submit_new') {
      throw const AppFailure('CAPACITY', 'No admission');
    }
    return super.call(op, args);
  }
}

Future<WorkspaceController> ready(FakeAppApi api) async {
  api.inventoryObserved = true;
  final w = WorkspaceController(api, pollInterval: const Duration(days: 1));
  await w.synchronize();
  expect(w.stale, false, reason: w.error);
  return w;
}

void main() {
  for (final oldStatus in ['failed', 'succeeded']) {
    test(
      'resolving old $oldStatus identity cannot overwrite a newer success',
      () async {
        final api = FakeAppApi();
        final w = await ready(api);
        addTearDown(w.dispose);
        api.unknownAdmission = true;
        final c = w.selected;
        c.edit('old prompt');
        await c.send();
        final old = c.pendingId!;
        c.edit('new local draft');
        api.jobs[old] = terminal(
          'submit',
          id: old,
          status: oldStatus,
          result: oldStatus == 'succeeded' ? answer() : null,
          error: oldStatus == 'failed' ? interrupted : null,
        );
        api.jobs[_newer] = terminal(
          'submit',
          id: _newer,
          result: {...answer(), 'assistant_text': 'newer answer'},
        );
        api.history.addAll([
          {'role': 'user', 'content': 'newer prompt'},
          {'role': 'assistant', 'content': 'newer answer'},
        ]);
        await w.synchronize();
        expect(c.pendingId, isNull);
        expect(c.operationStatus, 'succeeded');
        expect(c.error, isNull);
        expect(c.provisionalAnswers, isEmpty);
        expect(c.history.last['content'], 'newer answer');
        expect(c.draft, 'new local draft');
        expect(api.calls.where((op) => op == 'submit'), hasLength(1));
      },
    );
  }
  test(
    'resolving old identity adopts newer running work without replay',
    () async {
      final api = FakeAppApi();
      final w = await ready(api);
      addTearDown(w.dispose);
      api.unknownAdmission = true;
      w.selected.edit('old');
      await w.selected.send();
      final old = w.selected.pendingId!;
      api.jobs[old] = terminal(
        'submit',
        id: old,
        status: 'failed',
        error: interrupted,
      );
      api.jobs[_newer] = {
        ...terminal('submit', id: _newer, status: 'running'),
        'phase': 'generation',
      };
      await w.synchronize();
      expect(w.selected.pendingId, _newer);
      expect(w.selected.operationStatus, 'running');
      expect(w.selected.error, isNull);
      expect(api.calls.where((op) => op == 'submit'), hasLength(1));
    },
  );
  for (final fresh in [false, true]) {
    test(
      'accepted ${fresh ? 'submit_new' : 'submit'} poll failure preserves identity and query-only recovery',
      () async {
        final api = PollFault();
        final w = await ready(api);
        addTearDown(w.dispose);
        if (fresh) w.newChat();
        final c = w.selected;
        c.edit('accepted prompt');
        await c.send();
        final id = c.pendingId;
        expect(id, isNotNull);
        expect(api.jobs, hasLength(1));
        expect(c.canSend, false);
        expect(w.stale, true);
        expect(c.uncertain, true);
        c.edit('new draft');
        await c.send();
        expect(api.jobs, hasLength(1));
        api.fault = false;
        await w.synchronize();
        expect(c.pendingId, isNull);
        expect(c.draft, 'new draft');
        expect(w.selected, same(c));
        expect(c.history.last['content'], 'useful answer');
        expect(api.calls.where((op) => op.startsWith('submit')), hasLength(1));
      },
    );
  }
  test(
    'definitive admission rejection alone releases the local request',
    () async {
      final api = AdmissionRejected();
      final w = await ready(api);
      addTearDown(w.dispose);
      w.selected.edit('not admitted');
      await w.selected.send();
      expect(w.selected.pendingId, isNull);
      expect(api.jobs, isEmpty);
      expect(w.selected.draft, isEmpty);
      expect(w.selected.localSends.values.single.text, 'not admitted');
      expect(w.selected.localSends.values.single.active, false);
      expect(w.selected.canSend, false);
    },
  );
  test(
    'valid migrated submit speech_error and inert speak expiry remain readable',
    () {
      validateV2(
        'poll',
        terminal('submit', result: answer(speechError: expiredAudio)),
      );
      validateV2('poll', terminal('speak', error: expiredAudio));
      validateV2(
        'poll',
        terminal(
          'transcribe',
          result: {'text': '', 'language': '', 'duration_ms': 0},
        ),
      );
      for (final bad in [
        terminal('speak', result: {'audio': 'payload'}, error: expiredAudio),
        terminal('speak', error: interrupted),
        terminal(
          'submit',
          result: answer(
            speechError: {'code': 'AUDIO_EXPIRED', 'message': 'x' * 1025},
          ),
        ),
        terminal('submit_new', result: answer(speechError: expiredAudio)),
      ]) {
        expect(() => validateV2('poll', bad), throwsA(isA<AppFailure>()));
      }
    },
  );
  test(
    'reattached migrated reply preserves text without executing speech',
    () async {
      final api = FakeAppApi()
        ..inventoryObserved = true
        ..detailFailure = true;
      api.jobs[testInstance] = terminal(
        'submit',
        result: answer(speechError: expiredAudio),
      );
      final w = WorkspaceController(api, pollInterval: const Duration(days: 1));
      addTearDown(w.dispose);
      await w.synchronize();
      expect(w.selected.provisionalAnswers, {testInstance: 'saved answer'});
      expect(
        api.calls.where(
          (op) => ['submit', 'submit_new', 'speak', 'transcribe'].contains(op),
        ),
        isEmpty,
      );
    },
  );
  for (final status in ['failed', 'interrupted']) {
    test(
      'reattach exposes latest $status submission despite successful detail read',
      () async {
        final api = FakeAppApi();
        api.jobs[testInstance] = terminal(
          'submit',
          status: status,
          error: interrupted,
        );
        final w = await ready(api);
        addTearDown(w.dispose);
        expect(w.selected.error, contains('INTERRUPTED'));
        expect(w.selected.operationStatus, status);
        expect(w.selected.history, isEmpty);
        expect(w.selected.pendingId, isNull);
        expect(api.calls.where((op) => op.startsWith('submit')), isEmpty);
        api.jobs[_newer] = terminal('submit', id: _newer, result: answer());
        await w.synchronize();
        expect(w.selected.error, isNull);
        expect(w.selected.operationStatus, 'succeeded');
      },
    );
  }
  test(
    'newer success suppresses older terminal failure on first reattach',
    () async {
      final api = FakeAppApi();
      api.jobs[testInstance] = terminal(
        'submit',
        status: 'interrupted',
        error: interrupted,
      );
      api.jobs[_newer] = terminal('submit', id: _newer, result: answer());
      final w = await ready(api);
      addTearDown(w.dispose);
      expect(w.selected.error, isNull);
      expect(api.calls.where((op) => op == 'poll'), isEmpty);
    },
  );
  test('lost rename receipt resolves identity without rolling metadata or history backwards', () async {
    final api = FakeAppApi();
    final w = await ready(api);
    addTearDown(w.dispose);
    api.unknownAdmission = true;
    await expectLater(
      w.mutate(
        'conversation_rename',
        testConversation,
        0,
        0,
        title: 'Old receipt',
      ),
      throwsA(isA<AppFailure>()),
    );
    api.summaries[testConversation]!.addAll({
      'title': 'Newest title',
      'metadata_revision': 2,
      'history_revision': 3,
    });
    await w.synchronize();
    final s = w.selected.summary!;
    expect(s['title'], 'Newest title');
    expect(s['metadata_revision'], 2);
    expect(s['history_revision'], 3);
    expect(w.mutations, isEmpty);
    expect(api.calls.where((op) => op == 'conversation_rename'), hasLength(1));
  });
  test(
    'old archive receipt cannot undo a newer restore or switch selection',
    () async {
      final api = FakeAppApi();
      final w = await ready(api);
      addTearDown(w.dispose);
      final selected = w.selected;
      api.unknownAdmission = true;
      await expectLater(
        w.mutate('conversation_archive', testConversation, 0, 0),
        throwsA(isA<AppFailure>()),
      );
      api.summaries[testConversation]!.addAll({
        'archived': false,
        'metadata_revision': 2,
      });
      await w.synchronize();
      expect(w.selected, same(selected));
      expect(w.selected.archived, false);
      expect(w.mutations, isEmpty);
    },
  );
  test(
    'lost delete receipt remains queryable after target disappears',
    () async {
      final api = FakeAppApi();
      final w = await ready(api);
      addTearDown(w.dispose);
      api.unknownAdmission = true;
      await expectLater(
        w.mutate('conversation_delete', testConversation, 0, 0),
        throwsA(isA<AppFailure>()),
      );
      await w.synchronize();
      expect(w.conversations, isEmpty);
      expect(w.mutations, isEmpty);
      expect(w.selected, same(w.blank));
      expect(w.stale, false);
    },
  );
  for (final revision in [
    9007199254740991,
    '9007199254740992',
    '18446744073709551615',
  ]) {
    test(
      'configuration revision $revision stays exact across state, poll, configure and admission echo',
      () async {
        expect(wireConfigRevision(revision), true);
        final api = FakeAppApi()..config['revision'] = revision;
        final w = await ready(api);
        addTearDown(w.dispose);
        expect(w.config['revision'], revision);
        w.selected.edit('existing');
        await w.selected.send();
        w.newChat();
        w.selected.edit('new');
        await w.selected.send();
        for (final op in ['submit', 'submit_new']) {
          final args = api.arguments[api.calls.indexOf(op)];
          expect(args['expected_config_revision'], revision);
          validateV2Arguments(op, args);
        }
        validateV2('configure', {
          'model': 'local:exact',
          'voice': null,
          'revision': revision,
        });
        validateV2(
          'poll',
          terminal('submit', revision: revision, result: answer()),
        );
        final modelOp = {
          ...terminal(
            'verify_model',
            revision: revision,
            result: {
              'model': 'local:exact',
              'sha256': 'a' * 64,
              'verified': true,
              'assistant_text': 'ok',
            },
          ),
          'conversation_id': null,
        };
        AppReply.parse(
          jsonEncode({
            'schema_version': 2,
            'service': 'smith-next',
            'protocol': 'app',
            'pid': 123,
            'instance_id': testInstance,
            'call_id': testInstance,
            'ok': true,
            'result': modelOp,
            'error': null,
          }),
          testInstance,
        );
      },
    );
  }
  test('configuration revision parser rejects noncanonical, unsafe numeric and overflow values', () {
    for (final value in [
      null,
      -1,
      1.0,
      9007199254740992,
      '0',
      '1',
      '9007199254740991',
      '09007199254740992',
      '+9007199254740992',
      '9007199254740992 ',
      '9.007199254740992e15',
      '18446744073709551616',
      '9' * 100,
    ]) {
      expect(wireConfigRevision(value), false, reason: '$value');
      expect(
        () => validateV2('configure', {
          'model': 'local:exact',
          'voice': null,
          'revision': value,
        }),
        throwsA(isA<AppFailure>()),
      );
    }
    expect(wireRevision('9007199254740992'), false);
  });
  test('readiness requires coherent error observations and current-instance inventory', () {
    final api = FakeAppApi()..inventoryObserved = true;
    final incoherent =
        jsonDecode(jsonEncode(api.state)) as Map<String, dynamic>;
    incoherent['inventory']['providers']['llm']['error'] = interrupted;
    expect(() => validateV2('state', incoherent), throwsA(isA<AppFailure>()));
    final unobserved = {...api.state, 'inventory_observed': false};
    expect(() => validateV2('state', unobserved), throwsA(isA<AppFailure>()));
    api.inventoryObserved = false;
    validateV2('state', api.state);
  });
  test('bounded histories alternate complete pairs and permit legacy empty messages', () {
    final summary = FakeAppApi().summaries[testConversation];
    void check(List<dynamic> history) => validateV2('conversation_get', {
      'conversation': summary,
      'history': history,
    });
    check([
      {'role': 'user', 'content': ''},
      {'role': 'assistant', 'content': ''},
    ]);
    for (final history in [
      [
        {'role': 'assistant', 'content': 'wrong start'},
        {'role': 'user', 'content': ''},
      ],
      [
        {'role': 'user', 'content': 'orphan'},
      ],
      [
        {'role': 'user', 'content': ''},
        {'role': 'user', 'content': 'wrong role'},
      ],
      [
        {'role': 'user', 'content': 'x' * 16385},
        {'role': 'assistant', 'content': ''},
      ],
    ]) {
      expect(() => check(history), throwsA(isA<AppFailure>()));
    }
  });
}
