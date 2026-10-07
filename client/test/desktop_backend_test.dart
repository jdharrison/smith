import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:smith_client/bridge/desktop_backend.dart';
import 'package:smith_client/bridge/desktop_setup.dart';
import 'desktop_setup_test.dart' as setup_fixture;
import 'package:smith_client/bridge/smith_bridge.dart';
import 'package:smith_client/chat/chat_controller.dart';
import 'package:smith_client/home/desktop_boot_shell.dart';
import 'package:smith_client/home/halo_theme.dart';
import 'package:smith_client/home/offline_chat_screen.dart';

const desktopSession = '0123456789abcdef0123456789abcdef';
const session = desktopSession;
HealthInfo health({
  String? id,
  String? model = 'fixture:local',
  bool? providerAvailable = true,
  bool? modelAvailable = true,
  String? provider = 'ollama',
  String? profile = 'chat-only',
  bool echo = false,
}) => HealthInfo(
  ok: true,
  service: 'smith-bridge',
  echo: echo,
  isolation: 'sealed',
  profile: profile,
  provider: provider,
  model: model,
  providerAvailable: providerAvailable,
  modelAvailable: modelAvailable,
  desktopSession: id,
);
BackendProbe ready({String? id, String model = 'fixture:local'}) =>
    BackendProbe(ProbeKind.ready, health(id: id, model: model));

class FakeChild implements BackendProcess {
  final exit = Completer<int>();
  final effects = <String>[];
  @override
  bool alive = true;
  @override
  Future<int> get exitCode => exit.future;
  void finish() {
    alive = false;
    if (!exit.isCompleted) exit.complete(0);
  }

  @override
  Future<void> closeInput() async {
    effects.add('EOF');
  }

  @override
  bool terminate({bool force = false}) {
    effects.add(force ? 'KILL' : 'TERM');
    return true;
  }
}

class FakeRuntime implements BackendRuntime {
  BackendProbe result = const BackendProbe(ProbeKind.absent);
  BackendProbe? afterLaunch;
  final child = FakeChild();
  int launches = 0;
  int resolves = 0;
  int probes = 0;
  Completer<String>? pendingResolve;
  void Function()? onResolve;
  void Function()? onProbe;
  String? path;
  List<String>? argv;
  Completer<BackendProcess>? pendingLaunch;
  Completer<BackendProbe>? pendingProbe;
  bool rejectFile = false;
  @override
  Future<BackendProbe> probe() async {
    probes++;
    onProbe?.call();
    return pendingProbe == null ? result : pendingProbe!.future;
  }

  @override
  Future<String> resolve(String executable) async {
    resolves++;
    onResolve?.call();
    if (rejectFile) throw const FormatException('unsafe file');
    return pendingResolve == null ? executable : pendingResolve!.future;
  }

  @override
  String session() => desktopSession;
  @override
  Future<BackendProcess> launch(
    String executable,
    List<String> arguments,
  ) async {
    launches++;
    path = executable;
    argv = arguments;
    if (afterLaunch != null) result = afterLaunch!;
    return pendingLaunch == null ? child : pendingLaunch!.future;
  }
}

class FakeClock implements BackendClock {
  DateTime time = DateTime(2026);
  final waits = <Duration>[];
  final cleanup = <bool>[true];
  bool forceTimeout = false;
  int? timeoutAtBound;
  final bounds = <Duration>[];
  Completer<bool>? pendingExit;
  @override
  Future<T> bound<T>(Future<T> future, Duration duration) {
    bounds.add(duration);
    if (forceTimeout || bounds.length == timeoutAtBound) {
      future.ignore();
      time = time.add(duration);
      return Future<T>.error(TimeoutException('fake deadline'));
    }
    return future;
  }

  @override
  DateTime now() => time;
  @override
  Future<void> delay(Duration duration) async {
    time = time.add(duration);
    await Future<void>.delayed(Duration.zero);
  }

  @override
  Future<bool> exited(BackendProcess child, Duration duration) async {
    waits.add(duration);
    final answer = pendingExit == null
        ? (cleanup.isEmpty ? true : cleanup.removeAt(0))
        : await pendingExit!.future;
    if (answer) (child as FakeChild).finish();
    return answer;
  }
}

class FakeFiles implements ExecutableFiles {
  @override
  int effectiveUid = 1000;
  final inspected = <String>[];
  final metadata = <String, ExecutableMetadata>{
    '/': const ExecutableMetadata(0x41ed, 0),
    '/packaged': const ExecutableMetadata(0x41ed, 1000),
    '/packaged/smith': const ExecutableMetadata(0x81ed, 1000),
  };
  List<int> bytes = [
    127,
    69,
    76,
    70,
    2,
    1,
    1,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    3,
    0,
    62,
    0,
  ];
  @override
  Future<ExecutableMetadata> inspect(String path) async {
    inspected.add(path);
    return metadata[path]!;
  }

  @override
  Future<List<int>> header(String path) async => bytes;
}

DesktopBackendController controller(FakeRuntime runtime, FakeClock clock) =>
    DesktopBackendController(runtime: runtime, clock: clock)
      ..configure('/packaged/smith', 'fixture:local');

void main() {
  for (final scenario in <(HealthInfo, String)>[
    (
      health(profile: null, provider: null, model: null),
      'existing legacy Smith bridge',
    ),
    (health(profile: 'orchestration'), 'different profile or echo mode'),
    (health(echo: true), 'different profile or echo mode'),
    (health(provider: 'other'), 'different or unknown provider'),
    (health(provider: null), 'different or unknown provider'),
    (health(model: 'different:local'), 'different or unknown model'),
    (health(model: null), 'different or unknown model'),
    (
      health(providerAvailable: null),
      'unavailable or unknown provider/model readiness',
    ),
    (
      health(modelAvailable: false),
      'unavailable or unknown provider/model readiness',
    ),
  ]) {
    test(
      'occupied listener explains ${scenario.$2} without lifecycle effects',
      () async {
        final runtime = FakeRuntime()
          ..result = BackendProbe(ProbeKind.conflict, scenario.$1);
        final c = controller(runtime, FakeClock());
        await c.check();
        expect(c.state, BootState.conflict);
        expect(c.detail, contains(scenario.$2));
        expect(c.detail, contains('127.0.0.1:4783'));
        expect(c.detail, contains('Check readiness'));
        expect(c.canStart, false);
        await c.start();
        expect(runtime.launches, 0);
        expect(runtime.child.effects, isEmpty);
        expect(c.model, 'fixture:local');
        c.dispose();
      },
    );
  }
  test(
    'setup discovery inspects only actual executable directory and exact bundled child',
    () async {
      final files = FakeFiles();
      files.metadata['/packaged/smith_backend'] = const ExecutableMetadata(
        0x41ed,
        1000,
      );
      files.metadata['/packaged/smith_backend/smith'] =
          const ExecutableMetadata(0x81ed, 1000);
      final runtime = NativeSetupRuntime(
        resolver: NativeExecutableResolver(files: files),
        desktopExecutable: () => '/packaged/halo',
      );
      expect(await runtime.discover(), '/packaged/smith_backend/smith');
      expect(files.inspected, [
        '/',
        '/packaged',
        '/packaged/smith_backend',
        '/packaged/smith_backend/smith',
      ]);
      files.metadata['/packaged/smith_backend/smith'] =
          const ExecutableMetadata(0xa1ed, 1000);
      await expectLater(runtime.discover(), throwsFormatException);
      expect(
        files.inspected.every(
          (path) => path == '/' || path.startsWith('/packaged'),
        ),
        true,
      );
    },
  );
  for (final responseAfterExit in [false, true]) {
    for (final checkBeforeStop in [false, true]) {
      test(
        'stop blocks readiness: check ${checkBeforeStop ? 'before' : 'after'} stop, response ${responseAfterExit ? 'after' : 'before'} exit',
        () async {
          final runtime = FakeRuntime()..afterLaunch = ready(id: session);
          final clock = FakeClock();
          final c = controller(runtime, clock);
          await c.check();
          await c.start();
          expect(c.state, BootState.owned);
          var posts = 0;
          final chat = ChatController(
            chatOnly: true,
            readinessGuard: c.accepts,
            bridge: SmithBridge(
              client: MockClient((request) async {
                if (request.method == 'POST') posts++;
                return http.Response(
                  jsonEncode({
                    'ok': true,
                    'service': 'smith-bridge',
                    'profile': 'chat-only',
                    'echo': false,
                    'provider': 'ollama',
                    'model': 'fixture:local',
                    'provider_available': true,
                    'desktop_session': session,
                  }),
                  200,
                );
              }),
            ),
          );
          await chat.refreshHealth();
          expect(chat.canSend, isTrue);
          final promotions = <BootState>[];
          c.addListener(() {
            if (c.ready) promotions.add(c.state);
          });
          runtime.pendingProbe = Completer<BackendProbe>();
          clock.pendingExit = Completer<bool>();
          final probesBefore = runtime.probes;
          final Future<void> check;
          if (checkBeforeStop) {
            check = c.check();
          } else {
            check = Future.value();
          }
          final stop = c.stop();
          expect(c.state, BootState.stopping);
          expect(c.ready, isFalse);
          expect(c.accepts(health(id: session)), isFalse);
          final duringStop = c.check();
          expect(runtime.probes, probesBefore + (checkBeforeStop ? 1 : 0));
          if (responseAfterExit) {
            clock.pendingExit!.complete(true);
            await stop;
            expect(c.state, BootState.checking);
            expect(c.ownsProcess, isFalse);
            runtime.pendingProbe!.complete(ready(id: session));
          } else {
            runtime.pendingProbe!.complete(ready(id: session));
            await Future.wait([check, duringStop]);
            expect(c.state, BootState.stopping);
            expect(c.ready, isFalse);
            expect(chat.canSend, isFalse);
            await chat.send('Must not be sent during cleanup');
            clock.pendingExit!.complete(true);
            await stop;
          }
          await Future.wait([check, duringStop]);
          expect(c.state, BootState.checking);
          expect(c.observedModel, isNull);
          expect(c.ready, isFalse);
          expect(c.accepts(health(id: session)), isFalse);
          expect(c.accepts(health()), isFalse);
          expect(chat.canSend, isFalse);
          await chat.send('Must not be sent from stale health');
          expect(posts, 0);
          expect(promotions, isEmpty);
          // Only a fresh observation after cleanup may attach to a listener.
          runtime.pendingProbe = null;
          runtime.result = ready();
          await c.check();
          expect(c.state, BootState.external);
          chat.dispose();
          c.dispose();
        },
      );
    }
  }

  test(
    'never-completing resolver exhausts the whole 15s startup budget without launch',
    () async {
      final runtime = FakeRuntime()..pendingResolve = Completer<String>();
      final clock = FakeClock()..forceTimeout = true;
      final c = controller(runtime, clock);
      await c.check();
      final probesBefore = runtime.probes;
      await c.start();
      expect(clock.bounds, [const Duration(seconds: 15)]);
      expect(
        clock.now().difference(DateTime(2026)),
        const Duration(seconds: 15),
      );
      expect(runtime.probes, probesBefore);
      expect(runtime.launches, 0);
      expect(c.state, BootState.unknown);
      expect(c.detail, contains('15-second budget'));
      expect(c.ready, isFalse);
      expect(c.accepts(health()), isFalse);
      expect(c.cleanupConfirmed, isTrue);
      expect(c.configurable, isTrue);
      clock.forceTimeout = false;
      runtime.pendingResolve = null;
      await c.check();
      expect(c.canStart, isTrue);
      c.dispose();
    },
  );

  test(
    'prelaunch probe timeout cannot attach its late compatible response',
    () async {
      final runtime = FakeRuntime();
      final clock = FakeClock()..timeoutAtBound = 2;
      final c = controller(runtime, clock);
      await c.check();
      runtime.pendingProbe = Completer<BackendProbe>();
      runtime.onResolve = () {
        clock.time = clock.time.add(const Duration(seconds: 6));
      };
      await c.start();
      expect(clock.bounds, [
        const Duration(seconds: 15),
        const Duration(seconds: 9),
      ]);
      expect(
        clock.now().difference(DateTime(2026)),
        const Duration(seconds: 15),
      );
      expect(runtime.launches, 0);
      expect(c.state, BootState.unknown);
      runtime.pendingProbe!.complete(ready());
      await Future<void>.delayed(Duration.zero);
      expect(c.ready, isFalse);
      expect(c.accepts(health()), isFalse);
      expect(c.state, BootState.unknown);
      expect(c.cleanupConfirmed, isTrue);
      runtime.pendingProbe = null;
      await c.check();
      expect(c.canStart, isTrue);
      c.dispose();
    },
  );

  test('expired resolver result cannot proceed to probe or launch', () async {
    final runtime = FakeRuntime();
    final clock = FakeClock();
    final c = controller(runtime, clock);
    await c.check();
    final probesBefore = runtime.probes;
    runtime.onResolve = () {
      clock.time = clock.time.add(const Duration(seconds: 15));
    };
    await c.start();
    expect(runtime.probes, probesBefore);
    expect(runtime.launches, 0);
    expect(c.state, BootState.unknown);
    expect(c.ready, isFalse);
    c.dispose();
  });

  test(
    'stop invalidates prelaunch probe before launch or attachment',
    () async {
      final runtime = FakeRuntime();
      final c = controller(runtime, FakeClock());
      await c.check();
      runtime.pendingProbe = Completer<BackendProbe>();
      final start = c.start();
      while (runtime.probes < 2) {
        await Future<void>.delayed(Duration.zero);
      }
      await c.stop();
      runtime.pendingProbe!.complete(ready());
      await start;
      expect(runtime.launches, 0);
      expect(c.ready, isFalse);
      expect(c.accepts(health()), isFalse);
      c.dispose();
    },
  );

  test('preflight and launch share remaining startup budget', () async {
    final runtime = FakeRuntime()..pendingLaunch = Completer<BackendProcess>();
    final clock = FakeClock()..timeoutAtBound = 3;
    final c = controller(runtime, clock);
    await c.check();
    runtime.onResolve = () {
      clock.time = clock.time.add(const Duration(seconds: 4));
    };
    runtime.onProbe = () {
      clock.time = clock.time.add(const Duration(seconds: 3));
    };
    await c.start();
    expect(clock.bounds, [
      const Duration(seconds: 15),
      const Duration(seconds: 11),
      const Duration(seconds: 8),
    ]);
    expect(clock.now().difference(DateTime(2026)), const Duration(seconds: 15));
    expect(c.state, BootState.cleanupUnknown);
    expect(runtime.launches, 1);
    runtime.pendingLaunch!.complete(runtime.child);
    await Future<void>.delayed(Duration.zero);
    expect(c.cleanupConfirmed, isTrue);
    expect(runtime.child.effects, ['EOF', 'TERM']);
    c.dispose();
  });

  test(
    'stop invalidates a resolver result before the prelaunch probe',
    () async {
      final runtime = FakeRuntime()..pendingResolve = Completer<String>();
      final c = controller(runtime, FakeClock());
      await c.check();
      final probesBefore = runtime.probes;
      final start = c.start();
      await c.stop();
      runtime.pendingResolve!.complete('/packaged/smith');
      await start;
      expect(runtime.probes, probesBefore);
      expect(runtime.launches, 0);
      expect(c.ready, isFalse);
      c.dispose();
    },
  );

  test(
    'resolver walks only exact ancestors and accepts a safe native executable',
    () async {
      final files = FakeFiles();
      expect(
        await NativeExecutableResolver(files: files).resolve('/packaged/smith'),
        '/packaged/smith',
      );
      expect(files.inspected, ['/', '/packaged', '/packaged/smith']);
    },
  );

  test(
    'resolver rejects unsafe ownership/modes, directories, symlinks and non-native headers',
    () async {
      for (final bad in [
        const ExecutableMetadata(0x81ed, 2000),
        const ExecutableMetadata(0x81ff, 1000),
        const ExecutableMetadata(0x89ed, 1000),
        const ExecutableMetadata(0x85ed, 1000),
        const ExecutableMetadata(0x81a4, 1000),
        const ExecutableMetadata(0x41ed, 1000),
        const ExecutableMetadata(0xa1ed, 1000),
      ]) {
        final files = FakeFiles()..metadata['/packaged/smith'] = bad;
        await expectLater(
          NativeExecutableResolver(files: files).resolve('/packaged/smith'),
          throwsFormatException,
        );
      }
      for (final bad in [
        const ExecutableMetadata(0xa1ed, 1000),
        const ExecutableMetadata(0x41ff, 1000),
        const ExecutableMetadata(0x41ed, 2000),
      ]) {
        final files = FakeFiles()..metadata['/packaged'] = bad;
        await expectLater(
          NativeExecutableResolver(files: files).resolve('/packaged/smith'),
          throwsFormatException,
        );
      }
      for (final index in [0, 4, 5, 6, 16, 18]) {
        final files = FakeFiles();
        files.bytes[index] = 0;
        await expectLater(
          NativeExecutableResolver(files: files).resolve('/packaged/smith'),
          throwsFormatException,
        );
      }
      for (final bytes in [<int>[], utf8.encode('#!/bin/sh\n')]) {
        final files = FakeFiles()..bytes = bytes;
        await expectLater(
          NativeExecutableResolver(files: files).resolve('/packaged/smith'),
          throwsFormatException,
        );
      }
      final root = FakeFiles()..effectiveUid = 0;
      await expectLater(
        NativeExecutableResolver(files: root).resolve('/packaged/smith'),
        throwsFormatException,
      );
    },
  );

  test(
    'pending spawn stop returns unknown promptly and late child is cleaned',
    () async {
      final runtime = FakeRuntime()
        ..pendingLaunch = Completer<BackendProcess>();
      final c = controller(runtime, FakeClock());
      await c.check();
      final start = c.start();
      while (runtime.launches == 0) {
        await Future<void>.delayed(Duration.zero);
      }
      await c.stop();
      expect(c.state, BootState.cleanupUnknown);
      expect(c.cleanupConfirmed, isFalse);
      await c.start();
      expect(runtime.launches, 1);
      runtime.pendingLaunch!.complete(runtime.child);
      await start;
      expect(c.cleanupConfirmed, isTrue);
      expect(c.ready, isFalse);
      c.dispose();
    },
  );

  test(
    'configuration change invalidates a pending readiness callback',
    () async {
      final runtime = FakeRuntime()..pendingProbe = Completer<BackendProbe>();
      final c = controller(runtime, FakeClock());
      final check = c.check();
      c.configure('/other/smith', 'other:tag');
      runtime.pendingProbe!.complete(ready());
      await check;
      expect(c.model, 'other:tag');
      expect(c.ready, isFalse);
      expect(runtime.launches, 0);
      c.dispose();
    },
  );

  test(
    '15s pending spawn timeout blocks restart and captures late child for cleanup',
    () async {
      final runtime = FakeRuntime()
        ..pendingLaunch = Completer<BackendProcess>();
      final clock = FakeClock()..timeoutAtBound = 3;
      final c = controller(runtime, clock);
      await c.check();
      await c.start();
      expect(
        clock.now().difference(DateTime(2026)),
        const Duration(seconds: 15),
      );
      expect(c.state, BootState.cleanupUnknown);
      expect(c.cleanupConfirmed, isFalse);
      c.configure('/other/smith', 'other:tag');
      expect(c.executable, '/packaged/smith');
      await c.check();
      await c.start();
      expect(runtime.launches, 1);
      runtime.pendingLaunch!.complete(runtime.child);
      await Future<void>.delayed(Duration.zero);
      expect(c.cleanupConfirmed, isTrue);
      expect(c.ready, isFalse);
      expect(runtime.child.effects, ['EOF', 'TERM']);
      c.dispose();
    },
  );

  test(
    'current backend exact-tag availability contract and additive unknowns decode fail closed',
    () {
      final body = <String, dynamic>{
        'ok': true,
        'service': 'smith-bridge',
        'profile': 'chat-only',
        'echo': false,
        'provider': 'ollama',
        'model': 'fixture:local',
        'provider_available': true,
        'desktop_session': session,
      };
      final h = HealthInfo.fromJson(body);
      expect(h.modelAvailable, isTrue);
      expect(chatReady(h, model: 'fixture:local', session: session), isTrue);
      expect(
        chatReady(HealthInfo.fromJson({...body, 'model_available': null})),
        isFalse,
      );
      expect(
        chatReady(HealthInfo.fromJson({...body, 'model_available': false})),
        isFalse,
      );
      expect(
        chatReady(HealthInfo.fromJson({...body, 'provider_available': null})),
        isFalse,
      );
      expect(chatReady(HealthInfo.fromJson({...body, 'echo': null})), isFalse);
    },
  );

  test('positive readiness requires every fact, not unknown or fallback', () {
    for (final h in [
      health(providerAvailable: null),
      health(modelAvailable: null),
      health(providerAvailable: false),
      health(modelAvailable: false),
      health(provider: 'cloud'),
      health(profile: 'legacy'),
      health(echo: true),
      health(model: ''),
    ]) {
      expect(chatReady(h), isFalse);
    }
    expect(chatReady(health()), isTrue);
    expect(chatReady(health(), model: 'other:tag'), isFalse);
    expect(chatReady(health(id: session), session: 'different'), isFalse);
  });

  test('checks never launch; only refused allows explicit Start', () async {
    final runtime = FakeRuntime();
    final clock = FakeClock();
    final c = controller(runtime, clock);
    for (final kind in [
      ProbeKind.unknown,
      ProbeKind.conflict,
      ProbeKind.absent,
    ]) {
      runtime.result = BackendProbe(kind);
      await c.check();
      expect(c.canStart, kind == ProbeKind.absent);
      expect(runtime.launches, 0);
      expect(runtime.resolves, 0);
    }
    c.dispose();
  });

  test(
    'external compatible attachment does not start/reset/stop; model conflict blocks',
    () async {
      final runtime = FakeRuntime()..result = ready();
      final c = controller(runtime, FakeClock());
      await c.check();
      expect(c.state, BootState.external);
      expect(c.detail, contains('externally managed'));
      await c.start();
      await c.stop();
      expect(runtime.launches, 0);
      expect(runtime.child.effects, isEmpty);
      c.configure('/packaged/smith', 'other:tag');
      await c.check();
      expect(c.state, BootState.conflict);
      expect(c.ready, isFalse);
      c.dispose();
    },
  );

  test('exact absolute argv, one launch, session and child liveness', () async {
    final runtime = FakeRuntime()..afterLaunch = ready(id: session);
    final clock = FakeClock();
    final c = controller(runtime, clock);
    await c.check();
    await Future.wait([c.start(), c.start(), c.check()]);
    expect(runtime.launches, 1);
    expect(runtime.path, '/packaged/smith');
    expect(runtime.argv, [
      'serve',
      '--chat-only',
      '--port',
      '4783',
      '--model',
      'fixture:local',
      '--desktop-session',
      session,
    ]);
    expect(c.state, BootState.owned);
    expect(runtime.child.effects, isEmpty);
    expect(c.accepts(health(id: session)), isTrue);
    expect(c.accepts(health(id: 'wrong')), isFalse);
    runtime.child.finish();
    await Future<void>.delayed(Duration.zero);
    expect(c.ready, isFalse);
    expect(c.accepts(health(id: session)), isFalse);
    await c.stop();
    expect(c.ownsProcess, isFalse);
    c.dispose();
  });

  test(
    'port occupied between check and Start is never adopted as owned',
    () async {
      final runtime = FakeRuntime();
      final c = controller(runtime, FakeClock());
      await c.check();
      runtime.result = ready();
      await c.start();
      expect(runtime.launches, 0);
      expect(c.state, BootState.external);
      c.dispose();
    },
  );

  test(
    'resolver failure and invalid tag never launch or leak raw error',
    () async {
      final runtime = FakeRuntime()..rejectFile = true;
      final c = controller(runtime, FakeClock());
      await c.check();
      await c.start();
      expect(runtime.launches, 0);
      expect(c.detail, isNot(contains('unsafe file')));
      c.configure('/packaged/smith', 'bad tag');
      await c.check();
      await c.start();
      expect(runtime.launches, 0);
      c.dispose();
    },
  );

  test(
    'owned session mismatch cleans captured child, never external listener',
    () async {
      final runtime = FakeRuntime()..afterLaunch = ready(id: 'wrong');
      final c = controller(runtime, FakeClock());
      await c.check();
      await c.start();
      expect(c.ready, isFalse);
      expect(c.ownsProcess, isFalse);
      expect(runtime.child.effects, ['EOF', 'TERM']);
      c.dispose();
    },
  );

  test(
    '15s startup timeout stops; 5s TERM then 2s KILL unknown blocks restart',
    () async {
      final runtime = FakeRuntime();
      final clock = FakeClock()
        ..cleanup.clear()
        ..cleanup.addAll([false, false]);
      final c = controller(runtime, clock);
      await c.check();
      await c.start();
      expect(
        clock.now().difference(DateTime(2026)),
        const Duration(seconds: 15),
      );
      expect(clock.waits, [
        const Duration(seconds: 5),
        const Duration(seconds: 2),
      ]);
      expect(runtime.child.effects, ['EOF', 'TERM', 'KILL']);
      expect(c.state, BootState.cleanupUnknown);
      expect(c.canStart, isFalse);
      await c.check();
      await c.start();
      expect(runtime.launches, 1);
      await c.stop();
      c.dispose();
    },
  );

  test(
    'stop concurrent with pending launch cleans late captured handle',
    () async {
      final runtime = FakeRuntime()
        ..pendingLaunch = Completer<BackendProcess>();
      final c = controller(runtime, FakeClock());
      await c.check();
      final start = c.start();
      while (runtime.launches == 0) {
        await Future<void>.delayed(Duration.zero);
      }
      final stop = c.stop();
      runtime.pendingLaunch!.complete(runtime.child);
      await Future.wait([start, stop]);
      expect(c.ready, isFalse);
      expect(c.ownsProcess, isFalse);
      expect(runtime.child.effects, ['EOF', 'TERM']);
      c.dispose();
    },
  );

  test('stale probe after stop cannot restore ready state', () async {
    final runtime = FakeRuntime()..pendingProbe = Completer<BackendProbe>();
    final c = controller(runtime, FakeClock());
    final check = c.check();
    final stop = c.stop();
    runtime.pendingProbe!.complete(ready());
    await Future.wait([check, stop]);
    expect(c.ready, isFalse);
    expect(runtime.launches, 0);
    c.dispose();
  });

  test(
    'chatOnly readiness blocks unknown provider/model without changing legacy',
    () async {
      for (final fields in [
        <String, dynamic>{},
        {'provider_available': true},
        {
          'provider_available': true,
          'model_available': true,
          'provider': 'other',
          'model': 'fixture:local',
        },
        {
          'provider_available': true,
          'model_available': true,
          'provider': 'ollama',
          'model': 'fixture:local',
        },
      ]) {
        final bridge = SmithBridge(
          client: MockClient(
            (request) async => http.Response(
              jsonEncode({
                'ok': true,
                'service': 'smith-bridge',
                'profile': 'chat-only',
                'echo': false,
                ...fields,
              }),
              200,
            ),
          ),
        );
        final chat = ChatController(bridge: bridge, chatOnly: true);
        await chat.refreshHealth();
        expect(chat.canSend, fields['provider'] == 'ollama');
        chat.dispose();
      }
      final legacy = ChatController();
      expect(legacy.canSend, isTrue);
      legacy.dispose();
    },
  );

  test(
    'native resolver rejects relative paths and symlink ancestors without execution',
    () async {
      final resolver = NativeExecutableResolver();
      for (final path in [
        'smith',
        '/usr/../bin/smith',
        '/bin/sh',
        '/tmp/smith',
      ]) {
        await expectLater(resolver.resolve(path), throwsA(anything));
      }
    },
  );

  testWidgets(
    'Home remains mounted across pulse/recovery; draft and transcript preserved; setup accessible',
    (tester) async {
      tester.view.physicalSize = const Size(1100, 1000);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final runtime = FakeRuntime()..result = ready();
      final backend = controller(runtime, FakeClock());
      var posts = 0;
      final bridge = SmithBridge(
        client: MockClient((request) async {
          if (request.method == 'POST') {
            posts++;
            return http.Response(
              jsonEncode({
                'v': 1,
                'profile': 'chat-only',
                'mode': 'chat-only',
                'text': 'retained answer',
                'session': 'default',
                'attachments': [],
                'pipeline': null,
                'task': null,
              }),
              200,
            );
          }
          return http.Response(
            jsonEncode({
              'ok': true,
              'service': 'smith-bridge',
              'profile': 'chat-only',
              'echo': false,
              'provider': 'ollama',
              'model': 'fixture:local',
              'provider_available': true,
              'model_available': true,
            }),
            200,
          );
        }),
      );
      final chat = ChatController(
        bridge: bridge,
        chatOnly: true,
        readinessGuard: backend.accepts,
      );
      await tester.pumpWidget(
        MaterialApp(
          theme: buildHaloTheme(),
          home: DesktopBootShell(
            backend: backend,
            chat: chat,
            setup: DesktopSetup(
              runtime: setup_fixture.SetupFakeRuntime()
                ..script = (child, _) {
                  child.output.add(utf8.encode(setup_fixture.inventory()));
                  child.finish();
                },
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      final screenState = tester.state(find.byType(OfflineChatScreen));
      expect(posts, 0);
      expect(runtime.launches, 0);
      await chat.send('retained question');
      await tester.pumpAndSettle();
      await tester.enterText(
        find.byKey(const Key('text-chat-draft')),
        'keep my draft',
      );
      runtime.result = const BackendProbe(ProbeKind.unknown);
      await backend.check();
      await tester.pumpAndSettle();
      expect(
        identical(screenState, tester.state(find.byType(OfflineChatScreen))),
        isTrue,
      );
      expect(find.text('keep my draft'), findsOneWidget);
      expect(chat.messages.any((m) => m.text == 'retained question'), isTrue);
      expect(chat.canSend, isFalse);
      expect(find.text('Backend setup'), findsOneWidget);
      final semantics = tester.ensureSemantics();
      await tester.tap(find.text('Backend setup'));
      await tester.pumpAndSettle();
      expect(find.text('Select a model'), findsOneWidget);
      expect(find.textContaining('Local runtime'), findsWidgets);
      await tester.tap(find.text('Close setup'));
      await tester.pumpAndSettle();
      semantics.dispose();
      runtime.result = ready();
      await backend.check();
      await tester.pumpAndSettle();
      expect(
        identical(screenState, tester.state(find.byType(OfflineChatScreen))),
        isTrue,
      );
      expect(find.text('keep my draft'), findsOneWidget);
      expect(posts, 1);
      await tester.pumpWidget(const SizedBox());
      chat.dispose();
      backend.dispose();
    },
  );
}
