import 'package:flutter/material.dart';

import 'boot_controller.dart';
import 'control_api.dart';
import 'native_control.dart';
import 'app_api.dart';
import 'audio.dart';
import 'voice_chat.dart';

void main() => runApp(const SmithApp());

class SmithApp extends StatelessWidget {
  const SmithApp({super.key, this.controlApi, this.appApi, this.audio});
  final ControlApi? controlApi;
  final AppApi? appApi;
  final AudioIo? audio;

  @override
  Widget build(BuildContext context) => MaterialApp(
    title: 'smith',
    debugShowCheckedModeBanner: false,
    theme: ThemeData(
      useMaterial3: true,
      brightness: Brightness.dark,
      scaffoldBackgroundColor: const Color(0xFF080B09),
      colorScheme: const ColorScheme.dark(
        primary: Color(0xFF75E89B),
        onPrimary: Color(0xFF062010),
        surface: Color(0xFF141A16),
        onSurface: Color(0xFFE3E9E5),
        onSurfaceVariant: Color(0xFFBAC5BE),
        outline: Color(0xFF85968B),
        error: Color(0xFFFFB4AB),
      ),
    ),
    home: BootShell(controlApi: controlApi, appApi: appApi, audio: audio),
  );
}

class BootShell extends StatefulWidget {
  const BootShell({super.key, this.controlApi, this.appApi, this.audio});
  final ControlApi? controlApi;
  final AppApi? appApi;
  final AudioIo? audio;
  @override
  State<BootShell> createState() => _BootShellState();
}

class _BootShellState extends State<BootShell> {
  late final BootController controller;
  ChatController? chat;
  ChatController get chatController {
    final existing = chat;
    if (existing != null) return existing;
    final created = ChatController(
      widget.appApi ?? NativeAppApi(),
      widget.audio ?? NativeAudio(),
    );
    chat = created;
    created.synchronize();
    return created;
  }

  bool viewingSetup = false;
  int stage = 0;

  @override
  void initState() {
    super.initState();
    controller = BootController(widget.controlApi ?? NativeControlApi());
    controller.refresh();
  }

  @override
  void dispose() {
    chat?.dispose();
    controller.dispose();
    super.dispose();
  }

  Future<bool> _confirm(String action, ControlReply reply) async =>
      await showDialog<bool>(
        context: context,
        barrierDismissible: false,
        builder: (context) => Dialog(
          insetPadding: const EdgeInsets.all(16),
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(24),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  serviceActions[action]!,
                  style: Theme.of(context).textTheme.titleLarge,
                ),
                const SizedBox(height: 16),
                Text(reply.effects['description'] as String),
                const SizedBox(height: 12),
                for (final key in [
                  'binary_path',
                  'unit_path',
                  'state_path',
                  'socket_path',
                ])
                  SelectableText('$key: ${reply.effects[key]}'),
                const SizedBox(height: 16),
                const Text(
                  'The service continues after desktop close. '
                  'This does not enable login autostart or lingering. '
                  'Logout/reboot operation is not guaranteed.',
                ),
                const SizedBox(height: 16),
                Wrap(
                  spacing: 12,
                  runSpacing: 8,
                  children: [
                    TextButton(
                      onPressed: () => Navigator.pop(context, false),
                      child: const Text('Cancel'),
                    ),
                    ElevatedButton(
                      onPressed: () => Navigator.pop(context, true),
                      child: const Text('Confirm'),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ),
      ) ??
      false;

  String get coreStatus {
    final state = controller.observation;
    if (controller.error != null) return 'Core needs attention';
    if (state == null) return 'Core status unverified';
    if (state.ready) return 'Core running · setup complete';
    if (controller.page == BootPage.home) return 'Core needs repair';
    return switch (controller.actions.firstOrNull) {
      'install-binary' => 'Core binary not installed',
      'install-unit' => 'Core service not registered',
      'start' => 'Core service stopped',
      'complete-setup' => 'Core running · finish setup',
      _ => 'Core status unverified',
    };
  }

  Widget _action(String action, {bool primary = false}) {
    final callback = controller.busy
        ? null
        : () => controller.act(action, _confirm);
    return primary
        ? ElevatedButton(
            onPressed: callback,
            child: Text(serviceActions[action]!),
          )
        : TextButton(onPressed: callback, child: Text(serviceActions[action]!));
  }

  Widget _core(BuildContext context) {
    final state = controller.observation;
    final actions = controller.actions;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(coreStatus, style: Theme.of(context).textTheme.titleLarge),
        const SizedBox(height: 8),
        if (controller.page == BootPage.home && state?.ready != true)
          Text(
            state?.hasPersistedSetup == true
                ? 'Repair required. Service reports persisted setup complete; '
                      'current readiness is not verified.'
                : 'Repair required. Previously verified setup is remembered '
                      'only for this desktop session; current readiness is not verified.',
          ),
        if (controller.error != null) ...[
          Text('Error: ${controller.error}'),
          if (state != null) Text(state.message),
          const Text('Refresh to inspect current state. No automatic retry.'),
        ],
        const SizedBox(height: 16),
        Wrap(
          spacing: 12,
          runSpacing: 8,
          children: [
            if (actions.isNotEmpty) _action(actions.first, primary: true),
            TextButton(
              onPressed: controller.busy ? null : controller.refresh,
              child: const Text('Refresh'),
            ),
          ],
        ),
        const SizedBox(height: 8),
        ExpansionTile(
          key: const ValueKey('core-details'),
          tilePadding: EdgeInsets.zero,
          title: const Text('Details'),
          expandedCrossAxisAlignment: CrossAxisAlignment.start,
          children: [
            if (state != null) ...[
              Text(
                'Service: smith-next\n'
                'Helper ok: ${state.ok}\n'
                'Binary: ${state.binary}\nUnit: ${state.unit}\n'
                'Runtime: ${state.runtime}\nBridge: ${state.bridge}\n'
                'Setup complete: ${state.setupComplete ?? 'unknown'}',
              ),
              if (controller.error == null) Text(state.message),
            ] else
              const Text('Current service status is unverified.'),
            for (final action in actions.skip(1)) _action(action),
            const SizedBox(height: 12),
            const Text(
              'Each Core action prepares exact effects for review before '
              'confirmation. Finish setup records only Core debug completion, '
              'not provider or chat readiness.',
            ),
            const SizedBox(height: 12),
          ],
        ),
        const SizedBox(height: 16),
        const Text(
          'Closing this desktop does not stop the service. '
          'No login autostart or lingering is enabled by these actions. '
          'Logout/reboot operation is not guaranteed.',
        ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    body: SafeArea(
      child: ListenableBuilder(
        listenable: controller,
        builder: (context, _) {
          final loading = controller.page == BootPage.loader;
          final wizard =
              !loading && (viewingSetup || controller.page == BootPage.setup);
          return SingleChildScrollView(
            padding: const EdgeInsets.all(24),
            child: Center(
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 640),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Semantics(
                      header: true,
                      child: Text(
                        'Smith',
                        style: Theme.of(context).textTheme.headlineLarge,
                      ),
                    ),
                    const SizedBox(height: 16),
                    Semantics(
                      header: true,
                      child: Text(
                        loading
                            ? 'Loading service status…'
                            : wizard
                            ? 'Setup'
                            : 'Core Home',
                        style: Theme.of(context).textTheme.headlineSmall,
                      ),
                    ),
                    const SizedBox(height: 8),
                    if (!loading)
                      const Text(
                        'Core lifecycle and local model configuration are separate. Speech is optional.',
                      ),
                    if (wizard) ...[
                      const SizedBox(height: 16),
                      Wrap(
                        spacing: 8,
                        runSpacing: 8,
                        children: [
                          for (final (index, label) in [
                            '1 Core',
                            '2 LLM',
                            '3 Services',
                          ].indexed)
                            OutlinedButton(
                              onPressed: controller.busy
                                  ? null
                                  : () => setState(() => stage = index),
                              child: Semantics(
                                selected: stage == index,
                                child: Text(label),
                              ),
                            ),
                        ],
                      ),
                      const SizedBox(height: 16),
                      Semantics(
                        header: true,
                        child: Text(
                          [
                            'Step 1 · Core',
                            'Step 2 · LLM',
                            'Step 3 · Services',
                          ][stage],
                          style: Theme.of(context).textTheme.titleLarge,
                        ),
                      ),
                      const SizedBox(height: 16),
                    ],
                    if (!loading)
                      if (!wizard || stage == 0)
                        Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            _core(context),
                            if (!wizard &&
                                controller.observation?.ready == true) ...[
                              const Divider(height: 32),
                              ChatPanel(controller: chatController),
                            ],
                          ],
                        )
                      else
                        ChatPanel(
                          controller: chatController,
                          settings: true,
                          services: stage == 2,
                        ),
                    if (controller.busy) ...[
                      const SizedBox(height: 12),
                      Semantics(
                        liveRegion: true,
                        child: const Text('Working…'),
                      ),
                    ],
                    if (!loading) ...[
                      const SizedBox(height: 16),
                      if (!wizard)
                        OutlinedButton(
                          onPressed: controller.busy
                              ? null
                              : () => setState(() {
                                  viewingSetup = true;
                                  stage = 0;
                                }),
                          child: const Text('View setup'),
                        )
                      else if (controller.page == BootPage.home)
                        OutlinedButton(
                          onPressed: controller.busy
                              ? null
                              : () => setState(() => viewingSetup = false),
                          child: const Text('Return to Core Home'),
                        ),
                    ],
                  ],
                ),
              ),
            ),
          );
        },
      ),
    ),
  );
}
