import 'dart:ui' show PointerDeviceKind;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/chat_panel.dart';
import 'package:smith_client/text_chat.dart';
import 'package:smith_client/ui/smith_theme.dart';

import 'app_fakes.dart';

void main() {
  for (final width in [360.0, 1440.0]) {
    for (final scale in [1.0, 2.0]) {
      testWidgets('edge scrollbar at width $width / text $scale', (
        tester,
      ) async {
        await tester.binding.setSurfaceSize(Size(width, 800));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final c = ChatController(LegacyFakeAppApi())
          ..stale = false
          ..history = List.generate(
            30,
            (i) => {
              'role': i.isEven ? 'assistant' : 'user',
              'content': 'Message $i: ${'readable conversation text ' * 8}',
            },
          );
        final other = ChatController(LegacyFakeAppApi())..stale = false;
        addTearDown(c.dispose);
        addTearDown(other.dispose);

        Widget harness(ChatController controller) => MaterialApp(
          theme: SmithTheme.dark().copyWith(platform: TargetPlatform.linux),
          home: MediaQuery(
            data: MediaQueryData(textScaler: TextScaler.linear(scale)),
            child: Scaffold(
              body: ChatPanel(controller: controller, independentView: true),
            ),
          ),
        );
        await tester.pumpWidget(harness(c));
        await tester.pumpAndSettle();
        final history = find.byKey(const ValueKey('conversation-history'));
        final scroll = tester
            .widget<SingleChildScrollView>(history)
            .controller!;
        final viewport = tester.getRect(history);
        expect(viewport.left, 0);
        expect(viewport.right, width);
        expect(scroll.offset, scroll.position.maxScrollExtent);

        // Count both explicit and desktop-injected RawScrollbars belonging to
        // this controller; editor/recovery scrollbars are intentionally separate.
        final bars = find.byWidgetPredicate(
          (w) => w is RawScrollbar && w.controller == scroll,
        );
        expect(bars, findsOneWidget);
        final bar = tester.widget<RawScrollbar>(bars);
        expect(bar.scrollbarOrientation, ScrollbarOrientation.right);
        final theme = ScrollbarTheme.of(tester.element(bars));
        expect(theme.crossAxisMargin, 0);
        expect(theme.thickness!.resolve({WidgetState.hovered}), 8);
        expect(
          find.ancestor(of: history, matching: find.byType(Scrollbar)),
          findsOneWidget,
        );

        final inset = width < 600 ? 16.0 : (width - 960) / 2 + 24;
        for (var i = 0; i < 30; i++) {
          final bubble = tester.getRect(
            find
                .ancestor(
                  of: find.text(c.history[i]['content'] as String),
                  matching: find.byType(Container),
                )
                .first,
          );
          expect(bubble.left, greaterThanOrEqualTo(inset));
          expect(bubble.right, lessThanOrEqualTo(width - inset));
          expect(bubble.right, lessThanOrEqualTo(viewport.right - 16));
          if (i.isEven) {
            expect(bubble.left, closeTo(inset, .01));
          } else {
            expect(bubble.right, closeTo(width - inset, .01));
          }
        }
        final composer = find.byKey(const ValueKey('chat-composer'));
        final composerRect = tester.getRect(composer);
        final composerInset = width < 600 ? 8.0 : (width - 960) / 2 + 24;
        expect(composerRect.left, composerInset);
        expect(
          tester.getRect(find.byTooltip('Send')).right,
          width - composerInset,
        );
        expect(composerRect.bottom, 792);
        expect(viewport.bottom, lessThan(composerRect.top));

        // Hover reveals the real thumb, then drag at the pane's right edge.
        // This also rejects a scrollbar still painted at the centered column.
        final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
        await mouse.addPointer(location: const Offset(0, 0));
        await mouse.moveTo(Offset(width - 4, viewport.bottom - 14));
        await tester.pumpAndSettle();
        await mouse.down(Offset(width - 4, viewport.bottom - 14));
        await mouse.moveBy(const Offset(0, -24));
        await tester.pump();
        await mouse.moveBy(const Offset(0, -120));
        await mouse.up();
        await tester.pumpAndSettle();
        expect(scroll.offset, lessThan(scroll.position.maxScrollExtent - 50));
        expect(scroll.offset, greaterThan(0));
        final bookmark = scroll.offset;
        expect(tester.getRect(composer), composerRect);

        c.edit('draft update must not move the transcript');
        await tester.pumpAndSettle();
        expect(scroll.offset, closeTo(bookmark, .01));
        await tester.pumpWidget(harness(other));
        await tester.pumpAndSettle();
        await tester.pumpWidget(harness(c));
        await tester.pumpAndSettle();
        expect(scroll.offset, closeTo(bookmark, .01));
        expect(tester.getRect(composer).bottom, composerRect.bottom);
        expect(tester.takeException(), isNull);
        await mouse.removePointer();
        await tester.pumpWidget(const SizedBox());
      });
    }
  }
}
