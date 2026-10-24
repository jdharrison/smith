import 'dart:async';

import 'package:flutter/foundation.dart';

import 'app_api.dart';
import 'projects_controller.dart';
import 'retained_jobs.dart';

/// Desktop-wide admission fence; no outbox and no mutation replay.
class RetainedJobsController extends ChangeNotifier {
  RetainedJobsController(this.api, this.projects);
  final AppApi api;
  final ProjectsController projects;
  RetainedState? state;
  String? instanceId, error;
  Map<String, dynamic>? pending;
  RetainedJob? _pendingJob;
  bool fresh = false, busy = false, loading = false, stopped = false;
  bool _closed = false;
  int _epoch = 0, _attempts = 0;
  Timer? _timer;
  final Set<String> _attempted = {};
  final Set<String> _admitted = {};
  final Map<String, ValueChanged<String>> _admissionObservers = {};

  void _recordAdmission(String operationId) {
    if (!_admitted.add(operationId)) return;
    _admissionObservers.remove(operationId)?.call(operationId);
  }

  final Map<String, RetainedDetail> details = {};
  final Set<String> readingDetails = {};
  final Map<String, String> detailErrors = {};
  bool get canMutate =>
      !_closed &&
      fresh &&
      !loading &&
      !busy &&
      pending == null &&
      instanceId != null;
  bool get canStart =>
      canMutate &&
      state?.available == true &&
      state?.accepting == true &&
      projects.canMutate &&
      projects.instanceId == instanceId;
  RetainedJob? job(String id) {
    for (final j in state?.jobs ?? <RetainedJob>[]) {
      if (j.sessionId == id) return j;
    }
    return null;
  }

  bool canAct(String id, String action) {
    final j = job(id);
    if (!canMutate || j == null || j.deleted) return false;
    if (action == 'prompt') {
      return state!.available &&
          j.retained &&
          {'ready', 'failed'}.contains(j.status) &&
          details[id]?.revision == state!.revision &&
          details[id]!.turns.length < 32;
    }
    if (!{'finish', 'delete'}.contains(action)) return false;
    if (j.retained && !state!.available) return false;
    return j.canFinish || (action == 'delete' && j.status == 'finished');
  }

  Future<void> refresh({bool automatic = false}) async {
    if (_closed || loading || busy) return;
    if (!automatic) {
      _attempts = 0;
      stopped = false;
    }
    _timer?.cancel();
    loading = true;
    fresh = false;
    final epoch = _epoch;
    final originalPending = pending;
    notifyListeners();
    try {
      AppReply? receiptReply;
      if (originalPending != null) {
        try {
          receiptReply = await api.call('dev_session_receipt_v1', {
            'operation_id': originalPending['operation_id'],
          });
        } on Object {
          // Absence or transport failure never proves non-admission.
        }
        if (_closed || epoch != _epoch) return;
      }
      final receipt = receiptReply == null
          ? null
          : RetainedReceipt.parse(receiptReply.result);
      if (receipt != null) _checkPendingReceipt(receipt, originalPending!);
      final reply = await api.call('dev_sessions_state_v1', {});
      if (_closed || epoch != _epoch) return;
      final next = RetainedState.parse(reply.result);
      if (instanceId == reply.instanceId &&
          state != null &&
          next.revision < state!.revision) {
        throw const AppFailure('PROTOCOL', 'Stale retained state.');
      }
      if (instanceId == reply.instanceId) {
        for (final nextJob in next.jobs) {
          final previous = job(nextJob.sessionId);
          if (previous != null && !previous.sameIdentity(nextJob)) {
            throw const AppFailure('PROTOCOL', 'Retained identity changed.');
          }
        }
      }
      RetainedDetail? recovered;
      if (receipt != null) {
        if (receiptReply!.instanceId != reply.instanceId ||
            receipt.revision > next.revision) {
          throw const AppFailure('PROTOCOL', 'Receipt and state disagree.');
        }
        final detailReply = await api.call('dev_session_get_v1', {
          'session_id': receipt.sessionId,
        });
        if (_closed || epoch != _epoch) return;
        recovered = RetainedDetail.parse(detailReply.result);
        final row = next.jobs
            .where((j) => j.sessionId == receipt.sessionId)
            .firstOrNull;
        if (detailReply.instanceId != reply.instanceId ||
            recovered.revision != next.revision ||
            !receipt.job.sameIdentity(recovered.job) ||
            (receipt.job.deleted && !recovered.job.deleted) ||
            (row == null
                ? !recovered.job.deleted
                : !row.sameSnapshot(recovered.job))) {
          throw const AppFailure(
            'PROTOCOL',
            'Receipt details disagree with state.',
          );
        }
        if (originalPending!.containsKey('prompt') &&
            !recovered.turns.any(
              (t) =>
                  t.operationId == receipt.operationId &&
                  t.prompt == originalPending['prompt'],
            )) {
          throw const AppFailure(
            'PROTOCOL',
            'Receipt turn does not match the original prompt.',
          );
        }
      }
      if (instanceId != reply.instanceId) {
        details.clear();
        detailErrors.clear();
        _epoch++;
      }
      instanceId = reply.instanceId;
      state = next;
      if (recovered != null && identical(pending, originalPending)) {
        details[recovered.job.sessionId] = recovered;
        detailErrors.remove(recovered.job.sessionId);
        pending = null;
        _pendingJob = null;
        _recordAdmission(receipt!.operationId);
      }
      fresh = true;
      error = null;
    } on Object {
      if (!_closed) error = 'Retained jobs unavailable. Refresh to observe.';
    } finally {
      if (!_closed) {
        loading = false;
        notifyListeners();
        _schedule();
      }
    }
  }

  void _checkPendingReceipt(
    RetainedReceipt receipt,
    Map<String, dynamic> args,
  ) {
    if (receipt.operationId != args['operation_id'] ||
        receipt.revision <= (args['expected_revision'] as int) ||
        (args.containsKey('session_id') &&
            receipt.sessionId != args['session_id']) ||
        (_pendingJob != null && !_pendingJob!.sameIdentity(receipt.job)) ||
        (args.containsKey('project_id') &&
            (receipt.job.projectId != args['project_id'] ||
                receipt.job.jobType != args['job_type']))) {
      throw const AppFailure(
        'PROTOCOL',
        'Receipt does not match the original operation.',
      );
    }
  }

  void _schedule() {
    if (_closed ||
        busy ||
        !(pending != null || state?.jobs.any((j) => j.active) == true)) {
      return;
    }
    if (_attempts >= 60) {
      stopped = true;
      notifyListeners();
      return;
    }
    _timer = Timer(const Duration(seconds: 2), () {
      _attempts++;
      unawaited(refresh(automatic: true));
    });
  }

  Future<void> loadDetail(String id) async {
    final original = job(id);
    final instance = instanceId;
    final epoch = _epoch;
    final revision = state?.revision;
    if (_closed ||
        original == null ||
        instance == null ||
        readingDetails.contains(id)) {
      return;
    }
    readingDetails.add(id);
    detailErrors.remove(id);
    notifyListeners();
    try {
      final r = await api.call('dev_session_get_v1', {'session_id': id});
      if (_closed || epoch != _epoch) return;
      final d = RetainedDetail.parse(r.result);
      if (r.instanceId != instance ||
          instanceId != instance ||
          !original.sameIdentity(d.job) ||
          state?.revision != revision ||
          d.revision != revision ||
          job(id)?.status != d.job.status ||
          job(id)?.retained != d.job.retained ||
          job(id)?.deleted != d.job.deleted) {
        throw const AppFailure('PROTOCOL', 'Retained detail changed.');
      }
      details[id] = d;
    } on Object {
      if (!_closed && epoch == _epoch) {
        detailErrors[id] = 'Details unavailable or stale. Refresh to observe.';
      }
    } finally {
      if (!_closed) {
        readingDetails.remove(id);
        notifyListeners();
      }
    }
  }

  Map<String, dynamic> captureStart(
    String projectId,
    String type,
    String prompt,
  ) {
    if (!canStart || !projects.projects.any((p) => p.projectId == projectId)) {
      throw const AppFailure('STALE', 'Review again.');
    }
    final args = <String, dynamic>{
      'operation_id': appId(),
      'instance_id': instanceId,
      'expected_revision': state!.revision,
      'expected_projects_revision': projects.revision,
      'project_id': projectId,
      'job_type': type,
      'prompt': prompt,
    };
    validateRetainedArguments('dev_session_start_v1', args);
    return Map.unmodifiable(args);
  }

  bool validReview(Map<String, dynamic> args) =>
      canStart &&
      !_attempted.contains(args['operation_id']) &&
      args['instance_id'] == instanceId &&
      args['expected_revision'] == state!.revision &&
      args['expected_projects_revision'] == projects.revision &&
      projects.projects.any((p) => p.projectId == args['project_id']);
  Future<bool> start(Map<String, dynamic> args) async {
    if (!validReview(args)) return false;
    return _mutate('start', args);
  }

  Future<bool> act(
    String id,
    String action, {
    String? prompt,
    ValueChanged<String>? onAdmitted,
  }) async {
    if (!canAct(id, action)) return false;
    final args = <String, dynamic>{
      'operation_id': appId(),
      'instance_id': instanceId,
      'expected_revision': state!.revision,
      'session_id': id,
      if (action == 'prompt') 'prompt': prompt,
    };
    return _mutate(action, args, onAdmitted: onAdmitted);
  }

  Future<bool> _mutate(
    String action,
    Map<String, dynamic> args, {
    ValueChanged<String>? onAdmitted,
  }) async {
    final op = 'dev_session_${action}_v1';
    validateRetainedArguments(op, args);
    final operationId = args['operation_id'] as String;
    if (!_attempted.add(operationId)) return false;
    if (onAdmitted != null) _admissionObservers[operationId] = onAdmitted;
    final original = args['session_id'] == null
        ? null
        : job(args['session_id'] as String);
    pending = Map.unmodifiable(args);
    _pendingJob = original;
    busy = true;
    fresh = false;
    _timer?.cancel();
    _epoch++;
    final epoch = _epoch;
    notifyListeners();
    String? refusal;
    try {
      final reply = await api.call(op, args);
      if (_closed || epoch != _epoch) return false;
      final r = RetainedReceipt.parse(reply.result);
      if (reply.instanceId != args['instance_id'] ||
          r.operationId != args['operation_id'] ||
          r.revision <= (args['expected_revision'] as int) ||
          (original != null && !original.sameIdentity(r.job)) ||
          (action == 'start' &&
              (r.job.projectId != args['project_id'] ||
                  r.job.jobType != args['job_type']))) {
        throw const AppFailure('PROTOCOL', 'Receipt mismatch.');
      }
      pending = null;
      _pendingJob = null;
      _recordAdmission(operationId);
      error = null;
    } on Object catch (e) {
      // Only documented pre-write refusals release the original identity fence.
      if (!_closed &&
          e is AppFailure &&
          !e.unknown &&
          {
            'INVALID_REQUEST',
            'CONTEXT_LIMIT',
            'STALE_INSTANCE',
            'STALE_REVISION',
            'REQUEST_CONFLICT',
            'JOB_NOT_FOUND',
            'JOB_BLOCKED',
            'CAPACITY',
            'DEV_RUNTIME_UNAVAILABLE',
          }.contains(e.code)) {
        pending = null;
        _pendingJob = null;
        _admissionObservers.remove(operationId);
        refusal = 'Action refused. Review again.';
        error = refusal;
      }
    } finally {
      if (!_closed) {
        busy = false;
        notifyListeners();
      }
    }
    if (!_closed) await refresh();
    if (!_closed && refusal != null) {
      error = refusal;
      notifyListeners();
    }
    return !_closed && _admitted.contains(operationId);
  }

  @override
  void dispose() {
    if (_closed) return;
    _closed = true;
    _epoch++;
    _timer?.cancel();
    _admissionObservers.clear();
    super.dispose();
  }
}
