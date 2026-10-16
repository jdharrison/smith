import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/chat_panel.dart';

import 'cloud_routing_test.dart';

void main() {
  testWidgets('narrow composer Enter asks transfer; Cancel preserves draft', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(400, 400);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final api = CloudFake();
    final w = await readyCloud(api);
    final c = w.selected;
    c.selectRoute(cloud);
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(body: ChatPanel(controller: c)),
      ),
    );
    await tester.enterText(
      find.byKey(const ValueKey('chat-composer')),
      'keep private',
    );
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(find.text('Transfer conversation context'), findsOneWidget);
    expect(find.textContaining('charges may occur'), findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(c.draft, 'keep private');
    expect(api.accepted, isEmpty);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
    w.dispose();
  });
}
