import 'dart:async';
import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/dev_jobs.dart';
import 'package:smith_client/dev_jobs_controller.dart';
import 'package:smith_client/projects_controller.dart';

import 'dev_jobs_test.dart' as fixtures;

const _a = '44444444444444444444444444444444';
const _b = '55555555555555555555555555555555';
const _otherProject = '66666666666666666666666666666666';
const _otherInstance = '77777777777777777777777777777777';

Map<String, dynamic> _job(
  String id, {
  String status = 'completed',
  int created = 1700000000000,
  String project = fixtures.projectId,
}) => {
  ...fixtures.summary(id, status),
  'created_ms': created,
  'project_id': project,
  'project_key': project == fixtures.projectId ? 'smith' : 'other-project',
};

AppReply _detailReply(
  Map<String, dynamic> job, {
  String instance = fixtures.instance,
  String? reply,
}) => AppReply(instance, {...fixtures.detail(job), 'reply': ?reply});

AppReply _stateReply(_ControlledApi api, {List<Map<String, dynamic>>? jobs}) =>
    AppReply(api.serviceInstance, {
      'jobs': jobs ?? api.jobs.toList(),
      'used': api.used ?? (jobs ?? api.jobs).length,
      'model': api.localModel,
      'scheduler': api.queueState,
    });

/// Unqueued calls keep the existing shared fixture's assumptions and traces.
class _ControlledApi extends fixtures.JobsApi {
  final _states = <Completer<AppReply>>[];
  final _gets = <String, List<Completer<AppReply>>>{};
  final _deletions = <String, List<Completer<AppReply>>>{};

  Completer<AppReply> holdDeletion(String id) {
    final gate = Completer<AppReply>();
    (_deletions[id] ??= []).add(gate);
    return gate;
  }

  Completer<AppReply> holdState() {
    final gate = Completer<AppReply>();
    _states.add(gate);
    return gate;
  }

  Completer<AppReply> holdGet(String id) {
    final gate = Completer<AppReply>();
    (_gets[id] ??= []).add(gate);
    return gate;
  }

  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) {
    if (op == 'dev_jobs_state_v4' && _states.isNotEmpty) {
      validateDevArguments(op, args);
      calls.add(op);
      return _states.removeAt(0).future;
    }
    final deletion = _deletions[args['request_id']];
    if (op == 'dev_job_deletion_v3' &&
        deletion != null &&
        deletion.isNotEmpty) {
      validateDevArguments(op, args);
      calls.add(op);
      deletionQueries.add(args['request_id']);
      return deletion.removeAt(0).future;
    }
    final queued = _gets[args['request_id']];
    if (op == 'dev_job_get_v4' && queued != null && queued.isNotEmpty) {
      validateDevArguments(op, args);
      calls.add(op);
      gets.add(args['request_id']);
      return queued.removeAt(0).future;
    }
    return super.call(op, args);
  }
}

/// Exercise NativeAppApi's real framing, parsing and result validation without
/// launching a helper or granting the fixture any live service capability.
class _JobsExecutor implements AppExecutor {
  _JobsExecutor(this.api);
  final fixtures.JobsApi api;

  @override
  Future<String> exchange(List<int> frame) async {
    final request = jsonDecode(utf8.decode(frame)) as Map<String, dynamic>;
    AppReply? reply;
    AppFailure? failure;
    try {
      reply = await api.call(
        request['op'] as String,
        request['args'] as Map<String, dynamic>,
      );
    } on AppFailure catch (e) {
      failure = e;
    }
    return jsonEncode({
      'schema_version': 3,
      'service': 'smith-next',
      'protocol': 'app',
      'call_id': request['call_id'],
      'pid': 1,
      'instance_id': reply?.instanceId ?? api.serviceInstance,
      'ok': failure == null,
      'result': reply?.result,
      'error': failure == null
          ? null
          : {'code': failure.code, 'message': failure.message},
    });
  }

  @override
  void dispose() {}
}

({ProjectsController projects, DevJobsController jobs}) _controllers(
  AppApi api,
) {
  final projects = ProjectsController(api);
  final jobs = DevJobsController(api, projects);
  addTearDown(() {
    jobs.dispose();
    projects.dispose();
  });
  return (projects: projects, jobs: jobs);
}

Future<void> _flush() => Future<void>.delayed(Duration.zero);

void main() {
  testWidgets(
    'v4 shared poll observes all executing and queued rows without a modal',
    (tester) async {
      final api = _ControlledApi()
        ..jobs.addAll([
          _job(fixtures.requestId, status: 'running'),
          _job(_a, status: 'cloning'),
          _job(_b, status: 'ready'),
          _job(_otherProject, status: 'queued'),
        ]);
      final c = _controllers(api);
      await fixtures.load(c.projects, c.jobs);
      expect(c.jobs.canStart, isTrue);
      expect(api.gets.toSet(), api.jobs.map((j) => j['request_id']).toSet());
      final before = api.gets.length;
      await tester.pump(const Duration(seconds: 2));
      await tester.pump();
      expect(
        api.gets.skip(before).toSet(),
        api.jobs.map((j) => j['request_id']).toSet(),
      );
      expect(api.gets.length - before, 4);
      api.jobs[3] = _job(_otherProject, status: 'failed');
      await tester.pump(const Duration(seconds: 2));
      await tester.pump();
      expect(c.jobs.detailState(_otherProject).data!.job.status, 'failed');
      expect(c.jobs.protocolBlocked, isFalse);
      expect(api.starts, isEmpty);
      for (var i = 0; i < api.jobs.length; i++) {
        api.jobs[i] = _job(api.jobs[i]['request_id'], status: 'failed');
      }
      await c.jobs.refresh();
    },
  );

  test('v4 accepts a queued acknowledgement while three jobs execute, never replays', () async {
    final api = _ControlledApi()
      ..startStatus = 'queued'
      ..jobs.addAll([
        _job(fixtures.requestId, status: 'running'),
        _job(_a, status: 'running'),
        _job(_b, status: 'running'),
      ]);
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    expect(c.jobs.canStart, isTrue);
    final args = c.jobs.capture(fixtures.projectId, 'Queue this exact task');
    expect(await c.jobs.start(args), isTrue);
    await c.jobs.refresh();
    expect(c.jobs.jobs.where((j) => j.status == 'queued'), hasLength(1));
    expect(c.jobs.jobs.where((j) => j.executing), hasLength(3));
    expect(c.jobs.canStart, isTrue);
    expect(await c.jobs.start(args), isFalse);
    expect(api.starts, hasLength(1));
  });

  test('v4 scheduler is authoritative independently of verified unknown history and occupancy', () async {
    final api = _ControlledApi()
      ..jobs.add({
        ..._job(fixtures.requestId, status: 'unknown'),
        'cleanup_verified': true,
      })
      ..schedulerOverride = fixtures.scheduler(occupied: 5, accepting: true);
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    expect(c.jobs.canStart, isTrue);
    expect(c.jobs.jobs.single.canDelete, isFalse);
    api.schedulerOverride = fixtures.scheduler(occupied: 0, accepting: false);
    await c.jobs.refresh();
    expect(c.jobs.canStart, isFalse);
    api.schedulerOverride = fixtures.scheduler();
    api.localModel = fixtures.model(ready: false);
    await c.jobs.refresh();
    expect(c.jobs.canStart, isFalse);
  });

  test('fix concurrent deletion proofs do not turn epoch changes into missing jobs', () async {
    final a = _job(_a);
    final b = _job(_b);
    final api = _ControlledApi()..jobs.addAll([a, b]);
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    api.jobs.clear();
    api.deletedJobs.addAll({_a: a, _b: b});
    final proofB = api.holdDeletion(_b);
    final detailB = c.jobs.loadDetail(_b, force: true);
    await _flush();
    final proofA = api.holdDeletion(_a);
    final refresh = c.jobs.refresh();
    await _flush();
    proofB.complete(AppReply(fixtures.instance, {'job': b, 'deleted': true}));
    await detailB;
    proofA.complete(AppReply(fixtures.instance, {'job': a, 'deleted': true}));
    await refresh;
    expect(c.jobs.isDeleted(_a), isTrue);
    expect(c.jobs.isDeleted(_b), isTrue);
    expect(c.jobs.protocolBlocked, isFalse);
    await c.jobs.refresh();
    expect(c.jobs.jobs, isEmpty);
    expect(c.jobs.canStart, isTrue);
    expect(api.deletes, isEmpty);
  });

  test(
    'fix native CAPACITY prewrite refusal needs fresh original-instance state',
    () async {
      final backing = _ControlledApi()
        ..jobs.add(_job(fixtures.requestId))
        ..deleteFailure = const AppFailure(
          'CAPACITY',
          'Prepublication capacity',
        );
      final api = NativeAppApi(executor: _JobsExecutor(backing));
      addTearDown(api.dispose);
      final c = _controllers(api);
      await fixtures.load(c.projects, c.jobs);
      final state = backing.holdState();
      final deletion = c.jobs.deleteJob(fixtures.requestId);
      await _flush();
      expect(c.jobs.canStart, isFalse);
      expect(c.jobs.canDeleteJob(fixtures.requestId), isFalse);
      state.complete(_stateReply(backing));
      await deletion;
      expect(c.jobs.pendingDeletionRequestId, isNull);
      expect(c.jobs.deletionError, contains('CAPACITY'));
      expect(c.jobs.canStart, isTrue);
      expect(c.jobs.canDeleteJob(fixtures.requestId), isTrue);
      await c.jobs.resolveDeletion();
      expect(backing.deletes, hasLength(1));
      expect(backing.deletionQueries, isEmpty);
    },
  );

  test('fix foreign-instance refusal cannot clear original pending deletion or unknown job', () async {
    final backing = _ControlledApi()
      ..jobs.addAll([_job(fixtures.requestId), _job(_a, status: 'unknown')])
      ..deleteGate = Completer<void>()
      ..deleteFailure = const AppFailure('CAPACITY', 'Prepublication capacity');
    final api = NativeAppApi(executor: _JobsExecutor(backing));
    addTearDown(api.dispose);
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    final deletion = c.jobs.deleteJob(fixtures.requestId);
    await _flush();
    backing.serviceInstance = _otherInstance;
    backing.deleteGate!.complete();
    await deletion;
    await c.jobs.resolveDeletion();
    expect(c.jobs.pendingDeletionRequestId, fixtures.requestId);
    expect(c.jobs.isDeleted(fixtures.requestId), isFalse);
    expect(
      c.jobs.jobs.any((j) => j.requestId == _a && j.status == 'unknown'),
      isTrue,
    );
    expect(c.jobs.canStart, isFalse);
    expect(backing.deletes, hasLength(1));
  });

  for (final invalid in ['false', 'identity', 'instance']) {
    test('fix concurrent epoch advance still rejects $invalid proof', () async {
      final a = _job(_a);
      final b = _job(_b);
      final api = _ControlledApi()..jobs.addAll([a, b]);
      final c = _controllers(api);
      await fixtures.load(c.projects, c.jobs);
      api.jobs.clear();
      api.deletedJobs.addAll({_a: a, _b: b});
      final proofB = api.holdDeletion(_b);
      final detailB = c.jobs.loadDetail(_b, force: true);
      await _flush();
      final proofA = api.holdDeletion(_a);
      final refresh = c.jobs.refresh();
      await _flush();
      proofB.complete(AppReply(fixtures.instance, {'job': b, 'deleted': true}));
      await detailB;
      proofA.complete(
        AppReply(invalid == 'instance' ? _otherInstance : fixtures.instance, {
          'job': {...a, if (invalid == 'identity') 'created_ms': 1700000000001},
          'deleted': invalid != 'false',
        }),
      );
      await refresh;
      expect(c.jobs.protocolBlocked, isTrue);
      expect(c.jobs.isDeleted(_a), isFalse);
      expect(c.jobs.isDeleted(_b), isTrue);
      expect(c.jobs.canStart, isFalse);
    });
  }

  test('fix prewrite refusal recovery survives unavailable state without clearing unknown job', () async {
    final api = _ControlledApi()
      ..jobs.addAll([_job(fixtures.requestId), _job(_a, status: 'unknown')])
      ..deleteFailure = const AppFailure('CAPACITY', 'Prepublication capacity');
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    final state = api.holdState();
    final deletion = c.jobs.deleteJob(fixtures.requestId);
    await _flush();
    state.completeError(
      const AppFailure('APP_UNAVAILABLE', 'State unavailable'),
    );
    await deletion;
    expect(c.jobs.pendingDeletionRequestId, fixtures.requestId);
    expect(c.jobs.canDeleteJob(fixtures.requestId), isFalse);
    await c.jobs.resolveDeletion();
    expect(c.jobs.pendingDeletionRequestId, isNull);
    expect(
      c.jobs.jobs.any((j) => j.requestId == _a && j.status == 'unknown'),
      isTrue,
    );
    expect(c.jobs.canStart, isFalse);
    expect(c.jobs.canDeleteJob(fixtures.requestId), isTrue);
    expect(api.deletes, hasLength(1));
    expect(api.deletionQueries, isEmpty);
  });

  for (final failure in [
    const AppFailure('CAPACITY', 'Ambiguous', unknown: true),
    const AppFailure('DEV_UNAVAILABLE', 'Publication may have happened'),
    const AppFailure('TRANSPORT', 'Lost ack', unknown: true),
  ]) {
    test(
      'fix ${failure.code} unknown=${failure.unknown} remains pending after false proof',
      () async {
        final api = _ControlledApi()
          ..jobs.add(_job(fixtures.requestId))
          ..deleteFailure = failure;
        final c = _controllers(api);
        await fixtures.load(c.projects, c.jobs);
        await c.jobs.deleteJob(fixtures.requestId);
        await c.jobs.resolveDeletion();
        expect(c.jobs.pendingDeletionRequestId, fixtures.requestId);
        expect(c.jobs.canStart, isFalse);
        expect(api.deletes, hasLength(1));
        expect(api.deletionQueries, [fixtures.requestId]);
      },
    );
  }
  test(
    'delete removes cached identity and never redispatches retained original',
    () async {
      final api = _ControlledApi()..jobs.add(_job(fixtures.requestId));
      final c = _controllers(api);
      await fixtures.load(c.projects, c.jobs);
      expect(c.jobs.canDeleteJob(fixtures.requestId), isTrue);
      await c.jobs.deleteJob(fixtures.requestId);
      expect(c.jobs.isDeleted(fixtures.requestId), isTrue);
      expect(c.jobs.jobs, isEmpty);
      expect(c.jobs.detailState(fixtures.requestId).data, isNull);
      expect(c.jobs.pendingDeletionRequestId, isNull);
      expect(c.jobs.canStart, isTrue);
      final calls = api.calls.length;
      await c.jobs.loadDetail(fixtures.requestId, force: true);
      await c.jobs.deleteJob(fixtures.requestId);
      expect(api.calls.length, calls);
      await expectLater(
        api.call('dev_job_start_v4', fixtures.startArgs()),
        throwsA(isA<AppFailure>().having((e) => e.code, 'code', 'JOB_DELETED')),
      );
      expect(api.starts, isEmpty);
    },
  );

  test(
    'more than 32 retained rows still admit a fresh reviewed start',
    () async {
      final api = _ControlledApi()
        ..jobs.addAll(
          List.generate(40, (i) => _job(i.toRadixString(16).padLeft(32, '0'))),
        );
      final c = _controllers(api);
      await fixtures.load(c.projects, c.jobs);
      expect(c.jobs.jobs, hasLength(40));
      expect(c.jobs.canStart, isTrue);
      final captured = c.jobs.capture(
        fixtures.projectId,
        'Fresh reviewed task',
      );
      expect(await c.jobs.start(captured), isTrue);
      await _flush();
      expect(api.starts, hasLength(1));
      expect(c.jobs.jobs, hasLength(41));
    },
  );

  test(
    'NativeAppApi dispatches both deletion operations with correlated proof',
    () async {
      final backing = fixtures.JobsApi()..jobs.add(_job(fixtures.requestId));
      final api = NativeAppApi(executor: _JobsExecutor(backing));
      addTearDown(api.dispose);
      final before = await api.call('dev_job_deletion_v3', {
        'request_id': fixtures.requestId,
      });
      expect(DevJobDeletion.parse(before.result).deleted, isFalse);
      final deleted = await api.call('dev_job_delete_v3', {
        'request_id': fixtures.requestId,
        'instance_id': fixtures.instance,
      });
      expect(DevJobDeletion.parse(deleted.result).deleted, isTrue);
      final after = await api.call('dev_job_deletion_v3', {
        'request_id': fixtures.requestId,
      });
      expect(
        DevJobDeletion.parse(after.result).job.requestId,
        fixtures.requestId,
      );
      expect(backing.calls, [
        'dev_job_deletion_v3',
        'dev_job_delete_v3',
        'dev_job_deletion_v3',
      ]);
    },
  );

  test(
    'successful delete proof stays pending until fresh same-instance omission',
    () async {
      final api = _ControlledApi()..jobs.add(_job(fixtures.requestId));
      final c = _controllers(api);
      await fixtures.load(c.projects, c.jobs);
      final state = api.holdState();
      final deletion = c.jobs.deleteJob(fixtures.requestId);
      await _flush();
      expect(c.jobs.isDeleted(fixtures.requestId), isTrue);
      expect(c.jobs.pendingDeletionRequestId, fixtures.requestId);
      expect(c.jobs.deletingRequestId, isNull);
      expect(c.jobs.canStart, isFalse);
      state.completeError(const AppFailure('APP_UNAVAILABLE', 'State lost'));
      await deletion;
      expect(c.jobs.pendingDeletionRequestId, fixtures.requestId);
      await c.jobs.resolveDeletion();
      expect(c.jobs.pendingDeletionRequestId, isNull);
      expect(api.deletes, hasLength(1));
    },
  );

  for (final mismatch in ['request', 'project', 'key', 'created', 'instance']) {
    test(
      'delete ack rejects $mismatch identity without throwing or replay',
      () async {
        final job = _job(fixtures.requestId);
        final api = _ControlledApi()
          ..jobs.add(job)
          ..deleteReply = AppReply(
            mismatch == 'instance' ? _otherInstance : fixtures.instance,
            {
              'job': {
                ...job,
                if (mismatch == 'request') 'request_id': _a,
                if (mismatch == 'project') 'project_id': _otherProject,
                if (mismatch == 'key') 'project_key': 'other',
                if (mismatch == 'created') 'created_ms': 1700000000001,
              },
              'deleted': true,
            },
          );
        final c = _controllers(api);
        await fixtures.load(c.projects, c.jobs);
        await c.jobs.deleteJob(fixtures.requestId);
        expect(c.jobs.isDeleted(fixtures.requestId), isFalse);
        expect(c.jobs.deletionError, isNotNull);
        expect(c.jobs.pendingDeletionRequestId, fixtures.requestId);
        expect(c.jobs.jobs.single.requestId, fixtures.requestId);
        expect(c.jobs.canStart, isFalse);
        await c.jobs.resolveDeletion();
        expect(c.jobs.isDeleted(fixtures.requestId), isTrue);
        expect(api.deletes, hasLength(1));
      },
    );
  }

  test(
    'bare JOB_DELETED detail error needs successful instance-bound proof',
    () async {
      final job = _job(fixtures.requestId);
      final api = _ControlledApi()..jobs.add(job);
      final c = _controllers(api);
      await fixtures.load(c.projects, c.jobs);
      final get = api.holdGet(fixtures.requestId);
      final read = c.jobs.loadDetail(fixtures.requestId, force: true);
      get.completeError(const AppFailure('JOB_DELETED', 'Deleted'));
      await read;
      expect(c.jobs.isDeleted(fixtures.requestId), isFalse);
      expect(c.jobs.protocolBlocked, isTrue);
      expect(api.deletionQueries, [fixtures.requestId]);
    },
  );

  test(
    'known GET deletion proof clears cached detail without resurrection',
    () async {
      final job = _job(fixtures.requestId);
      final api = _ControlledApi()..jobs.add(job);
      final c = _controllers(api);
      await fixtures.load(c.projects, c.jobs);
      api.jobs.clear();
      api.deletedJobs[fixtures.requestId] = job;
      await c.jobs.loadDetail(fixtures.requestId, force: true);
      expect(c.jobs.isDeleted(fixtures.requestId), isTrue);
      expect(c.jobs.detailState(fixtures.requestId).data, isNull);
      expect(c.jobs.jobs, isEmpty);
      expect(c.jobs.protocolBlocked, isFalse);
      await c.jobs.refresh();
      expect(c.jobs.canStart, isTrue);
    },
  );

  test(
    'deleting historical job never clears another unknown job blocker',
    () async {
      final api = _ControlledApi()
        ..jobs.addAll([_job(fixtures.requestId), _job(_a, status: 'unknown')]);
      final c = _controllers(api);
      await fixtures.load(c.projects, c.jobs);
      expect(c.jobs.canDeleteJob(fixtures.requestId), isTrue);
      expect(c.jobs.canDeleteJob(_a), isFalse);
      await c.jobs.deleteJob(fixtures.requestId);
      expect(c.jobs.isDeleted(fixtures.requestId), isTrue);
      expect(c.jobs.jobs.single.status, 'unknown');
      expect(c.jobs.canStart, isFalse);
      expect(c.jobs.pendingDeletionRequestId, isNull);
    },
  );

  test(
    'one proved disappearance cannot hide another missing nondeleted identity',
    () async {
      final deleted = _job(fixtures.requestId);
      final api = _ControlledApi()..jobs.addAll([deleted, _job(_a)]);
      final c = _controllers(api);
      await fixtures.load(c.projects, c.jobs);
      api.jobs.clear();
      api.deletedJobs[fixtures.requestId] = deleted;
      await c.jobs.refresh();
      expect(c.jobs.isDeleted(fixtures.requestId), isTrue);
      expect(c.jobs.isDeleted(_a), isFalse);
      expect(c.jobs.protocolBlocked, isTrue);
      expect(c.jobs.canStart, isFalse);
    },
  );

  test('unknown admission prevents delete even of safe history', () async {
    final api = _ControlledApi()
      ..jobs.add(_job(fixtures.requestId))
      ..startFailure = const AppFailure('APP_UNAVAILABLE', 'Lost start ack');
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    final args = c.jobs.capture(fixtures.projectId, 'Reviewed start');
    expect(await c.jobs.start(args), isFalse);
    expect(c.jobs.canDeleteJob(fixtures.requestId), isFalse);
    await c.jobs.deleteJob(fixtures.requestId);
    expect(api.deletes, isEmpty);
    expect(c.jobs.pendingRequestId, args['request_id']);
  });

  test('disposed late delete reply cannot mark identity deleted', () async {
    final api = _ControlledApi()
      ..jobs.add(_job(fixtures.requestId))
      ..deleteGate = Completer<void>();
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    final deletion = c.jobs.deleteJob(fixtures.requestId);
    c.jobs.dispose();
    api.deleteGate!.complete();
    await deletion;
    expect(c.jobs.isDeleted(fixtures.requestId), isFalse);
    expect(api.calls.where((op) => op == 'dev_jobs_state_v4'), hasLength(1));
  });

  test('foreign late delete reply remains unknown and resolve never follows new instance', () async {
    final api = _ControlledApi()
      ..jobs.add(_job(fixtures.requestId))
      ..deleteGate = Completer<void>();
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    final deletion = c.jobs.deleteJob(fixtures.requestId);
    api.serviceInstance = _otherInstance;
    api.deleteGate!.complete();
    await deletion;
    expect(c.jobs.pendingDeletionRequestId, fixtures.requestId);
    expect(c.jobs.isDeleted(fixtures.requestId), isFalse);
    await c.jobs.resolveDeletion();
    expect(c.jobs.isDeleted(fixtures.requestId), isFalse);
    expect(c.jobs.pendingDeletionRequestId, fixtures.requestId);
    expect(api.deletes, hasLength(1));
  });

  for (final accepted in [false, true]) {
    test('lost delete ack resolve queries only; accepted=$accepted', () async {
      final api = _ControlledApi()
        ..jobs.add(_job(fixtures.requestId))
        ..deleteFailure = const AppFailure('APP_UNAVAILABLE', 'Lost ack')
        ..deleteBeforeFailure = accepted;
      final c = _controllers(api);
      await fixtures.load(c.projects, c.jobs);
      await c.jobs.deleteJob(fixtures.requestId);
      expect(c.jobs.pendingDeletionRequestId, fixtures.requestId);
      expect(c.jobs.canStart, isFalse);
      await c.jobs.resolveDeletion();
      expect(api.deletes, hasLength(1));
      expect(api.deletionQueries, [fixtures.requestId]);
      expect(
        c.jobs.pendingDeletionRequestId,
        accepted ? isNull : fixtures.requestId,
      );
      expect(c.jobs.isDeleted(fixtures.requestId), accepted);
      if (!accepted) {
        final job = api.jobs.removeAt(0);
        api.deletedJobs[fixtures.requestId] = job;
        await c.jobs.resolveDeletion();
        expect(c.jobs.pendingDeletionRequestId, isNull);
        expect(api.deletes, hasLength(1));
      }
    });
  }

  for (final mismatch in [
    'request',
    'project',
    'key',
    'created',
    'instance',
    'false',
  ]) {
    test('cross-window missing row rejects $mismatch proof', () async {
      final job = _job(fixtures.requestId);
      final api = _ControlledApi()..jobs.add(job);
      final c = _controllers(api);
      await fixtures.load(c.projects, c.jobs);
      api.jobs.clear();
      api.deletionReply = AppReply(
        mismatch == 'instance' ? _otherInstance : fixtures.instance,
        {
          'job': {
            ...job,
            if (mismatch == 'request') 'request_id': _a,
            if (mismatch == 'project') 'project_id': _otherProject,
            if (mismatch == 'key') 'project_key': 'other',
            if (mismatch == 'created') 'created_ms': 1700000000001,
          },
          'deleted': mismatch != 'false',
        },
      );
      await c.jobs.refresh();
      expect(c.jobs.protocolBlocked, isTrue);
      expect(c.jobs.isDeleted(fixtures.requestId), isFalse);
      expect(c.jobs.jobs.single.requestId, fixtures.requestId);
      expect(c.jobs.canStart, isFalse);
    });
  }

  test('cross-window successful proof removes only deleted row; unknown stays blocking', () async {
    final job = _job(fixtures.requestId);
    final api = _ControlledApi()
      ..jobs.addAll([job, _job(_a, status: 'unknown')]);
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    expect(c.jobs.canDeleteJob(fixtures.requestId), isTrue);
    api.jobs.remove(job);
    api.deletedJobs[fixtures.requestId] = job;
    await c.jobs.refresh();
    expect(c.jobs.isDeleted(fixtures.requestId), isTrue);
    expect(c.jobs.jobs.single.requestId, _a);
    expect(c.jobs.canStart, isFalse);
    expect(c.jobs.protocolBlocked, isFalse);
  });

  test(
    'delete rejects late cached detail and pre-delete state resurrection',
    () async {
      final job = _job(fixtures.requestId);
      final api = _ControlledApi()..jobs.add(job);
      final c = _controllers(api);
      await fixtures.load(c.projects, c.jobs);
      final get = api.holdGet(fixtures.requestId);
      final detail = c.jobs.loadDetail(fixtures.requestId, force: true);
      await c.jobs.deleteJob(fixtures.requestId);
      get.complete(_detailReply(job));
      await detail;
      expect(c.jobs.detailState(fixtures.requestId).data, isNull);
      final state = api.holdState();
      final read = c.jobs.refresh();
      state.complete(_stateReply(api, jobs: [job]));
      await read;
      expect(c.jobs.jobs, isEmpty);
      expect(c.jobs.isDeleted(fixtures.requestId), isTrue);
      expect(c.jobs.protocolBlocked, isTrue);
    },
  );

  test('delete allowed without model readiness but not during start or second delete', () async {
    final api = _ControlledApi()
      ..jobs.add(_job(fixtures.requestId))
      ..localModel = fixtures.model(ready: false)
      ..deleteGate = Completer<void>();
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    expect(c.jobs.canDeleteJob(fixtures.requestId), isTrue);
    final deletion = c.jobs.deleteJob(fixtures.requestId);
    expect(c.jobs.deletingRequestId, fixtures.requestId);
    expect(c.jobs.canStart, isFalse);
    await c.jobs.deleteJob(fixtures.requestId);
    api.deleteGate!.complete();
    await deletion;
    expect(api.deletes, hasLength(1));
    expect(c.jobs.deletingRequestId, isNull);
  });
  for (final blocker in [
    'none',
    'active',
    'unknown',
    'cleanup_unknown',
    'unverified_failure',
    'capacity',
    'model',
    'metadata',
    'instance',
    'protocol',
    'stale',
  ]) {
    test(
      'reconciled terminal preserves other admission blocker: $blocker',
      () async {
        final api = _ControlledApi()
          ..jobs.add(_job(fixtures.requestId, status: 'reconciled'));
        if (blocker == 'active') api.jobs.add(_job(_a, status: 'running'));
        if (blocker == 'unknown' || blocker == 'cleanup_unknown') {
          api.jobs.add(_job(_a, status: blocker));
        }
        if (blocker == 'unverified_failure') {
          api.jobs.add({
            ..._job(_a, status: 'failed'),
            'cleanup_verified': false,
          });
        }
        if (blocker == 'capacity') api.used = 32;
        if (blocker == 'model') api.localModel = fixtures.model(ready: false);
        final c = _controllers(api);
        if (blocker == 'metadata') {
          await c.jobs.refresh();
        } else {
          await fixtures.load(c.projects, c.jobs);
        }
        if (blocker == 'instance') {
          api.serviceInstance = _otherInstance;
          await c.jobs.refresh();
        }
        if (blocker == 'protocol') {
          final job = api.jobs.removeAt(0);
          await c.jobs.refresh();
          api.jobs.add(job);
          await c.jobs.refresh();
          expect(c.jobs.protocolBlocked, isTrue);
        }
        if (blocker == 'stale') {
          final gate = api.holdState();
          final read = c.jobs.refresh();
          gate.completeError(
            const AppFailure('APP_UNAVAILABLE', 'Unavailable'),
          );
          await read;
        }
        expect(
          c.jobs.canStart,
          blocker == 'none' || blocker == 'capacity' || blocker == 'active',
        );
        if (blocker == 'none' || blocker == 'capacity' || blocker == 'active') {
          final next = c.jobs.capture(
            fixtures.projectId,
            'Fresh explicit review',
          );
          expect(next['request_id'], isNot(fixtures.requestId));
          await c.jobs.loadDetail(fixtures.requestId);
          expect(
            c.jobs.detailState(fixtures.requestId).data!.failedStage,
            'running',
          );
        } else {
          expect(
            () => c.jobs.capture(fixtures.projectId, 'Blocked'),
            throwsA(isA<AppFailure>()),
          );
        }
        expect(api.starts, isEmpty);
      },
    );
  }

  test('historical reconciliation cannot resolve another pending identity or replay it', () async {
    final api = _ControlledApi()..jobs.add(_job(_a, status: 'reconciled'));
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    api.startFailure = const AppFailure(
      'APP_UNAVAILABLE',
      'Lost acknowledgement',
    );
    final args = c.jobs.capture(fixtures.projectId, 'Explicit task');
    expect(await c.jobs.start(args), isFalse);
    await c.jobs.loadDetail(_a);
    expect(c.jobs.pendingRequestId, args['request_id']);
    expect(c.jobs.canStart, isFalse);
    final before = api.calls.length;
    await c.jobs.refresh();
    expect(api.calls.skip(before), ['dev_job_get_v4']);
    expect(api.gets.last, args['request_id']);
    expect(c.jobs.pendingRequestId, args['request_id']);
    api.jobs.add(_job(args['request_id'], status: 'reconciled'));
    await c.jobs.refresh();
    expect(c.jobs.pendingRequestId, isNull);
    expect(c.jobs.current!.requestId, args['request_id']);
    expect(c.jobs.current!.status, 'reconciled');
    expect(c.jobs.canStart, isTrue);
    expect(await c.jobs.start(args), isFalse);
    expect(api.starts, hasLength(1));
  });

  test('REVIEW-1 NativeAppApi preserves the client-only correlation failure subtype', () async {
    final server = _ControlledApi();
    final api = NativeAppApi(executor: _JobsExecutor(server));
    addTearDown(api.dispose);
    final gate = server.holdGet(_a);
    final observation = api.call('dev_job_get_v4', {'request_id': _a});
    final checked = expectLater(
      observation,
      throwsA(
        isA<DevJobCorrelationFailure>()
            .having((failure) => failure.code, 'unchanged code', 'PROTOCOL')
            .having((failure) => failure.unknown, 'unknown', isFalse),
      ),
    );
    gate.complete(_detailReply(_job(_b)));
    await checked;
    expect(server.gets, [_a]);
    expect(server.starts, isEmpty);
  });
  for (final path in ['browse', 'refresh']) {
    test(
      'REVIEW-1 NativeAppApi wrong-request $path GET causes a sticky protocol fence',
      () async {
        final server = _ControlledApi()
          ..jobs.addAll([
            _job(fixtures.requestId, created: 1700000000001),
            _job(_a),
          ]);
        final api = NativeAppApi(executor: _JobsExecutor(server));
        addTearDown(api.dispose);
        final c = _controllers(api);
        await fixtures.load(c.projects, c.jobs);
        final id = path == 'browse' ? _a : fixtures.requestId;
        if (path == 'browse') await c.jobs.loadDetail(id);
        final previous = c.jobs.detailState(id).data;
        final current = c.jobs.current;
        final detail = c.jobs.detail;
        final gate = server.holdGet(id);
        final observation = path == 'browse'
            ? c.jobs.loadDetail(id, force: true)
            : c.jobs.refresh();
        await _flush();
        gate.complete(_detailReply(_job(_b)));
        await observation;
        expect(c.jobs.protocolBlocked, isTrue);
        expect(c.jobs.canStart, isFalse);
        expect(c.jobs.detailState(id).error, contains('PROTOCOL'));
        expect(c.jobs.detailState(id).data, same(previous));
        expect(c.jobs.current, same(current));
        expect(c.jobs.detail, same(detail));
        await c.jobs.refresh();
        expect(c.jobs.protocolBlocked, isTrue);
        expect(c.jobs.canStart, isFalse);
        expect(
          () => c.jobs.capture(
            fixtures.projectId,
            'No admission after contradiction',
          ),
          throwsA(isA<AppFailure>()),
        );
        expect(server.starts, isEmpty);
      },
    );
  }

  test('REVIEW-1 NativeAppApi wrong-request pending GET retains the original guard and sticky fence', () async {
    final server = _ControlledApi()
      ..startFailure = const AppFailure(
        'APP_UNAVAILABLE',
        'Lost acknowledgement',
      )
      ..acceptBeforeFailure = true;
    final api = NativeAppApi(executor: _JobsExecutor(server));
    addTearDown(api.dispose);
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    final args = c.jobs.capture(fixtures.projectId, 'Task');
    expect(await c.jobs.start(args), isFalse);
    final gate = server.holdGet(args['request_id']);
    final before = server.calls.length;
    final resolve = c.jobs.refresh();
    await _flush();
    gate.complete(_detailReply(_job(_b)));
    await resolve;
    expect(server.calls.skip(before), ['dev_job_get_v4']);
    expect(c.jobs.protocolBlocked, isTrue);
    expect(c.jobs.pendingRequestId, args['request_id']);
    expect(c.jobs.jobs, isEmpty);
    expect(c.jobs.current, isNull);
    expect(c.jobs.canStart, isFalse);
    await c.jobs.refresh();
    expect(c.jobs.pendingRequestId, isNull);
    expect(c.jobs.current!.requestId, args['request_id']);
    expect(c.jobs.protocolBlocked, isTrue);
    expect(c.jobs.canStart, isFalse);
    expect(await c.jobs.start(args), isFalse);
    expect(server.starts, hasLength(1));
  });

  test('REVIEW-1 NativeAppApi generic malformed-detail PROTOCOL remains keyed, not a proven contradiction', () async {
    final server = _ControlledApi()
      ..jobs.addAll([
        _job(fixtures.requestId, created: 1700000000001),
        _job(_a),
      ]);
    final api = NativeAppApi(executor: _JobsExecutor(server));
    addTearDown(api.dispose);
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    final gate = server.holdGet(_a);
    final observation = c.jobs.loadDetail(_a);
    await _flush();
    gate.complete(
      AppReply(fixtures.instance, {
        ...fixtures.detail(_job(_a)),
        'unexpected': true,
      }),
    );
    await observation;
    expect(c.jobs.detailState(_a).error, contains('PROTOCOL'));
    expect(c.jobs.detailState(_a).data, isNull);
    expect(c.jobs.protocolBlocked, isFalse);
    expect(c.jobs.canStart, isTrue);
    expect(c.jobs.jobsStale, isFalse);
    expect(c.jobs.error, isNull);
    expect(server.starts, isEmpty);
  });

  for (final terminal in ['completed', 'failed', 'reconciled']) {
    testWidgets(
      'REVIEW-2 pending GET running then state $terminal admits from authoritative rows without a manual refresh',
      (tester) async {
        final api = _ControlledApi()
          ..startFailure = const AppFailure(
            'APP_UNAVAILABLE',
            'Lost acknowledgement',
          )
          ..acceptBeforeFailure = true;
        final c = _controllers(api);
        await fixtures.load(c.projects, c.jobs);
        final args = c.jobs.capture(fixtures.projectId, 'Task');
        expect(await c.jobs.start(args), isFalse);
        api.jobs[0] = _job(args['request_id'], status: terminal);
        final gate = api.holdGet(args['request_id']);
        final before = api.calls.length;
        final resolve = c.jobs.refresh();
        gate.complete(
          _detailReply(_job(args['request_id'], status: 'running')),
        );
        await resolve;
        expect(api.calls.skip(before), ['dev_job_get_v4', 'dev_jobs_state_v4']);
        expect(c.jobs.pendingRequestId, isNull);
        expect(c.jobs.jobs.single.status, terminal);
        expect(c.jobs.jobs.single.cleanupVerified, isTrue);
        expect(c.jobs.current!.status, 'running');
        expect(c.jobs.detail!.job.status, 'running');
        expect(c.jobs.detailState(args['request_id']).stale, isTrue);
        expect(c.jobs.protocolBlocked, isFalse);
        expect(c.jobs.jobsStale, isFalse);
        expect(c.jobs.canStart, isTrue);
        final next = c.jobs.capture(
          fixtures.projectId,
          'Fresh explicit review',
        );
        expect(next['request_id'], isNot(args['request_id']));
        final calls = api.calls.toList();
        await tester.pump(const Duration(seconds: 20));
        expect(api.calls, calls);
        expect(c.jobs.canStart, isTrue);
        expect(api.starts, hasLength(1));
      },
    );
  }

  test(
    'LIST-1 immutable sorted snapshots, legacy capacity and coalesced refresh',
    () async {
      final api = _ControlledApi()
        ..jobs.addAll([
          _job(_b),
          _job(fixtures.requestId, created: 1700000000001),
          _job(_a),
        ])
        ..used = 32;
      final c = _controllers(api);
      expect(c.jobs.jobs, isEmpty);
      expect(c.jobs.loaded, isFalse);
      expect(c.jobs.jobsStale, isTrue);
      final gate = api.holdState();
      final first = c.jobs.refresh();
      final second = c.jobs.refresh();
      expect(identical(first, second), isTrue);
      expect(c.jobs.refreshing, isTrue);
      gate.complete(_stateReply(api));
      await Future.wait([c.projects.refresh(), first, second]);
      expect(c.jobs.jobs.map((job) => job.requestId), [
        fixtures.requestId,
        _a,
        _b,
      ]);
      expect(() => c.jobs.jobs.clear(), throwsUnsupportedError);
      expect(c.jobs.ledgerFull, isFalse);
      expect(c.jobs.canStart, isTrue);
      expect(c.jobs.refreshing, isFalse);
      expect(c.jobs.jobsStale, isFalse);
      final before = c.jobs.jobs;
      api.jobs[0] = {...api.jobs[0], 'message': 'A newer service message.'};
      await c.jobs.refresh();
      expect(before.last.message, 'Fixed service stage message.');
      expect(c.jobs.jobs.last.message, 'A newer service message.');
      expect(api.calls.where((op) => op == 'dev_jobs_state_v4'), hasLength(2));
      expect(api.starts, isEmpty);
    },
  );

  test('LIST-1 failed refresh retains stale rows, never reports unavailable as empty', () async {
    final api = _ControlledApi()..jobs.add(_job(fixtures.requestId));
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    final before = c.jobs.jobs;
    final gate = api.holdState();
    final refresh = c.jobs.refresh();
    gate.completeError(const AppFailure('APP_UNAVAILABLE', 'Unavailable'));
    await refresh;
    expect(identical(c.jobs.jobs, before), isTrue);
    expect(c.jobs.jobsStale, isTrue);
    expect(c.jobs.loaded, isTrue);
    expect(c.jobs.canStart, isFalse);
    expect(c.jobs.error, contains('APP_UNAVAILABLE'));
    await c.jobs.loadDetail(fixtures.requestId, force: true);
    expect(c.jobs.jobsStale, isTrue);
    expect(c.jobs.canStart, isFalse);
    await c.jobs.refresh();
    expect(c.jobs.jobsStale, isFalse);
    expect(c.jobs.canStart, isTrue);
  });

  test('LIST-1 acknowledgement immediately upserts before its state read completes', () async {
    final api = _ControlledApi()..jobs.add(_job(_a, created: 1600000000000));
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    final original = c.jobs.jobs;
    final args = c.jobs.capture(fixtures.projectId, ' exact\n\t task ');
    final gate = api.holdState();
    expect(await c.jobs.start(args), isTrue);
    expect(c.jobs.jobs.map((job) => job.requestId), [args['request_id'], _a]);
    expect(original.map((job) => job.requestId), [_a]);
    expect(c.jobs.current!.requestId, args['request_id']);
    expect(c.jobs.refreshing, isTrue);
    expect(c.jobs.jobsStale, isTrue);
    expect(
      api.starts.single['expected_config_revision'],
      fixtures.configRevision,
    );
    expect(api.starts.single['prompt'], ' exact\n\t task ');
    expect(await c.jobs.start(args), isFalse);
    gate.complete(_stateReply(api));
    await c.jobs.refresh();
    expect(c.jobs.jobs, hasLength(2));
    expect(c.jobs.detail!.reply, 'A saved final reply.');
    expect(c.jobs.canStart, isTrue);
    expect(await c.jobs.start(args), isFalse);
    expect(api.starts, hasLength(1));
  });

  test(
    'LIST-3 unknown IDs are nonthrowing keyed errors with zero operations',
    () async {
      final api = _ControlledApi();
      final c = _controllers(api);
      await fixtures.load(c.projects, c.jobs);
      final before = api.calls.toList();
      final state = c.jobs.detailState(_a);
      expect(state.data, isNull);
      expect(state.loading, isFalse);
      expect(state.stale, isTrue);
      await c.jobs.loadDetail(_a);
      await c.jobs.loadDetail('invalid', force: true);
      expect(api.calls, before);
      expect(c.jobs.detailState(_a).error, contains('Unknown Dev job'));
      expect(c.jobs.detailState('invalid').error, isNotNull);
      expect(c.jobs.error, isNull);
      expect(c.jobs.protocolBlocked, isFalse);
      expect(c.jobs.jobsStale, isFalse);
      expect(c.jobs.canStart, isTrue);
      expect(state.error, isNull);
    },
  );

  test('LIST-3 per-ID coalescing, cache, force and late A/B keep selection operationally inert', () async {
    final api = _ControlledApi()
      ..jobs.addAll([
        _job(fixtures.requestId, created: 1700000000001),
        _job(_a, project: _otherProject),
        _job(_b),
      ]);
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    final current = c.jobs.current;
    final detail = c.jobs.detail;
    final rows = c.jobs.jobs;
    final a = api.holdGet(_a);
    final b = api.holdGet(_b);
    final first = c.jobs.loadDetail(_a);
    final duplicate = c.jobs.loadDetail(_a, force: true);
    expect(identical(first, duplicate), isTrue);
    final oldA = c.jobs.detailState(_a);
    final second = c.jobs.loadDetail(_b);
    b.complete(_detailReply(_job(_b), reply: 'B reply'));
    await second;
    expect(c.jobs.detailState(_b).data!.reply, 'B reply');
    expect(c.jobs.detailState(_a).loading, isTrue);
    expect(c.jobs.canStart, isTrue);
    a.complete(
      _detailReply(_job(_a, project: _otherProject), reply: 'A reply'),
    );
    await Future.wait([first, duplicate]);
    expect(c.jobs.detailState(_a).data!.reply, 'A reply');
    expect(c.jobs.detailState(_a).data!.job.projectId, _otherProject);
    expect(c.jobs.detailState(_b).data!.reply, 'B reply');
    expect(oldA.data, isNull);
    expect(oldA.loading, isTrue);
    final before = api.gets.length;
    await c.jobs.loadDetail(_a);
    expect(api.gets.length, before);
    await c.jobs.loadDetail(_a, force: true);
    expect(api.gets.length, before + 1);
    expect(identical(c.jobs.current, current), isTrue);
    expect(identical(c.jobs.detail, detail), isTrue);
    expect(identical(c.jobs.jobs, rows), isTrue);
    expect(c.jobs.pendingRequestId, isNull);
    expect(c.jobs.error, isNull);
    expect(c.jobs.jobsStale, isFalse);
    expect(c.jobs.canStart, isTrue);
    expect(api.starts, isEmpty);
  });

  test('LIST-3 detail failure preserves immutable data and stays keyed, not admission freshness', () async {
    final api = _ControlledApi()
      ..jobs.addAll([
        _job(fixtures.requestId, created: 1700000000001),
        _job(_a),
      ]);
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    await c.jobs.loadDetail(_a);
    final previous = c.jobs.detailState(_a);
    final current = c.jobs.current;
    final detail = c.jobs.detail;
    final gate = api.holdGet(_a);
    final load = c.jobs.loadDetail(_a, force: true);
    gate.completeError(const AppFailure('TRANSPORT', 'Read unavailable'));
    await load;
    expect(c.jobs.detailState(_a).data, same(previous.data));
    expect(c.jobs.detailState(_a).stale, isTrue);
    expect(c.jobs.detailState(_a).loading, isFalse);
    expect(c.jobs.detailState(_a).error, contains('TRANSPORT'));
    expect(previous.stale, isFalse);
    expect(previous.error, isNull);
    expect(c.jobs.error, isNull);
    expect(c.jobs.protocolBlocked, isFalse);
    expect(c.jobs.jobsStale, isFalse);
    expect(c.jobs.canStart, isTrue);
    expect(c.jobs.current, same(current));
    expect(c.jobs.detail, same(detail));
    await c.jobs.loadDetail(_a);
    expect(c.jobs.detailState(_a).stale, isFalse);
    expect(c.jobs.detailState(_a).error, isNull);
  });

  test('LIST-3 a historical GET cannot update rows or override scheduler admission', () async {
    final api = _ControlledApi()
      ..jobs.addAll([
        _job(fixtures.requestId, status: 'running'),
        _job(_a, created: 1600000000000),
      ]);
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    final current = c.jobs.current;
    final detail = c.jobs.detail;
    final rows = c.jobs.jobs;
    final gate = api.holdGet(fixtures.requestId);
    final load = c.jobs.loadDetail(fixtures.requestId, force: true);
    gate.complete(_detailReply(_job(fixtures.requestId)));
    await load;
    expect(
      c.jobs.detailState(fixtures.requestId).data!.job.status,
      'completed',
    );
    expect(c.jobs.current, same(current));
    expect(c.jobs.detail, same(detail));
    expect(c.jobs.jobs, same(rows));
    expect(c.jobs.jobs.first.status, 'running');
    expect(c.jobs.canStart, isTrue);
    expect(api.starts, isEmpty);
  });

  for (final field in [
    'request_id',
    'project_id',
    'project_key',
    'created_ms',
  ]) {
    test(
      'LIST-4 proven historical $field contradiction is a sticky fence',
      () async {
        final api = _ControlledApi()
          ..jobs.addAll([
            _job(fixtures.requestId, created: 1700000000001),
            _job(_a),
          ]);
        final c = _controllers(api);
        await fixtures.load(c.projects, c.jobs);
        final gate = api.holdGet(_a);
        final load = c.jobs.loadDetail(_a);
        final wrong = <String, dynamic>{
          ..._job(_a),
          field: switch (field) {
            'request_id' => _b,
            'project_id' => _otherProject,
            'project_key' => 'wrong-project',
            _ => 1600000000000,
          },
        };
        gate.complete(_detailReply(wrong));
        await load;
        expect(c.jobs.protocolBlocked, isTrue);
        expect(c.jobs.canStart, isFalse);
        expect(c.jobs.detailState(_a).error, contains('PROTOCOL'));
        expect(c.jobs.detailState(_a).data, isNull);
        await c.jobs.refresh();
        expect(c.jobs.protocolBlocked, isTrue);
        expect(c.jobs.canStart, isFalse);
      },
    );
  }

  for (final code in ['UNKNOWN_REQUEST', 'REQUEST_UNKNOWN']) {
    test(
      'LIST-4 $code for any known ID sticks even after authoritative reappearance',
      () async {
        final api = _ControlledApi()
          ..jobs.addAll([
            _job(fixtures.requestId, created: 1700000000001),
            _job(_a),
          ]);
        final c = _controllers(api);
        await fixtures.load(c.projects, c.jobs);
        final current = c.jobs.current;
        final gate = api.holdGet(_a);
        final load = c.jobs.loadDetail(_a);
        gate.completeError(AppFailure(code, 'Unknown'));
        await load;
        expect(c.jobs.protocolBlocked, isTrue);
        expect(c.jobs.current, same(current));
        expect(c.jobs.jobs.map((job) => job.requestId), [
          fixtures.requestId,
          _a,
        ]);
        expect(c.jobs.detailState(_a).error, contains(code));
        await c.jobs.refresh();
        expect(c.jobs.protocolBlocked, isTrue);
        expect(c.jobs.canStart, isFalse);
        expect(api.starts, isEmpty);
      },
    );
  }

  test('LIST-4 a non-current never-browsed identity cannot disappear from a verified list', () async {
    final api = _ControlledApi()
      ..jobs.addAll([
        _job(fixtures.requestId, created: 1700000000001),
        _job(_a),
      ]);
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    expect(api.gets, [fixtures.requestId]);
    final original = c.jobs.jobs;
    api.jobs.removeWhere((job) => job['request_id'] == _a);
    await c.jobs.refresh();
    expect(c.jobs.jobs, same(original));
    expect(c.jobs.jobsStale, isTrue);
    expect(c.jobs.protocolBlocked, isTrue);
    expect(c.jobs.error, contains('Previously accepted'));
    api.jobs.add(_job(_a));
    await c.jobs.refresh();
    expect(c.jobs.protocolBlocked, isTrue);
    expect(c.jobs.canStart, isFalse);
  });

  test('LIST-4 acknowledged non-current identity is retained even before its first successful refresh', () async {
    final api = _ControlledApi()..jobs.add(_job(_a, created: 1600000000000));
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    final args = c.jobs.capture(fixtures.projectId, 'Task');
    final gate = api.holdState();
    expect(await c.jobs.start(args), isTrue);
    gate.complete(_stateReply(api, jobs: [_job(_a, created: 1600000000000)]));
    await c.jobs.refresh();
    expect(c.jobs.jobs.map((job) => job.requestId), [args['request_id'], _a]);
    expect(c.jobs.protocolBlocked, isTrue);
    expect(c.jobs.pendingRequestId, isNull);
    expect(api.starts, hasLength(1));
  });

  test('LIST-2/LIST-4 original unknown GET precedes state and is never resent or fabricated', () async {
    final api = _ControlledApi()
      ..startFailure = const AppFailure(
        'APP_UNAVAILABLE',
        'Lost acknowledgement',
      );
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    final args = c.jobs.capture(fixtures.projectId, 'Task');
    expect(await c.jobs.start(args), isFalse);
    final before = api.calls.length;
    final gate = api.holdGet(args['request_id']);
    final first = c.jobs.refresh();
    final second = c.jobs.refresh();
    expect(identical(first, second), isTrue);
    gate.completeError(const AppFailure('UNKNOWN_REQUEST', 'Unknown'));
    await Future.wait([first, second]);
    expect(api.calls.skip(before), ['dev_job_get_v4']);
    expect(c.jobs.pendingRequestId, args['request_id']);
    expect(c.jobs.jobs, isEmpty);
    expect(c.jobs.protocolBlocked, isFalse);
    expect(c.jobs.canStart, isFalse);
    await c.jobs.loadDetail(args['request_id']);
    expect(api.calls.skip(before), ['dev_job_get_v4']);
    api.jobs.add(_job(args['request_id']));
    final resolvedBefore = api.calls.length;
    await c.jobs.refresh();
    expect(api.calls.skip(resolvedBefore), [
      'dev_job_get_v4',
      'dev_jobs_state_v4',
    ]);
    expect(c.jobs.pendingRequestId, isNull);
    expect(c.jobs.detail!.reply, 'A saved final reply.');
    expect(await c.jobs.start(args), isFalse);
    expect(api.starts, hasLength(1));
  });

  test('LIST-3 historical cross-project browsing while pending cannot resolve or mutate admission', () async {
    final api = _ControlledApi()
      ..jobs.addAll([
        _job(fixtures.requestId, created: 1700000000001),
        _job(_a, project: _otherProject),
      ])
      ..startFailure = const AppFailure('TRANSPORT', 'Lost ack', unknown: true);
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    final args = c.jobs.capture(fixtures.projectId, 'Task');
    await c.jobs.start(args);
    final current = c.jobs.current;
    final detail = c.jobs.detail;
    final error = c.jobs.error;
    final before = api.calls.length;
    await c.jobs.loadDetail(_a);
    expect(api.calls.skip(before), ['dev_job_get_v4']);
    expect(c.jobs.detailState(_a).data!.job.projectId, _otherProject);
    expect(c.jobs.detailState(_a).error, isNull);
    expect(c.jobs.current, same(current));
    expect(c.jobs.detail, same(detail));
    expect(c.jobs.error, error);
    expect(c.jobs.pendingRequestId, args['request_id']);
    expect(c.jobs.protocolBlocked, isFalse);
    expect(c.jobs.canStart, isFalse);
    expect(api.starts, hasLength(1));
  });

  test('LIST-3 pending recovery also reloads changed historical cache without selecting it', () async {
    final api = _ControlledApi()
      ..jobs.addAll([
        _job(_a, status: 'failed', created: 1600000000000),
        _job(fixtures.requestId),
      ])
      ..startFailure = const AppFailure(
        'APP_UNAVAILABLE',
        'Lost acknowledgement',
      )
      ..acceptBeforeFailure = true;
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    await c.jobs.loadDetail(_a);
    final previous = c.jobs.detailState(_a);
    final args = c.jobs.capture(fixtures.projectId, 'Task');
    await c.jobs.start(args);
    api.jobs[0] = {
      ...api.jobs[0],
      'message': 'Updated historical stage message.',
    };
    final before = api.gets.length;
    await c.jobs.refresh();
    expect(api.gets.skip(before), [args['request_id'], _a]);
    expect(c.jobs.pendingRequestId, isNull);
    expect(c.jobs.current!.requestId, args['request_id']);
    expect(c.jobs.detail!.job.requestId, args['request_id']);
    expect(
      c.jobs.detailState(_a).data!.job.message,
      'Updated historical stage message.',
    );
    expect(c.jobs.detailState(_a).stale, isFalse);
    expect(previous.data!.job.message, 'Fixed service stage message.');
    expect(c.jobs.canStart, isTrue);
    expect(api.starts, hasLength(1));
  });

  test(
    'LIST-4 pending GET is not retired when subsequent state is unavailable',
    () async {
      final api = _ControlledApi()
        ..jobs.add(_job(_a, created: 1600000000000))
        ..startFailure = const AppFailure(
          'APP_UNAVAILABLE',
          'Lost acknowledgement',
        )
        ..acceptBeforeFailure = true;
      final c = _controllers(api);
      await fixtures.load(c.projects, c.jobs);
      final current = c.jobs.current;
      final detail = c.jobs.detail;
      final args = c.jobs.capture(fixtures.projectId, 'Task');
      await c.jobs.start(args);
      final gate = api.holdState();
      final refresh = c.jobs.refresh();
      await _flush();
      gate.completeError(
        const AppFailure('APP_UNAVAILABLE', 'State unavailable'),
      );
      await refresh;
      expect(c.jobs.pendingRequestId, args['request_id']);
      expect(c.jobs.current, same(current));
      expect(c.jobs.detail, same(detail));
      expect(c.jobs.jobs.map((job) => job.requestId), [_a]);
      expect(c.jobs.canStart, isFalse);
      // Once GET has proved existence, even a later unknown cannot erase it.
      final unknown = api.holdGet(args['request_id']);
      final retry = c.jobs.refresh();
      unknown.completeError(const AppFailure('UNKNOWN_REQUEST', 'Unknown'));
      await retry;
      expect(c.jobs.protocolBlocked, isTrue);
      expect(c.jobs.pendingRequestId, args['request_id']);
      await c.jobs.refresh();
      expect(c.jobs.pendingRequestId, isNull);
      expect(c.jobs.canStart, isFalse);
      expect(api.starts, hasLength(1));
    },
  );

  test('LIST-4 pending recovery requires GET/state on one observed instance, not original uptime', () async {
    final api = _ControlledApi()
      ..startFailure = const AppFailure(
        'APP_UNAVAILABLE',
        'Lost acknowledgement',
      )
      ..acceptBeforeFailure = true;
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    final args = c.jobs.capture(fixtures.projectId, 'Task');
    await c.jobs.start(args);
    final gate = api.holdGet(args['request_id']);
    final refresh = c.jobs.refresh();
    gate.complete(
      _detailReply(_job(args['request_id']), instance: _otherInstance),
    );
    await refresh;
    expect(c.jobs.pendingRequestId, args['request_id']);
    expect(c.jobs.current, isNull);
    expect(c.jobs.jobs, isEmpty);
    expect(c.jobs.canStart, isFalse);
    expect(c.jobs.protocolBlocked, isFalse);
    api.serviceInstance = _otherInstance;
    await c.jobs.refresh();
    expect(c.jobs.pendingRequestId, isNull);
    expect(c.jobs.current!.requestId, args['request_id']);
    expect(c.jobs.canStart, isFalse); // Projects still belongs to old uptime.
    await c.projects.refresh();
    expect(c.jobs.canStart, isTrue);
    expect(api.starts, hasLength(1));
  });

  for (final mismatch in ['missing', 'project']) {
    test(
      'LIST-4 pending GET/state $mismatch contradiction retains the original guard',
      () async {
        final api = _ControlledApi()
          ..startFailure = const AppFailure(
            'APP_UNAVAILABLE',
            'Lost acknowledgement',
          )
          ..acceptBeforeFailure = true;
        final c = _controllers(api);
        await fixtures.load(c.projects, c.jobs);
        final args = c.jobs.capture(fixtures.projectId, 'Task');
        await c.jobs.start(args);
        final gate = api.holdState();
        final refresh = c.jobs.refresh();
        await _flush();
        gate.complete(
          _stateReply(
            api,
            jobs: [
              if (mismatch == 'project')
                _job(args['request_id'], project: _otherProject),
            ],
          ),
        );
        await refresh;
        expect(c.jobs.pendingRequestId, args['request_id']);
        expect(c.jobs.current, isNull);
        expect(c.jobs.jobs, isEmpty);
        expect(c.jobs.protocolBlocked, isTrue);
        expect(c.jobs.canStart, isFalse);
        expect(api.starts, hasLength(1));
      },
    );
  }

  test(
    'LIST-4 legitimate mutable phase races do not contradict pending identity',
    () async {
      final api = _ControlledApi()
        ..startFailure = const AppFailure(
          'APP_UNAVAILABLE',
          'Lost acknowledgement',
        )
        ..acceptBeforeFailure = true;
      final c = _controllers(api);
      await fixtures.load(c.projects, c.jobs);
      final args = c.jobs.capture(fixtures.projectId, 'Task');
      await c.jobs.start(args);
      api.jobs[0] = _job(args['request_id'], status: 'running');
      final gate = api.holdGet(args['request_id']);
      final refresh = c.jobs.refresh();
      gate.complete(_detailReply(_job(args['request_id'], status: 'ready')));
      await refresh;
      expect(c.jobs.pendingRequestId, isNull);
      expect(c.jobs.protocolBlocked, isFalse);
      expect(c.jobs.jobs.single.status, 'running');
      expect(c.jobs.current!.status, 'ready');
      expect(c.jobs.detail!.job.status, 'ready');
      expect(c.jobs.detailState(args['request_id']).stale, isTrue);
      expect(c.jobs.canStart, isTrue);
      await c.jobs.refresh();
      expect(
        c.jobs.detailState(args['request_id']).data!.job.status,
        'running',
      );
      expect(c.jobs.detailState(args['request_id']).stale, isFalse);
    },
  );

  testWidgets(
    'LIST-4 terminal resolved current does not suppress another active row polling',
    (tester) async {
      final api = _ControlledApi()
        ..startFailure = const AppFailure(
          'APP_UNAVAILABLE',
          'Lost acknowledgement',
        )
        ..acceptBeforeFailure = true;
      final c = _controllers(api);
      await fixtures.load(c.projects, c.jobs);
      final args = c.jobs.capture(fixtures.projectId, 'Task');
      await c.jobs.start(args);
      api.jobs.add(_job(_a, status: 'running', project: _otherProject));
      await c.jobs.refresh();
      expect(api.gets, [args['request_id'], _a]);
      expect(c.jobs.current!.requestId, args['request_id']);
      expect(c.jobs.current!.active, isFalse);
      expect(c.jobs.canStart, isTrue);
      await tester.pump(const Duration(seconds: 2));
      await tester.pump();
      expect(api.gets, [args['request_id'], _a, _a]);
      expect(c.jobs.detailState(_a).data!.job.status, 'running');
      api.jobs[1] = _job(_a, project: _otherProject);
      await tester.pump(const Duration(seconds: 2));
      await tester.pump();
      expect(c.jobs.jobs.every((job) => job.permitsNewAttempt), isTrue);
      expect(c.jobs.detailState(_a).data!.job.status, 'completed');
      expect(c.jobs.canStart, isTrue);
      expect(api.starts, hasLength(1));
    },
  );

  testWidgets(
    'LIST-1/LIST-4 later shared GET advances rows/cache and coalesces with browsing',
    (tester) async {
      final api = _ControlledApi()
        ..jobs.add(_job(fixtures.requestId, status: 'running'));
      final c = _controllers(api);
      final gate = api.holdGet(fixtures.requestId);
      final refresh = fixtures.load(c.projects, c.jobs);
      await tester.pump();
      expect(c.jobs.detailState(fixtures.requestId).loading, isTrue);
      final browse = c.jobs.loadDetail(fixtures.requestId, force: true);
      expect(api.gets, [fixtures.requestId]);
      gate.complete(_detailReply(_job(fixtures.requestId)));
      await Future.wait([refresh, browse]);
      expect(c.jobs.jobs.single.status, 'completed');
      expect(c.jobs.current!.status, 'completed');
      expect(c.jobs.detail!.job.status, 'completed');
      expect(c.jobs.detailState(fixtures.requestId).data, same(c.jobs.detail));
      expect(c.jobs.detailState(fixtures.requestId).stale, isFalse);
      expect(c.jobs.canStart, isTrue);
      expect(c.jobs.protocolBlocked, isFalse);
      final before = api.calls.length;
      await tester.pump(const Duration(seconds: 20));
      expect(api.calls.length, before);
      expect(api.starts, isEmpty);
    },
  );

  test('LIST-1/LIST-3 shared refresh updates active and changed historical cached details', () async {
    final api = _ControlledApi()
      ..jobs.addAll([
        _job(fixtures.requestId, status: 'running', created: 1700000000001),
        _job(_a, status: 'failed', project: _otherProject),
      ]);
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    await c.jobs.loadDetail(_a);
    final previous = c.jobs.detailState(_a);
    api.jobs[0] = {
      ...api.jobs[0],
      'message': 'Still running; new session evidence.',
      'session_id': 'ses_new',
    };
    api.jobs[1] = _job(_a, project: _otherProject);
    final before = api.gets.length;
    await c.jobs.refresh();
    expect(api.gets.skip(before), [fixtures.requestId, _a]);
    expect(c.jobs.detail!.job.sessionId, 'ses_new');
    expect(c.jobs.current!.requestId, fixtures.requestId);
    expect(c.jobs.detailState(_a).data!.reply, 'A saved final reply.');
    expect(c.jobs.detailState(_a).data!.job.status, 'completed');
    expect(c.jobs.detailState(_a).stale, isFalse);
    expect(previous.data!.job.status, 'failed');
    expect(previous.data!.reply, isNull);
    expect(c.jobs.jobs.last.status, 'completed');
    expect(c.jobs.canStart, isTrue);
    expect(api.starts, isEmpty);
  });

  test('LIST-3 changed summary fences old GET and its cleanup cannot clear the new generation', () async {
    final api = _ControlledApi()
      ..jobs.addAll([
        _job(fixtures.requestId, created: 1700000000001),
        _job(_a, status: 'failed'),
      ]);
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    await c.jobs.loadDetail(_a);
    final oldGate = api.holdGet(_a);
    final oldLoad = c.jobs.loadDetail(_a, force: true);
    api.jobs[1] = _job(_a);
    final newGate = api.holdGet(_a);
    final refresh = c.jobs.refresh();
    await _flush();
    final newLoad = c.jobs.loadDetail(_a);
    expect(c.jobs.detailState(_a).loading, isTrue);
    expect(c.jobs.detailState(_a).stale, isTrue);
    expect(c.jobs.detailState(_a).data!.job.status, 'failed');
    oldGate.complete(
      _detailReply(_job(_b)),
    ); // Even this contradiction is obsolete.
    await oldLoad;
    expect(c.jobs.detailState(_a).loading, isTrue);
    expect(identical(newLoad, c.jobs.loadDetail(_a, force: true)), isTrue);
    expect(c.jobs.protocolBlocked, isFalse);
    newGate.complete(_detailReply(_job(_a), reply: 'Fresh generation'));
    await Future.wait([refresh, newLoad]);
    expect(c.jobs.detailState(_a).data!.reply, 'Fresh generation');
    expect(c.jobs.detailState(_a).stale, isFalse);
    expect(c.jobs.detailState(_a).loading, isFalse);
    expect(c.jobs.protocolBlocked, isFalse);
  });

  test('LIST-3 changed service instance fences late success/error and refreshes all cached identities', () async {
    final api = _ControlledApi()
      ..jobs.addAll([
        _job(fixtures.requestId, created: 1700000000001),
        _job(_a),
      ]);
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    await c.jobs.loadDetail(_a);
    final oldGate = api.holdGet(_a);
    final oldLoad = c.jobs.loadDetail(_a, force: true);
    api.serviceInstance = _otherInstance;
    final newGate = api.holdGet(_a);
    final refresh = c.jobs.refresh();
    await _flush();
    expect(c.jobs.detailState(_a).loading, isTrue);
    oldGate.completeError(const AppFailure('UNKNOWN_REQUEST', 'Old uptime'));
    await oldLoad;
    expect(c.jobs.protocolBlocked, isFalse);
    expect(c.jobs.detailState(_a).loading, isTrue);
    newGate.complete(
      _detailReply(_job(_a), instance: _otherInstance, reply: 'New uptime'),
    );
    await refresh;
    expect(c.jobs.detailState(_a).data!.reply, 'New uptime');
    expect(c.jobs.detailState(_a).error, isNull);
    expect(c.jobs.canStart, isFalse);
    await c.projects.refresh();
    expect(c.jobs.canStart, isTrue);
  });

  test('LIST-3 unexpected detail instance stays keyed and cannot establish freshness', () async {
    final api = _ControlledApi()
      ..jobs.addAll([
        _job(fixtures.requestId, created: 1700000000001),
        _job(_a),
      ]);
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    final gate = api.holdGet(_a);
    final load = c.jobs.loadDetail(_a);
    gate.complete(_detailReply(_job(_a), instance: _otherInstance));
    await load;
    expect(c.jobs.detailState(_a).data, isNull);
    expect(c.jobs.detailState(_a).stale, isTrue);
    expect(c.jobs.detailState(_a).error, contains('Service changed'));
    expect(c.jobs.protocolBlocked, isFalse);
    expect(c.jobs.canStart, isTrue);
    expect(c.jobs.jobsStale, isFalse);
  });

  test('LIST-3 historical refresh errors remain keyed, while state keeps admission freshness', () async {
    final api = _ControlledApi()
      ..jobs.addAll([
        _job(fixtures.requestId, created: 1700000000001),
        _job(_a, status: 'failed'),
      ]);
    final c = _controllers(api);
    await fixtures.load(c.projects, c.jobs);
    await c.jobs.loadDetail(_a);
    api.jobs[1] = _job(_a);
    final gate = api.holdGet(_a);
    final refresh = c.jobs.refresh();
    await _flush();
    gate.completeError(
      const AppFailure('APP_UNAVAILABLE', 'History unavailable'),
    );
    await refresh;
    expect(c.jobs.detailState(_a).stale, isTrue);
    expect(c.jobs.detailState(_a).error, contains('APP_UNAVAILABLE'));
    expect(c.jobs.detailState(_a).data!.job.status, 'failed');
    expect(c.jobs.jobs.last.status, 'completed');
    expect(c.jobs.canStart, isTrue);
    expect(c.jobs.jobsStale, isFalse);
    expect(c.jobs.error, isNull);
  });

  test(
    'LIST-3 disposal fences in-flight list/detail results and notifications',
    () async {
      final api = _ControlledApi()
        ..jobs.addAll([
          _job(fixtures.requestId, created: 1700000000001),
          _job(_a),
        ]);
      final c = _controllers(api);
      await fixtures.load(c.projects, c.jobs);
      final detailGate = api.holdGet(_a);
      final load = c.jobs.loadDetail(_a);
      final stateGate = api.holdState();
      final refresh = c.jobs.refresh();
      var notifications = 0;
      c.jobs.addListener(() => notifications++);
      final current = c.jobs.current;
      final detail = c.jobs.detail;
      final rows = c.jobs.jobs;
      final keyed = c.jobs.detailState(_a);
      final calls = api.calls.toList();
      c.jobs.dispose();
      stateGate.complete(_stateReply(api, jobs: []));
      detailGate.complete(_detailReply(_job(_b)));
      await Future.wait([refresh, load]);
      await c.jobs.refresh();
      await c.jobs.loadDetail(_a, force: true);
      expect(c.jobs.current, same(current));
      expect(c.jobs.detail, same(detail));
      expect(c.jobs.jobs, same(rows));
      expect(c.jobs.detailState(_a), same(keyed));
      expect(c.jobs.protocolBlocked, isFalse);
      expect(c.jobs.canStart, isFalse);
      expect(notifications, 0);
      expect(api.calls, calls);
    },
  );

  for (final operation in ['refresh', 'detail']) {
    test(
      'LIST-3 disposal during $operation loading notification prevents dispatch',
      () async {
        final api = _ControlledApi()
          ..jobs.addAll([
            _job(fixtures.requestId, created: 1700000000001),
            _job(_a),
          ]);
        final c = _controllers(api);
        await fixtures.load(c.projects, c.jobs);
        final calls = api.calls.toList();
        c.jobs.addListener(c.jobs.dispose);
        await (operation == 'refresh'
            ? c.jobs.refresh()
            : c.jobs.loadDetail(_a));
        expect(api.calls, calls);
        expect(c.jobs.canStart, isFalse);
      },
    );
  }

  testWidgets(
    'LIST-4 history/force reads neither postpone the timer nor reset/rearm its 60x2s budget',
    (tester) async {
      final api = _ControlledApi()
        ..jobs.addAll([
          _job(fixtures.requestId, status: 'running', created: 1700000000001),
          _job(_a),
        ]);
      final c = _controllers(api);
      await fixtures.load(c.projects, c.jobs);
      await tester.pump(const Duration(seconds: 1));
      final current = c.jobs.current;
      final detail = c.jobs.detail;
      await c.jobs.loadDetail(_a);
      await c.jobs.loadDetail(_a, force: true);
      expect(c.jobs.current, same(current));
      expect(c.jobs.detail, same(detail));
      expect(api.calls.where((op) => op == 'dev_jobs_state_v4'), hasLength(1));
      await tester.pump(const Duration(seconds: 1));
      await tester.pump();
      expect(api.calls.where((op) => op == 'dev_jobs_state_v4'), hasLength(2));
      for (var n = 1; n < 60; n++) {
        await c.jobs.loadDetail(_a, force: true);
        await tester.pump(const Duration(seconds: 2));
        await tester.pump();
      }
      expect(c.jobs.observationStopped, isTrue);
      expect(api.calls.where((op) => op == 'dev_jobs_state_v4'), hasLength(61));
      final stopped = api.calls.length;
      await tester.pump(const Duration(seconds: 20));
      expect(api.calls.length, stopped);
      await c.jobs.loadDetail(_a, force: true);
      final before = api.calls.length;
      await c.jobs.refresh();
      expect(api.calls.skip(before), ['dev_jobs_state_v4', 'dev_job_get_v4']);
      expect(c.jobs.observationStopped, isTrue);
      final manual = api.calls.length;
      await tester.pump(const Duration(seconds: 20));
      expect(api.calls.length, manual);
      expect(api.starts, isEmpty);
      expect(api.calls.toSet(), {
        'projects_list_v3',
        'dev_jobs_state_v4',
        'dev_job_get_v4',
      });
    },
  );
}
