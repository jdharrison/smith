import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/chat_panel.dart';
import 'package:smith_client/main.dart';
import 'package:smith_client/projects.dart';
import 'package:smith_client/ui/projects_view.dart';
import 'package:smith_client/ui/smith_icon.dart';
import 'package:smith_client/ui/settings_view.dart';
import 'package:smith_client/workspace_controller.dart';
import 'package:smith_client/workspace_preferences.dart';
import 'package:smith_client/window_launcher.dart';

import 'app_fakes.dart';
import 'projects_view_test.dart'
    show
        ProjectsUiApi,
        alphaProject,
        betaProject,
        loadProjectTestFonts,
        restartedProjectInstance;
import 'split_workspace_widget_test.dart' show inside, layout;
import 'widget_test.dart' show FakeControlApi, profile;
import 'workspace_restore_test.dart' show MemoryStore;

WorkspaceController projectWorkspace(WidgetTester tester) =>
    tester.widget<ConversationScreen>(find.byType(ConversationScreen)).chat
        as WorkspaceController;
WorkspacePresentation projectPresentation(WidgetTester tester) => tester
    .widget<ConversationScreen>(find.byType(ConversationScreen))
    .presentation!;

Future<void> mountProjectWorkspace(
  WidgetTester tester,
  ProjectsUiApi api, {
  MemoryStore? store,
}) async {
  tester.view.physicalSize = const Size(2600, 1000);
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

Future<void> projectPage(WidgetTester tester, int tile, String tooltip) async {
  if (tooltip == 'Projects' &&
      inside(tile, find.byTooltip(tooltip)).evaluate().isEmpty) {
    await tester.tap(inside(tile, find.byTooltip('Dev navigation')).first);
    await tester.pumpAndSettle();
    if (find.byType(SimpleDialog).evaluate().isNotEmpty) {
      await tester.tap(find.widgetWithText(SimpleDialogOption, 'Projects'));
      await tester.pumpAndSettle();
      return;
    }
  }
  await tester.tap(inside(tile, find.byTooltip(tooltip)).first);
  await tester.pumpAndSettle();
}

Finder paneKey(int pane, String key) => inside(pane, find.byKey(ValueKey(key)));
Future<void> paneTap(WidgetTester tester, int pane, String key) async {
  await tester.pumpAndSettle();
  final target = paneKey(pane, key);
  // Scroll the Projects region, not the surrounding two-axis workspace canvas.
  await Scrollable.of(tester.element(target)).position
      .ensureVisible(tester.renderObject(target));
  await tester.pumpAndSettle();
  await tester.tap(target);
  await tester.pumpAndSettle();
}

Future<void> configurePaneRepository(
  WidgetTester tester,
  int pane,
  int index,
) async {
  final expand = 'expand-project-repository-$index';
  if (tester
      .widget<IconButton>(paneKey(pane, expand))
      .tooltip!
      .startsWith('Configure')) {
    await paneTap(tester, pane, expand);
  }
  final readOnly = 'project-repository-$index-read-only';
  if (tester.widget<SwitchListTile>(paneKey(pane, readOnly)).value) {
    await paneTap(tester, pane, readOnly);
  }
  final feature = 'project-repository-$index-feature-selected';
  if (!tester.widget<CheckboxListTile>(paneKey(pane, feature)).value!) {
    await paneTap(tester, pane, feature);
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(loadProjectTestFonts);
  testWidgets(
    'same-revision restart fences each pane editor independently without losing chat drafts',
    (tester) async {
      final api = ProjectsUiApi()..projectRevision = 2;
      await mountProjectWorkspace(tester, api);
      final workspace = projectWorkspace(tester);
      workspace.selected.edit('Retained chat draft through restart');
      await layout(tester, 2, 1);
      await projectPage(tester, 0, 'Projects');
      await projectPage(tester, 1, 'Projects');
      await paneTap(tester, 0, 'project-$alphaProject');
      await paneTap(tester, 1, 'project-$betaProject');
      await tester.enterText(
        paneKey(0, 'project-key'),
        'pane-zero-before-restart',
      );
      await tester.enterText(
        paneKey(1, 'project-key'),
        'pane-one-before-restart',
      );
      await tester.pumpAndSettle();
      final oldSaves = [
        for (var i = 0; i < 2; i++)
          tester.widget<FilledButton>(paneKey(i, 'save-project')).onPressed!,
      ];
      final controller = tester
          .widget<ProjectsView>(inside(0, find.byType(ProjectsView)))
          .controller;
      api.instance = restartedProjectInstance;
      await controller.refresh();
      await tester.pumpAndSettle();
      for (var i = 0; i < 2; i++) {
        expect(
          tester.widget<FilledButton>(paneKey(i, 'save-project')).onPressed,
          isNull,
        );
        oldSaves[i]();
      }
      await tester.pumpAndSettle();
      expect(api.writes, isEmpty);
      await paneTap(tester, 0, 'review-project-revision');
      await tester.tap(
        find.widgetWithText(TextButton, 'Keep edits at this revision'),
      );
      await tester.pumpAndSettle();
      await paneTap(tester, 0, 'save-project');
      expect(api.writes, ['project_update_v3']);
      expect(api.records[alphaProject]!['key'], 'pane-zero-before-restart');
      expect(api.records[alphaProject]!['name'], 'alpha');
      expect(
        tester.widget<FilledButton>(paneKey(1, 'save-project')).onPressed,
        isNull,
      );
      expect(
        tester.widget<TextField>(paneKey(1, 'project-key')).controller!.text,
        'pane-one-before-restart',
      );
      expect(workspace.selected.draft, 'Retained chat draft through restart');
      final args = api.arguments.singleWhere(
        (args) => args.containsKey('expected_revision'),
      );
      expect(args['instance_id'], restartedProjectInstance);
      expect(args['expected_revision'], 2);
      expect(api.calls.where((op) => op.startsWith('submit')), isEmpty);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'keyboard Dev disclosure exposes Projects and Agents before Settings',
    (tester) async {
      final api = ProjectsUiApi();
      await mountProjectWorkspace(tester, api);
      await tester.tap(inside(0, find.byTooltip('Dev navigation')));
      await tester.pumpAndSettle();
      final icon = inside(
        0,
        find.descendant(
          of: find.byTooltip('Projects'),
          matching: find.byType(SmithIcon),
        ),
      );
      Focus.of(tester.element(icon)).requestFocus();
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(projectPresentation(tester).pages[0], 'projects');
      expect(find.byType(ProjectsView), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(projectPresentation(tester).pages[0], 'settings');
      expect(find.byType(SettingsView), findsOneWidget);
      await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(projectPresentation(tester).pages[0], 'projects');
      expect(api.projectCalls, ['projects_list_v3']);
      expect(api.writes, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );
  testWidgets(
    'Projects is lazy and explicit; tab switches retain mounted chat, draft, reading and project edits',
    (tester) async {
      final api = ProjectsUiApi();
      for (var i = 0; i < 32; i++) {
        api.history.add({
          'role': i.isEven ? 'user' : 'assistant',
          'content':
              'Retained conversation line $i\nReading position stays put\nAnother line',
        });
      }
      await mountProjectWorkspace(tester, api);
      final chat = projectWorkspace(tester).selected;
      final chatState = tester.state(inside(0, find.byType(ChatPanel)));
      final draftField = inside(0, find.byType(TextField));
      final editor = tester.widget<TextField>(draftField).controller!;
      await tester.enterText(draftField, 'Unsent chat draft');
      editor.selection = const TextSelection(baseOffset: 2, extentOffset: 8);
      final history = tester
          .widget<SingleChildScrollView>(
            inside(0, find.byKey(const ValueKey('conversation-history'))),
          )
          .controller!;
      history.jumpTo(30);
      await tester.pumpAndSettle();
      final reading = history.offset;
      expect(api.projectCalls, isEmpty);
      expect(find.byType(ProjectsView, skipOffstage: false), findsNothing);
      final callsBeforeDev = List.of(api.calls);
      await projectPage(tester, 0, 'Dev navigation');
      await projectPage(tester, 0, 'Dev navigation');
      await projectPage(tester, 0, 'Dev');
      expect(api.calls, callsBeforeDev);
      expect(
        editor.selection,
        const TextSelection(baseOffset: 2, extentOffset: 8),
      );
      await projectPage(tester, 0, 'Settings');
      expect(api.projectCalls, isEmpty);
      await projectPage(tester, 0, 'Projects');
      expect(api.projectCalls, ['projects_list_v3']);
      expect(find.byType(SettingsView), findsNothing);
      expect(inside(0, find.byTooltip('New chat')), findsNothing);
      expect(
        inside(0, find.byKey(const ValueKey('compact-chat-title'))),
        findsNothing,
      );
      final projectsState = tester.state(find.byType(ProjectsView));
      expect(
        FocusManager.instance.primaryFocus?.context
            ?.findAncestorWidgetOfExactType<ChatPanel>(),
        isNull,
      );
      await paneTap(tester, 0, 'project-$alphaProject');
      await tester.enterText(paneKey(0, 'project-key'), 'unsent-project-draft');
      await paneTap(tester, 0, 'add-project-repository');
      await tester.enterText(
        paneKey(0, 'project-repository-0'),
        'git@example.invalid:team/backend.git',
      );
      await configurePaneRepository(tester, 0, 0);
      await tester.enterText(paneKey(0, 'project-repository-0-base'), 'main');
      await tester.enterText(
        paneKey(0, 'project-repository-0-feature'),
        'feature/*',
      );
      await projectPage(tester, 0, 'Settings');
      expect(find.byType(ProjectsView), findsNothing);
      expect(find.byType(ProjectsView, skipOffstage: false), findsOneWidget);
      expect(paneKey(0, 'project-key'), findsNothing);
      expect(find.byKey(const ValueKey('global-instructions')), findsOneWidget);
      await projectPage(tester, 0, 'Dev');
      await projectPage(tester, 0, 'Dev navigation');
      await projectPage(tester, 0, 'Projects');
      expect(tester.state(find.byType(ProjectsView)), same(projectsState));
      expect(
        tester.widget<TextField>(paneKey(0, 'project-key')).controller!.text,
        'unsent-project-draft',
      );
      expect(
        tester
            .widget<TextField>(paneKey(0, 'project-repository-0'))
            .controller!
            .text,
        'git@example.invalid:team/backend.git',
      );
      expect(
        tester
            .widget<TextField>(paneKey(0, 'project-repository-0-base'))
            .controller!
            .text,
        'main',
      );
      expect(
        tester
            .widget<TextField>(paneKey(0, 'project-repository-0-feature'))
            .controller!
            .text,
        'feature/*',
      );
      await projectPage(tester, 0, 'Return to Chat');
      expect(tester.state(inside(0, find.byType(ChatPanel))), same(chatState));
      expect(
        tester.widget<TextField>(inside(0, find.byType(TextField))).controller,
        same(editor),
      );
      expect(editor.text, 'Unsent chat draft');
      expect(chat.draft, 'Unsent chat draft');
      expect(history.offset, reading);
      expect(api.projectCalls, ['projects_list_v3']);
      expect(api.calls.where((op) => op.startsWith('submit')), isEmpty);
      expect(api.writes, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      expect(api.disposedCount, 1);
    },
  );

  testWidgets(
    'panes share one coalesced registry but retain independent selections and dirty editors through hiding',
    (tester) async {
      final api = ProjectsUiApi()..listGate = Completer<AppReply>();
      await mountProjectWorkspace(tester, api);
      await layout(tester, 2, 1);
      final presentation = projectPresentation(tester);
      presentation.setPage(0, 'projects');
      presentation.setPage(1, 'projects');
      await tester.pumpAndSettle();
      expect(api.projectCalls, ['projects_list_v3']);
      final views = tester
          .widgetList<ProjectsView>(find.byType(ProjectsView))
          .toList();
      expect(views, hasLength(2));
      expect(views[0].controller, same(views[1].controller));
      api.listGate!.complete(
        AppReply(api.instance, {
          'revision': 0,
          'projects': api.records.values.toList(),
        }),
      );
      api.listGate = null;
      await tester.pumpAndSettle();
      await paneTap(tester, 0, 'project-$alphaProject');
      await paneTap(tester, 1, 'project-$betaProject');
      await tester.enterText(paneKey(0, 'project-key'), 'pane-zero-edits');
      await tester.enterText(paneKey(1, 'project-key'), 'pane-one-edits');
      final state = tester.state(inside(1, find.byType(ProjectsView)));
      await layout(tester, 1, 1);
      await layout(tester, 2, 1);
      expect(tester.state(inside(1, find.byType(ProjectsView))), same(state));
      expect(
        tester.widget<TextField>(paneKey(0, 'project-key')).controller!.text,
        'pane-zero-edits',
      );
      expect(
        tester.widget<TextField>(paneKey(1, 'project-key')).controller!.text,
        'pane-one-edits',
      );
      await paneTap(tester, 0, 'save-project');
      expect(api.records[alphaProject]!['key'], 'pane-zero-edits');
      expect(
        tester.widget<FilledButton>(paneKey(1, 'save-project')).onPressed,
        isNull,
      );
      expect(
        tester.widget<TextField>(paneKey(1, 'project-key')).controller!.text,
        'pane-one-edits',
      );
      expect(api.writes, ['project_update_v3']);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      expect(api.disposedCount, 1);
    },
  );

  testWidgets(
    'accepted chat and local queue survive Projects and hidden panes without dispatch or inference effects',
    (tester) async {
      final api = ProjectsUiApi()..holdChats = true;
      await mountProjectWorkspace(tester, api);
      final workspace = projectWorkspace(tester);
      final chat = workspace.selected;
      chat.edit('Explicit original chat send');
      await chat.send();
      chat.edit('Waiting only in this desktop');
      chat.messageQueue.enqueue();
      chat.messageQueue.pause();
      chat.edit('Preserve newer draft');
      await tester.pumpAndSettle();
      final pending = chat.pendingId;
      final entries = chat.messageQueue.entries
          .map((entry) => (entry.id, entry.text, entry.state))
          .toList();
      final sends = api.calls.where((op) => op.startsWith('submit')).length;
      await projectPage(tester, 0, 'Projects');
      await paneTap(tester, 0, 'new-project');
      await tester.enterText(paneKey(0, 'project-key'), 'still-not-saved');
      await layout(tester, 2, 1);
      await projectPage(tester, 0, 'Settings');
      await layout(tester, 1, 1);
      await projectPage(tester, 0, 'Return to Chat');
      expect(workspace.selected, same(chat));
      expect(chat.pendingId, pending);
      expect(chat.draft, 'Preserve newer draft');
      expect(
        chat.messageQueue.entries.map(
          (entry) => (entry.id, entry.text, entry.state),
        ),
        entries,
      );
      expect(chat.messageQueue.paused, isTrue);
      expect(
        api.calls.where((op) => op.startsWith('submit')),
        hasLength(sends),
      );
      expect(api.writes, isEmpty);
      expect(api.projectCalls, ['projects_list_v3']);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'restored Projects queries metadata only; preferences never contain project selection or edits',
    (tester) async {
      final saved = WorkspacePreferences(
        layout: '1x1',
        activeTile: 0,
        slots: [
          WorkspaceSlotPreferences(
            conversationId: testConversation,
            page: 'projects',
          ),
          for (var i = 1; i < 4; i++)
            WorkspaceSlotPreferences(conversationId: null, page: 'chat'),
        ],
      );
      final store = MemoryStore(saved);
      final api = ProjectsUiApi();
      await mountProjectWorkspace(tester, api, store: store);
      expect(find.text('No project selected'), findsOneWidget);
      expect(api.projectCalls, ['projects_list_v3']);
      expect(store.writes, isEmpty);
      await paneTap(tester, 0, 'project-$alphaProject');
      await tester.enterText(
        paneKey(0, 'project-key'),
        'never-persist-project-edit',
      );
      await tester.enterText(
        paneKey(0, 'project-description'),
        'Private project description',
      );
      await tester.enterText(
        paneKey(0, 'project-prompt'),
        'Private\nproject context',
      );
      await paneTap(tester, 0, 'add-project-repository');
      await tester.enterText(
        paneKey(0, 'project-repository-0'),
        'https://example.invalid/private/unsaved',
      );
      expect(store.writes, isEmpty);
      await projectPage(tester, 0, 'Settings');
      await projectPage(tester, 0, 'Projects');
      final encoded = jsonEncode(store.value!.toJson());
      expect(store.value!.toJson()['version'], 4);
      expect(store.value!.slots[0].page, 'projects');
      for (final value in [
        alphaProject,
        'never-persist',
        'example.invalid',
        'projectId',
        'repositories',
        'description',
        'prompt',
        'Private project',
        'editor',
      ]) {
        expect(encoded, isNot(contains(value)));
      }
      expect(api.writes, isEmpty);
      await tester.pumpWidget(const SizedBox());
      await tester.pumpAndSettle();
      final restoredApi = ProjectsUiApi();
      await mountProjectWorkspace(
        tester,
        restoredApi,
        store: MemoryStore(store.value),
      );
      expect(find.text('No project selected'), findsOneWidget);
      expect(paneKey(0, 'project-key'), findsNothing);
      expect(restoredApi.projectCalls, ['projects_list_v3']);
      expect(restoredApi.writes, isEmpty);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'four retained Projects panes preserve scroll, selections and disclosure through every layout, destination and hidden focus fence',
    (tester) async {
      final api = ProjectsUiApi();
      final repositories = [
        for (var i = 0; i < 3; i++)
          ProjectRepository(url: 'https://example.invalid/team/repo-$i'),
      ];
      api.records[alphaProject] = api.record(
        alphaProject,
        'alpha',
        repositories,
      );
      for (var i = 0; i < 20; i++) {
        final id = i.toRadixString(16).padLeft(32, '0');
        api.records[id] = api.record(
          id,
          'Other project $i',
          [],
          'alpha-other-$i',
        );
      }
      await mountProjectWorkspace(tester, api);
      await layout(tester, 2, 2);
      final states = <State>[];
      final names = <TextEditingController>[];
      final patterns = <TextEditingController>[];
      final descriptions = <TextEditingController>[];
      final prompts = <TextEditingController>[];
      final searches = <TextEditingController>[];
      final focuses = <FocusNode>[];
      final lists = <ScrollController>[];
      final editors = <ScrollController>[];
      for (var i = 0; i < 4; i++) {
        await projectPage(tester, i, 'Projects');
        await paneTap(tester, i, 'project-$alphaProject');
        await tester.enterText(
          paneKey(i, 'project-key'),
          'pane-$i-local-draft',
        );
        await tester.enterText(
          paneKey(i, 'project-description'),
          'Pane $i description',
        );
        await tester.enterText(
          paneKey(i, 'project-prompt'),
          'Pane $i\ncontext',
        );
        await tester.enterText(
          paneKey(i, 'search-projects'),
          i.isEven ? ' ALPHA ' : 'alpha',
        );
        descriptions.add(
          tester
              .widget<TextField>(paneKey(i, 'project-description'))
              .controller!,
        );
        prompts.add(
          tester.widget<TextField>(paneKey(i, 'project-prompt')).controller!,
        );
        searches.add(
          tester.widget<TextField>(paneKey(i, 'search-projects')).controller!,
        );
        await configurePaneRepository(tester, i, 0);
        await tester.enterText(
          paneKey(i, 'project-repository-0-feature'),
          'pane-$i/*',
        );
        states.add(tester.state(inside(i, find.byType(ProjectsView))));
        names.add(
          tester.widget<TextField>(paneKey(i, 'project-key')).controller!,
        );
        patterns.add(
          tester
              .widget<TextField>(paneKey(i, 'project-repository-0-feature'))
              .controller!,
        );
        focuses.add(
          tester
              .widget<EditableText>(
                find.descendant(
                  of: paneKey(i, 'project-repository-0-feature'),
                  matching: find.byType(EditableText),
                ),
              )
              .focusNode,
        );
        lists.add(
          tester
              .widget<SingleChildScrollView>(paneKey(i, 'project-list'))
              .controller!,
        );
        editors.add(
          tester
              .widget<SingleChildScrollView>(
                paneKey(i, 'project-editor-scroll'),
              )
              .controller!,
        );
      }
      expect(lists.toSet(), hasLength(4));
      expect(editors.toSet(), hasLength(4));
      expect({...lists, ...editors}, hasLength(8));
      tester.view.physicalSize = const Size(1600, 900);
      await tester.pumpAndSettle();
      for (var i = 0; i < 4; i++) {
        // The window is wide; each actual pane is compact.
        expect(paneKey(i, 'back-to-projects'), findsOneWidget);
        names[i].selection = const TextSelection(
          baseOffset: 1,
          extentOffset: 5,
        );
        patterns[i].selection = const TextSelection(
          baseOffset: 1,
          extentOffset: 4,
        );
        descriptions[i].selection = const TextSelection(
          baseOffset: 1,
          extentOffset: 5,
        );
        prompts[i].selection = const TextSelection(
          baseOffset: 1,
          extentOffset: 4,
        );
      }
      await tester.pumpAndSettle();
      for (var i = 0; i < 4; i++) {
        lists[i].jumpTo(200);
        editors[i].jumpTo(300);
      }
      await tester.pumpAndSettle();
      await projectPage(tester, 0, 'Settings');
      expect(focuses[0].canRequestFocus, isFalse);
      focuses[0].requestFocus();
      await tester.pump();
      expect(focuses[0].hasFocus, isFalse);
      await projectPage(tester, 0, 'Projects');
      focuses[3].requestFocus();
      await tester.pumpAndSettle();
      // Focus visibility can scroll the focused field; establish its bookmark
      // afterwards, independently of the layout's focus repair.
      editors[3].jumpTo(300);
      await tester.pumpAndSettle();
      for (final (columns, rows) in [(1, 1), (1, 2), (2, 1), (2, 2)]) {
        await layout(tester, columns, rows);
        if (columns * rows < 4) {
          expect(focuses[3].hasFocus, isFalse);
          expect(focuses[3].canRequestFocus, isFalse);
          focuses[3].requestFocus();
          await tester.pump();
          expect(focuses[3].hasFocus, isFalse);
        }
        for (var i = 0; i < 4; i++) {
          expect(names[i].text, 'pane-$i-local-draft');
          expect(
            names[i].selection,
            const TextSelection(baseOffset: 1, extentOffset: 5),
          );
          expect(descriptions[i].text, 'Pane $i description');
          expect(prompts[i].text, 'Pane $i\ncontext');
          expect(searches[i].text, i.isEven ? ' ALPHA ' : 'alpha');
          expect(
            descriptions[i].selection,
            const TextSelection(baseOffset: 1, extentOffset: 5),
          );
          expect(
            prompts[i].selection,
            const TextSelection(baseOffset: 1, extentOffset: 4),
          );
          expect(patterns[i].text, 'pane-$i/*');
          expect(
            patterns[i].selection,
            const TextSelection(baseOffset: 1, extentOffset: 4),
          );
          expect(lists[i].hasClients, isTrue);
          expect(editors[i].hasClients, isTrue);
        }
      }
      for (var i = 0; i < 4; i++) {
        expect(
          tester.state(inside(i, find.byType(ProjectsView))),
          same(states[i]),
        );
        expect(
          tester.widget<TextField>(paneKey(i, 'project-key')).controller,
          same(names[i]),
        );
        expect(
          tester
              .widget<TextField>(paneKey(i, 'project-description'))
              .controller,
          same(descriptions[i]),
        );
        expect(
          tester.widget<TextField>(paneKey(i, 'project-prompt')).controller,
          same(prompts[i]),
        );
        expect(
          tester
              .widget<TextField>(
                find
                    .byKey(ValueKey('search-projects'), skipOffstage: false)
                    .at(i),
              )
              .controller,
          same(searches[i]),
        );
        expect(paneKey(i, 'project-repository-0-feature'), findsOneWidget);
        expect(lists[i].offset, 200);
        expect(editors[i].offset, 300);
      }
      final back = tester
          .widget<TextButton>(paneKey(3, 'back-to-projects'))
          .onPressed!;
      back();
      await tester.pumpAndSettle();
      expect(paneKey(3, 'project-key'), findsNothing);
      expect(focuses[3].canRequestFocus, isFalse);
      expect(lists[3].offset, 200);
      await paneTap(tester, 3, 'project-$alphaProject');
      expect(
        tester.widget<TextField>(paneKey(3, 'project-key')).controller,
        same(names[3]),
      );
      expect(paneKey(3, 'project-repository-0-feature'), findsOneWidget);
      expect(editors[3].offset, 300);
      expect(api.projectCalls, ['projects_list_v3']);
      expect(api.writes, isEmpty);
      expect(api.calls.where((op) => op.startsWith('submit')), isEmpty);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'session-only startup never constructs preferences, still uses the shared service registry',
    (tester) async {
      final store = workspacePreferencesForStartup([
        sessionOnlyWindowArgument,
      ], () => throw StateError('Must not access preference storage'));
      expect(store, isNull);
      final api = ProjectsUiApi();
      await mountProjectWorkspace(tester, api);
      expect(projectPresentation(tester).store, isNull);
      expect(projectPresentation(tester).pages, [
        'chat',
        'chat',
        'chat',
        'chat',
      ]);
      expect(api.projectCalls, isEmpty);
      await projectPage(tester, 0, 'Projects');
      await paneTap(tester, 0, 'project-$alphaProject');
      await tester.enterText(
        paneKey(0, 'project-key'),
        'session-only-saved-metadata',
      );
      await paneTap(tester, 0, 'save-project');
      expect(api.records[alphaProject]!['key'], 'session-only-saved-metadata');
      expect(api.writes, ['project_update_v3']);
      await tester.pumpWidget(const SizedBox());
      expect(api.disposedCount, 1);
    },
  );
}
