import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/cloud_routing.dart';

import 'app_fakes.dart';
import 'cloud_routing_test.dart'
    show cloud, catalog, token, generation, setProviderFacts;

class SchemaExecutor implements AppExecutor {
  SchemaExecutor(this.version, {this.value});
  final int version;
  final Map<String, dynamic>? value;
  final requests = <Map<String, dynamic>>[];
  @override
  Future<String> exchange(List<int> frame) async {
    final request = jsonDecode(utf8.decode(frame)) as Map<String, dynamic>;
    requests.add(request);
    return jsonEncode({
      'schema_version': version,
      'service': 'smith-next',
      'protocol': 'app',
      'pid': 1,
      'instance_id': testInstance,
      'call_id': request['call_id'],
      'ok': true,
      'result': value ?? catalog(),
      'error': null,
    });
  }

  @override
  void dispose() {}
}

void main() {
  test('production schema3 parses eligible advisory facts without another exchange', () async {
    final value = catalog();
    setProviderFacts(value, 'openai');
    setProviderFacts(
      value,
      'anthropic',
      state: 'missing',
      validationStatus: 'interrupted',
    );
    final executor = SchemaExecutor(3, value: value);
    final api = NativeAppApi(executor: executor);
    addTearDown(api.dispose);
    final result = RouteCatalog((await api.call('route_catalog', {})).result);
    expect(
      result.choices
          .where((c) => c.route.cloud)
          .every((c) => c.available && !c.validated),
      true,
    );
    expect(result.providers[0]['credential_state'], 'not_observed');
    expect(result.providers[1]['validation']['delivery'], 'may_have_sent');
    expect(executor.requests, hasLength(1));
    expect(executor.requests.single['op'], 'route_catalog');
  });

  for (final name in cloudModels.keys) {
    test(
      '$name: advisory catalog keeps all nested required nullable fields and exact keys',
      () {
        Map<String, dynamic> value() {
          final v = catalog();
          setProviderFacts(
            v,
            name,
            state: 'locked',
            validationStatus: 'failed',
          );
          return jsonDecode(jsonEncode(v)) as Map<String, dynamic>;
        }

        Map<String, dynamic> target(Map<String, dynamic> v, String part) {
          final p = (v['providers'] as List).firstWhere(
            (p) => p['provider'] == name,
          );
          final m = (v['models'] as List).firstWhere(
            (m) => m['route']['provider'] == name,
          );
          return switch (part) {
            'provider' => p,
            'credential_error' => p['credential_error'],
            'validation' => p['validation'],
            'validation_error' => p['validation']['error'],
            'model' => m,
            _ => m['route'],
          };
        }

        RouteCatalog(value());
        for (final part in [
          'provider',
          'credential_error',
          'validation',
          'validation_error',
          'model',
          'route',
        ]) {
          for (final key in target(value(), part).keys) {
            final v = value();
            target(v, part).remove(key);
            expect(
              () => RouteCatalog(v),
              throwsA(isA<AppFailure>()),
              reason: '$part omitted $key',
            );
          }
          final v = value();
          target(v, part)['extra'] = null;
          expect(
            () => RouteCatalog(v),
            throwsA(isA<AppFailure>()),
            reason: '$part extra field',
          );
        }
      },
    );
    test(
      '$name: advisory status never relaxes malformed identities/types/validation facts',
      () {
        for (final change
            in <void Function(Map<String, dynamic>, Map<String, dynamic>)>[
              (p, m) => p['credential_state'] = 'invented',
              (p, m) => p['credential_state'] = 'not_configured',
              (p, m) => p['credential_generation'] = 'invalid',
              (p, m) => p['revision'] = 9007199254740992,
              (p, m) => p['credential_error'] = null,
              (p, m) => p['credential_error']['code'] = 'bad-code',
              (p, m) => p['validation']['status'] = 'running',
              (p, m) => p['validation']['credential_generation'] = 'e' * 32,
              (p, m) => p['validation']['model'] =
                  cloudModels[name == 'openai' ? 'anthropic' : 'openai']!.first,
              (p, m) => p['validation']['error'] = null,
              (p, m) => p['validation']['delivery'] = 'unknown',
              (p, m) => p['validation']['completed_ms'] = -1,
              (p, m) => m['available'] = 'true',
              (p, m) => m['validated_model'] = 0,
              (p, m) => m['validated_model'] = true,
              (p, m) => m['unavailable_reason'] = {
                'code': 'KEYRING_LOCKED',
                'message': 'Advisory is not a disabling reason.',
              },
              (p, m) => m['route']['credential_generation'] = 'e' * 32,
              (p, m) => m['route']['model'] = 'unknown',
              (p, m) => m['group'] = 'local',
              (p, m) => m['label'] = 'line\nbreak',
            ]) {
          final v = catalog();
          setProviderFacts(
            v,
            name,
            state: 'locked',
            validationStatus: 'failed',
          );
          final p = (v['providers'] as List).firstWhere(
            (p) => p['provider'] == name,
          );
          final m = (v['models'] as List).firstWhere(
            (m) => m['route']['provider'] == name,
          );
          change(p, m);
          expect(() => RouteCatalog(v), throwsA(isA<AppFailure>()));
        }
      },
    );
  }
  test(
    'production emits schema3 and refuses schema2 without fallback',
    () async {
      for (final schema in [2, 3]) {
        final executor = SchemaExecutor(schema);
        final api = NativeAppApi(executor: executor);
        if (schema == 2) {
          await expectLater(
            api.call('route_catalog', {}),
            throwsA(isA<AppFailure>()),
          );
        } else {
          await api.call('route_catalog', {});
        }
        expect(executor.requests.length, 1);
        expect(executor.requests.single['schema_version'], 3);
        api.dispose();
      }
    },
  );
  test('routed arguments retain full u64 revision and forbid key or endpoint fields', () {
    final args = <String, dynamic>{
      'request_id': token,
      'instance_id': testInstance,
      'conversation_id': null,
      'text': 'hello',
      'route': cloud.toJson(),
      'expected_config_revision': '18446744073709551615',
      'expected_history_revision': null,
      'approval_token': generation,
      'confirm_transfer': true,
    };
    validateV3Arguments('submit_routed', args);
    for (final key in args.keys) {
      expect(
        () => validateV3Arguments('submit_routed', Map.of(args)..remove(key)),
        throwsA(anything),
      );
    }
    for (final key in ['api_key', 'endpoint', 'output']) {
      expect(
        () => validateV3Arguments('submit_routed', {...args, key: 'forbidden'}),
        throwsA(anything),
      );
    }
    expect(
      () => validateV3Arguments('submit_routed', {
        ...args,
        'route': {
          'provider': 'local',
          'model': 'local:exact',
          'credential_generation': null,
        },
      }),
      throwsA(anything),
    );
  });
  test('preparation and provenance reject missing nullable and inconsistent fields', () {
    final prep = <String, dynamic>{
      'request_id': token,
      'conversation_id': null,
      'route': cloud.toJson(),
      'config_revision': 1,
      'history_revision': null,
      'context_sha256': 'a' * 64,
      'context_messages': 1,
      'context_bytes': 5,
      'requires_consent': true,
      'approval_token': generation,
      'expires_at_ms': 60000,
      'dispatch_expires_at_ms': 600000,
    };
    RoutePreparation(prep);
    for (final key in prep.keys) {
      expect(
        () => RoutePreparation(Map.of(prep)..remove(key)),
        throwsA(anything),
      );
    }
    expect(
      () => RoutePreparation({...prep, 'context_messages': 2}),
      throwsA(anything),
    );
    expect(
      () => RoutePreparation({...prep, 'approval_token': null}),
      throwsA(anything),
    );
    final routing = <String, dynamic>{
      'request_id': token,
      'route': cloud.toJson(),
      'delivery': 'may_have_sent',
      'context_sha256': 'b' * 64,
      'dispatch_expires_at_ms': 600000,
    };
    RequestRouting(routing);
    for (final key in routing.keys) {
      expect(
        () => RequestRouting(Map.of(routing)..remove(key)),
        throwsA(anything),
      );
    }
    expect(
      () => RequestRouting({...routing, 'delivery': 'not_applicable'}),
      throwsA(anything),
    );
  });
}
