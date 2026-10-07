import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:smith_client/bridge/smith_bridge.dart';
import 'package:smith_client/chat/chat_controller.dart';
import 'package:smith_client/home/halo_presentation.dart';
import 'package:smith_client/home/halo_theme.dart';
import 'package:smith_client/home/halo_widget.dart';
import 'package:smith_client/listen/voice_turn.dart';

class ChatView extends StatelessWidget {
  const ChatView({
    super.key,
    required this.chat,
    required this.voice,
    required this.draft,
    required this.scrollController,
    required this.onOpenTask,
    required this.showSessions,
  });

  final ChatController chat;
  final VoiceTurnController voice;
  final TextEditingController draft;
  final ScrollController scrollController;
  final ValueChanged<BoardTask> onOpenTask;
  final bool showSessions;

  @override
  Widget build(BuildContext context) {
    final conversation = Column(
      children: [
        _ConversationHeader(chat: chat),
        Expanded(
          child: Align(
            alignment: Alignment.topCenter,
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 840),
              child: _MessageList(
                chat: chat,
                voice: voice,
                controller: scrollController,
              ),
            ),
          ),
        ),
        if (chat.tasks.isNotEmpty)
          Align(
            alignment: Alignment.center,
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 840),
              child: _TaskHandoff(
                task: chat.tasks.first,
                onOpen: () => onOpenTask(chat.tasks.first),
              ),
            ),
          ),
        Align(
          alignment: Alignment.center,
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 840),
            child: Padding(
              padding: const EdgeInsets.fromLTRB(24, 8, 24, 20),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  _VoiceStatus(voice: voice),
                  if (voice.error != null) ...[
                    const SizedBox(height: 8),
                    VoiceErrorNotice(
                      message: voice.error!,
                      onRetry: voice.refreshWhisperHealth,
                    ),
                  ],
                  const SizedBox(height: 8),
                  SmithComposer(chat: chat, voice: voice, draft: draft),
                ],
              ),
            ),
          ),
        ),
      ],
    );

    if (!showSessions) return conversation;
    return Row(
      children: [
        SizedBox(
          width: 220,
          child: _SessionPane(
            onNewSession: chat.busy ? null : chat.requestNewSession,
          ),
        ),
        const VerticalDivider(width: 1),
        Expanded(child: conversation),
      ],
    );
  }
}

class _SessionPane extends StatelessWidget {
  const _SessionPane({required this.onNewSession});

  final VoidCallback? onNewSession;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      label: 'Chat sessions',
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            OutlinedButton.icon(
              onPressed: onNewSession,
              icon: const Icon(Icons.add, size: 18),
              label: const Text('New session'),
            ),
            const SizedBox(height: 24),
            Text(
              'CURRENT',
              style: Theme.of(context).textTheme.labelSmall?.copyWith(
                color: HaloColors.secondary,
                letterSpacing: 1.2,
              ),
            ),
            const SizedBox(height: 8),
            Container(
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: HaloColors.accent.withValues(alpha: 0.08),
                borderRadius: BorderRadius.circular(12),
                border: Border.all(color: HaloColors.border),
              ),
              child: const Row(
                children: [
                  Icon(Icons.message_outlined, color: HaloColors.accent),
                  SizedBox(width: 12),
                  Expanded(child: Text('Current session')),
                ],
              ),
            ),
            const SizedBox(height: 12),
            Text(
              'Session history is not available from the current local bridge.',
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: HaloColors.secondary,
                height: 1.4,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _ConversationHeader extends StatelessWidget {
  const _ConversationHeader({required this.chat});

  final ChatController chat;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(24, 18, 24, 8),
      child: Row(
        children: [
          const SmithHalo(
            state: HaloState.idle,
            size: 32,
            semanticStatus: false,
          ),
          const SizedBox(width: 12),
          const Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text('Current session'),
                Text(
                  'Voice messages are sent here',
                  style: TextStyle(color: HaloColors.secondary, fontSize: 13),
                ),
              ],
            ),
          ),
          TextButton.icon(
            onPressed: chat.busy ? null : chat.requestNewSession,
            icon: const Icon(Icons.refresh, size: 18),
            label: const Text('Reset'),
          ),
        ],
      ),
    );
  }
}

class _MessageList extends StatelessWidget {
  const _MessageList({
    required this.chat,
    required this.voice,
    required this.controller,
  });

  final ChatController chat;
  final VoiceTurnController voice;
  final ScrollController controller;

  @override
  Widget build(BuildContext context) {
    if (chat.messages.isEmpty) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(32),
          child: Text(
            chat.bridgeStatus == BridgeStatus.offline
                ? 'Smith bridge is offline. Your draft is preserved.'
                : 'Start with a bounded objective. Smith will keep resulting work visible.',
            textAlign: TextAlign.center,
            style: Theme.of(
              context,
            ).textTheme.bodyLarge?.copyWith(color: HaloColors.secondary),
          ),
        ),
      );
    }

    return ListView.builder(
      controller: controller,
      padding: const EdgeInsets.fromLTRB(24, 16, 24, 12),
      itemCount: chat.messages.length,
      itemBuilder: (context, index) => _MessageRow(
        message: chat.messages[index],
        voice: voice,
        onOpen: chat.openAttachment,
      ),
    );
  }
}

class _MessageRow extends StatelessWidget {
  const _MessageRow({
    required this.message,
    required this.voice,
    required this.onOpen,
  });

  final ChatMessage message;
  final VoiceTurnController voice;
  final Future<void> Function(ChatAttachment) onOpen;

  String get _phase => switch (message.phase) {
    MessagePhase.awaitingTranscript => 'Transcribing',
    MessagePhase.typing => 'Transcript received',
    MessagePhase.sent => 'Sent',
    MessagePhase.awaitingReply => 'Waiting for Smith',
    MessagePhase.revealing => 'Response received',
    MessagePhase.complete => '',
  };

  @override
  Widget build(BuildContext context) {
    final isSystem = message.role == ChatRole.system;
    final name = switch (message.role) {
      ChatRole.user => 'YOU',
      ChatRole.assistant => 'SMITH',
      ChatRole.system => 'SYSTEM',
    };
    return Semantics(
      label: '$name message${_phase.isEmpty ? '' : ', $_phase'}',
      child: Padding(
        padding: const EdgeInsets.only(bottom: 24),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            if (message.role == ChatRole.assistant) ...[
              const Padding(
                padding: EdgeInsets.only(top: 2),
                child: SmithHalo(
                  state: HaloState.idle,
                  size: 28,
                  semanticStatus: false,
                ),
              ),
              const SizedBox(width: 12),
            ],
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Text(
                        name,
                        style: Theme.of(context).textTheme.labelSmall?.copyWith(
                          color: message.role == ChatRole.assistant
                              ? HaloColors.accent
                              : HaloColors.secondary,
                          letterSpacing: 1.1,
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                      if (_phase.isNotEmpty) ...[
                        const SizedBox(width: 8),
                        Text(
                          '· $_phase',
                          style: Theme.of(context).textTheme.labelSmall
                              ?.copyWith(color: HaloColors.secondary),
                        ),
                      ],
                    ],
                  ),
                  const SizedBox(height: 6),
                  if (message.text.isEmpty && message.showSpinner)
                    const SizedBox.square(
                      dimension: 18,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  else if (message.role == ChatRole.assistant)
                    MarkdownBody(
                      data: message.text,
                      selectable: true,
                      styleSheet: MarkdownStyleSheet(
                        p: Theme.of(
                          context,
                        ).textTheme.bodyLarge?.copyWith(height: 1.55),
                        code: Theme.of(context).textTheme.bodyMedium?.copyWith(
                          color: HaloColors.primary,
                          backgroundColor: HaloColors.surface,
                        ),
                        codeblockDecoration: BoxDecoration(
                          color: HaloColors.surface,
                          borderRadius: BorderRadius.circular(12),
                          border: Border.all(color: HaloColors.border),
                        ),
                      ),
                    )
                  else
                    SelectableText(
                      message.text,
                      style: Theme.of(context).textTheme.bodyLarge?.copyWith(
                        height: 1.55,
                        color: isSystem
                            ? HaloColors.secondary
                            : HaloColors.primary,
                        fontStyle: isSystem ? FontStyle.italic : null,
                      ),
                    ),
                  if (message.audioPath?.isNotEmpty == true)
                    TextButton.icon(
                      onPressed: () => voice.togglePlay(message.audioPath!),
                      icon: Icon(
                        voice.isPlaying(message.audioPath)
                            ? Icons.stop_circle_outlined
                            : Icons.play_circle_outline,
                      ),
                      label: Text(
                        voice.isPlaying(message.audioPath)
                            ? 'Stop recording'
                            : 'Play recording',
                      ),
                    ),
                  ...message.attachments.map(
                    (attachment) => TextButton.icon(
                      onPressed: () => onOpen(attachment),
                      icon: const Icon(Icons.attach_file, size: 18),
                      label: Text(
                        attachment.name.isEmpty ? 'Open file' : attachment.name,
                      ),
                    ),
                  ),
                  if (message.role == ChatRole.assistant &&
                      message.text.isNotEmpty &&
                      !message.showSpinner)
                    IconButton(
                      tooltip: 'Copy response',
                      onPressed: () =>
                          Clipboard.setData(ClipboardData(text: message.text)),
                      icon: const Icon(Icons.copy_outlined, size: 18),
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

class _TaskHandoff extends StatelessWidget {
  const _TaskHandoff({required this.task, required this.onOpen});

  final BoardTask task;
  final VoidCallback onOpen;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 24),
      child: Card(
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Row(
            children: [
              const Icon(Icons.work_outline, color: HaloColors.accent),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(task.title),
                    const SizedBox(height: 2),
                    Text(
                      '${task.id} · ${task.status}',
                      style: const TextStyle(
                        color: HaloColors.secondary,
                        fontSize: 13,
                      ),
                    ),
                  ],
                ),
              ),
              TextButton.icon(
                onPressed: onOpen,
                iconAlignment: IconAlignment.end,
                icon: const Icon(Icons.arrow_outward, size: 18),
                label: const Text('View task'),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _VoiceStatus extends StatelessWidget {
  const _VoiceStatus({required this.voice});

  final VoiceTurnController voice;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      label: 'Voice status: ${voice.statusLabel}',
      child: Row(
        children: [
          Icon(
            voice.whisperOnline
                ? Icons.check_circle_outline
                : Icons.mic_off_outlined,
            size: 16,
            color: voice.whisperOnline
                ? HaloColors.accent
                : HaloColors.secondary,
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              voice.whisperOnline
                  ? 'Voice connected · ${voice.statusLabel}'
                  : 'Voice unavailable · text chat remains available',
              style: Theme.of(
                context,
              ).textTheme.bodySmall?.copyWith(color: HaloColors.secondary),
            ),
          ),
        ],
      ),
    );
  }
}

class SmithComposer extends StatefulWidget {
  const SmithComposer({
    super.key,
    required this.chat,
    required this.voice,
    required this.draft,
    this.expanded = true,
    this.onExpandedChanged,
    this.recordingControl,
  });

  final ChatController chat;
  final VoiceTurnController? voice;
  final TextEditingController draft;
  final bool expanded;
  final ValueChanged<bool>? onExpandedChanged;
  final Widget? recordingControl;

  @override
  State<SmithComposer> createState() => _SmithComposerState();
}

class _SmithComposerState extends State<SmithComposer> {
  final FocusNode _focus = FocusNode();

  @override
  void dispose() {
    _focus.dispose();
    super.dispose();
  }

  void _submit() {
    final text = widget.draft.text.trim();
    if (text.isEmpty || !widget.chat.canSend) return;
    widget.draft.clear();
    unawaited(widget.chat.send(text));
    _focus.requestFocus();
  }

  @override
  Widget build(BuildContext context) {
    final expanded = widget.expanded;
    return Shortcuts(
      shortcuts: const {
        SingleActivator(LogicalKeyboardKey.escape): _CollapseIntent(),
      },
      child: Actions(
        actions: {
          _CollapseIntent: CallbackAction<_CollapseIntent>(
            onInvoke: (_) {
              widget.onExpandedChanged?.call(false);
              return null;
            },
          ),
        },
        child: Container(
          padding: const EdgeInsets.all(8),
          decoration: BoxDecoration(
            color: HaloColors.surface,
            borderRadius: BorderRadius.circular(12),
            border: Border.all(color: HaloColors.border),
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Expanded(
                child: TextField(
                  key: const Key('shared-composer-field'),
                  controller: widget.draft,
                  focusNode: _focus,
                  minLines: expanded ? 2 : 1,
                  maxLines: expanded ? 6 : 1,
                  textInputAction: expanded
                      ? TextInputAction.newline
                      : TextInputAction.send,
                  onTap: () => widget.onExpandedChanged?.call(true),
                  onSubmitted: expanded ? null : (_) => _submit(),
                  decoration: const InputDecoration(
                    hintText: 'Type to Smith…',
                    filled: false,
                    border: InputBorder.none,
                    enabledBorder: InputBorder.none,
                    focusedBorder: InputBorder.none,
                    contentPadding: EdgeInsets.symmetric(
                      horizontal: 8,
                      vertical: 12,
                    ),
                  ),
                ),
              ),
              if (expanded)
                IconButton(
                  tooltip:
                      'Attachments can be opened from replies; upload is unavailable',
                  onPressed: null,
                  icon: const Icon(Icons.attach_file),
                ),
              widget.recordingControl ?? HoldToTalkButton(voice: widget.voice!),
              IconButton.filled(
                tooltip: widget.chat.canSend
                    ? 'Send message'
                    : 'Wait for the current request',
                onPressed: widget.chat.canSend ? _submit : null,
                icon: const Icon(Icons.arrow_upward),
              ),
              if (widget.onExpandedChanged != null)
                IconButton(
                  tooltip: expanded ? 'Collapse composer' : 'Expand composer',
                  onPressed: () => widget.onExpandedChanged?.call(!expanded),
                  icon: Icon(
                    expanded
                        ? Icons.keyboard_arrow_down
                        : Icons.keyboard_arrow_up,
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

class HoldToTalkButton extends StatelessWidget {
  const HoldToTalkButton({super.key, required this.voice});

  final VoiceTurnController voice;

  @override
  Widget build(BuildContext context) {
    return RecordingControlButton(
      active: voice.isRecording,
      enabled: voice.canRecord || voice.isRecording,
      onStart: voice.onPointerDown,
      onStop: voice.onPointerUp,
    );
  }
}

class RecordingControlButton extends StatelessWidget {
  const RecordingControlButton({
    super.key,
    required this.active,
    required this.enabled,
    required this.onStart,
    required this.onStop,
    this.focusNode,
  });

  final bool active;
  final bool enabled;
  final VoidCallback onStart;
  final VoidCallback onStop;
  final FocusNode? focusNode;

  @override
  Widget build(BuildContext context) {
    final action = active ? onStop : onStart;
    final label = active ? 'Stop recording' : 'Start recording';
    return GestureDetector(
      onLongPressStart: enabled && !active ? (_) => onStart() : null,
      onLongPressEnd: active ? (_) => onStop() : null,
      child: IconButton(
        key: const Key('recording-control'),
        focusNode: focusNode,
        tooltip: enabled ? label : 'Voice is currently unavailable',
        onPressed: enabled ? action : null,
        style: IconButton.styleFrom(
          backgroundColor: active
              ? HaloColors.accent.withValues(alpha: 0.14)
              : Colors.transparent,
        ),
        icon: Icon(active ? Icons.mic : Icons.mic_none),
      ),
    );
  }
}

class VoiceErrorNotice extends StatelessWidget {
  const VoiceErrorNotice({
    super.key,
    required this.message,
    required this.onRetry,
  });

  final String message;
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      liveRegion: true,
      label: 'Voice error: $message',
      child: Container(
        padding: const EdgeInsets.all(12),
        decoration: BoxDecoration(
          color: HaloColors.danger.withValues(alpha: 0.08),
          border: Border.all(color: HaloColors.danger.withValues(alpha: 0.45)),
          borderRadius: BorderRadius.circular(12),
        ),
        child: Row(
          children: [
            const Icon(Icons.mic_off_outlined, color: HaloColors.danger),
            const SizedBox(width: 10),
            Expanded(
              child: Text(
                message,
                style: const TextStyle(color: HaloColors.primary),
              ),
            ),
            TextButton(onPressed: onRetry, child: const Text('Retry')),
          ],
        ),
      ),
    );
  }
}

class _CollapseIntent extends Intent {
  const _CollapseIntent();
}
