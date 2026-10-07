import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/bridge/smith_bridge.dart';
import 'package:smith_client/chat/chat_controller.dart';
import 'package:smith_client/listen/kokoro_client.dart';
import 'package:smith_client/listen/smith_tts.dart';
import 'package:smith_client/listen/voice_turn.dart';
import 'package:smith_client/listen/whisper_client.dart';

class FakeVoiceBridge extends SmithBridge {
  FakeVoiceBridge(this.operations, {this.ensureError});

  final List<String> operations;
  final Object? ensureError;

  @override
  Future<HealthInfo> health({
    Duration timeout = const Duration(seconds: 2),
  }) async {
    operations.add('bridgeHealth');
    return HealthInfo(
      ok: true,
      service: 'smith-bridge',
      echo: false,
      isolation: 'sealed',
    );
  }

  @override
  Future<void> ensureSidecars({
    Duration timeout = const Duration(seconds: 120),
  }) async {
    operations.add('ensureSidecars');
    if (ensureError != null) throw ensureError!;
  }

  @override
  void close() {}
}

class FakeWhisperClient extends WhisperClient {
  FakeWhisperClient(this.operations, {required this.online});

  final List<String> operations;
  final bool online;

  @override
  Future<WhisperHealth> health({
    Duration timeout = const Duration(seconds: 2),
  }) async {
    operations.add('whisperHealth');
    return WhisperHealth(
      ok: online,
      service: 'smith-whisper',
      model: online ? 'test-model' : '',
      error: online ? null : 'Whisper sidecar not ready',
    );
  }

  @override
  void close() {}
}

class FakeKokoroClient extends KokoroClient {
  @override
  Future<KokoroHealth> health({
    Duration timeout = const Duration(seconds: 2),
  }) async => KokoroHealth(
    ok: false,
    service: 'smith-tts',
    backend: 'kokoro',
    voice: '',
    voices: const [],
  );

  @override
  void close() {}
}

class QuietSmithTts extends SmithTts {
  QuietSmithTts() : super(kokoro: FakeKokoroClient());

  @override
  Future<void> stop() async {}

  @override
  Future<void> dispose() async {}
}

Future<void> settleAsyncWork() => pumpEventQueue(times: 20);

void main() {
  test('Whisper startup probes are observational', () async {
    final operations = <String>[];
    final bridge = FakeVoiceBridge(operations);
    final chat = ChatController(bridge: bridge);
    await chat.refreshHealth();
    operations.clear();

    final voice = VoiceTurnController(
      chat: chat,
      whisper: FakeWhisperClient(operations, online: false),
      tts: QuietSmithTts(),
    );
    addTearDown(chat.dispose);
    addTearDown(voice.dispose);
    await settleAsyncWork();

    expect(operations, ['whisperHealth']);
    expect(voice.whisperOnline, isFalse);
    expect(voice.canRecord, isTrue);
  });

  test('explicit mic start ensures sidecars before probing Whisper', () async {
    final operations = <String>[];
    final bridge = FakeVoiceBridge(operations);
    final chat = ChatController(bridge: bridge);
    await chat.refreshHealth();
    final voice = VoiceTurnController(
      chat: chat,
      whisper: FakeWhisperClient(operations, online: false),
      tts: QuietSmithTts(),
    );
    addTearDown(chat.dispose);
    addTearDown(voice.dispose);
    await settleAsyncWork();
    operations.clear();

    voice.onPointerDown();
    await settleAsyncWork();

    expect(operations, ['ensureSidecars', 'whisperHealth']);
    expect(voice.state, VoiceTurnState.idle);
    expect(voice.error, 'Whisper sidecar not ready');
    voice.onPointerUp();
  });

  test('explicit sidecar start failure is visible', () async {
    final operations = <String>[];
    final bridge = FakeVoiceBridge(
      operations,
      ensureError: SmithBridgeException('sidecar launch rejected'),
    );
    final chat = ChatController(bridge: bridge);
    await chat.refreshHealth();
    final voice = VoiceTurnController(
      chat: chat,
      whisper: FakeWhisperClient(operations, online: false),
      tts: QuietSmithTts(),
    );
    addTearDown(chat.dispose);
    addTearDown(voice.dispose);
    await settleAsyncWork();
    operations.clear();

    voice.onPointerDown();
    await settleAsyncWork();

    expect(operations, ['ensureSidecars']);
    expect(voice.state, VoiceTurnState.idle);
    expect(voice.error, contains('sidecar launch rejected'));
    voice.onPointerUp();
  });
}
