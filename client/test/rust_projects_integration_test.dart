import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/projects.dart';
import 'package:smith_client/projects_controller.dart';

import 'rust_cloud_integration_test.dart' show RustCloudFixture;

class _ProjectExchange implements AppExecutor {
  _ProjectExchange(this.fixture);

  final RustCloudFixture fixture;
  final operations = <String>[];
  bool loseNextMutation = false;
  bool loseNextBeforeDispatch = false;
  final arguments = <Map<String, dynamic>>[];

  @override
  Future<String> exchange(List<int> frame) async {
    final request = jsonDecode(utf8.decode(frame)) as Map<String, dynamic>;
    final op = request['op'] as String;
    operations.add(op);
    arguments.add(Map<String, dynamic>.from(request['args'] as Map));
    if (loseNextBeforeDispatch && op == 'project_create_v3') {
      loseNextBeforeDispatch = false;
      throw const AppFailure(
        'TRANSPORT',
        'Synthetic loss before Rust admission.',
        unknown: true,
      );
    }
    final response = await fixture.exchange(frame);
    if (loseNextMutation &&
        const {
          'project_create_v3',
          'project_update_v3',
          'project_delete_v3',
        }.contains(op)) {
      loseNextMutation = false;
      throw const AppFailure(
        'TRANSPORT',
        'Synthetic lost Projects acknowledgement after Rust publication.',
        unknown: true,
      );
    }
    return response;
  }

  @override
  void dispose() {}
}

ProjectRepository _repository({
  bool readOnly = true,
  String? name = 'backend',
  String url = 'https://example.invalid/team/backend.git',
  String? base = 'main',
  String? work = 'dev',
  String? feature = 'feature/*',
  String? fix = 'fix/*',
  String? release = 'release/*',
  String? hotfix = 'hotfix/*',
}) => ProjectRepository(
  readOnly: readOnly,
  name: name,
  url: url,
  branch: ProjectBranches(
    base: base,
    work: work,
    types: ProjectBranchTypes(
      feature: feature,
      fix: fix,
      release: release,
      hotfix: hotfix,
    ),
  ),
);

void main() {
  final executable = Platform.environment['SMITH_TEST_CLOUD_SERVICE'];
  final fixtureRequired = Platform.environment.containsKey(
    'SMITH_TEST_SERVICE',
  );
  final skipFixture = executable == null && !fixtureRequired
      ? 'Run make client/test for the mandatory compiled Projects fixture.'
      : false;

  File diskFile(_ProjectExchange exchange) =>
      File('${exchange.fixture.root.path}/data/smith-next/app-state.json');

  Future<(_ProjectExchange, NativeAppApi, ProjectsController)> setup({
    bool existingEmpty = true,
  }) async {
    expect(executable, isNotNull);
    final fixture = await RustCloudFixture.start(executable!, savedOnly: true);
    addTearDown(fixture.close);
    final exchange = _ProjectExchange(fixture);
    if (existingEmpty) {
      // These purpose-specific fixtures exercise an EXISTING empty registry,
      // not fresh-profile initialization. Saved-only bootstrap has no pending
      // work; preserve every other current-schema durable field verbatim.
      final file = diskFile(exchange);
      final disk =
          jsonDecode(await file.readAsString()) as Map<String, dynamic>;
      expect(disk['schema_version'], 10);
      expect(
        (disk['jobs'] as List).any(
          (job) =>
              const ['queued', 'running'].contains(job['operation']['status']),
        ),
        false,
      );
      disk['projects'] = {
        'revision': 0,
        'projects': [],
        'receipts': [],
        'expired_ids': [],
      };
      await file.writeAsString(jsonEncode(disk), flush: true);
      await fixture.control('restart');
      expect(jsonDecode(await file.readAsString()), disk);
    }
    final api = NativeAppApi(executor: exchange);
    addTearDown(api.dispose);
    // The compiled fixture also requires observation of its cached full catalog.
    await api.call('route_catalog', {});
    final projects = ProjectsController(api);
    addTearDown(projects.dispose);
    await projects.refresh();
    expect(projects.error, isNull);
    expect(projects.loaded, true);
    return (exchange, api, projects);
  }

  Future<void> expectNoProviderEffects(_ProjectExchange exchange) async {
    final stats = await exchange.fixture.control('stats');
    expect(stats['local_calls'], 0);
    expect(stats['http_calls'], 0);
    expect(stats['key_reads'], 0);
    expect(stats['messages'], isEmpty);
    expect(
      exchange.operations,
      isNot(contains(anyOf('models', 'submit_routed', 'provider_probe'))),
    );
  }

  // Bypass client operation validation only to exercise the actual Rust guards.
  Future<AppReply> rawCall(
    _ProjectExchange exchange,
    String op,
    Map<String, dynamic> args,
  ) async {
    final callId = appId();
    final text = await exchange.exchange(
      utf8.encode(
        '${jsonEncode({'schema_version': 3, 'service': 'smith-next', 'protocol': 'app', 'call_id': callId, 'op': op, 'args': args})}\n',
      ),
    );
    return AppReply.parse(text, callId, schemaVersion: 3);
  }

  test(
    'compiled fresh profile persists an ordinary Smith starter without resurrection or extra effects',
    () async {
      // No existing-state preparation: Rust opens an absent app-state path.
      final (exchange, api, projects) = await setup(existingEmpty: false);
      expect(projects.revision, 1);
      final starter = projects.projects.single;
      final id = starter.projectId;
      expect(projectWireId(id), true);
      expect(starter.name, 'smith');
      expect(starter.agent, 'opencode');
      expect(starter.key, 'smith');
      expect(starter.description, 'local autonomy subtrate');
      expect(starter.prompt, '');
      final repository = <String, dynamic>{
        'name': 'smith',
        'read_only': true,
        'url': 'https://github.com/jdharrison/smith',
        'branch': {
          'base': 'main',
          'work': 'main',
          'types': {
            'feature': null,
            'fix': null,
            'release': null,
            'hotfix': null,
          },
        },
      };
      expect(starter.repositories.single.toJson(), repository);
      final record = {
        'project_id': id,
        'key': starter.key,
        'description': 'local autonomy subtrate',
        'prompt': '',
        'name': 'smith',
        'repositories': [repository],
        'agent': 'opencode',
      };
      expect((await api.call('projects_list_v3', {})).result, {
        'revision': 1,
        'projects': [record],
      });
      final initial = jsonDecode(
        await diskFile(exchange).readAsString(),
      ) as Map<String, dynamic>;
      expect(initial['schema_version'], 10);
      final registry = initial['projects'] as Map<String, dynamic>;
      expect(registry['revision'], 1);
      expect(registry['projects'], [record]);
      expect(registry['receipts'], hasLength(1));
      expect(registry['expired_ids'], isEmpty);
      final originalInstance = projects.instanceId;
      await exchange.fixture.control('restart');
      await projects.refresh();
      expect(projects.error, isNull);
      expect(projects.instanceId, isNot(originalInstance));
      expect(projects.revision, 1);
      expect(projects.projects.single.projectId, id);
      expect(projects.projects.single.repositories.single.toJson(), repository);
      expect(jsonDecode(await diskFile(exchange).readAsString()), initial);

      final changed = _repository(
        name: 'contribution',
        url: 'git@example.invalid:operator/smith.git',
        base: 'stable',
        work: 'team/next',
      );
      final repositories = [
        changed,
        ProjectRepository(
          name: 'another',
          url: 'https://example.invalid/another/repository',
        ),
      ];
      await projects.update(
        id,
        'smith',
        repositories,
        key: 'operator-renamed-smith',
        expectedInstanceId: projects.instanceId!,
        expectedRevision: 1,
      );
      expect(projects.error, isNull);
      expect(projects.revision, 2);
      await exchange.fixture.control('restart');
      await projects.refresh();
      expect(projects.error, isNull);
      expect(projects.projects.single.projectId, id);
      expect(projects.projects.single.name, 'smith');
      expect(projects.projects.single.key, 'operator-renamed-smith');
      expect(projects.projects.single.repositories, repositories);
      expect(projects.projects.single.agent, 'opencode');
      expect(projects.revision, 2);
      // Clearing all repositories is also an ordinary persisted update.
      await projects.update(
        id,
        'smith',
        [],
        key: 'operator-renamed-smith',
        expectedInstanceId: projects.instanceId!,
        expectedRevision: 2,
      );
      expect(projects.error, isNull);
      await exchange.fixture.control('restart');
      await projects.refresh();
      expect(projects.error, isNull);
      expect(projects.projects.single.repositories, isEmpty);
      expect(projects.revision, 3);
      await projects.delete(
        id,
        expectedInstanceId: projects.instanceId!,
        expectedRevision: 3,
      );
      expect(projects.error, isNull);
      expect(projects.projects, isEmpty);
      expect(projects.revision, 4);
      final deletedBytes = await diskFile(exchange).readAsBytes();
      for (var restart = 0; restart < 2; restart++) {
        await exchange.fixture.control('restart');
        await projects.refresh();
        expect(projects.error, isNull);
        expect(projects.projects, isEmpty);
        expect(projects.revision, 4);
        expect(await diskFile(exchange).readAsBytes(), deletedBytes);
      }
      final deleted =
          jsonDecode(utf8.decode(deletedBytes)) as Map<String, dynamic>;
      Map<String, dynamic> unrelated(Map<String, dynamic> disk) =>
          Map<String, dynamic>.of(disk)
            ..remove('projects')
            ..remove('revision');
      expect(unrelated(deleted), unrelated(initial));
      expect(
        exchange.operations.where((op) => op == 'project_update_v3'),
        hasLength(2),
      );
      expect(
        exchange.operations.where((op) => op == 'project_delete_v3'),
        hasLength(1),
      );
      expect(exchange.operations.toSet(), {
        'route_catalog',
        'projects_list_v3',
        'project_update_v3',
        'project_delete_v3',
      });
      await expectNoProviderEffects(exchange);
    },
    skip: skipFixture,
    timeout: const Timeout(Duration(seconds: 30)),
  );

  test(
    'compiled Projects CRUD persists structured repositories without changing chat projections',
    () async {
      final (exchange, api, projects) = await setup();
      final before = (await api.call('state', {})).result;
      expect(projects.revision, 0);
      expect(projects.projects, isEmpty);
      final repositories = [
        _repository(),
        _repository(
          name: 'frontend',
          url: 'ssh://git@example.invalid:2222/team/web.git',
          work: 'main',
        ),
        ProjectRepository(
          name: 'shared',
          url: 'git@example.invalid:team/shared.git',
        ),
      ];
      await projects.create(
        'VPS web',
        repositories,
        key: 'vps-web',
        description: ' exact description 🛠 ',
        prompt: ' exact\n\tproject prompt 🛠 ',
        expectedInstanceId: projects.instanceId!,
        expectedRevision: 0,
      );
      expect(projects.error, isNull);
      final webId = projects.projects.single.projectId;
      expect(projects.projects.single.repositories, repositories);
      expect(projects.projects.single.agent, 'opencode');
      expect(projects.projects.single.key, 'vps-web');
      expect(projects.projects.single.description, ' exact description 🛠 ');
      expect(projects.projects.single.prompt, ' exact\n\tproject prompt 🛠 ');
      await projects.create(
        'Smith',
        [],
        expectedInstanceId: projects.instanceId!,
        expectedRevision: 1,
      );
      expect(projects.error, isNull);
      final smithId = projects.projects.last.projectId;
      expect(projects.projects.every((p) => p.agent == 'opencode'), true);
      await projects.update(
        webId,
        'VPS web',
        [_repository(readOnly: false, work: 'team/*')],
        key: 'web-renamed',
        description: '',
        prompt: 'updated\n\texact prompt',
        expectedInstanceId: projects.instanceId!,
        expectedRevision: 2,
      );
      expect(projects.error, isNull);
      expect(projects.projects.first.projectId, webId);
      expect(projects.projects.first.name, 'VPS web');
      await projects.delete(
        smithId,
        expectedInstanceId: projects.instanceId!,
        expectedRevision: 3,
      );
      expect(projects.error, isNull);
      expect(projects.projects.single.projectId, webId);
      expect(projects.revision, 4);
      final after = (await api.call('state', {})).result;
      final beforeChat = Map<String, dynamic>.of(before)..remove('revision');
      final afterChat = Map<String, dynamic>.of(after)..remove('revision');
      expect(afterChat, beforeChat);
      await exchange.fixture.control('restart');
      await projects.refresh();
      expect(projects.error, isNull);
      expect(projects.projects.single.projectId, webId);
      expect(projects.projects.single.name, 'VPS web');
      expect(projects.projects.single.repositories, [
        _repository(readOnly: false, work: 'team/*'),
      ]);
      expect(projects.projects.single.key, 'web-renamed');
      expect(projects.projects.single.description, '');
      expect(projects.projects.single.prompt, 'updated\n\texact prompt');
      expect(projects.revision, 4);
      final disk = jsonDecode(
        await diskFile(exchange).readAsString(),
      ) as Map<String, dynamic>;
      expect(disk['schema_version'], 10);
      final independent = ProjectsController(api);
      addTearDown(independent.dispose);
      await independent.refresh();
      expect(independent.projects.single.projectId, webId);
      expect(independent.projects.single.repositories, [
        _repository(readOnly: false, work: 'team/*'),
      ]);
      expect(independent.revision, 4);
      await expectNoProviderEffects(exchange);
    },
    skip: skipFixture,
    timeout: const Timeout(Duration(seconds: 30)),
  );

  test(
    'compiled lost Projects acknowledgement resolves across restart without replay',
    () async {
      final (exchange, _, projects) = await setup();
      final repository = _repository();
      exchange.loseNextMutation = true;
      await projects.create(
        'Once only',
        [repository],
        expectedInstanceId: projects.instanceId!,
        expectedRevision: 0,
      );
      expect(projects.uncertain, true);
      expect(projects.canMutate, false);
      await exchange.fixture.control('restart');
      await projects.resolvePending();
      expect(projects.error, isNull);
      expect(projects.uncertain, false);
      expect(projects.projects.single.name, 'Once only');
      expect(projects.projects.single.repositories, [repository]);
      expect(projects.revision, 1);
      expect(
        exchange.operations.where((op) => op == 'project_create_v3'),
        hasLength(1),
      );
      expect(
        exchange.operations.where((op) => op == 'project_receipt'),
        hasLength(1),
      );
      await expectNoProviderEffects(exchange);
    },
    skip: skipFixture,
    timeout: const Timeout(Duration(seconds: 30)),
  );

  test(
    'compiled rich uncertain update preserves exact context and inactive custom types across restart',
    () async {
      final (exchange, api, projects) = await setup();
      const name = 'Hidden compatibility Name';
      const description = ' exact e\u0301 🛠 description ';
      const prompt = '  exact e\u0301 🛠\n\tproject context\n ';
      final repository = _repository(
        work: 'team/work/*',
        feature: 'team/feature/*',
        fix: null,
        release: 'team/release/*',
        hotfix: null,
      );
      await projects.create(
        name,
        [repository],
        key: 'original-key',
        description: description,
        prompt: prompt,
        expectedInstanceId: projects.instanceId!,
        expectedRevision: 0,
      );
      expect(projects.error, isNull);
      final id = projects.projects.single.projectId;
      final writable = _repository(
        readOnly: false,
        work: 'team/work/*',
        feature: 'team/feature/*',
        fix: null,
        release: 'team/release/*',
        hotfix: null,
      );
      final input = [writable];
      exchange.loseNextMutation = true;
      await projects.update(
        id,
        name,
        input,
        key: 'renamed-rich-key',
        description: description,
        prompt: prompt,
        expectedInstanceId: projects.instanceId!,
        expectedRevision: 1,
      );
      final request = exchange.arguments.last['request_id'];
      input.clear();
      expect(projects.uncertain, true);
      final oldInstance = projects.instanceId;
      await exchange.fixture.control('restart');
      final beforeResolve = exchange.operations.length;
      await projects.resolvePending();
      expect(exchange.operations.sublist(beforeResolve), [
        'project_receipt',
        'projects_list_v3',
      ]);
      expect(exchange.arguments[beforeResolve], {'request_id': request});
      expect(projects.instanceId, isNot(oldInstance));
      expect(projects.error, isNull);
      expect(projects.uncertain, false);
      expect(projects.revision, 2);
      final saved = projects.projects.single;
      expect(saved.projectId, id);
      expect(saved.name, name);
      expect(saved.key, 'renamed-rich-key');
      expect(saved.description, description);
      expect(saved.prompt, prompt);
      expect(saved.repositories, [writable]);

      // Read-only toggles must not fill null leaves or replace custom patterns.
      await projects.update(
        id,
        name,
        [repository],
        key: saved.key,
        description: description,
        prompt: prompt,
        expectedInstanceId: projects.instanceId!,
        expectedRevision: 2,
      );
      expect(projects.error, isNull);
      await exchange.fixture.control('restart');
      await projects.refresh();
      expect(projects.projects.single.repositories, [repository]);

      // Clearing one selected leaf leaves every other inactive setting exact.
      final cleared = _repository(
        work: 'team/work/*',
        feature: null,
        fix: null,
        release: 'team/release/*',
        hotfix: null,
      );
      await projects.update(
        id,
        name,
        [cleared],
        key: saved.key,
        description: description,
        prompt: prompt,
        expectedInstanceId: projects.instanceId!,
        expectedRevision: 3,
      );
      expect(projects.error, isNull);
      await exchange.fixture.control('restart');
      await projects.refresh();
      expect(projects.error, isNull);
      expect(projects.projects.single.repositories, [cleared]);
      expect(projects.projects.single.name, name);
      expect(projects.projects.single.key, saved.key);
      expect(projects.projects.single.description, description);
      expect(projects.projects.single.prompt, prompt);
      final disk = jsonDecode(
        await diskFile(exchange).readAsString(),
      ) as Map<String, dynamic>;
      expect(disk['schema_version'], 10);
      expect(
        disk['projects']['projects'],
        (await api.call('projects_list_v3', {})).result['projects'],
      );
      expect(
        exchange.operations.where((op) => op == 'project_update_v3'),
        hasLength(3),
      );
      await expectNoProviderEffects(exchange);
    },
    skip: skipFixture,
    timeout: const Timeout(Duration(seconds: 30)),
  );

  for (final expired in [false, true]) {
    test(
      'compiled ${expired ? 'REQUEST_EXPIRED' : 'REQUEST_UNKNOWN'} observes Rust receipt and list without resending across restart',
      () async {
        final (exchange, api, projects) = await setup();
        if (expired) {
          exchange.loseNextMutation = true;
        } else {
          exchange.loseNextBeforeDispatch = true;
        }
        await projects.create(
          'Unconfirmed',
          [_repository(feature: 'team/custom/*', fix: null)],
          key: 'unconfirmed',
          prompt: ' exact\n\tcontext ',
          expectedInstanceId: projects.instanceId!,
          expectedRevision: 0,
        );
        final request = exchange.arguments.last['request_id'];
        expect(projects.uncertain, true);
        if (expired) {
          final other = ProjectsController(api);
          addTearDown(other.dispose);
          await other.refresh();
          final saved = other.projects.single;
          // The production ledger retains 32 receipts. Ordinary accepted updates
          // evict the original create into its durable expired-ID tombstones.
          for (var i = 0; i < 32; i++) {
            await other.update(
              saved.projectId,
              saved.name,
              saved.repositories,
              key: saved.key,
              description: 'revision-$i',
              prompt: saved.prompt,
              expectedInstanceId: other.instanceId!,
              expectedRevision: other.revision!,
            );
            expect(other.error, isNull);
          }
        }
        await exchange.fixture.control('restart');
        final code = expired ? 'REQUEST_EXPIRED' : 'REQUEST_UNKNOWN';
        await expectLater(
          api.call('project_receipt', {'request_id': request}),
          throwsA(isA<AppFailure>().having((e) => e.code, 'code', code)),
        );
        for (var observation = 0; observation < 2; observation++) {
          final before = exchange.operations.length;
          await projects.resolvePending();
          expect(exchange.operations.sublist(before), [
            'project_receipt',
            'projects_list_v3',
          ]);
          expect(exchange.arguments[before], {'request_id': request});
          expect(projects.uncertain, true);
          expect(projects.canMutate, false);
          expect(projects.error, contains(expired ? 'expired' : 'unknown'));
          expect(projects.projects, hasLength(expired ? 1 : 0));
          if (expired) {
            expect(projects.projects.single.key, 'unconfirmed');
            expect(projects.projects.single.prompt, ' exact\n\tcontext ');
          }
          final blocked = exchange.operations.length;
          await projects.create(
            'Forbidden replacement',
            [],
            key: 'forbidden',
            expectedInstanceId: projects.instanceId!,
            expectedRevision: projects.revision!,
          );
          expect(exchange.operations.length, blocked);
          await projects.refresh();
          expect(projects.uncertain, true);
          expect(projects.canMutate, false);
        }
        expect(
          exchange.operations.where((op) => op == 'project_create_v3'),
          hasLength(1),
        );
        expect(
          exchange.operations.where((op) => op == 'project_update_v3'),
          hasLength(expired ? 32 : 0),
        );
        await expectNoProviderEffects(exchange);
      },
      skip: skipFixture,
      timeout: const Timeout(Duration(seconds: 60)),
    );
  }

  test(
    'compiled historical create receipt cannot recreate a deleted project',
    () async {
      final (exchange, api, projects) = await setup();
      exchange.loseNextMutation = true;
      await projects.create(
        'Deleted elsewhere',
        [_repository()],
        expectedInstanceId: projects.instanceId!,
        expectedRevision: 0,
      );
      expect(projects.uncertain, true);
      final other = ProjectsController(api);
      addTearDown(other.dispose);
      await other.refresh();
      expect(other.projects, hasLength(1));
      await other.delete(
        other.projects.single.projectId,
        expectedInstanceId: other.instanceId!,
        expectedRevision: 1,
      );
      expect(other.error, isNull);
      await exchange.fixture.control('restart');
      await projects.resolvePending();
      expect(projects.error, isNull);
      expect(projects.uncertain, false);
      expect(projects.projects, isEmpty);
      expect(projects.revision, 2);
      expect(
        exchange.operations.where((op) => op == 'project_create_v3'),
        hasLength(1),
      );
      expect(
        exchange.operations.where((op) => op == 'project_delete_v3'),
        hasLength(1),
      );
      await expectNoProviderEffects(exchange);
    },
    skip: skipFixture,
    timeout: const Timeout(Duration(seconds: 30)),
  );

  test(
    'compiled concurrent project edits and stale service instances fail closed',
    () async {
      final (exchange, api, projects) = await setup();
      await projects.create(
        'Original',
        [],
        expectedInstanceId: projects.instanceId!,
        expectedRevision: 0,
      );
      final id = projects.projects.single.projectId;
      final other = ProjectsController(api);
      addTearDown(other.dispose);
      await other.refresh();
      await projects.update(
        id,
        'Original',
        [_repository()],
        key: 'current',
        expectedInstanceId: projects.instanceId!,
        expectedRevision: 1,
      );
      expect(projects.error, isNull);
      await other.update(
        id,
        'Original',
        [],
        key: 'stale-overwrite',
        expectedInstanceId: other.instanceId!,
        expectedRevision: 1,
      );
      expect(other.error, contains('changed'));
      expect(other.uncertain, false);
      await other.refresh();
      expect(other.projects.single.name, 'Original');
      expect(other.projects.single.key, 'current');
      expect(other.projects.single.repositories, [_repository()]);
      expect(other.revision, 2);
      final editorInstance = projects.instanceId!;
      await exchange.fixture.control('restart');
      await projects.update(
        id,
        'Original',
        [],
        key: 'stale-instance',
        expectedInstanceId: editorInstance,
        expectedRevision: 2,
      );
      expect(projects.error, contains('restarted'));
      expect(projects.uncertain, false);
      await projects.refresh();
      expect(projects.projects.single.name, 'Original');
      expect(projects.projects.single.key, 'current');
      expect(projects.projects.single.repositories, [_repository()]);
      expect(projects.revision, 2);
      expect(projects.instanceId, isNot(editorInstance));
      final dispatched = exchange.operations.length;
      await projects.update(
        id,
        'Original',
        [],
        key: 'old-editor-after-refresh',
        expectedInstanceId: editorInstance,
        expectedRevision: 2,
      );
      expect(projects.error, contains('restarted'));
      expect(exchange.operations, hasLength(dispatched));
      await expectNoProviderEffects(exchange);
    },
    skip: skipFixture,
    timeout: const Timeout(Duration(seconds: 30)),
  );

  test(
    'compiled disk5 URLs migrate without invented repository names or branches',
    () async {
      final (exchange, api, projects) = await setup();
      final before = (await api.call('state', {})).result;
      final seed = await exchange.fixture.control('seed_disk5_projects');
      expect(seed['disk_schema_seeded'], 5);
      await projects.refresh();
      expect(projects.error, isNull);
      expect(projects.revision, seed['registry_revision']);
      expect(projects.projects.single.projectId, seed['legacy_project_id']);
      expect(
        projects.projects.single.key,
        'project-${seed['legacy_project_id']}',
      );
      expect(projects.projects.single.description, '');
      expect(projects.projects.single.prompt, '');
      final migrated = ProjectRepository(url: seed['url'] as String);
      expect(projects.projects.single.repositories, [migrated]);
      expect(projects.projects.single.repositories.single.name, isNull);
      expect(
        projects.projects.single.repositories.single.branch,
        ProjectBranches(),
      );
      final receipt = await api.call('project_receipt', {
        'request_id': seed['legacy_request_id'],
      });
      expect(receipt.result, seed['legacy_receipt']);
      final id = projects.projects.single.projectId;
      final beforeRejected = exchange.operations.length;
      await projects.update(
        id,
        projects.projects.single.name,
        projects.projects.single.repositories,
        key: 'renamed-without-inventing-configuration',
        expectedInstanceId: projects.instanceId!,
        expectedRevision: projects.revision!,
      );
      expect(projects.error, isNotNull);
      expect(exchange.operations, hasLength(beforeRejected));
      expect(projects.projects.single.repositories, [migrated]);
      final configured = ProjectRepository(
        name: 'backend',
        url: migrated.url,
        branch: ProjectBranches(
          base: 'main',
          work: 'main',
          types: ProjectBranchTypes(feature: 'feature/*'),
        ),
      );
      await projects.update(
        id,
        projects.projects.single.name,
        [configured],
        key: 'configured',
        expectedInstanceId: projects.instanceId!,
        expectedRevision: projects.revision!,
      );
      expect(projects.error, isNull);
      await exchange.fixture.control('restart');
      await projects.refresh();
      expect(projects.error, isNull);
      expect(projects.projects.single.repositories, [configured]);
      final historical = await api.call('project_receipt', {
        'request_id': seed['legacy_request_id'],
      });
      expect(historical.result, seed['legacy_receipt']);
      final disk = jsonDecode(
        await diskFile(exchange).readAsString(),
      ) as Map<String, dynamic>;
      expect(disk['schema_version'], 10);
      final after = (await api.call('state', {})).result;
      // Inventory is deliberately volatile; restarting clears prior observations.
      expect(after['inventory_observed'], false);
      expect((after['inventory'] as Map)['models'], isEmpty);
      Map<String, dynamic> durableProjection(Map<String, dynamic> state) =>
          Map<String, dynamic>.of(state)
            ..remove('revision')
            ..remove('inventory')
            ..remove('inventory_observed');
      // Legacy migration advances configuration revision once; retain
      // exact comparison of every other chat/configuration projection field.
      final expectedBefore = Map<String, dynamic>.of(before);
      final config = Map<String, dynamic>.from(before['config'] as Map);
      config['revision'] = (config['revision'] as int) + 1;
      expectedBefore['config'] = config;
      expect(durableProjection(after), durableProjection(expectedBefore));
      await expectNoProviderEffects(exchange);
    },
    skip: skipFixture,
    timeout: const Timeout(Duration(seconds: 30)),
  );

  test(
    'compiled legacy Projects calls cannot erase settings or bypass replay identities',
    () async {
      final (exchange, api, projects) = await setup();
      final seed = await exchange.fixture.control('seed_disk5_projects');
      await projects.refresh();
      final diskBefore = await diskFile(exchange).readAsBytes();
      final identity = <String, dynamic>{
        'request_id': appId(),
        'instance_id': projects.instanceId,
        'expected_revision': projects.revision,
      };
      final oldCreate = Map<String, dynamic>.from(seed['legacy_args'] as Map);
      final attempts = <(String, Map<String, dynamic>)>[
        ('projects_list', {}),
        ('projects_list_v2', {}),
        ('project_create_v2', oldCreate),
        (
          'project_update_v2',
          {
            ...identity,
            'project_id': seed['legacy_project_id'],
            'name': 'Lossy v2 writer',
            'repositories': [],
            'agent': 'opencode',
          },
        ),
        (
          'project_delete_v2',
          {...identity, 'project_id': seed['legacy_project_id']},
        ),
        ('project_create', oldCreate),
        (
          'project_update',
          {
            ...identity,
            'project_id': seed['legacy_project_id'],
            'name': 'Lossy old writer',
            'repositories': [],
            'agent': 'opencode',
          },
        ),
        (
          'project_delete',
          {...identity, 'project_id': seed['legacy_project_id']},
        ),
      ];
      for (final (op, args) in attempts) {
        await expectLater(
          rawCall(exchange, op, args),
          throwsA(
            isA<AppFailure>().having((e) => e.code, 'code', 'UPGRADE_REQUIRED'),
          ),
        );
      }
      for (final (request, expected) in [
        (seed['legacy_request_id'], 'ID_CONFLICT'),
        (seed['expired_request_id'], 'REQUEST_EXPIRED'),
      ]) {
        await expectLater(
          api.call('project_create_v3', {
            ...identity,
            'request_id': request,
            'name': 'No resurrected identity',
            'key': 'no-resurrected-identity',
            'description': '',
            'prompt': '',
            'repositories': [],
            'agent': 'opencode',
          }),
          throwsA(isA<AppFailure>().having((e) => e.code, 'code', expected)),
        );
      }
      expect(await diskFile(exchange).readAsBytes(), diskBefore);
      await exchange.fixture.control('restart');
      final receipt = await api.call('project_receipt', {
        'request_id': seed['legacy_request_id'],
      });
      expect(receipt.result, seed['legacy_receipt']);
      await projects.refresh();
      expect(projects.error, isNull);
      expect(projects.projects.single.projectId, seed['legacy_project_id']);
      expect(projects.revision, seed['registry_revision']);
      await expectNoProviderEffects(exchange);
    },
    skip: skipFixture,
    timeout: const Timeout(Duration(seconds: 30)),
  );

  test(
    'compiled branch-only uncertain update resolves original nested payload without replay',
    () async {
      final (exchange, _, projects) = await setup();
      await projects.create(
        'Branch settings',
        [_repository()],
        expectedInstanceId: projects.instanceId!,
        expectedRevision: 0,
      );
      final id = projects.projects.single.projectId;
      final changed = _repository(work: 'team/next');
      final callerOwned = [changed];
      exchange.loseNextMutation = true;
      await projects.update(
        id,
        'Branch settings',
        callerOwned,
        expectedInstanceId: projects.instanceId!,
        expectedRevision: 1,
      );
      callerOwned.clear();
      expect(projects.uncertain, true);
      expect(projects.canMutate, false);
      await projects.refresh();
      expect(projects.projects.single.repositories, [changed]);
      expect(projects.uncertain, true);
      expect(projects.canMutate, false);
      await exchange.fixture.control('restart');
      await projects.resolvePending();
      expect(projects.error, isNull);
      expect(projects.uncertain, false);
      expect(projects.projects.single.name, 'Branch settings');
      expect(projects.projects.single.repositories, [changed]);
      expect(projects.revision, 2);
      expect(
        exchange.operations.where((op) => op == 'project_update_v3'),
        hasLength(1),
      );
      expect(
        exchange.operations.where((op) => op == 'project_receipt'),
        hasLength(1),
      );
      await expectNoProviderEffects(exchange);
    },
    skip: skipFixture,
    timeout: const Timeout(Duration(seconds: 30)),
  );
}
