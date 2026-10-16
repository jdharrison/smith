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
        {'version': 2},
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
      'future': utf8.encode(
        jsonEncode({...preferences().toJson(), 'version': 2}),
      ),
      'unknown member': utf8.encode(
        jsonEncode({...preferences().toJson(), 'draft': 'private'}),
      ),
      'oversize': List.filled(4097, 32),
      'invalid utf8': [0xff],
      'duplicate member': utf8.encode(
        jsonEncode(preferences().toJson())
            .replaceFirst('"version":1', '"version":2,"version":1'),
      ),
      'escaped duplicate': utf8.encode(
        jsonEncode(preferences().toJson())
            .replaceFirst('"version":1', r'"\u0076ersion":2,"version":1'),
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
