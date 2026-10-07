import 'dart:convert';

import 'package:http/http.dart' as http;

class SmithBridgeException implements Exception {
  SmithBridgeException(this.message, {this.statusCode});

  final String message;
  final int? statusCode;

  @override
  String toString() => message;
}

class ChatAttachment {
  ChatAttachment({required this.kind, required this.path, required this.name});

  final String kind;
  final String path;
  final String name;

  bool get isImage => kind == 'image';

  factory ChatAttachment.fromJson(Map<String, dynamic> json) {
    return ChatAttachment(
      kind: (json['kind'] as String?) ?? 'file',
      path: (json['path'] as String?) ?? '',
      name: (json['name'] as String?) ?? '',
    );
  }
}

class HealthInfo {
  HealthInfo({
    required this.ok,
    required this.service,
    required this.echo,
    required this.isolation,
    this.profile,
    this.provider,
    this.model,
    this.providerAvailable,
    this.modelAvailable,
    this.desktopSession,
  });

  factory HealthInfo.fromJson(Map<String, dynamic> body) {
    final providerAvailable = body['provider_available'] as bool?;
    final model = body['model'] as String?;
    // Chat-only provider_available is the backend's exact installed-local-tag
    // probe (not merely Ollama listener liveness). Explicit additive model
    // availability, including null, takes precedence and fails closed.
    final exactTagProbe =
        body['service'] == 'smith-bridge' &&
        body['profile'] == 'chat-only' &&
        body['echo'] == false &&
        body['provider'] == 'ollama' &&
        model != null &&
        model.isNotEmpty;
    return HealthInfo(
      ok: body['ok'] == true,
      service: body['service'] as String? ?? 'unknown',
      echo: body['echo'] != false,
      isolation: body['isolation'] as String? ?? 'unknown',
      profile: body['profile'] as String?,
      provider: body['provider'] as String?,
      model: model,
      providerAvailable: providerAvailable,
      modelAvailable: body.containsKey('model_available')
          ? body['model_available'] as bool?
          : exactTagProbe
          ? providerAvailable
          : null,
      desktopSession: body['desktop_session'] as String?,
    );
  }

  final String? desktopSession;
  final String? profile;
  final String? provider;
  final String? model;
  final bool? providerAvailable;
  final bool? modelAvailable;
  final bool ok;
  final String service;
  final bool echo;
  final String isolation;
}

class PipelineStatus {
  PipelineStatus({
    required this.jobId,
    required this.kind,
    required this.state,
    this.summary = '',
    this.plan,
    this.project,
    this.branch,
  });

  final String jobId;
  final String kind;
  final String state;
  final String summary;
  final String? plan;
  final String? project;
  final String? branch;

  factory PipelineStatus.fromJson(Map<String, dynamic> json) {
    return PipelineStatus(
      jobId: (json['job_id'] as String?) ?? '',
      kind: (json['kind'] as String?) ?? '',
      state: (json['state'] as String?) ?? '',
      summary: (json['summary'] as String?) ?? '',
      plan: json['plan'] as String?,
      project: json['project'] as String?,
      branch: json['branch'] as String?,
    );
  }

  String get shortLabel {
    final parts = <String>[kind, state];
    if (project != null && branch != null) {
      parts.add('$project/$branch');
    }
    return parts.where((s) => s.isNotEmpty).join(' · ');
  }
}

class BoardTask {
  BoardTask({
    required this.id,
    required this.title,
    required this.kind,
    required this.stage,
    required this.status,
    this.userText = '',
    this.ackText = '',
    this.resultText = '',
    this.expectedResult = '',
    this.jobId,
    this.session = 'default',
    this.createdAt = 0,
    this.updatedAt = 0,
    this.error,
  });

  final String id;
  final String title;
  final String kind;
  final String stage;
  final String status;
  final String userText;
  final String ackText;
  final String resultText;
  final String expectedResult;
  final String? jobId;
  final String session;
  final int createdAt;
  final int updatedAt;
  final String? error;

  bool get isRunning => status == 'queued' || status == 'running';

  bool get isTerminalInject =>
      status == 'completed' ||
      status == 'failed' ||
      status == 'interrupted' ||
      status == 'awaiting_signoff';

  factory BoardTask.fromJson(Map<String, dynamic> json) {
    return BoardTask(
      id: (json['id'] as String?) ?? '',
      title: (json['title'] as String?) ?? '',
      kind: (json['kind'] as String?) ?? '',
      stage: (json['stage'] as String?) ?? '',
      status: (json['status'] as String?) ?? '',
      userText: (json['user_text'] as String?) ?? '',
      ackText: (json['ack_text'] as String?) ?? '',
      resultText: (json['result_text'] as String?) ?? '',
      expectedResult: (json['expected_result'] as String?) ?? '',
      jobId: json['job_id'] as String?,
      session: (json['session'] as String?) ?? 'default',
      createdAt: (json['created_at'] as num?)?.toInt() ?? 0,
      updatedAt: (json['updated_at'] as num?)?.toInt() ?? 0,
      error: json['error'] as String?,
    );
  }

  String get snippet {
    final r = resultText.trim();
    if (r.isNotEmpty) return r;
    final a = ackText.trim();
    if (a.isNotEmpty) return a;
    return userText.trim();
  }
}

class ChatReply {
  ChatReply({
    required this.text,
    required this.session,
    this.isolation = 'sealed',
    this.mode = 'chat',
    this.attachments = const [],
    this.pipeline,
    this.task,
  });

  final String text;
  final String session;
  final String isolation;
  final String mode;
  final List<ChatAttachment> attachments;
  final PipelineStatus? pipeline;
  final BoardTask? task;
}

/// Loopback client for `smith serve` (http://127.0.0.1:4783).
class SmithBridge {
  SmithBridge({this.host = '127.0.0.1', this.port = 4783, http.Client? client})
    : _client = client ?? http.Client();

  final String host;
  final int port;
  final http.Client _client;

  Uri get _base => Uri(scheme: 'http', host: host, port: port);

  Future<HealthInfo> health({
    Duration timeout = const Duration(seconds: 2),
  }) async {
    final request = http.Request('GET', _base.replace(path: '/v1/health'))
      ..followRedirects = false;
    final res = await (() async {
      final streamed = await _client.send(request);
      return http.Response.fromStream(streamed);
    })().timeout(timeout);
    if (res.statusCode != 200) {
      throw SmithBridgeException(
        'health failed (${res.statusCode})',
        statusCode: res.statusCode,
      );
    }
    final body = jsonDecode(res.body) as Map<String, dynamic>;
    return HealthInfo.fromJson(body);
  }

  Future<ChatReply> chat(
    String text, {
    bool newSession = false,
    String session = 'default',
    Duration timeout = const Duration(minutes: 5),
  }) => _chat(
    text,
    newSession: newSession,
    session: session,
    timeout: timeout,
    chatOnly: false,
  );

  /// Exclusive text-only route. Never fall back to legacy orchestration.
  Future<ChatReply> chatOnly(
    String text, {
    bool newSession = false,
    Duration timeout = const Duration(minutes: 5),
  }) => _chat(
    text,
    newSession: newSession,
    session: 'default',
    timeout: timeout,
    chatOnly: true,
  );

  Future<ChatReply> _chat(
    String text, {
    required bool newSession,
    required String session,
    required Duration timeout,
    required bool chatOnly,
  }) async {
    final request =
        http.Request(
            'POST',
            _base.replace(path: chatOnly ? '/v1/chat-only' : '/v1/chat'),
          )
          ..followRedirects = false
          ..headers['Content-Type'] = 'application/json'
          ..body = jsonEncode({
            'v': 1,
            'text': text,
            'session': session,
            'new': newSession,
          });
    final res = await (() async {
      final streamed = await _client.send(request);
      return http.Response.fromStream(streamed);
    })().timeout(timeout);

    Object? decoded;
    if (!chatOnly) {
      decoded = jsonDecode(res.body);
    } else {
      try {
        decoded = jsonDecode(res.body);
      } on FormatException {
        if (res.statusCode == 200) {
          throw SmithBridgeException('Invalid chat response: expected JSON');
        }
      }
    }
    if (res.statusCode != 200) {
      final err = !chatOnly
          ? decoded is Map<String, dynamic>
                ? (decoded['error'] as String?) ?? res.body
                : res.body
          : decoded is Map<String, dynamic> && decoded['error'] is String
          ? decoded['error'] as String
          : 'Chat request failed (${res.statusCode})';
      throw SmithBridgeException(err, statusCode: res.statusCode);
    }
    if (chatOnly &&
        (decoded is! Map<String, dynamic> ||
            decoded['v'] != 1 ||
            decoded['profile'] != 'chat-only' ||
            decoded['mode'] != 'chat-only' ||
            decoded['session'] != 'default' ||
            decoded['text'] is! String ||
            decoded['attachments'] is! List ||
            (decoded['attachments'] as List).isNotEmpty ||
            decoded['pipeline'] != null ||
            decoded['task'] != null)) {
      throw SmithBridgeException(
        'Invalid chat-only response: expected the text-only profile schema. No fallback is attempted.',
      );
    }
    final body = decoded as Map<String, dynamic>;
    final rawAtt = body['attachments'];
    final attachments = <ChatAttachment>[];
    if (rawAtt is List) {
      for (final item in rawAtt) {
        if (item is Map<String, dynamic>) {
          attachments.add(ChatAttachment.fromJson(item));
        }
      }
    }
    PipelineStatus? pipeline;
    final rawPipe = body['pipeline'];
    if (rawPipe is Map<String, dynamic>) {
      pipeline = PipelineStatus.fromJson(rawPipe);
    }
    BoardTask? task;
    final rawTask = body['task'];
    if (rawTask is Map<String, dynamic>) {
      task = BoardTask.fromJson(rawTask);
    }
    return ChatReply(
      text: (body['text'] as String?) ?? '',
      session: (body['session'] as String?) ?? session,
      isolation: (body['isolation'] as String?) ?? 'sealed',
      mode: (body['mode'] as String?) ?? 'chat',
      attachments: attachments,
      pipeline: pipeline,
      task: task,
    );
  }

  Future<List<BoardTask>> listTasks({
    bool activeOnly = false,
    int? since,
    Duration timeout = const Duration(seconds: 5),
  }) async {
    final params = <String, String>{};
    if (activeOnly) params['active'] = '1';
    if (since != null) params['since'] = '$since';
    final res = await _client
        .get(_base.replace(path: '/v1/tasks', queryParameters: params))
        .timeout(timeout);
    final decoded = jsonDecode(res.body);
    if (res.statusCode != 200) {
      final err = decoded is Map<String, dynamic>
          ? (decoded['error'] as String?) ?? res.body
          : res.body;
      throw SmithBridgeException(err, statusCode: res.statusCode);
    }
    final body = decoded as Map<String, dynamic>;
    final raw = body['tasks'];
    final out = <BoardTask>[];
    if (raw is List) {
      for (final item in raw) {
        if (item is Map<String, dynamic>) {
          out.add(BoardTask.fromJson(item));
        }
      }
    }
    return out;
  }

  Future<BoardTask> getTask(
    String id, {
    Duration timeout = const Duration(seconds: 5),
  }) async {
    final res = await _client
        .get(_base.replace(path: '/v1/tasks/$id'))
        .timeout(timeout);
    final decoded = jsonDecode(res.body);
    if (res.statusCode != 200) {
      final err = decoded is Map<String, dynamic>
          ? (decoded['error'] as String?) ?? res.body
          : res.body;
      throw SmithBridgeException(err, statusCode: res.statusCode);
    }
    return BoardTask.fromJson(decoded as Map<String, dynamic>);
  }

  Future<void> openPath(String path) async {
    final res = await _client.post(
      _base.replace(path: '/v1/open'),
      headers: const {'Content-Type': 'application/json'},
      body: jsonEncode({'path': path}),
    );
    if (res.statusCode != 200) {
      final decoded = jsonDecode(res.body);
      final err = decoded is Map<String, dynamic>
          ? (decoded['error'] as String?) ?? res.body
          : res.body;
      throw SmithBridgeException(err, statusCode: res.statusCode);
    }
  }

  Future<ServicesSnapshot> listServices({
    Duration timeout = const Duration(seconds: 5),
  }) async {
    final res = await _client
        .get(_base.replace(path: '/v1/services'))
        .timeout(timeout);
    final decoded = jsonDecode(res.body);
    if (res.statusCode != 200) {
      final err = decoded is Map<String, dynamic>
          ? (decoded['error'] as String?) ?? res.body
          : res.body;
      throw SmithBridgeException(err, statusCode: res.statusCode);
    }
    return ServicesSnapshot.fromJson(decoded as Map<String, dynamic>);
  }

  Future<ServicesSnapshot> patchServices({
    bool? autostart,
    String? id,
    String? mode,
    Duration timeout = const Duration(seconds: 10),
  }) async {
    final body = <String, dynamic>{};
    if (autostart != null) body['autostart'] = autostart;
    if (id != null) body['id'] = id;
    if (mode != null) body['mode'] = mode;
    final res = await _client
        .patch(
          _base.replace(path: '/v1/services'),
          headers: const {'Content-Type': 'application/json'},
          body: jsonEncode(body),
        )
        .timeout(timeout);
    final decoded = jsonDecode(res.body);
    if (res.statusCode != 200) {
      final err = decoded is Map<String, dynamic>
          ? (decoded['error'] as String?) ?? res.body
          : res.body;
      throw SmithBridgeException(err, statusCode: res.statusCode);
    }
    return ServicesSnapshot.fromJson(decoded as Map<String, dynamic>);
  }

  Future<void> applyServices({
    Duration timeout = const Duration(seconds: 120),
  }) async {
    final res = await _client
        .post(_base.replace(path: '/v1/services/apply'))
        .timeout(timeout);
    if (res.statusCode != 200) {
      final decoded = jsonDecode(res.body);
      final err = decoded is Map<String, dynamic>
          ? (decoded['error'] as String?) ?? res.body
          : res.body;
      throw SmithBridgeException(err, statusCode: res.statusCode);
    }
  }

  Future<void> ensureSidecars({
    Duration timeout = const Duration(seconds: 120),
  }) async {
    final res = await _client
        .post(_base.replace(path: '/v1/sidecars/ensure'))
        .timeout(timeout);
    if (res.statusCode != 200) {
      final decoded = jsonDecode(res.body);
      final err = decoded is Map<String, dynamic>
          ? (decoded['error'] as String?) ?? res.body
          : res.body;
      throw SmithBridgeException(err, statusCode: res.statusCode);
    }
  }

  void close() => _client.close();
}

class ServiceRow {
  ServiceRow({
    required this.id,
    required this.group,
    required this.mode,
    required this.running,
    required this.healthy,
    this.url,
  });

  final String id;
  final String group;
  final String mode;
  final bool running;
  final bool healthy;
  final String? url;

  factory ServiceRow.fromJson(Map<String, dynamic> json) {
    return ServiceRow(
      id: (json['id'] as String?) ?? '',
      group: (json['group'] as String?) ?? '',
      mode: (json['mode'] as String?) ?? 'on_demand',
      running: json['running'] == true,
      healthy: json['healthy'] == true,
      url: json['url'] as String?,
    );
  }

  bool get isWarm => mode == 'warm';
}

class SupervisorStatus {
  const SupervisorStatus({
    required this.registered,
    required this.loadState,
    required this.enabledState,
    required this.enabledAtLogin,
    required this.activeState,
    required this.subState,
    required this.linger,
    required this.bridgeIdentity,
    required this.bridgeHealthy,
    required this.detail,
  });

  final bool registered;
  final String loadState;
  final String enabledState;
  final bool? enabledAtLogin;
  final String activeState;
  final String subState;
  final bool? linger;
  final String bridgeIdentity;
  final bool bridgeHealthy;
  final String? detail;

  factory SupervisorStatus.fromJson(
    Map<String, dynamic> json, {
    bool registeredFallback = false,
  }) {
    return SupervisorStatus(
      registered: json['registered'] is bool
          ? json['registered'] as bool
          : registeredFallback,
      loadState: _observedString(json['load_state']),
      enabledState: _observedString(json['enabled_state']),
      enabledAtLogin: json['enabled_at_login'] as bool?,
      activeState: _observedString(json['active_state']),
      subState: _observedString(json['sub_state']),
      linger: json['linger'] as bool?,
      bridgeIdentity: _observedString(json['bridge_identity']),
      bridgeHealthy: json['bridge_healthy'] == true,
      detail: json['detail'] as String?,
    );
  }

  factory SupervisorStatus.legacy({required bool registered}) {
    return SupervisorStatus.fromJson(
      const <String, dynamic>{},
      registeredFallback: registered,
    );
  }

  static String _observedString(Object? value) {
    if (value is String && value.trim().isNotEmpty) return value;
    return 'unknown';
  }
}

class ServicesSnapshot {
  ServicesSnapshot({
    required this.autostart,
    required this.osRegistered,
    required this.services,
    SupervisorStatus? supervisor,
  }) : supervisor =
           supervisor ?? SupervisorStatus.legacy(registered: osRegistered);

  final bool autostart;

  /// Legacy registration field retained for existing client callers.
  final bool osRegistered;
  final SupervisorStatus supervisor;
  final List<ServiceRow> services;

  factory ServicesSnapshot.fromJson(Map<String, dynamic> json) {
    final raw = json['services'];
    final list = <ServiceRow>[];
    if (raw is List) {
      for (final item in raw) {
        if (item is Map<String, dynamic>) {
          list.add(ServiceRow.fromJson(item));
        }
      }
    }

    final legacyRegistered = json['os_registered'] == true;
    final rawSupervisor = json['supervisor'];
    final supervisor = rawSupervisor is Map<String, dynamic>
        ? SupervisorStatus.fromJson(
            rawSupervisor,
            registeredFallback: legacyRegistered,
          )
        : SupervisorStatus.legacy(registered: legacyRegistered);
    return ServicesSnapshot(
      autostart: json['autostart'] == true,
      osRegistered: supervisor.registered,
      supervisor: supervisor,
      services: list,
    );
  }
}
