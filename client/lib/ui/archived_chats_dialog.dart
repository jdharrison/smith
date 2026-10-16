import 'package:flutter/material.dart';

import '../chat_title.dart';
import 'chat_sidebar.dart';
import 'smith_icon.dart';
import 'smith_theme.dart';

const _dialogShape = RoundedRectangleBorder(
  borderRadius: BorderRadius.all(Radius.circular(8)),
  side: BorderSide(color: SmithColors.strokeSubtle),
);

class ArchivedChatsDialog extends StatelessWidget {
  const ArchivedChatsDialog({
    super.key,
    required this.chats,
    this.onReactivate,
    this.onRename,
    this.onDelete,
    this.loading = false,
    this.error,
    this.onRetry,
  });

  final List<ChatSummary> chats;
  final ChatAction? onReactivate;
  final RenameChatAction? onRename;
  final ChatAction? onDelete;
  final bool loading;
  final String? error;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) {
    final archived = chats.where((chat) => chat.archived).toList();
    return AlertDialog(
      shape: _dialogShape,
      insetPadding: const EdgeInsets.all(12),
      titleTextStyle: Theme.of(context).textTheme.titleMedium,
      title: Row(
        children: [
          const Expanded(child: Text('Archived chats')),
          IconButton(
            autofocus: true,
            tooltip: 'Close archived chats',
            constraints: const BoxConstraints(minWidth: 48, minHeight: 48),
            onPressed: () => Navigator.pop(context),
            icon: const SmithIcon('x'),
          ),
        ],
      ),
      content: SizedBox(
        width: 560,
        height: 400,
        child: Column(
          children: [
            if (loading)
              const LinearProgressIndicator(
                semanticsLabel: 'Loading archived chats',
              ),
            Expanded(
              child: ListView(
                children: [
                  if (error != null) ...[
                    Text(
                      error!,
                      semanticsLabel: 'Archived chats error: $error',
                    ),
                    const Text(
                      'Refresh to check whether the change completed.',
                    ),
                    if (onRetry != null)
                      TextButton(
                        onPressed: loading ? null : onRetry,
                        child: const Text('Refresh'),
                      ),
                  ],
                  if (archived.isEmpty && !loading)
                    const Text('No archived chats'),
                  for (final chat in archived)
                    Row(
                      key: ValueKey(chat.id),
                      children: [
                        Expanded(
                          child: Padding(
                            padding: const EdgeInsets.symmetric(vertical: 12),
                            child: Text(chat.displayTitle),
                          ),
                        ),
                        ChatActionsMenu(
                          chat: chat,
                          onReactivate: onReactivate,
                          onRename: onRename,
                          onDelete: onDelete,
                          busy: loading,
                        ),
                      ],
                    ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// A presentation action lost its authority before dispatch; nothing was sent.
class ChatActionUnavailable implements Exception {
  const ChatActionUnavailable();
}

const _unavailableMessage =
    'This action is no longer available. No change was sent.';

Future<void> showChatActionUnavailable(BuildContext context) =>
    showDialog<void>(
      context: context,
      builder: (context) => AlertDialog(
        shape: _dialogShape,
        insetPadding: const EdgeInsets.all(12),
        scrollable: true,
        title: const Text('Action unavailable'),
        content: const Text(_unavailableMessage),
        actions: [
          TextButton(
            autofocus: true,
            onPressed: () => Navigator.pop(context),
            child: const Text('Close'),
          ),
        ],
      ),
    );

// The callbacks supplied by ChatActionsMenu revalidate current widget authority
// at confirmation, not only when this route is opened.
Future<void> showChatRenameDialog(
  BuildContext context, {
  required ChatSummary chat,
  required RenameChatAction onRename,
}) => showDialog<void>(
  context: context,
  builder: (_) =>
      _MutationDialog(chat: chat, action: 'Rename', onRename: onRename),
);

Future<void> showChatDeleteDialog(
  BuildContext context, {
  required ChatSummary chat,
  required ChatAction onDelete,
}) => showDialog<void>(
  context: context,
  builder: (_) =>
      _MutationDialog(chat: chat, action: 'Delete', onAction: onDelete),
);

Future<void> showChatActionDialog(
  BuildContext context, {
  required ChatSummary chat,
  required String action,
  required ChatAction onAction,
}) => showDialog<void>(
  context: context,
  barrierDismissible: false,
  builder: (_) => _MutationDialog(
    chat: chat,
    action: action,
    onAction: onAction,
    runImmediately: true,
  ),
);

class _MutationDialog extends StatefulWidget {
  const _MutationDialog({
    required this.chat,
    required this.action,
    this.onAction,
    this.onRename,
    this.runImmediately = false,
  });
  final ChatSummary chat;
  final String action;
  final ChatAction? onAction;
  final RenameChatAction? onRename;
  final bool runImmediately;

  @override
  State<_MutationDialog> createState() => _MutationDialogState();
}

class _MutationDialogState extends State<_MutationDialog> {
  late final TextEditingController _title;
  bool _pending = false;
  bool _failed = false;
  bool _unavailable = false;

  @override
  void initState() {
    super.initState();
    _title = TextEditingController(text: widget.chat.title);
    if (widget.runImmediately) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) _apply();
      });
    }
  }

  @override
  void dispose() {
    _title.dispose();
    super.dispose();
  }

  bool get _titleTooLong => _title.text.trim().runes.length > maxChatTitleRunes;

  Future<void> _apply() async {
    if (_pending ||
        _failed ||
        (widget.onRename != null &&
            (_titleTooLong || _title.text.trim().isEmpty))) {
      return;
    }
    setState(() => _pending = true);
    try {
      if (widget.onRename != null) {
        await widget.onRename!(widget.chat.id, _title.text.trim());
      } else {
        await widget.onAction!(widget.chat.id);
      }
      if (mounted) Navigator.pop(context);
    } on ChatActionUnavailable {
      if (mounted) {
        setState(() {
          _pending = false;
          _failed = true;
          _unavailable = true;
        });
      }
    } catch (_) {
      if (mounted) {
        setState(() {
          _pending = false;
          _failed = true;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) => PopScope(
    canPop: !_pending,
    child: AlertDialog(
      shape: _dialogShape,
      insetPadding: const EdgeInsets.all(12),
      scrollable: true,
      title: Text('${widget.action} chat'),
      content: SizedBox(
        width: 400,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(widget.chat.displayTitle),
            if (widget.action == 'Delete')
              const Text(
                'Deletes this chat, history and results. You can’t undo this. Backups are kept.',
              ),
            if (widget.onRename != null)
              TextField(
                controller: _title,
                autofocus: true,
                enabled: !_pending && !_failed,
                minLines: 1,
                maxLines: 3,
                decoration: InputDecoration(
                  labelText: 'Chat title',
                  helperText: 'Up to 40 characters.',
                  counterText:
                      '${_title.text.trim().runes.length}/$maxChatTitleRunes',
                  errorText: _titleTooLong
                      ? 'Use 40 characters or fewer.'
                      : null,
                ),
                onChanged: (_) => setState(() {}),
              ),
            if (_pending) ...[
              const SizedBox(height: 12),
              LinearProgressIndicator(
                semanticsLabel: '${widget.action} in progress',
              ),
            ],
            if (_failed)
              Text(
                _unavailable ? _unavailableMessage : 'Change not confirmed. Refresh the chat list before trying again.',
              ),
          ],
        ),
      ),
      actions: [
        TextButton(
          autofocus: widget.onRename == null,
          onPressed: _pending ? null : () => Navigator.pop(context),
          child: Text(_failed || widget.runImmediately ? 'Close' : 'Cancel'),
        ),
        if (!widget.runImmediately && !_failed)
          TextButton(
            onPressed:
                _pending ||
                    (widget.onRename != null &&
                        (_title.text.trim().isEmpty || _titleTooLong))
                ? null
                : _apply,
            child: Text(widget.action),
          ),
      ],
    ),
  );
}
