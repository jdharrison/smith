import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/main.dart';
import 'package:smith_client/ui/agents_view.dart';
import 'package:smith_client/ui/workspace_entrance.dart';

import 'package:smith_client/workspace_controller.dart';
import 'package:smith_client/workspace_preferences.dart';
import 'package:smith_client/window_launcher.dart';

import 'agents_view_test.dart';
import 'split_workspace_widget_test.dart' show inside, layout;
import 'widget_test.dart' show FakeControlApi, profile;
import 'workspace_restore_test.dart' show MemoryStore, presentation;

Future<void> mountAgentShell(
  WidgetTester tester,
  AgentsUiApi api, {
  MemoryStore? store,
  Size size = const Size(2600, 1400),
}) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  api.inventoryObserved = true;
  await tester.pumpWidget(
    SmithApp(
      controlApi: FakeControlApi(profile()),
      appApi: api,
      preferencesStore: store,
    ),
  );
  await tester.pumpAndSettle();
}

Future<void> agentPaneTap(WidgetTester tester, int pane, Finder target) async {
  await tester.pumpAndSettle();
  final finder = inside(pane, target).first;
  await Scrollable.of(tester.element(finder)).position
      .ensureVisible(tester.renderObject(finder));
  await tester.pumpAndSettle();
  await tester.tap(finder);
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('only Agents and Projects omit the outer destination header', (
    tester,
  ) async {
    final api = AgentsUiApi();
    await mountAgentShell(tester, api);
    final pages = presentation(tester);
    final title = find.byWidgetPredicate(
      (widget) =>
          widget is WorkspaceReveal && widget.region == WorkspaceRegion.title,
    );
    for (final destination in [
      'agents',
      'settings',
      'projects',
      'chat',
      'dev',
    ]) {
      pages.setPage(0, destination);
      await tester.pumpAndSettle();
      expect(
        inside(0, title),
        destination == 'agents' || destination == 'projects'
            ? findsNothing
            : findsOneWidget,
      );
      if (destination == 'agents') {
        expect(inside(0, find.text('AGENTS')), findsOneWidget);
        expect(inside(0, find.text('NEW AGENT')), findsOneWidget);
        expect(inside(0, find.text('Search agents')), findsNothing);
        expect(inside(0, find.text('Refresh agents')), findsNothing);
      }
    }
    expect(api.writes, isEmpty);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
    'compact sidebar keeps separate inline disclosure and hides Agents',
    (tester) async {
      final api = AgentsUiApi();
      await mountAgentShell(tester, api, size: const Size(400, 400));
      expect(api.agentCalls, isEmpty);
      final dev = inside(0, find.byTooltip('Dev navigation'));
      final button = find.descendant(
        of: dev,
        matching: find.byType(IconButton),
      );
      Focus.of(
        tester.element(
          find.descendant(of: button, matching: find.byType(Icon)).first,
        ),
      ).requestFocus();
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(find.byType(SimpleDialog), findsNothing);
      expect(api.agentCalls, isEmpty);
      expect(find.byTooltip('Agents'), findsNothing);
      expect(find.byTooltip('Projects'), findsOneWidget);
      final navigation = inside(
        0,
        find.byKey(const ValueKey('workspace-navigation')),
      );
      expect(tester.getSize(navigation).width, 104);
      expect(
        find.descendant(of: navigation, matching: find.byType(Text)),
        findsNothing,
      );
      final root = inside(0, find.byTooltip('Dev'));
      final rootRect = tester.getRect(root);
      final disclosureRect = tester.getRect(dev);
      expect(rootRect.top, disclosureRect.top);
      expect(rootRect.right, lessThanOrEqualTo(disclosureRect.left));
      expect(rootRect.width, greaterThanOrEqualTo(48));
      expect(disclosureRect.width, greaterThanOrEqualTo(48));
      expect(
        tester.getTopLeft(find.byTooltip('Projects')).dx,
        greaterThan(rootRect.left),
      );
      tester.view.physicalSize = const Size(2600, 1400);
      await tester.pumpAndSettle();
      expect(find.byTooltip('Projects'), findsOneWidget);
      expect(
        find.descendant(of: navigation, matching: find.text('Projects')),
        findsOneWidget,
      );
      tester.view.physicalSize = const Size(400, 400);
      await tester.pumpAndSettle();
      final child = tester.widget<IconButton>(
        find.descendant(
          of: find.byTooltip('Projects'),
          matching: find.byType(IconButton),
        ),
      );
      child.focusNode!.requestFocus();
      await tester.pump();
      tester.widget<IconButton>(button).onPressed!();
      await tester.pumpAndSettle();
      expect(find.byTooltip('Projects'), findsNothing);
      expect(tester.widget<IconButton>(button).focusNode!.hasFocus, true);
      expect(api.agentCalls, isEmpty);
      presentation(tester).setPage(0, 'agents');
      await tester.pumpAndSettle();
      expect(presentation(tester).pages[0], 'agents');
      expect(api.agentCalls, ['agents_list_v2']);
      expect(find.byType(AgentsView), findsOneWidget);
      expect(find.text('Run'), findsNothing);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'four panes share one registry and retain independent drafts, selection, scroll and hidden focus',
    (tester) async {
      final api = AgentsUiApi();
      await mountAgentShell(tester, api);
      final owner =
          tester
                  .widget<ConversationScreen>(find.byType(ConversationScreen))
                  .chat
              as WorkspaceController;
      owner.selected.edit('Chat remains untouched');
      await layout(tester, 2, 2);
      final pages = presentation(tester);
      for (var i = 0; i < 4; i++) {
        pages.setPage(i, 'agents');
      }
      await tester.pumpAndSettle();
      expect(api.agentCalls, ['agents_list_v2']);
      final controllers = tester
          .widgetList<AgentsView>(find.byType(AgentsView))
          .map((v) => v.controller)
          .toSet();
      expect(controllers.length, 1);
      final editors = <TextEditingController>[];
      final scrolls = <ScrollController>[];
      for (var i = 0; i < 4; i++) {
        await agentPaneTap(tester, i, agentKey('agent-card-developer'));
        await tester.enterText(
          inside(i, agentKey('agent-prompt')),
          'Pane $i\n' * 60,
        );
        final field = tester
            .widget<TextField>(inside(i, agentKey('agent-prompt')))
            .controller!;
        field.selection = const TextSelection(baseOffset: 2, extentOffset: 5);
        editors.add(field);
        await tester.pumpAndSettle();
        final scroll = tester
            .widget<SingleChildScrollView>(
              inside(i, agentKey('agent-editor-scroll')),
            )
            .controller!;
        scroll.jumpTo(200 + i * 10);
        scrolls.add(scroll);
      }
      await tester.pumpAndSettle();
      final editable = find.descendant(
        of: inside(3, agentKey('agent-prompt')),
        matching: find.byType(EditableText),
      );
      final focus = tester.widget<EditableText>(editable).focusNode;
      expect(focus.hasFocus, isTrue);
      await layout(tester, 1, 1);
      expect(focus.hasFocus, isFalse);
      expect(focus.canRequestFocus, isFalse);
      pages.setPage(0, 'settings');
      await tester.pumpAndSettle();
      pages.setPage(0, 'agents');
      await layout(tester, 2, 2);
      final callsBeforeDev = List.of(api.calls);
      for (var i = 0; i < 4; i++) {
        await agentPaneTap(tester, i, find.byTooltip('Dev navigation'));
        await agentPaneTap(tester, i, find.byTooltip('Dev navigation'));
        pages.setPage(i, 'dev');
      }
      await tester.pumpAndSettle();
      expect(api.calls.take(callsBeforeDev.length), callsBeforeDev);
      expect(api.calls.skip(callsBeforeDev.length), [
        for (var i = 0; i < 4; i++) ...[
          'projects_list_v3',
          'dev_jobs_state_v4',
        ],
      ]);
      expect(api.writes, isEmpty);
      for (var i = 0; i < 4; i++) {
        pages.setPage(i, 'agents');
      }
      await tester.pumpAndSettle();
      for (var i = 0; i < 4; i++) {
        expect(
          identical(
            editors[i],
            tester
                .widget<TextField>(inside(i, agentKey('agent-prompt')))
                .controller,
          ),
          isTrue,
        );
        expect(editors[i].text, 'Pane $i\n' * 60);
        expect(
          editors[i].selection,
          const TextSelection(baseOffset: 2, extentOffset: 5),
        );
        expect(scrolls[i].offset, closeTo(200 + i * 10, 1));
      }
      expect(owner.selected.draft, 'Chat remains untouched');
      expect(api.writes, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'v3 restoration observes Agents only and saves no editor data; session-only creates no store',
    (tester) async {
      final store = MemoryStore(
        WorkspacePreferences(
          layout: '1x1',
          activeTile: 0,
          slots: List.generate(
            4,
            (_) =>
                WorkspaceSlotPreferences(conversationId: null, page: 'agents'),
          ),
        ),
      );
      final api = AgentsUiApi();
      await mountAgentShell(tester, api, store: store);
      expect(api.agentCalls, ['agents_list_v2']);
      await agentPaneTap(tester, 0, agentKey('agent-card-developer'));
      await tester.enterText(
        inside(0, agentKey('agent-prompt')),
        'Private unsaved definition',
      );
      presentation(tester).setPage(0, 'settings');
      await tester.pumpAndSettle();
      final saved = jsonEncode(store.value!.toJson());
      expect(store.value!.toJson()['version'], 4);
      for (final forbidden in ['Private', 'prompt', 'developer', 'selection']) {
        expect(saved, isNot(contains(forbidden)));
      }
      expect(api.writes, isEmpty);
      await tester.pumpWidget(const SizedBox());
      var constructed = 0;
      final sessionStore = workspacePreferencesForStartup(
        [sessionOnlyWindowArgument],
        () {
          constructed++;
          return store;
        },
      );
      expect(sessionStore, isNull);
      expect(constructed, 0);
      expect(store.loads, 1);
    },
  );
}
