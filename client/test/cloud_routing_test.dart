import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/app_v2.dart';
import 'package:smith_client/cloud_routing.dart';
import 'package:smith_client/workspace_controller.dart';
import 'package:smith_client/ui/cloud_selector.dart';

import 'app_fakes.dart';

const generation = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
const token = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';
const cloud = ChatRoute('openai', 'gpt-6-astra', generation);
const local = ChatRoute('local', 'local:exact', null);
Map<String, dynamic> provider(String name) => {
  'provider': name,
  'revision': 1,
  'credential_generation': generation,
  'credential_state': 'available',
  'credential_error': null,
  'validation': {
    'request_id': token,
    'model': cloudModels[name]!.first,
    'credential_generation': generation,
    'status': 'succeeded',
    'completed_ms': 1,
    'error': null,
    'delivery': 'received',
  },
};
Map<String, dynamic> catalog() => {
  'config_revision': 1,
  'inventory_observed': true,
  'providers': [provider('openai'), provider('anthropic')],
  'models': [
    {
      'group': 'local',
      'label': 'Local',
      'route': local.toJson(),
      'available': true,
      'unavailable_reason': null,
      'validated_model': false,
    },
    for (final p in cloudModels.entries)
      for (final model in p.value)
        {
          'group': 'cloud',
          'label': model,
          'route': ChatRoute(p.key, model, generation).toJson(),
          'available': true,
          'unavailable_reason': null,
          'validated_model': model == p.value.first,
        },
  ],
};

void setProviderFacts(
  Map<String, dynamic> value,
  String name, {
  String state = 'not_observed',
  String? validationStatus,
}) {
  final p = (value['providers'] as List).firstWhere(
    (p) => p['provider'] == name,
  );
  final configured = state != 'not_configured';
  const codes = {
    'not_observed': 'NOT_OBSERVED',
    'locked': 'KEYRING_LOCKED',
    'missing': 'KEYRING_MISSING',
    'unavailable': 'KEYRING_UNAVAILABLE',
    'conflict': 'KEYRING_CONFLICT',
  };
  p['credential_generation'] = configured ? generation : null;
  p['credential_state'] = state;
  p['credential_error'] = codes.containsKey(state)
      ? {
          'code': codes[state],
          'message': 'Synthetic last credential observation.',
        }
      : null;
  p['validation'] = validationStatus == null
      ? null
      : <String, dynamic>{
          ...provider(name)['validation'],
          'status': validationStatus,
          'error': validationStatus == 'succeeded'
              ? null
              : {
                  'code': 'CLOUD_AUTH',
                  'message': 'Synthetic previous check failed.',
                },
          'delivery': validationStatus == 'interrupted'
              ? 'may_have_sent'
              : 'received',
        };
  for (final model in value['models']) {
    if (model['route']['provider'] != name) continue;
    model['route']['credential_generation'] = p['credential_generation'];
    model['available'] = configured;
    model['unavailable_reason'] = configured
        ? null
        : {
            'code': 'PROVIDER_NOT_CONFIGURED',
            'message': 'Save a key in the installer.',
          };
    model['validated_model'] =
        validationStatus == 'succeeded' &&
        model['route']['model'] == cloudModels[name]!.first;
  }
}

class CloudFake extends FakeAppApi {
  bool approved = false;
  bool loseAck = false;
  final accepted = <String, Map<String, dynamic>>{};
  final lastRoutes = <String, Map<String, dynamic>>{};
  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    if (![
      'route_catalog',
      'conversation_routing',
      'request_routing',
      'route_prepare',
      'submit_routed',
    ].contains(op)) {
      return super.call(op, args);
    }
    calls.add(op);
    arguments.add(Map.of(args));
    dynamic result;
    switch (op) {
      case 'route_catalog':
        result = catalog();
      case 'conversation_routing':
        final id = args['conversation_id'];
        result = {
          'conversation_id': id,
          'history_revision': summaries[id]!['history_revision'],
          'last_route': lastRoutes[id],
          'approved_destinations': [],
          'turn_routes': [
            for (var i = 0; i < histories[id]!.length ~/ 2; i++)
              {'request_id': null, 'route': null},
          ],
        };
      case 'route_prepare':
        result = {
          'request_id': args['request_id'],
          'conversation_id': args['conversation_id'],
          'route': args['route'],
          'config_revision': args['expected_config_revision'],
          'history_revision': args['expected_history_revision'],
          'context_sha256': 'c' * 64,
          'context_messages': 1,
          'context_bytes': utf8.encode(args['text']).length,
          'requires_consent': !approved,
          'approval_token': token,
          'expires_at_ms': 60000,
          'dispatch_expires_at_ms': 600000,
        };
      case 'submit_routed':
        accepted[args['request_id']] = Map.of(args);
        final id =
            args['conversation_id'] ?? 'dddddddddddddddddddddddddddddddd';
        if (!summaries.containsKey(id)) {
          summaries[id] = summary(
            id,
            'New chat',
            1,
            origin: args['request_id'],
          );
          histories[id] = [];
        }
        lastRoutes[id] = args['route'];
        if (args['confirm_transfer'] == true) approved = true;
        jobs[args['request_id']] = {
          'request_id': args['request_id'],
          'conversation_id': id,
          'kind': args['conversation_id'] == null
              ? 'submit_new_routed'
              : 'submit_routed',
          'status': 'running',
          'phase': 'generation',
          'config_revision': 1,
          'result': null,
          'error': null,
        };
        result = brief(jobs[args['request_id']]!);
        if (loseAck) throw const AppFailure('TRANSPORT', 'lost', unknown: true);
      case 'request_routing':
        final a = accepted[args['request_id']]!;
        result = {
          'request_id': args['request_id'],
          'route': a['route'],
          'delivery': a['route']['provider'] == 'local'
              ? 'not_applicable'
              : 'may_have_sent',
          'context_sha256': 'c' * 64,
          'dispatch_expires_at_ms': a['route']['provider'] == 'local'
              ? null
              : 600000,
        };
    }
    return AppReply(instance, Map<String, dynamic>.from(result));
  }

  void complete(String request) {
    final a = accepted[request]!;
    final job = jobs[request]!;
    final id = job['conversation_id'];
    histories[id]!.addAll([
      {'role': 'user', 'content': a['text']},
      {'role': 'assistant', 'content': 'answer'},
    ]);
    summaries[id]!['history_revision']++;
    job.addAll({
      'status': 'succeeded',
      'phase': 'complete',
      'result': {
        'assistant_text': 'answer',
        'audio': null,
        'speech_error': null,
      },
    });
  }
}

class CatalogFake extends CloudFake {
  final catalogValue = catalog();
  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    if (op == 'route_catalog') {
      calls.add(op);
      arguments.add(Map.of(args));
      return AppReply(instance, catalogValue);
    }
    return super.call(op, args);
  }
}

Future<WorkspaceController> readyCloud(CloudFake api) async {
  api.inventoryObserved = true;
  final w = WorkspaceController(
    api,
    routingEnabled: true,
    pollInterval: const Duration(days: 1),
  );
  await w.synchronize();
  expect(w.stale, false, reason: w.error);
  return w;
}

Future<void> settleQueue() async {
  for (var i = 0; i < 20; i++) {
    await Future<void>.delayed(Duration.zero);
  }
}

void main() {
  for (final name in cloudModels.keys) {
    for (final state in [
      'not_observed',
      'locked',
      'missing',
      'unavailable',
      'conflict',
    ]) {
      for (final status in <String?>[null, 'failed', 'interrupted']) {
        test(
          '$name $state / $status permits a fresh explicit send without preliminary work',
          () async {
            final api = CatalogFake();
            setProviderFacts(
              api.catalogValue,
              name,
              state: state,
              validationStatus: status,
            );
            final beforeFacts = jsonEncode(api.catalogValue);
            final w = await readyCloud(api);
            addTearDown(w.dispose);
            final c = w.selected;
            final calls = List.of(api.calls);
            c.selectRoute(
              ChatRoute(name, cloudModels[name]!.first, generation),
            );
            c.edit('explicit attempt');
            expect(c.canSend, true);
            expect(
              api.calls,
              calls,
              reason: 'Selection must not probe or validate.',
            );
            var confirmations = 0;
            await c.sendWithConsent((p) async {
              confirmations++;
              expect(p.route, c.selectedRoute);
              return TransferDecision.send;
            });
            expect(confirmations, 1);
            expect(api.accepted, hasLength(1));
            expect(
              api.calls.where((op) => op == 'route_prepare'),
              hasLength(1),
            );
            expect(
              api.calls.where((op) => op == 'submit_routed'),
              hasLength(1),
            );
            expect(api.calls, isNot(contains('provider_probe')));
            expect(api.calls, isNot(contains('provider_prepare')));
            expect(api.calls, isNot(contains('provider_apply')));
            expect(jsonEncode(api.catalogValue), beforeFacts);
          },
        );
      }
    }
    test(
      '$name without a saved generation stays disabled without losing text/history',
      () async {
        final api = CatalogFake();
        setProviderFacts(api.catalogValue, name, state: 'not_configured');
        final w = await readyCloud(api);
        addTearDown(w.dispose);
        final c = w.selected;
        final history = List.of(c.history);
        c.selectRoute(ChatRoute(name, cloudModels[name]!.first, null));
        c.edit('preserved text');
        expect(c.canSend, false);
        await c.sendWithConsent(
          (_) => throw StateError('Unconfigured route cannot prepare consent.'),
        );
        expect(c.history, history);
        expect(c.draft, 'preserved text');
        expect(api.accepted, isEmpty);
        expect(api.calls, isNot(contains('route_prepare')));
      },
    );
  }
  test('Resume explicitly rebinds queued text to approved destination, never accepted work', () async {
    final api = CloudFake()..approved = true;
    final w = await readyCloud(api);
    addTearDown(w.dispose);
    final c = w.selected;
    c.edit('accepted local');
    await c.send();
    final first = c.pendingId!;
    c.edit('queued under local');
    c.messageQueue.enqueue();
    c.selectRoute(cloud);
    c.edit('newer composer');
    api.complete(first);
    await w.synchronize();
    await settleQueue();
    expect(api.accepted, hasLength(1));
    expect(c.messageQueue.paused, true);
    expect(c.messageQueue.entries.single.status, contains('LOCAL'));
    await c.messageQueue.resume(c.messageQueue.revision);
    await settleQueue();
    expect(api.accepted, hasLength(2));
    expect(api.accepted[first]!['route'], local.toJson());
    expect(api.accepted.values.last['route'], cloud.toJson());
    expect(api.accepted.values.last['text'], 'queued under local');
    expect(api.accepted.values.last['confirm_transfer'], false);
    expect(c.draft, 'newer composer');
  });
  test(
    'strict catalog rejects omitted nullable keys, extras, unreviewed models',
    () {
      RouteCatalog(catalog());
      for (final key in catalog().keys) {
        final v = catalog()..remove(key);
        expect(() => RouteCatalog(v), throwsA(anything));
      }
      expect(
        () => RouteCatalog(catalog()..['extra'] = null),
        throwsA(anything),
      );
      final v = catalog();
      v['models'][1]['route']['model'] = 'invented';
      expect(() => RouteCatalog(v), throwsA(anything));
    },
  );
  test('schema3 delegates unchanged shapes and preserves v2 strict kinds', () {
    final state = FakeAppApi().state;
    validateV3('state', state);
    validateV2('state', state);
    final job = {
      'request_id': token,
      'conversation_id': testConversation,
      'kind': 'submit_routed',
      'status': 'queued',
      'phase': 'queued',
      'config_revision': '18446744073709551615',
    };
    operationV3(job);
    expect(() => operation(job), throwsA(isA<AppFailure>()));
    expect(
      () => operationV3({...job, 'extra': null}),
      throwsA(isA<AppFailure>()),
    );
  });
  test('Boot reads no keyring; decline preserves text and history; same destination skips dialog', () async {
    final api = CloudFake();
    final w = await readyCloud(api);
    addTearDown(w.dispose);
    expect(api.calls, isNot(contains('provider_probe')));
    final c = w.selected;
    c.selectRoute(cloud);
    c.edit('private');
    await c.sendWithConsent((p) async {
      expect(p.disclosure, contains('charges'));
      return TransferDecision.cancel;
    });
    expect(c.draft, 'private');
    expect(api.accepted, isEmpty);
    expect(c.localSends, isEmpty);
    api.approved = true;
    await c.sendWithConsent((_) => throw StateError('redundant dialog'));
    expect(api.accepted.length, 1);
    expect(api.accepted.values.single['confirm_transfer'], false);
  });
  test(
    'route or draft changes during consent invalidate frozen send',
    () async {
      final api = CloudFake();
      final w = await readyCloud(api);
      addTearDown(w.dispose);
      final c = w.selected;
      c.selectRoute(cloud);
      c.edit('original');
      final dialog = Completer<TransferDecision>();
      final shown = Completer<void>();
      final send = c.sendWithConsent((_) {
        shown.complete();
        return dialog.future;
      });
      await shown.future;
      c.edit('new draft');
      c.selectRoute(local);
      dialog.complete(TransferDecision.send);
      await send;
      expect(api.accepted, isEmpty);
      expect(c.draft, 'new draft');
    },
  );
  test('accepted route survives selection change and lost acknowledgement recovery', () async {
    final api = CloudFake()..loseAck = true;
    final w = await readyCloud(api);
    addTearDown(w.dispose);
    final c = w.selected;
    c.selectRoute(cloud);
    c.edit('hello');
    await c.sendWithConsent((_) async => TransferDecision.send);
    final request = c.pendingId!;
    c.selectRoute(local);
    await w.synchronize();
    expect(api.accepted.length, 1);
    expect(c.selectedRoute, local);
    expect(c.requestRoutes[request]!.route, cloud);
    expect(
      c.requestRoutes[request]!.label,
      contains('billing may have occurred'),
    );
  });
  test(
    'queued destination cannot silently reroute or bypass first consent',
    () async {
      final api = CloudFake();
      final w = await readyCloud(api);
      addTearDown(w.dispose);
      final c = w.selected;
      c.edit('first');
      await c.send();
      final first = c.pendingId!;
      c.edit('next');
      c.messageQueue.enqueue();
      c.selectRoute(cloud);
      api.complete(first);
      await w.synchronize();
      await settleQueue();
      expect(api.accepted.length, 1);
      expect(c.messageQueue.paused, true);
      await c.messageQueue.resume(c.messageQueue.revision);
      await settleQueue();
      expect(api.accepted.length, 1);
      expect(c.messageQueue.paused, true);
      await w.reviewQueuedCloud(c, (_) async => TransferDecision.send);
      expect(api.accepted.length, 2);
      expect(api.accepted.values.last['route'], cloud.toJson());
      expect(api.accepted.values.last['confirm_transfer'], true);
    },
  );
  test(
    'new chat defaults local and unavailable selection is retained',
    () async {
      final api = CloudFake();
      final w = await readyCloud(api);
      addTearDown(w.dispose);
      w.selected.selectRoute(cloud);
      w.newChat();
      expect(w.selected.selectedRoute, local);
      final old = const ChatRoute(
        'openai',
        'gpt-6-astra',
        'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',
      );
      w.selected.selectRoute(old);
      w.selected.edit('keep');
      await w.synchronize();
      expect(w.selected.selectedRoute, old);
      expect(w.selected.draft, 'keep');
      expect(w.selected.canSend, false);
    },
  );
  testWidgets(
    'selector groups local/cloud and remains usable during pending work',
    (tester) async {
      final api = CloudFake();
      final w = await readyCloud(api);
      final c = w.selected;
      c.edit('first');
      await c.send();
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(body: CloudSelector(controller: c, active: true)),
        ),
      );
      await tester.tap(find.byKey(const ValueKey('chat-route-selector')));
      await tester.pumpAndSettle();
      expect(find.textContaining('CLOUD'), findsWidgets);
      expect(find.textContaining('LOCAL'), findsWidgets);
      await tester.enterText(
        find.byKey(const ValueKey('chat-route-filter')),
        'gpt-6-astra',
      );
      await tester.ensureVisible(find.byKey(const ValueKey(cloud)));
      await tester.tap(find.byKey(const ValueKey(cloud)));
      await tester.pumpAndSettle();
      expect(c.selectedRoute, cloud);
      expect(api.accepted.values.single['route'], local.toJson());
      await tester.pumpWidget(const SizedBox());
      w.dispose();
    },
  );
}
