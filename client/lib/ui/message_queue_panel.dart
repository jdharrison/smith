import 'package:flutter/material.dart';

import '../message_queue.dart';
import '../text_chat.dart';

/// Local intentions are deliberately separate from the authoritative transcript.
class MessageQueuePanel extends StatefulWidget {
  const MessageQueuePanel({
    super.key,
    required this.controller,
    required this.active,
  });

  final ChatController controller;
  final bool active;

  @override
  State<MessageQueuePanel> createState() => _MessageQueuePanelState();
}

class _MessageQueuePanelState extends State<MessageQueuePanel> {
  bool eligible(MessageQueue queue) =>
      mounted &&
      widget.active &&
      identical(widget.controller.messageQueue, queue);

  Widget action(String label, VoidCallback? callback) => Tooltip(
    message: label,
    child: TextButton(onPressed: callback, child: Text(label)),
  );

  Future<void> confirm(MessageQueue queue, {required bool resume}) async {
    if (!eligible(queue)) return;
    final revision = queue.revision;
    final count = queue.entries.where((row) => row.canRemove).length;
    final reason = queue.pauseReason;
    final approved = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        scrollable: true,
        title: Text(
          resume ? 'Resume queued messages?' : 'Clear $count unsent messages?',
        ),
        content: Text(
          resume
              ? '${reason ?? 'The queue is paused.'}\n\nContinue queued turns using the current conversation, service and model configuration. If that binding changed, this explicitly accepts the new binding. This does not steer or cancel an active reply.'
              : 'Remove $count removable local messages. This cannot be undone. Accepted or running work is not cancelled.',
        ),
        actions: [
          action('Keep queue', () => Navigator.pop(context, false)),
          action(
            resume ? 'Confirm resume' : 'Confirm clear',
            () => Navigator.pop(context, true),
          ),
        ],
      ),
    );
    // The captured token is passed unchanged: the controller, not a dialog,
    // decides whether the reviewed snapshot still grants this action.
    if (approved != true || !eligible(queue)) return;
    if (resume) {
      await queue.resume(revision);
    } else {
      queue.clear(revision);
    }
  }

  String stateLabel(QueuedMessage row) => switch (row.state) {
    QueuedMessageState.waiting => 'Waiting locally—not sent',
    QueuedMessageState.sending => 'Sending—acceptance not yet confirmed',
    QueuedMessageState.accepted => 'Accepted by service',
    QueuedMessageState.failed => 'Blocked—failed / not admitted',
    QueuedMessageState.unknown => 'Blocked—outcome unknown; observe only',
  };

  Widget contents(MessageQueue queue, {required bool detail}) {
    final revision = queue.revision;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Wrap(
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            Text('Up next · ${queue.waitingCount} waiting locally'),
            if (!detail) action('Queue details', () => details(queue)),
          ],
        ),
        Text(
          'Session-only: closing loses unsent messages. Queue next turn—not steering.',
        ),
        Text(
          'Capacity: ${MessageQueue.perConversationLimit} local messages per chat · ${MessageQueue.desktopLimit} desktop-wide · 16 KiB each',
        ),
        if (widget.controller.draft.trim().isNotEmpty && !queue.canEnqueue)
          const Text('Cannot queue this draft right now; draft kept.'),
        if (queue.paused)
          Text('Paused · ${queue.pauseReason ?? 'Dispatch paused'}'),
        Wrap(
          children: [
            if (!queue.paused)
              action(
                'Pause',
                eligible(queue)
                    ? () {
                        if (eligible(queue)) queue.pause();
                      }
                    : null,
              ),
            if (queue.canResume)
              action(
                'Resume',
                eligible(queue) ? () => confirm(queue, resume: true) : null,
              ),
            if (queue.entries.any((row) => row.canRemove))
              action(
                'Clear unsent',
                eligible(queue)
                    ? () {
                        // Do not allow a callback from an older presentation to review a
                        // different set of messages under the same action.
                        if (eligible(queue) && queue.revision == revision) {
                          confirm(queue, resume: false);
                        }
                      }
                    : null,
              ),
          ],
        ),
        for (final row in queue.entries)
          Padding(
            key: ValueKey('queue-row-${row.id}'),
            padding: const EdgeInsets.only(bottom: 8),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(stateLabel(row)),
                Text(
                  row.text,
                  maxLines: detail ? null : 2,
                  overflow: detail ? null : TextOverflow.ellipsis,
                ),
                Text(row.status),
                Wrap(
                  children: [
                    if (row.canRetry)
                      action(
                        'Retry queued message',
                        eligible(queue)
                            ? () {
                                if (eligible(queue)) {
                                  queue.retry(row.id, row.actionRevision);
                                }
                              }
                            : null,
                      ),
                    if (row.canSkip)
                      action(
                        'Skip and continue',
                        eligible(queue)
                            ? () {
                                if (eligible(queue)) {
                                  queue.skip(row.id, row.actionRevision);
                                }
                              }
                            : null,
                      ),
                    if (row.canRemove)
                      action(
                        'Remove unsent message',
                        eligible(queue)
                            ? () {
                                if (eligible(queue)) {
                                  queue.remove(row.id, row.actionRevision);
                                }
                              }
                            : null,
                      ),
                  ],
                ),
              ],
            ),
          ),
      ],
    );
  }

  Future<void> details(MessageQueue queue) async {
    if (!eligible(queue)) return;
    final owner = widget.controller;
    await showDialog<void>(
      context: context,
      builder: (context) => AnimatedBuilder(
        animation: owner,
        builder: (context, _) => AlertDialog(
          scrollable: true,
          title: const Text('Local message queue'),
          content: SizedBox(
            width: 480,
            child:
                mounted &&
                    identical(widget.controller, owner) &&
                    identical(owner.messageQueue, queue)
                ? contents(queue, detail: true)
                : const Text(
                    'The originating queue view is no longer available.',
                  ),
          ),
          actions: [action('Close', () => Navigator.pop(context))],
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final queue = widget.controller.messageQueue;
    if (queue == null) return const SizedBox.shrink();
    return SingleChildScrollView(
      key: const ValueKey('message-queue-shelf'),
      primary: false,
      child: contents(queue, detail: false),
    );
  }
}
