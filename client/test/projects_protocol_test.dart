import 'dart:async';
import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/cloud_routing.dart';
import 'package:smith_client/projects.dart';
import 'package:smith_client/projects_controller.dart';

const instanceId = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
const requestId = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';
const projectId = 'cccccccccccccccccccccccccccccccc';

Map<String, dynamic> record() => {
  'project_id': projectId,
  'name': 'Smith',
  'key': 'smith',
  'description': '',
  'prompt': '',
  'repositories': [
    ProjectRepository(url: 'https://example.org/team/repo.git').toJson(),
  ],
  'agent': 'opencode',
};

Map<String, dynamic> snapshot() => {
  'revision': 7,
  'projects': [record()],
};

Map<String, dynamic> receipt({bool deleted = false}) => {
  'request_id': requestId,
  'project_id': projectId,
  'revision': 8,
  'deleted': deleted,
};

Map<String, dynamic> arguments(String op) => switch (op) {
  'projects_list_v3' => {},
  'project_receipt' => {'request_id': requestId},
  _ => {
    'request_id': requestId,
    'instance_id': instanceId,
    'expected_revision': 7,
    if (op != 'project_create_v3') 'project_id': projectId,
    if (op != 'project_delete_v3') ...{
      'name': 'Smith',
      'key': 'smith',
      'description': '',
      'prompt': '',
      'repositories': [
        ProjectRepository(
          name: 'repo',
          url: 'https://example.org/team/repo.git',
        ).toJson(),
      ],
      'agent': 'opencode',
    },
  },
};

class ProjectsExecutor implements AppExecutor {
  int schema = 3;
  String instance = instanceId;
  Map<String, dynamic>? result;
  Map<String, dynamic>? error;
  final requests = <Map<String, dynamic>>[];
  Completer<void>? gate;

  @override
  Future<String> exchange(List<int> frame) async {
    expect(frame.last, 10);
    final req = jsonDecode(utf8.decode(frame)) as Map<String, dynamic>;
    requests.add(req);
    await gate?.future;
    return jsonEncode({
      'schema_version': schema,
      'service': 'smith-next',
      'protocol': 'app',
      'pid': 123,
      'instance_id': instance,
      'call_id': req['call_id'],
      'ok': error == null,
      'result': error != null
          ? null
          : result ??
                (req['op'] == 'projects_list_v3'
                    ? snapshot()
                    : receipt(deleted: req['op'] == 'project_delete_v3')),
      'error': error,
    });
  }

  @override
  void dispose() {}
}

void main() {
  final protocolFailure = throwsA(
    isA<AppFailure>().having((e) => e.code, 'code', 'PROTOCOL'),
  );

  test('v3 metadata scalar bounds and exact text are validated without normalization', () {
    for (final key in ['a', '0', 'smith-2', 'a' * 40]) {
      expect(projectKeyError(key), isNull);
      ProjectRecord.parse({...record(), 'key': key});
    }
    for (final key in [
      '',
      '-a',
      'a-',
      'a--b',
      'A',
      'a_b',
      'a b',
      'é',
      'a' * 41,
      'a\n',
    ]) {
      expect(projectKeyError(key), isNotNull);
      expect(
        () => ProjectRecord.parse({...record(), 'key': key}),
        protocolFailure,
      );
    }
    for (final description in ['', ' exact ', '🛠' * 60, 'e\u0301' * 30]) {
      expect(projectDescriptionError(description), isNull);
      expect(
        ProjectRecord.parse({...record(), 'description': description})
            .description,
        description,
      );
    }
    for (final prompt in [
      '',
      ' exact\n\tprompt ',
      '🛠' * 1000,
      'e\u0301' * 500,
      '\u2028\u2029',
    ]) {
      expect(projectPromptError(prompt), isNull);
      expect(
        ProjectRecord.parse({...record(), 'prompt': prompt}).prompt,
        prompt,
      );
    }
    for (final description in [
      '🛠' * 61,
      'e\u0301' * 31,
      '\u2028',
      '\u2029',
      String.fromCharCode(0xd800),
      String.fromCharCode(0xdc00),
      for (var c = 0; c < 32; c++) String.fromCharCode(c),
      for (var c = 127; c < 160; c++) String.fromCharCode(c),
    ]) {
      expect(projectDescriptionError(description), isNotNull);
      expect(
        () => ProjectRecord.parse({...record(), 'description': description}),
        protocolFailure,
      );
    }
    for (final prompt in [
      '🛠' * 1001,
      'e\u0301' * 501,
      String.fromCharCode(0xd800),
      String.fromCharCode(0xdc00),
      for (var c = 0; c < 32; c++)
        if (c != 9 && c != 10) String.fromCharCode(c),
      for (var c = 127; c < 160; c++) String.fromCharCode(c),
    ]) {
      expect(projectPromptError(prompt), isNotNull);
      expect(
        () => ProjectRecord.parse({...record(), 'prompt': prompt}),
        protocolFailure,
      );
    }
    expect(
      () => ProjectSnapshot.parse({
        'revision': 0,
        'projects': [
          record(),
          {...record(), 'project_id': requestId},
        ],
      }),
      protocolFailure,
    );
  });

  test('read-only is required on wire, defaults true only in constructors and participates in equality', () {
    final defaultRepository = ProjectRepository(url: 'git@host:repo');
    final writable = ProjectRepository(
      url: defaultRepository.url,
      readOnly: false,
    );
    expect(defaultRepository.readOnly, true);
    expect(writable, isNot(defaultRepository));
    expect(ProjectRepository.parse(writable.toJson()).readOnly, false);
    for (final invalid in [null, 0, 1, 'true', [], {}]) {
      expect(
        () => ProjectRepository.parse({
          ...defaultRepository.toJson(),
          'read_only': invalid,
        }),
        protocolFailure,
      );
    }
    expect(
      () => ProjectRepository.parse(
        defaultRepository.toJson()..remove('read_only'),
      ),
      protocolFailure,
    );
    final original = ProjectRecord.parse(record());
    expect(original, ProjectRecord.parse(record()));
    expect(original.hashCode, ProjectRecord.parse(record()).hashCode);
    for (final change in <String, dynamic>{
      'key': 'changed',
      'description': 'changed',
      'prompt': 'changed',
      'name': 'Changed',
      'repositories': [writable.toJson()],
    }.entries) {
      expect(
        original,
        isNot(ProjectRecord.parse({...record(), change.key: change.value})),
      );
    }
  });

  test('native v3 companion preserves complete rich metadata and nullable custom type leaves', () async {
    final executor = ProjectsExecutor();
    final api = NativeAppApi(executor: executor);
    addTearDown(api.dispose);
    final repository = ProjectRepository(
      name: null,
      url: 'git@host:team/repo',
      readOnly: false,
      branch: ProjectBranches(
        base: 'main',
        work: 'team/work/*',
        types: ProjectBranchTypes(
          feature: 'team/features/*',
          fix: null,
          release: 'stable/releases/*',
          hotfix: null,
        ),
      ),
    );
    final wire = {
      ...record(),
      'key': 'renamed-project',
      'description': ' exact e\u0301 🛠 ',
      'prompt': ' exact\n\tprompt 🛠 ',
      'repositories': [repository.toJson()],
    };
    executor.result = {
      'revision': 7,
      'projects': [wire],
    };
    final reply = await api.call('projects_list_v3', {});
    final saved = ProjectSnapshot.parse(reply.result).projects.single;
    expect(saved.name, 'Smith');
    expect(saved.key, wire['key']);
    expect(saved.description, wire['description']);
    expect(saved.prompt, wire['prompt']);
    expect(saved.repositories, [repository]);
    executor.result = receipt();
    final args = {
      ...arguments('project_update_v3'),
      'key': saved.key,
      'description': saved.description,
      'prompt': saved.prompt,
      'repositories': [
        {...saved.repositories.single.toJson(), 'name': 'backend'},
      ],
    };
    await api.call('project_update_v3', args);
    expect(executor.requests.last['args'], args);
    expect(executor.requests.map((r) => r['op']), [
      'projects_list_v3',
      'project_update_v3',
    ]);
  });

  test('Work accepts exact branches or one safe terminal wildcard but Base remains exact', () {
    for (final work in ['main', 'team/*', 'team/feature/*', '${'a' * 253}/*']) {
      expect(projectWorkBranchError(work), isNull);
      expect(
        ProjectBranches.parse(ProjectBranches(work: work).toJson()).work,
        work,
      );
      if (work.endsWith('/*')) {
        expect(() => ProjectBranches(base: work), protocolFailure);
      }
    }
    for (final work in [
      '*',
      '/*',
      'a/**',
      'a/*/b',
      'a/b*',
      'refs/a/*',
      'HEAD/*',
      'a.lock/*',
      '${'a' * 254}/*',
    ]) {
      expect(projectWorkBranchError(work), isNotNull);
      expect(() => ProjectBranches(work: work), protocolFailure);
    }
  });

  test(
    'legacy operations fail locally with zero exchange, even with empty arrays',
    () async {
      final executor = ProjectsExecutor();
      final api = NativeAppApi(executor: executor);
      addTearDown(api.dispose);
      for (final op in [
        'projects_list',
        'project_create',
        'project_update',
        'project_delete',
        'projects_list_v2',
        'project_create_v2',
        'project_update_v2',
        'project_delete_v2',
      ]) {
        final args = arguments(
          op.endsWith('_v2') ? op.replaceAll('_v2', '_v3') : '${op}_v3',
        );
        if (args.containsKey('repositories')) args['repositories'] = [];
        expect(
          () => validateProjectArguments(op, args),
          throwsA(isA<AppFailure>().having((e) => e.code, 'code', 'OP')),
        );
        await expectLater(
          api.call(op, args),
          throwsA(isA<AppFailure>().having((e) => e.code, 'code', 'OP')),
        );
      }
      expect(executor.requests, isEmpty);
    },
  );

  for (final code in ['OP', 'UPGRADE_REQUIRED']) {
    for (final op in [
      'projects_list_v3',
      'project_create_v3',
      'project_update_v3',
      'project_delete_v3',
    ]) {
      test('$op $code incompatibility never falls back', () async {
        final executor = ProjectsExecutor()
          ..error = {'code': code, 'message': 'Incompatible pair'};
        final api = NativeAppApi(executor: executor);
        addTearDown(api.dispose);
        await expectLater(
          api.call(op, arguments(op)),
          throwsA(isA<AppFailure>().having((e) => e.code, 'code', code)),
        );
        expect(executor.requests.map((r) => r['op']), [op]);
      });
    }
  }

  test('invalid nested shape never reaches the private exchange', () async {
    final executor = ProjectsExecutor();
    final api = NativeAppApi(executor: executor);
    addTearDown(api.dispose);
    Map<String, dynamic> fresh() =>
        ProjectRepository(url: 'git@host:repo').toJson();
    final invalid = <dynamic>[
      'git@host:repo',
      ['git@host:repo'],
    ];
    for (final path in [
      <String>[],
      ['branch'],
      ['branch', 'types'],
    ]) {
      Map<String, dynamic> at(Map<String, dynamic> root) {
        var node = root;
        for (final key in path) {
          node = node[key] as Map<String, dynamic>;
        }
        return node;
      }

      for (final key in at(fresh()).keys) {
        final wire = fresh();
        at(wire).remove(key);
        invalid.add(wire);
      }
      final wire = fresh();
      at(wire)['extra'] = null;
      invalid.add(wire);
    }
    invalid.addAll([
      {...fresh(), 'branch': null},
      {...fresh(), 'url': null},
      {...fresh(), 'name': ''},
      {
        ...fresh(),
        'branch': {'base': null, 'work': null, 'types': null},
      },
    ]);
    for (final repository in invalid) {
      for (final op in ['project_create_v3', 'project_update_v3']) {
        await expectLater(
          api.call(op, {
            ...arguments(op),
            'repositories': [repository],
          }),
          protocolFailure,
        );
      }
    }
    expect(executor.requests, isEmpty);
  });

  test(
    'queued native requests capture all nested values before caller mutation',
    () async {
      final executor = ProjectsExecutor()..gate = Completer<void>();
      final api = NativeAppApi(executor: executor);
      addTearDown(api.dispose);
      final first = api.call('projects_list_v3', {});
      await Future<void>.delayed(Duration.zero);
      final repository = ProjectRepository(
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
      ).toJson();
      final args = {
        ...arguments('project_create_v3'),
        'repositories': [repository],
      };
      final original = jsonDecode(jsonEncode(args));
      final second = api.call('project_create_v3', args);
      repository['branch']['types'].clear();
      repository['branch'].clear();
      repository.clear();
      args.clear();
      executor.gate!.complete();
      await Future.wait([first, second]);
      expect(executor.requests.last['args'], original);
      expect(executor.requests.map((r) => r['op']), [
        'projects_list_v3',
        'project_create_v3',
      ]);
    },
  );

  test('all 128 nullable leaf combinations round trip without presets', () {
    for (var mask = 0; mask < 128; mask++) {
      String? leaf(int bit, String value) =>
          mask & (1 << bit) == 0 ? null : value;
      final repository = ProjectRepository(
        name: leaf(0, 'backend'),
        url: 'git@host:repo',
        branch: ProjectBranches(
          base: leaf(1, 'main'),
          work: leaf(2, 'main'),
          types: ProjectBranchTypes(
            feature: leaf(3, 'feature/*'),
            fix: leaf(4, 'fix/*'),
            release: leaf(5, 'release/*'),
            hotfix: leaf(6, 'hotfix/*'),
          ),
        ),
      );
      final wire = repository.toJson();
      final parsed = ProjectRepository.parse(wire);
      expect(parsed, repository);
      expect(parsed.hashCode, repository.hashCode);
      expect(parsed.branch, repository.branch);
      expect(parsed.branch.hashCode, repository.branch.hashCode);
      expect(parsed.branch.types, repository.branch.types);
      expect(parsed.branch.types.hashCode, repository.branch.types.hashCode);
      expect(parsed.toJson(), wire);
      final args = {
        ...arguments('project_create_v3'),
        'repositories': [wire],
      };
      if (repository.name == null) {
        expect(
          () => validateProjectArguments('project_create_v3', args),
          protocolFailure,
        );
      } else {
        validateProjectArguments('project_create_v3', args);
      }
      ProjectRecord.parse({
        ...record(),
        'repositories': [wire],
      });
    }
    final empty = ProjectRepository(url: 'git@host:repo');
    expect(empty.toJson(), {
      'name': null,
      'url': 'git@host:repo',
      'read_only': true,
      'branch': {
        'base': null,
        'work': null,
        'types': {
          'feature': null,
          'fix': null,
          'release': null,
          'hotfix': null,
        },
      },
    });
    expect(ProjectRepository(url: empty.url, branch: null), empty);
    expect(ProjectBranches(types: null), ProjectBranches());
  });

  test(
    'nested objects have exact required keys and independently typed leaves',
    () {
      final original = ProjectRepository(
        name: 'Backend',
        url: 'git@host:repo',
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
      ).toJson();
      for (final path in [
        <String>[],
        ['branch'],
        ['branch', 'types'],
      ]) {
        Map<String, dynamic> at(Map<String, dynamic> root) {
          var node = root;
          for (final key in path) {
            node = node[key] as Map<String, dynamic>;
          }
          return node;
        }

        Map<String, dynamic> fresh() =>
            jsonDecode(jsonEncode(original)) as Map<String, dynamic>;
        for (final key in at(original).keys) {
          final missing = fresh();
          at(missing).remove(key);
          expect(() => ProjectRepository.parse(missing), protocolFailure);
          for (final bad in [
            if (key != 'read_only') true,
            7,
            1.0,
            [],
            <String, dynamic>{},
            '',
          ]) {
            final changed = fresh();
            at(changed)[key] = bad;
            expect(
              () => ProjectRepository.parse(changed),
              protocolFailure,
              reason: '$path.$key=$bad',
            );
            expect(
              () => validateProjectArguments('project_update_v3', {
                ...arguments('project_update_v3'),
                'repositories': [changed],
              }),
              protocolFailure,
            );
          }
        }
        final extra = fresh();
        at(extra)['unknown'] = null;
        expect(() => ProjectRepository.parse(extra), protocolFailure);
      }
      for (final bad in [
        null,
        [],
        ['name', 'git@host:repo', null],
        'git@host:repo',
      ]) {
        expect(() => ProjectRepository.parse(bad), protocolFailure);
        expect(() => ProjectBranches.parse(bad), protocolFailure);
        expect(() => ProjectBranchTypes.parse(bad), protocolFailure);
      }
      for (final key in ['url', 'branch', 'read_only']) {
        expect(
          () => ProjectRepository.parse({...original, key: null}),
          protocolFailure,
        );
      }
      expect(
        () =>
            ProjectBranches.parse({'base': null, 'work': null, 'types': null}),
        protocolFailure,
      );
      for (final bad in [
        ['git@host:repo'],
        [original, 'git@host:other'],
        [
          [original],
        ],
      ]) {
        expect(
          () => ProjectRecord.parse({...record(), 'repositories': bad}),
          protocolFailure,
        );
      }
    },
  );

  test('branch and pattern ASCII grammar and exact 255-byte boundaries', () {
    final good = <String?>[
      null,
      'main',
      'dev',
      'a',
      'A_1.-b',
      'a/b',
      'head',
      'Refs/main',
      'refs',
      'a.locked',
      'a' * 255,
    ];
    final bad = [
      '',
      'HEAD',
      'refs/main',
      '/a',
      'a/',
      'a//b',
      '.a',
      '-a',
      '_a',
      'a/.b',
      'a/-b',
      'a..b',
      'a/b..c',
      'a.',
      'a/b.',
      'a.lock',
      'a/b.LoCk',
      'a' * 256,
      'é',
      '🛠',
      'a b',
      'a@b',
      'a@{b',
      'a:b',
      'a~b',
      'a^b',
      'a?b',
      'a*b',
      'a[b',
      'a\\b',
      for (var c = 0; c < 32; c++) 'a${String.fromCharCode(c)}b',
      for (var c = 127; c < 160; c++) 'a${String.fromCharCode(c)}b',
    ];
    for (final name in good) {
      expect(projectBranchNameError(name), isNull, reason: '$name');
      ProjectBranches(base: name, work: name);
    }
    for (final name in bad) {
      expect(projectBranchNameError(name), isNotNull, reason: name);
      expect(() => ProjectBranches(base: name), protocolFailure);
      expect(() => ProjectBranches(work: name), protocolFailure);
    }
    for (final pattern in <String?>[
      null,
      'feature/*',
      'a/b/*',
      '${'a' * 253}/*',
    ]) {
      expect(projectBranchTypeError(pattern), isNull, reason: '$pattern');
      ProjectBranchTypes(
        feature: pattern,
        fix: pattern,
        release: pattern,
        hotfix: pattern,
      );
    }
    for (final pattern in [
      '',
      '*',
      '/*',
      'feature',
      'feature/*/a',
      'feature/**',
      'feature/?',
      '${'a' * 254}/*',
      for (final name in bad) '$name/*',
    ]) {
      expect(projectBranchTypeError(pattern), isNotNull, reason: pattern);
      expect(() => ProjectBranchTypes(feature: pattern), protocolFailure);
      expect(() => ProjectBranchTypes(fix: pattern), protocolFailure);
      expect(() => ProjectBranchTypes(release: pattern), protocolFailure);
      expect(() => ProjectBranchTypes(hotfix: pattern), protocolFailure);
    }
  });

  test(
    'repository IDs are required only at the create and update boundary',
    () async {
      final executor = ProjectsExecutor();
      final api = NativeAppApi(executor: executor);
      addTearDown(api.dispose);
      for (final id in ['a', '0', 'smith-2', 'a' * 40]) {
        expect(projectRepositoryIdError(id), isNull);
        expect(projectKeyError(id), isNull);
        for (final op in ['project_create_v3', 'project_update_v3']) {
          validateProjectArguments(op, {
            ...arguments(op),
            'repositories': [
              ProjectRepository(name: id, url: 'git@host:repo').toJson(),
            ],
          });
        }
      }
      for (final id in <String?>[
        null,
        '',
        'Backend',
        'two words',
        'a_b',
        'a--b',
        'é',
        'a' * 41,
      ]) {
        expect(projectRepositoryIdError(id), contains('ID'));
        for (final op in ['project_create_v3', 'project_update_v3']) {
          await expectLater(
            api.call(op, {
              ...arguments(op),
              'repositories': [
                {
                  ...ProjectRepository(url: 'git@host:repo').toJson(),
                  'name': id,
                },
              ],
            }),
            protocolFailure,
          );
        }
      }
      for (final name in <String?>[null, 'Legacy Repo 🚀']) {
        final repository = ProjectRepository(name: name, url: 'git@host:repo');
        final saved = ProjectRecord.parse({
          ...record(),
          'repositories': [repository.toJson()],
        });
        expect(saved.repositories.single.name, name);
        expect(saved.repositories.single.toJson(), repository.toJson());
      }
      expect(executor.requests, isEmpty);
    },
  );

  test('optional repository names use the raw scalar predicate and exact uniqueness', () {
    for (final name in <String?>[null, 'Backend', '🛠' * 40, 'e\u0301' * 20]) {
      expect(projectRepositoryNameError(name), isNull);
      ProjectRepository(name: name, url: 'git@host:repo');
    }
    for (final name in [
      '',
      ' ',
      ' a',
      'a ',
      '🛠' * 41,
      'e\u0301' * 21,
      String.fromCharCode(0xd800),
      String.fromCharCode(0xdc00),
      for (var c = 0; c < 32; c++) 'a${String.fromCharCode(c)}b',
      for (var c = 127; c < 160; c++) 'a${String.fromCharCode(c)}b',
    ]) {
      expect(projectRepositoryNameError(name), isNotNull);
      expect(
        () => ProjectRepository(name: name, url: 'git@host:repo'),
        protocolFailure,
      );
    }
    List<Map<String, dynamic>> repos(String? a, String? b, String url) => [
      ProjectRepository(name: a, url: 'git@host:repo').toJson(),
      ProjectRepository(name: b, url: url).toJson(),
    ];
    for (final values in [
      repos(null, null, 'git@host:Repo'),
      repos('Backend', 'backend', 'git@host:Repo'),
    ]) {
      ProjectRecord.parse({...record(), 'repositories': values});
    }
    for (final values in [
      repos('same', 'same', 'git@host:other'),
      repos(null, null, 'git@host:repo'),
    ]) {
      expect(
        () => ProjectRecord.parse({...record(), 'repositories': values}),
        protocolFailure,
      );
      expect(
        () => validateProjectArguments('project_create_v3', {
          ...arguments('project_create_v3'),
          'repositories': values,
        }),
        protocolFailure,
      );
    }
  });

  test(
    'nested parsing and serialization never retain caller-owned containers',
    () {
      final wire = ProjectRepository(
        url: 'git@host:repo',
        branch: ProjectBranches(
          base: 'main',
          types: ProjectBranchTypes(feature: 'feature/*'),
        ),
      ).toJson();
      final parsed = ProjectRepository.parse(wire);
      wire['name'] = 'changed';
      wire['branch']['base'] = 'changed';
      wire['branch']['types']['feature'] = 'changed/*';
      final output = parsed.toJson();
      output['branch']['types']['feature'] = null;
      output['branch']['base'] = null;
      expect(parsed.name, isNull);
      expect(parsed.branch.base, 'main');
      expect(parsed.branch.types.feature, 'feature/*');
      expect(parsed.toJson()['branch']['types']['feature'], 'feature/*');
      expect(identical(parsed.toJson(), parsed.toJson()), isFalse);
      expect(parsed, isNot(ProjectRepository(url: parsed.url)));
      expect(parsed.branch, isNot(ProjectBranches()));
      expect(parsed.branch.types, isNot(ProjectBranchTypes()));
    },
  );

  test('immutable records and snapshots copy caller-owned lists', () {
    final repositories = [ProjectRepository(url: 'https://example.org/repo')];
    final p = ProjectRecord(projectId, 'Smith', repositories, 'opencode');
    final projects = [p];
    final s = ProjectSnapshot(0, projects);
    repositories.clear();
    projects.clear();
    expect(p.projectId, projectId);
    expect(p.name, 'Smith');
    expect(p.agent, 'opencode');
    expect(p.repositories, [
      ProjectRepository(url: 'https://example.org/repo'),
    ]);
    expect(s.revision, 0);
    expect(s.projects.single, same(p));
    expect(() => p.repositories.clear(), throwsUnsupportedError);
    expect(() => s.projects.clear(), throwsUnsupportedError);
    expect(ProjectRecord.parse(record()).name, 'Smith');
    expect(
      ProjectSnapshot.parse(snapshot()).projects.single.projectId,
      projectId,
    );
  });

  test(
    'all model fields are required, exactly typed and closed to extensions',
    () {
      for (final original in [record(), snapshot(), receipt()]) {
        dynamic parse(Map<String, dynamic> value) => switch (original) {
          _ when original.containsKey('name') => ProjectRecord.parse(value),
          _ when original.containsKey('projects') => ProjectSnapshot.parse(
            value,
          ),
          _ => ProjectReceipt.parse(value),
        };
        for (final key in original.keys) {
          expect(
            () => parse(Map.of(original)..remove(key)),
            protocolFailure,
            reason: 'missing $key',
          );
          expect(
            () => parse({...original, key: null}),
            protocolFailure,
            reason: 'null $key',
          );
        }
        expect(() => parse({...original, 'extra': null}), protocolFailure);
      }
      for (final value in [
        null,
        [],
        'record',
        0,
        <int, dynamic>{0: true},
      ]) {
        expect(() => ProjectRecord.parse(value), protocolFailure);
        expect(() => ProjectSnapshot.parse(value), protocolFailure);
        expect(() => ProjectReceipt.parse(value), protocolFailure);
      }
      for (final change in <String, dynamic>{
        'project_id': projectId.toUpperCase(),
        'name': 7,
        'repositories': 'https://example.org/repo',
        'agent': 'OpenCode',
      }.entries) {
        expect(
          () => ProjectRecord.parse({...record(), change.key: change.value}),
          protocolFailure,
        );
      }
      for (final bad in [
        '',
        'd' * 31,
        'd' * 33,
        '${'d' * 31}g',
        '$projectId\n',
      ]) {
        expect(
          () => ProjectRecord.parse({...record(), 'project_id': bad}),
          protocolFailure,
        );
        expect(
          () => ProjectReceipt.parse({...receipt(), 'request_id': bad}),
          protocolFailure,
        );
        expect(
          () => ProjectReceipt.parse({...receipt(), 'project_id': bad}),
          protocolFailure,
        );
      }
      for (final bad in [0, 'false', null]) {
        expect(
          () => ProjectReceipt.parse({...receipt(), 'deleted': bad}),
          protocolFailure,
        );
      }
    },
  );

  test('safe integer revisions, project bounds and stable unique IDs', () {
    for (final revision in [0, maxProjectRevision]) {
      ProjectSnapshot.parse({'revision': revision, 'projects': []});
      ProjectReceipt.parse({...receipt(), 'revision': revision});
    }
    for (final revision in [-1, maxProjectRevision + 1, 1.0, '1', true]) {
      expect(
        () => ProjectSnapshot.parse({...snapshot(), 'revision': revision}),
        protocolFailure,
      );
      expect(
        () => ProjectReceipt.parse({...receipt(), 'revision': revision}),
        protocolFailure,
      );
    }
    List<Map<String, dynamic>> records(int count) => List.generate(
      count,
      (i) => {
        ...record(),
        'project_id': i.toRadixString(16).padLeft(32, '0'),
        'key': 'project-$i',
      },
    );
    ProjectSnapshot.parse({'revision': 0, 'projects': records(32)});
    expect(
      () => ProjectSnapshot.parse({'revision': 0, 'projects': records(33)}),
      protocolFailure,
    );
    expect(
      () => ProjectSnapshot.parse({
        'revision': 0,
        'projects': [record(), record()],
      }),
      protocolFailure,
    );
    expect(
      () => ProjectSnapshot.parse({...snapshot(), 'projects': {}}),
      protocolFailure,
    );
  });

  test(
    'names are raw trimmed Unicode scalars, not graphemes or UTF-16 units',
    () {
      for (final name in [
        'Smith',
        'A B',
        '🛠' * 40,
        'é' * 40,
        'e\u0301' * 20,
      ]) {
        expect(projectNameError(name), isNull, reason: name);
        ProjectRecord.parse({...record(), 'name': name});
      }
      for (final name in [
        '',
        ' ',
        ' Smith',
        'Smith ',
        '\u00a0Smith',
        'Smith\u3000',
        '\ufeffSmith',
        'Smith\ufeff',
        '🛠' * 41,
        'e\u0301' * 21,
        String.fromCharCode(0xd800),
        String.fromCharCode(0xdc00),
        for (var c = 0; c < 32; c++) 'A${String.fromCharCode(c)}B',
        for (var c = 127; c < 160; c++) 'A${String.fromCharCode(c)}B',
      ]) {
        expect(projectNameError(name), isNotNull);
        expect(
          () => ProjectRecord.parse({...record(), 'name': name}),
          protocolFailure,
        );
      }
    },
  );

  test('locator grammar accepts only the three inert ASCII forms', () {
    for (final locator in [
      'https://example.org/team/repo.git',
      'https://EXAMPLE.org:443/Team/repo',
      'https://a:65535/a',
      'https://a:00001/a',
      'ssh://git@example.org:22/team/repo.git',
      'ssh://git@a/repo',
      'git@example.org:team/repo.git',
      'git@a:._~-abc/feature-1.2/...',
      'https://a-b.example/repo',
      'https://${'a' * 63}.example/repo',
      'https://${List.filled(4, 'a' * 62).join('.')}/repo',
      'https://host/${'a' * (2048 - 'https://host/'.length)}',
    ]) {
      expect(repositoryLocatorError(locator), isNull, reason: locator);
    }
    for (final locator in [
      '',
      '/tmp/repo',
      './repo',
      '../repo',
      'file:///repo',
      'http://host/repo',
      'HTTPS://host/repo',
      'git://host/repo',
      'ftp://host/repo',
      'https://host',
      'https://host/',
      'git@host:',
      'git@host:/repo',
      'https://host//repo',
      'https://host/repo/',
      'https://host/a//b',
      'https://host/./repo',
      'https://host/a/../repo',
      'git@host:..',
      'https://user@host/repo',
      'https://user:secret@host/repo',
      'ssh://user@host/repo',
      'ssh://git:secret@host/repo',
      'git@host:22/repo?query',
      'https://host/repo?query',
      'https://host/repo#ref',
      'https://host/r%65po',
      'https://host/répo',
      'https://höst/repo',
      'https://host/repo name',
      'https://host/repo\n',
      'https://host/repo\u0000',
      'https://host/repo;command',
      'https://host/repo&command',
      'https://host/repo|command',
      'https://host/repo\\name',
      'https://host/repo:branch',
      'https://host/repo@ref',
      'https://.host/repo',
      'https://host./repo',
      'https://a..b/repo',
      'https://-host/repo',
      'https://host-/repo',
      'https://a_b/repo',
      'https://[::1]/repo',
      'https://host:/repo',
      'https://host:0/repo',
      'https://host:65536/repo',
      'https://host:-1/repo',
      'https://host:+1/repo',
      'https://host:1.5/repo',
      'https://${'a' * 64}/repo',
      'https://${List.filled(4, 'a' * 63).join('.')}/repo',
      'https://host/${'a' * (2049 - 'https://host/'.length)}',
      'https://host/${'é' * 1024}',
    ]) {
      expect(repositoryLocatorError(locator), isNotNull, reason: locator);
    }
  });

  test(
    'repository and agent limits never normalize or deduplicate silently',
    () {
      final repositories = List.generate(
        16,
        (i) => ProjectRepository(url: 'git@host:repo$i').toJson(),
      );
      ProjectRecord.parse({...record(), 'repositories': []});
      ProjectRecord.parse({...record(), 'repositories': repositories});
      ProjectRecord.parse({
        ...record(),
        'repositories': [
          ProjectRepository(url: 'git@host:Repo').toJson(),
          ProjectRepository(url: 'git@host:repo').toJson(),
        ],
      });
      for (final bad in [
        [...repositories, 'git@host:more'],
        ['git@host:repo', 'git@host:repo'],
        [null],
        [7],
        ['https://host/../repo'],
      ]) {
        expect(
          () => ProjectRecord.parse({...record(), 'repositories': bad}),
          protocolFailure,
        );
      }
      for (final agent in ['', 'claude', 'OpenCode', 'opencode ', true, 0]) {
        expect(
          () => ProjectRecord.parse({...record(), 'agent': agent}),
          protocolFailure,
        );
      }
    },
  );

  for (final op in [
    'projects_list_v3',
    'project_create_v3',
    'project_update_v3',
    'project_delete_v3',
    'project_receipt',
  ]) {
    test('$op: app3-only exact arguments and result dispatch', () async {
      final args = arguments(op);
      validateV3Arguments(op, args);
      for (final key in args.keys) {
        expect(
          () => validateV3Arguments(op, Map.of(args)..remove(key)),
          protocolFailure,
        );
        expect(
          () => validateV3Arguments(op, {...args, key: null}),
          protocolFailure,
        );
      }
      for (final key in [
        'extra',
        'provider',
        'model',
        'tools',
        'path',
        'endpoint',
        'expected_instance_id',
      ]) {
        expect(
          () => validateV3Arguments(op, {...args, key: 'forbidden'}),
          protocolFailure,
        );
      }
      final executor = ProjectsExecutor();
      final api = NativeAppApi(executor: executor);
      addTearDown(api.dispose);
      await api.call(op, args);
      expect(executor.requests, hasLength(1));
      expect(executor.requests.single['schema_version'], 3);
      expect(executor.requests.single['op'], op);
      expect(executor.requests.single['args'], args);
      executor.schema = 2;
      await expectLater(api.call(op, args), protocolFailure);
      expect(executor.requests, hasLength(2));
    });
  }

  test(
    'mutation argument field validation happens before private exchange',
    () async {
      final executor = ProjectsExecutor();
      final api = NativeAppApi(executor: executor);
      addTearDown(api.dispose);
      for (final op in [
        'project_create_v3',
        'project_update_v3',
        'project_delete_v3',
      ]) {
        final args = arguments(op);
        for (final change in <String, dynamic>{
          'request_id': 'wrong',
          'instance_id': 'wrong',
          'expected_revision': -1,
          if (op != 'project_create_v3') 'project_id': 'wrong',
          if (op != 'project_delete_v3') ...{
            'name': ' bad ',
            'repositories': ['https://user@host/repo'],
            'agent': 'other',
          },
        }.entries) {
          await expectLater(
            api.call(op, {...args, change.key: change.value}),
            protocolFailure,
          );
        }
      }
      await expectLater(
        api.call('project_run', {}),
        throwsA(isA<AppFailure>()),
      );
      expect(executor.requests, isEmpty);
    },
  );

  test(
    'receipt results correlate request, target, deletion and strict fields',
    () async {
      final executor = ProjectsExecutor();
      final api = NativeAppApi(executor: executor);
      addTearDown(api.dispose);
      for (final op in [
        'project_create_v3',
        'project_update_v3',
        'project_delete_v3',
        'project_receipt',
      ]) {
        final good = receipt(deleted: op == 'project_delete_v3');
        for (final change in [
          {'request_id': 'd' * 32},
          {'revision': '8'},
          {'deleted': null},
          {'extra': true},
          if (op != 'project_create_v3' && op != 'project_receipt')
            {'project_id': 'd' * 32},
          if (op != 'project_receipt') {'deleted': op != 'project_delete_v3'},
        ]) {
          executor.result = {...good, ...change};
          await expectLater(api.call(op, arguments(op)), protocolFailure);
        }
      }
      executor.result = receipt(deleted: true);
      await api.call('project_receipt', arguments('project_receipt'));
    },
  );

  test('native app boundary never dispatches stale editor origins after list rebinding', () async {
    final executor = ProjectsExecutor();
    final api = NativeAppApi(executor: executor);
    final controller = ProjectsController(api);
    addTearDown(() {
      controller.dispose();
      api.dispose();
    });
    await controller.refresh();
    final editorInstance = controller.instanceId!;
    final editorRevision = controller.revision!;
    executor.instance = 'd' * 32;
    await controller.refresh();
    expect(controller.instanceId, executor.instance);
    expect(controller.revision, editorRevision);
    await controller.create(
      'Unsaved',
      [],
      expectedInstanceId: editorInstance,
      expectedRevision: editorRevision,
    );
    await controller.update(
      projectId,
      'Unsaved',
      [],
      expectedInstanceId: editorInstance,
      expectedRevision: editorRevision,
    );
    await controller.delete(
      projectId,
      expectedInstanceId: editorInstance,
      expectedRevision: editorRevision,
    );
    expect(executor.requests.map((r) => r['op']), [
      'projects_list_v3',
      'projects_list_v3',
    ]);
    expect(controller.uncertain, isFalse);
    expect(controller.error, contains('review'));
  });

  test(
    'strict correlated project errors are surfaced without retry or fallback',
    () async {
      final executor = ProjectsExecutor();
      final api = NativeAppApi(executor: executor);
      addTearDown(api.dispose);
      for (final code in [
        'STALE_REVISION',
        'INSTANCE_MISMATCH',
        'REQUEST_UNKNOWN',
        'REQUEST_EXPIRED',
        'CAPACITY',
        'INVALID_REQUEST',
        'STATE_UNAVAILABLE',
      ]) {
        executor.error = {'code': code, 'message': 'Bounded refusal'};
        final before = executor.requests.length;
        await expectLater(
          api.call('project_receipt', arguments('project_receipt')),
          throwsA(isA<AppFailure>().having((e) => e.code, 'code', code)),
        );
        expect(executor.requests.length, before + 1);
      }
      executor.error = {
        'code': 'REQUEST_UNKNOWN',
        'message': 'Unknown',
        'extra': true,
      };
      await expectLater(
        api.call('project_receipt', arguments('project_receipt')),
        protocolFailure,
      );
    },
  );
}
