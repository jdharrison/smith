import 'dart:async';
import 'dart:ui' show Tristate;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/ui/archived_chats_dialog.dart';
import 'package:smith_client/ui/chat_sidebar.dart' show ChatSidebar;
import 'package:smith_client/ui/smith_theme.dart';
import 'package:smith_client/ui/smith_icon.dart';
import 'package:smith_client/ui/smith_workspace.dart';

const _chats = [
  ChatSummary(id: 'a', title: 'Same title'),
  ChatSummary(id: 'b', title: 'Same title'),
  ChatSummary(id: 'old', title: 'Archived fixture', archived: true),
];

Widget _app({
  double scale = 1,
  Widget child = const SizedBox.expand(),
  List<ChatSummary> chats = _chats,
  ValueChanged<String>? select,
  VoidCallback? create,
  VoidCallback? retry,
  RenameChatAction? rename,
  ChatAction? archive,
  ChatAction? reactivate,
  ChatAction? delete,
  String? error,
  bool archivesLoading = false,
  String? archivesError,
}) => MaterialApp(
  theme: SmithTheme.dark(),
  builder: (context, child) => MediaQuery(
    data: MediaQuery.of(context).copyWith(textScaler: TextScaler.linear(scale)),
    child: child!,
  ),
  home: SmithWorkspace(
    chats: chats,
    title: 'Fixture conversation',
    version: 'test-version',
    connectionStatus: 'CONNECTION UNKNOWN',
    modelStatus: 'No observed model',
    selectedChatId: 'b',
    onSelectChat: select,
    onNewChat: create,
    onRetry: retry,
    onRenameChat: rename,
    onArchiveChat: archive,
    onReactivateChat: reactivate,
    onDeleteChat: delete,
    error: error,
    archivesLoading: archivesLoading,
    archivesError: archivesError,
    child: child,
  ),
);

Future<void> _size(WidgetTester tester, Size size) async {
  await tester.binding.setSurfaceSize(size);
  addTearDown(() => tester.binding.setSurfaceSize(null));
}

class _DraftProbe extends StatefulWidget {
  const _DraftProbe();
  @override
  State<_DraftProbe> createState() => _DraftProbeState();
}

class _DraftProbeState extends State<_DraftProbe> {
  final draft = TextEditingController();
  @override
  void dispose() {
    draft.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => ListView(
    children: [
      TextField(key: const ValueKey('fixture-draft'), controller: draft),
      for (var i = 0; i < 30; i++) Text('Fixture message $i'),
    ],
  );
}

void main() {
  testWidgets('HUD keeps branding and status; version is only in Settings', (
    tester,
  ) async {
    await _size(tester, const Size(1280, 720));
    await tester.pumpWidget(_app(retry: () {}));
    await tester.pumpAndSettle();
    expect(find.text('> SMITH'), findsOneWidget);
    expect(find.text('CONNECTION UNKNOWN'), findsOneWidget);
    expect(find.text('test-version'), findsNothing);
    expect(find.byTooltip('Retry observation'), findsNothing);

    await tester.tap(find.byTooltip('Settings'));
    await tester.pumpAndSettle();
    expect(find.text('test-version'), findsOneWidget);
    expect(find.byTooltip('Retry observation'), findsNothing);

    await tester.tap(find.byTooltip('Return to Chat'));
    await tester.pumpAndSettle();
    expect(find.text('test-version'), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'shell-only status observation semantics and distinct selected focus frame',
    (tester) async {
      await _size(tester, const Size(400, 400));
      final semantics = tester.ensureSemantics();
      await tester.pumpWidget(_app());
      await tester.pumpAndSettle();
      final status = tester
          .getSemantics(find.text('CONNECTION UNKNOWN'))
          .getSemanticsData();
      expect(status.label, 'CONNECTION UNKNOWN');
      expect(status.hint, contains('not continuous monitoring'));
      expect(
        find.byTooltip(
          'Last observed service connection, not continuous monitoring. Model readiness and work completion are separate.',
        ),
        findsNothing,
      );
      final icon = find.byWidgetPredicate(
        (widget) => widget is SmithIcon && widget.name == 'message-square',
      );
      final chat = tester.getSemantics(icon).getSemanticsData();
      expect(chat.label, 'Chat');
      expect(chat.hint, 'Open chat history');
      expect(chat.flagsCollection.isSelected, Tristate.isTrue);
      final button = tester.widget<IconButton>(
        find.ancestor(of: icon, matching: find.byType(IconButton)).first,
      );
      final selectedBorder = button.style!.side!.resolve({
        WidgetState.selected,
      })!;
      final focusedBorder = button.style!.side!.resolve({
        WidgetState.selected,
        WidgetState.focused,
      })!;
      expect(selectedBorder.color, SmithColors.stroke);
      expect(focusedBorder.color, SmithColors.accentBright);
      expect(focusedBorder.width, greaterThan(selectedBorder.width));
      semantics.dispose();
    },
  );
  testWidgets(
    'shell-only inline recovery and retry stay reachable at 400 text two',
    (tester) async {
      await _size(tester, const Size(400, 400));
      var calls = 0;
      await tester.pumpWidget(
        _app(
          scale: 2,
          child: const _DraftProbe(),
          error: 'Service observation unavailable. Repair externally, then observe again.',
          retry: () => calls++,
        ),
      );
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      await tester.ensureVisible(
        find.widgetWithText(TextButton, 'Retry observation'),
      );
      await tester.tap(find.widgetWithText(TextButton, 'Retry observation'));
      expect(calls, 1);
      expect(
        find.textContaining('Service observation unavailable'),
        findsOneWidget,
      );
      await tester.tap(find.byTooltip('Settings'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(
        find.textContaining('Service observation unavailable'),
        findsOneWidget,
      );
    },
  );
  testWidgets(
    'shell-only keyboard focus selection and archive close restoration',
    (tester) async {
      await _size(tester, const Size(1280, 720));
      final semantics = tester.ensureSemantics();

      final selected = <String>[];
      await tester.pumpWidget(_app(select: selected.add));
      await tester.pumpAndSettle();
      final row = find.descendant(
        of: find.byKey(const ValueKey('b')),
        matching: find.byType(TextButton),
      );
      final node = tester.getSemantics(row);
      expect(
        node.getSemanticsData().flagsCollection.isSelected,
        Tristate.isTrue,
      );
      for (var i = 0; i < 4; i++) {
        await tester.sendKeyEvent(LogicalKeyboardKey.tab);
        await tester.pump();
      }
      expect(selected, isEmpty);
      final icon = find.descendant(
        of: find.byTooltip('Archived chats'),
        matching: find.byType(SmithIcon),
      );
      // Focus the actual button's descendant, not its enclosing navigation scope.
      final focus = Focus.of(tester.element(icon.first));
      focus.requestFocus();
      await tester.pump();
      final previous = FocusManager.instance.primaryFocus;
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(find.byType(ArchivedChatsDialog), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(find.byType(ArchivedChatsDialog), findsNothing);
      expect(FocusManager.instance.primaryFocus, same(previous));
      semantics.dispose();
      expect(
        tester.getSize(find.byTooltip('Settings')).width,
        greaterThanOrEqualTo(48),
      );
    },
  );

  testWidgets(
    'shell-only open archive consumes parent updates and preserves child on errors',
    (tester) async {
      await _size(tester, const Size(1280, 720));
      await tester.pumpWidget(_app(child: const _DraftProbe()));
      await tester.pumpAndSettle();
      final state = tester.state(find.byType(_DraftProbe));
      await tester.tap(find.byTooltip('Archived chats'));
      await tester.pumpAndSettle();
      await tester.pumpWidget(
        _app(
          child: const _DraftProbe(),
          error: 'New observation failure',
          chats: const [
            ChatSummary(
              id: 'new-archive',
              title: 'Updated archive',
              archived: true,
            ),
          ],
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('Updated archive'), findsOneWidget);
      expect(find.text('Archived fixture'), findsNothing);
      expect(tester.takeException(), isNull);
      await tester.tap(find.byTooltip('Close archived chats'));
      await tester.pumpAndSettle();
      expect(tester.state(find.byType(_DraftProbe)), same(state));
    },
  );

  testWidgets(
    'shell-only delete confirmation remains reachable at 400 text two',
    (tester) async {
      await _size(tester, const Size(400, 400));
      final deleted = <String>[];
      await tester.pumpWidget(
        _app(scale: 2, delete: (id) async => deleted.add(id)),
      );
      await tester.tap(find.byTooltip('Open chat history'));
      await tester.pumpAndSettle();
      await tester.tap(find.byTooltip('Actions for Same title').first);
      await tester.pumpAndSettle();
      await tester.tap(find.text('Delete'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      await tester.tap(find.widgetWithText(TextButton, 'Delete'));
      await tester.pumpAndSettle();
      expect(deleted, ['a']);
    },
  );
  testWidgets('shell-only unsupported management controls are absent', (
    tester,
  ) async {
    await _size(tester, const Size(1280, 720));
    await tester.pumpWidget(
      _app(
        chats: const [ChatSummary(id: 'only', title: 'Existing')],
      ),
    );
    await tester.pumpAndSettle();
    expect(find.byTooltip('New chat'), findsNothing);
    expect(find.byTooltip('Archived chats'), findsNothing);
    expect(find.byTooltip('Actions for Existing'), findsNothing);
    expect(find.text('CONNECTION UNKNOWN'), findsOneWidget);
    expect(
      find.ancestor(
        of: find.text('CONNECTION UNKNOWN'),
        matching: find.byType(Tooltip),
      ),
      findsNothing,
    );
    expect(find.text('SYSTEM ONLINE'), findsNothing);
  });

  for (final size in [
    const Size(400, 400),
    const Size(1280, 720),
    const Size(4200, 2400),
  ]) {
    for (final scale in [1.0, 2.0]) {
      testWidgets(
        'shell-only geometry $size text $scale preserves child across views',
        (tester) async {
          await _size(tester, size);
          final app = _app(
            scale: scale,
            child: const _DraftProbe(),
            error: 'Observation unavailable',
          );
          await tester.pumpWidget(app);
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
          final state = tester.state<_DraftProbeState>(
            find.byType(_DraftProbe),
          );
          state.draft.text = 'Retained draft';
          await tester.tap(find.byTooltip('Settings'));
          await tester.pumpAndSettle();
          expect(find.text('Connection & About'), findsOneWidget);
          expect(tester.takeException(), isNull);
          await tester.tap(find.byTooltip('Return to Chat'));
          await tester.pumpAndSettle();
          expect(tester.state(find.byType(_DraftProbe)), same(state));
          expect(state.draft.text, 'Retained draft');
          if (find.byTooltip('Open chat history').evaluate().isNotEmpty) {
            await tester.tap(find.byTooltip('Open chat history'));
            await tester.pumpAndSettle();
            expect(find.text('Chats'), findsOneWidget);
            expect(tester.takeException(), isNull);
            await tester.tap(find.byTooltip('Close chat history'));
            await tester.pumpAndSettle();
            expect(tester.state(find.byType(_DraftProbe)), same(state));
          }
          await tester.binding.setSurfaceSize(const Size(1440, 900));
          await tester.pumpAndSettle();
          expect(tester.state(find.byType(_DraftProbe)), same(state));
          expect(tester.takeException(), isNull);
        },
      );
    }
  }

  testWidgets(
    'shell-only duplicate titles select stable IDs and supported actions',
    (tester) async {
      await _size(tester, const Size(1440, 900));
      final selected = <String>[];
      final mutations = <String>[];
      var created = 0;
      await tester.pumpWidget(
        _app(
          select: selected.add,
          create: () => created++,
          rename: (id, title) async => mutations.add('$id:$title'),
          archive: (id) async => mutations.add('archive:$id'),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('Same title').last);
      expect(selected, ['b']);
      await tester.tap(find.byTooltip('New chat'));
      expect(created, 1);
      await tester.tap(find.byTooltip('Actions for Same title').last);
      await tester.pumpAndSettle();
      expect(find.text('Delete'), findsNothing);
      await tester.tap(find.text('Rename'));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'Renamed');
      await tester.tap(find.widgetWithText(TextButton, 'Rename'));
      await tester.pumpAndSettle();
      expect(mutations, ['b:Renamed']);
      await tester.tap(find.byTooltip('Actions for Same title').last);
      await tester.pumpAndSettle();
      await tester.tap(find.text('Archive'));
      await tester.pumpAndSettle();
      expect(mutations.last, 'archive:b');
    },
  );

  testWidgets(
    'shell-only archive full collection and delete defaults to cancel',
    (tester) async {
      await _size(tester, const Size(1280, 720));
      final calls = <String>[];
      await tester.pumpWidget(
        _app(
          child: const _DraftProbe(),
          delete: (id) async => calls.add('delete:$id'),
          reactivate: (id) async => calls.add('reactivate:$id'),
        ),
      );
      await tester.pumpAndSettle();
      final state = tester.state(find.byType(_DraftProbe));
      await tester.tap(find.byTooltip('Archived chats'));
      await tester.pumpAndSettle();
      expect(find.text('Archived fixture'), findsOneWidget);
      await tester.tap(find.byTooltip('Actions for Archived fixture'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Delete'));
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(calls, isEmpty);
      expect(find.text('Delete chat'), findsNothing);
      await tester.tap(find.byTooltip('Actions for Archived fixture'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Reactivate'));
      await tester.pumpAndSettle();
      expect(calls, ['reactivate:old']);
      await tester.tap(find.byTooltip('Close archived chats'));
      await tester.pumpAndSettle();
      expect(tester.state(find.byType(_DraftProbe)), same(state));
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('shell-only mutation failure is visible without replay', (
    tester,
  ) async {
    await _size(tester, const Size(400, 400));
    var calls = 0;
    await tester.pumpWidget(
      _app(
        scale: 2,
        rename: (id, title) async {
          calls++;
          throw StateError('fixture failure');
        },
      ),
    );
    await tester.tap(find.byTooltip('Open chat history'));
    await tester.pumpAndSettle();
    await tester.tap(find.byTooltip('Actions for Same title').first);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Rename'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'Changed');
    await tester.tap(find.widgetWithText(TextButton, 'Rename'));
    await tester.pumpAndSettle();
    expect(calls, 1);
    expect(find.textContaining('Change not confirmed'), findsOneWidget);
    expect(tester.takeException(), isNull);
    await tester.tap(find.widgetWithText(TextButton, 'Close'));
    await tester.pumpAndSettle();
    expect(calls, 1);
  });

  testWidgets(
    'shell-only archive loading errors and long collection fit small modal',
    (tester) async {
      await _size(tester, const Size(400, 400));
      await tester.pumpWidget(
        _app(
          scale: 2,
          archivesLoading: true,
          archivesError: 'Collection unavailable',
          chats: List.generate(
            50,
            (i) =>
                ChatSummary(id: 'old$i', title: 'Archived $i', archived: true),
          ),
        ),
      );
      await tester.tap(find.byTooltip('Open chat history'));
      await tester.pumpAndSettle();
      await tester.tap(
        find.descendant(
          of: find.byType(ChatSidebar),
          matching: find.byTooltip('Archived chats'),
        ),
      );
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 300));
      expect(find.byType(ArchivedChatsDialog), findsOneWidget);
      expect(find.byType(LinearProgressIndicator), findsOneWidget);
      expect(find.text('Collection unavailable'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.tap(find.byTooltip('Close archived chats'));
      await tester.pumpAndSettle();
    },
  );

  testWidgets('shell-only pending mutation cannot be submitted twice', (
    tester,
  ) async {
    await _size(tester, const Size(1280, 720));
    final pending = Completer<void>();
    var calls = 0;
    await tester.pumpWidget(
      _app(
        delete: (id) {
          calls++;
          return pending.future;
        },
      ),
    );
    await tester.tap(find.byTooltip('Actions for Same title').first);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Delete'));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(TextButton, 'Delete'));
    await tester.pump();
    expect(calls, 1);
    expect(
      tester
          .widget<TextButton>(find.widgetWithText(TextButton, 'Delete'))
          .onPressed,
      isNull,
    );
    pending.complete();
    await tester.pumpAndSettle();
    expect(find.text('Delete chat'), findsNothing);
  });
}
