import 'dart:convert';

import 'app_api.dart';
import 'app_v2.dart' as v2;

const maxProjects = 32;
const maxProjectRepositories = 16;
const maxProjectNameScalars = 40;
const maxProjectKeyLength = 40;
const maxProjectDescriptionScalars = 60;
const maxProjectPromptScalars = 1000;
const maxRepositoryLocatorBytes = 2048;
const maxProjectRevision = 9007199254740991;
const projectAgent = 'opencode';

final _controls = RegExp(r'[\x00-\x1f\x7f-\x9f]');

final _urlLocator = RegExp(
  r'^(?:https://|ssh://git@)([A-Za-z0-9.-]+)(?::([0-9]+))?/([A-Za-z0-9._~/-]+)$',
);
final _scpLocator = RegExp(r'^git@([A-Za-z0-9.-]+):([A-Za-z0-9._~/-]+)$');
final _hostLabel = RegExp(r'^[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?$');

bool _wholeMatch(RegExp pattern, String value) {
  final match = pattern.firstMatch(value);
  return match != null && match.start == 0 && match.end == value.length;
}

bool projectWireId(dynamic value) =>
    value is String && value.length == 32 && v2.wireId(value);

Never _invalid() =>
    throw const AppFailure('PROTOCOL', 'Invalid Projects metadata.');

/// Validates without trimming, truncating or changing the saved identity.
String? projectNameError(String name) {
  if (name.isEmpty) return 'Enter a project name.';
  if (name != name.trim()) {
    return 'Remove leading and trailing whitespace from the project name.';
  }
  if (_controls.hasMatch(name) || utf8.decode(utf8.encode(name)) != name) {
    return 'Project names must not contain control or invalid characters.';
  }
  if (name.runes.length > maxProjectNameScalars) {
    return 'Project names must be 40 characters or fewer.';
  }
  return null;
}

String? projectKeyError(String key) =>
    key.length > maxProjectKeyLength ||
        !_wholeMatch(RegExp(r'^[a-z0-9]+(?:-[a-z0-9]+)*$'), key)
    ? 'Use a unique lowercase kebab key of 1–40 ASCII characters.'
    : null;

String? projectDescriptionError(String description) {
  if (description.runes.length > maxProjectDescriptionScalars ||
      _controls.hasMatch(description) ||
      description.contains('\u2028') ||
      description.contains('\u2029') ||
      utf8.decode(utf8.encode(description)) != description) {
    return 'Use a single-line description of at most 60 characters without controls.';
  }
  return null;
}

String? projectPromptError(String prompt) {
  if (prompt.runes.length > maxProjectPromptScalars ||
      RegExp(r'[\x00-\x08\x0b-\x1f\x7f-\x9f]').hasMatch(prompt) ||
      utf8.decode(utf8.encode(prompt)) != prompt) {
    return 'Use at most 1000 characters; only line feed and tab controls are allowed.';
  }
  return null;
}

/// Syntax only: this never resolves a host, probes a repository or opens a path.
String? repositoryLocatorError(String locator) {
  const invalid =
      'Use https://host/path, ssh://git@host/path or git@host:path without credentials, query or fragment.';
  if (utf8.encode(locator).length > maxRepositoryLocatorBytes) {
    return 'Repository locators must be 2048 UTF-8 bytes or fewer.';
  }
  final url = _urlLocator.firstMatch(locator);
  final scp = _scpLocator.firstMatch(locator);
  final match = url ?? scp;
  if (match == null || match.start != 0 || match.end != locator.length) {
    return invalid;
  }
  final host = match[1]!;
  if (host.length > 253 ||
      host
          .split('.')
          .any(
            (label) => label.length > 63 || !_wholeMatch(_hostLabel, label),
          )) {
    return invalid;
  }
  if (url != null && url[2] != null) {
    final port = BigInt.parse(url[2]!);
    if (port < BigInt.one || port > BigInt.from(65535)) return invalid;
  }
  final path = url != null ? url[3]! : scp![2]!;
  if (path.split('/').any((s) => s.isEmpty || s == '.' || s == '..')) {
    return invalid;
  }
  return null;
}

String? projectRepositoryNameError(String? name) =>
    name == null ? null : projectNameError(name);

/// New-write rule only; legacy repository Names remain readable without changes.
String? projectRepositoryIdError(String? id) {
  if (id == null || id.isEmpty) return 'Enter a repository ID.';
  return projectKeyError(id) == null
      ? null
      : 'Use a lowercase kebab repository ID of 1–40 ASCII characters.';
}

String? projectBranchNameError(String? name) {
  if (name == null) return null;
  const error = 'Use a safe ASCII branch name of 1–255 characters.';
  if (name.isEmpty ||
      name.length > 255 ||
      name == 'HEAD' ||
      name.startsWith('refs/')) {
    return error;
  }
  for (final component in name.split('/')) {
    if (!_wholeMatch(RegExp(r'^[A-Za-z0-9][A-Za-z0-9._-]*$'), component) ||
        component.contains('..') ||
        component.endsWith('.') ||
        component.toLowerCase().endsWith('.lock')) {
      return error;
    }
  }
  return null;
}

String? projectWorkBranchError(String? work) =>
    work != null && work.endsWith('/*')
    ? projectBranchTypeError(work)
    : projectBranchNameError(work);

String? projectBranchTypeError(String? pattern) {
  if (pattern == null) return null;
  if (pattern.length > 255 ||
      !pattern.endsWith('/*') ||
      projectBranchNameError(pattern.substring(0, pattern.length - 2)) !=
          null) {
    return 'Use a safe branch prefix followed by /*, at most 255 ASCII characters.';
  }
  return null;
}

String? _nullableString(dynamic value) {
  if (value != null && value is! String) _invalid();
  return value as String?;
}

final class ProjectBranchTypes {
  ProjectBranchTypes({this.feature, this.fix, this.release, this.hotfix}) {
    for (final value in [feature, fix, release, hotfix]) {
      if (projectBranchTypeError(value) != null) _invalid();
    }
  }

  factory ProjectBranchTypes.parse(dynamic value) {
    v2.exact(value, ['feature', 'fix', 'release', 'hotfix']);
    return ProjectBranchTypes(
      feature: _nullableString(value['feature']),
      fix: _nullableString(value['fix']),
      release: _nullableString(value['release']),
      hotfix: _nullableString(value['hotfix']),
    );
  }

  final String? feature;
  final String? fix;
  final String? release;
  final String? hotfix;

  Map<String, dynamic> toJson() => {
    'feature': feature,
    'fix': fix,
    'release': release,
    'hotfix': hotfix,
  };

  @override
  bool operator ==(Object other) =>
      other is ProjectBranchTypes &&
      feature == other.feature &&
      fix == other.fix &&
      release == other.release &&
      hotfix == other.hotfix;
  @override
  int get hashCode => Object.hash(feature, fix, release, hotfix);
}

final class ProjectBranches {
  ProjectBranches({this.base, this.work, ProjectBranchTypes? types})
    : types = types ?? ProjectBranchTypes() {
    if (projectBranchNameError(base) != null ||
        projectWorkBranchError(work) != null) {
      _invalid();
    }
  }

  factory ProjectBranches.parse(dynamic value) {
    v2.exact(value, ['base', 'work', 'types']);
    return ProjectBranches(
      base: _nullableString(value['base']),
      work: _nullableString(value['work']),
      types: ProjectBranchTypes.parse(value['types']),
    );
  }

  final String? base;
  final String? work;
  final ProjectBranchTypes types;

  Map<String, dynamic> toJson() => {
    'base': base,
    'work': work,
    'types': types.toJson(),
  };

  @override
  bool operator ==(Object other) =>
      other is ProjectBranches &&
      base == other.base &&
      work == other.work &&
      types == other.types;
  @override
  int get hashCode => Object.hash(base, work, types);
}

final class ProjectRepository {
  ProjectRepository({
    this.name,
    required this.url,
    ProjectBranches? branch,
    this.readOnly = true,
  }) : branch = branch ?? ProjectBranches() {
    if (projectRepositoryNameError(name) != null ||
        repositoryLocatorError(url) != null) {
      _invalid();
    }
  }

  factory ProjectRepository.parse(dynamic value) {
    v2.exact(value, ['name', 'url', 'branch', 'read_only']);
    if (value['url'] is! String || value['read_only'] is! bool) _invalid();
    return ProjectRepository(
      name: _nullableString(value['name']),
      url: value['url'],
      branch: ProjectBranches.parse(value['branch']),
      readOnly: value['read_only'],
    );
  }

  final String? name;
  final String url;
  final ProjectBranches branch;
  final bool readOnly;

  Map<String, dynamic> toJson() => {
    'name': name,
    'url': url,
    'branch': branch.toJson(),
    'read_only': readOnly,
  };

  @override
  bool operator ==(Object other) =>
      other is ProjectRepository &&
      name == other.name &&
      url == other.url &&
      branch == other.branch &&
      readOnly == other.readOnly;
  @override
  int get hashCode => Object.hash(name, url, branch, readOnly);
}

/// Fresh recursively immutable wire capture, including every nested container.
Map<String, dynamic> freezeProjectArguments(Map<String, dynamic> args) {
  dynamic freeze(dynamic value) {
    if (value is Map<String, dynamic>) {
      return Map<String, dynamic>.unmodifiable(
        value.map((key, value) => MapEntry(key, freeze(value))),
      );
    }
    if (value is List) return List<dynamic>.unmodifiable(value.map(freeze));
    return value;
  }

  return freeze(args) as Map<String, dynamic>;
}

void _metadata(
  dynamic name,
  dynamic repositories,
  dynamic agent,
  dynamic key,
  dynamic description,
  dynamic prompt, {
  bool wire = true,
}) {
  if (key is! String ||
      projectKeyError(key) != null ||
      description is! String ||
      projectDescriptionError(description) != null ||
      prompt is! String ||
      projectPromptError(prompt) != null ||
      name is! String ||
      projectNameError(name) != null ||
      agent is! String ||
      agent != projectAgent ||
      repositories is! List ||
      repositories.length > maxProjectRepositories) {
    _invalid();
  }
  final names = <String>{};
  final urls = <String>{};
  for (final value in repositories) {
    final ProjectRepository repository;
    if (wire) {
      repository = ProjectRepository.parse(value);
    } else {
      if (value is! ProjectRepository) _invalid();
      repository = value;
    }
    if (!urls.add(repository.url) ||
        (repository.name != null && !names.add(repository.name!))) {
      _invalid();
    }
  }
}

class ProjectRecord {
  ProjectRecord(
    this.projectId,
    this.name,
    List<ProjectRepository> repositories,
    this.agent, {
    String? key,
    this.description = '',
    this.prompt = '',
  }) : key = key ?? 'project-$projectId',
       repositories = List.unmodifiable(repositories) {
    if (!projectWireId(projectId)) _invalid();
    _metadata(
      name,
      this.repositories,
      agent,
      this.key,
      description,
      prompt,
      wire: false,
    );
  }

  factory ProjectRecord.parse(dynamic value) {
    v2.exact(value, [
      'project_id',
      'name',
      'repositories',
      'agent',
      'key',
      'description',
      'prompt',
    ]);
    if (!projectWireId(value['project_id'])) _invalid();
    _metadata(
      value['name'],
      value['repositories'],
      value['agent'],
      value['key'],
      value['description'],
      value['prompt'],
    );
    return ProjectRecord(
      value['project_id'],
      value['name'],
      (value['repositories'] as List).map(ProjectRepository.parse).toList(),
      value['agent'],
      key: value['key'],
      description: value['description'],
      prompt: value['prompt'],
    );
  }

  final String projectId;
  final String name;
  final List<ProjectRepository> repositories;
  final String agent;
  final String key;
  final String description;
  final String prompt;

  @override
  bool operator ==(Object other) =>
      other is ProjectRecord &&
      projectId == other.projectId &&
      name == other.name &&
      agent == other.agent &&
      key == other.key &&
      description == other.description &&
      prompt == other.prompt &&
      repositories.length == other.repositories.length &&
      List.generate(
        repositories.length,
        (i) => repositories[i] == other.repositories[i],
      ).every((equal) => equal);

  @override
  int get hashCode => Object.hash(
    projectId,
    name,
    agent,
    key,
    description,
    prompt,
    Object.hashAll(repositories),
  );
}

class ProjectSnapshot {
  ProjectSnapshot(this.revision, List<ProjectRecord> projects)
    : projects = List.unmodifiable(projects) {
    if (!v2.wireRevision(revision) || projects.length > maxProjects) _invalid();
    final ids = <String>{};
    final keys = <String>{};
    for (final project in projects) {
      if (!ids.add(project.projectId) || !keys.add(project.key)) _invalid();
    }
  }

  factory ProjectSnapshot.parse(dynamic value) {
    v2.exact(value, ['revision', 'projects']);
    if (!v2.wireRevision(value['revision']) ||
        value['projects'] is! List ||
        value['projects'].length > maxProjects) {
      _invalid();
    }
    return ProjectSnapshot(
      value['revision'],
      (value['projects'] as List).map(ProjectRecord.parse).toList(),
    );
  }

  final int revision;
  final List<ProjectRecord> projects;
}

/// Historical acknowledgement only, never evidence of current membership.
class ProjectReceipt {
  ProjectReceipt.parse(dynamic value) {
    v2.exact(value, ['request_id', 'project_id', 'revision', 'deleted']);
    if (!projectWireId(value['request_id']) ||
        !projectWireId(value['project_id']) ||
        !v2.wireRevision(value['revision']) ||
        value['deleted'] is! bool) {
      _invalid();
    }
    requestId = value['request_id'];
    projectId = value['project_id'];
    revision = value['revision'];
    deleted = value['deleted'];
  }

  late final String requestId;
  late final String projectId;
  late final int revision;
  late final bool deleted;
}

void validateProjectArguments(String op, Map<String, dynamic> args) {
  final keys = switch (op) {
    'projects_list_v3' => <String>[],
    'project_receipt' => ['request_id'],
    'project_create_v3' || 'project_update_v3' || 'project_delete_v3' => [
      'request_id',
      'instance_id',
      'expected_revision',
      if (op != 'project_create_v3') 'project_id',
      if (op != 'project_delete_v3') ...[
        'name',
        'repositories',
        'agent',
        'key',
        'description',
        'prompt',
      ],
    ],
    _ => throw const AppFailure('OP', 'Unsupported Projects operation.'),
  };
  v2.exact(args, keys);
  for (final key in keys) {
    if (key.endsWith('_id') && !projectWireId(args[key])) _invalid();
  }
  if (args.containsKey('expected_revision') &&
      !v2.wireRevision(args['expected_revision'])) {
    _invalid();
  }
  if (op == 'project_create_v3' || op == 'project_update_v3') {
    _metadata(
      args['name'],
      args['repositories'],
      args['agent'],
      args['key'],
      args['description'],
      args['prompt'],
    );
    for (final repository in args['repositories'] as List) {
      if (projectRepositoryIdError(repository['name'] as String?) != null) {
        _invalid();
      }
    }
  }
}

void validateProjectResult(String op, Map<String, dynamic> result) {
  if (!const {
    'projects_list_v3',
    'project_create_v3',
    'project_update_v3',
    'project_delete_v3',
    'project_receipt',
  }.contains(op)) {
    throw const AppFailure('OP', 'Unsupported Projects operation.');
  }
  if (op == 'projects_list_v3') {
    ProjectSnapshot.parse(result);
    return;
  }
  final receipt = ProjectReceipt.parse(result);
  if (op != 'project_receipt' &&
      receipt.deleted != (op == 'project_delete_v3')) {
    _invalid();
  }
}

void correlateProjectReceipt(
  String op,
  Map<String, dynamic> args,
  Map<String, dynamic> result,
) {
  final receipt = ProjectReceipt.parse(result);
  if (receipt.requestId != args['request_id'] ||
      (args.containsKey('project_id') &&
          receipt.projectId != args['project_id']) ||
      (op != 'project_receipt' &&
          receipt.deleted != (op == 'project_delete_v3'))) {
    _invalid();
  }
}
