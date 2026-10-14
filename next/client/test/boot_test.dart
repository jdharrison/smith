import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/boot.dart';
import 'package:smith_client/control_api.dart';
import 'package:smith_client/main.dart';
import 'package:smith_client/text_chat.dart';

import 'app_fakes.dart';
import 'widget_test.dart' show FakeControlApi, profile;

ControlReply bootReply(Map<String, dynamic> value, {bool prepare = false}) =>
    ControlReply.parse(jsonEncode(value), prepare: prepare);

class FakeBootClock implements BootClock {
  Duration current = Duration.zero;
  void Function(Duration elapsed)? onWait;

  @override
  Duration get elapsed => current;

  @override
  Future<void> wait(Duration duration) async {
    current += duration;
    onWait?.call(current);
  }
}

class BootControl implements ControlApi {
  BootControl(this.state);

  Map<String, dynamic> state;
  final calls = <String>[];
  final deadlines = <Duration>[];
  bool disposed = false;

  @override
  Future<ControlReply> inspect({Duration? deadline}) async {
    calls.add('inspect');
    if (deadline != null) deadlines.add(deadline);
    return bootReply(state);
  }

  @override
  Future<ControlReply> prepare(String action) async {
    calls.add('prepare:$action');
    return bootReply({
      ...state,
      'approval_token': 'original-start-token',
      'effects': {
        ...(state['effects'] as Map<String, dynamic>),
        'description': 'Start exact unit smith-next.service now. Login enablement is unchanged.',
      },
    }, prepare: true);
  }

  @override
  Future<ControlReply> apply(String action, String approvalToken) async {
    calls.add('apply:$action:$approvalToken');
    state = profile();
    return bootReply(state);
  }

  @override
  void dispose() {
    disposed = true;
  }
}

Future<void> flushWidget(WidgetTester tester) async {
  for (var index = 0; index < 30; index++) {
    await tester.pump();
  }
}

void main() {
  testWidgets(
    'launch shows only Boot, then mounts chat after ready synchronization',
    (tester) async {
      final control = FakeControlApi(profile());
      control.pendingInspect = Completer<ControlReply>();
      final app = FakeAppApi()
        ..history.add({'role': 'assistant', 'content': 'retained answer'});

      await tester.pumpWidget(SmithApp(controlApi: control, appApi: app));
      await tester.pump();
      expect(find.byKey(const ValueKey('boot-surface')), findsOneWidget);
      expect(find.byKey(const ValueKey('boot-mark')), findsOneWidget);
      expect(find.text('Starting Smith'), findsOneWidget);
      expect(find.byKey(const ValueKey('mode-sidebar')), findsNothing);
      expect(find.byType(TextField), findsNothing);
      expect(find.text('Retry'), findsNothing);
      expect(find.text('Send'), findsNothing);
      expect(app.calls, isEmpty);

      control.pendingInspect!.complete(bootReply(profile()));
      await tester.pumpAndSettle();
      expect(app.calls, ['state', 'models', 'poll']);
      expect(find.byKey(const ValueKey('boot-surface')), findsNothing);
      expect(find.byKey(const ValueKey('mode-sidebar')), findsOneWidget);
      expect(find.text('retained answer'), findsOneWidget);
      expect(find.byType(TextField), findsOneWidget);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'stopped polling is finite and exposes Start only at 15 seconds',
    (tester) async {
      final clock = FakeBootClock();
      final control = BootControl(
        profile(runtime: 'stopped', bridge: 'unavailable'),
      );
      await tester.pumpWidget(
        SmithApp(controlApi: control, appApi: FakeAppApi(), bootClock: clock),
      );
      await flushWidget(tester);

      expect(clock.elapsed, defaultBootObservationBudget);
      expect(find.text('Smith is stopped'), findsOneWidget);
      expect(find.text('Start Smith'), findsOneWidget);
      expect(find.byType(TextField), findsNothing);
      expect(control.deadlines, isNotEmpty);
      expect(
        control.deadlines.every((value) => value <= const Duration(seconds: 2)),
        isTrue,
      );
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'Start prepares, confirms exact effect, applies token once, observes',
    (tester) async {
      final clock = FakeBootClock();
      final control = BootControl(
        profile(runtime: 'stopped', bridge: 'unavailable'),
      );
      final app = FakeAppApi();
      await tester.pumpWidget(
        SmithApp(controlApi: control, appApi: app, bootClock: clock),
      );
      await flushWidget(tester);
      await tester.tap(find.text('Start Smith'));
      await tester.pumpAndSettle();

      expect(find.text('Start Smith?'), findsOneWidget);
      expect(
        find.text(
          'Start exact unit smith-next.service now. Login enablement is unchanged.',
        ),
        findsOneWidget,
      );
      expect(
        control.calls.where((call) => call == 'prepare:start'),
        hasLength(1),
      );
      expect(control.calls.where((call) => call.startsWith('apply:')), isEmpty);

      await tester.tap(find.widgetWithText(FilledButton, 'Start Smith'));
      await tester.pumpAndSettle();
      expect(
        control.calls
            .where((call) => call == 'apply:start:original-start-token')
            .length,
        1,
      );
      expect(control.calls.last, 'inspect');
      expect(app.calls, ['state', 'models', 'poll']);
      expect(find.byType(TextField), findsOneWidget);
      await tester.pumpWidget(const SizedBox());
    },
  );

  for (final unsafe in <Map<String, dynamic>>[
    profile(binary: 'missing'),
    profile(unit: 'missing', enablement: 'missing'),
    profile(enablement: 'disabled'),
    profile(enablement: 'conflict'),
    profile(enablement: 'unknown'),
    profile(runtime: 'conflict', bridge: 'conflict'),
    profile(runtime: 'unknown', bridge: 'unknown'),
    profile(runtime: 'unavailable', bridge: 'unavailable'),
  ]) {
    testWidgets(
      'unsafe ${unsafe['binary_state']}/${unsafe['unit_state']}/${unsafe['enablement_state']}/${unsafe['runtime_state']} never offers Start',
      (tester) async {
        final control = BootControl(unsafe);
        final app = FakeAppApi();
        await tester.pumpWidget(
          SmithApp(
            controlApi: control,
            appApi: app,
            bootClock: FakeBootClock(),
          ),
        );
        await flushWidget(tester);
        expect(find.byKey(const ValueKey('boot-surface')), findsOneWidget);
        expect(find.byType(TextField), findsNothing);
        expect(app.calls, isEmpty);
        expect(find.widgetWithText(FilledButton, 'Start Smith'), findsNothing);
        expect(
          control.calls.where((call) => call.startsWith('prepare:')),
          isEmpty,
        );
        expect(
          control.calls.where((call) => call.startsWith('apply:')),
          isEmpty,
        );
        await tester.pumpWidget(const SizedBox());
      },
    );
  }

  testWidgets('reduced motion uses one static mark and status live region', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    final control = BootControl(
      profile(runtime: 'stopped', bridge: 'unavailable'),
    );
    final chat = ChatController(FakeAppApi())..connected = false;
    final coordinator = BootCoordinator(control: control, chat: chat);

    await tester.pumpWidget(
      MaterialApp(
        home: MediaQuery(
          data: const MediaQueryData(disableAnimations: true),
          child: BootSurface(coordinator: coordinator),
        ),
      ),
    );
    await tester.pump();
    final before =
        (tester
                    .widget<CustomPaint>(
                      find.byKey(const ValueKey('boot-mark')),
                    )
                    .painter!
                as BootMarkPainter)
            .progress;
    await tester.pump(const Duration(seconds: 5));
    final after =
        (tester
                    .widget<CustomPaint>(
                      find.byKey(const ValueKey('boot-mark')),
                    )
                    .painter!
                as BootMarkPainter)
            .progress;
    expect(after, before);
    final node = tester.getSemantics(
      find.byKey(const ValueKey('boot-status-semantics')),
    );
    expect(node.label, 'Starting Smith');
    expect(node.flagsCollection.isLiveRegion, isTrue);
    expect(find.byKey(const ValueKey('boot-mark')), findsOneWidget);

    await tester.pumpWidget(const SizedBox());
    coordinator.dispose();
    chat.dispose();
    control.dispose();
    semantics.dispose();
  });

  testWidgets('Boot fits a 360x800 logical viewport at 2x text', (
    tester,
  ) async {
    tester.platformDispatcher.textScaleFactorTestValue = 2;
    addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
    tester.view.physicalSize = const Size(720, 1600);
    tester.view.devicePixelRatio = 2;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final control = BootControl(
      profile(
        enablement: 'disabled',
        runtime: 'stopped',
        bridge: 'unavailable',
      ),
    );
    await tester.pumpWidget(
      SmithApp(
        controlApi: control,
        appApi: FakeAppApi(),
        bootClock: FakeBootClock(),
      ),
    );
    await flushWidget(tester);
    expect(find.byKey(const ValueKey('boot-surface')), findsOneWidget);
    expect(find.byType(TextField), findsNothing);
    expect(find.text('Observe again').hitTestable(), findsOneWidget);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });
}
