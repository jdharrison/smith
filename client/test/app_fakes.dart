import 'dart:async';

import 'package:smith_client/app_api.dart';

const testInstance = '0123456789abcdef0123456789abcdef';
const testConversation = '11111111111111111111111111111111';

class FakeAppApi extends LegacyFakeAppApi {
  FakeAppApi() {
    summaries[testConversation] = summary(testConversation, 'Current chat', 0);
    histories[testConversation] = history;
  }
  final summaries = <String, Map<String, dynamic>>{};
  final histories = <String, List<Map<String, dynamic>>>{};
  bool inventoryObserved = false;
  bool holdChats = false;
  bool detailFailure = false;
  int disposedCount = 0;
  Map<String, dynamic> summary(
    String id,
    String title,
    int sequence, {
    String? origin,
  }) => {
    'conversation_id': id,
    'title': title,
    'archived': false,
    'metadata_revision': 0,
    'history_revision': 0,
    'sequence': sequence,
    'origin_request_id': origin,
  };
  Map<String, dynamic> get inventory => {
    'models': [
      if (providers['llm']['ready'] == true)
        {'name': 'local:exact', 'digest': 'abc'},
    ],
    'voices': [],
    'providers': {
      for (final key in ['llm', 'stt', 'tts'])
        key: {
          'ready': providers[key]['ready'],
          'error': providers[key]['error'] is String
              ? {'code': 'UNAVAILABLE', 'message': providers[key]['error']}
              : providers[key]['error'],
          if (key == 'stt') 'model': null,
        },
    },
  };
  Map<String, dynamic> brief(Map<String, dynamic> job) => Map.of(job)
    ..remove('result')
    ..remove('error');
  Map<String, dynamic> get state => {
    'config': config,
    'inventory_observed': inventoryObserved,
    'inventory': inventoryObserved
        ? inventory
        : {
            'models': [],
            'voices': [],
            'providers': {
              for (final key in ['llm', 'stt', 'tts'])
                key: {
                  'ready': false,
                  'error': {
                    'code': 'NOT_OBSERVED',
                    'message': 'Not observed in this instance',
                  },
                  if (key == 'stt') 'model': null,
                },
            },
          },
    'conversations': summaries.values.toList(),
    'operations': jobs.values.map(brief).toList(),
    'retention': {
      'max_conversations': 32,
      'max_turn_pairs': 16,
      'max_context_bytes': 65536,
      'max_terminal_jobs': 32,
      'max_expired_ids': 1024,
      'max_pending_jobs': 4,
      'queue_timeout_ms': 600000,
      'capacity_policy': 'reject_when_expiry_metadata_full',
      'missing_request_code': 'REQUEST_EXPIRED',
    },
    'revision': 0,
  };
  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    calls.add(op);
    arguments.add(Map.of(args));
    if (op == 'state') {
      if (stateFailure) throw const AppFailure('TRANSPORT', 'Unavailable');
      if (active != null) {
        jobs[active!['request_id']] = {
          'config_revision': 1,
          'result': null,
          'error': null,
          ...active!,
          ...?jobs[active!['request_id']],
          'conversation_id':
              active!['conversation_id'] ??
              (active!['kind'] == 'models' ? null : testConversation),
        };
      }
      return AppReply(instance, state);
    }
    if (op == 'conversation_get') {
      if (detailFailure || stateFailure) {
        throw const AppFailure('TRANSPORT', 'Detail unavailable');
      }
      final s = summaries[args['conversation_id']];
      if (s == null) throw const AppFailure('NOT_FOUND', 'Missing chat');
      return AppReply(instance, {
        'conversation': Map.of(s),
        'history': histories[args['conversation_id']],
      });
    }
    if (op == 'poll') {
      if (expiredPoll) throw const AppFailure('REQUEST_EXPIRED', 'Expired');
      if (instanceMismatchPoll || args['instance_id'] != instance) {
        throw const AppFailure('INSTANCE_MISMATCH', 'restart');
      }
      if (pollGate != null) return pollGate!.future;
      final job = jobs[args['request_id']];
      if (job == null) throw const AppFailure('NOT_FOUND', 'Unknown operation');
      if (stateFailureAfterPoll) stateFailure = true;
      return AppReply(instance, Map.of(job));
    }
    final request = args['request_id'] as String;
    if (op.startsWith('conversation_')) {
      final s = summaries[args['conversation_id']];
      if (s == null ||
          s['metadata_revision'] != args['expected_metadata_revision'] ||
          (op == 'conversation_delete' &&
              s['history_revision'] != args['expected_history_revision'])) {
        throw const AppFailure('STALE', 'Revision mismatch');
      }
      final id = s['conversation_id'];
      Map<String, dynamic> result;
      if (op == 'conversation_delete') {
        summaries.remove(id);
        histories.remove(id);
        jobs.removeWhere((_, o) => o['conversation_id'] == id);
        result = {
          'request_id': request,
          'conversation_id': id,
          'deleted': true,
        };
      } else {
        if (op == 'conversation_rename') s['title'] = args['title'];
        if (op == 'conversation_archive') s['archived'] = true;
        if (op == 'conversation_restore') s['archived'] = false;
        s['metadata_revision']++;
        result = {'request_id': request, 'conversation': Map.of(s)};
      }
      jobs[request] = {
        'request_id': request,
        'conversation_id': id,
        'kind': op,
        'status': 'succeeded',
        'phase': 'complete',
        'config_revision': config['revision'],
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
      return AppReply(instance, result);
    }
    String? id;
    Map<String, dynamic> result;
    if (op == 'models') {
      inventoryObserved = true;
      result = inventory;
    } else if (op == 'submit' || op == 'submit_new') {
      if (op == 'submit_new') {
        id = appId();
        summaries[id] = summary(
          id,
          'New chat',
          summaries.length,
          origin: request,
        );
        histories[id] = [];
      } else {
        id = args['conversation_id'];
      }
      result = {
        'assistant_text': 'useful answer',
        'audio': null,
        'speech_error': null,
      };
      if (!holdChats) {
        histories[id]!.addAll([
          {'role': 'user', 'content': args['text']},
          {'role': 'assistant', 'content': 'useful answer'},
        ]);
        summaries[id]!['history_revision']++;
      }
    } else {
      throw const AppFailure('OP', 'Unsupported fake operation');
    }
    final held = holdChats && op != 'models';
    jobs[request] = {
      'request_id': request,
      'conversation_id': id,
      'kind': op,
      'status': held ? 'running' : 'succeeded',
      'phase': held ? 'generation' : 'complete',
      'config_revision': config['revision'],
      'result': held ? null : result,
      'error': null,
    };
    if (unknownAdmission) {
      throw const AppFailure(
        'TRANSPORT',
        'Lost acknowledgement',
        unknown: true,
      );
    }
    return AppReply(instance, brief(jobs[request]!));
  }

  @override
  void dispose() {
    disposedCount++;
    super.dispose();
  }
}

class LegacyFakeAppApi implements AppApi {
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
