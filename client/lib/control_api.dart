import 'dart:convert';

const serviceActions = {
  'install-binary': 'Install binary',
  'install-unit': 'Register service',
  'enable': 'Enable at login',
  'start': 'Start service',
  'complete-setup': 'Finish setup',
};

enum EnablementState { missing, disabled, enabled, conflict, unknown }

class ControlFailure implements Exception {
  const ControlFailure(this.message, {this.outcomeUnknown = false});
  final String message;
  final bool outcomeUnknown;
  @override
  String toString() => message;
}

class ControlReply {
  ControlReply._(this.data);
  final Map<String, dynamic> data;
  bool get ok => data['ok'] as bool;
  String get binary => data['binary_state'] as String;
  String get unit => data['unit_state'] as String;
  String get runtime => data['runtime_state'] as String;
  String get bridge => data['bridge_state'] as String;
  EnablementState get enablement =>
      EnablementState.values.byName(data['enablement_state'] as String);
  bool? get setupComplete => data['setup_complete'] as bool?;
  String get message => data['message'] as String;
  String? get error => data['error'] as String?;
  String? get approvalToken => data['approval_token'] as String?;
  Map<String, dynamic> get effects => data['effects'] as Map<String, dynamic>;
  bool get chatReady =>
      ok &&
      binary == 'installed' &&
      unit == 'installed' &&
      enablement == EnablementState.enabled &&
      runtime == 'running' &&
      bridge == 'ready';

  bool get ready =>
      ok &&
      binary == 'installed' &&
      unit == 'installed' &&
      runtime == 'running' &&
      bridge == 'ready' &&
      setupComplete == true;

  bool get hasPersistedSetup =>
      binary == 'installed' &&
      unit == 'installed' &&
      setupComplete == true &&
      runtime != 'conflict' &&
      bridge != 'conflict' &&
      [
        'binary_path',
        'unit_path',
        'state_path',
        'socket_path',
      ].every((key) => (effects[key] as String).isNotEmpty);

  static ControlReply parse(String text, {bool prepare = false}) {
    try {
      final value = jsonDecode(text);
      const allowedKeys = {
        'schema_version',
        'service',
        'ok',
        'binary_state',
        'unit_state',
        'enablement_state',
        'runtime_state',
        'bridge_state',
        'setup_complete',
        'message',
        'error',
        'effects',
        'approval_token',
      };
      if (value is! Map<String, dynamic> ||
          value.keys.any((key) => !allowedKeys.contains(key)) ||
          value['schema_version'] is! int ||
          value['schema_version'] != 2 ||
          value['service'] != 'smith-next' ||
          value['ok'] is! bool ||
          value['message'] is! String ||
          !value.containsKey('error') ||
          (value['error'] != null && value['error'] is! String) ||
          !value.containsKey('setup_complete') ||
          (value['setup_complete'] != null &&
              value['setup_complete'] is! bool)) {
        throw const FormatException();
      }
      for (final key in ['binary_state', 'unit_state']) {
        if (![
          'missing',
          'installed',
          'conflict',
          'unknown',
        ].contains(value[key])) {
          throw const FormatException();
        }
      }
      if (!EnablementState.values
              .map((state) => state.name)
              .contains(value['enablement_state']) ||
          ![
            'stopped',
            'running',
            'unavailable',
            'conflict',
            'unknown',
          ].contains(value['runtime_state']) ||
          ![
            'ready',
            'unavailable',
            'conflict',
            'unknown',
          ].contains(value['bridge_state'])) {
        throw const FormatException();
      }
      final effects = value['effects'];
      const effectKeys = {
        'binary_path',
        'unit_path',
        'state_path',
        'socket_path',
        'description',
      };
      if (effects is! Map<String, dynamic> ||
          effects.keys.any((key) => !effectKeys.contains(key)) ||
          effects.length != effectKeys.length) {
        throw const FormatException();
      }
      for (final key in [
        'binary_path',
        'unit_path',
        'state_path',
        'socket_path',
      ]) {
        final path = effects[key];
        if (path is! String ||
            (!path.startsWith('/') &&
                !(path.isEmpty && value['ok'] == false)) ||
            path.contains('\u0000')) {
          throw const FormatException();
        }
      }
      if (effects['description'] is! String ||
          (prepare &&
              value['ok'] == true &&
              ((effects['description'] as String).isEmpty ||
                  value['approval_token'] is! String ||
                  (value['approval_token'] as String).isEmpty)) ||
          (value.containsKey('approval_token') &&
              value['approval_token'] != null &&
              value['approval_token'] is! String)) {
        throw const FormatException();
      }
      return ControlReply._(value);
    } on Object {
      throw const ControlFailure(
        'Invalid helper reply: schema or smith-next identity mismatch.',
      );
    }
  }
}

abstract class ControlApi {
  Future<ControlReply> inspect({Duration? deadline});
  Future<ControlReply> prepare(String action);
  Future<ControlReply> apply(String action, String approvalToken);

  /// Cleans only a captured short-lived control helper, never the daemon.
  void dispose();
}
