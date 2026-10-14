import 'dart:async';
import 'dart:convert';

import 'package:smith_client/app_api.dart';
import 'package:smith_client/audio.dart';

const testInstance = '0123456789abcdef0123456789abcdef';
Map<String, dynamic> testAudio(int rate) => {
  'mime': 'audio/wav',
  if (rate == 24000) 'sample_rate': rate,
  'data_base64': base64Encode(wavPcm([0, 0, 1, 0], rate)),
};

class FakeAudio implements AudioIo {
  int recordings = 0, stops = 0, plays = 0;
  bool disposed = false;
  bool unavailable = false;
  Completer<void>? playback;
  @override
  Future<void> startRecording() async {
    if (unavailable) throw StateError('missing');
    recordings++;
  }

  @override
  Future<Map<String, dynamic>> stopRecording() async {
    stops++;
    return testAudio(16000);
  }

  @override
  Future<void> play(Map<String, dynamic> audio) async {
    plays++;
    await playback?.future;
  }

  @override
  Future<void> stop() async {
    stops++;
    if (playback != null && !playback!.isCompleted) playback!.complete();
  }

  @override
  void dispose() {
    disposed = true;
  }
}

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
  bool speechFailure = false;
  bool unknownAdmission = false;
  bool disposed = false;
  bool expiredPoll = false;
  Completer<AppReply>? pollGate;
  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    calls.add(op);
    arguments.add(Map.from(args));
    if (op == 'state') {
      return AppReply(instance, {
        'config': config,
        'history': history,
        'providers': providers,
        'active': null,
        'retention': {},
      });
    }
    if (op == 'configure') {
      config = {...args, 'revision': 2};
      return AppReply(instance, config);
    }
    if (op == 'poll') {
      if (args['instance_id'] != instance) {
        throw const AppFailure('INSTANCE_MISMATCH', 'restart');
      }
      if (expiredPoll) {
        throw const AppFailure(
          'REQUEST_EXPIRED',
          'Request result no longer retained.',
        );
      }
      if (pollGate != null) return pollGate!.future;
      final job = jobs[args['request_id']];
      if (job == null) throw const AppFailure('NOT_FOUND', 'Unknown operation');
      return AppReply(instance, job);
    }
    final id = args['request_id'] as String;
    Map<String, dynamic> result = {};
    if (op == 'models') {
      result = {
        'models': [
          {'name': 'local:exact', 'digest': 'abc'},
        ],
        'voices': ['observed_voice'],
        'providers': providers,
      };
    }
    if (op == 'transcribe') {
      final audio = args['audio'] as Map;
      if (audio.length != 2 ||
          audio['mime'] != 'audio/wav' ||
          audio['data_base64'] is! String) {
        throw const AppFailure(
          'INVALID_REQUEST',
          'Input audio requires only mime and data_base64.',
        );
      }
      result = {
        'text': 'transcript to review',
        'language': 'en',
        'duration_ms': 1,
      };
    }
    if (op == 'speak') result = {'audio': testAudio(24000)};
    if (op == 'submit') {
      history.addAll([
        {'role': 'user', 'content': args['text']},
        {'role': 'assistant', 'content': 'useful answer'},
      ]);
      result = {
        'assistant_text': 'useful answer',
        'audio': args['output'] == 'speech' && !speechFailure
            ? testAudio(24000)
            : null,
        'speech_error': speechFailure
            ? {'code': 'TTS_UNAVAILABLE', 'message': 'missing'}
            : null,
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
