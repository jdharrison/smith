import 'package:flutter/material.dart';

import 'smith_theme.dart';
import 'workspace_entrance.dart';

/// Window-level presentation; service and layout ownership remain with callers.
class SmithAppHeader extends StatelessWidget {
  const SmithAppHeader({
    super.key,
    required this.connectionStatus,
    required this.connectionOnline,
    this.trailing,
  });
  final String connectionStatus;
  final bool connectionOnline;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) => WorkspaceReveal(
    region: WorkspaceRegion.header,
    child: Material(
      color: SmithColors.background,
      child: LayoutBuilder(
        builder: (context, bounds) => Container(
          constraints: const BoxConstraints(minHeight: 56),
          width: double.infinity,
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
          decoration: const BoxDecoration(
            border: Border(bottom: BorderSide(color: SmithColors.strokeSubtle)),
          ),
          child: Wrap(
            alignment: WrapAlignment.spaceBetween,
            crossAxisAlignment: WrapCrossAlignment.center,
            spacing: 16,
            runSpacing: 4,
            children: [
              const Text(
                '> SMITH',
                style: TextStyle(fontSize: 22, color: SmithColors.accent),
              ),
              ConstrainedBox(
                constraints: BoxConstraints(
                  maxWidth: (bounds.maxWidth - 24).clamp(0, 560),
                ),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: [
                    Flexible(
                      child: Semantics(
                        liveRegion: true,
                        label: connectionStatus,
                        hint: 'Last verified app service transport and state, not an ongoing login-enablement guarantee. Automatically checked 30 seconds after each completed check, not continuous monitoring. Model readiness and work completion are separate.',
                        excludeSemantics: true,
                        child: Wrap(
                          spacing: 8,
                          crossAxisAlignment: WrapCrossAlignment.center,
                          children: [
                            ExcludeSemantics(
                              child: Container(
                                width: 8,
                                height: 8,
                                decoration: BoxDecoration(
                                  shape: BoxShape.circle,
                                  color: connectionOnline
                                      ? SmithColors.statusOnline
                                      : SmithColors.textMuted,
                                ),
                              ),
                            ),
                            Text(
                              connectionStatus,
                              style: TextStyle(
                                color: connectionOnline
                                    ? SmithColors.statusOnline
                                    : SmithColors.textSecondary,
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                    if (trailing != null) ...[
                      const SizedBox(width: 16),
                      trailing!,
                    ],
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    ),
  );
}
