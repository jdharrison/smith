import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/boot.dart';
import 'package:smith_client/control_api.dart';
import 'package:smith_client/main.dart';
import 'package:smith_client/text_chat.dart';
import 'package:smith_client/ui/smith_workspace.dart';
import 'package:smith_client/ui/smith_theme.dart';

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

class HeldConversationApi extends FakeAppApi {
  final synchronization = Completer<void>();

  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    if (op == 'conversation_get') await synchronization.future;
    return super.call(op, args);
  }
}

class PresentationCoordinator extends BootCoordinator {
  PresentationCoordinator()
    : super(
        control: BootControl(profile()),
        chat: ChatController(LegacyFakeAppApi()),
      );

  void show(BootPhase phase) {
    state = BootState(
      phase: phase,
      status: phase == BootPhase.ready ? 'Connected to Smith' : 'Check Smith',
    );
    notifyListeners();
  }

  @override
  void dispose() {
    super.dispose();
    chat.dispose();
    control.dispose();
  }
}

Widget presentationHarness(
  PresentationCoordinator coordinator, {
  bool reducedMotion = false,
}) => MaterialApp(
  theme: SmithTheme.dark(),
  home: MediaQuery(
    data: MediaQueryData(disableAnimations: reducedMotion),
    child: BootPresentation(
      coordinator: coordinator,
      readyBuilder: (context) => const Text('Workspace mounted'),
    ),
  ),
);

double bootOpacity(WidgetTester tester) => tester
    .widget<Opacity>(find.byKey(const ValueKey('boot-transition')))
    .opacity;

Future<void> advancePresentation(WidgetTester tester, int milliseconds) async {
  await tester.pump();
  if (milliseconds < 500) {
    await tester.pump(Duration(milliseconds: milliseconds));
    return;
  }
  await tester.pump(bootIntroDuration);
  if (milliseconds < 1500) {
    await tester.pump(Duration(milliseconds: milliseconds - 500));
    return;
  }
  await tester.pump(bootMinimumHoldDuration);
  await tester.pump(Duration(milliseconds: milliseconds - 1500));
}

void main() {
  testWidgets('a delayed intro frame cannot consume the fully visible hold', (
    tester,
  ) async {
    final coordinator = PresentationCoordinator()..show(BootPhase.ready);
    await tester.pumpWidget(presentationHarness(coordinator));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));
    expect(bootOpacity(tester), lessThan(1));
    await tester.pump(const Duration(milliseconds: 1251));
    expect(bootOpacity(tester), 1);
    await tester.pump(const Duration(milliseconds: 999));
    expect(bootOpacity(tester), 1);
    expect(find.text('Workspace mounted'), findsNothing);
    await tester.pump(const Duration(milliseconds: 1));
    expect(bootOpacity(tester), 1);
    await tester.pump(const Duration(milliseconds: 499));
    expect(find.text('Workspace mounted'), findsNothing);
    await tester.pump(const Duration(milliseconds: 1));
    expect(find.text('Workspace mounted'), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
    coordinator.dispose();
  });

  testWidgets('fast launch uses 500ms intro, 1s hold and 500ms outro', (
    tester,
  ) async {
    final control = FakeControlApi(profile());
    final app = FakeAppApi();
    await tester.pumpWidget(SmithApp(controlApi: control, appApi: app));
    await flushWidget(tester);

    expect(control.calls, ['inspect']);
    expect(app.calls, ['state', 'conversation_get', 'models', 'poll']);
    expect(find.text('Connected to Smith'), findsOneWidget);
    expect(bootOpacity(tester), 0);
    expect(find.byType(ConversationScreen), findsNothing);

    await tester.pump(const Duration(milliseconds: 250));
    expect(bootOpacity(tester), allOf(greaterThan(0), lessThan(1)));
    await tester.pump(const Duration(milliseconds: 250));
    expect(bootOpacity(tester), 1);
    await tester.pump(const Duration(milliseconds: 999));
    expect(bootOpacity(tester), 1);
    expect(find.byType(ConversationScreen), findsNothing);

    await tester.pump(const Duration(milliseconds: 1));
    expect(bootOpacity(tester), 1);

    await tester.pump(const Duration(milliseconds: 250));
    expect(
      bootOpacity(tester),
      closeTo(1 - Curves.easeInOutCubic.transform(0.5), 0.001),
    );
    expect(find.byType(TextField), findsNothing);
    await tester.pump(const Duration(milliseconds: 249));
    expect(find.byType(ConversationScreen), findsNothing);
    await tester.pump(const Duration(milliseconds: 1, microseconds: 1));
    expect(find.byType(ConversationScreen), findsOneWidget);
    await flushWidget(tester);
    expect(find.byType(TextField), findsOneWidget);
    expect(find.byKey(const ValueKey('boot-surface')), findsNothing);
    expect(control.calls, ['inspect']);
    expect(app.calls, ['state', 'conversation_get', 'models', 'poll']);
    await tester.pumpWidget(const SizedBox());
  });

  for (final holdSynchronization in [false, true]) {
    testWidgets(
      'slow ${holdSynchronization ? 'synchronization' : 'Core readiness'} extends hold, then exits without another minimum',
      (tester) async {
        final control = FakeControlApi(profile());
        final app = HeldConversationApi();
        if (!holdSynchronization) {
          control.pendingInspect = Completer<ControlReply>();
          app.synchronization.complete();
        }
        await tester.pumpWidget(SmithApp(controlApi: control, appApi: app));
        await flushWidget(tester);
        await tester.pump(const Duration(milliseconds: 1500));
        await tester.pump(const Duration(milliseconds: 2500));
        expect(bootOpacity(tester), 1);
        expect(find.byType(ConversationScreen), findsNothing);
        expect(
          find.text(
            holdSynchronization ? 'Syncing conversation' : 'Starting Smith',
          ),
          findsOneWidget,
        );
        if (holdSynchronization) {
          app.synchronization.complete();
        } else {
          control.pendingInspect!.complete(bootReply(profile()));
        }
        await flushWidget(tester);
        await tester.pump(const Duration(milliseconds: 499));
        expect(find.byType(ConversationScreen), findsNothing);
        await tester.pump(const Duration(milliseconds: 1, microseconds: 1));
        expect(find.byType(ConversationScreen), findsOneWidget);
        expect(control.calls, ['inspect']);
        expect(app.calls, ['state', 'conversation_get', 'models', 'poll']);
        await tester.pumpWidget(const SizedBox());
      },
    );
  }

  testWidgets('reduced motion keeps a static opaque two-second sequence', (
    tester,
  ) async {
    final coordinator = PresentationCoordinator()..show(BootPhase.ready);
    await tester.pumpWidget(
      presentationHarness(coordinator, reducedMotion: true),
    );
    await tester.pump();
    for (final milliseconds in [250, 250, 1000, 250, 249]) {
      await tester.pump(Duration(milliseconds: milliseconds));
      if (milliseconds == 1000) {
        await tester.pump(const Duration(microseconds: 1));
      }
      expect(bootOpacity(tester), 1);
      final painter =
          tester
                  .widget<CustomPaint>(find.byKey(const ValueKey('boot-mark')))
                  .painter!
              as BootMarkPainter;
      expect(painter.progress, 0.12);
      expect(find.text('Workspace mounted'), findsNothing);
    }
    await tester.pump(const Duration(milliseconds: 1, microseconds: 1));
    expect(find.text('Workspace mounted'), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
    coordinator.dispose();
  });

  testWidgets('motion changes and rebuilds never restart presentation', (
    tester,
  ) async {
    final coordinator = PresentationCoordinator()..show(BootPhase.ready);
    await tester.pumpWidget(presentationHarness(coordinator));
    await advancePresentation(tester, 250);
    expect(bootOpacity(tester), allOf(greaterThan(0), lessThan(1)));
    await tester.pumpWidget(
      presentationHarness(coordinator, reducedMotion: true),
    );
    expect(bootOpacity(tester), 1);
    await tester.pump(const Duration(milliseconds: 250));
    await tester.pump(const Duration(milliseconds: 1000));
    await tester.pump(const Duration(microseconds: 1));
    await tester.pump(const Duration(milliseconds: 250));
    expect(find.text('Workspace mounted'), findsNothing);
    await tester.pumpWidget(presentationHarness(coordinator));
    expect(
      bootOpacity(tester),
      closeTo(1 - Curves.easeInOutCubic.transform(0.5), 0.001),
    );
    coordinator.show(BootPhase.ready);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250, microseconds: 1));
    expect(find.text('Workspace mounted'), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
    coordinator.dispose();
  });

  for (final milliseconds in [250, 750, 1750]) {
    testWidgets(
      'repair at ${milliseconds}ms cancels decoration and stale exit',
      (tester) async {
        final coordinator = PresentationCoordinator()..show(BootPhase.ready);
        await tester.pumpWidget(presentationHarness(coordinator));
        await advancePresentation(tester, milliseconds);
        coordinator.show(BootPhase.blocked);
        await tester.pump();
        expect(bootOpacity(tester), 1);
        expect(find.text('Observe again').hitTestable(), findsOneWidget);
        await tester.pump(const Duration(seconds: 5));
        expect(find.text('Workspace mounted'), findsNothing);
        expect(bootOpacity(tester), 1);
        coordinator.show(BootPhase.ready);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 499));
        expect(find.text('Workspace mounted'), findsNothing);
        await tester.pump(const Duration(milliseconds: 1, microseconds: 1));
        expect(find.text('Workspace mounted'), findsOneWidget);
        expect((coordinator.control as BootControl).calls, isEmpty);
        await tester.pumpWidget(const SizedBox());
        coordinator.dispose();
      },
    );
  }

  for (final milliseconds in [0, 250, 1000, 1750]) {
    testWidgets('unmount at ${milliseconds}ms safely disposes transition', (
      tester,
    ) async {
      final coordinator = PresentationCoordinator()..show(BootPhase.ready);
      await tester.pumpWidget(presentationHarness(coordinator));
      await advancePresentation(tester, milliseconds);
      await tester.pumpWidget(const SizedBox());
      coordinator.show(BootPhase.blocked);
      coordinator.dispose();
      await tester.pump(const Duration(seconds: 5));
      expect(tester.takeException(), isNull);
      expect(tester.binding.transientCallbackCount, 0);
    });
  }

  testWidgets(
    'launch shows only Boot, then mounts chat after ready synchronization',
    (tester) async {
      final control = FakeControlApi(profile());
      control.pendingInspect = Completer<ControlReply>();
      final app = FakeAppApi()
        ..history.addAll([
          {'role': 'user', 'content': 'retained question'},
          {'role': 'assistant', 'content': 'retained answer'},
        ]);

      await tester.pumpWidget(SmithApp(controlApi: control, appApi: app));
      await tester.pump();
      expect(find.byKey(const ValueKey('boot-surface')), findsOneWidget);
      expect(find.byKey(const ValueKey('boot-mark')), findsOneWidget);
      expect(find.text('Starting Smith'), findsOneWidget);
      expect(find.byType(SmithWorkspace), findsNothing);
      expect(find.byType(TextField), findsNothing);
      expect(find.text('Retry'), findsNothing);
      expect(find.byTooltip('Retry observation'), findsNothing);
      expect(find.byTooltip('Send'), findsNothing);
      expect(app.calls, isEmpty);
      await tester.pump(const Duration(seconds: 30));
      expect(app.calls, isEmpty);

      control.pendingInspect!.complete(bootReply(profile()));
      await tester.pumpAndSettle();
      expect(app.calls, ['state', 'conversation_get', 'models', 'poll']);
      expect(find.byKey(const ValueKey('boot-surface')), findsNothing);
      expect(find.byType(SmithWorkspace), findsOneWidget);
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
      expect(app.calls, ['state', 'conversation_get', 'models', 'poll']);
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
    final chat = ChatController(LegacyFakeAppApi())..connected = false;
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
