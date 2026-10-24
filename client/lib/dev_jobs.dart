import 'dart:convert';

import 'app_api.dart';
import 'app_v2.dart' as wire;
import 'projects.dart';

const devJobOperations = {
  'dev_jobs_state_v4',
  'dev_job_get_v4',
  'dev_job_start_v4',
  'dev_job_delete_v3',
  'dev_job_deletion_v3',
};
const maxDevPromptBytes = 16384;

Never _invalid() => throw const AppFailure('PROTOCOL', 'Invalid Dev job data.');

/// A well-formed GET returned another request, not merely malformed data.
final class DevJobCorrelationFailure extends AppFailure {
  const DevJobCorrelationFailure()
    : super('PROTOCOL', 'Dev detail request identity mismatch.');
}

bool _text(
  dynamic value,
  int limit, {
  bool empty = false,
  bool multiline = false,
}) =>
    value is String &&
    (empty || value.trim().isNotEmpty) &&
    utf8.encode(value).length <= limit &&
    utf8.decode(utf8.encode(value)) == value &&
    !(multiline
            ? RegExp(r'[\x00-\x08\x0b-\x1f\x7f-\x9f]')
            : RegExp(r'[\x00-\x1f\x7f-\x9f]'))
        .hasMatch(value);
bool _hex(dynamic value, int length) =>
    value is String &&
    value.length == length &&
    RegExp(r'^[0-9a-f]+$').hasMatch(value);

/// Keep the literal decimal string: config revisions may exceed Dart's int range.
bool devConfigRevision(dynamic value) =>
    value is String &&
    value.length <= 20 &&
    RegExp(r'^(0|[1-9][0-9]*)$').hasMatch(value) &&
    BigInt.parse(value) <= BigInt.parse('18446744073709551615');

String? devPromptError(String prompt) {
  if (prompt.trim().isEmpty) return 'Enter a task prompt.';
  if (!_text(prompt, maxDevPromptBytes, multiline: true)) {
    return 'Use at most 16 KiB UTF-8; only tab and line feed controls are allowed.';
  }
  return null;
}

final class DevLocalModel {
  DevLocalModel.parse(dynamic value, {bool state = true}) {
    wire.exact(
      value,
      state
          ? ['name', 'sha256', 'config_revision', 'ready']
          : ['name', 'sha256'],
    );
    if (!_text(value['name'], 256, empty: true) ||
        (value['sha256'] != null && !_hex(value['sha256'], 64)) ||
        (state &&
            (!devConfigRevision(value['config_revision']) ||
                value['ready'] is! bool))) {
      _invalid();
    }
    name = value['name'];
    sha256 = value['sha256'];
    configRevision = state ? value['config_revision'] : null;
    ready = state && value['ready'];
    if (ready && (name.isEmpty || sha256 == null)) _invalid();
  }
  late final String name;
  late final String? sha256, configRevision;
  late final bool ready;
}

final class DevJob {
  DevJob.parse(dynamic value) {
    wire.exact(value, [
      'request_id',
      'project_id',
      'project_key',
      'status',
      'message',
      'session_id',
      'created_ms',
      'reply_available',
      'cleanup_verified',
    ]);
    if (!projectWireId(value['request_id']) ||
        !projectWireId(value['project_id']) ||
        value['project_key'] is! String ||
        projectKeyError(value['project_key']) != null ||
        !const {
          'queued',
          'preparing',
          'cloning',
          'ready',
          'running',
          'completed',
          'failed',
          'unknown',
          'cleanup_unknown',
          'reconciled',
        }.contains(value['status']) ||
        !_text(value['message'], 1024) ||
        (value['session_id'] != null && !_text(value['session_id'], 256)) ||
        value['created_ms'] is! int ||
        value['created_ms'] <= 0 ||
        value['created_ms'] > 253402300799999 ||
        value['reply_available'] is! bool ||
        value['cleanup_verified'] is! bool) {
      _invalid();
    }
    requestId = value['request_id'];
    projectId = value['project_id'];
    projectKey = value['project_key'];
    status = value['status'];
    message = value['message'];
    sessionId = value['session_id'];
    createdMs = value['created_ms'];
    replyAvailable = value['reply_available'];
    cleanupVerified = value['cleanup_verified'];
    if (status == 'completed' && (!replyAvailable || !cleanupVerified)) {
      _invalid();
    }
    if (status == 'queued' &&
        (sessionId != null || replyAvailable || !cleanupVerified)) {
      _invalid();
    }
    if (status == 'cleanup_unknown' && cleanupVerified) _invalid();
    if (status == 'reconciled' && (!cleanupVerified || sessionId == null)) {
      _invalid();
    }
  }
  late final String requestId, projectId, projectKey, status, message;
  late final String? sessionId;
  late final int createdMs;
  late final bool replyAvailable, cleanupVerified;
  bool get active => status == 'queued' || executing;
  bool get executing =>
      const {'preparing', 'cloning', 'ready', 'running'}.contains(status);
  bool get canDelete => permitsNewAttempt;
  bool get permitsNewAttempt =>
      const {'completed', 'failed', 'reconciled'}.contains(status) &&
      cleanupVerified;
}

final class DevScheduler {
  DevScheduler.parse(dynamic value) {
    wire.exact(value, ['limit', 'occupied', 'queued', 'accepting']);
    if (value['limit'] != 3 ||
        value['limit'] is! int ||
        value['occupied'] is! int ||
        value['occupied'] < 0 ||
        value['queued'] is! int ||
        value['queued'] < 0 ||
        value['accepting'] is! bool) {
      _invalid();
    }
    occupied = value['occupied'];
    queued = value['queued'];
    accepting = value['accepting'];
  }
  final int limit = 3;
  late final int occupied, queued;
  late final bool accepting;
}

final class DevJobsState {
  DevJobsState.parse(dynamic value) {
    wire.exact(value, ['jobs', 'used', 'model', 'scheduler']);
    if (value['jobs'] is! List ||
        value['used'] is! int ||
        value['used'] < 0 ||
        value['used'] > 9007199254740991) {
      _invalid();
    }
    jobs = List.unmodifiable((value['jobs'] as List).map(DevJob.parse));
    used = value['used'];
    model = DevLocalModel.parse(value['model']);
    scheduler = DevScheduler.parse(value['scheduler']);
    if (used < jobs.length ||
        jobs.map((j) => j.requestId).toSet().length != jobs.length ||
        jobs.where((j) => j.executing).length > 3) {
      _invalid();
    }
  }
  late final List<DevJob> jobs;
  late final int used;
  late final DevLocalModel model;
  late final DevScheduler scheduler;
}

final class DevJobDeletion {
  DevJobDeletion.parse(dynamic value) {
    wire.exact(value, ['job', 'deleted']);
    job = DevJob.parse(value['job']);
    if (value['deleted'] is! bool) _invalid();
    deleted = value['deleted'];
    if (deleted && !job.canDelete) _invalid();
  }
  late final DevJob job;
  late final bool deleted;
}

final class DevRepositoryEvidence {
  DevRepositoryEvidence.parse(dynamic value) {
    wire.exact(value, ['url', 'ref', 'commit']);
    if (!_text(value['url'], 2048) ||
        repositoryLocatorError(value['url']) != null ||
        !(value['url'] as String).startsWith('https://') ||
        !_text(value['ref'], 1024) ||
        !_hex(value['commit'], 40)) {
      _invalid();
    }
    url = value['url'];
    ref = value['ref'];
    commit = value['commit'];
  }
  late final String url, ref, commit;
}

final class DevJobDetail {
  DevJobDetail.parse(dynamic value) {
    wire.exact(value, [
      'job',
      'model',
      'repositories',
      'reply',
      'failed_stage',
    ]);
    job = DevJob.parse(value['job']);
    model = DevLocalModel.parse(value['model'], state: false);
    if (value['repositories'] is! List ||
        (value['repositories'] as List).length > 16 ||
        (value['reply'] != null &&
            !_text(value['reply'], 65536, multiline: true)) ||
        (value['failed_stage'] != null && !_text(value['failed_stage'], 256))) {
      _invalid();
    }
    repositories = List.unmodifiable(
      (value['repositories'] as List).map(DevRepositoryEvidence.parse),
    );
    reply = value['reply'];
    failedStage = value['failed_stage'];
    if (job.replyAvailable != (reply != null)) _invalid();
    if (job.status == 'reconciled' &&
        (failedStage != 'running' || repositories.isEmpty)) {
      _invalid();
    }
  }
  late final DevJob job;
  late final DevLocalModel model;
  late final List<DevRepositoryEvidence> repositories;
  late final String? reply, failedStage;
}

// Kept for the existing NativeAppApi call site until its generic result hook is wired.
List<DevJob> parseDevJobs(dynamic value) => DevJobsState.parse(value).jobs;

void validateDevArguments(String op, Map<String, dynamic> args) {
  if (!devJobOperations.contains(op)) _invalid();
  wire.exact(
    args,
    op == 'dev_jobs_state_v4'
        ? []
        : op == 'dev_job_get_v4' || op == 'dev_job_deletion_v3'
        ? ['request_id']
        : op == 'dev_job_delete_v3'
        ? ['request_id', 'instance_id']
        : [
            'request_id',
            'instance_id',
            'project_id',
            'expected_projects_revision',
            'expected_config_revision',
            'prompt',
          ],
  );
  if (op == 'dev_jobs_state_v4') return;
  if (!projectWireId(args['request_id'])) _invalid();
  if (op == 'dev_job_get_v4' || op == 'dev_job_deletion_v3') return;
  if (op == 'dev_job_delete_v3') {
    if (!projectWireId(args['instance_id'])) _invalid();
    return;
  }
  if (!projectWireId(args['instance_id']) ||
      !projectWireId(args['project_id']) ||
      !wire.wireRevision(args['expected_projects_revision']) ||
      args['expected_projects_revision'] < 1 ||
      !devConfigRevision(args['expected_config_revision']) ||
      args['prompt'] is! String ||
      devPromptError(args['prompt']) != null) {
    _invalid();
  }
}

DevJob correlateDevJob(String op, Map<String, dynamic> args, dynamic result) {
  final job = op == 'dev_job_get_v4'
      ? DevJobDetail.parse(result).job
      : op == 'dev_job_delete_v3' || op == 'dev_job_deletion_v3'
      ? DevJobDeletion.parse(result).job
      : DevJob.parse(result);
  if (job.requestId != args['request_id'] ||
      (op == 'dev_job_start_v4' && job.projectId != args['project_id'])) {
    if (op == 'dev_job_get_v4') throw const DevJobCorrelationFailure();
    _invalid();
  }
  return job;
}

/// NativeAppApi's generic hook for every member of [devJobOperations].
void validateDevResult(String op, Map<String, dynamic> args, dynamic result) {
  validateDevArguments(op, args);
  if (op == 'dev_jobs_state_v4') {
    DevJobsState.parse(result);
  } else {
    correlateDevJob(op, args, result);
  }
}
