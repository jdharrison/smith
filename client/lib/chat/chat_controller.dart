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

/// First turn on launch / new session — smith reviews and greets.
const String kSessionKickoffPrompt =
    'Summarize current tasks and in-progress work if any, introduce yourself, '
    'and confirm ready status.';

/// Conversation + smith bridge. Voice turns mutate placeholders via ids.
class ChatController extends ChangeNotifier {
  ChatController({SmithBridge? bridge}) : _bridge = bridge ?? SmithBridge();

  final SmithBridge _bridge;
  final List<ChatMessage> _messages = <ChatMessage>[];
  final List<BoardTask> _tasks = <BoardTask>[];
  final Set<String> _injectedTaskIds = <String>{};
  int _msgSeq = 0;

  BridgeStatus _bridgeStatus = BridgeStatus.unknown;
  bool _bridgeEcho = false;
  String _isolation = 'sealed';
  PipelineStatus? _pipeline;
  bool _sendBusy = false;
  bool _voiceTurnBusy = false;
  String? _error;
  bool _newNext = false;
  bool _kickoffSent = false;
  Timer? _healthTimer;
  Timer? _taskPollTimer;
  int? _pollSince;
  bool _pollInFlight = false;
  BoardTask? _selectedTask;

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
  bool get canSend => !_sendBusy;
  bool get pollingTasks => _taskPollTimer != null;

  String _nextId(String prefix) {
    _msgSeq += 1;
    return '${prefix}_$_msgSeq';
  }

  Future<void> start() async {
    await refreshHealth();
    _healthTimer?.cancel();
    _healthTimer = Timer.periodic(const Duration(seconds: 4), (_) {
      unawaited(refreshHealth());
    });
    await refreshTasks();
    await _ensureKickoff();
  }

  @override
  void dispose() {
    _healthTimer?.cancel();
    _stopTaskPoll();
    _bridge.close();
    super.dispose();
  }

  Future<void> refreshHealth() async {
    try {
      final h = await _bridge.health();
      _bridgeStatus = h.ok ? BridgeStatus.online : BridgeStatus.offline;
      _bridgeEcho = h.echo;
      _isolation = h.isolation;
      if (_bridgeStatus == BridgeStatus.online &&
          _error != null &&
          _error!.startsWith('Bridge offline')) {
        _error = null;
      }
    } catch (_) {
      _bridgeStatus = BridgeStatus.offline;
    }
    notifyListeners();
  }

  Future<void> refreshTasks() async {
    if (_bridgeStatus != BridgeStatus.online) {
      await refreshHealth();
    }
    if (_bridgeStatus != BridgeStatus.online) return;
    try {
      final list = await _bridge.listTasks();
      _tasks
        ..clear()
        ..addAll(list);
      if (_selectedTask != null) {
        final id = _selectedTask!.id;
        for (final t in _tasks) {
          if (t.id == id) {
            _selectedTask = t;
            break;
          }
        }
      }
      notifyListeners();
    } catch (_) {
      // Keep last snapshot; poll will retry.
    }
  }

  void selectTask(BoardTask? task) {
    _selectedTask = task;
    notifyListeners();
  }

  Future<void> openTaskDetail(String id) async {
    try {
      final t = await _bridge.getTask(id);
      _selectedTask = t;
      final idx = _tasks.indexWhere((x) => x.id == id);
      if (idx >= 0) {
        _tasks[idx] = t;
      } else {
        _tasks.insert(0, t);
      }
      notifyListeners();
    } catch (e) {
      _error = e.toString();
      notifyListeners();
    }
  }

  /// Clear transcript and start a fresh brain session with the same kickoff.
  Future<void> requestNewSession() async {
    if (busy) return;
    _stopTaskPoll();
    _newNext = true;
    _kickoffSent = false;
    _messages.clear();
    _injectedTaskIds.clear();
    _error = null;
    notifyListeners();
    await _ensureKickoff();
  }

  Future<void> openAttachment(ChatAttachment att) async {
    if (att.path.isEmpty) return;
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
    m.text = text.trim().isEmpty ? '(empty reply)' : text.trim();
    m.phase = MessagePhase.complete;
    m.attachments = attachments;
    notifyListeners();
  }

  void failAssistant(String id, String errorText) {
    final m = messageById(id);
    if (m == null) return;
    m.text = errorText;
    m.phase = MessagePhase.complete;
    notifyListeners();
  }

  void _noteTaskFromReply(BoardTask? task) {
    if (task == null) return;
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
    if (_taskPollTimer != null) return;
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
    if (_bridgeStatus != BridgeStatus.online) return;
    if (_pollInFlight) return;
    _pollInFlight = true;
    try {
      final list = await _bridge.listTasks(since: _pollSince);
      // Merge into local cache (full refresh for Tasks tab).
      await refreshTasks();
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
        _messages.add(
          ChatMessage(
            id: _nextId('smith'),
            role: ChatRole.assistant,
            text: body,
            at: DateTime.now(),
          ),
        );
        notifyListeners();
        final speak = onSpeakAssistant;
        if (speak != null) {
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
    } catch (_) {
      // Transient; next tick retries.
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

    if (_bridgeStatus != BridgeStatus.online) {
      await refreshHealth();
    }
    if (_bridgeStatus != BridgeStatus.online) {
      throw SmithBridgeException(
        'Bridge offline — start `smith serve` on :4783',
      );
    }

    final reset = _newNext;
    _newNext = false;
    final reply = await _bridge.chat(
      trimmed,
      newSession: reset,
    );
    _isolation = reply.isolation;
    if (reply.pipeline != null) {
      _pipeline = reply.pipeline;
    }
    _noteTaskFromReply(reply.task);
    return reply;
  }

  /// Ask bridge to start on-demand Whisper/TTS sidecars if needed.
  Future<void> ensureVoiceSidecars() async {
    if (_bridgeStatus != BridgeStatus.online) {
      await refreshHealth();
    }
    if (_bridgeStatus != BridgeStatus.online) return;
    await _bridge.ensureSidecars();
  }

  /// Mark busy for an external voice-turn pipeline (independent of text send).
  void setTurnBusy(bool value) {
    _voiceTurnBusy = value;
    notifyListeners();
  }

  /// Wait until a non-voice `send` finishes (e.g. kickoff).
  /// Returns false if still busy after [maxWaits] polls.
  Future<bool> waitForSendIdle({int maxWaits = 120}) async {
    var waits = 0;
    while (_sendBusy && waits < maxWaits) {
      await Future<void>.delayed(const Duration(milliseconds: 100));
      waits++;
    }
    return !_sendBusy;
  }

  Future<void> _ensureKickoff() async {
    if (_kickoffSent || _sendBusy) return;

    for (var i = 0; i < 8; i++) {
      if (_bridgeStatus == BridgeStatus.online) break;
      await refreshHealth();
      if (_bridgeStatus == BridgeStatus.online) break;
      await Future<void>.delayed(const Duration(milliseconds: 500));
    }

    if (_bridgeStatus != BridgeStatus.online) {
      _messages.add(
        ChatMessage(
          id: _nextId('sys'),
          role: ChatRole.system,
          text:
              'Bridge offline — start `smith serve`, then tap refresh to engage.',
          at: DateTime.now(),
        ),
      );
      notifyListeners();
      return;
    }

    _kickoffSent = true;
    await send(kSessionKickoffPrompt, hideInChat: true);
  }

  Future<void> send(String raw, {bool hideInChat = false}) async {
    final text = raw.trim();
    if (text.isEmpty || _sendBusy) return;

    _sendBusy = true;
    _error = null;
    if (!hideInChat) {
      _messages.add(
        ChatMessage(
          id: _nextId('user'),
          role: ChatRole.user,
          text: text,
          at: DateTime.now(),
          phase: MessagePhase.sent,
        ),
      );
    }
    notifyListeners();

    String? pendingId;
    if (!hideInChat) {
      pendingId = beginAssistantPending();
    }

    try {
      final reply = await bridgeChat(text);
      if (pendingId != null) {
        completeAssistant(
          pendingId,
          reply.text,
          attachments: reply.attachments,
        );
      } else {
        _messages.add(
          ChatMessage(
            id: _nextId('smith'),
            role: ChatRole.assistant,
            text: reply.text.trim().isEmpty
                ? '(empty reply)'
                : reply.text.trim(),
            at: DateTime.now(),
            attachments: reply.attachments,
          ),
        );
        notifyListeners();
      }
    } catch (e) {
      _error = e.toString();
      if (text == kSessionKickoffPrompt) {
        _kickoffSent = false;
      }
      if (pendingId != null) {
        failAssistant(pendingId, 'Send failed: $_error');
      } else {
        _messages.add(
          ChatMessage(
            id: _nextId('sys'),
            role: ChatRole.system,
            text: 'Send failed: $_error',
            at: DateTime.now(),
          ),
        );
      }
    } finally {
      _sendBusy = false;
      notifyListeners();
    }
  }
}
