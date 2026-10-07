import 'package:flutter/material.dart';
import 'package:smith_client/bridge/smith_bridge.dart';
import 'package:smith_client/chat/chat_controller.dart';
import 'package:smith_client/home/chat_view.dart';
import 'package:smith_client/home/halo_presentation.dart';
import 'package:smith_client/home/halo_theme.dart';
import 'package:smith_client/home/halo_widget.dart';
import 'package:smith_client/home/settings_tab.dart';
import 'package:smith_client/home/tasks_tab.dart';
import 'package:smith_client/listen/voice_turn.dart';

enum SmithDestination { home, chat, work, settings }

class HomeScreen extends StatefulWidget {
  const HomeScreen({super.key, required this.chat, required this.voice});

  final ChatController chat;
  final VoiceTurnController voice;

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  final TextEditingController _draft = TextEditingController();
  final ScrollController _chatScroll = ScrollController();
  SmithDestination _destination = SmithDestination.home;
  SmithDestination _lastPrimary = SmithDestination.home;
  int _lastMessageCount = 0;

  ChatController get chat => widget.chat;
  VoiceTurnController get voice => widget.voice;

  @override
  void initState() {
    super.initState();
    chat.addListener(_onChanged);
    voice.addListener(_onChanged);
    chat.start();
  }

  @override
  void dispose() {
    chat.removeListener(_onChanged);
    voice.removeListener(_onChanged);
    _draft.dispose();
    _chatScroll.dispose();
    super.dispose();
  }

  void _onChanged() {
    if (!mounted) return;
    final messageCount = chat.messages.length;
    final hasNewMessage = messageCount > _lastMessageCount;
    final wasNearLatest =
        !_chatScroll.hasClients ||
        shouldAutoScrollChat(
          previousMessageCount: _lastMessageCount,
          currentMessageCount: messageCount,
          extentAfter: _chatScroll.position.extentAfter,
        );
    _lastMessageCount = messageCount;
    setState(() {});
    if (_destination != SmithDestination.chat ||
        !hasNewMessage ||
        !wasNearLatest) {
      return;
    }
    _scrollChatToLatest(animate: true);
  }

  void _scrollChatToLatest({required bool animate}) {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_chatScroll.hasClients) return;
      if (!animate || MediaQuery.disableAnimationsOf(context)) {
        _chatScroll.jumpTo(_chatScroll.position.maxScrollExtent);
        return;
      }
      _chatScroll.animateTo(
        _chatScroll.position.maxScrollExtent,
        duration: const Duration(milliseconds: 180),
        curve: Curves.easeOut,
      );
    });
  }

  void _navigate(SmithDestination destination) {
    setState(() {
      _destination = destination;
      if (destination != SmithDestination.settings) {
        _lastPrimary = destination;
      }
    });
    if (destination == SmithDestination.work) {
      chat.refreshTasks();
    } else if (destination == SmithDestination.chat) {
      _scrollChatToLatest(animate: false);
    }
  }

  void _openTask(BoardTask task) {
    chat.selectTask(task);
    chat.openTaskDetail(task.id);
    _navigate(SmithDestination.work);
  }

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final width = constraints.maxWidth;
        if (width >= 1100) {
          return _DesktopShell(
            destination: _destination,
            chat: chat,
            voice: voice,
            draft: _draft,
            chatScroll: _chatScroll,
            onNavigate: _navigate,
            onOpenTask: _openTask,
          );
        }
        if (width >= 700) {
          return _RailShell(
            destination: _destination,
            chat: chat,
            voice: voice,
            draft: _draft,
            chatScroll: _chatScroll,
            onNavigate: _navigate,
            onOpenTask: _openTask,
          );
        }
        return _CompactShell(
          destination: _destination,
          lastPrimary: _lastPrimary,
          chat: chat,
          voice: voice,
          draft: _draft,
          chatScroll: _chatScroll,
          onNavigate: _navigate,
          onOpenTask: _openTask,
        );
      },
    );
  }
}

bool shouldAutoScrollChat({
  required int previousMessageCount,
  required int currentMessageCount,
  required double extentAfter,
}) => currentMessageCount > previousMessageCount && extentAfter < 120;

class _DesktopShell extends StatelessWidget {
  const _DesktopShell({
    required this.destination,
    required this.chat,
    required this.voice,
    required this.draft,
    required this.chatScroll,
    required this.onNavigate,
    required this.onOpenTask,
  });

  final SmithDestination destination;
  final ChatController chat;
  final VoiceTurnController voice;
  final TextEditingController draft;
  final ScrollController chatScroll;
  final ValueChanged<SmithDestination> onNavigate;
  final ValueChanged<BoardTask> onOpenTask;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: SafeArea(
        child: Row(
          children: [
            SizedBox(
              width: 200,
              child: _Sidebar(
                destination: destination,
                chat: chat,
                onNavigate: onNavigate,
              ),
            ),
            const VerticalDivider(width: 1),
            Expanded(
              child: Column(
                children: [
                  SizedBox(
                    height: 72,
                    child: _TopBar(
                      destination: destination,
                      chat: chat,
                      voice: voice,
                      onSettings: () => onNavigate(SmithDestination.settings),
                    ),
                  ),
                  const Divider(height: 1),
                  Expanded(
                    child: _DestinationStack(
                      destination: destination,
                      chat: chat,
                      voice: voice,
                      draft: draft,
                      chatScroll: chatScroll,
                      wide: true,
                      showSessions: true,
                      onNavigate: onNavigate,
                      onOpenTask: onOpenTask,
                    ),
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

class _RailShell extends StatelessWidget {
  const _RailShell({
    required this.destination,
    required this.chat,
    required this.voice,
    required this.draft,
    required this.chatScroll,
    required this.onNavigate,
    required this.onOpenTask,
  });

  final SmithDestination destination;
  final ChatController chat;
  final VoiceTurnController voice;
  final TextEditingController draft;
  final ScrollController chatScroll;
  final ValueChanged<SmithDestination> onNavigate;
  final ValueChanged<BoardTask> onOpenTask;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: SafeArea(
        child: Row(
          children: [
            SizedBox(
              width: 72,
              child: _CompactRail(
                destination: destination,
                onNavigate: onNavigate,
              ),
            ),
            const VerticalDivider(width: 1),
            Expanded(
              child: Column(
                children: [
                  SizedBox(
                    height: 72,
                    child: _TopBar(
                      destination: destination,
                      chat: chat,
                      voice: voice,
                      onSettings: () => onNavigate(SmithDestination.settings),
                    ),
                  ),
                  const Divider(height: 1),
                  Expanded(
                    child: _DestinationStack(
                      destination: destination,
                      chat: chat,
                      voice: voice,
                      draft: draft,
                      chatScroll: chatScroll,
                      wide: false,
                      showSessions: false,
                      onNavigate: onNavigate,
                      onOpenTask: onOpenTask,
                    ),
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

class _CompactShell extends StatelessWidget {
  const _CompactShell({
    required this.destination,
    required this.lastPrimary,
    required this.chat,
    required this.voice,
    required this.draft,
    required this.chatScroll,
    required this.onNavigate,
    required this.onOpenTask,
  });

  final SmithDestination destination;
  final SmithDestination lastPrimary;
  final ChatController chat;
  final VoiceTurnController voice;
  final TextEditingController draft;
  final ScrollController chatScroll;
  final ValueChanged<SmithDestination> onNavigate;
  final ValueChanged<BoardTask> onOpenTask;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(_destinationLabel(destination)),
        backgroundColor: HaloColors.background,
        surfaceTintColor: Colors.transparent,
        actions: [
          _BridgeBadge(status: chat.bridgeStatus, compact: true),
          IconButton(
            tooltip: 'Settings',
            onPressed: () => onNavigate(SmithDestination.settings),
            icon: const Icon(Icons.settings_outlined),
          ),
        ],
      ),
      body: SafeArea(
        top: false,
        child: _DestinationStack(
          destination: destination,
          chat: chat,
          voice: voice,
          draft: draft,
          chatScroll: chatScroll,
          wide: false,
          showSessions: false,
          onNavigate: onNavigate,
          onOpenTask: onOpenTask,
        ),
      ),
      bottomNavigationBar: NavigationBar(
        selectedIndex:
            (destination == SmithDestination.settings
                    ? lastPrimary
                    : destination)
                .index,
        onDestinationSelected: (index) =>
            onNavigate(SmithDestination.values[index]),
        destinations: const [
          NavigationDestination(
            icon: Icon(Icons.home_outlined),
            selectedIcon: Icon(Icons.home),
            label: 'Home',
          ),
          NavigationDestination(
            icon: Icon(Icons.message_outlined),
            selectedIcon: Icon(Icons.message),
            label: 'Chat',
          ),
          NavigationDestination(
            icon: Icon(Icons.work_outline),
            selectedIcon: Icon(Icons.work),
            label: 'Work',
          ),
        ],
      ),
    );
  }
}

class _DestinationStack extends StatelessWidget {
  const _DestinationStack({
    required this.destination,
    required this.chat,
    required this.voice,
    required this.draft,
    required this.chatScroll,
    required this.wide,
    required this.showSessions,
    required this.onNavigate,
    required this.onOpenTask,
  });

  final SmithDestination destination;
  final ChatController chat;
  final VoiceTurnController voice;
  final TextEditingController draft;
  final ScrollController chatScroll;
  final bool wide;
  final bool showSessions;
  final ValueChanged<SmithDestination> onNavigate;
  final ValueChanged<BoardTask> onOpenTask;

  @override
  Widget build(BuildContext context) {
    return PersistentDestinationStack(
      index: destination.index,
      children: [
        HaloHome(
          chat: chat,
          voice: voice,
          draft: draft,
          onOpenChat: () => onNavigate(SmithDestination.chat),
          onOpenWork: () => onNavigate(SmithDestination.work),
        ),
        ChatView(
          chat: chat,
          voice: voice,
          draft: draft,
          scrollController: chatScroll,
          onOpenTask: onOpenTask,
          showSessions: showSessions,
        ),
        TasksTab(
          chat: chat,
          wide: wide,
          onOpenChat: () => onNavigate(SmithDestination.chat),
        ),
        SettingsTab(chat: chat, voice: voice),
      ],
    );
  }
}

class PersistentDestinationStack extends StatelessWidget {
  const PersistentDestinationStack({
    super.key,
    required this.index,
    required this.children,
  });

  final int index;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return IndexedStack(index: index, children: children);
  }
}

class HaloHome extends StatefulWidget {
  const HaloHome({
    super.key,
    required this.chat,
    required this.voice,
    required this.draft,
    required this.onOpenChat,
    required this.onOpenWork,
  });

  final ChatController chat;
  final VoiceTurnController voice;
  final TextEditingController draft;
  final VoidCallback onOpenChat;
  final VoidCallback onOpenWork;

  @override
  State<HaloHome> createState() => _HaloHomeState();
}

class _HaloHomeState extends State<HaloHome> {
  bool _composerExpanded = false;

  @override
  Widget build(BuildContext context) {
    final state = deriveHaloState(
      bridgeStatus: widget.chat.bridgeStatus,
      voiceState: widget.voice.state,
      tasks: widget.chat.tasks,
      ttsActive: widget.voice.ttsSpeaking,
      chatBusy: widget.chat.busy,
      tasksStale: taskSnapshotIsStale(
        tasks: widget.chat.tasks,
        updatedAt: widget.chat.tasksUpdatedAt,
        refreshError: widget.chat.tasksError,
        now: DateTime.now(),
      ),
    );
    final working = widget.chat.tasks
        .where((task) => task.status == 'running')
        .length;
    final queued = widget.chat.tasks
        .where((task) => task.status == 'queued')
        .length;
    final ready = widget.chat.tasks
        .where((task) => task.status == 'awaiting_signoff')
        .length;
    final failed = widget.chat.tasks
        .where((task) => task.status == 'failed')
        .length;

    return LayoutBuilder(
      builder: (context, constraints) {
        final haloSize = constraints.maxHeight < 680 ? 150.0 : 220.0;
        return SingleChildScrollView(
          padding: const EdgeInsets.all(24),
          child: ConstrainedBox(
            constraints: BoxConstraints(
              minHeight: (constraints.maxHeight - 48).clamp(0, double.infinity),
            ),
            child: Center(
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 670),
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    SmithHalo(state: state, size: haloSize),
                    const SizedBox(height: 28),
                    Text(
                      haloGreeting(state),
                      textAlign: TextAlign.center,
                      style: Theme.of(context).textTheme.displaySmall?.copyWith(
                        fontSize: 36,
                        fontWeight: FontWeight.w500,
                        letterSpacing: -0.8,
                      ),
                    ),
                    const SizedBox(height: 10),
                    _StateLabel(state: state),
                    const SizedBox(height: 28),
                    Semantics(
                      button: true,
                      label:
                          '$working tasks working, $queued queued, $ready ready for review, $failed failed',
                      child: OutlinedButton.icon(
                        onPressed: widget.onOpenWork,
                        iconAlignment: IconAlignment.end,
                        icon: const Icon(Icons.arrow_forward, size: 18),
                        label: Text(
                          '$working working${queued > 0 ? '  ·  $queued queued' : ''}  ·  $ready ready${failed > 0 ? '  ·  $failed failed' : ''}',
                        ),
                      ),
                    ),
                    const SizedBox(height: 32),
                    HomeControlCluster(
                      speakerControl: IconButton(
                        tooltip: widget.voice.ttsEnabled
                            ? 'Mute spoken replies'
                            : 'Enable spoken replies',
                        onPressed: widget.voice.toggleTts,
                        icon: Icon(
                          widget.voice.ttsEnabled
                              ? Icons.volume_up_outlined
                              : Icons.volume_off_outlined,
                        ),
                      ),
                      composer: SmithComposer(
                        chat: widget.chat,
                        voice: widget.voice,
                        draft: widget.draft,
                        expanded: _composerExpanded,
                        onExpandedChanged: (value) =>
                            setState(() => _composerExpanded = value),
                      ),
                    ),
                    if (widget.voice.error != null) ...[
                      const SizedBox(height: 12),
                      VoiceErrorNotice(
                        message: widget.voice.error!,
                        onRetry: widget.voice.refreshWhisperHealth,
                      ),
                    ],
                    const SizedBox(height: 16),
                    TextButton.icon(
                      onPressed: widget.onOpenChat,
                      iconAlignment: IconAlignment.end,
                      icon: const Icon(Icons.arrow_outward, size: 18),
                      label: const Text('Open chat'),
                    ),
                    const SizedBox(height: 28),
                    Text(
                      'Voice and your draft stay available as you move between views.',
                      textAlign: TextAlign.center,
                      style: Theme.of(context).textTheme.bodySmall?.copyWith(
                        color: HaloColors.secondary,
                      ),
                    ),
                    if (widget.chat.tasksError != null) ...[
                      const SizedBox(height: 12),
                      const Text(
                        'Task status may be stale. Open Work to retry.',
                        style: TextStyle(color: HaloColors.warning),
                      ),
                    ],
                  ],
                ),
              ),
            ),
          ),
        );
      },
    );
  }
}

class HomeControlCluster extends StatelessWidget {
  const HomeControlCluster({
    super.key,
    required this.speakerControl,
    required this.composer,
  });

  final Widget speakerControl;
  final Widget composer;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        if (constraints.maxWidth < 520) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Align(alignment: Alignment.centerLeft, child: speakerControl),
              const SizedBox(height: 8),
              composer,
            ],
          );
        }
        return Row(
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            speakerControl,
            const SizedBox(width: 12),
            Expanded(child: composer),
          ],
        );
      },
    );
  }
}

class _StateLabel extends StatelessWidget {
  const _StateLabel({required this.state});

  final HaloState state;

  @override
  Widget build(BuildContext context) {
    final color = switch (state) {
      HaloState.failed => HaloColors.danger,
      HaloState.review => HaloColors.warning,
      HaloState.disconnected || HaloState.unknown => HaloColors.secondary,
      _ => HaloColors.accent,
    };
    return Semantics(
      liveRegion: true,
      label: 'Smith is ${haloStateLabel(state)}',
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.circle, size: 10, color: color),
          const SizedBox(width: 9),
          Text(
            haloStateLabel(state),
            style: Theme.of(
              context,
            ).textTheme.bodyLarge?.copyWith(color: HaloColors.secondary),
          ),
        ],
      ),
    );
  }
}

class _Sidebar extends StatelessWidget {
  const _Sidebar({
    required this.destination,
    required this.chat,
    required this.onNavigate,
  });

  final SmithDestination destination;
  final ChatController chat;
  final ValueChanged<SmithDestination> onNavigate;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: const BoxDecoration(color: HaloColors.background),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 28, 16, 20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12),
              child: Text(
                'S M I T H',
                style: Theme.of(context).textTheme.titleLarge?.copyWith(
                  fontWeight: FontWeight.w600,
                  letterSpacing: 3.5,
                ),
              ),
            ),
            const SizedBox(height: 36),
            _SideItem(
              destination: SmithDestination.home,
              selected: destination == SmithDestination.home,
              icon: Icons.home_outlined,
              onTap: onNavigate,
            ),
            const SizedBox(height: 8),
            _SideItem(
              destination: SmithDestination.chat,
              selected: destination == SmithDestination.chat,
              icon: Icons.message_outlined,
              onTap: onNavigate,
            ),
            const SizedBox(height: 8),
            _SideItem(
              destination: SmithDestination.work,
              selected: destination == SmithDestination.work,
              icon: Icons.work_outline,
              onTap: onNavigate,
            ),
            const Spacer(),
            _BridgeBadge(status: chat.bridgeStatus),
            const SizedBox(height: 14),
            const Divider(),
            const SizedBox(height: 10),
            _SideItem(
              destination: SmithDestination.settings,
              selected: destination == SmithDestination.settings,
              icon: Icons.settings_outlined,
              onTap: onNavigate,
            ),
          ],
        ),
      ),
    );
  }
}

class _SideItem extends StatelessWidget {
  const _SideItem({
    required this.destination,
    required this.selected,
    required this.icon,
    required this.onTap,
  });

  final SmithDestination destination;
  final bool selected;
  final IconData icon;
  final ValueChanged<SmithDestination> onTap;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      selected: selected,
      button: true,
      child: InkWell(
        onTap: () => onTap(destination),
        borderRadius: BorderRadius.circular(12),
        child: Container(
          constraints: const BoxConstraints(minHeight: 52),
          padding: const EdgeInsets.symmetric(horizontal: 14),
          decoration: BoxDecoration(
            color: selected
                ? HaloColors.accent.withValues(alpha: 0.10)
                : Colors.transparent,
            borderRadius: BorderRadius.circular(12),
            border: selected
                ? Border.all(color: HaloColors.border)
                : Border.all(color: Colors.transparent),
          ),
          child: Row(
            children: [
              Icon(
                icon,
                color: selected ? HaloColors.accent : HaloColors.primary,
              ),
              const SizedBox(width: 14),
              Text(
                _destinationLabel(destination),
                style: TextStyle(
                  color: selected ? HaloColors.accent : HaloColors.primary,
                  fontWeight: selected ? FontWeight.w600 : FontWeight.w400,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _CompactRail extends StatelessWidget {
  const _CompactRail({required this.destination, required this.onNavigate});

  final SmithDestination destination;
  final ValueChanged<SmithDestination> onNavigate;

  @override
  Widget build(BuildContext context) {
    const items = [
      (SmithDestination.home, Icons.home_outlined),
      (SmithDestination.chat, Icons.message_outlined),
      (SmithDestination.work, Icons.work_outline),
    ];
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 12),
      child: Column(
        children: [
          const Padding(
            padding: EdgeInsets.symmetric(vertical: 12),
            child: Text('S', style: TextStyle(fontSize: 22, letterSpacing: 2)),
          ),
          const SizedBox(height: 12),
          ...items.map(
            (item) => _RailButton(
              destination: item.$1,
              icon: item.$2,
              selected: destination == item.$1,
              onTap: onNavigate,
            ),
          ),
          const Spacer(),
          _RailButton(
            destination: SmithDestination.settings,
            icon: Icons.settings_outlined,
            selected: destination == SmithDestination.settings,
            onTap: onNavigate,
          ),
        ],
      ),
    );
  }
}

class _RailButton extends StatelessWidget {
  const _RailButton({
    required this.destination,
    required this.icon,
    required this.selected,
    required this.onTap,
  });

  final SmithDestination destination;
  final IconData icon;
  final bool selected;
  final ValueChanged<SmithDestination> onTap;

  @override
  Widget build(BuildContext context) {
    final label = _destinationLabel(destination);
    return Tooltip(
      message: label,
      child: Semantics(
        label: label,
        selected: selected,
        button: true,
        child: Padding(
          padding: const EdgeInsets.symmetric(vertical: 4),
          child: IconButton(
            onPressed: () => onTap(destination),
            style: IconButton.styleFrom(
              backgroundColor: selected
                  ? HaloColors.accent.withValues(alpha: 0.12)
                  : Colors.transparent,
              foregroundColor: selected
                  ? HaloColors.accent
                  : HaloColors.primary,
            ),
            icon: Icon(icon),
          ),
        ),
      ),
    );
  }
}

class _TopBar extends StatelessWidget {
  const _TopBar({
    required this.destination,
    required this.chat,
    required this.voice,
    required this.onSettings,
  });

  final SmithDestination destination;
  final ChatController chat;
  final VoiceTurnController voice;
  final VoidCallback onSettings;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 24),
      child: Row(
        children: [
          Text(
            _destinationLabel(destination),
            style: Theme.of(
              context,
            ).textTheme.headlineSmall?.copyWith(fontWeight: FontWeight.w500),
          ),
          const Spacer(),
          _BridgeBadge(status: chat.bridgeStatus, compact: true),
          const SizedBox(width: 14),
          const SmithHalo(
            state: HaloState.idle,
            size: 34,
            semanticStatus: false,
          ),
          const SizedBox(width: 9),
          const Text('Smith'),
          const SizedBox(width: 6),
          IconButton(
            tooltip: voice.ttsEnabled
                ? 'Mute spoken replies'
                : 'Enable spoken replies',
            onPressed: voice.toggleTts,
            icon: Icon(
              voice.ttsEnabled
                  ? Icons.volume_up_outlined
                  : Icons.volume_off_outlined,
              size: 20,
            ),
          ),
          IconButton(
            tooltip: 'Settings',
            onPressed: onSettings,
            icon: const Icon(Icons.settings_outlined, size: 20),
          ),
        ],
      ),
    );
  }
}

class _BridgeBadge extends StatelessWidget {
  const _BridgeBadge({required this.status, this.compact = false});

  final BridgeStatus status;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final label = switch (status) {
      BridgeStatus.online => 'Local · Connected',
      BridgeStatus.offline => 'Local · Offline',
      BridgeStatus.unknown => 'Local · Checking',
    };
    final color = switch (status) {
      BridgeStatus.online => HaloColors.accent,
      BridgeStatus.offline => HaloColors.danger,
      BridgeStatus.unknown => HaloColors.secondary,
    };
    return Semantics(
      label: 'Bridge status: $label',
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.circle, size: 10, color: color),
          if (!compact) ...[
            const SizedBox(width: 8),
            Flexible(
              child: Text(
                label,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(
                  color: HaloColors.secondary,
                  fontSize: 13,
                ),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

String _destinationLabel(SmithDestination destination) => switch (destination) {
  SmithDestination.home => 'Home',
  SmithDestination.chat => 'Chat',
  SmithDestination.work => 'Work',
  SmithDestination.settings => 'Settings',
};
