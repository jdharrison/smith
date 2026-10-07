import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:smith_client/bridge/smith_bridge.dart';

enum ChatRole { user, assistant, system }

/// Lifecycle of a chat row during a voice turn (and normal messages).
enum MessagePhase {
  /// Final text shown.
  complete,

  /// User row: clip ready, waiting on transcript.
  awaitingTranscript,

  /// User row: typewriter animating transcript.
  typing,

  /// User row: transcript sent (or non-voice final).
  sent,

  /// Assistant row: waiting on smith reply.
  awaitingReply,

  /// Assistant row: full reply known, revealing word-by-word.
  revealing,
}

class ChatMessage {
  ChatMessage({
    required this.id,
    required this.role,
    required this.text,
    required this.at,
    this.attachments = const [],
    this.audioPath,
    this.phase = MessagePhase.complete,
  });

  final String id;
  final ChatRole role;
  String text;
  final DateTime at;
  List<ChatAttachment> attachments;
  String? audioPath;
  MessagePhase phase;

  bool get showSpinner =>
      phase == MessagePhase.awaitingTranscript ||
      phase == MessagePhase.awaitingReply ||
      phase == MessagePhase.typing;
}

enum BridgeStatus { unknown, online, offline }

/// Conversation + smith bridge. Voice turns mutate placeholders via ids.
class ChatController extends ChangeNotifier {
  ChatController({
    SmithBridge? bridge,
    this.chatOnly = false,
    this.readinessGuard,
  }) : _bridge = bridge ?? SmithBridge(),
       _newNext = chatOnly;

  final bool chatOnly;
  final bool Function(HealthInfo health)? readinessGuard;

  bool _ready(HealthInfo h) =>
      h.ok &&
      h.service == 'smith-bridge' &&
      h.profile == 'chat-only' &&
      !h.echo &&
      h.provider == 'ollama' &&
      h.providerAvailable == true &&
      h.modelAvailable == true &&
      h.model != null &&
      h.model!.isNotEmpty &&
      (readinessGuard?.call(h) ?? true);

  void readinessChanged() {
    if (!_disposed) notifyListeners();
  }

  HealthInfo? _health;
  String? _connectionError;
  String? _failedText;
  bool _healthInFlight = false;
  Future<void>? _healthRequest;

  HealthInfo? get health => _health;
  String? get connectionError => _connectionError;
  String? get failedText => _failedText;
  bool get healthChecking => _healthInFlight;
  bool get resetPending => _newNext;

  final SmithBridge _bridge;
  final List<ChatMessage> _messages = <ChatMessage>[];
  final List<BoardTask> _tasks = <BoardTask>[];
  final Set<String> _injectedTaskIds = <String>{};
  final Map<String, Timer> _revealTimers = <String, Timer>{};
  int _msgSeq = 0;
  bool _disposed = false;
  bool _assistantSpeechAuthorized = false;

  BridgeStatus _bridgeStatus = BridgeStatus.unknown;
  bool _bridgeEcho = false;
  String _isolation = 'sealed';
  PipelineStatus? _pipeline;
  bool _sendBusy = false;
  bool _voiceTurnBusy = false;
  String? _error;
  bool _newNext;
  Timer? _healthTimer;
  Timer? _taskPollTimer;
  int? _pollSince;
  bool _pollInFlight = false;
  BoardTask? _selectedTask;
  DateTime? _tasksUpdatedAt;
  String? _tasksError;
  String? _taskDetailError;
  int _taskRefreshGeneration = 0;
  int _taskDetailGeneration = 0;

  /// Optional TTS hook (wired by VoiceTurnController).
  Future<void> Function(String text)? onSpeakAssistant;

  BridgeStatus get bridgeStatus => _bridgeStatus;
  bool get bridgeEcho => _bridgeEcho;
  String get isolation => _isolation;
  PipelineStatus? get pipeline => _pipeline;
  bool get busy => _sendBusy || _voiceTurnBusy;
  bool get sendBusy => _sendBusy;
  String? get error => _error;
  List<ChatMessage> get messages => List.unmodifiable(_messages);
  List<BoardTask> get tasks => List.unmodifiable(_tasks);
  BoardTask? get selectedTask => _selectedTask;
  bool get canSend =>
      !_sendBusy &&
      (!chatOnly ||
          (_bridgeStatus == BridgeStatus.online &&
              _health != null &&
              _ready(_health!)));
  bool get pollingTasks => _taskPollTimer != null;
  DateTime? get tasksUpdatedAt => _tasksUpdatedAt;
  String? get tasksError => _tasksError;
  String? get taskDetailError => _taskDetailError;

  String _nextId(String prefix) {
    _msgSeq += 1;
    return '${prefix}_$_msgSeq';
  }

  Future<void> start() async {
    await refreshHealth();
    if (_disposed) return;
    _healthTimer?.cancel();
    _healthTimer = Timer.periodic(const Duration(seconds: 4), (_) {
      unawaited(refreshHealth());
    });
    if (!chatOnly) await refreshTasks();
  }

  @override
  void dispose() {
    _disposed = true;
    _taskRefreshGeneration += 1;
    _taskDetailGeneration += 1;
    _healthTimer?.cancel();
    _stopTaskPoll();
    for (final timer in _revealTimers.values) {
      timer.cancel();
    }
    _revealTimers.clear();
    _bridge.close();
    super.dispose();
  }

  Future<void> refreshHealth() {
    if (_disposed) return Future.value();
    return _healthRequest ??= _readHealth().whenComplete(
      () => _healthRequest = null,
    );
  }

  Future<void> _readHealth() async {
    if (_disposed) return;
    _healthInFlight = true;
    notifyListeners();
    try {
      final h = await _bridge.health();
      if (_disposed) return;
      _health = h;
      final validIdentity = h.service == 'smith-bridge';
      final validProfile = !chatOnly || (h.profile == 'chat-only' && !h.echo);
      final ready = !chatOnly || _ready(h);
      _bridgeStatus = h.ok && validIdentity && validProfile
          ? BridgeStatus.online
          : BridgeStatus.offline;
      _connectionError = !validIdentity
          ? 'The listener is not the Smith bridge. No message will be sent.'
          : !validProfile
          ? 'Chat-only profile required. Start the separately managed backend with smith serve --chat-only.'
          : !h.ok
          ? 'Smith is not ready. Check the local provider and preprovisioned model, then retry connection.'
          : chatOnly && !ready
          ? 'The local model is unavailable or its availability could not be confirmed. Check the preprovisioned Ollama model, then retry connection. No download or fallback is attempted.'
          : null;
      _bridgeEcho = h.echo;
      _isolation = h.isolation;
      if (_bridgeStatus == BridgeStatus.online &&
          _error != null &&
          _error!.startsWith('Bridge offline')) {
        _error = null;
      }
    } catch (_) {
      if (_disposed) return;
      _health = null;
      _bridgeStatus = BridgeStatus.offline;
      _connectionError =
          'Cannot reach the local Smith bridge. Start the separately managed chat-only backend, then retry connection. Your draft is retained.';
    } finally {
      _healthInFlight = false;
    }
    if (!_disposed) notifyListeners();
  }

  Future<void> refreshTasks() async {
    if (_disposed || chatOnly) return;
    final generation = ++_taskRefreshGeneration;
    if (_bridgeStatus != BridgeStatus.online) {
      await refreshHealth();
    }
    if (_disposed || generation != _taskRefreshGeneration) return;
    if (_bridgeStatus != BridgeStatus.online) {
      _tasksError = 'Bridge offline; task state may be stale.';
      notifyListeners();
      return;
    }
    try {
      final list = await _bridge.listTasks();
      if (_disposed || generation != _taskRefreshGeneration) return;
      _tasks
        ..clear()
        ..addAll(list);
      _tasksUpdatedAt = DateTime.now();
      _tasksError = null;
      if (_selectedTask != null) {
        final selectedIndex = _tasks.indexWhere(
          (task) => task.id == _selectedTask!.id,
        );
        _selectedTask = selectedIndex < 0 ? null : _tasks[selectedIndex];
      }
      if (list.any((task) => task.isRunning)) {
        _ensureTaskPoll();
      } else {
        _stopTaskPoll();
      }
      notifyListeners();
    } catch (error) {
      if (_disposed || generation != _taskRefreshGeneration) return;
      _tasksError = error.toString();
      notifyListeners();
      // Keep the last snapshot, but expose that it may be stale.
    }
  }

  void selectTask(BoardTask? task) {
    _selectedTask = task;
    _taskDetailError = null;
    if (task == null) _taskDetailGeneration += 1;
    notifyListeners();
  }

  Future<void> openTaskDetail(String id) async {
    if (chatOnly) return;
    final generation = ++_taskDetailGeneration;
    _taskDetailError = null;
    notifyListeners();
    try {
      final task = await _bridge.getTask(id);
      if (generation != _taskDetailGeneration || _selectedTask?.id != id) {
        return;
      }
      _selectedTask = task;
      final index = _tasks.indexWhere((candidate) => candidate.id == id);
      if (index >= 0) {
        _tasks[index] = task;
      } else {
        _tasks.insert(0, task);
      }
      notifyListeners();
    } catch (error) {
      if (generation != _taskDetailGeneration || _selectedTask?.id != id) {
        return;
      }
      _taskDetailError = error.toString();
      notifyListeners();
    }
  }

  /// Clear local conversation state; the next explicit send starts the session.
  Future<void> requestNewSession() async {
    if (busy) return;
    _newNext = true;
    _messages.clear();
    _injectedTaskIds.clear();
    _error = null;
    _failedText = null;
    notifyListeners();
  }

  Future<void> openAttachment(ChatAttachment att) async {
    if (chatOnly || att.path.isEmpty) return;
    try {
      await _bridge.openPath(att.path);
    } catch (e) {
      _error = e.toString();
      notifyListeners();
    }
  }

  ChatMessage? messageById(String id) {
    for (final m in _messages) {
      if (m.id == id) return m;
    }
    return null;
  }

  /// Instant user bubble on release: playable clip + loading for transcript.
  String beginVoiceUserMessage({String? audioPath}) {
    final id = _nextId('user');
    _messages.add(
      ChatMessage(
        id: id,
        role: ChatRole.user,
        text: '',
        at: DateTime.now(),
        audioPath: audioPath,
        phase: MessagePhase.awaitingTranscript,
      ),
    );
    notifyListeners();
    return id;
  }

  void setMessageAudio(String id, String? path) {
    final m = messageById(id);
    if (m == null) return;
    m.audioPath = path;
    notifyListeners();
  }

  void setMessagePhase(String id, MessagePhase phase) {
    final m = messageById(id);
    if (m == null) return;
    m.phase = phase;
    notifyListeners();
  }

  void setMessageText(String id, String text) {
    final m = messageById(id);
    if (m == null) return;
    m.text = text;
    notifyListeners();
  }

  void failVoiceUserMessage(String id, String errorText) {
    final m = messageById(id);
    if (m == null) return;
    m.text = errorText;
    m.phase = MessagePhase.complete;
    notifyListeners();
  }

  /// Pending smith row shown as soon as the user turn is sent.
  String beginAssistantPending() {
    final id = _nextId('smith');
    _messages.add(
      ChatMessage(
        id: id,
        role: ChatRole.assistant,
        text: '',
        at: DateTime.now(),
        phase: MessagePhase.awaitingReply,
      ),
    );
    notifyListeners();
    return id;
  }

  void completeAssistant(
    String id,
    String text, {
    List<ChatAttachment> attachments = const [],
  }) {
    final m = messageById(id);
    if (m == null) return;
    final full = text.trim().isEmpty ? 'Smith returned no text.' : text.trim();
    m.attachments = chatOnly ? const [] : attachments;
    if (chatOnly) {
      m.text = full;
      m.phase = MessagePhase.complete;
      notifyListeners();
    } else {
      _revealText(id, full);
    }
  }

  /// Word-chunked reveal of an already-known full reply — perceived
  /// streaming without depending on the (single-shot) bridge response.
  void _revealText(String id, String full) {
    _revealTimers.remove(id)?.cancel();
    final words = full.split(RegExp(r'(?<=\s)'));
    if (words.length <= 1) {
      setMessageText(id, full);
      setMessagePhase(id, MessagePhase.complete);
      return;
    }

    var next = 0;
    setMessagePhase(id, MessagePhase.revealing);
    _revealTimers[id] = Timer.periodic(const Duration(milliseconds: 28), (
      timer,
    ) {
      final m = messageById(id);
      if (m == null) {
        timer.cancel();
        _revealTimers.remove(id);
        return;
      }
      next = (next + 2).clamp(0, words.length);
      setMessageText(id, words.sublist(0, next).join());
      if (next >= words.length) {
        timer.cancel();
        _revealTimers.remove(id);
        setMessagePhase(id, MessagePhase.complete);
      }
    });
  }

  void failAssistant(String id, String errorText) {
    final m = messageById(id);
    if (m == null) return;
    m.text = errorText;
    m.phase = MessagePhase.complete;
    notifyListeners();
  }

  void _noteTaskFromReply(BoardTask? task) {
    if (chatOnly || task == null) return;
    final idx = _tasks.indexWhere((t) => t.id == task.id);
    if (idx >= 0) {
      _tasks[idx] = task;
    } else {
      _tasks.insert(0, task);
    }
    if (task.isRunning) {
      _ensureTaskPoll();
    } else if (task.isTerminalInject) {
      // Fast lane already delivered result in chat text — mark injected.
      _injectedTaskIds.add(task.id);
    }
    notifyListeners();
  }

  void _ensureTaskPoll() {
    if (chatOnly || _taskPollTimer != null) return;
    _pollSince = DateTime.now().millisecondsSinceEpoch ~/ 1000 - 5;
    _taskPollTimer = Timer.periodic(const Duration(seconds: 2), (_) {
      unawaited(_pollTasksOnce());
    });
    unawaited(_pollTasksOnce());
  }

  void _stopTaskPoll() {
    _taskPollTimer?.cancel();
    _taskPollTimer = null;
    _pollInFlight = false;
  }

  void _stopTaskPollIfIdle(List<BoardTask> snapshot) {
    final running = snapshot.any((t) => t.isRunning);
    if (!running) {
      _stopTaskPoll();
    }
  }

  Future<void> _pollTasksOnce() async {
    if (_disposed || chatOnly || _bridgeStatus != BridgeStatus.online) return;
    if (_pollInFlight) return;
    _pollInFlight = true;
    try {
      final list = await _bridge.listTasks(since: _pollSince);
      if (_disposed) return;
      // Merge into local cache (full refresh for Tasks tab).
      await refreshTasks();
      if (_disposed) return;
      var maxUpdated = _pollSince ?? 0;
      for (final t in list) {
        if (t.updatedAt > maxUpdated) maxUpdated = t.updatedAt;
        if (!t.isTerminalInject) continue;
        if (_injectedTaskIds.contains(t.id)) continue;
        // Claim before await gaps so overlapping ticks cannot double-inject.
        _injectedTaskIds.add(t.id);
        final body = t.resultText.trim().isNotEmpty
            ? t.resultText.trim()
            : (t.error?.trim().isNotEmpty == true
                  ? 'Failed: ${t.error}'
                  : 'Task ${t.status}.');
        if (body.isEmpty) continue;
        final id = _nextId('smith');
        _messages.add(
          ChatMessage(
            id: id,
            role: ChatRole.assistant,
            text: '',
            at: DateTime.now(),
            phase: MessagePhase.awaitingReply,
          ),
        );
        notifyListeners();
        _revealText(id, body);
        final speak = onSpeakAssistant;
        if (_assistantSpeechAuthorized && speak != null) {
          unawaited(speak(body));
        }
      }
      if (maxUpdated > 0) {
        _pollSince = maxUpdated;
      }
      // Idle from this poll snapshot (not stale cache if refresh failed).
      _stopTaskPollIfIdle([
        ...list,
        ..._tasks.where((t) => !list.any((x) => x.id == t.id)),
      ]);
    } catch (error) {
      if (_disposed) return;
      _tasksError = 'Background task refresh failed: $error';
      notifyListeners();
      // Keep the last snapshot, explicitly marked stale, and retry next tick.
    } finally {
      _pollInFlight = false;
    }
  }

  /// Bridge round-trip without adding chat rows (voice turn owns placeholders).
  Future<ChatReply> bridgeChat(String text) async {
    final trimmed = text.trim();
    if (trimmed.isEmpty) {
      throw SmithBridgeException('Empty message');
    }

    // This API is reached by an explicit text send or voice turn. Passive task
    // reconciliation must remain silent until one of those actions occurs.
    _assistantSpeechAuthorized = !chatOnly;

    if (chatOnly || _bridgeStatus != BridgeStatus.online) {
      await refreshHealth();
    }
    if (_bridgeStatus != BridgeStatus.online ||
        (chatOnly && (_health == null || !_ready(_health!)))) {
      throw SmithBridgeException(
        _connectionError ?? 'Bridge offline — retry connection before sending',
      );
    }

    final reset = _newNext;
    final reply = chatOnly
        ? await _bridge.chatOnly(trimmed, newSession: reset)
        : await _bridge.chat(trimmed, newSession: reset);
    if (_disposed) return reply;
    // Only a successful response acknowledges the reset. Never replay on health refresh.
    _newNext = false;
    _isolation = reply.isolation;
    if (!chatOnly && reply.pipeline != null) {
      _pipeline = reply.pipeline;
    }
    _noteTaskFromReply(reply.task);
    return reply;
  }

  /// Ask bridge to start on-demand Whisper/TTS sidecars if needed.
  Future<void> ensureVoiceSidecars() async {
    if (chatOnly) {
      throw SmithBridgeException('Voice is deferred in the text-only client');
    }
    if (_bridgeStatus != BridgeStatus.online) {
      await refreshHealth();
    }
    if (_bridgeStatus != BridgeStatus.online) {
      throw SmithBridgeException(
        'Bridge offline — cannot start voice sidecars',
      );
    }
    await _bridge.ensureSidecars();
  }

  /// Mark busy for an external voice-turn pipeline (independent of text send).
  void setTurnBusy(bool value) {
    _voiceTurnBusy = value;
    notifyListeners();
  }

  /// Wait until a non-voice `send` finishes.
  /// Returns false if still busy after [maxWaits] polls.
  Future<bool> waitForSendIdle({int maxWaits = 120}) async {
    var waits = 0;
    while (_sendBusy && waits < maxWaits) {
      await Future<void>.delayed(const Duration(milliseconds: 100));
      waits++;
    }
    return !_sendBusy;
  }

  Future<void> send(String raw) async {
    final text = raw.trim();
    if (_disposed || text.isEmpty || !canSend) return;

    _sendBusy = true;
    _error = null;
    _failedText = null;
    _messages.add(
      ChatMessage(
        id: _nextId('user'),
        role: ChatRole.user,
        text: text,
        at: DateTime.now(),
        phase: MessagePhase.sent,
      ),
    );
    notifyListeners();

    final pendingId = beginAssistantPending();

    try {
      final reply = await bridgeChat(text);
      completeAssistant(pendingId, reply.text, attachments: reply.attachments);
    } catch (e) {
      if (_disposed) return;
      _failedText = text;
      final status = e is SmithBridgeException ? e.statusCode : null;
      _error = e is TimeoutException || status == 504
          ? 'The request timed out. Its outcome is unknown; Smith may have processed it. Nothing will be resent automatically.'
          : status == 429
          ? 'Smith is handling another turn. Wait for it to finish, then edit and send again. Nothing is resent automatically.'
          : status == 502 || status == 503
          ? 'Local inference failed. Check the preprovisioned model and retry connection, then edit and send again. No download or fallback is attempted.'
          : status == 400 || status == 413
          ? 'Smith rejected the message. Check its length and edit before sending again.'
          : 'Send failed. The request may not have completed. Retry connection, then edit and send again. Nothing will be resent automatically.';
      if (chatOnly && e is SmithBridgeException) {
        // Keep explicit backend reset/failure information instead of hiding it
        // behind generic retry guidance. An unacknowledged reset stays pending.
        _error = '${e.message}\n$_error';
      }
      if (chatOnly) {
        _messages.removeWhere((message) => message.id == pendingId);
      } else {
        failAssistant(pendingId, 'Send failed: $_error');
      }
    } finally {
      _sendBusy = false;
      if (!_disposed) notifyListeners();
    }
  }
}
