import 'dart:async';
import 'package:flutter/material.dart';
import '../bridge/desktop_backend.dart';
import '../bridge/desktop_setup.dart';
import '../chat/chat_controller.dart';
import 'halo_presentation.dart';
import 'halo_widget.dart';
import 'offline_chat_screen.dart';

String _safeSetupDiagnostic(String value) => value
    .replaceAll(
      RegExp(r'[\u0000-\u001f\u007f-\u009f\u202a-\u202e\u2066-\u2069]'),
      '',
    )
    .replaceAll(RegExp(r'/[^\s,;:"<>()[\]{}]+'), '[path redacted]');

String _modelSize(int? bytes) {
  if (bytes == null) return 'size unknown';
  const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB', 'EiB'];
  var size = bytes.toDouble();
  var unit = 0;
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024;
    unit++;
  }
  return '${unit == 0 ? bytes : size.toStringAsFixed(2)} ${units[unit]}';
}

/// Home has a stable element for the entire application lifetime. Readiness
/// pulses change a strip, not the conversation/draft-owning screen.
class DesktopBootShell extends StatefulWidget {
  const DesktopBootShell({
    super.key,
    required this.backend,
    required this.chat,
    this.setup,
  });
  final DesktopSetup? setup;
  final DesktopBackendController backend;
  final ChatController chat;
  @override
  State<DesktopBootShell> createState() => _DesktopBootShellState();
}

class _DesktopBootShellState extends State<DesktopBootShell> {
  final _setupChanges = ValueNotifier<int>(0);
  @override
  void setState(VoidCallback fn) {
    super.setState(fn);
    _setupChanges.value++;
  }

  bool _homeShown = false;
  Timer? _pulse;
  final _path = TextEditingController();
  final _model = TextEditingController();
  DesktopBackendController get backend => widget.backend;
  late final DesktopSetup setup = widget.setup ?? DesktopSetup();
  SetupInventory? _inventory;
  int _step = 0;
  bool _working = false;
  String _setupDetail = 'Detecting bundled backend. No runtime is launched.';
  int _revision = 0;
  bool _pulling = false;
  DialogRoute<bool>? _approvalRoute;
  final _download = TextEditingController();
  bool get _editable =>
      !_working &&
      !setup.busy &&
      backend.configurable &&
      !backend.ready &&
      !backend.ownsProcess &&
      !widget.chat.busy;
  bool get _selectedInstalled =>
      _inventory?.models.any((m) => m.name == _model.text) == true;
  bool get _mayPull =>
      backend.state == BootState.absent &&
      backend.configurable &&
      !backend.ownsProcess &&
      !widget.chat.busy;
  Future<void> _detect() async {
    if (_working || setup.busy) return;
    setState(() {
      _working = true;
      _setupDetail = 'Validating bundled backend…';
    });
    try {
      final path = await setup.clock.bound(
        _path.text.isEmpty
            ? setup.discover()
            : setup.runtime.resolve(_path.text),
        setup.inspectTimeout,
      );
      if (!mounted) return;
      _path.text = path;
      _setupDetail =
          'Backend detected and validated. Checks do not launch or download.';
      _step = 1;
    } catch (_) {
      _setupDetail =
          'Bundled backend unavailable or invalid. Retry, or provide a reviewed absolute executable in Advanced.';
    } finally {
      if (mounted) setState(() => _working = false);
    }
    if (mounted && _step == 1) await _inspect();
  }

  Future<void> _inspect() async {
    if (_working || setup.busy || _path.text.isEmpty) return;
    final revision = _revision;
    setState(() {
      _working = true;
      _inventory = null;
      _setupDetail =
          'Inspecting local Ollama inventory; no inference or mutation.';
    });
    try {
      final inventory = await setup.inspect(_path.text);
      if (!mounted || revision != _revision) return;
      _inventory = inventory;
      _setupDetail = inventory.status == 'ready'
          ? 'Local runtime inventory observed. Inference is not yet verified.'
          : inventory.status == 'unavailable'
          ? 'Ollama is unavailable. Provision and run Ollama separately at $setupEndpoint, then Retry inspection. Smith cannot install or start Ollama. ${inventory.message ?? ''}'
          : 'Setup prerequisite check failed: ${inventory.message ?? 'unknown error'}. Setup requires Ubuntu 24.04 LTS x86_64 and compatible local inventory. No installation or download is attempted.';
    } catch (_) {
      if (mounted) {
        _setupDetail =
            'Inspection failed or incompatible setup protocol. Retry explicitly after checking the packaged backend.';
      }
    } finally {
      if (mounted) setState(() => _working = false);
    }
  }

  void _select(String tag) {
    if (!_editable) return;
    _revision++;
    _model.text = tag;
    backend.configure(_path.text, tag);
    unawaited(backend.check());
    setState(() {});
  }

  Future<void> _pull() async {
    if (!_editable || !_mayPull || !exactLocalTag(_download.text)) return;
    final tag = _download.text;
    final revision = _revision;
    var succeeded = false;
    String? terminalFailure;
    _pulling = true;
    setState(() {
      _working = true;
      _setupDetail = 'Waiting for model-specific download approval…';
    });
    try {
      await setup.pull(
        _path.text,
        tag,
        permitted: () => mounted && revision == _revision && _mayPull,
        confirm: () async {
          if (!mounted || revision != _revision || !_mayPull) return false;
          final route = _approvalRoute = DialogRoute<bool>(
            context: context,
            barrierDismissible: false,
            builder: (context) => AlertDialog(
              title: const Text('Confirm model download'),
              content: SingleChildScrollView(
                child: Text(
                  'Download exactly $tag using Ollama at $setupEndpoint?\n\n'
                  'Ollama will use the internet to contact its model registry. Download and disk size are unknown until reported. '
                  'Models are stored in Ollama’s external persistent cache, not this conversation. Provider metadata is not artifact authentication. '
                  'Cancel stops local observation/request but daemon work may continue and partial data may remain. No automatic retries or cache deletion. '
                  'This approval expires after 60 seconds.',
                ),
              ),
              actions: [
                TextButton(
                  onPressed: () => Navigator.pop(context, false),
                  child: const Text('Decline'),
                ),
                FilledButton(
                  onPressed: () => Navigator.pop(context, true),
                  child: const Text('Approve download'),
                ),
              ],
            ),
          );
          return await Navigator.of(context).push(route) ?? false;
        },
        onEvent: (event) {
          if (!mounted) return;
          if (event['event'] == 'terminal' && event['status'] != 'success') {
            terminalFailure =
                '${event['status']}: ${_safeSetupDiagnostic(event['code'] as String)} — ${_safeSetupDiagnostic(event['message'] as String)}';
          }
          setState(() {
            _setupDetail = event['event'] == 'progress'
                ? 'Downloading $tag: ${event['status']} · completed ${event['completed'] ?? 'unknown'} / ${event['total'] ?? 'unknown'} bytes'
                : event['event'] == 'terminal'
                ? terminalFailure ??
                      '${event['status']}: ${_safeSetupDiagnostic(event['message'] as String)}'
                : 'Awaiting explicit confirmation for $tag.';
          });
        },
      );
      succeeded = true;
    } catch (_) {
      if (mounted) {
        _setupDetail =
            '${terminalFailure == null ? 'Download stopped or failed.' : terminalFailure!} '
            'No automatic retry. Ollama cache/daemon outcome may remain unknown; Retry inspection before selecting a model.'
            '${setup.cleanupUnknown ? ' Setup cleanup unknown. No duplicate operation allowed. Cancel again to confirm captured-child exit.' : ''}';
      }
    } finally {
      _pulling = false;
      final route = _approvalRoute;
      _approvalRoute = null;
      if (route?.isActive == true) route!.navigator?.removeRoute(route);
      if (mounted) setState(() => _working = false);
    }
    if (mounted && succeeded && !setup.busy) await _inspect();
  }

  @override
  void initState() {
    super.initState();
    _path.text = backend.executable;
    _model.text = backend.model;
    backend.addListener(_changed);
    widget.chat.addListener(_chatChanged);
    unawaited(backend.check());
    unawaited(_detect());
    _pulse = Timer.periodic(
      const Duration(seconds: 4),
      (_) => unawaited(backend.check()),
    );
  }

  void _changed() {
    if (!mounted) return;
    if (_pulling && !_mayPull) unawaited(setup.cancel());
    widget.chat.readinessChanged();
    setState(() {
      if (backend.ready) _homeShown = true;
    });
  }

  void _chatChanged() {
    if (!mounted) return;
    if (_pulling && widget.chat.busy) unawaited(setup.cancel());
    scheduleMicrotask(() {
      if (mounted) setState(() {});
    });
  }

  @override
  void dispose() {
    _pulse?.cancel();
    widget.chat.removeListener(_chatChanged);
    backend.removeListener(_changed);
    _path.dispose();
    _model.dispose();
    _download.dispose();
    _setupChanges.dispose();
    unawaited(setup.cancel());
    super.dispose();
  }

  Future<void> _setup() async {
    await showDialog<void>(
      context: context,
      builder: (context) => ListenableBuilder(
        listenable: Listenable.merge([backend, widget.chat, _setupChanges]),
        builder: (context, _) => AlertDialog(
          title: const Text('Backend setup'),
          content: StatefulBuilder(
            builder: (context, refresh) =>
                SizedBox(width: 560, child: _setupContent(refresh: refresh)),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(context),
              child: const Text('Close setup'),
            ),
          ],
        ),
      ),
    );
  }

  Widget _setupContent({StateSetter? refresh}) => SingleChildScrollView(
    child: Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Semantics(liveRegion: true, child: Text(backend.detail)),
        const SizedBox(height: 16),
        Semantics(liveRegion: true, child: Text(_setupDetail)),
        const SizedBox(height: 16),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            for (var i = 0; i < 4; i++)
              OutlinedButton(
                onPressed:
                    !_working &&
                        i <=
                            (_inventory?.status == 'ready'
                                ? 3
                                : _path.text.isNotEmpty
                                ? 1
                                : 0)
                    ? () {
                        setState(() => _step = i);
                        refresh?.call(() {});
                      }
                    : null,
                child: Text(
                  '${i + 1}. ${['Detect backend', 'Local runtime', 'Select/install model', 'Review & Start'][i]}${_step == i ? ' · current' : ''}',
                ),
              ),
          ],
        ),
        const SizedBox(height: 16),
        if (_step == 0) ...[
          FilledButton(
            onPressed: !_working && !setup.busy ? _detect : null,
            child: const Text('Retry detection'),
          ),
          ExpansionTile(
            title: const Text('Advanced · explicit backend path'),
            children: [
              TextField(
                key: const Key('backend-executable'),
                controller: _path,
                enabled: _editable,
                decoration: const InputDecoration(
                  labelText: 'Absolute packaged backend executable',
                ),
                onChanged: (_) {
                  _revision++;
                  _inventory = null;
                  backend.configure(_path.text, _model.text);
                },
              ),
            ],
          ),
        ],
        if (_step == 1) ...[
          const Text(
            'Ollama must already be running at the fixed local endpoint. Smith does not install dependencies, use sudo, start services, or launch a browser.',
          ),
          TextButton(
            onPressed: !_working && !setup.busy ? _inspect : null,
            child: const Text('Retry inspection'),
          ),
          FilledButton(
            onPressed: _inventory?.status == 'ready' && !_working
                ? () {
                    setState(() => _step = 2);
                    refresh?.call(() {});
                  }
                : null,
            child: const Text('Select a model'),
          ),
        ],
        if (_step == 2) ...[
          const Text(
            'Choose an observed installed local tag, then Review Start. No model is recommended automatically.',
          ),
          // initialValue is not a controlled value: replace form state only
          // when the effective inventory selection changes, not on pulses.
          KeyedSubtree(
            key: ValueKey(_selectedInstalled ? _model.text : null),
            child: DropdownButtonFormField<String>(
              key: const Key('installed-model'),
              initialValue: _selectedInstalled ? _model.text : null,
              isExpanded: true,
              style: Theme.of(context).textTheme.bodyLarge,
              dropdownColor: Theme.of(context).colorScheme.surface,
              decoration: const InputDecoration(
                labelText: 'Installed local model',
              ),
              selectedItemBuilder: (context) => [
                for (final model in _inventory?.models ?? <InstalledModel>[])
                  Text(model.name, overflow: TextOverflow.ellipsis),
              ],
              items: [
                for (final model in _inventory?.models ?? <InstalledModel>[])
                  DropdownMenuItem(
                    value: model.name,
                    child: Text(
                      '${model.name} · ${_modelSize(model.size)}',
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
              ],
              onChanged: _editable
                  ? (value) {
                      if (value != null) _select(value);
                      refresh?.call(() {});
                    }
                  : null,
            ),
          ),
          if (_selectedInstalled)
            Text('Installed locally · ${_model.text} · No download needed.'),
          FilledButton(
            onPressed: _editable && _selectedInstalled
                ? () {
                    setState(() => _step = 3);
                    refresh?.call(() {});
                  }
                : null,
            child: const Text('Review Start'),
          ),
          ExpansionTile(
            title: const Text('Optional · download another model'),
            children: [
              const Text(
                'Only needed for a model not installed locally. Downloading does not fix an occupied bridge port.',
              ),
              TextField(
                key: const Key('download-model'),
                controller: _download,
                enabled: _editable,
                decoration: const InputDecoration(
                  labelText: 'Exact model name:tag to download',
                ),
                onChanged: (_) {
                  setState(() {});
                  refresh?.call(() {});
                },
              ),
              TextButton(
                onPressed:
                    _editable && _mayPull && exactLocalTag(_download.text)
                    ? _pull
                    : null,
                child: const Text('Review model download'),
              ),
            ],
          ),
        ],
        if (_step == 3) ...[
          Text(
            'Start the validated packaged backend with exact installed model ${_model.text}. Endpoint: $setupEndpoint. No download, inference, provider startup or service registration occurs on this page. Chat readiness is verified after launch.',
          ),
          if (_selectedInstalled)
            Text('Installed locally · ${_model.text} · No download needed.'),
          if (!_selectedInstalled)
            const Text(
              'Start is disabled: select a model from the current installed inventory. Retry inspection if inventory is unavailable, then return to Select/install model.',
            )
          else if (!_editable || !backend.canStart)
            Text(
              'Start is disabled: ${backend.detail}${!_editable ? ' Wait for active chat/setup or owned-backend cleanup to finish.' : ''}',
            ),
          FilledButton(
            onPressed: _editable && backend.canStart && _selectedInstalled
                ? backend.start
                : null,
            child: const Text('Start packaged backend'),
          ),
        ],
        if (setup.busy)
          TextButton(
            onPressed: () async {
              await setup.cancel();
              if (mounted) {
                setState(() {});
                refresh?.call(() {});
              }
            },
            child: const Text('Cancel setup operation'),
          ),
        const SizedBox(height: 16),
        Wrap(
          spacing: 12,
          runSpacing: 12,
          children: [
            TextButton(
              onPressed: backend.check,
              child: const Text('Check readiness'),
            ),

            if (backend.ownsProcess)
              TextButton(
                onPressed: backend.stop,
                child: const Text('Stop owned backend'),
              ),
          ],
        ),
        const SizedBox(height: 12),
        const Text(
          'External backends are never stopped. Conversation and draft are memory-only. Normal app close stops its owned child; parent-pipe EOF is a backup, not crash-proof termination.',
        ),
      ],
    ),
  );
  @override
  Widget build(BuildContext context) => Column(
    children: [
      if (_homeShown)
        Material(
          child: SafeArea(
            bottom: false,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
              child: Row(
                children: [
                  Expanded(
                    child: Semantics(
                      liveRegion: true,
                      child: Text(backend.detail),
                    ),
                  ),
                  TextButton(
                    onPressed: _setup,
                    child: const Text('Backend setup'),
                  ),
                ],
              ),
            ),
          ),
        ),
      Expanded(
        child: IndexedStack(
          index: _homeShown ? 1 : 0,
          children: [
            Scaffold(
              appBar: AppBar(title: const Text('Smith · Boot / Setup')),
              body: SafeArea(
                child: Center(
                  child: ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: 680),
                    child: SingleChildScrollView(
                      padding: const EdgeInsets.all(24),
                      child: Column(
                        children: [
                          const SmithHalo(state: HaloState.unknown, size: 96),
                          const SizedBox(height: 16),
                          _setupContent(),
                        ],
                      ),
                    ),
                  ),
                ),
              ),
            ),
            OfflineChatScreen(chat: widget.chat),
          ],
        ),
      ),
    ],
  );
}
