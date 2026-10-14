import 'package:flutter/material.dart';

import 'app_api.dart';
import 'boot.dart';
import 'chat_panel.dart';
import 'control_api.dart';
import 'native_control.dart';
import 'text_chat.dart';

void main() => runApp(const SmithApp());

class SmithApp extends StatelessWidget {
  const SmithApp({
    super.key,
    this.controlApi,
    this.appApi,
    this.bootClock,
    this.bootObservationBudget = defaultBootObservationBudget,
    this.bootPollInterval = defaultBootPollInterval,
  });

  final ControlApi? controlApi;
  final AppApi? appApi;
  final BootClock? bootClock;
  final Duration bootObservationBudget;
  final Duration bootPollInterval;

  @override
  Widget build(BuildContext context) => MaterialApp(
    title: 'smith',
    debugShowCheckedModeBanner: false,
    theme: ThemeData(
      useMaterial3: true,
      brightness: Brightness.dark,
      scaffoldBackgroundColor: const Color(0xFF212121),
      colorScheme: const ColorScheme.dark(
        primary: Color(0xFFECECEC),
        onPrimary: Color(0xFF212121),
        surface: Color(0xFF2F2F2F),
        onSurface: Color(0xFFE3E9E5),
        onSurfaceVariant: Color(0xFFBAC5BE),
        outline: Color(0xFF85968B),
        error: Color(0xFFFFB4AB),
      ),
    ),
    home: SmithShell(
      controlApi: controlApi,
      appApi: appApi,
      bootClock: bootClock,
      bootObservationBudget: bootObservationBudget,
      bootPollInterval: bootPollInterval,
    ),
  );
}

class SmithShell extends StatefulWidget {
  const SmithShell({
    super.key,
    this.controlApi,
    this.appApi,
    this.bootClock,
    this.bootObservationBudget = defaultBootObservationBudget,
    this.bootPollInterval = defaultBootPollInterval,
  });

  final ControlApi? controlApi;
  final AppApi? appApi;
  final BootClock? bootClock;
  final Duration bootObservationBudget;
  final Duration bootPollInterval;

  @override
  State<SmithShell> createState() => _SmithShellState();
}

class _SmithShellState extends State<SmithShell> {
  late final ControlApi control;
  late final ChatController chat;
  late final BootCoordinator boot;

  @override
  void initState() {
    super.initState();
    control = widget.controlApi ?? NativeControlApi();
    chat = ChatController(widget.appApi ?? NativeAppApi())..connected = false;
    boot = BootCoordinator(
      control: control,
      chat: chat,
      clock: widget.bootClock,
      observationBudget: widget.bootObservationBudget,
      pollInterval: widget.bootPollInterval,
    );
    boot.launch();
  }

  @override
  void dispose() {
    boot.dispose();
    chat.dispose();
    control.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: boot,
    builder: (context, _) => boot.state.phase == BootPhase.ready
        ? ConversationScreen(control: control, chat: chat)
        : BootSurface(coordinator: boot),
  );
}

class ConversationScreen extends StatefulWidget {
  const ConversationScreen({
    super.key,
    required this.control,
    required this.chat,
  });

  final ControlApi control;
  final ChatController chat;

  @override
  State<ConversationScreen> createState() => _ConversationScreenState();
}

class _ConversationScreenState extends State<ConversationScreen> {
  bool inspecting = false;
  bool closed = false;
  String status = 'Connected to Smith';

  ChatController get chat => widget.chat;

  Future<void> retry() async {
    if (inspecting || chat.busy || closed) return;
    setState(() => inspecting = true);
    chat.unavailable();
    try {
      final facts = await widget.control.inspect();
      if (closed) return;
      if (!facts.chatReady) {
        status =
            facts.error ??
            (facts.binary != 'installed'
                ? 'Smith binary is ${facts.binary}. Use the installer.'
                : facts.unit != 'installed'
                ? 'Smith service registration is ${facts.unit}. Use the installer.'
                : facts.enablement != EnablementState.enabled
                ? 'Smith login enablement is ${facts.enablement.name}. Use the installer.'
                : facts.runtime != 'running'
                ? 'Smith service is ${facts.runtime}.'
                : 'Smith bridge is ${facts.bridge}; connection identity is not verified. ${facts.message}');
      } else {
        chat.connected = true;
        await chat.synchronize();
        if (closed) return;
        // One inventory observation per explicit Retry, never an inference probe
        // or automatic retry. Uncertain work is resolved by state/poll first.
        if (!chat.stale && chat.pendingId == null) await chat.refreshModels();
        if (closed) return;
        status = 'Connected to Smith';
      }
    } on Object catch (error) {
      if (!closed) status = 'Smith connection unavailable: $error';
    } finally {
      if (!closed) setState(() => inspecting = false);
    }
  }

  String get connectionStatus {
    if (!chat.connected) return status;
    if (inspecting) return 'Checking Smith connection…';
    if (chat.stale) return 'Smith application unavailable. Retry queries only.';
    if (!chat.inventoryObserved) {
      return 'Installed-model inventory unavailable.';
    }
    if (chat.providers['llm']?['ready'] != true) {
      return 'Local model runtime unavailable. Use the installer for prerequisites.';
    }
    if (chat.config['model'] == null) {
      return 'No local model selected. Use the separate smith-installer, then Retry.';
    }
    if (!chat.models.contains(chat.config['model'])) {
      return 'Selected model is not installed. Use the installer.';
    }
    return 'Connected to Smith';
  }

  @override
  void dispose() {
    closed = true;
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    body: SafeArea(
      child: Row(
        children: [
          const _ModeSidebar(),
          const VerticalDivider(width: 1, thickness: 1),
          Expanded(
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 12),
              child: Center(
                child: ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 760),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      Text(
                        'Smith',
                        style: Theme.of(context).textTheme.titleLarge,
                      ),
                      const SizedBox(height: 8),
                      ListenableBuilder(
                        listenable: chat,
                        builder: (context, _) => Row(
                          children: [
                            Expanded(
                              child: Text(
                                connectionStatus,
                                maxLines: 2,
                                overflow: TextOverflow.ellipsis,
                                style: Theme.of(context).textTheme.bodySmall,
                              ),
                            ),
                            TextButton(
                              onPressed: inspecting || chat.busy ? null : retry,
                              child: const Text('Retry'),
                            ),
                          ],
                        ),
                      ),
                      const SizedBox(height: 8),
                      Expanded(child: ChatPanel(controller: chat)),
                      const SizedBox(height: 8),
                      Text(
                        'Bounded history. Work continues after close.',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        textAlign: TextAlign.center,
                        style: Theme.of(context).textTheme.bodySmall,
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ],
      ),
    ),
  );
}

class _ModeSidebar extends StatelessWidget {
  const _ModeSidebar();

  @override
  Widget build(BuildContext context) => Container(
    key: const ValueKey('mode-sidebar'),
    width: 56,
    color: const Color(0xFF171717),
    padding: const EdgeInsets.only(top: 12),
    alignment: Alignment.topCenter,
    child: Tooltip(
      message: 'Chat',
      child: Semantics(
        key: const ValueKey('mode-chat-selected'),
        container: true,
        selected: true,
        label: 'Chat',
        child: Container(
          width: 44,
          height: 44,
          decoration: BoxDecoration(
            color: Theme.of(context).colorScheme.surface,
            borderRadius: BorderRadius.circular(12),
          ),
          child: const Icon(Icons.chat_bubble, size: 22),
        ),
      ),
    ),
  );
}
