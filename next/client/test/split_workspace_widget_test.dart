import 'dart:ui' show PointerDeviceKind;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/chat_panel.dart';
import 'package:smith_client/main.dart';
import 'package:smith_client/ui/smith_workspace.dart';
import 'package:smith_client/ui/split_workspace.dart';
import 'package:smith_client/workspace_controller.dart';

import 'app_fakes.dart';
import 'widget_test.dart' show FakeControlApi, profile;

Finder tile(int index) => find.byKey(ValueKey('workspace-tile-$index'));
Finder inside(int index, Finder finder) =>
    find.descendant(of: tile(index), matching: finder);
Finder editor(int index) => inside(index, find.byType(TextField));
ScrollController transcript(WidgetTester tester, int index) => tester
    .widget<SingleChildScrollView>(
      inside(index, find.byKey(const ValueKey('conversation-history'))),
    )
    .controller!;

Future<WorkspaceController> mount(
  WidgetTester tester,
  FakeAppApi api, {
  Size size = const Size(1600, 1000),
  double scale = 1,
  TargetPlatform? platform,
}) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  api.inventoryObserved = true;
  await tester.pumpWidget(
    MaterialApp(
      theme: ThemeData(platform: platform),
      builder: (context, child) => MediaQuery(
        data: MediaQuery.of(context)
            .copyWith(textScaler: TextScaler.linear(scale)),
        child: child!,
      ),
      home: SmithShell(controlApi: FakeControlApi(profile()), appApi: api),
    ),
  );
  await tester.pumpAndSettle();
  return tester.widget<ConversationScreen>(find.byType(ConversationScreen)).chat
      as WorkspaceController;
}

Future<void> layout(WidgetTester tester, int columns, int rows) async {
  await tester.tap(find.byKey(const ValueKey('workspace-layout')));
  await tester.pumpAndSettle();
  final option = find.byWidgetPredicate(
    (widget) =>
        widget is CheckedPopupMenuItem<WorkspaceLayout> &&
        widget.value!.columns == columns &&
        widget.value!.rows == rows,
  );
  await tester.ensureVisible(option);
  await tester.tap(option);
  await tester.pumpAndSettle();
}

void main() {
  testWidgets(
    'Linux overflowing 2x2 canvas has one draggable scrollbar per axis',
    (tester) async {
      final api = FakeAppApi();
      for (var i = 0; i < 32; i++) {
        api.history.add({
          'role': i.isEven ? 'user' : 'assistant',
          'content': 'Retained message $i\nreading position\nlong history',
        });
      }
      await mount(
        tester,
        api,
        size: const Size(600, 500),
        platform: TargetPlatform.linux,
      );
      await layout(tester, 2, 2);
      expect(
        Theme.of(tester.element(find.byType(SplitWorkspace))).platform,
        TargetPlatform.linux,
      );
      final canvasViews = find
          .byType(SingleChildScrollView)
          .evaluate()
          .where(
            (element) =>
                element.findAncestorWidgetOfExactType<SplitWorkspace>() !=
                    null &&
                element.findAncestorWidgetOfExactType<SmithWorkspace>() == null,
          )
          .map((element) => element.widget as SingleChildScrollView)
          .toList();
      expect(canvasViews, hasLength(2));
      final horizontal = canvasViews
          .singleWhere((view) => view.scrollDirection == Axis.horizontal)
          .controller!;
      final vertical = canvasViews
          .singleWhere((view) => view.scrollDirection == Axis.vertical)
          .controller!;
      for (final controller in [horizontal, vertical]) {
        expect(controller.position.maxScrollExtent, greaterThan(0));
        // RawScrollbar includes both explicit Material bars and desktop-injected bars.
        expect(
          find.byWidgetPredicate(
            (widget) =>
                widget is RawScrollbar && widget.controller == controller,
          ),
          findsOneWidget,
        );
      }
      for (var i = 0; i < 4; i++) {
        expect(
          ScrollConfiguration.of(
            tester.element(inside(i, find.byType(SmithWorkspace))),
          ),
          same(
            ScrollConfiguration.of(tester.element(find.byType(SplitWorkspace))),
          ),
        );
        final controller = transcript(tester, i);
        expect(
          find.byWidgetPredicate(
            (widget) =>
                widget is RawScrollbar && widget.controller == controller,
          ),
          findsOneWidget,
        );
      }
      final viewport = tester.getRect(
        find.byWidgetPredicate(
          (widget) => widget is RawScrollbar && widget.controller == vertical,
        ),
      );
      final header = tester.getRect(
        find.byKey(const ValueKey('workspace-layout')),
      );
      final origin = tester.getTopLeft(tile(0));
      final readingOffsets = [
        for (var i = 0; i < 4; i++) transcript(tester, i).offset,
      ];
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(
        location: Offset(viewport.left + 20, viewport.bottom - 4),
      );
      await tester.pumpAndSettle();
      await mouse.down(Offset(viewport.left + 20, viewport.bottom - 4));
      await mouse.moveBy(const Offset(40, 0));
      await tester.pump();
      await mouse.moveBy(const Offset(40, 0));
      await mouse.up();
      await tester.pumpAndSettle();
      expect(
        horizontal.offset,
        inExclusiveRange(0, horizontal.position.maxScrollExtent),
      );
      expect(vertical.offset, 0);
      expect(
        tester.getTopLeft(tile(0)).dx,
        closeTo(origin.dx - horizontal.offset, .01),
      );
      final horizontalOffset = horizontal.offset;

      await mouse.moveTo(Offset(viewport.right - 4, viewport.top + 20));
      await tester.pumpAndSettle();
      await mouse.down(Offset(viewport.right - 4, viewport.top + 20));
      await mouse.moveBy(const Offset(0, 40));
      await tester.pump();
      await mouse.moveBy(const Offset(0, 40));
      await mouse.up();
      await tester.pumpAndSettle();
      expect(
        vertical.offset,
        inExclusiveRange(0, vertical.position.maxScrollExtent),
      );
      expect(horizontal.offset, horizontalOffset);
      expect(
        tester.getTopLeft(tile(0)).dy,
        closeTo(origin.dy - vertical.offset, .01),
      );
      expect(
        tester.getRect(find.byKey(const ValueKey('workspace-layout'))),
        header,
      );
      expect([
        for (var i = 0; i < 4; i++) transcript(tester, i).offset,
      ], readingOffsets);
      expect(api.calls.where((op) => op.startsWith('submit')), isEmpty);
      expect(tester.takeException(), isNull);
      await mouse.removePointer();
      await tester.pumpWidget(const SizedBox());
    },
  );

  for (final hidden in [true, false]) {
    testWidgets(
      'reading bookmark survives ${hidden ? 'pane four hiding' : 'visible resizing'} and clamping',
      (tester) async {
        final api = FakeAppApi();
        for (var i = 0; i < 4; i++) {
          api.history.add({
            'role': i.isEven ? 'user' : 'assistant',
            'content': 'Short retained message $i\nSecond line',
          });
        }
        final w = await mount(tester, api);
        await layout(tester, 2, 2);
        final index = hidden ? 3 : 0;
        await w.selectFor(w.panes[index], testConversation);
        await tester.pumpAndSettle();
        final scroll = transcript(tester, index);
        expect(scroll.position.maxScrollExtent, greaterThan(30));
        if (hidden) {
          expect(scroll.offset, scroll.position.maxScrollExtent);
        }
        final bookmark = scroll.position.maxScrollExtent * .6;
        scroll.jumpTo(bookmark);
        await tester.pumpAndSettle();
        final panel = tester.state(inside(index, find.byType(ChatPanel)));
        for (var cycle = 0; cycle < 2; cycle++) {
          if (hidden) {
            await layout(tester, 1, 1);
          } else {
            tester.view.physicalSize = const Size(1600, 1800);
            await tester.pumpAndSettle();
          }
          expect(scroll.position.maxScrollExtent, 0);
          expect(scroll.offset, 0);
          w.changed();
          await tester.pumpAndSettle();
          if (hidden) {
            await layout(tester, 2, 2);
          } else {
            tester.view.physicalSize = const Size(1600, 1000);
            await tester.pumpAndSettle();
          }
          expect(
            tester.state(inside(index, find.byType(ChatPanel))),
            same(panel),
          );
          expect(scroll.offset, closeTo(bookmark, .01));
        }
        // Navigating while clamped must preserve the old conversation's
        // desired bookmark without applying it to the new blank.
        tester.view.physicalSize = const Size(1600, 1800);
        await tester.pumpAndSettle();
        expect(scroll.position.maxScrollExtent, 0);
        w.newChatFor(w.panes[index]);
        await tester.pumpAndSettle();
        expect(scroll.offset, 0);
        await w.selectFor(w.panes[index], testConversation);
        await tester.pumpAndSettle();
        expect(scroll.offset, 0);
        tester.view.physicalSize = const Size(1600, 1000);
        await tester.pumpAndSettle();
        expect(scroll.offset, closeTo(bookmark, .01));

        // A real reader gesture in a partially clamped viewport replaces the
        // bookmark. Later geometry restoration must not fight that choice.
        tester.view.physicalSize = const Size(1600, 1150);
        await tester.pumpAndSettle();
        expect(scroll.position.maxScrollExtent, inExclusiveRange(30, bookmark));
        final beforeDrag = scroll.offset;
        await tester.timedDrag(
          inside(index, find.byKey(const ValueKey('conversation-history'))),
          const Offset(0, 35),
          const Duration(seconds: 1),
        );
        await tester.pumpAndSettle();
        final chosen = scroll.offset;
        expect(chosen, lessThan(beforeDrag));
        tester.view.physicalSize = const Size(1600, 1800);
        await tester.pumpAndSettle();
        expect(scroll.offset, 0);
        tester.view.physicalSize = const Size(1600, 1000);
        await tester.pumpAndSettle();
        expect(scroll.offset, closeTo(chosen, .01));
        await tester.pumpWidget(const SizedBox());
      },
    );
  }
  testWidgets(
    'complete stable shells retain independent navigation and blanks across presets',
    (tester) async {
      final api = FakeAppApi();
      final w = await mount(tester, api);
      expect(find.byType(SmithWorkspace), findsOneWidget);
      final firstState = tester.state(find.byType(ChatPanel));
      await layout(tester, 2, 1);
      expect(find.byType(SmithWorkspace), findsNWidgets(2));
      await tester.tap(editor(1));
      await tester.pump();
      await tester.enterText(editor(1), 'second blank');
      await tester.tap(inside(1, find.byTooltip('Settings')));
      await tester.pumpAndSettle();
      expect(inside(1, find.text('Settings')), findsWidgets);
      expect(inside(0, find.byType(ChatPanel)), findsOneWidget);
      final second = w.panes[1];
      await layout(tester, 1, 2);
      expect(w.panes[1], same(second));
      expect(tester.state(inside(0, find.byType(ChatPanel))), same(firstState));
      await layout(tester, 1, 1);
      expect(find.byType(SmithWorkspace), findsOneWidget);
      await layout(tester, 2, 2);
      expect(find.byType(SmithWorkspace), findsNWidgets(4));
      await tester.tap(inside(1, find.byTooltip('Return to Chat')));
      await tester.pumpAndSettle();
      expect(
        tester.widget<TextField>(editor(1)).controller!.text,
        'second blank',
      );
      expect(w.panes[0].selected, isNot(same(second.selected)));
      expect(api.calls.where((op) => op.startsWith('submit')), isEmpty);
      await tester.pumpWidget(const SizedBox());
      expect(api.disposedCount, 1);
    },
  );

  testWidgets(
    'duplicate saved views share drafts but not reading positions or send scrolling',
    (tester) async {
      final api = FakeAppApi()..holdChats = true;
      for (var i = 0; i < 32; i++) {
        api.history.add({
          'role': i.isEven ? 'user' : 'assistant',
          'content': 'Retained message $i\nreading position\nlong history',
        });
      }
      final w = await mount(tester, api);
      await layout(tester, 2, 1);
      await w.selectFor(w.panes[1], testConversation);
      await tester.pumpAndSettle();
      final a = transcript(tester, 0);
      final b = transcript(tester, 1);
      a.jumpTo(100);
      b.jumpTo(200);
      await tester.tap(editor(1));
      await tester.pump();
      await tester.enterText(editor(1), 'shared draft');
      await tester.pump();
      expect(
        tester.widget<TextField>(editor(0)).controller!.text,
        'shared draft',
      );
      expect(tester.widget<TextField>(editor(0)).readOnly, true);
      expect(tester.widget<TextField>(editor(1)).readOnly, false);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(api.calls.where((op) => op == 'submit'), hasLength(1));
      expect(a.offset, 100);
      expect(b.offset, b.position.maxScrollExtent);
      w.newChatFor(w.panes[1]);
      await tester.pumpAndSettle();
      await w.selectFor(w.panes[1], testConversation);
      await tester.pumpAndSettle();
      expect(a.offset, 100);
      expect(b.offset, b.position.maxScrollExtent);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'hidden composers lose focus and cannot send after layout reduction',
    (tester) async {
      final api = FakeAppApi();
      final w = await mount(tester, api);
      await layout(tester, 2, 1);
      await tester.tap(editor(1));
      await tester.pump();
      await tester.enterText(editor(1), 'retained hidden draft');
      final focus = tester.widget<TextField>(editor(1)).focusNode!;
      expect(focus.hasFocus, true);
      await layout(tester, 1, 1);
      expect(focus.hasFocus, false);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(w.panes[1].selected.draft, 'retained hidden draft');
      expect(api.calls.where((op) => op.startsWith('submit')), isEmpty);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets('duplicate editors preserve IME and keyboard send ownership', (
    tester,
  ) async {
    final api = FakeAppApi()..holdChats = true;
    final w = await mount(tester, api);
    await layout(tester, 2, 1);
    await tester.tap(inside(1, find.byTooltip('Open chat history')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Current chat').last);
    await tester.pumpAndSettle();
    expect(w.panes[1].selected, same(w.panes[0].selected));
    await tester.tap(editor(1));
    await tester.pump();
    tester.testTextInput.updateEditingValue(
      const TextEditingValue(
        text: 'composing',
        selection: TextSelection.collapsed(offset: 9),
        composing: TextRange(start: 0, end: 9),
      ),
    );
    await tester.pump();
    final editing = tester.widget<TextField>(editor(1)).controller!;
    expect(editing.value.composing, const TextRange(start: 0, end: 9));
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(api.calls.where((op) => op.startsWith('submit')), isEmpty);
    tester.testTextInput.updateEditingValue(
      const TextEditingValue(
        text: 'composing',
        selection: TextSelection.collapsed(offset: 9),
      ),
    );
    await tester.pump();
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    await tester.pump();
    expect(w.panes[0].selected.draft, 'composing\n');
    await tester.tap(editor(0));
    await tester.pump();
    expect(tester.widget<TextField>(editor(1)).readOnly, true);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.enter);
    await tester.sendKeyRepeatEvent(LogicalKeyboardKey.enter);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(api.calls.where((op) => op == 'submit'), hasLength(1));
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('400 square at 2x retains equal minimum tiles without overflow', (
    tester,
  ) async {
    final api = FakeAppApi();
    await mount(tester, api, size: const Size(400, 400), scale: 2);
    await layout(tester, 2, 2);
    final sizes = [for (var i = 0; i < 4; i++) tester.getSize(tile(i))];
    expect(sizes.every((size) => size == sizes.first), true);
    expect(sizes.first.width, greaterThanOrEqualTo(800));
    expect(sizes.first.height, greaterThanOrEqualTo(800));
    expect(tester.takeException(), isNull);
    await tester.ensureVisible(editor(3));
    await tester.pumpAndSettle();
    await tester.tap(editor(3));
    await tester.pump();
    await tester.enterText(editor(3), 'reachable');
    expect(tester.widget<TextField>(editor(3)).controller!.text, 'reachable');

    await tester.pumpWidget(const SizedBox());
  });
}
