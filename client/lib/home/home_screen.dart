import 'package:flutter/material.dart';
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
              if (_tab == 0 && chat.pipeline != null)
                Padding(
                  padding: const EdgeInsets.fromLTRB(16, 0, 16, 6),
                  child: Text(
                    'pipeline: ${chat.pipeline!.shortLabel}'
                    '${chat.pipeline!.summary.isNotEmpty ? ' — ${chat.pipeline!.summary}' : ''}',
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: GoogleFonts.dmSans(
                      fontSize: 11,
                      color: muted,
                    ),
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
          padding: const EdgeInsets.fromLTRB(16, 4, 16, 8),
          child: Center(
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Container(
                  width: 10,
                  height: 10,
                  decoration: BoxDecoration(
                    shape: BoxShape.circle,
                    color: statusColor,
                    boxShadow: [
                      BoxShadow(
                        color: statusColor.withValues(alpha: 0.45),
                        blurRadius: 6,
                      ),
                    ],
                  ),
                ),
                const SizedBox(width: 8),
                Text(
                  voice.statusLabel,
                  style: GoogleFonts.dmSans(
                    fontSize: 12,
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
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
          child: Center(
            child: _HoldTalkButton(
              holding: voice.holding,
              progress: voice.holdProgress,
              canStart: voice.canRecord,
              capturing: voice.isRecording,
              onPointerDown: voice.onPointerDown,
              onPointerUp: voice.onPointerUp,
            ),
          ),
        ),
      ],
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
    final hasAudio =
        message.audioPath != null && message.audioPath!.isNotEmpty;
    final showText = message.text.isNotEmpty;

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
                child: showText
                    ? SelectableText(
                        message.text,
                        style: GoogleFonts.dmSans(
                          fontSize: 15,
                          height: 1.45,
                          fontStyle:
                              italic ? FontStyle.italic : FontStyle.normal,
                          color: const Color(0xFF15262D),
                        ),
                      )
                    : const SizedBox.shrink(),
              ),
              if (message.showSpinner) ...[
                const SizedBox(width: 8),
                const SizedBox(
                  width: 16,
                  height: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
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

/// Circular hold-to-talk. Fill ring maps 0→1 over [kMaxHoldRecord].
class _HoldTalkButton extends StatelessWidget {
  const _HoldTalkButton({
    required this.holding,
    required this.progress,
    required this.canStart,
    required this.capturing,
    required this.onPointerDown,
    required this.onPointerUp,
  });

  final bool holding;
  final double progress;
  final bool canStart;
  final bool capturing;
  final VoidCallback onPointerDown;
  final VoidCallback onPointerUp;

  @override
  Widget build(BuildContext context) {
    const ink = Color(0xFF12353F);
    const accent = Color(0xFF1A6B78);
    const size = 88.0;

    return Listener(
      onPointerDown: canStart ? (_) => onPointerDown() : null,
      onPointerUp: capturing ? (_) => onPointerUp() : null,
      onPointerCancel: capturing ? (_) => onPointerUp() : null,
      child: Opacity(
        opacity: canStart || capturing ? 1 : 0.45,
        child: SizedBox(
          width: size,
          height: size,
          child: Stack(
            alignment: Alignment.center,
            children: [
              SizedBox(
                width: size,
                height: size,
                child: CircularProgressIndicator(
                  value: holding ? progress.clamp(0.0, 1.0) : 0,
                  strokeWidth: 5,
                  backgroundColor: const Color(0xFFD0D8DE),
                  color: holding ? accent : ink,
                ),
              ),
              AnimatedContainer(
                duration: const Duration(milliseconds: 120),
                width: 68,
                height: 68,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: holding ? ink : Colors.white,
                  border: Border.all(
                    color: holding ? ink : const Color(0xFFD0D8DE),
                    width: 1.5,
                  ),
                  boxShadow: holding
                      ? const [
                          BoxShadow(
                            color: Color(0x3312353F),
                            blurRadius: 12,
                            offset: Offset(0, 4),
                          ),
                        ]
                      : null,
                ),
                child: Icon(
                  holding || capturing ? Icons.mic : Icons.mic_none,
                  color: holding ? Colors.white : ink,
                  size: 28,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
