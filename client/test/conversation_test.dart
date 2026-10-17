import 'dart:async';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/chat_panel.dart';
import 'package:smith_client/main.dart';
import 'package:smith_client/text_chat.dart';
import 'package:smith_client/ui/smith_workspace.dart';

import 'app_fakes.dart';
import 'widget_test.dart' show FakeControlApi, profile;

void main() {
  testWidgets(
    'M1 lost acknowledgement recovery does not duplicate saved turns when follow-up state fails',
    (tester) async {
      final api = LegacyFakeAppApi();
      final c = ChatController(api);
      await c.synchronize();
      await c.refreshModels();
      api.unknownAdmission = true;
      c.edit('lost acknowledgement request');
      await c.send();
      final id = c.pendingId!;
      expect(api.history, hasLength(2));
      c.edit('new draft');
      api.stateFailureAfterPoll = true;
      await c.synchronize();
      expect(c.history, api.history);
      expect(c.history, hasLength(2));
      expect(c.provisionalAnswers, {id: 'useful answer'});
      expect(c.pendingId, isNull);
      expect(c.stale, isTrue);
      expect(c.draft, 'new draft');
      expect(api.arguments[api.calls.lastIndexOf('poll')]['request_id'], id);
      expect(api.calls, [
        'state',
        'models',
        'poll',
        'submit',
        'state',
        'poll',
        'state',
      ]);
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(body: ChatPanel(controller: c)),
        ),
      );
      expect(find.text('lost acknowledgement request'), findsOneWidget);
      expect(find.text('useful answer'), findsOneWidget);
      expect(
        find.text('Answer text is already visible in saved history.'),
        findsOneWidget,
      );
      expect(find.textContaining('stale / unavailable'), findsOneWidget);
      api.stateFailure = false;
      api.stateFailureAfterPoll = false;
      await c.synchronize();
      await tester.pumpAndSettle();
      expect(c.history, hasLength(2));
      expect(c.provisionalAnswers, isEmpty);
      expect(
        find.text('Verified answer · awaiting history sync'),
        findsNothing,
      );
      expect(find.text('useful answer'), findsOneWidget);
      expect(api.calls.where((op) => op == 'submit'), hasLength(1));
      await tester.pumpWidget(const SizedBox());
      c.dispose();
    },
  );

  testWidgets(
    'M2 reattached submit retains verified answer without inventing user text when follow-up state fails',
    (tester) async {
      final api = LegacyFakeAppApi();
      api.active = {
        'request_id': testInstance,
        'kind': 'submit',
        'status': 'running',
        'phase': 'generation',
        'result': null,
        'error': null,
      };
      api.jobs[testInstance] = {
        ...api.active!,
        'status': 'succeeded',
        'phase': 'complete',
        'result': {
          'assistant_text': 'reattached answer',
          'audio': null,
          'speech_error': null,
        },
      };
      api.stateFailureAfterPoll = true;
      final c = ChatController(api)..edit('unsent local draft');
      await c.synchronize();
      expect(c.history, isEmpty);
      expect(c.provisionalAnswers, {testInstance: 'reattached answer'});
      expect(c.pendingId, isNull);
      expect(c.stale, isTrue);
      expect(c.draft, 'unsent local draft');
      expect(api.calls, ['state', 'poll', 'state']);
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(body: ChatPanel(controller: c)),
        ),
      );
      expect(find.text('reattached answer'), findsOneWidget);
      expect(
        find.byKey(const ValueKey('terminal-answer-$testInstance')),
        findsOneWidget,
      );
      expect(
        find.text('Verified answer · awaiting history sync'),
        findsOneWidget,
      );
      expect(find.text('YOU'), findsNothing);
      expect(find.textContaining('stale / unavailable'), findsOneWidget);
      api.stateFailure = false;
      api.stateFailureAfterPoll = false;
      api.active = null;
      api.history.addAll([
        {'role': 'user', 'content': 'service-owned input'},
        {'role': 'assistant', 'content': 'reattached answer'},
      ]);
      await c.synchronize();
      await tester.pumpAndSettle();
      expect(c.history, api.history);
      expect(c.provisionalAnswers, isEmpty);
      expect(
        find.text('Verified answer · awaiting history sync'),
        findsNothing,
      );
      expect(find.text('reattached answer'), findsOneWidget);
      expect(find.text('service-owned input'), findsOneWidget);
      expect(c.draft, 'unsent local draft');
      expect(api.calls.where((op) => op == 'submit'), isEmpty);
      await tester.pumpWidget(const SizedBox());
      c.dispose();
    },
  );
  test(
    'poll instance mismatch keeps query-only identity until synchronization',
    () async {
      final api = LegacyFakeAppApi();
      final c = ChatController(api);
      await c.synchronize();
      await c.refreshModels();
      api.instanceMismatchPoll = true;
      c.edit('work');
      await c.send();
      final id = c.pendingId;
      expect(id, isNotNull);
      expect(c.error, contains('INSTANCE_MISMATCH'));
      await c.send();
      api.instanceMismatchPoll = false;
      api.instance = 'ffffffffffffffffffffffffffffffff';
      await c.synchronize();
      expect(c.pendingId, isNull);
      expect(api.arguments[api.calls.lastIndexOf('poll')]['request_id'], id);
      expect(api.calls.where((op) => op == 'submit').length, 1);
      c.dispose();
    },
  );
  test(
    'reattach queries active legacy work without executing or resending it',
    () async {
      final api = LegacyFakeAppApi();
      final job = <String, dynamic>{
        'request_id': testInstance,
        'kind': 'speak',
        'status': 'succeeded',
        'phase': 'complete',
        'result': {
          'audio': {'expired': true},
        },
        'error': null,
      };
      api.active = job;
      api.jobs[testInstance] = job;
      final c = ChatController(api)..edit('keep draft');
      await c.synchronize();
      expect(api.calls, ['state', 'poll']);
      expect(c.pendingId, isNull);
      expect(c.draft, 'keep draft');
      c.dispose();
    },
  );

  test(
    'malformed terminal response retains pending identity and draft',
    () async {
      final api = LegacyFakeAppApi();
      final c = ChatController(api);
      await c.synchronize();
      await c.refreshModels();
      api.pollGate = Completer<AppReply>();
      c.edit('keep draft');
      final work = c.send();
      for (var i = 0; i < 10; i++) {
        await Future<void>.delayed(Duration.zero);
      }
      final id = c.pendingId;
      api.pollGate!.complete(
        AppReply(testInstance, {
          ...api.jobs.values.last,
          'result': {'assistant_text': null},
        }),
      );
      await work;
      expect(c.pendingId, id);
      expect(c.draft, 'keep draft');
      expect(c.error, contains('PROTOCOL'));
      expect(api.calls.where((op) => op == 'submit').length, 1);
      c.dispose();
    },
  );
  testWidgets('Chat and Settings rail has exactly one selected mode', (
    tester,
  ) async {
    await tester.pumpWidget(
      SmithApp(controlApi: FakeControlApi(profile()), appApi: FakeAppApi()),
    );
    await tester.pumpAndSettle();

    expect(find.byType(SmithWorkspace), findsOneWidget);
    final chat = find.byTooltip('Open chat history');
    final settings = find.byTooltip('Settings');
    expect(chat, findsOneWidget);
    expect(settings, findsOneWidget);
    expect(
      tester.getSemantics(chat).flagsCollection.isSelected,
      ui.Tristate.isTrue,
    );
    expect(
      tester.getSemantics(settings).flagsCollection.isSelected,
      ui.Tristate.isFalse,
    );
    await tester.tap(settings);
    await tester.pumpAndSettle();
    expect(
      tester
          .getSemantics(find.byTooltip('Return to Chat'))
          .flagsCollection
          .isSelected,
      ui.Tristate.isFalse,
    );
    expect(
      tester
          .getSemantics(find.byTooltip('Settings').first)
          .flagsCollection
          .isSelected,
      ui.Tristate.isTrue,
    );
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('Enter sends the focused composer exactly once', (tester) async {
    final api = FakeAppApi();
    await tester.pumpWidget(
      SmithApp(controlApi: FakeControlApi(profile()), appApi: api),
    );
    await tester.pumpAndSettle();

    final composer = find.byType(TextField);
    await tester.tap(composer);
    await tester.enterText(composer, 'send with enter');
    await tester.sendKeyDownEvent(LogicalKeyboardKey.enter);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();

    expect(api.calls.where((op) => op == 'submit'), hasLength(1));
    expect(api.history.first['content'], 'send with enter');
    expect(tester.widget<TextField>(composer).controller!.text, isEmpty);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('Shift+Enter inserts a newline without sending', (tester) async {
    final api = FakeAppApi();
    await tester.pumpWidget(
      SmithApp(controlApi: FakeControlApi(profile()), appApi: api),
    );
    await tester.pumpAndSettle();

    final composer = find.byType(TextField);
    await tester.tap(composer);
    await tester.enterText(composer, 'first line');
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.enter);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.enter);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    await tester.pump();

    expect(tester.widget<TextField>(composer).controller!.text, 'first line\n');
    expect(api.calls.where((op) => op == 'submit'), isEmpty);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('Enter preserves the draft when sending is unavailable', (
    tester,
  ) async {
    final api = FakeAppApi()..config['model'] = null;
    await tester.pumpWidget(
      SmithApp(controlApi: FakeControlApi(profile()), appApi: api),
    );
    await tester.pumpAndSettle();

    final composer = find.byType(TextField);
    await tester.tap(composer);
    await tester.enterText(composer, 'keep disabled draft');
    await tester.sendKeyDownEvent(LogicalKeyboardKey.enter);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.enter);
    await tester.pump();

    expect(
      tester.widget<TextField>(composer).controller!.text,
      'keep disabled draft',
    );
    expect(api.calls.where((op) => op == 'submit'), isEmpty);
    await tester.pumpWidget(const SizedBox());
  });

  for (final setup in [true, false, null]) {
    testWidgets('conversation connects without setup gate: $setup', (
      tester,
    ) async {
      final core = FakeControlApi(profile(setup: setup));
      final api = FakeAppApi();
      await tester.pumpWidget(SmithApp(controlApi: core, appApi: api));
      await tester.pumpAndSettle();
      expect(core.calls, ['inspect']);
      expect(api.calls, ['state', 'conversation_get', 'models', 'poll']);
      expect(find.text('SYSTEM ONLINE'), findsOneWidget);
      expect(find.text('Message Smith…'), findsOneWidget);
      for (final label in [
        'Core Home',
        'Setup',
        'Install binary',
        'Register service',
        'Start service',
        'Finish setup',
        'Open installer',
        'Save model',
        'Microphone',
        'Speak',
        'Text+speech',
      ]) {
        expect(find.text(label), findsNothing);
      }
      expect(api.calls, isNot(contains('submit')));
      await tester.pumpWidget(const SizedBox());
      expect(core.disposed, isTrue);
      expect(api.disposed, isTrue);
    });
  }

  for (final facts in [
    profile(binary: 'missing', setup: false),
    profile(unit: 'missing', setup: false),
    profile(
      runtime: 'stopped',
      bridge: 'unavailable',
      enablement: 'disabled',
      setup: false,
    ),
    profile(bridge: 'conflict', setup: true),
    profile(ok: false),
  ]) {
    testWidgets(
      'unverified Core never connects: ${facts['binary_state']}/${facts['unit_state']}/${facts['runtime_state']}/${facts['bridge_state']}/${facts['ok']}',
      (tester) async {
        final core = FakeControlApi(facts);
        final api = FakeAppApi();
        await tester.pumpWidget(SmithApp(controlApi: core, appApi: api));
        await tester.pumpAndSettle();
        expect(core.calls, ['inspect']);
        expect(api.calls, isEmpty);
        expect(find.byKey(const ValueKey('boot-surface')), findsOneWidget);
        expect(find.byType(TextField), findsNothing);
        expect(find.text('Open installer'), findsNothing);
        await tester.pumpWidget(const SizedBox());
      },
    );
  }

  testWidgets('model configuration stays in the separate installer', (
    tester,
  ) async {
    final core = FakeControlApi(profile(setup: false));
    final api = FakeAppApi()..config['model'] = null;
    await tester.pumpWidget(SmithApp(controlApi: core, appApi: api));
    await tester.pumpAndSettle();
    expect(find.byType(DropdownButton<String>), findsNothing);
    expect(find.text('Save model'), findsNothing);
    expect(find.textContaining('No local model selected'), findsOneWidget);
    expect(
      tester
          .widget<IconButton>(
            find.ancestor(
              of: find.byTooltip('Send'),
              matching: find.byType(IconButton),
            ),
          )
          .onPressed,
      isNull,
    );
    await tester.enterText(find.byType(TextField), 'waiting draft');
    api.config = {...api.config, 'model': 'local:exact'};
    tester.widget<SmithWorkspace>(find.byType(SmithWorkspace)).onRetry!();
    await tester.pumpAndSettle();
    expect(find.text('waiting draft'), findsOneWidget);
    expect(
      tester
          .widget<IconButton>(
            find.ancestor(
              of: find.byTooltip('Send'),
              matching: find.byType(IconButton),
            ),
          )
          .onPressed,
      isNotNull,
    );
    expect(api.calls, isNot(contains('configure')));
    expect(api.calls, isNot(contains('submit')));
    expect(core.calls, ['inspect']);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
    'outage preserves visible history and editable draft; Retry never sends',
    (tester) async {
      final core = FakeControlApi(profile());
      final api = FakeAppApi()
        ..history.addAll([
          {'role': 'user', 'content': 'saved question'},
          {'role': 'assistant', 'content': 'saved answer'},
        ]);
      await tester.pumpWidget(SmithApp(controlApi: core, appApi: api));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'keep draft');
      api.stateFailure = true;
      tester.widget<SmithWorkspace>(find.byType(SmithWorkspace)).onRetry!();
      await tester.pumpAndSettle();
      expect(find.text('saved answer'), findsOneWidget);
      expect(find.text('keep draft'), findsOneWidget);
      expect(find.text('SYSTEM UNAVAILABLE'), findsOneWidget);
      expect(
        tester
            .widget<IconButton>(
              find.ancestor(
                of: find.byTooltip('Send'),
                matching: find.byType(IconButton),
              ),
            )
            .onPressed,
        isNull,
      );
      expect(api.calls, [
        'state',
        'conversation_get',
        'models',
        'poll',
        'state',
      ]);
      api.stateFailure = false;
      tester.widget<SmithWorkspace>(find.byType(SmithWorkspace)).onRetry!();
      await tester.pumpAndSettle();
      expect(api.calls.where((op) => op == 'submit'), isEmpty);
      expect(core.calls, ['inspect']);
      expect(find.text('SYSTEM ONLINE'), findsOneWidget);
      expect(find.text('keep draft'), findsOneWidget);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'one failed inventory observation, no automatic retry or inference',
    (tester) async {
      final api = FakeAppApi()
        ..providers['llm'] = {'ready': false, 'error': 'unavailable'};
      await tester.pumpWidget(
        SmithApp(controlApi: FakeControlApi(profile()), appApi: api),
      );
      await tester.pumpAndSettle();
      await tester.pump(const Duration(seconds: 5));
      expect(api.calls, ['state', 'conversation_get', 'models', 'poll']);
      expect(
        find.textContaining('Local model runtime unavailable'),
        findsOneWidget,
      );
      expect(api.calls, isNot(contains('submit')));
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets('narrow enlarged text keeps Boot available without setup', (
    tester,
  ) async {
    tester.platformDispatcher.textScaleFactorTestValue = 2;
    addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
    tester.view.physicalSize = const Size(360, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await tester.pumpWidget(
      SmithApp(
        controlApi: FakeControlApi(profile(binary: 'missing')),
        appApi: FakeAppApi(),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Open installer'), findsNothing);
    expect(find.byKey(const ValueKey('boot-surface')), findsOneWidget);
    expect(find.byType(TextField), findsNothing);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('long history scrolls without moving the composer', (
    tester,
  ) async {
    final api = FakeAppApi();
    for (var i = 0; i < 32; i++) {
      api.history.add({
        'role': i.isEven ? 'user' : 'assistant',
        'content': 'Saved answer number $i',
      });
    }
    await tester.pumpWidget(
      SmithApp(controlApi: FakeControlApi(profile()), appApi: api),
    );
    await tester.pumpAndSettle();
    final before = tester.getRect(find.byType(TextField));
    final transcript = tester
        .widget<SingleChildScrollView>(
          find.byKey(const ValueKey('conversation-history')),
        )
        .controller!;
    expect(transcript.offset, transcript.position.maxScrollExtent);

    await tester.drag(
      find.byKey(const ValueKey('conversation-history')),
      const Offset(0, 300),
    );
    await tester.pumpAndSettle();
    final readingOffset = transcript.offset;
    expect(readingOffset, lessThan(transcript.position.maxScrollExtent));
    await tester.enterText(find.byType(TextField), 'draft while reading');
    await tester.pump();
    expect(transcript.offset, readingOffset);
    expect(tester.getRect(find.byType(TextField)), before);
    expect(before.bottom, lessThan(tester.view.physicalSize.height));
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  for (final size in [const Size(360, 800), const Size(800, 600)]) {
    for (final expired in [false, true]) {
      testWidgets('enlarged multiline recovery fits $size expired=$expired', (
        tester,
      ) async {
        tester.platformDispatcher.textScaleFactorTestValue = 2;
        addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
        tester.view.physicalSize = size;
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        final api = FakeAppApi();
        await tester.pumpWidget(
          SmithApp(controlApi: FakeControlApi(profile()), appApi: api),
        );
        await tester.pumpAndSettle();
        final bootCallCount = api.calls.length;
        await tester.enterText(
          find.byType(TextField),
          'one\ntwo\nthree\nfour\nfive',
        );
        await tester.pumpAndSettle();
        if (expired) {
          api.active = {
            'request_id': testInstance,
            'kind': 'submit',
            'status': 'running',
            'phase': 'generation',
          };
          api.expiredPoll = true;
          await tester.pump(const Duration(seconds: 30));
          await tester.pumpAndSettle();
        }
        expect(tester.takeException(), isNull);
        const enteredDraft = 'one\ntwo\nthree\nfour\nfive';
        expect(
          tester.widget<TextField>(find.byType(TextField)).controller!.text,
          enteredDraft,
        );
        for (final control in [
          find.byType(TextField),
          expired
              ? find.byTooltip(
                  'Queue next turn, not steering the current reply',
                )
              : find.byTooltip('Send'),
        ]) {
          final rect = tester.getRect(control);
          expect(rect.top, greaterThanOrEqualTo(0));
          expect(rect.bottom, lessThanOrEqualTo(size.height));
          expect(control.hitTestable(), findsOneWidget);
        }
        if (expired) {
          final controller = tester
              .widget<ChatPanel>(find.byType(ChatPanel))
              .controller;
          expect(controller.pendingId, testInstance);
          expect(controller.canAbandonExpiredRequest, isTrue);
          Future<void> openResolution() async {
            final details = find.byTooltip('View recovery details');
            if (details.evaluate().isNotEmpty) {
              await tester.ensureVisible(details);
              await tester.pumpAndSettle();
              expect(details.hitTestable(), findsOneWidget);
              await tester.tap(details);
              await tester.pumpAndSettle();
            }
            final resolve = find.text('Resolve expired request');
            await tester.ensureVisible(resolve);
            await tester.pumpAndSettle();
            expect(resolve.hitTestable(), findsOneWidget);
            await tester.tap(resolve);
            await tester.pumpAndSettle();
          }

          await openResolution();
          expect(find.text('Keep pending').hitTestable(), findsOneWidget);
          await tester.tap(find.text('Keep pending'));
          await tester.pumpAndSettle();
          expect(
            tester.widget<TextField>(find.byType(TextField)).controller!.text,
            enteredDraft,
          );
          expect(controller.pendingId, testInstance);
          await openResolution();
          await tester.tap(find.text('Forget request'));
          await tester.pumpAndSettle();
          expect(
            tester.widget<TextField>(find.byType(TextField)).controller!.text,
            enteredDraft,
          );
          expect(controller.pendingId, isNull);
          expect(tester.takeException(), isNull);
        }
        expect(
          api.calls
              .skip(bootCallCount)
              .where(
                (op) => !['state', 'conversation_get', 'poll'].contains(op),
              ),
          isEmpty,
        );
        await tester.pumpWidget(const SizedBox());
      });
    }
  }

  test('late submit preserves newer draft, text-only result survives failed state read', () async {
    final api = LegacyFakeAppApi();
    final c = ChatController(api);
    await c.synchronize();
    await c.refreshModels();
    c.edit('first draft');
    api.pollGate = Completer<AppReply>();
    final work = c.send();
    for (var i = 0; i < 10; i++) {
      await Future<void>.delayed(Duration.zero);
    }
    c.edit('new draft');
    api.stateFailure = true;
    api.pollGate!.complete(AppReply(testInstance, api.jobs.values.last));
    await work;
    expect(c.draft, 'new draft');
    expect(c.history, isEmpty);
    expect(c.provisionalAnswers, {api.jobs.keys.last: 'useful answer'});
    expect(c.stale, isTrue);
    expect(api.arguments[api.calls.indexOf('submit')]['output'], 'chat');
    expect(api.calls.where((op) => op == 'submit').length, 1);
    c.dispose();
  });

  test('unobserved selection cannot be configured or submitted', () async {
    final api = LegacyFakeAppApi();
    final c = ChatController(api);
    await c.synchronize();
    c.edit('draft');
    await c.send();
    await c.configure('local:exact');
    expect(api.calls, ['state']);
    await c.refreshModels();
    await c.configure('invented:tag');
    expect(api.calls, isNot(contains('configure')));
    c.dispose();
  });
}
