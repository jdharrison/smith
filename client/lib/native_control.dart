import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'control_api.dart';

const helperDeadline = Duration(seconds: 45);
const helperOutputLimit = 16 * 1024;
const helperCleanupGrace = Duration(seconds: 11);
const helperKillConfirmation = Duration(seconds: 1);
const bootInspectDeadline = Duration(seconds: 2);
const bootInspectCleanupGrace = Duration(seconds: 1);
const bootInspectKillConfirmation = Duration(seconds: 1);

String helperPath(String executable, String override) {
  if (override.isNotEmpty) {
    if (!override.startsWith('/') || override.contains('\u0000')) {
      throw const ControlFailure(
        'SMITH_SERVICE_EXECUTABLE must be an absolute path.',
      );
    }
    return override;
  }
  return '${File(executable).parent.path}/smith_backend/smith-service';
}

abstract class HelperExecutor {
  Future<String> execute(List<String> arguments, {Duration? deadline});
  void dispose();
}

class NativeControlApi implements ControlApi {
  NativeControlApi({HelperExecutor? executor, HelperExecutor? inspectExecutor})
    : _executor = executor ?? NativeHelperExecutor(),
      _inspectExecutor =
          inspectExecutor ??
          executor ??
          NativeHelperExecutor(
            deadline: bootInspectDeadline,
            cleanupGrace: bootInspectCleanupGrace,
            killConfirmation: bootInspectKillConfirmation,
          );
  final HelperExecutor _executor;
  final HelperExecutor _inspectExecutor;

  @override
  Future<ControlReply> inspect({Duration? deadline}) async =>
      ControlReply.parse(
        await _inspectExecutor.execute(['inspect'], deadline: deadline),
      );
  @override
  Future<ControlReply> prepare(String action) async {
    _checkAction(action);
    return ControlReply.parse(
      await _executor.execute(['prepare', '--action', action]),
      prepare: true,
    );
  }

  @override
  Future<ControlReply> apply(String action, String approvalToken) async {
    _checkAction(action);
    if (approvalToken.isEmpty) {
      throw const ControlFailure('Missing approval token.');
    }
    return ControlReply.parse(
      await _executor.execute([
        'apply',
        '--action',
        action,
        '--approval',
        approvalToken,
      ]),
    );
  }

  void _checkAction(String action) {
    if (!serviceActions.containsKey(action)) {
      throw const ControlFailure('Unsupported service action.');
    }
  }

  @override
  void dispose() {
    _inspectExecutor.dispose();
    if (!identical(_executor, _inspectExecutor)) _executor.dispose();
  }
}

typedef HelperLauncher = Future<Process> Function(
  String path,
  List<String> arguments,
);

class NativeHelperExecutor implements HelperExecutor {
  NativeHelperExecutor({
    Duration deadline = helperDeadline,
    Duration cleanupGrace = helperCleanupGrace,
    Duration killConfirmation = helperKillConfirmation,
  }) {
    _deadline = deadline;
    _cleanupGrace = cleanupGrace;
    _killConfirmation = killConfirmation;
    _validateBudgets();
  }

  // Test-only seams keep validation from executing a real helper or manager.
  NativeHelperExecutor.forTesting({
    required Future<String> Function() validate,
    required HelperLauncher launch,
    Duration deadline = helperDeadline,
    Duration cleanupGrace = helperCleanupGrace,
    Duration killConfirmation = helperKillConfirmation,
  }) : _validateOverride = validate,
       _launchOverride = launch {
    _deadline = deadline;
    _cleanupGrace = cleanupGrace;
    _killConfirmation = killConfirmation;
    _validateBudgets();
  }

  late final Duration _deadline;
  late final Duration _cleanupGrace;
  late final Duration _killConfirmation;

  void _validateBudgets() {
    if (_cleanupGrace <= Duration.zero || _killConfirmation <= Duration.zero) {
      throw ArgumentError('Cleanup budgets must be positive and finite.');
    }
    if (_deadline <= Duration.zero) {
      throw ArgumentError.value(
        _deadline,
        'deadline',
        'Must be positive and finite.',
      );
    }
  }

  Future<void>? _cleanupInFlight;

  Future<String> Function()? _validateOverride;
  HelperLauncher? _launchOverride;
  Completer<String>? _activeFailure;
  bool _pendingLaunch = false;
  Process? _child;
  bool _busy = false;
  bool _disposed = false;

  Future<String> _validatedPath() async {
    if (!Platform.isLinux) {
      throw const ControlFailure('Only Linux is supported.');
    }
    final path = helperPath(
      Platform.resolvedExecutable,
      const String.fromEnvironment('SMITH_SERVICE_EXECUTABLE'),
    );
    final file = File(path);
    if (await FileSystemEntity.type(path, followLinks: false) !=
            FileSystemEntityType.file ||
        await file.resolveSymbolicLinks() != path) {
      throw const ControlFailure(
        'Helper must be a canonical regular file, not a symlink.',
      );
    }
    // Reject writable ancestors as well as writable executable artifacts.
    var current = path;
    while (true) {
      final stat = await FileStat.stat(current);
      if (stat.mode & 0x12 != 0) {
        throw const ControlFailure('Helper path is group/world writable.');
      }
      final parent = File(current).parent.path;
      if (parent == current) break;
      current = parent;
    }
    final stat = await file.stat();
    if (stat.mode & 0x49 == 0) {
      throw const ControlFailure('Helper is not executable.');
    }
    final handle = await file.open();
    late List<int> header;
    try {
      header = await handle.read(20);
    } finally {
      await handle.close();
    }
    if (header.length != 20 ||
        header[0] != 0x7f ||
        header[1] != 69 ||
        header[2] != 76 ||
        header[3] != 70 ||
        header[4] != 2 ||
        header[5] != 1 ||
        header[18] != 62 ||
        header[19] != 0) {
      throw const ControlFailure(
        'Helper must be a native Linux x86_64 ELF executable.',
      );
    }
    return path;
  }

  @override
  Future<String> execute(List<String> arguments, {Duration? deadline}) =>
      exchange(arguments, deadline: deadline);

  Future<String> exchange(
    List<String> arguments, {
    List<int>? input,
    int outputLimit = helperOutputLimit,
    bool app = false,
    Duration? deadline,
  }) async {
    final effectiveDeadline = deadline ?? _deadline;
    if (effectiveDeadline <= Duration.zero) {
      throw ArgumentError.value(
        effectiveDeadline,
        'deadline',
        'Must be positive and finite.',
      );
    }
    if (_disposed || _busy || _pendingLaunch || _child != null) {
      throw const ControlFailure(
        'Helper unavailable: disposed, busy, or cleanup unverified.',
      );
    }
    _busy = true;
    final output = <int>[];
    final failure = Completer<String>();
    _activeFailure = failure;
    var captured = 0;
    final subscriptions = <StreamSubscription<List<int>>>[];
    final timer = Timer(effectiveDeadline, () {
      if (!failure.isCompleted) {
        failure.completeError(
          ControlFailure(
            'Helper exceeded ${effectiveDeadline.inMilliseconds}-millisecond deadline. '
            'Outcome unknown; no rollback assumed. Refresh before retry.',
            outcomeUnknown: true,
          ),
        );
      }
    });
    Future<String> work() async {
      final path = await (_validateOverride?.call() ?? _validatedPath());
      if (_disposed || failure.isCompleted) {
        throw const ControlFailure(
          'Helper cancelled. Outcome unknown; no rollback assumed. Refresh before retry.',
          outcomeUnknown: true,
        );
      }
      _pendingLaunch = true;
      late Process child;
      try {
        child =
            await (_launchOverride?.call(path, arguments) ??
                Process.start(path, arguments, runInShell: false));
        _child = child;
        unawaited(
          child.exitCode.then(
            (_) {
              if (identical(_child, child)) _child = null;
            },
            onError: (Object _) {
              // Keep captured ownership if exit observation itself fails.
            },
          ),
        );
        if (_disposed || failure.isCompleted) {
          // A late launch can arrive after execute's finally has already run.
          // Drain without retaining diagnostics while the helper cooperates.
          final drains = [
            child.stdout.listen((_) {}, onError: (Object _) {}),
            child.stderr.listen((_) {}, onError: (Object _) {}),
          ];
          try {
            await _cleanup();
          } finally {
            for (final drain in drains) {
              await drain.cancel();
            }
          }
          throw const ControlFailure(
            'Helper cancelled. Outcome unknown; no rollback assumed. Refresh before retry.',
            outcomeUnknown: true,
          );
        }
      } finally {
        _pendingLaunch = false;
      }
      void capture(List<int> bytes, bool stdout) {
        captured += bytes.length;
        if (captured > outputLimit) {
          if (!failure.isCompleted) {
            failure.completeError(
              const ControlFailure(
                'Helper output exceeded its bound. Outcome unknown; Refresh before retry.',
                outcomeUnknown: true,
              ),
            );
          }
          return;
        }
        if (stdout) output.addAll(bytes);
      }

      final outDone = Completer<void>();
      final errDone = Completer<void>();
      void streamError(Object error) {
        if (!failure.isCompleted) {
          failure.completeError(
            const ControlFailure('Cannot capture helper output.'),
          );
        }
      }

      subscriptions.add(
        child.stdout.listen(
          (b) => capture(b, true),
          onDone: outDone.complete,
          onError: streamError,
        ),
      );
      subscriptions.add(
        child.stderr.listen(
          (b) => capture(b, false),
          onDone: errDone.complete,
          onError: streamError,
        ),
      );
      final stdin = child.stdin;
      if (input != null) stdin.add(input);
      await stdin.close();
      final code = await child.exitCode;
      await Future.wait([outDone.future, errDone.future]);
      final text = utf8.decode(output);
      final bool ok;
      if (app) {
        final value = jsonDecode(text);
        if (value is! Map<String, dynamic> || value['ok'] is! bool) {
          throw const ControlFailure('Invalid app helper reply.');
        }
        ok = value['ok'] as bool;
      } else {
        ok = ControlReply.parse(text).ok;
      }
      if (code != 0 && ok) {
        throw const ControlFailure(
          'Helper exit status contradicts successful reply.',
        );
      }
      return text;
    }

    try {
      return await Future.any([work(), failure.future]);
    } on ControlFailure {
      rethrow;
    } on Object {
      throw const ControlFailure(
        'Cannot validate or execute bundled smith-service helper.',
      );
    } finally {
      timer.cancel();
      if (identical(_activeFailure, failure)) _activeFailure = null;
      await _cleanup();
      for (final sub in subscriptions) {
        await sub.cancel();
      }
      _busy = false;
    }
  }

  Future<void> _cleanup() {
    final pending = _cleanupInFlight;
    if (pending != null) return pending;
    final child = _child;
    if (child == null) return Future<void>.value();
    late final Future<void> cleanup;
    cleanup = _cleanupChild(child).whenComplete(() {
      if (identical(_cleanupInFlight, cleanup)) _cleanupInFlight = null;
    });
    _cleanupInFlight = cleanup;
    return cleanup;
  }

  Future<void> _cleanupChild(Process child) async {
    try {
      child.kill(ProcessSignal.sigterm);
      try {
        await child.exitCode.timeout(_cleanupGrace);
      } on TimeoutException {
        if (identical(_child, child)) child.kill(ProcessSignal.sigkill);
        await child.exitCode.timeout(_killConfirmation);
      }
      if (identical(_child, child)) _child = null;
    } on Object {
      // Retain ownership and block reuse when exit cannot be confirmed. Helper
      // exit is not rollback or proof that every manager descendant was reaped.
    }
  }

  @override
  void dispose() {
    _disposed = true;
    final failure = _activeFailure;
    if (failure != null && !failure.isCompleted) {
      failure.completeError(
        const ControlFailure(
          'Helper cancelled. Outcome unknown; no rollback assumed. Refresh before retry.',
          outcomeUnknown: true,
        ),
      );
    }
    unawaited(_cleanup());
  }
}
