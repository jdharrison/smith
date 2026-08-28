import 'dart:async';
import 'dart:io';

import 'package:audioplayers/audioplayers.dart';
import 'package:flutter/foundation.dart';
import 'package:path_provider/path_provider.dart';
import 'package:record/record.dart';
import 'package:smith_client/chat/chat_controller.dart';
import 'package:smith_client/listen/smith_tts.dart';
import 'package:smith_client/listen/whisper_client.dart';

/// Max clip length while holding the record control.
const Duration kMaxHoldRecord = Duration(seconds: 30);

const Duration _kTypewriterStep = Duration(milliseconds: 18);

/// Full voice-turn lifecycle (mic + Whisper transcript + smith reply).
enum VoiceTurnState {
  idle,
  starting,
  recording,
  processing,
  replying,
  stopping,
}

/// Hold scopes one WAV recording; [processing] transcribes via local Whisper
/// sidecar, then sends to smith. Record is only enabled in [idle].
class VoiceTurnController extends ChangeNotifier {
  VoiceTurnController({
    required ChatController chat,
    WhisperClient? whisper,
    SmithTts? tts,
  }) : _chat = chat,
       _whisper = whisper ?? WhisperClient(),
       _tts = tts ?? SmithTts() {
    _recorder = AudioRecorder();
    _player = AudioPlayer();
    _chat.onSpeakAssistant = (text) => _tts.speak(text);
    _player.onPlayerComplete.listen((_) {
      if (_disposed) return;
      _playingPath = null;
      notifyListeners();
    });
    unawaited(refreshWhisperHealth());
    _healthTimer = Timer.periodic(const Duration(seconds: 5), (_) {
      unawaited(refreshWhisperHealth());
    });
  }

  final ChatController _chat;
  final WhisperClient _whisper;
  final SmithTts _tts;
  late final AudioRecorder _recorder;
  late final AudioPlayer _player;
  Timer? _healthTimer;
  bool _disposed = false;

  VoiceTurnState _state = VoiceTurnState.idle;
  bool _wantDown = false;
  bool _clipRecording = false;
  bool _whisperOnline = false;
  String? _whisperModel;

  int _sessionId = 0;
  String? _activeClipPath;
  String? _error;
  Duration _holdElapsed = Duration.zero;
  Timer? _holdTimer;
  String? _playingPath;

  VoiceTurnState get state => _state;
  bool get isIdle => _state == VoiceTurnState.idle;
  /// Mic only from idle with Whisper reachable (avoids dead-end holds).
  bool get canRecord =>
      _state == VoiceTurnState.idle && _whisperOnline && !_chat.busy;
  bool get isRecording =>
      _state == VoiceTurnState.starting || _state == VoiceTurnState.recording;
  bool get holding => _state == VoiceTurnState.recording;
  String? get error => _error;
  /// Live partials removed (no WinRT); kept for UI compatibility.
  String get partial => '';
  Duration get holdElapsed => _holdElapsed;
  double get holdProgress =>
      (_holdElapsed.inMilliseconds / kMaxHoldRecord.inMilliseconds)
          .clamp(0.0, 1.0);
  String? get playingPath => _playingPath;
  bool isPlaying(String? path) =>
      path != null && path.isNotEmpty && _playingPath == path;
  bool get whisperOnline => _whisperOnline;
  String? get whisperModel => _whisperModel;
  bool get ttsEnabled => _tts.enabled;
  double get ttsRate => _tts.rate;
  String get ttsBackend => _tts.backendLabel;
  bool get ttsKokoroOnline => _tts.kokoroOnline;
  String? get ttsVoice => _tts.voice;
  List<String> get ttsVoices => _tts.voices;

  void toggleTts() {
    _tts.toggleEnabled();
    _safeNotify();
  }

  void setTtsEnabled(bool value) {
    if (_tts.enabled == value) return;
    _tts.setEnabled(value);
    _safeNotify();
  }

  Future<void> setTtsRate(double value) async {
    await _tts.setRate(value);
    _safeNotify();
  }

  Future<void> setTtsVoice(String? name) async {
    await _tts.setVoice(name);
    _safeNotify();
  }

  Future<void> refreshTtsVoices() async {
    await _tts.refreshVoices();
    _safeNotify();
  }

  Future<void> testTts([String? phrase]) async {
    await _tts.speak(
      phrase ??
          'Smith speech check. Voice ${ttsVoice ?? "default"}, rate ${(ttsRate * 100).round()} percent.',
    );
  }

  Future<void> stopTts() => _tts.stop();

  bool get statusGreen =>
      _state == VoiceTurnState.starting || _state == VoiceTurnState.recording;
  bool get statusBlue =>
      _state == VoiceTurnState.processing ||
      _state == VoiceTurnState.replying ||
      _state == VoiceTurnState.stopping;
  bool get statusRed => _state == VoiceTurnState.idle;

  String get statusLabel => switch (_state) {
        VoiceTurnState.idle => _whisperOnline
            ? (_chat.busy ? 'brain busy' : 'idle')
            : 'whisper offline',
        VoiceTurnState.starting => 'starting',
        VoiceTurnState.recording => 'recording',
        VoiceTurnState.processing => 'processing',
        VoiceTurnState.replying => 'replying',
        VoiceTurnState.stopping => 'stopping',
      };

  Future<void> refreshWhisperHealth() async {
    if (_disposed) return;
    try {
      await _chat.ensureVoiceSidecars();
    } catch (_) {}
    try {
      final h = await _whisper.health();
      if (_disposed) return;
      _whisperOnline = h.ok;
      _whisperModel = h.model;
      // Only surface / clear sidecar errors while idle — never clobber turn errors.
      if (_state == VoiceTurnState.idle) {
        if (h.ok) {
          if (_error != null &&
              (_error!.startsWith('Whisper') ||
                  _error!.contains('sidecar') ||
                  _error!.contains('4790'))) {
            _error = null;
          }
        } else {
          _error = h.error ?? 'Whisper sidecar not ready';
        }
      }
    } catch (_) {
      if (_disposed) return;
      _whisperOnline = false;
      if (_state == VoiceTurnState.idle) {
        _error = 'Whisper offline — start client/sidecar (port 4790)';
      }
    }
    _safeNotify();
  }

  void onPointerDown() {
    _wantDown = true;
    if (!canRecord) return;
    unawaited(_handlePressed());
  }

  void onPointerUp() {
    _wantDown = false;
    if (_state == VoiceTurnState.starting) {
      // Abort in-flight start; do not create a chat bubble.
      unawaited(_abortStarting());
      return;
    }
    unawaited(_handleReleased());
  }

  Future<void> togglePlay(String path) async {
    if (path.isEmpty || !File(path).existsSync()) return;
    if (_playingPath == path) {
      await stopPlayback();
      return;
    }
    try {
      await _player.stop();
      await _player.play(DeviceFileSource(path));
      _playingPath = path;
      _safeNotify();
    } catch (e) {
      _error = 'Playback failed: $e';
      _playingPath = null;
      _safeNotify();
    }
  }

  Future<void> stopPlayback() async {
    try {
      await _player.stop();
    } catch (_) {}
    if (_playingPath != null) {
      _playingPath = null;
      _safeNotify();
    }
  }

  Future<void> _handlePressed() async {
    if (_state != VoiceTurnState.idle) return;
    _setState(VoiceTurnState.starting);
    _error = null;
    _holdElapsed = Duration.zero;
    await _tts.stop();
    await stopPlayback();

    try {
      await refreshWhisperHealth();
      if (!_wantDown || _disposed) {
        await _abortStarting();
        return;
      }
      if (!_whisperOnline) {
        _error = 'Whisper offline — start client/sidecar (port 4790)';
        _setState(VoiceTurnState.idle);
        return;
      }

      final permitted = await _recorder.hasPermission();
      if (!_wantDown || _disposed) {
        await _abortStarting();
        return;
      }
      if (!permitted) {
        _error = 'Microphone permission denied';
        _setState(VoiceTurnState.idle);
        return;
      }

      _sessionId += 1;
      final id = _sessionId;

      await _startClipCapture(id);
      if (!_wantDown || _state != VoiceTurnState.starting || _disposed) {
        await _cancelCaptureQuietly();
        if (_state == VoiceTurnState.starting) {
          _setState(VoiceTurnState.idle);
        }
        return;
      }
      if (!_clipRecording) {
        _error = 'Could not start audio capture';
        _setState(VoiceTurnState.idle);
        return;
      }

      _holdTimer?.cancel();
      _holdTimer = Timer.periodic(const Duration(milliseconds: 50), (_) {
        if (_disposed) return;
        if (_state != VoiceTurnState.recording || id != _sessionId) return;
        _holdElapsed += const Duration(milliseconds: 50);
        if (_holdElapsed >= kMaxHoldRecord) {
          _wantDown = false;
          unawaited(_handleReleased());
        } else {
          _safeNotify();
        }
      });

      _setState(VoiceTurnState.recording);
      if (!_wantDown) {
        await _handleReleased();
      }
    } catch (e) {
      _error = e.toString();
      await _cancelCaptureQuietly();
      _holdTimer?.cancel();
      _holdTimer = null;
      if (_state == VoiceTurnState.starting ||
          _state == VoiceTurnState.recording) {
        _setState(VoiceTurnState.idle);
      }
    }
  }

  Future<void> _handleReleased() async {
    if (_state != VoiceTurnState.recording) return;

    final elapsed = _holdElapsed;
    _setState(VoiceTurnState.processing);
    _chat.setTurnBusy(true);
    _holdTimer?.cancel();
    _holdTimer = null;

    try {
      // Mic off — clip is the sole capture artifact for this session.
      final clipPath = await _stopClipCapture(elapsed: elapsed);
      final userId = _chat.beginVoiceUserMessage(audioPath: clipPath);

      if (clipPath == null) {
        _chat.failVoiceUserMessage(userId, '(no clip captured)');
        return;
      }

      // Tiny taps: still a file, but almost no samples — fail fast.
      if (elapsed < const Duration(milliseconds: 200)) {
        _chat.failVoiceUserMessage(userId, '(hold longer to capture speech)');
        return;
      }

      final String transcript;
      try {
        transcript = await _whisper.transcribe(clipPath);
      } catch (e) {
        _error = e.toString();
        _chat.failVoiceUserMessage(
          userId,
          '(whisper failed — play clip to diagnose)',
        );
        return;
      }

      if (transcript.isEmpty) {
        _chat.failVoiceUserMessage(
          userId,
          '(empty transcript — play clip to diagnose)',
        );
        return;
      }

      _chat.setMessagePhase(userId, MessagePhase.typing);
      await _typewriter(userId, transcript);

      _chat.setMessagePhase(userId, MessagePhase.sent);

      _setState(VoiceTurnState.replying);
      final assistantId = _chat.beginAssistantPending();

      try {
        final idle = await _chat.waitForSendIdle(maxWaits: 300);
        if (!idle) {
          _chat.failAssistant(
            assistantId,
            'Brain still busy — try again in a moment',
          );
          return;
        }
        final reply = await _chat.bridgeChat(transcript);
        _chat.completeAssistant(
          assistantId,
          reply.text,
          attachments: reply.attachments,
        );
        unawaited(_tts.speak(reply.text));
      } catch (e) {
        _chat.failAssistant(assistantId, 'Send failed: $e');
        _error = e.toString();
      }
    } finally {
      await _enterStoppingThenIdle();
    }
  }

  Future<void> _enterStoppingThenIdle() async {
    if (_disposed) {
      _chat.setTurnBusy(false);
      return;
    }
    _setState(VoiceTurnState.stopping);
    _holdElapsed = Duration.zero;
    _activeClipPath = null;
    await Future<void>.delayed(const Duration(milliseconds: 40));
    _chat.setTurnBusy(false);
    _setState(VoiceTurnState.idle);
  }

  Future<void> _abortStarting() async {
    await _cancelCaptureQuietly();
    _holdTimer?.cancel();
    _holdTimer = null;
    if (_state == VoiceTurnState.starting) {
      _setState(VoiceTurnState.idle);
    }
  }

  Future<void> _typewriter(String messageId, String full) async {
    if (full.isEmpty) return;
    final buf = StringBuffer();
    for (var i = 0; i < full.length; i++) {
      if (_disposed || _state != VoiceTurnState.processing) return;
      buf.write(full[i]);
      _chat.setMessageText(messageId, buf.toString());
      final delay = full.length > 80 ? 8 : _kTypewriterStep.inMilliseconds;
      await Future<void>.delayed(Duration(milliseconds: delay));
    }
  }

  Future<void> _startClipCapture(int sessionId) async {
    _activeClipPath = null;
    _clipRecording = false;
    try {
      final dir = await getTemporaryDirectory();
      final path =
          '${dir.path}${Platform.pathSeparator}smith_clip_$sessionId.wav';
      await _recorder.start(
        const RecordConfig(
          encoder: AudioEncoder.wav,
          sampleRate: 16000,
          numChannels: 1,
          bitRate: 256000,
          autoGain: false,
          echoCancel: false,
          noiseSuppress: false,
        ),
        path: path,
      );
      _activeClipPath = path;
      _clipRecording = true;
    } catch (e) {
      _clipRecording = false;
      _activeClipPath = null;
      _error = 'Clip capture failed: $e';
      debugPrint(_error);
    }
  }

  Future<String?> _stopClipCapture({Duration? elapsed}) async {
    if (!_clipRecording) {
      _clipRecording = false;
      return _usableClip(_activeClipPath);
    }
    _clipRecording = false;
    try {
      final stopped = await _recorder.stop();
      final path = stopped ?? _activeClipPath;
      _activeClipPath = path;
      return _usableClip(path);
    } catch (e) {
      debugPrint('Clip stop failed: $e');
      return _usableClip(_activeClipPath);
    }
  }

  Future<String?> _usableClip(String? path) async {
    if (path == null) return null;
    final file = File(path);
    if (!await file.exists()) return null;
    final bytes = await file.length();
    if (bytes < 44) return null;
    return path;
  }

  Future<void> _cancelCaptureQuietly() async {
    try {
      if (_clipRecording) {
        await _recorder.stop();
      }
    } catch (_) {}
    _clipRecording = false;
    final path = _activeClipPath;
    _activeClipPath = null;
    if (path != null) {
      try {
        final f = File(path);
        if (await f.exists()) await f.delete();
      } catch (_) {}
    }
  }

  void _setState(VoiceTurnState next) {
    if (_disposed || _state == next) return;
    _state = next;
    _safeNotify();
  }

  void _safeNotify() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _healthTimer?.cancel();
    _holdTimer?.cancel();
    _chat.onSpeakAssistant = null;
    _chat.setTurnBusy(false);
    _whisper.close();
    unawaited(_tts.dispose());
    unawaited(_recorder.dispose());
    unawaited(_player.dispose());
    super.dispose();
  }
}
