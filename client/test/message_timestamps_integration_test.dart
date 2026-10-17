import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/cloud_routing.dart';
import 'package:smith_client/message_timestamps.dart';
import 'package:smith_client/workspace_controller.dart';

import 'rust_cloud_integration_test.dart' show RustCloudFixture, observeUntil;

void main() {
  final executable = Platform.environment['SMITH_TEST_CLOUD_SERVICE'];
  final required = Platform.environment.containsKey('SMITH_TEST_SERVICE');
  test(
    'compiled Rust local/cloud timestamps survive desktop reattach without provider metadata',
    () async {
      expect(executable, isNotNull);
      final fixture = await RustCloudFixture.start(executable!);
      addTearDown(fixture.close);
      var w = WorkspaceController(
        NativeAppApi(executor: fixture),
        pollInterval: const Duration(milliseconds: 20),
      );
      addTearDown(() => w.dispose());
      await w.synchronize();
      expect(w.stale, false, reason: w.error);
      var c = w.selected;
      c.edit('timestamp local fixture');
      await c.send();
      await observeUntil(
        w,
        () =>
            c.pendingId == null &&
            c.history.length == 2 &&
            c.historyTimestamps.length == 2,
      );
      expect(c.historyTimestamps, hasLength(2));
      expect(
        c.historyTimestamps.every((time) => time != null && time > 0),
        true,
      );
      final localTimes = List<int?>.of(c.historyTimestamps);
      final id = c.id!;
      final cloud = w.routeCatalog!.choices
          .firstWhere((choice) => choice.route.provider == 'openai')
          .route;
      c.selectRoute(cloud);
      c.edit('timestamp cloud fixture');
      await c.sendWithConsent((_) async => TransferDecision.send);
      await observeUntil(
        w,
        () =>
            c.pendingId == null &&
            c.history.length == 4 &&
            c.historyTimestamps.length == 4,
      );
      final allTimes = List<int?>.of(c.historyTimestamps);
      expect(allTimes, hasLength(4));
      expect(allTimes.take(2), localTimes);
      expect(allTimes.every((time) => time != null && time > 0), true);
      final beforeObservation = await fixture.control('stats');
      expect(beforeObservation['http_calls'], 1);
      expect(beforeObservation['local_calls'], 1);
      final metadata = ConversationTimestamps(
        (await w.api.call('conversation_timestamps', {
          'conversation_id': id,
        })).result,
      );
      expect(metadata.messageTimes, allTimes);
      expect(metadata.historyRevision, c.summary!['history_revision']);
      final history =
          (await w.api.call('conversation_get', {
                'conversation_id': id,
              })).result['history']
              as List;
      for (final message in history) {
        expect((message as Map).keys.toSet(), {'role', 'content'});
      }
      w.dispose();
      w = WorkspaceController(
        NativeAppApi(executor: fixture),
        pollInterval: const Duration(milliseconds: 20),
      );
      await w.synchronize();
      await w.select(id);
      c = w.selected;
      expect(c.historyTimestamps, allTimes);
      final afterObservation = await fixture.control('stats');
      expect(afterObservation['http_calls'], beforeObservation['http_calls']);
      expect(afterObservation['local_calls'], beforeObservation['local_calls']);
      expect(afterObservation['key_reads'], beforeObservation['key_reads']);
      expect(fixture.submissions, hasLength(2));
    },
    skip: executable == null && !required
        ? 'Requires the compiled, fake-injected Rust schema3 fixture.'
        : false,
    timeout: const Timeout(Duration(seconds: 45)),
  );
}
