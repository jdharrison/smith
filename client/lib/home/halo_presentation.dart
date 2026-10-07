import 'package:smith_client/bridge/smith_bridge.dart';
import 'package:smith_client/chat/chat_controller.dart';
import 'package:smith_client/listen/voice_turn.dart';

enum WorkFilter { all, running, ready, queued, failed }

enum HaloState {
  disconnected,
  unknown,
  listening,
  thinking,
  speaking,
  review,
  failed,
  working,
  idle,
}

String workFilterLabel(WorkFilter filter) => switch (filter) {
  WorkFilter.all => 'All',
  WorkFilter.running => 'Running',
  WorkFilter.ready => 'Ready',
  WorkFilter.queued => 'Queued',
  WorkFilter.failed => 'Failed',
};

bool taskMatchesFilter(BoardTask task, WorkFilter filter) => switch (filter) {
  WorkFilter.all => true,
  WorkFilter.running => task.status == 'running',
  WorkFilter.ready => task.status == 'awaiting_signoff',
  WorkFilter.queued => task.status == 'queued',
  WorkFilter.failed => task.status == 'failed',
};

String taskStatusLabel(String status) => switch (status) {
  'awaiting_signoff' => 'Ready for review',
  'running' => 'Running',
  'queued' => 'Queued',
  'failed' => 'Failed',
  'interrupted' => 'Interrupted — recovery required',
  'completed' => 'Completed',
  '' => 'Status unknown',
  _ => 'Unknown state: $status',
};

bool taskSnapshotIsStale({
  required List<BoardTask> tasks,
  required DateTime? updatedAt,
  required String? refreshError,
  required DateTime now,
}) {
  if (refreshError != null || updatedAt == null) return true;
  final hasActiveWork = tasks.any((task) => task.isRunning);
  return hasActiveWork &&
      now.difference(updatedAt) > const Duration(seconds: 8);
}

String taskStageLabel(String stage) => switch (stage) {
  'new' => 'New',
  'discovery' => 'Discovery',
  'preproduction' => 'Preproduction',
  'production' => 'Production',
  'review/signoff' || 'review' || 'signoff' => 'Review / signoff',
  'completion' => 'Completion',
  'failed' => 'Failed',
  '' => 'Stage unknown',
  _ => 'Unknown stage: $stage',
};

HaloState deriveHaloState({
  required BridgeStatus bridgeStatus,
  required VoiceTurnState voiceState,
  required List<BoardTask> tasks,
  required bool ttsActive,
  required bool chatBusy,
  required bool tasksStale,
}) {
  if (voiceState == VoiceTurnState.recording ||
      voiceState == VoiceTurnState.starting) {
    return HaloState.listening;
  }
  if (ttsActive) return HaloState.speaking;
  if (chatBusy ||
      voiceState == VoiceTurnState.processing ||
      voiceState == VoiceTurnState.replying ||
      voiceState == VoiceTurnState.stopping) {
    return HaloState.thinking;
  }
  if (bridgeStatus == BridgeStatus.offline) return HaloState.disconnected;
  if (bridgeStatus == BridgeStatus.unknown || tasksStale) {
    return HaloState.unknown;
  }
  if (tasks.any(
    (task) => task.status == 'failed' || task.status == 'interrupted',
  )) {
    return HaloState.failed;
  }
  if (tasks.any((task) => task.status == 'awaiting_signoff')) {
    return HaloState.review;
  }
  if (tasks.any((task) => task.status == 'running')) {
    return HaloState.working;
  }
  return HaloState.idle;
}

String haloStateLabel(HaloState state) => switch (state) {
  HaloState.disconnected => 'Disconnected',
  HaloState.unknown => 'Status unknown',
  HaloState.listening => 'Listening',
  HaloState.thinking => 'Thinking',
  HaloState.speaking => 'Speaking',
  HaloState.review => 'Review ready',
  HaloState.failed => 'Recovery needed',
  HaloState.working => 'Working',
  HaloState.idle => 'Ready',
};

String haloGreeting(HaloState state) => switch (state) {
  HaloState.disconnected => 'I’ll be here.',
  HaloState.unknown => 'Checking in.',
  HaloState.listening => 'I’m listening.',
  HaloState.thinking => 'On it.',
  HaloState.speaking => 'Here’s what I found.',
  HaloState.review => 'Ready when you are.',
  HaloState.failed => 'Something needs attention.',
  HaloState.working => 'I’m on it.',
  HaloState.idle => 'I’m here.',
};
