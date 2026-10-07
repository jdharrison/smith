import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:smith_client/home/halo_presentation.dart';
import 'package:smith_client/home/halo_theme.dart';

class SmithHalo extends StatefulWidget {
  const SmithHalo({
    super.key,
    required this.state,
    this.size = 220,
    this.semanticStatus = true,
  });

  final HaloState state;
  final double size;
  final bool semanticStatus;

  @override
  State<SmithHalo> createState() => _SmithHaloState();
}

class _SmithHaloState extends State<SmithHalo>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: const Duration(seconds: 7),
  );

  bool get _moves => switch (widget.state) {
    HaloState.listening ||
    HaloState.thinking ||
    HaloState.speaking ||
    HaloState.working => true,
    _ => false,
  };

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _syncAnimation();
  }

  @override
  void didUpdateWidget(covariant SmithHalo oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.state != widget.state) _syncAnimation();
  }

  void _syncAnimation() {
    final reduceMotion =
        MediaQuery.maybeOf(context)?.disableAnimations ?? false;
    if (_moves && !reduceMotion) {
      _controller.repeat();
    } else {
      _controller.stop();
      _controller.value = 0;
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final reduceMotion =
        MediaQuery.maybeOf(context)?.disableAnimations ?? false;
    final animation = reduceMotion
        ? const AlwaysStoppedAnimation(0.0)
        : _controller;
    final halo = SizedBox.square(
      dimension: widget.size,
      child: AnimatedBuilder(
        animation: animation,
        builder: (context, _) => CustomPaint(
          painter: _HaloPainter(progress: animation.value, state: widget.state),
        ),
      ),
    );
    if (!widget.semanticStatus) return ExcludeSemantics(child: halo);
    return Semantics(
      image: true,
      label: 'Smith status: ${haloStateLabel(widget.state)}',
      child: halo,
    );
  }
}

class _HaloPainter extends CustomPainter {
  const _HaloPainter({required this.progress, required this.state});

  final double progress;
  final HaloState state;

  @override
  void paint(Canvas canvas, Size size) {
    final center = size.center(Offset.zero);
    final radius = size.shortestSide / 2 - 3;
    final paint = Paint()
      ..color = switch (state) {
        HaloState.disconnected => HaloColors.secondary,
        HaloState.failed => HaloColors.danger,
        HaloState.review => HaloColors.warning,
        _ => HaloColors.accent,
      }
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2
      ..strokeCap = StrokeCap.round;

    final path = Path();
    const points = 180;
    for (var index = 0; index <= points; index++) {
      final angle = (index / points) * math.pi * 2;
      final edge = _edgeOffset(angle);
      final point = Offset(
        center.dx + math.cos(angle) * (radius + edge),
        center.dy + math.sin(angle) * (radius + edge),
      );
      if (index == 0) {
        path.moveTo(point.dx, point.dy);
      } else {
        path.lineTo(point.dx, point.dy);
      }
    }
    path.close();
    canvas.drawPath(path, paint);
  }

  double _edgeOffset(double angle) {
    final moving = switch (state) {
      HaloState.listening ||
      HaloState.thinking ||
      HaloState.speaking ||
      HaloState.working => 1.0,
      _ => 0.0,
    };
    final waveCenter = 0.72 + progress * math.pi * 2;
    final distance = math.atan2(
      math.sin(angle - waveCenter),
      math.cos(angle - waveCenter),
    );
    final localized = math.exp(-distance * distance / 0.12);
    final wave = math.sin(angle * 9 + progress * math.pi * 2) * 5;
    return localized * wave * moving;
  }

  @override
  bool shouldRepaint(covariant _HaloPainter oldDelegate) =>
      oldDelegate.progress != progress || oldDelegate.state != state;
}
