import 'dart:convert';

import 'app_api.dart';
import 'chat_title.dart';

Never invalid() =>
    throw const AppFailure('PROTOCOL', 'Invalid v2 application data.');
bool wireId(dynamic v) => v is String && RegExp(r'^[0-9a-f]{32}$').hasMatch(v);
bool wireRevision(dynamic v) => v is int && v >= 0 && v <= 9007199254740991;

// Only configuration revisions preserve the full migrated u64 domain. Keep the
// original wire value for echo; never convert a large decimal through double.
bool wireConfigRevision(dynamic v) {
  if (wireRevision(v)) return true;
  if (v is! String || !RegExp(r'^[1-9][0-9]{0,19}$').hasMatch(v)) return false;
  final value = BigInt.parse(v);
  return value > BigInt.from(9007199254740991) &&
      value <= BigInt.parse('18446744073709551615');
}

void exact(dynamic v, List<String> keys) {
  if (v is! Map<String, dynamic> ||
      v.length != keys.length ||
      !keys.every(v.containsKey)) {
    invalid();
  }
}

void bounded(dynamic v, int bytes, {bool empty = false}) {
  if (v is! String ||
      (!empty && v.trim().isEmpty) ||
      utf8.encode(v).length > bytes) {
    invalid();
  }
}

// Rust str::trim uses Unicode White_Space, not Dart's additional U+FEFF.
// Keep this title-only: other protocol fields retain their existing checks.
final _rustTitleBoundaryWhitespace = RegExp(
  r'^[\u0009-\u000d\u0020\u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]|[\u0009-\u000d\u0020\u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]$',
);

void conversation(dynamic v) {
  exact(v, [
    'conversation_id',
    'title',
    'archived',
    'metadata_revision',
    'history_revision',
    'sequence',
    'origin_request_id',
  ]);
  if (!wireId(v['conversation_id']) ||
      v['archived'] is! bool ||
      !wireRevision(v['metadata_revision']) ||
      !wireRevision(v['history_revision']) ||
      !wireRevision(v['sequence']) ||
      (v['origin_request_id'] != null && !wireId(v['origin_request_id']))) {
    invalid();
  }
  bounded(v['title'], 256, empty: true);
  if ((v['title'] as String).isEmpty ||
      _rustTitleBoundaryWhitespace.hasMatch(v['title']) ||
      RegExp(
        r'[\x00-\x1f\x7f-\x9f\u061c\u200e\u200f\u2028-\u202e\u2066-\u2069]',
      ).hasMatch(v['title'])) {
    invalid();
  }
}

void wireError(dynamic e) {
  exact(e, ['code', 'message']);
  bounded(e['code'], 256);
  if (!RegExp(r'^[A-Z][A-Z0-9_]{0,255}$').hasMatch(e['code'])) invalid();
  bounded(e['message'], 1024);
}

void inventory(dynamic v) {
  exact(v, ['models', 'voices', 'providers']);
  if (v['models'] is! List ||
      v['voices'] is! List ||
      v['providers'] is! Map<String, dynamic>) {
    invalid();
  }
  if ((v['models'] as List).length > 256 ||
      (v['voices'] as List).length > 256) {
    invalid();
  }
  exact(v['providers'], ['llm', 'stt', 'tts']);
  for (final key in ['llm', 'stt', 'tts']) {
    final p = v['providers'][key];
    exact(p, ['ready', 'error', if (key == 'stt') 'model']);
    if (p['ready'] is! bool || p['ready'] != (p['error'] == null)) invalid();
    if (p['error'] != null) wireError(p['error']);
    if (key == 'stt' && p['model'] != null) bounded(p['model'], 256);
  }
  if ((!v['providers']['llm']['ready'] && (v['models'] as List).isNotEmpty) ||
      (v['providers']['stt']['ready'] &&
          v['providers']['stt']['model'] == null) ||
      v['providers']['tts']['ready'] != (v['voices'] as List).isNotEmpty) {
    invalid();
  }
  for (final voice in v['voices']) {
    bounded(voice, 256);
  }
  final names = <String>{};
  for (final model in v['models']) {
    exact(model, ['name', 'digest']);
    bounded(model['name'], 256);
    bounded(model['digest'], 256, empty: true);
    if (!names.add(model['name'])) invalid();
  }
}

void operation(dynamic v, {bool terminal = false}) {
  exact(v, [
    'request_id',
    'conversation_id',
    'kind',
    'status',
    'phase',
    'config_revision',
    if (terminal) 'result',
    if (terminal) 'error',
  ]);
  if (!wireId(v['request_id']) ||
      (v['conversation_id'] != null && !wireId(v['conversation_id'])) ||
      !wireConfigRevision(v['config_revision']) ||
      ![
        'queued',
        'running',
        'succeeded',
        'failed',
        'interrupted',
      ].contains(v['status']) ||
      ![
        'submit',
        'submit_new',
        'speak',
        'transcribe',
        'models',
        'configure',
        'configure_checked',
        'verify_model',
        'remove_model',
        'conversation_rename',
        'conversation_archive',
        'conversation_restore',
        'conversation_delete',
      ].contains(v['kind'])) {
    invalid();
  }
  bounded(v['phase'], 1024);
  if ((v['kind'] == 'submit' ||
          v['kind'] == 'submit_new' ||
          (v['kind'] as String).startsWith('conversation_')) &&
      v['conversation_id'] == null) {
    invalid();
  }
  if ([
        'models',
        'configure',
        'configure_checked',
        'verify_model',
        'remove_model',
      ].contains(v['kind']) &&
      v['conversation_id'] != null) {
    invalid();
  }
  if (['speak', 'transcribe'].contains(v['kind']) &&
      ['queued', 'running'].contains(v['status'])) {
    invalid();
  }
  if (!['queued', 'running'].contains(v['status']) &&
      v['phase'] != 'complete') {
    invalid();
  }
  if (!terminal) return;
  if (['queued', 'running'].contains(v['status'])) {
    if (v['result'] != null || v['error'] != null) invalid();
  } else if (v['status'] == 'succeeded') {
    // A retained voice result is an inert expiry receipt, never playable audio.
    if (v['kind'] == 'speak') {
      if (v['result'] != null) invalid();
      wireError(v['error']);
      if (v['error']['code'] != 'AUDIO_EXPIRED') invalid();
      return;
    }
    if (v['error'] != null || v['result'] is! Map<String, dynamic>) invalid();
    final r = v['result'];
    if (['submit', 'submit_new'].contains(v['kind'])) {
      exact(r, ['assistant_text', 'audio', 'speech_error']);
      bounded(r['assistant_text'], 65536, empty: true);
      if ((r['assistant_text'] as String).isEmpty || r['audio'] != null) {
        invalid();
      }
      if (r['speech_error'] != null) {
        if (v['kind'] != 'submit') invalid();
        wireError(r['speech_error']);
      }
    } else if (v['kind'] == 'transcribe') {
      exact(r, ['text', 'language', 'duration_ms']);
      bounded(r['text'], 16384, empty: true);
      bounded(r['language'], 64, empty: true);
      if (!wireRevision(r['duration_ms']) || r['duration_ms'] > 60000) {
        invalid();
      }
    } else if (v['kind'] == 'models') {
      inventory(r);
    } else if ((v['kind'] as String).startsWith('conversation_')) {
      mutation(v['kind'], r);
      if (r['request_id'] != v['request_id'] ||
          (r['conversation_id'] ?? r['conversation']['conversation_id']) !=
              v['conversation_id']) {
        invalid();
      }
    }
  } else {
    if (v['result'] != null) invalid();
    wireError(v['error']);
  }
}

void mutation(String op, dynamic r) {
  if (op == 'conversation_delete') {
    exact(r, ['request_id', 'conversation_id', 'deleted']);
    if (!wireId(r['conversation_id']) || r['deleted'] != true) invalid();
  } else {
    exact(r, ['request_id', 'conversation']);
    conversation(r['conversation']);
  }
  if (!wireId(r['request_id'])) invalid();
}

void validateV2Arguments(String op, Map<String, dynamic> a) {
  final keys = switch (op) {
    'state' => <String>[],
    'conversation_get' => ['conversation_id'],
    'models' => ['request_id'],
    'configure' => ['model', 'voice'],
    'poll' => ['request_id', 'instance_id'],
    'submit_new' => [
      'request_id',
      'text',
      'output',
      'expected_config_revision',
    ],
    'submit' => [
      'request_id',
      'conversation_id',
      'text',
      'output',
      'expected_config_revision',
      'expected_history_revision',
    ],
    'conversation_rename' => [
      'request_id',
      'conversation_id',
      'expected_metadata_revision',
      'title',
    ],
    'conversation_archive' || 'conversation_restore' => [
      'request_id',
      'conversation_id',
      'expected_metadata_revision',
    ],
    'conversation_delete' => [
      'request_id',
      'conversation_id',
      'expected_metadata_revision',
      'expected_history_revision',
    ],
    _ => throw const AppFailure('OP', 'Unsupported application operation.'),
  };
  exact(a, keys);
  for (final key in keys) {
    if (key.endsWith('_id') && !wireId(a[key])) invalid();
    if (key == 'expected_config_revision') {
      if (!wireConfigRevision(a[key])) invalid();
    } else if (key.endsWith('_revision') && !wireRevision(a[key])) {
      invalid();
    }
  }
  if (a.containsKey('text')) {
    bounded(a['text'], 16384);
    if (a['output'] != 'chat') invalid();
  }
  if (a.containsKey('title')) {
    bounded(a['title'], 256);
    if ((a['title'] as String).runes.length > maxChatTitleRunes) {
      throw const AppFailure(
        'TITLE',
        'Chat titles must be 40 characters or fewer.',
      );
    }
    if (a['title'] != (a['title'] as String).trim() ||
        RegExp(
          r'[\x00-\x1f\x7f-\x9f\u061c\u200e\u200f\u2028-\u202e\u2066-\u2069]',
        ).hasMatch(a['title'])) {
      invalid();
    }
  }
  if (op == 'configure') {
    bounded(a['model'], 256);
    if (a['voice'] != null) invalid();
  }
}

void validateV2(String op, Map<String, dynamic> r) {
  switch (op) {
    case 'state':
      exact(r, [
        'config',
        'inventory_observed',
        'inventory',
        'conversations',
        'operations',
        'retention',
        'revision',
      ]);
      exact(r['config'], ['model', 'voice', 'revision']);
      if (!wireConfigRevision(r['config']['revision']) ||
          !wireRevision(r['revision']) ||
          r['inventory_observed'] is! bool) {
        invalid();
      }
      for (final key in ['model', 'voice']) {
        if (r['config'][key] != null) {
          bounded(r['config'][key], 256, empty: true);
          if ((r['config'][key] as String).isEmpty) invalid();
        }
      }
      inventory(r['inventory']);
      if (!r['inventory_observed'] &&
          ((r['inventory']['models'] as List).isNotEmpty ||
              (r['inventory']['voices'] as List).isNotEmpty ||
              (r['inventory']['providers'] as Map).values.any(
                (p) => p['ready'] == true,
              ))) {
        invalid();
      }
      if (r['conversations'] is! List ||
          r['operations'] is! List ||
          r['conversations'].length > 32 ||
          r['operations'].length > 32 ||
          r['retention'] is! Map) {
        invalid();
      }
      exact(r['retention'], [
        'max_conversations',
        'max_turn_pairs',
        'max_context_bytes',
        'max_terminal_jobs',
        'max_expired_ids',
        'max_pending_jobs',
        'queue_timeout_ms',
        'capacity_policy',
        'missing_request_code',
      ]);
      for (final key in [
        'max_conversations',
        'max_turn_pairs',
        'max_context_bytes',
        'max_terminal_jobs',
        'max_expired_ids',
        'max_pending_jobs',
        'queue_timeout_ms',
      ]) {
        if (!wireRevision(r['retention'][key])) invalid();
      }
      bounded(r['retention']['capacity_policy'], 256);
      bounded(r['retention']['missing_request_code'], 256);
      final ids = <String>{};
      for (final c in r['conversations']) {
        conversation(c);
        if (!ids.add(c['conversation_id'])) invalid();
      }
      final requests = <String>{};
      for (final o in r['operations']) {
        operation(o);
        if (!requests.add(o['request_id']) ||
            (o['conversation_id'] != null &&
                o['kind'] != 'conversation_delete' &&
                !ids.contains(o['conversation_id']))) {
          invalid();
        }
      }
    case 'conversation_get':
      exact(r, ['conversation', 'history']);
      conversation(r['conversation']);
      if (r['history'] is! List ||
          r['history'].length > 32 ||
          r['history'].length.isOdd) {
        invalid();
      }
      var bytes = 0;
      var index = 0;
      for (final m in r['history']) {
        exact(m, ['role', 'content']);
        final user = index++ % 2 == 0;
        if (m['role'] != (user ? 'user' : 'assistant')) invalid();
        bounded(m['content'], user ? 16384 : 65536, empty: true);
        bytes += utf8.encode(m['content']).length;
      }
      if (bytes > 65536) invalid();
    case 'configure':
      exact(r, ['model', 'voice', 'revision']);
      if (!wireConfigRevision(r['revision']) || r['voice'] != null) invalid();
      bounded(r['model'], 256);
    case 'poll':
      operation(r, terminal: true);
    case 'conversation_rename':
    case 'conversation_archive':
    case 'conversation_restore':
    case 'conversation_delete':
      mutation(op, r);
    default:
      operation(r);
  }
}
