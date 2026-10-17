import 'package:flutter/material.dart';

import 'smith_theme.dart';

/// Uses the workstation timezone unless an explicit offset is supplied.
String? formatMessageTimestamp(int? timestampMs, {Duration? utcOffset}) {
  if (timestampMs == null ||
      timestampMs <= 0 ||
      timestampMs > 253402300799999) {
    return null;
  }
  final utc = DateTime.fromMillisecondsSinceEpoch(timestampMs, isUtc: true);
  final local = utcOffset == null ? utc.toLocal() : utc.add(utcOffset);
  final offset = utcOffset ?? local.timeZoneOffset;
  String two(int value) => value.toString().padLeft(2, '0');
  final offsetMinutes = offset.inMinutes.abs();
  final zone =
      '${offset.isNegative ? '-' : '+'}'
      '${two(offsetMinutes ~/ 60)}:${two(offsetMinutes % 60)}';
  return '${local.year.toString().padLeft(4, '0')}-'
      '${two(local.month)}-${two(local.day)} '
      '${two(local.hour)}:${two(local.minute)}:${two(local.second)} $zone';
}

class MessageHeader extends StatelessWidget {
  const MessageHeader({super.key, this.user = false, this.timestampMs});

  final bool user;
  final int? timestampMs;

  @override
  Widget build(BuildContext context) {
    final style = Theme.of(context).textTheme.labelSmall;
    final timestamp = formatMessageTimestamp(timestampMs);
    final event = user ? 'Accepted by Smith' : 'Reply completed by Smith';
    final description = '$event · $timestamp (local time)';
    return Wrap(
      alignment: user ? WrapAlignment.end : WrapAlignment.start,
      crossAxisAlignment: WrapCrossAlignment.center,
      spacing: 8,
      runSpacing: 4,
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
              style: style?.copyWith(
                color: user ? SmithColors.textPrimary : SmithColors.accent,
              ),
            ),
          ],
        ),
        if (timestamp != null)
          Tooltip(
            message: description,
            excludeFromSemantics: true,
            child: Text(
              timestamp,
              style: style,
              textAlign: user ? TextAlign.right : TextAlign.left,
              semanticsLabel: description,
            ),
          ),
      ],
    );
  }
}
