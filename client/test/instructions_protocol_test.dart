import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/cloud_routing.dart';
import 'package:smith_client/instructions.dart';

const instructionTestInstance = '0123456789abcdef0123456789abcdef';
const instructionTestRequest = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
const instructionTestRoute = ChatRoute('local', 'local:exact', null);

Map<String, dynamic> instructionFixture({
  Object revision = 1,
  InstructionSettings settings = const InstructionSettings(),
  ChatRoute? route,
}) => {
  'config_revision': revision,
  'settings': settings.toJson(),
  'renderer_version': 'smith-context-v1',
  'core': {'version': 'smith-core-v1', 'text': 'Smith safety'},
  'product': {'version': 'smith-product-v1', 'text': 'Smith product'},
  'vessel': route == null ? null : instructionVessel,
  'local': {
    'text':
        'Smith safety\nSmith product\n${settings.instructions}\n${settings.personality}\n${settings.guardrails}',
    'sha256': '0' * 64,
  },
  'cloud': {
    'text':
        'Smith safety\nSmith product\n${settings.instructions}\n${settings.personality}\n${settings.guardrails}',
    'sha256': '0' * 64,
    'bundle_sha256': '1' * 64,
    'personal_included': settings.hasPersonalText,
    'eligible': !settings.hasPersonalText || settings.cloudEnabled,
  },
  'limits': {'max_personal_bytes': 1024, 'max_preamble_bytes': 2048},
};

class InstructionExecutor implements AppExecutor {
  final calls = <Map<String, dynamic>>[];
  Map<String, dynamic> Function(Map<String, dynamic>)? result;
  @override
  Future<String> exchange(List<int> frame) async {
    final request = jsonDecode(utf8.decode(frame)) as Map<String, dynamic>;
    calls.add(request);
    return jsonEncode({
      'schema_version': 3,
      'service': 'smith-next',
      'protocol': 'app',
      'call_id': request['call_id'],
      'pid': 1,
      'instance_id': instructionTestInstance,
      'ok': true,
      'result': result?.call(request) ?? instructionFixture(),
      'error': null,
    });
  }

  @override
  void dispose() {}
}

void main() {
  test('settings preserve text and count combined UTF8, controls and invalid Unicode', () {
    const text = InstructionSettings(
      instructions: '  exact\n\ttext  ',
      personality: 'kind',
      guardrails: 'safe',
    );
    expect(InstructionSettings.parse(text.toJson()), text);
    expect(text.copyWith(cloudEnabled: true).cloudEnabled, true);
    expect(InstructionSettings(instructions: 'é' * 512).validationError, null);
    expect(
      InstructionSettings(
        instructions: 'é' * 512,
        guardrails: 'x',
      ).validationError,
      isNotNull,
    );
    for (final invalid in [
      '\r',
      '\u0000',
      '\u007f',
      '\u0085',
      String.fromCharCode(0xd800),
    ]) {
      expect(
        InstructionSettings(instructions: invalid).validationError,
        isNotNull,
      );
    }
  });

  test(
    'strict snapshot shape, eligibility, bounds, route, immutable fields',
    () {
      final value = instructionFixture(
        settings: const InstructionSettings(guardrails: 'private'),
      );
      final snapshot = InstructionSnapshot.parse(value);
      expect(snapshot.cloud.eligible, false);
      expect(snapshot.cloud.personalIncluded, true);
      expect(snapshot.local.bundleSha256, null);
      expect(
        () => snapshot.matchRoute(instructionTestRoute),
        throwsA(isA<AppFailure>()),
      );
      for (final mutate in <void Function(Map<String, dynamic>)>[
        (v) => v['extra'] = true,
        (v) => v.remove('vessel'),
        (v) => v['settings']['extra'] = '',
        (v) => v['cloud']['eligible'] = true,
        (v) => v['cloud']['personal_included'] = false,
        (v) => v['cloud']['text'] = 'omitted',
        (v) => v['limits']['max_preamble_bytes'] = 2049,
        (v) => v['local']['text'] = 'x' * 2049,
        (v) => v['vessel'] = 'chat::other::assistant',
        (v) => v['local']['sha256'] = 'G' * 64,
        (v) => v['renderer_version'] = 1,
      ]) {
        final copy = jsonDecode(jsonEncode(value)) as Map<String, dynamic>;
        mutate(copy);
        expect(
          () => InstructionSnapshot.parse(copy),
          throwsA(isA<AppFailure>()),
        );
      }
    },
  );

  test('full u64 revision is echoed and receipt increments exactly', () async {
    final executor = InstructionExecutor();
    final api = NativeAppApi(executor: executor);
    const large = '18446744073709551614';
    executor.result = (_) => {
      'request_id': instructionTestRequest,
      'config_revision': '18446744073709551615',
      'changed': true,
    };
    await api.call('instructions_update', {
      'request_id': instructionTestRequest,
      'instance_id': instructionTestInstance,
      'expected_config_revision': large,
      'settings': const InstructionSettings().toJson(),
    });
    expect(executor.calls.single['args']['expected_config_revision'], large);
    executor.result = (_) => {
      'request_id': instructionTestRequest,
      'config_revision': large,
      'changed': true,
    };
    await expectLater(
      api.call('instructions_update', {
        'request_id': instructionTestRequest,
        'instance_id': instructionTestInstance,
        'expected_config_revision': large,
        'settings': const InstructionSettings().toJson(),
      }),
      throwsA(isA<AppFailure>()),
    );
    api.dispose();
  });

  test(
    'Native API validates exact args before exchange and route after reply',
    () async {
      final executor = InstructionExecutor();
      final api = NativeAppApi(executor: executor);
      for (final args in <Map<String, dynamic>>[
        {},
        {'route': null, 'extra': true},
        {
          'route': {'provider': 'local'},
        },
      ]) {
        await expectLater(
          api.call('instructions_get', args),
          throwsA(isA<AppFailure>()),
        );
      }
      expect(executor.calls, isEmpty);
      await api.call('instructions_get', {'route': null});
      await expectLater(
        api.call('instructions_get', {'route': instructionTestRoute.toJson()}),
        throwsA(isA<AppFailure>()),
      );
      executor.result = (_) => {
        'request_id': 'b' * 32,
        'config_revision': 1,
        'changed': false,
      };
      await expectLater(
        api.call('instructions_receipt', {
          'request_id': instructionTestRequest,
        }),
        throwsA(isA<AppFailure>()),
      );
      api.dispose();
    },
  );

  test(
    'queued instruction arguments are captured deeply before caller mutation',
    () async {
      final executor = InstructionExecutor();
      final api = NativeAppApi(executor: executor);
      executor.result = (_) => {
        'request_id': instructionTestRequest,
        'config_revision': 2,
        'changed': true,
      };
      final settings = const InstructionSettings(instructions: 'reviewed')
          .toJson();
      final args = {
        'request_id': instructionTestRequest,
        'instance_id': instructionTestInstance,
        'expected_config_revision': 1,
        'settings': settings,
      };
      final call = api.call('instructions_update', args);
      settings['instructions'] = 'changed later';
      args['expected_config_revision'] = 9;
      await call;
      expect(
        executor.calls.single['args']['settings']['instructions'],
        'reviewed',
      );
      expect(executor.calls.single['args']['expected_config_revision'], 1);
      api.dispose();
    },
  );
}
