import 'app_v2.dart' as v2;
import 'message_timestamps.dart';

const cloudModels = {
  'openai': [
    'gpt-6-astra',
    'gpt-6-sol',
    'gpt-6.1-sol',
    'gpt-6-luna',
    'gpt-5.6-sol',
    'gpt-5.6-terra',
    'gpt-5.6-luna',
  ],
  'anthropic': [
    'claude-sonnet-5-5',
    'claude-haiku-5-5',
    'claude-opus-5-5',
    'claude-fable-5-1',
    'claude-opus-5',
    'claude-opus-4-8',
    'claude-opus-4-7',
    'claude-sonnet-5',
  ],
};
bool _hash(dynamic v) => v is String && RegExp(r'^[0-9a-f]{64}$').hasMatch(v);
void _label(dynamic v) {
  v2.bounded(v, 256);
  if (RegExp(r'[\x00-\x1f\x7f-\x9f]').hasMatch(v)) v2.invalid();
}

class ChatRoute {
  const ChatRoute(this.provider, this.model, this.generation);
  final String provider, model;
  final String? generation;
  bool get cloud => provider != 'local';
  String get label => '${provider.toUpperCase()} · $model';
  factory ChatRoute.parse(dynamic value, {bool catalog = false}) {
    v2.exact(value, ['provider', 'model', 'credential_generation']);
    _label(value['model']);
    final provider = value['provider'];
    final generation = value['credential_generation'];
    if (provider == 'local') {
      if (generation != null) v2.invalid();
    } else if (!cloudModels.containsKey(provider) ||
        !cloudModels[provider]!.contains(value['model']) ||
        !(v2.wireId(generation) || (catalog && generation == null))) {
      v2.invalid();
    }
    return ChatRoute(provider, value['model'], generation);
  }
  Map<String, dynamic> toJson() => {
    'provider': provider,
    'model': model,
    'credential_generation': generation,
  };
  @override
  bool operator ==(Object other) =>
      other is ChatRoute &&
      provider == other.provider &&
      model == other.model &&
      generation == other.generation;
  @override
  int get hashCode => Object.hash(provider, model, generation);
}

void _destination(dynamic d) {
  v2.exact(d, ['provider', 'credential_generation']);
  if (!cloudModels.containsKey(d['provider']) ||
      !v2.wireId(d['credential_generation'])) {
    v2.invalid();
  }
}

void _validation(dynamic v) {
  v2.exact(v, [
    'request_id',
    'model',
    'credential_generation',
    'status',
    'completed_ms',
    'error',
    'delivery',
  ]);
  if (!v2.wireId(v['request_id']) ||
      !v2.wireId(v['credential_generation']) ||
      !v2.wireRevision(v['completed_ms']) ||
      !['succeeded', 'failed', 'interrupted'].contains(v['status']) ||
      !['not_sent', 'may_have_sent', 'received'].contains(v['delivery'])) {
    v2.invalid();
  }
  _label(v['model']);
  if (v['status'] == 'succeeded') {
    if (v['error'] != null || v['delivery'] != 'received') v2.invalid();
  } else {
    v2.wireError(v['error']);
  }
}

void providerView(dynamic p) {
  v2.exact(p, [
    'provider',
    'revision',
    'credential_generation',
    'credential_state',
    'credential_error',
    'validation',
  ]);
  if (!cloudModels.containsKey(p['provider']) ||
      !v2.wireRevision(p['revision']) ||
      (p['credential_generation'] != null &&
          !v2.wireId(p['credential_generation'])) ||
      ![
        'not_configured',
        'not_observed',
        'available',
        'locked',
        'unavailable',
        'missing',
        'conflict',
      ].contains(p['credential_state'])) {
    v2.invalid();
  }
  if ((p['credential_state'] == 'not_configured') !=
      (p['credential_generation'] == null)) {
    v2.invalid();
  }
  if (['not_configured', 'available'].contains(p['credential_state'])) {
    if (p['credential_error'] != null) v2.invalid();
  } else {
    v2.wireError(p['credential_error']);
  }
  if (p['validation'] != null) {
    _validation(p['validation']);
    if (p['validation']['credential_generation'] !=
            p['credential_generation'] ||
        !cloudModels[p['provider']]!.contains(p['validation']['model'])) {
      v2.invalid();
    }
  }
}

class RouteChoice {
  RouteChoice(dynamic v) {
    v2.exact(v, [
      'group',
      'label',
      'route',
      'available',
      'unavailable_reason',
      'validated_model',
    ]);
    route = ChatRoute.parse(v['route'], catalog: true);
    _label(v['label']);
    label = v['label'];
    if (v['group'] != (route.cloud ? 'cloud' : 'local') ||
        v['available'] is! bool ||
        v['validated_model'] is! bool ||
        v['available'] != (v['unavailable_reason'] == null)) {
      v2.invalid();
    }
    available = v['available'];
    validated = v['validated_model'];
    if ((!route.cloud && validated) ||
        (available && route.cloud && route.generation == null)) {
      v2.invalid();
    }
    if (v['unavailable_reason'] != null) v2.wireError(v['unavailable_reason']);
    reason = v['unavailable_reason']?['message'];
  }
  late final ChatRoute route;
  late final String label;
  late final bool available, validated;
  late final String? reason;
}

class RouteCatalog {
  RouteCatalog(Map<String, dynamic> v) {
    v2.exact(v, [
      'config_revision',
      'inventory_observed',
      'providers',
      'models',
    ]);
    if (!v2.wireConfigRevision(v['config_revision']) ||
        v['inventory_observed'] is! bool ||
        v['providers'] is! List ||
        v['providers'].length != 2 ||
        v['models'] is! List ||
        v['models'].length > 272) {
      v2.invalid();
    }
    configRevision = v['config_revision'];
    providers = List<Map<String, dynamic>>.from(v['providers']);
    for (var i = 0; i < 2; i++) {
      providerView(providers[i]);
      if (providers[i]['provider'] != cloudModels.keys.elementAt(i)) {
        v2.invalid();
      }
    }
    choices = (v['models'] as List).map(RouteChoice.new).toList();
    if (choices.where((c) => !c.route.cloud).length > 256 ||
        (v['inventory_observed'] == false &&
            choices.any((c) => !c.route.cloud && c.available))) {
      v2.invalid();
    }
    final seen = <String>{};
    var previous = 0;
    for (final c in choices) {
      final order = ['local', 'openai', 'anthropic'].indexOf(c.route.provider);
      if (order < previous ||
          !seen.add('${c.route.provider}/${c.route.model}')) {
        v2.invalid();
      }
      previous = order;
      if (c.route.cloud) {
        final p = providers.firstWhere(
          (p) => p['provider'] == c.route.provider,
        );
        if (p['credential_generation'] != c.route.generation ||
            c.available != (p['credential_generation'] != null) ||
            c.validated !=
                (p['validation']?['status'] == 'succeeded' &&
                    p['validation']?['model'] == c.route.model)) {
          v2.invalid();
        }
      }
    }
  }
  late final Object configRevision;
  late final List<Map<String, dynamic>> providers;
  late final List<RouteChoice> choices;
  bool available(ChatRoute? route) =>
      choices.any((c) => c.route == route && c.available);
}

class RoutePreparation {
  RoutePreparation(this.value) {
    final v = value;
    v2.exact(v, [
      'request_id',
      'conversation_id',
      'route',
      'config_revision',
      'history_revision',
      'context_sha256',
      'context_messages',
      'context_bytes',
      'requires_consent',
      'approval_token',
      'expires_at_ms',
      'dispatch_expires_at_ms',
    ]);
    route = ChatRoute.parse(v['route']);
    if (!v2.wireId(v['request_id']) ||
        (v['conversation_id'] != null && !v2.wireId(v['conversation_id'])) ||
        !v2.wireConfigRevision(v['config_revision']) ||
        (v['conversation_id'] == null
            ? v['history_revision'] != null
            : !v2.wireRevision(v['history_revision'])) ||
        !_hash(v['context_sha256']) ||
        !v2.wireRevision(v['context_messages']) ||
        v['context_messages'] < 1 ||
        v['context_messages'] > 33 ||
        v['context_messages'].isEven ||
        !v2.wireRevision(v['context_bytes']) ||
        v['context_bytes'] > 81920 ||
        v['requires_consent'] is! bool) {
      v2.invalid();
    }
    if (route.cloud) {
      if (!v2.wireId(v['approval_token']) ||
          !v2.wireRevision(v['expires_at_ms']) ||
          !v2.wireRevision(v['dispatch_expires_at_ms']) ||
          v['dispatch_expires_at_ms'] < v['expires_at_ms']) {
        v2.invalid();
      }
    } else if (v['requires_consent'] ||
        v['approval_token'] != null ||
        v['expires_at_ms'] != null ||
        v['dispatch_expires_at_ms'] != null) {
      v2.invalid();
    }
  }
  final Map<String, dynamic> value;
  late final ChatRoute route;
  bool get requiresConsent => value['requires_consent'];
  void match(Map<String, dynamic> args) {
    for (final k in ['request_id', 'conversation_id']) {
      if (value[k] != args[k]) v2.invalid();
    }
    if (route != ChatRoute.parse(args['route']) ||
        value['config_revision'] != args['expected_config_revision'] ||
        value['history_revision'] != args['expected_history_revision']) {
      v2.invalid();
    }
  }

  String get disclosure =>
      'Send to ${route.label}?\n\nThe proposed message and retained conversation context (${value['context_messages']} messages, ${value['context_bytes']} UTF-8 bytes) will be transferred to ${route.provider}. This destination is credential generation ${route.generation}, not a verified account identity. Provider processing and retention apply; charges may occur. Local deletion cannot delete provider records.\n\nApproval is retained for later explicit Sends in this chat to this provider/generation, including then-retained context and intervening turns. Changing models at the same approved destination will not ask again. No other destination is approved.';
}

enum TransferDecision { send, cancel, newChat }

typedef ConfirmTransfer = Future<TransferDecision> Function(
  RoutePreparation preparation,
);

class ConversationRouting {
  ConversationRouting(Map<String, dynamic> v) {
    v2.exact(v, [
      'conversation_id',
      'history_revision',
      'last_route',
      'approved_destinations',
      'turn_routes',
    ]);
    if (!v2.wireId(v['conversation_id']) ||
        !v2.wireRevision(v['history_revision']) ||
        v['approved_destinations'] is! List ||
        v['approved_destinations'].length > 16 ||
        v['turn_routes'] is! List ||
        v['turn_routes'].length > 16) {
      v2.invalid();
    }
    id = v['conversation_id'];
    revision = v['history_revision'];
    lastRoute = v['last_route'] == null
        ? null
        : ChatRoute.parse(v['last_route']);
    final seen = <String>{};
    for (final d in v['approved_destinations']) {
      _destination(d);
      if (!seen.add('${d['provider']}/${d['credential_generation']}')) {
        v2.invalid();
      }
    }
    turns = [];
    for (final t in v['turn_routes']) {
      v2.exact(t, ['request_id', 'route']);
      if (t['request_id'] == null) {
        if (t['route'] != null) v2.invalid();
        turns.add(null);
      } else {
        if (!v2.wireId(t['request_id'])) v2.invalid();
        turns.add(ChatRoute.parse(t['route']));
      }
    }
  }
  late final String id;
  late final int revision;
  late final ChatRoute? lastRoute;
  late final List<ChatRoute?> turns;
}

class RequestRouting {
  RequestRouting(Map<String, dynamic> v) {
    v2.exact(v, [
      'request_id',
      'route',
      'delivery',
      'context_sha256',
      'dispatch_expires_at_ms',
    ]);
    if (!v2.wireId(v['request_id']) ||
        ![
          'not_applicable',
          'not_sent',
          'may_have_sent',
          'received',
        ].contains(v['delivery']) ||
        (v['context_sha256'] != null && !_hash(v['context_sha256'])) ||
        (v['dispatch_expires_at_ms'] != null &&
            !v2.wireRevision(v['dispatch_expires_at_ms']))) {
      v2.invalid();
    }
    request = v['request_id'];
    delivery = v['delivery'];
    route = v['route'] == null ? null : ChatRoute.parse(v['route']);
    if (route?.cloud == true) {
      if (delivery == 'not_applicable' || v['dispatch_expires_at_ms'] == null) {
        v2.invalid();
      }
    } else if (delivery != 'not_applicable' ||
        v['dispatch_expires_at_ms'] != null ||
        (route == null && v['context_sha256'] != null)) {
      v2.invalid();
    }
  }
  late final String request, delivery;
  late final ChatRoute? route;
  String get label =>
      '${route?.label ?? 'Legacy route unknown'} · ${delivery == 'may_have_sent' ? 'Remote processing / billing may have occurred; do not resend automatically' : delivery}';
}

bool chatOperation(dynamic kind) => [
  'submit',
  'submit_new',
  'submit_routed',
  'submit_new_routed',
].contains(kind);
void operationV3(dynamic value, {bool terminal = false}) {
  if (value is! Map<String, dynamic>) v2.invalid();
  final kind = value['kind'];
  if (kind == 'submit_routed' || kind == 'submit_new_routed') {
    final projected = Map<String, dynamic>.from(value)..['kind'] = 'submit_new';
    v2.operation(projected, terminal: terminal);
    return;
  }
  if (['provider_probe', 'provider_save', 'provider_validate'].contains(kind)) {
    v2.exact(value, [
      'request_id',
      'conversation_id',
      'kind',
      'status',
      'phase',
      'config_revision',
      if (terminal) 'result',
      if (terminal) 'error',
    ]);
    if (!v2.wireId(value['request_id']) ||
        value['conversation_id'] != null ||
        !v2.wireConfigRevision(value['config_revision']) ||
        ![
          'running',
          'succeeded',
          'failed',
          'interrupted',
        ].contains(value['status']) ||
        value['phase'] !=
            (value['status'] == 'running'
                ? (kind == 'provider_probe'
                      ? 'inventory'
                      : kind == 'provider_save'
                      ? 'credential'
                      : 'verification')
                : 'complete')) {
      v2.invalid();
    }
    if (terminal) {
      if (value['status'] == 'running') {
        if (value['result'] != null || value['error'] != null) v2.invalid();
      } else if (value['status'] == 'succeeded') {
        if (value['error'] != null) v2.invalid();
        v2.exact(value['result'], [
          'provider',
          if (kind == 'provider_validate') ...['route', 'assistant_text'],
        ]);
        providerView(value['result']['provider']);
        if (kind == 'provider_save') {
          final p = value['result']['provider'];
          if (p['credential_state'] != 'available' ||
              p['credential_generation'] == null ||
              p['validation'] != null) {
            v2.invalid();
          }
        }
        if (kind == 'provider_validate') {
          final route = ChatRoute.parse(value['result']['route']);
          final p = value['result']['provider'];
          v2.bounded(value['result']['assistant_text'], 65536);
          if (!route.cloud ||
              route.provider != p['provider'] ||
              route.generation != p['credential_generation'] ||
              p['validation']?['request_id'] != value['request_id'] ||
              p['validation']?['status'] != 'succeeded' ||
              p['validation']?['model'] != route.model) {
            v2.invalid();
          }
        }
      } else {
        if (value['result'] != null) v2.invalid();
        v2.wireError(value['error']);
      }
    }
    return;
  }
  v2.operation(value, terminal: terminal);
}

void validateV3(String op, Map<String, dynamic> r) {
  switch (op) {
    case 'conversation_timestamps':
      ConversationTimestamps(r);
    case 'route_catalog':
      RouteCatalog(r);
    case 'conversation_routing':
      ConversationRouting(r);
    case 'request_routing':
      RequestRouting(r);
    case 'route_prepare':
      RoutePreparation(r);
    case 'submit_routed':
    case 'provider_probe':
      operationV3(r);
    case 'poll':
      operationV3(r, terminal: true);
    case 'state':
      if (r['operations'] is! List) v2.invalid();
      final jobs = r['operations'] as List;
      if (jobs.length > 32) v2.invalid();
      final ids = <String>{};
      for (final o in jobs) {
        operationV3(o);
        if (!ids.add(o['request_id'])) v2.invalid();
      }
      final projected = Map<String, dynamic>.from(r)
        ..['operations'] = jobs
            .where(
              (o) => ![
                'provider_probe',
                'provider_save',
                'provider_validate',
              ].contains(o['kind']),
            )
            .map(
              (o) => Map<String, dynamic>.from(o)
                ..['kind'] =
                    ['submit_routed', 'submit_new_routed'].contains(o['kind'])
                    ? (o['kind'] == 'submit_new_routed'
                          ? 'submit_new'
                          : 'submit')
                    : o['kind'],
            )
            .toList();
      v2.validateV2(op, projected);
    default:
      v2.validateV2(op, r);
  }
}

void validateV3Arguments(String op, Map<String, dynamic> a) {
  if (op == 'route_catalog') {
    v2.exact(a, []);
    return;
  }
  if (op == 'conversation_routing' || op == 'conversation_timestamps') {
    v2.exact(a, ['conversation_id']);
    if (!v2.wireId(a['conversation_id'])) v2.invalid();
    return;
  }
  if (op == 'request_routing' || op == 'provider_probe') {
    v2.exact(a, [
      'request_id',
      'instance_id',
      if (op == 'provider_probe') ...['provider', 'expected_provider_revision'],
    ]);
    if (!v2.wireId(a['request_id']) || !v2.wireId(a['instance_id'])) {
      v2.invalid();
    }
    if (op == 'provider_probe' &&
        (!cloudModels.containsKey(a['provider']) ||
            !v2.wireRevision(a['expected_provider_revision']))) {
      v2.invalid();
    }
    return;
  }
  if (op == 'route_prepare' || op == 'submit_routed') {
    v2.exact(a, [
      'request_id',
      'instance_id',
      'conversation_id',
      'text',
      'route',
      'expected_config_revision',
      'expected_history_revision',
      if (op == 'submit_routed') ...['approval_token', 'confirm_transfer'],
    ]);
    if (!v2.wireId(a['request_id']) ||
        !v2.wireId(a['instance_id']) ||
        !v2.wireConfigRevision(a['expected_config_revision']) ||
        (a['conversation_id'] == null
            ? a['expected_history_revision'] != null
            : !v2.wireId(a['conversation_id']) ||
                  !v2.wireRevision(a['expected_history_revision']))) {
      v2.invalid();
    }
    v2.bounded(a['text'], 16384);
    final route = ChatRoute.parse(a['route']);
    if (op == 'submit_routed' &&
        (a['confirm_transfer'] is! bool ||
            (route.cloud
                ? !v2.wireId(a['approval_token'])
                : a['approval_token'] != null ||
                      a['confirm_transfer'] != false))) {
      v2.invalid();
    }
    return;
  }
  v2.validateV2Arguments(op, a);
}
