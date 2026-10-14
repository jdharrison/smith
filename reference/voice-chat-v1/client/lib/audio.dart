import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:typed_data';

void _privateMode(String path, int mode) {
  final libc = DynamicLibrary.open('libc.so.6');
  final malloc = libc
      .lookupFunction<
        Pointer<Void> Function(IntPtr),
        Pointer<Void> Function(int)
      >('malloc');
  final free = libc
      .lookupFunction<
        Void Function(Pointer<Void>),
        void Function(Pointer<Void>)
      >('free');
  final open = libc
      .lookupFunction<
        Int32 Function(Pointer<Uint8>, Int32),
        int Function(Pointer<Uint8>, int)
      >('open');
  final chmod = libc
      .lookupFunction<Int32 Function(Int32, Uint32), int Function(int, int)>(
        'fchmod',
      );
  final close = libc.lookupFunction<Int32 Function(Int32), int Function(int)>(
    'close',
  );
  final bytes = utf8.encode(path);
  final pointer = malloc(bytes.length + 1).cast<Uint8>();
  if (pointer == nullptr) {
    throw StateError('Audio permission allocation failed.');
  }
  var fd = -1;
  try {
    pointer.asTypedList(bytes.length + 1).setAll(0, [...bytes, 0]);
    // Linux O_NOFOLLOW | O_CLOEXEC: chmod only the opened captured object.
    fd = open(pointer, 0x20000 | 0x80000);
    if (fd < 0 || chmod(fd, mode) != 0) {
      throw StateError('Private audio permissions unavailable.');
    }
  } finally {
    if (fd >= 0) close(fd);
    free(pointer.cast<Void>());
  }
}

abstract class AudioIo {
  Future<void> startRecording();
  Future<Map<String, dynamic>> stopRecording();
  Future<void> play(Map<String, dynamic> audio);
  Future<void> stop();
  void dispose();
}

Uint8List wavPcm(List<int> bytes, int rate) {
  final result = Uint8List(44 + bytes.length);
  final b = ByteData.sublistView(result);
  void text(int at, String s) =>
      result.setRange(at, at + s.length, ascii.encode(s));
  text(0, 'RIFF');
  b.setUint32(4, result.length - 8, Endian.little);
  text(8, 'WAVE');
  text(12, 'fmt ');
  b.setUint32(16, 16, Endian.little);
  b.setUint16(20, 1, Endian.little);
  b.setUint16(22, 1, Endian.little);
  b.setUint32(24, rate, Endian.little);
  b.setUint32(28, rate * 2, Endian.little);
  b.setUint16(32, 2, Endian.little);
  b.setUint16(34, 16, Endian.little);
  text(36, 'data');
  b.setUint32(40, bytes.length, Endian.little);
  result.setRange(44, result.length, bytes);
  return result;
}

Uint8List validateWav(Map<String, dynamic> audio, int rate) {
  if (audio['mime'] != 'audio/wav' || audio['data_base64'] is! String) {
    throw const FormatException('Invalid WAV audio.');
  }
  final encoded = audio['data_base64'] as String;
  if (encoded.length > 4 * 1024 * 1024) {
    throw const FormatException('Audio too large.');
  }
  final bytes = base64Decode(encoded);
  if (bytes.length < 44 || bytes.length > rate * 120 + 44) {
    throw const FormatException('Audio size exceeds 60 seconds.');
  }
  final b = ByteData.sublistView(bytes);
  String text(int a, int n) =>
      ascii.decode(bytes.sublist(a, a + n), allowInvalid: true);
  if (text(0, 4) != 'RIFF' ||
      text(8, 4) != 'WAVE' ||
      b.getUint32(4, Endian.little) != bytes.length - 8) {
    throw const FormatException('Invalid WAV container.');
  }
  var format = false;
  Uint8List? pcm;
  for (var at = 12; at + 8 <= bytes.length;) {
    final size = b.getUint32(at + 4, Endian.little);
    if (at + 8 + size > bytes.length) {
      throw const FormatException('Invalid WAV chunk.');
    }
    if (text(at, 4) == 'fmt ') {
      if (size < 16 ||
          b.getUint16(at + 8, Endian.little) != 1 ||
          b.getUint16(at + 10, Endian.little) != 1 ||
          b.getUint32(at + 12, Endian.little) != rate ||
          b.getUint32(at + 16, Endian.little) != rate * 2 ||
          b.getUint16(at + 20, Endian.little) != 2 ||
          b.getUint16(at + 22, Endian.little) != 16) {
        throw const FormatException('Expected mono PCM16 WAV.');
      }
      format = true;
    }
    if (text(at, 4) == 'data') {
      if (pcm != null || size == 0 || size.isOdd || size > rate * 120) {
        throw const FormatException('Invalid PCM data.');
      }
      pcm = Uint8List.sublistView(bytes, at + 8, at + 8 + size);
    }
    at += 8 + size + (size % 2);
  }
  if (!format || pcm == null) throw const FormatException('Missing WAV audio.');
  return pcm;
}

/// Private transient WAV files are removed after use. Providers receive bytes,
/// never paths; native audio executables receive PCM through anonymous pipes.
class NativeAudio implements AudioIo {
  NativeAudio() : observeTemporaryFile = null;
  NativeAudio.forTesting({
    required Future<Process> Function(String, List<String>) launch,
    this.observeTemporaryFile,
  }) : _testLaunch = launch;
  Future<Process> Function(String, List<String>)? _testLaunch;
  final Future<void> Function(File)? observeTemporaryFile;
  Process? _child;
  bool _exited = false;
  bool _disposed = false;
  bool _starting = false;
  int _epoch = 0;
  final _pcm = <int>[];
  StreamSubscription<List<int>>? _out;
  StreamSubscription<List<int>>? _err;
  Timer? _timer;
  String? _failure;
  Future<void>? _cleanup;

  Future<T> _withWav<T>(List<int> bytes, Future<T> Function(File) use) async {
    final root = await Directory.systemTemp.resolveSymbolicLinks();
    final dir = await Directory(root).createTemp('smith-audio-');
    final file = File('${dir.path}/audio.wav');
    try {
      _privateMode(dir.path, 0x1c0);
      if ((await dir.stat()).mode & 0x3f != 0 ||
          await dir.resolveSymbolicLinks() != dir.path) {
        throw StateError('Private audio directory unavailable.');
      }
      await file.create();
      _privateMode(file.path, 0x180);
      await file.writeAsBytes(bytes, flush: true);
      await observeTemporaryFile?.call(file);
      return await use(file);
    } finally {
      // Delete only the captured regular file and then the empty directory.
      // Unknown objects, symlinks or extra contents are preserved, not traversed.
      if (await FileSystemEntity.type(dir.path, followLinks: false) ==
              FileSystemEntityType.directory &&
          await dir.resolveSymbolicLinks() == dir.path) {
        final type = await FileSystemEntity.type(file.path, followLinks: false);
        if (type == FileSystemEntityType.file) await file.delete();
        if (type == FileSystemEntityType.file ||
            type == FileSystemEntityType.notFound) {
          await dir.delete();
        }
      }
    }
  }

  Future<Process> _launch(String path, List<String> args) =>
      _launchCaptured(path, args).timeout(
        const Duration(seconds: 2),
        onTimeout: () {
          _epoch++;
          throw StateError(
            'Audio launch deadline exceeded; late children will be cleaned.',
          );
        },
      );

  Future<Process> _launchCaptured(String path, List<String> args) async {
    if (_disposed || _child != null || _starting) {
      throw StateError('Audio busy or unavailable.');
    }
    _starting = true;
    final epoch = _epoch;
    try {
      final p =
          await (_testLaunch?.call(path, args) ??
              Process.start(path, args, runInShell: false));
      _child = p;
      _exited = false;
      unawaited(
        p.exitCode.then((_) {
          if (identical(_child, p)) _exited = true;
        }, onError: (Object _) {}),
      );
      if (_disposed || epoch != _epoch) {
        final drains = [p.stdout.listen((_) {}), p.stderr.listen((_) {})];
        try {
          await stop();
        } finally {
          for (final sub in drains) {
            await sub.cancel();
          }
        }
        throw StateError('Audio closed.');
      }
      return p;
    } finally {
      _starting = false;
    }
  }

  @override
  Future<void> startRecording() async {
    _pcm.clear();
    _failure = null;
    final p = await _launch('/usr/bin/parecord', [
      '--raw',
      '--format=s16le',
      '--rate=16000',
      '--channels=1',
    ]);
    await p.stdin.close();
    _err = p.stderr.listen((_) {});
    _out = p.stdout.listen(
      (bytes) {
        if (_disposed) return;
        if (_pcm.length + bytes.length > 16000 * 120) {
          _failure = 'Recording reached 60-second limit.';
          unawaited(
            stop().catchError((Object _) {
              _failure = 'Audio cleanup unconfirmed.';
            }),
          );
        } else {
          _pcm.addAll(bytes);
        }
      },
      onError: (Object _) {
        _failure = 'Recording unavailable.';
      },
    );
    _timer = Timer(
      const Duration(seconds: 60),
      () => unawaited(
        stop().catchError((Object _) {
          _failure = 'Audio cleanup unconfirmed.';
        }),
      ),
    );
  }

  @override
  Future<Map<String, dynamic>> stopRecording() async {
    await stop();
    if (_failure != null || _pcm.isEmpty || _pcm.length.isOdd) {
      throw StateError(_failure ?? 'No microphone audio captured.');
    }
    final wav = wavPcm(_pcm, 16000);
    _pcm.clear();
    return _withWav(
      wav,
      (file) async => {
        'mime': 'audio/wav',
        'data_base64': base64Encode(await file.readAsBytes()),
      },
    );
  }

  @override
  Future<void> play(Map<String, dynamic> audio) async {
    final pcm = validateWav(audio, 24000);
    final epoch = _epoch;
    await _withWav(wavPcm(pcm, 24000), (file) async {
      if (_disposed || epoch != _epoch) throw StateError('Playback cancelled.');
      final p = await _launch('/usr/bin/paplay', [
        '--raw',
        '--format=s16le',
        '--rate=24000',
        '--channels=1',
      ]);
      _out = p.stdout.listen((_) {});
      _err = p.stderr.listen((_) {});
      try {
        await (() async {
          final stdin = p.stdin;
          await stdin.addStream(file.openRead(44));
          await stdin.close();
          final code = await p.exitCode;
          if (code != 0) throw StateError('Playback unavailable.');
        })().timeout(const Duration(seconds: 65));
      } finally {
        await stop();
      }
    });
  }

  @override
  Future<void> stop() {
    _epoch++;
    if (_cleanup != null) return _cleanup!;
    return _cleanup = _stop().whenComplete(() => _cleanup = null);
  }

  Future<void> _stop() async {
    _timer?.cancel();
    _timer = null;
    final p = _child;
    if (p != null) {
      if (!_exited) p.kill(ProcessSignal.sigterm);
      try {
        await p.exitCode.timeout(const Duration(seconds: 1));
      } on TimeoutException {
        if (!_exited) p.kill(ProcessSignal.sigkill);
        await p.exitCode.timeout(const Duration(seconds: 1));
      }
      if (identical(_child, p)) _child = null;
    }
    await _out?.cancel();
    await _err?.cancel();
    _out = null;
    _err = null;
  }

  @override
  void dispose() {
    _disposed = true;
    _pcm.clear();
    unawaited(stop().catchError((Object _) {}));
  }
}
