import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:smith_client/main.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/message_queue.dart';
import 'package:smith_client/queue_exit_guard.dart';
import 'package:smith_client/ui/message_queue_panel.dart';
import 'package:smith_client/ui/smith_theme.dart';

import 'message_queue_widget_test.dart' as fixture;
import 'app_fakes.dart';
import 'widget_test.dart' show FakeControlApi, profile;

void main() {
  testWidgets(
    'assurance: actual shell native channel cancels then confirms without service mutation',
    (tester) async {
      final control = FakeControlApi(profile());
      final app = FakeAppApi();
      await tester.pumpWidget(SmithApp(controlApi: control, appApi: app));
      await tester.pumpAndSettle();
      await tester.enterText(
        find.byKey(const ValueKey('chat-composer')),
        'retain draft',
      );
      const channel = MethodChannel('dev.smith/queue_exit');
      Future<bool> close() {
        final result = Completer<bool>();
        tester.binding.defaultBinaryMessenger.handlePlatformMessage(
          channel.name,
          channel.codec.encodeMethodCall(const MethodCall('requestClose')),
          (data) =>
              result.complete(channel.codec.decodeEnvelope(data!) as bool),
        );
        return result.future;
      }

      final first = close();
      await tester.pumpAndSettle();
      expect(find.text('Discard unsent text and close?'), findsOneWidget);
      await tester.tap(find.text('Keep open'));
      await tester.pumpAndSettle();
      expect(await first, isFalse);
      expect(
        tester
            .widget<TextField>(find.byKey(const ValueKey('chat-composer')))
            .controller!
            .text,
        'retain draft',
      );
      final second = close();
      await tester.pumpAndSettle();
      await tester.tap(
        find.widgetWithText(TextButton, 'Discard unsent text and close'),
      );
      await tester.pumpAndSettle();
      expect(await second, isTrue);
      expect(control.calls, ['inspect']);
      expect(
        app.calls.every(
          (op) => ['state', 'models', 'poll', 'conversation_get'].contains(op),
        ),
        isTrue,
      );
      await tester.pumpWidget(const SizedBox());
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('assurance: queue details fits actual navigator 400x400 at 2x', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(400, 400));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final c = fixture.QueueController();
    c.queue.entries = [fixture.row('long', text: 'long content ' * 80)];
    await tester.pumpWidget(
      MaterialApp(
        theme: SmithTheme.dark(),
        builder: (context, child) => MediaQuery(
          data: MediaQuery.of(context)
              .copyWith(textScaler: const TextScaler.linear(2)),
          child: child!,
        ),
        home: Scaffold(body: MessageQueuePanel(controller: c, active: true)),
      ),
    );
    await tester.tap(find.text('Queue details'));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    await tester.tap(find.text('Close'));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
  });

  test(
    'assurance: synchronous reentry during hold acquisition shares decision',
    () async {
      late QueueExitGuard guard;
      Future<bool>? reentrant;
      var acquisitions = 0;
      guard = QueueExitGuard(
        acquireHold: () {
          acquisitions++;
          reentrant = guard.requestClose();
          return const QueueExitHold(messages: 1, drafts: 0);
        },
        releaseHold: (_) {},
        confirm: (_) async => false,
      );
      addTearDown(guard.dispose);
      final first = guard.requestClose();
      expect(identical(first, reentrant), isTrue);
      expect(await first, isFalse);
      expect(acquisitions, 1);
    },
  );
  testWidgets('assurance: close consent fits 400x400 at 2x text', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(400, 400));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    late BuildContext dialogContext;
    await tester.pumpWidget(
      MaterialApp(
        theme: SmithTheme.dark(),
        builder: (context, child) => MediaQuery(
          data: MediaQuery.of(context)
              .copyWith(textScaler: const TextScaler.linear(2)),
          child: child!,
        ),
        home: Builder(
          builder: (context) {
            dialogContext = context;
            return const Scaffold();
          },
        ),
      ),
    );
    final decision = showQueueExitDialog(
      dialogContext,
      const QueueExitHold(messages: 32, drafts: 4),
    );
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    await tester.ensureVisible(find.text('Keep open'));
    await tester.tap(find.text('Keep open'));
    await tester.pumpAndSettle();
    expect(await decision, isFalse);
  });

  testWidgets('assurance: empty draft is not described as queue unavailable', (
    tester,
  ) async {
    final c = fixture.QueueController();
    c.queue.entries = [fixture.row('waiting')];
    await tester.pumpWidget(fixture.harness(c));
    expect(c.queue.accepting, isTrue);
    expect(
      find.text('Cannot queue this draft right now; draft kept.'),
      findsNothing,
    );
  });

  testWidgets(
    'assurance: open details cannot mutate after originating pane becomes inactive',
    (tester) async {
      final c = fixture.QueueController();
      c.queue.entries = [fixture.row('waiting')];
      final active = ValueNotifier(true);
      addTearDown(active.dispose);
      await tester.pumpWidget(
        MaterialApp(
          theme: SmithTheme.dark(),
          home: Scaffold(
            body: ValueListenableBuilder<bool>(
              valueListenable: active,
              builder: (context, value, _) =>
                  MessageQueuePanel(controller: c, active: value),
            ),
          ),
        ),
      );
      await tester.tap(find.text('Queue details'));
      await tester.pumpAndSettle();
      active.value = false;
      await tester.pumpAndSettle();
      final remove = find.descendant(
        of: find.byType(AlertDialog),
        matching: find.text('Remove unsent message'),
      );
      await tester.ensureVisible(remove);
      await tester.tap(remove);
      expect(c.queue.actions, isEmpty);
    },
  );
}
