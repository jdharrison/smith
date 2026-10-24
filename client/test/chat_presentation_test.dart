import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/chat_panel.dart';
import 'package:smith_client/main.dart';
import 'package:smith_client/text_chat.dart';
import 'package:smith_client/ui/smith_theme.dart';
import 'package:smith_client/ui/smith_icon.dart';

import 'app_fakes.dart';
import 'widget_test.dart' show FakeControlApi, profile;

// Presentation-only spy: no service, native helper or inference is constructed.
class PresentationController extends ChatController {
  PresentationController() : super(LegacyFakeAppApi());
  int sends = 0;
  bool expired = false;
  bool get observed => hasListeners;
  @override
  bool get canSend => !busy && pendingId == null && draft.isNotEmpty;
  @override
  bool get canAbandonExpiredRequest => expired;
  @override
  Future<void> send() async {
    sends++;
  }
}

Widget harness(
  ChatController c, {
  double scale = 1,
  VoidCallback? retry,
  double topInset = 0,
}) => MaterialApp(
  theme: SmithTheme.dark(),
  home: MediaQuery(
    data: MediaQueryData(textScaler: TextScaler.linear(scale)),
    child: Scaffold(
      body: Padding(
        padding: EdgeInsets.only(top: topInset),
        child: ChatPanel(controller: c, onRetry: retry),
      ),
    ),
  ),
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
  for (final status in ['Sending…', 'Queued…', 'Thinking…']) {
    for (final scale in [1.0, 2.0]) {
      testWidgets('compact $status is one status at text scale $scale', (
        tester,
      ) async {
        await tester.binding.setSurfaceSize(const Size(360, 400));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final c = PresentationController()
          ..stale = false
          ..busy = true
          ..pendingId = 'active'
          ..pendingKind = 'submit';
        c.localSends['active'] = LocalTextSend('Question')..status = status;
        await tester.pumpWidget(harness(c, scale: scale));
        await tester.pumpAndSettle();
        final statusWidget = find.text(status);
        expect(statusWidget, findsOneWidget);
        expect(tester.widget<Text>(statusWidget).maxLines, 2);
        expect(
          tester.getSize(statusWidget).height,
          lessThanOrEqualTo(42 * scale),
        );
        expect(find.text('Request in progress'), findsNothing);
        expect(find.byIcon(Icons.hourglass_top), findsNothing);
        expect(
          find.byKey(const ValueKey('conversation-recovery')),
          findsNothing,
        );
        expect(find.text('Request pending'), findsNothing);
        expect(find.text('Retry'), findsNothing);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox());
        c.dispose();
      });
    }
  }

  for (final problem in [
    'error',
    'stale',
    'disconnected',
    'unknown',
    'expired',
  ]) {
    testWidgets('$problem keeps pending recovery visible', (tester) async {
      final c = PresentationController()
        ..stale = problem == 'stale'
        ..connected = problem != 'disconnected'
        ..pendingId = 'active'
        ..pendingKind = 'submit'
        ..expired = problem == 'expired'
        ..error = problem == 'error' ? 'Request failed' : null;
      c.localSends['active'] = LocalTextSend('Question')
        ..active = problem != 'unknown'
        ..status = problem == 'unknown' ? 'Outcome unknown' : 'Sending…';
      await tester.pumpWidget(harness(c));
      await tester.pumpAndSettle();
      expect(
        find.byKey(const ValueKey('conversation-recovery')),
        findsOneWidget,
      );
      expect(find.text('Retry'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      c.dispose();
    });
  }

  for (final size in [
    const Size(400, 400),
    const Size(360, 800),
    const Size(1440, 900),
    const Size(4200, 2400),
  ]) {
    testWidgets('bounded geometry, long lines and recovery at $size / 2x', (
      tester,
    ) async {
      await tester.binding.setSurfaceSize(size);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final c = PresentationController()
        ..history = [
          {'role': 'assistant', 'content': 'long_unbroken_line_' * 100},
          {'role': 'user', 'content': 'User message'},
        ]
        ..error = 'Unavailable. ' * 40
        ..pendingId = 'expired-request'
        ..pendingKind = 'submit'
        ..phase = 'expired'
        ..expired = true
        ..edit('multiline\ndraft\nwith\nbounded\ngrowth');
      var retries = 0;
      await tester.pumpWidget(harness(c, scale: 2, retry: () => retries++));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      final send = find.byTooltip('Send');
      final sendRect = tester.getRect(send);
      expect(sendRect.size, const Size(48, 48));
      expect(sendRect.bottom, lessThanOrEqualTo(size.height));
      expect(
        tester.getSize(find.byKey(const ValueKey('chat-composer'))).width,
        lessThanOrEqualTo(904),
      );
      expect(find.byType(SelectableText), findsNWidgets(3));
      await tester.ensureVisible(find.text('Retry'));
      await tester.tap(find.text('Retry'));
      expect(retries, 1);
      await tester.ensureVisible(find.text('Resolve expired request'));
      await tester.tap(find.text('Resolve expired request'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      await tester.ensureVisible(find.text('Keep pending'));
      await tester.tap(find.text('Keep pending'));
      await tester.pumpAndSettle();
      expect(c.pendingId, 'expired-request');
      expect(c.sends, 0);
      await tester.pumpWidget(const SizedBox());
      c.dispose();
    });
  }

  testWidgets(
    'compact recovery shows full details and explicit actions without resending',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(400, 400));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final c = PresentationController()
        ..error = 'Unavailable. ' * 20
        ..pendingId = 'expired'
        ..pendingKind = 'submit'
        ..expired = true
        ..edit('draft\nwith\nmultiple\nlines');
      var retries = 0;
      await tester.pumpWidget(
        harness(c, scale: 2, topInset: 220, retry: () => retries++),
      );
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(
        tester.getRect(find.byTooltip('Send')).bottom,
        lessThanOrEqualTo(400),
      );
      expect(
        tester.getSize(find.byKey(const ValueKey('chat-composer'))).height,
        lessThanOrEqualTo(64),
      );
      expect(find.text('Needs attention').hitTestable(), findsOneWidget);
      await tester.tap(find.byTooltip('View recovery details'));
      await tester.pumpAndSettle();
      expect(find.text(c.error!), findsOneWidget);
      expect(
        find.text(
          'Pending submit · Ready. Retry queries only; it never resends.',
        ),
        findsOneWidget,
      );
      expect(retries, 0);
      expect(c.sends, 0);
      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();
      expect(retries, 0);
      expect(c.draft, 'draft\nwith\nmultiple\nlines');
      await tester.tap(find.byTooltip('View recovery details'));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.text('Retry'));
      await tester.tap(find.text('Retry'));
      await tester.pumpAndSettle();
      expect(retries, 1);
      await tester.tap(find.byTooltip('View recovery details'));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.text('Resolve expired request'));
      await tester.tap(find.text('Resolve expired request'));
      await tester.pumpAndSettle();
      expect(find.text('Forget expired request locally?'), findsOneWidget);
      await tester.ensureVisible(find.text('Keep pending'));
      await tester.tap(find.text('Keep pending'));
      await tester.pumpAndSettle();
      expect(c.pendingId, 'expired');
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      c.dispose();
    },
  );

  testWidgets(
    'production 400x400 at 2x preserves draft through Settings and failure',
    (tester) async {
      tester.platformDispatcher.textScaleFactorTestValue = 2;
      addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
      await tester.binding.setSurfaceSize(const Size(400, 400));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final api = FakeAppApi();
      final core = FakeControlApi(profile());
      await tester.pumpWidget(SmithApp(controlApi: core, appApi: api));
      await tester.pumpAndSettle();
      for (final failed in [false, true]) {
        if (failed) {
          api.stateFailure = true;
          await tester.pump(const Duration(seconds: 30));
          await tester.pumpAndSettle();
          expect(find.text('SYSTEM UNAVAILABLE'), findsOneWidget);
          expect(find.text('Needs attention').hitTestable(), findsOneWidget);
          final calls = List<String>.from(api.calls);
          await tester.tap(find.byTooltip('View recovery details'));
          await tester.pumpAndSettle();
          expect(find.textContaining('TRANSPORT'), findsOneWidget);
          await tester.tap(find.text('Close'));
          await tester.pumpAndSettle();
          expect(api.calls, calls);
        }
        await tester.enterText(
          find.byType(TextField),
          'Review the complete wrapped draft\nand retain it',
        );
        await tester.pumpAndSettle();
        final field = tester.widget<TextField>(find.byType(TextField));
        final height = tester.getSize(find.byType(TextField)).height;
        final padding = field.decoration!.contentPadding!.resolve(
          TextDirection.ltr,
        );
        final lineHeight = 2 * field.style!.fontSize! * field.style!.height!;
        expect(height - padding.vertical - 2, greaterThanOrEqualTo(lineHeight));
        final editable = tester.state<EditableTextState>(
          find.descendant(
            of: find.byType(TextField),
            matching: find.byType(EditableText),
          ),
        );
        expect(editable.widget.focusNode.hasFocus, isTrue);
        final render = editable.renderEditable;
        final caret = render.getLocalRectForCaret(
          field.controller!.selection.extent,
        );
        final caretTop = render.localToGlobal(caret.topLeft).dy;
        final caretBottom = render.localToGlobal(caret.bottomLeft).dy;
        final fieldRect = tester.getRect(find.byType(TextField));
        expect(caretTop, greaterThanOrEqualTo(fieldRect.top));
        expect(caretBottom, lessThanOrEqualTo(fieldRect.bottom));
        expect(find.byTooltip('Send').hitTestable(), findsOneWidget);
        expect(tester.takeException(), isNull);
        final appCalls = List<String>.from(api.calls);
        final coreCalls = List<String>.from(core.calls);
        await tester.tap(find.byTooltip('Settings'));
        await tester.pumpAndSettle();
        await tester.ensureVisible(find.byTooltip('Return to Chat'));
        await tester.pumpAndSettle();
        expect(find.byTooltip('Return to Chat').hitTestable(), findsOneWidget);
        await tester.tap(find.byTooltip('Return to Chat'));
        await tester.pumpAndSettle();
        expect(
          tester.widget<TextField>(find.byType(TextField)).controller!.text,
          'Review the complete wrapped draft\nand retain it',
        );
        expect(api.calls, [...appCalls, 'instructions_get']);
        expect(core.calls, coreCalls);
        expect(
          api.calls.where((op) => ['submit', 'configure'].contains(op)),
          isEmpty,
        );
        expect(tester.takeException(), isNull);
      }
      await tester.pumpWidget(const SizedBox());
    },
  );

  for (final replaceController in [false, true]) {
    for (final resolve in [false, true]) {
      testWidgets(
        'compact recovery revalidates identity replace=$replaceController resolve=$resolve',
        (tester) async {
          await tester.binding.setSurfaceSize(const Size(400, 400));
          addTearDown(() => tester.binding.setSurfaceSize(null));
          final a = PresentationController()
            ..stale = false
            ..pendingId = 'original-request'
            ..pendingKind = 'submit'
            ..expired = resolve
            ..edit('retained draft');
          final b = PresentationController()..edit('replacement draft');
          var retries = 0;
          await tester.pumpWidget(
            harness(a, scale: 2, topInset: 220, retry: () => retries++),
          );
          expect(
            find.text(resolve ? 'Needs attention' : 'Request pending'),
            findsOneWidget,
          );
          await tester.tap(find.byTooltip('View recovery details'));
          await tester.pumpAndSettle();
          if (replaceController) {
            await tester.pumpWidget(
              harness(b, scale: 2, topInset: 220, retry: () => retries++),
            );
          } else {
            a.pendingId = 'different-request';
          }
          final action = find.text(
            resolve ? 'Resolve expired request' : 'Retry',
          );
          await tester.ensureVisible(action);
          await tester.tap(action);
          await tester.pumpAndSettle();
          expect(retries, 0);
          expect(find.text('Forget expired request locally?'), findsNothing);
          expect(
            a.pendingId,
            replaceController ? 'original-request' : 'different-request',
          );
          expect(a.draft, 'retained draft');
          expect(b.draft, 'replacement draft');
          expect((a.api as LegacyFakeAppApi).calls, isEmpty);
          expect((b.api as LegacyFakeAppApi).calls, isEmpty);
          await tester.pumpWidget(const SizedBox());
          a.dispose();
          b.dispose();
        },
      );
    }
  }

  testWidgets(
    'native selection, Enter variants, repeat, disabled send and IME',
    (tester) async {
      final c = PresentationController()..stale = false;
      await tester.pumpWidget(harness(c));
      final field = find.byType(TextField);
      await tester.enterText(field, 'draft');
      await tester.sendKeyDownEvent(LogicalKeyboardKey.enter);
      await tester.sendKeyRepeatEvent(LogicalKeyboardKey.enter);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.enter);
      expect(c.sends, 1);
      await tester.sendKeyEvent(LogicalKeyboardKey.numpadEnter);
      expect(c.sends, 2);
      await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
      expect(c.sends, 2);
      expect(c.draft, 'draft\n');
      final editing = tester.widget<TextField>(field).controller!;
      editing.selection = const TextSelection(baseOffset: 0, extentOffset: 5);
      await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftRight);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftRight);
      expect(c.draft, '\n\n');
      c.busy = true;
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      expect(c.sends, 2);
      expect(c.draft, '\n\n');
      c.busy = false;
      tester.testTextInput.updateEditingValue(
        const TextEditingValue(
          text: 'composing',
          selection: TextSelection.collapsed(offset: 9),
          composing: TextRange(start: 0, end: 9),
        ),
      );
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      expect(c.sends, 2);
      await tester.pumpWidget(const SizedBox());
      c.dispose();
    },
  );

  testWidgets(
    'one initial scroll; parent rebuild and edits preserve reading/selection',
    (tester) async {
      final c = PresentationController()
        ..stale = false
        ..history = List.generate(
          40,
          (i) => {
            'role': i.isEven ? 'user' : 'assistant',
            'content': 'message $i',
          },
        )
        ..edit('preserved draft');
      await tester.pumpWidget(harness(c));
      await tester.pumpAndSettle();
      final scroll = tester
          .widget<SingleChildScrollView>(
            find.byKey(const ValueKey('conversation-history')),
          )
          .controller!;
      expect(scroll.offset, scroll.position.maxScrollExtent);
      scroll.jumpTo(100);
      final editing = tester
          .widget<TextField>(find.byType(TextField))
          .controller!;
      editing.selection = const TextSelection(baseOffset: 0, extentOffset: 4);
      await tester.pumpWidget(harness(c));
      await tester.pump();
      expect(
        editing.selection,
        const TextSelection(baseOffset: 0, extentOffset: 4),
      );
      c.edit('newer draft');
      await tester.pump();
      expect(scroll.offset, 100);
      expect(editing.text, 'newer draft');
      expect(find.text('YOU'), findsWidgets);
      expect(find.text('SMITH'), findsWidgets);
      await tester.pumpWidget(const SizedBox());
      c.dispose();
    },
  );

  testWidgets(
    'replacement detaches old listener without disposing either controller',
    (tester) async {
      final a = PresentationController()..edit('first');
      final b = PresentationController()..edit('second');
      await tester.pumpWidget(harness(a));
      await tester.pumpWidget(harness(b));
      final editing = tester
          .widget<TextField>(find.byType(TextField))
          .controller!;
      expect(editing.text, 'second');
      expect(a.observed, isFalse);
      a.edit('old notification');
      b.edit('new notification');
      await tester.pump();
      expect(editing.text, 'new notification');
      expect((a.api as LegacyFakeAppApi).disposed, isFalse);
      await tester.pumpWidget(const SizedBox());
      expect(b.observed, isFalse);
      expect((b.api as LegacyFakeAppApi).disposed, isFalse);
      a.edit('still owned externally');
      b.edit('also external');
      expect(tester.takeException(), isNull);
      a.dispose();
      b.dispose();
    },
  );

  testWidgets('desktop geometry snapshot and selectable message text', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1440, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final c = PresentationController()
      ..stale = false
      ..history = [
        {'role': 'assistant', 'content': 'Smith selectable message'},
        {'role': 'user', 'content': 'User selectable message'},
      ];
    await tester.pumpWidget(harness(c));
    await tester.pumpAndSettle();
    Rect bubbleRect(String text) => tester.getRect(
      find
          .ancestor(of: find.text(text), matching: find.byType(Container))
          .first,
    );
    final smith = bubbleRect('Smith selectable message');
    final user = bubbleRect('User selectable message');
    expect(smith.left, 264);
    expect(user.right, 1176);
    expect(smith.width, lessThanOrEqualTo(720));
    expect(user.width, lessThanOrEqualTo(720));
    final smithRole = tester.getRect(find.text('SMITH'));
    final userRole = tester.getRect(find.text('YOU'));
    expect(smithRole.bottom, smith.top - 8);
    expect(userRole.bottom, user.top - 8);
    expect(userRole.right, user.right);
    final smithRow = tester.getRect(
      find.ancestor(of: find.text('SMITH'), matching: find.byType(Row)).first,
    );
    expect(smithRow.left, smith.left);
    expect(
      tester.widget<Text>(find.text('SMITH')).style!.color,
      SmithColors.accent,
    );
    expect(
      tester.widget<Text>(find.text('YOU')).style!.color,
      SmithColors.textPrimary,
    );
    final chevron = tester
        .widgetList<SmithIcon>(find.byType(SmithIcon))
        .where((icon) => icon.name == 'chevron-right')
        .single;
    expect(chevron.size, 20);
    expect(chevron.color, SmithColors.accent);
    expect(find.text('›'), findsNothing);
    final divider = tester.getRect(find.byType(Divider));
    expect(divider.height, 1);
    expect(
      divider.bottom,
      lessThan(tester.getRect(find.byKey(const ValueKey('chat-composer'))).top),
    );
    final history = tester.getRect(
      find.byKey(const ValueKey('conversation-history')),
    );
    expect(history.width, 1440);
    expect(history.center.dx, 720);
    expect(tester.getRect(find.byTooltip('Send')).bottom, 892);
    await tester.longPress(find.text('Smith selectable message'));
    await tester.pumpAndSettle();
    final selected = tester
        .widgetList<EditableText>(find.byType(EditableText))
        .where((e) => e.controller.text == 'Smith selectable message')
        .single;
    expect(selected.controller.selection.isCollapsed, isFalse);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
    c.dispose();
  });

  testWidgets('real controller keeps a newer draft through terminal recovery', (
    tester,
  ) async {
    final api = LegacyFakeAppApi();
    final c = ChatController(api);
    await c.synchronize();
    await c.refreshModels();
    api.unknownAdmission = true;
    await tester.pumpWidget(harness(c));
    await tester.enterText(find.byType(TextField), 'submitted draft');
    await tester.pump();
    await tester.tap(find.byTooltip('Send'));
    await tester.pumpAndSettle();
    expect(c.pendingId, isNotNull);
    await tester.enterText(find.byType(TextField), 'newer unsent draft');
    api.stateFailureAfterPoll = true;
    await tester.ensureVisible(find.text('Retry'));
    await tester.tap(find.text('Retry'));
    await tester.pumpAndSettle();
    expect(c.draft, 'newer unsent draft');
    expect(
      tester.widget<TextField>(find.byType(TextField)).controller!.text,
      'newer unsent draft',
    );
    expect(c.provisionalAnswers, isNotEmpty);
    expect(api.calls.where((op) => op == 'submit'), hasLength(1));
    expect(find.text('useful answer'), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
    c.dispose();
  });

  testWidgets('expired confirmation cannot resolve a replaced controller', (
    tester,
  ) async {
    Future<ChatController> expiredController() async {
      final api = LegacyFakeAppApi();
      final c = ChatController(api);
      await c.synchronize();
      await c.refreshModels();
      api.unknownAdmission = true;
      c.edit('original draft');
      await c.send();
      api.expiredPoll = true;
      await c.synchronize();
      return c;
    }

    final a = await expiredController();
    final b = await expiredController();
    final aId = a.pendingId;
    final bId = b.pendingId;
    await tester.pumpWidget(harness(a));
    await tester.ensureVisible(find.text('Resolve expired request'));
    await tester.tap(find.text('Resolve expired request'));
    await tester.pumpAndSettle();
    await tester.pumpWidget(harness(b));
    await tester.tap(find.text('Forget request'));
    await tester.pumpAndSettle();
    expect(a.pendingId, aId);
    expect(b.pendingId, bId);
    await tester.ensureVisible(find.text('Resolve expired request'));
    await tester.tap(find.text('Resolve expired request'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Forget request'));
    await tester.pumpAndSettle();
    expect(b.pendingId, isNull);
    expect(b.draft, 'original draft');
    expect(b.history, hasLength(2));
    expect(
      (b.api as LegacyFakeAppApi).calls.where((op) => op == 'submit'),
      hasLength(1),
    );
    await tester.pumpWidget(const SizedBox());
    a.dispose();
    b.dispose();
  });

  testWidgets(
    'provisional answers stay distinct without duplicating saved content',
    (tester) async {
      final c = PresentationController()
        ..history = [
          {'role': 'assistant', 'content': 'saved answer'},
        ];
      c.provisionalAnswers.addAll({
        'one': 'saved answer',
        'two': 'unsynced answer',
      });
      await tester.pumpWidget(harness(c));
      expect(find.text('saved answer'), findsOneWidget);
      expect(find.text('unsynced answer'), findsOneWidget);
      expect(
        find.text('Verified answer · awaiting history sync'),
        findsNWidgets(2),
      );
      expect(find.byKey(const ValueKey('terminal-answer-two')), findsOneWidget);
      await tester.pumpWidget(const SizedBox());
      c.dispose();
    },
  );
}
