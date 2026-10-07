import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:smith_client/bridge/desktop_backend.dart';
import 'package:smith_client/bridge/desktop_setup.dart';
import 'package:smith_client/bridge/smith_bridge.dart';
import 'package:smith_client/chat/chat_controller.dart';
import 'package:smith_client/home/desktop_boot_shell.dart';
import 'package:smith_client/home/offline_chat_screen.dart';

import 'desktop_backend_test.dart' as backend_fixture;
import 'desktop_setup_test.dart' as fixture;

class WizardFixture {
  WizardFixture({this.setupTimeout = const Duration(seconds: 10)});
  final Duration setupTimeout;
  final runtime = backend_fixture.FakeRuntime();
  final setupRuntime = fixture.SetupFakeRuntime();
  late final backend = DesktopBackendController(
    runtime: runtime,
    clock: backend_fixture.FakeClock(),
  );
  int posts = 0;
  late final chat = ChatController(
    chatOnly: true,
    readinessGuard: backend.accepts,
    bridge: SmithBridge(
      client: MockClient((request) async {
        if (request.method == 'POST') posts++;
        return http.Response(
          jsonEncode({
            'ok': true,
            'service': 'smith-bridge',
            'profile': 'chat-only',
            'echo': false,
            'provider': 'ollama',
            'model': 'fixture:local',
            'provider_available': true,
            'model_available': true,
          }),
          200,
        );
      }),
    ),
  );
  late final setup = DesktopSetup(
    runtime: setupRuntime,
    inspectTimeout: setupTimeout,
  );
  Future<void> mount(WidgetTester tester, {double scale = 1}) async {
    await tester.pumpWidget(
      MaterialApp(
        home: MediaQuery(
          data: MediaQueryData(textScaler: TextScaler.linear(scale)),
          child: DesktopBootShell(backend: backend, chat: chat, setup: setup),
        ),
      ),
    );
    await tester.pumpAndSettle();
    await tester.pump(const Duration(milliseconds: 10));
    await tester.pumpAndSettle();

    expect(
      setup.busy,
      false,
      reason: tester
          .widgetList<Text>(find.byType(Text))
          .map((t) => t.data)
          .join('\n'),
    );
  }

  void inventory({String status = 'ready'}) {
    setupRuntime.script = (child, argv) {
      child.output.add(utf8.encode(fixture.inventory(status: status)));
      child.finish();
    };
  }

  Future<void> close(WidgetTester tester) async {
    await tester.pumpWidget(const SizedBox());
    chat.dispose();
    backend.dispose();
    await tester.pump();
  }
}

Future<void> tap(WidgetTester tester, String text) async {
  await tester.pumpAndSettle();
  final finder = find.text(text).last;
  await tester.ensureVisible(finder);
  await tester.tap(finder);
  await tester.pumpAndSettle();
  await tester.pump(const Duration(milliseconds: 10));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets(
    'installed gpt-oss exact tag reaches Review through legacy conflict without downloads',
    (tester) async {
      final f = WizardFixture();
      final inventory = jsonDecode(fixture.inventory()) as Map<String, dynamic>;
      inventory['models'] = [
        {'name': 'fixture:local', 'digest': 'fixture-digest', 'size': 42},
        {
          'name': 'gpt-oss:20b',
          'digest': 'gpt-oss-digest',
          'size': 13793441244,
        },
        {'name': 'other:local', 'digest': 'other-digest', 'size': null},
      ];
      f.setupRuntime.script = (child, _) {
        child.output.add(utf8.encode(jsonEncode(inventory)));
        child.finish();
      };
      f.runtime.result = BackendProbe(
        ProbeKind.conflict,
        HealthInfo(
          ok: true,
          service: 'smith-bridge',
          echo: false,
          isolation: 'sealed',
        ),
      );
      await f.mount(tester);
      await tap(tester, 'Select a model');
      expect(find.byKey(const Key('download-model')), findsNothing);
      await tester.tap(find.byKey(const Key('installed-model')));
      await tester.pumpAndSettle();
      expect(find.text('gpt-oss:20b · 12.85 GiB'), findsOneWidget);
      expect(find.text('other:local · size unknown'), findsOneWidget);
      expect(find.textContaining('13793441244'), findsNothing);
      await tap(tester, 'fixture:local · 42 B');
      await tester.tap(find.byKey(const Key('installed-model')));
      await tester.pumpAndSettle();
      await tap(tester, 'gpt-oss:20b · 12.85 GiB');
      expect(f.backend.model, 'gpt-oss:20b');
      expect(find.text('gpt-oss:20b'), findsOneWidget);
      expect(
        find.text('Installed locally · gpt-oss:20b · No download needed.'),
        findsOneWidget,
      );
      final pickerState = tester.state<FormFieldState<String>>(
        find.byKey(const Key('installed-model')),
      );
      await tap(tester, 'Check readiness');
      expect(
        tester.state<FormFieldState<String>>(
          find.byKey(const Key('installed-model')),
        ),
        same(pickerState),
      );
      expect(pickerState.value, 'gpt-oss:20b');
      await tap(tester, '2. Local runtime');
      await tap(tester, 'Retry inspection');
      await tap(tester, 'Select a model');
      expect(
        tester
            .state<FormFieldState<String>>(
              find.byKey(const Key('installed-model')),
            )
            .value,
        'gpt-oss:20b',
      );
      expect(find.text('gpt-oss:20b'), findsOneWidget);
      await tap(tester, 'Review Start');
      expect(f.backend.state, BootState.conflict);
      expect(
        find.textContaining('existing legacy Smith bridge on 127.0.0.1:4783'),
        findsWidgets,
      );
      expect(
        find.textContaining('Stop it outside this app, then Check readiness'),
        findsWidgets,
      );
      expect(
        find.textContaining('No download fixes an occupied port'),
        findsWidgets,
      );
      expect(
        tester
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Start packaged backend'),
            )
            .onPressed,
        isNull,
      );
      expect(find.text('Stop owned backend'), findsNothing);
      expect(f.runtime.launches, 0);
      expect(f.runtime.child.effects, isEmpty);
      expect(f.setupRuntime.calls.every((c) => c[2] == 'inspect'), true);
      expect(f.setupRuntime.children.every((c) => c.approvals.isEmpty), true);
      expect(find.text('Confirm model download'), findsNothing);
      expect(f.posts, 0);
      f.runtime.result = const BackendProbe(ProbeKind.unknown);
      await tap(tester, 'Check readiness');
      expect(
        find.textContaining('Start is disabled: Readiness unknown'),
        findsOneWidget,
      );
      expect(
        tester
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Start packaged backend'),
            )
            .onPressed,
        isNull,
      );
      f.runtime.result = const BackendProbe(ProbeKind.absent);
      await tap(tester, 'Check readiness');
      expect(f.runtime.launches, 0);
      expect(
        tester
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Start packaged backend'),
            )
            .onPressed,
        isNotNull,
      );
      f.runtime.afterLaunch = backend_fixture.ready(
        id: backend_fixture.desktopSession,
        model: 'gpt-oss:20b',
      );
      await tap(tester, 'Start packaged backend');
      expect(f.runtime.launches, 1);
      expect(f.runtime.argv, [
        'serve',
        '--chat-only',
        '--port',
        '4783',
        '--model',
        'gpt-oss:20b',
        '--desktop-session',
        backend_fixture.desktopSession,
      ]);
      expect(f.setupRuntime.calls.every((c) => c[2] == 'inspect'), true);
      expect(f.setupRuntime.children.every((c) => c.approvals.isEmpty), true);
      expect(f.posts, 0);
      await f.close(tester);
    },
  );
  testWidgets('Review explains missing selected inventory and blocks Start', (
    tester,
  ) async {
    final f = WizardFixture()..inventory();
    await f.mount(tester);
    await tap(tester, '4. Review & Start');
    expect(
      find.textContaining(
        'Start is disabled: select a model from the current installed inventory',
      ),
      findsOneWidget,
    );
    expect(
      tester
          .widget<FilledButton>(
            find.widgetWithText(FilledButton, 'Start packaged backend'),
          )
          .onPressed,
      isNull,
    );
    expect(f.runtime.launches, 0);
    expect(f.posts, 0);
    await f.close(tester);
  });
  testWidgets(
    'stalled discovery reaches the setup deadline without launching a child',
    (tester) async {
      final f = WizardFixture(setupTimeout: const Duration(milliseconds: 10));
      f.setupRuntime.pendingResolve = Completer<String>();
      await f.mount(tester);
      await tester.pump(const Duration(milliseconds: 20));
      await tester.pumpAndSettle();
      expect(
        find.textContaining('Bundled backend unavailable or invalid'),
        findsOneWidget,
      );
      expect(f.setupRuntime.calls, isEmpty);
      f.setupRuntime.pendingResolve!.complete('/late/smith');
      await tester.pumpAndSettle();
      expect(f.setupRuntime.calls, isEmpty);
      expect(f.runtime.launches, 0);
      await f.close(tester);
    },
  );
  for (final preflight in [true, false]) {
    testWidgets(
      '${preflight ? 'preflight' : 'post-approval'} terminal error retains sanitized code/message and disclaimer',
      (tester) async {
        final f = WizardFixture()..inventory();
        await f.mount(tester);
        await tap(tester, 'Select a model');
        await tap(tester, 'Optional · download another model');
        await tester.enterText(
          find.byKey(const Key('download-model')),
          'operator:exact',
        );
        f.setupRuntime.script = (child, _) {
          final error =
              fixture.terminalEvent(seq: preflight ? 0 : 1, status: 'error')
                ..['code'] = 'provider_unavailable'
                ..['message'] =
                    'Provider prerequisite failed at /private/fixture-cache.';
          child.errors.add(utf8.encode('private raw stderr diagnostic'));
          if (preflight) {
            child.emit(error);
            child.finish(1);
          } else {
            child.emit(fixture.approvalEvent(model: 'operator:exact'));
            child.onApproval = () {
              child.emit(error);
              child.finish(1);
            };
          }
        };
        await tap(tester, 'Review model download');
        if (!preflight) await tap(tester, 'Approve download');
        expect(
          find.textContaining(
            'provider_unavailable — Provider prerequisite failed',
          ),
          findsOneWidget,
        );
        expect(find.textContaining('[path redacted]'), findsOneWidget);
        expect(find.textContaining('/private/fixture-cache'), findsNothing);
        expect(find.textContaining('private raw stderr'), findsNothing);
        expect(find.textContaining('No automatic retry.'), findsOneWidget);
        expect(
          find.textContaining('cache/daemon outcome may remain unknown'),
          findsOneWidget,
        );
        expect(find.text('Confirm model download'), findsNothing);
        expect(
          f.setupRuntime.children.last.approvals.length,
          preflight ? 0 : 1,
        );
        expect(f.setup.busy, false);
        expect(f.setupRuntime.calls.length, 2);
        expect(f.posts, 0);
        await f.close(tester);
      },
    );
  }
  testWidgets(
    'explicit approval writes once; Cancel preserves external cache warning and never retries',
    (tester) async {
      final f = WizardFixture()..inventory();
      await f.mount(tester);
      await tap(tester, 'Select a model');
      await tap(tester, 'Optional · download another model');
      await tester.enterText(
        find.byKey(const Key('download-model')),
        'operator:exact',
      );
      f.setupRuntime.script = (child, _) {
        child.emit(fixture.approvalEvent(model: 'operator:exact'));
        child.onApproval = () => child.emit(
          fixture.envelope('progress', 1, {'status': 'downloading'}),
        );
      };
      await tap(tester, 'Review model download');
      await tap(tester, 'Approve download');
      expect(f.setupRuntime.children.last.approvals.length, 1);
      expect(find.textContaining('completed unknown'), findsOneWidget);
      await tap(tester, 'Cancel setup operation');
      expect(
        find.textContaining('cache/daemon outcome may remain unknown'),
        findsOneWidget,
      );
      expect(f.setupRuntime.calls.length, 2);
      expect(f.setupRuntime.children.last.approvals.length, 1);
      expect(f.posts, 0);
      await f.close(tester);
    },
  );
  testWidgets('busy chat blocks model selection and downloads', (tester) async {
    final f = WizardFixture()..inventory();
    await f.mount(tester);
    await tap(tester, 'Select a model');
    await tap(tester, 'Optional · download another model');
    f.chat.setTurnBusy(true);
    await tester.pumpAndSettle();
    expect(
      tester
          .widget<DropdownButtonFormField<String>>(
            find.byKey(const Key('installed-model')),
          )
          .onChanged,
      isNull,
    );
    expect(
      tester.widget<TextField>(find.byKey(const Key('download-model'))).enabled,
      false,
    );
    expect(f.setupRuntime.calls.length, 1);
    await f.close(tester);
  });
  testWidgets(
    'packaged first run steps, installed picker and explicit Start only',
    (tester) async {
      final f = WizardFixture()..inventory();
      await f.mount(tester);
      expect(f.setupRuntime.calls.single.skip(1), ['setup', 'inspect']);
      expect(f.runtime.launches, 0);
      expect(f.posts, 0);
      await tap(tester, 'Select a model');
      await tester.tap(find.byKey(const Key('installed-model')));
      await tester.pumpAndSettle();
      await tap(tester, 'fixture:local · 42 B');
      await tap(tester, 'Review Start');
      expect(f.backend.model, 'fixture:local');
      expect(f.runtime.launches, 0);
      f.runtime.afterLaunch = backend_fixture.ready(
        id: backend_fixture.desktopSession,
      );
      await tap(tester, 'Start packaged backend');
      expect(f.runtime.launches, 1);
      expect(find.byType(OfflineChatScreen), findsOneWidget);
      expect(f.posts, 0);
      await f.close(tester);
    },
  );
  testWidgets(
    'missing Ollama prerequisite, explicit inspection retry, enlarged text',
    (tester) async {
      final f = WizardFixture()..inventory(status: 'unavailable');
      await f.mount(tester, scale: 2);
      expect(
        find.textContaining('Provision and run Ollama separately'),
        findsOneWidget,
      );
      expect(find.text('Install Ollama'), findsNothing);
      expect(find.text('Start Ollama'), findsNothing);
      expect(tester.takeException(), isNull);
      f.inventory();
      await tap(tester, 'Retry inspection');
      await tap(tester, 'Select a model');
      expect(find.byKey(const Key('download-model')), findsNothing);
      await tap(tester, 'Optional · download another model');
      expect(find.byKey(const Key('download-model')), findsOneWidget);
      expect(f.runtime.launches, 0);
      expect(f.setupRuntime.calls.every((c) => c[2] == 'inspect'), true);
      expect(tester.takeException(), isNull);
      await f.close(tester);
    },
  );
  testWidgets(
    'invalid discovery never falls back; reviewed advanced path and retry',
    (tester) async {
      final f = WizardFixture()..inventory();
      f.setupRuntime.invalid = true;
      await f.mount(tester);
      expect(f.setupRuntime.calls, isEmpty);
      await tap(tester, 'Advanced · explicit backend path');
      await tester.enterText(
        find.byKey(const Key('backend-executable')),
        '/reviewed/smith',
      );
      f.setupRuntime.invalid = false;
      await tap(tester, 'Retry detection');
      expect(f.setupRuntime.calls.single.first, '/reviewed/smith');
      expect(f.runtime.launches, 0);
      await f.close(tester);
    },
  );
  testWidgets(
    'download disclosure decline sends zero consent and no automatic retry',
    (tester) async {
      final f = WizardFixture()..inventory();
      await f.mount(tester);
      await tap(tester, 'Select a model');
      await tap(tester, 'Optional · download another model');
      await tester.enterText(
        find.byKey(const Key('download-model')),
        'operator:exact',
      );
      f.setupRuntime.script = (child, argv) {
        child.emit(fixture.approvalEvent(model: 'operator:exact'));
      };
      await tap(tester, 'Review model download');
      expect(find.text('Confirm model download'), findsOneWidget);
      expect(find.textContaining('internet'), findsOneWidget);
      expect(find.textContaining('persistent cache'), findsOneWidget);
      expect(f.setupRuntime.children.last.approvals, isEmpty);
      await tap(tester, 'Decline');
      expect(f.setupRuntime.children.last.approvals, isEmpty);
      expect(f.setupRuntime.calls.length, 2);
      expect(f.runtime.launches, 0);
      expect(f.posts, 0);
      await f.close(tester);
    },
  );
  testWidgets(
    'external activation during approval invalidates dialog and consent',
    (tester) async {
      final f = WizardFixture()..inventory();
      await f.mount(tester);
      await tap(tester, 'Select a model');
      await tap(tester, 'Optional · download another model');
      await tester.enterText(
        find.byKey(const Key('download-model')),
        'operator:exact',
      );
      f.setupRuntime.script = (child, _) {
        child.emit(fixture.approvalEvent(model: 'operator:exact'));
      };
      await tap(tester, 'Review model download');
      expect(find.text('Confirm model download'), findsOneWidget);
      f.runtime.result = backend_fixture.ready();
      await f.backend.check();
      await tester.pumpAndSettle();
      expect(find.text('Approve download'), findsNothing);
      expect(f.setupRuntime.children.last.approvals, isEmpty);
      expect(f.runtime.launches, 0);
      await tester.enterText(
        find.byKey(const Key('text-chat-draft')),
        'preserved',
      );
      await tap(tester, 'Backend setup');
      expect(find.text('preserved', skipOffstage: false), findsWidgets);
      await tap(tester, 'Close setup');
      expect(find.text('preserved'), findsOneWidget);
      await f.close(tester);
    },
  );
}
