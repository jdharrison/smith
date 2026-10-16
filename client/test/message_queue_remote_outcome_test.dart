import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/message_queue.dart';

import 'app_fakes.dart';
import 'message_queue_controller_test.dart'
    show QueueApi, ready, start, enqueue, settle;

void main() {
  for (final reattached in [false, true]) {
    test(
      'unknown remote terminal outcome blocks ${reattached ? 'reattached' : 'local'} predecessor retry and skip',
      () async {
        final api = QueueApi();
        if (reattached) {
          await api.call('submit', {
            'request_id': appId(),
            'conversation_id': testConversation,
            'text': 'external predecessor',
            'output': 'chat',
            'expected_config_revision': 1,
            'expected_history_revision': 0,
          });
        }
        final workspace = await ready(api);
        final chat = workspace.selected;
        if (!reattached) await start(chat);
        final request = chat.pendingId!;
        enqueue(chat, 'must wait');
        api.complete(request, success: false);
        api.jobs[request]!['error'] = {
          'code': 'REMOTE_OUTCOME_UNKNOWN',
          'message': 'Remote processing could not be confirmed',
        };
        await workspace.synchronize();
        await settle();
        final queue = chat.messageQueue;
        final head = queue.entries.first;
        expect(head.state, QueuedMessageState.unknown);
        expect(head.canRetry, false);
        expect(head.canSkip, false);
        expect(head.canRemove, false);
        expect(queue.paused, true);
        expect(queue.canResume, false);
        await queue.retry(head.id, head.actionRevision);
        queue.skip(head.id, head.actionRevision);
        queue.remove(head.id, head.actionRevision);
        await queue.resume(queue.revision);
        await workspace.synchronize();
        await settle();
        expect(api.sends, hasLength(1));
        expect(queue.entries.first.id, head.id);
        expect(queue.waitingCount, 1);
        queue.clear(queue.revision);
        await settle();
        expect(queue.entries.first.id, head.id);
        expect(queue.canResume, false);
        expect(api.sends, hasLength(1));
      },
    );
  }
}
