import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/dev_jobs.dart';
import 'package:smith_client/dev_jobs_controller.dart';
import 'package:smith_client/projects_controller.dart';
import 'package:smith_client/ui/dev_jobs_view.dart';

const instance = '11111111111111111111111111111111';
const projectId = '22222222222222222222222222222222';
const requestId = '33333333333333333333333333333333';
const configRevision = '18446744073709551615';
final digest = List.filled(64, 'a').join();
final commit = List.filled(40, 'b').join();
Map<String, dynamic> summary([
  String id = requestId,
  String status = 'completed',
]) => {
  'request_id': id,
  'project_id': projectId,
  'project_key': 'smith',
  'status': status,
  'message': 'Fixed service stage message.',
  'session_id': status == 'queued' ? null : 'ses_test',
  'created_ms': 1700000000000,
  'reply_available': status == 'completed' || status == 'cleanup_unknown',
  'cleanup_verified':
      status == 'queued' ||
      status == 'completed' ||
      status == 'failed' ||
      status == 'reconciled',
};
Map<String, dynamic> model({
  bool ready = true,
  String revision = configRevision,
}) => {
  'name': 'installed-local',
  'sha256': ready ? digest : null,
  'config_revision': revision,
  'ready': ready,
};
Map<String, dynamic> detail(Map<String, dynamic> job) => {
  'job': job,
  'model': {'name': 'installed-local', 'sha256': digest},
  'repositories': [
    {'url': 'https://example.org/owner/repo', 'ref': 'main', 'commit': commit},
  ],
  'reply': job['reply_available'] == true ? 'A saved final reply.' : null,
  'failed_stage': job['status'] == 'reconciled'
      ? 'running'
      : job['status'] == 'cleanup_unknown'
      ? 'cleanup'
      : null,
};
Map<String, dynamic> scheduler({
  int occupied = 0,
  int queued = 0,
  bool accepting = true,
}) => {
  'limit': 3,
  'occupied': occupied,
  'queued': queued,
  'accepting': accepting,
};

Map<String, dynamic> startArgs() => {
  'request_id': requestId,
  'instance_id': instance,
  'project_id': projectId,
  'expected_projects_revision': 1,
  'expected_config_revision': configRevision,
  'prompt': ' exact\n\t task ',
};

class JobsApi implements AppApi {
  final calls = <String>[];
  final gets = <String>[];
  final starts = <Map<String, dynamic>>[];
  final jobs = <Map<String, dynamic>>[];
  Map<String, dynamic> localModel = model();
  Map<String, dynamic>? schedulerOverride;
  Map<String, dynamic> get queueState =>
      schedulerOverride ??
      scheduler(
        occupied: jobs
            .where(
              (j) =>
                  DevJob.parse(j).executing || !DevJob.parse(j).cleanupVerified,
            )
            .length,
        queued: jobs.where((j) => j['status'] == 'queued').length,
        accepting: !jobs.any(
          (j) => !DevJob.parse(j).active && !DevJob.parse(j).cleanupVerified,
        ),
      );
  int projectRevision = 1;
  int? used;
  String serviceInstance = instance;
  AppFailure? startFailure;
  bool acceptBeforeFailure = false;
  bool missingGet = false;
  bool foreignGet = false;
  bool foreignStart = false;
  String startStatus = 'completed';
  Completer<void>? startGate;
  final deletedJobs = <String, Map<String, dynamic>>{};
  final deletes = <Map<String, dynamic>>[];
  final deletionQueries = <String>[];
  AppFailure? deleteFailure;
  bool deleteBeforeFailure = false;
  bool deletionResult = true;
  Completer<void>? deleteGate;
  AppReply? deletionReply;
  AppReply? deleteReply;

  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    calls.add(op);
    switch (op) {
      case 'dev_sessions_state_v1':
        expect(args, isEmpty);
        return AppReply(serviceInstance, {
          'revision': 0,
          'jobs': <Map<String, dynamic>>[],
          'capability': {
            'available': false,
            'reason': 'runtime_not_prepared',
            'job_types': ['read', 'write'],
          },
          'scheduler': {
            'limit': 3,
            'occupied': 0,
            'queued': 0,
            'accepting': false,
          },
        });
      case 'projects_list_v3':
        return AppReply(serviceInstance, {
          'revision': projectRevision,
          'projects': [
            {
              'project_id': projectId,
              'name': 'Smith',
              'key': 'smith',
              'description': '',
              'prompt': 'Exact project context',
              'repositories': [
                {
                  'name': 'repo',
                  'url': 'https://example.org/owner/repo',
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
                  'read_only': false,
                },
              ],
              'agent': 'opencode',
            },
          ],
        });
      case 'dev_jobs_state_v4':
        validateDevArguments(op, args);
        return AppReply(serviceInstance, {
          'jobs': jobs.toList(),
          'used': used ?? (jobs.length + deletedJobs.length),
          'model': localModel,
          'scheduler': queueState,
        });
      case 'dev_job_start_v4':
        validateDevArguments(op, args);
        if (deletedJobs.containsKey(args['request_id'])) {
          throw const AppFailure('JOB_DELETED', 'Deleted');
        }
        starts.add(Map.of(args));
        await startGate?.future;
        final job = summary(
          foreignStart ? projectId : args['request_id'],
          startStatus,
        );
        if (startFailure == null || acceptBeforeFailure) jobs.add(job);
        if (startFailure != null) throw startFailure!;
        return AppReply(serviceInstance, job);
      case 'dev_job_delete_v3':
        validateDevArguments(op, args);
        deletes.add(Map.of(args));
        await deleteGate?.future;
        if (args['instance_id'] != serviceInstance) {
          throw const AppFailure('STALE_INSTANCE', 'Changed');
        }
        final id = args['request_id'] as String;
        final job =
            deletedJobs[id] ??
            jobs.where((j) => j['request_id'] == id).firstOrNull;
        if (job == null) throw const AppFailure('REQUEST_UNKNOWN', 'Unknown');
        if (!DevJob.parse(job).canDelete) {
          throw const AppFailure('BUSY', 'Not deletable');
        }
        if (deleteFailure == null || deleteBeforeFailure) {
          deletedJobs[id] = job;
          jobs.removeWhere((j) => j['request_id'] == id);
        }
        if (deleteFailure != null) throw deleteFailure!;
        return deleteReply ??
            AppReply(serviceInstance, {'job': job, 'deleted': true});
      case 'dev_job_deletion_v3':
        validateDevArguments(op, args);
        deletionQueries.add(args['request_id']);
        if (deletionReply != null) return deletionReply!;
        final id = args['request_id'] as String;
        final job =
            deletedJobs[id] ??
            jobs.where((j) => j['request_id'] == id).firstOrNull;
        if (job == null) throw const AppFailure('REQUEST_UNKNOWN', 'Unknown');
        return AppReply(serviceInstance, {
          'job': job,
          'deleted': deletionResult && deletedJobs.containsKey(id),
        });
      case 'dev_job_get_v4':
        validateDevArguments(op, args);
        gets.add(args['request_id']);
        if (deletedJobs.containsKey(args['request_id'])) {
          throw const AppFailure('JOB_DELETED', 'Deleted');
        }
        if (missingGet) throw const AppFailure('REQUEST_UNKNOWN', 'Unknown');
        final found = jobs
            .where((j) => j['request_id'] == args['request_id'])
            .firstOrNull;
        if (found == null) throw const AppFailure('REQUEST_UNKNOWN', 'Unknown');
        return AppReply(
          serviceInstance,
          detail(foreignGet ? summary(projectId) : found),
        );
      default:
        throw StateError(
          'Unexpected operation $op (Agents, cloud and probes forbidden)',
        );
    }
  }

  @override
  void dispose() {}
}

class Executor implements AppExecutor {
  Map<String, dynamic>? sent;
  dynamic result;
  @override
  Future<String> exchange(List<int> frame) async {
    sent = jsonDecode(utf8.decode(frame));
    return jsonEncode({
      'schema_version': 3,
      'service': 'smith-next',
      'protocol': 'app',
      'call_id': sent!['call_id'],
      'pid': 1,
      'instance_id': instance,
      'ok': true,
      'result': result ?? detail(summary(sent!['args']['request_id'])),
      'error': null,
    });
  }

  @override
  void dispose() {}
}

Future<void> load(ProjectsController projects, DevJobsController jobs) =>
    Future.wait([projects.refresh(), jobs.refresh()]);

void main() {
  test('v4 queue state is exact and limits execution, not queued history', () {
    final rows = [
      for (var i = 1; i <= 3; i++)
        summary(i.toString().padLeft(32, '0'), 'running'),
      for (var i = 4; i <= 8; i++)
        summary(i.toString().padLeft(32, '0'), 'queued'),
      {...summary(), 'status': 'unknown', 'cleanup_verified': true},
    ];
    final state = {
      'jobs': rows,
      'used': rows.length,
      'model': model(),
      'scheduler': scheduler(occupied: 7, queued: 5),
    };
    final parsed = DevJobsState.parse(state);
    expect(parsed.jobs.where((job) => job.active), hasLength(8));
    expect(parsed.jobs.where((job) => job.executing), hasLength(3));
    expect(parsed.scheduler.occupied, 7);
    expect(parsed.scheduler.accepting, isTrue);
    expect(parsed.jobs.last.cleanupVerified, isTrue);
    expect(parsed.jobs.last.canDelete, isFalse);
    for (final invalid in [
      {...state}..remove('scheduler'),
      {...state, 'extra': true},
      {
        ...state,
        'jobs': [...rows, summary(projectId, 'preparing')],
        'used': 10,
      },
      for (final bad in [
        {...scheduler(), 'limit': 2},
        {...scheduler(), 'limit': 3.0},
        {...scheduler(), 'occupied': -1},
        {...scheduler(), 'occupied': 0.5},
        {...scheduler(), 'queued': -1},
        {...scheduler(), 'queued': 1.0},
        {...scheduler(), 'accepting': 1},
        {...scheduler(), 'position': 1},
        {...scheduler()}..remove('accepting'),
      ])
        {...state, 'scheduler': bad},
    ]) {
      expect(() => DevJobsState.parse(invalid), throwsA(isA<AppFailure>()));
    }
  });

  test('queued summary/start/detail require no session or reply and verified cleanup', () {
    final job = summary(requestId, 'queued');
    expect(DevJob.parse(job).active, isTrue);
    expect(DevJob.parse(job).executing, isFalse);
    expect(DevJob.parse(job).canDelete, isFalse);
    validateDevResult('dev_job_start_v4', startArgs(), job);
    validateDevResult('dev_job_get_v4', {'request_id': requestId}, detail(job));
    for (final invalid in [
      {...job, 'session_id': 'session'},
      {...job, 'reply_available': true},
      {...job, 'cleanup_verified': false},
    ]) {
      expect(() => DevJob.parse(invalid), throwsA(isA<AppFailure>()));
    }
    expect(
      () => correlateDevJob('dev_job_get_v4', {
        'request_id': projectId,
      }, detail(job)),
      throwsA(isA<DevJobCorrelationFailure>()),
    );
    expect(
      () => correlateDevJob('dev_job_start_v4', {
        ...startArgs(),
        'project_id': requestId,
      }, job),
      throwsA(isA<AppFailure>()),
    );
  });

  test('native allowlist admits v4 only; App3 framing is unchanged', () async {
    final executor = Executor();
    final api = NativeAppApi(executor: executor);
    addTearDown(api.dispose);
    for (final op in [
      'dev_jobs_state_v2',
      'dev_job_get_v2',
      'dev_job_start_v2',
    ]) {
      await expectLater(
        api.call(op, {}),
        throwsA(isA<AppFailure>().having((e) => e.code, 'code', 'OP')),
      );
      expect(executor.sent, isNull);
    }
    executor.result = summary(requestId, 'queued');
    await api.call('dev_job_start_v4', startArgs());
    expect(executor.sent!['schema_version'], 3);
    expect(executor.sent!['op'], 'dev_job_start_v4');
    executor.result = {
      'jobs': [],
      'used': 0,
      'model': model(),
      'scheduler': scheduler(),
    };
    await api.call('dev_jobs_state_v4', {});
    expect(executor.sent!['op'], 'dev_jobs_state_v4');
  });

  test('deletion schemas and safe terminals; no fixed history count cap', () {
    final rows = List.generate(
      65,
      (i) => summary(i.toRadixString(16).padLeft(32, '0')),
    );
    expect(
      DevJobsState.parse({
        'jobs': rows,
        'used': 100,
        'model': model(),
        'scheduler': scheduler(),
      }).jobs,
      hasLength(65),
    );
    for (final status in [
      'completed',
      'failed',
      'reconciled',
      'unknown',
      'running',
      'cleanup_unknown',
    ]) {
      final job = summary(requestId, status);
      expect(
        DevJob.parse(job).canDelete,
        ['completed', 'failed', 'reconciled'].contains(status),
      );
    }
    for (final op in ['dev_job_delete_v3', 'dev_job_deletion_v3']) {
      final args = {
        'request_id': requestId,
        if (op == 'dev_job_delete_v3') 'instance_id': instance,
      };
      validateDevResult(op, args, {'job': summary(), 'deleted': true});
      expect(
        () => validateDevResult(op, args, {
          'job': summary(projectId),
          'deleted': true,
        }),
        throwsA(isA<AppFailure>()),
      );
      expect(
        () => validateDevResult(op, args, {'job': summary(), 'deleted': 1}),
        throwsA(isA<AppFailure>()),
      );
    }
  });
  test(
    'reconciled preserves uncertain history and requires quiescence evidence',
    () {
      for (final hasReply in [false, true]) {
        final job = {
          ...summary(requestId, 'reconciled'),
          'reply_available': hasReply,
        };
        final parsed = DevJobDetail.parse(detail(job));
        expect(parsed.job.active, isFalse);
        expect(parsed.job.permitsNewAttempt, isTrue);
        expect(parsed.job.message, job['message']);
        expect(parsed.job.sessionId, job['session_id']);
        expect(parsed.failedStage, 'running');
        expect(parsed.reply, hasReply ? 'A saved final reply.' : null);
        expect(parsed.repositories.single.commit, commit);
        validateDevResult('dev_jobs_state_v4', {}, {
          'jobs': [job],
          'used': 1,
          'model': model(),
          'scheduler': scheduler(),
        });
        validateDevResult('dev_job_get_v4', {
          'request_id': requestId,
        }, detail(job));
      }
      final job = summary(requestId, 'reconciled');
      for (final invalid in [
        {...job, 'cleanup_verified': false},
        {...job, 'cleanup_verified': null},
        {...job, 'session_id': null},
        {...job, 'session_id': ''},
        {...job, 'session_id': '   '},
        {...job, 'original_status': 'unknown'},
      ]) {
        expect(() => DevJob.parse(invalid), throwsA(isA<AppFailure>()));
      }
      final result = detail(job);
      for (final invalid in [
        {...result, 'failed_stage': null},
        {...result, 'failed_stage': 'cleanup'},
        {...result, 'failed_stage': 'Running'},
        {...result, 'repositories': []},
        {...result, 'repositories': null},
        {
          ...result,
          'repositories': [
            {'url': 'https://example.org/repo', 'ref': 'main', 'commit': 'bad'},
          ],
        },
        {
          ...result,
          'repositories': [
            {'url': 'http://example.org/repo', 'ref': 'main', 'commit': commit},
          ],
        },
        {
          ...result,
          'repositories': [
            {'url': 'https://example.org/repo', 'ref': '', 'commit': commit},
          ],
        },
        {...result, 'reply': 'Contradicts reply_available'},
        {...result, 'original_status': 'unknown'},
      ]) {
        expect(() => DevJobDetail.parse(invalid), throwsA(isA<AppFailure>()));
        expect(
          () => validateDevResult('dev_job_get_v4', {
            'request_id': requestId,
          }, invalid),
          throwsA(isA<AppFailure>()),
        );
      }
    },
  );

  test('frozen pre-reconciled v2 status allowlist rejects the new value', () {
    // Compatibility fixture: this is the exact pre-extension status allowlist,
    // intentionally independent of the current reader.
    const legacyStatuses = {
      'preparing',
      'cloning',
      'ready',
      'running',
      'completed',
      'failed',
      'unknown',
      'cleanup_unknown',
    };
    for (final status in legacyStatuses) {
      expect(DevJob.parse(summary(requestId, status)).status, status);
    }
    final job = summary(requestId, 'reconciled');
    expect(legacyStatuses.contains(job['status']), isFalse);
    expect(DevJob.parse(job).status, 'reconciled');
  });

  test(
    'NativeAppApi validates reconciled state and detail responses',
    () async {
      final executor = Executor();
      final api = NativeAppApi(executor: executor);
      addTearDown(api.dispose);
      final job = summary(requestId, 'reconciled');
      executor.result = detail(job);
      await api.call('dev_job_get_v4', {'request_id': requestId});
      for (final invalid in [
        {...detail(job), 'failed_stage': null},
        {...detail(job), 'repositories': []},
        {
          ...detail(job),
          'job': {...job, 'cleanup_verified': false},
        },
        {
          ...detail(job),
          'job': {...job, 'session_id': null},
        },
      ]) {
        executor.result = invalid;
        await expectLater(
          api.call('dev_job_get_v4', {'request_id': requestId}),
          throwsA(isA<AppFailure>().having((e) => e.code, 'code', 'PROTOCOL')),
        );
      }
      executor.result = {
        'jobs': [job],
        'used': 1,
        'model': model(),
        'scheduler': scheduler(),
      };
      await api.call('dev_jobs_state_v4', {});
      executor.result = {
        'jobs': [
          {...job, 'cleanup_verified': false},
        ],
        'used': 1,
        'model': model(),
        'scheduler': scheduler(),
      };
      await expectLater(
        api.call('dev_jobs_state_v4', {}),
        throwsA(isA<AppFailure>()),
      );
    },
  );

  test('strict exact v2 summary, statuses, reply and cleanup invariants', () {
    expect(DevJob.parse(summary()).permitsNewAttempt, isTrue);
    for (final status in ['preparing', 'cloning', 'ready', 'running']) {
      expect(DevJob.parse(summary(requestId, status)).active, isTrue);
    }
    for (final invalid in [
      {...summary(), 'agent_id': 'developer'},
      {...summary(), 'prompt': 'private'},
      {...summary(), 'status': 'recorded'},
      {...summary(), 'status': 'interrupted'},
      {...summary(), 'reply_available': false},
      {...summary(), 'cleanup_verified': false},
      {...summary(), 'reply_available': 1},
      {...summary(), 'created_ms': 0},
    ]) {
      expect(() => DevJob.parse(invalid), throwsA(isA<AppFailure>()));
    }
    final uncertain = DevJob.parse(summary(requestId, 'cleanup_unknown'));
    expect(uncertain.replyAvailable, isTrue);
    expect(uncertain.permitsNewAttempt, isFalse);
    expect(
      DevJob.parse({...summary(requestId, 'failed'), 'cleanup_verified': false})
          .permitsNewAttempt,
      isFalse,
    );
  });

  test(
    'canonical u64 config revision stays a literal string at all boundaries',
    () {
      expect(devConfigRevision(configRevision), isTrue);
      expect(devConfigRevision('0'), isTrue);
      for (final invalid in [
        1,
        1.0,
        '01',
        '-1',
        '+1',
        ' 1',
        '1\n',
        '18446744073709551616',
      ]) {
        expect(devConfigRevision(invalid), isFalse);
        expect(
          () => validateDevArguments('dev_job_start_v4', {
            ...startArgs(),
            'expected_config_revision': invalid,
          }),
          throwsA(isA<AppFailure>()),
        );
      }
      validateDevArguments('dev_job_start_v4', startArgs());
      expect(
        () => validateDevArguments('dev_job_start_v4', {
          ...startArgs(),
          'agent_id': 'developer',
        }),
        throwsA(isA<AppFailure>()),
      );
      expect(
        () => validateDevArguments('dev_job_start', startArgs()),
        throwsA(isA<AppFailure>()),
      );
    },
  );

  test('strict state, capacity, model and detail schemas and correlation', () {
    final state = {
      'jobs': [summary()],
      'used': 2,
      'model': model(),
      'scheduler': scheduler(),
    };
    expect(DevJobsState.parse(state).used, 2);
    for (final invalid in [
      {...state, 'used': 0},
      {...state, 'used': 9007199254740992},
      {...state, 'model': null},
      {
        ...state,
        'model': {...model(), 'provider': 'cloud'},
      },
      {
        ...state,
        'model': {...model(), 'config_revision': 1},
      },
      {
        ...state,
        'model': {...model(), 'sha256': null},
      },
      {
        ...state,
        'jobs': [summary(), summary()],
      },
    ]) {
      expect(() => DevJobsState.parse(invalid), throwsA(isA<AppFailure>()));
    }
    final result = detail(summary());
    validateDevResult('dev_job_get_v4', {'request_id': requestId}, result);
    expect(
      () =>
          correlateDevJob('dev_job_get_v4', {'request_id': projectId}, result),
      throwsA(isA<AppFailure>()),
    );
    expect(
      () => correlateDevJob('dev_job_start_v4', {
        ...startArgs(),
        'project_id': requestId,
      }, summary()),
      throwsA(isA<AppFailure>()),
    );
    for (final invalid in [
      {...result, 'reply': null},
      {...result, 'reply': List.filled(65537, 'x').join()},
      {
        ...result,
        'repositories': [
          {'url': 'https://example.org/repo', 'ref': 'HEAD', 'commit': 'bad'},
        ],
      },
      {...result, 'model': model()},
      {...result, 'agent': 'developer'},
    ]) {
      expect(() => DevJobDetail.parse(invalid), throwsA(isA<AppFailure>()));
    }
  });

  test(
    'NativeAppApi freezes v2 get arguments without legacy fallback',
    () async {
      final executor = Executor();
      final api = NativeAppApi(executor: executor);
      final args = <String, dynamic>{'request_id': requestId};
      final result = api.call('dev_job_get_v4', args);
      args['request_id'] = projectId;
      await result;
      expect(executor.sent!['op'], 'dev_job_get_v4');
      expect(executor.sent!['args']['request_id'], requestId);
      api.dispose();
    },
  );

  for (final failure in [
    const AppFailure('TRANSPORT', 'Lost ack', unknown: true),
    const AppFailure('APP_UNAVAILABLE', 'Admission reply unavailable'),
  ]) {
    test('${failure.code}: shared guard, original identity, query-only Resolve', () async {
      final api = JobsApi()..startFailure = failure;
      final projects = ProjectsController(api);
      final jobs = DevJobsController(api, projects);
      addTearDown(() {
        jobs.dispose();
        projects.dispose();
      });
      await load(projects, jobs);
      expect(api.calls, ['projects_list_v3', 'dev_jobs_state_v4']);
      final args = jobs.capture(projectId, ' exact\n\t task ');
      expect(args['expected_config_revision'], configRevision);
      expect(() => args['prompt'] = 'changed', throwsUnsupportedError);
      expect(await jobs.start(args), isFalse);
      expect(jobs.pendingRequestId, args['request_id']);
      expect(jobs.canStart, isFalse);
      expect(await jobs.start(args), isFalse);
      await Future.wait([jobs.refresh(), jobs.refresh()]);
      await jobs.refresh();
      expect(api.starts, hasLength(1));
      expect(api.gets, [args['request_id'], args['request_id']]);
      expect(jobs.pendingRequestId, args['request_id']);
      // Once the original request becomes visible, Resolve still gets only it.
      api.jobs.add(summary(args['request_id']));
      final before = api.calls.length;
      await jobs.refresh();
      expect(api.calls.skip(before), ['dev_job_get_v4', 'dev_jobs_state_v4']);
      expect(jobs.current!.requestId, args['request_id']);
      expect(jobs.pendingRequestId, isNull);
      expect(jobs.detail!.reply, 'A saved final reply.');
      expect(api.starts, hasLength(1));
    });
  }

  test('APP_UNAVAILABLE accepted before ack preserves original detail and capacity', () async {
    final api = JobsApi()
      ..startFailure = const AppFailure('APP_UNAVAILABLE', 'Lost publication')
      ..acceptBeforeFailure = true;
    final projects = ProjectsController(api);
    final jobs = DevJobsController(api, projects);
    addTearDown(() {
      jobs.dispose();
      projects.dispose();
    });
    await load(projects, jobs);
    final args = jobs.capture(projectId, 'Task');
    await jobs.start(args);
    await jobs.refresh();
    expect(jobs.current!.requestId, args['request_id']);
    expect(jobs.pendingRequestId, isNull);
    expect(await jobs.start(args), isFalse); // Never replay an old capture.
    expect(api.starts, hasLength(1));
  });

  test('Resolve keeps original result and observes other active jobs without blocking admission', () async {
    final api = JobsApi()
      ..startFailure = const AppFailure(
        'APP_UNAVAILABLE',
        'Lost acknowledgement',
      )
      ..acceptBeforeFailure = true;
    final projects = ProjectsController(api);
    final jobs = DevJobsController(api, projects);
    addTearDown(() {
      jobs.dispose();
      projects.dispose();
    });
    await load(projects, jobs);
    final args = jobs.capture(projectId, 'Task');
    await jobs.start(args);
    api.jobs.add(summary(requestId, 'running'));
    await jobs.refresh();
    expect(api.gets, [args['request_id'], requestId]);
    expect(jobs.current!.requestId, args['request_id']);
    expect(jobs.canStart, isTrue);
  });

  test('foreign admission or detail never releases uncertainty', () async {
    final api = JobsApi()..foreignStart = true;
    final projects = ProjectsController(api);
    final jobs = DevJobsController(api, projects);
    addTearDown(() {
      jobs.dispose();
      projects.dispose();
    });
    await load(projects, jobs);
    final args = jobs.capture(projectId, 'Task');
    expect(await jobs.start(args), isFalse);
    expect(jobs.pendingRequestId, args['request_id']);
    api.jobs.add(summary(args['request_id']));
    api.foreignGet = true;
    await jobs.refresh();
    expect(jobs.pendingRequestId, args['request_id']);
    expect(jobs.canStart, isFalse);
  });

  test(
    'missing model/readiness, service mismatch, revision drift gate Start',
    () async {
      final api = JobsApi()..localModel = model(ready: false);
      final projects = ProjectsController(api);
      final jobs = DevJobsController(api, projects);
      addTearDown(() {
        jobs.dispose();
        projects.dispose();
      });
      await load(projects, jobs);
      expect(jobs.canStart, isFalse);
      api.localModel = model();
      await jobs.refresh();
      final args = jobs.capture(projectId, 'Task');
      api.localModel = model(revision: '2');
      await jobs.refresh();
      expect(await jobs.start(args), isFalse);
      final next = jobs.capture(projectId, 'Task');
      api.projectRevision = 2;
      await projects.refresh();
      expect(await jobs.start(next), isFalse);
      api.serviceInstance = requestId;
      await jobs.refresh();
      expect(jobs.canStart, isFalse);
      expect(api.starts, isEmpty);
    },
  );

  test(
    'missing previously accepted identity is a sticky protocol block',
    () async {
      final api = JobsApi()..jobs.add(summary());
      final projects = ProjectsController(api);
      final jobs = DevJobsController(api, projects);
      addTearDown(() {
        jobs.dispose();
        projects.dispose();
      });
      await load(projects, jobs);
      expect(jobs.canStart, isTrue);
      api.jobs.clear();
      await jobs.refresh();
      expect(jobs.current!.requestId, requestId);
      expect(jobs.protocolBlocked, isTrue);
      expect(jobs.error, contains('Previously accepted'));
      api.jobs.add(summary());
      await jobs.refresh();
      expect(jobs.canStart, isFalse);
    },
  );

  test('used includes legacy capacity without loading Agent records', () async {
    final api = JobsApi()..used = 32;
    final projects = ProjectsController(api);
    final jobs = DevJobsController(api, projects);
    addTearDown(() {
      jobs.dispose();
      projects.dispose();
    });
    await load(projects, jobs);
    expect(jobs.ledgerFull, isFalse);
    expect(jobs.canStart, isTrue);
  });

  testWidgets(
    'form has no Agent, picker or cloud switch; Enter/IME never submit',
    (tester) async {
      final api = JobsApi()..localModel = model(ready: false);
      final projects = ProjectsController(api);
      final jobs = DevJobsController(api, projects);
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(body: DevJobsView(controller: jobs)),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(FilledButton, 'New job'));
      await tester.pumpAndSettle();
      expect(find.text('Project'), findsOneWidget);
      expect(find.text('Saved agent role'), findsNothing);
      expect(find.byType(DropdownButtonFormField<String>), findsOneWidget);
      expect(find.byType(Switch), findsNothing);
      expect(find.textContaining('Local model not ready.'), findsOneWidget);
      await tester.enterText(find.byType(TextField), 'exact\nmultiline task');
      await tester.testTextInput.receiveAction(TextInputAction.newline);
      await tester.pump();
      expect(
        tester
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Review start'),
            )
            .onPressed,
        isNull,
      );
      expect(api.starts, isEmpty);
      expect(api.calls, [
        'projects_list_v3',
        'dev_jobs_state_v4',
        'dev_sessions_state_v1',
      ]);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      jobs.dispose();
      projects.dispose();
    },
  );

  testWidgets(
    'confirmation disclosures, newer typing retained, selectable reply and cleanup uncertainty',
    (tester) async {
      final api = JobsApi()
        ..startStatus = 'cleanup_unknown'
        ..startGate = Completer<void>();
      final projects = ProjectsController(api);
      final jobs = DevJobsController(api, projects);
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(body: DevJobsView(controller: jobs)),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(FilledButton, 'New job'));
      await tester.pumpAndSettle();
      await tester.tap(find.byType(DropdownButtonFormField<String>));
      await tester.pumpAndSettle();
      await tester.tap(find.text('smith').last);
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), ' exact\n task ');
      await tester.pump();
      expect(jobs.canStart, isTrue);
      expect(
        tester
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Review start'),
            )
            .onPressed,
        isNotNull,
      );
      expect(api.starts, isEmpty);
      await tester.ensureVisible(
        find.widgetWithText(FilledButton, 'Review start'),
      );
      await tester.tap(find.widgetWithText(FilledButton, 'Review start'));
      await tester.pumpAndSettle();
      expect(find.text('Start local Dev job?'), findsOneWidget);
      expect(find.textContaining('Checkout ref: main'), findsOneWidget);
      expect(find.text('Exact project context'), findsOneWidget);
      expect(find.textContaining('saved global instructions'), findsOneWidget);
      expect(
        find.textContaining('Closing the window detaches'),
        findsOneWidget,
      );
      await tester.tap(
        find.descendant(
          of: find.byType(AlertDialog),
          matching: find.widgetWithText(FilledButton, 'Start job'),
        ),
      );
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'newer typing');
      api.startGate!.complete();
      await tester.pumpAndSettle();
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text,
        'newer typing',
      );
      expect(api.starts.single['prompt'], ' exact\n task ');
      expect(jobs.current!.status, 'cleanup_unknown');
      expect(jobs.canStart, isFalse);
      await tester.tap(find.widgetWithText(TextButton, 'Close'));
      await tester.pumpAndSettle();
      await tester.tap(
        find.byKey(ValueKey('dev-job-${api.starts.single['request_id']}')),
      );
      await tester.pumpAndSettle();
      expect(
        find.descendant(
          of: find.byType(AlertDialog),
          matching: find.text('Phase: cleanup_unknown'),
        ),
        findsOneWidget,
      );
      expect(find.text('A saved final reply.'), findsOneWidget);
      expect(
        find.byWidgetPredicate(
          (w) => w is SelectableText && w.data == 'A saved final reply.',
        ),
        findsOneWidget,
      );
      expect(find.textContaining('Cleanup not verified'), findsOneWidget);
      expect(find.textContaining('Commit: $commit'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      jobs.dispose();
      projects.dispose();
    },
  );

  testWidgets(
    'narrow enlarged form, IME, and cancelled confirmation retain exact draft',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(360, 740));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final api = JobsApi();
      final projects = ProjectsController(api);
      final jobs = DevJobsController(api, projects);
      await tester.pumpWidget(
        MaterialApp(
          builder: (context, child) => MediaQuery(
            data: MediaQuery.of(context)
                .copyWith(textScaler: const TextScaler.linear(1.5)),
            child: child!,
          ),
          home: Scaffold(body: DevJobsView(controller: jobs)),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(FilledButton, 'New job'));
      await tester.pumpAndSettle();
      await tester.tap(find.byType(DropdownButtonFormField<String>));
      await tester.pumpAndSettle();
      await tester.tap(find.text('smith').last);
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'exact task');
      tester.testTextInput.updateEditingValue(
        const TextEditingValue(
          text: 'exact task',
          selection: TextSelection.collapsed(offset: 10),
          composing: TextRange(start: 0, end: 5),
        ),
      );
      await tester.pump();
      expect(
        tester
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Review start'),
            )
            .onPressed,
        isNull,
      );
      tester.testTextInput.updateEditingValue(
        const TextEditingValue(
          text: 'exact task',
          selection: TextSelection.collapsed(offset: 10),
        ),
      );
      await tester.pump();
      await tester.ensureVisible(
        find.widgetWithText(FilledButton, 'Review start'),
      );
      await tester.tap(find.widgetWithText(FilledButton, 'Review start'));
      await tester.pumpAndSettle();
      expect(find.byType(AlertDialog), findsOneWidget);
      await tester.ensureVisible(find.widgetWithText(TextButton, 'Close'));
      await tester.tap(find.widgetWithText(TextButton, 'Close'));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(FilledButton, 'New job'));
      await tester.pumpAndSettle();
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text,
        'exact task',
      );
      expect(api.starts, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      jobs.dispose();
      projects.dispose();
    },
  );

  testWidgets(
    'hiding retained form invalidates open confirmation without dispatch',
    (tester) async {
      final api = JobsApi();
      final projects = ProjectsController(api);
      final jobs = DevJobsController(api, projects);
      final visible = ValueNotifier(true);
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: ValueListenableBuilder<bool>(
              valueListenable: visible,
              builder: (context, enabled, child) => ExcludeFocus(
                excluding: !enabled,
                child: TickerMode(enabled: enabled, child: child!),
              ),
              child: DevJobsView(controller: jobs),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(FilledButton, 'New job'));
      await tester.pumpAndSettle();
      await tester.tap(find.byType(DropdownButtonFormField<String>));
      await tester.pumpAndSettle();
      await tester.tap(find.text('smith').last);
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'retained task');
      await tester.pump();
      await tester.ensureVisible(
        find.widgetWithText(FilledButton, 'Review start'),
      );
      await tester.tap(find.widgetWithText(FilledButton, 'Review start'));
      await tester.pumpAndSettle();
      final obsoleteStart = tester
          .widget<FilledButton>(find.widgetWithText(FilledButton, 'Start job'))
          .onPressed!;
      visible.value = false;
      await tester.pumpAndSettle();
      expect(find.byType(AlertDialog), findsNothing);
      obsoleteStart();
      await tester.pumpAndSettle();
      expect(api.starts, isEmpty);
      visible.value = true;
      await tester.pump();
      await tester.tap(find.widgetWithText(FilledButton, 'New job'));
      await tester.pumpAndSettle();
      expect(find.widgetWithText(FilledButton, 'Start job'), findsNothing);
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text,
        'retained task',
      );
      await tester.pumpWidget(const SizedBox());
      visible.dispose();
      jobs.dispose();
      projects.dispose();
    },
  );

  testWidgets(
    'automatic observation is bounded to 60, manual Refresh remains read-only',
    (tester) async {
      final api = JobsApi()..jobs.add(summary(requestId, 'running'));
      final projects = ProjectsController(api);
      final jobs = DevJobsController(api, projects);
      await load(projects, jobs);
      for (var n = 0; n < 60; n++) {
        await tester.pump(const Duration(seconds: 2));
        await tester.pump();
      }
      expect(jobs.observationStopped, isTrue);
      final before = api.calls.length;
      await tester.pump(const Duration(seconds: 20));
      expect(api.calls.length, before);
      await jobs.refresh();
      expect(api.calls.skip(before), ['dev_jobs_state_v4', 'dev_job_get_v4']);
      expect(api.starts, isEmpty);
      jobs.dispose();
      projects.dispose();
    },
  );
}
