import 'app_fakes.dart';

import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/boot_controller.dart';
import 'package:smith_client/control_api.dart';
import 'package:smith_client/main.dart';
import 'package:smith_client/native_control.dart';

Map<String, dynamic> profile({
  String binary = 'installed',
  String unit = 'installed',
  String runtime = 'running',
  String bridge = 'ready',
  bool? setup = true,
  bool ok = true,
}) => {
  'schema_version': 1,
  'service': 'smith-next',
  'ok': ok,
  'binary_state': binary,
  'unit_state': unit,
  'runtime_state': runtime,
  'bridge_state': bridge,
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
  Future<ControlReply> inspect() async {
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
  @override
  Future<String> execute(List<String> arguments) async {
    calls.add(arguments);
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
      'schema_version': 2,
      'ok': 'true',
      'binary_state': 'ready',
      'unit_state': false,
      'runtime_state': 'active',
      'bridge_state': 'healthy',
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
    test('rejects freeform, multiple JSON replies and missing fields', () {
      for (final text in [
        'running',
        '${jsonEncode(profile())}\n${jsonEncode(profile())}',
        '{}',
      ]) {
        expect(() => ControlReply.parse(text), throwsA(isA<ControlFailure>()));
      }
      final missing = profile()..remove('setup_complete');
      expect(() => reply(missing), throwsA(isA<ControlFailure>()));
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
        await api.prepare('start');
        await api.apply('start', 'token');
        expect(executor.calls, [
          ['inspect'],
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

  group('controller', () {
    final cases = <String, Map<String, dynamic>>{
      'missing binary': profile(binary: 'missing', setup: null),
      'missing unit': profile(unit: 'missing', setup: null),
      'stopped': profile(
        runtime: 'stopped',
        bridge: 'unavailable',
        setup: false,
      ),
      'unverified': profile(bridge: 'unknown', setup: null),
      'incomplete': profile(setup: false),
      'unknown setup': profile(setup: null),
      'failed helper': profile(ok: false, setup: null),
      'conflict': profile(bridge: 'conflict'),
    };
    for (final entry in cases.entries) {
      test('${entry.key} routes to setup with reads only', () async {
        final api = FakeControlApi(entry.value);
        final controller = BootController(api);
        expect(controller.page, BootPage.loader);
        await controller.refresh();
        expect(controller.page, BootPage.setup);
        expect(api.calls, ['inspect']);
        controller.dispose();
      });
    }
    test(
      'ready completion returns Home on reopen; outage preserves session only',
      () async {
        final api = FakeControlApi(profile());
        final controller = BootController(api);
        await controller.refresh();
        expect(controller.page, BootPage.home);
        api.state = profile(bridge: 'unavailable', setup: null, ok: false);
        await controller.refresh();
        expect(controller.page, BootPage.home);
        expect(controller.observation!.ready, isFalse);
        final fresh = BootController(api);
        await fresh.refresh();
        expect(fresh.page, BootPage.setup);
        controller.dispose();
        fresh.dispose();
      },
    );
    test('preflight error is specific, nonactionable and visible', () async {
      final api = FakeControlApi(preflightError());
      final controller = BootController(api);
      await controller.refresh();
      expect(controller.page, BootPage.setup);
      expect(controller.error, api.state['error']);
      expect(controller.observation!.message, 'Root preflight failed.');
      expect(controller.actions, isEmpty);
      controller.dispose();
    });
    test(
      'failed preparation with empty paths never confirms or applies',
      () async {
        final api = FakeControlApi(profile(setup: false));
        final controller = BootController(api);
        await controller.refresh();
        api.pendingPrepare = Completer<ControlReply>()
          ..complete(reply(preflightError(), prepare: true));
        await controller.act(
          'complete-setup',
          (_, _) async => fail('Failed proposal cannot confirm'),
        );
        expect(controller.error, preflightError()['error']);
        expect(api.calls, ['inspect', 'prepare:complete-setup']);
        controller.dispose();
      },
    );
    for (final runtime in ['stopped', 'unavailable']) {
      test(
        'returning persisted setup enters Home repair with $runtime service',
        () async {
          final api = FakeControlApi(
            profile(runtime: runtime, bridge: 'unavailable'),
          );
          final controller = BootController(api);
          await controller.refresh();
          expect(controller.page, BootPage.home);
          expect(controller.observation!.ready, isFalse);
          expect(api.calls, ['inspect']);
          expect(
            controller.actions,
            runtime == 'stopped' ? ['start', 'install-unit'] : isEmpty,
          );
          controller.dispose();
        },
      );
    }
    test('failed control result retains safe persisted completion without readiness', () async {
      final api = FakeControlApi(
        profile(runtime: 'unavailable', bridge: 'unavailable', ok: false),
      );
      final controller = BootController(api);
      await controller.refresh();
      expect(controller.page, BootPage.home);
      expect(controller.observation!.ready, isFalse);
      expect(controller.error, 'Manager unavailable');
      expect(controller.actions, isEmpty);
      controller.dispose();
    });
    test(
      'installed stopped unit permits separately confirmed Register retry',
      () async {
        final api = FakeControlApi(
          profile(runtime: 'stopped', bridge: 'unavailable', setup: false),
        );
        final controller = BootController(api);
        await controller.refresh();
        expect(controller.actions, ['start', 'install-unit']);
        await controller.act('install-unit', (_, proposal) async => true);
        expect(api.calls, [
          'inspect',
          'prepare:install-unit',
          'apply:install-unit:bound-token',
        ]);
        api.prepareOk = false;
        await controller.act(
          'install-unit',
          (_, _) async => fail('Fresh preparation denied retry'),
        );
        expect(controller.error, 'Prepare failed');
        expect(api.calls.where((c) => c.startsWith('apply')).length, 1);
        controller.dispose();
      },
    );
    for (final action in serviceActions.keys) {
      test(
        '$action prepare/confirm/apply correlation and cancellation',
        () async {
          final state = switch (action) {
            'install-binary' => profile(binary: 'missing'),
            'install-unit' => profile(unit: 'missing'),
            'start' => profile(runtime: 'stopped'),
            _ => profile(setup: false),
          };
          final api = FakeControlApi(state);
          final controller = BootController(api);
          await controller.refresh();
          await controller.act(action, (_, proposal) async {
            expect(proposal.effects['description'], 'Exact proposed effect');
            return false;
          });
          expect(api.calls, ['inspect', 'prepare:$action']);
          await controller.act(action, (_, _) async => true);
          expect(api.calls.last, 'apply:$action:bound-token');
          controller.dispose();
        },
      );
    }
    test(
      'unknown action outcome requires fresh Refresh before retry',
      () async {
        final api = FakeControlApi(profile(setup: false));
        final controller = BootController(api);
        await controller.refresh();
        api.applyFailure = const ControlFailure(
          'Helper timed out. Outcome unknown; no rollback assumed. Refresh before retry.',
          outcomeUnknown: true,
        );
        await controller.act('complete-setup', (_, _) async => true);
        expect(controller.error, contains('Outcome unknown'));
        expect(controller.error, contains('no rollback'));
        expect(controller.observation, isNull);
        expect(controller.actions, isEmpty);
        await controller.act(
          'complete-setup',
          (_, _) async => fail('Refresh required'),
        );
        expect(api.calls, [
          'inspect',
          'prepare:complete-setup',
          'apply:complete-setup:bound-token',
        ]);
        await controller.refresh();
        expect(controller.error, isNull);
        expect(controller.actions, ['complete-setup']);
        api.applyFailure = null;
        await controller.act('complete-setup', (_, _) async => true);
        expect(api.calls.sublist(3), [
          'inspect',
          'prepare:complete-setup',
          'apply:complete-setup:bound-token',
        ]);
        controller.dispose();
      },
    );
    test(
      'failed Refresh cannot clear the unknown-outcome retry guard',
      () async {
        final api = FakeControlApi(profile(setup: false));
        final controller = BootController(api);
        await controller.refresh();
        api.applyFailure = const ControlFailure(
          'Outcome unknown; Refresh before retry.',
          outcomeUnknown: true,
        );
        await controller.act('complete-setup', (_, _) async => true);
        api.inspectFailure = const ControlFailure('Inspection unavailable');
        await controller.refresh();
        expect(controller.actions, isEmpty);
        api.inspectFailure = null;
        await controller.refresh();
        expect(controller.actions, ['complete-setup']);
        controller.dispose();
      },
    );
    test('failures remain visible and refresh clears action error', () async {
      final api = FakeControlApi(profile(setup: false));
      final controller = BootController(api);
      await controller.refresh();
      api.applyFailure = const ControlFailure('Apply failed');
      await controller.act('complete-setup', (_, _) async => true);
      expect(controller.error, 'Apply failed');
      await controller.refresh();
      expect(controller.error, isNull);
      api.prepareOk = false;
      await controller.act('complete-setup', (_, _) async => true);
      expect(controller.error, 'Prepare failed');
      expect(api.calls.where((c) => c.startsWith('apply')).length, 1);
      controller.dispose();
    });
    test(
      'busy blocks double click and refresh while confirmation is pending',
      () async {
        final api = FakeControlApi(profile(setup: false));
        final controller = BootController(api);
        await controller.refresh();
        final consent = Completer<bool>();
        final work = controller.act('complete-setup', (_, _) => consent.future);
        await Future<void>.delayed(Duration.zero);
        await controller.act('complete-setup', (_, _) async => true);
        await controller.refresh();
        expect(api.calls, ['inspect', 'prepare:complete-setup']);
        consent.complete(true);
        await work;
        expect(api.calls.last, 'apply:complete-setup:bound-token');
        controller.dispose();
      },
    );
    test(
      'disposed inspection ignores stale result and never stops daemon',
      () async {
        final api = FakeControlApi(profile())
          ..pendingInspect = Completer<ControlReply>();
        final controller = BootController(api);
        final work = controller.refresh();
        controller.dispose();
        api.pendingInspect!.complete(reply(profile()));
        await work;
        expect(controller.page, BootPage.loader);
        expect(api.disposed, isTrue);
        expect(api.calls, ['inspect']);
      },
    );
    test('dispose during preparation never confirms or applies', () async {
      final api = FakeControlApi(profile(setup: false));
      final controller = BootController(api);
      await controller.refresh();
      api.pendingPrepare = Completer<ControlReply>();
      final work = controller.act(
        'complete-setup',
        (_, _) async => fail('stale confirmation'),
      );
      controller.dispose();
      api.pendingPrepare!.complete(
        reply({...profile(), 'approval_token': 'token'}, prepare: true),
      );
      await work;
      expect(api.calls, ['inspect', 'prepare:complete-setup']);
    });
    test('dispose during confirmation never applies', () async {
      final api = FakeControlApi(profile(setup: false));
      final controller = BootController(api);
      await controller.refresh();
      final consent = Completer<bool>();
      final work = controller.act('complete-setup', (_, _) => consent.future);
      await Future<void>.delayed(Duration.zero);
      controller.dispose();
      consent.complete(true);
      await work;
      expect(api.calls, ['inspect', 'prepare:complete-setup']);
    });
    test('disposed apply cannot promote stale completion to Home', () async {
      final api = FakeControlApi(profile(setup: false));
      final controller = BootController(api);
      await controller.refresh();
      api.pendingApply = Completer<ControlReply>();
      final work = controller.act('complete-setup', (_, _) async => true);
      await Future<void>.delayed(Duration.zero);
      controller.dispose();
      api.pendingApply!.complete(reply(profile()));
      await work;
      expect(controller.page, BootPage.setup);
      expect(controller.observation!.setupComplete, isFalse);
      expect(api.disposed, isTrue);
    });
    test('malformed reply fails closed and refresh recovers', () async {
      final api = FakeControlApi({...profile(), 'service': 'wrong'});
      final controller = BootController(api);
      await controller.refresh();
      expect(controller.page, BootPage.setup);
      expect(controller.actions, isEmpty);
      expect(controller.error, contains('identity mismatch'));
      api.state = profile();
      await controller.refresh();
      expect(controller.page, BootPage.home);
      controller.dispose();
    });
  });

  testWidgets(
    'returning stopped service shows Home repair and explicit Start',
    (tester) async {
      final api = FakeControlApi(
        profile(runtime: 'stopped', bridge: 'unavailable'),
      );
      await tester.pumpWidget(
        SmithApp(appApi: FakeAppApi(), audio: FakeAudio(), controlApi: api),
      );
      await tester.pumpAndSettle();
      expect(find.text('Core Home'), findsOneWidget);
      expect(
        find.textContaining('Service reports persisted setup complete'),
        findsOneWidget,
      );
      expect(
        find.textContaining('current readiness is not verified'),
        findsOneWidget,
      );
      expect(find.text('Start service'), findsOneWidget);
      expect(find.text('Register service'), findsNothing);
      await tester.tap(find.text('Details'));
      await tester.pumpAndSettle();
      expect(find.text('Register service'), findsOneWidget);
      expect(find.text('Refresh'), findsOneWidget);
      expect(api.calls, ['inspect']);
    },
  );
  testWidgets('preflight message and error render without malformed masking', (
    tester,
  ) async {
    final api = FakeControlApi(preflightError());
    await tester.pumpWidget(
      SmithApp(appApi: FakeAppApi(), audio: FakeAudio(), controlApi: api),
    );
    await tester.pumpAndSettle();
    expect(find.text('Root preflight failed.'), findsOneWidget);
    expect(find.text('Error: ${api.state['error']}'), findsOneWidget);
    expect(find.byType(ElevatedButton), findsNothing);
    expect(find.text('Refresh'), findsOneWidget);
    expect(api.calls, ['inspect']);
  });
  testWidgets('loader reads only then renders Home with editable chat', (
    tester,
  ) async {
    final api = FakeControlApi(profile())
      ..pendingInspect = Completer<ControlReply>();
    await tester.pumpWidget(
      SmithApp(appApi: FakeAppApi(), audio: FakeAudio(), controlApi: api),
    );
    expect(find.text('Loading service status…'), findsOneWidget);
    api.pendingInspect!.complete(reply(profile()));
    await tester.pumpAndSettle();
    expect(find.text('Core Home'), findsOneWidget);
    expect(
      find.textContaining('Core lifecycle and local model'),
      findsOneWidget,
    );
    expect(find.byType(EditableText), findsOneWidget);
    expect(api.calls, ['inspect']);
    await tester.pumpWidget(const SizedBox());
    expect(api.disposed, isTrue);
    expect(api.calls, ['inspect']);
  });
  testWidgets('confirmation displays exact effects, cancel does not apply', (
    tester,
  ) async {
    final api = FakeControlApi(profile(setup: false));
    await tester.pumpWidget(
      SmithApp(appApi: FakeAppApi(), audio: FakeAudio(), controlApi: api),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('Finish setup'));
    await tester.pumpAndSettle();
    expect(find.text('Exact proposed effect'), findsOneWidget);
    for (final key in [
      'binary_path',
      'unit_path',
      'state_path',
      'socket_path',
    ]) {
      expect(
        find.text('$key: ${(api.state['effects'] as Map)[key]}'),
        findsOneWidget,
      );
    }
    expect(
      find.textContaining('continues after desktop close'),
      findsOneWidget,
    );
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(api.calls, ['inspect', 'prepare:complete-setup']);
    await tester.tap(find.text('Finish setup'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Confirm'));
    await tester.pumpAndSettle();
    expect(api.calls.last, 'apply:complete-setup:bound-token');
  });
  testWidgets(
    'fresh prepared effects precede consent and busy blocks duplicates',
    (tester) async {
      final api = FakeControlApi(profile(setup: false))
        ..pendingPrepare = Completer<ControlReply>();
      await tester.pumpWidget(
        SmithApp(appApi: FakeAppApi(), audio: FakeAudio(), controlApi: api),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('Finish setup'));
      await tester.pump();
      await tester.tap(find.text('Finish setup'));
      await tester.tap(find.text('Refresh'));
      await tester.tap(find.text('2 LLM'));
      await tester.pump();
      expect(find.text('Step 1 · Core'), findsOneWidget);
      expect(find.text('Confirm'), findsNothing);
      expect(api.calls, ['inspect', 'prepare:complete-setup']);
      final proposal = profile(setup: false);
      proposal['approval_token'] = 'fresh-token';
      proposal['effects'] = {
        'description': 'Fresh prepared effects, not inspection',
        'binary_path': '/prepared/bin',
        'unit_path': '/prepared/unit',
        'state_path': '/prepared/state',
        'socket_path': '/prepared/socket',
      };
      api.pendingPrepare!.complete(reply(proposal, prepare: true));
      await tester.pumpAndSettle();
      expect(
        find.text('Fresh prepared effects, not inspection'),
        findsOneWidget,
      );
      for (final key in [
        'binary_path',
        'unit_path',
        'state_path',
        'socket_path',
      ]) {
        expect(
          find.text('$key: ${(proposal['effects'] as Map)[key]}'),
          findsOneWidget,
        );
      }
      expect(api.calls, ['inspect', 'prepare:complete-setup']);
      await tester.tap(find.text('Confirm'));
      await tester.pumpAndSettle();
      expect(api.calls, [
        'inspect',
        'prepare:complete-setup',
        'apply:complete-setup:fresh-token',
      ]);
    },
  );

  testWidgets('failed preparation cannot open confirmation', (tester) async {
    final api = FakeControlApi(profile(setup: false))..prepareOk = false;
    await tester.pumpWidget(
      SmithApp(appApi: FakeAppApi(), audio: FakeAudio(), controlApi: api),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('Finish setup'));
    await tester.pumpAndSettle();
    expect(find.text('Confirm'), findsNothing);
    expect(find.text('Error: Prepare failed'), findsOneWidget);
    expect(api.calls, ['inspect', 'prepare:complete-setup']);
  });

  testWidgets('planned navigation cannot mutate or exit incomplete Core', (
    tester,
  ) async {
    final api = FakeControlApi(profile(setup: false));
    await tester.pumpWidget(
      SmithApp(appApi: FakeAppApi(), audio: FakeAudio(), controlApi: api),
    );
    await tester.pumpAndSettle();
    for (final label in ['2 LLM', '3 Services', '1 Core', '3 Services']) {
      await tester.tap(find.text(label));
      await tester.pumpAndSettle();
      expect(find.text('Return to Core Home'), findsNothing);
      expect(api.calls, ['inspect']);
    }
    expect(find.textContaining('Search remains planned'), findsOneWidget);
    expect(find.byType(TextField), findsNothing);
    await tester.tap(find.text('2 LLM'));
    await tester.pumpAndSettle();
    expect(find.text('Refresh models and speech status'), findsOneWidget);
    expect(api.calls, ['inspect']);
  });

  testWidgets('Core Home setup viewer preserves returning repair routing', (
    tester,
  ) async {
    final api = FakeControlApi(profile());
    await tester.pumpWidget(
      SmithApp(appApi: FakeAppApi(), audio: FakeAudio(), controlApi: api),
    );
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('View setup'));
    await tester.tap(find.text('View setup'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('2 LLM'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('3 Services'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Return to Core Home'));
    await tester.tap(find.text('Return to Core Home'));
    await tester.pumpAndSettle();
    expect(find.text('Core Home'), findsOneWidget);
    expect(api.calls, ['inspect']);
    api.inspectFailure = const ControlFailure('Inspection unavailable');
    await tester.ensureVisible(find.text('Refresh'));
    await tester.tap(find.text('Refresh'));
    await tester.pumpAndSettle();
    expect(find.text('Core Home'), findsOneWidget);
    expect(find.text('Core needs attention'), findsOneWidget);
    expect(find.byType(ElevatedButton), findsNothing);
    expect(api.calls, ['inspect', 'inspect']);
  });

  testWidgets(
    'confirmed Core completion returns Core Home, not full readiness',
    (tester) async {
      final api = FakeControlApi(profile(setup: false));
      await tester.pumpWidget(
        SmithApp(appApi: FakeAppApi(), audio: FakeAudio(), controlApi: api),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('Finish setup'));
      await tester.pumpAndSettle();
      api.state = profile();
      await tester.tap(find.text('Confirm'));
      await tester.pumpAndSettle();
      expect(find.text('Core Home'), findsOneWidget);
      expect(
        find.textContaining('Core lifecycle and local model'),
        findsOneWidget,
      );
      expect(api.calls, [
        'inspect',
        'prepare:complete-setup',
        'apply:complete-setup:bound-token',
      ]);
    },
  );

  for (final state in [
    profile(binary: 'conflict', setup: null),
    profile(unit: 'unknown', setup: null),
    profile(bridge: 'unknown', setup: null),
    profile(setup: null),
    preflightError(),
  ]) {
    testWidgets(
      'unverified facts do not offer confirmation: ${jsonEncode(state)}',
      (tester) async {
        final api = FakeControlApi(state);
        await tester.pumpWidget(
          SmithApp(appApi: FakeAppApi(), audio: FakeAudio(), controlApi: api),
        );
        await tester.pumpAndSettle();
        expect(find.byType(ElevatedButton), findsNothing);
        await tester.tap(find.text('2 LLM'));
        await tester.pumpAndSettle();
        await tester.tap(find.text('1 Core'));
        await tester.pumpAndSettle();
        expect(find.text('Confirm'), findsNothing);
        expect(api.calls, ['inspect']);
      },
    );
  }

  testWidgets(
    'dark wizard has labeled accessible controls and readable contrast',
    (tester) async {
      final semantics = tester.ensureSemantics();
      await tester.pumpWidget(
        SmithApp(
          appApi: FakeAppApi(),
          audio: FakeAudio(),
          controlApi: FakeControlApi(profile(setup: false)),
        ),
      );
      await tester.pumpAndSettle();
      final context = tester.element(find.text('Smith'));
      expect(Theme.of(context).brightness, Brightness.dark);
      expect(find.bySemanticsLabel('1 Core'), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
      expect(FocusManager.instance.primaryFocus?.context, isNotNull);
      await expectLater(tester, meetsGuideline(labeledTapTargetGuideline));
      await expectLater(tester, meetsGuideline(textContrastGuideline));
      semantics.dispose();
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(find.text('Step 2 · LLM'), findsOneWidget);
    },
  );

  for (final label in ['2 LLM', '3 Services']) {
    testWidgets('$label fits narrow enlarged text without mutation', (
      tester,
    ) async {
      tester.view.physicalSize = const Size(320, 240);
      tester.view.devicePixelRatio = 1;
      tester.platformDispatcher.textScaleFactorTestValue = 2;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
      final api = FakeControlApi(profile(setup: false));
      await tester.pumpWidget(
        SmithApp(appApi: FakeAppApi(), audio: FakeAudio(), controlApi: api),
      );
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.text(label));
      await tester.tap(find.text(label));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.text('Refresh models and speech status'));
      expect(tester.takeException(), isNull);
      expect(api.calls, ['inspect']);
    });
  }

  for (final home in [false, true]) {
    for (final size in [const Size(320, 240), const Size(1280, 800)]) {
      for (final scale in [1.0, 2.0]) {
        testWidgets('${home ? 'Home' : 'Setup'} fits $size at scale $scale', (
          tester,
        ) async {
          tester.view.physicalSize = size;
          tester.view.devicePixelRatio = 1;
          tester.platformDispatcher.textScaleFactorTestValue = scale;
          addTearDown(tester.view.resetPhysicalSize);
          addTearDown(tester.view.resetDevicePixelRatio);
          addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
          await tester.pumpWidget(
            SmithApp(
              appApi: FakeAppApi(),
              audio: FakeAudio(),
              controlApi: FakeControlApi(profile(setup: home)),
            ),
          );
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
          if (!home) {
            await tester.ensureVisible(find.text('Finish setup'));
            await tester.tap(find.text('Finish setup'));
            await tester.pumpAndSettle();
            expect(tester.takeException(), isNull);
            await tester.ensureVisible(find.text('Cancel'));
            await tester.tap(find.text('Cancel'));
            await tester.pumpAndSettle();
            expect(tester.takeException(), isNull);
          }
        });
      }
    }
  }
}
