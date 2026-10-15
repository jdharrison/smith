import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'text_chat.dart';
import 'ui/smith_icon.dart';
import 'ui/smith_theme.dart';

class ChatPanel extends StatefulWidget {
  const ChatPanel({
    super.key,
    required this.controller,
    this.onRetry,
    this.active = true,
    this.onActivate,
    this.independentView = false,
  });
  final ChatController controller;
  final bool active;
  final VoidCallback? onActivate;
  final bool independentView;

  /// The shell's observation-only connection/inventory recovery flow.
  /// Without a shell callback, Retry only synchronizes saved state/pending work.
  final VoidCallback? onRetry;

  @override
  State<ChatPanel> createState() => _ChatPanelState();
}

class _ChatPanelState extends State<ChatPanel> {
  late final TextEditingController draft;
  late final ScrollController transcript;
  bool initialScrollScheduled = false;
  bool initialScrollComplete = false;
  Set<String> observedLocalSends = {};
  final editorFocus = FocusNode(debugLabel: 'Chat composer');
  final positions = Expando<double>();
  final editingValues = Expando<TextEditingValue>();
  bool sendingHere = false;
  ScrollMetrics? readingMetrics;
  bool restoringPosition = false;
  bool navigationPending = false;
  bool metricsRestoreScheduled = false;

  bool sameGeometry(ScrollMetrics a, ScrollMetrics b) =>
      a.viewportDimension == b.viewportDimension &&
      a.minScrollExtent == b.minScrollExtent &&
      a.maxScrollExtent == b.maxScrollExtent;

  void readingPositionChanged() {
    if (!widget.independentView ||
        restoringPosition ||
        navigationPending ||
        !transcript.hasClients) {
      return;
    }
    final current = transcript.position;
    // Layout can correct pixels before the metrics notification arrives. Only
    // scrolling within the settled geometry changes the desired bookmark.
    if (readingMetrics == null || !sameGeometry(readingMetrics!, current)) {
      return;
    }
    positions[widget.controller] = current.pixels;
    initialScrollComplete = true;
  }

  void jumpToReadingPosition(double desired, {bool remember = true}) {
    if (widget.independentView && remember) {
      positions[widget.controller] = desired;
    }
    final target = desired.clamp(
      transcript.position.minScrollExtent,
      transcript.position.maxScrollExtent,
    );
    if (transcript.offset == target) return;
    restoringPosition = true;
    try {
      transcript.jumpTo(target);
    } finally {
      restoringPosition = false;
    }
  }

  bool readingMetricsChanged(ScrollMetricsNotification notification) {
    if (!widget.independentView ||
        notification.depth != 0 ||
        metricsRestoreScheduled) {
      return false;
    }
    metricsRestoreScheduled = true;
    final controller = widget.controller;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      metricsRestoreScheduled = false;
      if (!mounted ||
          controller != widget.controller ||
          navigationPending ||
          !transcript.hasClients) {
        return;
      }
      readingMetrics = transcript.position.copyWith();
      final desired = positions[controller];
      if (desired != null) {
        jumpToReadingPosition(desired, remember: false);
      }
    });
    WidgetsBinding.instance.ensureVisualUpdate();
    return false;
  }

  double? position(ChatController controller) =>
      widget.independentView ? positions[controller] : controller.scrollOffset;

  void savePosition(ChatController controller) {
    if (!transcript.hasClients) return;
    if (widget.independentView) {
      // The current pixels may be a temporary geometry clamp, not the reader's
      // chosen offset. Preserve the desired position across navigation too.
      positions[controller] ??= transcript.offset;
    } else {
      controller.scrollOffset = transcript.offset;
    }
  }

  void editorFocusChanged() {
    if (editorFocus.hasFocus) widget.onActivate?.call();
  }

  void send() {
    if (!widget.active || !widget.controller.canSend) return;
    sendingHere = true;
    try {
      widget.controller.send();
    } finally {
      sendingHere = false;
    }
  }

  @override
  void initState() {
    super.initState();
    draft = TextEditingController(text: widget.controller.draft);
    editorFocus.addListener(editorFocusChanged);
    transcript = ScrollController(
      initialScrollOffset: position(widget.controller) ?? 0,
    );
    transcript.addListener(readingPositionChanged);
    initialScrollComplete = position(widget.controller) != null;
    widget.controller.addListener(sync);
    observedLocalSends = widget.controller.localSends.keys.toSet();
    scheduleInitialScroll();
  }

  @override
  void didUpdateWidget(covariant ChatPanel oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != widget.controller) {
      savePosition(oldWidget.controller);
      navigationPending = true;
      readingMetrics = null;
      editingValues[oldWidget.controller] = draft.value;
      oldWidget.controller.removeListener(sync);
      widget.controller.addListener(sync);
      observedLocalSends = widget.controller.localSends.keys.toSet();
      initialScrollComplete = position(widget.controller) != null;
      final controller = widget.controller;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted ||
            controller != widget.controller ||
            !transcript.hasClients) {
          return;
        }
        navigationPending = false;
        readingMetrics = transcript.position.copyWith();
        jumpToReadingPosition(
          position(controller) ?? transcript.position.maxScrollExtent,
        );
      });
      sync();
      final saved = editingValues[widget.controller];
      if (saved != null && saved.text == draft.text) {
        draft.value = saved.copyWith(composing: TextRange.empty);
      }
    }
  }

  void scheduleInitialScroll() {
    if (initialScrollComplete ||
        initialScrollScheduled ||
        (widget.controller.history.isEmpty &&
            widget.controller.provisionalAnswers.isEmpty &&
            widget.controller.localSends.isEmpty)) {
      return;
    }
    initialScrollScheduled = true;
    final controller = widget.controller;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      initialScrollScheduled = false;
      if (!mounted ||
          controller != widget.controller ||
          initialScrollComplete ||
          !transcript.hasClients) {
        return;
      }
      jumpToReadingPosition(transcript.position.maxScrollExtent);
      initialScrollComplete = true;
    });
  }

  void sync() {
    final currentSends = widget.controller.localSends.keys.toSet();
    final hasNewSend = currentSends.difference(observedLocalSends).isNotEmpty;
    observedLocalSends = currentSends;
    if (hasNewSend && (!widget.independentView || sendingHere)) {
      final controller = widget.controller;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted &&
            controller == widget.controller &&
            transcript.hasClients) {
          jumpToReadingPosition(transcript.position.maxScrollExtent);
          initialScrollComplete = true;
        }
      });
    } else if (hasNewSend && widget.independentView) {
      // Another view sent this message: do not treat it as initial history.
      initialScrollComplete = true;
    }
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

  KeyEventResult handleKey(FocusNode node, KeyEvent event) {
    final isEnter =
        event.logicalKey == LogicalKeyboardKey.enter ||
        event.logicalKey == LogicalKeyboardKey.numpadEnter;
    if (!isEnter) return KeyEventResult.ignored;
    if (!widget.active) return KeyEventResult.handled;
    // Let the platform confirm composition, rather than treating it as Send.
    if (draft.value.composing.isValid && !draft.value.composing.isCollapsed) {
      return KeyEventResult.ignored;
    }
    if (HardwareKeyboard.instance.isShiftPressed) {
      // Flutter's desktop key path does not insert a newline for Shift+Enter.
      // Replace only the native selection; leave all other editing keys alone.
      if (event is KeyDownEvent) {
        final value = draft.value;
        final selection = value.selection.isValid
            ? value.selection
            : TextSelection.collapsed(offset: value.text.length);
        draft.value = value.replaced(selection, '\n');
        widget.controller.edit(draft.text);
      }
      return KeyEventResult.handled;
    }
    if (event is KeyDownEvent && widget.controller.canSend) {
      send();
    }
    // Consume repeat/up too: a held Enter must not send again or add newlines.
    return KeyEventResult.handled;
  }

  @override
  void dispose() {
    savePosition(widget.controller);
    editorFocus.dispose();
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
        scrollable: true,
        title: const Text('Forget expired request locally?'),
        content: const Text(
          'The outcome may be unknown—check history before sending again. Clears only this device’s pending request; keeps your draft and history. Doesn’t resend or cancel work.',
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
    if (mounted && widget.controller == c && confirmed == true && id != null) {
      c.abandonExpiredRequest(id);
    }
  }

  Future<void> showRecoveryDetails() async {
    final c = widget.controller;
    final id = c.pendingId;
    final error = c.error;
    final pending = id == null
        ? null
        : 'Pending ${c.pendingKind} · ${c.phase}. Retry queries only; it never resends.';
    final retry = widget.onRetry;
    final action = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        scrollable: true,
        title: const Text('Recovery details'),
        content: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            if (error != null) SelectableText(error),
            if (pending != null) ...[
              const SizedBox(height: 8),
              SelectableText(pending),
            ],
            if (c.stale) const Text('Saved conversation · stale / unavailable'),
            const SizedBox(height: 8),
            TextButton(
              onPressed: c.busy ? null : () => Navigator.pop(context, 'retry'),
              child: const Text('Retry'),
            ),
            if (c.canAbandonExpiredRequest)
              TextButton(
                onPressed: c.busy
                    ? null
                    : () => Navigator.pop(context, 'resolve'),
                child: const Text('Resolve expired request'),
              ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Close'),
          ),
        ],
      ),
    );
    // The dialog describes a snapshot, never authority over a replacement
    // controller or a different request that arrived while it was open.
    if (!mounted || widget.controller != c || c.pendingId != id || c.busy) {
      return;
    }
    if (action == 'retry') {
      if (retry != null) {
        retry();
      } else {
        await c.synchronize();
      }
    } else if (action == 'resolve' && c.canAbandonExpiredRequest) {
      await forget();
    }
  }

  Widget bubble(
    String content, {
    bool user = false,
    String? note,
    Key? key,
    bool progress = false,
  }) {
    return LayoutBuilder(
      key: key,
      builder: (context, constraints) => Align(
        alignment: user ? Alignment.centerRight : Alignment.centerLeft,
        child: Container(
          constraints: BoxConstraints(
            maxWidth: math.min(
              720,
              constraints.maxWidth * (constraints.maxWidth < 600 ? .95 : .8),
            ),
          ),
          margin: const EdgeInsets.symmetric(vertical: 8),
          child: Column(
            crossAxisAlignment: user
                ? CrossAxisAlignment.end
                : CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Container(
                    width: 6,
                    height: 6,
                    decoration: BoxDecoration(
                      color: user ? SmithColors.userMarker : SmithColors.accent,
                      shape: BoxShape.circle,
                    ),
                  ),
                  const SizedBox(width: 8),
                  Text(
                    user ? 'YOU' : 'SMITH',
                    style: Theme.of(context).textTheme.labelSmall?.copyWith(
                      color: user
                          ? SmithColors.textPrimary
                          : SmithColors.accent,
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 8),
              Container(
                padding: const EdgeInsets.all(12),
                decoration: BoxDecoration(
                  color: SmithColors.surface,
                  border: Border.all(color: SmithColors.strokeSubtle),
                  borderRadius: BorderRadius.circular(6),
                ),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    if (note != null) ...[
                      Text(note, style: Theme.of(context).textTheme.labelSmall),
                      const SizedBox(height: 8),
                    ],
                    if (progress)
                      const ExcludeSemantics(
                        child: Padding(
                          padding: EdgeInsets.only(bottom: 8),
                          child: Icon(Icons.hourglass_top, size: 16),
                        ),
                      ),
                    SelectableText(
                      content,
                      style: Theme.of(context).textTheme.bodyMedium,
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final c = widget.controller;
    return LayoutBuilder(
      builder: (context, constraints) {
        final gutter = constraints.maxWidth < 600 ? 8.0 : 24.0;
        final short = constraints.maxHeight < 160;
        final compactRecovery = constraints.maxHeight < 200;
        final bodyStyle = Theme.of(context).textTheme.bodyMedium!;
        final minimumComposer = math.max(
          48.0,
          MediaQuery.textScalerOf(context).scale(bodyStyle.fontSize ?? 14) *
                  (bodyStyle.height ?? 1.5) +
              14,
        );
        final composerHeight = (constraints.maxHeight * .3)
            .clamp(
              short ? minimumComposer : math.max(64.0, minimumComposer),
              math.max(200.0, minimumComposer),
            )
            .toDouble();
        final composerGutter = short ? 0.0 : 8.0;
        final recoveryAvailable = math.max(
          0.0,
          constraints.maxHeight - composerHeight - composerGutter * 2 - 1,
        );
        Widget bounded(Widget child, {double? horizontal}) => Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 960),
            child: Padding(
              padding: EdgeInsets.symmetric(horizontal: horizontal ?? gutter),
              child: child,
            ),
          ),
        );

        return ColoredBox(
          color: SmithColors.background,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Expanded(
                child: NotificationListener<ScrollMetricsNotification>(
                  onNotification: readingMetricsChanged,
                  child: ScrollbarTheme(
                    data: ScrollbarTheme.of(context).copyWith(
                      crossAxisMargin: 0,
                      thickness: const WidgetStatePropertyAll(8),
                    ),
                    child: Scrollbar(
                      controller: transcript,
                      scrollbarOrientation: ScrollbarOrientation.right,
                      child: ScrollConfiguration(
                        behavior: ScrollConfiguration.of(context)
                            .copyWith(scrollbars: false),
                        child: SingleChildScrollView(
                          key: const ValueKey('conversation-history'),
                          controller: transcript,
                          child: bounded(
                            // Reserve the 8px thumb plus clearance even on narrow
                            // panes; symmetric gutters keep messages centered.
                            horizontal: math.max(gutter, 16),
                            Column(
                              crossAxisAlignment: CrossAxisAlignment.stretch,
                              children: [
                                if (c.stale)
                                  const Text(
                                    'Saved conversation · stale / unavailable',
                                  ),
                                if (c.history.isEmpty &&
                                    c.provisionalAnswers.isEmpty &&
                                    c.localSends.isEmpty)
                                  const Padding(
                                    padding: EdgeInsets.symmetric(vertical: 32),
                                    child: Text(
                                      'What would you like to work on?',
                                    ),
                                  ),
                                for (final turn in c.history)
                                  bubble(
                                    turn['content'] as String,
                                    user: turn['role'] == 'user',
                                  ),
                                for (final send in c.localSends.entries) ...[
                                  bubble(
                                    send.value.text,
                                    user: true,
                                    key: ValueKey('local-user-${send.key}'),
                                  ),
                                  if (!c.provisionalAnswers.containsKey(
                                    send.key,
                                  ))
                                    Semantics(
                                      liveRegion: true,
                                      child: bubble(
                                        send.value.status,
                                        progress: send.value.active,
                                        key: ValueKey(
                                          'local-status-${send.key}',
                                        ),
                                        note: send.value.active
                                            ? 'Request in progress'
                                            : 'Request status',
                                      ),
                                    ),
                                  if (c.provisionalAnswers.containsKey(
                                    send.key,
                                  ))
                                    bubble(
                                      c.provisionalAnswers[send.key]!,
                                      key: ValueKey(
                                        'terminal-answer-${send.key}',
                                      ),
                                      note: 'Verified answer · awaiting history sync',
                                    ),
                                ],
                                for (final answer
                                    in c.provisionalAnswers.entries.where(
                                      (entry) =>
                                          !c.localSends.containsKey(entry.key),
                                    ))
                                  bubble(
                                    // Text equality avoids duplicate display, not identity claims.
                                    c.history.any(
                                          (turn) =>
                                              turn['role'] == 'assistant' &&
                                              turn['content'] == answer.value,
                                        )
                                        ? 'Answer text is already visible in saved history.'
                                        : answer.value,
                                    key: ValueKey(
                                      'terminal-answer-${answer.key}',
                                    ),
                                    note: 'Verified answer · awaiting history sync',
                                  ),
                              ],
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
              bounded(
                Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    if (c.error != null || c.stale || c.pendingId != null)
                      if (compactRecovery)
                        SizedBox(
                          height: math.min(48, recoveryAvailable),
                          child: Tooltip(
                            message: 'View recovery details',
                            child: TextButton(
                              onPressed: showRecoveryDetails,
                              style: TextButton.styleFrom(
                                padding: EdgeInsets.zero,
                              ),
                              child: Text(
                                c.error != null ||
                                        c.stale ||
                                        c.canAbandonExpiredRequest
                                    ? 'Needs attention'
                                    : 'Request pending',
                              ),
                            ),
                          ),
                        )
                      else
                        ConstrainedBox(
                          constraints: BoxConstraints(
                            maxHeight: math.min(
                              recoveryAvailable,
                              math.max(48.0, recoveryAvailable * .5),
                            ),
                          ),
                          child: SingleChildScrollView(
                            key: const ValueKey('conversation-recovery'),
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.stretch,
                              children: [
                                if (c.error != null)
                                  Padding(
                                    padding: EdgeInsets.symmetric(
                                      vertical: short ? 0 : 8,
                                    ),
                                    child: SelectableText(
                                      c.error!,
                                      key: const ValueKey('conversation-error'),
                                      style: Theme.of(context)
                                          .textTheme
                                          .bodyMedium
                                          ?.copyWith(color: SmithColors.danger),
                                    ),
                                  ),
                                if (c.pendingId != null)
                                  Text(
                                    'Pending ${c.pendingKind} · ${c.phase}. Retry queries only; it never resends.',
                                  ),
                                Align(
                                  alignment: Alignment.centerLeft,
                                  child: Wrap(
                                    children: [
                                      TextButton(
                                        onPressed: c.busy
                                            ? null
                                            : widget.onRetry ?? c.synchronize,
                                        child: const Text('Retry'),
                                      ),
                                      if (c.canAbandonExpiredRequest)
                                        TextButton(
                                          onPressed: c.busy ? null : forget,
                                          child: const Text(
                                            'Resolve expired request',
                                          ),
                                        ),
                                    ],
                                  ),
                                ),
                              ],
                            ),
                          ),
                        ),
                    const Divider(
                      height: 1,
                      thickness: 1,
                      color: SmithColors.strokeSubtle,
                    ),
                    Padding(
                      padding: EdgeInsets.symmetric(vertical: composerGutter),
                      child: ConstrainedBox(
                        constraints: BoxConstraints(maxHeight: composerHeight),
                        child: Row(
                          crossAxisAlignment: CrossAxisAlignment.end,
                          children: [
                            Expanded(
                              child: Focus(
                                onKeyEvent: handleKey,
                                child: TextField(
                                  key: const ValueKey('chat-composer'),
                                  controller: draft,
                                  focusNode: editorFocus,
                                  readOnly: !widget.active,
                                  onTap: widget.onActivate,
                                  onChanged: (value) {
                                    if (widget.active) c.edit(value);
                                  },
                                  style: Theme.of(context).textTheme.bodyMedium,
                                  cursorHeight: MediaQuery.textScalerOf(context)
                                      .scale(bodyStyle.fontSize ?? 14),
                                  keyboardType: TextInputType.multiline,
                                  minLines: 1,
                                  maxLines: null,
                                  decoration: InputDecoration(
                                    hintText: 'Message Smith…',
                                    prefixIcon: const ExcludeSemantics(
                                      child: Center(
                                        widthFactor: 1,
                                        heightFactor: 1,
                                        child: SmithIcon(
                                          'chevron-right',
                                          size: 20,
                                          color: SmithColors.accent,
                                        ),
                                      ),
                                    ),
                                    prefixIconConstraints: const BoxConstraints(
                                      minWidth: 28,
                                    ),
                                    filled: true,
                                    fillColor: SmithColors.surface,
                                    contentPadding: EdgeInsets.symmetric(
                                      horizontal: 12,
                                      vertical: composerHeight < 90 ? 6 : 16,
                                    ),
                                    border: OutlineInputBorder(
                                      borderRadius: BorderRadius.circular(6),
                                    ),
                                    enabledBorder: OutlineInputBorder(
                                      borderRadius: BorderRadius.circular(6),
                                      borderSide: const BorderSide(
                                        color: SmithColors.stroke,
                                      ),
                                    ),
                                    focusedBorder: OutlineInputBorder(
                                      borderRadius: BorderRadius.circular(6),
                                      borderSide: const BorderSide(
                                        color: SmithColors.accent,
                                      ),
                                    ),
                                  ),
                                ),
                              ),
                            ),
                            const SizedBox(width: 8),
                            SizedBox(
                              width: 48,
                              height: 48,
                              child: IconButton.outlined(
                                tooltip: 'Send',
                                onPressed: widget.active && c.canSend
                                    ? send
                                    : null,
                                style: IconButton.styleFrom(
                                  shape: RoundedRectangleBorder(
                                    borderRadius: BorderRadius.circular(6),
                                  ),
                                  side: const BorderSide(
                                    color: SmithColors.stroke,
                                  ),
                                ),
                                icon: SmithIcon(
                                  'send',
                                  color: widget.active && c.canSend
                                      ? SmithColors.accent
                                      : SmithColors.textMuted,
                                ),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}
