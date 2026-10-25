import 'dart:async';

import 'package:flutter/foundation.dart';

import 'app_api.dart';
import 'projects.dart';

/// One desktop-wide owner of inert service metadata, not of the shared AppApi.
class ProjectsController extends ChangeNotifier {
  ProjectsController(this._api);

  final AppApi _api;
  List<ProjectRecord> _projects = const [];
  int? _revision;
  String? _instanceId;
  bool _loading = false;
  bool _busy = false;
  bool _uncertain = false;
  bool _loaded = false;
  bool _fresh = false;
  bool _closed = false;
  String? _error;
  int _epoch = 0;
  int _notificationDepth = 0;
  Future<void>? _refreshing;
  Future<void>? _action;
  _PendingProjectMutation? _pending;

  List<ProjectRecord> get projects => _projects;
  int? get revision => _revision;
  String? get instanceId => _instanceId;
  bool get loading => _loading;
  bool get busy => _busy;
  bool get uncertain => _uncertain;
  bool get loaded => _loaded;
  String? get error => _error;
  bool get canMutate =>
      !_closed &&
      _loaded &&
      _fresh &&
      !_loading &&
      !_busy &&
      !_uncertain &&
      _instanceId != null &&
      _revision! < maxProjectRevision;

  /// Coalesced observation only; it cannot clear an unresolved mutation.
  Future<void> refresh() {
    if (_closed) return Future.value();
    if (_action != null) return _action!;
    if (_refreshing != null) return _refreshing!;
    final done = Completer<void>();
    _refreshing = done.future;
    final epoch = _epoch;
    _error = null;
    unawaited(_refresh(done, epoch));
    return done.future;
  }

  Future<void> _refresh(Completer<void> done, int epoch) async {
    try {
      await _readList(epoch);
      if (_alive(epoch)) {
        _error = _uncertain ? _pendingMessage : null;
      }
    } on Object catch (e) {
      if (_alive(epoch)) {
        _fresh = false;
        _error = _message(e);
      }
    } finally {
      if (_alive(epoch)) {
        _refreshing = null;
        notifyListeners();
      }
      done.complete();
    }
  }

  Future<void> create(
    String name,
    List<ProjectRepository> repositories, {
    required String expectedInstanceId,
    required int expectedRevision,
    String? key,
    String description = '',
    String prompt = '',
  }) => _mutate(
    'project_create_v3',
    null,
    name,
    repositories,
    expectedInstanceId,
    expectedRevision,
    key: key,
    description: description,
    prompt: prompt,
  );

  Future<void> update(
    String projectId,
    String name,
    List<ProjectRepository> repositories, {
    required String expectedInstanceId,
    required int expectedRevision,
    String? key,
    String description = '',
    String prompt = '',
  }) => _mutate(
    'project_update_v3',
    projectId,
    name,
    repositories,
    expectedInstanceId,
    expectedRevision,
    key: key,
    description: description,
    prompt: prompt,
  );

  Future<void> delete(
    String projectId, {
    required String expectedInstanceId,
    required int expectedRevision,
  }) => _mutate(
    'project_delete_v3',
    projectId,
    null,
    null,
    expectedInstanceId,
    expectedRevision,
  );

  Future<void> _mutate(
    String op,
    String? projectId,
    String? name,
    List<ProjectRepository>? repositories,
    String expectedInstanceId,
    int expectedRevision, {
    String? key,
    String description = '',
    String prompt = '',
  }) {
    if (_closed) return Future.value();
    try {
      if (!canMutate) {
        throw const AppFailure('UNAVAILABLE', 'Projects action unavailable.');
      }
      if (expectedInstanceId != _instanceId) {
        throw const AppFailure(
          'INSTANCE_MISMATCH',
          'Project editor origin changed.',
        );
      }
      if (expectedRevision != _revision) {
        throw const AppFailure('STALE', 'Project editor revision changed.');
      }
      if (projectId != null &&
          !_projects.any((p) => p.projectId == projectId)) {
        throw const AppFailure(
          'PROJECT_NOT_FOUND',
          'Project no longer exists.',
        );
      }
      if (op == 'project_create_v3' && _projects.length >= maxProjects) {
        throw const AppFailure('CAPACITY', 'Project registry is full.');
      }
      final effectiveKey =
          key ??
          (projectId == null
              ? 'project-${appId()}'
              : _projects.firstWhere((p) => p.projectId == projectId).key);
      if (op != 'project_delete_v3' &&
          _projects.any(
            (p) => p.projectId != projectId && p.key == effectiveKey,
          )) {
        throw const AppFailure(
          'INVALID_ARGUMENT',
          'Project key must be unique.',
        );
      }
      final args = freezeProjectArguments(<String, dynamic>{
        'request_id': appId(),
        'instance_id': expectedInstanceId,
        'expected_revision': expectedRevision,
        'project_id': ?projectId,
        if (op != 'project_delete_v3') ...{
          'name': name,
          'key': effectiveKey,
          'description': description,
          'prompt': prompt,
          'repositories': repositories!.map((r) => r.toJson()).toList(),
          'agent': projectAgent,
        },
      });
      validateProjectArguments(op, args);
      final pending = _PendingProjectMutation(op, args);
      final done = Completer<void>();
      _pending = pending;
      _action = done.future;
      _busy = true;
      _fresh = false;
      _error = null;
      final epoch = ++_epoch;
      notifyListeners();
      unawaited(_dispatch(pending, args, done, epoch));
      return done.future;
    } on Object catch (e) {
      _error = _uncertain ? _pendingMessage : _message(e);
      notifyListeners();
      return Future.value();
    }
  }

  Future<void> _dispatch(
    _PendingProjectMutation pending,
    Map<String, dynamic> args,
    Completer<void> done,
    int epoch,
  ) async {
    var acknowledged = false;
    try {
      if (!_alive(epoch)) return;
      final reply = await _api.call(pending.op, args);
      if (!_alive(epoch)) return;
      _receipt(pending, reply);
      acknowledged = true;
      await _readList(epoch, minimumRevision: pending.receipt!.revision);
      if (!_alive(epoch)) return;
      _pending = null;
      _uncertain = false;
      _error = null;
    } on Object catch (e) {
      if (_alive(epoch)) {
        _fresh = false;
        if (acknowledged || !_definiteRejection(e)) {
          _uncertain = true;
          _error = acknowledged
              ? 'Project action acknowledged, but the current list is unavailable. Resolve the original receipt; nothing will be resent.'
              : _pendingMessage;
        } else {
          _pending = null;
          _uncertain = false;
          _error = _message(e);
        }
      }
    } finally {
      _finishAction(done, epoch);
    }
  }

  /// Queries the original ID, including across a service restart. Never replay.
  Future<void> resolvePending() {
    if (_closed || _pending == null) return Future.value();
    if (_action != null) return _action!;
    if (_refreshing != null) {
      return _refreshing!.then((_) => resolvePending());
    }
    final pending = _pending!;
    final done = Completer<void>();
    _action = done.future;
    _busy = true;
    _fresh = false;
    _error = null;
    final epoch = ++_epoch;
    notifyListeners();
    unawaited(_resolve(pending, done, epoch));
    return done.future;
  }

  Future<void> _resolve(
    _PendingProjectMutation pending,
    Completer<void> done,
    int epoch,
  ) async {
    var receiptReceived = false;
    try {
      if (!_alive(epoch)) return;
      final reply = await _api.call('project_receipt', {
        'request_id': pending.requestId,
      });
      if (!_alive(epoch)) return;
      receiptReceived = true;
      _receipt(pending, reply);
      await _readList(epoch, minimumRevision: pending.receipt!.revision);
      if (!_alive(epoch)) return;
      _pending = null;
      _uncertain = false;
      _error = null;
    } on Object catch (e) {
      if (_alive(epoch)) {
        _fresh = false;
        _uncertain = true;
        _error = switch (e) {
          AppFailure(code: 'REQUEST_EXPIRED') => 'The original project receipt has expired. Its outcome remains uncertain; writes are blocked and nothing was resent.',
          AppFailure(code: 'REQUEST_UNKNOWN') => 'The original project request is unknown. Its outcome remains uncertain; writes are blocked and nothing was resent.',
          _ => _pendingMessage,
        };
        if (!receiptReceived &&
            e is AppFailure &&
            !e.unknown &&
            const {'REQUEST_UNKNOWN', 'REQUEST_EXPIRED'}.contains(e.code)) {
          final receiptError = _error!;
          // Present metadata stays observable even when replay evidence is gone.
          // A fresh list cannot establish the original mutation's outcome.
          try {
            await _readList(epoch);
            if (_alive(epoch)) _error = receiptError;
          } on Object {
            if (_alive(epoch)) {
              _fresh = false;
              _error =
                  '$receiptError The current list is also unavailable. Refresh to observe again.';
            }
          }
        }
      }
    } finally {
      _finishAction(done, epoch);
    }
  }

  void _receipt(_PendingProjectMutation pending, AppReply reply) {
    if (!projectWireId(reply.instanceId)) {
      throw const AppFailure('PROTOCOL', 'Invalid service instance.');
    }
    correlateProjectReceipt(pending.op, {
      'request_id': pending.requestId,
      'project_id': ?pending.projectId,
    }, reply.result);
    final receipt = ProjectReceipt.parse(reply.result);
    if (receipt.revision != pending.expectedRevision + 1) {
      throw const AppFailure('PROTOCOL', 'Project receipt revision mismatch.');
    }
    pending.receipt = receipt;
  }

  Future<void> _readList(int epoch, {int? minimumRevision}) async {
    if (!_alive(epoch)) return;
    _loading = true;
    notifyListeners();
    // A listener may dispose the controller while observing loading/busy.
    if (!_alive(epoch)) return;
    try {
      final reply = await _api.call('projects_list_v3', {});
      if (!_alive(epoch)) return;
      if (!projectWireId(reply.instanceId)) {
        throw const AppFailure('PROTOCOL', 'Invalid service instance.');
      }
      final snapshot = ProjectSnapshot.parse(reply.result);
      final floor = minimumRevision ?? _pending?.receipt?.revision;
      if ((floor != null && snapshot.revision < floor) ||
          (_revision != null && snapshot.revision < _revision!)) {
        throw const AppFailure('PROTOCOL', 'Stale project snapshot.');
      }
      _projects = snapshot.projects;
      _revision = snapshot.revision;
      _instanceId = reply.instanceId;
      _loaded = true;
      _fresh = true;
    } finally {
      if (_alive(epoch)) _loading = false;
    }
  }

  bool _alive(int epoch) => !_closed && epoch == _epoch;

  void _finishAction(Completer<void> done, int epoch) {
    if (_alive(epoch)) {
      _busy = false;
      _action = null;
      notifyListeners();
    }
    done.complete();
  }

  static const _pendingMessage =
      'Project action outcome is uncertain. Resolve the original receipt before making another change; nothing will be resent.';

  // Unrecognized faults may be post-publication; only known admission refusals
  // prove that this attempt cannot have changed the registry.
  bool _definiteRejection(Object e) =>
      e is AppFailure &&
      !e.unknown &&
      const {
        'INSTANCE_MISMATCH',
        'STALE',
        'STALE_REVISION',
        'REVISION_MISMATCH',
        'REVISION_EXHAUSTED',
        'PROJECT_NOT_FOUND',
        'CAPACITY',
        'LIMIT',
        'INVALID_REQUEST',
        'INVALID_ARGUMENT',
        'ARGUMENT',
        'ID_CONFLICT',
        'REQUEST_EXPIRED',
        'OP',
        'UPGRADE_REQUIRED',
        'CLOSED',
      }.contains(e.code);

  String _message(Object e) => switch (e) {
    AppFailure(code: 'STALE' || 'STALE_REVISION' || 'REVISION_MISMATCH') => 'Projects changed. Refresh the list and review your original edits before saving again.',
    AppFailure(code: 'INSTANCE_MISMATCH') => 'Smith restarted. Refresh Projects and review your original edits; nothing was resent.',
    AppFailure(code: 'PROJECT_NOT_FOUND') =>
      'This project no longer exists. Your edits have not been applied.',
    AppFailure(code: 'CAPACITY' || 'LIMIT') => 'Project capacity is unavailable. Refresh the list; your edits have not been applied.',
    AppFailure(
      code: 'PROTOCOL' || 'INVALID_REQUEST' || 'INVALID_ARGUMENT' || 'ARGUMENT',
    ) =>
      'Project metadata could not be validated. Check the fields and refresh if needed; your edits are preserved.',
    AppFailure(code: 'UNAVAILABLE') => 'Projects are not ready for changes. Refresh or resolve the pending action first.',
    _ => 'Projects are unavailable. Refresh to observe again; no action will be resent.',
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
    _closed = true;
    _epoch++;
    // Fence immediately; ChangeNotifier cannot clean up inside a notification.
    if (_notificationDepth == 0) super.dispose();
  }
}

class _PendingProjectMutation {
  _PendingProjectMutation(this.op, Map<String, dynamic> args)
    : requestId = args['request_id'],
      instanceId = args['instance_id'],
      expectedRevision = args['expected_revision'],
      projectId = args['project_id'];

  final String op;
  final String requestId;
  final String instanceId;
  final int expectedRevision;
  final String? projectId;
  ProjectReceipt? receipt;
}
