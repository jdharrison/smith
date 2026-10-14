import 'package:flutter/foundation.dart';

import 'control_api.dart';

enum BootPage { loader, setup, home }

typedef ConfirmEffects = Future<bool> Function(
  String action,
  ControlReply reply,
);

class BootController extends ChangeNotifier {
  BootController(this.api);
  final ControlApi api;
  BootPage page = BootPage.loader;
  ControlReply? observation;
  String? error;
  bool busy = false;
  bool _disposed = false;
  bool _completedInSession = false;
  bool _needsRefresh = false;

  List<String> get actions {
    final state = observation;
    if (_needsRefresh ||
        state == null ||
        !state.ok ||
        [
          state.binary,
          state.unit,
          state.runtime,
          state.bridge,
        ].contains('conflict')) {
      return [];
    }
    if (state.binary == 'missing') return ['install-binary'];
    if (state.binary != 'installed') return [];
    if (state.unit == 'missing') return ['install-unit'];
    if (state.unit != 'installed') return [];
    // Schema v1 does not expose manager load/reload facts. Preparation remains
    // the authority for Start versus a consented registration/reload repair.
    if (state.runtime == 'stopped') return ['start', 'install-unit'];
    if (state.runtime == 'running' &&
        state.bridge == 'ready' &&
        state.setupComplete == false) {
      return ['complete-setup'];
    }
    return [];
  }

  void _observe(ControlReply reply) {
    observation = reply;
    if (reply.hasPersistedSetup) _completedInSession = true;
    page = _completedInSession ? BootPage.home : BootPage.setup;
    if (!reply.ok) error = reply.error ?? 'Helper reported failure.';
  }

  Future<void> refresh() async {
    if (_disposed || busy) return;
    busy = true;
    error = null;
    notifyListeners();
    try {
      final reply = await api.inspect();
      if (!_disposed) {
        _observe(reply);
        _needsRefresh = false;
      }
    } on Object catch (failure) {
      if (!_disposed) {
        observation = null;
        page = _completedInSession ? BootPage.home : BootPage.setup;
        error = _failureMessage(failure);
      }
    } finally {
      if (!_disposed) {
        busy = false;
        notifyListeners();
      }
    }
  }

  Future<void> act(String action, ConfirmEffects confirm) async {
    if (_disposed || busy || !actions.contains(action)) return;
    busy = true;
    error = null;
    notifyListeners();
    try {
      final proposal = await api.prepare(action);
      if (_disposed) return;
      if (!proposal.ok) {
        _observe(proposal);
        return;
      }
      final token = proposal.approvalToken;
      if (token == null || token.isEmpty) {
        throw const ControlFailure(
          'Preparation did not provide an approval token.',
        );
      }
      final approved = await confirm(action, proposal);
      if (_disposed || !approved) return;
      // The helper revalidates this exact action/token before any mutation.
      final result = await api.apply(action, token);
      if (!_disposed) _observe(result);
    } on Object catch (failure) {
      if (!_disposed) {
        _needsRefresh = true;
        if (failure is ControlFailure && failure.outcomeUnknown) {
          observation = null;
        }
        error = _failureMessage(failure);
      }
    } finally {
      if (!_disposed) {
        busy = false;
        notifyListeners();
      }
    }
  }

  String _failureMessage(Object failure) => failure is ControlFailure
      ? failure.message
      : 'Service control failed. Refresh to inspect current state.';

  @override
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    api.dispose();
    super.dispose();
  }
}
