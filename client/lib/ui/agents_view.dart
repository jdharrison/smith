import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';

import '../agents.dart';
import '../agents_controller.dart';
import '../app_api.dart' show appId;
import 'smith_theme.dart';

/// Volatile, per-pane input. Only the desktop registry is shared.
class AgentsView extends StatefulWidget {
  const AgentsView({super.key, required this.controller});
  final AgentsController controller;

  @override
  State<AgentsView> createState() => _AgentsViewState();
}

class _AgentsViewState extends State<AgentsView> {
  final _nameFocus = FocusNode();
  bool _creating = false;
  bool _needsReview = false;
  final _fields = List.generate(3, (_) => TextEditingController());
  final _listScroll = _AgentScrollController();
  final _editorScroll = _AgentScrollController();
  AgentRecord? _original;
  String? _instance;
  int? _revision;
  int _epoch = 0;
  bool _editorOpen = false;
  bool _saving = false;
  bool _choosing = false;
  bool _validation = false;
  String? _error;
  _Submission? _submission;

  AgentsController get registry => widget.controller;
  List<String> get _values => _fields.map((field) => field.text).toList();
  List<String> _recordValues(AgentRecord record) => [
    record.name,
    record.description,
    record.prompt,
  ];
  bool _same(List<String> a, List<String> b) =>
      List.generate(a.length, (i) => a[i] == b[i]).every((v) => v);
  bool get _hasEditor => _creating || _original != null;
  bool get _dirty => _creating
      ? _values.any((value) => value.isNotEmpty)
      : _original != null && !_same(_values, _recordValues(_original!));
  bool get _locked => _original?.isBuiltIn ?? false;
  String? _validationError(int i) =>
      _locked && i < 2 ? null : _fieldError(i, _fields[i].text);
  bool get _unboundCreate => _submission?.id == null && _submission != null;
  AgentRecord? _record(String id) {
    for (final record in registry.agents) {
      if (record.agentId == id) return record;
    }
    return null;
  }

  bool get _stale =>
      _needsReview ||
      _hasEditor &&
          (_instance != registry.instanceId ||
              _revision != registry.revision ||
              (_original != null && _record(_original!.agentId) == null));
  bool get _writable =>
      registry.canMutate &&
      !registry.loading &&
      !_saving &&
      !_stale &&
      !_unboundCreate;

  @override
  void initState() {
    super.initState();
    registry.addListener(_changed);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && !registry.loaded && !registry.loading) {
        unawaited(_observe());
      }
    });
  }

  @override
  void didUpdateWidget(covariant AgentsView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != registry) {
      oldWidget.controller.removeListener(_changed);
      registry.addListener(_changed);
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && !registry.loaded && !registry.loading) {
          unawaited(_observe());
        }
      });
    }
  }

  void _changed() {
    if (!mounted) return;
    if (!_saving) _reconcile();
    if (!_saving &&
        _submission == null &&
        !_dirty &&
        !_creating &&
        !_needsReview &&
        _stale &&
        registry.loaded &&
        !registry.loading &&
        !registry.busy &&
        !registry.uncertain &&
        registry.error == null) {
      final current = _record(_original!.agentId);
      if (current != null) _load(current);
    }
    setState(() {});
  }

  void _load(AgentRecord record) {
    _epoch++;
    _original = record;
    _creating = false;
    _needsReview = false;
    _instance = registry.instanceId;
    _revision = registry.revision;
    final values = _recordValues(record);
    for (var i = 0; i < 3; i++) {
      if (_fields[i].text != values[i]) _fields[i].text = values[i];
    }
    _submission = null;
    _validation = false;
    _error = null;
  }

  Future<void> _observe({bool resolve = false}) async {
    try {
      if (resolve) {
        await registry.resolvePending();
      } else {
        await registry.refresh();
      }
    } on Object {
      if (mounted) {
        setState(
          () => _error = 'Metadata observation failed. Your edits are preserved; no change was retried.',
        );
      }
    }
  }

  Future<bool> _discard() async =>
      !_dirty ||
      await showDialog<bool>(
            context: context,
            builder: (context) => AlertDialog(
              scrollable: true,
              title: const Text('Discard unsaved agent changes?'),
              content: const Text(
                'These edits exist only in this view. Saved definitions will not change.',
              ),
              actions: [
                _action(
                  'Keep editing',
                  () => Navigator.pop(context, false),
                  autofocus: true,
                ),
                _action('Discard changes', () => Navigator.pop(context, true)),
              ],
            ),
          ) ==
          true;

  Future<void> _select(String id, {bool cancel = false}) async {
    if (_saving || _choosing || registry.uncertain || _unboundCreate) return;
    if (!cancel && id == _original?.agentId) {
      setState(() => _editorOpen = true);
      return;
    }
    _choosing = true;
    final epoch = _epoch;
    final discard = await _discard();
    _choosing = false;
    if (!mounted || _saving || epoch != _epoch) return;
    if (!discard) {
      setState(() => _editorOpen = true);
      return;
    }
    final record = _record(id);
    if (record == null) return;
    setState(() {
      _load(record);
      _editorOpen = true;
      if (_editorScroll.hasClients) _editorScroll.jumpTo(0);
    });
  }

  Future<void> _newAgent() async {
    if (_saving ||
        _choosing ||
        !registry.canMutate ||
        _unboundCreate ||
        registry.agents.length >= maxAgents) {
      return;
    }
    _choosing = true;
    final epoch = _epoch;
    final discard = await _discard();
    _choosing = false;
    if (!mounted ||
        !discard ||
        epoch != _epoch ||
        !registry.canMutate ||
        _unboundCreate) {
      return;
    }
    setState(() {
      _epoch++;
      _creating = true;
      _needsReview = false;
      _original = null;
      _instance = registry.instanceId;
      _revision = registry.revision;
      _submission = null;
      _validation = false;
      _error = null;
      for (final field in _fields) {
        field.clear();
      }
      _editorOpen = true;
      if (_editorScroll.hasClients) _editorScroll.jumpTo(0);
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && _creating && _editorOpen) _nameFocus.requestFocus();
    });
  }

  Future<void> _cancel() async {
    if (!_creating) {
      await _select(_original!.agentId, cancel: true);
      return;
    }
    if (_saving || _choosing || registry.uncertain || _unboundCreate) return;
    _choosing = true;
    final epoch = _epoch;
    final discard = await _discard();
    _choosing = false;
    if (!mounted || !discard || epoch != _epoch) return;
    setState(() {
      _epoch++;
      _creating = false;
      _needsReview = false;
      _editorOpen = false;
      _instance = null;
      _revision = null;
      _validation = false;
      _error = null;
      for (final field in _fields) {
        field.clear();
      }
    });
  }

  Future<void> _save() async {
    if (!_writable || !_dirty) return;
    setState(() => _validation = true);
    if (List.generate(3, _validationError).any((e) => e != null)) {
      return;
    }
    final submission = _Submission(
      _epoch,
      _instance!,
      _revision!,
      _original?.agentId,
      appId(),
      List.unmodifiable(_values),
    );
    _submission = submission;
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      if (submission.id == null) {
        await registry.create(
          submission.values[0],
          submission.values[1],
          submission.values[2],
          expectedInstanceId: submission.instance,
          expectedRevision: submission.revision,
          requestId: submission.requestId,
        );
      } else {
        await registry.update(
          submission.id!,
          submission.values[0],
          submission.values[1],
          submission.values[2],
          expectedInstanceId: submission.instance,
          expectedRevision: submission.revision,
          requestId: submission.requestId,
        );
      }
    } on Object {
      if (mounted) _error = 'Agent metadata could not be saved. Your edits are preserved; nothing is retried automatically.';
    } finally {
      if (mounted) {
        _saving = false;
        _reconcile();
        setState(() {});
      }
    }
  }

  void _reconcile() {
    final submitted = _submission;
    if (submitted == null ||
        registry.busy ||
        registry.loading ||
        registry.uncertain) {
      return;
    }
    if (submitted.epoch != _epoch || !registry.loaded) return;
    final receipt = registry.lastReceipt;
    if (receipt == null || receipt.requestId != submitted.requestId) {
      // A definitive rejection permits a corrected attempt, not target inference.
      if (registry.error != null) _submission = null;
      return;
    }
    final current = _record(receipt.agentId);
    if (submitted.id == null) {
      if (current == null || current.isBuiltIn) {
        _error = 'The earlier creation was acknowledged, but its saved profile is unavailable. New profiles and saves remain blocked. Retry loading to observe it; nothing will be resent.';
        return;
      }
      // Bind only the receipt's service-owned identity. Keep the original origin
      // until explicit review whenever the snapshot or local typing has moved.
      _submission = null;
      if (submitted.instance == registry.instanceId &&
          registry.revision == submitted.revision + 1 &&
          _same(_recordValues(current), submitted.values) &&
          _same(_values, submitted.values)) {
        _load(current);
      } else {
        _creating = false;
        _original = current;
        _needsReview = true;
        _error = 'The earlier creation was acknowledged. Your newer edits are preserved; review current metadata before saving them.';
      }
      return;
    }
    _submission = null;
    // Acknowledgement alone cannot bind a later snapshot or a restarted service.
    if (current == null ||
        submitted.instance != registry.instanceId ||
        registry.revision != submitted.revision + 1 ||
        !_same(_recordValues(current), submitted.values)) {
      _needsReview = true;
      _error = 'The earlier change was acknowledged. Review current metadata before saving again.';
      return;
    }
    if (_same(_values, submitted.values)) {
      _load(current);
    } else {
      _original = current;
      _needsReview = true;
      _error = 'The earlier change was acknowledged. Your newer edits are preserved; review current metadata before saving them.';
    }
  }

  Future<void> _review() async {
    if (!_stale || !registry.canMutate || registry.loading || _saving) return;
    final current = _original == null ? null : _record(_original!.agentId);
    if (current == null && !_creating) return;
    if (_unboundCreate) return;
    final revision = registry.revision;
    final instance = registry.instanceId;
    final epoch = _epoch;
    final values = _values;
    bool eligible() =>
        mounted &&
        epoch == _epoch &&
        !_saving &&
        registry.canMutate &&
        !registry.loading &&
        registry.revision == revision &&
        registry.instanceId == instance &&
        _same(values, _values);
    final accepted = await showDialog<bool>(
      context: context,
      builder: (context) => ListenableBuilder(
        listenable: registry,
        builder: (context, _) => AlertDialog(
          scrollable: true,
          title: const Text('Review current agent metadata'),
          content: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('Registry revision: $revision'),
              const Text(
                'Bound to the currently observed Smith instance. Review does not save.',
              ),
              Text(
                current == null
                    ? 'New unsaved profile'
                    : 'ID: ${current.agentId} (immutable)',
              ),
              for (var i = 0; i < 3; i++) ...[
                const SizedBox(height: 16),
                Text('${_labels[i]} — currently saved'),
                SelectableText(
                  current == null || _recordValues(current)[i].isEmpty
                      ? 'Not configured'
                      : _recordValues(current)[i],
                ),
                Text('${_labels[i]} — your unsaved edits'),
                SelectableText(
                  values[i].isEmpty ? 'Not configured' : values[i],
                ),
              ],
              if (!eligible())
                const Text(
                  'Smith or the registry changed. Close and review again.',
                ),
            ],
          ),
          actions: [
            _action(
              'Keep blocked',
              () => Navigator.pop(context, false),
              autofocus: true,
            ),
            _action(
              'Keep edits at this revision',
              eligible() ? () => Navigator.pop(context, true) : null,
            ),
          ],
        ),
      ),
    );
    if (accepted != true || !eligible()) return;
    setState(() {
      _epoch++;
      _original = current;
      _instance = instance;
      _revision = revision;
      _needsReview = false;
      _submission = null;
      _error = null;
    });
  }

  Widget _action(
    String text,
    VoidCallback? onPressed, {
    bool autofocus = false,
    bool primary = false,
  }) => TextButton(
    style: TextButton.styleFrom(
      minimumSize: const Size(48, 48),
      backgroundColor: primary ? SmithColors.accent : null,
      foregroundColor: primary ? SmithColors.background : null,
      disabledBackgroundColor: primary ? SmithColors.surfaceSelected : null,
    ),
    autofocus: autofocus,
    onPressed: onPressed,
    child: Text(text),
  );

  List<Widget> _status() => [
    if (registry.loading) const Text('Loading agents…'),
    if (!registry.loaded && !registry.loading)
      const Text('Agents unavailable. Retry loading to observe profiles.'),
    if (!registry.uncertain &&
        !registry.loading &&
        !registry.busy &&
        (!registry.loaded || registry.error != null || _unboundCreate))
      _action('Retry loading', () => _observe()),
    if (_saving || registry.busy) const Text('Saving agent metadata…'),
    if (registry.error != null) Text(registry.error!),
    if (_error != null) Text(_error!),
    if (registry.uncertain) ...[
      const Text(
        'Change outcome unknown. Writes are blocked. Resolution queries the original receipt and current definitions only; it never resends.',
      ),
      _action(
        'Resolve pending change',
        registry.busy || registry.loading
            ? null
            : () => _observe(resolve: true),
      ),
    ],
    if (_stale) ...[
      const Text(
        'Saved definitions or Smith instance changed. Your edits are preserved.',
      ),
      _action(
        'Review current metadata',
        registry.canMutate && !registry.loading && !_saving && !_unboundCreate
            ? _review
            : null,
      ),
    ],
  ];

  Widget _list(bool compact) {
    final records = registry.agents;
    return SingleChildScrollView(
      key: const ValueKey('agents-list-scroll'),
      controller: _listScroll,
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Wrap(
            alignment: WrapAlignment.start,
            crossAxisAlignment: WrapCrossAlignment.center,
            spacing: 8,
            children: [
              Text('AGENTS', style: Theme.of(context).textTheme.labelLarge),
              _action(
                'NEW AGENT',
                registry.canMutate &&
                        !_saving &&
                        !_unboundCreate &&
                        registry.agents.length < maxAgents
                    ? _newAgent
                    : null,
              ),
            ],
          ),
          if (registry.loaded && registry.agents.length >= maxAgents)
            const Text('32-profile limit reached.'),
          const SizedBox(height: 16),
          if (compact && _hasEditor)
            _action(
              'Return to agent editor',
              () => setState(() => _editorOpen = true),
            ),
          if (compact || !_hasEditor) ..._status(),
          if (registry.loaded && records.isEmpty)
            const Text('Agent profiles unavailable.'),
          for (final record in records)
            Padding(
              padding: const EdgeInsets.only(top: 12),
              child: OutlinedButton(
                key: ValueKey('agent-card-${record.agentId}'),
                style: OutlinedButton.styleFrom(
                  padding: const EdgeInsets.all(12),
                  minimumSize: const Size(48, 48),
                  side: BorderSide(
                    color: _original?.agentId == record.agentId
                        ? SmithColors.accent
                        : SmithColors.strokeSubtle,
                  ),
                  backgroundColor: _original?.agentId == record.agentId
                      ? SmithColors.surfaceSelected
                      : null,
                ),
                onPressed: _saving || registry.uncertain || _unboundCreate
                    ? null
                    : () => _select(record.agentId),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Icon(_icon(record.agentId)),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(record.name),
                          if (record.description.isNotEmpty)
                            Text(
                              record.description,
                              maxLines: 2,
                              overflow: TextOverflow.ellipsis,
                              style: Theme.of(context).textTheme.bodySmall,
                            ),
                          const SizedBox(height: 4),
                          Text(
                            record.prompt.isEmpty
                                ? 'Prompt: Not configured'
                                : 'Prompt: Configured',
                            style: Theme.of(context).textTheme.bodySmall,
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
        ],
      ),
    );
  }

  Widget _editor(bool compact) => SingleChildScrollView(
    key: const ValueKey('agent-editor-scroll'),
    controller: _editorScroll,
    padding: const EdgeInsets.all(16),
    child: Align(
      alignment: Alignment.topLeft,
      child: ConstrainedBox(
        key: const ValueKey('agent-editor-form'),
        constraints: BoxConstraints(
          maxWidth:
              (Theme.of(context).extension<SmithTokens>() ??
                      const SmithTokens())
                  .readingWidth,
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            if (compact)
              _action(
                'Back to agents',
                () => setState(() => _editorOpen = false),
              ),
            if (!_hasEditor)
              const Text('No agent selected. Choose a definition to edit.')
            else ...[
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Icon(_icon(_original?.agentId ?? '')),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          _creating
                              ? 'New agent'
                              : (_record(_original!.agentId) ?? _original!)
                                    .name,
                          style: Theme.of(context).textTheme.titleLarge,
                        ),
                        if (!_creating)
                          SelectableText(
                            'ID: ${_original!.agentId} (immutable)',
                            style: Theme.of(context).textTheme.bodySmall,
                          ),
                      ],
                    ),
                  ),
                ],
              ),

              if (_dirty) const Text('Unsaved changes'),
              ..._status(),
              const SizedBox(height: 16),
              LayoutBuilder(
                builder: (context, bounds) {
                  final scale = MediaQuery.textScalerOf(context).scale(14) / 14;
                  final paired = bounds.maxWidth >= 600 * scale;
                  final fieldWidth = paired
                      ? (bounds.maxWidth - 16) / 2
                      : bounds.maxWidth;
                  // Keep the same parents when wrapping so focus/IME survive.
                  return Wrap(
                    spacing: 16,
                    runSpacing: 16,
                    children: [
                      for (var i = 0; i < 2; i++)
                        SizedBox(width: fieldWidth, child: _field(i)),
                    ],
                  );
                },
              ),
              const SizedBox(height: 24),
              _field(2),
              const SizedBox(height: 16),
              Wrap(
                alignment: WrapAlignment.end,
                spacing: 8,
                runSpacing: 8,
                children: [
                  _action(
                    'Cancel',
                    _saving || registry.uncertain || _unboundCreate
                        ? null
                        : _cancel,
                  ),
                  _action(
                    _creating ? 'Save' : 'Save Changes',
                    _writable && _dirty ? _save : null,
                    primary: true,
                  ),
                ],
              ),
            ],
          ],
        ),
      ),
    ),
  );

  IconData _icon(String id) => switch (id) {
    'developer' => Icons.code,
    'assurance' => Icons.shield_outlined,
    _ => Icons.person_outline,
  };

  Widget _field(int i) => TextField(
    key: ValueKey('agent-${_labels[i].toLowerCase()}'),
    controller: _fields[i],
    focusNode: i == 0 ? _nameFocus : null,
    readOnly: _locked && i < 2,
    minLines: i == 2 ? 12 : 1,
    maxLines: i == 2 ? null : 1,

    keyboardType: i == 2 ? TextInputType.multiline : TextInputType.text,
    textInputAction: i == 2 ? TextInputAction.newline : TextInputAction.next,
    onChanged: (_) => setState(() => _error = null),
    decoration: InputDecoration(
      labelText: _labels[i],
      helperText: i == 2
          ? '${utf8.encode(_fields[i].text).length}/4096 UTF-8 bytes'
          : _locked && i < 2
          ? 'Read only'
          : '${_fields[i].text.runes.length}/40',
      helperMaxLines: 2,
      errorMaxLines: 5,
      errorText: _validation ? _validationError(i) : null,
    ),
  );

  Widget _retained(bool visible, Widget child) => Offstage(
    offstage: !visible,
    child: TickerMode(
      enabled: visible,
      child: ExcludeFocus(excluding: !visible, child: child),
    ),
  );

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, bounds) {
      final scale = MediaQuery.textScalerOf(context).scale(14) / 14;
      final compact = bounds.maxWidth < 880 + (scale - 1).clamp(0, 2) * 320;
      final width = (bounds.maxWidth / 4).clamp(256.0, 384.0);
      return Stack(
        children: [
          Positioned(
            left: 0,
            top: 0,
            bottom: 0,
            width: compact ? bounds.maxWidth : width,
            child: _retained(!compact || !_editorOpen, _list(compact)),
          ),
          Positioned.fill(
            left: compact ? 0 : width,
            child: _retained(!compact || _editorOpen, _editor(compact)),
          ),
          if (!compact)
            Positioned(
              left: width,
              top: 0,
              bottom: 0,
              child: const VerticalDivider(
                width: 1,
                thickness: 1,
                color: SmithColors.strokeSubtle,
              ),
            ),
        ],
      );
    },
  );

  @override
  void dispose() {
    registry.removeListener(_changed);
    _nameFocus.dispose();
    for (final field in _fields) {
      field.dispose();
    }
    _listScroll.dispose();
    _editorScroll.dispose();
    super.dispose();
  }
}

const _labels = ['Name', 'Description', 'Prompt'];

String? _fieldError(int field, String value) => switch (field) {
  0 => agentNameError(value),
  1 => agentDescriptionError(value),
  _ => agentPromptError(value),
};

class _Submission {
  const _Submission(
    this.epoch,
    this.instance,
    this.revision,
    this.id,
    this.requestId,
    this.values,
  );
  final int epoch;
  final String instance;
  final int revision;
  final String? id;
  final String requestId;
  final List<String> values;
}

/// Retain the intended reading position even when a temporary layout clamps it.
class _AgentScrollController extends ScrollController {
  double desiredOffset = 0;
  @override
  ScrollPosition createScrollPosition(
    ScrollPhysics physics,
    ScrollContext context,
    ScrollPosition? oldPosition,
  ) => _AgentScrollPosition(this, physics, context, oldPosition);
}

class _AgentScrollPosition extends ScrollPositionWithSingleContext {
  _AgentScrollPosition(
    this.owner,
    ScrollPhysics physics,
    ScrollContext context,
    ScrollPosition? oldPosition,
  ) : super(
        physics: physics,
        context: context,
        initialPixels: owner.desiredOffset,
        keepScrollOffset: false,
        oldPosition: oldPosition,
      );
  final _AgentScrollController owner;
  @override
  Future<void> ensureVisible(
    RenderObject object, {
    double alignment = 0,
    Duration duration = Duration.zero,
    Curve curve = Curves.ease,
    ScrollPositionAlignmentPolicy alignmentPolicy =
        ScrollPositionAlignmentPolicy.explicit,
    RenderObject? targetRenderObject,
  }) async {
    Future<void> reveal(ScrollPositionAlignmentPolicy policy) =>
        super.ensureVisible(
          object,
          alignment: alignment,
          duration: duration,
          curve: curve,
          alignmentPolicy: policy,
          targetRenderObject: targetRenderObject,
        );
    await reveal(alignmentPolicy);
    if (alignmentPolicy != ScrollPositionAlignmentPolicy.explicit) {
      await reveal(
        alignmentPolicy == ScrollPositionAlignmentPolicy.keepVisibleAtEnd
            ? ScrollPositionAlignmentPolicy.keepVisibleAtStart
            : ScrollPositionAlignmentPolicy.keepVisibleAtEnd,
      );
    }
  }

  @override
  double setPixels(double newPixels) {
    final result = super.setPixels(newPixels);
    owner.desiredOffset = pixels;
    return result;
  }

  @override
  void forcePixels(double value) {
    owner.desiredOffset = value;
    super.forcePixels(value);
  }

  @override
  bool applyContentDimensions(double minScrollExtent, double maxScrollExtent) {
    final accepted = super.applyContentDimensions(
      minScrollExtent,
      maxScrollExtent,
    );
    final target = owner.desiredOffset.clamp(minScrollExtent, maxScrollExtent);
    if (pixels != target) {
      correctPixels(target);
      return false;
    }
    return accepted;
  }
}
