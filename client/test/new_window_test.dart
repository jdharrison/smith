import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/main.dart';
import 'package:smith_client/ui/smith_app_header.dart';
import 'package:smith_client/ui/split_workspace.dart';
import 'package:smith_client/window_launcher.dart';
import 'package:smith_client/workspace_controller.dart';
import 'package:smith_client/workspace_preferences.dart';

import 'app_fakes.dart';
import 'message_queue_controller_test.dart' as queue;
import 'split_workspace_widget_test.dart' show inside, layout;
import 'widget_test.dart' show FakeControlApi, profile;
import 'workspace_restore_test.dart' show MemoryStore;

const windowTooltip = 'Open a new window — layout isn’t saved.';
const launchError = 'Couldn’t launch a new window. You can try again.';
Finder get newWindow => find.byKey(const ValueKey('new-window'));
Finder get layoutButton => find.byKey(const ValueKey('workspace-layout'));

ConversationScreen screen(WidgetTester tester) =>
    tester.widget<ConversationScreen>(find.byType(ConversationScreen));
WorkspaceController owner(WidgetTester tester) =>
    screen(tester).chat as WorkspaceController;

Future<void> mount(
  WidgetTester tester, {
  required Future<void> Function() launch,
  List<String> arguments = const [sessionOnlyWindowArgument],
  WorkspacePreferencesStore Function()? storeFactory,
  FakeAppApi? api,
  Size size = const Size(1600, 1000),
  double scale = 1,
}) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  tester.platformDispatcher.textScaleFactorTestValue = scale;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
  final preferences = workspacePreferencesForStartup(
    arguments,
    storeFactory ??
        () => throw StateError('Session-only must not open storage'),
  );
  await tester.pumpWidget(
    SmithApp(
      key: UniqueKey(),
      openWindow: launch,
      preferencesStore: preferences,
      controlApi: FakeControlApi(profile()),
      appApi: api ?? (FakeAppApi()..inventoryObserved = true),
    ),
  );
  await tester.pumpAndSettle();
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('exact session-only flag never constructs the preference store', () {
    for (final arguments in [
      [sessionOnlyWindowArgument],
      ['unrelated', sessionOnlyWindowArgument],
      [sessionOnlyWindowArgument, sessionOnlyWindowArgument],
    ]) {
      expect(
        workspacePreferencesForStartup(
          arguments,
          () => throw StateError('Must not construct or lock storage'),
        ),
        isNull,
      );
    }
  });

  test(
    'normal and near-match startup retain ordinary preference injection',
    () {
      for (final arguments in [
        <String>[],
        ['--session-only-window=false'],
        ['--session-only-window-extra'],
        ['unrelated'],
      ]) {
        final store = MemoryStore();
        var constructions = 0;
        expect(
          workspacePreferencesForStartup(arguments, () {
            constructions++;
            return store;
          }),
          same(store),
        );
        expect(constructions, 1);
      }
    },
  );

  test(
    'launcher uses only resolved executable, fixed flag, detached/no shell',
    () async {
      var calls = 0;
      await launchSessionOnlyWindow(
        startProcess:
            (
              executable,
              arguments, {
              required mode,
              required runInShell,
            }) async {
              calls++;
              expect(executable, Platform.resolvedExecutable);
              expect(arguments, [sessionOnlyWindowArgument]);
              expect(mode, ProcessStartMode.detached);
              expect(runInShell, false);
            },
      );
      expect(calls, 1);
    },
  );

  test(
    'executable is one literal path and launch failure is not retried',
    () async {
      var calls = 0;
      await expectLater(
        launchSessionOnlyWindow(
          executable: '/fixture/Smith bundle/smith_client',
          startProcess:
              (
                executable,
                arguments, {
                required mode,
                required runInShell,
              }) async {
                calls++;
                expect(executable, '/fixture/Smith bundle/smith_client');
                expect(arguments, [sessionOnlyWindowArgument]);
                expect(mode, ProcessStartMode.detached);
                expect(runInShell, false);
                throw const ProcessException(
                  'fixture',
                  [],
                  'private launch detail',
                );
              },
        ),
        throwsA(isA<ProcessException>()),
      );
      expect(calls, 1);
    },
  );

  test(
    'launch does not copy drafts/queues or submit observed service work',
    () async {
      final api = queue.QueueApi();
      final parent = await queue.ready(api);
      await queue.start(parent.selected);
      queue.enqueue(parent.selected, 'parent-only follow-up');
      parent.selected.edit('parent-only draft');
      final entries = parent.selected.messageQueue.entries;
      final request = parent.selected.pendingId;
      await launchSessionOnlyWindow(
        startProcess: (_, _, {required mode, required runInShell}) async {},
      );
      final child = WorkspaceController(api);
      addTearDown(child.dispose);
      await child.synchronize();
      expect(child.selected.id, parent.selected.id);
      expect(child.selected.pendingId, request);
      expect(child.selected.draft, isEmpty);
      expect(child.selected.messageQueue.entries, isEmpty);
      expect(parent.selected.draft, 'parent-only draft');
      expect(
        parent.selected.messageQueue.entries.map((entry) => entry.text),
        entries.map((entry) => entry.text),
      );
      expect(api.sends, hasLength(1));
    },
  );

  testWidgets('one adjacent header action in every layout and Settings', (
    tester,
  ) async {
    var launches = 0;
    await mount(
      tester,
      launch: () async {
        launches++;
      },
    );
    expect(screen(tester).openWindow, isNotNull);
    for (final preset in WorkspaceLayout.values) {
      await layout(tester, preset.columns, preset.rows);
      expect(newWindow, findsOneWidget);
      expect(find.byTooltip(windowTooltip), findsOneWidget);
      expect(
        find.descendant(of: find.byType(SmithAppHeader), matching: newWindow),
        findsOneWidget,
      );
      expect(
        tester.getRect(newWindow).left,
        tester.getRect(layoutButton).right + 8,
      );
      await tester.tap(inside(0, find.byTooltip('Settings')));
      await tester.pumpAndSettle();
      await tester.tap(newWindow);
      await tester.pumpAndSettle();
      await tester.tap(inside(0, find.byTooltip('Return to Chat')));
      await tester.pumpAndSettle();
    }
    expect(launches, 4);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
    'pending launch is single-flight and leaves parent state untouched',
    (tester) async {
      final pending = Completer<void>();
      var launches = 0;
      final store = MemoryStore();
      final api = FakeAppApi()..inventoryObserved = true;
      await mount(
        tester,
        arguments: const [],
        storeFactory: () => store,
        api: api,
        launch: () {
          launches++;
          return pending.future;
        },
      );
      await layout(tester, 2, 1);
      owner(tester).panes.first.selected.edit('keep this draft');
      await tester.pumpAndSettle();
      final parent = owner(tester);
      final selected = parent.panes.first.selected;
      final writes = store.writes.length;
      final apiCalls = List.of(api.calls);
      await tester.tap(newWindow);
      await tester.pump();
      expect(tester.widget<IconButton>(newWindow).onPressed, isNull);
      expect(
        tester
            .widget<PopupMenuButton<WorkspaceLayout>>(layoutButton)
            .onSelected,
        isNotNull,
      );
      await tester.tap(newWindow);
      await tester.pump();
      expect(launches, 1);
      expect(selected.draft, 'keep this draft');
      expect(parent.panes.first.selected, same(selected));
      expect(screen(tester).presentation!.layout, WorkspaceLayout.sideBySide);
      expect(store.writes.length, writes);
      expect(api.calls, apiCalls);
      pending.complete();
      await tester.pumpAndSettle();
      expect(tester.widget<IconButton>(newWindow).onPressed, isNotNull);
      expect(find.textContaining('Window ready'), findsNothing);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets('failure is sanitized, announced, and allows deliberate retry', (
    tester,
  ) async {
    var launches = 0;
    await mount(
      tester,
      launch: () async {
        launches++;
        if (launches == 1) throw StateError('sensitive exception details');
      },
    );
    owner(tester).selected.edit('retained after failure');
    await tester.tap(newWindow);
    await tester.pumpAndSettle();
    expect(find.text(launchError), findsOneWidget);
    expect(find.textContaining('sensitive'), findsNothing);
    expect(owner(tester).selected.draft, 'retained after failure');
    expect(tester.widget<IconButton>(newWindow).onPressed, isNotNull);
    expect(launches, 1);
    await tester.tap(newWindow);
    await tester.pumpAndSettle();
    expect(launches, 2);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  for (final fail in [false, true]) {
    testWidgets('late ${fail ? 'failure' : 'success'} after disposal is safe', (
      tester,
    ) async {
      final pending = Completer<void>();
      await mount(tester, launch: () => pending.future);
      await tester.tap(newWindow);
      await tester.pump();
      await tester.pumpWidget(const SizedBox());
      if (fail) {
        pending.completeError(StateError('private failure'));
      } else {
        pending.complete();
      }
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(find.text(launchError), findsNothing);
    });
  }

  testWidgets(
    'fresh child and descendants remain session-only after primary disposal',
    (tester) async {
      final store = MemoryStore();
      var launches = 0;
      Future<void> launch() => launchSessionOnlyWindow(
        startProcess:
            (
              executable,
              arguments, {
              required mode,
              required runInShell,
            }) async {
              launches++;
              expect(arguments, [sessionOnlyWindowArgument]);
              expect(mode, ProcessStartMode.detached);
              expect(runInShell, false);
            },
      );
      await mount(
        tester,
        launch: launch,
        arguments: const [],
        storeFactory: () => store,
      );
      await layout(tester, 2, 2);
      owner(tester).selected.edit('parent-only draft');
      await tester.tap(inside(0, find.byTooltip('Settings')));
      await tester.pumpAndSettle();
      await tester.tap(newWindow);
      await tester.pumpAndSettle();
      await tester.pumpWidget(const SizedBox());
      await tester.pumpAndSettle();
      final writes = store.writes.length;
      expect(store.closes, 1);

      for (var generation = 0; generation < 2; generation++) {
        var constructions = 0;
        final api = FakeAppApi()..inventoryObserved = true;
        await mount(
          tester,
          launch: launch,
          api: api,
          storeFactory: () {
            constructions++;
            return store;
          },
        );
        expect(constructions, 0);
        expect(screen(tester).presentation!.store, isNull);
        expect(screen(tester).presentation!.layout, WorkspaceLayout.single);
        expect(screen(tester).presentation!.pages, everyElement('chat'));
        expect(screen(tester).presentation!.warning, isNull);
        expect(owner(tester).selected.id, testConversation);
        expect(owner(tester).selected.draft, isEmpty);
        expect(owner(tester).selected.messageQueue.entries, isEmpty);
        expect(
          api.calls.where(
            (op) =>
                !['state', 'conversation_get', 'models', 'poll'].contains(op),
          ),
          isEmpty,
        );
        await layout(tester, 1, 2);
        await tester.tap(inside(0, find.byTooltip('Settings')));
        await tester.pumpAndSettle();
        await tester.tap(newWindow);
        await tester.pumpAndSettle();
        await tester.pumpWidget(const SizedBox());
        await tester.pumpAndSettle();
        expect(constructions, 0);
        expect(store.writes.length, writes);
        expect(store.closes, 1);
      }
      expect(launches, 3);
    },
  );

  testWidgets('empty service starts blank without saved preferences', (
    tester,
  ) async {
    final api = FakeAppApi()..inventoryObserved = true;
    api.summaries.clear();
    await mount(tester, launch: () async {}, api: api);
    expect(owner(tester).selected.id, isNull);
    expect(owner(tester).selected.draft, isEmpty);
    expect(screen(tester).presentation!.warning, isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('keyboard reaches Layout then New window and activates once', (
    tester,
  ) async {
    var launches = 0;
    await mount(
      tester,
      launch: () async {
        launches++;
      },
    );
    final semantics = tester.ensureSemantics();
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pumpAndSettle();
    expect(
      FocusManager.instance.primaryFocus!.context!
          .findAncestorWidgetOfExactType<WorkspaceLayoutMenu>(),
      isNotNull,
    );
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pumpAndSettle();
    expect(
      tester.getSemantics(newWindow).getSemanticsData().tooltip,
      windowTooltip,
    );
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(launches, 1);
    await tester.sendKeyEvent(LogicalKeyboardKey.space);
    await tester.pumpAndSettle();
    expect(launches, 2);
    semantics.dispose();
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('400px at 2x text keeps both actions and failure reachable', (
    tester,
  ) async {
    await mount(
      tester,
      size: const Size(400, 400),
      scale: 2,
      launch: () async {
        throw StateError('private error');
      },
    );
    expect(tester.getRect(newWindow).right, lessThanOrEqualTo(400));
    expect(tester.getRect(layoutButton).left, greaterThanOrEqualTo(0));
    await tester.tap(newWindow);
    await tester.pumpAndSettle();
    expect(find.text(launchError), findsOneWidget);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('embedded constructors have no implicit host launcher', (
    tester,
  ) async {
    await tester.pumpWidget(
      SmithApp(
        controlApi: FakeControlApi(profile()),
        appApi: FakeAppApi()..inventoryObserved = true,
      ),
    );
    await tester.pumpAndSettle();
    expect(newWindow, findsNothing);
    await tester.pumpWidget(const SizedBox());
  });
}
