import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/chat_panel.dart';
import 'package:smith_client/text_chat.dart';

import 'app_fakes.dart';

class AnchorController extends ChatController {
  AnchorController() : super(LegacyFakeAppApi()) {
    stale = false;
  }

  void messages(int count, {int lines = 1}) {
    history = List.generate(
      count,
      (i) => {
        'role': i.isEven ? 'user' : 'assistant',
        'content': List.filled(lines, 'Message $i').join('\n'),
      },
    );
    notifyListeners();
  }
}

Widget harness(ChatController c, bool independent, {double scale = 1}) =>
    MaterialApp(
      home: MediaQuery(
        data: MediaQueryData(textScaler: TextScaler.linear(scale)),
        child: Scaffold(
          body: ChatPanel(controller: c, independentView: independent),
        ),
      ),
    );

ScrollController transcript(WidgetTester tester) => tester
    .widget<SingleChildScrollView>(
      find.byKey(const ValueKey('conversation-history')),
    )
    .controller!;

void main() {
  for (final independent in [false, true]) {
    testWidgets('bottom/locked Y follows all content changes ($independent)', (
      tester,
    ) async {
      final c = AnchorController();
      await tester.pumpWidget(harness(c, independent));
      await tester.pumpAndSettle();
      final scroll = transcript(tester);
      expect(scroll.position.maxScrollExtent, 0);
      for (final count in [20, 30, 16]) {
        c.messages(count);
        await tester.pumpAndSettle();
        expect(scroll.offset, scroll.position.maxScrollExtent);
      }
      scroll.jumpTo(120);
      c.messages(32, lines: 2);
      await tester.pumpAndSettle();
      expect(scroll.offset, 120);
      // Same message count, different rendering height.
      c.messages(32, lines: 4);
      await tester.pumpAndSettle();
      expect(scroll.offset, 120);
      c.messages(0);
      await tester.pumpAndSettle();
      expect(scroll.offset, 0);
      c.messages(32);
      await tester.pumpAndSettle();
      expect(scroll.offset, 120);
      scroll.jumpTo(scroll.position.maxScrollExtent);
      c.messages(40, lines: 3);
      await tester.pumpAndSettle();
      expect(scroll.offset, scroll.position.maxScrollExtent);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      c.dispose();
    });

    testWidgets(
      'render reflow and chat navigation retain anchors ($independent)',
      (tester) async {
        final a = AnchorController()..messages(20, lines: 2);
        final b = AnchorController()..messages(30);
        await tester.pumpWidget(harness(a, independent));
        await tester.pumpAndSettle();
        final scroll = transcript(tester);
        await tester.pumpWidget(harness(a, independent, scale: 2));
        await tester.pumpAndSettle();
        expect(scroll.offset, scroll.position.maxScrollExtent);
        scroll.jumpTo(150);
        await tester.pumpWidget(harness(a, independent));
        await tester.pumpAndSettle();
        expect(scroll.offset, 150);
        await tester.pumpWidget(harness(b, independent));
        await tester.pumpAndSettle();
        expect(scroll.offset, scroll.position.maxScrollExtent);
        await tester.pumpWidget(harness(a, independent));
        await tester.pumpAndSettle();
        expect(scroll.offset, 150);
        await tester.pumpWidget(harness(b, independent));
        await tester.pumpAndSettle();
        b.messages(40, lines: 2);
        await tester.pumpAndSettle();
        expect(scroll.offset, scroll.position.maxScrollExtent);
        // A pending restore for a previous selection cannot apply its bookmark.
        await tester.pumpWidget(harness(a, independent));
        await tester.pumpWidget(harness(b, independent));
        await tester.pumpAndSettle();
        expect(scroll.offset, scroll.position.maxScrollExtent);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox());
        a.dispose();
        b.dispose();
      },
    );

    testWidgets('only reaching bottom resumes following ($independent)', (
      tester,
    ) async {
      final c = AnchorController()..messages(30);
      await tester.pumpWidget(harness(c, independent));
      await tester.pumpAndSettle();
      final scroll = transcript(tester);
      final locked = scroll.position.maxScrollExtent - 2;
      scroll.jumpTo(locked);
      c.messages(40);
      await tester.pumpAndSettle();
      expect(scroll.offset, locked);
      scroll.jumpTo(scroll.position.maxScrollExtent - .5);
      c.messages(45);
      await tester.pumpAndSettle();
      expect(scroll.offset, scroll.position.maxScrollExtent);
      // Bottom intent survives a non-scrollable viewport and later growth.
      c.messages(0);
      await tester.pumpAndSettle();
      c.messages(30);
      await tester.pumpAndSettle();
      expect(scroll.offset, scroll.position.maxScrollExtent);
      await tester.pumpWidget(const SizedBox());
      c.dispose();
    });
  }

  testWidgets('ordinary remount resolves bottom to a finite offset', (
    tester,
  ) async {
    final c = AnchorController()..messages(30);
    await tester.pumpWidget(harness(c, false));
    await tester.pumpAndSettle();
    await tester.pumpWidget(const SizedBox());
    c.messages(40);
    await tester.pumpWidget(harness(c, false));
    await tester.pumpAndSettle();
    final scroll = transcript(tester);
    expect(scroll.offset.isFinite, isTrue);
    expect(scroll.offset, scroll.position.maxScrollExtent);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
    c.dispose();
  });
}
