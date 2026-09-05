import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:audioplayers/audioplayers.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter_tts/flutter_tts.dart';
import 'package:smith_client/listen/kokoro_client.dart';

/// Text-to-speech facade: prefers local Kokoro sidecar (`:4791`), else OS TTS.
class SmithTts {
  SmithTts({KokoroClient? kokoro})
    : _kokoro = kokoro ?? KokoroClient(),
      _fallback = Platform.isWindows
          ? _WindowsSapiTts()
          : Platform.isLinux
          ? _UnavailableTts()
          : _FlutterPluginTts() {
    _player = AudioPlayer();
    unawaited(refreshVoices());
  }

  final KokoroClient _kokoro;
  final _FallbackTts _fallback;
  late final AudioPlayer _player;

  bool _enabled = true;
  bool _disposed = false;
  bool _kokoroOnline = false;
  double _rate = 0.45;
  List<String> _voices = const [];
  String? _voice = 'am_onyx';

  bool get enabled => _enabled;
  bool get kokoroOnline => _kokoroOnline;
  double get rate => _rate;

  String get backendLabel => _kokoroOnline ? 'kokoro' : _fallback.backendLabel;

  List<String> get voices => _voices;
  String? get voice => _voice;

  void setEnabled(bool value) {
    _enabled = value;
    _fallback.setEnabled(value);
    if (!value) {
      // ignore: discarded_futures
      stop();
    }
  }

  void toggleEnabled() => setEnabled(!_enabled);

  Future<void> setRate(double value) async {
    _rate = value.clamp(0.0, 1.0);
    await _fallback.setRate(_rate);
  }

  Future<void> refreshVoices() async {
    try {
      final h = await _kokoro.health();
      _kokoroOnline = h.ok;
      if (h.ok && h.voices.isNotEmpty) {
        _voices = h.voices;
        if (_voice == null || !_voices.contains(_voice)) {
          _voice = h.voices.contains('am_onyx') ? 'am_onyx' : h.voice;
        }
        return;
      }
    } catch (_) {
      _kokoroOnline = false;
    }
    await _fallback.refreshVoices();
    _voices = _fallback.voices;
    _voice = _fallback.voice;
  }

  Future<void> setVoice(String? name) async {
    if (name == null || name.isEmpty) return;
    _voice = name;
    if (!_kokoroOnline) {
      await _fallback.setVoice(name);
    }
  }

  Future<void> speak(String text) async {
    if (_disposed || !_enabled) return;
    final cleaned = cleanForSpeech(text);
    if (cleaned.isEmpty) return;
    await stop();
    if (_disposed || !_enabled) return;

    // Prefer Kokoro when healthy (refresh lightly).
    if (!_kokoroOnline) {
      try {
        final h = await _kokoro.health(
          timeout: const Duration(milliseconds: 800),
        );
        _kokoroOnline = h.ok;
        if (h.ok && h.voices.isNotEmpty) {
          _voices = h.voices;
        }
      } catch (_) {
        _kokoroOnline = false;
      }
    }

    if (_kokoroOnline) {
      try {
        // Map UI rate 0..1 → Kokoro speed ~0.7..1.3
        final speed = (0.7 + _rate * 0.6).clamp(0.5, 2.0);
        final result = await _kokoro.speak(
          cleaned,
          voice: _voice,
          speed: speed,
        );
        await _player.stop();
        await _player.play(DeviceFileSource(result.path));
        return;
      } catch (e) {
        debugPrint('smith TTS kokoro failed, falling back: $e');
        _kokoroOnline = false;
      }
    }

    await _fallback.speak(cleaned);
  }

  Future<void> stop() async {
    try {
      await _player.stop();
    } catch (_) {}
    await _fallback.stop();
  }

  Future<void> dispose() async {
    _disposed = true;
    try {
      await _player.dispose();
    } catch (_) {}
    _kokoro.close();
    await _fallback.dispose();
  }

  /// Light cleanup so the voice does not read raw markdown / intent fences.
  static String cleanForSpeech(String raw) {
    var s = raw.trim();
    if (s.isEmpty) return '';
    if (s == '(empty reply)') return '';

    s = s.replaceAll(RegExp(r'```[\s\S]*?```'), ' ');

    final out = StringBuffer();
    for (final line in s.split('\n')) {
      final t = line.trim();
      if (t.startsWith(':::')) continue;
      var cleaned = t
          .replaceAll(RegExp(r'^#{1,6}\s*'), '')
          .replaceAll('`', '')
          .replaceAll('**', '')
          .replaceAll('__', '')
          .replaceAll('*', '')
          .trim();
      if (cleaned.isEmpty) continue;
      if (out.isNotEmpty) out.write(' ');
      out.write(cleaned);
    }

    return out.toString().replaceAll(RegExp(r'\s+'), ' ').trim();
  }
}

abstract class _FallbackTts {
  String get backendLabel;
  List<String> get voices;
  String? get voice;
  void setEnabled(bool value);
  Future<void> setRate(double value);
  Future<void> refreshVoices();
  Future<void> setVoice(String? name);
  Future<void> speak(String text);
  Future<void> stop();
  Future<void> dispose();
}

String? _pickPreferredVoice(List<String> names) {
  if (names.isEmpty) return null;
  String? match(bool Function(String) test) {
    for (final n in names) {
      if (test(n.toLowerCase())) return n;
    }
    return null;
  }

  return match((n) => n.contains('natural')) ??
      match((n) => n.contains('neural')) ??
      match((n) => n.contains('online')) ??
      match((n) => n.contains('zira')) ??
      match((n) => n.contains('aria')) ??
      match((n) => n.contains('jenny')) ??
      match((n) => !n.contains('david') && !n.contains('sam')) ??
      names.first;
}

class _UnavailableTts implements _FallbackTts {
  bool _enabled = true;

  @override
  String get backendLabel => 'unavailable (Kokoro required)';

  @override
  List<String> get voices => const [];

  @override
  String? get voice => null;

  @override
  void setEnabled(bool value) => _enabled = value;

  @override
  Future<void> setRate(double value) async {}

  @override
  Future<void> refreshVoices() async {}

  @override
  Future<void> setVoice(String? name) async {}

  @override
  Future<void> speak(String text) async {
    if (!_enabled || text.trim().isEmpty) return;
  }

  @override
  Future<void> stop() async {}

  @override
  Future<void> dispose() async {}
}

class _WindowsSapiTts implements _FallbackTts {
  _WindowsSapiTts() {
    unawaited(refreshVoices());
  }

  bool _enabled = true;
  bool _disposed = false;
  double _rate = 0.45;
  Process? _proc;
  List<String> _voices = const [];
  String? _voice;

  @override
  String get backendLabel => 'Windows SAPI (Desktop)';

  @override
  List<String> get voices => _voices;

  @override
  String? get voice => _voice;

  @override
  void setEnabled(bool value) {
    _enabled = value;
    if (!value) {
      // ignore: discarded_futures
      stop();
    }
  }

  @override
  Future<void> setRate(double value) async {
    _rate = value.clamp(0.0, 1.0);
  }

  @override
  Future<void> setVoice(String? name) async {
    if (name == null || name.isEmpty) {
      _voice = _pickPreferredVoice(_voices);
      return;
    }
    if (_voices.contains(name)) {
      _voice = name;
    }
  }

  @override
  Future<void> refreshVoices() async {
    try {
      final r = await Process.run('powershell', [
        '-NoProfile',
        '-NonInteractive',
        '-WindowStyle',
        'Hidden',
        '-Command',
        'Add-Type -AssemblyName System.Speech; '
            '\$s = New-Object System.Speech.Synthesis.SpeechSynthesizer; '
            '\$s.GetInstalledVoices() | ForEach-Object { \$_.VoiceInfo.Name }',
      ]);
      if (r.exitCode != 0) return;
      final list = (r.stdout as String)
          .split(RegExp(r'\r?\n'))
          .map((s) => s.trim())
          .where((s) => s.isNotEmpty)
          .toList();
      _voices = list;
      _voice ??= _pickPreferredVoice(list);
      if (_voice != null && !list.contains(_voice)) {
        _voice = _pickPreferredVoice(list);
      }
    } catch (e) {
      debugPrint('smith TTS voice list failed: $e');
    }
  }

  int get _sapiRate => ((_rate * 20) - 10).round().clamp(-10, 10);

  @override
  Future<void> speak(String text) async {
    if (_disposed || !_enabled) return;
    final cleaned = text.trim();
    if (cleaned.isEmpty) return;
    await stop();
    if (_disposed || !_enabled) return;

    final b64 = base64Encode(utf8.encode(cleaned));
    final voiceName = _voice;
    final voiceEscaped = voiceName == null
        ? ''
        : voiceName.replaceAll("'", "''");
    final voiceSelect = voiceEscaped.isEmpty
        ? ''
        : "try { \$s.SelectVoice('$voiceEscaped') } catch {} ";
    try {
      _proc = await Process.start('powershell', [
        '-NoProfile',
        '-NonInteractive',
        '-WindowStyle',
        'Hidden',
        '-Command',
        'Add-Type -AssemblyName System.Speech; '
            '\$s = New-Object System.Speech.Synthesis.SpeechSynthesizer; '
            '$voiceSelect'
            '\$s.Rate = $_sapiRate; '
            '\$t = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String(\'$b64\')); '
            '\$s.Speak(\$t);',
      ], mode: ProcessStartMode.normal);
      unawaited(
        _proc!.exitCode.then((_) {
          _proc = null;
        }),
      );
    } catch (e) {
      debugPrint('smith TTS (SAPI) failed: $e');
      _proc = null;
    }
  }

  @override
  Future<void> stop() async {
    final p = _proc;
    _proc = null;
    if (p == null) return;
    try {
      p.kill(ProcessSignal.sigterm);
    } catch (_) {
      try {
        p.kill();
      } catch (_) {}
    }
  }

  @override
  Future<void> dispose() async {
    _disposed = true;
    await stop();
  }
}

class _FlutterPluginTts implements _FallbackTts {
  _FlutterPluginTts() {
    _ready = _init();
  }

  final FlutterTts _tts = FlutterTts();
  late final Future<void> _ready;
  bool _enabled = true;
  bool _disposed = false;
  double _rate = 0.45;
  List<String> _voices = const [];
  String? _voice;

  @override
  String get backendLabel =>
      Platform.isMacOS ? 'flutter_tts (AVSpeech)' : 'flutter_tts';

  @override
  List<String> get voices => _voices;

  @override
  String? get voice => _voice;

  Future<void> _init() async {
    try {
      await _onUi(() async {
        await _tts.awaitSpeakCompletion(false);
        await _tts.setSpeechRate(_rate);
        await _tts.setVolume(1.0);
        await _tts.setPitch(1.0);
      });
      await refreshVoices();
    } catch (_) {}
  }

  @override
  void setEnabled(bool value) {
    _enabled = value;
    if (!value) {
      // ignore: discarded_futures
      stop();
    }
  }

  @override
  Future<void> setRate(double value) async {
    _rate = value.clamp(0.0, 1.0);
    await _ready;
    try {
      await _onUi(() => _tts.setSpeechRate(_rate));
    } catch (_) {}
  }

  @override
  Future<void> refreshVoices() async {
    await _ready;
    try {
      final raw = await _onUi(() => _tts.getVoices);
      final names = <String>[];
      if (raw is List) {
        for (final item in raw) {
          if (item is Map) {
            final n = item['name']?.toString();
            if (n != null && n.isNotEmpty) names.add(n);
          } else if (item is String && item.isNotEmpty) {
            names.add(item);
          }
        }
      }
      _voices = names;
      _voice ??= _pickPreferredVoice(names);
      if (_voice != null) {
        await _onUi(() => _tts.setVoice({'name': _voice!, 'locale': 'en-US'}));
      }
    } catch (e) {
      debugPrint('smith TTS voice list failed: $e');
    }
  }

  @override
  Future<void> setVoice(String? name) async {
    if (name == null || name.isEmpty) return;
    _voice = name;
    await _ready;
    try {
      await _onUi(() => _tts.setVoice({'name': name, 'locale': 'en-US'}));
    } catch (_) {}
  }

  @override
  Future<void> speak(String text) async {
    if (_disposed || !_enabled) return;
    final cleaned = text.trim();
    if (cleaned.isEmpty) return;
    await _ready;
    if (_disposed || !_enabled) return;
    try {
      await _onUi(() async {
        await _tts.stop();
        await _tts.speak(cleaned);
      });
    } catch (_) {}
  }

  @override
  Future<void> stop() async {
    if (_disposed) return;
    try {
      await _onUi(() => _tts.stop());
    } catch (_) {}
  }

  @override
  Future<void> dispose() async {
    _disposed = true;
    try {
      await _onUi(() => _tts.stop());
    } catch (_) {}
  }

  Future<T> _onUi<T>(FutureOr<T> Function() fn) {
    final completer = Completer<T>();
    SchedulerBinding.instance.scheduleTask(() async {
      try {
        completer.complete(await fn());
      } catch (e, st) {
        completer.completeError(e, st);
      }
    }, Priority.touch);
    return completer.future;
  }
}
