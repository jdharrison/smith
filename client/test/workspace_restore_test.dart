import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/main.dart';
import 'package:smith_client/ui/smith_workspace.dart';
import 'package:smith_client/ui/split_workspace.dart';
import 'package:smith_client/workspace_controller.dart';
import 'package:smith_client/workspace_preferences.dart';

import 'app_fakes.dart';
import 'split_workspace_widget_test.dart' show layout, inside;
import 'widget_test.dart' show FakeControlApi, profile;

class MemoryStore implements WorkspacePreferencesStore {
  MemoryStore([this.value]);
  WorkspacePreferences? value;
  Completer<WorkspacePreferences?>? loadGate;
  Completer<void>? saveGate;
  bool failLoad = false;
  bool failSave = false;
  int loads = 0;
  int closes = 0;
  int concurrent = 0;
  int maximumConcurrent = 0;
  final writes = <WorkspacePreferences>[];

  @override
  Future<WorkspacePreferences?> load() async {
    loads++;
    if (failLoad) throw StateError('private storage detail');
    return loadGate == null ? value : await loadGate!.future;
  }

  @override
  Future<void> save(WorkspacePreferences value) async {
    concurrent++;
    if (concurrent > maximumConcurrent) maximumConcurrent = concurrent;
    writes.add(value);
    try {
      await saveGate?.future;
      if (failSave) throw StateError('private storage detail');
      this.value = WorkspacePreferences.fromJson(value.toJson());
    } finally {
      concurrent--;
    }
  }

  @override
  Future<void> close() async => closes++;
}

WorkspacePreferences preferences({
  String preset = '2x2',
  int active = 0,
  List<String?> ids = const [null, testConversation, testConversation, null],
}) => WorkspacePreferences(
  layout: preset,
  activeTile: active,
  slots: List.generate(
    4,
    (i) => WorkspaceSlotPreferences(
      conversationId: ids[i],
      page: i.isOdd ? 'settings' : 'chat',
    ),
  ),
);

Future<void> mountApp(
  WidgetTester tester,
  MemoryStore store,
  FakeAppApi api,
) async {
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

WorkspaceController owner(WidgetTester tester) =>
    tester.widget<ConversationScreen>(find.byType(ConversationScreen)).chat
        as WorkspaceController;
WorkspacePresentation presentation(WidgetTester tester) => tester
    .widget<ConversationScreen>(find.byType(ConversationScreen))
    .presentation!;

void noMutations(FakeAppApi api) {
  // Boot's existing explicit inventory observation is not restoration work.
  expect(api.calls.where((op) => op == 'models'), hasLength(1));
  expect(
    api.calls.where(
      (op) => !['state', 'conversation_get', 'poll', 'models'].contains(op),
    ),
    isEmpty,
  );
}

void main() {
  testWidgets(
    'explicit first send persists promoted ID; restart never resubmits',
    (tester) async {
      final store = MemoryStore(preferences(preset: '1x1'));
      final api = FakeAppApi();
      await mountApp(tester, store, api);
      final w = owner(tester);
      w.selected.edit('Explicit fake-only submission');
      await w.selected.send();
      await tester.pumpAndSettle();
      final id = w.selected.id;
      expect(id, isNotNull);
      expect(store.value!.slots[0].conversationId, id);
      expect(api.calls.where((op) => op == 'submit_new'), hasLength(1));
      final saved = store.value!;
      expect(jsonEncode(saved.toJson()), isNot(contains('Explicit fake-only')));
      await tester.pumpWidget(const SizedBox());
      await tester.pumpAndSettle();
      final secondApi = FakeAppApi();
      secondApi.summaries[id!] = Map.of(api.summaries[id]!);
      secondApi.histories[id] = List.of(api.histories[id]!);
      await mountApp(tester, MemoryStore(saved), secondApi);
      expect(owner(tester).selected.id, id);
      noMutations(secondApi);
      await tester.pumpWidget(const SizedBox());
      await tester.pumpAndSettle();
    },
  );

  for (final preset in ['1x1', '2x1', '1x2', '2x2']) {
    testWidgets(
      'full app restart retains $preset, active, pages and hidden slots',
      (tester) async {
        tester.view.physicalSize = const Size(1800, 1200);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        final store = MemoryStore(preferences());
        final api = FakeAppApi();
        await mountApp(tester, store, api);
        final w = owner(tester);
        expect(w.panes[0].selected.id, isNull);
        expect(w.panes[1].selected, same(w.panes[2].selected));
        w.panes[2].selected.edit('DO NOT PERSIST THIS DRAFT');
        final columns = int.parse(preset[0]);
        final rows = int.parse(preset[2]);
        await layout(tester, columns, rows);
        final last = columns * rows - 1;
        // Exercise real navigation and focus callbacks, not only the owner model.
        if (presentation(tester).pages[last] == 'settings') {
          await tester.tap(
            inside(last, find.byTooltip('Return to Chat')).first,
          );
          await tester.pumpAndSettle();
        }
        await tester.tap(inside(last, find.byType(TextField)));
        await tester.pumpAndSettle();
        await tester.tap(inside(last, find.byTooltip('Settings')).first);
        await tester.pumpAndSettle();
        expect(presentation(tester).active, last);
        expect(store.value!.layout, preset);
        expect(store.value!.activeTile, last);
        expect(store.value!.slots[last].page, 'settings');
        expect(
          jsonEncode(store.value!.toJson()),
          isNot(contains('DO NOT PERSIST')),
        );
        final expected = store.value!;
        await tester.pumpWidget(const SizedBox());
        await tester.pumpAndSettle();
        expect(store.closes, 1);
        final secondStore = MemoryStore(expected);
        final secondApi = FakeAppApi();
        await mountApp(tester, secondStore, secondApi);
        expect(presentation(tester).layout.label, '$columns×$rows');
        expect(presentation(tester).active, last);
        expect(owner(tester).panes, hasLength(4));
        expect(
          owner(tester).panes[1].selected,
          same(owner(tester).panes[2].selected),
        );
        expect(owner(tester).panes[2].selected.draft, isEmpty);
        for (var i = 0; i < 4; i++) {
          expect(presentation(tester).pages[i], expected.slots[i].page);
        }
        expect(secondStore.writes, isEmpty);
        await layout(tester, 2, 2);
        expect(owner(tester).panes[1].selected.id, testConversation);
        expect(
          tester
              .widget<SmithWorkspace>(inside(3, find.byType(SmithWorkspace)))
              .page,
          expected.slots[3].page,
        );
        noMutations(api);
        noMutations(secondApi);
        await tester.pumpWidget(const SizedBox());
        await tester.pumpAndSettle();
        expect(secondStore.closes, 1);
      },
    );
  }

  testWidgets(
    'delayed load gates interaction after Boot and never saves defaults',
    (tester) async {
      final store = MemoryStore()
        ..loadGate = Completer<WorkspacePreferences?>();
      final api = FakeAppApi();
      await mountApp(tester, store, api);
      expect(find.text('Restoring workspace…'), findsOneWidget);
      expect(find.byType(SplitWorkspace), findsNothing);
      expect(store.loads, 1);
      expect(store.writes, isEmpty);
      expect(api.calls, contains('state'));
      store.loadGate!.complete(preferences(preset: '1x1'));
      await tester.pumpAndSettle();
      expect(owner(tester).selected.id, isNull);
      expect(store.writes, isEmpty);
      noMutations(api);
      await tester.pumpWidget(const SizedBox());
      await tester.pumpAndSettle();
    },
  );

  testWidgets('missing preferences retains first use selection', (
    tester,
  ) async {
    final store = MemoryStore();
    await mountApp(tester, store, FakeAppApi());
    expect(owner(tester).selected.id, testConversation);
    expect(presentation(tester).layout, WorkspaceLayout.single);
    expect(store.writes, isEmpty);
    await tester.pumpWidget(const SizedBox());
    await tester.pumpAndSettle();
  });

  for (final archived in [false, true]) {
    testWidgets(
      '${archived ? 'archived' : 'missing'} chats repair hidden and visible slots without mutation',
      (tester) async {
        final api = FakeAppApi();
        if (archived) {
          api.summaries[testConversation]!['archived'] = true;
        } else {
          api.summaries.clear();
        }
        final store = MemoryStore(
          preferences(preset: '1x1', ids: List.filled(4, testConversation)),
        );
        await mountApp(tester, store, api);
        expect(
          owner(tester).panes.every((pane) => pane.selected.id == null),
          isTrue,
        );
        expect(find.textContaining('missing or archived'), findsOneWidget);
        expect(presentation(tester).pages[3], 'settings');
        noMutations(api);
        await tester.pumpWidget(const SizedBox());
        await tester.pumpAndSettle();
        expect(store.closes, 1);
        expect(store.writes, hasLength(1));
        expect(
          store.value!.slots.every((slot) => slot.conversationId == null),
          isTrue,
        );
        for (var i = 0; i < 4; i++) {
          expect(store.value!.slots[i].page, i.isOdd ? 'settings' : 'chat');
        }

        // The chat is available again (reactivated for the archived case).
        // Restart from what was actually saved, with no intervening UI edits.
        final restartedStore = MemoryStore(store.value);
        final restartedApi = FakeAppApi();
        await mountApp(tester, restartedStore, restartedApi);
        expect(
          owner(tester).panes.every((pane) => pane.selected.id == null),
          isTrue,
        );
        expect(find.textContaining('missing or archived'), findsNothing);
        expect(presentation(tester).pages[3], 'settings');
        expect(restartedStore.writes, isEmpty);
        noMutations(restartedApi);
        await tester.pumpWidget(const SizedBox());
        await tester.pumpAndSettle();
        expect(restartedStore.writes, isEmpty);
        expect(restartedStore.closes, 1);
      },
    );
  }

  testWidgets('detail outage after Boot preserves restored IDs', (
    tester,
  ) async {
    final store = MemoryStore()..loadGate = Completer<WorkspacePreferences?>();
    final api = FakeAppApi();
    await mountApp(tester, store, api);
    api.detailFailure = true;
    store.loadGate!.complete(preferences());
    await tester.pumpAndSettle();
    expect(owner(tester).panes[1].selected.id, testConversation);
    expect(owner(tester).panes[2].selected.id, testConversation);
    expect(owner(tester).stale, isTrue);
    expect(find.textContaining('missing or archived'), findsNothing);
    noMutations(api);
    await tester.pumpWidget(const SizedBox());
    await tester.pumpAndSettle();
  });

  testWidgets('load error keeps usable session-only UI and never overwrites', (
    tester,
  ) async {
    final store = MemoryStore()..failLoad = true;
    await mountApp(tester, store, FakeAppApi());
    expect(find.textContaining('session-only'), findsOneWidget);
    expect(find.textContaining('private storage'), findsNothing);
    await layout(tester, 2, 2);
    expect(store.writes, isEmpty);
    await tester.pumpWidget(const SizedBox());
    await tester.pumpAndSettle();
    expect(store.closes, 1);
  });

  testWidgets('save error warns without disabling navigation', (tester) async {
    final store = MemoryStore(preferences(preset: '1x1'))..failSave = true;
    await mountApp(tester, store, FakeAppApi());
    await layout(tester, 2, 1);
    expect(find.textContaining('may not survive restart'), findsOneWidget);
    expect(find.textContaining('private storage'), findsNothing);
    await layout(tester, 1, 1);
    expect(presentation(tester).layout, WorkspaceLayout.single);
    await tester.pumpWidget(const SizedBox());
    await tester.pumpAndSettle();
  });

  testWidgets(
    'latest-only serial saves drain before single close; drafts do not save',
    (tester) async {
      final store = MemoryStore(preferences(preset: '1x1'))
        ..saveGate = Completer<void>();
      final api = FakeAppApi();
      await mountApp(tester, store, api);
      final p = presentation(tester);
      owner(tester).selected.edit('unsent secret');
      await tester.pump();
      expect(store.writes, isEmpty);
      p.setPage(0, 'settings');
      await tester.pump();
      expect(store.writes, hasLength(1));
      p.setPage(0, 'chat');
      p.setPage(1, 'chat');
      p.setPage(2, 'settings');
      await tester.pump();
      expect(store.writes, hasLength(1));
      await tester.pumpWidget(const SizedBox());
      expect(store.closes, 0);
      store.saveGate!.complete();
      await tester.pumpAndSettle();
      expect(store.writes, hasLength(2));
      expect(store.maximumConcurrent, 1);
      expect(store.value!.slots[0].page, 'chat');
      expect(store.value!.slots[2].page, 'settings');
      expect(store.closes, 1);
      noMutations(api);
    },
  );

  testWidgets('disposal during load ignores late hydration and closes once', (
    tester,
  ) async {
    final store = MemoryStore()..loadGate = Completer<WorkspacePreferences?>();
    await mountApp(tester, store, FakeAppApi());
    await tester.pumpWidget(const SizedBox());
    store.loadGate!.complete(preferences());
    await tester.pumpAndSettle();
    expect(store.writes, isEmpty);
    expect(store.closes, 1);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'post-hydration selection and authoritative removal persist without drafts',
    (tester) async {
      final store = MemoryStore(preferences(preset: '1x1'));
      final api = FakeAppApi();
      await mountApp(tester, store, api);
      final w = owner(tester);
      await w.selectFor(w.panes[0], testConversation);
      await tester.pumpAndSettle();
      expect(store.value!.slots[0].conversationId, testConversation);
      w.selected.edit('not stored');
      final count = store.writes.length;
      await tester.pump();
      expect(store.writes, hasLength(count));
      api.summaries.clear();
      await w.synchronize();
      await tester.pumpAndSettle();
      expect(
        store.value!.slots.every((slot) => slot.conversationId == null),
        isTrue,
      );
      noMutations(api);
      await tester.pumpWidget(const SizedBox());
      await tester.pumpAndSettle();
    },
  );
}
