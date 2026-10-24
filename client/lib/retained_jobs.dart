import 'dart:convert';

import 'app_api.dart';
import 'projects.dart' show projectWireId, maxProjectRevision;

const retainedJobOperations = {
  'dev_sessions_state_v1',
  'dev_session_get_v1',
  'dev_session_receipt_v1',
  'dev_session_start_v1',
  'dev_session_prompt_v1',
  'dev_session_finish_v1',
  'dev_session_delete_v1',
};
Never _bad() =>
    throw const AppFailure('PROTOCOL', 'Invalid retained job data.');
Map<String, dynamic> _map(dynamic v, List<String> keys) {
  if (v is! Map<String, dynamic> ||
      v.length != keys.length ||
      !keys.every(v.containsKey)) {
    _bad();
  }
  return v;
}

int retainedRevision(dynamic v) {
  if (v is! int || v < 0 || v > maxProjectRevision) _bad();
  return v;
}

String _id(dynamic v) {
  if (!projectWireId(v)) _bad();
  return v as String;
}

String _text(dynamic v, int max) {
  if (v is! String || utf8.encode(v).length > max) _bad();
  return v;
}

bool _bool(dynamic v) {
  if (v is! bool) _bad();
  return v;
}

String _choice(dynamic v, Set<String> choices) {
  if (v is! String || !choices.contains(v)) _bad();
  return v;
}

bool retainedPromptValid(String v) =>
    v.trim().isNotEmpty && utf8.encode(v).length <= 16384;

class RetainedJob {
  RetainedJob.parse(dynamic value) {
    final v = _map(value, [
      'session_id',
      'project_id',
      'project_key',
      'job_type',
      'status',
      'retained',
      'deleted',
      'created_ms',
    ]);
    sessionId = _id(v['session_id']);
    projectId = _id(v['project_id']);
    projectKey = _text(v['project_key'], 40);
    jobType = _choice(v['job_type'], {'read', 'write'});
    status = _choice(v['status'], {
      'queued',
      'creating',
      'ready',
      'prompting',
      'failed',
      'unknown',
      'finishing',
      'cleanup_unknown',
      'finished',
      'interrupted',
    });
    retained = _bool(v['retained']);
    deleted = _bool(v['deleted']);
    createdMs = retainedRevision(v['created_ms']);
    if (projectKey.isEmpty || (deleted && (retained || status != 'finished'))) {
      _bad();
    }
  }
  late final String sessionId, projectId, projectKey, jobType, status;
  late final bool retained, deleted;
  late final int createdMs;
  bool sameIdentity(RetainedJob other) =>
      sessionId == other.sessionId &&
      projectId == other.projectId &&
      projectKey == other.projectKey &&
      jobType == other.jobType &&
      createdMs == other.createdMs;
  bool sameSnapshot(RetainedJob other) =>
      sameIdentity(other) &&
      status == other.status &&
      retained == other.retained &&
      deleted == other.deleted;
  bool get active =>
      {'queued', 'creating', 'prompting', 'finishing'}.contains(status);
  bool get canFinish =>
      !deleted &&
          !active &&
          !{'unknown', 'cleanup_unknown', 'finished'}.contains(status) ||
      (!deleted && status == 'queued');
}

class RetainedTurn {
  RetainedTurn.parse(dynamic value) {
    final v = _map(value, ['operation_id', 'prompt', 'outcome', 'reply']);
    operationId = _id(v['operation_id']);
    prompt = _text(v['prompt'], 16384);
    if (!retainedPromptValid(prompt)) _bad();
    outcome = _choice(v['outcome'], {
      'pending',
      'succeeded',
      'failed',
      'unknown',
    });
    reply = v['reply'] == null ? null : _text(v['reply'], 32768);
  }
  late final String operationId, prompt, outcome;
  late final String? reply;
}

class RetainedRepository {
  RetainedRepository.parse(dynamic value) {
    final v = _map(value, ['url', 'reference', 'read_only']);
    url = _text(v['url'], 2048);
    reference = v['reference'] == null ? null : _text(v['reference'], 256);
    readOnly = _bool(v['read_only']);
  }
  late final String url;
  late final String? reference;
  late final bool readOnly;
}

class RetainedState {
  RetainedState.parse(Map<String, dynamic> value) {
    final v = _map(value, ['revision', 'jobs', 'capability', 'scheduler']);
    revision = retainedRevision(v['revision']);
    if (v['jobs'] is! List || (v['jobs'] as List).length > 32) _bad();
    jobs = List.unmodifiable((v['jobs'] as List).map(RetainedJob.parse));
    if (jobs.any((j) => j.deleted) ||
        jobs.map((j) => j.sessionId).toSet().length != jobs.length) {
      _bad();
    }
    final c = _map(v['capability'], ['available', 'reason', 'job_types']);
    available = _bool(c['available']);
    reason = c['reason'] == null && available ? null : _text(c['reason'], 256);
    if (c['job_types'] is! List ||
        (c['job_types'] as List).length != 2 ||
        !(c['job_types'] as List).toSet().containsAll(['read', 'write'])) {
      _bad();
    }
    final s = _map(v['scheduler'], [
      'limit',
      'occupied',
      'queued',
      'accepting',
    ]);
    limit = retainedRevision(s['limit']);
    occupied = retainedRevision(s['occupied']);
    queued = retainedRevision(s['queued']);
    accepting = _bool(s['accepting']);
  }
  late final int revision, limit, occupied, queued;
  late final List<RetainedJob> jobs;
  late final bool available, accepting;
  late final String? reason;
}

class RetainedDetail {
  RetainedDetail.parse(Map<String, dynamic> value) {
    final v = _map(value, ['revision', 'job', 'turns', 'repositories']);
    revision = retainedRevision(v['revision']);
    job = RetainedJob.parse(v['job']);
    if (v['turns'] is! List ||
        (v['turns'] as List).isEmpty ||
        (v['turns'] as List).length > 32 ||
        v['repositories'] is! List ||
        (v['repositories'] as List).length > 16) {
      _bad();
    }
    turns = List.unmodifiable((v['turns'] as List).map(RetainedTurn.parse));
    repositories = List.unmodifiable(
      (v['repositories'] as List).map(RetainedRepository.parse),
    );
    if (job.jobType == 'read' && repositories.any((r) => !r.readOnly)) {
      _bad();
    }
  }
  late final int revision;
  late final RetainedJob job;
  late final List<RetainedTurn> turns;
  late final List<RetainedRepository> repositories;
}

class RetainedReceipt {
  RetainedReceipt.parse(Map<String, dynamic> value) {
    final v = _map(value, ['receipt', 'job']);
    final r = _map(v['receipt'], ['operation_id', 'session_id', 'revision']);
    operationId = _id(r['operation_id']);
    sessionId = _id(r['session_id']);
    revision = retainedRevision(r['revision']);
    job = RetainedJob.parse(v['job']);
    if (job.sessionId != sessionId) _bad();
  }
  late final String operationId, sessionId;
  late final int revision;
  late final RetainedJob job;
}

void validateRetainedArguments(String op, Map<String, dynamic> args) {
  if (!retainedJobOperations.contains(op)) _bad();
  if (op == 'dev_sessions_state_v1') {
    _map(args, []);
    return;
  }
  if (op == 'dev_session_receipt_v1') {
    _map(args, ['operation_id']);
    _id(args['operation_id']);
    return;
  }
  if (op == 'dev_session_get_v1') {
    _map(args, ['session_id']);
    _id(args['session_id']);
    return;
  }
  final start = op == 'dev_session_start_v1';
  final prompt = start || op == 'dev_session_prompt_v1';
  _map(args, [
    'operation_id',
    'instance_id',
    'expected_revision',
    if (start) ...[
      'expected_projects_revision',
      'project_id',
      'job_type',
    ] else
      'session_id',
    if (prompt) 'prompt',
  ]);
  _id(args['operation_id']);
  _id(args['instance_id']);
  retainedRevision(args['expected_revision']);
  if (start) {
    retainedRevision(args['expected_projects_revision']);
    _id(args['project_id']);
    _choice(args['job_type'], {'read', 'write'});
  } else {
    _id(args['session_id']);
  }
  if (prompt && !retainedPromptValid(_text(args['prompt'], 16384))) _bad();
}

void validateRetainedResult(
  String op,
  Map<String, dynamic> args,
  Map<String, dynamic> result,
) {
  if (op == 'dev_sessions_state_v1') {
    RetainedState.parse(result);
  } else if (op == 'dev_session_get_v1') {
    if (RetainedDetail.parse(result).job.sessionId != args['session_id']) {
      _bad();
    }
  } else {
    final r = RetainedReceipt.parse(result);
    if (r.operationId != args['operation_id'] ||
        (args.containsKey('session_id') && r.sessionId != args['session_id'])) {
      _bad();
    }
  }
}
