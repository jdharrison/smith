import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/agents.dart';
import 'package:smith_client/agents_controller.dart';
import 'package:smith_client/app_api.dart';

import 'rust_cloud_integration_test.dart' show RustCloudFixture;

class _AgentExchange implements AppExecutor {
  _AgentExchange(this.fixture);
  final RustCloudFixture fixture;
  final operations = <String>[];
  final arguments = <Map<String, dynamic>>[];
  bool loseNextUpdate = false;
  @override
  Future<String> exchange(List<int> frame) async {
    final request = jsonDecode(utf8.decode(frame)) as Map<String, dynamic>;
    final op = request['op'] as String;
    operations.add(op);
    arguments.add(Map<String, dynamic>.from(request['args']));
    final response = await fixture.exchange(frame);
    if ((op == 'agent_update_v2' || op == 'agent_create_v1') &&
        loseNextUpdate) {
      loseNextUpdate = false;
      throw const AppFailure(
        'TRANSPORT',
        'Synthetic lost Agents acknowledgement after Rust publication.',
        unknown: true,
      );
    }
    return response;
  }

  @override
  void dispose() {}
}

void main() {
  final executable = Platform.environment['SMITH_TEST_CLOUD_SERVICE'];
  final fixtureRequired = Platform.environment.containsKey(
    'SMITH_TEST_SERVICE',
  );
  final skipFixture = executable == null && !fixtureRequired
      ? 'Run make client/test for the mandatory compiled Agents fixture.'
      : false;

  Future<(_AgentExchange, NativeAppApi, AgentsController)> setup() async {
    expect(executable, isNotNull);
    final fixture = await RustCloudFixture.start(executable!, savedOnly: true);
    addTearDown(fixture.close);
    final exchange = _AgentExchange(fixture);
    final api = NativeAppApi(executor: exchange);
    addTearDown(api.dispose);
    // Required cached observation in this existing synthetic fixture, no probe.
    await api.call('route_catalog', {});
    final c = AgentsController(api);
    addTearDown(c.dispose);
    await c.refresh();
    expect(c.error, isNull);
    expect(c.canMutate, true);
    return (exchange, api, c);
  }

  Future<void> noEffects(_AgentExchange exchange) async {
    final stats = await exchange.fixture.control('stats');
    expect(stats['local_calls'], 0);
    expect(stats['http_calls'], 0);
    expect(stats['key_reads'], 0);
    expect(stats['messages'], isEmpty);
    expect(
      exchange.operations.every(
        (op) => const {
          'route_catalog',
          'agents_list_v2',
          'agent_update_v2',
          'agent_create_v1',
          'agent_receipt',
        }.contains(op),
      ),
      true,
    );
  }

  test(
    'compiled built-in metadata is immutable while prompt remains editable',
    () async {
      final (exchange, api, c) = await setup();
      final original = c.agents.first;
      expect(original.isBuiltIn, true);
      expect(original.description.runes.length, lessThanOrEqualTo(40));
      for (final fields in [
        {'name': 'Changed', 'description': original.description},
        {'name': original.name, 'description': 'Changed'},
      ]) {
        await expectLater(
          api.call('agent_update_v2', {
            'request_id': appId(),
            'instance_id': c.instanceId!,
            'expected_revision': c.revision!,
            'agent_id': original.agentId,
            ...fields,
            'prompt': original.prompt,
          }),
          throwsA(isA<AppFailure>()),
        );
      }
      await c.refresh();
      expect(c.revision, 1);
      expect(c.agents.first, original);
      await c.update(
        original.agentId,
        original.name,
        original.description,
        'New prompt',
        expectedInstanceId: c.instanceId!,
        expectedRevision: c.revision!,
      );
      expect(c.error, isNull);
      expect(c.agents.first.prompt, 'New prompt');
      expect(c.agents.first.name, original.name);
      expect(c.agents.first.description, original.description);
      await noEffects(exchange);
    },
    skip: skipFixture,
  );

  test('compiled Agents exact fields, replay, durable restart and no provider effects', () async {
    final (exchange, api, c) = await setup();
    expect(c.revision, 1);
    expect(c.agents.map((a) => a.agentId), ['developer', 'assurance']);
    final before = List<AgentRecord>.of(c.agents);
    final origin = c.instanceId!;
    const prompt = ' Purpose\n\tExact bytes 😀\n ';
    await c.create(
      'Renamed',
      '',
      prompt,
      expectedInstanceId: origin,
      expectedRevision: 1,
    );
    expect(c.error, isNull);
    expect(c.revision, 2);
    expect(
      c.agents.last,
      AgentRecord(
        agentId: c.lastReceipt!.agentId,
        name: 'Renamed',
        description: '',
        prompt: prompt,
      ),
    );
    expect(c.agents.take(2), before);
    final args =
        exchange.arguments[exchange.operations.indexOf('agent_create_v1')];
    final replay = await api.call('agent_create_v1', args);
    expect(replay.result, {
      'request_id': args['request_id'],
      'agent_id': c.lastReceipt!.agentId,
      'revision': 2,
    });
    await expectLater(
      api.call('agent_create_v1', {...args, 'prompt': 'conflicting'}),
      throwsA(isA<AppFailure>().having((e) => e.code, 'code', 'ID_CONFLICT')),
    );
    final disk = File(
      '${exchange.fixture.root.path}/data/smith-next/app-state.json',
    );
    final saved = jsonDecode(await disk.readAsString()) as Map<String, dynamic>;
    expect(saved['schema_version'], 10);
    expect(saved['agents']['revision'], 2);
    await exchange.fixture.control('restart');
    await c.refresh();
    expect(c.instanceId, isNot(origin));
    expect(c.revision, 2);
    expect(c.agents.last.prompt, prompt);
    final count = exchange.operations.length;
    await c.update(
      'developer',
      'Stale',
      '',
      '',
      expectedInstanceId: origin,
      expectedRevision: 2,
    );
    expect(exchange.operations.length, count);
    await noEffects(exchange);
  }, skip: skipFixture);

  test('compiled lost acknowledgement resolves original receipt after restart without resend', () async {
    final (exchange, _, c) = await setup();
    exchange.loseNextUpdate = true;
    await c.create(
      'Independent review',
      'Evidence only',
      '',
      expectedInstanceId: c.instanceId!,
      expectedRevision: c.revision!,
    );
    expect(c.uncertain, true);
    final id = exchange.arguments.last['request_id'];
    await exchange.fixture.control('restart');
    await c.refresh();
    expect(c.uncertain, true);
    await c.resolvePending();
    expect(c.error, isNull);
    expect(c.canMutate, true);
    expect(c.agents.last.prompt, '');
    expect(c.agents.last.name, 'Independent review');
    expect(exchange.operations, [
      'route_catalog',
      'agents_list_v2',
      'agent_create_v1',
      'agents_list_v2',
      'agent_receipt',
      'agents_list_v2',
    ]);
    expect(exchange.arguments[4], {'request_id': id});
    await noEffects(exchange);
  }, skip: skipFixture);

  test(
    'compiled expired receipt retains write block and observable current list',
    () async {
      final (exchange, api, c) = await setup();
      await c.create(
        'Custom',
        '',
        '',
        expectedInstanceId: c.instanceId!,
        expectedRevision: c.revision!,
      );
      final customId = c.lastReceipt!.agentId;
      exchange.loseNextUpdate = true;
      await c.update(
        customId,
        'Original unknown',
        '',
        '',
        expectedInstanceId: c.instanceId!,
        expectedRevision: c.revision!,
      );
      expect(c.uncertain, true);
      final original = exchange.arguments.last['request_id'];
      // Another client advances the bounded receipt window; never replay the lost ID.
      for (var i = 0; i < 32; i++) {
        final list = await api.call('agents_list_v2', {});
        await api.call('agent_update_v2', {
          'request_id': appId(),
          'instance_id': list.instanceId,
          'expected_revision': list.result['revision'],
          'agent_id': customId,
          'name': 'Review $i',
          'description': '',
          'prompt': '',
        });
      }
      await c.resolvePending();
      expect(c.uncertain, true);
      expect(c.canMutate, false);
      expect(c.error, contains('expired'));
      expect(c.agents.last.name, 'Review 31');
      expect(
        exchange.arguments[exchange.operations.lastIndexOf('agent_receipt')],
        {'request_id': original},
      );
      final count = exchange.operations.length;
      await c.update(
        'developer',
        'Blocked',
        '',
        '',
        expectedInstanceId: c.instanceId!,
        expectedRevision: c.revision!,
      );
      expect(exchange.operations.length, count);
      await c.refresh();
      expect(c.uncertain, true);
      await noEffects(exchange);
    },
    skip: skipFixture,
  );
}
