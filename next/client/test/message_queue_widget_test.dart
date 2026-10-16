import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/chat_panel.dart';
import 'package:smith_client/message_queue.dart';
import 'package:smith_client/text_chat.dart';
import 'package:smith_client/ui/smith_theme.dart';
import 'package:smith_client/ui/message_queue_panel.dart';

import 'app_fakes.dart';

class QueueController extends ChatController {
  QueueController() : super(LegacyFakeAppApi()) {
    queue = FakeQueue(this);
  }
  late final FakeQueue queue;
  int sends = 0;
  @override
  MessageQueue get messageQueue => queue;
  @override
  bool get canSend => draft.isNotEmpty;
  @override
  Future<void> send() async {
    sends++;
  }

  void changed() => notifyListeners();
}

class FakeQueue extends MessageQueue {
  FakeQueue(this.owner);
  final QueueController owner;
  @override
  List<QueuedMessage> entries = [];
  @override
  int revision = 1;
  @override
  bool usesQueue = true;
  bool accepting = true;
  @override
  bool get canEnqueue => accepting && owner.draft.isNotEmpty;
  @override
  bool paused = false;
  @override
  String? pauseReason;
  @override
  int get waitingCount =>
      entries.where((r) => r.state == QueuedMessageState.waiting).length;
  @override
  bool get canResume => paused;
  final enqueued = <String>[];
  int? resumeToken;
  int? clearToken;
  final actions = <String>[];
  @override
  void enqueue() {
    if (!canEnqueue) return;
    final text = owner.draft;
    enqueued.add(text);
    entries = [...entries, row('q${enqueued.length}', text: text)];
    revision++;
    owner.edit('');
  }

  @override
  void pause() {
    paused = true;
    revision++;
    owner.changed();
  }

  @override
  Future<void> resume(int expectedRevision) async {
    resumeToken = expectedRevision;
    if (expectedRevision != revision) return;
    paused = false;
    owner.changed();
  }

  @override
  void clear(int expectedRevision) {
    clearToken = expectedRevision;
    if (expectedRevision != revision) return;
    entries = entries.where((r) => !r.canRemove).toList();
    owner.changed();
  }

  @override
  Future<void> retry(String id, int expectedRevision) async {
    actions.add('retry:$id:$expectedRevision');
  }

  @override
  void skip(String id, int expectedRevision) {
    actions.add('skip:$id:$expectedRevision');
  }

  @override
  void remove(String id, int expectedRevision) {
    actions.add('remove:$id:$expectedRevision');
  }
}

QueuedMessage row(
  String id, {
  String text = 'next task',
  QueuedMessageState state = QueuedMessageState.waiting,
  bool retry = false,
  bool skip = false,
  bool remove = true,
}) => QueuedMessage(
  id: id,
  text: text,
  state: state,
  status: 'Fixture status',
  actionRevision: 7,
  canRetry: retry,
  canSkip: skip,
  canRemove: remove,
);

Widget harness(QueueController c, {bool active = true, double scale = 1}) =>
    MaterialApp(
      theme: SmithTheme.dark(),
      home: MediaQuery(
        data: MediaQueryData(textScaler: TextScaler.linear(scale)),
        child: Scaffold(
          body: ChatPanel(controller: c, active: active),
        ),
      ),
    );

Future<void> details(WidgetTester tester) async {
  await tester.scrollUntilVisible(
    find.text('Queue details'),
    60,
    scrollable: find.descendant(
      of: find.byKey(const ValueKey('message-queue-shelf')),
      matching: find.byType(Scrollable),
    ),
  );
  await tester.tap(find.text('Queue details'));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets(
    'busy empty queue keeps Queue action without reserving shelf space',
    (tester) async {
      final c = QueueController();
      await tester.pumpWidget(harness(c));
      expect(find.text('Queue'), findsOneWidget);
      expect(find.byKey(const ValueKey('message-queue-shelf')), findsNothing);
      c.queue.entries = [row('waiting')];
      c.changed();
      await tester.pump();
      expect(find.byKey(const ValueKey('message-queue-shelf')), findsOneWidget);
      expect(
        find.text('Cannot queue this draft right now; draft kept.'),
        findsNothing,
      );
      c.queue.accepting = false;
      c.edit('retained draft');
      await tester.pump();
      expect(
        find.text('Cannot queue this draft right now; draft kept.'),
        findsOneWidget,
      );
    },
  );

  for (final revoke in ['inactive', 'replaced', 'disposed']) {
    testWidgets('all captured details actions reject $revoke owner', (
      tester,
    ) async {
      final c = QueueController();
      c.queue.entries = [
        row(
          'failed',
          state: QueuedMessageState.failed,
          retry: true,
          skip: true,
        ),
      ];
      final replacement = QueueController();
      final stage = ValueNotifier(0);
      addTearDown(stage.dispose);
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: ValueListenableBuilder<int>(
              valueListenable: stage,
              builder: (context, value, _) => value == 1 && revoke == 'disposed'
                  ? const SizedBox()
                  : MessageQueuePanel(
                      controller: value == 1 && revoke == 'replaced'
                          ? replacement
                          : c,
                      active: value == 0 || revoke != 'inactive',
                    ),
            ),
          ),
        ),
      );
      await tester.tap(find.text('Queue details'));
      await tester.pumpAndSettle();
      final callbacks = <VoidCallback>[];
      for (final label in [
        'Pause',
        'Retry queued message',
        'Skip and continue',
        'Remove unsent message',
        'Clear unsent',
      ]) {
        final button = find.descendant(
          of: find.byType(AlertDialog),
          matching: find.widgetWithText(TextButton, label),
        );
        callbacks.add(tester.widget<TextButton>(button).onPressed!);
      }
      stage.value = 1;
      await tester.pumpAndSettle();
      for (final callback in callbacks) {
        callback();
      }
      c.changed();
      await tester.pumpAndSettle();
      expect(c.queue.actions, isEmpty);
      expect(c.queue.paused, isFalse);
      expect(find.byType(AlertDialog), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  }
  testWidgets(
    'Pause and Remove affect local work through captured row authority',
    (tester) async {
      final c = QueueController();
      c.queue.entries = [row('local')];
      await tester.pumpWidget(harness(c));
      await details(tester);
      final pause = find.descendant(
        of: find.byType(AlertDialog),
        matching: find.text('Pause'),
      );
      await tester.ensureVisible(pause);
      await tester.tap(pause);
      await tester.pumpAndSettle();
      expect(c.queue.paused, isTrue);
      final remove = find.descendant(
        of: find.byType(AlertDialog),
        matching: find.text('Remove unsent message'),
      );
      await tester.ensureVisible(remove);
      await tester.tap(remove);
      expect(c.queue.actions, ['remove:local:7']);
      expect(c.sends, 0);
    },
  );

  testWidgets(
    'shared panes queue once without moving another reader or focus',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(900, 600));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final c = QueueController();
      c.history = List.generate(
        30,
        (i) => {'role': 'assistant', 'content': 'Saved answer $i'},
      );
      await tester.pumpWidget(
        MaterialApp(
          theme: SmithTheme.dark(),
          home: Scaffold(
            body: Row(
              children: [
                Expanded(
                  child: ChatPanel(
                    key: const ValueKey('active'),
                    controller: c,
                    independentView: true,
                  ),
                ),
                Expanded(
                  child: ChatPanel(
                    key: const ValueKey('inactive'),
                    controller: c,
                    active: false,
                    independentView: true,
                  ),
                ),
              ],
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      final otherHistory = find.descendant(
        of: find.byKey(const ValueKey('inactive')),
        matching: find.byKey(const ValueKey('conversation-history')),
      );
      final otherScroll = tester
          .widget<SingleChildScrollView>(otherHistory)
          .controller!;
      otherScroll.jumpTo(0);
      await tester.pumpAndSettle();
      final editor = find.descendant(
        of: find.byKey(const ValueKey('active')),
        matching: find.byKey(const ValueKey('chat-composer')),
      );
      await tester.enterText(editor, 'shared followup');
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(c.queue.enqueued, ['shared followup']);
      expect(otherScroll.offset, 0);
      final focus = tester.widget<TextField>(editor).focusNode!;
      expect(focus.hasFocus, isTrue);
      c.queue.pauseReason = 'Status update';
      c.changed();
      await tester.pumpAndSettle();
      expect(focus.hasFocus, isTrue);
      expect(otherScroll.offset, 0);
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets(
    'idle Send is unchanged; busy action explicitly queues, never adds YOU',
    (tester) async {
      final c = QueueController()..edit('first');
      c.queue.usesQueue = false;
      await tester.pumpWidget(harness(c));
      expect(find.byTooltip('Send'), findsOneWidget);
      expect(find.byKey(const ValueKey('message-queue-shelf')), findsNothing);
      await tester.tap(find.byTooltip('Send'));
      expect(c.sends, 1);
      c.queue.usesQueue = true;
      c.changed();
      await tester.pump();
      expect(find.text('Queue'), findsOneWidget);
      await tester.tap(find.text('Queue'));
      await tester.pump();
      expect(c.queue.enqueued, ['first']);
      expect(c.draft, isEmpty);
      expect(
        find.descendant(
          of: find.byKey(const ValueKey('conversation-history')),
          matching: find.text('first'),
        ),
        findsNothing,
      );
      await details(tester);
      expect(find.text('Waiting locally—not sent'), findsWidgets);
      expect(find.textContaining('Session-only:'), findsWidgets);
      expect(find.textContaining('Capacity:'), findsWidgets);
    },
  );

  testWidgets('Enter/numpad queue once; repeat, Shift and IME do not submit', (
    tester,
  ) async {
    final c = QueueController();
    await tester.pumpWidget(harness(c));
    final editor = find.byKey(const ValueKey('chat-composer'));
    await tester.enterText(editor, 'one');
    await tester.sendKeyDownEvent(LogicalKeyboardKey.enter);
    await tester.sendKeyRepeatEvent(LogicalKeyboardKey.enter);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(c.queue.enqueued, ['one']);
    await tester.enterText(editor, 'two');
    await tester.sendKeyEvent(LogicalKeyboardKey.numpadEnter);
    await tester.pump();
    expect(c.queue.enqueued, ['one', 'two']);
    await tester.enterText(editor, 'three');
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    expect(c.draft, 'three\n');
    tester.testTextInput.updateEditingValue(
      const TextEditingValue(
        text: 'compose',
        selection: TextSelection.collapsed(offset: 7),
        composing: TextRange(start: 0, end: 7),
      ),
    );
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    expect(c.queue.enqueued.length, 2);
  });

  testWidgets('inactive pane and rejected enqueue retain draft', (
    tester,
  ) async {
    final c = QueueController()..edit('keep');
    await tester.pumpWidget(harness(c, active: false));
    expect(
      tester
          .widget<OutlinedButton>(find.widgetWithText(OutlinedButton, 'Queue'))
          .onPressed,
      isNull,
    );
    await tester.tap(find.byKey(const ValueKey('chat-composer')));
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    expect(c.queue.enqueued, isEmpty);
    c.queue.accepting = false;
    await tester.pumpWidget(harness(c));
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    expect(c.draft, 'keep');
    expect(c.queue.enqueued, isEmpty);
  });

  testWidgets(
    'failed controls carry row token; unknown/running offer no retry or skip',
    (tester) async {
      final c = QueueController();
      c.queue.entries = [
        row(
          'failed',
          state: QueuedMessageState.failed,
          retry: true,
          skip: true,
          remove: false,
        ),
        row('unknown', state: QueuedMessageState.unknown, remove: false),
        row('running', state: QueuedMessageState.accepted, remove: false),
      ];
      await tester.pumpWidget(harness(c));
      await details(tester);
      final dialog = find.byType(AlertDialog);
      for (final label in ['Retry queued message', 'Skip and continue']) {
        final control = find.descendant(of: dialog, matching: find.text(label));
        expect(control, findsOneWidget);
        await tester.ensureVisible(control);
        await tester.tap(control);
      }
      expect(c.queue.actions, ['retry:failed:7', 'skip:failed:7']);
      expect(
        find.descendant(
          of: dialog,
          matching: find.text('Remove unsent message'),
        ),
        findsNothing,
      );
    },
  );

  for (final resume in [true, false]) {
    testWidgets(
      '${resume ? 'Resume' : 'Clear'} confirmation retains stale snapshot token',
      (tester) async {
        final c = QueueController();
        c.queue.entries = [row('waiting')];
        c.queue.paused = true;
        c.queue.pauseReason = 'Model configuration changed';
        await tester.pumpWidget(harness(c));
        await details(tester);
        final button = find.descendant(
          of: find.byType(AlertDialog),
          matching: find.text(resume ? 'Resume' : 'Clear unsent'),
        );
        await tester.ensureVisible(button);
        await tester.tap(button);
        await tester.pumpAndSettle();
        expect(
          find.text(
            resume ? 'Resume queued messages?' : 'Clear 1 unsent messages?',
          ),
          findsOneWidget,
        );
        final token = c.queue.revision;
        c.queue.revision++;
        c.changed();
        await tester.pump();
        await tester.tap(
          find.text(resume ? 'Confirm resume' : 'Confirm clear'),
        );
        await tester.pumpAndSettle();
        expect(resume ? c.queue.resumeToken : c.queue.clearToken, token);
        expect(c.queue.paused, isTrue);
        expect(c.queue.entries.length, 1);
      },
    );
  }

  testWidgets('400x400 enlarged long queue and recovery remain bounded', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(400, 400));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final c = QueueController()..error = 'Unavailable ' * 40;
    c.queue.entries = [row('long', text: 'unbroken_text' * 500)];
    await tester.pumpWidget(harness(c, scale: 2));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    expect(
      tester.getSize(find.byKey(const ValueKey('message-queue-shelf'))).height,
      lessThanOrEqualTo(120),
    );
    await details(tester);
    expect(tester.takeException(), isNull);
    await tester.tap(find.text('Close'));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
  });
}
