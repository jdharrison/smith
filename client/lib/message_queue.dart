/// Client-only messages, distinct from accepted service operations.
enum QueuedMessageState { waiting, sending, accepted, failed, unknown }

/// An immutable presentation snapshot. IDs identify logical messages, not retries.
class QueuedMessage {
  const QueuedMessage({
    required this.id,
    required this.text,
    required this.state,
    required this.status,
    required this.actionRevision,
    this.canRetry = false,
    this.canSkip = false,
    this.canRemove = false,
  });

  final String id;
  final String text;
  final QueuedMessageState state;
  final String status;
  final int actionRevision;
  final bool canRetry;
  final bool canSkip;
  final bool canRemove;
}

/// Mutations revalidate identity and eligibility; stale UI is never authority.
/// Implementations notify through their owning conversation controller.
abstract class MessageQueue {
  static const perConversationLimit = 8;
  static const desktopLimit = 32;
  static const maxMessageBytes = 16384;

  List<QueuedMessage> get entries;
  int get revision;
  bool get usesQueue;
  bool get canEnqueue;
  bool get paused;
  String? get pauseReason;
  int get waitingCount;
  bool get canResume;

  void enqueue();
  void pause();
  Future<void> resume(int expectedRevision);
  Future<void> retry(String id, int expectedRevision);
  void skip(String id, int expectedRevision);
  void remove(String id, int expectedRevision);
  void clear(int expectedRevision);
}

/// Identity-bound workspace hold; cancelling exit releases only this hold.
class QueueExitHold {
  const QueueExitHold({required this.messages, required this.drafts});

  final int messages;
  final int drafts;
  bool get hasUnsentText => messages > 0 || drafts > 0;
}
