import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/main.dart';
import 'package:smith_client/ui/workspace_entrance.dart';
import 'package:smith_client/workspace_preferences.dart';

import 'app_fakes.dart';
import 'widget_test.dart' show FakeControlApi, profile;
import 'workspace_restore_test.dart' show MemoryStore, preferences;

const _boundary = ValueKey('workspace-entrance-capture');

Future<void> _mountRestored(
  WidgetTester tester, {
  required Size size,
  String layout = '1x1',
  double textScale = 1,
}) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = size;
  tester.platformDispatcher.textScaleFactorTestValue = textScale;
  addTearDown(tester.view.resetDevicePixelRatio);
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
  final store = MemoryStore()..loadGate = Completer<WorkspacePreferences?>();
  final api = FakeAppApi()
    ..inventoryObserved = true
    ..history.addAll([
      {
        'role': 'user',
        'content': 'Let’s make the workspace feel more intentional.',
      },
      {
        'role': 'assistant',
        'content': 'A calm entrance keeps the focus on your conversation.',
      },
    ]);
  await tester.pumpWidget(
    RepaintBoundary(
      key: _boundary,
      child: SmithApp(
        controlApi: FakeControlApi(profile()),
        appApi: api,
        preferencesStore: store,
      ),
    ),
  );
  await tester.pumpAndSettle();
  expect(find.text('Restoring workspace…'), findsOneWidget);
  expect(find.byType(WorkspaceEntrance), findsNothing);
  store.loadGate!.complete(
    preferences(
      preset: layout,
      ids: const [testConversation, testConversation, testConversation, null],
    ),
  );
  for (var i = 0; i < 20; i++) {
    await tester.pump();
  }
  expect(find.byType(WorkspaceEntrance), findsOneWidget);
  expect(
    tester
        .widget<Opacity>(find.byKey(const ValueKey('workspace-reveal-header')))
        .opacity,
    0,
  );
}

Future<void> _capture(WidgetTester tester, String name) async {
  expect(tester.takeException(), isNull);
  await expectLater(
    find.byKey(_boundary),
    matchesGoldenFile('goldens/workspace-entrance/$name.png'),
  );
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    final icons = FontLoader('MaterialIcons')
      ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'));
    await icons.load();
    final mono = FontLoader('JetBrains Mono')
      ..addFont(
        rootBundle.load(
          'assets/fonts/jetbrains-mono/JetBrainsMono-Regular.ttf',
        ),
      );
    await mono.load();
  });

  testWidgets(
    'restored desktop reveals separate groups without layout changes',
    (tester) async {
      await _mountRestored(tester, size: const Size(1600, 1000));
      final composer = tester.getSize(
        find.byKey(const ValueKey('chat-composer')).first,
      );
      await _capture(tester, 'desktop-000ms');
      await tester.pump(const Duration(milliseconds: 160));
      await _capture(tester, 'desktop-160ms');
      await tester.pump(const Duration(milliseconds: 200));
      await _capture(tester, 'desktop-360ms');
      await tester.pump(const Duration(milliseconds: 260));
      await _capture(tester, 'desktop-620ms');
      expect(
        tester.getSize(find.byKey(const ValueKey('chat-composer')).first),
        composer,
      );
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets('restored mixed Chat and Settings tiles share the reveal', (
    tester,
  ) async {
    await _mountRestored(tester, size: const Size(2200, 1400), layout: '2x2');
    await tester.pump(const Duration(milliseconds: 360));
    await _capture(tester, 'mixed-grid-360ms');
    await tester.pump(const Duration(milliseconds: 260));
    await _capture(tester, 'mixed-grid-620ms');
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('compact enlarged-text entrance preserves usable geometry', (
    tester,
  ) async {
    await _mountRestored(tester, size: const Size(400, 400), textScale: 2);
    await tester.pump(const Duration(milliseconds: 360));
    await _capture(tester, 'compact-text2-360ms');
    await tester.pump(const Duration(milliseconds: 260));
    await _capture(tester, 'compact-text2-620ms');
    expect(
      tester
          .widget<IgnorePointer>(
            find.byKey(const ValueKey('workspace-entrance-interaction')),
          )
          .ignoring,
      isFalse,
    );
    await tester.pumpWidget(const SizedBox());
  });
}
