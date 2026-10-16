import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/ui/chat_sidebar.dart';
import 'package:smith_client/ui/smith_theme.dart';

class _AuthorityFixture extends StatefulWidget {
  const _AuthorityFixture({super.key});
  @override
  State<_AuthorityFixture> createState() => _AuthorityFixtureState();
}

class _AuthorityFixtureState extends State<_AuthorityFixture> {
  final calls = <String>[];
  String id = 'original-id';
  String title = 'Same title';
  bool archived = false;
  bool busy = false;
  bool present = true;
  bool rename = true;
  bool delete = true;
  bool archive = true;
  bool reactivate = true;
  bool replacement = false;

  Future<void> _rename(String id, String title) async =>
      calls.add('rename:$id:$title');
  Future<void> _newRename(String id, String title) async =>
      calls.add('new-rename:$id:$title');
  Future<void> _delete(String id) async => calls.add('delete:$id');
  Future<void> _newDelete(String id) async => calls.add('new-delete:$id');
  Future<void> _archive(String id) async => calls.add('archive:$id');
  Future<void> _reactivate(String id) async => calls.add('reactivate:$id');

  void change(String reason, String action) => setState(() {
    switch (reason) {
      case 'busy':
        busy = true;
      case 'revoked':
        switch (action) {
          case 'Rename':
            rename = false;
          case 'Delete':
            delete = false;
          case 'Archive':
            archive = false;
          case 'Reactivate':
            reactivate = false;
        }
      case 'identity':
        id = 'replacement-id';
      case 'archived':
        archived = !archived;
      case 'removed':
        present = false;
      case 'controller':
        replacement = true;
      case 'retitled':
        title = 'New display title';
      case 'restored':
        rename = true;
    }
  });

  @override
  Widget build(BuildContext context) => MaterialApp(
    theme: SmithTheme.dark(),
    home: Scaffold(
      body: Center(
        child: present
            ? ChatActionsMenu(
                // Deliberately reuse the widget slot to test identity changes without disposal.
                key: const ValueKey('authority-menu'),
                chat: ChatSummary(id: id, title: title, archived: archived),
                busy: busy,
                onRename: rename ? (replacement ? _newRename : _rename) : null,
                onDelete: delete ? (replacement ? _newDelete : _delete) : null,
                onArchive: archive ? _archive : null,
                onReactivate: reactivate ? _reactivate : null,
              )
            : const SizedBox.shrink(),
      ),
    ),
  );
}

Future<_AuthorityFixtureState> _open(WidgetTester tester, String action) async {
  await tester.binding.setSurfaceSize(const Size(1280, 720));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  final key = GlobalKey<_AuthorityFixtureState>();
  await tester.pumpWidget(_AuthorityFixture(key: key));
  final state = key.currentState!;
  if (action == 'Reactivate') {
    state.change('archived', action);
    await tester.pump();
  }
  await tester.tap(find.byTooltip('Actions for Same title'));
  await tester.pumpAndSettle();
  return state;
}

void main() {
  for (final action in ['Rename', 'Delete', 'Archive', 'Reactivate']) {
    final reasons = action == 'Rename'
        ? ['busy', 'revoked', 'identity', 'archived', 'removed', 'controller']
        : ['busy', 'revoked'];
    for (final reason in reasons) {
      testWidgets('shell authority open $action menu rejects $reason', (
        tester,
      ) async {
        final state = await _open(tester, action);
        state.change(reason, action);
        await tester.pumpAndSettle();
        await tester.tap(find.text(action));
        await tester.pumpAndSettle();
        expect(state.calls, isEmpty);
        expect(find.text('Action unavailable'), findsOneWidget);
        expect(find.textContaining('No change was sent'), findsOneWidget);
        expect(tester.takeException(), isNull);
        await tester.tap(find.widgetWithText(TextButton, 'Close'));
        await tester.pumpAndSettle();
        expect(state.calls, isEmpty);
      });
    }
  }

  for (final action in ['Rename', 'Delete']) {
    for (final reason in [
      'busy',
      'revoked',
      'identity',
      'archived',
      'removed',
      'controller',
    ]) {
      testWidgets(
        'shell authority open $action dialog rejects $reason at confirmation',
        (tester) async {
          final state = await _open(tester, action);
          await tester.tap(find.text(action));
          await tester.pumpAndSettle();
          if (action == 'Rename') {
            await tester.enterText(find.byType(TextField), 'Edited title');
          }
          state.change(reason, action);
          await tester.pumpAndSettle();
          await tester.tap(find.widgetWithText(TextButton, action));
          await tester.pumpAndSettle();
          expect(state.calls, isEmpty);
          expect(
            find.textContaining('This action is no longer available.'),
            findsOneWidget,
          );
          expect(find.textContaining('No change was sent'), findsOneWidget);
          expect(find.widgetWithText(TextButton, action), findsNothing);
          expect(tester.takeException(), isNull);
          await tester.tap(find.widgetWithText(TextButton, 'Close'));
          await tester.pumpAndSettle();
          expect(state.calls, isEmpty);
        },
      );
    }

    testWidgets(
      'shell authority $action permits title-only updates and binds original ID',
      (tester) async {
        final state = await _open(tester, action);
        await tester.tap(find.text(action));
        await tester.pumpAndSettle();
        if (action == 'Rename') {
          await tester.enterText(find.byType(TextField), 'Edited title');
        }
        state.change('retitled', action);
        await tester.pumpAndSettle();
        await tester.tap(find.widgetWithText(TextButton, action));
        await tester.pumpAndSettle();
        expect(state.calls, [
          action == 'Rename'
              ? 'rename:original-id:Edited title'
              : 'delete:original-id',
        ]);
        expect(tester.takeException(), isNull);
      },
    );
  }

  testWidgets(
    'shell authority restored capability does not revive old confirmation',
    (tester) async {
      final state = await _open(tester, 'Rename');
      await tester.tap(find.text('Rename'));
      await tester.pumpAndSettle();
      state.change('revoked', 'Rename');
      await tester.pumpAndSettle();
      state.change('restored', 'Rename');
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(TextButton, 'Rename'));
      await tester.pumpAndSettle();
      expect(state.calls, isEmpty);
      expect(
        find.textContaining('This action is no longer available.'),
        findsOneWidget,
      );
      expect(tester.takeException(), isNull);
    },
  );
}
