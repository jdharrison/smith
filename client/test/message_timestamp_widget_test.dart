import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/chat_panel.dart';
import 'package:smith_client/text_chat.dart';
import 'package:smith_client/ui/message_header.dart';
import 'package:smith_client/ui/smith_theme.dart';

import 'app_fakes.dart';

final accepted = DateTime.utc(2026, 10, 15, 13, 3, 12).millisecondsSinceEpoch;
final completed = DateTime.utc(2026, 10, 15, 13, 3, 27).millisecondsSinceEpoch;

ChatController savedConversation() => ChatController(LegacyFakeAppApi())
  ..stale = false
  ..history = [
    {'role': 'user', 'content': 'Saved question'},
    {'role': 'assistant', 'content': 'Saved reply'},
  ]
  ..historyTimestamps = [accepted, completed];

const captureKey = ValueKey('message-timestamp-preview');

Widget harness(ChatController controller, {double scale = 1}) =>
    RepaintBoundary(
      key: captureKey,
      child: MaterialApp(
        debugShowCheckedModeBanner: false,
        theme: SmithTheme.dark(),
        home: MediaQuery(
          data: MediaQueryData(textScaler: TextScaler.linear(scale)),
          child: Scaffold(body: ChatPanel(controller: controller)),
        ),
      ),
    );

Future<void> capturePreview(
  WidgetTester tester,
  String name, {
  bool fromStart = false,
}) async {
  if (!const bool.fromEnvironment('SMITH_TIMESTAMP_CAPTURES')) return;
  if (fromStart) {
    tester
        .widget<SingleChildScrollView>(
          find.byKey(const ValueKey('conversation-history')),
        )
        .controller!
        .jumpTo(0);
    await tester.pump();
  }
  final boundary = tester.renderObject<RenderRepaintBoundary>(
    find.byKey(captureKey),
  );
  await tester.runAsync(() async {
    final image = await boundary.toImage(pixelRatio: 1);
    try {
      final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
      expect(bytes, isNotNull);
      final file = File('build/message-timestamp-previews/$name.png');
      await file.parent.create(recursive: true);
      await file.writeAsBytes(bytes!.buffer.asUint8List());
    } finally {
      image.dispose();
    }
  });
}

Finder headerIn(String key) => find.descendant(
  of: find.byKey(ValueKey(key)),
  matching: find.byType(MessageHeader),
);

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    final loader = FontLoader(SmithTheme.defaultFontFamily);
    for (final weight in ['Regular', 'Medium', 'SemiBold']) {
      loader.addFont(
        rootBundle.load(
          'assets/fonts/jetbrains-mono/JetBrainsMono-$weight.ttf',
        ),
      );
    }
    await loader.load();
  });

  group('timestamp formatting', () {
    test('omits unknown and invalid instants rather than inventing dates', () {
      for (final value in [null, 0, -1, 253402300800000]) {
        expect(formatMessageTimestamp(value, utcOffset: Duration.zero), isNull);
      }
      expect(
        formatMessageTimestamp(253402300799999, utcOffset: Duration.zero),
        '9999-12-31 23:59:59 +00:00',
      );
    });

    test(
      'pads date/time and displays seconds without rounding milliseconds',
      () {
        expect(
          formatMessageTimestamp(
            DateTime.utc(2026, 1, 2, 3, 4, 5, 987).millisecondsSinceEpoch,
            utcOffset: Duration.zero,
          ),
          '2026-01-02 03:04:05 +00:00',
        );
      },
    );

    test('offsets preserve midnight, year and leap-day boundaries', () {
      final newYear = DateTime.utc(2027, 1, 1, 0, 15, 2).millisecondsSinceEpoch;
      expect(
        formatMessageTimestamp(newYear, utcOffset: const Duration(hours: -5)),
        '2026-12-31 19:15:02 -05:00',
      );
      expect(
        formatMessageTimestamp(
          newYear,
          utcOffset: const Duration(hours: 5, minutes: 45),
        ),
        '2027-01-01 06:00:02 +05:45',
      );
      expect(
        formatMessageTimestamp(
          DateTime.utc(2024, 3, 1, 2, 3, 4).millisecondsSinceEpoch,
          utcOffset: const Duration(hours: -3, minutes: -30),
        ),
        '2024-02-29 22:33:04 -03:30',
      );
    });

    test('explicit event offsets distinguish repeated DST wall times', () {
      expect(
        formatMessageTimestamp(
          DateTime.utc(2026, 10, 25, 0, 30).millisecondsSinceEpoch,
          utcOffset: const Duration(hours: 1),
        ),
        '2026-10-25 01:30:00 +01:00',
      );
      expect(
        formatMessageTimestamp(
          DateTime.utc(2026, 10, 25, 1, 30).millisecondsSinceEpoch,
          utcOffset: Duration.zero,
        ),
        '2026-10-25 01:30:00 +00:00',
      );
      expect(
        formatMessageTimestamp(
          DateTime.utc(2026, 3, 29, 1).millisecondsSinceEpoch,
          utcOffset: const Duration(hours: 1),
        ),
        '2026-03-29 02:00:00 +01:00',
      );
    });

    test('default conversion uses the workstation offset at the event', () {
      final local = DateTime.fromMillisecondsSinceEpoch(accepted);
      expect(
        formatMessageTimestamp(accepted),
        formatMessageTimestamp(accepted, utcOffset: local.timeZoneOffset),
      );
    });
  });

  testWidgets('saved speakers expose small timestamps and event semantics', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1440, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final semantics = tester.ensureSemantics();
    try {
      final c = savedConversation();
      addTearDown(c.dispose);
      await tester.pumpWidget(harness(c));
      await tester.pumpAndSettle();

      for (final (value, event) in [
        (accepted, 'Accepted by Smith'),
        (completed, 'Reply completed by Smith'),
      ]) {
        final timestamp = formatMessageTimestamp(value)!;
        final description = '$event · $timestamp (local time)';
        expect(find.text(timestamp), findsOneWidget);
        expect(find.byTooltip(description), findsOneWidget);
        expect(find.bySemanticsLabel(description), findsOneWidget);
        final text = tester.widget<Text>(find.text(timestamp));
        expect(text.style!.fontSize, 12);
        expect(text.style!.color, SmithColors.textSecondary);
        expect(text.maxLines, isNull);
      }
      expect(
        tester.widget<Text>(find.text('YOU')).style!.color,
        SmithColors.textPrimary,
      );
      expect(
        tester.widget<Text>(find.text('SMITH')).style!.color,
        SmithColors.accent,
      );
      expect(find.byType(SelectableText), findsNWidgets(2));
      expect((c.api as LegacyFakeAppApi).calls, isEmpty);
      expect(tester.takeException(), isNull);
      await capturePreview(tester, 'desktop-1440x900');
      await tester.pumpWidget(const SizedBox());
    } finally {
      semantics.dispose();
    }
  });

  testWidgets('mixed old and new history omits unknown timestamps', (
    tester,
  ) async {
    final c = savedConversation();
    addTearDown(c.dispose);
    c.history.insertAll(0, [
      {'role': 'user', 'content': 'Old question'},
      {'role': 'assistant', 'content': 'Old reply'},
    ]);
    c.historyTimestamps.insertAll(0, [null, null]);
    await tester.pumpWidget(harness(c));
    await tester.pumpAndSettle();
    expect(
      tester
          .widgetList<MessageHeader>(find.byType(MessageHeader))
          .map((header) => header.timestampMs),
      [null, null, accepted, completed],
    );
    expect(find.text('Time unknown'), findsNothing);
    expect(find.text(formatMessageTimestamp(accepted)!), findsOneWidget);
    expect(find.text(formatMessageTimestamp(completed)!), findsOneWidget);
    for (final header in find.byType(MessageHeader).evaluate().take(2)) {
      expect(
        find.descendant(
          of: find.byWidget(header.widget),
          matching: find.byType(Tooltip),
        ),
        findsNothing,
      );
    }
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  for (final width in [360.0, 400.0]) {
    testWidgets('metadata wraps within a $width by 400 pane at 2x text', (
      tester,
    ) async {
      await tester.binding.setSurfaceSize(Size(width, 400));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final c = savedConversation();
      addTearDown(c.dispose);
      await tester.pumpWidget(harness(c, scale: 2));
      await tester.pumpAndSettle();
      for (final (value, speaker, user) in [
        (accepted, 'YOU', true),
        (completed, 'SMITH', false),
      ]) {
        final label = find.text(formatMessageTimestamp(value)!);
        final header = find.ancestor(
          of: label,
          matching: find.byType(MessageHeader),
        );
        final rect = tester.getRect(header);
        final timestampRect = tester.getRect(label);
        expect(rect.left, greaterThanOrEqualTo(16));
        expect(rect.right, lessThanOrEqualTo(width - 16));
        expect(timestampRect.left, greaterThanOrEqualTo(rect.left));
        expect(timestampRect.right, lessThanOrEqualTo(rect.right));
        expect(
          timestampRect.height,
          greaterThan(tester.getSize(find.text(speaker)).height),
        );
        expect(
          timestampRect.top,
          greaterThan(tester.getRect(find.text(speaker)).bottom),
        );
        expect(
          tester.widget<Text>(label).textAlign,
          user ? TextAlign.right : TextAlign.left,
        );
        expect(
          user ? rect.right : rect.left,
          closeTo(user ? width - 16 : 16, .01),
        );
      }
      expect(
        tester.getRect(find.byTooltip('Send')).bottom,
        lessThanOrEqualTo(400),
      );
      expect(tester.takeException(), isNull);
      if (width == 400) {
        await capturePreview(tester, 'narrow-400x400-text2', fromStart: true);
      }
      await tester.pumpWidget(const SizedBox());
    });
  }

  for (final (status, active) in [
    ('Sending…', true),
    ('Queued…', true),
    ('Thinking…', true),
    ('Not sent', false),
    ('Failed', false),
    ('Outcome unknown', false),
  ]) {
    testWidgets('$status and local/provisional bubbles have no timestamp', (
      tester,
    ) async {
      final c = savedConversation();
      addTearDown(c.dispose);
      c.localSends['pending'] = LocalTextSend('Local question')
        ..status = status
        ..active = active;
      c.localSends['answered'] = LocalTextSend('Provisional question')
        ..active = false;
      c.provisionalAnswers['answered'] = 'Provisional reply';
      c.provisionalAnswers['reattached'] = 'Reattached reply';
      await tester.pumpWidget(harness(c));
      await tester.pumpAndSettle();
      for (final key in [
        'local-user-pending',
        'local-status-pending',
        'local-user-answered',
        'terminal-answer-answered',
        'terminal-answer-reattached',
      ]) {
        expect(tester.widget<MessageHeader>(headerIn(key)).timestampMs, isNull);
        expect(
          find.descendant(of: headerIn(key), matching: find.byType(Tooltip)),
          findsNothing,
        );
      }
      expect(find.text(status), findsOneWidget);
      expect(find.text('Time unknown'), findsNothing);
      expect(find.text(formatMessageTimestamp(accepted)!), findsOneWidget);
      expect(find.text(formatMessageTimestamp(completed)!), findsOneWidget);
      expect(
        find.text('Verified answer · awaiting history sync'),
        findsNWidgets(2),
      );
      expect((c.api as LegacyFakeAppApi).calls, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    });
  }

  testWidgets(
    'metadata arrival preserves draft, focus, selection and reading',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(400, 400));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final c = savedConversation()
        ..history = List.generate(
          24,
          (i) => {
            'role': i.isEven ? 'user' : 'assistant',
            'content': 'Retained message $i\nMore selectable message text',
          },
        )
        ..historyTimestamps = [];
      addTearDown(c.dispose);
      await tester.pumpWidget(harness(c, scale: 2));
      await tester.pumpAndSettle();
      final field = find.byType(TextField);
      await tester.tap(field);
      await tester.enterText(field, 'Keep this draft');
      await tester.pumpAndSettle();
      final editor = tester.widget<TextField>(field);
      editor.controller!.selection = const TextSelection(
        baseOffset: 2,
        extentOffset: 6,
      );
      final scroll = tester
          .widget<SingleChildScrollView>(
            find.byKey(const ValueKey('conversation-history')),
          )
          .controller!;
      scroll.jumpTo(100);
      c.historyTimestamps = List.generate(24, (i) => accepted + i * 1000);
      c.edit(c.draft);
      await tester.pumpAndSettle();
      expect(scroll.offset, closeTo(100, .01));
      expect(editor.focusNode!.hasFocus, isTrue);
      expect(editor.controller!.text, 'Keep this draft');
      expect(
        editor.controller!.selection,
        const TextSelection(baseOffset: 2, extentOffset: 6),
      );
      expect(find.byType(MessageHeader), findsNWidgets(24));
      expect((c.api as LegacyFakeAppApi).calls, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );
}
