import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/chat_panel.dart';
import 'package:smith_client/main.dart';
import 'package:smith_client/workspace_controller.dart';

import 'app_fakes.dart';
import 'widget_test.dart' show FakeControlApi, profile;
import 'workspace_controller_test.dart' show addOther, other;

Future<WorkspaceController> mount(WidgetTester tester, FakeAppApi api) async {
  tester.view.physicalSize = const Size(1280, 720);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  api.inventoryObserved = true;
  await tester.pumpWidget(
    SmithApp(controlApi: FakeControlApi(profile()), appApi: api),
  );
  await tester.pumpAndSettle();
  return tester.widget<ConversationScreen>(find.byType(ConversationScreen)).chat
      as WorkspaceController;
}

Future<void> action(WidgetTester tester, String title, String action) async {
  await tester.tap(find.byTooltip('Actions for $title'));
  await tester.pumpAndSettle();
  await tester.tap(find.text(action).last);
  await tester.pumpAndSettle();
}

void main() {
  testWidgets(
    'production Boot during A mounts immediately and B remains usable',
    (tester) async {
      final api = FakeAppApi()..holdChats = true;
      addOther(api);
      api.jobs[testInstance] = {
        'request_id': testInstance,
        'conversation_id': testConversation,
        'kind': 'submit',
        'status': 'running',
        'phase': 'generation',
        'config_revision': 1,
        'result': null,
        'error': null,
      };
      final w = await mount(tester, api);
      expect(find.byType(ChatPanel), findsOneWidget);
      expect(find.textContaining('generation'), findsOneWidget);
      await tester.enterText(find.byType(TextField), 'B');
      expect(w.selected.id, other);
      expect(w.selected.canSend, true);
      await tester.pump();
      await tester.tap(find.byTooltip('Send'));
      await tester.pumpAndSettle();
      expect(api.calls.where((s) => s == 'submit'), hasLength(1));
      expect(api.calls, isNot(contains('models')));
      await tester.pumpWidget(const SizedBox());
      expect(api.disposedCount, 1);
    },
  );
  testWidgets(
    'draft and reading position survive switch, blank reuse and Settings',
    (tester) async {
      final api = FakeAppApi();
      for (var i = 0; i < 32; i++) {
        api.history.add({
          'role': i.isEven ? 'user' : 'assistant',
          'content': 'Message $i\nlong retained history\nreading position',
        });
      }
      final w = await mount(tester, api);
      final panelState = tester.state(find.byType(ChatPanel));
      final scroll = tester
          .widget<SingleChildScrollView>(
            find.byKey(const ValueKey('conversation-history')),
          )
          .controller!;
      scroll.jumpTo(100);
      await tester.pump();
      await tester.enterText(find.byType(TextField), 'A draft');
      await tester.tap(find.byTooltip('New chat'));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'blank draft');
      final blank = w.selected;
      await tester.tap(find.byTooltip('Settings'));
      await tester.pumpAndSettle();
      await tester.tap(find.byTooltip('Return to Chat'));
      await tester.pumpAndSettle();
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text,
        'blank draft',
      );
      await tester.tap(find.text('Current chat').first);
      await tester.pumpAndSettle();
      expect(scroll.offset, 100);
      expect(w.selected.draft, 'A draft');
      expect(tester.state(find.byType(ChatPanel)), same(panelState));
      await tester.tap(find.byTooltip('New chat'));
      await tester.pumpAndSettle();
      expect(w.selected, same(blank));
      expect(w.selected.draft, 'blank draft');
      await tester.pumpWidget(const SizedBox());
    },
  );
  for (final name in ['Rename', 'Delete']) {
    testWidgets(
      '$name dialog captures revision at opening and rejects later metadata',
      (tester) async {
        final api = FakeAppApi();
        final w = await mount(tester, api);
        await action(tester, 'Current chat', name);
        api.summaries[testConversation]!['metadata_revision'] = 1;
        await w.synchronize();
        await tester.pumpAndSettle();
        if (name == 'Rename') {
          await tester.enterText(find.byType(TextField).last, 'changed title');
        }
        await tester.tap(find.widgetWithText(TextButton, name));
        await tester.pumpAndSettle();
        expect(
          api.calls.where((s) => s == 'conversation_${name.toLowerCase()}'),
          isEmpty,
        );
        expect(
          find.text('This action is no longer available. No change was sent.'),
          findsOneWidget,
        );
        expect(find.widgetWithText(TextButton, name), findsNothing);
        expect(api.summaries[testConversation]!['title'], 'Current chat');
        expect(api.summaries, contains(testConversation));
        await tester.pumpWidget(const SizedBox());
      },
    );
  }
  testWidgets(
    'Archive is immediate; restore does not select; Delete requires explicit confirmation',
    (tester) async {
      final api = FakeAppApi();
      final w = await mount(tester, api);
      w.newChat();
      w.selected.edit('keep blank');
      await w.select(testConversation);
      await tester.pumpAndSettle();
      await action(tester, 'Current chat', 'Archive');
      expect(api.calls.where((s) => s == 'conversation_archive'), hasLength(1));
      expect(w.selected, same(w.blank));
      expect(w.blank.draft, 'keep blank');
      await tester.tap(find.byTooltip('Archived chats'));
      await tester.pumpAndSettle();
      await action(tester, 'Current chat', 'Reactivate');
      expect(w.selected, same(w.blank));
      await tester.tap(find.byTooltip('Close archived chats'));
      await tester.pumpAndSettle();
      await action(tester, 'Current chat', 'Delete');
      expect(api.calls.where((s) => s == 'conversation_delete'), isEmpty);
      await tester.tap(find.widgetWithText(TextButton, 'Delete'));
      await tester.pumpAndSettle();
      expect(api.calls.where((s) => s == 'conversation_delete'), hasLength(1));
      expect(w.conversations, isEmpty);
      expect(w.blank.draft, 'keep blank');
      await tester.pumpWidget(const SizedBox());
    },
  );
  testWidgets(
    'multi-chat small 400x400 2x Settings and drawer preserve draft without overflow',
    (tester) async {
      final api = FakeAppApi();
      final w = await mount(tester, api);
      tester.view.physicalSize = const Size(400, 400);
      tester.platformDispatcher.textScaleFactorTestValue = 2;
      addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'small draft');
      await tester.tap(find.byTooltip('Settings'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      await tester.ensureVisible(find.byTooltip('Return to Chat'));
      await tester.pumpAndSettle();
      expect(find.byTooltip('Return to Chat').hitTestable(), findsOneWidget);
      await tester.tap(find.byTooltip('Return to Chat'));
      await tester.pumpAndSettle();
      await tester.tap(find.byTooltip('Open chat history'));
      await tester.pumpAndSettle();
      expect(find.byTooltip('New chat').hitTestable(), findsOneWidget);
      await tester.tap(find.byTooltip('Close chat history'));
      await tester.pumpAndSettle();
      expect(w.selected.draft, 'small draft');
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );
}
