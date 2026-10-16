import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/chat_panel.dart';
import 'package:smith_client/cloud_routing.dart';
import 'package:smith_client/main.dart' show ConversationScreen;
import 'package:smith_client/ui/cloud_selector.dart';
import 'package:smith_client/ui/smith_theme.dart';
import 'package:smith_client/ui/split_workspace.dart';
import 'package:smith_client/workspace_controller.dart';

import 'cloud_routing_test.dart';
import 'widget_test.dart' show FakeControlApi, profile;

const _filter = ValueKey('chat-route-filter');
const _trigger = ValueKey('chat-route-selector');
const _sol = ChatRoute('openai', 'gpt-6.1-sol', generation);

class _Host extends StatefulWidget {
  const _Host({super.key, required this.controller});
  final ConversationController controller;
  @override
  State<_Host> createState() => _HostState();
}

class _HostState extends State<_Host> {
  late ConversationController controller = widget.controller;
  bool active = true;
  bool visible = true;
  void update(VoidCallback change) => setState(change);
  @override
  Widget build(BuildContext context) => Scaffold(
    body: visible
        ? CloudSelector(controller: controller, active: active)
        : const SizedBox(),
  );
}

Future<GlobalKey<_HostState>> _mount(
  WidgetTester tester,
  ConversationController c,
) async {
  final key = GlobalKey<_HostState>();
  await tester.pumpWidget(
    MaterialApp(
      theme: SmithTheme.dark(),
      home: _Host(key: key, controller: c),
    ),
  );
  await tester.pumpAndSettle();
  return key;
}

Future<void> _open(WidgetTester tester) async {
  await tester.tap(find.byKey(_trigger));
  await tester.pumpAndSettle();
  expect(find.text('Choose model'), findsOneWidget);
}

Future<void> _search(WidgetTester tester, String query) async {
  await tester.enterText(find.byKey(_filter), query);
  await tester.pumpAndSettle();
}

Future<void> _choose(WidgetTester tester, ChatRoute route) async {
  final row = find.byKey(ValueKey(route));
  final label = find.descendant(of: row, matching: find.byType(Text)).first;
  await tester.ensureVisible(label);
  await tester.pumpAndSettle();
  await tester.tap(label);
  await tester.pumpAndSettle();
}

String _snapshot(CloudFake api, ConversationController c) => jsonEncode({
  'calls': api.calls,
  'arguments': api.arguments,
  'accepted': api.accepted,
  'route': c.selectedRoute?.toJson(),
  'route_revision': c.routeRevision,
  'route_chosen': c.routeChosen,
  'draft': c.draft,
  'draft_revision': c.draftRevision,
  'history': c.history,
  'pending': c.pendingId,
  'config': c.owner.config,
  'queue_revision': c.messageQueue.revision,
  'queue_paused': c.messageQueue.paused,
  'queue_reason': c.messageQueue.pauseReason,
  'queue': [
    for (final e in c.messageQueue.entries)
      [e.id, e.text, e.state.name, e.status, e.actionRevision],
  ],
});

Future<void> _unmount(WidgetTester tester, WorkspaceController w) async {
  await tester.pumpWidget(const SizedBox());
  await tester.pumpAndSettle();
  w.dispose();
  expect(tester.takeException(), isNull);
}

void main() {
  for (final name in cloudModels.keys) {
    for (final state in [
      'not_observed',
      'locked',
      'missing',
      'unavailable',
      'conflict',
    ]) {
      testWidgets(
        '$name: $state and historical validation are advisory with no picker effects',
        (tester) async {
          final api = CatalogFake();
          final status = state == 'missing'
              ? 'interrupted'
              : state == 'not_observed'
              ? null
              : 'failed';
          setProviderFacts(
            api.catalogValue,
            name,
            state: state,
            validationStatus: status,
          );
          final w = await readyCloud(api);
          final c = w.selected;
          final route = ChatRoute(name, cloudModels[name]!.first, generation);
          c.edit('preserve private draft');
          await _mount(tester, c);
          final before = _snapshot(api, c);
          await _open(tester);
          await _search(tester, route.model);
          expect(
            find.textContaining('Not individually verified'),
            findsWidgets,
          );
          expect(
            find.textContaining(switch (state) {
              'not_observed' =>
                'Credential access has not been checked since restart.',
              'locked' => 'Last credential check: keyring locked.',
              'missing' => 'Last credential check: saved item missing.',
              'conflict' => 'Last credential check: keyring conflict.',
              _ => 'Last credential check: KEYRING_UNAVAILABLE.',
            }),
            findsWidgets,
          );
          if (status == null) {
            expect(find.textContaining('Validation is optional'), findsWidgets);
          } else {
            expect(
              find.textContaining(
                'Last validation $status for ${route.model}: CLOUD_AUTH.',
              ),
              findsWidgets,
            );
          }
          if (status == 'interrupted') {
            expect(
              find.textContaining('may have been processed or billed'),
              findsWidgets,
            );
          }
          expect(
            tester.widget<TextButton>(find.byKey(ValueKey(route))).onPressed,
            isNotNull,
          );
          await tester.tap(find.text('Cancel'));
          await tester.pumpAndSettle();
          expect(_snapshot(api, c), before);
          await _open(tester);
          await _search(tester, route.model);
          await _choose(tester, route);
          expect(c.selectedRoute, route);
          expect(c.canSend, true);
          expect(api.calls, jsonDecode(before)['calls']);
          expect(api.accepted, isEmpty);
          expect(c.draft, 'preserve private draft');
          expect(
            api.catalogValue['providers'].firstWhere(
              (p) => p['provider'] == name,
            )['credential_state'],
            state,
          );
          await _unmount(tester, w);
        },
      );
    }
    testWidgets(
      '$name: cached locked observation does not erase exact past model verification',
      (tester) async {
        final api = CatalogFake();
        setProviderFacts(
          api.catalogValue,
          name,
          state: 'locked',
          validationStatus: 'succeeded',
        );
        final w = await readyCloud(api);
        final c = w.selected;
        await _mount(tester, c);
        await _open(tester);
        await _search(tester, cloudModels[name]!.first);
        expect(
          find.textContaining(
            'Past validation succeeded for ${cloudModels[name]!.first}',
          ),
          findsWidgets,
        );
        expect(
          find.textContaining('not a current availability guarantee'),
          findsWidgets,
        );
        expect(
          find.textContaining('Last credential check: keyring locked.'),
          findsWidgets,
        );
        expect(find.textContaining('Not individually verified'), findsNothing);
        await tester.tap(find.text('Cancel'));
        await tester.pumpAndSettle();
        await _unmount(tester, w);
      },
    );
  }
  testWidgets(
    'trimmed case-insensitive label/ID/provider/group search has no effects',
    (tester) async {
      final api = CloudFake();
      final w = await readyCloud(api);
      final c = w.selected;
      final value = catalog();
      value['models'][0]['label'] = 'Friendly workstation';
      w.routeCatalog = RouteCatalog(value);
      c.edit('retain draft');
      await _mount(tester, c);
      final before = _snapshot(api, c);
      await _open(tester);
      expect(
        tester.widget<TextField>(find.byKey(_filter)).focusNode!.hasFocus,
        true,
      );
      expect(find.text('LOCAL'), findsOneWidget);
      expect(find.text('CLOUD'), findsOneWidget);
      for (final query in ['  FRIENDLY  ', 'LOCAL', 'local:exact']) {
        await _search(tester, query);
        expect(find.byKey(const ValueKey(local)), findsOneWidget);
        expect(find.byKey(const ValueKey(cloud)), findsNothing);
        expect(find.text('CLOUD'), findsNothing);
      }
      await _search(tester, '  OpEnAi  ');
      expect(find.byKey(const ValueKey(cloud)), findsOneWidget);
      expect(find.text('LOCAL'), findsNothing);
      await _search(tester, 'Anthropic');
      expect(find.byKey(const ValueKey(cloud)), findsNothing);
      expect(
        find.byKey(
          const ValueKey(
            ChatRoute('anthropic', 'claude-sonnet-5-5', generation),
          ),
        ),
        findsOneWidget,
      );
      await _search(tester, 'gpt-6.1-sol');
      expect(find.byKey(const ValueKey(_sol)), findsOneWidget);
      expect(find.textContaining('Current: LOCAL'), findsOneWidget);
      await _search(tester, 'no-such-model');
      expect(find.text('No matching models'), findsOneWidget);
      expect(find.textContaining('Current: LOCAL'), findsOneWidget);
      await tester.tap(find.byTooltip('Clear filter'));
      await tester.pumpAndSettle();
      expect(
        tester.widget<TextField>(find.byKey(_filter)).controller!.text,
        isEmpty,
      );
      expect(find.text('LOCAL'), findsOneWidget);
      expect(find.text('CLOUD'), findsOneWidget);
      await tester.tap(find.text('Cancel'));
      await tester.pumpAndSettle();
      expect(_snapshot(api, c), before);
      expect(
        tester.widget<TextButton>(find.byKey(_trigger)).focusNode!.hasFocus,
        true,
      );
      await _unmount(tester, w);
    },
  );

  testWidgets(
    'opening/filtering/clear/Escape/barrier and current selection never pause a queue',
    (tester) async {
      final api = CloudFake();
      final w = await readyCloud(api);
      final c = w.selected;
      c.edit('accepted local');
      await c.send();
      c.edit('waiting local');
      c.messageQueue.enqueue();
      c.edit('newer draft');
      await _mount(tester, c);
      final before = _snapshot(api, c);
      await _open(tester);
      await _search(tester, 'local');
      await _choose(tester, local);
      expect(_snapshot(api, c), before);
      await _open(tester);
      await _search(tester, 'sol');
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(_snapshot(api, c), before);
      await _open(tester);
      await tester.tapAt(const Offset(2, 2));
      await tester.pumpAndSettle();
      expect(find.byType(Dialog), findsNothing);
      expect(_snapshot(api, c), before);
      await _unmount(tester, w);
    },
  );

  testWidgets(
    'enabled explicit choice is future-only and cannot resume/rebind waiting text',
    (tester) async {
      final api = CloudFake();
      final w = await readyCloud(api);
      final c = w.selected;
      c.edit('accepted local');
      await c.send();
      final request = c.pendingId!;
      c.edit('waiting local');
      c.messageQueue.enqueue();
      c.edit('untouched draft');
      await _mount(tester, c);
      final calls = List.of(api.calls);
      final revision = c.routeRevision;
      await _open(tester);
      await _search(tester, 'gpt-6.1-sol');
      expect(find.text('Not individually verified'), findsOneWidget);
      await _choose(tester, _sol);
      expect(c.selectedRoute, _sol);
      expect(c.routeRevision, revision + 1);
      expect(c.draft, 'untouched draft');
      expect(c.pendingId, request);
      expect(api.accepted[request]!['route'], local.toJson());
      expect(c.requestRoutes[request]!.route, local);
      expect(c.messageQueue.paused, true);
      expect(c.messageQueue.entries.last.status, contains('LOCAL'));
      expect(api.calls, calls);
      expect(api.accepted, hasLength(1));
      expect(find.byTooltip('Review queued cloud send'), findsOneWidget);
      final before = _snapshot(api, c);
      await _open(tester);
      await _search(tester, 'gpt-6.1-sol');
      await _choose(tester, _sol);
      expect(_snapshot(api, c), before);
      await _unmount(tester, w);
    },
  );

  testWidgets(
    'unavailable historic selection survives filtering and disabled reasons remain readable',
    (tester) async {
      final api = CloudFake();
      final w = await readyCloud(api);
      final c = w.selected;
      const historic = ChatRoute(
        'openai',
        'gpt-6-astra',
        'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',
      );
      c.selectRoute(historic);
      c.edit('history and draft retained');
      final value = catalog();
      setProviderFacts(value, 'openai', state: 'not_configured');
      w.routeCatalog = RouteCatalog(value);
      await _mount(tester, c);
      final before = _snapshot(api, c);
      await _open(tester);
      await _search(tester, 'gpt-6-astra');
      expect(
        find.textContaining(
          'Current: CLOUD · OPENAI · gpt-6-astra · unavailable',
        ),
        findsOneWidget,
      );
      expect(find.text('Save a key in the installer.'), findsOneWidget);
      final row = find.byKey(
        const ValueKey(ChatRoute('openai', 'gpt-6-astra', null)),
      );
      expect(tester.widget<TextButton>(row).onPressed, isNull);
      await tester.ensureVisible(row);
      await tester.tap(row);
      await tester.pumpAndSettle();
      expect(c.selectedRoute, historic);
      await _search(tester, 'no matches');
      expect(find.textContaining('Current: CLOUD'), findsOneWidget);
      await tester.tap(find.text('Cancel'));
      await tester.pumpAndSettle();
      expect(_snapshot(api, c), before);
      expect(c.canSend, false);
      await _unmount(tester, w);
    },
  );

  for (final change in [
    'disabled',
    'removed',
    'generation',
    'catalog missing',
  ]) {
    testWidgets('stale enabled callback cannot select after $change', (
      tester,
    ) async {
      final api = CloudFake();
      final w = await readyCloud(api);
      final c = w.selected;
      await _mount(tester, c);
      await _open(tester);
      await _search(tester, 'gpt-6.1-sol');
      final callback = tester
          .widget<TextButton>(find.byKey(const ValueKey(_sol)))
          .onPressed!;
      final before = _snapshot(api, c);
      final value = catalog();
      final model = (value['models'] as List).firstWhere(
        (m) => m['route']['model'] == _sol.model,
      );
      switch (change) {
        case 'disabled':
          setProviderFacts(value, 'openai', state: 'not_configured');
        case 'removed':
          value['models'].remove(model);
        case 'generation':
          value['providers'][0]['credential_generation'] =
              'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee';
          value['providers'][0]['validation']['credential_generation'] =
              'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee';
          for (final m in value['models']) {
            if (m['route']['provider'] == 'openai') {
              m['route']['credential_generation'] =
                  'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee';
            }
          }
        case 'catalog missing':
          break;
      }
      w.routeCatalog = change == 'catalog missing' ? null : RouteCatalog(value);
      // Deliberately invoke before a rebuild: displayed readiness is not authority.
      callback();
      await tester.pumpAndSettle();
      expect(_snapshot(api, c), before);
      await tester.tap(find.text('Cancel'));
      await tester.pumpAndSettle();
      await _unmount(tester, w);
    });
  }

  for (final change in [
    'route revision',
    'config revision',
    'instance',
    'origin membership',
    'active',
    'controller',
    'disposed',
  ]) {
    testWidgets('picker origin fences $change including captured callbacks', (
      tester,
    ) async {
      final api = CloudFake();
      final w = await readyCloud(api);
      final c = w.selected;
      final key = await _mount(tester, c);
      await _open(tester);
      await _search(tester, 'gpt-6.1-sol');
      final callback = tester
          .widget<TextButton>(find.byKey(const ValueKey(_sol)))
          .onPressed!;
      switch (change) {
        case 'route revision':
          c.selectRoute(local);
        case 'config revision':
          w.config = {...w.config, 'revision': 2};
        case 'instance':
          w.instanceId = 'cccccccccccccccccccccccccccccccc';
        case 'origin membership':
          w.conversations.remove(c.id);
          w.selected = w.blank;
        case 'active':
          key.currentState!.update(() => key.currentState!.active = false);
        case 'controller':
          final other = w.createPane().selected;
          key.currentState!.update(() => key.currentState!.controller = other);
        case 'disposed':
          key.currentState!.update(() => key.currentState!.visible = false);
      }
      await tester.pumpAndSettle();
      final before = _snapshot(api, c);
      callback();
      await tester.pumpAndSettle();
      expect(_snapshot(api, c), before);
      expect(c.selectedRoute, local);
      if (find.text('Cancel').evaluate().isNotEmpty) {
        await tester.tap(find.text('Cancel'));
        await tester.pumpAndSettle();
      }
      if (change == 'origin membership') c.dispose();
      await _unmount(tester, w);
    });
  }

  testWidgets(
    'dismissed row callbacks are inert, including during the pop transition',
    (tester) async {
      final api = CloudFake();
      final w = await readyCloud(api);
      final c = w.selected;
      await _mount(tester, c);
      await _open(tester);
      await _search(tester, 'gpt-6.1-sol');
      final callback = tester
          .widget<TextButton>(find.byKey(const ValueKey(_sol)))
          .onPressed!;
      final before = _snapshot(api, c);
      await tester.tap(find.text('Cancel'));
      callback();
      await tester.pumpAndSettle();
      callback();
      await tester.pumpAndSettle();
      expect(_snapshot(api, c), before);
      expect(find.byKey(_trigger), findsOneWidget);
      await _unmount(tester, w);
    },
  );

  for (final change in ['route revision', 'availability']) {
    testWidgets('dialog result rechecks $change before committing', (
      tester,
    ) async {
      final api = CloudFake();
      final w = await readyCloud(api);
      final c = w.selected;
      await _mount(tester, c);
      await _open(tester);
      await _search(tester, 'gpt-6.1-sol');
      final callback = tester
          .widget<TextButton>(find.byKey(const ValueKey(_sol)))
          .onPressed!;
      callback();
      // The row passed its guard, but the awaiting trigger must check again.
      if (change == 'route revision') {
        c.selectRoute(local);
      } else {
        w.routeCatalog = null;
      }
      final before = _snapshot(api, c);
      await tester.pumpAndSettle();
      expect(_snapshot(api, c), before);
      expect(find.byType(Dialog), findsNothing);
      await _unmount(tester, w);
    });
  }

  testWidgets(
    'real split-pane picker keeps unsent routes pane-local and fences deactivation',
    (tester) async {
      tester.view.physicalSize = const Size(2200, 1400);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final api = CloudFake();
      api.history.addAll([
        {'role': 'user', 'content': 'Saved question'},
        {'role': 'assistant', 'content': 'Saved answer'},
      ]);
      api.summaries.values.single['history_revision'] = 1;
      final w = await readyCloud(api);
      final saved = w.selected;
      final control = FakeControlApi(profile());
      await tester.pumpWidget(
        MaterialApp(
          theme: SmithTheme.dark(),
          home: ConversationScreen(control: control, chat: w),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('workspace-layout')));
      await tester.pumpAndSettle();
      await tester.tap(
        find.byWidgetPredicate(
          (widget) =>
              widget is CheckedPopupMenuItem<WorkspaceLayout> &&
              widget.value == WorkspaceLayout.sideBySide,
        ),
      );
      await tester.pumpAndSettle();
      Finder inTile(int index, Finder matching) => find.descendant(
        of: find.byKey(ValueKey('workspace-tile-$index')),
        matching: matching,
      );
      Finder editor(int index) =>
          inTile(index, find.byKey(const ValueKey('chat-composer')));
      Finder trigger(int index) => inTile(index, find.byKey(_trigger));
      ChatPanel panel(int index) =>
          tester.widget<ChatPanel>(inTile(index, find.byType(ChatPanel)));
      expect(find.byType(SplitWorkspace), findsOneWidget);
      expect(w.panes, hasLength(2));
      await tester.tap(inTile(0, find.byTooltip('New chat')));
      await tester.pumpAndSettle();
      await tester.tap(editor(0));
      await tester.pumpAndSettle();
      await tester.enterText(editor(0), 'First pane unsent draft');
      await tester.tap(editor(1));
      await tester.pumpAndSettle();
      await tester.enterText(editor(1), 'Second pane unsent draft');
      await tester.pumpAndSettle();
      final first = w.panes[0].selected;
      final second = w.panes[1].selected;
      expect(first, same(w.panes[0].blank));
      expect(second, same(w.panes[1].blank));
      expect(first, isNot(same(second)));
      expect(first.id, isNull);
      expect(second.id, isNull);
      expect(first.selectedRoute, local);
      expect(second.selectedRoute, local);
      expect(first.draft, 'First pane unsent draft');
      expect(second.draft, 'Second pane unsent draft');
      expect(saved.id, isNotNull);
      expect(saved.history, api.history);
      expect(first.routeReady && second.routeReady, true);
      expect(panel(0).active, false);
      expect(panel(1).active, true);
      expect(tester.widget<TextButton>(trigger(0)).onPressed, isNull);
      final calls = List.of(api.calls);
      final savedBefore = _snapshot(api, saved);
      final firstBefore = _snapshot(api, first);
      final secondBefore = _snapshot(api, second);
      await tester.tap(trigger(1));
      await tester.pumpAndSettle();
      expect(find.text('Choose model'), findsOneWidget);
      await _search(tester, 'gpt-6.1-sol');
      await tester.tap(find.byTooltip('Clear filter'));
      await tester.pumpAndSettle();
      expect(
        tester.widget<TextField>(find.byKey(_filter)).controller!.text,
        isEmpty,
      );
      await tester.tap(find.text('Cancel'));
      await tester.pumpAndSettle();
      expect(_snapshot(api, first), firstBefore);
      expect(_snapshot(api, second), secondBefore);
      expect(_snapshot(api, saved), savedBefore);
      await tester.tap(trigger(1));
      await tester.pumpAndSettle();
      await _search(tester, 'gpt-6.1-sol');
      await _choose(tester, _sol);
      expect(second.selectedRoute, _sol);
      expect(second.routeRevision, 1);
      expect(second.routeChosen, true);
      expect(second.routeReady, true);
      expect(second.canSend, true);
      expect(second.id, isNull);
      expect(second.draft, 'Second pane unsent draft');
      expect(second.history, isEmpty);
      expect(second.pendingId, isNull);
      expect(second.messageQueue.entries, isEmpty);
      expect(_snapshot(api, first), firstBefore);
      expect(_snapshot(api, saved), savedBefore);
      final chosen = _snapshot(api, second);
      await tester.tap(trigger(1));
      await tester.pumpAndSettle();
      await _search(tester, 'gpt-6-astra');
      final staleChoice = tester
          .widget<TextButton>(find.byKey(const ValueKey(cloud)))
          .onPressed!;
      // Drive the real ChatPanel/SplitWorkspace focus-activation path while
      // the picker is open, rather than changing a test host's active flag.
      tester.widget<TextField>(editor(0)).focusNode!.requestFocus();
      await tester.pumpAndSettle();
      expect(panel(0).active, true);
      expect(panel(1).active, false);
      expect(tester.widget<TextField>(editor(0)).focusNode!.hasFocus, true);
      expect(find.text('Choose model'), findsNothing);
      staleChoice();
      await tester.pumpAndSettle();
      expect(_snapshot(api, first), firstBefore);
      expect(_snapshot(api, second), chosen);
      await tester.tap(editor(1));
      await tester.pumpAndSettle();
      expect(panel(0).active, false);
      expect(panel(1).active, true);
      staleChoice();
      await tester.pumpAndSettle();
      expect(_snapshot(api, first), firstBefore);
      expect(_snapshot(api, second), chosen);
      expect(_snapshot(api, saved), savedBefore);
      expect(first.id, isNull);
      expect(second.id, isNull);
      expect(api.summaries, hasLength(1));
      expect(api.accepted, isEmpty);
      expect(api.jobs, isEmpty);
      expect(api.calls, calls);
      expect(control.calls, isEmpty);
      await _unmount(tester, w);
      control.dispose();
    },
  );

  testWidgets(
    'keyboard search Enter/IME/repeat is not selection; Tab/Enter selects once',
    (tester) async {
      final api = CloudFake();
      final w = await readyCloud(api);
      final c = w.selected;
      c.edit('must not send');
      await _mount(tester, c);
      tester.widget<TextButton>(find.byKey(_trigger)).focusNode!.requestFocus();
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      await _search(tester, 'gpt-6.1-sol');
      final before = _snapshot(api, c);
      await tester.sendKeyDownEvent(LogicalKeyboardKey.enter);
      await tester.sendKeyRepeatEvent(LogicalKeyboardKey.enter);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(_snapshot(api, c), before);
      expect(
        tester.widget<TextField>(find.byKey(_filter)).focusNode!.hasFocus,
        true,
      );
      tester
          .widget<TextField>(find.byKey(_filter))
          .controller!
          .value = const TextEditingValue(
        text: 'gpt-6.1-sol',
        composing: TextRange(start: 0, end: 3),
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(_snapshot(api, c), before);
      await tester.sendKeyEvent(LogicalKeyboardKey.tab); // Clear filter.
      await tester.sendKeyEvent(LogicalKeyboardKey.tab); // Exact enabled row.
      await tester.pumpAndSettle();
      expect(
        Focus.of(
          tester.element(
            find
                .descendant(
                  of: find.byKey(const ValueKey(_sol)),
                  matching: find.byType(Text),
                )
                .first,
          ),
        ).hasFocus,
        true,
      );
      await tester.sendKeyDownEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      await tester.sendKeyRepeatEvent(LogicalKeyboardKey.enter);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(c.selectedRoute, _sol);
      expect(c.routeRevision, 1);
      expect(find.byType(Dialog), findsNothing);
      expect(api.accepted, isEmpty);
      expect(c.draft, 'must not send');
      expect(
        tester.widget<TextButton>(find.byKey(_trigger)).focusNode!.hasFocus,
        true,
      );
      await _unmount(tester, w);
    },
  );

  testWidgets(
    'picker Enter never reaches the real chat composer Send handler',
    (tester) async {
      final api = CloudFake();
      final w = await readyCloud(api);
      final c = w.selected;
      c.edit('private composer draft');
      await tester.pumpWidget(
        MaterialApp(
          theme: SmithTheme.dark(),
          home: Scaffold(body: ChatPanel(controller: c)),
        ),
      );
      await tester.tap(find.byKey(const ValueKey('chat-composer')));
      await _open(tester);
      await _search(tester, 'gpt-6.1-sol');
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(api.accepted, isEmpty);
      expect(api.calls, isNot(contains('route_prepare')));
      expect(c.draft, 'private composer draft');
      expect(
        tester.widget<TextButton>(find.byKey(_trigger)).focusNode!.hasFocus,
        true,
      );
      await _unmount(tester, w);
    },
  );

  testWidgets(
    'empty and missing catalogs are not confused with no filter matches',
    (tester) async {
      final api = CloudFake();
      final w = await readyCloud(api);
      final c = w.selected;
      w.routeCatalog = null;
      await _mount(tester, c);
      await _open(tester);
      expect(find.textContaining('Model catalog unavailable.'), findsOneWidget);
      expect(find.text('No matching models'), findsNothing);
      final value = catalog()..['models'] = [];
      w.routeCatalog = RouteCatalog(value);
      w.changed();
      await tester.pumpAndSettle();
      expect(
        find.text('No models available in the observed catalog.'),
        findsOneWidget,
      );
      expect(find.text('No matching models'), findsNothing);
      await _unmount(tester, w);
    },
  );

  testWidgets(
    '400x400 at 2x keeps search, full statuses, last choice and Cancel reachable',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(400, 400);
      tester.platformDispatcher.textScaleFactorTestValue = 2;
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
      final api = CatalogFake();
      setProviderFacts(
        api.catalogValue,
        'anthropic',
        state: 'missing',
        validationStatus: 'interrupted',
      );
      final w = await readyCloud(api);
      final c = w.selected;
      await _mount(tester, c);
      await _open(tester);
      final dialog = tester.getRect(find.byType(Dialog));
      expect(dialog.left, greaterThanOrEqualTo(0));
      expect(dialog.right, lessThanOrEqualTo(400));
      expect(dialog.top, greaterThanOrEqualTo(0));
      expect(dialog.bottom, lessThanOrEqualTo(400));
      expect(find.byKey(_filter).hitTestable(), findsOneWidget);
      expect(find.text('Cancel').hitTestable(), findsOneWidget);
      const last = ChatRoute('anthropic', 'claude-sonnet-5', generation);
      await _search(tester, last.model);
      // This query also matches Sonnet 5.5; scroll to the exact final row.
      await tester.scrollUntilVisible(
        find.byKey(const ValueKey(last)),
        100,
        scrollable: find.descendant(
          of: find.byKey(const ValueKey('chat-route-results')),
          matching: find.byType(Scrollable),
        ),
      );
      final label = find
          .descendant(
            of: find.byKey(const ValueKey(last)),
            matching: find.byType(Text),
          )
          .first;
      await tester.ensureVisible(label);
      await tester.pumpAndSettle();
      expect(label.hitTestable(), findsOneWidget);
      expect(find.text('Cancel').hitTestable(), findsOneWidget);
      final statuses = find.textContaining('Not individually verified');
      expect(statuses, findsWidgets);
      for (final element in statuses.evaluate()) {
        final text = element.widget as Text;
        expect(text.overflow, isNot(TextOverflow.ellipsis));
        expect(text.maxLines, isNull);
      }
      expect(
        find.textContaining('Last credential check: saved item missing.'),
        findsWidgets,
      );
      expect(
        find.textContaining('may have been processed or billed'),
        findsWidgets,
      );
      expect(tester.takeException(), isNull);
      await _choose(tester, last);
      expect(c.selectedRoute, last);
      expect(api.accepted, isEmpty);
      await _unmount(tester, w);
    },
  );
}
