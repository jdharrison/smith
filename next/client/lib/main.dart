import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';

import 'workspace_preferences.dart';
import 'workspace_preferences_io.dart';

import 'app_api.dart';
import 'boot.dart';
import 'chat_panel.dart';
import 'control_api.dart';
import 'native_control.dart';
import 'text_chat.dart';
import 'workspace_controller.dart';
import 'ui/build_info.dart';
import 'ui/smith_theme.dart';
import 'ui/smith_app_header.dart';
import 'ui/smith_workspace.dart';
import 'ui/split_workspace.dart';

/// One owner for presentation hydration, latest-only writes and store lifetime.
class WorkspacePresentation extends ChangeNotifier {
  WorkspacePresentation(this.controller, this.store) {
    controller.addListener(_selectionChanged);
  }

  final WorkspaceController controller;
  final WorkspacePreferencesStore? store;
  WorkspaceLayout layout = WorkspaceLayout.single;
  int active = 0;
  final pages = List<String>.filled(4, 'chat');
  bool loading = true;
  bool _started = false;
  bool _closed = false;
  bool _saveEnabled = false;
  String? warning;
  String? _lastSnapshot;
  WorkspacePreferences? _pending;
  Future<void>? _writing;
  Future<void>? _hydration;

  Future<void> hydrate() => _hydration ??= _hydrate();

  Future<void> _hydrate() async {
    if (_started || _closed) return;
    _started = true;
    String? loadedSnapshot;
    try {
      final saved = await store?.load();
      if (_closed) return;
      if (saved != null) loadedSnapshot = jsonEncode(saved.toJson());
      while (controller.panes.length < 4) {
        controller.createPane();
      }
      if (saved != null) {
        layout = WorkspaceLayout.values.firstWhere(
          (value) => '${value.columns}x${value.rows}' == saved.layout,
        );
        active = saved.activeTile;
        if (active < 0 || active >= layout.count) active = 0;
        for (var i = 0; i < 4; i++) {
          pages[i] = saved.slots[i].page;
        }
        final missing = await controller.restorePaneSelections(
          saved.slots.map((slot) => slot.conversationId).toList(),
          layout.count,
        );
        if (_closed) return;
        if (missing) {
          warning = 'Some saved chats are missing or archived. Those tiles now use New chat.';
        }
      }
      _saveEnabled = store != null;
    } on Object {
      if (_closed) return;
      warning =
          'Workspace preferences couldn’t be loaded. Changes are session-only.';
      _saveEnabled = false;
    }
    if (_closed) return;
    loading = false;
    // Compare against the loaded record, not the repaired in-memory selection.
    // Missing preferences establish a baseline without creating a default file.
    _lastSnapshot = loadedSnapshot ?? jsonEncode(_snapshot().toJson());
    _selectionChanged();
    notifyListeners();
  }

  WorkspacePreferences _snapshot() => WorkspacePreferences(
    layout: '${layout.columns}x${layout.rows}',
    activeTile: active,
    slots: List.generate(
      4,
      (i) => WorkspaceSlotPreferences(
        conversationId: i < controller.panes.length
            ? controller.panes[i].selected.id
            : null,
        page: pages[i],
      ),
    ),
  );

  void setLayout(WorkspaceLayout value, int tile) {
    if (layout == value && active == tile) return;
    layout = value;
    active = tile;
    _selectionChanged();
  }

  void setPage(int index, String page) {
    if (pages[index] == page) return;
    pages[index] = page;
    _selectionChanged();
    notifyListeners();
  }

  void _selectionChanged() {
    if (loading || _closed || !_saveEnabled) return;
    final snapshot = _snapshot();
    final encoded = jsonEncode(snapshot.toJson());
    if (encoded == _lastSnapshot) return;
    _lastSnapshot = encoded;
    _pending = snapshot;
    _writing ??= _drain();
  }

  Future<void> _drain() async {
    // Yield so the owner records this future before a synchronous fake fails.
    await Future<void>.value();
    while (_pending != null) {
      final value = _pending!;
      _pending = null;
      try {
        await store!.save(value);
      } on Object {
        if (!_closed) {
          warning = 'Workspace changes couldn’t be saved. They may not survive restart.';
          notifyListeners();
        }
      }
    }
    _writing = null;
  }

  Future<void> _closeStore() async {
    await _hydration;
    await _writing;
    try {
      await store?.close();
    } on Object {
      // No mounted UI remains. Closing must not surface an unhandled exception.
    }
  }

  @override
  void dispose() {
    _closed = true;
    controller.removeListener(_selectionChanged);
    unawaited(_closeStore());
    super.dispose();
  }
}

void main() => runApp(
  SmithApp(preferencesStore: LinuxWorkspacePreferencesStore.fromEnvironment()),
);

class SmithApp extends StatelessWidget {
  const SmithApp({
    super.key,
    this.controlApi,
    this.appApi,
    this.preferencesStore,
    this.bootClock,
    this.bootObservationBudget = defaultBootObservationBudget,
    this.bootPollInterval = defaultBootPollInterval,
    this.fontFamily = const String.fromEnvironment(
      'SMITH_FONT_FAMILY',
      defaultValue: SmithTheme.defaultFontFamily,
    ),
  });

  final WorkspacePreferencesStore? preferencesStore;
  final String fontFamily;

  final ControlApi? controlApi;
  final AppApi? appApi;
  final BootClock? bootClock;
  final Duration bootObservationBudget;
  final Duration bootPollInterval;

  @override
  Widget build(BuildContext context) => MaterialApp(
    title: 'smith - local autonomy',
    debugShowCheckedModeBanner: false,
    theme: SmithTheme.dark(fontFamily: fontFamily),
    home: SmithShell(
      controlApi: controlApi,
      appApi: appApi,
      preferencesStore: preferencesStore,
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
    this.preferencesStore,
    this.bootClock,
    this.bootObservationBudget = defaultBootObservationBudget,
    this.bootPollInterval = defaultBootPollInterval,
  });

  final WorkspacePreferencesStore? preferencesStore;
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
  late final WorkspacePresentation presentation;

  @override
  void initState() {
    super.initState();
    control = widget.controlApi ?? NativeControlApi();
    chat = WorkspaceController(widget.appApi ?? NativeAppApi())
      ..connected = false;
    presentation = WorkspacePresentation(
      chat as WorkspaceController,
      widget.preferencesStore,
    );
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
    presentation.dispose();
    chat.dispose();
    control.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: boot,
    builder: (context, _) => boot.state.phase == BootPhase.ready
        ? ConversationScreen(
            control: control,
            chat: chat,
            presentation: presentation,
          )
        : BootSurface(coordinator: boot),
  );
}

class ConversationScreen extends StatefulWidget {
  const ConversationScreen({
    super.key,
    required this.control,
    required this.chat,
    this.presentation,
  });

  final WorkspacePresentation? presentation;
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
  BuildInfo buildInfo = const BuildInfo.development();

  @override
  void initState() {
    super.initState();
    widget.presentation?.addListener(_presentationChanged);
    widget.presentation?.hydrate();
    BuildInfo.load().then((info) {
      if (mounted) setState(() => buildInfo = info);
    });
  }

  void _presentationChanged() {
    if (mounted) setState(() {});
  }

  Future<void> retry() async {
    if (inspecting || chat.busy || closed) return;
    setState(() => inspecting = true);
    try {
      if (chat is WorkspaceController) {
        // Post-Boot availability is verified app transport/state, not a fresh
        // assertion about login enablement. Share the heartbeat's reconciler.
        await chat.synchronize();
        if (closed) return;
        if (!chat.stale) await chat.refreshModels();
        if (closed) return;
        status = 'Connected to Smith';
        return;
      }
      chat.unavailable();
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
    if (inspecting && chat is! WorkspaceController) {
      return 'Checking Smith connection…';
    }
    if (chat.stale) return 'Smith application unavailable. Retry queries only.';
    if (chat is WorkspaceController && chat.error != null) {
      return 'Observation unavailable: ${chat.error}';
    }
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
    widget.presentation?.removeListener(_presentationChanged);
    super.dispose();
  }

  List<ChatSummary> _summaries(WorkspaceController owner) => owner.ordered.map((
    c,
  ) {
    final s = c.summary!;
    final id = c.id!;
    final metadata = s['metadata_revision'] as int;
    final history = s['history_revision'] as int;
    return ChatSummary(
      id: id,
      title: s['title'],
      archived: c.archived,
      metadataRevision: metadata,
      historyRevision: history,
      status: c.uncertain
          ? 'Uncertain · Retry queries only'
          : c.pendingId != null
          ? c.operationStatus == 'queued'
                ? 'Queued'
                : c.operationStatus == 'running'
                ? 'Running · ${c.phase}'
                : c.phase
          : owner.mutations.containsKey(id)
          ? 'Change awaiting confirmation'
          : c.provisionalAnswers.isNotEmpty
          ? 'Answer awaiting history sync'
          : c.error != null
          ? 'Needs attention'
          : null,
      mutationsAllowed:
          !owner.stale && owner.connected && !owner.mutations.containsKey(id),
      destructiveAllowed: c.pendingId == null && !c.uncertain,
      rename: (_, title) => owner.mutate(
        'conversation_rename',
        id,
        metadata,
        history,
        title: title,
      ),
      archive: (_) =>
          owner.mutate('conversation_archive', id, metadata, history),
      restore: (_) =>
          owner.mutate('conversation_restore', id, metadata, history),
      delete: (_) => owner.mutate('conversation_delete', id, metadata, history),
    );
  }).toList();

  @override
  Widget build(BuildContext context) => Scaffold(
    body: SafeArea(
      child: widget.presentation?.loading == true
          ? const Center(child: Text('Restoring workspace…'))
          : Column(
              children: [
                if (widget.presentation?.warning != null)
                  Semantics(
                    liveRegion: true,
                    child: Padding(
                      padding: const EdgeInsets.all(8),
                      child: Text(widget.presentation!.warning!),
                    ),
                  ),
                Expanded(
                  child: ListenableBuilder(
                    listenable: chat,
                    builder: (context, _) {
                      final checkingLegacy =
                          inspecting && chat is! WorkspaceController;
                      final online =
                          chat.connected && !chat.stale && !checkingLegacy;
                      final status = checkingLegacy
                          ? 'CHECKING SYSTEM'
                          : online
                          ? 'SYSTEM ONLINE'
                          : 'SYSTEM UNAVAILABLE';
                      final guidance = connectionStatus == 'Connected to Smith'
                          ? null
                          : connectionStatus;
                      Widget shell(
                        WorkspacePane? pane,
                        bool active,
                        VoidCallback? activate,
                      ) {
                        final selected = pane?.selected ?? chat;
                        final index = pane == null
                            ? 0
                            : (chat as WorkspaceController).panes.indexOf(pane);
                        return SmithWorkspace(
                          showAppHeader: pane == null,
                          page: widget.presentation?.pages[index],
                          onPageChanged: widget.presentation == null
                              ? null
                              : (page) =>
                                    widget.presentation!.setPage(index, page),
                          chats: chat is WorkspaceController
                              ? _summaries(chat as WorkspaceController)
                              : const [
                                  ChatSummary(
                                    id: 'current',
                                    title: 'Current conversation',
                                  ),
                                ],
                          selectedChatId: chat is WorkspaceController
                              ? pane!.selected.id
                              : 'current',
                          title: chat is WorkspaceController
                              ? (pane!.selected.summary?['title'] as String? ??
                                    'New chat')
                              : 'Current conversation',
                          onNewChat: chat is WorkspaceController
                              ? () => (chat as WorkspaceController).newChatFor(
                                  pane!,
                                )
                              : null,
                          onSelectChat: chat is WorkspaceController
                              ? (id) => (chat as WorkspaceController).selectFor(
                                  pane!,
                                  id,
                                )
                              : null,
                          onRefreshArchives: chat is WorkspaceController
                              ? retry
                              : null,
                          version: buildInfo.buildVersion,
                          connectionStatus: status,
                          connectionOnline: online,
                          modelStatus: chat.inventoryObserved && !chat.stale
                              ? connectionStatus == 'Connected to Smith'
                                    ? 'Selected model: ${chat.config['model']}. Runtime available; inference not verified by this observation.'
                                    : connectionStatus
                              : 'Model availability not currently verified.',
                          recoveryGuidance: guidance,
                          onRetry: retry,
                          busy: inspecting || chat.busy,
                          child: ChatPanel(
                            controller: selected,
                            onRetry: retry,
                            active: active,
                            onActivate: activate,
                            independentView: pane != null,
                          ),
                        );
                      }

                      if (chat is! WorkspaceController) {
                        return shell(null, true, null);
                      }
                      return SplitWorkspace(
                        headerBuilder: (context, menu) => SmithAppHeader(
                          connectionStatus: status,
                          connectionOnline: online,
                          trailing: menu,
                        ),
                        controller: chat as WorkspaceController,
                        initialLayout:
                            widget.presentation?.layout ??
                            WorkspaceLayout.single,
                        initialActive: widget.presentation?.active ?? 0,
                        onPresentationChanged: widget.presentation?.setLayout,
                        builder: (context, pane, active, activate) =>
                            shell(pane, active, activate),
                      );
                    },
                  ),
                ),
              ],
            ),
    ),
  );
}
