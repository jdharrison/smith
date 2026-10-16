import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'message_queue.dart';

/// One desktop-wide decision, held until cancellation or workspace disposal.
class QueueExitGuard {
  QueueExitGuard({
    required this.acquireHold,
    required this.releaseHold,
    required this.confirm,
    this.channel = const MethodChannel('dev.smith/queue_exit'),
  }) {
    channel.setMethodCallHandler((call) async {
      if (call.method != 'requestClose') {
        throw MissingPluginException();
      }
      return requestClose();
    });
  }

  final QueueExitHold Function() acquireHold;
  final void Function(QueueExitHold) releaseHold;
  final Future<bool> Function(QueueExitHold) confirm;
  final MethodChannel channel;
  Future<bool>? _pending;
  bool _disposed = false;
  bool _confirmed = false;

  Future<bool> requestClose() {
    if (_disposed) return Future.value(false);
    if (_confirmed) return Future.value(true);
    if (_pending != null) return _pending!;
    final completion = Completer<bool>();
    // Publish before acquiring: listeners may synchronously request close again.
    _pending = completion.future;
    _decide(completion);
    return completion.future;
  }

  Future<void> _decide(Completer<bool> completion) async {
    QueueExitHold? hold;
    var close = false;
    try {
      hold = acquireHold();
      close = !hold.hasUnsentText || await confirm(hold);
    } catch (_) {
      // A missing route or failed decision must never authorize data loss.
      close = false;
    }
    if (_disposed) {
      completion.complete(false);
      return;
    }
    if (!close && hold != null) {
      try {
        releaseHold(hold);
      } catch (_) {
        // Keep the window open even if release fails.
      }
    }
    _confirmed = close;
    _pending = null;
    completion.complete(close);
  }

  void dispose() {
    _disposed = true;
    channel.setMethodCallHandler(null);
    // Never release here: the workspace is about to dispose. Late decisions
    // cannot resume dispatch or grant a native close after this owner is gone.
  }
}

Future<bool> showQueueExitDialog(
  BuildContext context,
  QueueExitHold hold,
) async {
  return await showDialog<bool>(
        context: context,
        builder: (context) => AlertDialog(
          scrollable: true,
          title: const Text('Discard unsent text and close?'),
          content: Text(
            '${hold.messages} local unsent messages and ${hold.drafts} drafts '
            'will be lost. This includes hidden chats.\n\n'
            'Already accepted service work may continue. Closing does not stop '
            'the Smith service. Unsent text is not protected against crashes '
            'or forced termination.',
          ),
          actions: [
            TextButton(
              autofocus: true,
              onPressed: () => Navigator.of(context).pop(false),
              child: const Text('Keep open'),
            ),
            TextButton(
              onPressed: () => Navigator.of(context).pop(true),
              child: const Text('Discard unsent text and close'),
            ),
          ],
        ),
      ) ??
      false;
}
