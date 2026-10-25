import 'dart:convert';

import 'app_api.dart';
import 'app_v2.dart' as v2;
import 'cloud_routing.dart';

const maxPersonalInstructionBytes = 1024;
const maxInstructionPreambleBytes = 2048;
const instructionVessel = 'chat::global::assistant';

bool _id(dynamic value) =>
    value is String && value.length == 32 && v2.wireId(value);
bool _hash(dynamic value) =>
    value is String &&
    RegExp(r'^[0-9a-f]{64}$').hasMatch(value) &&
    value.length == 64;
bool _version(dynamic value) =>
    value is String &&
    value.length <= 64 &&
    RegExp(r'^[A-Za-z0-9._-]+$').hasMatch(value);
Never _invalid() =>
    throw const AppFailure('PROTOCOL', 'Invalid shared instruction data.');

bool _text(dynamic value, int limit, {bool empty = true}) =>
    value is String &&
    (empty || value.isNotEmpty) &&
    utf8.encode(value).length <= limit &&
    utf8.decode(utf8.encode(value)) == value &&
    !value.runes.any(
      (c) => (c < 32 && c != 9 && c != 10) || (c >= 127 && c <= 159),
    );

/// Immutable editor value. Invalid drafts remain editable; validate before Apply.
final class InstructionSettings {
  const InstructionSettings({
    this.instructions = '',
    this.personality = '',
    this.guardrails = '',
    this.cloudEnabled = false,
  });

  factory InstructionSettings.parse(dynamic value) {
    v2.exact(value, [
      'instructions',
      'personality',
      'guardrails',
      'cloud_enabled',
    ]);
    if (value['instructions'] is! String ||
        value['personality'] is! String ||
        value['guardrails'] is! String ||
        value['cloud_enabled'] is! bool) {
      _invalid();
    }
    final settings = InstructionSettings(
      instructions: value['instructions'],
      personality: value['personality'],
      guardrails: value['guardrails'],
      cloudEnabled: value['cloud_enabled'],
    );
    if (settings.validationError != null) _invalid();
    return settings;
  }

  final String instructions;
  final String personality;
  final String guardrails;
  final bool cloudEnabled;
  int get personalBytes =>
      utf8.encode(instructions).length +
      utf8.encode(personality).length +
      utf8.encode(guardrails).length;
  bool get hasPersonalText =>
      instructions.isNotEmpty ||
      personality.isNotEmpty ||
      guardrails.isNotEmpty;
  String? get validationError {
    if (![
      instructions,
      personality,
      guardrails,
    ].every((s) => _text(s, maxPersonalInstructionBytes))) {
      return 'Use valid text with only LF and tab control characters, at most 1024 combined UTF-8 bytes.';
    }
    if (personalBytes > maxPersonalInstructionBytes) {
      return 'Personal instructions, personality and guardrails must total at most 1024 UTF-8 bytes.';
    }
    return null;
  }

  InstructionSettings copyWith({
    String? instructions,
    String? personality,
    String? guardrails,
    bool? cloudEnabled,
  }) => InstructionSettings(
    instructions: instructions ?? this.instructions,
    personality: personality ?? this.personality,
    guardrails: guardrails ?? this.guardrails,
    cloudEnabled: cloudEnabled ?? this.cloudEnabled,
  );
  Map<String, dynamic> toJson() => {
    'instructions': instructions,
    'personality': personality,
    'guardrails': guardrails,
    'cloud_enabled': cloudEnabled,
  };
  @override
  bool operator ==(Object other) =>
      other is InstructionSettings &&
      instructions == other.instructions &&
      personality == other.personality &&
      guardrails == other.guardrails &&
      cloudEnabled == other.cloudEnabled;
  @override
  int get hashCode =>
      Object.hash(instructions, personality, guardrails, cloudEnabled);
}

final class InstructionLayer {
  InstructionLayer.parse(dynamic value) {
    v2.exact(value, ['version', 'text']);
    if (!_version(value['version']) ||
        !_text(value['text'], maxInstructionPreambleBytes, empty: false)) {
      _invalid();
    }
    version = value['version'];
    text = value['text'];
  }
  late final String version, text;
}

final class InstructionPreview {
  InstructionPreview.parse(dynamic value, {bool cloud = false}) {
    v2.exact(value, [
      'text',
      'sha256',
      if (cloud) ...['bundle_sha256', 'personal_included', 'eligible'],
    ]);
    if (!_text(value['text'], maxInstructionPreambleBytes, empty: false) ||
        !_hash(value['sha256'])) {
      _invalid();
    }
    text = value['text'];
    sha256 = value['sha256'];
    if (cloud) {
      if (!_hash(value['bundle_sha256']) ||
          value['personal_included'] is! bool ||
          value['eligible'] is! bool) {
        _invalid();
      }
      bundleSha256 = value['bundle_sha256'];
      personalIncluded = value['personal_included'];
      eligible = value['eligible'];
    } else {
      bundleSha256 = null;
      personalIncluded = null;
      eligible = null;
    }
  }
  late final String text, sha256;
  late final String? bundleSha256;
  late final bool? personalIncluded, eligible;
}

final class InstructionSnapshot {
  InstructionSnapshot.parse(dynamic value) {
    v2.exact(value, [
      'config_revision',
      'settings',
      'renderer_version',
      'core',
      'product',
      'vessel',
      'local',
      'cloud',
      'limits',
    ]);
    if (!v2.wireConfigRevision(value['config_revision']) ||
        !_version(value['renderer_version']) ||
        (value['vessel'] != null && value['vessel'] != instructionVessel)) {
      _invalid();
    }
    configRevision = value['config_revision'];
    settings = InstructionSettings.parse(value['settings']);
    rendererVersion = value['renderer_version'];
    core = InstructionLayer.parse(value['core']);
    product = InstructionLayer.parse(value['product']);
    vessel = value['vessel'];
    local = InstructionPreview.parse(value['local']);
    cloud = InstructionPreview.parse(value['cloud'], cloud: true);
    v2.exact(value['limits'], ['max_personal_bytes', 'max_preamble_bytes']);
    if (value['limits']['max_personal_bytes'] != maxPersonalInstructionBytes ||
        value['limits']['max_preamble_bytes'] != maxInstructionPreambleBytes ||
        cloud.personalIncluded != settings.hasPersonalText ||
        cloud.eligible !=
            (!settings.hasPersonalText || settings.cloudEnabled) ||
        local.text != cloud.text ||
        local.sha256 != cloud.sha256) {
      _invalid();
    }
  }
  late final Object configRevision;
  late final InstructionSettings settings;
  late final String rendererVersion;
  late final InstructionLayer core, product;
  late final String? vessel;
  late final InstructionPreview local, cloud;
  int get maxPersonalBytes => maxPersonalInstructionBytes;
  int get maxPreambleBytes => maxInstructionPreambleBytes;
  void matchRoute(ChatRoute? route) {
    if (vessel != (route == null ? null : instructionVessel)) _invalid();
  }
}

final class InstructionReceipt {
  InstructionReceipt.parse(dynamic value) {
    v2.exact(value, ['request_id', 'config_revision', 'changed']);
    if (!_id(value['request_id']) ||
        !v2.wireConfigRevision(value['config_revision']) ||
        value['changed'] is! bool) {
      _invalid();
    }
    requestId = value['request_id'];
    configRevision = value['config_revision'];
    changed = value['changed'];
  }
  late final String requestId;
  late final Object configRevision;
  late final bool changed;
}

BigInt instructionRevisionNumber(Object revision) {
  if (!v2.wireConfigRevision(revision)) _invalid();
  return BigInt.parse('$revision');
}

void validateInstructionArguments(String op, Map<String, dynamic> args) {
  switch (op) {
    case 'instructions_get':
      v2.exact(args, ['route']);
      if (args['route'] != null) ChatRoute.parse(args['route']);
    case 'instructions_receipt':
      v2.exact(args, ['request_id']);
      if (!_id(args['request_id'])) _invalid();
    case 'instructions_update':
      v2.exact(args, [
        'request_id',
        'instance_id',
        'expected_config_revision',
        'settings',
      ]);
      if (!_id(args['request_id']) ||
          !_id(args['instance_id']) ||
          !v2.wireConfigRevision(args['expected_config_revision'])) {
        _invalid();
      }
      InstructionSettings.parse(args['settings']);
    default:
      throw const AppFailure('OP', 'Unsupported instruction operation.');
  }
}

void validateInstructionResult(String op, Map<String, dynamic> result) {
  if (!const {
    'instructions_get',
    'instructions_update',
    'instructions_receipt',
  }.contains(op)) {
    throw const AppFailure('OP', 'Unsupported instruction operation.');
  }
  if (op == 'instructions_get') {
    InstructionSnapshot.parse(result);
  } else {
    InstructionReceipt.parse(result);
  }
}

void correlateInstructionResult(
  String op,
  Map<String, dynamic> args,
  Map<String, dynamic> result,
) {
  if (op == 'instructions_get') {
    InstructionSnapshot.parse(
      result,
    ).matchRoute(args['route'] == null ? null : ChatRoute.parse(args['route']));
    return;
  }
  final receipt = InstructionReceipt.parse(result);
  if (receipt.requestId != args['request_id']) _invalid();
  if (op == 'instructions_update') {
    final expected = instructionRevisionNumber(
      args['expected_config_revision'],
    );
    if (instructionRevisionNumber(receipt.configRevision) !=
        expected + (receipt.changed ? BigInt.one : BigInt.zero)) {
      _invalid();
    }
  }
}
