import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:smith_client/chat/chat_controller.dart';
import 'package:smith_client/home/halo_presentation.dart';
import 'package:smith_client/home/halo_theme.dart';
import 'package:smith_client/home/halo_widget.dart';

/// Release surface. Legacy Work and voice screens are intentionally not mounted.
class OfflineChatScreen extends StatefulWidget {
  const OfflineChatScreen({super.key, required this.chat});

  final ChatController chat;

  @override
  State<OfflineChatScreen> createState() => _OfflineChatScreenState();
}

class _OfflineChatScreenState extends State<OfflineChatScreen> {
  final _draft = TextEditingController();
  final _scroll = ScrollController();
  int _destination = 0;
  int _lastPrimary = 0;
  int _messageCount = 0;

  ChatController get chat => widget.chat;

  @override
  void initState() {
    super.initState();
    assert(
      chat.chatOnly,
      'The release surface requires a chat-only controller',
    );
    chat.addListener(_changed);
    unawaited(chat.start());
  }

  @override
  void dispose() {
    chat.removeListener(_changed);
    _draft.dispose();
    _scroll.dispose();
    super.dispose();
  }

  void _changed() {
    if (!mounted) return;
    final added = chat.messages.length > _messageCount;
    final nearLatest =
        !_scroll.hasClients || _scroll.position.extentAfter < 120;
    _messageCount = chat.messages.length;
    setState(() {});
    if (_destination == 1 && added && nearLatest) _showLatest();
  }

  void _showLatest() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && _scroll.hasClients) {
        _scroll.jumpTo(_scroll.position.maxScrollExtent);
      }
    });
  }

  void _navigate(int index) {
    setState(() {
      _destination = index;
      if (index < 2) _lastPrimary = index;
    });
  }

  void _submit() {
    final text = _draft.text.trim();
    if (text.isEmpty || utf8.encode(text).length > 16384 || !chat.canSend) {
      return;
    }
    _draft.clear();
    _navigate(1);
    unawaited(chat.send(text));
    _showLatest();
  }

  Future<void> _newChat() async {
    if (chat.busy) return;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Start a new chat?'),
        content: const Text(
          'This clears the visible conversation, not your draft. There is no saved history to reopen. '
          'The next message you explicitly send resets the backend’s shared default conversation. '
          'It does not delete previously copied text.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('New chat'),
          ),
        ],
      ),
    );
    if (!mounted || confirmed != true || chat.busy) return;
    await chat.requestNewSession();
  }

  Widget _composer() =>
      TextChatComposer(draft: _draft, enabled: chat.canSend, onSend: _submit);

  Widget _home() {
    final state = chat.sendBusy
        ? HaloState.thinking
        : switch (chat.bridgeStatus) {
            BridgeStatus.online => HaloState.idle,
            BridgeStatus.offline => HaloState.disconnected,
            BridgeStatus.unknown => HaloState.unknown,
          };
    return LayoutBuilder(
      builder: (context, constraints) {
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
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    SmithHalo(state: state, size: 120),
                    const SizedBox(height: 24),
                    Text(
                      'What would you like to talk about?',
                      textAlign: TextAlign.center,
                      style: Theme.of(context).textTheme.headlineMedium,
                    ),
                    const SizedBox(height: 12),
                    const Text(
                      'Text conversation with your local Smith backend.',
                      textAlign: TextAlign.center,
                    ),
                    const SizedBox(height: 24),
                    _composer(),
                    const SizedBox(height: 12),
                    TextButton(
                      onPressed: () => _navigate(1),
                      child: const Text('Open chat'),
                    ),
                    const SizedBox(height: 12),
                    const Text(
                      'Your draft stays available between views. Chats are not saved by this client.',
                      textAlign: TextAlign.center,
                    ),
                  ],
                ),
              ),
            ),
          ),
        );
      },
    );
  }

  Widget _conversation() => LayoutBuilder(
    builder: (context, constraints) {
      final scrollable =
          constraints.maxHeight < 650 ||
          MediaQuery.textScalerOf(context).scale(16) > 24;
      final content = _conversationContent(scrollable);
      return scrollable
          ? SingleChildScrollView(controller: _scroll, child: content)
          : content;
    },
  );

  Widget _conversationContent(bool scrollable) => Column(
    mainAxisSize: scrollable ? MainAxisSize.min : MainAxisSize.max,
    children: [
      Padding(
        padding: const EdgeInsets.fromLTRB(24, 8, 24, 8),
        child: Wrap(
          spacing: 16,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            const Text('Current conversation · no saved history'),
            TextButton.icon(
              onPressed: chat.busy ? null : _newChat,
              icon: const Icon(Icons.add),
              label: const Text('New chat'),
            ),
          ],
        ),
      ),
      if (scrollable)
        _messages(scrollable: true)
      else
        Expanded(child: _messages(scrollable: false)),
      if (chat.error != null)
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 24),
          child: Semantics(
            liveRegion: true,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  chat.error!,
                  style: const TextStyle(color: HaloColors.danger),
                ),
                TextButton(
                  onPressed: chat.failedText == null || chat.busy
                      ? null
                      : () async {
                          if (_draft.text.isNotEmpty) {
                            final replace = await showDialog<bool>(
                              context: context,
                              builder: (context) => AlertDialog(
                                title: const Text('Replace your draft?'),
                                content: const Text(
                                  'The failed message will replace your current unsent draft. It will not be sent automatically.',
                                ),
                                actions: [
                                  TextButton(
                                    onPressed: () =>
                                        Navigator.pop(context, false),
                                    child: const Text('Cancel'),
                                  ),
                                  FilledButton(
                                    onPressed: () =>
                                        Navigator.pop(context, true),
                                    child: const Text('Replace draft'),
                                  ),
                                ],
                              ),
                            );
                            if (!mounted || replace != true) return;
                          }
                          _draft.text = chat.failedText ?? '';
                        },
                  child: const Text('Edit failed message'),
                ),
              ],
            ),
          ),
        ),
      Padding(
        padding: const EdgeInsets.fromLTRB(24, 8, 24, 16),
        child: _composer(),
      ),
    ],
  );

  Widget _messages({required bool scrollable}) => chat.messages.isEmpty
      ? Center(
          child: Padding(
            padding: const EdgeInsets.all(24),
            child: Text(
              chat.bridgeStatus == BridgeStatus.online
                  ? 'Ask Smith a question. Your first send starts a fresh backend conversation.'
                  : 'Connect to the chat-only backend to send. You can keep editing your draft.',
              textAlign: TextAlign.center,
            ),
          ),
        )
      : ListView.builder(
          controller: scrollable ? null : _scroll,
          shrinkWrap: scrollable,
          physics: scrollable ? const NeverScrollableScrollPhysics() : null,
          padding: const EdgeInsets.all(24),
          itemCount: chat.messages.length,
          itemBuilder: (context, index) =>
              _TextMessage(message: chat.messages[index]),
        );

  Widget _settings() {
    final health = chat.health;
    String availability(bool? value) => switch (value) {
      true => 'available',
      false => 'unavailable',
      null => 'unknown',
    };
    return ListView(
      padding: const EdgeInsets.all(24),
      children: [
        Text('Connection', style: Theme.of(context).textTheme.headlineSmall),
        const SizedBox(height: 16),
        const SelectableText('Endpoint: http://127.0.0.1:4783'),
        Text('Identity: ${health?.service ?? 'unknown'}'),
        Text('Profile: ${health?.profile ?? 'unknown'}'),
        Text(
          'Provider: ${health?.provider ?? 'unknown'} · ${availability(health?.providerAvailable)}',
        ),
        Text(
          'Model: ${health?.model ?? 'unknown'} · ${availability(health?.modelAvailable)}',
        ),
        const SizedBox(height: 24),
        const Text(
          'Settings are read-only. Halo does not start, install, configure, or download the backend or models.',
        ),
        const SizedBox(height: 16),
        const Text(
          'One shared default conversation. Client drafts and transcripts are in memory only and disappear when the app closes. '
          'The chat-only backend keeps bounded context in memory; restarting it clears that context. '
          'Reconnecting does not restore history or prove prior context survived. '
          'The first explicit send from a newly opened client resets the shared backend conversation.',
        ),
        const SizedBox(height: 16),
        const Text(
          'Voice, uploads, saved history, and integrations are deferred. No automatic speech or sidecar startup occurs.',
        ),
      ],
    );
  }

  Widget _persistentPage(int index, Widget child) => ExcludeFocus(
    excluding: _destination != index,
    child: TickerMode(enabled: _destination == index, child: child),
  );

  @override
  Widget build(BuildContext context) {
    const destinations = ['Home', 'Chat', 'Settings'];
    final body = Column(
      children: [
        ConnectionNotice(chat: chat),
        Expanded(
          child: IndexedStack(
            index: _destination,
            children: [
              _persistentPage(0, _home()),
              _persistentPage(1, _conversation()),
              _persistentPage(2, _settings()),
            ],
          ),
        ),
      ],
    );
    return LayoutBuilder(
      builder: (context, constraints) {
        final wide = constraints.maxWidth >= 700;
        return Scaffold(
          appBar: AppBar(
            title: Text(destinations[_destination]),
            actions: [
              IconButton(
                tooltip: 'Settings',
                onPressed: () => _navigate(2),
                icon: const Icon(Icons.settings_outlined),
              ),
            ],
          ),
          body: SafeArea(
            child: Row(
              children: [
                if (wide) ...[
                  NavigationRail(
                    extended: constraints.maxWidth >= 1100,
                    selectedIndex: _destination < 2 ? _destination : null,
                    onDestinationSelected: _navigate,
                    destinations: const [
                      NavigationRailDestination(
                        icon: Icon(Icons.home_outlined),
                        label: Text('Home'),
                      ),
                      NavigationRailDestination(
                        icon: Icon(Icons.chat_bubble_outline),
                        label: Text('Chat'),
                      ),
                    ],
                  ),
                  const VerticalDivider(width: 1),
                ],
                Expanded(
                  child: Center(
                    child: ConstrainedBox(
                      constraints: const BoxConstraints(maxWidth: 900),
                      child: body,
                    ),
                  ),
                ),
              ],
            ),
          ),
          bottomNavigationBar: wide
              ? null
              : NavigationBar(
                  selectedIndex: _destination < 2 ? _destination : _lastPrimary,
                  onDestinationSelected: _navigate,
                  destinations: const [
                    NavigationDestination(
                      icon: Icon(Icons.home_outlined),
                      label: 'Home',
                    ),
                    NavigationDestination(
                      icon: Icon(Icons.chat_bubble_outline),
                      label: 'Chat',
                    ),
                  ],
                ),
        );
      },
    );
  }
}

class ConnectionNotice extends StatelessWidget {
  const ConnectionNotice({super.key, required this.chat});

  final ChatController chat;

  @override
  Widget build(BuildContext context) {
    final label = switch (chat.bridgeStatus) {
      BridgeStatus.unknown => 'Local bridge · checking connection',
      BridgeStatus.offline => 'Local bridge · unavailable',
      BridgeStatus.online => 'Local bridge · connected · chat-only',
    };
    return Padding(
      padding: const EdgeInsets.fromLTRB(24, 8, 24, 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Wrap(
            spacing: 12,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              Text(label),
              TextButton(
                onPressed: chat.healthChecking ? null : chat.refreshHealth,
                child: const Text('Retry connection'),
              ),
            ],
          ),
          if (chat.connectionError != null)
            Text(
              chat.connectionError!,
              style: const TextStyle(color: HaloColors.warning),
            ),
        ],
      ),
    );
  }
}

class TextChatComposer extends StatelessWidget {
  const TextChatComposer({
    super.key,
    required this.draft,
    required this.enabled,
    required this.onSend,
  });

  final TextEditingController draft;
  final bool enabled;
  final VoidCallback onSend;

  @override
  Widget build(BuildContext context) => CallbackShortcuts(
    bindings: {
      const SingleActivator(LogicalKeyboardKey.enter, control: true): () {
        if (enabled && utf8.encode(draft.text.trim()).length <= 16384) onSend();
      },
    },
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        TextField(
          key: const Key('text-chat-draft'),
          controller: draft,
          minLines: 2,
          maxLines: 6,
          maxLength: 16384,
          textInputAction: TextInputAction.newline,
          decoration: const InputDecoration(
            labelText: 'Message Smith',
            hintText: 'Ask a question…',
          ),
        ),
        ValueListenableBuilder<TextEditingValue>(
          valueListenable: draft,
          builder: (context, value, _) =>
              utf8.encode(value.text.trim()).length > 16384
              ? const Text(
                  'Message exceeds 16384 UTF-8 bytes. Shorten it before sending.',
                  style: TextStyle(color: HaloColors.warning),
                )
              : const SizedBox.shrink(),
        ),
        const SizedBox(height: 8),
        Row(
          children: [
            const Expanded(
              child: Text('Ctrl+Enter to send · Enter for a new line'),
            ),
            const SizedBox(width: 8),
            ValueListenableBuilder<TextEditingValue>(
              valueListenable: draft,
              builder: (context, value, _) => FilledButton(
                onPressed:
                    enabled &&
                        value.text.trim().isNotEmpty &&
                        utf8.encode(value.text.trim()).length <= 16384
                    ? onSend
                    : null,
                child: const Text('Send'),
              ),
            ),
          ],
        ),
      ],
    ),
  );
}

class _TextMessage extends StatelessWidget {
  const _TextMessage({required this.message});

  final ChatMessage message;

  @override
  Widget build(BuildContext context) {
    final assistant = message.role == ChatRole.assistant;
    return Padding(
      padding: const EdgeInsets.only(bottom: 24),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            assistant ? 'SMITH' : 'YOU',
            style: const TextStyle(
              color: HaloColors.secondary,
              fontWeight: FontWeight.bold,
            ),
          ),
          const SizedBox(height: 8),
          if (message.showSpinner)
            Semantics(liveRegion: true, child: const Text('Waiting for Smith…'))
          else
            // Model prose is inert, including Markdown image resource paths.
            SelectableText(message.text),
          if (assistant && !message.showSpinner)
            IconButton(
              tooltip: 'Copy response',
              onPressed: () =>
                  Clipboard.setData(ClipboardData(text: message.text)),
              icon: const Icon(Icons.copy_outlined),
            ),
        ],
      ),
    );
  }
}
