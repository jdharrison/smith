import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart' show debugPrint;
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/dev_jobs.dart';
import 'package:smith_client/dev_jobs_controller.dart';
import 'package:smith_client/projects_controller.dart';

import 'rust_cloud_integration_test.dart' show RustCloudFixture;

String _id(int n) => n.toRadixString(16).padLeft(32, '0');

// Historical evidence only: no model installation, checkout or Start is needed.
Map<String, dynamic> _record(int n, {bool unknown = false}) {
  final status = unknown
      ? 'unknown'
      : ['completed', 'failed', 'reconciled'][(n - 1) % 3];
  final hasReply = status != 'failed';
  const model = {
    'name': 'qwen3-14b:q4_k_m',
    'sha256':
        '500a8806e85ee9c83f3ae08420295592451379b4f8cf2d0f41c15dffeb6b81f0',
  };
  const url = 'https://example.com/team/synthetic.git';
  return {
    'summary': {
      'request_id': _id(n),
      'project_id': _id(1000),
      'project_key': 'synthetic',
      'status': status,
      'message': 'Reply saved and owned runtime shutdown verified',
      'session_id': 'synthetic_session_$n',
      'created_ms': 1700000000000 + n,
      'reply_available': hasReply,
      'cleanup_verified': true,
    },
    'fingerprint': 'a' * 64,
    'config_revision': '18446744073709551615',
    'projects_revision': 7,
    'job': {
      'request_id': _id(n),
      'prompt': ' Exact synthetic task $n\n\tUnicode 😀 ',
      'model': model,
      'repositories': [
        {'url': url, 'ref': 'main'},
      ],
    },
    'repositories': [
      {'url': url, 'ref': 'refs/heads/main', 'commit': 'b' * 40},
    ],
    'image_id': 'sha256:${'c' * 64}',
    'message_id': hasReply ? 'synthetic_message_$n' : null,
    'reply': hasReply ? ' Exact saved reply $n\n\tUnicode 😀 ' : null,
    'failed_stage': status == 'completed' ? null : 'running',
    'may_have_sent': hasReply,
    'cleanup_required': false,
  };
}

class _DevExchange implements AppExecutor {
  _DevExchange(this.fixture);
  final RustCloudFixture fixture;
  final operations = <String>[];
  final arguments = <Map<String, dynamic>>[];
  bool loseNextDelete = false;
  bool loseBeforeDelete = false;

  @override
  Future<String> exchange(List<int> frame) async {
    final request = jsonDecode(utf8.decode(frame)) as Map<String, dynamic>;
    final op = request['op'] as String;
    // The external fixture owns a production runtime: never dispatch Start.
    if (op.startsWith('dev_job_start')) {
      throw StateError(
        'Dev Start is forbidden in the historical-ledger fixture.',
      );
    }
    operations.add(op);
    arguments.add(Map<String, dynamic>.from(request['args'] as Map));
    if (op == 'dev_job_delete_v3' && loseBeforeDelete) {
      loseBeforeDelete = false;
      throw const AppFailure(
        'TRANSPORT',
        'Synthetic transport loss before Rust deletion dispatch.',
        unknown: true,
      );
    }
    final response = await fixture.exchange(frame);
    if (op == 'dev_job_delete_v3' && loseNextDelete) {
      loseNextDelete = false;
      throw const AppFailure(
        'TRANSPORT',
        'Synthetic lost acknowledgement after Rust deletion publication.',
        unknown: true,
      );
    }
    return response;
  }

  @override
  void dispose() {}
}

class _Harness {
  _Harness(this.exchange, this.api, this.disk, this.original);
  final _DevExchange exchange;
  final NativeAppApi api;
  final File disk;
  final Map<String, dynamic> original;

  Future<DevJobsController> controller() async {
    final projects = ProjectsController(api);
    addTearDown(projects.dispose);
    await projects.refresh();
    expect(projects.error, isNull);
    final jobs = DevJobsController(api, projects);
    addTearDown(jobs.dispose);
    await jobs.refresh();
    expect(jobs.error, isNull);
    expect(jobs.loaded, true);
    return jobs;
  }

  Future<void> noEffects() async {
    final stats = await exchange.fixture.control('stats');
    expect(stats['local_calls'], 0);
    expect(stats['http_calls'], 0);
    expect(stats['key_reads'], 0);
    expect(stats['messages'], isEmpty);
    expect(exchange.fixture.submissions, isEmpty);
    expect(
      exchange.operations.every(
        (op) => const {
          'route_catalog',
          'projects_list_v3',
          'dev_jobs_state_v4',
          'dev_job_get_v4',

          'dev_job_delete_v3',
          'dev_job_deletion_v3',
        }.contains(op),
      ),
      true,
    );
    expect(
      exchange.operations.any((op) => op.startsWith('dev_job_start')),
      false,
    );
    // Bounded, synthetic counters only; no fixture root or provider data.
    debugPrint('Dev compiled effects: ${jsonEncode(stats)}');
  }
}

void main() {
  final executable = Platform.environment['SMITH_TEST_CLOUD_SERVICE'];
  final required = Platform.environment.containsKey('SMITH_TEST_SERVICE');
  final skipFixture = executable == null && !required
      ? 'Run make client/test for the mandatory compiled Dev jobs fixture.'
      : false;

  Future<_Harness> setup({bool unknown = false, int schemaVersion = 4}) async {
    expect(executable, isNotNull);
    final fixture = await RustCloudFixture.start(executable!, savedOnly: true);
    addTearDown(fixture.close);
    final exchange = _DevExchange(fixture);
    final api = NativeAppApi(executor: exchange);
    addTearDown(api.dispose);
    await api.call('route_catalog', {});
    final disk = File(
      '${fixture.root.path}/data/smith-next/dev-local-jobs.json',
    );
    expect(await disk.exists(), false);
    final original = <String, dynamic>{
      'schema_version': schemaVersion,
      'records': [
        for (var n = 1; n <= 40; n++) _record(n, unknown: unknown && n == 40),
      ],
    };
    // Only this fixture's private data is seeded. Reopen before any Dev read;
    // the default Docker adapter is constructed but never run by these calls.
    await disk.writeAsString(jsonEncode(original), flush: true);
    final mode = await Process.run('/usr/bin/chmod', ['0600', disk.path]);
    expect(mode.exitCode, 0);
    await fixture.control('restart');
    expect(jsonDecode(await disk.readAsString()), original);
    return _Harness(exchange, api, disk, original);
  }

  Future<void> fails(
    NativeAppApi api,
    String op,
    Map<String, dynamic> args,
    String code,
  ) => expectLater(
    api.call(op, args),
    throwsA(isA<AppFailure>().having((e) => e.code, 'code', code)),
  );

  test(
    'compiled App3 lists over 32 and durably deletes exact terminal evidence',
    () async {
      final h = await setup();
      final stateReply = await h.api.call('dev_jobs_state_v4', {});
      final state = DevJobsState.parse(stateReply.result);
      expect(state.jobs.length, 40);
      expect(state.used, 40);
      expect(state.scheduler.limit, 3);
      expect(state.scheduler.occupied, 0);
      expect(state.scheduler.queued, 0);
      expect(state.scheduler.accepting, false);
      expect(state.model.ready, false);
      expect(state.jobs.map((j) => j.requestId).toSet(), {
        for (var n = 1; n <= 40; n++) _id(n),
      });
      final initialBytes = await h.disk.readAsBytes();
      for (var n = 1; n <= 3; n++) {
        final args = {'request_id': _id(n)};
        final detail = DevJobDetail.parse(
          (await h.api.call('dev_job_get_v4', args)).result,
        );
        expect(detail.job.canDelete, true);
        expect(detail.reply, (h.original['records'] as List)[n - 1]['reply']);
        final before = DevJobDeletion.parse(
          (await h.api.call('dev_job_deletion_v3', args)).result,
        );
        expect(before.deleted, false);
        if (n == 1) expect(await h.disk.readAsBytes(), initialBytes);
        final deletion = await h.api.call('dev_job_delete_v3', {
          ...args,
          'instance_id': stateReply.instanceId,
        });
        final proof = DevJobDeletion.parse(deletion.result);
        expect(proof.deleted, true);
        expect(
          deletion.result['job'],
          (h.original['records'] as List)[n - 1]['summary'],
        );
        final saved = await h.disk.readAsBytes();
        expect(
          (await h.api.call('dev_job_deletion_v3', args)).result,
          deletion.result,
        );
        expect(
          (await h.api.call('dev_job_delete_v3', {
            ...args,
            'instance_id': stateReply.instanceId,
          })).result,
          deletion.result,
        );
        await fails(h.api, 'dev_job_get_v4', args, 'JOB_DELETED');

        expect(await h.disk.readAsBytes(), saved);
      }
      final expected = jsonDecode(jsonEncode(h.original));
      expected['schema_version'] = 4;
      for (var i = 0; i < 3; i++) {
        expected['records'][i]['deleted'] = true;
      }
      expect(jsonDecode(await h.disk.readAsString()), expected);
      final saved = await h.disk.readAsBytes();
      await h.exchange.fixture.control('restart');
      final reopened = await h.api.call('dev_jobs_state_v4', {});
      expect(reopened.instanceId, isNot(stateReply.instanceId));
      final remaining = DevJobsState.parse(reopened.result);
      expect(remaining.jobs.length, 37);
      expect(remaining.used, 40);
      expect(
        remaining.jobs.any((j) => [1, 2, 3].map(_id).contains(j.requestId)),
        false,
      );
      for (var n = 1; n <= 3; n++) {
        final proof = DevJobDeletion.parse(
          (await h.api.call('dev_job_deletion_v3', {
            'request_id': _id(n),
          })).result,
        );
        expect(proof.deleted, true);
        expect(proof.job.createdMs, 1700000000000 + n);
        await fails(h.api, 'dev_job_get_v4', {
          'request_id': _id(n),
        }, 'JOB_DELETED');
      }
      expect(await h.disk.readAsBytes(), saved);
      expect(jsonDecode(await h.disk.readAsString()), expected);
      await h.noEffects();
    },
    skip: skipFixture,
  );

  test('compiled deletion refuses unknown and wrong instance without clearing another fence', () async {
    final h = await setup(unknown: true);
    final reply = await h.api.call('dev_jobs_state_v4', {});
    final state = DevJobsState.parse(reply.result);
    expect(state.scheduler.occupied, 0);
    expect(state.scheduler.queued, 0);
    expect(
      state.jobs.firstWhere((job) => job.requestId == _id(40)).cleanupVerified,
      true,
    );
    final bytes = await h.disk.readAsBytes();
    await fails(h.api, 'dev_job_delete_v3', {
      'request_id': _id(1),
      'instance_id': _id(9999),
    }, 'STALE_INSTANCE');
    await fails(h.api, 'dev_job_delete_v3', {
      'request_id': _id(40),
      'instance_id': reply.instanceId,
    }, 'JOB_NOT_DELETABLE');
    for (final op in ['dev_job_get_v4', 'dev_job_deletion_v3']) {
      await fails(h.api, op, {'request_id': _id(9998)}, 'UNKNOWN_REQUEST');
    }
    await fails(h.api, 'dev_job_delete_v3', {
      'request_id': _id(9998),
      'instance_id': reply.instanceId,
    }, 'UNKNOWN_REQUEST');
    final negative = DevJobDeletion.parse(
      (await h.api.call('dev_job_deletion_v3', {'request_id': _id(40)})).result,
    );
    expect(negative.deleted, false);
    expect(negative.job.status, 'unknown');
    expect(await h.disk.readAsBytes(), bytes);
    final c = await h.controller();
    expect(c.canStart, false);
    expect(c.canDeleteJob(_id(40)), false);
    expect(c.canDeleteJob(_id(1)), true);
    await c.deleteJob(_id(1));
    expect(c.isDeleted(_id(1)), true);
    expect(c.pendingDeletionRequestId, isNull);
    expect(c.protocolBlocked, false);
    expect(
      c.jobs.any((j) => j.requestId == _id(40) && j.status == 'unknown'),
      true,
    );
    expect(c.canStart, false);
    final expected = jsonDecode(jsonEncode(h.original));
    expected['schema_version'] = 4;
    expected['records'][0]['deleted'] = true;
    expect(jsonDecode(await h.disk.readAsString()), expected);
    await h.noEffects();
  }, skip: skipFixture);

  test('compiled legacy ledger reads and deletion preserve schema 3', () async {
    final h = await setup(schemaVersion: 3);
    final before = await h.disk.readAsBytes();
    final reply = await h.api.call('dev_jobs_state_v4', {});
    final state = DevJobsState.parse(reply.result);
    expect(state.used, 40);
    expect(state.scheduler.limit, 3);
    expect(await h.disk.readAsBytes(), before);
    await h.api.call('dev_job_delete_v3', {
      'request_id': _id(1),
      'instance_id': reply.instanceId,
    });
    final expected = jsonDecode(jsonEncode(h.original));
    expected['records'][0]['deleted'] = true;
    expect(jsonDecode(await h.disk.readAsString()), expected);
    final saved = await h.disk.readAsBytes();
    await h.exchange.fixture.control('restart');
    final reopened = DevJobsState.parse(
      (await h.api.call('dev_jobs_state_v4', {})).result,
    );
    expect(reopened.jobs.length, 39);
    expect(reopened.used, 40);
    expect(await h.disk.readAsBytes(), saved);
    await h.noEffects();
  }, skip: skipFixture);

  test('v4 queue parser overlays compiled read-only state without starting work', () async {
    final h = await setup();
    final saved = await h.disk.readAsBytes();
    final reply = await h.api.call('dev_jobs_state_v4', {});
    final value = jsonDecode(jsonEncode(reply.result)) as Map<String, dynamic>;
    // Parser-only overlays never cross the fixture socket or enter its ledger.
    value['jobs'] = [
      for (var n = 1; n <= 4; n++)
        {
          ..._record(n)['summary'] as Map<String, dynamic>,
          'status': n == 4 ? 'queued' : 'running',
          'session_id': n == 4 ? null : 'synthetic_session_$n',
          'reply_available': false,
          'cleanup_verified': n == 4,
        },
    ];
    value['scheduler'] = {
      'limit': 3,
      'occupied': 3,
      'queued': 1,
      'accepting': true,
    };
    final state = DevJobsState.parse(value);
    expect(state.jobs.where((job) => job.executing).length, 3);
    expect(state.jobs.last.status, 'queued');
    expect(state.jobs.last.active, true);
    expect(state.jobs.last.executing, false);
    expect(state.jobs.last.canDelete, false);
    expect(state.scheduler.limit, 3);
    expect(state.scheduler.occupied, 3);
    expect(state.scheduler.queued, 1);
    expect(state.scheduler.accepting, true);
    final protocolFailure = throwsA(
      isA<AppFailure>().having((e) => e.code, 'code', 'PROTOCOL'),
    );
    value['scheduler']['limit'] = 4;
    expect(() => DevJobsState.parse(value), protocolFailure);
    value['scheduler']['limit'] = 3;
    value['jobs'][3]['status'] = 'running';
    expect(() => DevJobsState.parse(value), protocolFailure);
    value['jobs'][3]['status'] = 'queued';
    value['jobs'][3]['session_id'] = 'must_not_exist_while_queued';
    expect(() => DevJobsState.parse(value), protocolFailure);
    expect(await h.disk.readAsBytes(), saved);
    await h.noEffects();
  }, skip: skipFixture);

  test(
    'compiled negative deletion proof keeps uncertainty blocked without resend',
    () async {
      final h = await setup();
      final c = await h.controller();
      final saved = await h.disk.readAsBytes();
      h.exchange.loseBeforeDelete = true;
      await c.deleteJob(_id(1));
      expect(c.pendingDeletionRequestId, _id(1));
      final count = h.exchange.operations.length;
      await c.resolveDeletion();
      await c.refresh();
      await c.resolveDeletion();
      expect(c.pendingDeletionRequestId, _id(1));
      expect(c.isDeleted(_id(1)), false);
      expect(c.canDeleteJob(_id(2)), false);
      expect(c.canStart, false);
      expect(c.deletionError, contains('unknown'));
      expect(c.jobs.length, 40);
      expect(c.protocolBlocked, false);
      expect(
        h.exchange.operations.skip(count),
        isNot(contains('dev_job_delete_v3')),
      );
      expect(
        h.exchange.operations.skip(count),
        isNot(contains('dev_job_start_v4')),
      );
      expect(
        h.exchange.arguments
            .asMap()
            .entries
            .where((e) => h.exchange.operations[e.key] == 'dev_job_deletion_v3')
            .every((e) => e.value['request_id'] == _id(1)),
        true,
      );
      expect(await h.disk.readAsBytes(), saved);
      await h.noEffects();
    },
    skip: skipFixture,
  );

  test('compiled lost delete acknowledgement and second controller resolve by proof only', () async {
    final h = await setup();
    final first = await h.controller();
    final second = await h.controller();
    expect(first.jobs.length, 40);
    expect(first.ledgerFull, false);
    h.exchange.loseNextDelete = true;
    await first.deleteJob(_id(1));
    expect(first.pendingDeletionRequestId, _id(1));
    expect(first.deletionError, isNotNull);
    expect(first.canDeleteJob(_id(2)), false);
    expect(first.canStart, false);
    final saved = await h.disk.readAsBytes();
    final count = h.exchange.operations.length;
    await first.resolveDeletion();
    expect(
      h.exchange.operations.skip(count),
      isNot(contains('dev_job_delete_v3')),
    );
    expect(h.exchange.operations[count], 'dev_job_deletion_v3');
    expect(h.exchange.arguments[count], {'request_id': _id(1)});
    expect(first.isDeleted(_id(1)), true);
    expect(first.pendingDeletionRequestId, isNull);
    expect(first.deletionError, isNull);
    final secondCount = h.exchange.operations.length;
    await second.refresh();
    expect(second.error, isNull);
    expect(second.protocolBlocked, false);
    expect(second.isDeleted(_id(1)), true);
    expect(second.jobs.length, 39);
    expect(
      h.exchange.operations.skip(secondCount),
      contains('dev_job_deletion_v3'),
    );
    expect(
      h.exchange.operations.where((op) => op == 'dev_job_delete_v3').length,
      1,
    );
    expect(h.exchange.operations, isNot(contains('dev_job_start_v4')));
    expect(await h.disk.readAsBytes(), saved);
    await h.noEffects();
  }, skip: skipFixture);
}
