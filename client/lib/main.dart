import 'package:flutter/material.dart';
import 'package:smith_client/chat/chat_controller.dart';
import 'package:smith_client/home/halo_theme.dart';
import 'dart:async';
import 'dart:ui' show AppExitResponse;
import 'package:smith_client/bridge/desktop_backend.dart';
import 'package:smith_client/bridge/desktop_setup.dart';
import 'package:smith_client/home/desktop_boot_shell.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  runApp(const SmithClientApp());
}

class SmithClientApp extends StatefulWidget {
  const SmithClientApp({super.key});

  @override
  State<SmithClientApp> createState() => _SmithClientAppState();
}

class _SmithClientAppState extends State<SmithClientApp> {
  late final DesktopBackendController _backend = DesktopBackendController();
  late final ChatController _chat = ChatController(
    chatOnly: true,
    readinessGuard: _backend.accepts,
  );
  final DesktopSetup _setup = DesktopSetup();
  late final AppLifecycleListener _lifecycle;

  @override
  void initState() {
    super.initState();
    _lifecycle = AppLifecycleListener(
      onExitRequested: () async {
        await _setup.cancel();
        await _backend.stop();
        return _backend.cleanupConfirmed && !_setup.busy
            ? AppExitResponse.exit
            : AppExitResponse.cancel;
      },
    );
  }

  @override
  void dispose() {
    _lifecycle.dispose();
    unawaited(_setup.cancel());
    unawaited(_backend.stop());
    _backend.dispose();
    _chat.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Halo',
      debugShowCheckedModeBanner: false,
      theme: buildHaloTheme(),
      home: DesktopBootShell(backend: _backend, chat: _chat, setup: _setup),
    );
  }
}
