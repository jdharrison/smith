import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/projects.dart';
import 'package:smith_client/projects_controller.dart';

const firstInstance = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
const nextInstance = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';
const firstProject = 'cccccccccccccccccccccccccccccccc';

typedef Response = FutureOr<AppReply> Function(Map<String, dynamic> args);

class MetadataApi implements AppApi {
  String instance = firstInstance;
  int revision = 0;
  int disposed = 0;
  final projects = <Map<String, dynamic>>[];
  final receipts = <String, Map<String, dynamic>>{};
  final calls = <String>[];
  final arguments = <Map<String, dynamic>>[];
  final responses = <String, List<Response>>{};

  void next(String op, Response response) =>
      responses.putIfAbsent(op, () => []).add(response);

  AppReply listReply() => AppReply(instance, {
    'revision': revision,
    'projects': jsonDecode(jsonEncode(projects)),
  });

  AppReply commit(String op, Map<String, dynamic> args) {
    final id = args['project_id'] ?? firstProject;
    if (op == 'project_update_v3') {
      final saved = projects.singleWhere((p) => p['project_id'] == id);
      if (args['name'] != saved['name']) {
        throw const AppFailure(
          'INVALID_REQUEST',
          'Compatibility Name is immutable',
        );
      }
    }
    projects.removeWhere((p) => p['project_id'] == id);
    if (op != 'project_delete_v3') {
      projects.add({
        'project_id': id,
        'name': args['name'],
        'key': args['key'],
        'description': args['description'],
        'prompt': args['prompt'],
        'repositories': jsonDecode(jsonEncode(args['repositories'])),
        'agent': args['agent'],
      });
    }
    final receipt = <String, dynamic>{
      'request_id': args['request_id'],
      'project_id': id,
      'revision': ++revision,
      'deleted': op == 'project_delete_v3',
    };
    receipts[args['request_id']] = receipt;
    return AppReply(instance, receipt);
  }

  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    calls.add(op);
    arguments.add(jsonDecode(jsonEncode(args)) as Map<String, dynamic>);
    final scripted = responses[op];
    if (scripted != null && scripted.isNotEmpty) {
      return scripted.removeAt(0)(args);
    }
    switch (op) {
      case 'projects_list_v3':
        return listReply();
      case 'project_create_v3':
      case 'project_update_v3':
      case 'project_delete_v3':
        return commit(op, args);
      case 'project_receipt':
        final receipt = receipts[args['request_id']];
        if (receipt == null) {
          throw const AppFailure('REQUEST_UNKNOWN', 'Unknown');
        }
        return AppReply(instance, receipt);
      default:
        throw StateError('Unexpected non-project operation');
    }
  }

  @override
  void dispose() => disposed++;
}

Future<void> flush() async {
  for (var i = 0; i < 10; i++) {
    await Future<void>.delayed(Duration.zero);
  }
}

void main() {
  test('v3 captures exact metadata and read-only changes through query-only recovery', () async {
    final api = MetadataApi();
    final c = ProjectsController(api);
    addTearDown(c.dispose);
    await c.refresh();
    final repositories = [
      ProjectRepository(
        name: 'repo',
        url: 'git@host:repo',
        readOnly: false,
        branch: ProjectBranches(work: 'team/*'),
      ),
    ];
    api.next('project_create_v3', (args) {
      api.commit('project_create_v3', args);
      throw const AppFailure(
        'TRANSPORT',
        'Lost acknowledgement',
        unknown: true,
      );
    });
    await c.create(
      'Compatibility name',
      repositories,
      key: 'editable-key',
      description: ' exact description ',
      prompt: ' exact\n\tprompt ',
      expectedInstanceId: firstInstance,
      expectedRevision: 0,
    );
    expect(c.uncertain, true);
    final captured = api.arguments[1];
    expect(captured['key'], 'editable-key');
    expect(captured['description'], ' exact description ');
    expect(captured['prompt'], ' exact\n\tprompt ');
    expect(captured['repositories'], [repositories.single.toJson()]);
    repositories.clear();
    api.instance = nextInstance;
    await c.resolvePending();
    expect(c.uncertain, false);
    expect(c.projects.single.key, 'editable-key');
    expect(c.projects.single.description, ' exact description ');
    expect(c.projects.single.prompt, ' exact\n\tprompt ');
    expect(c.projects.single.repositories.single.readOnly, false);
    expect(api.calls, [
      'projects_list_v3',
      'project_create_v3',
      'project_receipt',
      'projects_list_v3',
    ]);
    expect(api.arguments[2]['request_id'], captured['request_id']);
    await c.update(
      firstProject,
      'Compatibility name',
      [ProjectRepository(name: 'repo', url: 'git@host:repo')],
      key: 'renamed-key',
      description: '',
      prompt: '',
      expectedInstanceId: nextInstance,
      expectedRevision: 1,
    );
    expect(c.projects.single.projectId, firstProject);
    expect(c.projects.single.key, 'renamed-key');
    expect(c.projects.single.name, 'Compatibility name');
    expect(c.projects.single.description, '');
    expect(c.projects.single.prompt, '');
    expect(c.projects.single.repositories.single.readOnly, true);
  });

  test(
    'legacy repository Names load but require explicit IDs before dispatch',
    () async {
      final api = MetadataApi()..revision = 7;
      final legacy = [
        ProjectRepository(url: 'git@host:unnamed', readOnly: false),
        ProjectRepository(
          name: 'Legacy Repo 🚀',
          url: 'git@host:legacy',
          branch: ProjectBranches(
            types: ProjectBranchTypes(feature: 'custom/features/*'),
          ),
        ),
      ];
      const prompt = ' exact hidden\n\tprompt ';
      api.projects.add({
        'project_id': firstProject,
        'name': 'Smith',
        'key': 'smith',
        'description': '',
        'prompt': prompt,
        'repositories': legacy.map((r) => r.toJson()).toList(),
        'agent': projectAgent,
      });
      final c = ProjectsController(api);
      addTearDown(c.dispose);
      await c.refresh();
      expect(c.error, isNull);
      expect(c.projects.single.repositories, legacy);
      await c.update(
        firstProject,
        'Smith',
        legacy,
        key: 'smith',
        prompt: prompt,
        expectedInstanceId: firstInstance,
        expectedRevision: 7,
      );
      expect(c.error, isNotNull);
      expect(c.busy || c.uncertain, isFalse);
      expect(c.revision, 7);
      expect(c.projects.single.repositories, legacy);
      await c.resolvePending();
      expect(api.calls, ['projects_list_v3']);
      final corrected = [
        for (var i = 0; i < legacy.length; i++)
          ProjectRepository(
            name: 'explicit-$i',
            url: legacy[i].url,
            branch: legacy[i].branch,
            readOnly: legacy[i].readOnly,
          ),
      ];
      await c.update(
        firstProject,
        'Smith',
        corrected,
        key: 'smith',
        prompt: prompt,
        expectedInstanceId: firstInstance,
        expectedRevision: 7,
      );
      expect(c.error, isNull);
      expect(c.revision, 8);
      expect(c.projects.single.name, 'Smith');
      expect(c.projects.single.description, '');
      expect(c.projects.single.prompt, prompt);
      expect(c.projects.single.repositories, corrected);
      expect(api.calls, [
        'projects_list_v3',
        'project_update_v3',
        'projects_list_v3',
      ]);
    },
  );

  test('hidden Name changes reject before fake mutation and preserve the registry and receipts', () async {
    final api = MetadataApi();
    final c = ProjectsController(api);
    addTearDown(c.dispose);
    await c.refresh();
    await c.create(
      'Retained Name',
      [],
      key: 'original',
      expectedInstanceId: firstInstance,
      expectedRevision: 0,
    );
    final saved = jsonEncode(api.projects);
    final receipts = jsonEncode(api.receipts);
    await c.update(
      firstProject,
      'Changed Name',
      [],
      key: 'edited-key',
      prompt: 'must not publish',
      expectedInstanceId: firstInstance,
      expectedRevision: 1,
    );
    expect(api.calls.last, 'project_update_v3');
    expect(c.error, isNotNull);
    expect(c.uncertain, false);
    expect(api.revision, 1);
    expect(jsonEncode(api.projects), saved);
    expect(jsonEncode(api.receipts), receipts);
    expect(c.projects.single.name, 'Retained Name');
    expect(c.projects.single.key, 'original');
    await c.refresh();
    await c.update(
      firstProject,
      'Retained Name',
      [],
      key: 'edited-key',
      expectedInstanceId: firstInstance,
      expectedRevision: 1,
    );
    expect(c.error, isNull);
    expect(c.projects.single.name, 'Retained Name');
    expect(c.projects.single.key, 'edited-key');
  });

  test('invalid and duplicate keys reject locally while own unchanged key is valid', () async {
    final api = MetadataApi();
    final c = ProjectsController(api);
    addTearDown(c.dispose);
    await c.refresh();
    await c.create(
      'First',
      [],
      key: 'first',
      expectedInstanceId: firstInstance,
      expectedRevision: 0,
    );
    for (final key in ['first', '', 'Not-kebab', 'x' * 41]) {
      final before = api.calls.length;
      await c.create(
        'Another',
        [],
        key: key,
        expectedInstanceId: firstInstance,
        expectedRevision: 1,
      );
      expect(api.calls.length, before);
      expect(c.error, isNotNull);
      expect(c.canMutate, true);
    }
    for (final metadata in [
      <String, String>{'description': 'x' * 61},
      <String, String>{'prompt': 'x' * 1001},
    ]) {
      final before = api.calls.length;
      await c.update(
        firstProject,
        'First',
        [],
        key: 'first',
        description: metadata['description'] ?? '',
        prompt: metadata['prompt'] ?? '',
        expectedInstanceId: firstInstance,
        expectedRevision: 1,
      );
      expect(api.calls.length, before);
    }
    await c.update(
      firstProject,
      'First',
      [],
      key: 'first',
      expectedInstanceId: firstInstance,
      expectedRevision: 1,
    );
    expect(c.error, isNull);
    expect(c.revision, 2);
  });
  for (final code in ['OP', 'UPGRADE_REQUIRED']) {
    test(
      '$code list incompatibility cannot authorize a mutation or legacy fallback',
      () async {
        final api = MetadataApi();
        final c = ProjectsController(api);
        addTearDown(c.dispose);
        api.next(
          'projects_list_v3',
          (_) => throw AppFailure(code, 'Incompatible pair'),
        );
        await c.refresh();
        await c.create(
          'Unsaved',
          [ProjectRepository(name: 'repo', url: 'git@host:repo')],
          expectedInstanceId: firstInstance,
          expectedRevision: 0,
        );
        await c.resolvePending();
        expect(api.calls, ['projects_list_v3']);
        expect(c.loaded || c.canMutate || c.uncertain, isFalse);
        expect(c.error, isNotNull);
      },
    );
  }

  test(
    'nested-only stale edits retain every value and never rebind origin',
    () async {
      final api = MetadataApi();
      final c = ProjectsController(api);
      addTearDown(c.dispose);
      await c.refresh();
      final original = ProjectRepository(name: 'repo', url: 'git@host:repo');
      await c.create(
        'Original',
        [original],
        expectedInstanceId: firstInstance,
        expectedRevision: 0,
      );
      final revision = c.revision!;
      final edited = ProjectRepository(
        name: original.name,
        url: original.url,
        branch: ProjectBranches(
          base: 'main',
          work: 'main',
          types: ProjectBranchTypes(
            feature: 'feature/*',
            fix: 'fix/*',
            release: 'release/*',
            hotfix: 'hotfix/*',
          ),
        ),
      );
      api.instance = nextInstance;
      await c.refresh();
      final before = api.calls.length;
      await c.update(
        firstProject,
        'Original',
        [edited],
        expectedInstanceId: firstInstance,
        expectedRevision: revision,
      );
      expect(api.calls, hasLength(before));
      expect(c.projects.single.repositories, [original]);
      expect(c.error, contains('review'));
      await c.update(
        firstProject,
        'Original',
        [edited],
        expectedInstanceId: nextInstance,
        expectedRevision: revision,
      );
      expect(c.projects.single.repositories, [edited]);
      expect(api.arguments[before]['repositories'], [edited.toJson()]);
    },
  );

  test('nested uncertain input is captured once and resolved query-only after restart', () async {
    final api = MetadataApi();
    final c = ProjectsController(api);
    addTearDown(c.dispose);
    await c.refresh();
    final original = ProjectRepository(
      name: 'backend',
      url: 'git@host:repo',
      branch: ProjectBranches(
        base: 'main',
        work: 'dev',
        types: ProjectBranchTypes(
          feature: 'feature/*',
          fix: 'fix/*',
          release: 'release/*',
          hotfix: 'hotfix/*',
        ),
      ),
    );
    final input = [original];
    api.next('project_create_v3', (args) {
      api.commit('project_create_v3', args);
      throw const AppFailure(
        'TRANSPORT',
        'Lost acknowledgement',
        unknown: true,
      );
    });
    await c.create(
      'Original',
      input,
      expectedInstanceId: firstInstance,
      expectedRevision: 0,
    );
    final request = api.arguments[1]['request_id'];
    input[0] = ProjectRepository(url: 'git@host:newer');
    expect(c.uncertain, isTrue);
    api.instance = nextInstance;
    await c.resolvePending();
    expect(api.calls, [
      'projects_list_v3',
      'project_create_v3',
      'project_receipt',
      'projects_list_v3',
    ]);
    expect(api.arguments[2], {'request_id': request});
    expect(c.projects.single.repositories, [original]);
    expect(input.single.url, 'git@host:newer');
    expect(c.uncertain, isFalse);
  });

  test(
    'construction is inert; refresh alone observes an immutable empty registry',
    () async {
      final api = MetadataApi();
      final c = ProjectsController(api);
      addTearDown(c.dispose);
      expect(api.calls, isEmpty);
      expect(c.projects, isEmpty);
      expect(c.revision, isNull);
      expect(c.instanceId, isNull);
      expect(
        c.loading || c.busy || c.uncertain || c.loaded || c.canMutate,
        isFalse,
      );
      expect(c.error, isNull);
      await c.refresh();
      expect(c.loaded, isTrue);
      expect(c.revision, 0);
      expect(c.instanceId, firstInstance);
      expect(c.canMutate, isTrue);
      expect(c.loading || c.busy || c.uncertain, isFalse);
      expect(api.calls, ['projects_list_v3']);
      expect(api.arguments.single, isEmpty);
      expect(() => c.projects.clear(), throwsUnsupportedError);
    },
  );

  test(
    'refresh calls coalesce, including reentrant listener refreshes',
    () async {
      final api = MetadataApi();
      final c = ProjectsController(api);
      addTearDown(c.dispose);
      final read = Completer<AppReply>();
      api.next('projects_list_v3', (_) => read.future);
      Future<void>? reentrant;
      c.addListener(() {
        if (c.loading) reentrant = c.refresh();
      });
      final first = c.refresh();
      final second = c.refresh();
      expect(first, same(second));
      expect(reentrant, same(first));
      expect(c.loading, isTrue);
      expect(c.canMutate, isFalse);
      read.complete(api.listReply());
      await Future.wait([first, second]);
      expect(api.calls, ['projects_list_v3']);
      expect(c.loading, isFalse);
      expect(c.error, isNull);
    },
  );

  test('CRUD sends only exact metadata arguments and always reads after acknowledgement', () async {
    final api = MetadataApi();
    final c = ProjectsController(api);
    addTearDown(c.dispose);
    await c.refresh();
    final repositories = [
      ProjectRepository(name: 'repo', url: 'git@example.org:team/repo'),
    ];
    await c.create(
      'Smith',
      repositories,
      expectedInstanceId: firstInstance,
      expectedRevision: 0,
    );
    expect(c.projects.single.name, 'Smith');
    expect(c.projects.single.repositories, repositories);
    expect(c.projects.single.agent, 'opencode');
    expect(c.revision, 1);
    expect(c.error, isNull);
    expect(c.canMutate, isTrue);
    await c.update(
      firstProject,
      'Smith',
      [],
      key: 'renamed-key',
      prompt: ' changed\n\tcontext ',
      expectedInstanceId: firstInstance,
      expectedRevision: 1,
    );
    expect(c.projects.single.name, 'Smith');
    expect(c.projects.single.key, 'renamed-key');
    expect(c.projects.single.prompt, ' changed\n\tcontext ');
    expect(c.projects.single.repositories, isEmpty);
    await c.delete(
      firstProject,
      expectedInstanceId: firstInstance,
      expectedRevision: 2,
    );
    expect(c.projects, isEmpty);
    expect(c.revision, 3);
    expect(c.error, isNull);
    expect(api.calls, [
      'projects_list_v3',
      'project_create_v3',
      'projects_list_v3',
      'project_update_v3',
      'projects_list_v3',
      'project_delete_v3',
      'projects_list_v3',
    ]);
    final ids = <String>{};
    for (final index in [1, 3, 5]) {
      final args = api.arguments[index];
      expect(args['request_id'], matches(RegExp(r'^[0-9a-f]{32}$')));
      ids.add(args['request_id']);
      expect(args['instance_id'], firstInstance);
      expect(args['expected_revision'], (index - 1) ~/ 2);
      expect(args.keys.toSet(), {
        'request_id',
        'instance_id',
        'expected_revision',
        if (index != 1) 'project_id',
        if (index != 5) ...[
          'name',
          'repositories',
          'agent',
          'key',
          'description',
          'prompt',
        ],
      });
    }
    expect(ids, hasLength(3));
    expect(repositories, [
      ProjectRepository(name: 'repo', url: 'git@example.org:team/repo'),
    ]);
  });

  test('mutation captures immutable inputs and fences reads and competing writes while busy', () async {
    final api = MetadataApi();
    final c = ProjectsController(api);
    addTearDown(c.dispose);
    await c.refresh();
    final acknowledged = Completer<AppReply>();
    Map<String, dynamic>? admitted;
    api.next('project_create_v3', (args) {
      admitted = args;
      return acknowledged.future;
    });
    final original = ProjectRepository(
      name: 'backend',
      url: 'git@host:original',
      branch: ProjectBranches(
        base: 'main',
        work: 'dev',
        types: ProjectBranchTypes(
          feature: 'feature/*',
          fix: 'fix/*',
          release: 'release/*',
          hotfix: 'hotfix/*',
        ),
      ),
    );
    final repositories = [original];
    final action = c.create(
      'Original',
      repositories,
      expectedInstanceId: firstInstance,
      expectedRevision: 0,
    );
    repositories[0] = ProjectRepository(url: 'git@host:changed');
    expect(c.busy, isTrue);
    expect(c.canMutate, isFalse);
    expect(c.uncertain, isFalse);
    expect(c.refresh(), same(action));
    expect(c.resolvePending(), same(action));
    await c.create(
      'Do not dispatch',
      [],
      expectedInstanceId: firstInstance,
      expectedRevision: 0,
    );
    await c.update(
      firstProject,
      'Do not dispatch',
      [],
      expectedInstanceId: firstInstance,
      expectedRevision: 0,
    );
    await c.delete(
      firstProject,
      expectedInstanceId: firstInstance,
      expectedRevision: 0,
    );
    expect(api.calls, ['projects_list_v3', 'project_create_v3']);
    expect(admitted!['repositories'], [original.toJson()]);
    expect(() => admitted!['name'] = 'Tampered', throwsUnsupportedError);
    final captured = (admitted!['repositories'] as List).single as Map;
    expect(() => captured['name'] = 'Tampered', throwsUnsupportedError);
    expect(
      () => captured['branch']['base'] = 'Tampered',
      throwsUnsupportedError,
    );
    expect(
      () => captured['branch']['types']['feature'] = 'Tampered/*',
      throwsUnsupportedError,
    );
    expect(() => captured['branch']['types'].clear(), throwsUnsupportedError);
    expect(
      () => (admitted!['repositories'] as List).clear(),
      throwsUnsupportedError,
    );
    acknowledged.complete(api.commit('project_create_v3', admitted!));
    await action;
    expect(c.projects.single.repositories, [original]);
    expect(c.error, isNull);
    expect(api.calls, [
      'projects_list_v3',
      'project_create_v3',
      'projects_list_v3',
    ]);
  });

  test('editor revision is never silently substituted after an intervening refresh', () async {
    final api = MetadataApi();
    final c = ProjectsController(api);
    addTearDown(c.dispose);
    await c.refresh();
    await c.create(
      'Original',
      [],
      expectedInstanceId: firstInstance,
      expectedRevision: 0,
    );
    final editorRevision = c.revision!;
    const name = 'Original';
    const key = 'unsaved-edit';
    final repositories = [
      ProjectRepository(name: 'repo', url: 'git@host:unsaved'),
    ];
    api.projects.single['key'] = 'other-window';
    api.revision++;
    await c.refresh();
    final before = api.calls.length;
    await c.update(
      firstProject,
      name,
      repositories,
      key: key,
      expectedInstanceId: firstInstance,
      expectedRevision: editorRevision,
    );
    await c.delete(
      firstProject,
      expectedInstanceId: firstInstance,
      expectedRevision: editorRevision,
    );
    await c.create(
      name,
      repositories,
      expectedInstanceId: firstInstance,
      expectedRevision: editorRevision,
    );
    expect(api.calls, hasLength(before));
    expect(c.error, contains('review'));
    expect(c.projects.single.name, 'Original');
    expect(c.projects.single.key, 'other-window');
    expect(key, 'unsaved-edit');
    expect(repositories, [
      ProjectRepository(name: 'repo', url: 'git@host:unsaved'),
    ]);
    api.projects.clear();
    api.revision++;
    await c.refresh();
    await c.update(
      firstProject,
      name,
      repositories,
      expectedInstanceId: firstInstance,
      expectedRevision: c.revision!,
    );
    expect(c.error, contains('no longer exists'));
    expect(api.calls.last, 'projects_list_v3');
    expect(c.projects, isEmpty);
  });

  for (final op in [
    'project_create_v3',
    'project_update_v3',
    'project_delete_v3',
  ]) {
    test(
      '$op never rebinds an old editor origin after a same-revision restart',
      () async {
        final api = MetadataApi();
        final c = ProjectsController(api);
        addTearDown(c.dispose);
        await c.refresh();
        await c.create(
          'Original',
          [],
          expectedInstanceId: firstInstance,
          expectedRevision: 0,
        );
        final editorInstance = c.instanceId!;
        final editorRevision = c.revision!;
        const name = 'Original';
        const key = 'unsaved-editor-key';
        final repositories = [
          ProjectRepository(name: 'repo', url: 'git@host:unsaved'),
        ];
        Future<void> save(String instance, int revision) => switch (op) {
          'project_create_v3' => c.create(
            name,
            repositories,
            expectedInstanceId: instance,
            expectedRevision: revision,
          ),
          'project_update_v3' => c.update(
            firstProject,
            name,
            repositories,
            key: key,
            expectedInstanceId: instance,
            expectedRevision: revision,
          ),
          _ => c.delete(
            firstProject,
            expectedInstanceId: instance,
            expectedRevision: revision,
          ),
        };
        api.instance = nextInstance;
        api.projects.single['key'] = 'observed-after-restart';
        await c.refresh();
        expect(c.instanceId, nextInstance);
        expect(c.revision, editorRevision);
        final before = api.calls.length;
        await save(editorInstance, editorRevision);
        expect(api.calls, hasLength(before));
        expect(c.error, contains('review'));
        expect(c.uncertain || c.busy, isFalse);
        expect(c.projects.single.name, 'Original');
        expect(c.projects.single.key, 'observed-after-restart');
        expect(key, 'unsaved-editor-key');
        expect(repositories, [
          ProjectRepository(name: 'repo', url: 'git@host:unsaved'),
        ]);
        // A separately reviewed action can explicitly capture the new origin.
        await save(c.instanceId!, c.revision!);
        expect(api.calls[before], op);
        expect(api.arguments[before]['instance_id'], nextInstance);
        expect(api.arguments[before]['expected_revision'], editorRevision);
        expect(c.error, isNull);
        expect(c.canMutate, isTrue);
      },
    );
  }

  test('invalid or unobserved caller instances never reach dispatch', () async {
    final api = MetadataApi();
    final c = ProjectsController(api);
    addTearDown(c.dispose);
    await c.refresh();
    for (final origin in [
      '',
      'invalid',
      firstInstance.toUpperCase(),
      nextInstance,
    ]) {
      await c.create(
        'Unsaved',
        [],
        expectedInstanceId: origin,
        expectedRevision: 0,
      );
      expect(api.calls, ['projects_list_v3']);
      expect(c.error, isNotNull);
      expect(c.canMutate, isTrue);
      expect(c.uncertain, isFalse);
    }
  });

  test('a late list cannot race a write or regress a newer revision', () async {
    final api = MetadataApi();
    final c = ProjectsController(api);
    addTearDown(c.dispose);
    await c.refresh();
    await c.create(
      'Original',
      [],
      expectedInstanceId: firstInstance,
      expectedRevision: 0,
    );
    final oldRevision = c.revision!;
    final read = Completer<AppReply>();
    api.next('projects_list_v3', (_) => read.future);
    final refresh = c.refresh();
    await c.update(
      firstProject,
      'Stale form',
      [],
      expectedInstanceId: firstInstance,
      expectedRevision: oldRevision,
    );
    expect(api.calls.where((op) => op == 'project_update_v3'), isEmpty);
    api.projects.single['key'] = 'newer-authoritative-key';
    api.revision++;
    read.complete(api.listReply());
    await refresh;
    expect(c.projects.single.key, 'newer-authoritative-key');
    expect(c.projects.single.name, 'Original');
    expect(c.revision, 2);
    api.next(
      'projects_list_v3',
      (_) => AppReply(firstInstance, {'revision': 1, 'projects': []}),
    );
    await c.refresh();
    expect(c.projects.single.key, 'newer-authoritative-key');
    expect(c.projects.single.name, 'Original');
    expect(c.revision, 2);
    expect(c.error, isNotNull);
    expect(c.canMutate, isFalse);
  });

  test(
    'invalid input, unsupported targets and full registries do not dispatch',
    () async {
      final api = MetadataApi();
      final c = ProjectsController(api);
      addTearDown(c.dispose);
      await c.refresh();
      for (final action in [
        () => c.create(
          ' bad ',
          [],
          expectedInstanceId: firstInstance,
          expectedRevision: 0,
        ),
        () => c.create(
          'A' * 41,
          [],
          expectedInstanceId: firstInstance,
          expectedRevision: 0,
        ),
        () => c.create(
          'Valid',
          List.generate(
            17,
            (i) => ProjectRepository(name: 'repo$i', url: 'git@host:repo$i'),
          ),
          expectedInstanceId: firstInstance,
          expectedRevision: 0,
        ),
        () => c.create(
          'Valid',
          [
            ProjectRepository(name: 'repo', url: 'git@host:repo'),
            ProjectRepository(name: 'repo', url: 'git@host:repo'),
          ],
          expectedInstanceId: firstInstance,
          expectedRevision: 0,
        ),
        () => c.create(
          'Valid',
          [],
          expectedInstanceId: firstInstance,
          expectedRevision: -1,
        ),
        () => c.update(
          'not-an-id',
          'Valid',
          [],
          expectedInstanceId: firstInstance,
          expectedRevision: 0,
        ),
        () => c.delete(
          firstProject,
          expectedInstanceId: firstInstance,
          expectedRevision: 0,
        ),
      ]) {
        await action();
        expect(c.error, isNotNull);
        expect(c.busy || c.uncertain, isFalse);
        expect(c.canMutate, isTrue);
      }
      expect(api.calls, ['projects_list_v3']);
      api.projects.addAll(
        List.generate(
          32,
          (i) => {
            'project_id': i.toRadixString(16).padLeft(32, '0'),
            'name': 'Project $i',
            'key': 'project-$i',
            'description': '',
            'prompt': '',
            'repositories': <dynamic>[],
            'agent': 'opencode',
          },
        ),
      );
      await c.refresh();
      await c.create(
        'Overflow',
        [],
        expectedInstanceId: firstInstance,
        expectedRevision: 0,
      );
      expect(c.error, contains('capacity'));
      expect(api.calls, ['projects_list_v3', 'projects_list_v3']);
      expect(c.canMutate, isTrue);
      api.revision = maxProjectRevision;
      await c.refresh();
      expect(c.canMutate, isFalse);
      await c.delete(
        api.projects.first['project_id'],
        expectedInstanceId: firstInstance,
        expectedRevision: maxProjectRevision,
      );
      expect(api.calls.last, 'projects_list_v3');
    },
  );

  for (final code in [
    'STALE_REVISION',
    'INSTANCE_MISMATCH',
    'CAPACITY',
    'PROJECT_NOT_FOUND',
    'INVALID_REQUEST',
    'OP',
    'UPGRADE_REQUIRED',
  ]) {
    test(
      '$code is a definitive rejection, preserves form and requires fresh observation',
      () async {
        final api = MetadataApi();
        final c = ProjectsController(api);
        addTearDown(c.dispose);
        await c.refresh();
        await c.create(
          'Original',
          [],
          expectedInstanceId: firstInstance,
          expectedRevision: 0,
        );
        api.next(
          'project_update_v3',
          (_) => throw AppFailure(code, 'Untrusted diagnostic /private/hidden'),
        );
        final repositories = [
          ProjectRepository(name: 'repo', url: 'git@host:unsaved'),
        ];
        await c.update(
          firstProject,
          'Original',
          repositories,
          key: 'unsaved-form',
          expectedInstanceId: firstInstance,
          expectedRevision: 1,
        );
        expect(c.projects.single.name, 'Original');
        expect(repositories, [
          ProjectRepository(name: 'repo', url: 'git@host:unsaved'),
        ]);
        expect(c.busy || c.uncertain, isFalse);
        expect(c.canMutate, isFalse);
        expect(c.error, isNot(contains('/private/hidden')));
        expect(api.calls.last, 'project_update_v3');
        await c.resolvePending();
        expect(api.calls.last, 'project_update_v3');
        await c.refresh();
        expect(c.canMutate, isTrue);
        expect(
          api.calls.where((op) => op == 'project_update_v3'),
          hasLength(1),
        );
      },
    );
  }

  for (final fault in <Object>[
    const AppFailure('TRANSPORT', 'Lost acknowledgement', unknown: true),
    const AppFailure('TRANSPORT', 'Unclassified transport'),
    const AppFailure('STATE_UNAVAILABLE', 'Publication uncertain'),
    const AppFailure('PROTOCOL', 'Malformed acknowledgement'),
    const AppFailure('FAULT', 'Unclassified fault'),
    const AppFailure('STALE_REVISION', 'But outcome unknown', unknown: true),
    const AppFailure('INVALID_REQUEST', 'But outcome unknown', unknown: true),
    StateError('Unexpected failure'),
  ]) {
    test(
      '$fault retains original identity and blocks every write without replay',
      () async {
        final api = MetadataApi();
        final c = ProjectsController(api);
        addTearDown(c.dispose);
        await c.refresh();
        api.next('project_create_v3', (args) {
          api.commit('project_create_v3', args);
          throw fault;
        });
        await c.create(
          'Committed but unacknowledged',
          [],
          expectedInstanceId: firstInstance,
          expectedRevision: 0,
        );
        final original = api.arguments[1];
        expect(c.projects, isEmpty);
        expect(c.revision, 0);
        expect(c.uncertain, isTrue);
        expect(c.canMutate || c.busy, isFalse);
        await c.create(
          'Replacement forbidden',
          [],
          expectedInstanceId: firstInstance,
          expectedRevision: 0,
        );
        await c.update(
          firstProject,
          'Replacement forbidden',
          [],
          expectedInstanceId: firstInstance,
          expectedRevision: 0,
        );
        await c.delete(
          firstProject,
          expectedInstanceId: firstInstance,
          expectedRevision: 0,
        );
        expect(api.calls, ['projects_list_v3', 'project_create_v3']);
        await c.refresh();
        expect(c.projects.single.name, 'Committed but unacknowledged');
        expect(c.uncertain, isTrue);
        expect(c.canMutate, isFalse);
        api.instance = nextInstance;
        await c.resolvePending();
        expect(api.calls, [
          'projects_list_v3',
          'project_create_v3',
          'projects_list_v3',
          'project_receipt',
          'projects_list_v3',
        ]);
        expect(api.arguments[3], {'request_id': original['request_id']});
        expect(original['instance_id'], firstInstance);
        expect(c.uncertain || c.busy || c.loading, isFalse);
        expect(c.canMutate, isTrue);
        expect(c.error, isNull);
        expect(c.revision, 1);
      },
    );
  }

  test('unknown and expired receipts remain observation-only despite a healthy list', () async {
    final api = MetadataApi();
    final c = ProjectsController(api);
    addTearDown(c.dispose);
    await c.refresh();
    api.next(
      'project_create_v3',
      (_) => throw const AppFailure('TRANSPORT', 'Unknown', unknown: true),
    );
    await c.create(
      'Unknown form',
      [],
      expectedInstanceId: firstInstance,
      expectedRevision: 0,
    );
    final id = api.arguments[1]['request_id'];
    await c.resolvePending();
    expect(c.error, contains('unknown'));
    api.next(
      'project_receipt',
      (_) => throw const AppFailure('REQUEST_EXPIRED', 'Expired'),
    );
    await c.resolvePending();
    expect(c.error, contains('expired'));
    api.next(
      'project_receipt',
      (_) => throw const AppFailure('STATE_UNAVAILABLE', 'Fault'),
    );
    await c.resolvePending();
    await c.refresh();
    expect(c.uncertain, isTrue);
    expect(c.canMutate, isFalse);
    expect(api.calls.where((op) => op == 'project_create_v3'), hasLength(1));
    for (var i = 0; i < api.calls.length; i++) {
      if (api.calls[i] == 'project_receipt') {
        expect(api.arguments[i], {'request_id': id});
      }
    }
  });

  for (final code in ['REQUEST_UNKNOWN', 'REQUEST_EXPIRED']) {
    test(
      '$code resolution coalesces a current-list read without releasing the original request',
      () async {
        final api = MetadataApi();
        final c = ProjectsController(api);
        addTearDown(c.dispose);
        await c.refresh();
        api.next('project_create_v3', (args) {
          api.commit('project_create_v3', args);
          throw const AppFailure(
            'TRANSPORT',
            'Acknowledgement lost',
            unknown: true,
          );
        });
        await c.create(
          'Unconfirmed',
          [],
          expectedInstanceId: firstInstance,
          expectedRevision: 0,
        );
        final request = api.arguments[1]['request_id'];
        api.receipts.clear();
        api.instance = nextInstance;
        api.projects.single['key'] = 'changed-in-another-window';
        api.revision++;
        api.next(
          'project_receipt',
          (_) => throw AppFailure(code, 'Receipt unavailable'),
        );
        final read = Completer<AppReply>();
        api.next('projects_list_v3', (_) => read.future);
        final resolution = c.resolvePending();
        await flush();
        expect(api.calls, [
          'projects_list_v3',
          'project_create_v3',
          'project_receipt',
          'projects_list_v3',
        ]);
        expect(api.arguments[2], {'request_id': request});
        expect(c.loading && c.busy && c.uncertain, isTrue);
        expect(c.canMutate, isFalse);
        expect(c.refresh(), same(resolution));
        expect(c.resolvePending(), same(resolution));
        read.complete(api.listReply());
        await resolution;
        expect(c.projects.single.name, 'Unconfirmed');
        expect(c.projects.single.key, 'changed-in-another-window');
        expect(c.revision, 2);
        expect(c.instanceId, nextInstance);
        expect(c.uncertain, isTrue);
        expect(c.canMutate || c.busy || c.loading, isFalse);
        expect(
          c.error,
          contains(code == 'REQUEST_UNKNOWN' ? 'unknown' : 'expired'),
        );
        final before = api.calls.length;
        await c.create(
          'Forbidden replacement',
          [],
          expectedInstanceId: nextInstance,
          expectedRevision: 2,
        );
        await c.update(
          firstProject,
          'Forbidden replacement',
          [],
          expectedInstanceId: nextInstance,
          expectedRevision: 2,
        );
        await c.delete(
          firstProject,
          expectedInstanceId: nextInstance,
          expectedRevision: 2,
        );
        expect(api.calls, hasLength(before));
        api.next(
          'project_receipt',
          (_) => throw AppFailure(code, 'Still unavailable'),
        );
        await c.resolvePending();
        expect(api.arguments[before], {'request_id': request});
        expect(api.calls.sublist(before), [
          'project_receipt',
          'projects_list_v3',
        ]);
        api.projects.single['key'] = 'observed-by-explicit-query-only-refresh';
        api.revision++;
        await c.refresh();
        expect(
          c.projects.single.key,
          'observed-by-explicit-query-only-refresh',
        );
        expect(c.uncertain, isTrue);
        expect(c.canMutate, isFalse);
        expect(
          api.calls.where((op) => op == 'project_create_v3'),
          hasLength(1),
        );
      },
    );

    test(
      '$code plus list outage preserves the last snapshot and the original uncertainty',
      () async {
        final api = MetadataApi();
        final c = ProjectsController(api);
        addTearDown(c.dispose);
        await c.refresh();
        await c.create(
          'Original',
          [],
          expectedInstanceId: firstInstance,
          expectedRevision: 0,
        );
        api.next('project_update_v3', (args) {
          api.commit('project_update_v3', args);
          throw const AppFailure('STATE_UNAVAILABLE', 'Publication uncertain');
        });
        await c.update(
          firstProject,
          'Original',
          [],
          key: 'unconfirmed',
          prompt: ' exact unconfirmed\n\tprompt ',
          expectedInstanceId: firstInstance,
          expectedRevision: 1,
        );
        final request = api.arguments[3]['request_id'];
        api.receipts.clear();
        api.next(
          'project_receipt',
          (_) => throw AppFailure(code, 'No receipt'),
        );
        api.next(
          'projects_list_v3',
          (_) => throw const AppFailure(
            'TRANSPORT',
            'Private path /private/hidden',
          ),
        );
        await c.resolvePending();
        expect(api.calls.sublist(4), ['project_receipt', 'projects_list_v3']);
        expect(c.projects.single.name, 'Original');
        expect(c.revision, 1);
        expect(c.instanceId, firstInstance);
        expect(c.uncertain, isTrue);
        expect(c.canMutate || c.busy || c.loading, isFalse);
        expect(
          c.error,
          contains(code == 'REQUEST_UNKNOWN' ? 'unknown' : 'expired'),
        );
        expect(c.error, contains('unavailable'));
        expect(c.error, isNot(contains('/private/hidden')));
        api.next(
          'project_receipt',
          (_) => throw AppFailure(code, 'Still unavailable'),
        );
        await c.resolvePending();
        expect(api.arguments[6], {'request_id': request});
        expect(c.projects.single.name, 'Original');
        expect(c.projects.single.key, 'unconfirmed');
        expect(c.projects.single.prompt, ' exact unconfirmed\n\tprompt ');
        expect(c.revision, 2);
        expect(c.uncertain, isTrue);
        expect(c.canMutate, isFalse);
        expect(
          api.calls.where((op) => op == 'project_update_v3'),
          hasLength(1),
        );
      },
    );

    test('$code fallback list callbacks are fenced after disposal', () async {
      final api = MetadataApi();
      final c = ProjectsController(api);
      await c.refresh();
      api.next('project_create_v3', (args) {
        api.commit('project_create_v3', args);
        throw const AppFailure(
          'TRANSPORT',
          'Lost acknowledgement',
          unknown: true,
        );
      });
      await c.create(
        'Unconfirmed',
        [],
        expectedInstanceId: firstInstance,
        expectedRevision: 0,
      );
      api.receipts.clear();
      api.next('project_receipt', (_) => throw AppFailure(code, 'No receipt'));
      final read = Completer<AppReply>();
      api.next('projects_list_v3', (_) => read.future);
      var notifications = 0;
      c.addListener(() => notifications++);
      final resolution = c.resolvePending();
      await flush();
      expect(api.calls.last, 'projects_list_v3');
      final before = notifications;
      final calls = api.calls.length;
      c.dispose();
      read.complete(api.listReply());
      await resolution;
      expect(c.projects, isEmpty);
      expect(c.revision, 0);
      expect(notifications, before);
      expect(api.calls, hasLength(calls));
      expect(api.disposed, 0);
    });
  }

  test('historical retained receipt after deletion and restart cannot resurrect a project', () async {
    final api = MetadataApi();
    final c = ProjectsController(api);
    addTearDown(c.dispose);
    await c.refresh();
    await c.create(
      'Original',
      [],
      expectedInstanceId: firstInstance,
      expectedRevision: 0,
    );
    api.next('project_update_v3', (args) {
      api.commit('project_update_v3', args);
      throw const AppFailure(
        'TRANSPORT',
        'Lost acknowledgement',
        unknown: true,
      );
    });
    await c.update(
      firstProject,
      'Original',
      [],
      key: 'later-deleted',
      expectedInstanceId: firstInstance,
      expectedRevision: 1,
    );
    final originalId = api.arguments[3]['request_id'];
    api.projects.clear();
    api.revision = 3;
    api.instance = nextInstance;
    await c.resolvePending();
    expect(api.arguments[4], {'request_id': originalId});
    expect(c.projects, isEmpty);
    expect(c.revision, 3);
    expect(c.uncertain, isFalse);
    expect(c.error, isNull);
    expect(api.calls, [
      'projects_list_v3',
      'project_create_v3',
      'projects_list_v3',
      'project_update_v3',
      'project_receipt',
      'projects_list_v3',
    ]);
  });

  test(
    'uncertain delete resolves its original deleted receipt without replay',
    () async {
      final api = MetadataApi();
      final c = ProjectsController(api);
      addTearDown(c.dispose);
      await c.refresh();
      await c.create(
        'Original',
        [],
        expectedInstanceId: firstInstance,
        expectedRevision: 0,
      );
      api.next('project_delete_v3', (args) {
        api.commit('project_delete_v3', args);
        throw const AppFailure(
          'TRANSPORT',
          'Lost acknowledgement',
          unknown: true,
        );
      });
      await c.delete(
        firstProject,
        expectedInstanceId: firstInstance,
        expectedRevision: 1,
      );
      final original = api.arguments[3]['request_id'];
      expect(c.projects.single.name, 'Original');
      expect(c.uncertain, isTrue);
      api.instance = nextInstance;
      await c.resolvePending();
      expect(api.arguments[4], {'request_id': original});
      expect(c.projects, isEmpty);
      expect(c.revision, 2);
      expect(c.uncertain, isFalse);
      expect(c.error, isNull);
      expect(api.calls.where((op) => op == 'project_delete_v3'), hasLength(1));
    },
  );

  test('acknowledgement never substitutes for a fresh list; outage retains receipt', () async {
    final api = MetadataApi();
    final c = ProjectsController(api);
    addTearDown(c.dispose);
    await c.refresh();
    api.next(
      'projects_list_v3',
      (_) => throw const AppFailure('TRANSPORT', 'List unavailable'),
    );
    await c.create(
      'Acknowledged',
      [],
      expectedInstanceId: firstInstance,
      expectedRevision: 0,
    );
    expect(c.projects, isEmpty);
    expect(c.revision, 0);
    expect(c.uncertain, isTrue);
    expect(c.error, contains('acknowledged'));
    final original = api.arguments[1]['request_id'];
    await c.resolvePending();
    expect(api.arguments[3], {'request_id': original});
    expect(c.projects.single.name, 'Acknowledged');
    expect(c.revision, 1);
    expect(c.error, isNull);
    expect(c.canMutate, isTrue);
  });

  test('INVALID_REQUEST after acknowledgement or in receipt queries cannot clear pending work', () async {
    final api = MetadataApi();
    final c = ProjectsController(api);
    addTearDown(c.dispose);
    await c.refresh();
    api.next(
      'projects_list_v3',
      (_) => throw const AppFailure('INVALID_REQUEST', 'List refused'),
    );
    await c.create(
      'Acknowledged',
      [],
      expectedInstanceId: firstInstance,
      expectedRevision: 0,
    );
    final id = api.arguments[1]['request_id'];
    expect(c.uncertain, isTrue);
    expect(c.projects, isEmpty);
    expect(c.canMutate, isFalse);
    api.next(
      'project_receipt',
      (_) => throw const AppFailure('INVALID_REQUEST', 'Receipt query refused'),
    );
    await c.resolvePending();
    expect(c.uncertain, isTrue);
    expect(c.canMutate, isFalse);
    expect(api.arguments[3], {'request_id': id});
    await c.resolvePending();
    expect(api.arguments[4], {'request_id': id});
    expect(c.projects.single.name, 'Acknowledged');
    expect(c.error, isNull);
    expect(c.uncertain, isFalse);
    expect(api.calls, [
      'projects_list_v3',
      'project_create_v3',
      'projects_list_v3',
      'project_receipt',
      'project_receipt',
      'projects_list_v3',
    ]);
  });

  test(
    'pre-acknowledgement snapshots are refused even after a valid receipt',
    () async {
      final api = MetadataApi();
      final c = ProjectsController(api);
      addTearDown(c.dispose);
      await c.refresh();
      final stale = api.listReply();
      api.next('projects_list_v3', (_) => stale);
      await c.create(
        'Acknowledged',
        [],
        expectedInstanceId: firstInstance,
        expectedRevision: 0,
      );
      expect(c.uncertain, isTrue);
      expect(c.projects, isEmpty);
      expect(c.revision, 0);
      await c.resolvePending();
      expect(c.projects.single.name, 'Acknowledged');
      expect(c.revision, 1);
      expect(c.uncertain, isFalse);
    },
  );

  for (final change in <Map<String, dynamic>>[
    {'request_id': 'd' * 32},
    {'project_id': 'd' * 32},
    {'deleted': true},
    {'revision': 0},
    {'revision': 3},
    {'extra': 'not allowed'},
  ]) {
    test('mismatched receipt $change never clears pending identity', () async {
      final api = MetadataApi();
      final c = ProjectsController(api);
      addTearDown(c.dispose);
      await c.refresh();
      await c.create(
        'Original',
        [],
        expectedInstanceId: firstInstance,
        expectedRevision: 0,
      );
      api.next('project_update_v3', (args) {
        final reply = api.commit('project_update_v3', args);
        return AppReply(reply.instanceId, {...reply.result, ...change});
      });
      await c.update(
        firstProject,
        'Original',
        [],
        key: 'changed-key',
        expectedInstanceId: firstInstance,
        expectedRevision: 1,
      );
      expect(c.uncertain, isTrue);
      expect(c.projects.single.name, 'Original');
      final original = api.arguments[3]['request_id'];
      api.next(
        'project_receipt',
        (_) => AppReply(firstInstance, {...api.receipts[original]!, ...change}),
      );
      await c.resolvePending();
      expect(c.uncertain, isTrue);
      expect(api.calls.last, 'project_receipt');
      await c.resolvePending();
      expect(c.uncertain, isFalse);
      expect(c.projects.single.name, 'Original');
      expect(c.projects.single.key, 'changed-key');
      expect(api.calls.where((op) => op == 'project_update_v3'), hasLength(1));
    });
  }

  test(
    'resolution is coalesced and waits behind an already-started refresh',
    () async {
      final api = MetadataApi();
      final c = ProjectsController(api);
      addTearDown(c.dispose);
      await c.refresh();
      api.next('project_create_v3', (args) {
        api.commit('project_create_v3', args);
        throw const AppFailure('STATE_UNAVAILABLE', 'Unknown publication');
      });
      await c.create(
        'Original',
        [],
        expectedInstanceId: firstInstance,
        expectedRevision: 0,
      );
      final read = Completer<AppReply>();
      api.next('projects_list_v3', (_) => read.future);
      final refresh = c.refresh();
      final first = c.resolvePending();
      final second = c.resolvePending();
      expect(api.calls.last, 'projects_list_v3');
      read.complete(api.listReply());
      await Future.wait([refresh, first, second]);
      expect(api.calls.where((op) => op == 'project_receipt'), hasLength(1));
      expect(c.uncertain, isFalse);
      expect(c.canMutate, isTrue);
    },
  );

  test('failed and malformed reads preserve last valid list and expose no raw diagnostics', () async {
    final api = MetadataApi();
    final c = ProjectsController(api);
    addTearDown(c.dispose);
    api.next(
      'projects_list_v3',
      (_) => throw const AppFailure(
        'STATE_UNAVAILABLE',
        'Private path /private/hidden',
      ),
    );
    await c.refresh();
    expect(c.loaded, isFalse);
    expect(c.revision, isNull);
    expect(c.error, isNot(contains('/private/hidden')));
    await c.refresh();
    await c.create(
      'Saved',
      [],
      expectedInstanceId: firstInstance,
      expectedRevision: 0,
    );
    for (final bad in [
      AppReply(firstInstance, {'revision': 2, 'projects': [], 'extra': true}),
      AppReply('invalid-instance', {'revision': 2, 'projects': []}),
      AppReply(firstInstance, {
        'revision': 2,
        'projects': [
          {'project_id': firstProject},
        ],
      }),
    ]) {
      api.next('projects_list_v3', (_) => bad);
      await c.refresh();
      expect(c.loaded, isTrue);
      expect(c.projects.single.name, 'Saved');
      expect(c.revision, 1);
      expect(c.canMutate, isFalse);
      expect(c.error, isNotNull);
    }
    await c.refresh();
    expect(c.error, isNull);
    expect(c.canMutate, isTrue);
  });

  for (final phase in ['list', 'mutation', 'receipt']) {
    test(
      'disposal fences late $phase callbacks and never disposes the shared API',
      () async {
        final api = MetadataApi();
        final c = ProjectsController(api);
        final late = Completer<AppReply>();
        var notifications = 0;
        c.addListener(() => notifications++);
        late Future<void> work;
        if (phase == 'list') {
          api.next('projects_list_v3', (_) => late.future);
          work = c.refresh();
        } else {
          await c.refresh();
          if (phase == 'mutation') {
            api.next('project_create_v3', (_) => late.future);
            work = c.create(
              'Pending',
              [],
              expectedInstanceId: firstInstance,
              expectedRevision: 0,
            );
          } else {
            api.next('project_create_v3', (args) {
              api.commit('project_create_v3', args);
              throw const AppFailure('TRANSPORT', 'Unknown', unknown: true);
            });
            await c.create(
              'Pending',
              [],
              expectedInstanceId: firstInstance,
              expectedRevision: 0,
            );
            api.next('project_receipt', (_) => late.future);
            work = c.resolvePending();
          }
        }
        final beforeNotifications = notifications;
        final beforeCalls = api.calls.length;
        final beforeRevision = c.revision;
        c.dispose();
        late.complete(api.listReply());
        await work;
        await flush();
        expect(notifications, beforeNotifications);
        expect(api.calls, hasLength(beforeCalls));
        expect(c.revision, beforeRevision);
        expect(api.disposed, 0);
        expect(c.canMutate, isFalse);
        await c.refresh();
        await c.create(
          'Disposed',
          [],
          expectedInstanceId: firstInstance,
          expectedRevision: 0,
        );
        await c.resolvePending();
        expect(api.calls, hasLength(beforeCalls));
        c.dispose();
      },
    );
  }

  test(
    'reentrant disposal before dispatch prevents metadata exchanges',
    () async {
      final reported = <FlutterErrorDetails>[];
      final previous = FlutterError.onError;
      FlutterError.onError = reported.add;
      addTearDown(() => FlutterError.onError = previous);
      final api = MetadataApi();
      final c = ProjectsController(api);
      await c.refresh();
      c.addListener(() {
        if (c.busy) c.dispose();
      });
      await c.create(
        'Do not dispatch',
        [],
        expectedInstanceId: firstInstance,
        expectedRevision: 0,
      );
      expect(api.calls, ['projects_list_v3']);
      expect(api.disposed, 0);
      expect(reported, isEmpty);
    },
  );
}
