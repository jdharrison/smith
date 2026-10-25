import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/workspace_preferences.dart';
import 'package:smith_client/workspace_preferences_io.dart';

WorkspacePreferences preferences({String layout = '2x2', int active = 3}) =>
    WorkspacePreferences(
      layout: layout,
      activeTile: active,
      slots: List.generate(
        4,
        (i) => WorkspaceSlotPreferences(
          conversationId: i.isEven ? '0123456789abcdef0123456789abcdef' : null,
          page: i.isEven ? 'chat' : 'settings',
        ),
      ),
    );

Future<void> permissions(String path, String mode) async {
  final result = await Process.run('chmod', [mode, path]);
  expect(result.exitCode, 0);
}

void main() {
  group('presentation schema', () {
    test('round trips immutable values and hidden slots', () {
      final value = preferences();
      expect(
        WorkspacePreferences.fromJson(jsonDecode(jsonEncode(value.toJson()))),
        value,
      );
      expect(() => value.slots.clear(), throwsUnsupportedError);
      final slots = List.of(value.slots);
      final copied = WorkspacePreferences(
        layout: '1x1',
        activeTile: 0,
        slots: slots,
      );
      slots.clear();
      expect(copied.slots.length, 4);
      expect(copied.slots[3].page, 'settings');
    });

    test('rejects unknown fields, versions, types and bounds', () {
      for (final change in <Map<String, Object?>>[
        {'version': 5},
        {'version': 0},
        {'version': '4'},
        {'version': 4.0},
        {'version': true},
        {'version': 2.0},
        {'version': 1.0},
        {'layout': '3x3'},
        {'activeTile': -1},
        {'activeTile': 4},
        {'activeTile': 0.0},
        {'slots': []},
        {'draft': 'never saved'},
        {'version': null},
      ]) {
        expect(
          () => WorkspacePreferences.fromJson({
            ...preferences().toJson(),
            ...change,
          }),
          throwsFormatException,
        );
      }
      expect(
        () => preferences(layout: '1x1', active: 1),
        throwsFormatException,
      );
      expect(() => WorkspacePreferences.fromJson({}), throwsFormatException);
      expect(
        () => WorkspaceSlotPreferences.fromJson({'page': 'chat'}),
        throwsFormatException,
      );
      expect(
        () => WorkspaceSlotPreferences.fromJson({
          'page': 'chat',
          'conversationId': null,
          'draft': '',
        }),
        throwsFormatException,
      );
      expect(
        () => WorkspaceSlotPreferences(conversationId: 'bad', page: 'chat'),
        throwsFormatException,
      );
      expect(
        () => WorkspaceSlotPreferences(conversationId: null, page: 'terminal'),
        throwsFormatException,
      );
    });
  });

  test('v1 migrates strictly; v2 adds Projects presentation slots', () {
    final old = {...preferences().toJson(), 'version': 1};
    final migrated = WorkspacePreferences.fromJson(old);
    expect(migrated, preferences());
    expect(migrated.toJson()['version'], 4);
    final slots = List.of(migrated.slots);
    slots[2] = WorkspaceSlotPreferences(conversationId: null, page: 'projects');
    final current = WorkspacePreferences(
      layout: '2x2',
      activeTile: 2,
      slots: slots,
    );
    expect(WorkspacePreferences.fromJson(current.toJson()), current);
    expect(
      () => WorkspacePreferences.fromJson({...current.toJson(), 'version': 1}),
      throwsFormatException,
    );
    for (final key in [
      'projectId',
      'project',
      'name',
      'repositories',
      'editor',
      'selection',
    ]) {
      final json = current.toJson();
      final entries = (json['slots'] as List).cast<Map<String, Object?>>();
      entries[2][key] = 'not presentation';
      expect(() => WorkspacePreferences.fromJson(json), throwsFormatException);
    }
    expect(jsonEncode(current.toJson()), isNot(contains('projectId')));
  });

  for (final version in [1, 2, 3, 4]) {
    for (final page in ['chat', 'settings', 'projects', 'agents', 'dev']) {
      test('v$version strictly validates $page in all four slots', () {
        final value = WorkspacePreferences(
          layout: '1x1',
          activeTile: 0,
          slots: List.generate(
            4,
            (_) => WorkspaceSlotPreferences(conversationId: null, page: page),
          ),
        );
        final json = {...value.toJson(), 'version': version};
        final introduced = const {
          'chat': 1,
          'settings': 1,
          'projects': 2,
          'agents': 3,
          'dev': 4,
        };
        final allowed = version >= introduced[page]!;
        final cases = [json];
        for (var index = 0; index < 4; index++) {
          final single = {
            ...preferences(layout: '1x1', active: 0).toJson(),
            'version': version,
          };
          (single['slots'] as List)[index] = value.slots[index].toJson();
          cases.add(single);
        }
        for (final sample in cases) {
          if (allowed) {
            expect(WorkspacePreferences.fromJson(sample).toJson(), {
              ...sample,
              'version': 4,
            });
          } else {
            expect(
              () => WorkspacePreferences.fromJson(sample),
              throwsFormatException,
            );
          }
        }
      });
    }
  }

  test('exact shape excludes disclosure, records, editor and prompt state', () {
    final value = preferences().toJson();
    expect(
      value.keys,
      unorderedEquals(['version', 'layout', 'activeTile', 'slots']),
    );
    expect(value['version'], 4);
    for (final slot in value['slots'] as List) {
      expect((slot as Map).keys, unorderedEquals(['conversationId', 'page']));
    }
    for (final key in value.keys) {
      final missing = Map.of(value)..remove(key);
      expect(
        () => WorkspacePreferences.fromJson(missing),
        throwsFormatException,
      );
    }
    for (var index = 0; index < 4; index++) {
      for (final key in ['conversationId', 'page']) {
        final missing = jsonDecode(jsonEncode(value)) as Map<String, dynamic>;
        (missing['slots'] as List)[index].remove(key);
        expect(
          () => WorkspacePreferences.fromJson(missing),
          throwsFormatException,
        );
      }
    }
    for (final field in [
      'devExpanded',
      'disclosure',
      'childSelection',
      'editor',
      'projectId',
      'agentId',
      'prompt',
      'description',
      'draft',
      'selection',
    ]) {
      for (final version in [1, 2, 3, 4]) {
        final json = {...preferences().toJson(), 'version': version};
        expect(
          () => WorkspacePreferences.fromJson({
            ...json,
            field: 'never persisted',
          }),
          throwsFormatException,
        );
        for (var index = 0; index < 4; index++) {
          final nested = jsonDecode(jsonEncode(json)) as Map<String, dynamic>;
          (nested['slots'] as List)[index][field] = 'never persisted';
          expect(
            () => WorkspacePreferences.fromJson(nested),
            throwsFormatException,
          );
        }
      }
    }
  });

  group('isolated Linux store', () {
    late Directory fixture;
    late String path;
    late LinuxWorkspacePreferencesStore store;
    final blocked = isA<WorkspacePreferencesException>();

    setUp(() async {
      fixture = await Directory.systemTemp.createTemp(
        'smith-preferences-test-',
      );
      await permissions(fixture.path, '700');
      path = '${fixture.path}/smith-next/client/workspace.json';
      store = LinuxWorkspacePreferencesStore(root: fixture.path);
    });
    tearDown(() async {
      await store.close();
      await fixture.delete(recursive: true);
    });

    Future<void> seed(List<int> bytes) async {
      await Directory('${fixture.path}/smith-next/client')
          .create(recursive: true);
      await permissions('${fixture.path}/smith-next', '700');
      await permissions('${fixture.path}/smith-next/client', '700');
      await File(path).writeAsBytes(bytes);
      await permissions(path, '600');
    }

    test('lazy construction, missing load, atomic save and restart', () async {
      expect(await Directory('${fixture.path}/smith-next').exists(), false);
      expect(await store.load(), null);
      await store.save(preferences());
      expect((await File(path).stat()).mode & 0xfff, 0x180);
      expect(
        (await Directory('${fixture.path}/smith-next/client').stat()).mode &
            0xfff,
        0x1c0,
      );
      final firstInode = await Process.run('stat', ['-c', '%i', path]);
      final latest = preferences(layout: '1x1', active: 0);
      await store.save(latest);
      final secondInode = await Process.run('stat', ['-c', '%i', path]);
      expect(secondInode.stdout, isNot(firstInode.stdout));
      expect(
        await Directory('${fixture.path}/smith-next/client')
            .list()
            .where((e) => e.path.endsWith('.tmp'))
            .length,
        0,
      );
      await store.close();
      store = LinuxWorkspacePreferencesStore(root: fixture.path);
      expect(await store.load(), latest);
    });

    for (final version in [1, 2, 3]) {
      test(
        'v$version bytes remain untouched until an explicit v4 save',
        () async {
          final bytes = utf8.encode(
            jsonEncode({...preferences().toJson(), 'version': version}),
          );
          await seed(bytes);
          final value = await store.load();
          expect(value, preferences());
          expect(await File(path).readAsBytes(), bytes);
          await store.save(value!);
          expect(jsonDecode(await File(path).readAsString())['version'], 4);
        },
      );
    }

    test(
      'mixed v3 legacy bytes and IDs survive load and explicit v4 save',
      () async {
        final legacy = {
          'version': 3,
          'layout': '1x1',
          'activeTile': 0,
          'slots': [
            {
              'conversationId': '0123456789abcdef0123456789abcdef',
              'page': 'chat',
            },
            {'conversationId': null, 'page': 'projects'},
            {
              'conversationId': 'fedcba9876543210fedcba9876543210',
              'page': 'agents',
            },
            {'conversationId': null, 'page': 'settings'},
          ],
        };
        final bytes = utf8.encode('  ${jsonEncode(legacy)}\n');
        await seed(bytes);
        final loaded = await store.load();
        expect(loaded, WorkspacePreferences.fromJson(legacy));
        expect(await File(path).readAsBytes(), bytes);
        await store.close();
        store = LinuxWorkspacePreferencesStore(root: fixture.path);
        expect(await store.load(), loaded);
        expect(await File(path).readAsBytes(), bytes);
        final slots = List.of(loaded!.slots);
        slots[0] = WorkspaceSlotPreferences(
          conversationId: slots[0].conversationId,
          page: 'dev',
        );
        final changed = WorkspacePreferences(
          layout: loaded.layout,
          activeTile: loaded.activeTile,
          slots: slots,
        );
        await store.save(changed);
        final saved =
            jsonDecode(await File(path).readAsString()) as Map<String, dynamic>;
        expect(saved['version'], 4);
        expect(WorkspacePreferences.fromJson(saved), changed);
        expect(saved['slots'], [
          {'conversationId': '0123456789abcdef0123456789abcdef', 'page': 'dev'},
          ...((legacy['slots'] as List).skip(1)),
        ]);
      },
    );

    test(
      'same-process competing open descriptions cannot take lifetime lock',
      () async {
        await store.load();
        final competitor = LinuxWorkspacePreferencesStore(root: fixture.path);
        addTearDown(competitor.close);
        await expectLater(competitor.load(), throwsA(blocked));
        await store.close();
        await expectLater(competitor.save(preferences()), throwsA(blocked));
        final replacement = LinuxWorkspacePreferencesStore(root: fixture.path);
        addTearDown(replacement.close);
        expect(await replacement.load(), null);
      },
    );

    test('another process cannot acquire the session lock', () async {
      await store.load();
      final lock = '${fixture.path}/smith-next/client/.workspace.lock';
      expect((await Process.run('flock', ['-n', lock, 'true'])).exitCode, 1);
      await store.close();
      expect((await Process.run('flock', ['-n', lock, 'true'])).exitCode, 0);
    });

    test('removed session lock fences publication', () async {
      await store.load();
      await File('${fixture.path}/smith-next/client/.workspace.lock').delete();
      await expectLater(store.save(preferences()), throwsA(blocked));
      expect(await File(path).exists(), false);
    });

    test('exact byte bound accepts a padded valid document', () async {
      final encoded = utf8.encode(jsonEncode(preferences().toJson()));
      await seed([...encoded, ...List.filled(4096 - encoded.length, 32)]);
      expect(await store.load(), preferences());
    });

    test('coalesces pending saves to newest snapshot', () async {
      await store.load();
      final latest = preferences(layout: '1x2', active: 1);
      await Future.wait([store.save(preferences()), store.save(latest)]);
      expect(
        WorkspacePreferences.fromJson(
          jsonDecode(await File(path).readAsString()),
        ),
        latest,
      );
    });

    test('save before load fails; close fences queued publication', () async {
      await expectLater(store.save(preferences()), throwsA(blocked));
      await store.load();
      final saving = store.save(preferences());
      final failed = expectLater(saving, throwsA(blocked));
      await store.close();
      await failed;
      expect(await File(path).exists(), false);
      await expectLater(store.load(), throwsA(blocked));
      await expectLater(store.save(preferences()), throwsA(blocked));
      await store.close();
    });

    test('close before load has no IO', () async {
      await store.close();
      await expectLater(store.load(), throwsA(blocked));
      expect(await Directory('${fixture.path}/smith-next').exists(), false);
    });

    for (final sample in <String, List<int>>{
      'corrupt': utf8.encode('{'),
      for (final version in [1, 2, 3])
        'v$version Dev destination': utf8.encode(
          jsonEncode({
            ...preferences().toJson(),
            'version': version,
            'slots': [
              for (var index = 0; index < 4; index++)
                WorkspaceSlotPreferences(
                  conversationId: null,
                  page: index == 3 ? 'dev' : 'chat',
                ).toJson(),
            ],
          }),
        ),
      'future': utf8.encode(
        jsonEncode({...preferences().toJson(), 'version': 5}),
      ),
      'unknown member': utf8.encode(
        jsonEncode({...preferences().toJson(), 'draft': 'private'}),
      ),
      'oversize': List.filled(4097, 32),
      'invalid utf8': [0xff],
      'legacy duplicate member': utf8.encode(
        jsonEncode({...preferences().toJson(), 'version': 3})
            .replaceFirst('"version":3', '"version":1,"version":3'),
      ),
      'legacy escaped duplicate': utf8.encode(
        jsonEncode({...preferences().toJson(), 'version': 3})
            .replaceFirst('"version":3', r'"\u0076ersion":1,"version":3'),
      ),
      'duplicate member': utf8.encode(
        jsonEncode(preferences().toJson())
            .replaceFirst('"version":4', '"version":1,"version":4'),
      ),
      'escaped duplicate': utf8.encode(
        jsonEncode(preferences().toJson())
            .replaceFirst('"version":4', r'"\u0076ersion":1,"version":4'),
      ),
    }.entries) {
      test(
        '${sample.key} is preserved and fences all subsequent writes',
        () async {
          await seed(sample.value);
          await expectLater(store.load(), throwsA(blocked));
          await expectLater(store.save(preferences()), throwsA(blocked));
          expect(await File(path).readAsBytes(), sample.value);
        },
      );
    }

    test('external replacement is preserved and disables saving', () async {
      await store.load();
      await store.save(preferences());
      await File(path).writeAsString('external change');
      await expectLater(store.save(preferences()), throwsA(blocked));
      expect(await File(path).readAsString(), 'external change');
    });

    test('symlink file is not read or overwritten', () async {
      await store.load();
      await store.close();
      final outside = File('${fixture.path}/sentinel');
      await outside.writeAsString('untouched');
      await Link(path).create(outside.path);
      store = LinuxWorkspacePreferencesStore(root: fixture.path);
      await expectLater(store.load(), throwsA(blocked));
      await expectLater(store.save(preferences()), throwsA(blocked));
      expect(await outside.readAsString(), 'untouched');
      expect(await Link(path).exists(), true);
    });

    test(
      'symlink directory is rejected without creating child state',
      () async {
        final outside = await Directory('${fixture.path}/outside').create();
        await Link('${fixture.path}/smith-next').create(outside.path);
        await expectLater(store.load(), throwsA(blocked));
        expect(await outside.list().length, 0);
      },
    );

    test('symlink root and relative root are rejected', () async {
      final alias = '${fixture.path}/alias';
      await Link(alias).create(fixture.path);
      for (final root in [alias, 'relative', '${fixture.path}/../bad']) {
        final unsafe = LinuxWorkspacePreferencesStore(root: root);
        await expectLater(unsafe.load(), throwsA(blocked));
        await unsafe.close();
      }
    });

    test('public file permissions are not silently repaired', () async {
      await seed(utf8.encode(jsonEncode(preferences().toJson())));
      await permissions(path, '644');
      await expectLater(store.load(), throwsA(blocked));
      expect((await File(path).stat()).mode & 0xfff, 0x1a4);
    });

    test('unsafe app directory is not silently repaired', () async {
      await seed(utf8.encode(jsonEncode(preferences().toJson())));
      await permissions('${fixture.path}/smith-next/client', '777');
      await expectLater(store.load(), throwsA(blocked));
    });

    test('FIFO rejected without blocking', () async {
      await store.load();
      await store.close();
      expect((await Process.run('mkfifo', ['-m', '600', path])).exitCode, 0);
      store = LinuxWorkspacePreferencesStore(root: fixture.path);
      await expectLater(store.load(), throwsA(blocked));
    }, timeout: const Timeout(Duration(seconds: 5)));

    test('hard-linked preference file is rejected', () async {
      await seed(utf8.encode(jsonEncode(preferences().toJson())));
      expect(
        (await Process.run('ln', [path, '${fixture.path}/hardlink'])).exitCode,
        0,
      );
      await expectLater(store.load(), throwsA(blocked));
    });

    test('exceptions do not expose root or persisted values', () async {
      await seed(utf8.encode('secret fixture text'));
      try {
        await store.load();
        fail('Expected rejection');
      } on WorkspacePreferencesException catch (e) {
        expect(e.toString(), isNot(contains(fixture.path)));
        expect(e.toString(), isNot(contains('secret')));
      }
    });
  });
}
