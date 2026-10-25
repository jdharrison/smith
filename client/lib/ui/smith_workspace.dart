import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../chat_title.dart';
import 'archived_chats_dialog.dart';
import 'chat_sidebar.dart';
import 'settings_view.dart';
import 'smith_icon.dart';
import 'smith_theme.dart';
import 'smith_app_header.dart';
import 'workspace_entrance.dart';

export 'chat_sidebar.dart' show ChatSummary, ChatAction, RenameChatAction;

/// A controller-free shell. Callbacks complete only after acknowledgement and
/// throw on failure/uncertainty; the parent supplies all authoritative updates.
class SmithWorkspace extends StatefulWidget {
  const SmithWorkspace({
    super.key,
    required this.child,
    required this.chats,
    required this.title,
    required this.version,
    required this.connectionStatus,
    required this.modelStatus,
    this.connectionOnline = false,
    this.selectedChatId,
    this.onSelectChat,
    this.onNewChat,
    this.onRenameChat,
    this.onArchiveChat,
    this.onReactivateChat,
    this.onDeleteChat,
    this.onRetry,
    this.busy = false,
    this.error,
    this.recoveryGuidance,
    this.archivesLoading = false,
    this.archivesError,
    this.onRefreshArchives,
    this.showAppHeader = true,
    this.page,
    this.onPageChanged,
    this.projects,
    this.agents,
    this.devJobs,
    this.instructions,
  });

  final bool showAppHeader;
  final String? page;
  final ValueChanged<String>? onPageChanged;
  final Widget child;
  final Widget? projects;
  final Widget? agents;
  final Widget? devJobs;
  final Widget? instructions;
  final List<ChatSummary> chats;
  final String title;
  final String version;
  final String connectionStatus;
  final String modelStatus;
  final bool connectionOnline;
  final String? selectedChatId;
  final ValueChanged<String>? onSelectChat;
  final VoidCallback? onNewChat;
  final RenameChatAction? onRenameChat;
  final ChatAction? onArchiveChat;
  final ChatAction? onReactivateChat;
  final ChatAction? onDeleteChat;
  final VoidCallback? onRetry;
  final bool busy;
  final String? error;
  final String? recoveryGuidance;
  final bool archivesLoading;
  final String? archivesError;
  final VoidCallback? onRefreshArchives;

  @override
  State<SmithWorkspace> createState() => _SmithWorkspaceState();
}

class _SmithWorkspaceState extends State<SmithWorkspace> {
  // Temporarily hide entry points without removing the retained Agents page.
  static const _showAgentsNavigation = false;

  String _localPage = 'chat';
  String get _page => widget.page ?? _localPage;
  bool get _chat => _page == 'chat';
  bool get _settings => _page == 'settings';
  bool get _projects => _page == 'projects';
  bool _projectsOpened = false;
  bool get _agents => _page == 'agents';
  bool _agentsOpened = false;
  bool get _dev => _page == 'dev';
  bool _devOpened = false;
  bool _devExpanded = false;

  final _devFocus = FocusNode(debugLabel: 'Dev navigation root');
  final _devDisclosureFocus = FocusNode(debugLabel: 'Dev disclosure');
  final _projectsFocus = FocusNode(debugLabel: 'Projects navigation');
  final _agentsFocus = FocusNode(debugLabel: 'Agents navigation');

  void _discloseDev() {
    if (_devExpanded && (_projectsFocus.hasFocus || _agentsFocus.hasFocus)) {
      _devDisclosureFocus.requestFocus();
    }
    setState(() => _devExpanded = !_devExpanded);
  }

  void _setPage(String page) {
    if (widget.page == null) {
      setState(() => _localPage = page);
    }
    widget.onPageChanged?.call(page);
  }

  bool _historyOpen = false;
  late final ValueNotifier<SmithWorkspace> _presentation;

  @override
  void initState() {
    super.initState();
    _presentation = ValueNotifier(widget);
  }

  @override
  void didUpdateWidget(covariant SmithWorkspace oldWidget) {
    super.didUpdateWidget(oldWidget);

    // Modal routes are siblings, not descendants of this build scope.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _presentation.value = widget;
    });
  }

  @override
  void dispose() {
    _presentation.dispose();
    _devFocus.dispose();
    _devDisclosureFocus.dispose();
    _projectsFocus.dispose();
    _agentsFocus.dispose();
    super.dispose();
  }

  bool _hasArchives(SmithWorkspace data) =>
      data.onArchiveChat != null ||
      data.onReactivateChat != null ||
      data.chats.any((chat) => chat.archived) ||
      data.onRefreshArchives != null;

  Future<void> _archives() => showDialog<void>(
    context: context,
    builder: (_) => ValueListenableBuilder<SmithWorkspace>(
      valueListenable: _presentation,
      builder: (_, data, _) => ArchivedChatsDialog(
        chats: data.chats,
        onReactivate: data.onReactivateChat,
        onRename: data.onRenameChat,
        onDelete: data.onDeleteChat,
        loading: data.archivesLoading || data.busy,
        error: data.archivesError,
        onRetry: data.onRefreshArchives,
      ),
    ),
  );

  Widget _sidebar(
    SmithWorkspace data, {
    BuildContext? drawerContext,
    bool showToolbar = true,
  }) => ChatSidebar(
    chats: data.chats,
    showToolbar: showToolbar,
    selectedChatId: data.selectedChatId,
    busy: data.busy,
    onSelect: data.onSelectChat == null
        ? null
        : (id) {
            if (!mounted ||
                widget.busy ||
                widget.onSelectChat == null ||
                !widget.chats.any((chat) => chat.id == id && !chat.archived)) {
              return;
            }
            final select = widget.onSelectChat!;
            if (drawerContext != null) Navigator.pop(drawerContext);
            select(id);
          },
    onNewChat: data.onNewChat == null
        ? null
        : () {
            if (!mounted || widget.busy || widget.onNewChat == null) return;
            final create = widget.onNewChat!;
            if (drawerContext != null) Navigator.pop(drawerContext);
            create();
          },
    onOpenArchived: _hasArchives(data) ? _archives : null,
    onRename: data.onRenameChat,
    onArchive: data.onArchiveChat,
    onDelete: data.onDeleteChat,
  );

  Future<void> _chooser(BuildContext anchorContext) async {
    if (_historyOpen) return;
    final anchor = anchorContext.findRenderObject()! as RenderBox;
    final navigator = Navigator.of(context, rootNavigator: true);
    final overlay = navigator.overlay!.context.findRenderObject()! as RenderBox;
    final bottom = anchor.localToGlobal(
      Offset(0, anchor.size.height),
      ancestor: overlay,
    );
    setState(() => _historyOpen = true);
    await showDialog<void>(
      context: context,
      builder: (dialogContext) => LayoutBuilder(
        builder: (_, bounds) {
          final width = math.min(360.0, bounds.maxWidth - 32);
          final height = math.min(360.0, bounds.maxHeight - 32);
          return Stack(
            children: [
              Positioned(
                left: bottom.dx.clamp(16.0, bounds.maxWidth - width - 16),
                top: bottom.dy.clamp(16.0, bounds.maxHeight - height - 16),
                width: width,
                height: height,
                child: Material(
                  key: const ValueKey('compact-chat-chooser'),
                  elevation: 8,
                  color: SmithColors.surface,
                  shape: const RoundedRectangleBorder(
                    side: BorderSide(color: SmithColors.strokeSubtle),
                    borderRadius: BorderRadius.all(Radius.circular(6)),
                  ),
                  clipBehavior: Clip.antiAlias,
                  child: Column(
                    children: [
                      Align(
                        alignment: Alignment.centerRight,
                        child: IconButton(
                          autofocus: true,
                          tooltip: 'Close chat chooser',
                          onPressed: () => Navigator.pop(dialogContext),
                          icon: const SmithIcon('x'),
                        ),
                      ),
                      Expanded(
                        child: ValueListenableBuilder<SmithWorkspace>(
                          valueListenable: _presentation,
                          builder: (_, data, _) => _sidebar(
                            data,
                            drawerContext: dialogContext,
                            showToolbar: false,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ],
          );
        },
      ),
    );
    if (mounted) setState(() => _historyOpen = false);
  }

  Future<void> _history() async {
    if (_historyOpen) return;
    setState(() => _historyOpen = true);
    await showDialog<void>(
      context: context,
      builder: (dialogContext) => Dialog(
        shape: const RoundedRectangleBorder(
          borderRadius: BorderRadius.all(Radius.circular(8)),
          side: BorderSide(color: SmithColors.strokeSubtle),
        ),
        alignment: Alignment.centerLeft,
        insetPadding: const EdgeInsets.fromLTRB(56, 8, 8, 8),
        child: SizedBox(
          width: 320,
          child: Column(
            children: [
              Align(
                alignment: Alignment.centerRight,
                child: IconButton(
                  autofocus: true,
                  tooltip: 'Close chat history',
                  constraints: const BoxConstraints(
                    minWidth: 48,
                    minHeight: 48,
                  ),
                  onPressed: () => Navigator.pop(dialogContext),
                  icon: const SmithIcon('x'),
                ),
              ),
              Expanded(
                child: ValueListenableBuilder<SmithWorkspace>(
                  valueListenable: _presentation,
                  builder: (_, data, _) =>
                      _sidebar(data, drawerContext: dialogContext),
                ),
              ),
            ],
          ),
        ),
      ),
    );
    if (mounted) setState(() => _historyOpen = false);
  }

  Widget _navigationHighlight({
    required String identity,
    required bool selected,
    required Widget child,
  }) => Container(
    key: ValueKey('navigation-highlight-$identity'),
    decoration: BoxDecoration(
      color: selected ? SmithColors.surfaceSelected : Colors.transparent,
      borderRadius: BorderRadius.circular(6),
    ),
    child: Stack(
      children: [
        child,
        if (selected)
          Positioned(
            left: 0,
            top: 6,
            bottom: 6,
            width: 2,
            child: const IgnorePointer(
              child: ColoredBox(color: SmithColors.accent),
            ),
          ),
      ],
    ),
  );

  Widget _railButton({
    required String label,
    required String semanticLabel,
    required String icon,
    required bool selected,
    required VoidCallback onPressed,
    bool? expanded,
    FocusNode? focusNode,
    Widget? glyph,
    bool showLabel = false,
    bool integrated = false,
    double bottomSpacing = 4,
    String? text,
    Color? foregroundColor,
  }) {
    final tint =
        foregroundColor ??
        (selected ? SmithColors.accent : SmithColors.textPrimary);
    final button = MergeSemantics(
      child: Semantics(
        label: semanticLabel,
        hint: label,
        selected: selected,
        expanded: expanded,
        child: Tooltip(
          message: label,
          excludeFromSemantics: true,
          child: IconButton(
            isSelected: selected,
            focusNode: focusNode,
            onPressed: onPressed,
            constraints: BoxConstraints.tightFor(
              width: showLabel ? null : 48,
              height: 48,
            ),
            style: ButtonStyle(
              foregroundColor: WidgetStatePropertyAll(tint),
              backgroundColor: const WidgetStatePropertyAll(Colors.transparent),
              alignment: Alignment.centerLeft,
              shape: WidgetStatePropertyAll(
                RoundedRectangleBorder(borderRadius: BorderRadius.circular(6)),
              ),
              side: WidgetStateProperty.resolveWith(
                (states) => BorderSide(
                  color: states.contains(WidgetState.focused)
                      ? SmithColors.accentBright
                      : Colors.transparent,
                  width: states.contains(WidgetState.focused) ? 2 : 1,
                ),
              ),
            ),
            icon: showLabel
                ? Row(
                    children: [
                      glyph ?? SmithIcon(icon, color: tint),
                      const SizedBox(width: 12),
                      Expanded(
                        child: ExcludeSemantics(
                          child: Text(
                            text ?? semanticLabel,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: Theme.of(context).textTheme.bodyMedium
                                ?.copyWith(color: SmithColors.textPrimary),
                          ),
                        ),
                      ),
                    ],
                  )
                : glyph ?? SmithIcon(icon, color: tint),
          ),
        ),
      ),
    );
    return Padding(
      padding: EdgeInsets.only(bottom: bottomSpacing),
      child: !integrated
          ? _navigationHighlight(
              identity: semanticLabel,
              selected: selected,
              child: button,
            )
          : button,
    );
  }

  Widget _panel(Widget child) => Container(
    padding: const EdgeInsets.all(1),
    clipBehavior: Clip.antiAlias,
    decoration: BoxDecoration(
      border: Border.all(color: SmithColors.strokeSubtle),
      borderRadius: BorderRadius.circular(6),
    ),
    child: child,
  );

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, bounds) {
      if (_projects) _projectsOpened = true;
      if (_agents) _agentsOpened = true;
      if (_dev) _devOpened = true;
      final scale = MediaQuery.textScalerOf(context).scale(14) / 14;
      final compact = bounds.maxWidth < 960 + (scale - 1).clamp(0, 2) * 320;
      final labelPainter = TextPainter(
        text: TextSpan(
          text: 'Projects',
          style: Theme.of(context).textTheme.bodyMedium,
        ),
        textScaler: MediaQuery.textScalerOf(context),
        textDirection: Directionality.of(context),
        maxLines: 1,
      )..layout();
      final navigationWidth = compact
          ? 104.0
          : math.max(184.0, labelPainter.width + 96);
      labelPainter.dispose();
      return Material(
        color: SmithColors.background,
        child: Column(
          children: [
            if (widget.showAppHeader)
              SmithAppHeader(
                connectionStatus: widget.connectionStatus,
                connectionOnline: widget.connectionOnline,
              ),
            Expanded(
              child: Padding(
                padding: const EdgeInsets.all(8),
                child: Row(
                  children: [
                    WorkspaceReveal(
                      region: WorkspaceRegion.navigation,
                      child: Container(
                        key: const ValueKey('workspace-navigation'),
                        width: navigationWidth,
                        height: double.infinity,
                        alignment: Alignment.topLeft,
                        padding: const EdgeInsets.only(left: 4, right: 3),
                        decoration: const BoxDecoration(
                          border: Border(
                            right: BorderSide(color: SmithColors.strokeSubtle),
                          ),
                        ),
                        child: SingleChildScrollView(
                          child: Column(
                            mainAxisSize: MainAxisSize.min,
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            children: [
                              _railButton(
                                semanticLabel: 'Chat',
                                text: 'Chats',
                                showLabel: !compact,
                                label: !_chat
                                    ? 'Return to Chat'
                                    : compact
                                    ? 'Open chat history'
                                    : 'Chat',
                                icon: 'message-square',
                                selected: _chat,
                                expanded: compact && _chat
                                    ? _historyOpen
                                    : null,
                                onPressed: () {
                                  if (!_chat) {
                                    _setPage('chat');
                                  } else if (compact && !_historyOpen) {
                                    _history();
                                  }
                                },
                              ),
                              Padding(
                                padding: const EdgeInsets.only(bottom: 4),
                                child: _navigationHighlight(
                                  identity: 'Dev',
                                  selected: _dev,
                                  child: Row(
                                    key: const ValueKey('dev-navigation-row'),
                                    children: [
                                      Expanded(
                                        child: _railButton(
                                          semanticLabel: 'Dev',
                                          label: 'Dev',
                                          icon: 'archive',
                                          glyph: const Icon(Icons.code),
                                          showLabel: !compact,
                                          integrated: true,
                                          bottomSpacing: 0,
                                          selected: _dev,
                                          focusNode: _devFocus,
                                          onPressed: () => _setPage('dev'),
                                        ),
                                      ),
                                      _railButton(
                                        semanticLabel: 'Dev navigation',
                                        label: 'Dev navigation',
                                        icon: 'archive',
                                        glyph: Icon(
                                          _devExpanded
                                              ? Icons.expand_more
                                              : Icons.chevron_right,
                                          size: 18,
                                        ),
                                        integrated: true,
                                        bottomSpacing: 0,
                                        foregroundColor: _dev
                                            ? SmithColors.accent
                                            : SmithColors.textPrimary,
                                        focusNode: _devDisclosureFocus,
                                        selected: false,
                                        expanded: _devExpanded,
                                        onPressed: _discloseDev,
                                      ),
                                    ],
                                  ),
                                ),
                              ),
                              if (_devExpanded)
                                Container(
                                  key: const ValueKey(
                                    'dev-navigation-children',
                                  ),
                                  margin: const EdgeInsets.only(left: 12),
                                  padding: const EdgeInsets.only(left: 4),
                                  decoration: const BoxDecoration(
                                    border: Border(
                                      left: BorderSide(
                                        color: SmithColors.strokeSubtle,
                                      ),
                                    ),
                                  ),
                                  child: Column(
                                    crossAxisAlignment:
                                        CrossAxisAlignment.stretch,
                                    children: [
                                      _railButton(
                                        semanticLabel: 'Projects',
                                        label: 'Projects',
                                        icon: 'archive',
                                        glyph: const Icon(
                                          Icons.view_in_ar_outlined,
                                        ),
                                        showLabel: !compact,
                                        selected: _projects,
                                        focusNode: _projectsFocus,
                                        onPressed: () => _setPage('projects'),
                                      ),
                                      if (_showAgentsNavigation)
                                        _railButton(
                                          semanticLabel: 'Agents',
                                          label: 'Agents',
                                          icon: 'archive',
                                          glyph: const Icon(
                                            Icons.smart_toy_outlined,
                                          ),
                                          showLabel: !compact,
                                          selected: _agents,
                                          focusNode: _agentsFocus,
                                          onPressed: () => _setPage('agents'),
                                        ),
                                    ],
                                  ),
                                ),
                              const SizedBox(height: 8),
                              _railButton(
                                semanticLabel: 'Settings',
                                showLabel: !compact,
                                label: 'Settings',
                                icon: 'settings',
                                selected: _settings,
                                onPressed: () => _setPage('settings'),
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
                    // Keep these slots stable through breakpoint changes; never move child.
                    SizedBox(
                      width: compact || !_chat ? 0 : 288,
                      child: compact || !_chat
                          ? null
                          : Padding(
                              padding: const EdgeInsets.only(right: 8),
                              child: WorkspaceReveal(
                                region: WorkspaceRegion.sidebar,
                                child: _panel(_sidebar(widget)),
                              ),
                            ),
                    ),
                    Expanded(
                      child: _panel(
                        Column(
                          children: [
                            if (!_projects && !_agents)
                              WorkspaceReveal(
                                region: WorkspaceRegion.title,
                                child: Container(
                                  width: double.infinity,
                                  constraints: const BoxConstraints(
                                    minHeight: 48,
                                  ),
                                  padding: EdgeInsets.all(
                                    bounds.maxHeight < 500 ? 8 : 12,
                                  ),
                                  decoration: const BoxDecoration(
                                    border: Border(
                                      bottom: BorderSide(
                                        color: SmithColors.strokeSubtle,
                                      ),
                                    ),
                                  ),
                                  child: compact && _chat
                                      ? Row(
                                          children: [
                                            Expanded(
                                              child: Builder(
                                                builder: (anchor) => Tooltip(
                                                  message:
                                                      'Choose chat: ${displayChatTitle(widget.title)}',
                                                  child: Semantics(
                                                    expanded: _historyOpen,
                                                    child: TextButton(
                                                      key: const ValueKey(
                                                        'compact-chat-title',
                                                      ),
                                                      onPressed: () =>
                                                          _chooser(anchor),
                                                      child: Row(
                                                        children: [
                                                          Expanded(
                                                            child: Text(
                                                              displayChatTitle(
                                                                widget.title,
                                                              ),
                                                              maxLines: 1,
                                                              overflow:
                                                                  TextOverflow
                                                                      .ellipsis,
                                                            ),
                                                          ),
                                                          const Icon(
                                                            Icons.expand_more,
                                                          ),
                                                        ],
                                                      ),
                                                    ),
                                                  ),
                                                ),
                                              ),
                                            ),
                                            if (widget.onNewChat != null)
                                              IconButton(
                                                tooltip: 'New chat',
                                                onPressed: widget.busy
                                                    ? null
                                                    : widget.onNewChat,
                                                icon: const SmithIcon('plus'),
                                              ),
                                            if (_hasArchives(widget))
                                              IconButton(
                                                tooltip: 'Archived chats',
                                                onPressed: _archives,
                                                icon: const SmithIcon(
                                                  'archive',
                                                ),
                                              ),
                                          ],
                                        )
                                      : Tooltip(
                                          message: !_chat
                                              ? _settings
                                                    ? 'Settings'
                                                    : _agents
                                                    ? 'Agents'
                                                    : _dev
                                                    ? 'Dev'
                                                    : 'Projects'
                                              : 'Chat / ${displayChatTitle(widget.title)}',
                                          child: Text.rich(
                                            TextSpan(
                                              children: !_chat
                                                  ? [
                                                      TextSpan(
                                                        text: _settings
                                                            ? 'Settings'
                                                            : _agents
                                                            ? 'Agents'
                                                            : _dev
                                                            ? 'Dev'
                                                            : 'Projects',
                                                      ),
                                                    ]
                                                  : [
                                                      const TextSpan(
                                                        text: 'Chat',
                                                        style: TextStyle(
                                                          color: SmithColors
                                                              .accent,
                                                        ),
                                                      ),
                                                      TextSpan(
                                                        text:
                                                            ' / ${displayChatTitle(widget.title)}',
                                                      ),
                                                    ],
                                            ),
                                            maxLines: bounds.maxHeight < 500
                                                ? 1
                                                : 2,
                                            overflow: TextOverflow.ellipsis,
                                            semanticsLabel: !_chat
                                                ? _settings
                                                      ? 'Settings'
                                                      : _agents
                                                      ? 'Agents'
                                                      : _dev
                                                      ? 'Dev'
                                                      : 'Projects'
                                                : 'Chat / ${displayChatTitle(widget.title)}',
                                          ),
                                        ),
                                ),
                              ),
                            if (!_dev &&
                                !_projects &&
                                !_agents &&
                                (widget.error != null ||
                                    widget.recoveryGuidance != null))
                              ConstrainedBox(
                                constraints: BoxConstraints(
                                  maxHeight: bounds.maxHeight * .22,
                                ),
                                child: SingleChildScrollView(
                                  padding: const EdgeInsets.all(8),
                                  child: Semantics(
                                    liveRegion: true,
                                    child: Column(
                                      crossAxisAlignment:
                                          CrossAxisAlignment.start,
                                      children: [
                                        if (widget.error != null)
                                          Text(
                                            widget.error!,
                                            style: const TextStyle(
                                              color: SmithColors.danger,
                                            ),
                                          ),
                                        if (widget.recoveryGuidance != null)
                                          Text(widget.recoveryGuidance!),
                                        if (widget.onRetry != null)
                                          TextButton(
                                            onPressed: widget.busy
                                                ? null
                                                : widget.onRetry,
                                            child: const Text(
                                              'Retry observation',
                                            ),
                                          ),
                                      ],
                                    ),
                                  ),
                                ),
                              ),
                            Expanded(
                              key: const ValueKey('workspace-chat-slot'),
                              child: Stack(
                                fit: StackFit.expand,
                                children: [
                                  Offstage(
                                    key: const ValueKey('destination-chat'),
                                    offstage: !_chat,
                                    child: TickerMode(
                                      enabled: _chat,
                                      child: ExcludeFocus(
                                        excluding: !_chat,
                                        child: widget.child,
                                      ),
                                    ),
                                  ),
                                  if (_projectsOpened)
                                    Offstage(
                                      key: const ValueKey(
                                        'destination-projects',
                                      ),
                                      offstage: !_projects,
                                      child: TickerMode(
                                        enabled: _projects,
                                        child: ExcludeFocus(
                                          excluding: !_projects,
                                          child:
                                              widget.projects ??
                                              const Center(
                                                child: Text(
                                                  'Projects unavailable in this view.',
                                                ),
                                              ),
                                        ),
                                      ),
                                    ),
                                  if (_agentsOpened)
                                    Offstage(
                                      key: const ValueKey('destination-agents'),
                                      offstage: !_agents,
                                      child: TickerMode(
                                        enabled: _agents,
                                        child: ExcludeFocus(
                                          excluding: !_agents,
                                          child:
                                              widget.agents ??
                                              const Center(
                                                child: Text(
                                                  'Agents unavailable in this view.',
                                                ),
                                              ),
                                        ),
                                      ),
                                    ),
                                  if (_devOpened)
                                    Offstage(
                                      key: const ValueKey('destination-dev'),
                                      offstage: !_dev,
                                      child: TickerMode(
                                        enabled: _dev,
                                        child: ExcludeFocus(
                                          excluding: !_dev,
                                          child:
                                              widget.devJobs ??
                                              const Center(
                                                child: Text(
                                                  'Dev jobs unavailable in this view.',
                                                ),
                                              ),
                                        ),
                                      ),
                                    ),
                                  if (_settings)
                                    WorkspaceReveal(
                                      key: const ValueKey(
                                        'destination-settings',
                                      ),
                                      region: WorkspaceRegion.settings,
                                      child: SettingsView(
                                        connectionStatus:
                                            widget.connectionStatus,
                                        modelStatus: widget.modelStatus,
                                        version: widget.version,
                                        instructions: widget.instructions,
                                      ),
                                    ),
                                ],
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
      );
    },
  );
}
