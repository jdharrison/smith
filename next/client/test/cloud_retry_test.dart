import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/cloud_routing.dart';
import 'package:smith_client/workspace_controller.dart';

import 'cloud_routing_test.dart';

class ProbeFake extends CloudFake {
  final observed = <String>{};
  String observationState = 'available';
  String? validationStatus = 'succeeded';
  bool failProbe = false;

  Map<String, dynamic> facts(String name) {
    final value = catalog();
    setProviderFacts(
      value,
      name,
      state: observed.contains(name) ? observationState : 'not_observed',
      validationStatus: validationStatus,
    );
    return (value['providers'] as List).firstWhere(
      (p) => p['provider'] == name,
    );
  }

  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    if (op == 'route_catalog') {
      calls.add(op);
      arguments.add(Map.of(args));
      final v = catalog();
      for (final name in ['openai', 'anthropic']) {
        setProviderFacts(
          v,
          name,
          state: observed.contains(name) ? observationState : 'not_observed',
          validationStatus: validationStatus,
        );
      }
      return AppReply(instance, v);
    }
    if (op == 'provider_probe') {
      calls.add(op);
      arguments.add(Map.of(args));
      observed.add(args['provider']);
      final job = <String, dynamic>{
        'request_id': args['request_id'],
        'conversation_id': null,
        'kind': op,
        'status': failProbe ? 'failed' : 'succeeded',
        'phase': 'complete',
        'config_revision': 1,
        'result': failProbe ? null : {'provider': facts(args['provider'])},
        'error': failProbe
            ? {'code': 'KEYRING_TIMEOUT', 'message': 'Synthetic failed probe.'}
            : null,
      };
      jobs[args['request_id']] = job;
      return AppReply(instance, brief(job));
    }
    return super.call(op, args);
  }
}

class RejectProbeFake extends ProbeFake {
  String? rejection;
  bool lostAck = false;
  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    if (op == 'provider_probe' && rejection != null) {
      calls.add(op);
      arguments.add(Map.of(args));
      throw AppFailure(rejection!, 'Synthetic admission rejection');
    }
    final reply = await super.call(op, args);
    if (op == 'provider_probe' && lostAck) {
      throw const AppFailure(
        'TRANSPORT',
        'Synthetic lost acknowledgement',
        unknown: true,
      );
    }
    return reply;
  }
}

void main() {
  for (final name in ['openai', 'anthropic']) {
    for (final state in ['locked', 'missing', 'unavailable', 'conflict']) {
      test(
        '$name: completed $state probe and failed validation leave a new send eligible',
        () async {
          final api = ProbeFake()
            ..observationState = state
            ..validationStatus = 'failed';
          final w = await readyCloud(api);
          addTearDown(w.dispose);
          await w.retryCloudProviders();
          final c = w.selected;
          final route = w.routeCatalog!.choices
              .firstWhere((choice) => choice.route.provider == name)
              .route;
          c.selectRoute(route);
          c.edit('explicit after repair');
          expect(c.canSend, true);
          final p = w.routeCatalog!.providers.firstWhere(
            (p) => p['provider'] == name,
          );
          expect(p['credential_state'], state);
          expect(p['validation']['status'], 'failed');
          final probes = api.calls.where((op) => op == 'provider_probe').length;
          api.observationState = 'available';
          await c.sendWithConsent((_) async => TransferDecision.send);
          expect(api.accepted, hasLength(1));
          expect(
            api.calls.where((op) => op == 'provider_probe'),
            hasLength(probes),
          );
          expect(api.calls, isNot(contains('provider_apply')));
          expect(
            p['credential_state'],
            state,
            reason: 'No preliminary observation rewrites the cached fact.',
          );
        },
      );
    }
    test(
      '$name: terminal failed probe releases reservation but never retries automatically',
      () async {
        final api = ProbeFake()
          ..observationState = 'unavailable'
          ..validationStatus = null
          ..failProbe = true;
        final w = await readyCloud(api);
        addTearDown(w.dispose);
        await w.retryCloudProviders();
        w.selected.selectRoute(
          w.routeCatalog!.choices
              .firstWhere((c) => c.route.provider == name)
              .route,
        );
        w.selected.edit('fresh attempt');
        expect(w.selected.canSend, true);
        expect(api.calls.where((op) => op == 'provider_probe'), hasLength(2));
        await w.synchronize();
        expect(w.selected.canSend, true);
        expect(api.calls.where((op) => op == 'provider_probe'), hasLength(2));
        await w.selected.sendWithConsent((_) async => TransferDecision.send);
        expect(api.accepted, hasLength(1));
        expect(api.calls.where((op) => op == 'provider_probe'), hasLength(2));
      },
    );
  }
  for (final code in ['BUSY', 'STALE_REVISION']) {
    test(
      '$code before probe admission releases reservation for local send and Retry',
      () async {
        final api = RejectProbeFake()..rejection = code;
        final w = await readyCloud(api);
        addTearDown(w.dispose);
        await w.retryCloudProviders();
        w.selected.edit('local after rejection');
        expect(w.selected.canSend, true);
        await w.selected.send();
        final request = w.selected.pendingId!;
        api.complete(request);
        await w.synchronize();
        api.rejection = null;
        await w.retryCloudProviders();
        expect(api.observed, {'openai', 'anthropic'});
        expect(api.calls.where((v) => v == 'provider_probe'), hasLength(3));
      },
    );
  }
  for (final code in ['STATE_UNAVAILABLE', 'TRANSPORT']) {
    test(
      '$code probe uncertainty retains reservation without automatic reissue',
      () async {
        final api = RejectProbeFake()..rejection = code;
        final w = await readyCloud(api);
        addTearDown(w.dispose);
        await w.retryCloudProviders();
        w.selected.selectRoute(cloud);
        w.selected.edit('do not send');
        expect(w.selected.routeReady, true);
        expect(w.selected.canSend, false);
        await w.retryCloudProviders();
        await w.synchronize();
        expect(api.calls.where((v) => v == 'provider_probe'), hasLength(1));
        expect(api.calls.where((v) => v == 'poll'), isNotEmpty);
      },
    );
  }
  test(
    'lost probe acknowledgement retains exact ID and recovers by poll',
    () async {
      final api = RejectProbeFake()..lostAck = true;
      final w = await readyCloud(api);
      addTearDown(w.dispose);
      w.selected.selectRoute(cloud);
      w.selected.edit('later');
      expect(w.selected.canSend, true);
      await w.retryCloudProviders();
      expect(w.selected.canSend, false);
      final request =
          api.arguments[api.calls.indexOf('provider_probe')]['request_id'];
      await w.synchronize();
      expect(api.arguments[api.calls.indexOf('poll')]['request_id'], request);
      expect(api.calls.where((v) => v == 'provider_probe'), hasLength(1));
      expect(w.selected.canSend, true);
    },
  );
  test(
    'only explicit Retry probes configured providers; no inference or save',
    () async {
      final api = ProbeFake();
      final w = await readyCloud(api);
      addTearDown(w.dispose);
      expect(api.observed, isEmpty);
      w.selected.selectRoute(cloud);
      w.selected.edit('already eligible');
      expect(w.selected.canSend, true);
      await w.synchronize();
      expect(api.observed, isEmpty);
      expect(w.selected.canSend, true);
      await w.retryCloudProviders();
      expect(api.observed, {'openai', 'anthropic'});
      expect(api.calls.where((op) => op == 'provider_probe').length, 2);
      expect(
        api.calls.where(
          (op) =>
              op.contains('submit') ||
              op == 'provider_apply' ||
              op == 'provider_prepare',
        ),
        isEmpty,
      );
      w.selected.selectRoute(cloud);
      w.selected.edit('ready');
      expect(w.selected.canSend, true);
    },
  );
}
