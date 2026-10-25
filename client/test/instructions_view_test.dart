import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/cloud_routing.dart';
import 'package:smith_client/instructions.dart';
import 'package:smith_client/instructions_controller.dart';
import 'package:smith_client/ui/instructions_view.dart';
import 'package:smith_client/ui/settings_view.dart';
import 'package:smith_client/ui/smith_theme.dart';

import 'instructions_controller_test.dart' show InstructionApi;
import 'instructions_protocol_test.dart' show instructionTestRoute;

class _ReadGateApi extends InstructionApi {
  Completer<void>? readGate;
  int readsStarted = 0;

  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    if (op == 'instructions_get') {
      readsStarted++;
      await readGate?.future;
    }
    return super.call(op, args);
  }
}

class ReviewCountingController extends InstructionsController {
  ReviewCountingController(super.api, {super.onApplied});
  int reviews = 0;

  @override
  InstructionReview? createReview() {
    reviews++;
    return super.createReview();
  }
}

Widget instructionsHarness(
  InstructionsController controller, {
  ChatRoute? route = instructionTestRoute,
  double scale = 1,
}) => MaterialApp(
  theme: SmithTheme.dark(),
  builder: (context, child) => MediaQuery(
    data: MediaQuery.of(context).copyWith(textScaler: TextScaler.linear(scale)),
    child: child!,
  ),
  home: Scaffold(
    body: SettingsView(
      connectionStatus: 'Observed connection',
      modelStatus: 'Observed model',
      version: 'Fixture',
      instructions: InstructionsView(controller: controller, route: route),
    ),
  ),
);

Finder instructionField(String name) => find.byKey(ValueKey(name));
Finder get reviewAction => find.byKey(const ValueKey('instructions-review'));
Finder get applyAction => find.byKey(const ValueKey('instructions-apply'));

Future<void> tapInstructionAction(WidgetTester tester, Finder target) async {
  await tester.ensureVisible(target);
  await tester.pumpAndSettle();
  await tester.tap(target);
  await tester.pumpAndSettle();
}

Future<void> editInstructionField(
  WidgetTester tester,
  String name,
  String text,
) async {
  final field = instructionField(name);
  await tester.ensureVisible(field);
  await tester.enterText(field, text);
  await tester.pumpAndSettle();
}

void main() {
  testWidgets(
    'inspector labels the observed route across panes and failed refresh',
    (tester) async {
      final api = InstructionApi();
      final c = InstructionsController(api);
      addTearDown(c.dispose);
      await tester.pumpWidget(instructionsHarness(c));
      await tester.pumpAndSettle();
      await tapInstructionAction(
        tester,
        find.byKey(const ValueKey('instructions-inspector')),
      );
      expect(find.text('Model: local:exact'), findsOneWidget);

      // Another pane observes the shared, vessel-free prefix.
      await c.refresh();
      await tester.pumpAndSettle();
      expect(c.snapshotRoute, isNull);
      expect(find.text('Model: Not selected'), findsOneWidget);
      expect(find.text('Model: local:exact'), findsNothing);
      expect(
        find.byKey(const ValueKey('instructions-route-mismatch')),
        findsOneWidget,
      );

      // A failed read must not relabel the retained observation.
      api.getError = 'STORAGE';
      await c.refresh(route: instructionTestRoute);
      await tester.pumpAndSettle();
      expect(c.snapshotRoute, isNull);
      expect(find.text('Model: Not selected'), findsOneWidget);

      api.getError = null;
      await c.refresh(route: instructionTestRoute);
      await tester.pumpAndSettle();
      expect(c.snapshotRoute, instructionTestRoute);
      expect(find.text('Model: local:exact'), findsOneWidget);
      expect(
        find.byKey(const ValueKey('instructions-route-mismatch')),
        findsNothing,
      );
      expect(api.calls.every((op) => op == 'instructions_get'), true);
    },
  );

  testWidgets('initial loading disables editing without writes or inference', (
    tester,
  ) async {
    final api = _ReadGateApi()..readGate = Completer<void>();
    final c = ReviewCountingController(api);
    addTearDown(c.dispose);
    await tester.pumpWidget(instructionsHarness(c));
    await tester.pumpAndSettle();
    expect(api.readsStarted, 1);
    expect(c.loading, true);
    expect(c.snapshot, isNull);
    expect(
      find.textContaining('Loading saved global instructions'),
      findsOneWidget,
    );
    expect(
      tester.widget<TextField>(instructionField('global-instructions')).enabled,
      false,
    );
    expect(tester.widget<OutlinedButton>(reviewAction).onPressed, isNull);
    expect(c.reviews, 0);
    api.readGate!.complete();
    await tester.pumpAndSettle();
    expect(c.loading, false);
    expect(c.snapshot!.configRevision, 1);
    expect(api.calls, ['instructions_get']);
  });

  testWidgets(
    'acknowledgement alone never displays the proposed revision as applied',
    (tester) async {
      final api = _ReadGateApi();
      var synchronized = 0;
      final c = ReviewCountingController(
        api,
        onApplied: () async {
          synchronized++;
        },
      );
      addTearDown(c.dispose);
      await tester.pumpWidget(instructionsHarness(c));
      await tester.pumpAndSettle();
      await editInstructionField(
        tester,
        'global-instructions',
        'await saved observation',
      );
      api.readGate = Completer<void>();
      await tapInstructionAction(tester, reviewAction);
      await tapInstructionAction(tester, applyAction);
      expect(api.calls, ['instructions_get', 'instructions_update']);
      expect(api.readsStarted, 2);
      expect(api.receipts, hasLength(1));
      expect(api.revision, 2);
      expect(c.busy, true);
      expect(c.snapshot!.configRevision, 1);
      expect(find.text('Last observed applied revision: 2'), findsNothing);
      expect(synchronized, 0);
      api.readGate!.complete();
      await tester.pumpAndSettle();
      expect(c.busy, false);
      expect(c.snapshot!.configRevision, 2);
      expect(find.text('Last observed applied revision: 2'), findsOneWidget);
      expect(synchronized, 1);
    },
  );

  for (final unknown in [false, true]) {
    testWidgets(
      '400x400 at 2x preserves ${unknown ? 'unknown Apply' : 'BUSY'} recovery',
      (tester) async {
        await tester.binding.setSurfaceSize(const Size(400, 400));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final api = InstructionApi()
          ..lostAck = unknown
          ..refusal = unknown ? null : 'BUSY';
        final c = ReviewCountingController(api);
        addTearDown(c.dispose);
        await tester.pumpWidget(instructionsHarness(c, scale: 2));
        await tester.pumpAndSettle();
        await editInstructionField(
          tester,
          'global-instructions',
          'preserve exact input\nthrough recovery',
        );
        await tapInstructionAction(tester, reviewAction);
        await tapInstructionAction(tester, applyAction);
        final request = c.pendingRequestId;
        final error = find.byKey(const ValueKey('instructions-error'));
        await Scrollable.ensureVisible(tester.element(error), alignment: 1);
        await tester.pumpAndSettle();
        expect(tester.getRect(error).bottom, lessThanOrEqualTo(400));
        final action = find.text(unknown ? 'Check apply status' : 'Refresh');
        await tester.ensureVisible(action);
        await tester.pumpAndSettle();
        expect(
          tester
              .getSize(
                find.ancestor(of: action, matching: find.byType(TextButton)),
              )
              .height,
          greaterThanOrEqualTo(48),
        );
        if (unknown) api.receiptError = 'REQUEST_UNKNOWN';
        await tapInstructionAction(tester, action);
        expect(c.draft.instructions, 'preserve exact input\nthrough recovery');
        expect(
          api.calls.where((op) => op == 'instructions_update'),
          hasLength(1),
        );
        expect(c.uncertain, unknown);
        expect(c.pendingRequestId, request);
        expect(tester.takeException(), isNull);
      },
    );
  }

  testWidgets(
    'opening reads once; fields and inspector are inert and model-independent',
    (tester) async {
      final api = InstructionApi();
      final c = ReviewCountingController(api);
      addTearDown(c.dispose);
      await tester.pumpWidget(instructionsHarness(c));
      await tester.pumpAndSettle();
      expect(api.calls, ['instructions_get']);
      expect(c.reviews, 0);
      expect(c.draft.cloudEnabled, false);
      expect(find.text('Vessel: chat::global::assistant'), findsOneWidget);
      expect(find.textContaining('Do not enter secrets'), findsOneWidget);

      await tapInstructionAction(
        tester,
        find.text('Applied preamble inspector'),
      );
      for (final text in [
        'Applied revision: 1',
        'Renderer source/version: smith-context-v1',
        'Core identity/safety source: smith-core-v1',
        'Product knowledge source: smith-product-v1',
        'Provider: local',
        'Model: local:exact',
      ]) {
        await tester.ensureVisible(find.text(text));
        expect(find.text(text), findsOneWidget);
      }
      expect(
        tester
            .widget<SelectableText>(
              find.byKey(const ValueKey('instructions-preamble')),
            )
            .data,
        c.snapshot!.local.text,
      );
      expect(
        find.textContaining('not your draft, the exact future conversation'),
        findsOneWidget,
      );
      expect(find.textContaining(c.snapshot!.local.sha256), findsWidgets);
      expect(api.calls, ['instructions_get']);

      const cloud = ChatRoute(
        'openai',
        'gpt-6-astra',
        'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
      );
      await tester.pumpWidget(instructionsHarness(c, route: cloud));
      await tester.pumpAndSettle();
      expect(c.snapshot!.vessel, instructionVessel);
      expect(find.text('Provider: openai'), findsOneWidget);
      expect(find.text('Model: gpt-6-astra'), findsOneWidget);
      expect(find.text('Vessel: chat::global::assistant'), findsOneWidget);
      expect(api.calls, ['instructions_get', 'instructions_get']);
      expect(c.reviews, 0);
      await tester.pumpWidget(const SizedBox());
      expect(api.disposals, 0);
    },
  );

  testWidgets(
    'three exact fields share UTF8 bound; clear and discard only edit drafts',
    (tester) async {
      final api = InstructionApi()
        ..settings = const InstructionSettings(instructions: 'saved');
      final c = ReviewCountingController(api);
      addTearDown(c.dispose);
      await tester.pumpWidget(instructionsHarness(c));
      await tester.pumpAndSettle();
      await editInstructionField(
        tester,
        'global-instructions',
        '  exact\n\ttext  ',
      );
      await editInstructionField(tester, 'global-personality', 'kind');
      await editInstructionField(tester, 'global-guardrails', 'ask first');
      expect(c.draft.instructions, '  exact\n\ttext  ');
      expect(c.draft.personality, 'kind');
      expect(c.draft.guardrails, 'ask first');
      expect(
        find.textContaining('cloud sends are blocked, not stripped'),
        findsOneWidget,
      );
      await tapInstructionAction(
        tester,
        find.byKey(const ValueKey('instructions-cloud-sharing')),
      );
      expect(c.draft.cloudEnabled, true);
      expect(
        find.textContaining('Allowing sharing is not Send consent'),
        findsOneWidget,
      );
      expect(api.calls, ['instructions_get']);

      await editInstructionField(tester, 'global-instructions', 'é' * 513);
      expect(c.draft.personalBytes, greaterThan(1024));
      expect(
        find.byKey(const ValueKey('instructions-validation')),
        findsOneWidget,
      );
      expect(tester.widget<OutlinedButton>(reviewAction).onPressed, isNull);
      await tapInstructionAction(tester, find.text('Clear draft'));
      expect(c.draft, const InstructionSettings());
      await tapInstructionAction(tester, find.text('Discard edits'));
      expect(c.draft, api.settings);
      expect(api.calls, ['instructions_get']);
      expect(c.reviews, 0);
    },
  );

  testWidgets(
    'review Cancel never binds; Apply waits for authoritative observation',
    (tester) async {
      final api = InstructionApi();
      var synchronized = 0;
      final c = ReviewCountingController(
        api,
        onApplied: () async {
          synchronized++;
        },
      );
      addTearDown(c.dispose);
      await tester.pumpWidget(instructionsHarness(c));
      await tester.pumpAndSettle();
      await editInstructionField(
        tester,
        'global-instructions',
        'exact proposed text',
      );
      await tapInstructionAction(tester, reviewAction);
      expect(find.text('Currently applied · revision 1'), findsOneWidget);
      expect(find.text('Proposed draft · not applied'), findsOneWidget);
      expect(find.text('exact proposed text'), findsWidgets);
      expect(
        find.textContaining('accepted work is queued or running'),
        findsOneWidget,
      );
      expect(c.reviews, 0);
      await tapInstructionAction(tester, find.text('Cancel'));
      expect(c.reviews, 0);
      expect(api.calls, ['instructions_get']);

      api.gate = Completer<void>();
      await tapInstructionAction(tester, reviewAction);
      await tapInstructionAction(tester, applyAction);
      expect(c.reviews, 1);
      expect(c.busy, true);
      expect(c.snapshot!.configRevision, 1);
      expect(synchronized, 0);
      expect(
        find.textContaining('Awaiting authoritative settings'),
        findsOneWidget,
      );
      expect(
        tester
            .widget<TextField>(instructionField('global-instructions'))
            .enabled,
        false,
      );
      api.gate!.complete();
      await tester.pumpAndSettle();
      expect(c.snapshot!.configRevision, 2);
      expect(c.snapshot!.settings.instructions, 'exact proposed text');
      expect(c.dirty, false);
      expect(synchronized, 1);
      expect(find.text('Last observed applied revision: 2'), findsOneWidget);
      expect(api.calls, [
        'instructions_get',
        'instructions_update',
        'instructions_get',
      ]);
      expect(api.arguments[1]['settings'], c.snapshot!.settings.toJson());
    },
  );

  testWidgets(
    'conflict compares applied and preserved draft before explicit rebinding',
    (tester) async {
      final api = InstructionApi()
        ..settings = const InstructionSettings(instructions: 'old saved');
      final c = ReviewCountingController(api);
      addTearDown(c.dispose);
      await tester.pumpWidget(instructionsHarness(c));
      await tester.pumpAndSettle();
      await editInstructionField(
        tester,
        'global-instructions',
        'my preserved draft',
      );
      api.revision = 2;
      api.settings = const InstructionSettings(
        instructions: 'other window saved',
      );
      await tapInstructionAction(tester, find.text('Refresh'));
      expect(c.conflict, true);
      expect(c.draftConfigRevision, 1);
      await tapInstructionAction(tester, reviewAction);
      expect(c.reviews, 0);
      expect(c.conflict, true);
      expect(
        find.byKey(const ValueKey('instructions-conflict-review')),
        findsOneWidget,
      );
      expect(find.text('Currently applied · revision 2'), findsOneWidget);
      expect(find.text('other window saved'), findsOneWidget);
      expect(find.text('my preserved draft'), findsWidgets);
      await tapInstructionAction(tester, find.text('Cancel'));
      expect(c.conflict, true);
      expect(c.reviews, 0);
      await tapInstructionAction(tester, reviewAction);
      await tapInstructionAction(tester, applyAction);
      expect(c.reviews, 1);
      expect(c.snapshot!.configRevision, 3);
      expect(
        api.arguments.lastWhere(
          (a) => a.containsKey('settings'),
        )['expected_config_revision'],
        2,
      );
      expect(api.settings.instructions, 'my preserved draft');
    },
  );

  testWidgets(
    'stale dialog callbacks cannot apply newer input or a refreshed snapshot',
    (tester) async {
      final api = InstructionApi();
      final c = ReviewCountingController(api);
      addTearDown(c.dispose);
      await tester.pumpWidget(instructionsHarness(c));
      await tester.pumpAndSettle();
      await editInstructionField(tester, 'global-instructions', 'reviewed');
      await tapInstructionAction(tester, reviewAction);
      final staleApply = tester.widget<TextButton>(applyAction).onPressed!;
      c.edit(c.draft.copyWith(instructions: 'newer unreviewed'));
      await c.refresh(route: instructionTestRoute);
      await tester.pumpAndSettle();
      expect(tester.widget<TextButton>(applyAction).onPressed, isNull);
      staleApply();
      await tester.pumpAndSettle();
      expect(api.calls.where((op) => op == 'instructions_update'), isEmpty);
      expect(c.draft.instructions, 'newer unreviewed');
      expect(c.reviews, 0);
    },
  );

  testWidgets(
    'unknown Apply recovers only by original receipt; no repeated write',
    (tester) async {
      final api = InstructionApi()..lostAck = true;
      final c = ReviewCountingController(api);
      addTearDown(c.dispose);
      await tester.pumpWidget(instructionsHarness(c));
      await tester.pumpAndSettle();
      await editInstructionField(
        tester,
        'global-instructions',
        'saved despite lost acknowledgement',
      );
      await tapInstructionAction(tester, reviewAction);
      await tapInstructionAction(tester, applyAction);
      final request = c.pendingRequestId;
      expect(c.uncertain, true);
      expect(c.draft.instructions, 'saved despite lost acknowledgement');
      expect(tester.widget<OutlinedButton>(reviewAction).onPressed, isNull);
      await tapInstructionAction(tester, find.text('Check apply status'));
      expect(c.uncertain, false);
      expect(c.snapshot!.configRevision, 2);
      expect(
        api.calls.where((op) => op == 'instructions_update'),
        hasLength(1),
      );
      expect(api.arguments[2], {'request_id': request});
      expect(api.calls, [
        'instructions_get',
        'instructions_update',
        'instructions_receipt',
        'instructions_get',
      ]);
    },
  );

  testWidgets(
    'BUSY and failed refresh preserve drafts and require deliberate recovery',
    (tester) async {
      final api = InstructionApi()..refusal = 'BUSY';
      final c = ReviewCountingController(api);
      addTearDown(c.dispose);
      await tester.pumpWidget(instructionsHarness(c));
      await tester.pumpAndSettle();
      await editInstructionField(
        tester,
        'global-instructions',
        'keep through BUSY',
      );
      await tapInstructionAction(tester, reviewAction);
      await tapInstructionAction(tester, applyAction);
      expect(find.textContaining('Wait for it to finish'), findsOneWidget);
      expect(c.draft.instructions, 'keep through BUSY');
      expect(c.uncertain, false);
      api.getError = 'TRANSPORT';
      await tapInstructionAction(tester, find.text('Refresh'));
      expect(c.draft.instructions, 'keep through BUSY');
      expect(find.textContaining('edits are preserved'), findsOneWidget);
      expect(
        api.calls.where((op) => op == 'instructions_update'),
        hasLength(1),
      );
      api.getError = null;
      api.refusal = null;
      await tapInstructionAction(tester, find.text('Refresh'));
      expect(c.canApply, true);
      expect(
        api.calls.where((op) => op == 'instructions_update'),
        hasLength(1),
      );
    },
  );

  testWidgets(
    '400x400 at 2x keeps editor, review, privacy and inspector reachable',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(400, 400));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final api = InstructionApi();
      final c = ReviewCountingController(api);
      addTearDown(c.dispose);
      await tester.pumpWidget(instructionsHarness(c, scale: 2));
      await tester.pumpAndSettle();
      await editInstructionField(
        tester,
        'global-instructions',
        'first line\nsecond line',
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pumpAndSettle();
      expect(api.calls, ['instructions_get']);
      await tapInstructionAction(tester, reviewAction);
      final dialog = find.byType(AlertDialog);
      for (final target in [
        find.text('Proposed draft · not applied'),
        find.textContaining('Local-only personal text').last,
        find.textContaining('Core safety and permissions stay code-owned'),
        applyAction,
      ]) {
        await tester.ensureVisible(target);
        await tester.pumpAndSettle();
        expect(
          tester
              .getRect(target)
              .overlaps(const Offset(0, 0) & const Size(400, 400)),
          true,
        );
        expect(tester.takeException(), isNull);
      }
      expect(tester.getSize(applyAction).height, greaterThanOrEqualTo(48));
      expect(dialog, findsOneWidget);
      await tapInstructionAction(tester, find.text('Cancel'));
      await tapInstructionAction(
        tester,
        find.text('Applied preamble inspector'),
      );
      await tester.ensureVisible(find.textContaining('Cloud export eligible:'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(c.draft.instructions, 'first line\nsecond line');
      expect(api.calls, ['instructions_get']);
      expect(c.reviews, 0);
    },
  );
}
