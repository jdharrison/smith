import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/ui/agents_view.dart';
import 'package:smith_client/ui/dev_jobs_view.dart';
import 'package:smith_client/ui/projects_view.dart';
import 'package:smith_client/ui/smith_workspace.dart';

import 'agents_navigation_test.dart' show mountAgentShell, agentPaneTap;
import 'agents_view_test.dart' show AgentsUiApi, agentKey;
import 'projects_view_test.dart' show ProjectsUiApi;
import 'split_workspace_widget_test.dart' show inside, layout;
import 'workspace_restore_test.dart' show presentation;

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
    'Dev root selects the jobs surface, keeps disclosure separate and admits no work',
    (tester) async {
      final api = NavigationApi();
      await mountAgentShell(tester, api);
      final before = List.of(api.calls);
      await tapNav(tester, 'Dev');
      expect(presentation(tester).pages[0], 'dev');
      expect(find.byType(DevJobsView), findsOneWidget);
      expect(inside(0, find.byTooltip('Projects')), findsNothing);
      expect(tester.widget<IconButton>(navButton('Dev')).isSelected, true);
      expect(
        tester.widget<IconButton>(navButton('Dev navigation')).isSelected,
        false,
      );
      expect(api.calls.skip(before.length), [
        'projects_list_v3',
        'dev_jobs_state_v4',
        'dev_sessions_state_v1',
      ]);
      expect(
        api.calls.where(
          (op) =>
              op.startsWith('project_create') ||
              op.startsWith('project_update') ||
              op.startsWith('project_delete') ||
              op.startsWith('submit'),
        ),
        isEmpty,
      );
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets('Dev disclosure only changes links and returns focused child', (
    tester,
  ) async {
    final api = NavigationApi();
    await mountAgentShell(tester, api);
    final before = List.of(api.calls);
    final pages = presentation(tester);
    await tapNav(tester, 'Dev navigation');
    expect(pages.pages[0], 'chat');
    expect(inside(0, find.byTooltip('Projects')), findsOneWidget);
    final child = tester.widget<IconButton>(navButton('Projects')).focusNode!;
    child.requestFocus();
    await tester.pump();
    final disclosure = tester.widget<IconButton>(navButton('Dev navigation'));
    disclosure.onPressed!();
    await tester.pumpAndSettle();
    expect(child.hasFocus, false);
    expect(disclosure.focusNode!.hasFocus, true);
    expect(inside(0, find.byTooltip('Projects')), findsNothing);
    expect(api.calls, before);
    await tapNav(tester, 'Dev');
    await tapNav(tester, 'Dev navigation');
    await tapNav(tester, 'Projects');
    expect(pages.pages[0], 'projects');
    expect(inside(0, find.byType(ProjectsView)), findsOneWidget);
    expect(tester.widget<IconButton>(navButton('Dev')).isSelected, false);
    expect(tester.widget<IconButton>(navButton('Projects')).isSelected, true);
    final state = tester.state(inside(0, find.byType(ProjectsView)));
    await tapNav(tester, 'Dev navigation');
    expect(pages.pages[0], 'projects');
    expect(tester.state(inside(0, find.byType(ProjectsView))), same(state));
    tester.view.physicalSize = const Size(400, 400);
    await tester.pumpAndSettle();
    tester.view.physicalSize = const Size(2600, 1400);
    await tester.pumpAndSettle();
    expect(inside(0, find.byTooltip('Projects')), findsNothing);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
    'Agents first then Dev then first Projects retains editor identity',
    (tester) async {
      final api = NavigationApi();
      await mountAgentShell(tester, api);
      presentation(tester).setPage(0, 'agents');
      await tester.pumpAndSettle();
      await agentPaneTap(tester, 0, agentKey('agent-card-developer'));
      await tester.enterText(agentKey('agent-prompt'), 'Retained draft\n' * 60);
      final state = tester.state(inside(0, find.byType(AgentsView)));
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
      expect(api.calls.skip(before.length), [
        'projects_list_v3',
        'dev_jobs_state_v4',
        'dev_sessions_state_v1',
      ]);
      final afterDev = List.of(api.calls);
      await tapNav(tester, 'Dev navigation');
      await tapNav(tester, 'Projects');
      expect(api.calls, afterDev);
      presentation(tester).setPage(0, 'agents');
      await tester.pumpAndSettle();
      expect(tester.state(inside(0, find.byType(AgentsView))), same(state));
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
    'compact 400 square 2x inline disclosure uses 104px and 48px targets',
    (tester) async {
      tester.platformDispatcher.textScaleFactorTestValue = 2;
      addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
      final api = NavigationApi();
      await mountAgentShell(tester, api, size: const Size(400, 400));
      await tapNav(tester, 'Dev');
      final before = List.of(api.calls);
      expect(
        tester
            .getSize(find.byKey(const ValueKey('workspace-navigation')))
            .width,
        104,
      );
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
      expect(find.byKey(const ValueKey('dev-chooser')), findsNothing);
      expect(inside(0, find.byTooltip('Projects')), findsOneWidget);
      expect(presentation(tester).pages[0], 'dev');
      expect(api.calls, before);
      await tapNav(tester, 'Projects');
      expect(presentation(tester).pages[0], 'projects');
      expect(inside(0, find.byType(ProjectsView)), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'four panes retain disclosure separately across layout and size changes',
    (tester) async {
      final api = NavigationApi();
      await mountAgentShell(tester, api, size: const Size(4200, 2400));
      await layout(tester, 2, 2);
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
        expect(inside(i, find.byType(DevJobsView)), findsOneWidget);
        expect(
          inside(i, find.byTooltip('Projects')),
          i.isOdd ? findsOneWidget : findsNothing,
        );
      }
      tester.view.physicalSize = const Size(1000, 900);
      await tester.pumpAndSettle();
      await tapNav(tester, 'Dev navigation', 0);
      expect(inside(0, find.byTooltip('Projects')), findsOneWidget);
      expect(presentation(tester).pages[0], 'dev');
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
          devJobs: const Text('Dev jobs body'),
          error: 'Unrelated error',
          recoveryGuidance: 'Unrelated guidance',
          onRetry: () => retries++,
          child: const Text('Chat body'),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Dev jobs body'), findsOneWidget);
    expect(find.text('Unrelated error'), findsNothing);
    expect(find.text('Unrelated guidance'), findsNothing);
    expect(find.text('Retry observation'), findsNothing);
    expect(retries, 0);
    expect(tester.takeException(), isNull);
  });
}
