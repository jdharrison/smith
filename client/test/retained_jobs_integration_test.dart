import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/retained_jobs.dart';

import 'rust_cloud_integration_test.dart' show RustCloudFixture, bound;

const _fixtureEntry = 'dart_cloud::dart_retained_fixture';
const _reply = 'Synthetic retained follow-up\nUnicode ✓';

// A separately selected Rust TEST executable, never the installed service/helper.
Future<RustCloudFixture> _start(String executable) async {
  final root = await Directory.systemTemp.createTemp('smith-retained-dart-');
  final mode = await Process.run('/usr/bin/chmod', [
    '0700',
    root.path,
  ]).timeout(bound);
  expect(mode.exitCode, 0);
  final child = await Process.start(
    executable,
    ['--exact', _fixtureEntry, '--ignored', '--nocapture'],
    includeParentEnvironment: false,
    environment: {'SMITH_DART_CLOUD_ROOT': root.path},
  );
  final fixture = RustCloudFixture(root, child);
  final clock = Stopwatch()..start();
  while (!await File(fixture.socket).exists()) {
    if (clock.elapsed > const Duration(seconds: 10)) {
      await fixture.close();
      fail('Compiled retained fixture readiness deadline exceeded.');
    }
    await Future<void>.delayed(const Duration(milliseconds: 5));
  }
  return fixture;
}

class _Exchange implements AppExecutor {
  _Exchange(this.fixture);
  final RustCloudFixture fixture;
  final operations = <String>[];
  String? loseAcknowledgement;
  bool corruptState = false;

  @override
  Future<String> exchange(List<int> frame) async {
    final request = jsonDecode(utf8.decode(frame)) as Map<String, dynamic>;
    final op = request['op'] as String;
    operations.add(op);
    final response = await fixture.exchange(frame);
    if (op == loseAcknowledgement) {
      loseAcknowledgement = null;
      throw const AppFailure(
        'TRANSPORT',
        'Synthetic lost acknowledgement after compiled Rust publication.',
        unknown: true,
      );
    }
    if (op == 'dev_sessions_state_v1' && corruptState) {
      corruptState = false;
      final value = jsonDecode(response) as Map<String, dynamic>;
      (value['result'] as Map<String, dynamic>)['unexpected'] = true;
      return jsonEncode(value);
    }
    return response;
  }

  @override
  void dispose() {}
}

Future<RetainedDetail> _wait(
  NativeAppApi api,
  String session,
  String status,
) async {
  final clock = Stopwatch()..start();
  while (clock.elapsed < bound) {
    final detail = RetainedDetail.parse(
      (await api.call('dev_session_get_v1', {'session_id': session})).result,
    );
    if (detail.job.status == status) return detail;
    await Future<void>.delayed(const Duration(milliseconds: 5));
  }
  fail('Compiled retained fixture did not reach $status.');
}

Future<RetainedReceipt> _mutate(
  NativeAppApi api,
  String op,
  Map<String, dynamic> args, {
  String? operationId,
}) async {
  final id = operationId ?? appId();
  final clock = Stopwatch()..start();
  while (true) {
    final observed = await api.call('dev_sessions_state_v1', {});
    final state = RetainedState.parse(observed.result);
    try {
      return RetainedReceipt.parse(
        (await api.call(op, {
          ...args,
          'operation_id': id,
          'instance_id': observed.instanceId,
          'expected_revision': state.revision,
        })).result,
      );
    } on AppFailure catch (e) {
      // Retry only explicit non-admission, never ambiguous publication/transport.
      if (e.code != 'STALE_REVISION' || clock.elapsed >= bound) rethrow;
    }
  }
}

Future<RetainedReceipt> _receipt(NativeAppApi api, String operationId) async =>
    RetainedReceipt.parse(
      (await api.call('dev_session_receipt_v1', {
        'operation_id': operationId,
      })).result,
    );

Future<void> _noTools(RustCloudFixture fixture) async {
  final stats = await fixture.control('stats');
  expect(stats['local_calls'], 0);
  expect(stats['http_calls'], 0);
  expect(stats['key_reads'], 0);
  expect(stats['messages'], isEmpty);
  for (final name in ['dev-local-jobs.json', 'dev-jobs.json']) {
    expect(
      await File('${fixture.root.path}/data/smith-next/$name').exists(),
      false,
    );
  }
}

void main() {
  final executable =
      Platform.environment['SMITH_TEST_RETAINED_SERVICE'] ??
      Platform.environment['SMITH_TEST_CLOUD_SERVICE'];
  final required = Platform.environment.containsKey('SMITH_TEST_SERVICE');
  final skip = executable == null && !required
      ? 'Requires compiled debug_service test binary, not an installed service.'
      : false;

  test('compiled Rust App3 retained lifecycle, strict Dart parsers, FIFO and lost acknowledgements', () async {
    expect(executable, isNotNull);
    final fixture = await _start(executable!);
    addTearDown(fixture.close);
    final exchange = _Exchange(fixture);
    final api = NativeAppApi(executor: exchange);
    addTearDown(api.dispose);
    final empty = RetainedState.parse(
      (await api.call('dev_sessions_state_v1', {})).result,
    );
    expect(empty.available, true); // Explicit in-process fake only.
    expect(empty.reason, isNull);
    expect(empty.jobs, isEmpty);
    expect(empty.limit, 3);
    final projects = (await api.call('projects_list_v3', {})).result;
    Map<String, dynamic> repository(String name, bool readOnly) => {
      'name': name,
      'url': 'https://example.invalid/$name.git',
      'read_only': readOnly,
      'branch': {
        'base': 'main',
        'work': null,
        'types': {
          'feature': null,
          'fix': null,
          'release': null,
          'hotfix': null,
        },
      },
    };
    final project = (await api.call('project_create_v3', {
      'request_id': appId(),
      'instance_id': (await api.call('dev_sessions_state_v1', {})).instanceId,
      'expected_revision': projects['revision'],
      'name': 'Retained protocol fixture',
      'key': 'retained-protocol-fixture',
      'description': '',
      'prompt': ' Exact frozen project context\nUnicode ✓ ',
      'repositories': [
        repository('source', false),
        repository('reference', true),
      ],
      'agent': 'opencode',
    })).result;
    final starts = <RetainedReceipt>[];
    for (var i = 0; i < 5; i++) {
      final started = await _mutate(api, 'dev_session_start_v1', {
        'expected_projects_revision': project['revision'],
        'project_id': project['project_id'],
        'job_type': i.isEven ? 'write' : 'read',
        'prompt': ' Exact task $i\nUnicode ✓ ',
      });
      starts.add(started);
      expect(started.job.status, 'queued');
      await _wait(api, started.sessionId, i < 3 ? 'failed' : 'queued');
    }
    var state = RetainedState.parse(
      (await api.call('dev_sessions_state_v1', {})).result,
    );
    expect(state.occupied, 3);
    expect(state.queued, 2);
    expect(state.jobs.length, 5);
    final first = starts.first.sessionId;
    final initial = await _wait(api, first, 'failed');
    expect(initial.job.retained, true);
    expect(initial.turns.single.outcome, 'failed');
    expect(initial.turns.single.reply, isNull);
    expect(initial.repositories.map((r) => r.readOnly), [false, true]);
    final read = await _wait(api, starts[1].sessionId, 'failed');
    expect(read.repositories.every((r) => r.readOnly), true);

    final promptId = appId();
    exchange.loseAcknowledgement = 'dev_session_prompt_v1';
    await expectLater(
      _mutate(api, 'dev_session_prompt_v1', {
        'session_id': first,
        'prompt': 'Fix the known failure; same retained environment.',
      }, operationId: promptId),
      throwsA(isA<AppFailure>().having((e) => e.unknown, 'unknown', true)),
    );
    final followed = await _wait(api, first, 'ready');
    expect(followed.turns.length, 2);
    expect(followed.turns.first.outcome, 'failed');
    expect(followed.turns.last.operationId, promptId);
    expect(followed.turns.last.outcome, 'succeeded');
    expect(followed.turns.last.reply, _reply);
    expect(
      exchange.operations.where((op) => op == 'dev_session_prompt_v1').length,
      1,
    );
    final disk = File(
      '${fixture.root.path}/data/smith-next/dev-retained-jobs-v1.json',
    );
    final beforeQuery = await disk.readAsBytes();
    final recovered = await _receipt(api, promptId);
    expect(recovered.sessionId, first);
    expect(recovered.operationId, promptId);
    expect(recovered.job.status, 'ready');
    expect(await disk.readAsBytes(), beforeQuery);

    final finish = await _mutate(api, 'dev_session_finish_v1', {
      'session_id': first,
    });
    await _wait(api, first, 'finished');
    await _wait(api, starts[3].sessionId, 'failed');
    await _wait(api, starts[4].sessionId, 'queued');
    expect((await _receipt(api, finish.operationId)).job.retained, false);
    List<String> creates(Map<String, dynamic> stats) =>
        (stats['retained_commands'] as List)
            .cast<Map<String, dynamic>>()
            .where((c) => c['action'] == 'create')
            .map((c) => c['session_id'] as String)
            .toList();
    expect(
      creates(await fixture.control('stats')),
      starts.take(4).map((r) => r.sessionId).toList(),
    );
    await _mutate(api, 'dev_session_finish_v1', {
      'session_id': starts[1].sessionId,
    });
    await _wait(api, starts[1].sessionId, 'finished');
    await _wait(api, starts[4].sessionId, 'failed');
    expect(
      creates(await fixture.control('stats')),
      starts.map((r) => r.sessionId).toList(),
    );
    for (final started in starts.skip(2)) {
      await _mutate(api, 'dev_session_finish_v1', {
        'session_id': started.sessionId,
      });
      await _wait(api, started.sessionId, 'finished');
    }
    final deleteId = appId();
    exchange.loseAcknowledgement = 'dev_session_delete_v1';
    await expectLater(
      _mutate(api, 'dev_session_delete_v1', {
        'session_id': first,
      }, operationId: deleteId),
      throwsA(isA<AppFailure>().having((e) => e.unknown, 'unknown', true)),
    );
    final deleted = await _receipt(api, deleteId);
    expect(deleted.job.deleted, true);
    expect(deleted.job.retained, false);
    expect((await _receipt(api, starts.first.operationId)).job.deleted, true);
    expect((await _wait(api, first, 'finished')).job.deleted, true);
    state = RetainedState.parse(
      (await api.call('dev_sessions_state_v1', {})).result,
    );
    expect(state.jobs.any((j) => j.sessionId == first), false);
    expect(state.occupied, 0);
    expect(state.queued, 0);
    final stats = await fixture.control('stats');
    expect(stats['retained_peak'], 3);
    expect(stats['retained_live'], 0);
    expect((stats['retained_commands'] as List).length, 16);
    await _noTools(fixture);
  }, skip: skip);

  test('compiled retained fixture refuses malformed input and all production execution paths', () async {
    expect(executable, isNotNull);
    final fixture = await _start(executable!);
    addTearDown(fixture.close);
    final exchange = _Exchange(fixture);
    final api = NativeAppApi(executor: exchange);
    addTearDown(api.dispose);
    await api.call('dev_sessions_state_v1', {});
    final before = exchange.operations.length;
    await expectLater(
      api.call('dev_sessions_state_v1', {'auto_start': true}),
      throwsA(isA<AppFailure>().having((e) => e.code, 'code', 'PROTOCOL')),
    );
    expect(exchange.operations.length, before);
    exchange.corruptState = true;
    await expectLater(
      api.call('dev_sessions_state_v1', {}),
      throwsA(isA<AppFailure>().having((e) => e.code, 'code', 'PROTOCOL')),
    );
    Future<AppReply> raw(String op, Map<String, dynamic> args) async {
      final callId = appId();
      return AppReply.parse(
        await fixture.exchangeRaw(
          utf8.encode(
            '${jsonEncode({'schema_version': 3, 'service': 'smith-next', 'protocol': 'app', 'call_id': callId, 'op': op, 'args': args})}\n',
          ),
        ),
        callId,
        schemaVersion: 3,
      );
    }

    await expectLater(
      raw('dev_sessions_state_v1', {'auto_start': true}),
      throwsA(
        isA<AppFailure>().having((e) => e.code, 'code', 'INVALID_REQUEST'),
      ),
    );
    for (final op in [
      'dev_job_start_v4',
      'dev_job_start',
      'submit_routed',
      'provider_probe',
    ]) {
      await expectLater(
        raw(op, {}),
        throwsA(
          isA<AppFailure>().having(
            (e) => e.code,
            'code',
            'FIXTURE_OPERATION_DENIED',
          ),
        ),
      );
    }
    expect((await fixture.control('stats'))['retained_commands'], isEmpty);
    await _noTools(fixture);
  }, skip: skip);
}
