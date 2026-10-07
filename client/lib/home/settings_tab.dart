import 'dart:io';

import 'package:flutter/material.dart';

import 'package:smith_client/bridge/smith_bridge.dart';
import 'package:smith_client/chat/chat_controller.dart';
import 'package:smith_client/home/halo_theme.dart';
import 'package:smith_client/listen/voice_turn.dart';

/// Settings tab — TTS, services policy, bridge status.
class SettingsTab extends StatefulWidget {
  const SettingsTab({super.key, required this.chat, required this.voice});

  final ChatController chat;
  final VoiceTurnController voice;

  @override
  State<SettingsTab> createState() => _SettingsTabState();
}

class _SettingsTabState extends State<SettingsTab> {
  static const _ink = HaloColors.primary;
  static const _muted = HaloColors.secondary;

  final SmithBridge _bridge = SmithBridge();
  ServicesSnapshot? _services;
  String? _servicesError;
  bool _servicesBusy = false;

  ChatController get chat => widget.chat;
  VoiceTurnController get voice => widget.voice;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _refreshServices());
  }

  @override
  void dispose() {
    _bridge.close();
    super.dispose();
  }

  Future<void> _refreshServices() async {
    if (_servicesBusy) return;
    setState(() {
      _servicesBusy = true;
      _servicesError = null;
    });
    try {
      final snap = await _bridge.listServices();
      if (!mounted) return;
      setState(() => _services = snap);
    } catch (e) {
      if (!mounted) return;
      setState(() => _servicesError = e.toString());
    } finally {
      if (mounted) setState(() => _servicesBusy = false);
    }
  }

  Future<void> _setAutostart(bool on) async {
    setState(() => _servicesBusy = true);
    try {
      final snap = await _bridge.patchServices(autostart: on);
      if (!mounted) return;
      setState(() => _services = snap);
    } catch (e) {
      if (!mounted) return;
      setState(() => _servicesError = e.toString());
    } finally {
      if (mounted) setState(() => _servicesBusy = false);
    }
  }

  Future<void> _toggleMode(ServiceRow row) async {
    final next = row.isWarm ? 'on_demand' : 'warm';
    setState(() => _servicesBusy = true);
    try {
      var snap = await _bridge.patchServices(id: row.id, mode: next);
      if (next == 'warm') {
        await _bridge.applyServices();
        if (row.id == 'services.whisper' || row.id == 'services.tts') {
          await _bridge.ensureSidecars();
        }
        snap = await _bridge.listServices();
      }
      if (!mounted) return;
      setState(() => _services = snap);
    } catch (e) {
      if (!mounted) return;
      setState(() => _servicesError = e.toString());
    } finally {
      if (mounted) setState(() => _servicesBusy = false);
    }
  }

  Future<void> _openSearx() async {
    const url = 'http://127.0.0.1:4792';
    try {
      if (Platform.isWindows) {
        await Process.run('cmd', ['/c', 'start', '', url]);
      } else if (Platform.isMacOS) {
        await Process.run('open', [url]);
      } else {
        await Process.run('xdg-open', [url]);
      }
    } catch (_) {
      // URL shown in subtitle; open is best-effort.
    }
  }

  @override
  Widget build(BuildContext context) {
    final online = chat.bridgeStatus == BridgeStatus.online;

    return ListView(
      padding: const EdgeInsets.fromLTRB(20, 8, 20, 32),
      children: [
        _section('Services'),
        const SizedBox(height: 8),
        if (_servicesError != null)
          Text(
            _servicesError!,
            style: TextStyle(fontSize: 12, color: HaloColors.danger),
          ),
        if (_services != null) ...[
          SupervisorStatusSummary(
            snapshot: _services!,
            busy: _servicesBusy,
            onAutostartChanged: _setAutostart,
          ),
          ..._groupedServiceTiles(_services!.services),
        ] else if (_servicesBusy)
          const Padding(
            padding: EdgeInsets.symmetric(vertical: 12),
            child: Center(
              child: SizedBox(
                width: 20,
                height: 20,
                child: CircularProgressIndicator(strokeWidth: 2),
              ),
            ),
          ),
        Align(
          alignment: Alignment.centerLeft,
          child: TextButton(
            onPressed: _servicesBusy ? null : _refreshServices,
            child: const Text('Refresh services'),
          ),
        ),
        const SizedBox(height: 16),
        _section('Speech'),
        const SizedBox(height: 8),
        _row('backend', voice.ttsBackend),
        _row(
          'kokoro',
          voice.ttsKokoroOnline ? 'online :4791' : 'offline (Kokoro required)',
        ),
        _row('voice', voice.ttsVoice ?? '—'),
        if (voice.ttsVoices.isNotEmpty) ...[
          const SizedBox(height: 4),
          Text('Voice', style: TextStyle(fontSize: 12, color: _muted)),
          DropdownButton<String>(
            isExpanded: true,
            value: voice.ttsVoices.contains(voice.ttsVoice)
                ? voice.ttsVoice
                : voice.ttsVoices.first,
            items: voice.ttsVoices
                .map(
                  (v) => DropdownMenuItem(
                    value: v,
                    child: Text(
                      v,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(fontSize: 13, color: _ink),
                    ),
                  ),
                )
                .toList(),
            onChanged: (v) {
              if (v != null) voice.setTtsVoice(v);
            },
          ),
        ],
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          dense: true,
          title: Text(
            'Speak replies',
            style: TextStyle(fontSize: 13, color: _ink),
          ),
          value: voice.ttsEnabled,
          onChanged: (v) => voice.setTtsEnabled(v),
        ),
        Text(
          'Rate  ${(voice.ttsRate * 100).round()}%',
          style: TextStyle(fontSize: 12, color: _muted),
        ),
        Slider(
          value: voice.ttsRate,
          min: 0.0,
          max: 1.0,
          divisions: 20,
          label: '${(voice.ttsRate * 100).round()}%',
          onChanged: (v) => voice.setTtsRate(v),
        ),
        Row(
          children: [
            Expanded(
              child: OutlinedButton(
                onPressed: () => voice.testTts(),
                child: const Text('Test'),
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: OutlinedButton(
                onPressed: () => voice.stopTts(),
                child: const Text('Stop'),
              ),
            ),
          ],
        ),
        const SizedBox(height: 24),
        _section('Whisper'),
        const SizedBox(height: 8),
        _row('status', voice.whisperOnline ? 'online' : 'offline'),
        _row('model', voice.whisperModel ?? '—'),
        Align(
          alignment: Alignment.centerLeft,
          child: TextButton(
            onPressed: () => voice.refreshWhisperHealth(),
            child: const Text('Refresh Whisper'),
          ),
        ),
        const SizedBox(height: 24),
        _section('Bridge'),
        const SizedBox(height: 8),
        _row('status', online ? 'online' : 'offline'),
        _row('echo', chat.bridgeEcho ? 'yes' : 'no'),
        _row('isolation', chat.isolation),
        Align(
          alignment: Alignment.centerLeft,
          child: TextButton(
            onPressed: () => chat.refreshHealth(),
            child: const Text('Refresh bridge'),
          ),
        ),
        const SizedBox(height: 24),
        _section('Projects / agents / pipelines'),
        const SizedBox(height: 8),
        Text(
          'Configure via CLI for now (`smith settings project`, '
          '`smith settings agent`, `smith run`).',
          style: TextStyle(fontSize: 13, height: 1.4, color: _muted),
        ),
      ],
    );
  }

  List<Widget> _groupedServiceTiles(List<ServiceRow> rows) {
    const order = ['bridge', 'brain', 'sandbox', 'agents', 'services'];
    final out = <Widget>[];
    for (final g in order) {
      final subset = rows.where((r) => r.group == g).toList();
      if (subset.isEmpty) continue;
      final title = switch (g) {
        'bridge' => 'Bridge',
        'brain' => 'Brain',
        'sandbox' => 'Sandbox',
        'agents' => 'Agents',
        'services' => 'Services',
        _ => g,
      };
      out.add(
        Padding(
          padding: const EdgeInsets.only(top: 10, bottom: 4),
          child: Text(
            title,
            style: TextStyle(
              fontSize: 12,
              fontWeight: FontWeight.w600,
              color: _ink,
            ),
          ),
        ),
      );
      for (final row in subset) {
        out.add(
          ListTile(
            contentPadding: EdgeInsets.zero,
            dense: true,
            title: Text(row.id, style: TextStyle(fontSize: 13, color: _ink)),
            subtitle: Text(
              '${row.mode} · ${row.running ? "running" : "stopped"}'
              '${row.url != null ? " · ${row.url}" : ""}',
              style: TextStyle(fontSize: 11, color: _muted),
            ),
            trailing: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                if (row.id == 'services.searxng')
                  IconButton(
                    tooltip: 'Open SearXNG',
                    icon: const Icon(Icons.open_in_new, size: 18),
                    onPressed: _openSearx,
                  ),
                Switch(
                  value: row.isWarm,
                  onChanged: _servicesBusy ? null : (_) => _toggleMode(row),
                ),
              ],
            ),
          ),
        );
      }
    }
    return out;
  }

  Widget _section(String title) {
    return Text(
      title,
      style: TextStyle(
        fontSize: 11,
        fontWeight: FontWeight.w700,
        letterSpacing: 0.7,
        color: _muted,
      ),
    );
  }

  Widget _row(String label, String value) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 4),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 72,
            child: Text(label, style: TextStyle(fontSize: 12, color: _muted)),
          ),
          Expanded(
            child: Text(value, style: TextStyle(fontSize: 12, color: _ink)),
          ),
        ],
      ),
    );
  }
}

/// Configured supervisor preference and independent observed service state.
class SupervisorStatusSummary extends StatelessWidget {
  const SupervisorStatusSummary({
    super.key,
    required this.snapshot,
    required this.busy,
    required this.onAutostartChanged,
  });

  final ServicesSnapshot snapshot;
  final bool busy;
  final ValueChanged<bool> onAutostartChanged;

  @override
  Widget build(BuildContext context) {
    final supervisor = snapshot.supervisor;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          dense: true,
          title: Text(
            'Configured autostart preference',
            style: TextStyle(fontSize: 13, color: HaloColors.primary),
          ),
          subtitle: Text(
            'Configured preference: ${snapshot.autostart ? "on" : "off"}. '
            'Observed service state may differ.',
            style: TextStyle(fontSize: 11, color: HaloColors.secondary),
          ),
          value: snapshot.autostart,
          onChanged: busy ? null : onAutostartChanged,
        ),
        _SupervisorStatusRow(
          label: 'unit registration',
          value: supervisor.registered ? 'yes' : 'no',
        ),
        _SupervisorStatusRow(
          label: 'unit load state',
          value: supervisor.loadState,
        ),
        _SupervisorStatusRow(
          label: 'runtime active state',
          value: supervisor.activeState,
        ),
        _SupervisorStatusRow(
          label: 'runtime substate',
          value: supervisor.subState,
        ),
        _SupervisorStatusRow(
          label: 'enabled at login',
          value: _yesNoUnknown(supervisor.enabledAtLogin),
        ),
        _SupervisorStatusRow(
          label: 'unit enablement state',
          value: supervisor.enabledState,
        ),
        _SupervisorStatusRow(
          label: 'linger',
          value: _yesNoUnknown(supervisor.linger),
        ),
        _SupervisorStatusRow(
          label: 'bridge identity',
          value: supervisor.bridgeIdentity,
        ),
        _SupervisorStatusRow(
          label: 'bridge health',
          value: supervisor.bridgeHealthy ? 'yes' : 'no',
        ),
        if (supervisor.detail != null)
          _SupervisorStatusRow(
            label: 'observation detail',
            value: supervisor.detail!,
          ),
      ],
    );
  }

  static String _yesNoUnknown(bool? value) {
    return switch (value) {
      true => 'yes',
      false => 'no',
      null => 'unknown',
    };
  }
}

class _SupervisorStatusRow extends StatelessWidget {
  const _SupervisorStatusRow({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 4),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 132,
            child: Text(
              label,
              style: TextStyle(fontSize: 12, color: HaloColors.secondary),
            ),
          ),
          Expanded(
            child: Text(
              value,
              style: TextStyle(fontSize: 12, color: HaloColors.primary),
            ),
          ),
        ],
      ),
    );
  }
}
