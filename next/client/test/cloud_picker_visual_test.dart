import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/cloud_routing.dart';
import 'package:smith_client/ui/cloud_selector.dart';
import 'package:smith_client/ui/smith_theme.dart';

import 'cloud_routing_test.dart';

const _capture = ValueKey('cloud-picker-capture');

Future<void> _save(WidgetTester tester, String name) async {
  expect(tester.takeException(), isNull);
  final boundary = tester.renderObject<RenderRepaintBoundary>(
    find.byKey(_capture),
  );
  await tester.runAsync(() async {
    final image = await boundary.toImage(pixelRatio: 1);
    try {
      final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
      final file = File('build/saved-key-attempts/$name.png');
      await file.parent.create(recursive: true);
      await file.writeAsBytes(bytes!.buffer.asUint8List());
    } finally {
      image.dispose();
    }
  });
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    await (FontLoader('JetBrains Mono')..addFont(
          rootBundle.load(
            'assets/fonts/jetbrains-mono/JetBrainsMono-Regular.ttf',
          ),
        ))
        .load();
    await (FontLoader(
      'MaterialIcons',
    )..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'))).load();
  });

  for (final scale in [1.0, 2.0]) {
    testWidgets('offscreen real-font 400x400 picker at ${scale}x text', (
      tester,
    ) async {
      tester.view.physicalSize = const Size(400, 400);
      tester.view.devicePixelRatio = 1;
      tester.platformDispatcher.textScaleFactorTestValue = scale;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
      final api = CloudFake();
      final w = await readyCloud(api);
      final c = w.selected;
      c.edit('Retained unsent draft');
      final calls = List.of(api.calls);
      final revision = c.routeRevision;
      final value = catalog();
      final model = (value['models'] as List).firstWhere(
        (m) => m['route']['model'] == 'gpt-6.1-sol',
      );
      model['label'] = 'GPT-6.1 Sol';
      w.routeCatalog = RouteCatalog(value);
      await tester.pumpWidget(
        RepaintBoundary(
          key: _capture,
          child: MaterialApp(
            debugShowCheckedModeBanner: false,
            theme: SmithTheme.dark(),
            home: Scaffold(body: CloudSelector(controller: c, active: true)),
          ),
        ),
      );
      await tester.tap(find.byKey(const ValueKey('chat-route-selector')));
      await tester.pumpAndSettle();
      await tester.enterText(
        find.byKey(const ValueKey('chat-route-filter')),
        'gpt-6.1-sol',
      );
      await tester.pumpAndSettle();
      final unverified = find.text('Not individually verified');
      expect(unverified, findsOneWidget);
      expect(find.text('Cancel').hitTestable(), findsOneWidget);
      await _save(tester, 'picker-400x400-${scale}x-initial');
      await tester.ensureVisible(unverified);
      await tester.pumpAndSettle();
      expect(unverified.hitTestable(), findsOneWidget);
      await _save(tester, 'picker-400x400-${scale}x');
      setProviderFacts(
        value,
        'openai',
        state: 'locked',
        validationStatus: 'failed',
      );
      w.routeCatalog = RouteCatalog(value);
      w.changed();
      await tester.pumpAndSettle();
      final advisory = find.textContaining(
        'Last credential check: keyring locked.',
      );
      await tester.ensureVisible(advisory);
      await tester.pumpAndSettle();
      expect(advisory.hitTestable(), findsOneWidget);
      final row = find.byKey(
        const ValueKey(ChatRoute('openai', 'gpt-6.1-sol', generation)),
      );
      expect(tester.widget<TextButton>(row).onPressed, isNotNull);
      expect(find.text('Cancel').hitTestable(), findsOneWidget);
      await _save(tester, 'picker-400x400-${scale}x-advisory');
      setProviderFacts(value, 'openai', state: 'not_configured');
      w.routeCatalog = RouteCatalog(value);
      w.changed();
      await tester.pumpAndSettle();
      final reason = find.text('Save a key in the installer.');
      await tester.ensureVisible(reason);
      await tester.pumpAndSettle();
      expect(reason.hitTestable(), findsOneWidget);
      expect(find.text('Cancel').hitTestable(), findsOneWidget);
      await _save(tester, 'picker-400x400-${scale}x-disabled');
      await tester.tap(find.text('Cancel'));
      await tester.pumpAndSettle();
      expect(c.routeRevision, revision);
      expect(api.calls, calls);
      expect(c.draft, 'Retained unsent draft');
      await tester.pumpWidget(const SizedBox());
      w.dispose();
    });
  }
}
