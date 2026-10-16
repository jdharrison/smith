const maxChatTitleRunes = 40;

/// Presentation only; authoritative metadata and rename editor text stay raw.
String displayChatTitle(String title) => String.fromCharCodes(
  (title.trim().isEmpty ? 'Untitled chat' : title).runes.take(
    maxChatTitleRunes,
  ),
);
