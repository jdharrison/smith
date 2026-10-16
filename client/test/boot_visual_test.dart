import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/control_api.dart';
import 'package:smith_client/main.dart';

import 'app_fakes.dart';
import 'boot_test.dart' show bootReply, flushWidget;
import 'widget_test.dart' show FakeControlApi, profile;

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    final font = FontLoader('JetBrains Mono')
      ..addFont(
        rootBundle.load(
          'assets/fonts/jetbrains-mono/JetBrainsMono-Regular.ttf',
        ),
      );
    await font.load();
  });

  testWidgets('loader intro, hold and outro use the production theme', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1280, 720);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    final semantics = tester.ensureSemantics();
    final control = FakeControlApi(profile())
      ..pendingInspect = Completer<ControlReply>();
    final app = FakeAppApi();
    const boundary = ValueKey('boot-capture');
    await tester.pumpWidget(
      RepaintBoundary(
        key: boundary,
        child: SmithApp(controlApi: control, appApi: app),
      ),
    );
    await tester.pump();
    expect(
      tester
          .getSemantics(find.byKey(const ValueKey('boot-status-semantics')))
          .label,
      'Starting Smith',
    );
    await tester.pump(const Duration(milliseconds: 250));
    await expectLater(
      find.byKey(boundary),
      matchesGoldenFile('goldens/boot/intro.png'),
    );
    await tester.pump(const Duration(milliseconds: 250));
    await expectLater(
      find.byKey(boundary),
      matchesGoldenFile('goldens/boot/hold.png'),
    );
    expect(find.byType(TextField), findsNothing);
    expect(app.calls, isEmpty);
    await tester.pump(const Duration(milliseconds: 1000, microseconds: 1));
    control.pendingInspect!.complete(bootReply(profile()));
    await flushWidget(tester);
    await tester.pump(const Duration(milliseconds: 250));
    await expectLater(
      find.byKey(boundary),
      matchesGoldenFile('goldens/boot/outro.png'),
    );
    expect(find.byType(ConversationScreen), findsNothing);
    await tester.pumpAndSettle();
    expect(find.byType(TextField), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
    semantics.dispose();
  });

  testWidgets('loader repair stays readable at 400x400 and 2x text', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(400, 400);
    tester.platformDispatcher.textScaleFactorTestValue = 2;
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
    const boundary = ValueKey('boot-repair-capture');
    final app = FakeAppApi();
    await tester.pumpWidget(
      RepaintBoundary(
        key: boundary,
        child: SmithApp(
          controlApi: FakeControlApi(profile(enablement: 'disabled')),
          appApi: app,
        ),
      ),
    );
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Observe again'));
    await tester.pumpAndSettle();
    expect(find.text('Observe again').hitTestable(), findsOneWidget);
    expect(find.byType(TextField), findsNothing);
    expect(app.calls, isEmpty);
    expect(tester.takeException(), isNull);
    await expectLater(
      find.byKey(boundary),
      matchesGoldenFile('goldens/boot/repair-400x400-text2.png'),
    );
    await tester.pumpWidget(const SizedBox());
  });
}
