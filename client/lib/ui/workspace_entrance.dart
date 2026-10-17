import 'package:flutter/material.dart';

const workspaceEntranceDuration = Duration(milliseconds: 620);

enum WorkspaceRegion {
  header(0, 220, Offset(0, -4)),
  navigation(60, 220, Offset(-6, 0)),
  sidebar(120, 220, Offset(-8, 0)),
  title(180, 220, Offset(0, 4)),
  transcript(240, 260, Offset(0, 6)),
  settings(240, 260, Offset(0, 6)),
  composer(340, 280, Offset(0, 8));

  const WorkspaceRegion(this.delayMs, this.durationMs, this.offset);

  final int delayMs;
  final int durationMs;
  final Offset offset;

  double progress(double timeline) => Curves.easeOutCubic.transform(
    ((timeline * workspaceEntranceDuration.inMilliseconds - delayMs) /
            durationMs)
        .clamp(0.0, 1.0),
  );
}

/// One presentation-only clock, mounted after authoritative workspace hydration.
class WorkspaceEntrance extends StatefulWidget {
  const WorkspaceEntrance({
    super.key,
    required this.child,
    this.skipMotion = false,
  });

  final Widget child;
  final bool skipMotion;

  @override
  State<WorkspaceEntrance> createState() => _WorkspaceEntranceState();
}

class _WorkspaceEntranceState extends State<WorkspaceEntrance>
    with SingleTickerProviderStateMixin {
  late final AnimationController animation = AnimationController(
    vsync: this,
    duration: workspaceEntranceDuration,
    animationBehavior: AnimationBehavior.preserve,
  )..addListener(_stopAtEnd);
  bool started = false;

  void _stopAtEnd() {
    if (animation.value == 1) animation.stop();
  }

  void _settle() {
    started = true;
    animation.value = 1;
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (widget.skipMotion || MediaQuery.disableAnimationsOf(context)) {
      _settle();
    } else if (!started) {
      started = true;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && animation.value < 1) animation.forward();
      });
    }
  }

  @override
  void didUpdateWidget(WorkspaceEntrance oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.skipMotion && animation.value < 1) _settle();
  }

  @override
  void dispose() {
    animation.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => _WorkspaceEntranceScope(
    animation: animation,
    child: AnimatedBuilder(
      animation: animation,
      child: widget.child,
      builder: (context, child) {
        final entering = animation.value < 1;
        return IgnorePointer(
          key: const ValueKey('workspace-entrance-interaction'),
          ignoring: entering,
          child: ExcludeFocus(
            excluding: entering,
            child: ExcludeSemantics(excluding: entering, child: child),
          ),
        );
      },
    ),
  );
}

class _WorkspaceEntranceScope extends InheritedWidget {
  const _WorkspaceEntranceScope({
    required this.animation,
    required super.child,
  });

  final Animation<double> animation;

  @override
  bool updateShouldNotify(_WorkspaceEntranceScope oldWidget) =>
      animation != oldWidget.animation;
}

/// Keeps the real child mounted in its final layout; only its paint changes.
class WorkspaceReveal extends StatelessWidget {
  const WorkspaceReveal({super.key, required this.region, required this.child});

  final WorkspaceRegion region;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final scope = context
        .dependOnInheritedWidgetOfExactType<_WorkspaceEntranceScope>();
    if (scope == null) return child;
    return AnimatedBuilder(
      animation: scope.animation,
      child: child,
      builder: (context, child) {
        final progress = region.progress(scope.animation.value);
        return Opacity(
          key: ValueKey('workspace-reveal-${region.name}'),
          opacity: progress,
          child: Transform.translate(
            key: ValueKey('workspace-offset-${region.name}'),
            offset: region.offset * (1 - progress),
            child: child,
          ),
        );
      },
    );
  }
}
