import 'dart:io';

const sessionOnlyWindowArgument = '--session-only-window';

typedef WindowProcessStarter = Future<void> Function(
  String executable,
  List<String> arguments, {
  required ProcessStartMode mode,
  required bool runInShell,
});

/// Process creation only; the new desktop performs its own ordinary Boot.
Future<void> launchSessionOnlyWindow({
  String? executable,
  WindowProcessStarter? startProcess,
}) => (startProcess ?? _startProcess)(
  executable ?? Platform.resolvedExecutable,
  const [sessionOnlyWindowArgument],
  mode: ProcessStartMode.detached,
  runInShell: false,
);

Future<void> _startProcess(
  String executable,
  List<String> arguments, {
  required ProcessStartMode mode,
  required bool runInShell,
}) async {
  // Do not retain or terminate the detached desktop with the originating one.
  await Process.start(
    executable,
    arguments,
    mode: mode,
    runInShell: runInShell,
  );
}
