import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:smith_client/bridge/smith_bridge.dart';
import 'package:smith_client/chat/chat_controller.dart';
import 'package:smith_client/home/halo_theme.dart';
import 'package:smith_client/home/offline_chat_screen.dart';

class ChatFixture {
  final operations = <String>[];
  final requests = <Map<String, dynamic>>[];
  String? profile = 'chat-only';
  String service = 'smith-bridge';
  bool providerAvailable = true;
  bool offline = false;
  Object? sendError;
  String replyText = 'A local answer';
  Completer<http.Response>? pending;

  late final bridge = SmithBridge(
    client: MockClient((request) async {
      operations.add('${request.method} ${request.url.path}');
      expect(request.url.host, '127.0.0.1');
      expect(request.followRedirects, isFalse);
      if (request.method == 'GET' && request.url.path == '/v1/health') {
        if (offline) throw http.ClientException('offline');
        return http.Response(
          jsonEncode({
            'ok': true,
            'service': service,
            'profile': profile,
            'echo': false,
            'provider': 'ollama',
            'model': 'fixture:local',
            'provider_available': providerAvailable,
          }),
          200,
        );
      }
      if (request.method == 'POST' && request.url.path == '/v1/chat-only') {
        requests.add(jsonDecode(request.body) as Map<String, dynamic>);
        if (sendError != null) throw sendError!;
        if (pending != null) return pending!.future;
        return reply();
      }
      fail(
        'Forbidden release-client request: ${request.method} ${request.url.path}',
      );
    }),
  );

  http.Response reply() => http.Response(
    jsonEncode({
      'v': 1,
      'profile': 'chat-only',
      'mode': 'chat-only',
      'text': replyText,
      'session': 'default',
      'pipeline': null,
      'task': null,
      'attachments': [],
    }),
    200,
  );

  ChatController controller() => ChatController(bridge: bridge, chatOnly: true);
}

Future<void> mount(
  WidgetTester tester,
  ChatController chat, {
  double width = 1000,
  double scale = 1,
}) async {
  tester.view.physicalSize = Size(width, 1000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(
    MaterialApp(
      theme: buildHaloTheme(),
      builder: (context, child) => MediaQuery(
        data: MediaQuery.of(
          context,
        ).copyWith(textScaler: TextScaler.linear(scale)),
        child: child!,
      ),
      home: OfflineChatScreen(chat: chat),
    ),
  );
  await tester.pumpAndSettle();
}

Finder get draftField => find.byKey(const Key('text-chat-draft'));

void main() {
  test(
    'release startup and explicit reads use health only, no tasks or speech',
    () async {
      final fixture = ChatFixture();
      final chat = fixture.controller();
      addTearDown(chat.dispose);
      await chat.start();
      await chat.refreshTasks();
      await chat.refreshHealth();
      expect(fixture.operations, everyElement('GET /v1/health'));
      expect(chat.messages, isEmpty);
      expect(chat.tasks, isEmpty);
      expect(chat.pollingTasks, isFalse);
      expect(chat.resetPending, isTrue);
      await expectLater(
        chat.ensureVoiceSidecars(),
        throwsA(isA<SmithBridgeException>()),
      );
      expect(fixture.operations, everyElement('GET /v1/health'));
    },
  );

  test('unknown/legacy profile and wrong identity fail closed', () async {
    for (final profile in [null, 'legacy', 'chat-only']) {
      final fixture = ChatFixture()..profile = profile;
      if (profile == 'chat-only') fixture.service = 'unrelated';
      final chat = fixture.controller();
      await chat.start();
      expect(chat.canSend, isFalse);
      await chat.send('Must not leave client');
      expect(fixture.requests, isEmpty);
      chat.dispose();
    }
  });

  test(
    'profile is rechecked before sending after the listener changes',
    () async {
      final fixture = ChatFixture();
      final chat = fixture.controller();
      addTearDown(chat.dispose);
      await chat.start();
      fixture.profile = 'legacy';
      await chat.send('Do not dispatch to legacy');
      expect(fixture.requests, isEmpty);
      expect(chat.failedText, 'Do not dispatch to legacy');
      expect(chat.bridgeStatus, BridgeStatus.offline);
    },
  );

  test(
    'listener replacement after health never dispatches legacy chat/reset',
    () async {
      var replaced = false;
      var legacyChats = 0;
      var legacyResets = 0;
      final operations = <String>[];
      final bridge = SmithBridge(
        client: MockClient((request) async {
          operations.add('${request.method} ${request.url.path}');
          if (request.url.path == '/v1/health') {
            // The healthy process is replaced before the subsequent POST arrives.
            replaced = true;
            return http.Response(
              jsonEncode({
                'ok': true,
                'service': 'smith-bridge',
                'profile': 'chat-only',
                'echo': false,
                'provider_available': true,
                'model_available': true,
                'provider': 'ollama',
                'model': 'fixture:local',
              }),
              200,
            );
          }
          expect(replaced, isTrue);
          if (request.url.path == '/v1/chat') {
            legacyChats++;
            if ((jsonDecode(request.body) as Map)['new'] == true) {
              legacyResets++;
            }
            return http.Response('{"text":"Legacy dispatched"}', 200);
          }
          expect(request.url.path, '/v1/chat-only');
          expect(request.followRedirects, isFalse);
          return http.Response('Not Found', 404);
        }),
      );
      final chat = ChatController(bridge: bridge, chatOnly: true);
      addTearDown(chat.dispose);
      await chat.start();
      operations.clear();
      replaced = false;
      await chat.send('Never orchestrate this');
      expect(operations, ['GET /v1/health', 'POST /v1/chat-only']);
      expect(legacyChats, 0);
      expect(legacyResets, 0);
      expect(chat.failedText, 'Never orchestrate this');
      expect(chat.resetPending, isTrue);
      expect(chat.error, contains('404'));
      await chat.refreshHealth();
      expect(operations.where((op) => op.startsWith('POST')), hasLength(1));
    },
  );

  test(
    'backend reset failure detail remains visible with no automatic retry',
    () async {
      final fixture = ChatFixture()..pending = Completer<http.Response>();
      final chat = fixture.controller();
      addTearDown(chat.dispose);
      await chat.start();
      final sending = chat.send('Reset then fail');
      await pumpEventQueue();
      fixture.pending!.complete(
        http.Response(
          jsonEncode({
            'error':
                'Session reset; previous history cleared. local inference failed',
          }),
          502,
        ),
      );
      await sending;
      expect(chat.error, contains('Session reset; previous history cleared.'));
      expect(chat.error, contains('No download or fallback'));
      expect(chat.resetPending, isTrue);
      await chat.refreshHealth();
      expect(fixture.requests, hasLength(1));
    },
  );

  test(
    'reopening starts with no local history and resets only on explicit send',
    () async {
      final fixture = ChatFixture();
      final first = fixture.controller();
      await first.start();
      await first.send('Earlier conversation');
      first.dispose();
      final reopened = fixture.controller();
      addTearDown(reopened.dispose);
      final count = fixture.requests.length;
      await reopened.start();
      expect(reopened.messages, isEmpty);
      expect(fixture.requests, hasLength(count));
      await reopened.send('Fresh conversation');
      expect(fixture.requests.last['new'], isTrue);
    },
  );

  test(
    'model unavailable blocks send without claiming bridge disconnected',
    () async {
      final fixture = ChatFixture()..providerAvailable = false;
      final chat = fixture.controller();
      addTearDown(chat.dispose);
      await chat.start();
      expect(chat.bridgeStatus, BridgeStatus.online);
      expect(chat.canSend, isFalse);
      expect(chat.connectionError, contains('local model'));
    },
  );

  test(
    'successful text-only sends reset once without tasks/pipelines/attachments',
    () async {
      final fixture = ChatFixture();
      final chat = fixture.controller();
      addTearDown(chat.dispose);
      await chat.start();
      await chat.send('First');
      await chat.send('Second');
      expect(fixture.requests.map((request) => request['new']), [true, false]);
      expect(chat.messages.last.text, 'A local answer');
      expect(chat.messages.last.phase, MessagePhase.complete);
      expect(chat.messages.last.attachments, isEmpty);
      expect(chat.tasks, isEmpty);
      expect(chat.pipeline, isNull);
      expect(chat.pollingTasks, isFalse);
      expect(
        fixture.operations.where((operation) => operation != 'GET /v1/health'),
        everyElement('POST /v1/chat-only'),
      );
    },
  );

  test(
    'timeout retains failed text and pending reset without automatic resend',
    () async {
      final fixture = ChatFixture()..sendError = TimeoutException('fixture');
      final chat = fixture.controller();
      addTearDown(chat.dispose);
      await chat.start();
      await chat.send('Keep this message');
      expect(chat.failedText, 'Keep this message');
      expect(chat.error, contains('outcome is unknown'));
      expect(chat.messages.single.role, ChatRole.user);
      expect(chat.resetPending, isTrue);
      await chat.refreshHealth();
      expect(fixture.requests, hasLength(1));
      fixture.sendError = null;
      await chat.send('Deliberate resend');
      expect(fixture.requests.last['new'], isTrue);
      expect(chat.resetPending, isFalse);
    },
  );

  test(
    'new chat is local and preserves context reset until explicit send',
    () async {
      final fixture = ChatFixture();
      final chat = fixture.controller();
      addTearDown(chat.dispose);
      await chat.start();
      await chat.send('First');
      fixture.operations.clear();
      await chat.requestNewSession();
      expect(fixture.operations, isEmpty);
      expect(chat.messages, isEmpty);
      await chat.send('New conversation');
      expect(fixture.requests.last['new'], isTrue);
    },
  );

  test('blank and overlapping sends do not duplicate requests', () async {
    final fixture = ChatFixture()..pending = Completer<http.Response>();
    final chat = fixture.controller();
    addTearDown(chat.dispose);
    await chat.start();
    await chat.send('   ');
    final first = chat.send('First');
    await pumpEventQueue();
    await chat.send('Second');
    expect(fixture.requests, hasLength(1));
    fixture.pending!.complete(fixture.reply());
    await first;
    expect(
      chat.messages.where((message) => message.role == ChatRole.user),
      hasLength(1),
    );
  });

  for (final resource in [
    'http://example.invalid/image.png',
    'https://example.invalid/image.png',
    'http://127.0.0.1:11434/image.png',
    'file:///tmp/smith-image-must-not-read.png',
    'relative/image.png',
    'data:image/png;base64,iVBORw0KGgo=',
  ]) {
    testWidgets('assistant image is inert selectable text: $resource', (
      tester,
    ) async {
      final prose = '![untrusted image]($resource)';
      final fixture = ChatFixture()..replyText = prose;
      final chat = fixture.controller();
      await mount(tester, chat);
      await tester.enterText(draftField, 'Show inert prose');
      await tester.pump();
      var fileReads = 0;
      var networkClients = 0;
      await IOOverrides.runZoned(
        () async {
          await HttpOverrides.runZoned(
            () async {
              await tester.tap(find.text('Send'));
              await tester.pumpAndSettle();
              expect(find.byType(Image), findsNothing);
              expect(
                find.byWidgetPredicate(
                  (widget) => widget is SelectableText && widget.data == prose,
                ),
                findsOneWidget,
              );
            },
            createHttpClient: (context) {
              networkClients++;
              throw StateError('Image network access forbidden');
            },
          );
        },
        createFile: (path) {
          fileReads++;
          throw StateError('Image file access forbidden');
        },
      );
      expect(fileReads, 0);
      expect(networkClients, 0);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      chat.dispose();
    });
  }

  for (final width in [360.0, 800.0, 1200.0]) {
    testWidgets(
      'Home send opens Chat, draft survives navigation at width $width',
      (tester) async {
        final fixture = ChatFixture();
        final chat = fixture.controller();
        await mount(tester, chat, width: width);
        expect(find.text('Work'), findsNothing);
        expect(find.text('View task'), findsNothing);
        expect(find.byTooltip('Start recording'), findsNothing);
        await tester.enterText(draftField, 'Unsent draft');
        await tester.tap(find.byTooltip('Settings'));
        await tester.pumpAndSettle();
        expect(find.text('Connection'), findsOneWidget);
        expect(find.textContaining('Model: fixture:local'), findsOneWidget);
        expect(find.byType(Switch), findsNothing);
        await tester.tap(find.byIcon(Icons.home_outlined).last);
        await tester.pumpAndSettle();
        expect(find.text('Unsent draft'), findsOneWidget);
        await tester.pump();
        await tester.tap(find.text('Send'));
        await tester.pumpAndSettle();
        expect(
          find.text('Current conversation · no saved history'),
          findsOneWidget,
        );
        expect(find.text('Unsent draft'), findsOneWidget);
        expect(find.text('A local answer'), findsOneWidget);
        expect(find.text('Unrelated work'), findsNothing);
        expect(find.text('Hidden file'), findsNothing);
        expect(fixture.requests, hasLength(1));
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox());
        chat.dispose();
      },
    );
  }

  testWidgets(
    'offline drafting, checking and reconnect are truthful and read-only',
    (tester) async {
      final fixture = ChatFixture()..offline = true;
      final chat = fixture.controller();
      await mount(tester, chat);
      await tester.enterText(draftField, 'Retained offline');
      expect(
        tester
            .widget<FilledButton>(find.widgetWithText(FilledButton, 'Send'))
            .onPressed,
        isNull,
      );
      fixture.offline = false;
      await tester.tap(find.text('Retry connection'));
      await tester.pumpAndSettle();
      expect(find.text('Retained offline'), findsOneWidget);
      expect(fixture.requests, isEmpty);
      expect(fixture.operations, everyElement('GET /v1/health'));
      await tester.pumpWidget(const SizedBox());
      chat.dispose();
    },
  );

  testWidgets('Ctrl+Enter sends and pending response blocks new chat', (
    tester,
  ) async {
    final fixture = ChatFixture()..pending = Completer<http.Response>();
    final chat = fixture.controller();
    await mount(tester, chat);
    await tester.enterText(draftField, 'Keyboard message');
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pump();
    expect(find.text('Waiting for Smith…'), findsOneWidget);
    expect(
      tester
          .widget<TextButton>(find.widgetWithText(TextButton, 'New chat'))
          .onPressed,
      isNull,
    );
    fixture.pending!.complete(fixture.reply());
    await tester.pumpAndSettle();
    expect(find.text('A local answer'), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
    chat.dispose();
  });

  testWidgets(
    'disconnect remains visible with an existing transcript and keeps the draft',
    (tester) async {
      final fixture = ChatFixture();
      final chat = fixture.controller();
      await mount(tester, chat);
      await tester.enterText(draftField, 'First');
      await tester.pump();
      await tester.tap(find.text('Send'));
      await tester.pumpAndSettle();
      await tester.enterText(draftField, 'Unsent during disconnect');
      fixture.offline = true;
      await chat.refreshHealth();
      await tester.pumpAndSettle();
      expect(find.text('Local bridge · unavailable'), findsOneWidget);
      expect(find.text('A local answer'), findsOneWidget);
      expect(
        tester.widget<TextField>(draftField).controller!.text,
        'Unsent during disconnect',
      );
      expect(
        tester
            .widget<FilledButton>(find.widgetWithText(FilledButton, 'Send'))
            .onPressed,
        isNull,
      );
      expect(fixture.requests, hasLength(1));
      await tester.pumpWidget(const SizedBox());
      chat.dispose();
    },
  );

  testWidgets(
    'new chat confirms local clearing without sending or losing draft',
    (tester) async {
      final fixture = ChatFixture();
      final chat = fixture.controller();
      await mount(tester, chat);
      await tester.enterText(draftField, 'First');
      await tester.pump();
      await tester.tap(find.text('Send'));
      await tester.pumpAndSettle();
      await tester.enterText(draftField, 'Preserved draft');
      fixture.operations.clear();
      await tester.tap(find.text('New chat'));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(FilledButton, 'New chat'));
      await tester.pumpAndSettle();
      expect(chat.messages, isEmpty);
      expect(find.text('Preserved draft'), findsOneWidget);
      expect(fixture.requests, hasLength(1));
      expect(fixture.operations, isEmpty);
      await tester.pumpWidget(const SizedBox());
      chat.dispose();
    },
  );

  testWidgets(
    'failed send is distinct and editing never resends or overwrites a draft silently',
    (tester) async {
      final fixture = ChatFixture()..sendError = TimeoutException('fixture');
      final chat = fixture.controller();
      await mount(tester, chat);
      await tester.enterText(draftField, 'Failed text');
      await tester.pump();
      await tester.tap(find.text('Send'));
      await tester.pumpAndSettle();
      expect(find.textContaining('outcome is unknown'), findsOneWidget);
      expect(chat.messages.single.role, ChatRole.user);
      await tester.enterText(draftField, 'Different draft');
      await tester.tap(find.text('Edit failed message'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Cancel'));
      await tester.pumpAndSettle();
      expect(find.text('Different draft'), findsOneWidget);
      await tester.tap(find.text('Edit failed message'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Replace draft'));
      await tester.pumpAndSettle();
      expect(
        tester.widget<TextField>(draftField).controller!.text,
        'Failed text',
      );
      expect(fixture.requests, hasLength(1));
      await tester.pumpWidget(const SizedBox());
      chat.dispose();
    },
  );

  testWidgets('UTF-8 byte bound disables oversized text without losing it', (
    tester,
  ) async {
    final fixture = ChatFixture();
    final chat = fixture.controller();
    await mount(tester, chat);
    final oversized = List.filled(6000, '界').join();
    await tester.enterText(draftField, oversized);
    await tester.pump();
    expect(find.textContaining('exceeds 16384 UTF-8 bytes'), findsOneWidget);
    expect(
      tester
          .widget<FilledButton>(find.widgetWithText(FilledButton, 'Send'))
          .onPressed,
      isNull,
    );
    expect(tester.widget<TextField>(draftField).controller!.text, oversized);
    expect(fixture.requests, isEmpty);
    await tester.pumpWidget(const SizedBox());
    chat.dispose();
  });

  testWidgets('compact enlarged text leaves controls usable', (tester) async {
    final fixture = ChatFixture();
    final chat = fixture.controller();
    await mount(tester, chat, width: 360, scale: 2);
    expect(tester.takeException(), isNull);
    await tester.ensureVisible(find.text('Open chat'));
    await tester.tap(find.text('Open chat'));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
    chat.dispose();
  });
}
