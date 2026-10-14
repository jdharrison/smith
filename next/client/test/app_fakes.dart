import 'dart:async';

import 'package:smith_client/app_api.dart';

const testInstance = '0123456789abcdef0123456789abcdef';

class FakeAppApi implements AppApi {
  String instance = testInstance;
  final calls = <String>[];
  final arguments = <Map<String, dynamic>>[];
  final jobs = <String, Map<String, dynamic>>{};
  final history = <Map<String, dynamic>>[];
  Map<String, dynamic> config = {
    'model': 'local:exact',
    'voice': null,
    'revision': 1,
  };
  Map<String, dynamic> providers = {
    'llm': {'ready': true, 'error': null},
    'stt': {'ready': false, 'error': 'missing'},
    'tts': {'ready': false, 'error': 'missing'},
  };
  Map<String, dynamic>? active;
  bool stateFailure = false;
  bool stateFailureAfterPoll = false;
  bool unknownAdmission = false;
  bool disposed = false;
  bool expiredPoll = false;
  bool instanceMismatchPoll = false;
  Completer<AppReply>? pollGate;
  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    calls.add(op);
    arguments.add(Map.from(args));
    if (op == 'state') {
      if (stateFailure) throw const AppFailure('TRANSPORT', 'Unavailable');
      return AppReply(instance, {
        'config': config,
        'history': history,
        'providers': providers,
        'active': active,
        'retention': {},
      });
    }
    if (op == 'configure') {
      config = {...args, 'revision': 2};
      return AppReply(instance, config);
    }
    if (op == 'poll') {
      if (instanceMismatchPoll || args['instance_id'] != instance) {
        throw const AppFailure('INSTANCE_MISMATCH', 'restart');
      }
      if (expiredPoll) {
        throw const AppFailure(
          'REQUEST_EXPIRED',
          'Request result no longer retained.',
        );
      }
      final AppReply reply;
      if (pollGate != null) {
        reply = await pollGate!.future;
      } else {
        final job = jobs[args['request_id']];
        if (job == null) {
          throw const AppFailure('NOT_FOUND', 'Unknown operation');
        }
        reply = AppReply(instance, job);
      }
      if (stateFailureAfterPoll) stateFailure = true;
      return reply;
    }
    final id = args['request_id'] as String;
    Map<String, dynamic> result = {};
    if (op == 'models') {
      result = {
        'models': [
          {'name': 'local:exact', 'digest': 'abc'},
        ],
        'voices': [],
        'providers': providers,
      };
    }
    if (op == 'submit') {
      history.addAll([
        {'role': 'user', 'content': args['text']},
        {'role': 'assistant', 'content': 'useful answer'},
      ]);
      result = {
        'assistant_text': 'useful answer',
        'audio': null,
        'speech_error': null,
      };
    }
    jobs[id] = {
      'request_id': id,
      'kind': op,
      'status': 'succeeded',
      'phase': 'complete',
      'config_revision': 1,
      'result': result,
      'error': null,
    };
    if (unknownAdmission) {
      throw const AppFailure(
        'TRANSPORT',
        'Lost acknowledgement',
        unknown: true,
      );
    }
    return AppReply(instance, {
      'request_id': id,
      'status': 'running',
      'phase': op,
      'config_revision': 1,
    });
  }

  @override
  void dispose() {
    disposed = true;
  }
}
