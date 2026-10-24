import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/projects_controller.dart';
import 'package:smith_client/retained_jobs.dart';
import 'package:smith_client/retained_jobs_controller.dart';
import 'package:smith_client/ui/retained_jobs_view.dart';

import 'dev_jobs_test.dart' as fixture;

const sid = '44444444444444444444444444444444';
Map<String, dynamic> job({String status = 'ready', bool retained = true}) => {
  'session_id': sid,
  'project_id': fixture.projectId,
  'project_key': 'smith',
  'job_type': 'write',
  'status': status,
  'retained': retained,
  'deleted': false,
  'created_ms': 1700000000000,
};

class Api extends fixture.JobsApi {
  bool available = false;
  int revision = 1;
  List<Map<String, dynamic>> rows = [];
  final mutations = <Map<String, dynamic>>[];
  final operations = <String>[];
  final receiptQueries = <String>[];
  final receipts = <String, Map<String, dynamic>>{};
  AppReply? receiptOverride;
  Completer<AppReply>? receiptGate;
  Map<String, dynamic>? tombstone;
  List<Map<String, dynamic>>? savedTurns;
  AppFailure? failure;
  bool foreignReceipt = false;
  Completer<void>? mutationGate;
  Completer<AppReply>? getGate;
  Map<String, dynamic> state() => {
    'revision': revision,
    'jobs': rows,
    'capability': {
      'available': available,
      'reason': available ? null : 'runtime_not_prepared',
      'job_types': ['read', 'write'],
    },
    'scheduler': {
      'limit': 3,
      'occupied': rows.where((j) => j['retained'] == true).length,
      'queued': 0,
      'accepting': available,
    },
  };
  Map<String, dynamic> detail() => {
    'revision': revision,
    'job': rows.isEmpty ? tombstone : rows.first,
    'turns':
        savedTurns ??
        [
          {
            'operation_id': fixture.requestId,
            'prompt': 'original prompt',
            'outcome': 'succeeded',
            'reply': 'saved reply',
          },
        ],
    'repositories': [
      {
        'url': 'https://example.org/repo',
        'reference': 'main',
        'read_only':
            (rows.isEmpty ? tombstone : rows.first)?['job_type'] == 'read',
      },
    ],
  };
  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    if (!retainedJobOperations.contains(op)) return super.call(op, args);
    validateRetainedArguments(op, args);
    operations.add(op);
    if (op == 'dev_sessions_state_v1') {
      return AppReply(serviceInstance, state());
    }
    if (op == 'dev_session_get_v1') {
      return getGate?.future ?? AppReply(serviceInstance, detail());
    }
    if (op == 'dev_session_receipt_v1') {
      final id = args['operation_id'] as String;
      receiptQueries.add(id);
      if (receiptGate != null) return receiptGate!.future;
      if (receiptOverride != null) return receiptOverride!;
      final result = receipts[id];
      if (result == null) {
        throw const AppFailure('JOB_NOT_FOUND', 'Receipt not found');
      }
      return AppReply(serviceInstance, result);
    }
    mutations.add(Map.of(args));
    await mutationGate?.future;
    if (failure != null && !acceptBeforeFailure) throw failure!;
    revision++;
    if (op == 'dev_session_start_v1') {
      rows = [job()..['job_type'] = args['job_type']];
    }
    if (op == 'dev_session_finish_v1') {
      rows = [job(status: 'finished', retained: false)];
    }
    final resultJob = Map<String, dynamic>.of(rows.first);
    if (op == 'dev_session_delete_v1') {
      resultJob['deleted'] = true;
      rows = [];
    }
    final result = <String, dynamic>{
      'receipt': {
        'operation_id': foreignReceipt
            ? fixture.requestId
            : args['operation_id'],
        'session_id': sid,
        'revision': revision,
      },
      'job': resultJob,
    };
    if (acceptBeforeFailure) {
      receipts[args['operation_id'] as String] = result;
      if (op == 'dev_session_prompt_v1') {
        savedTurns = [
          {
            'operation_id': args['operation_id'],
            'prompt': args['prompt'],
            'outcome': 'succeeded',
            'reply': 'accepted reply',
          },
        ];
      }
    }
    if (failure != null) throw failure!;
    return AppReply(serviceInstance, result);
  }
}

class Executor implements AppExecutor {
  Executor(this.api);
  final Api api;
  @override
  Future<String> exchange(List<int> frame) async {
    final req = jsonDecode(utf8.decode(frame)) as Map<String, dynamic>;
    final r = await api.call(
      req['op'] as String,
      Map<String, dynamic>.from(req['args'] as Map),
    );
    return jsonEncode({
      'schema_version': 3,
      'service': 'smith-next',
      'protocol': 'app',
      'call_id': req['call_id'],
      'pid': 1,
      'instance_id': r.instanceId,
      'ok': true,
      'result': r.result,
      'error': null,
    });
  }

  @override
  void dispose() {}
}

Future<RetainedJobsController> setup(Api api) async {
  final p = ProjectsController(api);
  await p.refresh();
  final c = RetainedJobsController(api, p);
  await c.refresh();
  addTearDown(() {
    c.dispose();
    p.dispose();
  });
  return c;
}

Map<String, dynamic> receiptResult(
  String operation,
  Map<String, dynamic> summary, {
  int revision = 2,
}) => {
  'receipt': {
    'operation_id': operation,
    'session_id': summary['session_id'],
    'revision': revision,
  },
  'job': summary,
};

void main() {
  testWidgets(
    'late recovery cannot clear a replacement composer with identical text',
    (tester) async {
      final api = Api()
        ..available = true
        ..rows = [job()]
        ..failure = const AppFailure('TRANSPORT', 'lost', unknown: true);
      final c = await setup(api);
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(body: RetainedJobsView(controller: c)),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('smith · write'));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'same text');
      await tester.pump();
      await tester.tap(find.text('Send'));
      await tester.pumpAndSettle();
      final operation = c.pending!['operation_id'] as String;
      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('smith · write'));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'same text');
      api.revision = 2;
      api.receipts[operation] = receiptResult(operation, job());
      api.savedTurns = [
        {
          'operation_id': operation,
          'prompt': 'same text',
          'outcome': 'succeeded',
          'reply': 'accepted reply',
        },
      ];
      await tester.pump();
      final check = find.descendant(
        of: find.byType(AlertDialog),
        matching: find.widgetWithText(TextButton, 'Check acknowledgement'),
      );
      await tester.tap(check);
      await tester.pumpAndSettle();
      expect(c.pending, isNull);
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text,
        'same text',
      );
      expect(api.mutations.length, 1);
      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();
    },
  );
  for (final recovery in ['immediate', 'later']) {
    for (final edit in ['unchanged', 'text', 'selection', 'composition']) {
      testWidgets(
        '$recovery lost-ack prompt recovery clears only unchanged editing value ($edit)',
        (tester) async {
          final api = Api()
            ..available = true
            ..rows = [job()]
            ..failure = const AppFailure('TRANSPORT', 'lost', unknown: true)
            ..acceptBeforeFailure = recovery == 'immediate';
          final c = await setup(api);
          await tester.pumpWidget(
            MaterialApp(
              home: Scaffold(body: RetainedJobsView(controller: c)),
            ),
          );
          await tester.pumpAndSettle();
          await tester.tap(find.text('smith · write'));
          await tester.pumpAndSettle();
          final composer = tester
              .widget<TextField>(find.byType(TextField))
              .controller!;
          const submitted = TextEditingValue(
            text: ' exact\n prompt ',
            selection: TextSelection(baseOffset: 2, extentOffset: 5),
          );
          composer.value = submitted;
          await tester.pump();
          api.mutationGate = Completer<void>();
          await tester.tap(find.text('Send'));
          await tester.pump();
          final newer = switch (edit) {
            'text' => const TextEditingValue(
              text: 'newer draft',
              selection: TextSelection.collapsed(offset: 4),
            ),
            'selection' => submitted.copyWith(
              selection: const TextSelection.collapsed(offset: 7),
            ),
            'composition' => submitted.copyWith(
              composing: const TextRange(start: 1, end: 4),
            ),
            _ => submitted,
          };
          composer.value = newer;
          api.mutationGate!.complete();
          await tester.pumpAndSettle();
          final operation = api.mutations.single['operation_id'] as String;
          if (recovery == 'later') {
            expect(c.pending!['operation_id'], operation);
            expect(composer.value, newer);
            api.revision = 2;
            api.receipts[operation] = receiptResult(operation, job());
            api.savedTurns = [
              {
                'operation_id': operation,
                'prompt': submitted.text,
                'outcome': 'succeeded',
                'reply': 'accepted reply',
              },
            ];
            final check = find.descendant(
              of: find.byType(AlertDialog),
              matching: find.widgetWithText(
                TextButton,
                'Check acknowledgement',
              ),
            );
            await tester.ensureVisible(check);
            await tester.tap(check);
            await tester.pumpAndSettle();
          }
          expect(c.pending, isNull);
          expect(
            composer.value,
            edit == 'unchanged'
                ? const TextEditingValue(
                    selection: TextSelection.collapsed(offset: 0),
                  )
                : newer,
          );
          expect(api.mutations.length, 1);
          expect(api.mutations.single['prompt'], submitted.text);
          expect(c.details[sid]!.turns.single.operationId, operation);
          // A repeated observation must not consume later drafts a second time.
          composer.value = const TextEditingValue(text: 'next draft');
          await c.refresh();
          await tester.pump();
          expect(composer.text, 'next draft');
          await tester.tap(find.text('Close'));
          await tester.pumpAndSettle();
        },
      );
    }
  }

  test('immediate recovered admission returns true and notifies original operation once', () async {
    final api = Api()
      ..available = true
      ..rows = [job()]
      ..failure = const AppFailure('TRANSPORT', 'lost', unknown: true)
      ..acceptBeforeFailure = true;
    final c = await setup(api);
    await c.loadDetail(sid);
    final admitted = <String>[];
    expect(
      await c.act(
        sid,
        'prompt',
        prompt: 'exact prompt',
        onAdmitted: admitted.add,
      ),
      true,
    );
    expect(admitted, [api.mutations.single['operation_id']]);
    expect(c.pending, isNull);
    await c.refresh();
    expect(admitted.length, 1);
    expect(api.mutations.length, 1);
  });

  testWidgets(
    'CONTEXT_LIMIT preserves prompt, clears pending and allows corrected Send',
    (tester) async {
      final api = Api()
        ..available = true
        ..rows = [job()]
        ..failure = const AppFailure(
          'CONTEXT_LIMIT',
          'Composed task exceeds limit',
        );
      final c = await setup(api);
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(body: RetainedJobsView(controller: c)),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('smith · write'));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'short prompt');
      await tester.pump();
      await tester.tap(find.text('Send'));
      await tester.pumpAndSettle();
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text,
        'short prompt',
      );
      expect(c.pending, isNull);
      expect(find.text('Action refused. Review again.'), findsWidgets);
      expect(api.receiptQueries, isEmpty);
      expect(api.mutations.length, 1);
      final refusedOperation = api.mutations.single['operation_id'];
      api.failure = null;
      await tester.enterText(find.byType(TextField), 'corrected prompt');
      await tester.pump();
      await tester.tap(find.text('Send'));
      await tester.pumpAndSettle();
      expect(api.mutations.length, 2);
      expect(api.mutations.last['operation_id'], isNot(refusedOperation));
      expect(api.mutations.last['prompt'], 'corrected prompt');
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text,
        isEmpty,
      );
      expect(c.pending, isNull);
      expect(api.receiptQueries, isEmpty);
      expect(retainedPromptValid('é' * 8192), true);
      expect(retainedPromptValid('é' * 8193), false);
      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();
    },
  );
  testWidgets(
    'CONTEXT_LIMIT preserves Start draft and allows corrected reviewed Start',
    (tester) async {
      final api = Api()
        ..available = true
        ..failure = const AppFailure(
          'CONTEXT_LIMIT',
          'Composed task exceeds limit',
        );
      final c = await setup(api);
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(body: RetainedJobsView(controller: c)),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('New read/write job'));
      await tester.pumpAndSettle();
      await tester.tap(find.byType(DropdownButtonFormField<String>).first);
      await tester.pumpAndSettle();
      await tester.tap(find.text('smith').last);
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'original task');
      await tester.pump();
      await tester.tap(find.text('Review'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Start'));
      await tester.pumpAndSettle();
      expect(c.pending, isNull);
      expect(c.canStart, true);
      expect(find.text('Action refused. Review again.'), findsWidgets);
      expect(api.receiptQueries, isEmpty);
      expect(api.mutations.length, 1);
      final refusedOperation = api.mutations.single['operation_id'];
      expect(api.mutations.single['prompt'], 'original task');
      await tester.tap(find.text('Edit'));
      await tester.pumpAndSettle();
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text,
        'original task',
      );
      api.failure = null;
      await tester.enterText(find.byType(TextField), 'corrected task');
      await tester.pump();
      await tester.tap(find.text('Review'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Start'));
      await tester.pumpAndSettle();
      expect(api.mutations.length, 2);
      expect(api.mutations.last['operation_id'], isNot(refusedOperation));
      expect(api.mutations.last['prompt'], 'corrected task');
      expect(c.pending, isNull);
      expect(api.receiptQueries, isEmpty);
      expect(find.byType(AlertDialog), findsNothing);
    },
  );

  testWidgets('pending acknowledgement uses contextual query-only check', (
    tester,
  ) async {
    final api = Api()
      ..available = true
      ..failure = const AppFailure('TRANSPORT', 'lost', unknown: true);
    final c = await setup(api);
    await c.start(c.captureStart(fixture.projectId, 'write', 'prompt'));
    final original = c.pending!;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(body: RetainedJobsView(controller: c)),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Check acknowledgement'), findsOneWidget);
    expect(find.text('Retained jobs'), findsNothing);
    final before = api.receiptQueries.length;
    await tester.tap(find.text('Check acknowledgement'));
    await tester.pumpAndSettle();
    expect(api.receiptQueries.length, before + 1);
    expect(api.receiptQueries.last, original['operation_id']);
    expect(c.pending, same(original));
    expect(api.mutations.length, 1);
    await tester.pumpWidget(const SizedBox());
    c.dispose();
  });
  test(
    'receipt query is exact, typed and native operation-correlated',
    () async {
      final api = Api();
      final native = NativeAppApi(executor: Executor(api));
      addTearDown(native.dispose);
      final args = {'operation_id': fixture.requestId};
      api.receipts[fixture.requestId] = receiptResult(fixture.requestId, job());
      final reply = await native.call('dev_session_receipt_v1', args);
      expect(RetainedReceipt.parse(reply.result).sessionId, sid);
      expect(api.mutations, isEmpty);
      for (final invalid in <Map<String, dynamic>>[
        {},
        {'operation_id': 'bad'},
        {...args, 'session_id': sid},
      ]) {
        expect(
          () => validateRetainedArguments('dev_session_receipt_v1', invalid),
          throwsA(isA<AppFailure>()),
        );
      }
      api.receipts[fixture.requestId] = receiptResult(sid, job());
      await expectLater(
        native.call('dev_session_receipt_v1', args),
        throwsA(isA<AppFailure>()),
      );
      api.receipts[fixture.requestId] = {
        ...receiptResult(fixture.requestId, job()),
        'extra': true,
      };
      await expectLater(
        native.call('dev_session_receipt_v1', args),
        throwsA(isA<AppFailure>()),
      );
    },
  );

  for (final action in ['start', 'prompt', 'finish', 'delete']) {
    test(
      'lost $action acknowledgement recovers only original receipt and consistent saved job',
      () async {
        final api = Api()..available = true;
        if (action != 'start') {
          api.rows = [
            job(
              status: action == 'delete' ? 'finished' : 'ready',
              retained: action != 'delete',
            ),
          ];
        }
        final c = await setup(api);
        await c.loadDetail(sid);
        api.failure = const AppFailure('TRANSPORT', 'lost', unknown: true);
        if (action == 'start') {
          await c.start(
            c.captureStart(fixture.projectId, 'write', 'exact prompt'),
          );
        } else {
          await c.act(sid, action, prompt: 'exact prompt');
        }
        final original = c.pending!;
        expect(api.mutations.length, 1);
        // A missing receipt remains fenced, even with otherwise plausible rows.
        await c.refresh();
        expect(c.pending, same(original));
        api.revision = 2;
        final saved = job(
          status: {'finish', 'delete'}.contains(action) ? 'finished' : 'ready',
          retained: !{'finish', 'delete'}.contains(action),
        );
        if (action == 'delete') {
          saved['deleted'] = true;
          api.tombstone = saved;
          api.rows = [];
        } else {
          api.rows = [saved];
        }
        api.savedTurns = [
          {
            'operation_id': original['operation_id'],
            'prompt': 'exact prompt',
            'outcome': 'succeeded',
            'reply': 'retained reply',
          },
        ];
        api.receipts[original['operation_id'] as String] = receiptResult(
          original['operation_id'] as String,
          saved,
        );
        await c.refresh();
        expect(c.pending, isNull);
        expect(c.fresh, true);
        expect(c.details[sid]!.job.deleted, action == 'delete');
        expect(c.details[sid]!.turns.single.reply, 'retained reply');
        expect(c.state!.jobs.isEmpty, action == 'delete');
        expect(api.receiptQueries.toSet(), {original['operation_id']});
        expect(api.mutations.length, 1);
        expect(api.operations.skip(api.operations.length - 3), [
          'dev_session_receipt_v1',
          'dev_sessions_state_v1',
          'dev_session_get_v1',
        ]);
      },
    );
  }

  for (final mismatch in [
    'operation',
    'session',
    'project',
    'revision',
    'future_revision',
    'instance',
    'detail_revision',
    'detail_instance',
    'detail_job',
    'missing_row',
    'missing_turn',
    'malformed',
  ]) {
    test(
      'receipt recovery rejects $mismatch without releasing or replaying pending identity',
      () async {
        final api = Api()
          ..available = true
          ..rows = [job()];
        final c = await setup(api);
        await c.loadDetail(sid);
        api.failure = const AppFailure('TRANSPORT', 'lost', unknown: true);
        final admitted = <String>[];
        await c.act(
          sid,
          'prompt',
          prompt: 'exact prompt',
          onAdmitted: admitted.add,
        );
        final original = c.pending!;
        final operation = original['operation_id'] as String;
        api.revision = 2;
        api.savedTurns = [
          {
            'operation_id': operation,
            'prompt': 'exact prompt',
            'outcome': 'pending',
            'reply': null,
          },
        ];
        var proof = receiptResult(operation, job());
        if (mismatch == 'operation') {
          proof = receiptResult(fixture.requestId, job());
        }
        if (mismatch == 'session') {
          proof = receiptResult(operation, {
            ...job(),
            'session_id': fixture.requestId,
          });
        }
        if (mismatch == 'project') {
          proof = receiptResult(operation, {
            ...job(),
            'project_id': fixture.requestId,
          });
        }
        if (mismatch == 'revision') {
          proof = receiptResult(operation, job(), revision: 1);
        }
        if (mismatch == 'future_revision') {
          proof = receiptResult(operation, job(), revision: 3);
        }
        if (mismatch == 'instance') {
          api.receiptOverride = AppReply(fixture.requestId, proof);
        }
        if (mismatch == 'malformed') proof = {...proof, 'receipt': null};
        if (mismatch == 'missing_turn') {
          api.savedTurns = [
            {
              'operation_id': fixture.requestId,
              'prompt': 'exact prompt',
              'outcome': 'pending',
              'reply': null,
            },
          ];
        }
        if (mismatch == 'missing_row') {
          api.tombstone = job();
          api.rows = [];
        }
        if (mismatch.startsWith('detail_')) {
          var detail = api.detail();
          if (mismatch == 'detail_revision') {
            detail = {...detail, 'revision': 3};
          }
          if (mismatch == 'detail_job') {
            detail = {...detail, 'job': job(status: 'failed')};
          }
          api.getGate = Completer<AppReply>()
            ..complete(
              AppReply(
                mismatch == 'detail_instance'
                    ? fixture.requestId
                    : fixture.instance,
                detail,
              ),
            );
        }
        api.receipts[operation] = proof;
        await c.refresh();
        expect(c.pending, same(original));
        expect(c.canMutate, false);
        expect(admitted, isEmpty);
        expect(api.mutations.length, 1);
      },
    );
  }

  test('receipt recovery across restart requires one consistent observing instance', () async {
    final api = Api()
      ..available = true
      ..rows = [job()];
    final c = await setup(api);
    api.failure = const AppFailure('TRANSPORT', 'lost', unknown: true);
    await c.act(sid, 'finish');
    final operation = c.pending!['operation_id'] as String;
    api.serviceInstance = fixture.requestId;
    api.revision = 2;
    api.rows = [job(status: 'finished', retained: false)];
    api.receipts[operation] = receiptResult(operation, api.rows.single);
    await c.refresh();
    expect(c.pending, isNull);
    expect(c.instanceId, fixture.requestId);
    expect(api.mutations.length, 1);
  });

  test(
    'in-flight receipt queries coalesce and disposed recovery cannot publish',
    () async {
      final api = Api()
        ..available = true
        ..rows = [job()];
      final projects = ProjectsController(api);
      final c = RetainedJobsController(api, projects);
      await c.refresh();
      api.failure = const AppFailure('TRANSPORT', 'lost', unknown: true);
      await c.act(sid, 'finish');
      final original = c.pending!;
      final before = api.receiptQueries.length;
      api.receiptGate = Completer<AppReply>();
      final reading = c.refresh();
      await c.refresh();
      expect(api.receiptQueries.length, before + 1);
      c.dispose();
      api.receiptGate!.complete(
        AppReply(
          fixture.instance,
          receiptResult(original['operation_id'] as String, job()),
        ),
      );
      await reading;
      expect(c.pending, same(original));
      expect(api.mutations.length, 1);
      projects.dispose();
    },
  );
  for (final action in ['prompt', 'finish']) {
    test(
      'lost $action acknowledgement remains fenced after matching GET',
      () async {
        final api = Api()
          ..available = true
          ..rows = [job()];
        final c = await setup(api);
        await c.loadDetail(sid);
        api.failure = const AppFailure('TRANSPORT', 'lost', unknown: true);
        expect(await c.act(sid, action, prompt: 'exact followup'), false);
        final operation = c.pending!['operation_id'];
        await c.loadDetail(sid);
        await c.refresh();
        expect(c.pending!['operation_id'], operation);
        expect(await c.act(sid, action, prompt: 'exact followup'), false);
        expect(api.mutations.length, 1);
      },
    );
  }
  testWidgets('followup acknowledgement preserves newer typing', (
    tester,
  ) async {
    final api = Api()
      ..available = true
      ..rows = [job()];
    final c = await setup(api);
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(body: RetainedJobsView(controller: c)),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('smith · write'));
    await tester.pumpAndSettle();
    api.mutationGate = Completer<void>();
    await tester.enterText(find.byType(TextField), 'send this');
    await tester.pump();
    await tester.tap(find.text('Send'));
    await tester.pump();
    await tester.enterText(find.byType(TextField), 'newer draft');
    api.mutationGate!.complete();
    await tester.pumpAndSettle();
    expect(
      tester.widget<TextField>(find.byType(TextField)).controller!.text,
      'newer draft',
    );
    expect(api.mutations.single['prompt'], 'send this');
    await tester.tap(find.text('Close'));
    await tester.pumpAndSettle();
  });
  testWidgets('hidden origin retires only its review route', (tester) async {
    final api = Api()..rows = [job()];
    final c = await setup(api);
    final visible = ValueNotifier(true);
    addTearDown(visible.dispose);
    final key = GlobalKey<NavigatorState>();
    await tester.pumpWidget(
      MaterialApp(
        navigatorKey: key,
        home: Scaffold(
          body: ValueListenableBuilder<bool>(
            valueListenable: visible,
            builder: (_, enabled, _) => TickerMode(
              enabled: enabled,
              child: RetainedJobsView(controller: c),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('smith · write'));
    await tester.pumpAndSettle();
    unawaited(
      showDialog<void>(
        context: key.currentContext!,
        builder: (_) => const AlertDialog(title: Text('Other dialog')),
      ),
    );
    await tester.pumpAndSettle();
    visible.value = false;
    await tester.pumpAndSettle();
    expect(find.text('Other dialog'), findsOneWidget);
    expect(find.text('Retained job'), findsNothing);
    expect(api.mutations, isEmpty);
    key.currentState!.pop();
    await tester.pumpAndSettle();
  });
  test('strict state and typed detail parse exact backend shapes', () {
    final api = Api()..rows = [job()];
    expect(RetainedState.parse(api.state()).available, false);
    final d = RetainedDetail.parse(api.detail());
    expect(d.turns.single.operationId, fixture.requestId);
    expect(d.repositories.single.reference, 'main');
    expect(
      () => RetainedState.parse({...api.state(), 'receipt': {}}),
      throwsA(isA<AppFailure>()),
    );
    expect(
      () => RetainedJob.parse({...job(), 'status': 'completed'}),
      throwsA(isA<AppFailure>()),
    );
    expect(
      () => RetainedState.parse({...api.state(), 'revision': 9007199254740992}),
      throwsA(isA<AppFailure>()),
    );
    expect(retainedPromptValid('é' * 8192), true);
    expect(retainedPromptValid('é' * 8193), false);
  });
  test(
    'native framing accepts retained operations and validates correlation',
    () async {
      final api = Api()..rows = [job()];
      final native = NativeAppApi(executor: Executor(api));
      addTearDown(native.dispose);
      expect(
        (await native.call('dev_sessions_state_v1', {})).result['revision'],
        1,
      );
      expect(
        (await native.call('dev_session_get_v1', {
          'session_id': sid,
        })).result['turns'],
        isNotEmpty,
      );
      await expectLater(
        native.call('dev_session_get_v1', {'session_id': fixture.requestId}),
        throwsA(isA<AppFailure>()),
      );
      expect(api.mutations, isEmpty);
    },
  );
  test('unavailable has no local-model dependency and never admits', () async {
    final api = Api();
    final c = await setup(api);
    expect(c.canStart, false);
    expect(api.calls, ['projects_list_v3']);
    expect(
      () => c.captureStart(fixture.projectId, 'read', 'prompt'),
      throwsA(isA<AppFailure>()),
    );
    expect(api.mutations, isEmpty);
  });
  for (final type in ['read', 'write']) {
    test(
      'explicit $type start captures exact revision identity and prompt',
      () async {
        final api = Api()..available = true;
        final c = await setup(api);
        final args = c.captureStart(
          fixture.projectId,
          type,
          ' exact\n prompt ',
        );
        expect(args.keys.toSet(), {
          'operation_id',
          'instance_id',
          'expected_revision',
          'expected_projects_revision',
          'project_id',
          'job_type',
          'prompt',
        });
        expect(await c.start(args), true);
        expect(api.mutations.single, args);
        expect(c.pending, isNull);
        expect(await c.start(args), false);
      },
    );
  }
  test('stale project or service review never submits', () async {
    final api = Api()..available = true;
    final c = await setup(api);
    final args = c.captureStart(fixture.projectId, 'write', 'prompt');
    api.projectRevision++;
    await c.projects.refresh();
    expect(await c.start(args), false);
    final next = c.captureStart(fixture.projectId, 'read', 'prompt');
    api.serviceInstance = fixture.requestId;
    await c.refresh();
    expect(await c.start(next), false);
    expect(api.mutations, isEmpty);
  });
  test('unknown start retains original identity despite observed jobs and instance change', () async {
    final api = Api()
      ..available = true
      ..failure = const AppFailure('TRANSPORT', 'lost', unknown: true);
    final c = await setup(api);
    final args = c.captureStart(fixture.projectId, 'read', 'prompt');
    expect(await c.start(args), false);
    expect(c.pending, args);
    api.rows = [job()];
    await c.refresh();
    await c.loadDetail(sid);
    api.serviceInstance = fixture.requestId;
    await c.refresh();
    expect(c.pending!['operation_id'], args['operation_id']);
    expect(c.canStart, false);
    expect(await c.start(args), false);
    expect(api.mutations.length, 1);
  });
  test('foreign receipt stays uncertain', () async {
    final api = Api()
      ..available = true
      ..foreignReceipt = true;
    final c = await setup(api);
    final args = c.captureStart(fixture.projectId, 'write', 'prompt');
    expect(await c.start(args), false);
    expect(c.pending, args);
    expect(c.canStart, false);
  });
  test('known prewrite refusal releases fence only for fresh review', () async {
    final api = Api()
      ..available = true
      ..failure = const AppFailure('DEV_RUNTIME_UNAVAILABLE', 'not prepared');
    final c = await setup(api);
    expect(
      await c.start(c.captureStart(fixture.projectId, 'read', 'prompt')),
      false,
    );
    expect(c.pending, isNull);
    expect(api.mutations.length, 1);
  });
  test(
    'GET rejects foreign identity, stale revision, and late instance',
    () async {
      final api = Api()
        ..available = true
        ..rows = [job()];
      final c = await setup(api);
      api.getGate = Completer<AppReply>();
      final reading = c.loadDetail(sid);
      api.getGate!.complete(
        AppReply(fixture.instance, {
          ...api.detail(),
          'job': {...job(), 'project_id': fixture.requestId},
        }),
      );
      await reading;
      expect(c.details, isEmpty);
      api.getGate = Completer<AppReply>();
      final stale = c.loadDetail(sid);
      api.revision++;
      await c.refresh();
      api.getGate!.complete(AppReply(fixture.instance, api.detail()));
      await stale;
      expect(c.details, isEmpty);
      api.getGate = Completer<AppReply>();
      final late = c.loadDetail(sid);
      api.serviceInstance = fixture.requestId;
      await c.refresh();
      api.getGate!.complete(AppReply(fixture.instance, api.detail()));
      await late;
      expect(c.details, isEmpty);
    },
  );
  test('followup and finish/delete use receipts; unknown acknowledgement never replays', () async {
    final api = Api()
      ..available = true
      ..rows = [job()];
    final c = await setup(api);
    await c.loadDetail(sid);
    expect(c.canAct(sid, 'prompt'), true);
    expect(await c.act(sid, 'prompt', prompt: 'followup'), true);
    expect(api.mutations.single['prompt'], 'followup');
    expect(await c.act(sid, 'finish'), true);
    expect(api.mutations.last.containsKey('prompt'), false);
    api.failure = const AppFailure('DEV_UNAVAILABLE', 'storage');
    expect(await c.act(sid, 'delete'), false);
    final pending = c.pending;
    await c.refresh();
    expect(c.pending, pending);
    expect(await c.act(sid, 'delete'), false);
    expect(api.mutations.length, 3);
  });
  test('unavailable runtime allows only nonretained finish/delete', () async {
    final api = Api()..rows = [job()];
    final c = await setup(api);
    expect(c.canAct(sid, 'finish'), false);
    expect(c.canAct(sid, 'delete'), false);
    api.rows = [job(status: 'interrupted', retained: false)];
    api.revision++;
    await c.refresh();
    expect(c.canAct(sid, 'finish'), true);
    expect(await c.act(sid, 'finish'), true);
    expect(await c.act(sid, 'delete'), true);
    expect(c.state!.jobs, isEmpty);
  });
  testWidgets('unavailable UI is truthful; history is observation only', (
    tester,
  ) async {
    final api = Api()..rows = [job()];
    final c = await setup(api);
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(body: RetainedJobsView(controller: c)),
      ),
    );
    await tester.pumpAndSettle();
    expect(
      find.text('Read/write jobs unavailable: runtime not prepared.'),
      findsOneWidget,
    );
    expect(find.text('Retained jobs'), findsNothing);
    expect(find.text('Refresh retained jobs'), findsNothing);
    expect(find.text('Check acknowledgement'), findsNothing);
    expect(
      tester
          .widget<FilledButton>(
            find.widgetWithText(FilledButton, 'New read/write job'),
          )
          .onPressed,
      isNull,
    );
    await tester.tap(find.text('smith · write'));
    await tester.pumpAndSettle();
    expect(find.text('saved reply'), findsOneWidget);
    expect(find.text('original prompt'), findsOneWidget);
    expect(
      tester
          .widget<FilledButton>(find.widgetWithText(FilledButton, 'Send'))
          .onPressed,
      isNull,
    );
    await tester.tap(find.text('Close'));
    await tester.pumpAndSettle();
    expect(api.mutations, isEmpty);
  });
  testWidgets('new job review/start and Finish/Delete confirmations', (
    tester,
  ) async {
    final api = Api()..available = true;
    final c = await setup(api);
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(body: RetainedJobsView(controller: c)),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('New read/write job'));
    await tester.pumpAndSettle();
    await tester.tap(find.byType(DropdownButtonFormField<String>).first);
    await tester.pumpAndSettle();
    await tester.tap(find.text('smith').last);
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'prompt');
    await tester.pump();
    await tester.tap(find.text('Review'));
    await tester.pumpAndSettle();
    expect(api.mutations, isEmpty);
    await tester.tap(find.text('Start'));
    await tester.pumpAndSettle();
    expect(api.mutations.length, 1);
    await tester.tap(find.text('smith · read'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Finish'));
    await tester.pumpAndSettle();
    expect(api.mutations.length, 1);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(api.mutations.length, 1);
    await tester.tap(find.text('Close'));
    await tester.pumpAndSettle();
  });
  testWidgets('observation stops at bounded budget without mutation replay', (
    tester,
  ) async {
    final api = Api()
      ..available = true
      ..failure = const AppFailure('TRANSPORT', 'lost', unknown: true);
    final c = await setup(api);
    final args = c.captureStart(fixture.projectId, 'read', 'prompt');
    await c.start(args);
    for (var i = 0; i < 65; i++) {
      await tester.pump(const Duration(seconds: 2));
    }
    expect(c.stopped, true);
    expect(
      api.operations.where((op) => op == 'dev_sessions_state_v1').length,
      62,
    );
    expect(api.mutations.length, 1);
    expect(c.pending, args);
  });
}
