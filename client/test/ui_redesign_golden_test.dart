// Synthetic visual fixtures only. No production records or capabilities.
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/chat_panel.dart';
import 'package:smith_client/main.dart';
import 'package:smith_client/text_chat.dart';
import 'package:smith_client/ui/chat_sidebar.dart';
import 'package:smith_client/ui/smith_app_header.dart';
import 'package:smith_client/ui/settings_view.dart';
import 'package:smith_client/ui/smith_theme.dart';
import 'package:smith_client/ui/smith_workspace.dart';

import 'app_fakes.dart';
import 'widget_test.dart' show FakeControlApi, profile;

class _GoldenController extends ChatController {
  _GoldenController() : super(LegacyFakeAppApi()) {
    stale = false;
    history = [
      {'role': 'user', 'content': 'Let’s refine the workspace interface.'},
      {
        'role': 'assistant',
        'content': 'Keep the conversation at the center. A compact history column and a quiet navigation rail leave room for the work.',
      },
      {
        'role': 'user',
        'content': 'What should remain visible when the window is small?',
      },
      {
        'role': 'assistant',
        'content': 'The current conversation, composer, and observed connection status. Chat history opens from the rail without losing your draft or reading position.',
      },
      {'role': 'user', 'content': 'Use restrained borders and clear labels.'},
      {
        'role': 'assistant',
        'content': 'Thin outlines separate the panels. The selected chat has a teal edge, and actions stay available through the row menu. No extra dashboard or decorative effects.',
      },
    ];
    edit('Review the next detail…');
  }

  int retries = 0;
  int sends = 0;
  void retryObservation() => retries++;

  @override
  bool get canSend => !busy && !stale && draft.isNotEmpty;
  @override
  Future<void> send() async {
    sends++;
  } // Fixture capability, never inference.
}

class _VisualFixture extends StatefulWidget {
  const _VisualFixture({
    required this.controller,
    required this.scale,
    this.fontFamily = 'JetBrains Mono',
  });
  final _GoldenController controller;
  final double scale;
  final String fontFamily;

  @override
  State<_VisualFixture> createState() => _VisualFixtureState();
}

class _VisualFixtureState extends State<_VisualFixture> {
  var chats = const [
    ChatSummary(id: 'ui', title: 'Workspace interface'),
    ChatSummary(id: 'model', title: 'Local model notes'),
    ChatSummary(id: 'release', title: 'Release checklist'),
    ChatSummary(id: 'keyboard', title: 'Keyboard shortcuts'),
    ChatSummary(id: 'recovery', title: 'Recovery behavior'),
    ChatSummary(id: 'tests', title: 'Widget test plan'),
    ChatSummary(id: 'models', title: 'Model inventory'),
    ChatSummary(id: 'drafts', title: 'Draft preservation'),
    ChatSummary(id: 'fonts', title: 'Typography review'),
    ChatSummary(id: 'linux', title: 'Linux desktop notes'),
    ChatSummary(id: 'history', title: 'Bounded history'),
    ChatSummary(id: 'access', title: 'Accessibility checks'),
    ChatSummary(id: 'archive1', title: 'Earlier layout review', archived: true),
    ChatSummary(id: 'archive2', title: 'Keyboard navigation', archived: true),
    ChatSummary(id: 'archive3', title: 'Connection recovery', archived: true),
  ];
  String? selected = 'ui';

  Future<void> _update(
    String id, {
    String? title,
    bool? archived,
    bool delete = false,
  }) async {
    setState(() {
      chats = [
        for (final chat in chats)
          if (chat.id != id)
            chat
          else if (!delete)
            ChatSummary(
              id: id,
              title: title ?? chat.title,
              archived: archived ?? chat.archived,
            ),
      ];
    });
  }

  @override
  Widget build(BuildContext context) => MaterialApp(
    debugShowCheckedModeBanner: false,
    theme: SmithTheme.dark(fontFamily: widget.fontFamily),
    builder: (context, child) => MediaQuery(
      data: MediaQuery.of(context)
          .copyWith(textScaler: TextScaler.linear(widget.scale)),
      child: child!,
    ),
    home: SmithWorkspace(
      chats: chats,
      selectedChatId: selected,
      title: 'Workspace interface',
      version: 'v0.0.fixture',
      connectionStatus: widget.controller.stale
          ? 'UNAVAILABLE'
          : 'SYSTEM ONLINE',
      connectionOnline: !widget.controller.stale,
      modelStatus: 'Synthetic local model observation — visual fixture only',
      onSelectChat: (id) => setState(() => selected = id),
      onNewChat: () => setState(() => selected = null),
      onRenameChat: (id, title) => _update(id, title: title),
      onArchiveChat: (id) => _update(id, archived: true),
      onReactivateChat: (id) => _update(id, archived: false),
      onDeleteChat: (id) => _update(id, delete: true),
      onRetry: widget.controller.retryObservation,
      child: ChatPanel(
        controller: widget.controller,
        onRetry: widget.controller.retryObservation,
      ),
    ),
  );
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    final materialIcons = FontLoader('MaterialIcons')
      ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'));
    await materialIcons.load();
    for (final (family, directory, prefix) in [
      ('JetBrains Mono', 'jetbrains-mono', 'JetBrainsMono'),
      ('IBM Plex Mono', 'ibm-plex-mono', 'IBMPlexMono'),
    ]) {
      final loader = FontLoader(family);
      // FontLoader has no explicit weight-registration parameter. Keep glyph
      // captures unambiguously Regular; production uses the weighted manifest.
      loader.addFont(
        rootBundle.load('assets/fonts/$directory/$prefix-Regular.ttf'),
      );
      await loader.load();
    }
  });

  testWidgets(
    'historical single-conversation presentation golden (not production v2 gates)',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(1280, 720);
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPhysicalSize);
      final control = FakeControlApi(profile());
      final api = LegacyFakeAppApi()
        ..history.addAll([
          {
            'role': 'user',
            'content': 'Show the existing conversation interface.',
          },
          {
            'role': 'assistant',
            'content': 'This is synthetic saved history supplied by a fake API. The real application exposes only its currently supported single conversation.',
          },
        ]);
      // Preserve the original visual reference; production v2 capability and
      // interaction contracts are exercised in multi_chat_widget_test.dart.
      final chat = ChatController(api);
      await chat.synchronize();
      await chat.refreshModels();
      const boundary = ValueKey('production-shell-fixture-capture');
      await tester.pumpWidget(
        RepaintBoundary(
          key: boundary,
          child: MaterialApp(
            debugShowCheckedModeBanner: false,
            theme: SmithTheme.dark(),
            home: ConversationScreen(control: control, chat: chat),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.byType(ChatPanel), findsOneWidget);
      final workspace = tester.widget<SmithWorkspace>(
        find.byType(SmithWorkspace),
      );
      expect(workspace.chats, hasLength(1));
      expect(workspace.chats.single.title, 'Current conversation');
      expect(workspace.onNewChat, isNull);
      expect(workspace.onArchiveChat, isNull);
      expect(workspace.onRenameChat, isNull);
      expect(workspace.onDeleteChat, isNull);
      expect(find.byTooltip('New chat'), findsNothing);
      expect(find.byTooltip('Archived chats'), findsNothing);
      expect(find.byTooltip('Actions for Current conversation'), findsNothing);
      expect(control.calls, isEmpty);
      expect(api.calls, ['state', 'models', 'poll']);
      expect(tester.takeException(), isNull);
      await expectLater(
        find.byKey(boundary),
        matchesGoldenFile(
          'goldens/ui-redesign/fixture-production-shell-1280x720.png',
        ),
      );
      await tester.pumpWidget(const SizedBox.shrink());
      chat.dispose();
      control.dispose();
      expect(api.disposed, isTrue);
    },
  );

  test(
    'redesign golden regular body and production font manifest weights',
    () async {
      final manifest =
          jsonDecode(await rootBundle.loadString('FontManifest.json')) as List;
      for (final family in ['JetBrains Mono', 'IBM Plex Mono']) {
        final entry = manifest.cast<Map<String, dynamic>>().singleWhere(
          (entry) => entry['family'] == family,
        );
        final fonts = (entry['fonts'] as List).cast<Map<String, dynamic>>();
        expect(
          fonts.map((font) => font['weight']),
          containsAll([400, 500, 600]),
        );
        expect(
          fonts.singleWhere((font) => font['weight'] == 400)['asset'],
          endsWith('-Regular.ttf'),
        );
        expect(
          SmithTheme.dark(fontFamily: family)
                  .textTheme
                  .bodyMedium!
                  .fontWeight ??
              FontWeight.normal,
          FontWeight.w400,
        );
      }
    },
  );

  for (final (name, size, scale, surface) in [
    ('fixture-chat-400x400', const Size(400, 400), 1.0, 'chat'),
    ('fixture-chat-1440x900', const Size(1440, 900), 1.0, 'chat'),
    ('fixture-chat-1280x720', const Size(1280, 720), 1.0, 'chat'),
    ('fixture-chat-1440x900-ibm', const Size(1440, 900), 1.0, 'chat'),
    ('fixture-chat-4200x2400', const Size(4200, 2400), 1.0, 'chat'),
    ('fixture-chat-400x400-text2', const Size(400, 400), 2.0, 'chat'),
    ('fixture-archive-1440x900', const Size(1440, 900), 1.0, 'archive'),
    ('fixture-settings-1440x900', const Size(1440, 900), 1.0, 'settings'),
    ('fixture-overflow-1440x900', const Size(1440, 900), 1.0, 'overflow'),
    ('fixture-archive-400x400-text2', const Size(400, 400), 2.0, 'archive'),
    ('fixture-settings-400x400-text2', const Size(400, 400), 2.0, 'settings'),
    ('fixture-overflow-400x400-text2', const Size(400, 400), 2.0, 'overflow'),
    ('fixture-history-400x400-text2', const Size(400, 400), 2.0, 'history'),
    ('fixture-recovery-400x400-text2', const Size(400, 400), 2.0, 'recovery'),
    (
      'fixture-recovery-details-400x400-text2',
      const Size(400, 400),
      2.0,
      'recovery-details',
    ),
  ]) {
    testWidgets('redesign golden $name bundled fonts real ChatPanel', (
      tester,
    ) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = size;
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPhysicalSize);
      final controller = _GoldenController();
      if (surface.startsWith('recovery')) {
        controller.stale = true;
        controller.error =
            'Fixture observation unavailable. Retry only observes; no resend.';
      }
      const boundary = ValueKey('synthetic-redesign-capture');
      await tester.pumpWidget(
        RepaintBoundary(
          key: boundary,
          child: _VisualFixture(
            controller: controller,
            scale: scale,
            fontFamily: name.endsWith('-ibm')
                ? 'IBM Plex Mono'
                : 'JetBrains Mono',
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(find.byType(SmithAppHeader), findsOneWidget);
      expect(find.text('v0.0.fixture'), findsNothing);
      expect(find.byTooltip('Retry observation'), findsNothing);
      if (size.width < 960 + (scale - 1) * 320) {
        expect(
          find.byTooltip('Choose chat: Workspace interface'),
          findsOneWidget,
        );
        expect(find.byTooltip('New chat').hitTestable(), findsOneWidget);
        expect(find.byTooltip('Archived chats').hitTestable(), findsOneWidget);
      }
      expect(
        tester.getRect(find.byTooltip('Send')).bottom,
        lessThanOrEqualTo(size.height),
      );
      final chatState = tester.state(find.byType(ChatPanel));
      if (surface == 'recovery-details') {
        await tester.tap(find.text('Needs attention'));
        await tester.pumpAndSettle();
        expect(find.byType(AlertDialog), findsOneWidget);
        expect(
          find.byWidgetPredicate(
            (widget) =>
                widget is SelectableText && widget.data == controller.error,
          ),
          findsOneWidget,
        );
        expect(controller.retries, 0);
        expect(controller.sends, 0);
      } else if (surface == 'settings') {
        await tester.tap(find.byTooltip('Settings'));
      } else if (surface != 'chat' && surface != 'recovery') {
        if (size.width < 960) {
          await tester.tap(find.byTooltip('Open chat history'));
          await tester.pumpAndSettle();
        }
        if (surface == 'archive') {
          await tester.tap(
            find.descendant(
              of: find.byType(ChatSidebar),
              matching: find.byTooltip('Archived chats'),
            ),
          );
        } else if (surface == 'overflow') {
          await tester.tap(find.byTooltip('Actions for Workspace interface'));
        }
      }
      await tester.pumpAndSettle();
      if (surface == 'settings') {
        expect(find.byKey(const ValueKey('compact-chat-title')), findsNothing);
      }
      // Let pointer tooltips disappear so each fixture captures the requested surface.
      await tester.pump(const Duration(seconds: 2));
      expect(tester.takeException(), isNull);
      expect(
        tester.state(find.byType(ChatPanel, skipOffstage: false)),
        same(chatState),
      );
      await expectLater(
        find.byKey(boundary),
        matchesGoldenFile('goldens/ui-redesign/$name.png'),
      );
      if (surface == 'settings') {
        final version = find.text('v0.0.fixture');
        await tester.scrollUntilVisible(
          version,
          200,
          scrollable: find
              .descendant(
                of: find.byType(SettingsView),
                matching: find.byType(Scrollable),
              )
              .first,
        );
        await tester.pumpAndSettle();
        expect(version.hitTestable(), findsOneWidget);
      }
      if (surface == 'recovery-details') {
        final retry = find.widgetWithText(TextButton, 'Retry');
        final dialogScroll = find
            .descendant(
              of: find.byType(AlertDialog),
              matching: find.byType(Scrollable),
            )
            .first;
        await tester.scrollUntilVisible(retry, 80, scrollable: dialogScroll);
        await tester.pumpAndSettle();
        expect(retry.hitTestable(), findsOneWidget);
        expect(tester.takeException(), isNull);
        expect(controller.retries, 0);
        expect(controller.sends, 0);
        await tester.tap(find.widgetWithText(TextButton, 'Close'));
        await tester.pumpAndSettle();
        expect(controller.retries, 0);
        expect(controller.draft, 'Review the next detail…');
        expect(tester.state(find.byType(ChatPanel)), same(chatState));
        await tester.tap(find.text('Needs attention'));
        await tester.pumpAndSettle();
        await tester.scrollUntilVisible(
          retry,
          80,
          scrollable: find
              .descendant(
                of: find.byType(AlertDialog),
                matching: find.byType(Scrollable),
              )
              .first,
        );
        await tester.pumpAndSettle();
        await tester.tap(retry);
        await tester.pumpAndSettle();
        expect(controller.retries, 1);
        expect(controller.sends, 0);
        expect(tester.takeException(), isNull);
      }
      await tester.pumpWidget(const SizedBox.shrink());
      controller.dispose();
    });
  }
}
