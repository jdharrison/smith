import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:smith_client/message_queue.dart';
import 'package:smith_client/queue_exit_guard.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('dev.smith/queue_exit');
  const hold = QueueExitHold(messages: 5, drafts: 3);

  Future<bool> nativeClose() async {
    final reply = Completer<bool>();
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .handlePlatformMessage(
          channel.name,
          channel.codec.encodeMethodCall(const MethodCall('requestClose')),
          (data) => reply.complete(channel.codec.decodeEnvelope(data!) as bool),
        );
    return reply.future;
  }

  test(
    'native requests share one synchronous hold and pending decision',
    () async {
      final decision = Completer<bool>();
      var acquired = 0;
      var dialogs = 0;
      QueueExitHold? released;
      final guard = QueueExitGuard(
        acquireHold: () {
          acquired++;
          return hold;
        },
        releaseHold: (value) => released = value,
        confirm: (value) {
          expect(identical(value, hold), isTrue);
          dialogs++;
          return decision.future;
        },
      );
      addTearDown(guard.dispose);
      final first = nativeClose();
      final second = nativeClose();
      expect(acquired, 1);
      expect(dialogs, 1);
      decision.complete(false);
      expect(await first, isFalse);
      expect(await second, isFalse);
      expect(identical(released, hold), isTrue);
    },
  );

  test('confirm retains hold, including repeated close and disposal', () async {
    var acquired = 0;
    final guard = QueueExitGuard(
      acquireHold: () {
        acquired++;
        return hold;
      },
      releaseHold: (_) => fail('confirmed exit must not resume dispatch'),
      confirm: (_) async => true,
    );
    expect(await nativeClose(), isTrue);
    expect(await nativeClose(), isTrue);
    expect(acquired, 1);
    guard.dispose();
    expect(await guard.requestClose(), isFalse);
  });

  test('empty workspace exits without a dialog and retains hold', () async {
    final guard = QueueExitGuard(
      acquireHold: () => const QueueExitHold(messages: 0, drafts: 0),
      releaseHold: (_) => fail('must retain hold'),
      confirm: (_) async => throw StateError('unexpected dialog'),
    );
    addTearDown(guard.dispose);
    expect(await nativeClose(), isTrue);
  });

  test(
    'decision exception fails closed and permits another decision',
    () async {
      var releases = 0;
      final guard = QueueExitGuard(
        acquireHold: () => hold,
        releaseHold: (value) {
          expect(identical(value, hold), isTrue);
          releases++;
        },
        confirm: (_) async => throw StateError('route unavailable'),
      );
      addTearDown(guard.dispose);
      expect(await nativeClose(), isFalse);
      expect(await nativeClose(), isFalse);
      expect(releases, 2);
    },
  );

  test('acquisition exception fails closed', () async {
    final guard = QueueExitGuard(
      acquireHold: () => throw StateError('unavailable'),
      releaseHold: (_) => fail('no hold acquired'),
      confirm: (_) async => true,
    );
    addTearDown(guard.dispose);
    expect(await nativeClose(), isFalse);
  });

  test('late confirmation after disposal cannot close or release', () async {
    final decision = Completer<bool>();
    final guard = QueueExitGuard(
      acquireHold: () => hold,
      releaseHold: (_) => fail('disposed workspace must not be touched'),
      confirm: (_) => decision.future,
    );
    final pending = nativeClose();
    guard.dispose();
    decision.complete(true);
    expect(await pending, isFalse);
  });

  for (final action in [
    'Keep open',
    'Escape',
    'Discard unsent text and close',
  ]) {
    testWidgets('dialog counts use the hold; $action', (tester) async {
      late BuildContext context;
      await tester.pumpWidget(
        MaterialApp(
          home: Builder(
            builder: (value) {
              context = value;
              return const Scaffold();
            },
          ),
        ),
      );
      final guard = QueueExitGuard(
        acquireHold: () => hold,
        releaseHold: (_) {},
        confirm: (value) => showQueueExitDialog(context, value),
      );
      addTearDown(guard.dispose);
      final pending = nativeClose();
      await tester.pumpAndSettle();
      expect(
        find.textContaining('5 local unsent messages and 3 drafts'),
        findsOneWidget,
      );
      expect(
        find.textContaining('Already accepted service work may continue'),
        findsOneWidget,
      );
      if (action == 'Escape') {
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      } else {
        await tester.tap(find.widgetWithText(TextButton, action));
      }
      await tester.pumpAndSettle();
      expect(await pending, action == 'Discard unsent text and close');
    });
  }
}
