import 'dart:convert';

import 'package:http/http.dart' as http;

class KokoroException implements Exception {
  KokoroException(this.message, {this.statusCode});

  final String message;
  final int? statusCode;

  @override
  String toString() => message;
}

class KokoroHealth {
  KokoroHealth({
    required this.ok,
    required this.service,
    required this.backend,
    required this.voice,
    required this.voices,
    this.error,
  });

  final bool ok;
  final String service;
  final String backend;
  final String voice;
  final List<String> voices;
  final String? error;
}

class KokoroSpeakResult {
  KokoroSpeakResult({
    required this.path,
    required this.voice,
  });

  final String path;
  final String voice;
}

/// Loopback client for the local Kokoro TTS sidecar (`127.0.0.1:4791`).
class KokoroClient {
  KokoroClient({
    this.host = '127.0.0.1',
    this.port = 4791,
    http.Client? client,
  }) : _client = client ?? http.Client();

  final String host;
  final int port;
  final http.Client _client;

  Uri get _base => Uri(scheme: 'http', host: host, port: port);

  Future<KokoroHealth> health({
    Duration timeout = const Duration(seconds: 2),
  }) async {
    final res = await _client
        .get(_base.replace(path: '/v1/health'))
        .timeout(timeout);
    if (res.statusCode != 200) {
      throw KokoroException(
        'TTS health failed (${res.statusCode})',
        statusCode: res.statusCode,
      );
    }
    final json = jsonDecode(res.body) as Map<String, dynamic>;
    final voicesRaw = json['voices'];
    final voices = <String>[];
    if (voicesRaw is List) {
      for (final v in voicesRaw) {
        final s = v.toString();
        if (s.isNotEmpty) voices.add(s);
      }
    }
    return KokoroHealth(
      ok: json['ok'] == true,
      service: (json['service'] as String?) ?? 'smith-tts',
      backend: (json['backend'] as String?) ?? 'kokoro',
      voice: (json['voice'] as String?) ?? 'am_onyx',
      voices: voices,
      error: json['error'] as String?,
    );
  }

  Future<KokoroSpeakResult> speak(
    String text, {
    String? voice,
    double speed = 1.0,
    Duration timeout = const Duration(seconds: 120),
  }) async {
    final body = <String, dynamic>{
      'v': 1,
      'text': text,
      'speed': speed,
    };
    if (voice != null && voice.isNotEmpty) {
      body['voice'] = voice;
    }
    final res = await _client
        .post(
          _base.replace(path: '/v1/speak'),
          headers: {'Content-Type': 'application/json'},
          body: jsonEncode(body),
        )
        .timeout(timeout);

    if (res.statusCode != 200) {
      String detail = res.body;
      try {
        final err = jsonDecode(res.body) as Map<String, dynamic>;
        detail = (err['detail'] ?? err['error'] ?? res.body).toString();
      } catch (_) {}
      throw KokoroException(
        'TTS speak failed (${res.statusCode}): $detail',
        statusCode: res.statusCode,
      );
    }

    final json = jsonDecode(res.body) as Map<String, dynamic>;
    final path = ((json['path'] as String?) ?? '').trim();
    if (path.isEmpty) {
      throw KokoroException('TTS speak returned empty path');
    }
    return KokoroSpeakResult(
      path: path,
      voice: (json['voice'] as String?) ?? voice ?? '',
    );
  }

  void close() => _client.close();
}
