import 'dart:convert';

import 'package:http/http.dart' as http;

class WhisperException implements Exception {
  WhisperException(this.message, {this.statusCode});

  final String message;
  final int? statusCode;

  @override
  String toString() => message;
}

class WhisperHealth {
  WhisperHealth({
    required this.ok,
    required this.service,
    required this.model,
    this.error,
  });

  final bool ok;
  final String service;
  final String model;
  final String? error;
}

/// Loopback client for the local Whisper sidecar (`127.0.0.1:4790`).
class WhisperClient {
  WhisperClient({
    this.host = '127.0.0.1',
    this.port = 4790,
    http.Client? client,
  }) : _client = client ?? http.Client();

  final String host;
  final int port;
  final http.Client _client;

  Uri get _base => Uri(scheme: 'http', host: host, port: port);

  Future<WhisperHealth> health({
    Duration timeout = const Duration(seconds: 2),
  }) async {
    final res = await _client
        .get(_base.replace(path: '/v1/health'))
        .timeout(timeout);
    if (res.statusCode != 200) {
      throw WhisperException(
        'Whisper health failed (${res.statusCode})',
        statusCode: res.statusCode,
      );
    }
    final json = jsonDecode(res.body) as Map<String, dynamic>;
    return WhisperHealth(
      ok: json['ok'] == true,
      service: (json['service'] as String?) ?? 'smith-whisper',
      model: (json['model'] as String?) ?? '',
      error: json['error'] as String?,
    );
  }

  /// Transcribe a local WAV path (same machine as the sidecar).
  Future<String> transcribe(
    String absolutePath, {
    Duration timeout = const Duration(seconds: 120),
  }) async {
    final res = await _client
        .post(
          _base.replace(path: '/v1/transcribe'),
          headers: {'Content-Type': 'application/json'},
          body: jsonEncode({'v': 1, 'path': absolutePath}),
        )
        .timeout(timeout);

    if (res.statusCode != 200) {
      String detail = res.body;
      try {
        final err = jsonDecode(res.body) as Map<String, dynamic>;
        detail = (err['detail'] ?? err['error'] ?? res.body).toString();
      } catch (_) {}
      throw WhisperException(
        'Whisper transcribe failed (${res.statusCode}): $detail',
        statusCode: res.statusCode,
      );
    }

    final json = jsonDecode(res.body) as Map<String, dynamic>;
    return ((json['text'] as String?) ?? '').trim();
  }

  void close() => _client.close();
}
