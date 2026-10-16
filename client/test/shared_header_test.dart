import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/main.dart';
import 'package:smith_client/ui/smith_theme.dart';
import 'package:smith_client/ui/smith_app_header.dart';
import 'package:smith_client/ui/smith_workspace.dart';
import 'package:smith_client/chat_panel.dart';
import 'package:smith_client/workspace_controller.dart';
import 'package:smith_client/ui/split_workspace.dart';

import 'app_fakes.dart';
import 'split_workspace_widget_test.dart' show tile, inside, editor;
import 'widget_test.dart' show FakeControlApi, profile;
import 'workspace_restore_test.dart' show MemoryStore;

Finder get trigger => find.byKey(const ValueKey('workspace-layout'));
Finder get popup => find.byType(PopupMenuButton<WorkspaceLayout>);
Finder option(WorkspaceLayout value) => find.byWidgetPredicate(
  (w) => w is CheckedPopupMenuItem<WorkspaceLayout> && w.value == value,
);
InkWell triggerInk(WidgetTester tester) => tester.widget<InkWell>(
  find.descendant(of: trigger, matching: find.byType(InkWell)).last,
);
Set<WidgetState> triggerStates(WidgetTester tester) =>
    triggerInk(tester).statesController!.value;
Color? overlay(WidgetTester tester, Set<WidgetState> states) {
  final button = tester.widget<IconButton>(
    find.descendant(of: popup, matching: find.byType(IconButton)).first,
  );
  return (button.style?.overlayColor ??
          SmithTheme.dark().iconButtonTheme.style!.overlayColor)!
      .resolve(states);
}

Future<void> mount(WidgetTester tester, {MemoryStore? store}) async {
  tester.view.physicalSize = const Size(1600, 1000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(
    SmithApp(
      preferencesStore: store,
      controlApi: FakeControlApi(profile()),
      appApi: FakeAppApi()..inventoryObserved = true,
    ),
  );
  await tester.pumpAndSettle();
}

Future<TestGesture> mouseAt(WidgetTester tester, Offset point) async {
  final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
  await mouse.addPointer(location: point);
  await tester.pumpAndSettle();
  return mouse;
}

Future<void> click(
  WidgetTester tester,
  TestGesture mouse,
  Finder target,
) async {
  await mouse.moveTo(tester.getCenter(target));
  await mouse.down(tester.getCenter(target));
  await mouse.up();
  await tester.pumpAndSettle();
}

WorkspacePresentation presentation(WidgetTester tester) => tester
    .widget<ConversationScreen>(find.byType(ConversationScreen))
    .presentation!;

void main() {
  for (final remove in [false, true]) {
    testWidgets('mouse ${remove ? 'removal' : 'window exit'} clears hover', (
      tester,
    ) async {
      await mount(tester);
      final mouse = await mouseAt(tester, tester.getCenter(trigger));
      expect(triggerStates(tester), contains(WidgetState.hovered));
      if (remove) {
        await mouse.removePointer();
      } else {
        await mouse.moveTo(const Offset(1601, -1));
      }
      await tester.pumpAndSettle();
      expect(triggerStates(tester), isNot(contains(WidgetState.hovered)));
      expect(triggerStates(tester), isNot(contains(WidgetState.pressed)));
      if (!remove) await mouse.removePointer();
      await tester.pumpWidget(const SizedBox());
    });
  }

  for (final cancel in [false, true]) {
    testWidgets(
      'mouse menu ${cancel ? 'cancel' : 'select'} preserves active pane and trigger',
      (tester) async {
        final store = MemoryStore();
        await mount(tester, store: store);
        final mouse = await mouseAt(tester, tester.getCenter(trigger));
        await click(tester, mouse, trigger);
        await click(tester, mouse, option(WorkspaceLayout.grid));
        await click(tester, mouse, editor(1));
        expect(presentation(tester).active, 1);
        final state = tester.state(popup);
        final writesBefore = store.writes.length;
        await click(tester, mouse, trigger);
        expect(presentation(tester).active, 1);
        if (cancel) {
          await mouse.moveTo(const Offset(20, 200));
          await mouse.down(const Offset(20, 200));
          await mouse.up();
          await tester.pumpAndSettle();
        } else {
          await click(tester, mouse, option(WorkspaceLayout.sideBySide));
        }
        await mouse.moveTo(const Offset(1601, -1));
        await tester.pumpAndSettle();
        expect(tester.state(popup), same(state));
        expect(presentation(tester).active, 1);
        expect(
          presentation(tester).layout,
          cancel ? WorkspaceLayout.grid : WorkspaceLayout.sideBySide,
        );
        expect(store.writes.length - writesBefore, cancel ? 0 : 1);
        expect(triggerStates(tester), isNot(contains(WidgetState.hovered)));
        expect(triggerStates(tester), isNot(contains(WidgetState.pressed)));
        expect(overlay(tester, triggerStates(tester)), Colors.transparent);
        await mouse.removePointer();
        await tester.pumpWidget(const SizedBox());
      },
    );
  }

  testWidgets(
    'keyboard has one trigger stop, visible focus and native menu navigation',
    (tester) async {
      await mount(tester);
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pumpAndSettle();
      expect(triggerStates(tester), contains(WidgetState.focused));
      expect(
        find.descendant(of: trigger, matching: find.byType(InkWell)),
        findsOneWidget,
      );
      final button = tester.widget<IconButton>(
        find.descendant(of: trigger, matching: find.byType(IconButton)),
      );
      final side =
          (button.style?.side ?? SmithTheme.dark().iconButtonTheme.style!.side)!
              .resolve({WidgetState.focused})!;
      expect(side.color, SmithColors.accentBright);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(option(WorkspaceLayout.single), findsOneWidget);
      // The native popup initially focuses its route; Down enters item one.
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.pumpAndSettle();
      expect(
        FocusManager.instance.primaryFocus!.context!
            .findAncestorWidgetOfExactType<
              CheckedPopupMenuItem<WorkspaceLayout>
            >()!
            .value,
        WorkspaceLayout.single,
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.pumpAndSettle();
      expect(
        FocusManager.instance.primaryFocus!.context!
            .findAncestorWidgetOfExactType<
              CheckedPopupMenuItem<WorkspaceLayout>
            >()!
            .value,
        WorkspaceLayout.sideBySide,
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(presentation(tester).layout, WorkspaceLayout.sideBySide);
      expect(triggerStates(tester), contains(WidgetState.focused));
      await tester.sendKeyEvent(LogicalKeyboardKey.space);
      await tester.pumpAndSettle();
      expect(option(WorkspaceLayout.single), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(triggerStates(tester), contains(WidgetState.focused));
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pumpAndSettle();
      expect(triggerStates(tester), isNot(contains(WidgetState.focused)));
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'menu uses current callback after status rebuild and ignores completion after disposal',
    (tester) async {
      final owner = WorkspaceController(FakeAppApi());
      var visible = true;
      var online = true;
      var replacement = false;
      final calls = <String>[];
      late StateSetter update;
      await tester.pumpWidget(
        MaterialApp(
          theme: SmithTheme.dark(),
          home: StatefulBuilder(
            builder: (context, setState) {
              update = setState;
              return Scaffold(
                body: visible
                    ? SplitWorkspace(
                        controller: owner,
                        onPresentationChanged: replacement
                            ? (layout, active) => calls.add('current')
                            : (layout, active) => calls.add('old'),
                        headerBuilder: (context, menu) => SmithAppHeader(
                          connectionStatus: online
                              ? 'SYSTEM ONLINE'
                              : 'SYSTEM UNAVAILABLE',
                          connectionOnline: online,
                          trailing: menu,
                        ),
                        builder: (context, pane, active, activate) =>
                            const SizedBox.expand(),
                      )
                    : const SizedBox(),
              );
            },
          ),
        ),
      );
      await tester.pumpAndSettle();
      final state = tester.state(popup);
      final mouse = await mouseAt(tester, tester.getCenter(trigger));
      await click(tester, mouse, trigger);
      update(() {
        online = false;
        replacement = true;
      });
      await tester.pumpAndSettle();
      expect(find.text('SYSTEM UNAVAILABLE'), findsOneWidget);
      expect(tester.state(popup), same(state));
      await click(tester, mouse, option(WorkspaceLayout.sideBySide));
      expect(calls, ['current']);
      await click(tester, mouse, trigger);
      update(() => visible = false);
      await tester.pumpAndSettle();
      expect(trigger, findsNothing);
      await click(tester, mouse, option(WorkspaceLayout.grid));
      expect(calls, ['current']);
      expect(tester.takeException(), isNull);
      await mouse.removePointer();
      await tester.pumpWidget(const SizedBox());
      owner.dispose();
    },
  );

  testWidgets(
    'all presets retain body identity, neutral borders and a fixed header at 2x',
    (tester) async {
      await mount(tester);
      final mouse = await mouseAt(tester, tester.getCenter(trigger));
      await click(tester, mouse, trigger);
      await click(tester, mouse, option(WorkspaceLayout.grid));
      final bodies = [
        for (var i = 0; i < 4; i++)
          tester.state(inside(i, find.byType(SmithWorkspace))),
      ];
      final panels = [
        for (var i = 0; i < 4; i++)
          tester.state(inside(i, find.byType(ChatPanel))),
      ];
      final editors = [
        for (var i = 0; i < 4; i++)
          tester.widget<TextField>(editor(i)).controller,
      ];
      await click(tester, mouse, inside(1, find.byTooltip('Settings')));
      for (final value in [
        WorkspaceLayout.single,
        WorkspaceLayout.sideBySide,
        WorkspaceLayout.stacked,
        WorkspaceLayout.grid,
      ]) {
        await click(tester, mouse, trigger);
        await click(tester, mouse, option(value));
        expect(find.byType(SmithAppHeader), findsOneWidget);
        expect(find.text('> SMITH'), findsOneWidget);
        for (var i = 0; i < value.count; i++) {
          expect(
            tester.state(inside(i, find.byType(SmithWorkspace))),
            same(bodies[i]),
          );
          expect(inside(i, find.byType(SmithAppHeader)), findsNothing);
          final decoration =
              tester.widget<Container>(tile(i)).decoration! as BoxDecoration;
          expect(
            decoration.border,
            Border.all(color: SmithColors.strokeSubtle),
          );
        }
      }
      await click(tester, mouse, inside(1, find.byTooltip('Return to Chat')));
      for (var i = 0; i < 4; i++) {
        expect(
          tester.state(inside(i, find.byType(ChatPanel))),
          same(panels[i]),
        );
        expect(
          tester.widget<TextField>(editor(i)).controller,
          same(editors[i]),
        );
      }
      final status = find.byWidgetPredicate(
        (w) => w is Semantics && w.properties.label == 'SYSTEM ONLINE',
      );
      expect(tester.getRect(trigger).left - tester.getRect(status).right, 16);
      tester.view.physicalSize = const Size(400, 400);
      tester.platformDispatcher.textScaleFactorTestValue = 2;
      addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
      await tester.pumpAndSettle();
      final headerRect = tester.getRect(find.byType(SmithAppHeader));
      await tester.ensureVisible(editor(3));
      await tester.pumpAndSettle();
      expect(tester.getRect(find.byType(SmithAppHeader)), headerRect);
      expect(tester.getRect(trigger).right, lessThanOrEqualTo(400));
      expect(tester.takeException(), isNull);
      await mouse.removePointer();
      await tester.pumpWidget(const SizedBox());
    },
  );
  testWidgets('production focus feedback is distinct from hover', (
    tester,
  ) async {
    await mount(tester);
    final mouse = await mouseAt(tester, tester.getCenter(trigger));
    expect(triggerStates(tester), contains(WidgetState.hovered));
    await click(tester, mouse, trigger);
    await click(tester, mouse, option(WorkspaceLayout.single));
    await mouse.moveTo(const Offset(20, 200));
    await tester.pumpAndSettle();
    expect(triggerStates(tester), isNot(contains(WidgetState.hovered)));
    expect(triggerStates(tester), isNot(contains(WidgetState.pressed)));
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    expect(
      overlay(tester, {WidgetState.focused}),
      isNot(overlay(tester, {WidgetState.hovered})),
    );
    await mouse.removePointer();
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('one fixed app header above all bodies', (tester) async {
    await mount(tester);
    final mouse = await mouseAt(tester, tester.getCenter(trigger));
    await click(tester, mouse, trigger);
    await click(tester, mouse, option(WorkspaceLayout.grid));
    expect(find.text('> SMITH'), findsOneWidget);
    expect(trigger, findsOneWidget);
    expect(find.text('SYSTEM ONLINE'), findsOneWidget);
    expect(
      tester.getBottomRight(trigger).dy,
      lessThanOrEqualTo(tester.getTopLeft(tile(0)).dy),
    );
    await mouse.removePointer();
    await tester.pumpWidget(const SizedBox());
  });
}
