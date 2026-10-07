import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'desktop_backend.dart';

const setupEndpoint = 'http://127.0.0.1:11434';
bool exactLocalTag(String value) =>
    value.length <= 200 &&
    RegExp(
      r'^[A-Za-z0-9][A-Za-z0-9._-]*(/[A-Za-z0-9][A-Za-z0-9._-]*)*:[A-Za-z0-9][A-Za-z0-9._-]*$',
    ).hasMatch(value) &&
    !value.toLowerCase().contains('cloud');

abstract class SetupChild implements BackendProcess {
  Stream<List<int>> get stdout;
  Stream<List<int>> get stderr;
  Future<void> writeApproval(String line);
}

abstract class SetupRuntime {
  Future<String> discover();
  Future<String> resolve(String path);
  Future<SetupChild> launch(String path, List<String> argv);
}

class NativeSetupRuntime implements SetupRuntime {
  NativeSetupRuntime({
    NativeExecutableResolver? resolver,
    String Function()? desktopExecutable,
  }) : resolver = resolver ?? NativeExecutableResolver(),
       desktopExecutable =
           desktopExecutable ?? (() => Platform.resolvedExecutable);
  final NativeExecutableResolver resolver;
  final String Function() desktopExecutable;
  @override
  Future<String> discover() {
    final executable = desktopExecutable();
    if (!executable.startsWith('/')) {
      throw const FormatException('Desktop executable must be absolute.');
    }
    return resolve('${File(executable).parent.path}/smith_backend/smith');
  }

  @override
  Future<String> resolve(String path) => resolver.resolve(path);
  @override
  Future<SetupChild> launch(String path, List<String> argv) async =>
      _SetupProcess(await Process.start(path, argv, runInShell: false));
}

class _SetupProcess implements SetupChild {
  _SetupProcess(this.process) {
    exitCode = process.exitCode.then((value) {
      alive = false;
      return value;
    });
  }
  final Process process;
  @override
  late final Future<int> exitCode;
  @override
  bool alive = true;
  @override
  Stream<List<int>> get stdout => process.stdout;
  @override
  Stream<List<int>> get stderr => process.stderr;
  @override
  Future<void> writeApproval(String line) async {
    process.stdin.write(line);
    await process.stdin.flush();
  }

  @override
  Future<void> closeInput() => process.stdin.close();
  @override
  bool terminate({bool force = false}) =>
      process.kill(force ? ProcessSignal.sigkill : ProcessSignal.sigterm);
}

Map<String, dynamic> _object(dynamic value) {
  if (value is! Map<String, dynamic>) {
    throw const FormatException('Expected JSON object.');
  }
  return value;
}

String _text(dynamic value, {int limit = 2048}) {
  if (value is! String ||
      value.isEmpty ||
      value.length > limit ||
      value.runes.any((r) => r < 32 || r == 127)) {
    throw const FormatException('Invalid protocol text.');
  }
  return value;
}

int? _size(dynamic value) {
  if (value == null) return null;
  if (value is! int || value < 0 || value > 9007199254740991) {
    throw const FormatException('Invalid byte count.');
  }
  return value;
}

void _envelope(Map<String, dynamic> value, String command) {
  if (value['schema_version'] != 1 ||
      value['schema_version'] is! int ||
      value['command'] != command) {
    throw const FormatException('Unsupported setup protocol.');
  }
}

class InstalledModel {
  InstalledModel(this.name, this.digest, this.size);
  final String name;
  final String digest;
  final int? size;
}

class SetupInventory {
  SetupInventory(this.status, this.models, this.message);
  final String status;
  final List<InstalledModel> models;
  final String? message;
  static SetupInventory parse(String text) {
    final v = _object(jsonDecode(text));
    _envelope(v, 'inspect');
    if (v['provider'] != 'ollama' ||
        v['endpoint'] != setupEndpoint ||
        !['ready', 'unavailable', 'error'].contains(v['status']) ||
        v['models'] is! List ||
        !v.containsKey('error')) {
      throw const FormatException('Invalid inspect identity/status.');
    }
    final models = <InstalledModel>[];
    final names = <String>{};
    for (final raw in v['models']) {
      final m = _object(raw);
      final name = _text(m['name'], limit: 256);
      if (!exactLocalTag(name) || !names.add(name) || !m.containsKey('size')) {
        throw const FormatException('Invalid model inventory.');
      }
      models.add(
        InstalledModel(name, _text(m['digest'], limit: 256), _size(m['size'])),
      );
    }
    String? message;
    if (v['error'] != null) {
      final error = _object(v['error']);
      _text(error['code'], limit: 128);
      message = _text(error['message']);
    }
    if ((v['status'] == 'ready') != (v['error'] == null) ||
        (v['status'] != 'ready' && models.isNotEmpty)) {
      throw const FormatException('Inconsistent inspect result.');
    }
    return SetupInventory(v['status'], List.unmodifiable(models), message);
  }
}

class PullProtocol {
  PullProtocol(this.model);
  final String model;
  String? operation;
  int sequence = 0;
  bool approved = false;
  bool terminal = false;
  String? status;
  Map<String, dynamic> accept(String line) {
    if (terminal) throw const FormatException('Event after terminal.');
    final v = _object(jsonDecode(line));
    _envelope(v, 'pull');
    final id = _text(v['operation_id'], limit: 128);

    if (v['seq'] is! int ||
        v['seq'] != sequence ||
        (operation != null && id != operation)) {
      throw const FormatException('Invalid operation/sequence.');
    }
    if (operation == null && v['event'] == 'terminal') {
      // Preflight can fail before an approval request exists. This exception
      // permits only a fully validated seq-zero error, never consent or success.
      if (v['status'] != 'error') {
        throw const FormatException('Invalid preflight terminal result.');
      }
      _text(v['code'], limit: 128);
      _text(v['message']);
      operation = id;
      terminal = true;
      status = 'error';
    } else if (operation == null) {
      if (v['event'] != 'approval_required' ||
          v['model'] != model ||
          v['endpoint'] != setupEndpoint) {
        throw const FormatException('Approval identity mismatch.');
      }
      operation = id;
    } else if (v['event'] == 'progress') {
      if (!approved) throw const FormatException('Progress before consent.');
      _text(v['status']);
      final completed = _size(v['completed']);
      final total = _size(v['total']);
      if (completed != null && total != null && completed > total) {
        throw const FormatException('Invalid progress.');
      }
    } else if (v['event'] == 'terminal') {
      if (!['success', 'error', 'cancelled'].contains(v['status']) ||
          (v['status'] == 'success' && !approved)) {
        throw const FormatException('Invalid terminal result.');
      }
      _text(v['code'], limit: 128);
      _text(v['message']);
      terminal = true;
      status = v['status'];
    } else {
      throw const FormatException('Unexpected pull event.');
    }
    sequence++;
    return v;
  }

  String approval() {
    if (operation == null || approved || terminal) {
      throw const FormatException('No pending approval.');
    }
    approved = true;
    return '${jsonEncode({'schema_version': 1, 'operation_id': operation, 'model': model, 'approved': true})}\n';
  }
}

/// Captures exactly one child. Unknown cleanup retains ownership and blocks reuse.
/// Bounds apply to bytes before decoding, including split UTF-8/NDJSON frames.
class DesktopSetup {
  DesktopSetup({
    SetupRuntime? runtime,
    BackendClock? clock,
    this.inspectTimeout = const Duration(seconds: 10),
    this.overallTimeout = const Duration(minutes: 15),
    this.idleTimeout = const Duration(seconds: 30),
    this.approvalTimeout = const Duration(seconds: 60),
  }) : runtime = runtime ?? NativeSetupRuntime(),
       clock = clock ?? SystemBackendClock();
  final SetupRuntime runtime;
  final BackendClock clock;
  final Duration inspectTimeout, overallTimeout, idleTimeout, approvalTimeout;
  SetupChild? _child;
  bool _busy = false;
  bool _launchPending = false;
  bool _cancelled = false;
  int _generation = 0;
  Future<void>? _cleanup;
  Completer<void>? _abort;
  void Function()? _approvedActivity;
  bool get busy => _busy || _launchPending || _child != null;
  bool get cleanupUnknown => !_busy && (_launchPending || _child != null);
  Future<String> discover() => runtime.discover();
  Future<void> cancel() async {
    _cancelled = true;
    _generation++;
    if (!(_abort?.isCompleted ?? true)) {
      _abort!.completeError(
        const FormatException(
          'Setup cancelled; provider cache/outcome may remain unknown.',
        ),
      );
    }
    await _clean();
  }

  Future<void> _clean() =>
      _cleanup ??= _cleanChild().whenComplete(() => _cleanup = null);
  Future<void> _cleanChild() async {
    final child = _child;
    if (child == null) return;
    try {
      child.closeInput().timeout(const Duration(seconds: 1)).ignore();
    } catch (_) {
      // Broken stdin must not prevent captured-child TERM/KILL cleanup.
    }
    try {
      if (child.alive) child.terminate();
    } catch (_) {
      /* Still attempt exit confirmation. */
    }
    try {
      await clock.bound(child.exitCode, const Duration(seconds: 5));
      if (identical(_child, child)) _child = null;
      return;
    } catch (_) {
      /* Escalate only the captured child. */
    }
    try {
      if (child.alive) child.terminate(force: true);
    } catch (_) {
      /* Confirm below. */
    }
    try {
      await clock.bound(child.exitCode, const Duration(seconds: 2));
      if (identical(_child, child)) _child = null;
    } catch (_) {
      /* Unknown cleanup deliberately retains ownership. */
    }
  }

  Future<SetupInventory> inspect(String path) async {
    final output = StringBuffer();
    final exit = await _run(
      path,
      ['setup', 'inspect'],
      pull: false,
      onLine: (line) async {
        output.write(line);
      },
    );
    final result = SetupInventory.parse(output.toString());
    if (exit != 0 && result.status == 'ready') {
      throw const FormatException('Inspect exited unsuccessfully.');
    }
    return result;
  }

  Future<void> pull(
    String path,
    String model, {
    required Future<bool> Function() confirm,
    required void Function(Map<String, dynamic>) onEvent,
    required bool Function() permitted,
  }) async {
    if (!exactLocalTag(model) || !permitted()) {
      throw const FormatException('Model download is blocked.');
    }
    final protocol = PullProtocol(model);
    final generation = _generation;
    final exit = await _run(
      path,
      ['setup', 'pull', '--model', model],
      pull: true,
      onLine: (line) async {
        final event = protocol.accept(line);
        onEvent(event);
        if (event['event'] == 'approval_required') {
          // Keep decoding while the operator decides: a terminal, malformed or
          // stale event must invalidate consent before any stdin write.
          unawaited(() async {
            try {
              final consent = await clock.bound(confirm(), approvalTimeout);
              if (generation != _generation ||
                  _cancelled ||
                  protocol.terminal) {
                return;
              }
              if (!consent || !permitted()) {
                await cancel();
                return;
              }
              await _child!.writeApproval(protocol.approval());
              _approvedActivity?.call();
            } catch (error) {
              if (generation == _generation && !(_abort?.isCompleted ?? true)) {
                _abort!.completeError(error);
              }
            }
          }());
        }
      },
    );
    if (!protocol.terminal || exit != 0 || protocol.status != 'success') {
      throw const FormatException(
        'Download did not complete successfully. Cache/outcome may remain unknown; retry only explicitly.',
      );
    }
  }

  Future<int> _run(
    String path,
    List<String> argv, {
    required bool pull,
    required Future<void> Function(String) onLine,
  }) async {
    if (busy) {
      throw const FormatException('Setup child active or cleanup unknown.');
    }
    _busy = true;
    _cancelled = false;
    final abort = _abort = Completer<void>();
    abort.future.ignore();
    final finished = Completer<int>();
    final subscriptions = <StreamSubscription<List<int>>>[];
    Timer? overall, idle;
    var bytes = <int>[];
    var count = 0;
    var stderrCount = 0;
    Future<void> chain = Future.value();
    void fail(Object error) {
      if (!finished.isCompleted) finished.completeError(error);
    }

    void armIdle(Duration duration) {
      idle?.cancel();
      idle = Timer(
        duration,
        () => fail(TimeoutException('Setup idle/approval deadline.')),
      );
    }

    try {
      final executable = await clock.bound(
        runtime.resolve(path),
        inspectTimeout,
      );
      if (_cancelled) throw const FormatException('Setup cancelled.');
      // Do not lose ownership if launch resolves after cancellation/deadline.
      _launchPending = true;
      final launching = runtime
          .launch(executable, argv)
          .then(
            (child) async {
              _launchPending = false;
              _child = child;
              if (_cancelled || finished.isCompleted) await _clean();
              return child;
            },
            onError: (Object error, StackTrace stack) {
              _launchPending = false;
              Error.throwWithStackTrace(error, stack);
            },
          );
      launching.ignore();
      final child = await clock.bound(launching, inspectTimeout);
      if (_cancelled) throw const FormatException('Setup cancelled.');
      void armOverall(Duration duration) {
        overall?.cancel();
        overall = Timer(
          duration,
          () => fail(TimeoutException('Setup overall deadline.')),
        );
      }

      armOverall(pull ? approvalTimeout : inspectTimeout);
      _approvedActivity = () {
        armOverall(overallTimeout);
        armIdle(idleTimeout);
      };
      armIdle(pull ? approvalTimeout : inspectTimeout);
      abort.future.then(
        (_) {},
        onError: (Object e) {
          fail(e);
        },
      );
      subscriptions.add(
        child.stderr.listen((chunk) {
          stderrCount += chunk.length;
          if (stderrCount > 16384) {
            fail(const FormatException('Setup stderr exceeds bound.'));
          }
        }, onError: fail),
      );
      subscriptions.add(
        child.stdout.listen(
          (chunk) {
            count += chunk.length;
            if (count > (pull ? 16 * 1024 * 1024 : 256 * 1024)) {
              fail(const FormatException('Setup stdout exceeds bound.'));
              return;
            }
            for (final byte in chunk) {
              if (pull && byte == 10) {
                final frame = bytes;
                bytes = [];
                chain = chain.then((_) async {
                  if (finished.isCompleted || _cancelled) return;
                  final line = utf8.decode(frame);
                  final event = _object(jsonDecode(line));
                  if (event['event'] == 'approval_required') {
                    armOverall(approvalTimeout);
                    armIdle(approvalTimeout);
                  }
                  await onLine(line);
                  if (event['event'] != 'approval_required') {
                    armIdle(idleTimeout);
                  }
                });
                chain.then(
                  (_) {},
                  onError: (Object e) {
                    fail(e);
                  },
                );
              } else {
                bytes.add(byte);
                if (bytes.length > (pull ? 16384 : 256 * 1024)) {
                  fail(const FormatException('Setup frame exceeds bound.'));
                  return;
                }
              }
            }
          },
          onError: fail,
          onDone: () {
            chain = chain.then((_) async {
              if (pull && bytes.isNotEmpty) {
                throw const FormatException('Unterminated setup event.');
              }
              if (!pull) await onLine(utf8.decode(bytes));
              final exit = await child.exitCode;

              if (!finished.isCompleted) finished.complete(exit);
            });
            chain.then(
              (_) {},
              onError: (Object e) {
                fail(e);
              },
            );
          },
        ),
      );
      return await finished.future;
    } catch (_) {
      _cancelled = true;
      rethrow;
    } finally {
      overall?.cancel();
      idle?.cancel();
      for (final subscription in subscriptions) {
        // Listener detachment is immediate; never let an untrusted stream's
        // cancellation future hold captured-child cleanup indefinitely.
        subscription.cancel().ignore();
      }
      await _clean();
      _generation++;
      _busy = false;
      _abort = null;
      _approvedActivity = null;
    }
  }
}
