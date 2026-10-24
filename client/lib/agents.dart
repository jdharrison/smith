import 'dart:convert';

import 'app_api.dart';
import 'app_v2.dart' as v2;

const agentIds = ['developer', 'assurance'];
const maxAgentNameScalars = 40;
const maxAgents = 32;
const maxAgentDescriptionScalars = 40;
const _maxBuiltInDescriptionScalars = 256;
const maxAgentPromptBytes = 4096;
const maxAgentRevision = 9007199254740991;

final _controls = RegExp(r'[\x00-\x1f\x7f-\x9f]');
final _promptControls = RegExp(r'[\x00-\x08\x0b-\x1f\x7f-\x9f]');
bool _validUnicode(String value) => utf8.decode(utf8.encode(value)) == value;
bool agentWireId(dynamic value) =>
    value is String && (agentIds.contains(value) || _identity(value));
bool _identity(dynamic value) =>
    value is String && value.length == 32 && v2.wireId(value);

Never _invalid() =>
    throw const AppFailure('PROTOCOL', 'Invalid Agents metadata.');

/// Validation only: these helpers never trim or otherwise rewrite input.
String? agentNameError(String name) {
  if (name.isEmpty) return 'Enter an agent name.';
  if (name != name.trim()) {
    return 'Remove leading and trailing whitespace from the name.';
  }
  if (!_validUnicode(name) || _controls.hasMatch(name)) {
    return 'Names must not contain control or invalid characters.';
  }
  if (name.runes.length > maxAgentNameScalars) {
    return 'Names must be 40 characters or fewer.';
  }
  return null;
}

String? agentDescriptionError(String description) =>
    _descriptionError(description, maxAgentDescriptionScalars);

String? _descriptionError(String description, int maximum) {
  if (description != description.trim()) {
    return 'Remove leading and trailing whitespace from the description.';
  }
  if (!_validUnicode(description) ||
      _controls.hasMatch(description) ||
      description.contains('\u2028') ||
      description.contains('\u2029')) {
    return 'Descriptions must be a single line without control or invalid characters.';
  }
  if (description.runes.length > maximum) {
    return 'Descriptions must be $maximum characters or fewer.';
  }
  return null;
}

String? agentPromptError(String prompt) {
  if (!_validUnicode(prompt) || _promptControls.hasMatch(prompt)) {
    return 'Prompts may contain line feeds and tabs, but no other control or invalid characters.';
  }
  if (utf8.encode(prompt).length > maxAgentPromptBytes) {
    return 'Prompts must be 4096 UTF-8 bytes or fewer.';
  }
  return null;
}

final class AgentRecord {
  AgentRecord({
    required this.agentId,
    required this.name,
    required this.description,
    required this.prompt,
  }) {
    if (!agentWireId(agentId) ||
        agentNameError(name) != null ||
        _descriptionError(
              description,
              isBuiltIn
                  ? _maxBuiltInDescriptionScalars
                  : maxAgentDescriptionScalars,
            ) !=
            null ||
        agentPromptError(prompt) != null) {
      _invalid();
    }
  }

  factory AgentRecord.parse(dynamic value) {
    v2.exact(value, ['agent_id', 'name', 'description', 'prompt']);
    if (value['agent_id'] is! String ||
        value['name'] is! String ||
        value['description'] is! String ||
        value['prompt'] is! String) {
      _invalid();
    }
    return AgentRecord(
      agentId: value['agent_id'],
      name: value['name'],
      description: value['description'],
      prompt: value['prompt'],
    );
  }

  bool get isBuiltIn => agentIds.contains(agentId);

  final String agentId;
  final String name;
  final String description;
  final String prompt;

  Map<String, dynamic> toJson() => {
    'agent_id': agentId,
    'name': name,
    'description': description,
    'prompt': prompt,
  };
  @override
  bool operator ==(Object other) =>
      other is AgentRecord &&
      agentId == other.agentId &&
      name == other.name &&
      description == other.description &&
      prompt == other.prompt;
  @override
  int get hashCode => Object.hash(agentId, name, description, prompt);
}

final class AgentSnapshot {
  AgentSnapshot(this.revision, List<AgentRecord> agents)
    : agents = List.unmodifiable(agents) {
    if (!v2.wireRevision(revision) ||
        revision < 1 ||
        agents.length < 2 ||
        agents.length > maxAgents ||
        agents.map((a) => a.agentId).toSet().length != agents.length ||
        agents[0].agentId != agentIds[0] ||
        agents[1].agentId != agentIds[1]) {
      _invalid();
    }
  }
  factory AgentSnapshot.parse(dynamic value) {
    v2.exact(value, ['revision', 'agents']);
    if (!v2.wireRevision(value['revision']) || value['agents'] is! List) {
      _invalid();
    }
    return AgentSnapshot(
      value['revision'],
      (value['agents'] as List).map(AgentRecord.parse).toList(),
    );
  }
  final int revision;
  final List<AgentRecord> agents;
}

/// Historical acknowledgement, not a replacement for a current list query.
final class AgentReceipt {
  AgentReceipt.parse(dynamic value) {
    v2.exact(value, ['request_id', 'agent_id', 'revision']);
    if (!_identity(value['request_id']) ||
        !agentWireId(value['agent_id']) ||
        !v2.wireRevision(value['revision']) ||
        value['revision'] < 2) {
      _invalid();
    }
    requestId = value['request_id'];
    agentId = value['agent_id'];
    revision = value['revision'];
  }
  late final String requestId;
  late final String agentId;
  late final int revision;
}

void validateAgentArguments(String op, Map<String, dynamic> args) {
  switch (op) {
    case 'agents_list_v2':
      v2.exact(args, []);
    case 'agent_receipt':
      v2.exact(args, ['request_id']);
      if (!_identity(args['request_id'])) _invalid();
    case 'agent_create_v1':
    case 'agent_update_v2':
      v2.exact(args, [
        'request_id',
        'instance_id',
        'expected_revision',
        if (op == 'agent_update_v2') 'agent_id',
        'name',
        'description',
        'prompt',
      ]);
      if (!_identity(args['request_id']) ||
          !_identity(args['instance_id']) ||
          !v2.wireRevision(args['expected_revision']) ||
          args['expected_revision'] < 1) {
        _invalid();
      }
      if (op == 'agent_create_v1') {
        if (args['name'] is! String ||
            args['description'] is! String ||
            args['prompt'] is! String ||
            agentNameError(args['name']) != null ||
            agentDescriptionError(args['description']) != null ||
            agentPromptError(args['prompt']) != null) {
          _invalid();
        }
        return;
      }
      AgentRecord.parse({
        for (final key in ['agent_id', 'name', 'description', 'prompt'])
          key: args[key],
      });
    default:
      throw const AppFailure('OP', 'Unsupported Agents operation.');
  }
}

void validateAgentResult(String op, Map<String, dynamic> result) {
  switch (op) {
    case 'agents_list_v2':
      AgentSnapshot.parse(result);
    case 'agent_create_v1':
    case 'agent_update_v2':
    case 'agent_receipt':
      AgentReceipt.parse(result);
    default:
      throw const AppFailure('OP', 'Unsupported Agents operation.');
  }
}

void correlateAgentReceipt(
  String op,
  Map<String, dynamic> args,
  Map<String, dynamic> result,
) {
  final receipt = AgentReceipt.parse(result);
  if (receipt.requestId != args['request_id'] ||
      (args.containsKey('agent_id') && receipt.agentId != args['agent_id']) ||
      (op == 'agent_create_v1' && agentIds.contains(receipt.agentId)) ||
      ((op == 'agent_update_v2' || op == 'agent_create_v1') &&
          args.containsKey('expected_revision') &&
          receipt.revision != args['expected_revision'] + 1)) {
    _invalid();
  }
}
