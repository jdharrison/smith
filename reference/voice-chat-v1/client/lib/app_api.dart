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
    return AppReply(v['instance_id'], v['result']);
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
    if (![
      'state',
      'models',
      'configure',
      'submit',
      'poll',
      'transcribe',
      'speak',
    ].contains(op)) {
      throw const AppFailure('OP', 'Unsupported app operation.');
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
