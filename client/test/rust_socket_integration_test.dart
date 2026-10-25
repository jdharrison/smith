import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/app_v2.dart';
import 'package:smith_client/workspace_controller.dart';

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

// The optional v1 branch is installer-contract fixture coverage only. The
// production parser accepts v2 exclusively; it is never used as a fallback.
Future<AppReply> _appState(
  String path,
  int pid, {
  bool installerV1 = false,
  String op = 'state',
  Map<String, dynamic> args = const {},
  void Function(String)? inspectFrame,
}) async {
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
        '${jsonEncode({'schema_version': installerV1 ? 1 : 2, 'service': 'smith-next', 'protocol': 'app', 'call_id': id, 'op': op, 'args': args})}\n',
      ),
    );
    await socket.flush().timeout(_ioDeadline);
    final text = await done.future.timeout(_ioDeadline);
    inspectFrame?.call(text);
    expect(jsonDecode(text)['pid'], pid);
    if (installerV1) {
      final envelope = jsonDecode(text) as Map<String, dynamic>;
      expect(envelope.keys.toSet(), {
        'schema_version',
        'service',
        'protocol',
        'pid',
        'instance_id',
        'call_id',
        'ok',
        'result',
        'error',
      });
      expect(envelope['schema_version'], 1);
      expect(envelope['service'], 'smith-next');
      expect(envelope['protocol'], 'app');
      expect(envelope['call_id'], id);
      expect(envelope['ok'], true);
      expect(envelope['error'], isNull);
      final result = Map<String, dynamic>.from(envelope['result']);
      expect(result.keys.toSet(), {
        'config',
        'history',
        'active',
        'providers',
        'retention',
      });
      expect(result['active'], isNull);
      expect(result['history'], isA<List>());
      expect(() => AppReply.parse(text, id), throwsA(isA<AppFailure>()));
      return AppReply(envelope['instance_id'], result);
    }
    final reply = AppReply.parse(text, id);
    validateV2(op, reply.result);
    return reply;
  } finally {
    socket.destroy();
    await sub.cancel();
  }
}

class _SocketAppApi implements AppApi {
  _SocketAppApi(this.path, this.pid);
  final String path;
  final int pid;
  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) =>
      _appState(path, pid, op: op, args: args);
  @override
  void dispose() {}
}

Future<void> _legacyFixture(
  String source,
  String revision,
  Future<void> Function(String path, int pid) check,
) async {
  expect(Platform.isLinux, isTrue);
  expect(source.startsWith('/'), isTrue);
  expect(
    await FileSystemEntity.type(source, followLinks: false),
    FileSystemEntityType.file,
  );
  final root = await Directory.systemTemp.createTemp('smith-dart-legacy-');
  _CapturedChild? child;
  try {
    await _utility('/usr/bin/chmod', ['0700', root.path]);
    for (final name in ['data', 'config', 'runtime', 'data/smith-next']) {
      final directory = Directory('${root.path}/$name');
      await directory.create();
      await _utility('/usr/bin/chmod', ['0700', directory.path]);
    }
    final binary = await File(source).copy('${root.path}/smith-service');
    await _utility('/usr/bin/chmod', ['0755', binary.path]);
    Map<String, dynamic> job(
      String id,
      String kind, {
      String status = 'succeeded',
      dynamic result,
      dynamic error,
    }) => {
      'fingerprint': 'a' * 64,
      'operation': {
        'request_id': id,
        'kind': kind,
        'status': status,
        'phase': status == 'running' ? 'generation' : 'complete',
        'config_revision': '__CONFIG_REVISION__',
        'result': result,
        'error': error,
      },
    };
    const audioExpired = {
      'code': 'AUDIO_EXPIRED',
      'message': 'Legacy audio expired',
    };
    final state = {
      'schema_version': 1,
      'config': {
        'model': null,
        'voice': 'inert-legacy-voice',
        'revision': '__CONFIG_REVISION__',
      },
      'history': [
        {'role': 'user', 'content': ''},
        {'role': 'assistant', 'content': ''},
      ],
      'jobs': [
        job(
          '11111111111111111111111111111111',
          'submit',
          result: {
            'assistant_text': 'Saved legacy answer',
            'audio': null,
            'speech_error': audioExpired,
          },
        ),
        job('22222222222222222222222222222222', 'speak', error: audioExpired),
        job('33333333333333333333333333333333', 'submit', status: 'running'),
      ],
      'expired_ids': <String>[],
    };
    // Dart native ints cannot encode u64::MAX. The legacy file contains a
    // numeric JSON token, not the new v2 string representation.
    final file = File('${root.path}/data/smith-next/app-state.json');
    await file.writeAsString(
      jsonEncode(state).replaceAll('"__CONFIG_REVISION__"', revision),
      flush: true,
    );
    await _utility('/usr/bin/chmod', ['0600', file.path]);
    child = _CapturedChild(
      await Process.start(
        binary.path,
        [
          'serve',
          '--data-root',
          '${root.path}/data',
          '--config-root',
          '${root.path}/config',
          '--runtime-root',
          '${root.path}/runtime',
        ],
        runInShell: false,
        includeParentEnvironment: false,
      ),
    );
    await child.process.stdin.close().timeout(_ioDeadline);
    await _waitForSocket('${root.path}/runtime/smith-next/service.sock', child);
    await check('${root.path}/runtime/smith-next/app.sock', child.process.pid);
    expect(await child.stop(), 0);
    expect(child.failure, isNull);
  } finally {
    if (child != null) await child.stop();
    if (child == null || child.exited) await root.delete(recursive: true);
  }
}

void main() {
  final source = Platform.environment['SMITH_TEST_SERVICE'];
  for (final revision in ['1', '9007199254740992']) {
    test(
      'real Rust migrated inert voice, interrupted job and config revision $revision',
      () async {
        await _legacyFixture(source!, revision, (path, pid) async {
          final originalRevision = revision == '1' ? 1 : revision;
          final expectedRevision = revision == '1' ? 2 : '9007199254740993';
          final state = await _appState(path, pid);
          expect(state.result['config']['revision'], expectedRevision);
          final operations = state.result['operations'] as List;
          expect(operations, hasLength(3));
          for (final o in operations) {
            expect(o['config_revision'], originalRevision);
          }
          final conversation =
              state.result['conversations'].single['conversation_id'];
          final detail = await _appState(
            path,
            pid,
            op: 'conversation_get',
            args: {'conversation_id': conversation},
          );
          expect((detail.result['history'] as List).map((m) => m['content']), [
            '',
            '',
          ]);
          for (final id in [
            '11111111111111111111111111111111',
            '22222222222222222222222222222222',
          ]) {
            final poll = await _appState(
              path,
              pid,
              op: 'poll',
              args: {'request_id': id, 'instance_id': state.instanceId},
            );
            expect(poll.result['config_revision'], originalRevision);
            if (poll.result['kind'] == 'submit') {
              expect(
                poll.result['result']['assistant_text'],
                'Saved legacy answer',
              );
              expect(
                poll.result['result']['speech_error']['code'],
                'AUDIO_EXPIRED',
              );
            } else {
              expect(poll.result['result'], isNull);
              expect(poll.result['error']['code'], 'AUDIO_EXPIRED');
            }
          }
          final workspace = WorkspaceController(
            _SocketAppApi(path, pid),
            pollInterval: const Duration(days: 1),
          );
          try {
            await workspace.synchronize();
            expect(workspace.stale, false, reason: workspace.error);
            expect(workspace.selected.error, contains('INTERRUPTED'));
            expect(workspace.selected.pendingId, isNull);
          } finally {
            workspace.dispose();
          }
          await _appState(
            path,
            pid,
            installerV1: true,
            inspectFrame: (text) {
              final wire = revision == '1' ? '2' : '9007199254740993';
              expect(text, contains('"revision":$wire'));
              expect(text, isNot(contains('"revision":"$revision"')));
            },
          );
          // Model=null forces definitive admission rejection before any worker or
          // inference. MODEL_UNAVAILABLE proves the exact revision echo parsed.
          for (final op in ['submit', 'submit_new']) {
            await expectLater(
              _appState(
                path,
                pid,
                op: op,
                args: {
                  'request_id': appId(),
                  'text': 'Not admitted; no model',
                  'output': 'chat',
                  'expected_config_revision': expectedRevision,
                  if (op == 'submit') 'conversation_id': conversation,
                  if (op == 'submit')
                    'expected_history_revision':
                        detail.result['conversation']['history_revision'],
                },
              ),
              throwsA(
                isA<AppFailure>().having(
                  (e) => e.code,
                  'code',
                  'MODEL_UNAVAILABLE',
                ),
              ),
            );
          }
        });
      },
      skip: source == null
          ? 'SMITH_TEST_SERVICE required for isolated Rust migration fixture.'
          : false,
    );
  }
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
        final installerState = await _appState(
          appPath,
          first.process.pid,
          installerV1: true,
        );
        final appFirst = await _appState(appPath, first.process.pid);
        expect(installerState.result['history'], isEmpty);
        expect(installerState.instanceId, appFirst.instanceId);
        expect(installerState.result['config'], appFirst.result['config']);
        expect(appFirst.result['config'], isA<Map>());
        expect(appFirst.result['conversations'], isEmpty);
        expect(appFirst.result['operations'], isEmpty);
        expect(appFirst.result['inventory_observed'], isFalse);
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
        expect(appSecond.result['conversations'], isEmpty);
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
