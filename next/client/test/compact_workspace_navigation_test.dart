import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/ui/chat_sidebar.dart';
import 'package:smith_client/ui/smith_workspace.dart';
import 'package:smith_client/ui/split_workspace.dart';

import 'app_fakes.dart';
import 'split_workspace_widget_test.dart'
    show mount, layout, inside, tile, editor;

Finder menu() => find.byKey(const ValueKey('workspace-layout'));

bool menuFocused(WidgetTester tester) => Focus.of(
  tester.element(
    find.descendant(of: menu(), matching: find.byType(Icon)).first,
  ),
).hasFocus;
Finder title(int index) =>
    inside(index, find.byKey(const ValueKey('compact-chat-title')));

Future<void> chooseFrom(WidgetTester tester, WorkspaceLayout value) async {
  await tester.ensureVisible(menu());
  await tester.tap(menu());
  await tester.pumpAndSettle();
  await tester.tap(
    find.byWidgetPredicate(
      (widget) =>
          widget is CheckedPopupMenuItem<WorkspaceLayout> &&
          widget.value == value,
    ),
  );
  await tester.pumpAndSettle();
}

class Fixture extends StatefulWidget {
  const Fixture({super.key});
  @override
  State<Fixture> createState() => FixtureState();
}

class FixtureState extends State<Fixture> {
  List<ChatSummary> chats = [const ChatSummary(id: 'one', title: 'First chat')];
  final calls = <String>[];
  bool replacement = false;
  bool busy = false;

  void change(VoidCallback update) => setState(update);

  @override
  Widget build(BuildContext context) => MaterialApp(
    home: SmithWorkspace(
      chats: chats,
      title: 'A very long title that must remain bounded and accessible',
      selectedChatId: 'one',
      version: 'test',
      connectionStatus: 'SYSTEM ONLINE',
      modelStatus: 'test',
      busy: busy,
      onSelectChat: replacement
          ? (id) => calls.add('replacement:$id')
          : (id) => calls.add('original:$id'),
      onNewChat: () => calls.add('new'),
      onRefreshArchives: () async {},
      onDeleteChat: (id) async => calls.add('delete:$id'),
      child: const SizedBox.expand(),
    ),
  );
}

Future<FixtureState> fixture(
  WidgetTester tester, {
  Size size = const Size(600, 600),
  double scale = 1,
}) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  tester.platformDispatcher.textScaleFactorTestValue = scale;
  addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
  final key = GlobalKey<FixtureState>();
  await tester.pumpWidget(Fixture(key: key));
  await tester.pumpAndSettle();
  return key.currentState!;
}

void main() {
  testWidgets('shared status header controls the grid including Settings', (
    tester,
  ) async {
    await mount(tester, FakeAppApi());
    final shellTop = tester.getTopLeft(tile(0)).dy;
    expect(shellTop, greaterThan(0));
    await layout(tester, 2, 2);
    for (var i = 0; i < 4; i++) {
      expect(menu(), findsOneWidget);
      final status = find.text('SYSTEM ONLINE');
      expect(inside(i, find.text('> SMITH')), findsNothing);
      expect(tester.getRect(menu()).overlaps(tester.getRect(status)), false);
      expect(
        (tester.getCenter(menu()).dy - tester.getCenter(status).dy).abs(),
        lessThan(30),
      );
    }
    await tester.tap(editor(3));
    await tester.pump();
    await tester.tap(inside(3, find.byTooltip('Settings')));
    await tester.pumpAndSettle();
    expect(title(3), findsNothing);
    expect(inside(3, find.byTooltip('New chat')), findsNothing);
    await chooseFrom(tester, WorkspaceLayout.sideBySide);
    expect(find.byKey(const ValueKey('workspace-layout')), findsOneWidget);
    expect(menuFocused(tester), true);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(
      find.byType(CheckedPopupMenuItem<WorkspaceLayout>),
      findsNWidgets(4),
    );
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('title chooser and direct New act only on originating tile', (
    tester,
  ) async {
    final api = FakeAppApi();
    final owner = await mount(tester, api);
    await layout(tester, 2, 1);
    final first = owner.panes[0].selected;
    final blank = owner.panes[1].selected;
    await tester.tap(title(1));
    await tester.pumpAndSettle();
    final chooser = find.byKey(const ValueKey('compact-chat-chooser'));
    expect(chooser, findsOneWidget);
    expect(
      find.descendant(of: chooser, matching: find.byTooltip('New chat')),
      findsNothing,
    );
    await tester.tap(
      find.descendant(of: chooser, matching: find.text('Current chat')),
    );
    await tester.pumpAndSettle();
    expect(owner.panes[1].selected, same(first));
    await tester.tap(inside(1, find.byTooltip('New chat')));
    await tester.pumpAndSettle();
    expect(owner.panes[1].selected, same(blank));
    expect(owner.panes[0].selected, same(first));
    expect(api.calls.where((call) => call.startsWith('submit')), isEmpty);
    await tester.tap(inside(1, find.byTooltip('Archived chats')));
    await tester.pumpAndSettle();
    expect(find.text('Archived chats'), findsWidgets);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
    'chooser follows live membership and current selection callbacks',
    (tester) async {
      final state = await fixture(tester);
      await tester.tap(find.byKey(const ValueKey('compact-chat-title')));
      await tester.pumpAndSettle();
      state.change(() {
        state.replacement = true;
        state.chats = [const ChatSummary(id: 'two', title: 'Second chat')];
      });
      await tester.pumpAndSettle();
      expect(find.text('First chat'), findsNothing);
      await tester.tap(find.text('Second chat'));
      await tester.pumpAndSettle();
      expect(state.calls, ['replacement:two']);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'stale chooser callbacks recheck membership, busy and current owner',
    (tester) async {
      final state = await fixture(tester);
      await tester.tap(find.byKey(const ValueKey('compact-chat-title')));
      await tester.pumpAndSettle();
      final select = tester
          .widget<ChatSidebar>(find.byType(ChatSidebar))
          .onSelect!;
      state.change(() {
        state.replacement = true;
        state.chats = [
          const ChatSummary(id: 'one', title: 'First chat', archived: true),
        ];
      });
      await tester.pumpAndSettle();
      select('one');
      expect(state.calls, isEmpty);
      state.change(() {
        state.chats = [const ChatSummary(id: 'one', title: 'First chat')];
        state.busy = true;
      });
      await tester.pumpAndSettle();
      select('one');
      expect(state.calls, isEmpty);
      state.change(() => state.busy = false);
      await tester.pumpAndSettle();
      select('one');
      await tester.pumpAndSettle();
      expect(state.calls, ['replacement:one']);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets('open chooser action cannot dispatch after authority revoked', (
    tester,
  ) async {
    final state = await fixture(tester);
    await tester.tap(find.byKey(const ValueKey('compact-chat-title')));
    await tester.pumpAndSettle();
    await tester.tap(find.byTooltip('Actions for First chat'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Delete'));
    await tester.pumpAndSettle();
    state.change(() => state.busy = true);
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(TextButton, 'Delete'));
    await tester.pumpAndSettle();
    expect(state.calls, isEmpty);
    expect(find.textContaining('No change was sent'), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
    '400 square at 2x bounds long title and chooser; empty and breakpoint states',
    (tester) async {
      final state = await fixture(tester, size: const Size(400, 400), scale: 2);
      final button = find.byKey(const ValueKey('compact-chat-title'));
      expect(button, findsOneWidget);
      await tester.tap(button);
      await tester.pumpAndSettle();
      final rect = tester.getRect(
        find.byKey(const ValueKey('compact-chat-chooser')),
      );
      expect(rect.left, greaterThanOrEqualTo(0));
      expect(rect.right, lessThanOrEqualTo(400));
      expect(rect.bottom, lessThanOrEqualTo(400));
      state.change(() => state.chats = []);
      await tester.pumpAndSettle();
      expect(find.text('No chats'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.tap(find.byTooltip('Close chat chooser'));
      await tester.pumpAndSettle();
      tester.view.physicalSize = const Size(1800, 900);
      await tester.pumpAndSettle();
      expect(button, findsNothing);
      expect(find.byType(ChatSidebar), findsOneWidget);
      expect(find.byTooltip('New chat'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'hiding fourth tile keeps its draft and focuses visible layout not composer',
    (tester) async {
      final api = FakeAppApi();
      final owner = await mount(tester, api);
      await layout(tester, 2, 2);
      await tester.tap(editor(3));
      await tester.enterText(editor(3), 'retained');
      final controller = owner.panes[3].selected;
      await chooseFrom(tester, WorkspaceLayout.single);
      expect(menuFocused(tester), true);
      expect(tester.widget<TextField>(editor(0)).focusNode!.hasFocus, false);
      expect(controller.draft, 'retained');
      expect(api.calls.where((call) => call.startsWith('submit')), isEmpty);
      await layout(tester, 2, 2);
      expect(owner.panes[3].selected, same(controller));
      expect(tester.widget<TextField>(editor(3)).controller!.text, 'retained');
      await tester.pumpWidget(const SizedBox());
    },
  );
}
