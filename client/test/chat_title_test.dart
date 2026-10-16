import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/app_v2.dart';
import 'package:smith_client/chat_title.dart';
import 'package:smith_client/ui/archived_chats_dialog.dart';
import 'package:smith_client/ui/chat_sidebar.dart';
import 'package:smith_client/ui/smith_workspace.dart';

void main() {
  final forty = List.filled(40, '😀').join();
  final legacy = '${forty}legacy';
  final id = List.filled(32, 'a').join();

  test('display truncates scalar values, never authoritative titles', () {
    expect(displayChatTitle(legacy), forty);
    expect(displayChatTitle(''), 'Untitled chat');
    expect(displayChatTitle('e\u0301' * 21).runes.length, 40);
    final chat = ChatSummary(id: id, title: legacy);
    expect(chat.displayTitle, forty);
    expect(chat.title, legacy);
  });

  test('old wire titles accepted; new renames limited to 40 runes', () {
    final metadata = <String, dynamic>{
      'conversation_id': id,
      'title': legacy,
      'archived': false,
      'metadata_revision': 0,
      'history_revision': 0,
      'sequence': 0,
      'origin_request_id': null,
    };
    conversation(metadata);
    expect(metadata['title'], legacy);
    final args = <String, dynamic>{
      'request_id': id,
      'conversation_id': id,
      'expected_metadata_revision': 0,
      'title': forty,
    };
    validateV2Arguments('conversation_rename', args);
    args['title'] = legacy;
    expect(
      () => validateV2Arguments('conversation_rename', args),
      throwsA(isA<AppFailure>()),
    );
  });

  Map<String, dynamic> historicalTitle(String title) => {
    'conversation_id': id,
    'title': title,
    'archived': false,
    'metadata_revision': 0,
    'history_revision': 0,
    'sequence': 0,
    'origin_request_id': null,
  };

  test('historical FEFF titles survive details and lifecycle receipts', () {
    for (final title in [
      '\ufeff',
      '\ufeff$legacy',
      '$legacy\ufeff',
      '\ufeff$legacy\ufeff',
    ]) {
      final metadata = historicalTitle(title);
      validateV2('conversation_get', {'conversation': metadata, 'history': []});
      for (final kind in [
        'conversation_rename',
        'conversation_archive',
        'conversation_restore',
      ]) {
        final receipt = {'request_id': id, 'conversation': metadata};
        validateV2(kind, receipt);
        validateV2('poll', {
          'request_id': id,
          'conversation_id': id,
          'kind': kind,
          'status': 'succeeded',
          'phase': 'complete',
          'config_revision': 0,
          'result': receipt,
          'error': null,
        });
        expect((receipt['conversation'] as Map)['title'], title);
      }
      expect(metadata['title'], title);
    }
  });

  test('incoming title boundaries match Rust Unicode White_Space', () {
    final whitespace = [
      for (var rune = 0x09; rune <= 0x0d; rune++) rune,
      0x20,
      0x85,
      0xa0,
      0x1680,
      for (var rune = 0x2000; rune <= 0x200a; rune++) rune,
      0x2028,
      0x2029,
      0x202f,
      0x205f,
      0x3000,
    ];
    for (final rune in whitespace) {
      final space = String.fromCharCode(rune);
      for (final title in [space, '${space}title', 'title$space']) {
        expect(
          () => conversation(historicalTitle(title)),
          throwsA(isA<AppFailure>()),
          reason: 'Rust whitespace U+${rune.toRadixString(16)}',
        );
      }
    }
    for (final rune in [0x180e, 0x200b, 0x2060, 0xfeff]) {
      final nonWhitespace = String.fromCharCode(rune);
      conversation(historicalTitle('${nonWhitespace}title$nonWhitespace'));
    }
    // Non-control interior whitespace is allowed; edge trimming is not global removal.
    conversation(historicalTitle('a\u00a0b\u1680c\u2000d\u3000e'));
    for (final title in ['', 'a\u0085b', 'a\u2028b', 'x' * 257]) {
      expect(
        () => conversation(historicalTitle(title)),
        throwsA(isA<AppFailure>()),
      );
    }
    conversation(historicalTitle('\ufeff${'x' * 253}'));
    expect(
      () => conversation(historicalTitle('\ufeff${'x' * 254}')),
      throwsA(isA<AppFailure>()),
    );
  });

  for (final width in [600.0, 1280.0]) {
    testWidgets('Settings hides chats and preserves draft at $width', (
      tester,
    ) async {
      await tester.binding.setSurfaceSize(Size(width, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(
        MaterialApp(
          home: SmithWorkspace(
            chats: [ChatSummary(id: id, title: legacy)],
            title: legacy,
            version: 'test',
            connectionStatus: 'Offline',
            modelStatus: 'Unknown',
            onNewChat: () {},
            onArchiveChat: (_) async {},
            child: const TextField(),
          ),
        ),
      );
      expect(
        find.byTooltip(width < 960 ? 'Choose chat: $forty' : 'Chat / $forty'),
        findsOneWidget,
      );
      await tester.enterText(find.byType(TextField), 'keep draft');
      final state = tester.state(find.byType(TextField));
      await tester.tap(find.byTooltip('Settings'));
      await tester.pumpAndSettle();
      expect(find.byType(ChatSidebar), findsNothing);
      expect(find.byTooltip('New chat'), findsNothing);
      expect(find.byTooltip('Archived chats'), findsNothing);
      expect(find.byTooltip('Open chat history'), findsNothing);
      expect(find.byType(TextField), findsNothing);
      await tester.tap(find.byTooltip('Return to Chat'));
      await tester.pumpAndSettle();
      expect(tester.state(find.byType(TextField)), same(state));
      expect(find.text('keep draft'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets('archive, menus, confirmation bounded; editor retains raw text', (
    tester,
  ) async {
    String? renamed;
    final chat = ChatSummary(id: id, title: legacy, archived: true);
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: ArchivedChatsDialog(
            chats: [chat],
            onRename: (_, title) async {
              renamed = title;
            },
            onDelete: (_) async {},
          ),
        ),
      ),
    );
    expect(find.text(forty), findsOneWidget);
    expect(find.byTooltip('Actions for $forty'), findsOneWidget);
    expect(find.text(legacy), findsNothing);
    await tester.tap(find.byTooltip('Actions for $forty'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Delete'));
    await tester.pumpAndSettle();
    expect(find.text(forty), findsNWidgets(2));
    expect(find.text(legacy), findsNothing);
    expect(
      find.text(
        'Deletes this chat, history and results. You can’t undo this. Backups are kept.',
      ),
      findsOneWidget,
    );
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    await tester.tap(find.byTooltip('Actions for $forty'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Rename'));
    await tester.pumpAndSettle();
    expect(
      tester.widget<TextField>(find.byType(TextField)).controller!.text,
      legacy,
    );
    expect(find.text('Use 40 characters or fewer.'), findsOneWidget);
    expect(
      tester
          .widget<TextButton>(find.widgetWithText(TextButton, 'Rename'))
          .onPressed,
      isNull,
    );
    expect(renamed, isNull);
    await tester.enterText(find.byType(TextField), forty);
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(TextButton, 'Rename'));
    await tester.pumpAndSettle();
    expect(renamed, forty);
    expect(tester.takeException(), isNull);
  });
}
