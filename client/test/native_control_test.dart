import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/control_api.dart';
import 'package:smith_client/native_control.dart';

import 'widget_test.dart' show profile, preflightError;

class FakeProcess implements Process {
  FakeProcess({this.exitOnTerm = true, this.exitOnKill = true}) {
    input.stream.listen((_) {});
  }
  final bool exitOnTerm;
  final bool exitOnKill;
  final out = StreamController<List<int>>();
  final err = StreamController<List<int>>();
  final exited = Completer<int>();
  final input = StreamController<List<int>>();
  final signals = <ProcessSignal>[];
  @override
  Stream<List<int>> get stdout => out.stream;
  @override
  Stream<List<int>> get stderr => err.stream;
  @override
  IOSink get stdin => IOSink(input.sink);
  @override
  Future<int> get exitCode => exited.future;
  @override
  int get pid => 123;
  @override
  bool kill([ProcessSignal signal = ProcessSignal.sigterm]) {
    signals.add(signal);
    if ((signal == ProcessSignal.sigterm && exitOnTerm) ||
        (signal == ProcessSignal.sigkill && exitOnKill)) {
      if (!exited.isCompleted) {
        exited.complete(signal == ProcessSignal.sigterm ? -15 : -9);
        out.close();
        err.close();
      }
    }
    return true;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
  void finish(String text, {int code = 0}) {
    out.add(utf8.encode(text));
    out.close();
    err.close();
    exited.complete(code);
  }
}

NativeHelperExecutor executor(FakeProcess process) =>
    NativeHelperExecutor.forTesting(
      validate: () async => '/test/smith-service',
      launch: (path, arguments) async => process,
      cleanupGrace: const Duration(milliseconds: 30),
      killConfirmation: const Duration(milliseconds: 30),
    );

Future<void> flush() async {
  for (var i = 0; i < 10; i++) {
    await Future<void>.delayed(Duration.zero);
  }
}

void main() {
  test(
    'captures one JSON observation and does not kill an exited helper',
    () async {
      final child = FakeProcess();
      final helper = executor(child);
      final result = helper.execute(['inspect']);
      await flush();
      child.finish(jsonEncode(profile()));
      expect(ControlReply.parse(await result).ready, isTrue);
      helper.dispose();
      expect(child.signals, isEmpty);
    },
  );
  test('combined stdout and stderr capture over 16 KiB fails and kills captured child', () async {
    final child = FakeProcess();
    final helper = executor(child);
    final result = helper.execute(['inspect']);
    final expectation = expectLater(result, throwsA(isA<ControlFailure>()));
    await flush();
    child.out.add(List.filled(8192, 32));
    child.err.add(List.filled(8193, 32));
    await expectation;
    expect(child.signals, [ProcessSignal.sigterm]);
    expect(child.exited.isCompleted, isTrue);
    helper.dispose();
  });
  test('nonzero exit cannot claim success', () async {
    final child = FakeProcess();
    final helper = executor(child);
    final result = helper.execute(['inspect']);
    final expectation = expectLater(result, throwsA(isA<ControlFailure>()));
    await flush();
    child.finish(jsonEncode(profile()), code: 1);
    await expectation;
    expect(child.signals, isEmpty);
    helper.dispose();
  });
  test('nonzero exit preserves valid structured failure', () async {
    final child = FakeProcess();
    final helper = executor(child);
    final result = helper.execute(['inspect']);
    await flush();
    child.finish(jsonEncode(profile(ok: false)), code: 1);
    expect(ControlReply.parse(await result).error, 'Manager unavailable');
    helper.dispose();
  });
  test('invalid UTF8 and freeform output fail closed', () async {
    final child = FakeProcess();
    final helper = executor(child);
    final result = helper.execute(['inspect']);
    final expectation = expectLater(result, throwsA(isA<ControlFailure>()));
    await flush();
    child.out.add([255]);
    child.finish('running');
    await expectation;
    helper.dispose();
  });
  test('dispose cancels only active helper and blocks reuse', () async {
    final child = FakeProcess();
    final helper = executor(child);
    final result = helper.execute(['inspect']);
    final expectation = expectLater(result, throwsA(isA<ControlFailure>()));
    await flush();
    helper.dispose();
    await expectation;
    expect(child.signals, [ProcessSignal.sigterm]);
    expect(child.exited.isCompleted, isTrue);
    await expectLater(
      helper.execute(['inspect']),
      throwsA(isA<ControlFailure>()),
    );
  });
  test('dispose before late launch cleans that captured helper', () async {
    final child = FakeProcess();
    final launch = Completer<Process>();
    final helper = NativeHelperExecutor.forTesting(
      validate: () async => '/test/smith-service',
      launch: (_, _) => launch.future,
    );
    final result = helper.execute(['inspect']);
    final expectation = expectLater(result, throwsA(isA<ControlFailure>()));
    await flush();
    helper.dispose();
    await expectation;
    launch.complete(child);
    await flush();
    expect(child.signals, [ProcessSignal.sigterm]);
    expect(child.exited.isCompleted, isTrue);
  });
  test('finite test deadline cleans only captured helper', () async {
    final child = FakeProcess();
    final helper = NativeHelperExecutor.forTesting(
      validate: () async => '/test/smith-service',
      launch: (_, _) async => child,
      deadline: const Duration(milliseconds: 200),
    );
    final result = helper.execute(['inspect']);
    final expectation = expectLater(result, throwsA(isA<ControlFailure>()));
    await flush();
    await expectation;
    expect(child.signals, [ProcessSignal.sigterm]);
    expect(child.exited.isCompleted, isTrue);
    helper.dispose();
  });
  test(
    'default budget allows apply beyond the old 15-second deadline',
    () async {
      expect(helperDeadline, const Duration(seconds: 45));
      final child = FakeProcess();
      final helper = executor(child);
      final result = helper.execute([
        'apply',
        '--action',
        'install-unit',
        '--approval',
        'token',
      ]);
      await flush();
      await Future<void>.delayed(const Duration(seconds: 16));
      expect(child.signals, isEmpty);
      child.finish(
        jsonEncode(profile(runtime: 'stopped', bridge: 'unavailable')),
      );
      expect(ControlReply.parse(await result).ok, isTrue);
      helper.dispose();
    },
  );
  test(
    'native nonzero preflight preserves error with empty effect paths',
    () async {
      final child = FakeProcess();
      final helper = executor(child);
      final api = NativeControlApi(executor: helper);
      final result = api.inspect();
      await flush();
      child.finish(jsonEncode(preflightError()), code: 1);
      expect((await result).error, preflightError()['error']);
      api.dispose();
    },
  );
  test(
    'TERM-resistant helper escalates once to KILL and confirms exit',
    () async {
      final child = FakeProcess(exitOnTerm: false);
      final helper = executor(child);
      final result = helper.execute(['inspect']);
      final expectation = expectLater(
        result,
        throwsA(
          isA<ControlFailure>().having(
            (e) => e.outcomeUnknown,
            'unknown',
            true,
          ),
        ),
      );
      await flush();
      helper.dispose();
      helper.dispose();
      await expectation;
      expect(child.signals, [ProcessSignal.sigterm, ProcessSignal.sigkill]);
      expect(child.exited.isCompleted, isTrue);
    },
  );
  test('grace permits cooperative delayed exit without KILL', () async {
    expect(helperCleanupGrace, const Duration(seconds: 11));
    expect(helperKillConfirmation, const Duration(seconds: 1));
    final child = FakeProcess(exitOnTerm: false);
    final helper = NativeHelperExecutor.forTesting(
      validate: () async => '/test/smith-service',
      launch: (_, _) async => child,
      cleanupGrace: const Duration(milliseconds: 200),
      killConfirmation: const Duration(milliseconds: 30),
    );
    final result = helper.execute(['inspect']);
    final expectation = expectLater(result, throwsA(isA<ControlFailure>()));
    await flush();
    helper.dispose();
    await Future<void>.delayed(const Duration(milliseconds: 20));
    child.finish(jsonEncode(profile(ok: false)), code: 1);
    await expectation;
    expect(child.signals, [ProcessSignal.sigterm]);
    expect(child.exited.isCompleted, isTrue);
  });
  test('cleanup unknown retains child and blocks adapter reuse', () async {
    final child = FakeProcess(exitOnTerm: false, exitOnKill: false);
    final helper = NativeHelperExecutor.forTesting(
      validate: () async => '/test/smith-service',
      launch: (_, _) async => child,
      deadline: const Duration(milliseconds: 100),
      cleanupGrace: const Duration(milliseconds: 20),
      killConfirmation: const Duration(milliseconds: 20),
    );
    final result = helper.execute(['inspect']);
    await expectLater(
      result,
      throwsA(
        isA<ControlFailure>().having((e) => e.outcomeUnknown, 'unknown', true),
      ),
    );
    expect(child.signals, [ProcessSignal.sigterm, ProcessSignal.sigkill]);
    expect(child.exited.isCompleted, isFalse);
    await expectLater(
      helper.execute(['inspect']),
      throwsA(isA<ControlFailure>()),
    );
    expect(child.signals.length, 2);
    child.finish(jsonEncode(profile(ok: false)), code: 1);
    await flush();
    helper.dispose();
    expect(child.signals.length, 2);
  });
  test(
    'late TERM-resistant launch is drained and cleaned with TERM then KILL',
    () async {
      final child = FakeProcess(exitOnTerm: false);
      final launch = Completer<Process>();
      final helper = NativeHelperExecutor.forTesting(
        validate: () async => '/test/smith-service',
        launch: (_, _) => launch.future,
        cleanupGrace: const Duration(milliseconds: 20),
        killConfirmation: const Duration(milliseconds: 20),
      );
      final result = helper.execute(['inspect']);
      final expectation = expectLater(result, throwsA(isA<ControlFailure>()));
      await flush();
      helper.dispose();
      await expectation;
      launch.complete(child);
      await Future<void>.delayed(const Duration(milliseconds: 60));
      expect(child.signals, [ProcessSignal.sigterm, ProcessSignal.sigkill]);
      expect(child.exited.isCompleted, isTrue);
    },
  );
  test('nonpositive cleanup budgets cannot remove bounded cleanup', () {
    for (final value in [Duration.zero, const Duration(seconds: -1)]) {
      expect(
        () => NativeHelperExecutor.forTesting(
          validate: () async => '/test/smith-service',
          launch: (_, _) async => FakeProcess(),
          cleanupGrace: value,
        ),
        throwsArgumentError,
      );
      expect(
        () => NativeHelperExecutor.forTesting(
          validate: () async => '/test/smith-service',
          launch: (_, _) async => FakeProcess(),
          killConfirmation: value,
        ),
        throwsArgumentError,
      );
    }
  });
  test('nonpositive test deadlines cannot remove the finite bound', () {
    for (final deadline in [Duration.zero, const Duration(seconds: -1)]) {
      expect(
        () => NativeHelperExecutor.forTesting(
          validate: () async => '/test/smith-service',
          launch: (_, _) async => FakeProcess(),
          deadline: deadline,
        ),
        throwsArgumentError,
      );
    }
  });
  test('validation rejection never launches', () async {
    var launched = false;
    final helper = NativeHelperExecutor.forTesting(
      validate: () async => throw const ControlFailure('Unsafe helper'),
      launch: (_, _) async {
        launched = true;
        return FakeProcess();
      },
    );
    await expectLater(
      helper.execute(['inspect']),
      throwsA(isA<ControlFailure>()),
    );
    expect(launched, isFalse);
    helper.dispose();
  });
}
