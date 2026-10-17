import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/cloud_routing.dart';
import 'package:smith_client/message_queue.dart';
import 'package:smith_client/workspace_controller.dart';

import 'cloud_catalog_expansion_test.dart' show expectedCloudModels;

const bound = Duration(seconds: 5);

class RustCloudFixture implements AppExecutor {
  RustCloudFixture(this.root, this.child) {
    void capture(List<int> data) {
      if (output.length + data.length > 65536) {
        child.kill(ProcessSignal.sigkill);
        throw StateError('Fixture output exceeded bound');
      }
      output.addAll(data);
    }

    child.stdout.listen(capture);
    child.stderr.listen(capture);
  }
  final Directory root;
  final Process child;
  final output = <int>[];
  final submissions = <Map<String, dynamic>>[];
  final requests = <String>[];
  bool loseNextAdmission = false;
  String get socket => '${root.path}/app.sock';
  String get diagnostics => utf8.decode(output, allowMalformed: true);
  static Future<RustCloudFixture> start(
    String executable, {
    bool savedOnly = false,
  }) async {
    final root = await Directory.systemTemp.createTemp('smith-cloud-dart-');
    final mode = await Process.run('/usr/bin/chmod', [
      '0700',
      root.path,
    ]).timeout(bound);
    expect(mode.exitCode, 0);
    final child = await Process.start(
      executable,
      ['--exact', 'dart_cloud::dart_cloud_fixture', '--ignored', '--nocapture'],
      includeParentEnvironment: false,
      environment: {
        'SMITH_DART_CLOUD_ROOT': root.path,
        if (savedOnly) 'SMITH_DART_CLOUD_SAVED_ONLY': '1',
      },
    );
    final fixture = RustCloudFixture(root, child);
    final clock = Stopwatch()..start();
    while (!await File(fixture.socket).exists()) {
      if (clock.elapsed > const Duration(seconds: 10)) {
        await fixture.close();
        throw StateError(
          'Rust fixture did not become ready: ${fixture.diagnostics}',
        );
      }
      await Future<void>.delayed(const Duration(milliseconds: 10));
    }
    return fixture;
  }

  Future<String> exchangeRaw(List<int> frame) async {
    final peer = await Socket.connect(
      InternetAddress(socket, type: InternetAddressType.unix),
      0,
    ).timeout(bound);
    try {
      final response = peer
          .cast<List<int>>()
          .transform(utf8.decoder)
          .transform(const LineSplitter())
          .first
          .timeout(bound);
      peer.add(frame);
      await peer.flush().timeout(bound);
      return await response;
    } finally {
      peer.destroy();
    }
  }

  @override
  Future<String> exchange(List<int> frame) async {
    final request = jsonDecode(utf8.decode(frame)) as Map<String, dynamic>;
    expect(request['schema_version'], 3);
    requests.add(request['op'] as String);
    if (request['op'] == 'submit_routed') {
      submissions.add(Map<String, dynamic>.from(request['args']));
    }
    final response = await exchangeRaw(frame);
    if (request['op'] == 'submit_routed' && loseNextAdmission) {
      loseNextAdmission = false;
      throw const AppFailure(
        'TRANSPORT',
        'Synthetic lost acknowledgement after Rust admission',
        unknown: true,
      );
    }
    return response;
  }

  Future<Map<String, dynamic>> control(
    String name, [
    Map<String, dynamic> args = const {},
  ]) async => Map<String, dynamic>.from(
    jsonDecode(
      await exchangeRaw(
        utf8.encode('${jsonEncode({'fixture': name, ...args})}\n'),
      ),
    ),
  );
  // Installer-only protocol actions stay in this synthetic fixture, not NativeAppApi.
  Future<AppReply> providerCall(String op, Map<String, dynamic> args) async {
    expect(['provider_prepare', 'provider_apply'], contains(op));
    requests.add(op);
    final call = appId();
    return AppReply.parse(
      await exchangeRaw(
        utf8.encode(
          '${jsonEncode({'schema_version': 3, 'service': 'smith-next', 'protocol': 'app', 'call_id': call, 'op': op, 'args': args})}\n',
        ),
      ),
      call,
      schemaVersion: 3,
    );
  }

  @override
  void dispose() {} // Workspace detach must not terminate the fixture service.
  Future<void> close() async {
    try {
      if (await File(socket).exists()) await control('shutdown');
      final code = await child.exitCode.timeout(bound);
      expect(code, 0, reason: diagnostics);
    } finally {
      child.kill(ProcessSignal.sigkill);
      await child.exitCode.timeout(bound);
      await root.delete(recursive: true);
    }
  }
}

Future<void> observeUntil(WorkspaceController w, bool Function() done) async {
  final clock = Stopwatch()..start();
  while (!done()) {
    if (clock.elapsed > bound) {
      fail(
        'Timed out: history=${w.selected.history.length}, pending=${w.selected.pendingId}, routeReady=${w.selected.routeReady}; ${w.error}; ${w.selected.error}; ${w.selected.messageQueue.pauseReason}',
      );
    }
    await w.synchronize();
    await Future<void>.delayed(const Duration(milliseconds: 10));
  }
}

Future<Map<String, dynamic>> validateSaved(
  WorkspaceController w,
  RustCloudFixture fixture,
  RouteChoice choice,
) async {
  final request = appId();
  final p = w.routeCatalog!.providers.firstWhere(
    (p) => p['provider'] == choice.route.provider,
  );
  final prepared = (await fixture.providerCall('provider_prepare', {
    'request_id': request,
    'instance_id': w.instanceId,
    'provider': choice.route.provider,
    'action': 'validate',
    'model': choice.route.model,
    'expected_provider_revision': p['revision'],
  })).result;
  expect(prepared['request_id'], request);
  expect(prepared['provider'], choice.route.provider);
  expect(prepared['model'], choice.route.model);
  expect(prepared['credential_generation'], choice.route.generation);
  expect(prepared['approval_token'], isA<String>());
  expect(prepared['expires_at_ms'], isA<int>());
  final admission = (await fixture.providerCall('provider_apply', {
    'request_id': request,
    'instance_id': w.instanceId,
    'approval_token': prepared['approval_token'],
    'api_key': null,
  })).result;
  operationV3(admission);
  expect(admission['request_id'], request);
  expect(admission['kind'], 'provider_validate');
  final clock = Stopwatch()..start();
  while (true) {
    expect(clock.elapsed, lessThan(bound));
    final result = (await w.api.call('poll', {
      'request_id': request,
      'instance_id': w.instanceId,
    })).result;
    if (result['status'] == 'running') {
      await Future<void>.delayed(const Duration(milliseconds: 10));
      continue;
    }
    final routing = RequestRouting(
      (await w.api.call('request_routing', {
        'request_id': request,
        'instance_id': w.instanceId,
      })).result,
    );
    expect(routing.route, choice.route);
    expect(routing.delivery, 'received');
    return result;
  }
}

void main() {
  final executable = Platform.environment['SMITH_TEST_CLOUD_SERVICE'];
  final required = Platform.environment.containsKey('SMITH_TEST_SERVICE');
  final skipFixture = executable == null && !required
      ? 'Run make client/test for mandatory compiled schema3 fixture.'
      : false;

  test(
    'compiled saved-only cold credentials allow both vendors to Send without probe or validation',
    () async {
      expect(executable, isNotNull);
      final fixture = await RustCloudFixture.start(
        executable!,
        savedOnly: true,
      );
      addTearDown(fixture.close);
      final w = WorkspaceController(
        NativeAppApi(executor: fixture),
        pollInterval: const Duration(milliseconds: 20),
      );
      addTearDown(w.dispose);
      await w.synchronize();
      expect(w.stale, false, reason: w.error);
      final originalInstance = w.instanceId;
      final generations = [
        for (final p in w.routeCatalog!.providers) p['credential_generation'],
      ];
      expect(
        w.routeCatalog!.providers.every((p) => p['validation'] == null),
        true,
      );
      await fixture.control('restart');
      await w.synchronize();
      expect(w.stale, false, reason: w.error);
      expect(w.instanceId, isNot(originalInstance));
      expect([
        for (final p in w.routeCatalog!.providers) p['credential_generation'],
      ], generations);
      expect(
        w.routeCatalog!.providers.every(
          (p) =>
              p['credential_state'] == 'not_observed' &&
              p['validation'] == null,
        ),
        true,
      );
      expect(
        w.routeCatalog!.choices
            .where((c) => c.route.cloud)
            .every((c) => c.available && !c.validated),
        true,
      );
      var stats = await fixture.control('stats');
      expect(stats['key_reads'], 0);
      expect(stats['http_calls'], 0);
      for (final name in cloudModels.keys) {
        final choice = w.routeCatalog!.choices.firstWhere(
          (c) => c.route.provider == name,
        );
        final c = w.selected;
        c.selectRoute(choice.route);
        c.edit('explicit cold $name');
        expect(
          c.canSend,
          true,
          reason:
              '$name routeReady=${c.routeReady}, connected=${c.connected}, stale=${c.stale}, detailStale=${c.detailStale}, busy=${c.busy}, pending=${c.pendingId}, queue=${c.messageQueue.pauseReason}, entries=${c.messageQueue.entries.length}; ${w.error}; ${c.error}',
        );
        final before = (await fixture.control('stats'))['http_calls'];
        await c.sendWithConsent((p) async {
          expect(p.route, choice.route);
          expect(p.requiresConsent, true);
          return TransferDecision.send;
        });
        await observeUntil(
          w,
          () =>
              c.pendingId == null &&
              c.history.any((m) => m['content'] == 'explicit cold $name'),
        );
        // A timer poll can clear pending before its detail/queue reconciliation
        // finishes. Drain that workflow before the next route-changing gesture.
        await w.synchronize();
        stats = await fixture.control('stats');
        expect(stats['http_calls'], before + 1);
        expect(stats['key_reads'], before + 1);
        expect(
          w.routeCatalog!.providers.every((p) => p['validation'] == null),
          true,
        );
        expect(
          w.routeCatalog!.choices
              .where((c) => c.route.cloud)
              .every((c) => !c.validated),
          true,
        );
      }
      expect(fixture.submissions, hasLength(2));
      expect(stats['http_calls'], 2);
      expect(stats['key_reads'], 2);
      expect(fixture.requests, isNot(contains('provider_probe')));
      expect(fixture.requests, isNot(contains('provider_prepare')));
      expect(fixture.requests, isNot(contains('provider_apply')));
      await w.synchronize();
      expect((await fixture.control('stats'))['http_calls'], 2);
    },
    skip: skipFixture,
    timeout: const Timeout(Duration(seconds: 45)),
  );

  for (final name in cloudModels.keys) {
    test(
      'compiled $name saved-only cold Validate needs no prior observation or inference',
      () async {
        expect(executable, isNotNull);
        final fixture = await RustCloudFixture.start(
          executable!,
          savedOnly: true,
        );
        addTearDown(fixture.close);
        await fixture.control('restart');
        final w = WorkspaceController(
          NativeAppApi(executor: fixture),
          pollInterval: const Duration(milliseconds: 20),
        );
        addTearDown(w.dispose);
        await w.synchronize();
        expect(w.stale, false, reason: w.error);
        final choice = w.routeCatalog!.choices.firstWhere(
          (c) => c.route.provider == name,
        );
        expect(choice.available && !choice.validated, true);
        expect(
          w.routeCatalog!.providers.every(
            (p) =>
                p['credential_state'] == 'not_observed' &&
                p['validation'] == null,
          ),
          true,
        );
        final initial = await fixture.control('stats');
        expect(initial['key_reads'], 0);
        expect(initial['http_calls'], 0);
        final result = await validateSaved(w, fixture, choice);
        expect(result['status'], 'succeeded');
        await w.synchronize();
        expect(w.selected.history, isEmpty);
        final choices = w.routeCatalog!.choices.where(
          (c) => c.route.provider == name,
        );
        expect(choices.every((c) => c.available), true);
        expect(choices.where((c) => c.validated).map((c) => c.route), [
          choice.route,
        ]);
        expect(
          w.routeCatalog!.providers.firstWhere(
            (p) => p['provider'] != name,
          )['validation'],
          isNull,
        );
        final stats = await fixture.control('stats');
        expect(stats['key_reads'], 1);
        expect(stats['http_calls'], 1);
        expect(fixture.submissions, isEmpty);
        expect(fixture.requests, isNot(contains('provider_probe')));
        expect(
          fixture.requests.where((op) => op == 'provider_prepare'),
          hasLength(1),
        );
        expect(
          fixture.requests.where((op) => op == 'provider_apply'),
          hasLength(1),
        );
      },
      skip: skipFixture,
      timeout: const Timeout(Duration(seconds: 45)),
    );

    test(
      'compiled $name failed validation/send report real errors; fresh explicit send rereads without retry or probe',
      () async {
        expect(executable, isNotNull);
        final fixture = await RustCloudFixture.start(
          executable!,
          savedOnly: true,
        );
        addTearDown(fixture.close);
        await fixture.control('restart');
        final w = WorkspaceController(
          NativeAppApi(executor: fixture),
          pollInterval: const Duration(milliseconds: 20),
        );
        addTearDown(w.dispose);
        await w.synchronize();
        expect(w.stale, false, reason: w.error);
        final choice = w.routeCatalog!.choices.firstWhere(
          (c) => c.route.provider == name,
        );
        await fixture.control('fail_http');
        final failed = await validateSaved(w, fixture, choice);
        expect(failed['status'], 'failed');
        expect(failed['error']['code'], 'CLOUD_UNAVAILABLE');
        await w.synchronize();
        final validation = w.routeCatalog!.providers.firstWhere(
          (p) => p['provider'] == name,
        )['validation'];
        expect(validation['status'], 'failed');
        expect(validation['error']['code'], 'CLOUD_UNAVAILABLE');
        expect(validation['delivery'], 'received');
        expect(w.selected.history, isEmpty);
        final c = w.selected;
        c.selectRoute(choice.route);
        c.edit('explicit failing send');
        expect(c.canSend, true);
        await fixture.control('fail_http');
        await c.sendWithConsent((_) async => TransferDecision.send);
        await observeUntil(w, () => c.pendingId == null);
        expect(c.error, contains('CLOUD_UNAVAILABLE'));
        expect(c.history, isEmpty);
        expect(fixture.submissions, hasLength(1));
        for (var i = 0; i < 3; i++) {
          await w.synchronize();
        }
        var stats = await fixture.control('stats');
        expect(stats['key_reads'], 2);
        expect(stats['http_calls'], 2);
        expect(
          w.routeCatalog!.choices
              .where((c) => c.route.provider == name)
              .every((c) => c.available && !c.validated),
          true,
        );
        c.edit('new explicit recovery');
        expect(c.canSend, true);
        await c.sendWithConsent(
          (_) => throw StateError(
            'Same destination retains prior explicit consent.',
          ),
        );
        await observeUntil(
          w,
          () => c.pendingId == null && c.history.length == 2,
        );
        stats = await fixture.control('stats');
        expect(stats['key_reads'], 3);
        expect(stats['http_calls'], 3);
        expect(fixture.submissions, hasLength(2));
        expect(
          fixture.submissions.first['request_id'],
          isNot(fixture.submissions.last['request_id']),
        );
        expect(fixture.requests, isNot(contains('provider_probe')));
        expect(
          fixture.requests.where((op) => op == 'provider_prepare'),
          hasLength(1),
        );
        expect(
          fixture.requests.where((op) => op == 'provider_apply'),
          hasLength(1),
        );
        expect(
          w.routeCatalog!.providers.firstWhere(
            (p) => p['provider'] == name,
          )['validation'],
          validation,
        );
      },
      skip: skipFixture,
      timeout: const Timeout(Duration(seconds: 45)),
    );
  }
  test(
    'compiled Rust schema3 routed local/cloud, consent, expiry, reattach and queue retry',
    () async {
      expect(
        executable,
        isNotNull,
        reason: 'Dagger must supply the compiled fake-injected Rust fixture, not silently skip schema3 coverage.',
      );
      final fixture = await RustCloudFixture.start(executable!);
      addTearDown(fixture.close);
      var w = WorkspaceController(
        NativeAppApi(executor: fixture),
        pollInterval: const Duration(milliseconds: 20),
      );
      addTearDown(() => w.dispose());
      await w.synchronize();
      expect(w.stale, false, reason: w.error);
      expect(w.routingEnabled, true);
      final expanded = RouteCatalog(
        (await w.api.call('route_catalog', {})).result,
      );
      expect(cloudModels, expectedCloudModels);
      expect(expanded.choices.where((c) => c.route.cloud), hasLength(15));
      for (final entry in expectedCloudModels.entries) {
        final choices = expanded.choices.where(
          (c) => c.route.provider == entry.key,
        );
        expect(choices.map((c) => c.route.model), entry.value);
        for (final choice in choices) {
          expect(
            ChatRoute.parse(choice.route.toJson(), catalog: true),
            choice.route,
          );
        }
      }
      final initial = await fixture.control('stats');
      expect(initial['http_calls'], 0);
      expect(initial['key_reads'], 0);
      var c = w.selected;
      final local = c.selectedRoute!;
      final cloud = w.routeCatalog!.choices
          .firstWhere((v) => v.route.provider == 'openai')
          .route;
      c.edit('local first');
      await c.send();
      await observeUntil(w, () => c.pendingId == null && c.history.length == 2);
      final conversation = c.id!;
      expect(c.routing!.turns.single, local);

      await fixture.control('hold', {'value': true});
      c.selectRoute(cloud);
      c.edit('cloud with local history');
      var consentCount = 0;
      await c.sendWithConsent((p) async {
        consentCount++;
        expect(p.requiresConsent, true);
        expect(p.value['context_messages'], 3);
        return TransferDecision.send;
      });
      expect(consentCount, 1);
      final cloudRequest = c.pendingId!;
      c.selectRoute(local);
      expect(c.requestRoutes[cloudRequest]!.route, cloud);
      expect(fixture.submissions.last['confirm_transfer'], true);

      // A new production adapter reattaches while the original job is running.
      w.dispose();
      w = WorkspaceController(
        NativeAppApi(executor: fixture),
        pollInterval: const Duration(milliseconds: 20),
      );
      await w.synchronize();
      c = w.selected;
      expect(c.id, conversation);
      expect(c.pendingId, cloudRequest);
      expect(c.selectedRoute, cloud);
      expect(c.requestRoutes[cloudRequest]!.route, cloud);
      expect(c.requestRoutes[cloudRequest]!.delivery, 'may_have_sent');
      await fixture.control('hold', {'value': false});
      await observeUntil(w, () => c.pendingId == null && c.history.length == 4);
      expect(c.routing!.turns, [local, cloud]);
      final args = <String, dynamic>{
        'request_id': appId(),
        'instance_id': w.instanceId,
        'conversation_id': c.id,
        'text': 'expired synthetic request',
        'route': cloud.toJson(),
        'expected_config_revision': w.config['revision'],
        'expected_history_revision': c.summary!['history_revision'],
      };
      final prepared = RoutePreparation(
        (await w.api.call('route_prepare', args)).result,
      );
      expect(prepared.requiresConsent, false);
      await fixture.control('advance', {'milliseconds': 61000});
      await expectLater(
        w.api.call('submit_routed', {
          ...args,
          'approval_token': prepared.value['approval_token'],
          'confirm_transfer': false,
        }),
        throwsA(
          isA<AppFailure>().having((e) => e.code, 'code', 'CONSENT_EXPIRED'),
        ),
      );
      await expectLater(
        w.api.call('route_prepare', {
          ...args,
          'request_id': appId(),
          'expected_history_revision': 0,
        }),
        throwsA(
          isA<AppFailure>().having((e) => e.code, 'code', 'STALE_REVISION'),
        ),
      );
      expect(
        (await w.api.call('conversation_get', {
          'conversation_id': conversation,
        })).result['history'],
        hasLength(4),
      );

      // Already-approved destination: a queued local intent changes only after Resume.
      await fixture.control('hold', {'value': true});
      c.selectRoute(local);
      c.edit('local predecessor');
      await c.send();
      final predecessor = c.pendingId!;
      c.edit('queued rebind');
      c.messageQueue.enqueue();
      c.selectRoute(cloud);
      c.edit('newer draft retained');
      await fixture.control('hold', {'value': false});
      await observeUntil(w, () => c.pendingId == null && c.history.length == 6);
      expect(c.messageQueue.paused, true);
      expect(c.messageQueue.entries.single.status, contains('LOCAL'));
      final beforeResume = fixture.submissions.length;
      await w.synchronize();
      expect(fixture.submissions.length, beforeResume);
      await c.messageQueue.resume(c.messageQueue.revision);
      await observeUntil(w, () => c.pendingId == null && c.history.length == 8);
      expect(fixture.submissions.last['route'], cloud.toJson());
      expect(fixture.submissions.last['confirm_transfer'], false);
      expect(
        fixture.submissions.firstWhere(
          (a) => a['request_id'] == predecessor,
        )['route'],
        local.toJson(),
      );
      expect(c.draft, 'newer draft retained');

      // First destination in a separate chat cannot be authorized by queue dispatch.
      w.newChat();
      c = w.selected;
      await fixture.control('hold', {'value': true});
      c.edit('new local predecessor');
      await c.send();
      c.edit('queued first cloud');
      c.messageQueue.enqueue();
      c.selectRoute(cloud);
      c.edit('newer retry draft');
      await fixture.control('hold', {'value': false});
      await observeUntil(w, () => c.pendingId == null && c.history.length == 2);
      await c.messageQueue.resume(c.messageQueue.revision);
      await observeUntil(w, () => c.messageQueue.paused);
      final beforeConsent = fixture.submissions.length;
      await fixture.control('fail_http');
      await w.reviewQueuedCloud(c, (p) async {
        expect(p.requiresConsent, true);
        return TransferDecision.send;
      });
      await observeUntil(
        w,
        () => c.messageQueue.entries.any(
          (e) => e.state == QueuedMessageState.failed,
        ),
      );
      expect(fixture.submissions.length, beforeConsent + 1);
      final failed = c.messageQueue.entries.first;
      final failedRequest = fixture.submissions.last['request_id'];
      expect(failed.canRetry, true);
      expect(c.draft, 'newer retry draft');
      await c.messageQueue.retry(failed.id, failed.actionRevision);
      await observeUntil(w, () => c.pendingId == null && c.history.length == 4);
      expect(fixture.submissions.last['request_id'], isNot(failedRequest));
      expect(fixture.submissions.last['text'], 'queued first cloud');
      expect(fixture.submissions.last['confirm_transfer'], false);
      expect(c.draft, 'newer retry draft');
      expect(c.routing!.turns, [local, cloud]);

      // Lost acknowledgement preserves the accepted identity and never resubmits.
      fixture.loseNextAdmission = true;
      c.edit('lost acknowledgement');
      await c.sendWithConsent(
        (_) => throw StateError('same destination must not prompt'),
      );
      final lost = fixture.submissions.last['request_id'];
      expect(c.pendingId, lost);
      final count = fixture.submissions.length;
      await observeUntil(w, () => c.pendingId == null && c.history.length == 6);
      expect(fixture.submissions.length, count);
      expect(c.requestRoutes[lost]!.route, cloud);
      final stats = await fixture.control('stats');
      expect(
        stats['http_calls'],
        5,
      ); // first, rebound, failed, retried, lost-ack
      expect(stats['local_calls'], 3);
      expect(jsonEncode(stats['messages']), contains('local first'));
    },
    skip: executable == null && !required
        ? 'Run make client/test for mandatory compiled schema3 fixture.'
        : false,
    timeout: const Timeout(Duration(seconds: 75)),
  );
}
