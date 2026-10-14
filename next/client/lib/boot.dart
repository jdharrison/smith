import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';

import 'control_api.dart';
import 'native_control.dart';
import 'text_chat.dart';

const defaultBootObservationBudget = Duration(seconds: 15);
const defaultBootPollInterval = Duration(milliseconds: 500);
const bootAnimationDuration = Duration(milliseconds: 2400);

enum BootPhase {
  observing,
  syncing,
  ready,
  blocked,
  preparingStart,
  confirmingStart,
  applyingStart,
}

class BootState {
  const BootState({
    required this.phase,
    required this.status,
    this.detail,
    this.canStart = false,
    this.confirmation,
  });

  final BootPhase phase;
  final String status;
  final String? detail;
  final bool canStart;
  final String? confirmation;

  bool get isWaiting =>
      phase == BootPhase.observing ||
      phase == BootPhase.syncing ||
      phase == BootPhase.preparingStart ||
      phase == BootPhase.applyingStart;
}

abstract class BootClock {
  Duration get elapsed;
  Future<void> wait(Duration duration);
}

class SystemBootClock implements BootClock {
  SystemBootClock() {
    _stopwatch.start();
  }

  final Stopwatch _stopwatch = Stopwatch();

  @override
  Duration get elapsed => _stopwatch.elapsed;

  @override
  Future<void> wait(Duration duration) => Future<void>.delayed(duration);
}

class BootCoordinator extends ChangeNotifier {
  BootCoordinator({
    required this.control,
    required this.chat,
    BootClock? clock,
    this.observationBudget = defaultBootObservationBudget,
    this.pollInterval = defaultBootPollInterval,
  }) : clock = clock ?? SystemBootClock();

  final ControlApi control;
  final ChatController chat;
  final BootClock clock;
  final Duration observationBudget;
  final Duration pollInterval;

  BootState state = const BootState(
    phase: BootPhase.observing,
    status: 'Starting Smith',
  );
  bool _closed = false;
  bool _observing = false;
  bool _acting = false;
  ControlReply? _lastFacts;
  String? _preparedToken;

  Future<void> launch() => observe();

  Future<void> observe() async {
    if (_closed || _observing || _acting) return;
    _observing = true;
    _preparedToken = null;
    chat.unavailable();
    _set(const BootState(phase: BootPhase.observing, status: 'Starting Smith'));
    final started = clock.elapsed;
    ControlReply? latest;
    Object? lastFailure;
    try {
      while (!_closed) {
        final elapsed = clock.elapsed - started;
        final remaining = observationBudget - elapsed;
        if (remaining <= Duration.zero) break;

        // The dedicated native inspect executor has a two-second cleanup bound.
        // Reserve it so a timed-out helper remains owned and joined within the
        // total Boot observation budget instead of being raced or abandoned.
        final cleanupReserve =
            bootInspectCleanupGrace + bootInspectKillConfirmation;
        if (remaining <= cleanupReserve) {
          await clock.wait(remaining);
          break;
        }
        final available = remaining - cleanupReserve;
        final inspectDeadline = available < bootInspectDeadline
            ? available
            : bootInspectDeadline;
        try {
          latest = await control.inspect(deadline: inspectDeadline);
          if (_closed) return;
          _lastFacts = latest;
          lastFailure = null;
          if (latest.chatReady) {
            await _synchronizeConversation();
            return;
          }
          final terminal = _terminalState(latest);
          if (terminal != null) {
            _set(terminal);
            return;
          }
          _set(
            BootState(
              phase: BootPhase.observing,
              status: latest.runtime == 'running'
                  ? 'Connecting to Smith'
                  : 'Starting Smith',
            ),
          );
        } on Object catch (error) {
          lastFailure = error;
        }

        final afterInspect = clock.elapsed - started;
        final waitRemaining = observationBudget - afterInspect;
        if (waitRemaining <= Duration.zero) break;
        await clock.wait(
          waitRemaining < pollInterval ? waitRemaining : pollInterval,
        );
      }
      if (_closed) return;
      _set(_boundedOutcome(latest, lastFailure));
    } finally {
      _observing = false;
    }
  }

  Future<void> _synchronizeConversation() async {
    _set(
      const BootState(phase: BootPhase.syncing, status: 'Syncing conversation'),
    );
    chat.connected = true;
    await chat.synchronize();
    if (_closed) return;
    if (chat.stale) {
      chat.unavailable();
      _set(
        const BootState(
          phase: BootPhase.blocked,
          status: 'Conversation unavailable',
          detail:
              'Observe Smith again to synchronize saved conversation state.',
        ),
      );
      return;
    }
    final synchronized = !chat.stale;
    if (chat.pendingId == null) await chat.refreshModels();
    if (_closed) return;
    if (synchronized) {
      _set(
        const BootState(phase: BootPhase.ready, status: 'Connected to Smith'),
      );
    }
  }

  BootState? _terminalState(ControlReply facts) {
    if (!facts.ok) {
      return BootState(
        phase: BootPhase.blocked,
        status: 'Smith status is unavailable',
        detail:
            facts.error ?? 'Observe again after checking the local service.',
      );
    }
    if (facts.binary == 'missing') {
      return const BootState(
        phase: BootPhase.blocked,
        status: 'Smith is not installed',
        detail: 'Install Smith with the separate smith-installer.',
      );
    }
    if (facts.binary == 'conflict' || facts.unit == 'conflict') {
      return const BootState(
        phase: BootPhase.blocked,
        status: 'Smith installation conflict',
        detail: 'Open smith-installer to inspect and repair the installation.',
      );
    }
    if (facts.unit == 'missing' ||
        facts.enablement == EnablementState.missing) {
      return const BootState(
        phase: BootPhase.blocked,
        status: 'Smith service is not registered',
        detail: 'Register Smith with the separate smith-installer.',
      );
    }
    if (facts.enablement == EnablementState.disabled) {
      return const BootState(
        phase: BootPhase.blocked,
        status: 'Smith is disabled at login',
        detail: 'Enable Smith with the separate smith-installer, then observe again.',
      );
    }
    if (facts.enablement == EnablementState.conflict ||
        facts.runtime == 'conflict' ||
        facts.bridge == 'conflict') {
      return const BootState(
        phase: BootPhase.blocked,
        status: 'Smith service conflict',
        detail: 'Inspect the service with smith-installer before continuing.',
      );
    }
    if (facts.enablement == EnablementState.unknown ||
        facts.binary == 'unknown' ||
        facts.unit == 'unknown' ||
        facts.runtime == 'unknown' ||
        facts.bridge == 'unknown') {
      return const BootState(
        phase: BootPhase.blocked,
        status: 'Smith status is unknown',
        detail: 'Observe again after checking the local service.',
      );
    }
    return null;
  }

  BootState _boundedOutcome(ControlReply? facts, Object? failure) {
    if (facts != null && _canStart(facts)) {
      return const BootState(
        phase: BootPhase.blocked,
        status: 'Smith is stopped',
        detail:
            'Smith is installed and enabled at login, but is not running now.',
        canStart: true,
      );
    }
    final terminal = facts == null ? null : _terminalState(facts);
    if (terminal != null) return terminal;
    if (facts?.runtime == 'running') {
      return const BootState(
        phase: BootPhase.blocked,
        status: 'Smith is not accepting connections',
        detail:
            'The service is running. Observe again after checking its bridge.',
      );
    }
    return BootState(
      phase: BootPhase.blocked,
      status: 'Smith status is unavailable',
      detail: failure == null
          ? 'Observe again after checking the local service.'
          : 'A read-only observation did not complete. Observe again; no action was retried.',
    );
  }

  bool _canStart(ControlReply facts) =>
      facts.ok &&
      facts.binary == 'installed' &&
      facts.unit == 'installed' &&
      facts.enablement == EnablementState.enabled &&
      facts.runtime == 'stopped' &&
      facts.bridge == 'unavailable';

  Future<void> prepareStart() async {
    if (_closed || _acting || !state.canStart || _lastFacts == null) return;
    _acting = true;
    _set(
      const BootState(
        phase: BootPhase.preparingStart,
        status: 'Preparing Start Smith',
      ),
    );
    try {
      final prepared = await control.prepare('start');
      if (_closed) return;
      if (!prepared.ok || prepared.approvalToken == null) {
        _set(
          BootState(
            phase: BootPhase.blocked,
            status: 'Start Smith unavailable',
            detail: prepared.error ?? prepared.message,
          ),
        );
        return;
      }
      _preparedToken = prepared.approvalToken;
      _set(
        BootState(
          phase: BootPhase.confirmingStart,
          status: 'Confirm Start Smith',
          confirmation: prepared.effects['description'] as String,
        ),
      );
    } on Object {
      if (!_closed) await _freshObservationAfterUnknown();
    } finally {
      _acting = false;
    }
  }

  void cancelStartConfirmation() {
    if (_closed || state.phase != BootPhase.confirmingStart) return;
    _preparedToken = null;
    final facts = _lastFacts;
    _set(
      facts != null && _canStart(facts)
          ? const BootState(
              phase: BootPhase.blocked,
              status: 'Smith is stopped',
              detail: 'Smith is installed and enabled at login, but is not running now.',
              canStart: true,
            )
          : const BootState(
              phase: BootPhase.blocked,
              status: 'Smith status changed',
              detail: 'Observe again before taking action.',
            ),
    );
  }

  Future<void> applyPreparedStart() async {
    if (_closed || _acting || state.phase != BootPhase.confirmingStart) return;
    final token = _preparedToken;
    if (token == null) return;
    _preparedToken = null;
    _acting = true;
    _set(
      const BootState(phase: BootPhase.applyingStart, status: 'Starting Smith'),
    );
    try {
      await control.apply('start', token);
    } on Object {
      // An action failure or timeout is not interpreted as stopped or started.
      // Fresh inspection below is the only recovery path.
    } finally {
      _acting = false;
    }
    if (!_closed) await observe();
  }

  Future<void> _freshObservationAfterUnknown() async {
    _acting = false;
    await observe();
    _acting = true;
  }

  void _set(BootState value) {
    if (_closed) return;
    state = value;
    notifyListeners();
  }

  @override
  void dispose() {
    _closed = true;
    super.dispose();
  }
}

class BootSurface extends StatefulWidget {
  const BootSurface({super.key, required this.coordinator});

  final BootCoordinator coordinator;

  @override
  State<BootSurface> createState() => _BootSurfaceState();
}

class _BootSurfaceState extends State<BootSurface>
    with SingleTickerProviderStateMixin {
  late final AnimationController animation = AnimationController(
    vsync: this,
    duration: bootAnimationDuration,
  );
  bool dialogOpen = false;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _syncMotion();
  }

  @override
  void didUpdateWidget(BootSurface oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.coordinator != widget.coordinator) {
      oldWidget.coordinator.removeListener(_changed);
      widget.coordinator.addListener(_changed);
    }
    _syncMotion();
  }

  @override
  void initState() {
    super.initState();
    widget.coordinator.addListener(_changed);
  }

  void _changed() {
    if (!mounted) return;
    _syncMotion();
    setState(() {});
    if (widget.coordinator.state.phase == BootPhase.confirmingStart &&
        !dialogOpen) {
      unawaited(_showStartConfirmation());
    }
  }

  void _syncMotion() {
    final disabled = MediaQuery.maybeOf(context)?.disableAnimations ?? false;
    if (disabled || !widget.coordinator.state.isWaiting) {
      animation.stop();
      if (disabled) animation.value = 0.12;
    } else if (!animation.isAnimating) {
      animation.repeat();
    }
  }

  Future<void> _showStartConfirmation() async {
    dialogOpen = true;
    final confirmation = widget.coordinator.state.confirmation!;
    final approved = await showDialog<bool>(
      context: context,
      barrierDismissible: false,
      builder: (context) => AlertDialog(
        title: const Text('Start Smith?'),
        content: Text(confirmation, key: const ValueKey('start-confirmation')),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('Start Smith'),
          ),
        ],
      ),
    );
    dialogOpen = false;
    if (!mounted) return;
    if (approved == true) {
      await widget.coordinator.applyPreparedStart();
    } else {
      widget.coordinator.cancelStartConfirmation();
    }
  }

  @override
  void dispose() {
    widget.coordinator.removeListener(_changed);
    animation.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final state = widget.coordinator.state;
    return Scaffold(
      key: const ValueKey('boot-surface'),
      body: SafeArea(
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(24),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 420),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  AnimatedBuilder(
                    animation: animation,
                    builder: (context, _) => CustomPaint(
                      key: const ValueKey('boot-mark'),
                      size: const Size.square(112),
                      painter: BootMarkPainter(animation.value),
                    ),
                  ),
                  const SizedBox(height: 28),
                  Semantics(
                    key: const ValueKey('boot-status-semantics'),
                    container: true,
                    liveRegion: true,
                    excludeSemantics: true,
                    label: state.status,
                    child: Text(
                      state.status,
                      key: const ValueKey('boot-status'),
                      textAlign: TextAlign.center,
                      style: Theme.of(context).textTheme.titleMedium,
                    ),
                  ),
                  if (state.detail != null) ...[
                    const SizedBox(height: 12),
                    Text(
                      state.detail!,
                      textAlign: TextAlign.center,
                      style: Theme.of(context).textTheme.bodyMedium,
                    ),
                  ],
                  if (state.canStart) ...[
                    const SizedBox(height: 24),
                    FilledButton(
                      onPressed: state.phase == BootPhase.blocked
                          ? widget.coordinator.prepareStart
                          : null,
                      child: const Text('Start Smith'),
                    ),
                  ] else if (state.phase == BootPhase.blocked) ...[
                    const SizedBox(height: 24),
                    OutlinedButton(
                      onPressed: widget.coordinator.observe,
                      child: const Text('Observe again'),
                    ),
                  ],
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class BootMarkPainter extends CustomPainter {
  const BootMarkPainter(this.progress);

  final double progress;

  Path _loop(Size size) {
    final center = size.center(Offset.zero);
    final radius = size.shortestSide * 0.36;
    final points = List.generate(6, (index) {
      final angle = -math.pi / 2 + index * math.pi / 3;
      return center + Offset(math.cos(angle), math.sin(angle)) * radius;
    });
    final path = Path();
    const corner = 0.22;
    for (var index = 0; index < points.length; index++) {
      final previous = points[(index + points.length - 1) % points.length];
      final current = points[index];
      final next = points[(index + 1) % points.length];
      final enter = Offset.lerp(current, previous, corner)!;
      final leave = Offset.lerp(current, next, corner)!;
      if (index == 0) {
        path.moveTo(enter.dx, enter.dy);
      } else {
        path.lineTo(enter.dx, enter.dy);
      }
      path.quadraticBezierTo(current.dx, current.dy, leave.dx, leave.dy);
    }
    path.close();
    return path;
  }

  @override
  void paint(Canvas canvas, Size size) {
    final path = _loop(size);
    final base = Paint()
      ..color = const Color(0xFF66736B)
      ..style = PaintingStyle.stroke
      ..strokeWidth = 5
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round;
    canvas.drawPath(path, base);

    final metric = path.computeMetrics().single;
    final segment = metric.length * 0.18;
    final start = metric.length * progress;
    final highlight = Paint()
      ..color = const Color(0xFFE3E9E5)
      ..style = PaintingStyle.stroke
      ..strokeWidth = 5
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round;
    final end = start + segment;
    if (end <= metric.length) {
      canvas.drawPath(metric.extractPath(start, end), highlight);
    } else {
      canvas.drawPath(metric.extractPath(start, metric.length), highlight);
      canvas.drawPath(metric.extractPath(0, end - metric.length), highlight);
    }
  }

  @override
  bool shouldRepaint(BootMarkPainter oldDelegate) =>
      oldDelegate.progress != progress;
}
