import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/cloud_routing.dart';

import 'cloud_routing_test.dart' show catalog, generation, setProviderFacts;

// Independent expected identities from LOCAL-SMITH-NEXT-CLOUD-CATALOG-EXPANSION.
const expectedCloudModels = {
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

void main() {
  test('exact ordered seven/eight catalog retains defaults and uniqueness', () {
    expect(cloudModels, expectedCloudModels);
    expect(cloudModels.keys, ['openai', 'anthropic']);
    expect(cloudModels['openai'], hasLength(7));
    expect(cloudModels['anthropic'], hasLength(8));
    for (final models in cloudModels.values) {
      expect(models.toSet(), hasLength(models.length));
      expect(models.length, lessThanOrEqualTo(8));
    }
    final parsed = RouteCatalog(catalog());
    for (final entry in expectedCloudModels.entries) {
      final choices = parsed.choices.where(
        (c) => c.route.provider == entry.key,
      );
      expect(choices.map((c) => c.route.model), entry.value);
      expect(choices.every((c) => c.available), true);
      expect(choices.where((c) => c.validated).map((c) => c.route.model), [
        entry.value.first,
      ]);
    }
  });

  for (final entry in expectedCloudModels.entries) {
    test(
      '${entry.key}: all exact IDs parse but wrong provider/aliases do not',
      () {
        for (final model in entry.value) {
          final route = ChatRoute(entry.key, model, generation);
          expect(ChatRoute.parse(route.toJson()), route);
          expect(
            ChatRoute.parse({
              ...route.toJson(),
              'credential_generation': null,
            }, catalog: true).model,
            model,
          );
          for (final bad in [
            {
              ...route.toJson(),
              'provider': entry.key == 'openai' ? 'anthropic' : 'openai',
            },
            {...route.toJson(), 'model': '$model-2099-01-01'},
            {...route.toJson(), 'model': model.toUpperCase()},
            {...route.toJson(), 'model': ' $model'},
            {...route.toJson(), 'credential_generation': null},
          ]) {
            expect(() => ChatRoute.parse(bad), throwsA(isA<AppFailure>()));
          }
        }
      },
    );
  }

  for (final entry in expectedCloudModels.entries) {
    for (final state in [
      'not_observed',
      'available',
      'locked',
      'missing',
      'unavailable',
      'conflict',
    ]) {
      for (final status in <String?>[
        null,
        'failed',
        'interrupted',
        'succeeded',
      ]) {
        test(
          '${entry.key}: saved $state / $status enables every exact choice without changing facts',
          () {
            final value = catalog();
            setProviderFacts(
              value,
              entry.key,
              state: state,
              validationStatus: status,
            );
            final parsed = RouteCatalog(value);
            final choices = parsed.choices.where(
              (c) => c.route.provider == entry.key,
            );
            expect(choices.map((c) => c.route.model), entry.value);
            expect(choices.every((c) => c.available && c.reason == null), true);
            expect(
              choices.where((c) => c.validated).map((c) => c.route.model),
              status == 'succeeded' ? [entry.value.first] : isEmpty,
            );
            final facts = parsed.providers.firstWhere(
              (p) => p['provider'] == entry.key,
            );
            expect(facts['credential_state'], state);
            expect(facts['validation']?['status'], status);
            expect(
              facts['credential_error'],
              (value['providers'] as List).firstWhere(
                (p) => p['provider'] == entry.key,
              )['credential_error'],
            );
            expect(
              parsed.choices
                  .where((c) => c.route.cloud && c.route.provider != entry.key)
                  .every((c) => c.available),
              true,
            );

            final model = (value['models'] as List).firstWhere(
              (m) => m['route']['provider'] == entry.key,
            );
            model['available'] = false;
            model['unavailable_reason'] = {
              'code': 'ROUTE_UNAVAILABLE',
              'message': 'Obsolete cached gate.',
            };
            expect(() => RouteCatalog(value), throwsA(isA<AppFailure>()));
          },
        );
      }
    }
    test(
      '${entry.key}: no generation disables choices and cannot borrow another provider validation',
      () {
        final value = catalog();
        setProviderFacts(value, entry.key, state: 'not_configured');
        final parsed = RouteCatalog(value);
        expect(
          parsed.choices
              .where((c) => c.route.provider == entry.key)
              .every(
                (c) =>
                    !c.available &&
                    !c.validated &&
                    c.route.generation == null &&
                    c.reason != null,
              ),
          true,
        );
        expect(
          parsed.choices
              .where((c) => c.route.cloud && c.route.provider != entry.key)
              .every((c) => c.available),
          true,
        );
        final model = (value['models'] as List).firstWhere(
          (m) => m['route']['provider'] == entry.key,
        );
        model['available'] = true;
        model['unavailable_reason'] = null;
        expect(() => RouteCatalog(value), throwsA(isA<AppFailure>()));
      },
    );
  }

  test(
    'catalog generation, order, duplicates and unknown IDs remain strict',
    () {
      for (final change in <void Function(Map<String, dynamic>)>[
        (v) => v['models'][1]['route']['credential_generation'] =
            'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',
        (v) => v['models'][1]['route']['model'] = 'not-a-compiled-model',
        (v) => v['models'].add(v['models'][1]),
        (v) => v['models'].insert(0, v['models'].removeLast()),
        (v) => v['models'][2]['validated_model'] = true,
      ]) {
        final value = catalog();
        change(value);
        expect(() => RouteCatalog(value), throwsA(isA<AppFailure>()));
      }
    },
  );

  test('expanded choices fit unchanged 256-local/272-total catalog bounds', () {
    final value = catalog();
    final local = Map<String, dynamic>.from(value['models'][0]);
    value['models'] = [
      for (var i = 0; i < 256; i++)
        {
          ...local,
          'route': {
            'provider': 'local',
            'model': 'installed:$i',
            'credential_generation': null,
          },
        },
      ...value['models'].skip(1),
    ];
    expect(RouteCatalog(value).choices, hasLength(271));
    value['models'].insert(0, {
      ...local,
      'route': {
        'provider': 'local',
        'model': 'installed:extra',
        'credential_generation': null,
      },
    });
    expect(() => RouteCatalog(value), throwsA(isA<AppFailure>()));
    value['models'].add(local);
    expect(value['models'], hasLength(273));
    expect(() => RouteCatalog(value), throwsA(isA<AppFailure>()));
  });
}
