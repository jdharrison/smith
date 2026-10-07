import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:math';

import 'package:flutter/foundation.dart';
import 'smith_bridge.dart';

enum ProbeKind { ready, absent, conflict, unknown }

class BackendProbe {
  const BackendProbe(this.kind, [this.health]);
  final ProbeKind kind;
  final HealthInfo? health;
}

bool chatReady(HealthInfo h, {String? model, String? session}) =>
    h.ok &&
    h.service == 'smith-bridge' &&
    h.profile == 'chat-only' &&
    !h.echo &&
    h.provider == 'ollama' &&
    h.providerAvailable == true &&
    h.modelAvailable == true &&
    h.model != null &&
    h.model!.isNotEmpty &&
    (model == null || model.isEmpty || h.model == model) &&
    (session == null || h.desktopSession == session);

abstract interface class BackendProcess {
  Future<int> get exitCode;
  bool get alive;
  Future<void> closeInput();
  bool terminate({bool force = false});
}

abstract interface class BackendRuntime {
  Future<BackendProbe> probe();
  Future<String> resolve(String executable);
  Future<BackendProcess> launch(String executable, List<String> arguments);
  String session();
}

abstract interface class BackendClock {
  DateTime now();
  Future<void> delay(Duration duration);
  Future<T> bound<T>(Future<T> future, Duration duration);
  Future<bool> exited(BackendProcess child, Duration duration);
}

class SystemBackendClock implements BackendClock {
  @override
  DateTime now() => DateTime.now();
  @override
  Future<void> delay(Duration duration) => Future<void>.delayed(duration);
  @override
  Future<T> bound<T>(Future<T> future, Duration duration) =>
      future.timeout(duration);
  @override
  Future<bool> exited(BackendProcess child, Duration duration) async {
    try {
      await child.exitCode.timeout(duration);
      return true;
    } catch (_) {
      return false;
    }
  }
}

class ExecutableMetadata {
  const ExecutableMetadata(this.mode, this.owner);
  final int mode;
  final int owner;
}

abstract interface class ExecutableFiles {
  int get effectiveUid;
  Future<ExecutableMetadata> inspect(String path);
  Future<List<int>> header(String path);
}

/// Dart FileStat does not expose uid. Ownership uses glibc lstat, guarded by
/// the Linux x86_64 ABI check in the resolver; file contents use dart:io.
class LinuxExecutableFiles implements ExecutableFiles {
  final _libc = DynamicLibrary.open('libc.so.6');
  @override
  int get effectiveUid =>
      _libc.lookupFunction<Uint32 Function(), int Function()>('geteuid')();
  @override
  Future<ExecutableMetadata> inspect(String path) async {
    final malloc = _libc
        .lookupFunction<
          Pointer<Void> Function(IntPtr),
          Pointer<Void> Function(int)
        >('malloc');
    final free = _libc
        .lookupFunction<
          Void Function(Pointer<Void>),
          void Function(Pointer<Void>)
        >('free');
    final lstat = _libc
        .lookupFunction<
          Int32 Function(Pointer<Uint8>, Pointer<Void>),
          int Function(Pointer<Uint8>, Pointer<Void>)
        >('lstat');
    final bytes = utf8.encode(path);
    final name = malloc(bytes.length + 1).cast<Uint8>();
    // Linux x86_64 glibc struct stat: uid at 28, mode at 24, size 144.
    final buffer = malloc(144);
    try {
      if (name == nullptr || buffer == nullptr) {
        throw const FormatException('File checks unavailable.');
      }
      name.asTypedList(bytes.length + 1).setAll(0, [...bytes, 0]);
      if (lstat(name, buffer) != 0) {
        throw const FormatException('Executable or ancestor unavailable.');
      }
      final fields = buffer.cast<Uint32>();
      return ExecutableMetadata(fields[6], fields[7]);
    } finally {
      free(name.cast<Void>());
      free(buffer);
    }
  }

  @override
  Future<List<int>> header(String path) async {
    final file = await File(path).open();
    try {
      return await file.read(20);
    } finally {
      await file.close();
    }
  }
}

/// No shell, PATH, discovery or probes that execute the configured file.
/// These checks do not authenticate a package or prevent same-user file races.
class NativeExecutableResolver {
  NativeExecutableResolver({this.files});
  final ExecutableFiles? files;
  Future<String> resolve(String path) async {
    if (!Platform.isLinux || Abi.current() != Abi.linuxX64) {
      throw const FormatException('Native Linux x86_64 required.');
    }
    final parts = path.split('/');
    if (!path.startsWith('/') ||
        path.contains('\u0000') ||
        path.length > 4096 ||
        parts.skip(1).any((p) => p.isEmpty || p == '.' || p == '..')) {
      throw const FormatException('Use a normalized absolute executable path.');
    }
    final io = files ?? LinuxExecutableFiles();
    final uid = io.effectiveUid;
    if (uid == 0) throw const FormatException('Run as an unprivileged user.');
    for (var i = 0; i < parts.length; i++) {
      final current = i == 0 ? '/' : parts.take(i + 1).join('/');
      final stat = await io.inspect(current);
      final mode = stat.mode;
      final owner = stat.owner;
      final last = i == parts.length - 1;
      if ((owner != 0 && owner != uid) ||
          (mode & 0x12) != 0 ||
          (mode & 0xC00) != 0 ||
          (mode & 0xF000) != (last ? 0x8000 : 0x4000) ||
          (last && (mode & (owner == uid ? 0x40 : 1)) == 0)) {
        throw const FormatException(
          'Unsafe ownership, permissions, file type or symlink ancestor.',
        );
      }
    }
    final h = await io.header(path);
    if (h.length != 20 ||
        h[0] != 0x7f ||
        h[1] != 69 ||
        h[2] != 76 ||
        h[3] != 70 ||
        h[4] != 2 ||
        h[5] != 1 ||
        h[6] != 1 ||
        (h[16] != 2 && h[16] != 3) ||
        h[17] != 0 ||
        h[18] != 62 ||
        h[19] != 0) {
      throw const FormatException(
        'A native x86_64 ELF executable is required.',
      );
    }
    return path;
  }
}

class NativeBackendRuntime implements BackendRuntime {
  NativeBackendRuntime({NativeExecutableResolver? resolver})
    : resolver = resolver ?? NativeExecutableResolver();
  final NativeExecutableResolver resolver;
  @override
  Future<String> resolve(String executable) => resolver.resolve(executable);
  @override
  String session() {
    final random = Random.secure();
    return List.generate(
      16,
      (_) => random.nextInt(256).toRadixString(16).padLeft(2, '0'),
    ).join();
  }

  @override
  Future<BackendProbe> probe() async {
    // Direct dart:io preserves errno; HTTP wrappers can erase refused vs timeout.
    final client = HttpClient()..findProxy = (_) => 'DIRECT';
    try {
      return await (() async {
        final request = await client.getUrl(
          Uri.parse('http://127.0.0.1:4783/v1/health'),
        );
        request.followRedirects = false;
        final response = await request.close();
        if (response.statusCode != 200) {
          return const BackendProbe(ProbeKind.conflict);
        }
        final bytes = <int>[];
        await for (final chunk in response) {
          if (bytes.length + chunk.length > 16384) {
            return const BackendProbe(ProbeKind.unknown);
          }
          bytes.addAll(chunk);
        }
        final body = jsonDecode(utf8.decode(bytes)) as Map<String, dynamic>;
        final h = HealthInfo.fromJson(body);
        final unknownAvailability =
            h.service == 'smith-bridge' &&
            h.profile == 'chat-only' &&
            !h.echo &&
            (h.provider == null ||
                h.model == null ||
                h.providerAvailable == null ||
                h.modelAvailable == null);
        return BackendProbe(
          chatReady(h)
              ? ProbeKind.ready
              : unknownAvailability
              ? ProbeKind.unknown
              : ProbeKind.conflict,
          h,
        );
      })().timeout(const Duration(seconds: 2));
    } on SocketException catch (e) {
      return BackendProbe(
        e.osError?.errorCode == 111 ? ProbeKind.absent : ProbeKind.unknown,
      );
    } catch (_) {
      return const BackendProbe(ProbeKind.unknown);
    } finally {
      client.close(force: true);
    }
  }

  @override
  Future<BackendProcess> launch(
    String executable,
    List<String> arguments,
  ) async => _NativeProcess(
    await Process.start(executable, arguments, runInShell: false),
  );
}

class _NativeProcess implements BackendProcess {
  _NativeProcess(this.process) {
    // Drain and discard each chunk; never accumulate or expose raw child output.
    process.stdout.listen((_) {}, onError: (Object _) {});
    process.stderr.listen((_) {}, onError: (Object _) {});
    exitCode = process.exitCode.then((code) {
      alive = false;
      return code;
    });
  }
  final Process process;
  @override
  late final Future<int> exitCode;
  @override
  bool alive = true;
  @override
  Future<void> closeInput() => process.stdin.close();
  @override
  bool terminate({bool force = false}) =>
      process.kill(force ? ProcessSignal.sigkill : ProcessSignal.sigterm);
}

enum BootState {
  checking,
  absent,
  conflict,
  unknown,
  starting,
  owned,
  external,
  stopping,
  cleanupUnknown,
}

class DesktopBackendController extends ChangeNotifier {
  DesktopBackendController({BackendRuntime? runtime, BackendClock? clock})
    : runtime = runtime ?? NativeBackendRuntime(),
      clock = clock ?? SystemBackendClock();
  final BackendRuntime runtime;
  final BackendClock clock;
  BootState state = BootState.checking;
  String executable = '';
  String model = '';
  String? observedModel;
  String detail = 'Checking local Smith readiness. No backend is started.';
  BackendProcess? _child;
  String? _session;
  bool _disposed = false;
  bool _busy = false;
  bool _launchPending = false;
  bool _launching = false;
  Future<void>? _cleanupTask;
  int _generation = 0;
  Future<void>? _stop;
  bool get _cleanupActive =>
      state == BootState.stopping || _cleanupTask != null || _stop != null;
  bool get ready =>
      !_disposed &&
      !_cleanupActive &&
      (state == BootState.owned || state == BootState.external);
  bool get ownsProcess => _child != null;
  bool get cleanupConfirmed => _child == null && !_launchPending;
  bool get configurable =>
      !_launching &&
      !_cleanupActive &&
      cleanupConfirmed &&
      state != BootState.cleanupUnknown;
  bool accepts(HealthInfo health) =>
      ready &&
      chatReady(
        health,
        model: model,
        session: _child == null ? null : _session,
      ) &&
      (_child == null || _child!.alive);
  bool get canStart =>
      !_busy &&
      !_cleanupActive &&
      cleanupConfirmed &&
      state == BootState.absent;
  void _set(BootState next, String text) {
    if (_disposed) return;
    state = next;
    detail = text;
    notifyListeners();
  }

  void configure(String path, String tag) {
    if (!configurable) return;
    executable = path;
    model = tag;
    _generation++;
    _set(
      BootState.checking,
      'Configuration changed. Check readiness before starting.',
    );
  }

  Future<void> check() async {
    if (_disposed ||
        _busy ||
        _cleanupActive ||
        state == BootState.cleanupUnknown) {
      return;
    }
    _busy = true;
    final generation = _generation;
    try {
      final result = await runtime.probe();
      if (_disposed || generation != _generation || _cleanupActive) return;
      _apply(result);
    } finally {
      _busy = false;
      if (!_disposed) notifyListeners();
    }
  }

  void _apply(BackendProbe result) {
    if (_disposed || _cleanupActive || state == BootState.cleanupUnknown) {
      return;
    }
    final h = result.health;
    observedModel = h?.model;
    if (_child != null) {
      if (_child!.alive &&
          h != null &&
          chatReady(h, model: model, session: _session)) {
        _set(BootState.owned, 'Connected · owned desktop backend · ${h.model}');
      } else {
        _set(
          BootState.conflict,
          'Owned backend readiness or desktop session could not be verified. Draft retained. Stop before another start.',
        );
      }
    } else if (result.kind == ProbeKind.ready &&
        h != null &&
        chatReady(h, model: model)) {
      _set(
        BootState.external,
        'Connected · externally managed backend · ${h.model}. This app will not stop it.',
      );
    } else {
      _set(
        switch (result.kind) {
          ProbeKind.absent => BootState.absent,
          ProbeKind.unknown => BootState.unknown,
          _ => BootState.conflict,
        },
        switch (result.kind) {
          ProbeKind.absent =>
            'No listener (connection refused). Configure the packaged backend and exact installed model, then explicitly Start.',
          ProbeKind.unknown =>
            'Readiness unknown (timeout or malformed response). Start is blocked; check again.',
          _ =>
            h != null && h.ok && h.service == 'smith-bridge'
                ? h.profile == null &&
                          h.provider == null &&
                          h.model == null &&
                          !h.echo
                      ? 'An existing legacy Smith bridge on 127.0.0.1:4783 uses a different profile, not chat-only. Start and sending are blocked. Stop it outside this app, then Check readiness. No download fixes an occupied port; the existing listener is untouched.'
                      : h.profile != 'chat-only' || h.echo
                      ? 'The existing Smith bridge on 127.0.0.1:4783 uses a different profile or echo mode, not non-echo chat-only. Stop it outside this app, then Check readiness. Start and sending are blocked; downloading a model does not fix the occupied port.'
                      : h.provider != 'ollama'
                      ? 'The existing chat-only Smith bridge on 127.0.0.1:4783 uses a different or unknown provider, not Ollama. Stop it outside this app, then Check readiness. Start and sending are blocked; the existing listener is untouched.'
                      : h.model == null ||
                            (model.isNotEmpty && h.model != model)
                      ? 'The existing chat-only Smith bridge on 127.0.0.1:4783 has a different or unknown model from the selected exact tag. No model is silently substituted. Stop it outside this app, then Check readiness. Start and sending are blocked; downloading does not fix the occupied port.'
                      : 'The existing chat-only Smith bridge on 127.0.0.1:4783 has unavailable or unknown provider/model readiness. Start and sending are blocked. Check readiness after resolving the provider/model availability outside this app; the existing listener is untouched.'
                : 'An incompatible or unidentified listener occupies 127.0.0.1:4783. Start and sending are blocked. Resolve the listener outside this app, then Check readiness. Downloading a model does not fix the occupied port; the existing listener is untouched.',
        },
      );
    }
  }

  Future<void> start() async {
    if (_disposed || !canStart) return;
    _busy = true;
    _launching = true;
    final generation = ++_generation;
    final deadline = clock.now().add(const Duration(seconds: 15));
    final path = executable;
    final tag = model;
    _set(
      BootState.starting,
      'Validating backend and checking port. No provider is started or model downloaded.',
    );
    try {
      if (!RegExp(r'^[A-Za-z0-9][A-Za-z0-9._:/-]*$').hasMatch(tag) ||
          !tag.contains(':') ||
          tag.length > 256) {
        throw const FormatException(
          'Enter an exact installed model tag including its tag suffix.',
        );
      }
      if (_disposed || generation != _generation || _cleanupActive) return;
      final resolveBudget = _remaining(deadline);
      final resolved = await clock.bound(runtime.resolve(path), resolveBudget);
      if (_disposed || generation != _generation || _cleanupActive) return;
      final prelaunchBudget = _remaining(deadline);
      final before = await clock.bound(runtime.probe(), prelaunchBudget);
      if (_disposed || generation != _generation || _cleanupActive) return;
      _remaining(deadline);
      if (before.kind != ProbeKind.absent) {
        _apply(before);
        return;
      }
      final session = runtime.session();
      if (!RegExp(r'^[0-9a-f]{32}$').hasMatch(session)) {
        throw const FormatException('Desktop session generation failed.');
      }
      if (_disposed || generation != _generation || _cleanupActive) return;
      _session = session;
      final launchBudget = _remaining(deadline);
      _launchPending = true;
      var timedOut = false;
      final spawn = runtime
          .launch(resolved, [
            'serve',
            '--chat-only',
            '--port',
            '4783',
            '--model',
            tag,
            '--desktop-session',
            session,
          ])
          .then(
            (child) async {
              _launchPending = false;
              _child = child;
              if (timedOut || _disposed || generation != _generation) {
                await _cleanup(child);
              }
              return child;
            },
            onError: (Object error, StackTrace stack) {
              _launchPending = false;
              Error.throwWithStackTrace(error, stack);
            },
          );
      spawn.ignore();
      late final BackendProcess child;
      try {
        child = await clock.bound(spawn, launchBudget);
      } on TimeoutException {
        timedOut = true;
        rethrow;
      }
      unawaited(
        child.exitCode.then((_) {
          if (!_disposed &&
              identical(_child, child) &&
              !_busy &&
              !_cleanupActive) {
            _set(
              BootState.conflict,
              'Owned backend exited. Stop to confirm cleanup; draft and transcript retained.',
            );
          }
        }),
      );
      if (_disposed || generation != _generation) {
        await _cleanup(child);
        return;
      }
      while (clock.now().isBefore(deadline) &&
          child.alive &&
          generation == _generation &&
          !_disposed) {
        final probeBudget = _remaining(deadline);
        final result = await clock.bound(runtime.probe(), probeBudget);
        if (generation != _generation || _disposed) break;
        if (clock.now().isBefore(deadline) &&
            child.alive &&
            result.health != null &&
            chatReady(result.health!, model: tag, session: session)) {
          _apply(result);
          return;
        }
        if (result.kind == ProbeKind.conflict ||
            result.kind == ProbeKind.ready) {
          break;
        }
        final remaining = _remaining(deadline);
        await clock.delay(
          remaining < const Duration(milliseconds: 200)
              ? remaining
              : const Duration(milliseconds: 200),
        );
      }
      await _cleanup(child);
      if (state != BootState.cleanupUnknown) {
        _set(
          BootState.unknown,
          'Startup did not verify the owned desktop session within 15 seconds. Owned child stopped; check before retrying.',
        );
      }
    } catch (error) {
      if (_disposed || generation != _generation) {
        if (_child != null) await _cleanup(_child!);
        return;
      }
      if (_child != null) await _cleanup(_child!);
      if (_launchPending) {
        _set(
          BootState.cleanupUnknown,
          'Startup exceeded 15 seconds while launch was pending. Cleanup is unknown; another start is blocked. Any late captured child will be stopped.',
        );
      } else if (state != BootState.cleanupUnknown) {
        _set(
          BootState.unknown,
          error is TimeoutException
              ? 'Startup exceeded the 15-second budget. Readiness is unverified; check before retrying. No fallback attempted.'
              : 'Backend validation or launch failed. Check absolute path, safe ownership/modes, native x86_64 ELF and exact installed model tag. No fallback attempted.',
        );
      }
    } finally {
      _busy = false;
      _launching = false;
      if (!_disposed) notifyListeners();
    }
  }

  Duration _remaining(DateTime deadline) {
    final remaining = deadline.difference(clock.now());
    if (remaining <= Duration.zero) {
      throw TimeoutException('Startup budget exhausted.');
    }
    return remaining;
  }

  Future<void> _cleanup(BackendProcess child) {
    if (!identical(_child, child)) return Future.value();
    return _cleanupTask ??= _cleanChild(
      child,
    ).whenComplete(() => _cleanupTask = null);
  }

  Future<void> _cleanChild(BackendProcess child) async {
    ++_generation;
    observedModel = null;
    _set(BootState.stopping, 'Stopping only the captured owned backend.');
    // Do not let a blocked pipe close defeat the bounded termination budget.
    unawaited(Future<void>.sync(child.closeInput).catchError((Object _) {}));
    _signal(child);
    var exited = await clock.exited(child, const Duration(seconds: 5));
    if (!exited) {
      _signal(child, force: true);
      exited = await clock.exited(child, const Duration(seconds: 2));
    }
    if (exited && identical(_child, child)) {
      _child = null;
      _session = null;
    }
    // No observation from before or during cleanup may describe the next
    // listener, even after the captured child has been cleared.
    ++_generation;
    observedModel = null;
    _set(
      exited ? BootState.checking : BootState.cleanupUnknown,
      exited
          ? 'Owned backend stopped. Check readiness before starting again.'
          : 'Owned cleanup unknown. Another start is blocked; external processes are untouched.',
    );
  }

  void _signal(BackendProcess child, {bool force = false}) {
    if (!child.alive) return;
    try {
      child.terminate(force: force);
    } catch (_) {
      // Exit confirmation, not a successful signal call, decides cleanup.
    }
  }

  Future<void> stop() =>
      _stop ??= _stopOwned().whenComplete(() => _stop = null);
  Future<void> _stopOwned() async {
    ++_generation;
    if (_child == null) {
      if (_launchPending) {
        _set(
          BootState.cleanupUnknown,
          'Launch is still pending; cleanup is unknown and another start is blocked. Any late captured child will be stopped.',
        );
      }
      return;
    }
    await _cleanup(_child!);
  }

  @override
  void dispose() {
    _disposed = true;
    ++_generation;
    unawaited(stop());
    super.dispose();
  }
}
