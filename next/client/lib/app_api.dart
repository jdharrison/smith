import 'dart:convert';
import 'dart:math';

import 'control_api.dart';
import 'native_control.dart';

const appFrameLimit = 4 * 1024 * 1024;
final _idPattern = RegExp(r'^[0-9a-f]{32}$');
String appId() => List.generate(
  16,
  (_) => Random.secure().nextInt(256).toRadixString(16).padLeft(2, '0'),
).join();

class AppFailure implements Exception {
  const AppFailure(this.code, this.message, {this.unknown = false});
  final String code;
  final String message;
  final bool unknown;
  @override
  String toString() => '$code: $message';
}

class AppReply {
  AppReply(this.instanceId, this.result);
  final String instanceId;
  final Map<String, dynamic> result;
  static AppReply parse(String text, String callId) {
    final dynamic v;
    try {
      v = jsonDecode(text);
    } on Object {
      throw const AppFailure('PROTOCOL', 'Invalid app reply.');
    }
    if (v is! Map<String, dynamic> ||
        v['schema_version'] != 1 ||
        v['service'] != 'smith-next' ||
        v['protocol'] != 'app' ||
        v['call_id'] != callId ||
        v['pid'] is! int ||
        v['pid'] <= 0 ||
        v['instance_id'] is! String ||
        !_idPattern.hasMatch(v['instance_id']) ||
        v['ok'] is! bool) {
      throw const AppFailure('PROTOCOL', 'App identity/correlation mismatch.');
    }
    if (!v['ok']) {
      final e = v['error'];
      if (v['result'] != null ||
          e is! Map ||
          e['code'] is! String ||
          e['message'] is! String) {
        throw const AppFailure('PROTOCOL', 'Invalid app error.');
      }
      throw AppFailure(e['code'], e['message']);
    }
    if (v['error'] != null || v['result'] is! Map<String, dynamic>) {
      throw const AppFailure('PROTOCOL', 'Invalid app result.');
    }
    final result = v['result'] as Map<String, dynamic>;
    _validateModelOperation(result);
    if (result['active'] is Map<String, dynamic>) {
      _validateModelOperation(result['active'] as Map<String, dynamic>);
    }
    return AppReply(v['instance_id'], result);
  }
}

// Observational compatibility only: these operations cannot be submitted by
// NativeAppApi, but installer-owned work may be adopted through state/poll.
void _validateModelOperation(Map<String, dynamic> operation) {
  final kind = operation['kind'];
  if (kind != 'verify_model' && kind != 'remove_model') return;
  bool exact(Map value, List<String> keys) =>
      value.length == keys.length && keys.every(value.containsKey);
  bool identity(dynamic model, dynamic sha) =>
      model is String &&
      model.isNotEmpty &&
      model.length <= 256 &&
      RegExp(r'^[a-zA-Z0-9/:_.-]+$').hasMatch(model) &&
      sha is String &&
      RegExp(r'^[0-9a-f]{64}$').hasMatch(sha);
  bool validError(dynamic error) =>
      error is Map &&
      exact(error, ['code', 'message']) &&
      error['code'] is String &&
      RegExp(r'^[A-Z][A-Z0-9_]{0,255}$').hasMatch(error['code']) &&
      error['message'] is String &&
      error['message'].trim().isNotEmpty &&
      utf8.encode(error['message']).length <= 1024;
  bool valid =
      exact(operation, [
        'request_id',
        'kind',
        'status',
        'phase',
        'config_revision',
        'result',
        'error',
      ]) &&
      operation['request_id'] is String &&
      _idPattern.hasMatch(operation['request_id']) &&
      operation['config_revision'] is int &&
      operation['config_revision'] >= 0;
  final status = operation['status'];
  final result = operation['result'];
  if (status == 'running') {
    valid =
        valid &&
        operation['phase'] ==
            (kind == 'verify_model' ? 'verification' : 'removal') &&
        result == null &&
        operation['error'] == null;
  } else if (status == 'succeeded') {
    valid =
        valid &&
        operation['phase'] == 'complete' &&
        operation['error'] == null &&
        result is Map &&
        identity(result['model'], result['sha256']);
    if (valid) {
      if (kind == 'verify_model') {
        valid =
            exact(result, ['model', 'sha256', 'verified', 'assistant_text']) &&
            result['verified'] == true &&
            result['assistant_text'] is String &&
            result['assistant_text'].trim().isNotEmpty &&
            utf8.encode(result['assistant_text']).length <= 65536;
      } else {
        valid =
            exact(result, ['model', 'sha256', 'removed']) &&
            result['removed'] == true;
      }
    }
  } else {
    valid =
        valid &&
        ['failed', 'interrupted'].contains(status) &&
        operation['phase'] == 'complete' &&
        result == null &&
        validError(operation['error']);
  }
  if (!valid) {
    throw const AppFailure('PROTOCOL', 'Invalid model operation.');
  }
}

abstract class AppApi {
  Future<AppReply> call(String op, Map<String, dynamic> args);
  void dispose();
}

abstract class AppExecutor {
  Future<String> exchange(List<int> frame);
  void dispose();
}

class NativeAppExecutor implements AppExecutor {
  NativeAppExecutor({NativeHelperExecutor? helper})
    : _helper = helper ?? NativeHelperExecutor();
  final NativeHelperExecutor _helper;
  @override
  Future<String> exchange(List<int> frame) {
    if (frame.length > appFrameLimit + 1) {
      throw const AppFailure('LIMIT', 'App request exceeds 4 MiB.');
    }
    return _helper.exchange(
      ['app'],
      input: frame,
      outputLimit: appFrameLimit + 1,
      app: true,
    );
  }

  @override
  void dispose() => _helper.dispose();
}

class NativeAppApi implements AppApi {
  NativeAppApi({AppExecutor? executor})
    : _executor = executor ?? NativeAppExecutor();
  final AppExecutor _executor;
  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    if (!['state', 'models', 'configure', 'submit', 'poll'].contains(op)) {
      throw const AppFailure('OP', 'Unsupported app operation.');
    }
    if ((op == 'configure' && args['voice'] != null) ||
        (op == 'submit' && args['output'] != 'chat')) {
      throw const AppFailure(
        'OP',
        'Only text chat and voice:null configuration are supported.',
      );
    }
    final id = appId();
    final payload = utf8.encode(
      jsonEncode({
        'schema_version': 1,
        'service': 'smith-next',
        'protocol': 'app',
        'call_id': id,
        'op': op,
        'args': args,
      }),
    );
    if (payload.length > appFrameLimit) {
      throw const AppFailure('LIMIT', 'App request exceeds 4 MiB.');
    }
    try {
      return AppReply.parse(await _executor.exchange([...payload, 10]), id);
    } on ControlFailure catch (e) {
      throw AppFailure('TRANSPORT', e.message, unknown: e.outcomeUnknown);
    }
  }

  @override
  void dispose() => _executor.dispose();
}
