import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'text_chat.dart';

class ChatPanel extends StatefulWidget {
  const ChatPanel({super.key, required this.controller});
  final ChatController controller;
  @override
  State<ChatPanel> createState() => _ChatPanelState();
}

class _ChatPanelState extends State<ChatPanel> {
  late final TextEditingController draft;
  late final ScrollController transcript;
  bool initialScrollScheduled = false;
  bool initialScrollComplete = false;

  @override
  void initState() {
    super.initState();
    draft = TextEditingController(text: widget.controller.draft);
    transcript = ScrollController();
    widget.controller.addListener(sync);
    scheduleInitialScroll();
  }

  void scheduleInitialScroll() {
    if (initialScrollComplete ||
        initialScrollScheduled ||
        (widget.controller.history.isEmpty &&
            widget.controller.provisionalAnswers.isEmpty)) {
      return;
    }
    initialScrollScheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      initialScrollScheduled = false;
      if (!mounted || !transcript.hasClients) return;
      transcript.jumpTo(transcript.position.maxScrollExtent);
      initialScrollComplete = true;
    });
  }

  void sync() {
    if (draft.text != widget.controller.draft) {
      draft.value = TextEditingValue(
        text: widget.controller.draft,
        selection: TextSelection.collapsed(
          offset: widget.controller.draft.length,
        ),
      );
    }
    scheduleInitialScroll();
    setState(() {});
  }

  void insertNewline() {
    final value = draft.value;
    final selection = value.selection.isValid
        ? value.selection
        : TextSelection.collapsed(offset: value.text.length);
    draft.value = value.replaced(selection, '\n');
    widget.controller.edit(draft.text);
  }

  @override
  void dispose() {
    widget.controller.removeListener(sync);
    draft.dispose();
    transcript.dispose();
    super.dispose();
  }

  Future<void> forget() async {
    final c = widget.controller;
    final id = c.pendingId;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Forget expired request locally?'),
        content: const Text(
          'Its outcome may be unknown. Review saved history before sending new work to avoid duplicates. This forgets only local pending metadata; draft and history remain. Nothing is resent or cancelled.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('Keep pending'),
          ),
          TextButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('Forget request'),
          ),
        ],
      ),
    );
    if (mounted && confirmed == true && id != null) c.abandonExpiredRequest(id);
  }

  @override
  Widget build(BuildContext context) {
    final c = widget.controller;

    return LayoutBuilder(
      builder: (context, constraints) {
        final composerHeight = (constraints.maxHeight * 0.3).clamp(64.0, 200.0);
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Expanded(
              child: SingleChildScrollView(
                key: const ValueKey('conversation-history'),
                controller: transcript,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    if (c.stale)
                      const Text('Saved conversation · stale / unavailable'),
                    if (c.history.isEmpty && c.provisionalAnswers.isEmpty)
                      const Padding(
                        padding: EdgeInsets.symmetric(vertical: 32),
                        child: Text('What would you like to work on?'),
                      ),
                    for (final turn in c.history)
                      Padding(
                        padding: const EdgeInsets.symmetric(vertical: 10),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              turn['role'] == 'user' ? 'You' : 'Smith',
                              style: Theme.of(context).textTheme.labelLarge,
                            ),
                            const SizedBox(height: 4),
                            SelectableText(turn['content'] as String),
                          ],
                        ),
                      ),
                    for (final answer in c.provisionalAnswers.entries)
                      Padding(
                        key: ValueKey('terminal-answer-${answer.key}'),
                        padding: const EdgeInsets.symmetric(vertical: 10),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              'Verified answer · awaiting history sync',
                              style: Theme.of(context).textTheme.labelLarge,
                            ),
                            const SizedBox(height: 4),
                            // History has no request IDs. Avoid rendering identical text
                            // twice without claiming it identifies this request's turn.
                            if (c.history.any(
                              (turn) =>
                                  turn['role'] == 'assistant' &&
                                  turn['content'] == answer.value,
                            ))
                              const Text(
                                'Answer text is already visible in saved history.',
                              )
                            else
                              SelectableText(answer.value),
                          ],
                        ),
                      ),
                    if (c.error != null)
                      Padding(
                        padding: const EdgeInsets.symmetric(vertical: 8),
                        child: Text(c.error!),
                      ),
                    if (c.pendingId != null)
                      Text(
                        'Pending ${c.pendingKind} · ${c.phase}. Retry queries only; it never resends.',
                      ),
                  ],
                ),
              ),
            ),
            if (c.canAbandonExpiredRequest)
              TextButton(
                onPressed: c.busy ? null : forget,
                child: const Text('Resolve expired request'),
              ),
            const SizedBox(height: 16),
            ConstrainedBox(
              constraints: BoxConstraints(maxHeight: composerHeight),
              child: Focus(
                onKeyEvent: (_, event) {
                  final isEnter =
                      event.logicalKey == LogicalKeyboardKey.enter ||
                      event.logicalKey == LogicalKeyboardKey.numpadEnter;
                  if (event is! KeyDownEvent || !isEnter) {
                    return KeyEventResult.ignored;
                  }
                  if (HardwareKeyboard.instance.isShiftPressed) {
                    insertNewline();
                  } else if (c.canSend) {
                    c.send();
                  }
                  return KeyEventResult.handled;
                },
                child: TextField(
                  controller: draft,
                  onChanged: c.edit,
                  keyboardType: TextInputType.multiline,
                  minLines: 1,
                  maxLines: null,
                  decoration: InputDecoration(
                    hintText: 'Message',
                    filled: true,
                    fillColor: Theme.of(context).colorScheme.surface,
                    border: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(24),
                      borderSide: BorderSide.none,
                    ),
                  ),
                ),
              ),
            ),
            const SizedBox(height: 8),
            Align(
              alignment: Alignment.centerRight,
              child: ElevatedButton(
                onPressed: c.canSend ? c.send : null,
                child: const Text('Send'),
              ),
            ),
          ],
        );
      },
    );
  }
}
