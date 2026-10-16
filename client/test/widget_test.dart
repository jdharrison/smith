import 'dart:async';
import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/control_api.dart';
import 'package:smith_client/native_control.dart';

Map<String, dynamic> profile({
  String binary = 'installed',
  String unit = 'installed',
  String runtime = 'running',
  String bridge = 'ready',
  String enablement = 'enabled',
  bool? setup = true,
  bool ok = true,
}) => {
  'schema_version': 2,
  'service': 'smith-next',
  'ok': ok,
  'binary_state': binary,
  'unit_state': unit,
  'runtime_state': runtime,
  'bridge_state': bridge,
  'enablement_state': enablement,
  'setup_complete': setup,
  'message': 'Observed service state.',
  'error': ok ? null : 'Manager unavailable',
  'effects': {
    'binary_path': '/test/bin/smith-service',
    'unit_path': '/test/unit',
    'state_path': '/test/state',
    'socket_path': '/test/socket',
    'description': 'Exact proposed effect',
  },
};
Map<String, dynamic> preflightError() => {
  ...profile(
    binary: 'unknown',
    unit: 'unknown',
    runtime: 'unknown',
    bridge: 'unknown',
    setup: null,
    ok: false,
  ),
  'message': 'Root preflight failed.',
  'error': 'XDG_RUNTIME_DIR must be an absolute private directory',
  'effects': {
    'binary_path': '',
    'unit_path': '',
    'state_path': '',
    'socket_path': '',
    'description': 'No effects; root discovery failed.',
  },
};

ControlReply reply(Map<String, dynamic> data, {bool prepare = false}) =>
    ControlReply.parse(jsonEncode(data), prepare: prepare);

class FakeControlApi implements ControlApi {
  FakeControlApi(this.state);
  Map<String, dynamic> state;
  final calls = <String>[];
  Completer<ControlReply>? pendingInspect;
  Completer<ControlReply>? pendingPrepare;
  Completer<ControlReply>? pendingApply;
  Object? inspectFailure;
  Object? applyFailure;
  bool prepareOk = true;
  bool disposed = false;
  @override
  Future<ControlReply> inspect({Duration? deadline}) async {
    calls.add('inspect');
    if (inspectFailure != null) throw inspectFailure!;
    return pendingInspect == null ? reply(state) : pendingInspect!.future;
  }

  @override
  Future<ControlReply> prepare(String action) async {
    calls.add('prepare:$action');
    return pendingPrepare == null
        ? reply({
            ...state,
            'ok': prepareOk,
            'error': prepareOk ? null : 'Prepare failed',
            'approval_token': 'bound-token',
          }, prepare: true)
        : pendingPrepare!.future;
  }

  @override
  Future<ControlReply> apply(String action, String approvalToken) async {
    calls.add('apply:$action:$approvalToken');
    if (applyFailure != null) throw applyFailure!;
    return pendingApply == null ? reply(state) : pendingApply!.future;
  }

  @override
  void dispose() {
    disposed = true;
  }
}

class FakeExecutor implements HelperExecutor {
  final calls = <List<String>>[];
  final deadlines = <Duration?>[];
  @override
  Future<String> execute(List<String> arguments, {Duration? deadline}) async {
    calls.add(arguments);
    deadlines.add(deadline);
    return jsonEncode({...profile(), 'approval_token': 'token'});
  }

  @override
  void dispose() {}
}

void main() {
  group('strict helper contract', () {
    test('accepts exact schema, nullable facts and structured failure', () {
      expect(reply(profile()).ready, isTrue);
      expect(reply(profile(setup: null, ok: false)).ready, isFalse);
    });
    test('empty effect paths preserve factual failed replies only', () {
      final data = preflightError();
      expect(reply(data).error, data['error']);
      expect(reply(data, prepare: true).approvalToken, isNull);
      for (final key in [
        'binary_path',
        'unit_path',
        'state_path',
        'socket_path',
      ]) {
        final good = profile();
        (good['effects'] as Map)[key] = '';
        expect(() => reply(good), throwsA(isA<ControlFailure>()));
        expect(
          () => reply({...good, 'approval_token': 'token'}, prepare: true),
          throwsA(isA<ControlFailure>()),
        );
        final bad = preflightError();
        (bad['effects'] as Map)[key] = 'relative';
        expect(() => reply(bad), throwsA(isA<ControlFailure>()));
      }
      expect(
        () => reply({...data, 'service': 'wrong'}),
        throwsA(isA<ControlFailure>()),
      );
    });
    final invalid = <String, dynamic>{
      'service': 'smith',
      'schema_version': 1,
      'ok': 'true',
      'binary_state': 'ready',
      'unit_state': false,
      'runtime_state': 'active',
      'bridge_state': 'healthy',
      'enablement_state': 'automatic',
      'setup_complete': 1,
      'message': null,
      'error': 5,
      'effects': {},
    };
    for (final entry in invalid.entries) {
      test('rejects malformed ${entry.key}', () {
        expect(
          () => reply({...profile(), entry.key: entry.value}),
          throwsA(isA<ControlFailure>()),
        );
      });
    }
    test('rejects freeform, multiple replies, missing and unknown fields', () {
      for (final text in [
        'running',
        '${jsonEncode(profile())}\n${jsonEncode(profile())}',
        '{}',
      ]) {
        expect(() => ControlReply.parse(text), throwsA(isA<ControlFailure>()));
      }
      final missing = profile()..remove('setup_complete');
      expect(() => reply(missing), throwsA(isA<ControlFailure>()));
      expect(
        () => reply({...profile(), 'unexpected': true}),
        throwsA(isA<ControlFailure>()),
      );
      final unknownEffect = profile();
      unknownEffect['effects'] = Map<String, dynamic>.from(
        unknownEffect['effects'] as Map,
      )..['unexpected'] = true;
      expect(() => reply(unknownEffect), throwsA(isA<ControlFailure>()));
    });
    test('chat readiness requires exact persistent login enablement', () {
      expect(reply(profile()).chatReady, isTrue);
      for (final enablement in ['missing', 'disabled', 'conflict', 'unknown']) {
        expect(reply(profile(enablement: enablement)).chatReady, isFalse);
      }
    });
    test('requires absolute effects and nonempty preparation token', () {
      final data = profile();
      (data['effects'] as Map)['state_path'] = 'relative';
      expect(() => reply(data), throwsA(isA<ControlFailure>()));
      expect(
        () => reply(profile(), prepare: true),
        throwsA(isA<ControlFailure>()),
      );
    });
    test(
      'native adapter uses exact argument lists and action-bound token',
      () async {
        final executor = FakeExecutor();
        final api = NativeControlApi(executor: executor);
        await api.inspect();
        await api.prepare('enable');
        await api.prepare('start');
        await api.apply('start', 'token');
        expect(executor.calls, [
          ['inspect'],
          ['prepare', '--action', 'enable'],
          ['prepare', '--action', 'start'],
          ['apply', '--action', 'start', '--approval', 'token'],
        ]);
        await expectLater(api.prepare('serve'), throwsA(isA<ControlFailure>()));
        await expectLater(
          api.apply('start', ''),
          throwsA(isA<ControlFailure>()),
        );
      },
    );
    test(
      'native adapter keeps Boot inspection separate from 45s actions',
      () async {
        final actions = FakeExecutor();
        final inspections = FakeExecutor();
        final api = NativeControlApi(
          executor: actions,
          inspectExecutor: inspections,
        );
        await api.inspect(deadline: const Duration(seconds: 2));
        await api.prepare('start');
        expect(inspections.calls, [
          ['inspect'],
        ]);
        expect(inspections.deadlines, [const Duration(seconds: 2)]);
        expect(actions.calls, [
          ['prepare', '--action', 'start'],
        ]);
        expect(actions.deadlines.single, isNull);
        api.dispose();
      },
    );
    test(
      'helper discovery is executable-relative or explicit absolute only',
      () {
        expect(
          helperPath('/bundle/smith', ''),
          '/bundle/smith_backend/smith-service',
        );
        expect(
          helperPath('/bundle/smith', '/dev/smith-service'),
          '/dev/smith-service',
        );
        expect(
          () => helperPath('/bundle/smith', 'smith-service'),
          throwsA(isA<ControlFailure>()),
        );
        expect(helperDeadline, const Duration(seconds: 45));
        expect(helperOutputLimit, 16384);
      },
    );
  });
}
