import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/bridge/smith_bridge.dart';
import 'package:smith_client/chat/chat_controller.dart';
import 'package:smith_client/home/halo_presentation.dart';
import 'package:smith_client/listen/voice_turn.dart';

BoardTask task({String status = 'queued', String stage = 'new'}) => BoardTask(
  id: 'task-1',
  title: 'A task',
  kind: 'manual',
  stage: stage,
  status: status,
);

HaloState state({
  BridgeStatus bridge = BridgeStatus.online,
  VoiceTurnState voice = VoiceTurnState.idle,
  List<BoardTask> tasks = const [],
  bool ttsActive = false,
  bool chatBusy = false,
  bool tasksStale = false,
}) => deriveHaloState(
  bridgeStatus: bridge,
  voiceState: voice,
  tasks: tasks,
  ttsActive: ttsActive,
  chatBusy: chatBusy,
  tasksStale: tasksStale,
);

void main() {
  group('work presentation', () {
    test(
      'active snapshots become stale when polling no longer refreshes them',
      () {
        final now = DateTime(2026, 1, 1, 12);
        expect(
          taskSnapshotIsStale(
            tasks: [task(status: 'running')],
            updatedAt: now.subtract(const Duration(seconds: 9)),
            refreshError: null,
            now: now,
          ),
          isTrue,
        );
        expect(
          taskSnapshotIsStale(
            tasks: [task(status: 'completed')],
            updatedAt: now.subtract(const Duration(minutes: 2)),
            refreshError: null,
            now: now,
          ),
          isFalse,
        );
      },
    );

    test('maps signoff to ready for review without claiming completion', () {
      expect(taskStatusLabel('awaiting_signoff'), 'Ready for review');
      expect(
        taskMatchesFilter(task(status: 'awaiting_signoff'), WorkFilter.ready),
        isTrue,
      );
      expect(
        taskMatchesFilter(task(status: 'awaiting_signoff'), WorkFilter.running),
        isFalse,
      );
    });

    test('filters use real bridge statuses', () {
      expect(
        taskMatchesFilter(task(status: 'running'), WorkFilter.running),
        isTrue,
      );
      expect(
        taskMatchesFilter(task(status: 'queued'), WorkFilter.queued),
        isTrue,
      );
      expect(
        taskMatchesFilter(task(status: 'failed'), WorkFilter.failed),
        isTrue,
      );
      expect(
        taskMatchesFilter(task(status: 'completed'), WorkFilter.ready),
        isFalse,
      );
    });

    test('interrupted work is explicit recovery-required terminal state', () {
      final interrupted = task(status: 'interrupted');
      expect(
        taskStatusLabel(interrupted.status),
        'Interrupted — recovery required',
      );
      expect(interrupted.isTerminalInject, isTrue);
      expect(state(tasks: [interrupted]), HaloState.failed);
    });

    test('unknown values remain explicit', () {
      expect(taskStatusLabel('paused'), 'Unknown state: paused');
      expect(taskStageLabel('mystery'), 'Unknown stage: mystery');
      expect(taskStatusLabel(''), 'Status unknown');
    });
  });

  group('halo presentation', () {
    test('active capture is never masked by bridge state', () {
      expect(
        state(
          bridge: BridgeStatus.offline,
          voice: VoiceTurnState.recording,
          tasks: [task(status: 'running')],
        ),
        HaloState.listening,
      );
      expect(state(bridge: BridgeStatus.unknown), HaloState.unknown);
    });

    test('capture is only labeled listening while capture is active', () {
      expect(state(voice: VoiceTurnState.recording), HaloState.listening);
      expect(state(), HaloState.idle);
    });

    test('immediate activity takes precedence over stale task data', () {
      expect(state(chatBusy: true, tasksStale: true), HaloState.thinking);
      expect(state(ttsActive: true, tasksStale: true), HaloState.speaking);
      expect(
        state(bridge: BridgeStatus.offline, voice: VoiceTurnState.processing),
        HaloState.thinking,
      );
      expect(state(tasksStale: true), HaloState.unknown);
    });

    test('failure takes precedence over review and working', () {
      expect(
        state(
          tasks: [
            task(status: 'awaiting_signoff'),
            task(status: 'failed'),
            task(status: 'running'),
          ],
        ),
        HaloState.failed,
      );
      expect(
        state(tasks: [task(status: 'awaiting_signoff')]),
        HaloState.review,
      );
      expect(state(tasks: [task(status: 'running')]), HaloState.working);
      expect(state(tasks: [task(status: 'queued')]), HaloState.idle);
    });
  });
}
