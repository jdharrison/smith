import 'dart:convert';

import 'package:flutter/material.dart';

import 'app_api.dart';

class ChatController extends ChangeNotifier {
  ChatController(
    this.api, {
    this.pollInterval = const Duration(milliseconds: 500),
    this.operationBudget = const Duration(seconds: 210),
  });
  final AppApi api;
  final Duration pollInterval;
  final Duration operationBudget;
  String draft = '';
  int _draftRevision = 0;
  bool busy = false;
  bool _closed = false;
  String phase = 'Ready';
  bool stale = true;
  bool connected = true;
  bool get canSend =>
      connected &&
      !stale &&
      !busy &&
      pendingId == null &&
      inventoryObserved &&
      models.contains(config['model']) &&
      providers['llm'] is Map &&
      providers['llm']['ready'] == true &&
      draft.trim().isNotEmpty;

  void unavailable() {
    connected = false;
    stale = true;
    inventoryObserved = false;
    _notify();
  }

  bool inventoryObserved = false;
  String? error;
  String? instanceId;
  String? pendingId;
  String? pendingKind;
  String? _expiredRequestId;
  bool get canAbandonExpiredRequest =>
      pendingId != null && pendingId == _expiredRequestId;
  int? _submittedRevision;
  Map<String, dynamic> config = {};
  Map<String, dynamic> providers = {};
  List<Map<String, dynamic>> history = [];
  // Verified terminal answers are not service history and may lack user text.
  // Retain them by request identity until a later state read succeeds.
  final Map<String, String> provisionalAnswers = {};
  List<String> models = [];
  void _notify() {
    if (!_closed) notifyListeners();
  }

  void edit(String value) {
    draft = value;
    _draftRevision++;
    _notify();
  }

  void abandonExpiredRequest(String requestId) {
    if (_closed ||
        busy ||
        !canAbandonExpiredRequest ||
        pendingId != requestId) {
      return;
    }
    pendingId = null;
    pendingKind = null;
    _expiredRequestId = null;
    _submittedRevision = null;
    phase = 'Ready';
    error = 'Expired request forgotten locally. Its outcome may be unknown; review saved history before sending new work. Nothing was resent or cancelled.';
    _notify();
  }

  Future<void> synchronize() async {
    if (busy || _closed) return;
    busy = true;
    error = null;
    _notify();
    try {
      final reply = await api.call('state', {});
      if (_closed) return;
      final old = instanceId;
      instanceId = reply.instanceId;
      _readState(reply.result);
      stale = false;
      if (old != null && old != instanceId && pendingId != null) {
        error = 'Service restarted. Querying saved work; nothing is resent.';
      }
      final active = reply.result['active'];
      if (pendingId == null && active is Map<String, dynamic>) {
        pendingId = active['request_id'] as String?;
        pendingKind = active['kind'] as String?;
      }
      if (pendingId != null) await _poll();
    } on Object catch (e) {
      stale = true;
      error = e.toString();
    } finally {
      busy = false;
      _notify();
    }
  }

  void _readState(Map<String, dynamic> state) {
    final savedConfig = Map<String, dynamic>.from(state['config'] as Map);
    final savedHistory = (state['history'] as List)
        .map((v) => Map<String, dynamic>.from(v as Map))
        .toList();
    final savedProviders = Map<String, dynamic>.from(state['providers'] as Map);
    config = savedConfig;
    history = savedHistory;
    providers = savedProviders;
    provisionalAnswers.clear();
  }

  Future<void> configure(String model) async {
    if (busy ||
        pendingId != null ||
        _closed ||
        !connected ||
        stale ||
        !inventoryObserved ||
        !models.contains(model)) {
      return;
    }
    busy = true;
    error = null;
    _notify();
    try {
      final reply = await api.call('configure', {
        'model': model,
        'voice': null,
      });
      _correlate(reply);
      if (!_closed) {
        final state = await api.call('state', {});
        if (_closed) return;
        _correlate(state);
        _readState(state.result);
      }
    } on Object catch (e) {
      stale = true;
      error = e.toString();
    } finally {
      busy = false;
      _notify();
    }
  }

  void _correlate(AppReply reply) {
    if (instanceId != null && reply.instanceId != instanceId) {
      throw const AppFailure(
        'RESTART',
        'Service instance changed; synchronize, never resend.',
      );
    }
    instanceId = reply.instanceId;
  }

  Future<void> refreshModels() {
    inventoryObserved = false;
    return _operate('models', {});
  }

  Future<void> send() async {
    if (!canSend || _closed) {
      return;
    }
    if (utf8.encode(draft).length > 16384) {
      error = 'Draft exceeds 16 KiB.';
      _notify();
      return;
    }
    await _operate('submit', {'text': draft, 'output': 'chat'});
  }

  Future<void> _operate(String kind, Map<String, dynamic> args) async {
    if (busy || pendingId != null || _closed) return;
    if (instanceId == null) {
      error = 'Synchronize service state first.';
      _notify();
      return;
    }
    busy = true;
    error = null;
    phase = kind;
    pendingId = appId();
    pendingKind = kind;
    _expiredRequestId = null;
    if (kind == 'submit') {
      _submittedRevision = _draftRevision;
    }
    _notify();
    try {
      final reply = await api.call(kind, {'request_id': pendingId, ...args});
      if (_closed) return;
      _correlate(reply);
      if (reply.result['request_id'] != pendingId) {
        throw const AppFailure('PROTOCOL', 'Admission request mismatch.');
      }
      await _poll();
    } on AppFailure catch (e) {
      stale = true;
      error = e.toString();
      if (e.code == 'REQUEST_EXPIRED') {
        _expiredRequestId = pendingId;
        phase = 'Request expired · resolution required';
      }
      // A definitive rejected admission is not outstanding work. A transport,
      // malformed acknowledgement or changed instance remains query-only.
      if (!e.unknown &&
          ![
            'PROTOCOL',
            'TRANSPORT',
            'RESTART',
            'INSTANCE_MISMATCH',
            'NOT_FOUND',
            'REQUEST_EXPIRED',
          ].contains(e.code)) {
        pendingId = null;
        pendingKind = null;
        _submittedRevision = null;
      }
    } on Object {
      stale = true;
      error = 'Acknowledgement unknown. Synchronize/query this request; it will not be resent.';
    } finally {
      busy = false;
      _notify();
    }
  }

  Future<void> _poll() async {
    final deadline = DateTime.now().add(operationBudget);
    while (!_closed && pendingId != null) {
      if (DateTime.now().isAfter(deadline)) {
        throw const AppFailure(
          'TIMEOUT',
          'Polling budget elapsed; operation may continue. Synchronize to query, not resend.',
          unknown: true,
        );
      }
      final AppReply reply;
      try {
        reply = await api.call('poll', {
          'request_id': pendingId,
          'instance_id': instanceId,
        });
      } on AppFailure catch (e) {
        if (!_closed && e.code == 'REQUEST_EXPIRED') {
          _expiredRequestId = pendingId;
          phase = 'Request expired · resolution required';
        }
        rethrow;
      }
      if (_closed) return;
      _correlate(reply);
      final job = reply.result;
      if (job['request_id'] != pendingId ||
          job['kind'] != pendingKind ||
          ![
            'running',
            'succeeded',
            'failed',
            'interrupted',
          ].contains(job['status'])) {
        throw const AppFailure(
          'PROTOCOL',
          'Operation correlation/status mismatch.',
        );
      }
      phase = job['phase'] as String;
      _notify();
      if (job['status'] == 'running') {
        await Future<void>.delayed(pollInterval);
        continue;
      }
      final kind = pendingKind;
      final requestId = pendingId!;
      // Validate terminal data before forgetting the correlation identity.
      // Malformed results remain query-only, even after a success status.
      if (job['status'] == 'succeeded') {
        final terminal = Map<String, dynamic>.from(job['result'] as Map);
        if (kind == 'submit' && terminal['assistant_text'] is! String) {
          throw const AppFailure('PROTOCOL', 'Invalid text result.');
        }
        if (kind == 'models') {
          (terminal['models'] as List).map((v) => v['name'] as String).toList();
          Map<String, dynamic>.from(terminal['providers'] as Map);
        }
      }
      pendingId = null;
      pendingKind = null;
      _expiredRequestId = null;
      if (job['status'] != 'succeeded') {
        _submittedRevision = null;
        throw AppFailure(
          'OPERATION',
          job['error']?.toString() ??
              'Operation interrupted. No automatic retry.',
        );
      }
      final result = Map<String, dynamic>.from(job['result'] as Map);
      if (kind == 'models') {
        inventoryObserved = true;
        models = (result['models'] as List)
            .map((v) => v['name'] as String)
            .toList();
        providers = Map<String, dynamic>.from(result['providers'] as Map);
      } else if (kind == 'submit') {
        provisionalAnswers[requestId] = result['assistant_text'] as String;
        if (_submittedRevision == _draftRevision) edit('');
        _submittedRevision = null;
        _notify();
        // Only state can replace history. A failed read leaves the verified
        // answer available separately, including for work adopted on reattach.
        final state = await api.call('state', {});
        if (_closed) return;
        _correlate(state);
        _readState(state.result);
        stale = false;
        _notify();
      }
      phase = 'Ready';
      _notify();
      return;
    }
  }

  @override
  void dispose() {
    _closed = true;
    api.dispose();
    super.dispose();
  }
}
