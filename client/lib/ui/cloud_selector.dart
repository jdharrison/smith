import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../cloud_routing.dart';
import '../workspace_controller.dart';

Future<TransferDecision> confirmCloudTransfer(
  BuildContext context,
  RoutePreparation preparation,
) async =>
    await showDialog<TransferDecision>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Transfer conversation context'),
        content: SingleChildScrollView(
          child: SelectableText(preparation.disclosure),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, TransferDecision.newChat),
            child: const Text('New chat'),
          ),
          TextButton(
            onPressed: () => Navigator.pop(context, TransferDecision.cancel),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, TransferDecision.send),
            child: const Text('Send'),
          ),
        ],
      ),
    ) ??
    TransferDecision.cancel;

RouteChoice? _choiceFor(RouteCatalog? catalog, ChatRoute? route) =>
    catalog?.choices.where((choice) => choice.route == route).firstOrNull;

String _summary(RouteCatalog? catalog, ChatRoute? route) {
  if (route == null) return 'Choose model';
  final choice = _choiceFor(catalog, route);
  return '${route.cloud ? 'CLOUD' : 'LOCAL'} · ${choice?.label ?? route.label}${choice?.available == true ? '' : ' · unavailable'}';
}

String? _status(RouteCatalog? catalog, RouteChoice? choice) {
  if (choice?.available != true) {
    return choice?.reason ??
        'Unavailable. History is retained. Choose a replacement or Retry; manage saved keys in the installer.';
  }
  if (!choice!.route.cloud) return null;
  final provider = catalog!.providers.firstWhere(
    (p) => p['provider'] == choice.route.provider,
  );
  final validation = provider['validation'];
  final lines = <String>[
    if (!choice.validated)
      'Not individually verified'
    else
      'Past validation succeeded for ${choice.route.model};\nnot a current availability guarantee.',
    if (validation == null) 'Key saved. Validation is optional;\nSend checks credentials when attempted.',
    if (validation != null && validation['status'] != 'succeeded')
      'Last validation ${validation['status']} for ${validation['model']}: ${validation['error']['code']}.\nA new explicit attempt is available.',
    if (validation?['delivery'] == 'may_have_sent') 'The previous validation may have been processed or billed.\nNothing is retried automatically.',
    switch (provider['credential_state']) {
      'not_observed' => 'Credential access has not been checked since restart.\nSend checks it when attempted.',
      'locked' => 'Last credential check: keyring locked.\nUnlock externally, then Send or Validate again.',
      'missing' => 'Last credential check: saved item missing.\nA new attempt checks again; repair externally or explicitly replace the key if needed.',
      'conflict' => 'Last credential check: keyring conflict.\nRepair externally; a new explicit attempt checks again.',
      'unavailable' =>
        'Last credential check: ${provider['credential_error']['code']}.\nRepair externally if needed; Send or Validate checks again.',
      _ => '',
    },
  ]..removeWhere((line) => line.isEmpty);
  return lines.join('\n');
}

KeyEventResult _ignoreRepeatedEnter(FocusNode node, KeyEvent event) {
  final enter =
      event.logicalKey == LogicalKeyboardKey.enter ||
      event.logicalKey == LogicalKeyboardKey.numpadEnter;
  return enter && event is! KeyDownEvent
      ? KeyEventResult.handled
      : KeyEventResult.ignored;
}

class CloudSelector extends StatefulWidget {
  const CloudSelector({
    super.key,
    required this.controller,
    required this.active,
  });
  final ConversationController controller;
  final bool active;

  @override
  State<CloudSelector> createState() => _CloudSelectorState();
}

class _CloudSelectorState extends State<CloudSelector> {
  final triggerFocus = FocusNode(debugLabel: 'Choose chat model');
  DialogRoute<ChatRoute>? picker;
  int originRevision = 0;

  void dismissPicker() {
    final route = picker;
    if (route == null) return;
    // A tile can be replaced or hidden during its parent's build/disposal.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (route.isActive) route.navigator?.removeRoute(route);
    });
  }

  @override
  void didUpdateWidget(covariant CloudSelector oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != widget.controller ||
        oldWidget.active != widget.active) {
      originRevision++;
      dismissPicker();
    }
  }

  Future<void> chooseModel() async {
    if (!widget.active || picker != null) return;
    final origin = widget.controller;
    final revision = originRevision;
    final routeRevision = origin.routeRevision;
    final instance = origin.owner.instanceId;
    final configRevision = origin.owner.config['revision'];
    final conversation = origin.id;
    bool current() =>
        mounted &&
        revision == originRevision &&
        widget.active &&
        identical(widget.controller, origin) &&
        origin.routeRevision == routeRevision &&
        origin.owner.instanceId == instance &&
        origin.owner.config['revision'] == configRevision &&
        origin.id == conversation &&
        !origin.archived &&
        !origin.owner.queueExitPending &&
        origin.owner.all.contains(origin) &&
        origin.owner.panes.any((pane) => identical(pane.selected, origin));

    triggerFocus.requestFocus();
    final route = DialogRoute<ChatRoute>(
      context: context,
      builder: (context) => _ModelPicker(controller: origin, current: current),
    );
    picker = route;
    try {
      final selected = await Navigator.of(
        context,
        rootNavigator: true,
      ).push(route);
      if (selected != null &&
          current() &&
          origin.owner.routeCatalog?.available(selected) == true &&
          selected != origin.selectedRoute) {
        origin.selectRoute(selected);
      }
    } finally {
      picker = null;
      if (mounted &&
          revision == originRevision &&
          widget.active &&
          identical(widget.controller, origin) &&
          ModalRoute.of(context)?.isCurrent == true) {
        triggerFocus.requestFocus();
      }
    }
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: widget.controller,
    builder: (context, _) {
      final c = widget.controller;
      final catalog = c.owner.routeCatalog;
      final selected = c.selectedRoute;
      final summary = _summary(catalog, selected);
      return SizedBox(
        height: 48,
        child: Row(
          children: [
            Expanded(
              child: Focus(
                skipTraversal: true,
                onKeyEvent: _ignoreRepeatedEnter,
                child: Tooltip(
                  message:
                      '$summary\nRoute for future sends only. Accepted work keeps its original route.\n${_status(catalog, _choiceFor(catalog, selected)) ?? ''}',
                  child: TextButton(
                    key: const ValueKey('chat-route-selector'),
                    focusNode: triggerFocus,
                    onPressed: widget.active ? chooseModel : null,
                    child: Row(
                      children: [
                        Expanded(
                          child: Text(
                            summary,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                        const Icon(Icons.expand_more),
                      ],
                    ),
                  ),
                ),
              ),
            ),
            if (c.messageQueue.paused && selected?.cloud == true)
              IconButton(
                tooltip: 'Review queued cloud send',
                icon: const Icon(Icons.fact_check_outlined),
                onPressed: widget.active
                    ? () => c.owner.reviewQueuedCloud(
                        c,
                        (p) => confirmCloudTransfer(context, p),
                      )
                    : null,
              ),
          ],
        ),
      );
    },
  );

  @override
  void dispose() {
    originRevision++;
    dismissPicker();
    triggerFocus.dispose();
    super.dispose();
  }
}

class _ModelPicker extends StatefulWidget {
  const _ModelPicker({required this.controller, required this.current});
  final ConversationController controller;
  final bool Function() current;

  @override
  State<_ModelPicker> createState() => _ModelPickerState();
}

class _ModelPickerState extends State<_ModelPicker> {
  final query = TextEditingController();
  final searchFocus = FocusNode(debugLabel: 'Filter chat models');

  void choose(ChatRoute route) {
    if (mounted &&
        ModalRoute.of(context)?.isCurrent == true &&
        widget.current() &&
        widget.controller.owner.routeCatalog?.available(route) == true) {
      Navigator.pop(context, route);
    }
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: widget.controller.owner,
    builder: (context, _) {
      final c = widget.controller;
      final catalog = c.owner.routeCatalog;
      final selected = c.selectedRoute;
      final filter = query.text.trim().toLowerCase();
      final choices = (catalog?.choices ?? <RouteChoice>[]).where((choice) {
        final route = choice.route;
        return [
          choice.label,
          route.model,
          route.provider,
          route.cloud ? 'cloud' : 'local',
        ].any((field) => field.toLowerCase().contains(filter));
      }).toList();
      final current = widget.current();
      final selectedChoice = _choiceFor(catalog, selected);
      final selectedStatus = _status(catalog, selectedChoice);
      return Dialog(
        insetPadding: const EdgeInsets.all(12),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(6)),
        child: SizedBox(
          width: 560,
          height: 600,
          child: Padding(
            padding: const EdgeInsets.all(12),
            child: Focus(
              onKeyEvent: _ignoreRepeatedEnter,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Text(
                    'Choose model',
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                  const SizedBox(height: 8),
                  TextField(
                    key: const ValueKey('chat-route-filter'),
                    controller: query,
                    focusNode: searchFocus,
                    autofocus: true,
                    textInputAction: TextInputAction.search,
                    onChanged: (_) => setState(() {}),
                    // Filtering is not selection, even with exactly one match.
                    onSubmitted: (_) => searchFocus.requestFocus(),
                    decoration: InputDecoration(
                      hintText: 'Filter models',
                      contentPadding: const EdgeInsets.all(8),
                      suffixIcon: IconButton(
                        tooltip: 'Clear filter',
                        onPressed: query.text.isEmpty
                            ? null
                            : () {
                                query.clear();
                                setState(() {});
                                searchFocus.requestFocus();
                              },
                        icon: const Icon(Icons.clear),
                      ),
                    ),
                  ),
                  const SizedBox(height: 8),
                  Expanded(
                    child: ListView(
                      key: const ValueKey('chat-route-results'),
                      children: [
                        Padding(
                          padding: const EdgeInsets.symmetric(vertical: 8),
                          child: Column(
                            key: const ValueKey('chat-route-current'),
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            children: [
                              Text('Current: ${_summary(catalog, selected)}'),
                              if (selected != null && selectedStatus != null)
                                for (final line in selectedStatus.split('\n'))
                                  Text(
                                    line,
                                    style: Theme.of(context)
                                        .textTheme
                                        .bodySmall,
                                  ),
                            ],
                          ),
                        ),
                        if (!current)
                          const Text(
                            'Model choices changed. Close and reopen the picker.',
                          ),
                        if (catalog == null)
                          const Text(
                            'Model catalog unavailable. Close and Retry to observe models.',
                          ),
                        if (catalog != null && catalog.choices.isEmpty)
                          const Text(
                            'No models available in the observed catalog.',
                          ),
                        if (catalog != null &&
                            catalog.choices.isNotEmpty &&
                            choices.isEmpty)
                          const Text('No matching models'),
                        for (final cloud in [false, true])
                          if (choices.any(
                            (choice) => choice.route.cloud == cloud,
                          )) ...[
                            Padding(
                              padding: const EdgeInsets.symmetric(vertical: 8),
                              child: Text(
                                cloud ? 'CLOUD' : 'LOCAL',
                                style: Theme.of(context).textTheme.labelSmall,
                              ),
                            ),
                            for (final choice in choices.where(
                              (choice) => choice.route.cloud == cloud,
                            ))
                              Semantics(
                                selected: choice.route == selected,
                                child: TextButton(
                                  key: ValueKey(choice.route),
                                  onPressed: current && choice.available
                                      ? () => choose(choice.route)
                                      : null,
                                  style: TextButton.styleFrom(
                                    alignment: Alignment.centerLeft,
                                    padding: const EdgeInsets.all(12),
                                  ),
                                  child: Column(
                                    crossAxisAlignment:
                                        CrossAxisAlignment.stretch,
                                    children: [
                                      Text(choice.label),
                                      Text(
                                        choice.route.label,
                                        style: Theme.of(context)
                                            .textTheme
                                            .bodySmall,
                                      ),
                                      if (_status(catalog, choice)
                                          case final String status)
                                        for (final line in status.split('\n'))
                                          Text(
                                            line,
                                            style: Theme.of(context)
                                                .textTheme
                                                .bodySmall,
                                          ),
                                    ],
                                  ),
                                ),
                              ),
                          ],
                      ],
                    ),
                  ),
                  Align(
                    alignment: Alignment.centerRight,
                    child: TextButton(
                      onPressed: () => Navigator.pop(context),
                      child: const Text('Cancel'),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      );
    },
  );

  @override
  void dispose() {
    query.dispose();
    searchFocus.dispose();
    super.dispose();
  }
}
