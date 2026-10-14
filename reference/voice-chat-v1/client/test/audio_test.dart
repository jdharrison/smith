import 'dart:async';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/audio.dart';

import 'app_fakes.dart';
import 'native_control_test.dart' show FakeProcess, flush;

void main() {
  test(
    'fixed native recording executable and bounded WAV without real mic',
    () async {
      final child = FakeProcess();
      File? captured;
      String? executable;
      List<String>? arguments;
      final audio = NativeAudio.forTesting(
        observeTemporaryFile: (file) async {
          captured = file;
          expect((await file.stat()).mode & 0x1ff, 0x180);
          expect((await file.parent.stat()).mode & 0x1ff, 0x1c0);
          expect((await file.length()), lessThanOrEqualTo(2 * 1024 * 1024));
        },
        launch: (path, args) async {
          executable = path;
          arguments = args;
          return child;
        },
      );
      await audio.startRecording();
      expect(executable, '/usr/bin/parecord');
      expect(arguments, [
        '--raw',
        '--format=s16le',
        '--rate=16000',
        '--channels=1',
      ]);
      child.out.add([0, 0, 1, 0]);
      await flush();
      final wav = await audio.stopRecording();
      expect(validateWav(wav, 16000), [0, 0, 1, 0]);
      expect(child.signals, [ProcessSignal.sigterm]);
      expect(await captured!.exists(), false);
      expect(await captured!.parent.exists(), false);
      audio.dispose();
    },
  );
  test(
    'fixed native playback uses stdin, disposal only kills captured child',
    () async {
      final child = FakeProcess();
      final launched = Completer<void>();
      String? executable;
      List<String>? arguments;
      final audio = NativeAudio.forTesting(
        launch: (path, args) async {
          executable = path;
          arguments = args;
          launched.complete();
          return child;
        },
      );
      final work = audio.play(testAudio(24000));
      final check = expectLater(work, throwsStateError);
      await launched.future.timeout(const Duration(seconds: 2));
      await flush();
      expect(executable, '/usr/bin/paplay');
      expect(arguments, [
        '--raw',
        '--format=s16le',
        '--rate=24000',
        '--channels=1',
      ]);
      audio.dispose();
      await check;
      expect(child.signals, contains(ProcessSignal.sigterm));
    },
  );
  test('late audio launch after stop is cleaned rather than started', () async {
    final child = FakeProcess();
    final gate = Completer<Process>();
    final audio = NativeAudio.forTesting(launch: (_, _) => gate.future);
    final work = audio.startRecording();
    final check = expectLater(work, throwsStateError);
    await flush();
    await audio.stop();
    gate.complete(child);
    await check;
    expect(child.signals, [ProcessSignal.sigterm]);
    audio.dispose();
  });
  test('oversized recording fails within memory bound', () async {
    final child = FakeProcess();
    final audio = NativeAudio.forTesting(launch: (_, _) async => child);
    await audio.startRecording();
    child.out.add(List.filled(16000 * 120 + 1, 0));
    await flush();
    await expectLater(audio.stopRecording(), throwsStateError);
    expect(child.signals, contains(ProcessSignal.sigterm));
    audio.dispose();
  });
}
