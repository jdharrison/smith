import 'dart:async';

import 'package:flutter/foundation.dart';

import 'app_api.dart';
import 'dev_jobs.dart';
import 'projects.dart' show projectWireId;
import 'projects_controller.dart';
import 'retained_jobs_controller.dart';

@immutable
final class DevJobDetailState {
  const DevJobDetailState({
    this.data,
    this.loading = false,
    this.stale = true,
    this.error,
  });

  final DevJobDetail? data;
  final bool loading, stale;
  final String? error;
}

/// Shared desktop admission guard. Observation never repeats a start request.
class DevJobsController extends ChangeNotifier {
  DevJobsController(this._api, this.projects)
    : retainedJobs = RetainedJobsController(_api, projects);
  final RetainedJobsController retainedJobs;
  final AppApi _api;
  final ProjectsController projects;
  DevJob? current;
  DevJobDetail? detail;
  DevLocalModel? model;
  DevScheduler? scheduler;
  String? pendingRequestId;
  String? error;
  String? deletingRequestId;
  String? pendingDeletionRequestId;
  String? deletionError;
  String? _deletionInstance;
  DevJob? _deletionJob;
  bool _resolvingDeletion = false;
  bool _deletionRefused = false;
  final Map<String, DevJob> _deleted = {};
  String? _instanceId;
  Map<String, dynamic>? _pending;
  final Set<String> _submitted = {};
  final Map<String, DevJob> _known = {};
  final Map<String, DevJobDetailState> _details = {};
  final Map<String, _DevDetailRead> _detailReads = {};
  final Map<String, int> _detailGenerations = {};
  List<DevJob> _jobs = const [];
  bool loaded = false;
  bool busy = false;
  bool observationStopped = false;
  bool protocolBlocked = false;
  bool _closed = false;
  bool _fresh = false;
  bool _jobsStale = true;

  int _epoch = 0;
  int _notificationDepth = 0;
  int _attempts = 0;

  Timer? _timer;
  Future<void>? _reading;

  /// Newest first; equal creation times use ascending request identity.
  List<DevJob> get jobs => _jobs;
  bool get refreshing => !_closed && _reading != null;
  bool get jobsStale => _jobsStale;
  // Retained count is not an admission limit.
  bool get ledgerFull => false;

  /// Admission follows authoritative rows, not a possibly older current snapshot.
  bool get canStart =>
      !_closed &&
      loaded &&
      _fresh &&
      !protocolBlocked &&
      scheduler?.accepting == true &&
      !busy &&
      _reading == null &&
      pendingRequestId == null &&
      deletingRequestId == null &&
      pendingDeletionRequestId == null &&
      model?.ready == true &&
      projects.canMutate &&
      _instanceId == projects.instanceId;

  DevJobDetailState detailState(String requestId) =>
      _details[requestId] ?? const DevJobDetailState();

  /// Browsing is keyed observation only, never admission recovery or polling.
  Future<void> loadDetail(String requestId, {bool force = false}) {
    if (_closed || isDeleted(requestId)) return Future.value();
    if (!_known.containsKey(requestId) || _instanceId == null) {
      _details[requestId] = const DevJobDetailState(
        error: 'Unknown Dev job. Refresh the jobs list before reading details.',
      );
      notifyListeners();
      return Future.value();
    }
    final reading = _detailReads[requestId];
    if (reading != null) return reading.done.future;
    final cached = detailState(requestId);
    if (!force && cached.data != null && !cached.stale) return Future.value();
    return _beginDetail(requestId).done.future;
  }

  bool isDeleted(String id) => _deleted.containsKey(id);

  bool canDeleteJob(String id) =>
      !_closed &&
      loaded &&
      _fresh &&
      !protocolBlocked &&
      !busy &&
      _reading == null &&
      deletingRequestId == null &&
      pendingDeletionRequestId == null &&
      pendingRequestId == null &&
      projects.canMutate &&
      _instanceId == projects.instanceId &&
      !isDeleted(id) &&
      _known[id]?.canDelete == true;

  DevJobDeletion _decodeDeletion(AppReply reply, DevJob job, String instance) {
    final proof = DevJobDeletion.parse(reply.result);
    if (reply.instanceId != instance || !_sameIdentity(proof.job, job)) {
      throw const _DevContradiction(
        'Dev deletion identity or instance mismatch.',
      );
    }
    return proof;
  }

  void _acceptDeletion(DevJob job) {
    _epoch++;
    for (final id in _details.keys.toList()) {
      _invalidateDetail(id);
    }
    _details.remove(job.requestId);
    _known.remove(job.requestId);
    _deleted[job.requestId] = job;
    _fresh = false;
    _jobsStale = true;
    _jobs = List.unmodifiable(_jobs.where((j) => j.requestId != job.requestId));

    if (current?.requestId == job.requestId) current = null;
    if (detail?.job.requestId == job.requestId) detail = null;
    notifyListeners();
  }

  Future<bool> _queryDeletion(DevJob job, String instance) async {
    final reply = await _api.call('dev_job_deletion_v3', {
      'request_id': job.requestId,
    });
    if (_closed) throw const _DevObservationChanged();
    final proof = _decodeDeletion(reply, job, instance);
    if (_instanceId != instance) throw const _DevObservationChanged();
    // Deletion is monotonic for this immutable identity and instance. An
    // unrelated deletion's epoch cannot invalidate a correlated proof.
    if (proof.deleted) _acceptDeletion(proof.job);
    return proof.deleted;
  }

  Future<void> deleteJob(String id) async {
    if (!canDeleteJob(id)) return;
    final job = _known[id]!;
    final instance = _instanceId!;
    final epoch = _epoch;
    deletingRequestId = id;
    pendingDeletionRequestId = id;
    _deletionJob = job;
    _deletionInstance = instance;
    _deletionRefused = false;
    deletionError = null;
    _timer?.cancel();
    notifyListeners();
    try {
      if (_closed) return;
      final reply = await _api.call('dev_job_delete_v3', {
        'request_id': id,
        'instance_id': instance,
      });
      if (_closed || epoch != _epoch || _instanceId != instance) return;
      final proof = _decodeDeletion(reply, job, instance);
      if (proof.deleted) {
        _acceptDeletion(proof.job);
      } else {
        deletionError = 'Deletion remains unknown. Resolve the original job.';
      }
    } on Object catch (e) {
      if (!_closed) {
        deletionError = _message(e);
        // These delete-path codes are guaranteed prewrite refusals, unlike
        // DEV_UNAVAILABLE/publication failures. AppFailure has no instance;
        // retain the fence until fresh original-instance state binds recovery.
        _deletionRefused =
            e is AppFailure &&
            !e.unknown &&
            const {
              'CAPACITY',
              'INVALID_REQUEST',
              'STALE_INSTANCE',
              'JOB_NOT_DELETABLE',
            }.contains(e.code);
        if (_deletionRefused) {
          _fresh = false;
          _jobsStale = true;
        }
      }
    } finally {
      deletingRequestId = null;
      if (!_closed) notifyListeners();
    }
    if (!_closed && (isDeleted(id) || _deletionRefused)) await refresh();
  }

  Future<void> resolveDeletion() async {
    if (_closed ||
        deletingRequestId != null ||
        _resolvingDeletion ||
        pendingDeletionRequestId == null) {
      return;
    }
    _resolvingDeletion = true;
    try {
      if (_deletionRefused) {
        await refresh();
        return;
      }
      final proven = await _queryDeletion(_deletionJob!, _deletionInstance!);
      if (_closed) return;
      if (proven) {
        await refresh();
      } else {
        deletionError = 'Deletion remains unknown. Resolve the original job.';
      }
    } on Object catch (e) {
      if (!_closed) deletionError = _message(e);
    } finally {
      _resolvingDeletion = false;
      if (!_closed) notifyListeners();
    }
  }

  Map<String, dynamic> capture(String projectId, String prompt) {
    if (!canStart) {
      throw const AppFailure(
        'UNAVAILABLE',
        'Refresh and review the saved project and ready local model before starting.',
      );
    }
    if (!projects.projects.any((p) => p.projectId == projectId)) {
      throw const AppFailure('UNCONFIGURED', 'Select a saved project.');
    }
    final args = Map<String, dynamic>.unmodifiable({
      'request_id': appId(),
      'instance_id': _instanceId,
      'project_id': projectId,
      'expected_projects_revision': projects.revision,
      'expected_config_revision': model!.configRevision,
      'prompt': prompt,
    });
    validateDevArguments('dev_job_start_v4', args);
    return args;
  }

  /// Read-only and coalesced across all mounted panes. Resolve always gets the
  /// original request before a consistent state read can retire its identity.
  Future<void> refresh() {
    if (_closed || busy || deletingRequestId != null) return Future.value();
    if (_reading != null) return _reading!;
    final done = Completer<void>();
    _reading = done.future;
    notifyListeners();
    unawaited(_read(done));
    return done.future;
  }

  DevJobDetail _decodeDetail(
    String id,
    AppReply reply, {
    String? expectedInstance,
    String? expectedProject,
    DevJob? expectedJob,
  }) {
    _validateInstance(reply.instanceId);
    if (expectedInstance != null && reply.instanceId != expectedInstance) {
      throw const AppFailure(
        'PROTOCOL',
        'Service changed during Dev observation. Refresh saved metadata; no replay.',
      );
    }
    final result = DevJobDetail.parse(reply.result);
    if (result.job.requestId != id ||
        (expectedProject != null && result.job.projectId != expectedProject) ||
        (expectedJob != null && !_sameIdentity(result.job, expectedJob))) {
      throw const _DevContradiction('Dev detail identity or project mismatch.');
    }
    return result;
  }

  _DevDetailRead _beginDetail(String id) {
    final existing = _detailReads[id];
    if (existing != null) return existing;
    final reading = _DevDetailRead(
      _known[id]!,
      _instanceId!,
      _epoch,
      _detailGenerations[id] ?? 0,
    );
    _detailReads[id] = reading;
    final previous = detailState(id);
    _details[id] = DevJobDetailState(
      data: previous.data,
      loading: true,
      stale: previous.stale,
    );
    notifyListeners();
    unawaited(_readDetail(reading));
    return reading;
  }

  bool _detailAlive(_DevDetailRead reading) =>
      !_closed &&
      reading.epoch == _epoch &&
      reading.instanceId == _instanceId &&
      reading.generation == (_detailGenerations[reading.job.requestId] ?? 0) &&
      identical(_detailReads[reading.job.requestId], reading);

  Future<void> _readDetail(_DevDetailRead reading) async {
    final id = reading.job.requestId;
    try {
      if (!_detailAlive(reading)) return;
      final reply = await _api.call('dev_job_get_v4', {'request_id': id});
      if (!_detailAlive(reading)) return;
      final found = _decodeDetail(
        id,
        reply,
        expectedInstance: reading.instanceId,
        expectedJob: reading.job,
      );
      reading.result = found;
      _details[id] = DevJobDetailState(data: found, stale: false);
    } on Object catch (e) {
      if (_detailAlive(reading)) {
        reading.failure = e;
        final contradiction = _contradictionMessage(e);
        if (e is AppFailure && e.code == 'JOB_DELETED') {
          try {
            await _queryDeletion(reading.job, reading.instanceId);
            if (isDeleted(id)) return;
          } on Object {
            // An uncorrelated error alone never proves deletion.
          }
        }
        if (!_detailAlive(reading)) return;
        if (contradiction != null ||
            _unknown(e) ||
            (e is AppFailure && e.code == 'JOB_DELETED')) {
          _block(contradiction ?? _missingMessage);
        }
        _details[id] = DevJobDetailState(
          data: detailState(id).data,
          error: _message(e),
        );
      }
    } finally {
      if (_detailAlive(reading)) {
        _detailReads.remove(id);
        notifyListeners();
      }
      reading.done.complete();
    }
  }

  Future<DevJobDetail?> _observeDetail(String id) async {
    final reading = _beginDetail(id);
    await reading.done.future;
    if (_closed) return null;
    if (reading.epoch != _epoch || isDeleted(id)) {
      throw const AppFailure('UNAVAILABLE', 'Dev detail observation changed.');
    }
    if (reading.failure != null) throw reading.failure!;
    final found = reading.result;
    if (found == null) {
      throw const AppFailure('UNAVAILABLE', 'Dev detail observation changed.');
    }
    // Only shared refresh may advance rows/admission from a detail observation.
    // A modal GET deliberately leaves those operational facts untouched.
    _publishJobs([
      for (final job in _jobs) job.requestId == id ? found.job : job,
    ]);
    _details[id] = DevJobDetailState(data: found, stale: false);
    return found;
  }

  Future<void> _read(Completer<void> done) async {
    var epoch = _epoch;
    try {
      if (_closed) return;
      final pending = pendingRequestId;
      DevJobDetail? resolved;
      String? resolvedInstance;
      if (pending != null) {
        final reply = await _api.call('dev_job_get_v4', {
          'request_id': pending,
        });
        if (_closed || epoch != _epoch) return;
        resolved = _decodeDetail(
          pending,
          reply,
          expectedProject: _pending!['project_id'],
          expectedJob: _known[pending],
        );
        resolvedInstance = reply.instanceId;
        // A successful GET is evidence of a retained identity even if the next
        // state read fails. It is not yet a resolved admission or a list row.
        _known[pending] = resolved.job;
      }
      final reply = await _api.call('dev_jobs_state_v4', {});
      if (_closed || epoch != _epoch) return;
      _validateInstance(reply.instanceId);
      final state = DevJobsState.parse(reply.result);
      if (resolvedInstance != null && reply.instanceId != resolvedInstance) {
        throw const AppFailure(
          'PROTOCOL',
          'Service changed during Dev admission recovery; retain the original request. No replay.',
        );
      }
      try {
        await _verifyJobs(state.jobs, reply.instanceId);
      } finally {
        // Verification can itself accept deletion proofs and advance the epoch.
        epoch = _epoch;
      }
      if (_closed) return;
      if (_instanceId != reply.instanceId) {
        _epoch++;
        for (final id in _details.keys.toList()) {
          _invalidateDetail(id);
        }
      }
      _instanceId = reply.instanceId;

      model = state.model;
      scheduler = state.scheduler;
      _publishJobs(state.jobs);
      final deletionId = pendingDeletionRequestId;
      if (deletionId != null && reply.instanceId == _deletionInstance) {
        final listed = state.jobs
            .where((job) => job.requestId == deletionId)
            .firstOrNull;
        final provedDeleted = isDeleted(deletionId) && listed == null;
        final confirmedRefusal =
            _deletionRefused &&
            listed != null &&
            _sameIdentity(listed, _deletionJob!);
        if (provedDeleted || confirmedRefusal) {
          pendingDeletionRequestId = null;
          _deletionJob = null;
          _deletionInstance = null;
          _deletionRefused = false;
          if (provedDeleted) deletionError = null;
        }
      }
      if (resolved != null) {
        current = resolved.job;
        detail = resolved;
        final listed = _known[pending]!;
        _details[pending!] = DevJobDetailState(
          data: resolved,
          stale: !_sameSummary(resolved.job, listed),
        );
        pendingRequestId = null;
        _pending = null;
        // Retain the original compatibility selection. Any other active row
        // drives bounded observation independently of current.
      } else {
        final active = _jobs.where((job) => job.active).firstOrNull;
        final blocking = _jobs
            .where((job) => !job.cleanupVerified && !job.active)
            .firstOrNull;
        final selected = active ?? blocking ?? _jobs.firstOrNull;
        if (selected != null) {
          final found = await _observeDetail(selected.requestId);
          if (_closed) return;
          current = found!.job;
          detail = found;
        }
      }
      // Observe every active row through the shared bounded poller, regardless
      // of which request a modal happens to show.
      for (final job in _jobs.where((job) => job.active)) {
        if (job.requestId == pending || job.requestId == current?.requestId) {
          continue;
        }
        await _observeDetail(job.requestId);
        if (_closed) return;
      }
      // Reload changed historical snapshots after either observation path,
      // without selecting them. The original pending GET was already observed;
      // a legitimate phase race there stays stale until the next observation.
      final staleIds = _jobs
          .where((job) => _details.containsKey(job.requestId))
          .map((job) => job.requestId)
          .where((id) => id != pending && detailState(id).stale)
          .toList();
      for (final id in staleIds) {
        try {
          await _observeDetail(id);
        } on Object {
          // Historical availability does not determine admission freshness.
          // Contradictions still retain their global protocol fence.
          if (protocolBlocked) rethrow;
        }
        if (_closed) return;
      }
      loaded = true;
      _fresh = true;
      _jobsStale = false;
      if (!protocolBlocked) error = null;
    } on Object catch (e) {
      if (!_closed && epoch == _epoch) {
        _fresh = false;
        _jobsStale = true;
        final contradiction = _contradictionMessage(e);
        if (contradiction != null) _block(contradiction);
        if (_unknown(e) && _known.containsKey(pendingRequestId)) {
          _block(_missingMessage);
        }
        if (pendingDeletionRequestId != null) {
          deletionError =
              'Deletion confirmation pending. Resolve the original job.';
        }
        error = pendingRequestId != null
            ? 'Admission remains unknown. Resolve queries only the original request; nothing will be resent.'
            : _message(e);
      }
    } finally {
      _reading = null;
      if (!_closed) {
        _schedule();
        notifyListeners();
      }
      done.complete();
    }
  }

  static const _missingMessage =
      'Previously accepted Dev job is missing; retain its identity. New starts are blocked.';

  Future<void> _verifyJobs(List<DevJob> jobs, String instance) async {
    final byId = {for (final job in jobs) job.requestId: job};
    for (final id in _deleted.keys) {
      if (byId.containsKey(id)) {
        throw const _DevContradiction('Deleted Dev job reappeared.');
      }
    }
    for (final known in _known.values.toList()) {
      final listed = byId[known.requestId];
      if (listed == null) {
        try {
          if (_instanceId != instance ||
              !await _queryDeletion(known, instance)) {
            throw const _DevContradiction(_missingMessage);
          }
        } on _DevObservationChanged {
          rethrow;
        } on _DevContradiction {
          rethrow;
        } on Object {
          throw const _DevContradiction(_missingMessage);
        }
        continue;
      }
      if (!_sameIdentity(known, listed)) {
        throw const _DevContradiction('Dev job identity or project mismatch.');
      }
    }
  }

  void _publishJobs(List<DevJob> jobs) {
    if (jobs.any((job) => isDeleted(job.requestId))) {
      throw const _DevContradiction('Deleted Dev job reappeared.');
    }
    for (final job in jobs) {
      final previous = _known[job.requestId];
      if (previous != null && !_sameSummary(previous, job)) {
        _invalidateDetail(job.requestId);
      }
      _known[job.requestId] = job;
    }
    final sorted = jobs.toList()
      ..sort((a, b) {
        final created = b.createdMs.compareTo(a.createdMs);
        return created != 0 ? created : a.requestId.compareTo(b.requestId);
      });
    _jobs = List.unmodifiable(sorted);
  }

  void _invalidateDetail(String id) {
    _detailGenerations[id] = (_detailGenerations[id] ?? 0) + 1;
    _detailReads.remove(id);
    final previous = _details[id];
    if (previous != null) {
      _details[id] = DevJobDetailState(
        data: previous.data,
        error: previous.error,
      );
    }
  }

  bool _sameIdentity(DevJob a, DevJob b) =>
      a.requestId == b.requestId &&
      a.projectId == b.projectId &&
      a.projectKey == b.projectKey &&
      a.createdMs == b.createdMs;

  bool _sameSummary(DevJob a, DevJob b) =>
      _sameIdentity(a, b) &&
      a.status == b.status &&
      a.message == b.message &&
      a.sessionId == b.sessionId &&
      a.replyAvailable == b.replyAvailable &&
      a.cleanupVerified == b.cleanupVerified;

  void _validateInstance(String instanceId) {
    if (!projectWireId(instanceId)) {
      throw const AppFailure('PROTOCOL', 'Invalid Dev service instance.');
    }
  }

  String? _contradictionMessage(Object e) => switch (e) {
    _DevContradiction(:final message) => message,
    DevJobCorrelationFailure(:final message) => message,
    _ => null,
  };

  bool _unknown(Object e) =>
      e is AppFailure &&
      const {'UNKNOWN_REQUEST', 'REQUEST_UNKNOWN'}.contains(e.code);

  void _block(String message) {
    protocolBlocked = true;
    _fresh = false;
    _jobsStale = true;
    error = message;
    _timer?.cancel();
  }

  Future<bool> start(Map<String, dynamic> captured) async {
    if (!canStart ||
        _submitted.contains(captured['request_id']) ||
        _known.containsKey(captured['request_id'])) {
      if (!_closed) {
        error = 'Start unavailable or saved origin changed. Refresh and review again; draft retained.';
        notifyListeners();
      }
      return false;
    }
    final args = Map<String, dynamic>.unmodifiable(captured);
    validateDevArguments('dev_job_start_v4', args);
    if (args['instance_id'] != _instanceId ||
        args['expected_projects_revision'] != projects.revision ||
        args['expected_config_revision'] != model!.configRevision ||
        !projects.projects.any((p) => p.projectId == args['project_id'])) {
      error = 'Saved origin changed during confirmation. Refresh and review again; draft retained.';
      notifyListeners();
      return false;
    }
    busy = true;
    _timer?.cancel();
    pendingRequestId = args['request_id'];
    _pending = args;
    _submitted.add(args['request_id'] as String);
    error = null;
    notifyListeners();
    var accepted = false;
    try {
      if (_closed) return false;
      final reply = await _api.call('dev_job_start_v4', args);
      if (_closed) return false;
      final job = correlateDevJob('dev_job_start_v4', args, reply.result);
      if (reply.instanceId != args['instance_id']) {
        throw const AppFailure(
          'PROTOCOL',
          'Dev admission instance mismatch.',
          unknown: true,
        );
      }
      final known = _known[job.requestId];
      if (known != null && !_sameIdentity(known, job)) {
        throw const _DevContradiction('Dev admission identity mismatch.');
      }
      if (isDeleted(job.requestId)) {
        throw const _DevContradiction('Deleted Dev job reappeared.');
      }
      current = job;
      detail = null;
      pendingRequestId = null;
      _pending = null;
      _publishJobs([
        ..._jobs.where((saved) => saved.requestId != job.requestId),
        job,
      ]);

      _attempts = 0;
      observationStopped = false;
      accepted = true;
      // A terminal acknowledgement can already have a reply; get it without
      // resubmitting, just as an active job's next observation would.
      _fresh = false;
      _jobsStale = true;
    } on Object catch (e) {
      if (!_closed) {
        if (e is _DevContradiction) _block(e.message);
        // Only explicit pre-admission refusals establish non-admission.
        if (e is AppFailure &&
            !e.unknown &&
            const {
              'INVALID_REQUEST',
              'BUSY',
              'CAPACITY',
              'STALE_INSTANCE',
              'STALE_REVISION',
              'NOT_CONFIGURED',
            }.contains(e.code)) {
          pendingRequestId = null;
          _pending = null;
          _fresh = false;
          error =
              '${_message(e)} Refresh saved metadata and review again. Draft retained.';
        } else {
          error = 'Admission unknown. Resolve the original request only; no resubmission.';
        }
      }
    } finally {
      busy = false;
      if (!_closed) {
        _schedule();
        notifyListeners();
      }
    }
    if (accepted && !_closed) unawaited(refresh());
    return accepted;
  }

  void _schedule() {
    _timer?.cancel();
    if (_closed ||
        protocolBlocked ||
        pendingRequestId != null ||
        pendingDeletionRequestId != null ||
        deletingRequestId != null ||
        !_jobs.any((job) => job.active)) {
      return;
    }
    if (_attempts >= 60) {
      observationStopped = true;
      return;
    }
    _timer = Timer(const Duration(seconds: 2), () {
      _attempts++;
      unawaited(refresh());
    });
  }

  String _message(Object e) => switch (e) {
    _DevContradiction(:final message) => 'PROTOCOL: $message',
    AppFailure() => '${e.code}: ${e.message}',
    _ => 'Dev observation unavailable. Refresh to observe only.',
  };

  @override
  void notifyListeners() {
    if (_closed) return;
    _notificationDepth++;
    try {
      super.notifyListeners();
    } finally {
      _notificationDepth--;
      if (_closed && _notificationDepth == 0) super.dispose();
    }
  }

  @override
  void dispose() {
    if (_closed) return;
    retainedJobs.dispose();
    _closed = true;
    _epoch++;
    _timer?.cancel();
    if (_notificationDepth == 0) super.dispose();
  }
}

final class _DevDetailRead {
  _DevDetailRead(this.job, this.instanceId, this.epoch, this.generation);

  final DevJob job;
  final String instanceId;
  final int epoch, generation;
  final done = Completer<void>();
  DevJobDetail? result;
  Object? failure;
}

final class _DevObservationChanged extends AppFailure {
  const _DevObservationChanged()
    : super(
        'UNAVAILABLE',
        'Dev observation changed. Refresh to observe; no replay.',
      );
}

final class _DevContradiction implements Exception {
  const _DevContradiction(this.message);
  final String message;
}
