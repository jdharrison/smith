import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/main.dart';
import 'package:smith_client/ui/agents_view.dart';
import 'package:smith_client/ui/projects_view.dart';
import 'package:smith_client/ui/smith_workspace.dart';
import 'package:smith_client/ui/split_workspace.dart';

import 'agents_navigation_test.dart' show mountAgentShell, agentPaneTap;
import 'agents_view_test.dart' show AgentsUiApi, agentKey;
import 'projects_view_test.dart' show ProjectsUiApi;
import 'split_workspace_widget_test.dart' show inside, layout;
import 'workspace_restore_test.dart' show presentation;

const placeholder =
    'Development workspace placeholder. Use Projects or Agents from Dev navigation.';

class NavigationApi extends AgentsUiApi {
  final projects = ProjectsUiApi();

  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) {
    if (op.startsWith('project')) {
      calls.add(op);
      arguments.add(Map.of(args));
      return projects.call(op, args);
    }
    return super.call(op, args);
  }
}

Finder nav(String label, [int pane = 0]) =>
    inside(pane, find.byTooltip(label)).first;
Finder navButton(String label, [int pane = 0]) =>
    find.descendant(of: nav(label, pane), matching: find.byType(IconButton));

Future<void> tapNav(WidgetTester tester, String label, [int pane = 0]) async {
  await tester.ensureVisible(nav(label, pane));
  await tester.pumpAndSettle();
  await tester.tap(nav(label, pane));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets(
    'Dev root is inert, independent, truthful and lazy at both widths',
    (tester) async {
      final api = NavigationApi();
      await mountAgentShell(tester, api);
      final before = List.of(api.calls);
      expect(find.byType(AgentsView), findsNothing);
      expect(find.byType(ProjectsView), findsNothing);
      for (final size in [const Size(2600, 1400), const Size(400, 400)]) {
        tester.view.physicalSize = size;
        await tester.pumpAndSettle();
        await tapNav(tester, 'Dev');
        expect(presentation(tester).pages[0], 'dev');
        expect(find.text(placeholder), findsOneWidget);
        expect(find.byType(SimpleDialog), findsNothing);
        expect(find.byTooltip('Projects'), findsNothing);
        expect(tester.widget<IconButton>(navButton('Dev')).isSelected, true);
        expect(
          tester.widget<IconButton>(navButton('Dev navigation')).isSelected,
          false,
        );
        expect(find.text('Run'), findsNothing);
        expect(find.text('Retry observation'), findsNothing);
        expect(api.calls, before);
      }
      expect(find.byType(AgentsView), findsNothing);
      expect(find.byType(ProjectsView), findsNothing);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets('wide disclosure only changes links and returns focused child', (
    tester,
  ) async {
    final api = NavigationApi();
    await mountAgentShell(tester, api);
    final before = List.of(api.calls);
    final pages = presentation(tester);
    await tapNav(tester, 'Dev navigation');
    expect(pages.pages[0], 'chat');
    expect(find.byTooltip('Projects'), findsOneWidget);
    final child = tester.widget<IconButton>(navButton('Projects')).focusNode!;
    child.requestFocus();
    await tester.pump();
    final disclosure = tester.widget<IconButton>(navButton('Dev navigation'));
    disclosure.onPressed!();
    await tester.pumpAndSettle();
    expect(child.hasFocus, false);
    expect(disclosure.focusNode!.hasFocus, true);
    expect(find.byTooltip('Projects'), findsNothing);
    expect(api.calls, before);
    await tapNav(tester, 'Dev');
    await tapNav(tester, 'Dev navigation');
    await tapNav(tester, 'Dev');
    expect(find.byTooltip('Projects'), findsOneWidget);
    expect(find.text(placeholder), findsOneWidget);
    await tapNav(tester, 'Projects');
    expect(tester.widget<IconButton>(navButton('Dev')).isSelected, false);
    expect(tester.widget<IconButton>(navButton('Projects')).isSelected, true);
    final state = tester.state(find.byType(ProjectsView));
    await tapNav(tester, 'Dev navigation');
    expect(pages.pages[0], 'projects');
    expect(tester.state(find.byType(ProjectsView)), same(state));
    tester.view.physicalSize = const Size(400, 400);
    await tester.pumpAndSettle();
    tester.view.physicalSize = const Size(2600, 1400);
    await tester.pumpAndSettle();
    expect(navButton('Projects'), findsNothing);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
    'Agents first then Dev then first Projects retains editor identity',
    (tester) async {
      final api = NavigationApi();
      await mountAgentShell(tester, api);
      await tapNav(tester, 'Dev navigation');
      await tapNav(tester, 'Agents');
      await agentPaneTap(tester, 0, agentKey('agent-card-developer'));
      await tester.enterText(agentKey('agent-prompt'), 'Retained draft\n' * 60);
      final state = tester.state(find.byType(AgentsView));
      final field = tester
          .widget<TextField>(agentKey('agent-prompt'))
          .controller!;
      field.selection = const TextSelection(baseOffset: 2, extentOffset: 8);
      final scroll = tester
          .widget<SingleChildScrollView>(agentKey('agent-editor-scroll'))
          .controller!;
      scroll.jumpTo(180);
      await tester.pumpAndSettle();
      final before = List.of(api.calls);
      await tapNav(tester, 'Dev');
      expect(api.calls, before);
      await tapNav(tester, 'Projects');
      expect(api.calls.sublist(before.length), ['projects_list_v2']);
      await tapNav(tester, 'Agents');
      expect(tester.state(find.byType(AgentsView)), same(state));
      expect(
        tester.widget<TextField>(agentKey('agent-prompt')).controller,
        same(field),
      );
      expect(field.text, 'Retained draft\n' * 60);
      expect(
        field.selection,
        const TextSelection(baseOffset: 2, extentOffset: 8),
      );
      expect(scroll.offset, closeTo(180, 1));
      expect(api.agentCalls, ['agents_list_v2']);
      expect(api.projects.writes, isEmpty);
      expect(api.writes, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'compact 400 square 2x keyboard chooser uses actual expanded state',
    (tester) async {
      tester.platformDispatcher.textScaleFactorTestValue = 2;
      addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
      final api = NavigationApi();
      await mountAgentShell(tester, api, size: const Size(400, 400));
      await tapNav(tester, 'Dev');
      final before = List.of(api.calls);
      for (final label in ['Dev', 'Dev navigation']) {
        final size = tester.getSize(navButton(label));
        expect(size.width, greaterThanOrEqualTo(48));
        expect(size.height, greaterThanOrEqualTo(48));
      }
      final button = tester.widget<IconButton>(navButton('Dev navigation'));
      button.focusNode!.requestFocus();
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      final chooser = find.byKey(const ValueKey('dev-chooser'));
      expect(chooser, findsOneWidget);
      final rect = tester.getRect(chooser);
      expect(rect.left, greaterThanOrEqualTo(0));
      expect(rect.right, lessThanOrEqualTo(400));
      expect(rect.bottom, lessThanOrEqualTo(400));
      final disclosure = find
          .byWidgetPredicate(
            (w) => w is Semantics && w.properties.label == 'Dev navigation',
            skipOffstage: false,
          )
          .first;
      expect(tester.widget<Semantics>(disclosure).properties.expanded, true);
      expect(api.calls, before);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(tester.widget<Semantics>(disclosure).properties.expanded, false);
      expect(presentation(tester).pages[0], 'dev');
      expect(button.focusNode!.hasFocus, true);
      await tapNav(tester, 'Dev navigation');
      await tester.ensureVisible(
        find.widgetWithText(SimpleDialogOption, 'Agents'),
      );
      await tester.tap(find.widgetWithText(SimpleDialogOption, 'Agents'));
      await tester.pumpAndSettle();
      expect(presentation(tester).pages[0], 'agents');
      expect(api.agentCalls, ['agents_list_v2']);
      await tapNav(tester, 'Dev navigation');
      expect(
        find.ancestor(
          of: find.widgetWithText(SimpleDialogOption, 'Agents'),
          matching: find.byWidgetPredicate(
            (w) => w is Semantics && w.properties.selected == true,
          ),
        ),
        findsOneWidget,
      );
      expect(
        find.ancestor(
          of: find.widgetWithText(SimpleDialogOption, 'Projects'),
          matching: find.byWidgetPredicate(
            (w) => w is Semantics && w.properties.selected == true,
          ),
        ),
        findsNothing,
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets('chooser ignores origin page changes, including away and back', (
    tester,
  ) async {
    final api = NavigationApi();
    await mountAgentShell(tester, api, size: const Size(400, 400));
    final pages = presentation(tester);
    final before = List.of(api.calls);
    await tapNav(tester, 'Dev navigation');
    pages.setPage(0, 'dev');
    await tester.pumpAndSettle();
    pages.setPage(0, 'chat');
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(SimpleDialogOption, 'Agents'));
    await tester.pumpAndSettle();
    expect(pages.pages[0], 'chat');
    expect(api.calls, before);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
    'four panes retain disclosure separately and ignore hidden chooser',
    (tester) async {
      final api = NavigationApi();
      await mountAgentShell(tester, api, size: const Size(4200, 2400));
      await layout(tester, 2, 2);
      final before = List.of(api.calls);
      for (var i = 0; i < 4; i++) {
        await tapNav(tester, 'Dev', i);
        if (i.isOdd) await tapNav(tester, 'Dev navigation', i);
        expect(
          inside(i, find.byTooltip('Projects')),
          i.isOdd ? findsOneWidget : findsNothing,
        );
      }
      await layout(tester, 1, 1);
      await layout(tester, 2, 2);
      for (var i = 0; i < 4; i++) {
        expect(inside(i, find.text(placeholder)), findsOneWidget);
        expect(
          inside(i, find.byTooltip('Projects')),
          i.isOdd ? findsOneWidget : findsNothing,
        );
      }
      tester.view.physicalSize = const Size(1000, 900);
      await tester.pumpAndSettle();
      await tapNav(tester, 'Dev navigation', 3);
      final screen = tester.widget<ConversationScreen>(
        find.byType(ConversationScreen),
      );
      // Use the current layout callback while the chooser is above its origin.
      tester
          .widget<WorkspaceLayoutMenu>(find.byType(WorkspaceLayoutMenu))
          .onSelected(WorkspaceLayout.single);
      await tester.pumpAndSettle();
      tester
          .widget<WorkspaceLayoutMenu>(find.byType(WorkspaceLayoutMenu))
          .onSelected(WorkspaceLayout.grid);
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(SimpleDialogOption, 'Agents'));
      await tester.pumpAndSettle();
      expect(screen.presentation!.pages[3], 'dev');
      expect(api.calls, before);
      tester.view.physicalSize = const Size(4200, 2400);
      await tester.pumpAndSettle();
      expect(inside(3, find.byTooltip('Projects')), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets('Dev suppresses unrelated recovery content and callbacks', (
    tester,
  ) async {
    var retries = 0;
    await tester.pumpWidget(
      MaterialApp(
        home: SmithWorkspace(
          page: 'dev',
          chats: const [],
          title: 'Hidden chat',
          version: 'test',
          connectionStatus: 'offline',
          modelStatus: 'unavailable',
          error: 'Unrelated error',
          recoveryGuidance: 'Unrelated guidance',
          onRetry: () => retries++,
          child: const Text('Chat body'),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text(placeholder), findsOneWidget);
    expect(find.text('Unrelated error'), findsNothing);
    expect(find.text('Unrelated guidance'), findsNothing);
    expect(find.text('Retry observation'), findsNothing);
    expect(retries, 0);
    expect(tester.takeException(), isNull);
  });
}
