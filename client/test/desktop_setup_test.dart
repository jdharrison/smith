import 'dart:async';
import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/bridge/desktop_backend.dart';
import 'package:smith_client/bridge/desktop_setup.dart';

String inventory({String status = 'ready'}) => jsonEncode({
  'schema_version': 1,
  'command': 'inspect',
  'status': status,
  'provider': 'ollama',
  'endpoint': setupEndpoint,
  'models': status == 'ready'
      ? [
          {'name': 'fixture:local', 'digest': 'fixture-digest', 'size': 42},
        ]
      : [],
  'error': status == 'ready'
      ? null
      : {'code': 'unavailable', 'message': 'Ollama unavailable'},
});
Map<String, dynamic> envelope(
  String event,
  int seq, [
  Map<String, dynamic> fields = const {},
]) => {
  'schema_version': 1,
  'command': 'pull',
  'operation_id': 'operation-fixture',
  'seq': seq,
  'event': event,
  ...fields,
};
Map<String, dynamic> approvalEvent({String model = 'fixture:local'}) =>
    envelope('approval_required', 0, {
      'model': model,
      'endpoint': setupEndpoint,
    });
Map<String, dynamic> terminalEvent({int seq = 1, String status = 'success'}) =>
    envelope('terminal', seq, {
      'status': status,
      'code': 'fixture',
      'message': 'Fixture result',
    });

class SetupFakeChild implements SetupChild {
  final output = StreamController<List<int>>();
  final errors = StreamController<List<int>>();
  final exit = Completer<int>();
  final approvals = <String>[];
  final effects = <String>[];
  bool exitOnSignal = true;
  void Function()? onApproval;
  @override
  bool alive = true;
  @override
  Future<int> get exitCode => exit.future;
  @override
  Stream<List<int>> get stdout => output.stream;
  @override
  Stream<List<int>> get stderr => errors.stream;
  void emit(Map<String, dynamic> value) =>
      output.add(utf8.encode('${jsonEncode(value)}\n'));
  void finish([int code = 0]) {
    alive = false;
    if (!exit.isCompleted) exit.complete(code);
    unawaited(output.close());
    unawaited(errors.close());
  }

  @override
  Future<void> closeInput() async {
    effects.add('EOF');
  }

  @override
  bool terminate({bool force = false}) {
    effects.add(force ? 'KILL' : 'TERM');
    if (exitOnSignal) finish();
    return true;
  }

  @override
  Future<void> writeApproval(String line) async {
    approvals.add(line);
    onApproval?.call();
  }
}

class SetupFakeRuntime implements SetupRuntime {
  final children = <SetupFakeChild>[];
  final calls = <List<String>>[];
  bool invalid = false;
  Completer<SetupChild>? pendingLaunch;
  Completer<String>? pendingResolve;
  String discovered = '/bundle/smith_backend/smith';
  void Function(SetupFakeChild child, List<String> argv)? script;
  @override
  Future<String> discover() => resolve(discovered);
  @override
  Future<String> resolve(String path) async {
    if (invalid) throw const FormatException('Unsafe fixture');
    return pendingResolve == null ? path : pendingResolve!.future;
  }

  @override
  Future<SetupChild> launch(String path, List<String> argv) async {
    calls.add([path, ...argv]);
    if (pendingLaunch != null) return pendingLaunch!.future;
    final child = SetupFakeChild();
    children.add(child);
    Timer.run(() => script?.call(child, argv));
    return child;
  }
}

class ImmediateCleanupClock extends SystemBackendClock {
  @override
  Future<T> bound<T>(Future<T> future, Duration duration) {
    if (duration == const Duration(seconds: 5) ||
        duration == const Duration(seconds: 2)) {
      future.ignore();
      return Future.error(TimeoutException('fixture cleanup unknown'));
    }
    return super.bound(future, duration);
  }
}

Future<void> pull(
  DesktopSetup setup, {
  Future<bool> Function()? confirm,
  bool Function()? permitted,
}) => setup.pull(
  '/bundle/smith_backend/smith',
  'fixture:local',
  confirm: confirm ?? (() async => true),
  permitted: permitted ?? (() => true),
  onEvent: (_) {},
);

void main() {
  test(
    'seq-zero preflight error is validated, cleaned up and never requests consent',
    () async {
      final runtime = SetupFakeRuntime()
        ..script = (child, _) {
          child.emit(terminalEvent(seq: 0, status: 'error'));
          child.finish(1);
        };
      var confirmations = 0;
      final events = <Map<String, dynamic>>[];
      final setup = DesktopSetup(runtime: runtime);
      await expectLater(
        setup.pull(
          '/fixture',
          'fixture:local',
          confirm: () async {
            confirmations++;
            return true;
          },
          permitted: () => true,
          onEvent: events.add,
        ),
        throwsFormatException,
      );
      expect(events.single['status'], 'error');
      expect(events.single['code'], 'fixture');
      expect(confirmations, 0);
      expect(runtime.children.single.approvals, isEmpty);
      expect(runtime.children.single.effects, contains('EOF'));
      expect(setup.busy, false);
      expect(setup.cleanupUnknown, false);
    },
  );
  test(
    'preflight rejects success, cancelled and invalid envelope/identity/diagnostics without consent',
    () async {
      for (final change in <Map<String, dynamic>>[
        {'status': 'success'},
        {'status': 'cancelled'},
        {'schema_version': 2},
        {'schema_version': 1.0},
        {'command': 'inspect'},
        {'seq': 1},
        {'seq': 0.0},
        {'operation_id': ''},
        {'operation_id': 1},
        {'code': ''},
        {'code': null},
        {'message': ''},
        {'message': 'unsafe\nmessage'},
        {'message': null},
        {'code': List.filled(129, 'x').join()},
        {'message': List.filled(2049, 'x').join()},
      ]) {
        var confirmations = 0;
        final runtime = SetupFakeRuntime()
          ..script = (child, _) {
            child.emit(terminalEvent(seq: 0, status: 'error')..addAll(change));
            child.finish();
          };
        final setup = DesktopSetup(runtime: runtime);
        await expectLater(
          pull(
            setup,
            confirm: () async {
              confirmations++;
              return true;
            },
          ),
          throwsFormatException,
        );
        expect(confirmations, 0, reason: change.toString());
        expect(runtime.children.single.approvals, isEmpty);
        expect(setup.busy, false);
      }
      final protocol = PullProtocol('fixture:local')
        ..accept(jsonEncode(terminalEvent(seq: 0, status: 'error')));
      expect(protocol.approval, throwsFormatException);
      expect(
        () => protocol.accept(jsonEncode(approvalEvent())),
        throwsFormatException,
      );
    },
  );
  test(
    'late spawn retains ownership through timeout and blocks duplicates',
    () async {
      final runtime = SetupFakeRuntime()
        ..pendingLaunch = Completer<SetupChild>();
      final setup = DesktopSetup(
        runtime: runtime,
        inspectTimeout: const Duration(milliseconds: 10),
      );
      await expectLater(
        setup.inspect('/fixture'),
        throwsA(isA<TimeoutException>()),
      );
      expect(setup.cleanupUnknown, true);
      await expectLater(setup.inspect('/fixture'), throwsFormatException);
      expect(runtime.calls.length, 1);
      final child = SetupFakeChild();
      runtime.pendingLaunch!.complete(child);
      await Future<void>.delayed(Duration.zero);
      expect(child.effects, containsAll(['EOF', 'TERM']));
      expect(setup.busy, false);
    },
  );
  test(
    'terminal while consent pending prevents stale approval to a subsequent child',
    () async {
      final consent = Completer<bool>();
      final runtime = SetupFakeRuntime()
        ..script = (child, _) {
          child.emit(approvalEvent());
          child.emit(terminalEvent(status: 'cancelled'));
          child.finish();
        };
      final setup = DesktopSetup(runtime: runtime);
      await expectLater(
        pull(setup, confirm: () => consent.future),
        throwsFormatException,
      );
      runtime.script = (child, _) {
        child.emit(approvalEvent());
        child.onApproval = () {
          child.emit(terminalEvent());
          child.finish();
        };
      };
      final next = pull(setup);
      consent.complete(true);
      await next;
      expect(runtime.children.first.approvals, isEmpty);
      expect(runtime.children.last.approvals.length, 1);
    },
  );
  test(
    'inspect exact validated argv, split UTF8, read-only inventory',
    () async {
      final runtime = SetupFakeRuntime()
        ..script = (child, argv) {
          final bytes = utf8.encode(inventory());
          for (final byte in bytes) {
            child.output.add([byte]);
          }
          child.finish();
        };
      final setup = DesktopSetup(runtime: runtime);
      expect(await setup.discover(), '/bundle/smith_backend/smith');
      final result = await setup.inspect(await setup.discover());
      expect(result.models.single.name, 'fixture:local');
      expect(runtime.calls.single, [
        '/bundle/smith_backend/smith',
        'setup',
        'inspect',
      ]);
      expect(runtime.children.single.approvals, isEmpty);
      expect(setup.busy, false);
    },
  );
  test('invalid executable never starts process', () async {
    final runtime = SetupFakeRuntime()..invalid = true;
    final setup = DesktopSetup(runtime: runtime);
    await expectLater(setup.inspect('/bad'), throwsFormatException);
    expect(runtime.calls, isEmpty);
  });
  test('inspect schema, identity, models and exit validation', () async {
    for (final change in <Map<String, dynamic>>[
      {'schema_version': 2},
      {'endpoint': 'http://elsewhere'},
      {'provider': 'cloud'},
      {
        'error': {'code': 'bad', 'message': 'bad'},
      },
      {
        'models': [
          {'name': 'fixture:cloud', 'digest': 'x', 'size': 1},
        ],
      },
      {
        'models': [
          {'name': 'fixture:local', 'digest': 'x', 'size': -1},
        ],
      },
    ]) {
      final value = jsonDecode(inventory()) as Map<String, dynamic>;
      value.addAll(change);
      expect(
        () => SetupInventory.parse(jsonEncode(value)),
        throwsFormatException,
      );
    }
    final runtime = SetupFakeRuntime()
      ..script = (child, _) {
        child.output.add(utf8.encode(inventory()));
        child.finish(1);
      };
    await expectLater(
      DesktopSetup(runtime: runtime).inspect('/fixture'),
      throwsFormatException,
    );
  });
  test(
    'pull split frames sends exactly one operation-bound approval',
    () async {
      final runtime = SetupFakeRuntime()
        ..script = (child, _) {
          for (final byte in utf8.encode('${jsonEncode(approvalEvent())}\n')) {
            child.output.add([byte]);
          }
          child.onApproval = () {
            child.emit(
              envelope('progress', 1, {
                'status': 'downloading',
                'completed': 1,
                'total': 2,
              }),
            );
            child.emit(terminalEvent(seq: 2));
            child.finish();
          };
        };
      await pull(DesktopSetup(runtime: runtime));
      expect(runtime.calls.single.skip(1), [
        'setup',
        'pull',
        '--model',
        'fixture:local',
      ]);
      expect(jsonDecode(runtime.children.single.approvals.single), {
        'schema_version': 1,
        'operation_id': 'operation-fixture',
        'model': 'fixture:local',
        'approved': true,
      });
    },
  );
  test(
    'decline, mismatched identity, stale permission send zero approvals',
    () async {
      for (var mode = 0; mode < 3; mode++) {
        var permitted = true;
        final runtime = SetupFakeRuntime()
          ..script = (child, _) {
            child.emit(
              approvalEvent(model: mode == 1 ? 'other:local' : 'fixture:local'),
            );
          };
        final setup = DesktopSetup(runtime: runtime);
        await expectLater(
          pull(
            setup,
            confirm: () async {
              if (mode == 2) permitted = false;
              return mode != 0;
            },
            permitted: () => permitted,
          ),
          throwsFormatException,
        );
        expect(runtime.children.single.approvals, isEmpty);
        expect(setup.busy, false);
      }
    },
  );
  test(
    'cancel while confirmation pending never approves stale consent',
    () async {
      final consent = Completer<bool>();
      final awaiting = Completer<void>();
      final runtime = SetupFakeRuntime()
        ..script = (child, _) {
          child.emit(approvalEvent());
        };
      final setup = DesktopSetup(runtime: runtime);
      final operation = pull(
        setup,
        confirm: () {
          awaiting.complete();
          return consent.future;
        },
      );
      final assertion = expectLater(operation, throwsFormatException);
      await awaiting.future;
      await setup.cancel();
      consent.complete(true);
      await assertion;
      expect(runtime.children.single.approvals, isEmpty);
      expect(runtime.children.single.effects, containsAll(['EOF', 'TERM']));
    },
  );
  test('approval expires at bounded deadline with zero approvals', () async {
    final runtime = SetupFakeRuntime()
      ..script = (child, _) {
        child.emit(approvalEvent());
      };
    final setup = DesktopSetup(
      runtime: runtime,
      approvalTimeout: const Duration(milliseconds: 10),
    );
    await expectLater(
      pull(setup, confirm: () => Completer<bool>().future),
      throwsA(isA<TimeoutException>()),
    );
    expect(runtime.children.single.approvals, isEmpty);
  });
  test('idle and overall deadlines stop only captured child', () async {
    for (final idle in [true, false]) {
      final runtime = SetupFakeRuntime()
        ..script = (child, _) {
          child.emit(approvalEvent());
        };
      final setup = DesktopSetup(
        runtime: runtime,
        idleTimeout: Duration(milliseconds: idle ? 10 : 100),
        overallTimeout: Duration(milliseconds: idle ? 100 : 10),
      );
      await expectLater(pull(setup), throwsA(isA<TimeoutException>()));
      expect(runtime.children.single.effects, contains('TERM'));
    }
  });
  test(
    'malformed, oversize, invalid version/sequence/terminal exit fail closed',
    () async {
      for (var mode = 0; mode < 8; mode++) {
        final runtime = SetupFakeRuntime()
          ..script = (child, _) {
            if (mode == 0) {
              child.output.add(utf8.encode('{bad}\n'));
              return;
            }
            if (mode == 1) {
              child.output.add(List.filled(16385, 65));
              return;
            }
            final first = approvalEvent();
            if (mode == 2) first['schema_version'] = 2;
            if (mode == 3) first['seq'] = 4;
            child.emit(first);
            child.onApproval = () {
              if (mode == 4) {
                child.finish();
                return;
              }
              child.emit(terminalEvent(seq: mode == 5 ? 0 : 1));
              if (mode == 6) child.emit(terminalEvent(seq: 2));
              child.finish(mode == 7 ? 1 : 0);
            };
          };
        await expectLater(
          pull(DesktopSetup(runtime: runtime)),
          throwsFormatException,
        );
        if (mode < 4) expect(runtime.children.single.approvals, isEmpty);
      }
    },
  );
  test('stderr and inspect stdout bounds clean up', () async {
    for (final stdout in [true, false]) {
      final runtime = SetupFakeRuntime()
        ..script = (child, _) {
          (stdout ? child.output : child.errors).add(
            List.filled(stdout ? 262145 : 16385, 65),
          );
        };
      await expectLater(
        DesktopSetup(runtime: runtime).inspect('/fixture'),
        throwsFormatException,
      );
      expect(runtime.children.single.effects, contains('TERM'));
    }
  });
  test('unknown TERM/KILL cleanup blocks duplicate process', () async {
    final runtime = SetupFakeRuntime()
      ..script = (child, _) {
        child.exitOnSignal = false;
        child.output.add(utf8.encode('{bad}\n'));
      };
    final setup = DesktopSetup(
      runtime: runtime,
      clock: ImmediateCleanupClock(),
    );
    await expectLater(pull(setup), throwsFormatException);
    expect(setup.cleanupUnknown, true);
    await expectLater(pull(setup), throwsFormatException);
    expect(runtime.calls.length, 1);
    expect(
      runtime.children.single.effects,
      containsAll(['EOF', 'TERM', 'KILL']),
    );
    runtime.children.single.finish();
  });
  test(
    'protocol rejects unapproved progress, double approval and changed operation',
    () {
      final p = PullProtocol('fixture:local')
        ..accept(jsonEncode(approvalEvent()));
      expect(
        () => p.accept(jsonEncode(envelope('progress', 1, {'status': 'busy'}))),
        throwsFormatException,
      );
      p.approval();
      expect(p.approval, throwsFormatException);
      final v = terminalEvent()..['operation_id'] = 'other';
      expect(() => p.accept(jsonEncode(v)), throwsFormatException);
    },
  );
  test('exact name:tag only; no URL/cloud/control or implicit tag', () {
    for (final value in [
      'name',
      'https://host/model:tag',
      'name:cloud',
      'name:tag\n',
      ' name:tag',
      'name:',
    ]) {
      expect(exactLocalTag(value), false, reason: value);
    }
    expect(exactLocalTag('namespace/name:tag'), true);
  });
}
