/// Presentation-only state. Never add conversation content or service authority.
final class WorkspaceSlotPreferences {
  WorkspaceSlotPreferences({required this.conversationId, required this.page}) {
    if ((conversationId != null &&
            (conversationId!.length != 32 ||
                !RegExp(r'^[0-9a-f]{32}$').hasMatch(conversationId!))) ||
        !const {'chat', 'settings'}.contains(page)) {
      throw const FormatException('Invalid workspace slot.');
    }
  }

  final String? conversationId;
  final String page;

  Map<String, Object?> toJson() => {
    'conversationId': conversationId,
    'page': page,
  };

  factory WorkspaceSlotPreferences.fromJson(Object? json) {
    if (!_keys(json, const {'conversationId', 'page'}) ||
        (json as Map)['page'] is! String ||
        (json['conversationId'] != null && json['conversationId'] is! String)) {
      throw const FormatException('Invalid workspace slot.');
    }
    return WorkspaceSlotPreferences(
      conversationId: json['conversationId'] as String?,
      page: json['page'] as String,
    );
  }

  @override
  bool operator ==(Object other) =>
      other is WorkspaceSlotPreferences &&
      conversationId == other.conversationId &&
      page == other.page;

  @override
  int get hashCode => Object.hash(conversationId, page);
}

final class WorkspacePreferences {
  WorkspacePreferences({
    required this.layout,
    required this.activeTile,
    required List<WorkspaceSlotPreferences> slots,
  }) : slots = List.unmodifiable(slots) {
    final count = const {'1x1': 1, '2x1': 2, '1x2': 2, '2x2': 4}[layout];
    if (count == null ||
        activeTile < 0 ||
        activeTile >= count ||
        slots.length != 4) {
      throw const FormatException('Invalid workspace preferences.');
    }
  }

  final String layout;
  final int activeTile;
  final List<WorkspaceSlotPreferences> slots;

  Map<String, Object?> toJson() => {
    'version': 1,
    'layout': layout,
    'activeTile': activeTile,
    'slots': slots.map((slot) => slot.toJson()).toList(),
  };

  factory WorkspacePreferences.fromJson(Object? json) {
    if (!_keys(json, const {'version', 'layout', 'activeTile', 'slots'}) ||
        (json as Map)['version'] is! int ||
        json['version'] != 1 ||
        json['layout'] is! String ||
        json['activeTile'] is! int ||
        json['slots'] is! List ||
        (json['slots'] as List).length != 4) {
      throw const FormatException(
        'Invalid or unsupported workspace preferences.',
      );
    }
    return WorkspacePreferences(
      layout: json['layout'] as String,
      activeTile: json['activeTile'] as int,
      slots: (json['slots'] as List)
          .map(WorkspaceSlotPreferences.fromJson)
          .toList(),
    );
  }

  @override
  bool operator ==(Object other) =>
      other is WorkspacePreferences &&
      layout == other.layout &&
      activeTile == other.activeTile &&
      List.generate(
        4,
        (i) => slots[i] == other.slots[i],
      ).every((equal) => equal);

  @override
  int get hashCode => Object.hash(layout, activeTile, Object.hashAll(slots));
}

bool _keys(Object? value, Set<String> keys) =>
    value is Map &&
    value.length == keys.length &&
    keys.every(value.containsKey);

abstract class WorkspacePreferencesStore {
  Future<WorkspacePreferences?> load();
  Future<void> save(WorkspacePreferences value);
  Future<void> close();
}

/// Deliberately contains no paths, native error text, or persisted values.
class WorkspacePreferencesException implements Exception {
  const WorkspacePreferencesException(this.message);
  final String message;
  @override
  String toString() => 'Workspace preferences: $message';
}
