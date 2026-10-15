import 'dart:async';
import 'dart:convert';
import 'dart:collection';

import 'app_api.dart';
import 'app_v2.dart';
import 'text_chat.dart';

/// Process-local presentation state. The workspace alone owns the shared API.
class ConversationController extends ChatController {
  ConversationController(this.owner) : super(owner.api, ownsApi: false);
  final WorkspaceController owner;
  final String presentationId = appId();
  Map<String, dynamic>? summary;
  int draftRevision = 0;
  int? submittedRevision;
  bool detailLoaded = false;
  bool detailStale = false;
  bool uncertain = false;
  bool observing = false;
  bool _detached = false;
  String? _queuedSend;
  String? expired;
  String? operationStatus;
  String? latestObservedRequestId;
  Map<String, dynamic>? latestObservedOperation;
  String? terminalErrorRequestId;
  late final Stopwatch observation = owner.observationClock();
  String? get id => summary?['conversation_id'] as String?;
  bool get archived => summary?['archived'] == true;
  @override
  bool get canSubmit =>
      !_detached &&
      _queuedSend == null &&
      !detailStale &&
      !archived &&
      (id == null || detailLoaded) &&
      owner._globalRequest == null &&
      !owner.mutations.containsKey(id) &&
      owner.all.where((c) => c.pendingId != null).length < 4 &&
      super.canSubmit;
  @override
  bool get canAbandonExpiredRequest =>
      pendingId != null && pendingId == expired;
  @override
  void edit(String value) {
    draftRevision++;
    super.edit(value);
  }

  void changed() {
    final send = localSends[pendingId];
    if (send != null && !provisionalAnswers.containsKey(pendingId)) {
      send.active = !uncertain && connected && !stale;
      send.status = !send.active
          ? 'Outcome unknown. Retry checks status; nothing is resent.'
          : operationStatus == 'queued'
          ? 'Queued…'
          : operationStatus == 'running'
          ? 'Thinking…'
          : 'Sending…';
    }
    if (!_detached) notifyListeners();
  }

  void reconcileLocalHistory() {
    // History revisions advance only when a completed pair is published. A
    // fresh detail supersedes local presentation, including before poll returns.
    final revision = summary?['history_revision'] as int? ?? 0;
    localSends.removeWhere(
      (request, send) =>
          provisionalAnswers.containsKey(request) ||
          (request == pendingId && revision > send.historyRevision),
    );
    provisionalAnswers.clear();
  }

  @override
  void dispose() {
    _detached = true;
    super.dispose();
  }

  @override
  Future<void> send() => owner.submit(this);
  @override
  Future<void> synchronize() => owner.synchronize();
  @override
  Future<void> refreshModels() => owner.refreshModels();
  @override
  Future<void> configure(String model) => owner.configure(model);
  @override
  void abandonExpiredRequest(String requestId) {
    if (!canAbandonExpiredRequest || pendingId != requestId) return;
    final send = localSends[pendingId];
    if (send != null) {
      send.active = false;
      send.status = 'Request expired. Outcome unknown; nothing was resent.';
    }
    pendingId = pendingKind = expired = null;
    uncertain = false;
    error = 'Expired request forgotten locally. Review saved history; nothing was resent or cancelled.';
    owner.changed();
  }
}

/// A retained presentation slot; only its owner creates or replaces blanks.
class WorkspacePane {
  WorkspacePane._(this.blank) : selected = blank;

  ConversationController selected;
  ConversationController blank;
  int _selectionRevision = 0;
}

class WorkspaceController extends ChatController {
  WorkspaceController(
    super.api, {
    super.pollInterval,
    super.operationBudget,
    this.queueBudget = const Duration(minutes: 10),
    this.observationClock = Stopwatch.new,
  }) {
    final primary = WorkspacePane._(ConversationController(this));
    _panes.add(primary);
    _visiblePanes.add(primary);
  }
  final Duration queueBudget;
  final Stopwatch Function() observationClock;
  final List<WorkspacePane> _panes = [];
  late final List<WorkspacePane> panes = UnmodifiableListView(_panes);
  final Set<WorkspacePane> _visiblePanes = {};
  ConversationController get blank => panes.first.blank;
  set blank(ConversationController value) => panes.first.blank = value;
  ConversationController get selected => panes.first.selected;
  set selected(ConversationController value) => panes.first.selected = value;

  WorkspacePane createPane() {
    if (_closed || panes.length >= 4) {
      throw StateError('Workspace supports at most four retained panes.');
    }
    final pane = WorkspacePane._(ConversationController(this));
    _panes.add(pane);
    changed();
    return pane;
  }

  /// Apply presentation references only after Boot's authoritative index sync.
  Future<bool> restorePaneSelections(
    List<String?> ids,
    int visibleCount,
  ) async {
    while (panes.length < 4) {
      createPane();
    }
    _initialSelection = false;
    var missing = false;
    for (var i = 0; i < 4; i++) {
      final id = ids[i];
      final conversation = conversations[id];
      final pane = panes[i];
      pane._selectionRevision++;
      if (id != null && (conversation == null || conversation.archived)) {
        missing = true;
      }
      pane.selected = conversation != null && !conversation.archived
          ? conversation
          : pane.blank;
    }
    setVisiblePanes(panes.take(visibleCount).toSet());
    changed();
    final loaded = <String>{};
    for (final pane in panes.take(visibleCount)) {
      final id = pane.selected.id;
      if (id != null && loaded.add(id)) await selectFor(pane, id);
      if (_closed) return missing;
    }
    return missing;
  }

  void _checkPane(WorkspacePane pane) {
    if (!_panes.contains(pane)) {
      throw ArgumentError('Pane belongs to another workspace.');
    }
  }

  /// Changes snapshot interest only, without starting observation or work.
  void setVisiblePanes(Set<WorkspacePane> panes) {
    for (final pane in panes) {
      _checkPane(pane);
    }
    _visiblePanes
      ..clear()
      ..addAll(panes);
  }

  Set<ConversationController> get _visibleSelections => {
    for (final pane in _visiblePanes) pane.selected,
  };

  void _repairSelections(ConversationController removed) {
    for (final pane in panes) {
      if (identical(pane.selected, removed)) {
        pane._selectionRevision++;
        pane.selected = pane.blank;
      }
    }
  }

  final Map<String, ConversationController> conversations = {};
  final Map<String, PendingMutation> mutations = {};
  final Set<String> _finished = {};
  bool _closed = false;

  bool _polling = false;
  bool _initialSelection = true;
  Timer? _timer;
  Timer? _connectionTimer;
  bool _connectionObservationStarted = false;
  Future<void> _reconciliation = Future<void>.value();
  Future<void>? _manualSnapshot;
  Future<void>? _automaticSnapshot;

  // Serialize whole reconciliations, not only individual native exchanges: an
  // older state/detail response must not publish over a newer operation result.
  Future<void> _serialize(Future<void> Function() action) {
    final next = _reconciliation.then((_) async {
      if (!_closed) await action();
    });
    _reconciliation = next.then<void>(
      (_) {},
      onError: (Object _, StackTrace _) {},
    );
    return next;
  }

  void startConnectionObservation() {
    if (_closed || _connectionObservationStarted) return;
    _connectionObservationStarted = true;
    _armConnectionObservation();
  }

  void _armConnectionObservation() {
    if (_closed || !_connectionObservationStarted || _connectionTimer != null) {
      return;
    }
    _connectionTimer = Timer(const Duration(seconds: 30), () async {
      _connectionTimer = null;
      await _snapshot(automatic: true);
      _armConnectionObservation();
    });
  }

  Future<void> _snapshot({required bool automatic}) {
    final existing = automatic ? _automaticSnapshot : _manualSnapshot;
    if (existing != null) return existing;
    final next = _serialize(() => _synchronize(automatic: automatic));
    final completed = next.whenComplete(() {
      if (automatic) {
        _automaticSnapshot = null;
      } else {
        _manualSnapshot = null;
      }
    });
    if (automatic) {
      _automaticSnapshot = completed;
    } else {
      _manualSnapshot = completed;
    }
    return completed;
  }

  String? _globalRequest;
  String? _globalKind;
  bool _inventoryAttempted = false;
  late final Stopwatch _globalObservation = observationClock();
  bool get outstanding => all.any((c) => c.pendingId != null);
  Iterable<ConversationController> get all => {
    ...conversations.values,
    for (final pane in panes) pane.blank,
  };
  List<ConversationController> get ordered => conversations.values.toList()
    ..sort(
      (a, b) => (b.summary!['sequence'] as int).compareTo(
        a.summary!['sequence'] as int,
      ),
    );

  void changed() {
    if (_closed) return;
    for (final c in all) {
      c.connected = connected;
      c.stale = stale || c.detailStale;
      c.config = config;
      c.models = models;
      c.providers = providers;
      c.inventoryObserved = inventoryObserved;
      c.instanceId = instanceId;
      c.changed();
    }
    notifyListeners();
  }

  @override
  void unavailable() {
    connected = false;
    stale = true;
    inventoryObserved = false;
    changed();
  }

  void newChat() => newChatFor(panes.first);

  void newChatFor(WorkspacePane pane) {
    _checkPane(pane);
    if (_closed) return;
    if (identical(pane, panes.first)) _initialSelection = false;
    pane._selectionRevision++;
    pane.selected = pane.blank;
    changed();
  }

  Future<void> select(String id) => selectFor(panes.first, id);

  Future<void> selectFor(WorkspacePane pane, String id) {
    _checkPane(pane);
    final c = conversations[id];
    if (_closed || c == null || c.archived) return Future<void>.value();
    if (identical(pane, panes.first)) _initialSelection = false;
    final revision = ++pane._selectionRevision;
    pane.selected = c;
    c.detailStale = true;
    changed();
    return _serialize(() async {
      if (pane._selectionRevision != revision ||
          !identical(pane.selected, c) ||
          !identical(conversations[id], c) ||
          c.archived) {
        return;
      }
      try {
        await _detail(c);
      } on Object catch (e) {
        _observeFailure(e);
        c.error = '$e';
        stale = true;
      }
      changed();
    });
  }

  void _correlate(AppReply r) {
    if (r.instanceId != instanceId) {
      stale = true;
      inventoryObserved = false;
      throw const AppFailure(
        'RESTART',
        'Service changed. Observe again; never resend.',
        unknown: true,
      );
    }
  }

  void _observeFailure(Object e) {
    if (e is AppFailure &&
        [
          'TRANSPORT',
          'PROTOCOL',
          'RESTART',
          'INSTANCE_MISMATCH',
          'STATE_UNAVAILABLE',
        ].contains(e.code)) {
      stale = true;
      if (['RESTART', 'INSTANCE_MISMATCH'].contains(e.code)) {
        inventoryObserved = false;
      }
    }
  }

  void _inventory(Map<String, dynamic> value) {
    inventory(value);
    models = (value['models'] as List).map((m) => m['name'] as String).toList();
    providers = Map<String, dynamic>.from(value['providers']);
    inventoryObserved = true;
  }

  ConversationController _summary(Map<String, dynamic> s) {
    final id = s['conversation_id'] as String;
    var c = conversations[id];
    if (c == null) {
      for (final pane in panes) {
        if (pane.blank.pendingId != null &&
            s['origin_request_id'] == pane.blank.pendingId) {
          c = pane.blank;
          pane.blank = ConversationController(this);
          break;
        }
      }
    }
    c ??= ConversationController(this);
    if (c.detailLoaded &&
        c.summary?['history_revision'] != s['history_revision']) {
      c.detailStale = true;
    }
    c.summary = Map.of(s);
    conversations[id] = c;
    return c;
  }

  Future<void> _detail(ConversationController c) async {
    final id = c.id;
    if (id == null) return;
    final r = await api.call('conversation_get', {'conversation_id': id});
    if (_closed) return;
    _correlate(r);
    validateV2('conversation_get', r.result);
    if (r.result['conversation']['conversation_id'] != id) invalid();
    c.summary = Map<String, dynamic>.from(r.result['conversation']);
    c.history = (r.result['history'] as List)
        .map((m) => Map<String, dynamic>.from(m))
        .toList();
    c.detailLoaded = true;
    c.detailStale = false;
    if (c.provisionalAnswers.isNotEmpty && c.pendingId == null) c.error = null;
    c.reconcileLocalHistory();
  }

  @override
  Future<void> synchronize() => _snapshot(automatic: false);

  Future<void> _synchronize({required bool automatic}) async {
    if (_closed) return;
    if (!automatic) {
      for (final c in all.where((c) => c.pendingId != null)) {
        c.observation.reset();
        c.expired = null;
      }
      for (final m in mutations.values) {
        m.clock.reset();
      }
      _globalObservation.reset();
    }
    try {
      final r = await api.call('state', {});
      if (_closed) return;
      validateV2('state', r.result);
      if (instanceId != r.instanceId) {
        inventoryObserved = false;
        models = [];
        providers = {};
        if (!automatic) _inventoryAttempted = false;
        for (final c in conversations.values) {
          c.detailStale = true;
        }
      }
      instanceId = r.instanceId;
      config = Map<String, dynamic>.from(r.result['config']);
      inventoryObserved = r.result['inventory_observed'] == true;
      if (inventoryObserved) {
        _inventory(Map<String, dynamic>.from(r.result['inventory']));
      }
      final present = <String>{};
      for (final s in r.result['conversations']) {
        final c = _summary(Map<String, dynamic>.from(s));
        present.add(c.id!);
      }
      for (final id in conversations.keys.toList()) {
        if (!present.contains(id)) {
          final c = conversations[id]!;
          if (automatic && (c.pendingId != null || mutations.containsKey(id))) {
            c.detailStale = true;
            continue;
          }
          final removed = conversations.remove(id)!;
          _repairSelections(removed);
          removed.dispose();
        }
      }
      if (_initialSelection) {
        _initialSelection = false;
        selected = ordered.where((c) => !c.archived).firstOrNull ?? blank;
      }
      for (final c in conversations.values.where((c) => c.archived)) {
        _repairSelections(c);
      }
      final detailFailures = <ConversationController, Object>{};
      for (final c in _visibleSelections) {
        if (c.id == null || (automatic && c.detailLoaded && !c.detailStale)) {
          continue;
        }
        try {
          await _detail(c);
        } on Object catch (e) {
          _observeFailure(e);
          c.detailStale = true;
          detailFailures[c] = e;
        }
        if (_closed) return;
      }
      if (_closed) return;
      final jobs = List<Map<String, dynamic>>.from(
        (r.result['operations'] as List).reversed,
      );
      // The service retains jobs in admission order. Inspect the newest submit
      // per conversation, including successes, so an old failure cannot replace
      // a newer outcome. Local pending IDs still resolve their original receipt.
      final latest = <String, Map<String, dynamic>>{};
      for (final o in jobs) {
        if (['submit', 'submit_new'].contains(o['kind'])) {
          latest.putIfAbsent(o['conversation_id'] as String, () => o);
        }
      }
      for (final c in conversations.values) {
        final o = latest[c.id];
        if (o == null) continue;
        c.latestObservedRequestId = o['request_id'];
        c.latestObservedOperation = Map.of(o);
        if (c.terminalErrorRequestId != null &&
            c.terminalErrorRequestId != o['request_id'] &&
            o['status'] == 'succeeded') {
          c.error = null;
          c.terminalErrorRequestId = null;
          c.operationStatus = 'succeeded';
        }
      }
      for (final o in jobs) {
        if (_finished.contains(o['request_id'])) continue;
        final c = conversations[o['conversation_id']];
        if (c != null && ['submit', 'submit_new'].contains(o['kind'])) {
          if (c.pendingId == null &&
              (latest[c.id]?['request_id'] != o['request_id'] ||
                  (o['status'] == 'succeeded' &&
                      !detailFailures.containsKey(c)))) {
            continue;
          }
          if (c.pendingId != null && c.pendingId != o['request_id']) continue;
          if (c.pendingId == null) c.observation.reset();
          c.pendingId = o['request_id'];
          c.pendingKind = o['kind'];
          c.phase = o['phase'];
          c.operationStatus = o['status'];
          if (!c.observation.isRunning) c.observation.start();
        } else if (o['conversation_id'] == null &&
            (_globalRequest == null || _globalRequest == o['request_id']) &&
            ['queued', 'running'].contains(o['status'])) {
          if (_globalRequest == null) _globalObservation.reset();
          _globalRequest = o['request_id'];
          _globalKind = o['kind'];
          _globalObservation.start();
        }
      }
      stale = detailFailures.isNotEmpty;
      connected = true;
      for (final c in _visibleSelections) {
        if (c.error == error) c.error = null;
      }
      error = null;
      // One observation per known identity, never wait for generation at Boot.
      await _observeOnce();
      for (final entry in detailFailures.entries) {
        error ??= '${entry.value}';
        entry.key.error = '${entry.value}';
      }
    } on Object catch (e) {
      _observeFailure(e);
      stale = true;
      error = '$e';
      for (final c in _visibleSelections) {
        c.error = error;
      }
    } finally {
      changed();
      _schedule();
    }
  }

  void _schedule() {
    if (_closed || _timer != null) return;
    final budget = queueBudget + operationBudget;
    var expired = false;
    for (final c in all) {
      if (c.pendingId != null &&
          c.observation.elapsed > budget &&
          !c.uncertain) {
        c.uncertain = true;
        c.error = 'Observation budget elapsed. Retry queries only; work may continue.';
        expired = true;
      }
    }
    if (_globalRequest != null &&
        _globalObservation.elapsed >= operationBudget &&
        error == null) {
      error = 'Global observation budget elapsed. Retry queries only; work may continue.';
      expired = true;
    }
    if (expired) changed();
    if (!all.any(
          (c) =>
              c.pendingId != null &&
              c.expired == null &&
              c.observation.elapsed <= budget,
        ) &&
        !mutations.values.any((m) => m.clock.elapsed <= budget) &&
        !(_globalRequest != null &&
            _globalObservation.elapsed < operationBudget)) {
      return;
    }
    _timer = Timer(pollInterval, () async {
      _timer = null;
      await _serialize(() async {
        await _observeOnce();
        if (!_connectionObservationStarted &&
            !outstanding &&
            mutations.isEmpty &&
            !inventoryObserved &&
            !_inventoryAttempted &&
            !stale &&
            _globalRequest == null) {
          await _refreshModels();
        }
        changed();
      });
      _schedule();
    });
  }

  Future<void> _observeOnce() async {
    if (_polling || _closed || instanceId == null) return;
    _polling = true;
    try {
      for (final c
          in all
              .where((c) => c.pendingId != null && c.expired == null)
              .take(4)
              .toList()) {
        if (c.uncertain &&
            c.observation.elapsed > queueBudget + operationBudget) {
          continue;
        }
        if (c.observation.elapsed > queueBudget + operationBudget) {
          c.uncertain = true;
          c.error = 'Observation budget elapsed. Retry queries only; work may continue.';
          continue;
        }
        if (_closed) return;
        try {
          await _pollChat(c);
        } on Object catch (e) {
          _observeFailure(e);
          c.uncertain = true;
          c.error = '$e';
          if (e is AppFailure && e.code == 'REQUEST_EXPIRED') {
            c.expired = c.pendingId;
          }
        }
      }
      if (_closed) return;
      for (final m in mutations.values.toList()) {
        if (_closed) return;
        if (m.inFlight) continue;
        if (m.clock.elapsed > queueBudget + operationBudget) {
          continue;
        }
        try {
          final r = await api.call('poll', {
            'request_id': m.request,
            'instance_id': instanceId,
          });
          if (_closed) return;
          _correlate(r);
          validateV2('poll', r.result);
          if (r.result['request_id'] != m.request ||
              r.result['conversation_id'] != m.id ||
              r.result['kind'] != m.op) {
            invalid();
          }
          if (r.result['status'] == 'succeeded') {
            _applyMutation(m, Map<String, dynamic>.from(r.result['result']));
          } else if (['failed', 'interrupted'].contains(r.result['status'])) {
            mutations.remove(m.id);
            conversations[m.id]?.error = '${r.result['error']}';
          }
        } on Object catch (e) {
          _observeFailure(e);
          conversations[m.id]?.error = 'Mutation outcome unknown: $e';
        }
      }
      if (!_closed &&
          _globalRequest != null &&
          _globalObservation.elapsed < operationBudget) {
        final r = await api.call('poll', {
          'request_id': _globalRequest,
          'instance_id': instanceId,
        });
        if (_closed) return;
        _correlate(r);
        validateV2('poll', r.result);
        if (r.result['request_id'] != _globalRequest ||
            r.result['kind'] != _globalKind ||
            r.result['conversation_id'] != null) {
          invalid();
        }
        if (!['running', 'queued'].contains(r.result['status'])) {
          _globalRequest = null;
          _globalKind = null;
          if (r.result['kind'] == 'models' &&
              r.result['status'] == 'succeeded') {
            _inventory(Map<String, dynamic>.from(r.result['result']));
          }
        }
      }
    } on Object catch (e) {
      _observeFailure(e);
      error = '$e';
    } finally {
      _polling = false;
    }
  }

  Future<void> _pollChat(ConversationController c) async {
    if (c.observing || c.pendingId == null) return;
    c.observing = true;
    try {
      await _readChatPoll(c);
    } finally {
      c.observing = false;
    }
  }

  Future<void> _readChatPoll(ConversationController c) async {
    final request = c.pendingId!;
    final r = await api.call('poll', {
      'request_id': request,
      'instance_id': instanceId,
    });
    if (_closed) return;
    _correlate(r);
    validateV2('poll', r.result);
    final o = r.result;
    if (o['request_id'] != request ||
        o['kind'] != c.pendingKind ||
        (c.id != null && o['conversation_id'] != c.id)) {
      invalid();
    }
    final newer = c.latestObservedOperation;
    if (!['queued', 'running'].contains(o['status']) &&
        newer != null &&
        newer['request_id'] != request) {
      // Resolve the captured old identity without publishing its outcome over
      // newer authoritative work. Observe that newer work next, never replay it.
      _finished.add(request);
      if (_finished.length > 32) _finished.remove(_finished.first);
      c.localSends.remove(request);
      c.provisionalAnswers.remove(request);
      c.pendingId = c.pendingKind = c.expired = null;
      c.submittedRevision = null;
      c.uncertain = false;
      c.error = null;
      c.operationStatus = newer['status'];
      c.phase = 'Ready';
      if (newer['status'] != 'succeeded' || c.detailStale) {
        c.pendingId = newer['request_id'];
        c.pendingKind = newer['kind'];
        c.phase = newer['phase'];
        c.observation.reset();
        c.observation.start();
      }
      return;
    }
    if (o['status'] == 'succeeded') {
      c.provisionalAnswers[request] = o['result']['assistant_text'];
      c.localSends[request]?.active = false;
      c.submittedRevision = null;
    }
    if (c.id == null) {
      // Promotion needs authoritative metadata, not an invented local row.
      final detail = await api.call('conversation_get', {
        'conversation_id': o['conversation_id'],
      });
      if (_closed) return;
      _correlate(detail);
      validateV2('conversation_get', detail.result);
      if (detail.result['conversation']['conversation_id'] !=
              o['conversation_id'] ||
          detail.result['conversation']['origin_request_id'] != request) {
        invalid();
      }
      _summary(Map<String, dynamic>.from(detail.result['conversation']));
      c.history = (detail.result['history'] as List)
          .map((m) => Map<String, dynamic>.from(m))
          .toList();
      c.detailLoaded = true;
      c.detailStale = false;
      c.reconcileLocalHistory();
    }
    c.phase = o['phase'];
    c.operationStatus = o['status'];
    c.uncertain = false;
    c.error = null;
    if (['queued', 'running'].contains(o['status'])) return;
    _finished.add(request);
    if (_finished.length > 32) _finished.remove(_finished.first);
    c.pendingId = c.pendingKind = c.expired = null;
    if (o['status'] == 'succeeded') {
      c.changed();
      try {
        await _detail(c);
      } on Object catch (e) {
        _observeFailure(e);
        c.error = 'Answer received; saved history unavailable: $e';
        c.detailStale = true;
      }
    } else if (c.latestObservedRequestId == null ||
        c.latestObservedRequestId == request) {
      c.error = '${o['error']}';
      final send = c.localSends[request];
      if (send != null) {
        send.active = false;
        send.status =
            '${o['status'] == 'interrupted' ? 'Interrupted' : 'Failed'}. ${o['error']} Nothing was resent.';
      }
      c.terminalErrorRequestId = request;
    }
    c.phase = 'Ready';
  }

  @override
  Future<void> refreshModels() => _serialize(_refreshModels);

  Future<void> _refreshModels() async {
    if (_closed || stale || outstanding || _globalRequest != null) return;
    _inventoryAttempted = true;
    _globalRequest = appId();
    _globalKind = 'models';
    _globalObservation.reset();
    _globalObservation.start();
    try {
      final r = await api.call('models', {'request_id': _globalRequest});
      if (_closed) return;
      _correlate(r);
      validateV2('models', r.result);
      if (r.result['request_id'] != _globalRequest ||
          r.result['kind'] != 'models' ||
          r.result['conversation_id'] != null) {
        invalid();
      }
      await _observeOnce();
    } on Object catch (e) {
      _observeFailure(e);
      error = '$e';
      if (e is AppFailure &&
          !e.unknown &&
          ![
            'PROTOCOL',
            'TRANSPORT',
            'RESTART',
            'INSTANCE_MISMATCH',
          ].contains(e.code)) {
        _globalRequest = _globalKind = null;
      }
    }
    changed();
    _schedule();
  }

  @override
  Future<void> configure(String model) => _serialize(() => _configure(model));

  Future<void> _configure(String model) async {
    if (_closed ||
        stale ||
        !connected ||
        outstanding ||
        mutations.isNotEmpty ||
        _globalRequest != null ||
        !inventoryObserved ||
        !models.contains(model)) {
      return;
    }
    try {
      final r = await api.call('configure', {'model': model, 'voice': null});
      if (_closed) return;
      _correlate(r);
      validateV2('configure', r.result);
      await _synchronize(automatic: false);
    } on Object catch (e) {
      _observeFailure(e);
      stale = true;
      error = '$e';
      changed();
    }
  }

  Future<void> submit(ConversationController c) {
    if (_closed || !c.canSend) return Future<void>.value();
    final text = c.draft;
    if (utf8.encode(text).length > 16384) {
      c.error = 'Draft exceeds 16 KiB.';
      changed();
      return Future<void>.value();
    }
    final request = appId();
    final intent = _SendIntent(
      request: request,
      text: text,
      draftRevision: c.draftRevision,
      instance: instanceId,
      conversation: c.id,
      configRevision: config['revision'],
      historyRevision: c.summary?['history_revision'] as int?,
    );
    c._queuedSend = request;
    c.localSends[request] = LocalTextSend(
      text,
      historyRevision: c.summary?['history_revision'] as int? ?? 0,
    );
    c.busy = true;
    c.error = null;
    c.edit('');
    changed();
    return _serialize(() async {
      c._queuedSend = null;
      c.busy = false;
      await _submit(c, intent);
    }).whenComplete(() {
      // Also release the local reservation when disposal fences queued work.
      if (c._queuedSend == request) {
        c._queuedSend = null;
        c.busy = false;
      }
    });
  }

  Future<void> _submit(ConversationController c, _SendIntent intent) async {
    final request = intent.request;
    if (_closed ||
        !c.canSubmit ||
        instanceId != intent.instance ||
        c.id != intent.conversation ||
        config['revision'] != intent.configRevision ||
        c.summary?['history_revision'] != intent.historyRevision ||
        (c.id == null
            ? !panes.any((pane) => identical(c, pane.blank))
            : !identical(conversations[c.id], c)) ||
        stale ||
        mutations.containsKey(c.id) ||
        _globalRequest != null) {
      final send = c.localSends[request]!;
      send.active = false;
      send.status = 'Not sent. Readiness changed before submission.';
      c.changed();
      return;
    }
    final op = intent.conversation == null ? 'submit_new' : 'submit';
    c.pendingId = request;
    c.pendingKind = op;
    c.submittedRevision = intent.draftRevision;
    c.phase = 'Submitting';
    c.operationStatus = null;
    c.uncertain = false;
    c.error = null;
    c.terminalErrorRequestId = null;
    c.latestObservedRequestId = request;
    c.latestObservedOperation = null;
    c.observation.reset();
    c.observation.start();
    changed();
    var admissionObserved = false;
    try {
      final r = await api.call(op, {
        'request_id': request,
        if (intent.conversation != null) 'conversation_id': intent.conversation,
        'text': intent.text,
        'output': 'chat',
        'expected_config_revision': intent.configRevision,
        if (intent.conversation != null)
          'expected_history_revision': intent.historyRevision,
      });
      if (_closed) return;
      _correlate(r);
      validateV2(op, r.result);
      if (r.result['request_id'] != request ||
          r.result['kind'] != op ||
          (c.id != null && r.result['conversation_id'] != c.id)) {
        invalid();
      }
      admissionObserved = true;
      c.phase = r.result['phase'];
      c.operationStatus = r.result['status'];
      changed();
      await _pollChat(c);
    } on Object catch (e) {
      _observeFailure(e);
      c.error = '$e';
      c.uncertain = true;
      if (admissionObserved) stale = true;
      // Only a definitive failure of the admission exchange can release this
      // identity. Once accepted, even a definitive poll error is query-only.
      if (!admissionObserved &&
          e is AppFailure &&
          !e.unknown &&
          ![
            'PROTOCOL',
            'TRANSPORT',
            'RESTART',
            'INSTANCE_MISMATCH',
            'NOT_FOUND',
            'REQUEST_EXPIRED',
          ].contains(e.code)) {
        c.pendingId = c.pendingKind = null;
        c.uncertain = false;
        final send = c.localSends[request]!;
        send.active = false;
        send.status = 'Not sent. $e Nothing was resent.';
      }
    }
    changed();
    _schedule();
  }

  Future<void> mutate(
    String op,
    String id,
    int metadata,
    int history, {
    String? title,
  }) => _serialize(() => _mutate(op, id, metadata, history, title: title));

  Future<void> _mutate(
    String op,
    String id,
    int metadata,
    int history, {
    String? title,
  }) async {
    final c = conversations[id];
    if (_closed ||
        stale ||
        !connected ||
        c == null ||
        mutations.containsKey(id) ||
        c.summary!['metadata_revision'] != metadata ||
        (op == 'conversation_delete' &&
            c.summary!['history_revision'] != history) ||
        (op != 'conversation_rename' && c.pendingId != null)) {
      throw const AppFailure(
        'STALE',
        'Chat changed or action unavailable. Reopen the action.',
      );
    }
    final m = PendingMutation(appId(), op, id, observationClock());
    final args = <String, dynamic>{
      'request_id': m.request,
      'conversation_id': id,
      'expected_metadata_revision': metadata,
      if (op == 'conversation_delete') 'expected_history_revision': history,
      'title': ?title,
    };
    validateV2Arguments(op, args);
    mutations[id] = m;
    changed();
    try {
      final r = await api.call(op, args);
      if (_closed) return;
      _correlate(r);
      validateV2(op, r.result);
      _applyMutation(m, r.result);
    } on Object catch (e) {
      _observeFailure(e);
      c.error = 'Action not confirmed: $e';
      if (e is AppFailure &&
          !e.unknown &&
          ![
            'PROTOCOL',
            'TRANSPORT',
            'RESTART',
            'INSTANCE_MISMATCH',
          ].contains(e.code)) {
        mutations.remove(id);
      }
      rethrow;
    } finally {
      m.inFlight = false;
      changed();
      _schedule();
    }
  }

  void _applyMutation(PendingMutation m, Map<String, dynamic> r) {
    if (r['request_id'] != m.request ||
        (r['conversation_id'] ?? r['conversation']['conversation_id']) !=
            m.id) {
      invalid();
    }
    mutations.remove(m.id);
    if (m.op == 'conversation_delete') {
      final removed = conversations.remove(m.id);
      if (removed != null) {
        _repairSelections(removed);
        removed.dispose();
      }
    } else {
      final current = conversations[m.id];
      // A receipt proves the old action, not present membership or metadata.
      // Never resurrect a target absent from a newer authoritative index.
      if (current == null) return;
      final receipt = Map<String, dynamic>.from(r['conversation']);
      final saved = current.summary!;
      final merged = Map<String, dynamic>.of(saved);
      if (receipt['metadata_revision'] > saved['metadata_revision']) {
        for (final key in ['title', 'archived', 'metadata_revision']) {
          merged[key] = receipt[key];
        }
      }
      if (receipt['history_revision'] > saved['history_revision']) {
        merged['history_revision'] = receipt['history_revision'];
      }
      final c = _summary(merged);
      if (c.archived) _repairSelections(c);
    }
  }

  @override
  void dispose() {
    _closed = true;
    _timer?.cancel();
    _connectionTimer?.cancel();
    for (final c in all) {
      c.dispose();
    }
    super.dispose();
  }
}

/// The authority and payload of one explicit Send, captured before queueing.
class _SendIntent {
  const _SendIntent({
    required this.request,
    required this.text,
    required this.draftRevision,
    required this.instance,
    required this.conversation,
    required this.configRevision,
    required this.historyRevision,
  });
  final String request, text;
  final int draftRevision;
  final String? instance, conversation;
  final Object? configRevision;
  final int? historyRevision;
}

class PendingMutation {
  PendingMutation(this.request, this.op, this.id, this.clock) {
    clock.start();
  }
  final String request, op, id;
  bool inFlight = true;
  final Stopwatch clock;
}
