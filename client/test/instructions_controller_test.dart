import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/instructions.dart';
import 'package:smith_client/instructions_controller.dart';

import 'instructions_protocol_test.dart'
    show instructionFixture, instructionTestInstance, instructionTestRoute;

class InstructionApi implements AppApi {
  final calls = <String>[];
  final arguments = <Map<String, dynamic>>[];
  final receipts = <String, Map<String, dynamic>>{};
  InstructionSettings settings = const InstructionSettings();
  Object revision = 1;
  String instance = instructionTestInstance;
  bool lostAck = false;
  String ackError = 'TRANSPORT';
  String? getError;
  Object? forcedReceiptRevision;
  bool wrongReceiptId = false;
  String? refusal, receiptError;
  Completer<void>? gate;
  int disposals = 0;
  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    calls.add(op);
    arguments.add(args);
    if (op == 'instructions_get') {
      if (getError != null) {
        throw AppFailure(getError!, 'Synthetic read failure');
      }
      return AppReply(
        instance,
        instructionFixture(
          revision: revision,
          settings: settings,
          route: args['route'] == null ? null : instructionTestRoute,
        ),
      );
    }
    if (op == 'instructions_receipt') {
      if (receiptError != null) {
        throw AppFailure(receiptError!, 'Synthetic receipt refusal');
      }
      return AppReply(instance, receipts[args['request_id']]!);
    }
    if (op == 'instructions_update') {
      if (refusal != null) {
        throw AppFailure(refusal!, 'Synthetic admission refusal');
      }
      final next = InstructionSettings.parse(args['settings']);
      final changed = next != settings;
      if (changed) revision = (revision as int) + 1;
      settings = next;
      final receipt = {
        'request_id': wrongReceiptId ? 'f' * 32 : args['request_id'],
        'config_revision': forcedReceiptRevision ?? revision,
        'changed': changed,
      };
      receipts[args['request_id']] = receipt;
      await gate?.future;
      if (lostAck) {
        throw AppFailure(
          ackError,
          'Synthetic acknowledgement failure',
          unknown: ackError == 'TRANSPORT',
        );
      }
      return AppReply(instance, receipt);
    }
    throw const AppFailure('OP', 'Unsupported service');
  }

  @override
  void dispose() {
    disposals++;
  }
}

void main() {
  test('receipt revision mismatch stays write-blocked', () async {
    final api = InstructionApi()..forcedReceiptRevision = 1;
    final c = InstructionsController(api);
    await c.refresh();
    c.edit(c.draft.copyWith(instructions: 'saved'));
    await c.apply(review: c.createReview()!);
    expect(c.uncertain, true);
    await c.resolvePending();
    expect(c.uncertain, true);
    expect(c.canApply, false);
    expect(api.calls.where((op) => op == 'instructions_update').length, 1);
    c.dispose();
  });

  test(
    'malformed acknowledgement remains uncertain and never retries a write',
    () async {
      final api = InstructionApi()..wrongReceiptId = true;
      final c = InstructionsController(api);
      await c.refresh();
      c.edit(c.draft.copyWith(instructions: 'preserved'));
      await c.apply(review: c.createReview()!);
      expect(c.uncertain, true);
      final id = c.pendingRequestId;
      await c.resolvePending();
      expect(c.uncertain, true);
      expect(c.pendingRequestId, id);
      expect(api.calls.where((op) => op == 'instructions_update').length, 1);
      c.dispose();
    },
  );

  test(
    'storage uncertainty recovers via receipt and observation across restart',
    () async {
      final api = InstructionApi()
        ..lostAck = true
        ..ackError = 'STATE_UNAVAILABLE';
      final c = InstructionsController(api);
      await c.refresh();
      c.edit(c.draft.copyWith(instructions: 'saved'));
      await c.apply(review: c.createReview()!);
      api.instance = 'c' * 32;
      await c.resolvePending();
      expect(c.uncertain, false);
      expect(c.instanceId, api.instance);
      expect(c.draft.instructions, 'saved');
      expect(c.conflict, false);
      c.dispose();
    },
  );

  test('acknowledged write with unavailable get remains pending', () async {
    final api = InstructionApi()..gate = Completer<void>();
    final c = InstructionsController(api);
    await c.refresh();
    c.edit(c.draft.copyWith(instructions: 'saved'));
    final action = c.apply(review: c.createReview()!);
    api.getError = 'STORAGE';
    api.gate!.complete();
    await action;
    expect(c.uncertain, true);
    expect(c.draft.instructions, 'saved');
    api.getError = null;
    await c.resolvePending();
    expect(c.uncertain, false);
    expect(api.calls.where((op) => op == 'instructions_update').length, 1);
    c.dispose();
  });

  test('maximum u64 snapshot is readable but cannot apply', () async {
    final api = InstructionApi()..revision = '18446744073709551615';
    final c = InstructionsController(api);
    await c.refresh();
    c.edit(c.draft.copyWith(instructions: 'draft'));
    expect(c.configRevision, '18446744073709551615');
    expect(c.canApply, false);
    expect(api.calls, ['instructions_get']);
    c.dispose();
  });

  test(
    'dispose from loading notification fences API and does not own API',
    () async {
      final api = InstructionApi();
      final c = InstructionsController(api);
      c.addListener(() {
        if (c.loading) c.dispose();
      });
      await c.refresh();
      expect(api.calls, isEmpty);
      expect(api.disposals, 0);
    },
  );

  test('unsupported get has no fallback or implicit refresh retry', () async {
    final api = InstructionApi()..getError = 'OP';
    final c = InstructionsController(api);
    await c.refresh();
    expect(c.snapshot, null);
    expect(c.error, contains('compatible paired service'));
    expect(api.calls, ['instructions_get']);
    expect(c.canApply, false);
    c.dispose();
  });

  test('opening, editing, reviewing, discarding have no write or inference effects', () async {
    final api = InstructionApi();
    final c = InstructionsController(api);
    await c.refresh();
    c.edit(c.draft.copyWith(instructions: 'draft'));
    expect(c.dirty, true);
    expect(c.canApply, true);
    expect(c.createReview(), isNotNull);
    c.discard();
    expect(c.dirty, false);
    expect(api.calls, ['instructions_get']);
    c.dispose();
    expect(api.disposals, 0);
  });

  test(
    'dirty draft survives route refresh, config drift requires explicit review',
    () async {
      final api = InstructionApi();
      final c = InstructionsController(api);
      await c.refresh();
      c.edit(c.draft.copyWith(instructions: 'preserved'));
      final stale = c.createReview()!;
      await c.refresh(route: instructionTestRoute);
      expect(c.draft.instructions, 'preserved');
      expect(c.conflict, false);
      api.revision = 2;
      api.settings = const InstructionSettings(personality: 'other window');
      await c.refresh();
      expect(c.draft.instructions, 'preserved');
      expect(c.draftConfigRevision, 1);
      expect(c.conflict, true);
      expect(c.canApply, false);
      await c.apply(review: stale);
      expect(api.calls.where((s) => s == 'instructions_update'), isEmpty);
      final reviewed = c.createReview()!;
      expect(c.conflict, false);
      await c.apply(review: reviewed);
      expect(api.settings.instructions, 'preserved');
      expect(c.dirty, false);
      expect(c.configRevision, 3);
      c.dispose();
    },
  );

  test(
    'review binds exact draft and originating controller and instance',
    () async {
      final api = InstructionApi();
      final c = InstructionsController(api);
      await c.refresh();
      c.edit(c.draft.copyWith(instructions: 'first'));
      final review = c.createReview()!;
      c.edit(c.draft.copyWith(instructions: 'new typing'));
      await c.apply(review: review);
      expect(api.calls, ['instructions_get']);
      api.instance = 'b' * 32;
      await c.refresh();
      expect(c.conflict, true);
      await c.apply(review: review);
      expect(api.calls.where((s) => s == 'instructions_update'), isEmpty);
      c.discard();
      expect(c.conflict, false);
      expect(c.draft.instructions, '');
      c.dispose();
    },
  );

  test(
    'BUSY rejects definitively, preserves draft and requires fresh review',
    () async {
      final api = InstructionApi()..refusal = 'BUSY';
      final c = InstructionsController(api);
      await c.refresh();
      c.edit(c.draft.copyWith(guardrails: 'retained'));
      await c.apply(review: c.createReview()!);
      expect(c.uncertain, false);
      expect(c.dirty, true);
      expect(c.error, contains('queued or running'));
      expect(c.canApply, false);
      expect(api.calls, ['instructions_get', 'instructions_update']);
      c.dispose();
    },
  );

  test(
    'lost ack recovers original receipt and current settings without replay',
    () async {
      var synchronized = 0;
      final api = InstructionApi()..lostAck = true;
      final c = InstructionsController(
        api,
        onApplied: () async {
          synchronized++;
        },
      );
      await c.refresh();
      c.edit(c.draft.copyWith(instructions: 'applied'));
      await c.apply(review: c.createReview()!);
      final request = c.pendingRequestId;
      expect(c.uncertain, true);
      expect(c.canApply, false);
      c.edit(c.draft.copyWith(personality: 'new typing'));
      await c.resolvePending();
      expect(api.arguments[2]['request_id'], request);
      expect(api.calls, [
        'instructions_get',
        'instructions_update',
        'instructions_receipt',
        'instructions_get',
      ]);
      expect(c.uncertain, false);
      expect(c.draft.personality, 'new typing');
      expect(c.snapshot!.settings.personality, '');
      expect(c.conflict, true);
      expect(synchronized, 1);
      c.dispose();
    },
  );

  for (final code in ['REQUEST_UNKNOWN', 'REQUEST_EXPIRED']) {
    test(
      '$code leaves original identity write-blocked despite matching current settings',
      () async {
        final api = InstructionApi()..lostAck = true;
        final c = InstructionsController(api);
        await c.refresh();
        c.edit(c.draft.copyWith(instructions: 'applied'));
        await c.apply(review: c.createReview()!);
        final id = c.pendingRequestId;
        api.receiptError = code;
        await c.resolvePending();
        expect(c.snapshot!.settings.instructions, 'applied');
        expect(c.uncertain, true);
        expect(c.pendingRequestId, id);
        expect(c.canApply, false);
        expect(c.createReview(), null);
        c.discard();
        expect(c.dirty, true);
        await c.refresh();
        expect(c.uncertain, true);
        expect(api.calls.where((s) => s == 'instructions_update').length, 1);
        c.dispose();
      },
    );
  }

  test('newer typing while Apply awaits ack survives reconciliation', () async {
    final api = InstructionApi()..gate = Completer<void>();
    final c = InstructionsController(api);
    await c.refresh();
    c.edit(c.draft.copyWith(instructions: 'applied'));
    final apply = c.apply(review: c.createReview()!);
    expect(c.busy, true);
    c.edit(c.draft.copyWith(instructions: 'typed later'));
    api.gate!.complete();
    await apply;
    expect(c.draft.instructions, 'typed later');
    expect(c.snapshot!.settings.instructions, 'applied');
    expect(c.dirty, true);
    expect(c.conflict, true);
    c.dispose();
  });

  test('unsupported service exposes error without fallback', () async {
    final api = InstructionApi()..refusal = 'OP';
    final c = InstructionsController(api);
    await c.refresh();
    c.edit(c.draft.copyWith(instructions: 'kept'));
    await c.apply(review: c.createReview()!);
    expect(c.uncertain, false);
    expect(c.error, contains('compatible paired service'));
    expect(api.calls, ['instructions_get', 'instructions_update']);
    c.dispose();
  });

  test('successful apply invokes synchronization once; callback failure is not unknown apply', () async {
    final api = InstructionApi();
    final c = InstructionsController(
      api,
      onApplied: () async {
        throw StateError('sync');
      },
    );
    await c.refresh();
    c.edit(c.draft.copyWith(instructions: 'kept'));
    await c.apply(review: c.createReview()!);
    expect(c.uncertain, false);
    expect(c.error, contains('chat synchronization failed'));
    expect(c.dirty, false);
    c.dispose();
  });
}
