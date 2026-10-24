import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/cloud_routing.dart';

import 'cloud_routing_test.dart' show CloudFake, cloud, readyCloud;
import 'message_queue_controller_test.dart'
    show QueueApi, ready, start, enqueue, settle;

class LocalOnlyCloudApi extends CloudFake {
  bool refusePrepare = true;
  bool refuseSend = true;
  int preparationRefusals = 0, sendRefusals = 0;
  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    if (op == 'route_prepare' && refusePrepare) {
      preparationRefusals++;
      throw const AppFailure(
        'INSTRUCTIONS_LOCAL_ONLY',
        'Personal instructions are local-only.',
      );
    }
    if (op == 'submit_routed' && refuseSend) {
      sendRefusals++;
      throw const AppFailure(
        'INSTRUCTIONS_LOCAL_ONLY',
        'Personal instructions are local-only.',
      );
    }
    return super.call(op, args);
  }
}

void main() {
  test('queued local-only refusal preserves actionable reason without unknown acceptance', () async {
    final api = LocalOnlyCloudApi()
      ..refusePrepare = false
      ..refuseSend = false;
    final w = await readyCloud(api);
    final c = w.selected;
    c.selectRoute(cloud);
    c.edit('predecessor');
    await c.sendWithConsent((_) async => TransferDecision.send);
    enqueue(c, 'queued private text');
    api.refusePrepare = true;
    api.complete(c.pendingId!);
    await w.synchronize();
    await settle();
    expect(api.accepted.length, 1);
    expect(c.messageQueue.paused, true);
    expect(c.messageQueue.pauseReason, contains('Enable cloud sharing'));
    expect(c.messageQueue.entries.single.text, 'queued private text');
    expect(c.pendingId, null);
    expect(c.uncertain, false);
  });

  test(
    'global config drift latches queued followups and Resume uses new fence',
    () async {
      final api = QueueApi();
      final w = await ready(api);
      final c = w.selected;
      await start(c);
      enqueue(c, 'followup');
      api.complete(c.pendingId!);
      // Same model, newly applied instructions revision.
      api.config = {...api.config, 'revision': 2};
      await w.synchronize();
      await settle();
      expect(api.sends, hasLength(1));
      expect(c.messageQueue.paused, true);
      expect(c.messageQueue.pauseReason, contains('global instructions'));
      final stale = c.messageQueue.revision;
      c.messageQueue.pause();
      await c.messageQueue.resume(stale);
      await settle();
      expect(api.sends, hasLength(1));
      await c.messageQueue.resume(c.messageQueue.revision);
      await settle();
      expect(api.sends, hasLength(2));
      expect(api.sends.last['expected_config_revision'], 2);
      expect(api.sends.last['text'], 'followup');
    },
  );

  test(
    'local-only cloud preparation is actionable, unsent, and never unknown',
    () async {
      final api = LocalOnlyCloudApi();
      final w = await readyCloud(api);
      final c = w.selected;
      c.selectRoute(cloud);
      c.edit('private draft');
      var confirmations = 0;
      await c.sendWithConsent((_) async {
        confirmations++;
        return TransferDecision.send;
      });
      expect(api.preparationRefusals, 1);
      expect(api.sendRefusals, 0);
      expect(confirmations, 0);
      expect(c.pendingId, null);
      expect(c.uncertain, false);
      expect(c.draft, 'private draft');
      expect(c.error, contains('Enable cloud sharing'));
    },
  );

  test(
    'definitive local-only admission refusal releases observation identity',
    () async {
      final api = LocalOnlyCloudApi()..refusePrepare = false;
      final w = await readyCloud(api);
      final c = w.selected;
      c.selectRoute(cloud);
      c.edit('explicit send');
      await c.sendWithConsent((_) async => TransferDecision.send);
      expect(api.sendRefusals, 1);
      expect(c.pendingId, null);
      expect(c.uncertain, false);
      expect(c.error, contains('Nothing was sent'));
      expect(c.localSends.values.single.active, false);
      expect(c.localSends.values.single.status, contains('Not sent'));
    },
  );

  test('consent names all context categories and counts preamble budget', () {
    final preparation = RoutePreparation({
      'request_id': 'a' * 32,
      'conversation_id': null,
      'route': cloud.toJson(),
      'config_revision': 2,
      'history_revision': null,
      'context_sha256': 'b' * 64,
      'context_messages': 33,
      'context_bytes': 83968,
      'requires_consent': true,
      'approval_token': 'c' * 32,
      'expires_at_ms': 1,
      'dispatch_expires_at_ms': 2,
    });
    for (final category in [
      'core',
      'product',
      'global instructions',
      'guardrails',
      'vessel',
      'history',
      'current message',
      'including the instruction preamble',
      'renewed consent',
    ]) {
      expect(preparation.disclosure, contains(category));
    }
    expect(
      () => RoutePreparation({...preparation.value, 'context_bytes': 83969}),
      throwsA(isA<AppFailure>()),
    );
  });
}
