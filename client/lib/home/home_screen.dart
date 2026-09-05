import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:google_fonts/google_fonts.dart';
import 'package:smith_client/bridge/smith_bridge.dart';
import 'package:smith_client/chat/chat_controller.dart';
import 'package:smith_client/home/settings_tab.dart';
import 'package:smith_client/home/tasks_tab.dart';
import 'package:smith_client/listen/voice_turn.dart';

class HomeScreen extends StatefulWidget {
  const HomeScreen({
    super.key,
    required this.chat,
    required this.voice,
  });

  final ChatController chat;
  final VoiceTurnController voice;

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  final ScrollController _scroll = ScrollController();
  final FocusNode _focus = FocusNode();
  int _tab = 0;

  ChatController get chat => widget.chat;
  VoiceTurnController get voice => widget.voice;

  @override
  void initState() {
    super.initState();
    chat.addListener(_onChanged);
    voice.addListener(_onChanged);
    chat.start();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _focus.requestFocus();
    });
  }

  @override
  void dispose() {
    chat.removeListener(_onChanged);
    voice.removeListener(_onChanged);
    _scroll.dispose();
    _focus.dispose();
    super.dispose();
  }

  void _onChanged() {
    setState(() {});
    if (_tab != 0) return;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!_scroll.hasClients) return;
      _scroll.animateTo(
        _scroll.position.maxScrollExtent,
        duration: const Duration(milliseconds: 200),
        curve: Curves.easeOut,
      );
    });
  }

  Color get _statusColor {
    if (voice.statusGreen) return const Color(0xFF1FA88A);
    if (voice.statusBlue) return const Color(0xFF2F6FED);
    return const Color(0xFFB04A4A);
  }

  @override
  Widget build(BuildContext context) {
    final online = chat.bridgeStatus == BridgeStatus.online;
    const ink = Color(0xFF12353F);
    const muted = Color(0xFF6A7C86);
    const surface = Color(0xFFF4F6F8);

    return Focus(
      focusNode: _focus,
      autofocus: true,
      child: Scaffold(
        backgroundColor: surface,
        body: SafeArea(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 12, 12, 8),
                child: Row(
                  children: [
                    Text(
                      'smith',
                      style: GoogleFonts.fraunces(
                        fontSize: 22,
                        fontWeight: FontWeight.w600,
                        color: ink,
                        letterSpacing: -0.3,
                      ),
                    ),
                    const SizedBox(width: 8),
                    Container(
                      width: 7,
                      height: 7,
                      decoration: BoxDecoration(
                        shape: BoxShape.circle,
                        color: online
                            ? const Color(0xFF1FA88A)
                            : const Color(0xFFB04A4A),
                      ),
                    ),
                    if (chat.pollingTasks) ...[
                      const SizedBox(width: 10),
                      Text(
                        'working…',
                        style: GoogleFonts.dmSans(
                          fontSize: 11,
                          color: muted,
                        ),
                      ),
                    ],
                    const Spacer(),
                    IconButton(
                      tooltip: voice.ttsEnabled
                          ? 'Mute speech'
                          : 'Unmute speech',
                      onPressed: voice.toggleTts,
                      icon: Icon(
                        voice.ttsEnabled
                            ? Icons.volume_up_outlined
                            : Icons.volume_off_outlined,
                        color: voice.ttsEnabled ? ink : muted,
                        size: 22,
                      ),
                    ),
                    IconButton(
                      tooltip: 'New session',
                      onPressed: chat.busy
                          ? null
                          : () => chat.requestNewSession(),
                      icon: Icon(Icons.refresh, color: muted, size: 22),
                    ),
                  ],
                ),
              ),
              Expanded(
                child: IndexedStack(
                  index: _tab,
                  children: [
                    _ChatBody(
                      scroll: _scroll,
                      chat: chat,
                      voice: voice,
                      statusColor: _statusColor,
                    ),
                    TasksTab(chat: chat),
                    SettingsTab(chat: chat, voice: voice),
                  ],
                ),
              ),
            ],
          ),
        ),
        bottomNavigationBar: NavigationBar(
          selectedIndex: _tab,
          onDestinationSelected: (i) {
            setState(() => _tab = i);
            if (i == 1) {
              chat.refreshTasks();
            }
          },
          destinations: const [
            NavigationDestination(
              icon: Icon(Icons.chat_bubble_outline),
              selectedIcon: Icon(Icons.chat_bubble),
              label: 'Chat',
            ),
            NavigationDestination(
              icon: Icon(Icons.view_kanban_outlined),
              selectedIcon: Icon(Icons.view_kanban),
              label: 'Tasks',
            ),
            NavigationDestination(
              icon: Icon(Icons.settings_outlined),
              selectedIcon: Icon(Icons.settings),
              label: 'Settings',
            ),
          ],
        ),
      ),
    );
  }
}

class _ChatBody extends StatelessWidget {
  const _ChatBody({
    required this.scroll,
    required this.chat,
    required this.voice,
    required this.statusColor,
  });

  final ScrollController scroll;
  final ChatController chat;
  final VoiceTurnController voice;
  final Color statusColor;

  @override
  Widget build(BuildContext context) {
    const muted = Color(0xFF6A7C86);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Expanded(
          child: _MessagePane(
            scroll: scroll,
            chat: chat,
            voice: voice,
            onOpen: chat.openAttachment,
          ),
        ),
        if (voice.error != null)
          Padding(
            padding: const EdgeInsets.fromLTRB(24, 0, 24, 4),
            child: Text(
              voice.error!,
              textAlign: TextAlign.center,
              style: GoogleFonts.dmSans(
                fontSize: 12,
                color: Colors.red.shade700,
              ),
            ),
          ),
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 0, 16, 4),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Container(
                width: 8,
                height: 8,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: statusColor,
                  boxShadow: [
                    BoxShadow(
                      color: statusColor.withValues(alpha: 0.45),
                      blurRadius: 5,
                    ),
                  ],
                ),
              ),
              const SizedBox(width: 7),
              Text(
                voice.statusLabel,
                style: GoogleFonts.dmSans(
                  fontSize: 11,
                  fontWeight: FontWeight.w500,
                  color: muted,
                  letterSpacing: 0.2,
                ),
              ),
              if (voice.whisperOnline && voice.whisperModel != null) ...[
                const SizedBox(width: 10),
                Text(
                  voice.whisperModel!,
                  style: GoogleFonts.dmSans(
                    fontSize: 11,
                    color: muted.withValues(alpha: 0.85),
                  ),
                ),
              ],
            ],
          ),
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
          child: _InputBar(chat: chat, voice: voice),
        ),
      ],
    );
  }
}

/// ChatGPT/Claude-style input bar: text is primary, voice is a secondary,
/// optional affordance (never required — see docs/DESIGN.md).
class _InputBar extends StatefulWidget {
  const _InputBar({required this.chat, required this.voice});

  final ChatController chat;
  final VoiceTurnController voice;

  @override
  State<_InputBar> createState() => _InputBarState();
}

class _InputBarState extends State<_InputBar> {
  final TextEditingController _text = TextEditingController();
  final FocusNode _fieldFocus = FocusNode();

  @override
  void dispose() {
    _text.dispose();
    _fieldFocus.dispose();
    super.dispose();
  }

  void _submit() {
    final value = _text.text.trim();
    if (value.isEmpty || !widget.chat.canSend) return;
    _text.clear();
    unawaited(widget.chat.send(value));
    _fieldFocus.requestFocus();
  }

  @override
  Widget build(BuildContext context) {
    const ink = Color(0xFF12353F);
    const muted = Color(0xFF6A7C86);
    final chat = widget.chat;
    final voice = widget.voice;

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      decoration: BoxDecoration(
        color: Colors.white,
        borderRadius: BorderRadius.circular(24),
        border: Border.all(color: const Color(0xFFD0D8DE)),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          Expanded(
            child: TextField(
              controller: _text,
              focusNode: _fieldFocus,
              minLines: 1,
              maxLines: 6,
              textInputAction: TextInputAction.send,
              onSubmitted: (_) => _submit(),
              style: GoogleFonts.dmSans(fontSize: 15, color: ink),
              decoration: InputDecoration(
                border: InputBorder.none,
                isDense: true,
                contentPadding: const EdgeInsets.symmetric(
                  horizontal: 8,
                  vertical: 10,
                ),
                hintText: 'Message smith…',
                hintStyle: GoogleFonts.dmSans(fontSize: 15, color: muted),
              ),
            ),
          ),
          _CompactMicButton(voice: voice),
          IconButton(
            tooltip: 'Send',
            onPressed: chat.canSend ? _submit : null,
            icon: Icon(
              Icons.arrow_upward_rounded,
              color: chat.canSend ? ink : muted,
            ),
          ),
        ],
      ),
    );
  }
}

class _CompactMicButton extends StatelessWidget {
  const _CompactMicButton({required this.voice});

  final VoiceTurnController voice;

  @override
  Widget build(BuildContext context) {
    const ink = Color(0xFF12353F);
    const accent = Color(0xFF1A6B78);
    final holding = voice.holding;
    final capturing = voice.isRecording;
    final canStart = voice.canRecord;

    return Listener(
      onPointerDown: canStart ? (_) => voice.onPointerDown() : null,
      onPointerUp: capturing ? (_) => voice.onPointerUp() : null,
      onPointerCancel: capturing ? (_) => voice.onPointerUp() : null,
      child: Opacity(
        opacity: canStart || capturing ? 1 : 0.4,
        child: SizedBox(
          width: 40,
          height: 40,
          child: Stack(
            alignment: Alignment.center,
            children: [
              if (holding)
                SizedBox(
                  width: 36,
                  height: 36,
                  child: CircularProgressIndicator(
                    value: voice.holdProgress.clamp(0.0, 1.0),
                    strokeWidth: 3,
                    backgroundColor: const Color(0xFFD0D8DE),
                    color: accent,
                  ),
                ),
              Icon(
                holding || capturing ? Icons.mic : Icons.mic_none,
                color: holding ? accent : ink,
                size: 22,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _MessagePane extends StatelessWidget {
  const _MessagePane({
    required this.scroll,
    required this.chat,
    required this.voice,
    required this.onOpen,
  });

  final ScrollController scroll;
  final ChatController chat;
  final VoiceTurnController voice;
  final Future<void> Function(ChatAttachment att) onOpen;

  @override
  Widget build(BuildContext context) {
    return ListView.builder(
      controller: scroll,
      padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
      itemCount: chat.messages.length,
      itemBuilder: (context, index) {
        final msg = chat.messages[index];
        final who = switch (msg.role) {
          ChatRole.user => 'you',
          ChatRole.assistant => 'smith',
          ChatRole.system => '·',
        };
        return _Bubble(
          who: who,
          message: msg,
          playing: voice.isPlaying(msg.audioPath),
          onPlay: msg.audioPath == null || msg.audioPath!.isEmpty
              ? null
              : () => voice.togglePlay(msg.audioPath!),
          onOpen: onOpen,
        );
      },
    );
  }
}

class _Bubble extends StatelessWidget {
  const _Bubble({
    required this.who,
    required this.message,
    required this.playing,
    this.onPlay,
    this.onOpen,
  });

  final String who;
  final ChatMessage message;
  final bool playing;
  final VoidCallback? onPlay;
  final Future<void> Function(ChatAttachment att)? onOpen;

  @override
  Widget build(BuildContext context) {
    final italic = message.role == ChatRole.system;
    final markdown = message.role == ChatRole.assistant;
    final hasAudio =
        message.audioPath != null && message.audioPath!.isNotEmpty;
    final showText = message.text.isNotEmpty;
    const textColor = Color(0xFF15262D);

    return Padding(
      padding: const EdgeInsets.only(bottom: 14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            who,
            style: GoogleFonts.dmSans(
              fontSize: 10,
              fontWeight: FontWeight.w600,
              letterSpacing: 0.4,
              color: const Color(0xFF8A9AA3),
            ),
          ),
          const SizedBox(height: 3),
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              if (hasAudio) ...[
                IconButton(
                  tooltip: playing ? 'Stop clip' : 'Play clip',
                  padding: EdgeInsets.zero,
                  constraints: const BoxConstraints(
                    minWidth: 28,
                    minHeight: 28,
                  ),
                  onPressed: onPlay,
                  icon: Icon(
                    playing
                        ? Icons.stop_circle_outlined
                        : Icons.play_circle_outline,
                    size: 22,
                    color: const Color(0xFF12353F),
                  ),
                ),
                const SizedBox(width: 4),
              ],
              Expanded(
                child: !showText
                    ? const SizedBox.shrink()
                    : markdown
                    ? MarkdownBody(
                        data: message.text,
                        selectable: true,
                        styleSheet: MarkdownStyleSheet(
                          p: GoogleFonts.dmSans(
                            fontSize: 15,
                            height: 1.45,
                            color: textColor,
                          ),
                          code: GoogleFonts.dmSans(
                            fontSize: 13.5,
                            backgroundColor: const Color(0xFFEDF1F3),
                            color: textColor,
                          ),
                          codeblockDecoration: BoxDecoration(
                            color: const Color(0xFFEDF1F3),
                            borderRadius: BorderRadius.circular(8),
                          ),
                        ),
                      )
                    : SelectableText(
                        message.text,
                        style: GoogleFonts.dmSans(
                          fontSize: 15,
                          height: 1.45,
                          fontStyle:
                              italic ? FontStyle.italic : FontStyle.normal,
                          color: textColor,
                        ),
                      ),
              ),
              if (message.showSpinner) ...[
                const SizedBox(width: 8),
                const SizedBox(
                  width: 16,
                  height: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
              ],
              if (markdown && showText && !message.showSpinner) ...[
                const SizedBox(width: 4),
                IconButton(
                  tooltip: 'Copy',
                  padding: EdgeInsets.zero,
                  constraints: const BoxConstraints(
                    minWidth: 28,
                    minHeight: 28,
                  ),
                  onPressed: () => Clipboard.setData(
                    ClipboardData(text: message.text),
                  ),
                  icon: const Icon(
                    Icons.copy_outlined,
                    size: 16,
                    color: Color(0xFF8A9AA3),
                  ),
                ),
              ],
            ],
          ),
          if (message.attachments.isNotEmpty && onOpen != null)
            ...message.attachments.map(
              (att) => TextButton(
                onPressed: () => onOpen!(att),
                child: Text(att.name.isEmpty ? 'Open file' : att.name),
              ),
            ),
        ],
      ),
    );
  }
}

