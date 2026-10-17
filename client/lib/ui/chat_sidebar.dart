import 'package:flutter/material.dart';

import '../chat_title.dart';
import 'archived_chats_dialog.dart';
import 'smith_icon.dart';
import 'smith_theme.dart';

@immutable
class ChatSummary {
  const ChatSummary({
    required this.id,
    required this.title,
    this.archived = false,
    this.metadataRevision = 0,
    this.historyRevision = 0,
    this.status,
    this.mutationsAllowed = true,
    this.destructiveAllowed = true,
    this.rename,
    this.archive,
    this.restore,
    this.delete,
  });

  final String id;
  final String title;
  final bool archived;
  final int metadataRevision, historyRevision;
  final String? status;
  final bool mutationsAllowed, destructiveAllowed;
  final RenameChatAction? rename;
  final ChatAction? archive, restore, delete;

  String get displayTitle => displayChatTitle(title);
}

typedef ChatAction = Future<void> Function(String id);
typedef RenameChatAction = Future<void> Function(String id, String title);

/// Presentation only: membership and selection always come from the parent.
class ChatSidebar extends StatelessWidget {
  const ChatSidebar({
    super.key,
    required this.chats,
    this.selectedChatId,
    this.onSelect,
    this.onNewChat,
    this.onOpenArchived,
    this.onRename,
    this.onArchive,
    this.onDelete,
    this.busy = false,
    this.showToolbar = true,
  });

  final bool showToolbar;
  final List<ChatSummary> chats;
  final String? selectedChatId;
  final ValueChanged<String>? onSelect;
  final VoidCallback? onNewChat;
  final VoidCallback? onOpenArchived;
  final RenameChatAction? onRename;
  final ChatAction? onArchive;
  final ChatAction? onDelete;
  final bool busy;

  @override
  Widget build(BuildContext context) {
    final active = chats.where((chat) => !chat.archived).toList();
    return Material(
      color: SmithColors.surface,
      child: Column(
        children: [
          if (showToolbar)
            Container(
              constraints: const BoxConstraints(minHeight: 48),
              padding: const EdgeInsets.only(left: 12, right: 4),
              child: Row(
                children: [
                  const Expanded(child: Text('Chats')),
                  if (onOpenArchived != null)
                    IconButton(
                      tooltip: 'Archived chats',
                      constraints: const BoxConstraints(
                        minWidth: 48,
                        minHeight: 48,
                      ),
                      onPressed: onOpenArchived,
                      icon: const SmithIcon('archive'),
                    ),
                  if (onNewChat != null)
                    IconButton(
                      tooltip: 'New chat',
                      constraints: const BoxConstraints(
                        minWidth: 48,
                        minHeight: 48,
                      ),
                      onPressed: busy ? null : onNewChat,
                      icon: const SmithIcon('plus'),
                    ),
                ],
              ),
            ),
          if (showToolbar)
            const Divider(height: 1, color: SmithColors.strokeSubtle),
          Expanded(
            child: active.isEmpty
                ? const SingleChildScrollView(
                    padding: EdgeInsets.all(16),
                    child: Text('No chats'),
                  )
                : ListView.builder(
                    padding: const EdgeInsets.all(8),
                    itemCount: active.length,
                    itemBuilder: (context, index) {
                      final chat = active[index];
                      final selected = selectedChatId == chat.id;
                      return Container(
                        key: ValueKey(chat.id),
                        margin: const EdgeInsets.only(bottom: 4),
                        decoration: BoxDecoration(
                          color: selected ? SmithColors.surfaceSelected : null,
                          border: Border.all(color: SmithColors.strokeSubtle),
                          borderRadius: BorderRadius.circular(6),
                        ),
                        child: Stack(
                          children: [
                            if (selected)
                              Positioned(
                                left: 0,
                                top: 8,
                                bottom: 8,
                                child: Container(
                                  width: 2,
                                  color: SmithColors.accent,
                                ),
                              ),
                            Row(
                              children: [
                                Expanded(
                                  child: MergeSemantics(
                                    child: Semantics(
                                      selected: selected,
                                      child: onSelect == null
                                          ? Container(
                                              constraints: const BoxConstraints(
                                                minHeight: 48,
                                              ),
                                              alignment: Alignment.centerLeft,
                                              padding:
                                                  const EdgeInsets.symmetric(
                                                    horizontal: 12,
                                                    vertical: 8,
                                                  ),
                                              child: Text(
                                                chat.displayTitle,
                                                style: const TextStyle(
                                                  color:
                                                      SmithColors.textPrimary,
                                                ),
                                              ),
                                            )
                                          : TextButton(
                                              style: TextButton.styleFrom(
                                                alignment: Alignment.centerLeft,
                                                minimumSize: const Size(48, 48),
                                                foregroundColor:
                                                    SmithColors.textPrimary,
                                              ),
                                              onPressed: busy
                                                  ? null
                                                  : () => onSelect!(chat.id),
                                              child: Text(
                                                [
                                                  chat.displayTitle,
                                                  if (chat.status != null)
                                                    chat.status!,
                                                ].join('\n'),
                                              ),
                                            ),
                                    ),
                                  ),
                                ),
                                ChatActionsMenu(
                                  chat: chat,
                                  busy: busy,
                                  onRename: onRename,
                                  onArchive: onArchive,
                                  onDelete: onDelete,
                                ),
                              ],
                            ),
                          ],
                        ),
                      );
                    },
                  ),
          ),
        ],
      ),
    );
  }
}

/// Each menu captures the stable ID, never a mutable title or list position.
class ChatActionsMenu extends StatefulWidget {
  const ChatActionsMenu({
    super.key,
    required this.chat,
    this.onRename,
    this.onArchive,
    this.onReactivate,
    this.onDelete,
    this.busy = false,
  });

  final ChatSummary chat;
  final RenameChatAction? onRename;
  final ChatAction? onArchive;
  final ChatAction? onReactivate;
  final ChatAction? onDelete;
  final bool busy;

  @override
  State<ChatActionsMenu> createState() => _ChatActionsMenuState();
}

class _ChatActionsMenuState extends State<ChatActionsMenu> {
  int _revision = 0;
  bool _menuOpen = false;

  @override
  void didUpdateWidget(covariant ChatActionsMenu oldWidget) {
    super.didUpdateWidget(oldWidget);
    // A callback replacement may represent a new controller/authority owner.
    // Revocation followed by restoration must not revive an old interaction.
    if (oldWidget.chat.id != widget.chat.id ||
        oldWidget.chat.archived != widget.chat.archived ||
        oldWidget.chat.metadataRevision != widget.chat.metadataRevision ||
        oldWidget.chat.mutationsAllowed != widget.chat.mutationsAllowed ||
        oldWidget.chat.destructiveAllowed != widget.chat.destructiveAllowed ||
        oldWidget.busy != widget.busy ||
        oldWidget.onRename != widget.onRename ||
        oldWidget.onArchive != widget.onArchive ||
        oldWidget.onReactivate != widget.onReactivate ||
        oldWidget.onDelete != widget.onDelete) {
      _revision++;
    }
  }

  List<String> get _actions => [
    if (widget.chat.mutationsAllowed &&
        (widget.chat.rename != null || widget.onRename != null))
      'Rename',
    if (widget.chat.mutationsAllowed &&
        widget.chat.destructiveAllowed &&
        !widget.chat.archived &&
        (widget.chat.archive != null || widget.onArchive != null))
      'Archive',
    if (widget.chat.mutationsAllowed &&
        widget.chat.destructiveAllowed &&
        widget.chat.archived &&
        (widget.chat.restore != null || widget.onReactivate != null))
      'Reactivate',
    if (widget.chat.mutationsAllowed &&
        widget.chat.destructiveAllowed &&
        (widget.chat.delete != null || widget.onDelete != null))
      'Delete',
  ];

  bool _available(ChatSummary captured, int revision, String action) =>
      mounted &&
      revision == _revision &&
      !widget.busy &&
      widget.chat.id == captured.id &&
      widget.chat.archived == captured.archived &&
      (action != 'Delete' ||
          widget.chat.historyRevision == captured.historyRevision) &&
      _actions.contains(action);

  Future<void> _dispatch(
    ChatSummary captured,
    int revision,
    String action, {
    String? title,
  }) {
    if (!_available(captured, revision, action)) {
      throw const ChatActionUnavailable();
    }
    // No async gap between validation and reading/invoking the current callback.
    return switch (action) {
      'Rename' => (captured.rename ?? widget.onRename)!(captured.id, title!),
      'Delete' => (captured.delete ?? widget.onDelete)!(captured.id),
      'Archive' => (captured.archive ?? widget.onArchive)!(captured.id),
      'Reactivate' => (captured.restore ?? widget.onReactivate)!(captured.id),
      _ => throw const ChatActionUnavailable(),
    };
  }

  Future<void> _open() async {
    if (_menuOpen || widget.busy || _actions.isEmpty) return;
    setState(() => _menuOpen = true);
    final captured = widget.chat;
    final revision = _revision;
    final navigator = Navigator.of(context, rootNavigator: true);
    final button = context.findRenderObject()! as RenderBox;
    final overlay =
        Navigator.of(context).overlay!.context.findRenderObject()! as RenderBox;
    final position = RelativeRect.fromRect(
      Rect.fromPoints(
        button.localToGlobal(Offset.zero, ancestor: overlay),
        button.localToGlobal(
          button.size.bottomRight(Offset.zero),
          ancestor: overlay,
        ),
      ),
      Offset.zero & overlay.size,
    );
    // Await the route ourselves: PopupMenuButton suppresses onSelected when its
    // anchor is removed, preventing a visible unavailable outcome in that case.
    final action = await showMenu<String>(
      context: context,
      position: position,
      constraints: const BoxConstraints(minWidth: 160, maxWidth: 280),
      items: [
        for (final action in _actions)
          PopupMenuItem(value: action, height: 48, child: Text(action)),
      ],
    );
    if (mounted) setState(() => _menuOpen = false);
    if (action == null || !navigator.mounted) return;
    if (!_available(captured, revision, action)) {
      await showChatActionUnavailable(navigator.context);
      return;
    }
    if (action == 'Rename') {
      await showChatRenameDialog(
        navigator.context,
        chat: captured,
        onRename: (_, title) =>
            _dispatch(captured, revision, action, title: title),
      );
    } else if (action == 'Delete') {
      await showChatDeleteDialog(
        navigator.context,
        chat: captured,
        onDelete: (_) => _dispatch(captured, revision, action),
      );
    } else {
      await showChatActionDialog(
        navigator.context,
        chat: captured,
        action: action,
        onAction: (_) => _dispatch(captured, revision, action),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_actions.isEmpty) return const SizedBox.shrink();
    return Semantics(
      expanded: _menuOpen,
      child: IconButton(
        color: IconTheme.of(context).color,
        tooltip: 'Actions for ${widget.chat.displayTitle}',
        onPressed: widget.busy ? null : _open,
        icon: const SmithIcon('ellipsis-vertical'),
        style: IconButton.styleFrom(minimumSize: const Size(48, 48)),
      ),
    );
  }
}
