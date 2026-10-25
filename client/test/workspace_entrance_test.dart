import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/boot.dart';
import 'package:smith_client/chat_panel.dart';
import 'package:smith_client/cloud_routing.dart';
import 'package:smith_client/control_api.dart';
import 'package:smith_client/main.dart';
import 'package:smith_client/ui/settings_view.dart';
import 'package:smith_client/ui/smith_app_header.dart';
import 'package:smith_client/ui/smith_theme.dart';
import 'package:smith_client/ui/split_workspace.dart';
import 'package:smith_client/ui/workspace_entrance.dart';
import 'package:smith_client/workspace_controller.dart';
import 'package:smith_client/workspace_preferences.dart';

import 'app_fakes.dart';
import 'cloud_routing_test.dart'
    as cloud_fakes
    show CloudFake, readyCloud, local, cloud, generation;
import 'widget_test.dart' show FakeControlApi, profile, reply;
import 'workspace_restore_test.dart'
    as restore
    show MemoryStore, preferences, owner, presentation, noMutations;

// Keep the expected choreography independent of WorkspaceRegion.progress/fields.
const _schedule = <WorkspaceRegion, (int, int, Offset)>{
  WorkspaceRegion.header: (0, 220, Offset(0, -4)),
  WorkspaceRegion.navigation: (60, 220, Offset(-6, 0)),
  WorkspaceRegion.sidebar: (120, 220, Offset(-8, 0)),
  WorkspaceRegion.title: (180, 220, Offset(0, 4)),
  WorkspaceRegion.transcript: (240, 260, Offset(0, 6)),
  WorkspaceRegion.settings: (240, 260, Offset(0, 6)),
  WorkspaceRegion.composer: (340, 280, Offset(0, 8)),
};

Finder _opacity(WorkspaceRegion region) =>
    find.byKey(ValueKey('workspace-reveal-${region.name}'));
Finder _offset(WorkspaceRegion region) =>
    find.byKey(ValueKey('workspace-offset-${region.name}'));
Finder get _interaction =>
    find.byKey(const ValueKey('workspace-entrance-interaction'));

void _expectTimeline(
  WidgetTester tester,
  int milliseconds, {
  Iterable<WorkspaceRegion> regions = WorkspaceRegion.values,
  bool requireEveryRegion = true,
}) {
  for (final region in regions) {
    final (delay, duration, initialOffset) = _schedule[region]!;
    final linear = ((milliseconds - delay) / duration).clamp(0.0, 1.0);
    final progress = Curves.easeOutCubic.transform(linear);
    // Timeline normalization can land just above a delay boundary; Flutter's
    // cubic solver has finite precision there. Final settlement stays exact.
    final tolerance = milliseconds == delay ? 0.002 : 0.00001;
    final opacities = tester.widgetList<Opacity>(_opacity(region)).toList();
    final transforms = tester.widgetList<Transform>(_offset(region)).toList();
    if (requireEveryRegion) {
      expect(opacities, isNotEmpty, reason: '${region.name} must be mounted');
    }
    expect(transforms, hasLength(opacities.length));
    for (final opacity in opacities) {
      expect(
        opacity.opacity,
        milliseconds >= 620 ? equals(1.0) : closeTo(progress, tolerance),
        reason: '${region.name} opacity at ${milliseconds}ms',
      );
    }
    for (final transform in transforms) {
      expect(
        transform.transform.entry(0, 3),
        milliseconds >= 620
            ? equals(0.0)
            : closeTo(
                initialOffset.dx * (1 - progress),
                initialOffset.dx.abs() * tolerance + 0.00001,
              ),
        reason: '${region.name} horizontal offset at ${milliseconds}ms',
      );
      expect(
        transform.transform.entry(1, 3),
        milliseconds >= 620
            ? equals(0.0)
            : closeTo(
                initialOffset.dy * (1 - progress),
                initialOffset.dy.abs() * tolerance + 0.00001,
              ),
        reason: '${region.name} vertical offset at ${milliseconds}ms',
      );
    }
  }
}

void _expectGate(WidgetTester tester, {required bool blocked}) {
  expect(_interaction, findsOneWidget);
  expect(tester.widget<IgnorePointer>(_interaction).ignoring, blocked);
  expect(
    tester
        .widget<ExcludeFocus>(
          find
              .descendant(of: _interaction, matching: find.byType(ExcludeFocus))
              .first,
        )
        .excluding,
    blocked,
  );
  expect(
    tester
        .widget<ExcludeSemantics>(
          find
              .descendant(
                of: _interaction,
                matching: find.byType(ExcludeSemantics),
              )
              .first,
        )
        .excluding,
    blocked,
  );
}

Widget _regions([Iterable<WorkspaceRegion> regions = WorkspaceRegion.values]) =>
    Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        for (final region in regions)
          WorkspaceReveal(
            key: ValueKey('region-${region.name}'),
            region: region,
            child: SizedBox(
              key: ValueKey('child-${region.name}'),
              width: 160,
              height: 32,
              child: Text(region.name),
            ),
          ),
      ],
    );

Widget _harness({
  bool reducedMotion = false,
  bool skipMotion = false,
  Widget? child,
}) => MaterialApp(
  home: MediaQuery(
    data: MediaQueryData(disableAnimations: reducedMotion),
    child: Scaffold(
      body: WorkspaceEntrance(
        skipMotion: skipMotion,
        child: child ?? _regions(),
      ),
    ),
  ),
);

Future<void> _flush(WidgetTester tester) async {
  // Drain hydration/reconciliation without advancing any presentation clock.
  for (var frame = 0; frame < 20; frame++) {
    await tester.pump();
  }
}

Finder _inTile(int index, Finder matching) => find.descendant(
  of: find.byKey(ValueKey('workspace-tile-$index'), skipOffstage: false),
  matching: matching,
  skipOffstage: false,
);
Finder _editor(int index) => _inTile(
  index,
  find.descendant(
    of: find.byType(ChatPanel, skipOffstage: false),
    matching: find.byType(TextField, skipOffstage: false),
    skipOffstage: false,
  ),
);

void _appSize(WidgetTester tester) {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = const Size(2200, 1400);
  addTearDown(tester.view.resetDevicePixelRatio);
  addTearDown(tester.view.resetPhysicalSize);
}

Future<void> _mountApp(
  WidgetTester tester,
  restore.MemoryStore store,
  FakeAppApi api,
  FakeControlApi control,
) async {
  _appSize(tester);
  api.inventoryObserved = true;
  addTearDown(() async {
    await tester.pumpWidget(const SizedBox());
    await _flush(tester);
  });
  await tester.pumpWidget(
    SmithApp(controlApi: control, appApi: api, preferencesStore: store),
  );
}

Future<void> _holdAtRestoration(
  WidgetTester tester,
  restore.MemoryStore store,
  FakeAppApi api,
  FakeControlApi control,
) async {
  await _mountApp(tester, store, api, control);
  await tester.pumpAndSettle();
  expect(find.text('Restoring workspace…'), findsOneWidget);
  expect(find.byType(WorkspaceEntrance), findsNothing);
  expect(find.byType(SplitWorkspace), findsNothing);
  expect(store.loads, 1);
  expect(store.writes, isEmpty);
}

void _expectAppSettled(WidgetTester tester) {
  expect(find.byType(WorkspaceEntrance, skipOffstage: false), findsOneWidget);
  expect(_opacity(WorkspaceRegion.header), findsOneWidget);
  _expectTimeline(tester, 620, requireEveryRegion: false);
  _expectGate(tester, blocked: false);
}

void _expectNoEffects(FakeControlApi control, FakeAppApi api) {
  expect(control.calls, ['inspect']);
  restore.noMutations(api);
}

class _AvailabilityCloudFake extends cloud_fakes.CloudFake {
  final unavailable = <ChatRoute>{};
  final _credentialReplacements = <String, ChatRoute>{};

  ChatRoute replaceCredential(ChatRoute historical) {
    expect(historical.cloud, isTrue);
    expect(_credentialReplacements.containsKey(historical.provider), isFalse);
    final replacement = ChatRoute(
      historical.provider,
      historical.model,
      historical.provider == 'openai' ? 'c' * 32 : 'd' * 32,
    );
    expect(replacement.generation, isNot(historical.generation));
    _credentialReplacements[historical.provider] = replacement;
    return replacement;
  }

  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    final reply = await super.call(op, args);
    if (op == 'route_catalog') {
      for (final provider in reply.result['providers'] as List) {
        final replacement = _credentialReplacements[provider['provider']];
        if (replacement == null) continue;
        provider['revision'] += 1;
        provider['credential_generation'] = replacement.generation;
        provider['credential_state'] = 'available';
        provider['credential_error'] = null;
        provider['validation'] = null;
      }
      for (final choice in reply.result['models'] as List) {
        final route = ChatRoute.parse(choice['route'], catalog: true);
        final replacement = _credentialReplacements[route.provider];
        if (replacement != null) {
          choice['route']['credential_generation'] = replacement.generation;
          choice['available'] = true;
          choice['unavailable_reason'] = null;
          choice['validated_model'] = false;
        } else if (!route.cloud && unavailable.contains(route)) {
          choice['available'] = false;
          choice['unavailable_reason'] = {
            'code': 'MODEL_UNAVAILABLE',
            'message': 'Fake route temporarily unavailable',
          };
        }
      }
    }
    return reply;
  }
}

Future<WorkspacePresentation> _mountRoutedWorkspace(
  WidgetTester tester,
  WorkspaceController owner,
  FakeControlApi control,
  restore.MemoryStore store,
) async {
  _appSize(tester);
  final presentation = WorkspacePresentation(owner, store);
  addTearDown(() async {
    await tester.pumpWidget(const SizedBox());
    presentation.dispose();
    owner.dispose();
    control.dispose();
    await _flush(tester);
  });
  // readyCloud has already synchronized authoritative state. Hydrate before
  // mounting so assertions inspect the first eligible workspace frame itself.
  await presentation.hydrate();
  expect(presentation.loading, isFalse);
  expect(presentation.warning, isNull);
  await tester.pumpWidget(
    MaterialApp(
      theme: SmithTheme.dark(),
      home: ConversationScreen(
        control: control,
        chat: owner,
        presentation: presentation,
      ),
    ),
  );
  return presentation;
}

void _expectRouteState(ConversationController selected, {required bool ready}) {
  expect(selected.owner.routingEnabled, isTrue);
  expect(selected.owner.connected, isTrue);
  expect(selected.owner.stale, isFalse);
  expect(selected.owner.error, isNull);
  expect(selected.stale, isFalse);
  expect(selected.error, isNull);
  expect(selected.uncertain, isFalse);
  expect(selected.canAbandonExpiredRequest, isFalse);
  expect(selected.routeReady, ready);
}

void _expectCloudReadOnly(
  cloud_fakes.CloudFake api,
  FakeControlApi control,
  restore.MemoryStore store,
) {
  expect(control.calls, isEmpty);
  expect(api.accepted, isEmpty);
  expect(api.jobs, isEmpty);
  expect(
    api.calls.where(
      (op) => !{
        'state',
        'route_catalog',
        'conversation_get',
        'conversation_routing',
        'instructions_get',
      }.contains(op),
    ),
    isEmpty,
  );
  expect(store.loads, 1);
  expect(store.writes, isEmpty);
}

Future<void> _chooseLayout(WidgetTester tester, WorkspaceLayout layout) async {
  await tester.tap(find.byKey(const ValueKey('workspace-layout')));
  await tester.pumpAndSettle();
  await tester.tap(
    find.byWidgetPredicate(
      (widget) =>
          widget is CheckedPopupMenuItem<WorkspaceLayout> &&
          widget.value == layout,
    ),
  );
  await _flush(tester);
  // Inspect the first changed frame, before settling the popup's dismissal.
  _expectAppSettled(tester);
  await tester.pumpAndSettle();
}

void main() {
  group('WorkspaceEntrance presentation contract', () {
    testWidgets('a reveal without a scope leaves its child unchanged', (
      tester,
    ) async {
      var presses = 0;
      final child = TextButton(
        key: const ValueKey('standalone'),
        onPressed: () => presses++,
        child: const Text('Standalone action'),
      );
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: WorkspaceReveal(
              region: WorkspaceRegion.composer,
              child: child,
            ),
          ),
        ),
      );
      expect(
        tester.widget(find.byKey(const ValueKey('standalone'))),
        same(child),
      );
      expect(_opacity(WorkspaceRegion.composer), findsNothing);
      expect(_offset(WorkspaceRegion.composer), findsNothing);
      expect(_interaction, findsNothing);
      await tester.tap(find.text('Standalone action'));
      expect(presses, 1);
      await tester.pumpWidget(const SizedBox());
    });

    testWidgets('one post-frame clock staggers all regions through 620ms', (
      tester,
    ) async {
      expect(workspaceEntranceDuration, const Duration(milliseconds: 620));
      await tester.pumpWidget(_harness());
      _expectTimeline(tester, 0);
      _expectGate(tester, blocked: true);
      final entrance = tester.state(find.byType(WorkspaceEntrance));
      final wrappers = {
        for (final region in WorkspaceRegion.values)
          region: tester.element(_opacity(region)),
      };
      final children = {
        for (final region in WorkspaceRegion.values)
          region: tester.element(find.byKey(ValueKey('child-${region.name}'))),
      };
      await tester.pump();
      _expectTimeline(tester, 0);
      var previous = 0;
      for (final milliseconds in [
        59,
        60,
        119,
        120,
        179,
        180,
        220,
        240,
        280,
        340,
        400,
        500,
        619,
        620,
      ]) {
        await tester.pump(Duration(milliseconds: milliseconds - previous));
        previous = milliseconds;
        _expectTimeline(tester, milliseconds);
        _expectGate(tester, blocked: milliseconds < 620);
      }
      expect(tester.state(find.byType(WorkspaceEntrance)), same(entrance));
      for (final region in WorkspaceRegion.values) {
        expect(tester.element(_opacity(region)), same(wrappers[region]));
        final child = find.byKey(ValueKey('child-${region.name}'));
        expect(tester.element(child), same(children[region]));
        expect(tester.getSize(child), const Size(160, 32));
      }
      await tester.pump(const Duration(seconds: 2));
      _expectTimeline(tester, 620);
      expect(tester.binding.transientCallbackCount, 0);
      await tester.pumpWidget(const SizedBox());
    });

    testWidgets('newly exposed regions join the shared clock, not a new one', (
      tester,
    ) async {
      await tester.pumpWidget(
        _harness(
          child: _regions([WorkspaceRegion.header, WorkspaceRegion.composer]),
        ),
      );
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 300));
      final entrance = tester.state(find.byType(WorkspaceEntrance));
      final composer = tester.element(_opacity(WorkspaceRegion.composer));
      await tester.pumpWidget(
        _harness(
          child: _regions([
            WorkspaceRegion.header,
            WorkspaceRegion.sidebar,
            WorkspaceRegion.settings,
            WorkspaceRegion.composer,
          ]),
        ),
      );
      _expectTimeline(tester, 300, requireEveryRegion: false);
      expect(
        tester.element(_opacity(WorkspaceRegion.composer)),
        same(composer),
      );
      await tester.pump(const Duration(milliseconds: 320));
      await tester.pumpWidget(_harness());
      _expectTimeline(tester, 620);
      _expectGate(tester, blocked: false);
      expect(tester.state(find.byType(WorkspaceEntrance)), same(entrance));
      await tester.pumpWidget(const SizedBox());
    });

    testWidgets('initial reduced motion settles immediately and permanently', (
      tester,
    ) async {
      await tester.pumpWidget(_harness(reducedMotion: true));
      _expectTimeline(tester, 620);
      _expectGate(tester, blocked: false);
      final entrance = tester.state(find.byType(WorkspaceEntrance));
      await tester.pumpWidget(_harness());
      _expectTimeline(tester, 620);
      await tester.pump(const Duration(seconds: 2));
      _expectTimeline(tester, 620);
      expect(tester.state(find.byType(WorkspaceEntrance)), same(entrance));
      expect(tester.binding.transientCallbackCount, 0);
      await tester.pumpWidget(const SizedBox());
    });

    for (final elapsed in [0, 200]) {
      testWidgets('reduced motion at ${elapsed}ms settles without replay', (
        tester,
      ) async {
        await tester.pumpWidget(_harness());
        await tester.pump();
        await tester.pump(Duration(milliseconds: elapsed));
        _expectTimeline(tester, elapsed);
        await tester.pumpWidget(_harness(reducedMotion: true));
        _expectTimeline(tester, 620);
        _expectGate(tester, blocked: false);
        await tester.pumpWidget(_harness());
        _expectTimeline(tester, 620);
        await tester.pump(const Duration(seconds: 2));
        _expectTimeline(tester, 620);
        expect(tester.binding.transientCallbackCount, 0);
        await tester.pumpWidget(const SizedBox());
      });
    }

    testWidgets('skipMotion makes recovery immediately usable without replay', (
      tester,
    ) async {
      var recoveries = 0;
      Widget content() => WorkspaceReveal(
        region: WorkspaceRegion.composer,
        child: TextButton(
          onPressed: () => recoveries++,
          child: const Text('Recover workspace'),
        ),
      );
      await tester.pumpWidget(_harness(skipMotion: true, child: content()));
      _expectTimeline(tester, 620, regions: [WorkspaceRegion.composer]);
      _expectGate(tester, blocked: false);
      await tester.tap(find.text('Recover workspace'));
      expect(recoveries, 1);
      await tester.pumpWidget(_harness(child: content()));
      _expectTimeline(tester, 620, regions: [WorkspaceRegion.composer]);
      await tester.pump(const Duration(seconds: 2));
      _expectTimeline(tester, 620, regions: [WorkspaceRegion.composer]);
      expect(recoveries, 1);
      await tester.pumpWidget(const SizedBox());
    });

    testWidgets('mid-sequence skip settles permanently without remounting', (
      tester,
    ) async {
      await tester.pumpWidget(_harness());
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 300));
      _expectTimeline(tester, 300);
      final entrance = tester.state(find.byType(WorkspaceEntrance));
      final composer = tester.element(_opacity(WorkspaceRegion.composer));
      await tester.pumpWidget(_harness(skipMotion: true));
      _expectTimeline(tester, 620);
      _expectGate(tester, blocked: false);
      await tester.pumpWidget(_harness());
      await tester.pump(const Duration(milliseconds: 100));
      _expectTimeline(tester, 620);
      expect(tester.state(find.byType(WorkspaceEntrance)), same(entrance));
      expect(
        tester.element(_opacity(WorkspaceRegion.composer)),
        same(composer),
      );
      await tester.pumpWidget(const SizedBox());
    });

    testWidgets(
      'pointer, Tab/Enter and semantics are gated, then restored without autofocus',
      (tester) async {
        final semantics = tester.ensureSemantics();
        try {
          final outsideFocus = FocusNode();
          final actionFocus = FocusNode();
          final editorFocus = FocusNode();
          final editor = TextEditingController();
          addTearDown(outsideFocus.dispose);
          addTearDown(actionFocus.dispose);
          addTearDown(editorFocus.dispose);
          addTearDown(editor.dispose);
          var actions = 0;
          var edits = 0;
          await tester.pumpWidget(
            MaterialApp(
              home: Scaffold(
                body: FocusTraversalGroup(
                  policy: WidgetOrderTraversalPolicy(),
                  child: Column(
                    children: [
                      TextButton(
                        focusNode: outsideFocus,
                        onPressed: () {},
                        child: const Text('Outside workspace'),
                      ),
                      WorkspaceEntrance(
                        child: WorkspaceReveal(
                          region: WorkspaceRegion.composer,
                          child: Column(
                            children: [
                              TextButton(
                                focusNode: actionFocus,
                                onPressed: () => actions++,
                                child: const Text('Workspace action'),
                              ),
                              TextField(
                                controller: editor,
                                focusNode: editorFocus,
                                onChanged: (_) => edits++,
                              ),
                            ],
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          );
          await tester.pump();
          var previous = 0;
          for (final elapsed in [0, 400, 619]) {
            await tester.pump(Duration(milliseconds: elapsed - previous));
            previous = elapsed;
            _expectGate(tester, blocked: true);
            expect(find.bySemanticsLabel('Outside workspace'), findsOneWidget);
            expect(find.bySemanticsLabel('Workspace action'), findsNothing);
            await tester.tap(
              find.text('Workspace action'),
              warnIfMissed: false,
            );
            await tester.tap(find.byType(TextField), warnIfMissed: false);
            actionFocus.requestFocus();
            editorFocus.requestFocus();
            await tester.pump();
            expect(actionFocus.hasFocus, isFalse);
            expect(editorFocus.hasFocus, isFalse);
            outsideFocus.requestFocus();
            await tester.pump();
            for (var attempt = 0; attempt < 3; attempt++) {
              await tester.sendKeyEvent(LogicalKeyboardKey.tab);
              await tester.sendKeyEvent(LogicalKeyboardKey.enter);
              await tester.pump();
              expect(actionFocus.hasFocus, isFalse);
              expect(editorFocus.hasFocus, isFalse);
            }
            expect(actions, 0);
            expect(edits, 0);
            expect(editor.text, isEmpty);
          }
          await tester.pump(const Duration(milliseconds: 1));
          _expectGate(tester, blocked: false);
          expect(actionFocus.hasFocus, isFalse);
          expect(editorFocus.hasFocus, isFalse);
          expect(
            actions,
            0,
            reason: 'blocked input must not be buffered/replayed',
          );
          outsideFocus.requestFocus();
          await tester.pump();
          await tester.sendKeyEvent(LogicalKeyboardKey.tab);
          await tester.pump();
          expect(actionFocus.hasFocus, isTrue);
          await tester.sendKeyEvent(LogicalKeyboardKey.enter);
          await tester.pump();
          expect(actions, 1);
          final button = find.widgetWithText(TextButton, 'Workspace action');
          final node = tester.getSemantics(button);
          expect(
            node.getSemanticsData().hasAction(SemanticsAction.tap),
            isTrue,
          );
          tester
              .renderObject(button)
              .owner!
              .semanticsOwner!
              .performAction(node.id, SemanticsAction.tap);
          await tester.pump();
          expect(actions, 2);
          await tester.tap(button);
          expect(actions, 3);
          await tester.tap(find.byType(TextField));
          await tester.enterText(find.byType(TextField), 'Explicit edit');
          expect(editorFocus.hasFocus, isTrue);
          expect(edits, 1);
          expect(editor.text, 'Explicit edit');
          await tester.pumpWidget(const SizedBox());
        } finally {
          semantics.dispose();
        }
      },
    );

    testWidgets('a delayed tick settles the whole finite sequence', (
      tester,
    ) async {
      await tester.pumpWidget(_harness());
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 100));
      _expectTimeline(tester, 100);
      await tester.pump(const Duration(seconds: 5));
      _expectTimeline(tester, 620);
      _expectGate(tester, blocked: false);
      await tester.pump(const Duration(seconds: 5));
      expect(tester.binding.transientCallbackCount, 0);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    });

    testWidgets(
      'disposal before the first tick and mid-sequence cancels work',
      (tester) async {
        for (final elapsed in [0, 250]) {
          await tester.pumpWidget(_harness());
          if (elapsed != 0) {
            await tester.pump();
            await tester.pump(Duration(milliseconds: elapsed));
            _expectTimeline(tester, elapsed);
          }
          await tester.pumpWidget(const SizedBox());
          await tester.pump(const Duration(seconds: 2));
          expect(tester.takeException(), isNull);
          expect(tester.binding.transientCallbackCount, 0);
        }
      },
    );
  });

  group('SmithApp workspace entrance integration', () {
    testWidgets('entrance waits for held Boot, loader outro and restoration', (
      tester,
    ) async {
      final store = restore.MemoryStore()
        ..loadGate = Completer<WorkspacePreferences?>();
      final api = FakeAppApi();
      final control = FakeControlApi(profile())
        ..pendingInspect = Completer<ControlReply>();
      await _mountApp(tester, store, api, control);
      await _flush(tester);
      expect(store.loads, 0);
      expect(api.calls, isEmpty);
      expect(find.byType(WorkspaceEntrance), findsNothing);
      await tester.pump(bootIntroDuration);
      await tester.pump(bootMinimumHoldDuration);
      expect(find.byType(ConversationScreen), findsNothing);
      expect(store.loads, 0);
      control.pendingInspect!.complete(reply(profile()));
      await _flush(tester);
      expect(api.calls, ['state', 'conversation_get', 'models', 'poll']);
      await tester.pump(const Duration(milliseconds: 499));
      expect(find.byType(ConversationScreen), findsNothing);
      expect(find.byType(WorkspaceEntrance), findsNothing);
      expect(store.loads, 0);
      await tester.pump(const Duration(milliseconds: 1));
      await _flush(tester);
      expect(find.text('Restoring workspace…'), findsOneWidget);
      expect(store.loads, 1);
      expect(find.byType(SplitWorkspace), findsNothing);
      expect(find.byType(WorkspaceEntrance), findsNothing);
      final callsBeforeRestore = List.of(api.calls);
      await tester.pump(const Duration(seconds: 2));
      expect(api.calls, callsBeforeRestore);
      expect(store.writes, isEmpty);
      store.loadGate!.complete(restore.preferences(preset: '1x2', active: 1));
      await _flush(tester);
      expect(find.byKey(const ValueKey('boot-surface')), findsNothing);
      expect(find.text('Restoring workspace…'), findsNothing);
      expect(find.byType(WorkspaceEntrance), findsOneWidget);
      expect(restore.presentation(tester).layout, WorkspaceLayout.stacked);
      expect(restore.presentation(tester).active, 1);
      expect(_inTile(1, find.byType(SettingsView)), findsOneWidget);
      _expectTimeline(tester, 0, requireEveryRegion: false);
      _expectGate(tester, blocked: true);
      final callsBeforeEntrance = List.of(api.calls);
      await tester.pump(const Duration(milliseconds: 340));
      _expectTimeline(tester, 340, requireEveryRegion: false);
      await tester.pump(const Duration(milliseconds: 280));
      _expectAppSettled(tester);
      expect(api.calls, callsBeforeEntrance);
      expect(store.loads, 1);
      expect(store.writes, isEmpty);
      _expectNoEffects(control, api);
      await tester.pumpWidget(const SizedBox());
      await _flush(tester);
      expect(store.closes, 1);
      expect(api.disposedCount, 1);
      expect(control.disposed, isTrue);
    });

    for (final preset in ['1x1', '2x1', '1x2', '2x2']) {
      testWidgets('$preset restored Chat/Settings panes share one timeline', (
        tester,
      ) async {
        final count = int.parse(preset[0]) * int.parse(preset[2]);
        final saved = restore.preferences(preset: preset, active: count - 1);
        final store = restore.MemoryStore()
          ..loadGate = Completer<WorkspacePreferences?>();
        final api = FakeAppApi();
        final control = FakeControlApi(profile());
        await _holdAtRestoration(tester, store, api, control);
        store.loadGate!.complete(saved);
        await _flush(tester);
        expect(
          find.byType(WorkspaceEntrance, skipOffstage: false),
          findsOneWidget,
        );
        expect(find.byType(SmithAppHeader), findsOneWidget);
        expect(_opacity(WorkspaceRegion.header), findsOneWidget);
        expect(_opacity(WorkspaceRegion.navigation), findsNWidgets(count));
        expect(_opacity(WorkspaceRegion.title), findsNWidgets(count));
        expect(
          _opacity(WorkspaceRegion.sidebar),
          findsNWidgets((count + 1) ~/ 2),
        );
        expect(
          _opacity(WorkspaceRegion.transcript),
          findsNWidgets((count + 1) ~/ 2),
        );
        expect(
          _opacity(WorkspaceRegion.composer),
          findsNWidgets((count + 1) ~/ 2),
        );
        expect(_opacity(WorkspaceRegion.settings), findsNWidgets(count ~/ 2));
        expect(
          restore.presentation(tester).layout.label,
          preset.replaceAll('x', '×'),
        );
        expect(restore.presentation(tester).active, count - 1);
        expect(restore.owner(tester).panes, hasLength(4));
        for (var tile = 0; tile < 4; tile++) {
          expect(
            restore.presentation(tester).pages[tile],
            saved.slots[tile].page,
          );
          expect(
            tester.widget<TextField>(_editor(tile)).focusNode!.hasFocus,
            isFalse,
          );
        }
        final entrance = tester.state(find.byType(WorkspaceEntrance));
        final wrappers = [
          for (final region in WorkspaceRegion.values)
            ..._opacity(region).evaluate(),
        ];
        _expectTimeline(tester, 0, requireEveryRegion: false);
        final calls = List.of(api.calls);
        await tester.pump(const Duration(milliseconds: 300));
        _expectTimeline(tester, 300, requireEveryRegion: false);
        _expectGate(tester, blocked: true);
        await tester.pump(const Duration(milliseconds: 319));
        _expectTimeline(tester, 619, requireEveryRegion: false);
        _expectGate(tester, blocked: true);
        await tester.pump(const Duration(milliseconds: 1));
        _expectAppSettled(tester);
        expect(tester.state(find.byType(WorkspaceEntrance)), same(entrance));
        expect([
          for (final region in WorkspaceRegion.values)
            ..._opacity(region).evaluate(),
        ], orderedEquals(wrappers));
        expect(api.calls, calls);
        expect(store.writes, isEmpty);
        expect(
          store.value,
          isNull,
          reason: 'hydration/animation must not save defaults',
        );
        _expectNoEffects(control, api);
      });
    }

    testWidgets(
      'status, Settings, resize and layout never replay or remount editors',
      (tester) async {
        final saved = restore.preferences(
          ids: List.filled(4, testConversation),
        );
        final store = restore.MemoryStore()
          ..loadGate = Completer<WorkspacePreferences?>();
        final api = FakeAppApi();
        for (var i = 0; i < 24; i++) {
          api.history.add({
            'role': i.isEven ? 'user' : 'assistant',
            'content':
                'Retained history $i\nA second line for reading position',
          });
        }
        final control = FakeControlApi(profile());
        await _holdAtRestoration(tester, store, api, control);
        store.loadGate!.complete(saved);
        await _flush(tester);
        final entrance = tester.state(find.byType(WorkspaceEntrance));
        final owner = restore.owner(tester);
        final panels = [
          for (var tile = 0; tile < 4; tile++)
            tester.state(
              _inTile(tile, find.byType(ChatPanel, skipOffstage: false)),
            ),
        ];
        final editors = [
          for (var tile = 0; tile < 4; tile++)
            tester.widget<TextField>(_editor(tile)).controller!,
        ];
        final focusNodes = [
          for (var tile = 0; tile < 4; tile++)
            tester.widget<TextField>(_editor(tile)).focusNode!,
        ];
        final transcripts = [
          for (var tile = 0; tile < 4; tile++)
            tester
                .widget<SingleChildScrollView>(
                  _inTile(
                    tile,
                    find.byKey(
                      const ValueKey('conversation-history'),
                      skipOffstage: false,
                    ),
                  ),
                )
                .controller!,
        ];
        void retained() {
          expect(tester.state(find.byType(WorkspaceEntrance)), same(entrance));
          expect(restore.owner(tester), same(owner));
          for (var tile = 0; tile < 4; tile++) {
            expect(
              tester.state(
                _inTile(tile, find.byType(ChatPanel, skipOffstage: false)),
              ),
              same(panels[tile]),
            );
            final editor = tester.widget<TextField>(_editor(tile));
            expect(editor.controller, same(editors[tile]));
            expect(editor.focusNode, same(focusNodes[tile]));
            expect(editor.controller!.text, 'Unsent presentation-only draft');
            expect(
              tester
                  .widget<SingleChildScrollView>(
                    _inTile(
                      tile,
                      find.byKey(
                        const ValueKey('conversation-history'),
                        skipOffstage: false,
                      ),
                    ),
                  )
                  .controller,
              same(transcripts[tile]),
            );
          }
        }

        owner.panes[0].selected.edit('Unsent presentation-only draft');
        await tester.pump(const Duration(milliseconds: 300));
        api.summaries[testConversation]!['title'] = 'Observed updated title';
        await owner.synchronize();
        await _flush(tester);
        _expectTimeline(tester, 300, requireEveryRegion: false);
        retained();
        await tester.pump(const Duration(milliseconds: 320));
        _expectAppSettled(tester);
        retained();
        expect(focusNodes.every((node) => !node.hasFocus), isTrue);
        expect(store.writes, isEmpty);
        editors[0].selection = const TextSelection(
          baseOffset: 2,
          extentOffset: 8,
        );
        transcripts[0].jumpTo(100);
        await tester.pump();
        final readingOffset = transcripts[0].offset;

        api.stateFailure = true;
        await owner.synchronize();
        await _flush(tester);
        expect(
          find.descendant(
            of: find.byType(SmithAppHeader),
            matching: find.text('SYSTEM UNAVAILABLE'),
          ),
          findsOneWidget,
        );
        _expectAppSettled(tester);
        retained();
        api.stateFailure = false;
        await owner.synchronize();
        await _flush(tester);
        expect(
          find.descendant(
            of: find.byType(SmithAppHeader),
            matching: find.text('SYSTEM ONLINE'),
          ),
          findsOneWidget,
        );
        _expectAppSettled(tester);
        expect(store.writes, isEmpty);

        await tester.tap(_inTile(0, find.byTooltip('Settings')));
        await _flush(tester);
        _expectAppSettled(tester);
        retained();
        expect(_inTile(0, find.byType(SettingsView)), findsOneWidget);
        expect(store.writes, hasLength(1));
        await tester.tap(_inTile(0, find.byTooltip('Return to Chat')));
        await _flush(tester);
        _expectAppSettled(tester);
        retained();
        expect(
          editors[0].selection,
          const TextSelection(baseOffset: 2, extentOffset: 8),
        );
        expect(transcripts[0].offset, closeTo(readingOffset, 0.01));
        expect(store.writes, hasLength(2));

        tester.view.physicalSize = const Size(1100, 900);
        await _flush(tester);
        _expectAppSettled(tester);
        retained();
        expect(store.writes, hasLength(2));
        await _chooseLayout(tester, WorkspaceLayout.single);
        retained();
        expect(store.writes, hasLength(3));
        expect(store.value!.layout, '1x1');
        await _chooseLayout(tester, WorkspaceLayout.grid);
        retained();
        expect(store.writes, hasLength(4));
        tester.view.physicalSize = const Size(2200, 1400);
        await _flush(tester);
        _expectAppSettled(tester);
        retained();
        expect(_opacity(WorkspaceRegion.sidebar), findsNWidgets(2));
        expect(store.writes, hasLength(4));
        expect(store.value!.toJson(), saved.toJson());
        expect(jsonEncode(store.value!.toJson()), isNot(contains('Unsent')));
        expect(store.loads, 1);
        _expectNoEffects(control, api);
        expect(tester.takeException(), isNull);
      },
    );

    testWidgets('restoration warning bypasses entrance on its first frame', (
      tester,
    ) async {
      final semantics = tester.ensureSemantics();
      try {
        final store = restore.MemoryStore()
          ..loadGate = Completer<WorkspacePreferences?>();
        final api = FakeAppApi();
        final control = FakeControlApi(profile());
        await _holdAtRestoration(tester, store, api, control);
        store.loadGate!.complete(
          restore.preferences(
            preset: '1x1',
            ids: List.filled(4, 'ffffffffffffffffffffffffffffffff'),
          ),
        );
        await _flush(tester);
        _expectAppSettled(tester);
        expect(find.textContaining('missing or archived'), findsOneWidget);
        expect(
          find.bySemanticsLabel(RegExp('Some saved chats are missing')),
          findsOneWidget,
        );
        expect(
          find.byKey(const ValueKey('workspace-layout')).hitTestable(),
          findsOneWidget,
        );
        expect(
          store.writes,
          hasLength(1),
          reason: 'only the existing selection repair is saved',
        );
        expect(
          store.value!.slots.every((slot) => slot.conversationId == null),
          isTrue,
        );
        await tester.pump(const Duration(milliseconds: 300));
        _expectAppSettled(tester);
        expect(store.writes, hasLength(1));
        _expectNoEffects(control, api);
      } finally {
        semantics.dispose();
      }
    });

    testWidgets(
      'mid-entrance outage exposes recovery immediately and never replays',
      (tester) async {
        final store = restore.MemoryStore()
          ..loadGate = Completer<WorkspacePreferences?>();
        final api = FakeAppApi();
        final control = FakeControlApi(profile());
        await _holdAtRestoration(tester, store, api, control);
        store.loadGate!.complete(restore.preferences(preset: '1x1'));
        await _flush(tester);
        final entrance = tester.state(find.byType(WorkspaceEntrance));
        await tester.pump(const Duration(milliseconds: 200));
        _expectTimeline(tester, 200, requireEveryRegion: false);
        api.stateFailure = true;
        await restore.owner(tester).synchronize();
        await _flush(tester);
        _expectAppSettled(tester);
        expect(find.text('Retry observation').hitTestable(), findsOneWidget);
        api.stateFailure = false;
        await restore.owner(tester).synchronize();
        await _flush(tester);
        _expectAppSettled(tester);
        expect(tester.state(find.byType(WorkspaceEntrance)), same(entrance));
        expect(
          tester.widget<TextField>(_editor(0)).focusNode!.hasFocus,
          isFalse,
        );
        expect(store.writes, isEmpty);
        _expectNoEffects(control, api);
      },
    );
  });

  group('selected-route entrance recovery', () {
    for (final historical in [
      cloud_fakes.cloud,
      const ChatRoute('anthropic', 'claude-sonnet-5-5', cloud_fakes.generation),
    ]) {
      test(
        '${historical.provider} replacement keeps every active choice eligible and only the old generation unavailable',
        () async {
          final api = _AvailabilityCloudFake();
          final before = RouteCatalog(
            (await api.call('route_catalog', {})).result,
          );
          final previous = before.providers.firstWhere(
            (p) => p['provider'] == historical.provider,
          );
          expect(previous['validation']['status'], 'succeeded');
          final replacement = api.replaceCredential(historical);
          final reply = await api.call('route_catalog', {});
          final after = RouteCatalog(reply.result);
          final current = after.providers.firstWhere(
            (p) => p['provider'] == historical.provider,
          );
          expect(current['revision'], previous['revision'] + 1);
          expect(current['credential_generation'], replacement.generation);
          expect(current['credential_state'], 'available');
          expect(current['credential_error'], isNull);
          expect(current['validation'], isNull);
          expect(after.available(historical), isFalse);
          expect(after.available(replacement), isTrue);
          final choices = after.choices.where(
            (c) => c.route.provider == historical.provider,
          );
          expect(
            choices.length,
            before.choices
                .where((c) => c.route.provider == historical.provider)
                .length,
          );
          expect(
            choices.every(
              (c) =>
                  c.route.generation == replacement.generation &&
                  c.available &&
                  c.reason == null &&
                  !c.validated,
            ),
            isTrue,
          );
          expect(
            after.providers.where((p) => p['provider'] != historical.provider),
            before.providers.where((p) => p['provider'] != historical.provider),
          );
          expect(
            after.available(cloud_fakes.local),
            before.available(cloud_fakes.local),
          );
          expect(
            (await api.call('route_catalog', {})).result,
            reply.result,
            reason: 'Cached queries must not create further credential replacements or revisions.',
          );
          expect(api.calls, [
            'route_catalog',
            'route_catalog',
            'route_catalog',
          ]);
          expect(api.accepted, isEmpty);
          expect(api.jobs, isEmpty);
        },
      );
    }
    for (final historicalRoute in [
      const ChatRoute('local', 'local:retired', null),
      const ChatRoute(
        'openai',
        'gpt-6-astra',
        'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',
      ),
    ]) {
      testWidgets(
        'historical ${historicalRoute.provider} route unavailable on the first frame bypasses entrance',
        (tester) async {
          final api = cloud_fakes.CloudFake();
          api.lastRoutes[testConversation] = historicalRoute.toJson();
          api.history.addAll([
            {'role': 'user', 'content': 'Previously saved question'},
            {'role': 'assistant', 'content': 'Previously saved answer'},
          ]);
          api.summaries[testConversation]!['history_revision'] = 1;
          final owner = await cloud_fakes.readyCloud(api);
          final control = FakeControlApi(profile());
          final store = restore.MemoryStore(
            restore.preferences(
              preset: '1x1',
              ids: const [testConversation, null, null, null],
            ),
          );
          await _mountRoutedWorkspace(tester, owner, control, store);
          final selected = owner.selected;
          _expectRouteState(selected, ready: false);
          expect(selected.routing!.lastRoute, historicalRoute);
          expect(selected.selectedRoute, historicalRoute);
          expect(selected.routeChosen, isFalse);
          expect(selected.history, api.history);
          _expectAppSettled(tester);
          final selector = find.byKey(const ValueKey('chat-route-selector'));
          expect(selector.hitTestable(), findsOneWidget);
          expect(
            tester.widget<TextField>(_editor(0)).focusNode!.hasFocus,
            isFalse,
          );
          final entrance = tester.state(find.byType(WorkspaceEntrance));
          final calls = List.of(api.calls);
          final routeRevision = selected.routeRevision;
          final draft = selected.draft;
          final queueRevision = selected.messageQueue.revision;
          await tester.tap(selector);
          await tester.pumpAndSettle();
          expect(find.text('Choose model'), findsOneWidget);
          final retainedSelection = find.descendant(
            of: find.byKey(const ValueKey('chat-route-current')),
            matching: find.text(
              'Current: ${historicalRoute.cloud ? 'CLOUD' : 'LOCAL'} · ${historicalRoute.label} · unavailable',
            ),
          );
          expect(retainedSelection, findsOneWidget);
          await tester.enterText(
            find.byKey(const ValueKey('chat-route-filter')),
            'LOCAL',
          );
          await tester.pumpAndSettle();
          expect(retainedSelection, findsOneWidget);
          _expectRouteState(selected, ready: false);
          expect(selected.routing!.lastRoute, historicalRoute);
          expect(selected.selectedRoute, historicalRoute);
          expect(selected.routeChosen, isFalse);
          expect(selected.routeRevision, routeRevision);
          expect(selected.history, api.history);
          expect(selected.draft, draft);
          expect(selected.messageQueue.revision, queueRevision);
          expect(api.calls, calls);
          final replacement = find.byKey(const ValueKey(cloud_fakes.local));
          final replacementLabel = find
              .descendant(of: replacement, matching: find.byType(Text))
              .first;
          await tester.ensureVisible(replacementLabel);
          await tester.pumpAndSettle();
          expect(replacementLabel.hitTestable(), findsOneWidget);
          await tester.tap(replacementLabel);
          await tester.pumpAndSettle();
          expect(find.text('Choose model'), findsNothing);
          expect(selected.selectedRoute, cloud_fakes.local);
          expect(selected.routing!.lastRoute, historicalRoute);
          expect(selected.routeChosen, isTrue);
          expect(selected.routeRevision, routeRevision + 1);
          expect(selected.draft, draft);
          expect(selected.messageQueue.revision, queueRevision);
          _expectRouteState(selected, ready: true);
          _expectAppSettled(tester);
          await tester.pump(const Duration(milliseconds: 300));
          _expectAppSettled(tester);
          expect(tester.state(find.byType(WorkspaceEntrance)), same(entrance));
          expect(selected.history, api.history);
          expect(api.calls, calls);
          _expectCloudReadOnly(api, control, store);
        },
      );
    }

    for (final route in [cloud_fakes.local, cloud_fakes.cloud]) {
      testWidgets(
        'selected ${route.provider} route becoming unavailable mid-entrance settles permanently',
        (tester) async {
          final api = _AvailabilityCloudFake();
          api.lastRoutes[testConversation] = route.toJson();
          final owner = await cloud_fakes.readyCloud(api);
          final control = FakeControlApi(profile());
          final store = restore.MemoryStore(
            restore.preferences(
              preset: '1x1',
              ids: const [testConversation, null, null, null],
            ),
          );
          await _mountRoutedWorkspace(tester, owner, control, store);
          final selected = owner.selected;
          final history = List.of(selected.history);
          final routeRevision = selected.routeRevision;
          _expectRouteState(selected, ready: true);
          _expectTimeline(tester, 0, requireEveryRegion: false);
          _expectGate(tester, blocked: true);
          final entrance = tester.state(find.byType(WorkspaceEntrance));
          final panel = tester.state(_inTile(0, find.byType(ChatPanel)));
          final editor = tester.widget<TextField>(_editor(0)).controller;
          await tester.pump();
          await tester.pump(const Duration(milliseconds: 200));
          _expectTimeline(tester, 200, requireEveryRegion: false);
          final replacement = route.cloud ? api.replaceCredential(route) : null;
          if (!route.cloud) api.unavailable.add(route);
          await owner.synchronize();
          await tester.pump();
          _expectRouteState(selected, ready: false);
          expect(
            selected.selectedRoute,
            route,
            reason: 'no automatic fallback',
          );
          expect(selected.routing!.lastRoute, route);
          expect(selected.routeRevision, routeRevision);
          expect(selected.history, history);
          _expectAppSettled(tester);
          expect(
            find.byKey(const ValueKey('chat-route-selector')).hitTestable(),
            findsOneWidget,
          );
          if (replacement == null) {
            api.unavailable.remove(route);
            await owner.synchronize();
            await tester.pump();
          } else {
            expect(owner.routeCatalog!.available(replacement), isTrue);
            expect(
              owner.routeCatalog!.choices
                  .where((c) => c.route.provider == route.provider)
                  .every((c) => !c.validated),
              isTrue,
            );
            await owner.synchronize();
            await tester.pump();
            _expectRouteState(selected, ready: false);
            expect(
              selected.selectedRoute,
              route,
              reason: 'A replacement never silently retargets the historical selection.',
            );
            final callsBeforeSelection = List.of(api.calls);
            selected.selectRoute(replacement);
            await tester.pump();
            expect(selected.routeChosen, isTrue);
            expect(selected.routeRevision, routeRevision + 1);
            expect(api.calls, callsBeforeSelection);
          }
          _expectRouteState(selected, ready: true);
          expect(selected.selectedRoute, replacement ?? route);
          expect(selected.routing!.lastRoute, route);
          expect(selected.history, history);
          _expectAppSettled(tester);
          final calls = List.of(api.calls);
          await tester.pump(const Duration(milliseconds: 100));
          _expectAppSettled(tester);
          await tester.pump(const Duration(seconds: 1));
          _expectAppSettled(tester);
          expect(tester.state(find.byType(WorkspaceEntrance)), same(entrance));
          expect(tester.state(_inTile(0, find.byType(ChatPanel))), same(panel));
          expect(tester.widget<TextField>(_editor(0)).controller, same(editor));
          expect(
            tester.widget<TextField>(_editor(0)).focusNode!.hasFocus,
            isFalse,
          );
          expect(api.calls, calls);
          _expectCloudReadOnly(api, control, store);
        },
      );

      testWidgets(
        'healthy selected ${route.provider} route still animates when an unselected optional destination is unavailable',
        (tester) async {
          const optional = ChatRoute(
            'anthropic',
            'claude-sonnet-5-5',
            cloud_fakes.generation,
          );
          final api = _AvailabilityCloudFake();
          final replacement = api.replaceCredential(optional);
          api.lastRoutes[testConversation] = route.toJson();
          final owner = await cloud_fakes.readyCloud(api);
          final control = FakeControlApi(profile());
          final store = restore.MemoryStore(
            restore.preferences(
              preset: '1x1',
              ids: const [testConversation, null, null, null],
            ),
          );
          await _mountRoutedWorkspace(tester, owner, control, store);
          expect(owner.routeCatalog!.available(optional), isFalse);
          expect(owner.routeCatalog!.available(replacement), isTrue);
          expect(
            owner.routeCatalog!.choices
                .where((c) => c.route.provider == optional.provider)
                .every(
                  (c) =>
                      c.available &&
                      !c.validated &&
                      c.route.generation == replacement.generation,
                ),
            isTrue,
          );
          expect(owner.selected.selectedRoute, route);
          _expectRouteState(owner.selected, ready: true);
          expect(
            tester
                .widget<WorkspaceEntrance>(find.byType(WorkspaceEntrance))
                .skipMotion,
            isFalse,
          );
          _expectTimeline(tester, 0, requireEveryRegion: false);
          _expectGate(tester, blocked: true);
          final calls = List.of(api.calls);
          await tester.pump();
          await tester.pump(const Duration(milliseconds: 200));
          _expectTimeline(tester, 200, requireEveryRegion: false);
          _expectGate(tester, blocked: true);
          await tester.pump(const Duration(milliseconds: 420));
          _expectAppSettled(tester);
          expect(owner.selected.selectedRoute, route);
          expect(owner.routeCatalog!.available(optional), isFalse);
          expect(owner.routeCatalog!.available(replacement), isTrue);
          expect(api.calls, calls);
          _expectCloudReadOnly(api, control, store);
        },
      );
    }

    testWidgets(
      'a hidden unavailable historical route does not suppress the visible healthy pane',
      (tester) async {
        const historical = ChatRoute(
          'openai',
          'gpt-6-astra',
          'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',
        );
        final api = cloud_fakes.CloudFake();
        api.lastRoutes[testConversation] = historical.toJson();
        final owner = await cloud_fakes.readyCloud(api);
        final control = FakeControlApi(profile());
        final store = restore.MemoryStore(
          restore.preferences(
            preset: '1x1',
            ids: const [null, testConversation, null, null],
          ),
        );
        final presentation = await _mountRoutedWorkspace(
          tester,
          owner,
          control,
          store,
        );
        expect(presentation.layout, WorkspaceLayout.single);
        expect(owner.panes, hasLength(4));
        expect(owner.panes[0].selected.id, isNull);
        expect(owner.panes[0].selected.selectedRoute, cloud_fakes.local);
        _expectRouteState(owner.panes[0].selected, ready: true);
        final hidden = owner.panes[1].selected;
        expect(hidden.id, testConversation);
        expect(hidden.selectedRoute, historical);
        _expectRouteState(hidden, ready: false);
        expect(find.byKey(const ValueKey('workspace-tile-1')), findsNothing);
        expect(
          tester
              .widget<WorkspaceEntrance>(find.byType(WorkspaceEntrance))
              .skipMotion,
          isFalse,
        );
        _expectTimeline(tester, 0, requireEveryRegion: false);
        _expectGate(tester, blocked: true);
        final calls = List.of(api.calls);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 200));
        _expectTimeline(tester, 200, requireEveryRegion: false);
        _expectGate(tester, blocked: true);
        await tester.pump(const Duration(milliseconds: 420));
        _expectAppSettled(tester);
        _expectRouteState(hidden, ready: false);
        expect(api.calls, calls);
        _expectCloudReadOnly(api, control, store);
      },
    );
  });
}
