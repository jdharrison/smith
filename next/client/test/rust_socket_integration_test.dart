import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';

import 'widget_test.dart' show profile, reply;

const _ioDeadline = Duration(seconds: 2);
const _childDeadline = Duration(seconds: 15);
const _frameLimit = 4096;
const _outputLimit = 16 * 1024;

// Every process here is captured directly; no PID discovery or manager calls.
class _CapturedChild {
  _CapturedChild(this.process) {
    void capture(List<int> bytes, bool stdout) {
      _bytes += bytes.length;
      if (_bytes > _outputLimit) {
        failure = 'Captured child output exceeded 16 KiB.';
        if (!exited) process.kill(ProcessSignal.sigkill);
      } else if (stdout) {
        output.addAll(bytes);
      }
    }

    void pipeError(Object error, Completer<void> done) {
      failure = 'Cannot capture child output.';
      if (!exited) process.kill(ProcessSignal.sigkill);
      if (!done.isCompleted) done.complete();
    }

    subscriptions.add(
      process.stdout.listen(
        (b) => capture(b, true),
        onDone: _stdoutDone.complete,
        onError: (Object e) => pipeError(e, _stdoutDone),
      ),
    );
    subscriptions.add(
      process.stderr.listen(
        (b) => capture(b, false),
        onDone: _stderrDone.complete,
        onError: (Object e) => pipeError(e, _stderrDone),
      ),
    );
    exit = process.exitCode.then((code) {
      exited = true;
      timer.cancel();
      return code;
    });
    timer = Timer(_childDeadline, () {
      failure = 'Captured child exceeded 15 seconds.';
      if (!exited) process.kill(ProcessSignal.sigkill);
    });
  }

  final Process process;
  final output = <int>[];
  final _stdoutDone = Completer<void>();
  final _stderrDone = Completer<void>();
  Future<void> get pipesDone =>
      Future.wait([_stdoutDone.future, _stderrDone.future]).then((_) {});
  final subscriptions = <StreamSubscription<List<int>>>[];
  late final Future<int> exit;
  late final Timer timer;
  bool exited = false;
  int _bytes = 0;
  String? failure;

  Future<int> stop() async {
    try {
      if (!exited) process.kill(ProcessSignal.sigterm);
      int code;
      try {
        code = await exit.timeout(_ioDeadline);
      } on TimeoutException {
        // Failure cleanup is still restricted to this captured child.
        if (!exited) process.kill(ProcessSignal.sigkill);
        code = await exit.timeout(_ioDeadline);
      }
      await pipesDone.timeout(_ioDeadline);
      return code;
    } finally {
      timer.cancel();
      for (final subscription in subscriptions) {
        await subscription.cancel();
      }
    }
  }
}

Future<String> _utility(String executable, List<String> arguments) async {
  final child = _CapturedChild(
    await Process.start(
      executable,
      arguments,
      runInShell: false,
      includeParentEnvironment: false,
    ),
  );
  try {
    await child.process.stdin.close().timeout(_ioDeadline);
    expect(
      await child.exit.timeout(_ioDeadline),
      0,
      reason: 'Fixture permission/metadata utility must succeed.',
    );
    await child.pipesDone.timeout(_ioDeadline);
    expect(child.failure, isNull);
    return utf8.decode(child.output).trim();
  } finally {
    await child.stop();
  }
}

Future<Map<String, dynamic>> _request(
  String socketPath,
  String command,
  int pid, {
  required bool complete,
}) async {
  final socket = await Socket.connect(
    InternetAddress(socketPath, type: InternetAddressType.unix),
    0,
    timeout: _ioDeadline,
  );
  final response = Completer<List<int>>();
  final bytes = <int>[];
  final subscription = socket.listen(
    (chunk) {
      if (response.isCompleted) return;
      if (bytes.length + chunk.length > _frameLimit) {
        response.completeError(
          StateError('Unix response exceeded 4096 bytes.'),
        );
        return;
      }
      bytes.addAll(chunk);
      final newline = bytes.indexOf(10);
      if (newline >= 0) {
        if (newline != bytes.length - 1) {
          response.completeError(
            StateError('Expected one newline JSON frame.'),
          );
        } else {
          response.complete(bytes.sublist(0, newline));
        }
      }
    },
    onError: (Object error) {
      if (!response.isCompleted) response.completeError(error);
    },
    onDone: () {
      if (!response.isCompleted) {
        response.completeError(StateError('Truncated Unix response frame.'));
      }
    },
  );
  // Install the bounded reader before sending, including its error handler.
  final frame = response.future.timeout(_ioDeadline);
  try {
    final request = utf8.encode(
      '${jsonEncode({'schema_version': 1, 'service': 'smith-next', 'command': command})}\n',
    );
    expect(request.length, lessThanOrEqualTo(_frameLimit));
    socket.add(request);
    await socket.flush().timeout(_ioDeadline);
    final decoded = jsonDecode(utf8.decode(await frame));
    expect(decoded, isA<Map<String, dynamic>>());
    final reply = decoded as Map<String, dynamic>;
    expect(reply.keys.toSet(), {
      'schema_version',
      'service',
      'pid',
      'ok',
      'setup_complete',
      'error',
    });
    expect(reply['schema_version'], isA<int>());
    expect(reply['schema_version'], 1);
    expect(reply['service'], 'smith-next');
    expect(reply['pid'], isA<int>());
    expect(reply['pid'], pid);
    expect(reply['ok'], true);
    expect(reply['error'], isNull);
    expect(reply['setup_complete'], complete);
    return reply;
  } finally {
    socket.destroy();
    await subscription.cancel();
  }
}

Future<void> _waitForSocket(String path, _CapturedChild child) async {
  final deadline = Stopwatch()..start();
  while (deadline.elapsed < const Duration(seconds: 3)) {
    if (child.exited) {
      fail('Foreground Rust daemon exited before socket readiness.');
    }
    if (await FileSystemEntity.type(path, followLinks: false) !=
        FileSystemEntityType.notFound) {
      return;
    }
    await Future<void>.delayed(const Duration(milliseconds: 20));
  }
  fail(
    'Foreground Rust daemon did not publish its socket within three seconds.',
  );
}

Future<AppReply> _appState(String path, int pid) async {
  final socket = await Socket.connect(
    InternetAddress(path, type: InternetAddressType.unix),
    0,
    timeout: _ioDeadline,
  );
  final id = appId();
  final bytes = <int>[];
  final done = Completer<String>();
  final sub = socket.listen(
    (chunk) {
      if (bytes.length + chunk.length > appFrameLimit + 1) {
        if (!done.isCompleted) {
          done.completeError(StateError('App fixture frame exceeded bound.'));
        }
        return;
      }
      bytes.addAll(chunk);
      if (chunk.contains(10) && !done.isCompleted) {
        done.complete(utf8.decode(bytes));
      }
    },
    onError: (Object e) {
      if (!done.isCompleted) done.completeError(e);
    },
    onDone: () {
      if (!done.isCompleted) {
        done.completeError(StateError('Incomplete app frame.'));
      }
    },
  );
  try {
    socket.add(
      utf8.encode(
        '${jsonEncode({'schema_version': 1, 'service': 'smith-next', 'protocol': 'app', 'call_id': id, 'op': 'state', 'args': {}})}\n',
      ),
    );
    await socket.flush().timeout(_ioDeadline);
    final text = await done.future.timeout(_ioDeadline);
    expect(jsonDecode(text)['pid'], pid);
    return AppReply.parse(text, id);
  } finally {
    socket.destroy();
    await sub.cancel();
  }
}

void main() {
  final source = Platform.environment['SMITH_TEST_SERVICE'];
  test(
    'real Rust Unix socket: explicit completion survives detach and restart',
    () async {
      expect(
        Platform.isLinux,
        isTrue,
        reason: 'This integration contract is Linux-only.',
      );
      expect(
        source!.startsWith('/'),
        isTrue,
        reason: 'SMITH_TEST_SERVICE must explicitly name an absolute compiled binary.',
      );
      expect(
        await FileSystemEntity.type(source, followLinks: false),
        FileSystemEntityType.file,
        reason: 'The explicit source must be a regular file, not a symlink.',
      );

      final root = await Directory.systemTemp.createTemp('smith-dart-');
      final children = <_CapturedChild>[];
      try {
        await _utility('/usr/bin/chmod', ['0700', root.path]);
        final roots = [
          for (final name in ['data', 'config', 'runtime'])
            '${root.path}/$name',
        ];
        for (final path in [...roots, '${roots[0]}/smith-next']) {
          await Directory(path).create();
          await _utility('/usr/bin/chmod', ['0700', path]);
          expect((await Directory(path).stat()).mode & 0xfff, 0x1c0);
        }
        // Cargo's source may be group writable or hard-linked. Execute only this
        // independently copied, private, owner-owned, single-link 0755 fixture.
        final binary = await File(source).copy('${root.path}/smith-service');
        await _utility('/usr/bin/chmod', ['0755', binary.path]);
        final status = await File('/proc/self/status').readAsString();
        final uid = RegExp(
          r'^Uid:\s+\d+\s+(\d+)',
          multiLine: true,
        ).firstMatch(status)!.group(1)!;
        expect(
          await _utility('/usr/bin/stat', [
            '--format=%a:%h:%u',
            '--',
            binary.path,
          ]),
          '755:1:$uid',
        );
        final socketPath = '${roots[2]}/smith-next/service.sock';

        Future<_CapturedChild> launch() async {
          final child = _CapturedChild(
            await Process.start(
              binary.path,
              [
                'serve',
                '--data-root',
                roots[0],
                '--config-root',
                roots[1],
                '--runtime-root',
                roots[2],
              ],
              runInShell: false,
              includeParentEnvironment: false,
            ),
          );
          children.add(child);
          await child.process.stdin.close().timeout(_ioDeadline);
          await _waitForSocket(socketPath, child);
          return child;
        }

        final first = await launch();
        final appPath = '${roots[2]}/smith-next/app.sock';
        final appFirst = await _appState(appPath, first.process.pid);
        expect(appFirst.result['config'], isA<Map>());
        expect(appFirst.result['history'], isEmpty);
        expect(appFirst.result['active'], isNull);
        await _request(
          socketPath,
          'identity',
          first.process.pid,
          complete: false,
        );
        final initialCore = await _request(
          socketPath,
          'get_state',
          first.process.pid,
          complete: false,
        );
        // Real Rust flag=false remains valid for chat attachment. Managed
        // identity facts are supplied by the isolated control contract fixture,
        // not by invoking a real user manager.
        expect(
          reply(profile(setup: initialCore['setup_complete'] as bool))
              .chatReady,
          isTrue,
        );
        await _request(
          socketPath,
          'complete_setup',
          first.process.pid,
          complete: true,
        );
        // Disconnect without sending a command, then verify the same child still
        // answers. Closing a Dart client is not daemon lifecycle management.
        final detached = await Socket.connect(
          InternetAddress(socketPath, type: InternetAddressType.unix),
          0,
          timeout: _ioDeadline,
        );
        detached.destroy();
        await Future<void>.delayed(const Duration(milliseconds: 50));
        expect(first.exited, isFalse);
        await _request(
          socketPath,
          'get_state',
          first.process.pid,
          complete: true,
        );
        expect(
          await first.stop(),
          0,
          reason: 'Captured daemon must gracefully exit on SIGTERM.',
        );
        expect(first.failure, isNull);
        expect(
          await FileSystemEntity.type(socketPath, followLinks: false),
          FileSystemEntityType.notFound,
        );

        final second = await launch();
        final appSecond = await _appState(appPath, second.process.pid);
        expect(appSecond.instanceId, isNot(appFirst.instanceId));
        expect(appSecond.result['config'], appFirst.result['config']);
        expect(appSecond.result['history'], isEmpty);
        await _request(
          socketPath,
          'identity',
          second.process.pid,
          complete: true,
        );
        await _request(
          socketPath,
          'get_state',
          second.process.pid,
          complete: true,
        );
        expect(await second.stop(), 0);
        expect(second.failure, isNull);
        expect(
          await FileSystemEntity.type(socketPath, followLinks: false),
          FileSystemEntityType.notFound,
        );
      } finally {
        var allExited = true;
        for (final child in children.reversed) {
          try {
            await child.stop();
          } on Object {
            allExited = false;
          }
          allExited = allExited && child.exited;
        }
        // Never delete roots beneath a child whose exit is unconfirmed.
        if (allExited) await root.delete(recursive: true);
        expect(
          allExited,
          isTrue,
          reason: 'No captured Rust daemon may remain after this test.',
        );
      }
    },
    skip: source == null || source.isEmpty
        ? 'SMITH_TEST_SERVICE not supplied: set an explicit compiled Rust binary path to run the isolated Unix-socket integration test.'
        : false,
    timeout: const Timeout(Duration(seconds: 40)),
  );
}
