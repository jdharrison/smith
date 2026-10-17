import 'package:flutter/material.dart';

class SettingsView extends StatelessWidget {
  const SettingsView({
    super.key,
    required this.connectionStatus,
    required this.modelStatus,
    required this.version,
  });

  final String connectionStatus;
  final String modelStatus;
  final String version;

  @override
  Widget build(BuildContext context) => ListView(
    padding: const EdgeInsets.all(16),
    children: [
      Semantics(
        header: true,
        child: Text(
          'Connection & About',
          style: Theme.of(context).textTheme.titleMedium,
        ),
      ),
      const SizedBox(height: 24),
      const Text('Last observed service connection'),
      SelectableText(connectionStatus),
      const SizedBox(height: 8),
      const Text(
        'Automatic read-only checks run 30 seconds after each completed check, including in Settings and during outages. Status reports last verified app service transport and state, not ongoing login enablement. It is not continuous monitoring and not a guarantee of model readiness or completed work. Retry remains available for manual recovery.',
      ),
      const SizedBox(height: 24),
      const Text('Observed model'),
      SelectableText(modelStatus),
      const SizedBox(height: 24),
      const Text('Version'),
      SelectableText(version),
      const SizedBox(height: 24),
      const Text(
        'Conversation history and model context are bounded; this is not an unlimited archive.',
      ),
      const SizedBox(height: 16),
      const Text(
        'Closing this window detaches the client. It does not stop the Smith service or already-authorized work. Service Stop is a separate action.',
      ),
      const SizedBox(height: 16),
      const Text(
        'Model setup and service maintenance belong in the separate installer, not this view.',
      ),
    ],
  );
}
