import 'dart:async';

import 'package:flutter/foundation.dart';

import 'app_api.dart';
import 'agents.dart';
import 'projects.dart' show projectWireId;

/// One desktop-wide owner of inert service metadata, not of the shared AppApi.
class AgentsController extends ChangeNotifier {
  AgentsController(this._api);

  final AppApi _api;
  List<AgentRecord> _agents = const [];
  int? _revision;
  String? _instanceId;
  bool _loading = false;
  bool _busy = false;
  bool _uncertain = false;
  bool _loaded = false;
  bool _fresh = false;
  bool _closed = false;
  String? _error;
  AgentReceipt? _lastReceipt;
  AgentReceipt? get lastReceipt => _lastReceipt;
  int _epoch = 0;
  int _notificationDepth = 0;
  Future<void>? _refreshing;
  Future<void>? _action;
  _PendingAgentMutation? _pending;

  List<AgentRecord> get agents => _agents;
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
      _revision! < maxAgentRevision;

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
    String description,
    String prompt, {
    required String expectedInstanceId,
    required int expectedRevision,
    String? requestId,
  }) => _mutate(
    null,
    name,
    description,
    prompt,
    expectedInstanceId: expectedInstanceId,
    expectedRevision: expectedRevision,
    requestId: requestId,
  );

  Future<void> update(
    String agentId,
    String name,
    String description,
    String prompt, {
    required String expectedInstanceId,
    required int expectedRevision,
    String? requestId,
  }) => _mutate(
    agentId,
    name,
    description,
    prompt,
    expectedInstanceId: expectedInstanceId,
    expectedRevision: expectedRevision,
    requestId: requestId,
  );

  Future<void> _mutate(
    String? agentId,
    String name,
    String description,
    String prompt, {
    required String expectedInstanceId,
    required int expectedRevision,
    String? requestId,
  }) {
    final op = agentId == null ? 'agent_create_v1' : 'agent_update_v2';
    if (_closed) return Future.value();
    try {
      if (!canMutate) {
        throw const AppFailure('UNAVAILABLE', 'Agents action unavailable.');
      }
      if (expectedInstanceId != _instanceId) {
        throw const AppFailure(
          'INSTANCE_MISMATCH',
          'Agent editor origin changed.',
        );
      }
      if (expectedRevision != _revision) {
        throw const AppFailure('STALE', 'Agent editor revision changed.');
      }
      if (agentId == null && _agents.length >= maxAgents) {
        throw const AppFailure('CAPACITY', 'Agent limit reached.');
      }
      if (agentId != null && !_agents.any((p) => p.agentId == agentId)) {
        throw const AppFailure('AGENT_NOT_FOUND', 'Agent no longer exists.');
      }
      if (agentId != null) {
        final current = _agents.firstWhere((a) => a.agentId == agentId);
        if (current.isBuiltIn &&
            (current.name != name || current.description != description)) {
          throw const AppFailure(
            'INVALID_ARGUMENT',
            'Built-in metadata is immutable.',
          );
        }
      }
      final args = Map<String, dynamic>.unmodifiable({
        'request_id': requestId ?? appId(),
        'instance_id': expectedInstanceId,
        'expected_revision': expectedRevision,
        'agent_id': ?agentId,
        'name': name,
        'description': description,
        'prompt': prompt,
      });
      validateAgentArguments(op, args);
      final pending = _PendingAgentMutation(op, args);
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
    _PendingAgentMutation pending,
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
              ? 'Agent action acknowledged, but the current list is unavailable. Resolve the original receipt; nothing will be resent.'
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
    _PendingAgentMutation pending,
    Completer<void> done,
    int epoch,
  ) async {
    var receiptReceived = false;
    try {
      if (!_alive(epoch)) return;
      final reply = await _api.call('agent_receipt', {
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
          AppFailure(code: 'REQUEST_EXPIRED') => 'The original agent receipt has expired. Its outcome remains uncertain; writes are blocked and nothing was resent.',
          AppFailure(code: 'REQUEST_UNKNOWN') => 'The original agent request is unknown. Its outcome remains uncertain; writes are blocked and nothing was resent.',
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
                  '$receiptError The current list is also unavailable. Use contextual Retry to observe again.';
            }
          }
        }
      }
    } finally {
      _finishAction(done, epoch);
    }
  }

  void _receipt(_PendingAgentMutation pending, AppReply reply) {
    if (!projectWireId(reply.instanceId)) {
      throw const AppFailure('PROTOCOL', 'Invalid service instance.');
    }
    correlateAgentReceipt(pending.op, pending.arguments, reply.result);
    final receipt = AgentReceipt.parse(reply.result);
    if (receipt.revision != pending.expectedRevision + 1) {
      throw const AppFailure('PROTOCOL', 'Agent receipt revision mismatch.');
    }
    pending.receipt = receipt;
    _lastReceipt = receipt;
  }

  Future<void> _readList(int epoch, {int? minimumRevision}) async {
    if (!_alive(epoch)) return;
    _loading = true;
    notifyListeners();
    // A listener may dispose the controller while observing loading/busy.
    if (!_alive(epoch)) return;
    try {
      final reply = await _api.call('agents_list_v2', {});
      if (!_alive(epoch)) return;
      if (!projectWireId(reply.instanceId)) {
        throw const AppFailure('PROTOCOL', 'Invalid service instance.');
      }
      final snapshot = AgentSnapshot.parse(reply.result);
      final floor = minimumRevision ?? _pending?.receipt?.revision;
      if ((floor != null && snapshot.revision < floor) ||
          (_revision != null && snapshot.revision < _revision!)) {
        throw const AppFailure('PROTOCOL', 'Stale agent snapshot.');
      }
      final receipt = _pending?.receipt;
      if (receipt != null &&
          !snapshot.agents.any((a) => a.agentId == receipt.agentId)) {
        throw const AppFailure(
          'PROTOCOL',
          'Acknowledged agent missing from current list.',
        );
      }
      if (receipt != null && snapshot.revision == receipt.revision) {
        final saved = snapshot.agents.firstWhere(
          (a) => a.agentId == receipt.agentId,
        );
        final original = _pending!.arguments;
        if (saved.name != original['name'] ||
            saved.description != original['description'] ||
            saved.prompt != original['prompt']) {
          throw const AppFailure(
            'PROTOCOL',
            'Acknowledged agent fields mismatch.',
          );
        }
      }
      _agents = snapshot.agents;
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
      'Agent action outcome is uncertain. Resolve the original receipt before making another change; nothing will be resent.';

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
        'AGENT_NOT_FOUND',
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
    AppFailure(code: 'STALE' || 'STALE_REVISION' || 'REVISION_MISMATCH') => 'Agents changed. Retry loading the list and review your original edits before saving again.',
    AppFailure(code: 'INSTANCE_MISMATCH') => 'Smith restarted. Retry loading Agents and review your original edits; nothing was resent.',
    AppFailure(code: 'AGENT_NOT_FOUND') =>
      'This agent no longer exists. Your edits have not been applied.',
    AppFailure(code: 'CAPACITY' || 'LIMIT') => 'Agent capacity is unavailable. Retry loading the list; your edits have not been applied.',
    AppFailure(
      code: 'PROTOCOL' || 'INVALID_REQUEST' || 'INVALID_ARGUMENT' || 'ARGUMENT',
    ) =>
      'Agent metadata could not be validated. Check the fields and retry loading if needed; your edits are preserved.',
    AppFailure(code: 'UNAVAILABLE') => 'Agents are not ready for changes. Use contextual Retry or resolve the pending action first.',
    _ => 'Agents are unavailable. Use contextual Retry to observe again; no action will be resent.',
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

class _PendingAgentMutation {
  _PendingAgentMutation(this.op, Map<String, dynamic> args)
    : arguments = Map.unmodifiable(args),
      requestId = args['request_id'],
      instanceId = args['instance_id'],
      expectedRevision = args['expected_revision'],
      agentId = args['agent_id'];

  final String op;
  final String requestId;
  final String instanceId;
  final int expectedRevision;
  final String? agentId;
  final Map<String, dynamic> arguments;
  AgentReceipt? receipt;
}
