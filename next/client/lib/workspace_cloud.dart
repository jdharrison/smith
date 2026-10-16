part of 'workspace_controller.dart';

extension _CloudWorkspace on WorkspaceController {
  void _validate(String op, Map<String, dynamic> value) {
    if (routingEnabled) {
      validateV3(op, value);
    } else {
      validateV2(op, value);
    }
  }

  Future<void> _readCatalog() async {
    if (!routingEnabled) return;
    final r = await api.call('route_catalog', {});
    if (_closed) return;
    _correlate(r);
    final catalog = RouteCatalog(r.result);
    if (catalog.configRevision != config['revision']) invalid();
    routeCatalog = catalog;
    for (final c in all) {
      if (c.selectedRoute == null &&
          c.id == null &&
          config['model'] is String) {
        c.selectedRoute = ChatRoute('local', config['model'], null);
      }
    }
  }

  Future<void> _readRouting(ConversationController c) async {
    if (!routingEnabled || c.id == null) return;
    final r = await api.call('conversation_routing', {'conversation_id': c.id});
    if (_closed) return;
    _correlate(r);
    final routing = ConversationRouting(r.result);
    if (routing.id != c.id ||
        routing.revision != c.summary?['history_revision'] ||
        routing.turns.length * 2 != c.history.length) {
      invalid();
    }
    c.routing = routing;
    if (!c.routeChosen && c.selectedRoute == null) {
      c.selectedRoute =
          routing.lastRoute ??
          (config['model'] is String
              ? ChatRoute('local', config['model'], null)
              : null);
    }
  }

  Future<void> _readRequestRoute(
    ConversationController c,
    String request,
  ) async {
    if (!routingEnabled) return;
    final r = await api.call('request_routing', {
      'request_id': request,
      'instance_id': instanceId,
    });
    if (_closed) return;
    _correlate(r);
    final routing = RequestRouting(r.result);
    if (routing.request != request) invalid();
    final old = c.requestRoutes[request];
    if (old != null && old.route != routing.route) invalid();
    c.requestRoutes[request] = routing;
    while (c.requestRoutes.length > 32) {
      c.requestRoutes.remove(c.requestRoutes.keys.first);
    }
  }

  Future<_SendIntent?> _prepareIntent(
    ConversationController c,
    _SendIntent intent,
    ConfirmTransfer? confirm, {
    bool queued = false,
  }) async {
    if (!routingEnabled || intent.route?.cloud != true) return intent;
    if (queueExitPending ||
        c.selectedRoute != intent.route ||
        c.routeRevision != intent.routeRevision ||
        (!queued && c.draftRevision != intent.draftRevision) ||
        instanceId != intent.instance ||
        config['revision'] != intent.configRevision ||
        c.id != intent.conversation ||
        c.summary?['history_revision'] != intent.historyRevision) {
      c.error = 'Send cancelled: its frozen text, route or context changed before preparation.';
      return null;
    }
    final revision = c.routeRevision;
    final draftRevision = c.draftRevision;
    final r = await api.call('route_prepare', intent.routeArguments);
    if (_closed) return null;
    _correlate(r);
    final preparation = RoutePreparation(r.result)
      ..match(intent.routeArguments);
    var decision = TransferDecision.send;
    if (preparation.requiresConsent) {
      if (confirm == null) {
        c._queue.latch(
          'Cloud destination needs explicit transfer consent. Use Review queued cloud send.',
        );
        return null;
      }
      decision = await confirm(preparation);
    }
    if (_closed || c._detached) return null;
    if (decision != TransferDecision.send) {
      if (decision == TransferDecision.newChat) {
        for (final pane in panes.where((p) => identical(p.selected, c))) {
          newChatFor(pane);
          break;
        }
      }
      return null;
    }
    if (queueExitPending ||
        revision != c.routeRevision ||
        (!queued && draftRevision != c.draftRevision) ||
        c.selectedRoute != intent.route ||
        instanceId != intent.instance ||
        config['revision'] != intent.configRevision ||
        c.id != intent.conversation ||
        c.summary?['history_revision'] != intent.historyRevision) {
      c.error = 'Send cancelled: text, route or context changed. Review and Send again.';
      return null;
    }
    return intent.withPreparation(preparation);
  }
}

extension CloudWorkspaceActions on WorkspaceController {
  /// Explicit Retry only. Probes read configured local keyring items, never APIs.
  Future<void> retryCloudProviders() => _serialize(() async {
    while (routingEnabled &&
        !_closed &&
        _globalKind == 'models' &&
        _globalRequest != null &&
        _globalObservation.elapsed < operationBudget) {
      await _observeOnce();
      if (_globalRequest != null) await Future<void>.delayed(pollInterval);
    }
    if (!routingEnabled ||
        _closed ||
        outstanding ||
        _globalRequest != null ||
        stale) {
      return;
    }
    await _readCatalog();
    for (final p in routeCatalog!.providers) {
      if (_closed || outstanding || _globalRequest != null) break;
      if (p['credential_generation'] == null ||
          p['credential_state'] == 'available') {
        continue;
      }
      _globalRequest = appId();
      _globalKind = 'provider_probe';
      _globalObservation.reset();
      _globalObservation.start();
      var admitted = false;
      try {
        final r = await api.call('provider_probe', {
          'request_id': _globalRequest,
          'instance_id': instanceId,
          'provider': p['provider'],
          'expected_provider_revision': p['revision'],
        });
        _correlate(r);
        validateV3('provider_probe', r.result);
        if (r.result['request_id'] != _globalRequest ||
            r.result['kind'] != _globalKind) {
          invalid();
        }
        admitted = true;
        while (!_closed &&
            _globalRequest != null &&
            _globalObservation.elapsed < operationBudget) {
          await _observeOnce();
          if (_globalRequest != null) await Future<void>.delayed(pollInterval);
        }
      } on Object catch (e) {
        // Only proven rejection of the admission exchange releases the local
        // reservation. Lost/malformed acknowledgements and storage uncertainty
        // retain the original identity for observation, never a new probe.
        if (!admitted &&
            e is AppFailure &&
            !e.unknown &&
            const {
              'BUSY',
              'STALE_REVISION',
              'INVALID_REQUEST',
              'CAPACITY',
              'PROVIDER_NOT_CONFIGURED',
              'CREDENTIAL_CHANGED',
            }.contains(e.code)) {
          _globalRequest = _globalKind = null;
          _globalObservation.stop();
        }
        error = '$e';
        break;
      }
      await _readCatalog();
    }
    changed();
    _schedule();
  });

  Future<void> reviewQueuedCloud(
    ConversationController c,
    ConfirmTransfer confirm,
  ) => _serialize(() async {
    final q = c._queue;
    if (_closed ||
        queueExitPending ||
        c.pendingId != null ||
        q._actionPending ||
        q.dispatching) {
      return;
    }
    final e = q._entries.firstOrNull;
    if (e == null ||
        e.state != QueuedMessageState.waiting ||
        e.route != c.selectedRoute) {
      return;
    }
    // This explicit action grants a dialog opportunity, not automatic approval.
    q._reason = null;
    await q.pump(confirm: confirm);
  });
}
