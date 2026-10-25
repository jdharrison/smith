import 'dart:async';
import 'dart:convert';
import 'dart:collection';

import 'app_api.dart';
import 'app_v2.dart';
import 'text_chat.dart';
import 'message_queue.dart';
import 'cloud_routing.dart';
import 'message_timestamps.dart';

part 'workspace_cloud.dart';

/// Process-local presentation state. The workspace alone owns the shared API.
class ConversationController extends ChatController {
  ConversationController(this.owner) : super(owner.api, ownsApi: false);
  final WorkspaceController owner;
  ChatRoute? selectedRoute;
  bool routeChosen = false;
  int routeRevision = 0;
  bool preparingRoute = false;
  final Map<String, RequestRouting> requestRoutes = {};
  final Map<String, ChatRoute> submittedRoutes = {};
  ConversationRouting? routing;
  bool get routeReady => owner.routeCatalog?.available(selectedRoute) == true;
  void selectRoute(ChatRoute route) {
    selectedRoute = route;
    routeChosen = true;
    routeRevision++;
    if (_queue.retained) {
      _queue.latch(
        'Route changed. Resume explicitly to rebind unsent messages to ${route.label}.',
      );
    }
    owner.changed();
  }

  Future<void> sendWithConsent(ConfirmTransfer confirm) =>
      owner.submit(this, confirm: confirm);
  late final _ConversationQueue _queue = _ConversationQueue(this);
  @override
  MessageQueue get messageQueue => _queue;
  bool _missing = false;
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
  // Base revision from the same atomic state snapshot as the newest operation,
  // not from a later detail read that may already include its completed pair.
  String? _observedHistoryRequest;
  int? _observedHistoryBase;
  String? terminalErrorRequestId;
  late final Stopwatch observation = owner.observationClock();
  String? get id => summary?['conversation_id'] as String?;
  bool get archived => summary?['archived'] == true;
  @override
  bool get canSubmit =>
      !_detached &&
      !_missing &&
      !owner.queueExitPending &&
      (!_queue.blocksSend || _queue.dispatching) &&
      _queuedSend == null &&
      !detailStale &&
      !archived &&
      (id == null || detailLoaded) &&
      owner._globalRequest == null &&
      !owner.mutations.containsKey(id) &&
      owner.all.where((c) => c.pendingId != null).length < 4 &&
      (owner.routingEnabled
          ? connected &&
                !stale &&
                !busy &&
                !preparingRoute &&
                pendingId == null &&
                routeReady
          : super.canSubmit);
  @override
  bool get canAbandonExpiredRequest =>
      pendingId != null && pendingId == expired;
  @override
  void edit(String value) {
    draftRevision++;
    super.edit(value);
  }

  void changed() {
    error ??= _queue._lastSkipNotice;
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
    bool? routingEnabled,
    bool? timestampsEnabled,
  }) : routingEnabled = routingEnabled ?? api is NativeAppApi,
       timestampsEnabled = timestampsEnabled ?? api is NativeAppApi {
    final primary = WorkspacePane._(ConversationController(this));
    _panes.add(primary);
    _visiblePanes.add(primary);
  }
  final bool routingEnabled;
  final bool timestampsEnabled;
  RouteCatalog? routeCatalog;
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
  QueueExitHold? _exitHold;
  bool get queueExitPending => _exitHold != null;

  QueueExitHold acquireQueueExitHold() {
    if (_exitHold != null) return _exitHold!;
    final hold = QueueExitHold(
      messages: all.fold(0, (n, c) => n + c._queue.unsentCount),
      drafts: all.where((c) => c.draft.isNotEmpty).length,
    );
    _exitHold = hold;
    changed();
    return hold;
  }

  void releaseQueueExitHold(QueueExitHold hold) {
    if (!identical(_exitHold, hold)) return;
    _exitHold = null;
    changed();
  }

  bool _pumpScheduled = false;
  int _pumpCursor = 0;
  void _scheduleQueuePump() {
    if (_closed ||
        queueExitPending ||
        _pumpScheduled ||
        !all.any((c) => c._queue.readyToPump)) {
      return;
    }
    _pumpScheduled = true;
    unawaited(
      _serialize(() async {
        final candidates = all.toList();
        if (candidates.isEmpty) return;
        final start = _pumpCursor % candidates.length;
        for (var i = 0; i < candidates.length; i++) {
          if (_closed || queueExitPending) break;
          final c = candidates[(start + i) % candidates.length];
          if (c._queue.readyToPump) await c._queue.pump();
        }
        _pumpCursor = (start + 1) % candidates.length;
      }).whenComplete(() {
        _pumpScheduled = false;
        // A bounded pass yields the serializer to observations and UI actions.
        _scheduleQueuePump();
      }),
    );
  }

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
      if (routingEnabled &&
          c.id == null &&
          c.selectedRoute == null &&
          config['model'] is String) {
        c.selectedRoute = ChatRoute('local', config['model'], null);
      }
      c.connected = connected;
      c.stale = stale || c.detailStale;
      c.config = config;
      c.models = models;
      c.providers = providers;
      c.inventoryObserved = inventoryObserved;
      c.instanceId = instanceId;
      c._queue.observe();
      c.changed();
    }
    notifyListeners();
    _scheduleQueuePump();
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
    c._missing = false;
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
    c.historyTimestamps = [];
    c.history = (r.result['history'] as List)
        .map((m) => Map<String, dynamic>.from(m))
        .toList();
    c.detailLoaded = true;
    c.detailStale = false;
    if (c.provisionalAnswers.isNotEmpty && c.pendingId == null) c.error = null;
    c.reconcileLocalHistory();
    await _readRouting(c);
    await _readTimestamps(c);
  }

  Future<void> _readTimestamps(ConversationController c) async {
    if (!timestampsEnabled || c.id == null || c.history.isEmpty) return;
    final id = c.id;
    final instance = instanceId;
    final revision = c.summary?['history_revision'];
    final history = c.history;
    try {
      final reply = await api.call('conversation_timestamps', {
        'conversation_id': id,
      });
      if (_closed ||
          c._detached ||
          reply.instanceId != instance ||
          instanceId != instance ||
          c.id != id ||
          c.summary?['history_revision'] != revision ||
          !identical(c.history, history)) {
        return;
      }
      final times = ConversationTimestamps(reply.result);
      if (times.conversationId == id &&
          times.historyRevision == revision &&
          times.messageTimes.length == history.length) {
        c.historyTimestamps = times.messageTimes;
      }
    } on Object {
      // Optional display metadata must not turn readable history into an outage
      // or change recovery/queue decisions. Never substitute observation time.
    }
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
      _validate('state', r.result);
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
      await _readCatalog();
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
          if (c._queue.retained || c.pendingId != null || c.uncertain) {
            c._missing = true;
            c.detailStale = true;
            c._queue.latch('Chat no longer available. Local text is retained.');
            continue;
          }
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
        if (c._queue.retained) {
          c._queue.latch('Chat was archived. Local text is retained.');
          continue;
        }
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
        if (chatOperation(o['kind'])) {
          latest.putIfAbsent(o['conversation_id'] as String, () => o);
        }
      }
      for (final c in conversations.values) {
        final o = latest[c.id];
        if (o == null) continue;
        c.latestObservedRequestId = o['request_id'];
        c.latestObservedOperation = Map.of(o);
        if (c._observedHistoryRequest != o['request_id']) {
          final snapshot = (r.result['conversations'] as List).firstWhere(
            (s) => s['conversation_id'] == c.id,
          );
          final history = snapshot['history_revision'] as int;
          c._observedHistoryRequest = o['request_id'];
          c._observedHistoryBase = o['status'] == 'succeeded'
              ? (history > 0 ? history - 1 : null)
              : history;
        }
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
        if (c != null && chatOperation(o['kind'])) {
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
        _validate('poll', r.result);
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
            await _readCatalog();
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
    _validate('poll', r.result);
    await _readRequestRoute(c, request);
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
      c._queue.terminal(request, o);
      c._queue.latch(
        'Another operation changed this chat. Review before continuing.',
      );
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
      c.historyTimestamps = [];
      c.history = (detail.result['history'] as List)
          .map((m) => Map<String, dynamic>.from(m))
          .toList();
      c.detailLoaded = true;
      c.detailStale = false;
      c.reconcileLocalHistory();
      await _readTimestamps(c);
    }
    c.phase = o['phase'];
    c.operationStatus = o['status'];
    c.uncertain = false;
    c.error = null;
    if (['queued', 'running'].contains(o['status'])) return;
    c._queue.terminal(request, o);
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

  Future<void> submit(ConversationController c, {ConfirmTransfer? confirm}) {
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
      route: routingEnabled ? c.selectedRoute : null,
      routeRevision: c.routeRevision,
    );
    if (routingEnabled && intent.route?.cloud == true) {
      c.preparingRoute = true;
      changed();
      return _serialize(() async {
        try {
          final prepared = await _prepareIntent(c, intent, confirm);
          c.preparingRoute = false;
          if (prepared == null || !c.canSend) return;
          await _admitIntent(c, prepared, serialized: true);
        } on Object catch (e) {
          c.error = '$e';
        } finally {
          c.preparingRoute = false;
          changed();
        }
      });
    }
    return _admitIntent(c, intent);
  }

  Future<void> _admitIntent(
    ConversationController c,
    _SendIntent intent, {
    bool serialized = false,
  }) {
    final request = intent.request;
    final text = intent.text;
    c._queue.captureImmediate(intent);
    c._queuedSend = request;
    c.localSends[request] = LocalTextSend(
      text,
      historyRevision: c.summary?['history_revision'] as int? ?? 0,
    );
    c.busy = true;
    c.error = null;
    c.edit('');
    changed();
    Future<void> dispatch() async {
      c._queuedSend = null;
      c.busy = false;
      c._queue.dispatching = true;
      try {
        await _submit(c, intent);
      } finally {
        c._queue.dispatching = false;
        changed();
      }
    }

    return (serialized ? dispatch() : _serialize(dispatch)).whenComplete(() {
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
        (routingEnabled &&
            (c.selectedRoute != intent.route ||
                c.routeRevision != intent.routeRevision)) ||
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
      c._queue.rejected(request);
      changed();
      return;
    }
    final op = routingEnabled
        ? 'submit_routed'
        : intent.conversation == null
        ? 'submit_new'
        : 'submit';
    final kind = routingEnabled
        ? (intent.conversation == null ? 'submit_new_routed' : 'submit_routed')
        : op;
    c.pendingId = request;
    c.pendingKind = kind;
    if (intent.route != null) {
      c.submittedRoutes[request] = intent.route!;
      while (c.submittedRoutes.length > 32) {
        c.submittedRoutes.remove(c.submittedRoutes.keys.first);
      }
    }
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
    // Listeners may synchronously request Close while seeing Sending. The
    // reservation above is not admission; fence again before the exchange.
    if (_closed ||
        queueExitPending ||
        (routingEnabled &&
            (c.selectedRoute != intent.route ||
                c.routeRevision != intent.routeRevision))) {
      c.pendingId = c.pendingKind = null;
      c.observation.stop();
      c._queue.rejected(request);
      c.localSends[request]?.active = false;
      c.localSends[request]?.status =
          'Not sent. Close or route selection changed before dispatch.';
      changed();
      return;
    }
    var admissionObserved = false;
    try {
      c._queue.dispatched(request);
      final r = await api.call(
        op,
        routingEnabled
            ? {
                ...intent.routeArguments,
                'approval_token': intent.approvalToken,
                'confirm_transfer': intent.confirmTransfer,
              }
            : {
                'request_id': request,
                if (intent.conversation != null)
                  'conversation_id': intent.conversation,
                'text': intent.text,
                'output': 'chat',
                'expected_config_revision': intent.configRevision,
                if (intent.conversation != null)
                  'expected_history_revision': intent.historyRevision,
              },
      );
      if (_closed) return;
      _correlate(r);
      _validate(op, r.result);
      if (r.result['request_id'] != request ||
          r.result['kind'] != kind ||
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
      c.error =
          !admissionObserved &&
              e is AppFailure &&
              !e.unknown &&
              e.code == 'INSTRUCTIONS_LOCAL_ONLY'
          ? 'Cloud send blocked: global personal instructions are local-only. Enable cloud sharing in Instructions and review transfer consent, or select a local model. Nothing was sent.'
          : '$e';
      c.uncertain = true;
      if (admissionObserved) stale = true;
      // Only a definitive failure of the admission exchange can release this
      // identity. Once accepted, even a definitive poll error is query-only.
      if (!admissionObserved &&
          e is AppFailure &&
          !e.unknown &&
          const {
            'BUSY',
            'STALE',
            'STALE_REVISION',
            'INVALID_REQUEST',
            'CAPACITY',
            'MODEL_UNAVAILABLE',
            'CONVERSATION_ARCHIVED',
            'ROUTE_UNAVAILABLE',
            'PROVIDER_NOT_CONFIGURED',
            'PROVIDER_NOT_VALIDATED',
            'CREDENTIAL_CHANGED',
            'INSTRUCTIONS_LOCAL_ONLY',
            'CONSENT_REQUIRED',
            'CONSENT_EXPIRED',
            'CONSENT_MISMATCH',
          }.contains(e.code)) {
        c._queue.rejected(request);
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
        (op != 'conversation_rename' &&
            (c.pendingId != null || c._queue.retained))) {
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
    this.route,
    this.routeRevision = 0,
    this.approvalToken,
    this.confirmTransfer = false,
  });
  final String request, text;
  final int draftRevision;
  final String? instance, conversation;
  final Object? configRevision;
  final int? historyRevision;
  final ChatRoute? route;
  final int routeRevision;
  final String? approvalToken;
  final bool confirmTransfer;
  Map<String, dynamic> get routeArguments => {
    'request_id': request,
    'instance_id': instance,
    'conversation_id': conversation,
    'text': text,
    'route': route!.toJson(),
    'expected_config_revision': configRevision,
    'expected_history_revision': historyRevision,
  };
  _SendIntent withPreparation(RoutePreparation p) => _SendIntent(
    request: request,
    text: text,
    draftRevision: draftRevision,
    instance: instance,
    conversation: conversation,
    configRevision: configRevision,
    historyRevision: historyRevision,
    route: route,
    routeRevision: routeRevision,
    approvalToken: p.value['approval_token'],
    confirmTransfer: p.requiresConsent,
  );
}

// Logical messages survive attempts. Attempt IDs are never recycled, including
// after definite nonadmission; unknown attempts remain barriers even if the
// operator forgets their observation identity elsewhere in the UI.
class _QueueEntry {
  _QueueEntry(this.text, {this.allowance = false, this.route});
  ChatRoute? route;
  final String id = appId();
  final String text;
  bool allowance;
  String? request;
  bool dispatched = false;
  bool everDispatched = false;
  bool succeeded = false;
  String? answer;
  int actionRevision = 0;
  QueuedMessageState state = QueuedMessageState.waiting;
}

class _ConversationQueue extends MessageQueue {
  _ConversationQueue(this.c);
  final ConversationController c;
  WorkspaceController get w => c.owner;
  final List<_QueueEntry> _entries = [];
  int _revision = 0;
  String? _reason;
  String? _instance;
  Object? _config;
  int _expectedHistory = 0;
  String? _reviewedOperation;
  String? _lastSkipNotice;
  String? _foreignPredecessor;
  bool _foreignSucceeded = false;
  bool _foreignFailed = false;
  bool _foreignUnknown = false;
  String? _foreignAnswer;
  bool dispatching = false;
  bool _actionPending = false;

  bool _enabled = false;
  bool get retained => _entries.isNotEmpty || _foreignPredecessor != null;
  bool get blocksSend =>
      retained &&
      (_enabled ||
          _foreignPredecessor != null ||
          _entries.any((e) => e.state != QueuedMessageState.failed));
  int get capacityCount => _entries.where((e) => !e.allowance).length;
  int get unsentCount =>
      _entries
          .where((e) => !e.dispatched || e.state == QueuedMessageState.failed)
          .length +
      c.localSends.entries
          .where(
            (receipt) =>
                !receipt.value.active &&
                !c.provisionalAnswers.containsKey(receipt.key) &&
                !_entries.any((e) => e.request == receipt.key),
          )
          .length;
  @override
  int get revision => _revision;
  @override
  bool get usesQueue =>
      blocksSend || c.pendingId != null || c._queuedSend != null;
  @override
  bool get paused => _reason != null;
  @override
  String? get pauseReason => _reason;
  @override
  int get waitingCount => _entries
      .where((e) => !e.everDispatched && e.state == QueuedMessageState.waiting)
      .length;
  bool get _alive => !w._closed && !c._detached;
  bool get _ready =>
      _alive &&
      !w.queueExitPending &&
      !c._missing &&
      !c.archived &&
      w.connected &&
      !w.stale &&
      (w.routingEnabled
          ? c.routeReady
          : w.inventoryObserved &&
                w.models.contains(w.config['model']) &&
                w.providers['llm'] is Map &&
                w.providers['llm']['ready'] == true) &&
      w._globalRequest == null &&
      !w.mutations.containsKey(c.id);
  @override
  bool get canEnqueue =>
      _alive &&
      !w.queueExitPending &&
      !c._missing &&
      !c.archived &&
      (usesQueue || retained) &&
      c.draft.trim().isNotEmpty &&
      utf8.encode(c.draft).length <= MessageQueue.maxMessageBytes &&
      capacityCount < MessageQueue.perConversationLimit &&
      w.all.fold<int>(0, (n, chat) => n + chat._queue.capacityCount) <
          MessageQueue.desktopLimit;
  @override
  bool get canResume =>
      paused &&
      _ready &&
      !_actionPending &&
      c.pendingId == null &&
      _foreignPredecessor == null &&
      !_entries.any(
        (e) =>
            e.state == QueuedMessageState.unknown ||
            e.state == QueuedMessageState.accepted ||
            e.state == QueuedMessageState.sending,
      );

  bool _safe(_QueueEntry e) =>
      e.succeeded ||
      e.state == QueuedMessageState.waiting ||
      e.state == QueuedMessageState.failed;
  bool _retryable(_QueueEntry e) =>
      identical(_entries.firstOrNull, e) &&
      e.state == QueuedMessageState.failed &&
      _ready &&
      !paused &&
      !_actionPending &&
      c.pendingId == null;
  @override
  List<QueuedMessage> get entries => List.unmodifiable([
    if (_enabled && _foreignPredecessor != null)
      QueuedMessage(
        id: _foreignPredecessor!,
        text: '', // Reattached work has no invented local prompt.
        state: _foreignFailed
            ? QueuedMessageState.failed
            : c.uncertain || _foreignUnknown
            ? QueuedMessageState.unknown
            : QueuedMessageState.accepted,
        status: _foreignFailed
            ? 'Previous service message failed · Skip to continue'
            : _foreignSucceeded
            ? 'Waiting for saved history'
            : 'Previous service message · observation only',
        actionRevision: revision,
        canSkip:
            _alive &&
            !w.queueExitPending &&
            !_actionPending &&
            c.pendingId == null &&
            (_foreignFailed || _foreignSucceeded),
      ),
    ...(_enabled ? _entries : <_QueueEntry>[]).map(
      (e) => QueuedMessage(
        id: e.id,
        text: e.text,
        state: e.state,
        actionRevision: e.actionRevision,
        status:
            (e.succeeded
                ? 'Waiting for saved history'
                : switch (e.state) {
                    QueuedMessageState.waiting => 'Waiting locally · not sent',
                    QueuedMessageState.sending => 'Sending…',
                    QueuedMessageState.accepted =>
                      c.operationStatus == 'queued'
                          ? 'Queued by service'
                          : 'Thinking…',
                    QueuedMessageState.failed =>
                      'Failed / not sent · Retry or Skip',
                    QueuedMessageState.unknown =>
                      'Outcome unknown · observation only',
                  }) +
            (e.route == null ? '' : ' · ${e.route!.label}'),
        canRetry: _retryable(e),
        canSkip:
            _alive &&
            !w.queueExitPending &&
            !_actionPending &&
            identical(_entries.firstOrNull, e) &&
            _safe(e) &&
            _foreignPredecessor == null &&
            c.pendingId == null,
        canRemove: _alive && !w.queueExitPending && !_actionPending && _safe(e),
      ),
    ),
  ]);

  void _touch() {
    _revision++;
    for (final e in _entries) {
      e.actionRevision++;
    }
  }

  void _bind() {
    _instance = w.instanceId;
    _config = w.config['revision'];
    _expectedHistory = c.summary?['history_revision'] as int? ?? 0;
    _reviewedOperation = c.latestObservedRequestId;
  }

  void latch(String reason) {
    if (_reason == reason) return;
    _reason = reason;
    _touch();
  }

  void captureImmediate(_SendIntent intent) {
    // Without queued follow-ups, a definite rejection keeps the pre-queue
    // immediate-Send behavior. Unknown attempts can never enter this path.
    if (!_enabled) _entries.clear();
    // Presentation receipts are not a second, unbounded queue archive.
    while (c.localSends.length >= 32) {
      c.localSends.remove(c.localSends.keys.first);
    }
    _reason = null;
    _bind();
    final e = _QueueEntry(intent.text, allowance: true, route: intent.route)
      ..request = intent.request
      ..state = QueuedMessageState.sending;
    _entries.add(e);
    _touch();
  }

  @override
  void enqueue() {
    if (!canEnqueue) return;
    if (!retained) {
      _bind();
      _foreignPredecessor = c.pendingId;
      if (_foreignPredecessor != null) {
        if (c._observedHistoryRequest == _foreignPredecessor &&
            c._observedHistoryBase != null) {
          _expectedHistory = c._observedHistoryBase!;
        } else {
          latch(
            'Predecessor history binding unavailable. Observe before continuing.',
          );
        }
      }
    }
    _enabled = true;
    _entries.add(_QueueEntry(c.draft, route: c.selectedRoute));
    _touch();
    c.edit('');
    w.changed();
  }

  void dispatched(String request) {
    final e = _entries.where((e) => e.request == request).firstOrNull;
    if (e == null) return;
    e.dispatched = true;
    e.everDispatched = true;
    e.allowance = true;
    _touch();
  }

  void rejected(String request) {
    final e = _entries.where((e) => e.request == request).firstOrNull;
    if (e == null) return;
    e.state = QueuedMessageState.failed;
    _touch();
  }

  void terminal(String request, Map<String, dynamic> operation) {
    // A terminal service record can still represent unknown remote processing.
    // It is not proof that retrying or releasing a dependent turn is safe.
    final outcomeUnknown =
        operation['status'] != 'succeeded' &&
        operation['error']?['code'] == 'REMOTE_OUTCOME_UNKNOWN';
    if (_foreignPredecessor == request) {
      _foreignUnknown = outcomeUnknown;
      _foreignSucceeded = operation['status'] == 'succeeded';
      if (_foreignSucceeded) {
        _foreignAnswer = operation['result']['assistant_text'] as String;
      }
      _foreignFailed = !_foreignSucceeded && !outcomeUnknown;
      if (outcomeUnknown) {
        latch(
          'Remote outcome unknown. The queue remains blocked; nothing is resent.',
        );
      }
      _touch();
    }
    final e = _entries.where((e) => e.request == request).firstOrNull;
    if (e == null) return;
    if (operation['status'] == 'succeeded') {
      e.succeeded = true;
      e.answer = operation['result']['assistant_text'] as String;
    } else if (outcomeUnknown) {
      e.state = QueuedMessageState.unknown;
      latch(
        'Remote outcome unknown. The queue remains blocked; nothing is resent.',
      );
    } else {
      e.state = QueuedMessageState.failed;
    }
    _touch();
  }

  void observe() {
    if (!retained) return;
    if (_instance != w.instanceId || _config != w.config['revision']) {
      latch(
        'Service, model or global instructions configuration changed. Review and Resume explicitly.',
      );
    }
    final head = _entries.firstOrNull;
    final request = head?.request ?? _foreignPredecessor;
    if (_foreignPredecessor != null && c.uncertain) {
      latch('Outcome was unknown. Observe, then explicitly Resume.');
    }
    if (c.pendingId != null && c.pendingId != request) {
      latch(
        'Another operation is active in this chat. Review before continuing.',
      );
    }
    if (c.latestObservedRequestId != null &&
        c.latestObservedRequestId != request &&
        c.latestObservedRequestId != _reviewedOperation) {
      latch('Another operation changed this chat. Review before continuing.');
    }
    if (head != null &&
        head.request == c.pendingId &&
        head.dispatched &&
        !head.succeeded &&
        head.state != QueuedMessageState.failed) {
      final next = c.uncertain
          ? QueuedMessageState.unknown
          : c.operationStatus == null
          ? QueuedMessageState.sending
          : QueuedMessageState.accepted;
      if (next != head.state) {
        head.state = next;
        _touch();
      }
      if (c.uncertain) {
        latch('Outcome was unknown. Observe, then explicitly Resume.');
      }
    }
    final actual = c.summary?['history_revision'] as int? ?? 0;
    final succeeded = head?.succeeded == true || _foreignSucceeded;
    if (succeeded && c.pendingId == null && c.detailLoaded && !c.detailStale) {
      final pairMatches = _foreignSucceeded
          ? c.history.isNotEmpty && c.history.last['content'] == _foreignAnswer
          : head != null &&
                (c.history.length >= 2 &&
                    c.history[c.history.length - 2]['content'] == head.text &&
                    c.history.last['content'] == head.answer);
      if (actual != _expectedHistory + 1 || !pairMatches) {
        latch(
          'Saved history does not match the completed predecessor. Review before continuing.',
        );
      } else {
        _expectedHistory = actual;
        _reviewedOperation = request;
        if (head?.succeeded == true) _entries.removeAt(0);
        _foreignPredecessor = null;
        _foreignSucceeded = false;
        _foreignFailed = false;
        _foreignUnknown = false;
        if (_entries.isEmpty) _enabled = false;
        _touch();
      }
    } else if (actual != _expectedHistory &&
        !(request != null && actual == _expectedHistory + 1)) {
      latch(
        'Saved history changed unexpectedly. Review and Resume explicitly.',
      );
    }
  }

  @override
  void pause() {
    if (!_alive) return;
    latch('Paused by you.');
    w.changed();
  }

  @override
  Future<void> resume(int expectedRevision) async {
    if (revision != expectedRevision || !canResume) return;
    _actionPending = true;
    _touch();
    final token = revision;
    await w._serialize(() async {
      try {
        if (c.id != null) await w._detail(c);
        if (!_ready ||
            revision != token ||
            c.pendingId != null ||
            c._missing ||
            c.archived) {
          return;
        }
        _bind();
        for (final e in _entries.where(
          (e) => !e.dispatched || e.state == QueuedMessageState.failed,
        )) {
          e.route = c.selectedRoute;
        }
        _reason = null;
        _touch();
      } on Object catch (e) {
        w._observeFailure(e);
        latch('Cannot resume until saved history is available.');
      }
    });
    _actionPending = false;
    w.changed();
  }

  _QueueEntry? _find(String id, int token) {
    if (!_alive || w.queueExitPending || _actionPending) return null;
    return _entries
        .where((e) => e.id == id && e.actionRevision == token)
        .firstOrNull;
  }

  @override
  Future<void> retry(String id, int expectedRevision) async {
    final e = _find(id, expectedRevision);
    if (e == null || !_retryable(e)) return;
    // Reserve before yielding; a stale callback cannot retry a later attempt.
    e.state = QueuedMessageState.waiting;
    if (e.request != null) {
      if (c.latestObservedRequestId == e.request) {
        _reviewedOperation = e.request;
      }
      c.localSends.remove(e.request);
    }
    e.request = null;
    e.dispatched = false;
    _touch();
    w.changed();
  }

  @override
  void skip(String id, int expectedRevision) {
    if (_alive &&
        !w.queueExitPending &&
        !_actionPending &&
        id == _foreignPredecessor &&
        expectedRevision == revision &&
        c.pendingId == null &&
        (_foreignFailed || _foreignSucceeded)) {
      if (c.latestObservedRequestId == _foreignPredecessor) {
        _reviewedOperation = _foreignPredecessor;
      }
      _lastSkipNotice = 'Previous service message skipped as a queue dependency. Nothing was resent or cancelled.';
      c.error = _lastSkipNotice;
      _foreignPredecessor = null;
      _foreignSucceeded = _foreignFailed = _foreignUnknown = false;
      _touch();
      w.changed();
      return;
    }
    final e = _find(id, expectedRevision);
    if (e == null ||
        !identical(_entries.firstOrNull, e) ||
        !_safe(e) ||
        c.pendingId != null ||
        _foreignPredecessor != null) {
      return;
    }
    _entries.remove(e);
    if (e.request != null) {
      if (c.latestObservedRequestId == e.request) {
        _reviewedOperation = e.request;
      }
      c.localSends.remove(e.request);
    }
    _lastSkipNotice = e.everDispatched
        ? 'Previous message skipped as a queue dependency. Nothing was resent or cancelled.'
        : 'One local queued message skipped without sending. No active work was cancelled.';
    c.error = _lastSkipNotice;
    _touch();
    w.changed();
  }

  @override
  void remove(String id, int expectedRevision) {
    final e = _find(id, expectedRevision);
    if (e == null || !_safe(e)) return;
    _entries.remove(e);
    if (e.request != null) c.localSends.remove(e.request);
    latch('A dependency was removed. Review remaining messages and Resume.');
    _touch();
    w.changed();
  }

  @override
  void clear(int expectedRevision) {
    if (!_alive ||
        w.queueExitPending ||
        _actionPending ||
        revision != expectedRevision) {
      return;
    }
    for (final e in _entries.where(_safe).toList()) {
      _entries.remove(e);
      if (e.request != null) c.localSends.remove(e.request);
    }
    latch('Local waiting messages cleared; active work was not cancelled.');
    _touch();
    w.changed();
  }

  bool get readyToPump =>
      _ready &&
      !paused &&
      !_actionPending &&
      !dispatching &&
      c.pendingId == null &&
      c._queuedSend == null &&
      _foreignPredecessor == null &&
      _entries.firstOrNull?.state == QueuedMessageState.waiting &&
      w.all.where((chat) => chat.pendingId != null).length < 4;
  Future<void> pump({ConfirmTransfer? confirm}) async {
    if (!readyToPump) return;
    final e = _entries.first;
    final token = revision;
    try {
      if (c.id != null) await w._detail(c);
      if (!readyToPump ||
          revision != token ||
          !identical(_entries.firstOrNull, e)) {
        return;
      }
      if ((w.routingEnabled && e.route != c.selectedRoute) ||
          _instance != w.instanceId ||
          _config != w.config['revision'] ||
          (c.summary?['history_revision'] as int? ?? 0) != _expectedHistory) {
        latch('Dispatch binding changed. Review and Resume explicitly.');
        return;
      }
      final request = appId();
      var intent = _SendIntent(
        request: request,
        text: e.text,
        draftRevision: c.draftRevision,
        instance: _instance,
        conversation: c.id,
        configRevision: _config,
        historyRevision: c.summary?['history_revision'] as int?,
        route: e.route,
        routeRevision: c.routeRevision,
      );
      final prepared = await w._prepareIntent(c, intent, confirm, queued: true);
      if (prepared == null) {
        latch(
          _reason ??
              'Cloud send not approved. Review queued cloud send explicitly.',
        );
        return;
      }
      if (!readyToPump ||
          revision != token ||
          !identical(_entries.firstOrNull, e)) {
        return;
      }
      intent = prepared;
      e.request = request;
      e.state = QueuedMessageState.sending;
      _touch();
      c.localSends[request] = LocalTextSend(
        e.text,
        historyRevision: _expectedHistory,
        fromQueue: true,
      );
      dispatching = true;
      await w._submit(c, intent);
    } on Object catch (error) {
      w._observeFailure(error);
      if (error is AppFailure &&
          error.code == 'INSTRUCTIONS_LOCAL_ONLY' &&
          !error.unknown) {
        c.error = 'Cloud send blocked: global personal instructions are local-only. Enable cloud sharing in Instructions and review transfer consent, or select a local model. Nothing was sent.';
        latch(c.error!);
      } else {
        latch('Saved chat unavailable. Local messages remain unsent.');
      }
    } finally {
      dispatching = false;
      w.changed();
    }
  }
}

class PendingMutation {
  PendingMutation(this.request, this.op, this.id, this.clock) {
    clock.start();
  }
  final String request, op, id;
  bool inFlight = true;
  final Stopwatch clock;
}
