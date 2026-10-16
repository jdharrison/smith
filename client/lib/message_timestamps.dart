import 'app_v2.dart';

/// Display metadata kept separate from role/content history and provider context.
class ConversationTimestamps {
  ConversationTimestamps(Map<String, dynamic> value) {
    exact(value, ['conversation_id', 'history_revision', 'turns']);
    if (!wireId(value['conversation_id']) ||
        !wireRevision(value['history_revision']) ||
        value['turns'] is! List ||
        value['turns'].length > 16) {
      invalid();
    }
    conversationId = value['conversation_id'];
    historyRevision = value['history_revision'];
    final times = <int?>[];
    for (final turn in value['turns']) {
      exact(turn, ['user_at_ms', 'assistant_at_ms']);
      for (final key in ['user_at_ms', 'assistant_at_ms']) {
        final time = turn[key];
        if (time != null &&
            (time is! int || time <= 0 || time > 253402300799999)) {
          invalid();
        }
        times.add(time as int?);
      }
    }
    messageTimes = List.unmodifiable(times);
  }

  late final String conversationId;
  late final int historyRevision;
  late final List<int?> messageTimes;
}
