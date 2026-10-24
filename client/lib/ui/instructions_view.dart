import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';

import '../cloud_routing.dart';
import '../instructions.dart';
import '../instructions_controller.dart';
import 'smith_icon.dart';
import 'smith_theme.dart';

/// The owning screen retains the editor value, including while Settings unmounts.
class InstructionsView extends StatefulWidget {
  const InstructionsView({super.key, required this.controller, this.route});

  final InstructionsController controller;
  final ChatRoute? route;

  @override
  State<InstructionsView> createState() => _InstructionsViewState();
}

class _InstructionsViewState extends State<InstructionsView> {
  late final TextEditingController instructions;
  late final TextEditingController personality;
  late final TextEditingController guardrails;
  bool reviewing = false;

  @override
  void initState() {
    super.initState();
    final draft = widget.controller.draft;
    instructions = TextEditingController(text: draft.instructions);
    personality = TextEditingController(text: draft.personality);
    guardrails = TextEditingController(text: draft.guardrails);
    widget.controller.addListener(changed);
    refreshAfterMount();
  }

  void refreshAfterMount() {
    final controller = widget.controller;
    final route = widget.route;
    // Shared panes may already be listening; never notify them during a build.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && widget.controller == controller && widget.route == route) {
        unawaited(controller.refresh(route: route));
      }
    });
  }

  @override
  void didUpdateWidget(covariant InstructionsView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != widget.controller) {
      oldWidget.controller.removeListener(changed);
      widget.controller.addListener(changed);
      syncFields();
    }
    if (oldWidget.controller != widget.controller ||
        oldWidget.route != widget.route) {
      refreshAfterMount();
    }
  }

  void syncFields() {
    final draft = widget.controller.draft;
    for (final (editor, value) in [
      (instructions, draft.instructions),
      (personality, draft.personality),
      (guardrails, draft.guardrails),
    ]) {
      if (editor.text != value) {
        editor.value = TextEditingValue(
          text: value,
          selection: TextSelection.collapsed(offset: value.length),
        );
      }
    }
  }

  void changed() {
    if (!mounted) return;
    syncFields();
    setState(() {});
  }

  @override
  void dispose() {
    widget.controller.removeListener(changed);
    instructions.dispose();
    personality.dispose();
    guardrails.dispose();
    super.dispose();
  }

  Future<void> reviewChanges() async {
    final c = widget.controller;
    final saved = c.snapshot;
    if (reviewing ||
        saved == null ||
        c.loading ||
        c.busy ||
        c.uncertain ||
        c.draft.validationError != null) {
      return;
    }
    final draft = c.draft;
    final instance = c.instanceId;
    final draftInstance = c.draftInstanceId;
    final draftRevision = c.draftConfigRevision;
    final route = widget.route;
    final conflict = c.conflict;
    bool current() =>
        mounted &&
        identical(widget.controller, c) &&
        widget.route == route &&
        identical(c.snapshot, saved) &&
        c.instanceId == instance &&
        c.draftInstanceId == draftInstance &&
        c.draftConfigRevision == draftRevision &&
        c.draft == draft &&
        !c.loading &&
        !c.busy &&
        !c.uncertain;

    setState(() => reviewing = true);
    try {
      final confirmed = await showDialog<bool>(
        context: context,
        builder: (context) => AnimatedBuilder(
          animation: c,
          builder: (context, _) => AlertDialog(
            insetPadding: const EdgeInsets.all(12),
            scrollable: true,
            title: const Text('Review global instructions'),
            content: SizedBox(
              width: 560,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                mainAxisSize: MainAxisSize.min,
                children: [
                  const Text(
                    'Review these exact personal values before replacing the saved settings. This is not a rendered future conversation or a token-fit guarantee.',
                  ),
                  if (conflict) ...[
                    const SizedBox(height: 16),
                    Text(
                      'Your preserved draft began at revision $draftRevision. The current applied revision is ${saved.configRevision}; the service instance may also have changed. Apply globally explicitly rebinds this draft to the current saved state shown below, replacing it without merging.',
                      key: const ValueKey('instructions-conflict-review'),
                    ),
                  ],
                  const SizedBox(height: 16),
                  _SettingsValues(
                    heading:
                        'Currently applied · revision ${saved.configRevision}',
                    settings: saved.settings,
                  ),
                  const Divider(height: 32),
                  _SettingsValues(
                    heading: 'Proposed draft · not applied',
                    settings: draft,
                  ),
                  const SizedBox(height: 16),
                  Text(_privacy(draft)),
                  const SizedBox(height: 16),
                  const Text(
                    'Applies globally to future conversational prompts, including existing chats. Accepted work is unchanged. Smith rejects Apply with BUSY while accepted work is queued or running; your draft is kept. Unsent queues need revision review. No inference or cloud transfer runs from Apply.',
                  ),
                  const SizedBox(height: 16),
                  const Text(
                    'Core safety and permissions stay code-owned. Installer and provider checks remain isolated synthetic prompts without personal instructions or history.',
                  ),
                  if (!current()) ...[
                    const SizedBox(height: 16),
                    const Text(
                      'This review changed or its originating view is unavailable. Cancel and review again; nothing is applied.',
                    ),
                  ] else if (!c.canApply &&
                      !(c.conflict && c.error == null)) ...[
                    const SizedBox(height: 16),
                    const Text(
                      'Apply is unavailable. Refresh saved settings, then review the preserved draft again.',
                    ),
                  ],
                ],
              ),
            ),
            actions: [
              TextButton(
                style: _actionStyle,
                onPressed: () => Navigator.pop(context, false),
                child: const Text('Cancel'),
              ),
              TextButton(
                key: const ValueKey('instructions-apply'),
                style: _actionStyle,
                onPressed:
                    current() && (c.canApply || (c.conflict && c.error == null))
                    ? () => Navigator.pop(context, true)
                    : null,
                child: const Text('Apply globally'),
              ),
            ],
          ),
        ),
      );
      if (confirmed != true || !current()) return;
      // Only this explicit, compared confirmation may rebind a stale draft.
      final review = c.createReview();
      if (review != null && c.canApply) await c.apply(review: review);
    } finally {
      if (mounted) setState(() => reviewing = false);
    }
  }

  Widget field(
    String label,
    String key,
    TextEditingController editor,
    bool enabled,
    void Function(String) update,
  ) => Padding(
    padding: const EdgeInsets.only(top: 16),
    child: TextField(
      key: ValueKey(key),
      controller: editor,
      enabled: enabled,
      minLines: 2,
      maxLines: 4,
      keyboardType: TextInputType.multiline,
      textInputAction: TextInputAction.newline,
      onChanged: update,
      decoration: InputDecoration(labelText: label, alignLabelWithHint: true),
    ),
  );

  @override
  Widget build(BuildContext context) {
    final c = widget.controller;
    final saved = c.snapshot;
    final draft = c.draft;
    final editable = saved != null && !c.loading && !c.busy && !c.uncertain;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        Semantics(
          header: true,
          child: Row(
            children: [
              const SmithIcon('chevron-right', color: SmithColors.accent),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  'SMITH · Global instructions',
                  style: Theme.of(context).textTheme.titleMedium,
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        const SelectableText('Vessel: $instructionVessel'),
        const Text(
          'One Smith identity across conversations and models. Global settings are not shared chat memory. Tools and Dev orchestration are not supplied by these settings.',
        ),
        const SizedBox(height: 16),
        const Text(
          'Do not enter secrets, API keys, credentials or sensitive host paths.',
        ),
        const Text(
          'Additional guardrails are guidance, not permission to weaken enforced safety or approve actions. Model setup, keys and service maintenance remain installer-only.',
        ),
        const SizedBox(height: 16),
        Semantics(
          liveRegion: true,
          child: Text(
            c.loading
                ? 'Loading saved global instructions… Edits are preserved.'
                : c.busy
                ? 'Applying or checking the original change… Awaiting authoritative settings, not just an acknowledgement.'
                : c.uncertain
                ? 'Apply outcome unknown. Check apply status; do not resend.'
                : saved == null
                ? 'Global instructions have not been observed.'
                : 'Last observed applied revision: ${saved.configRevision}${c.dirty ? ' · Unsaved draft' : ''}',
            key: const ValueKey('instructions-status'),
          ),
        ),
        if (c.error != null) ...[
          const SizedBox(height: 8),
          Semantics(
            liveRegion: true,
            child: Text(
              c.error!,
              key: const ValueKey('instructions-error'),
              style: const TextStyle(color: SmithColors.danger),
            ),
          ),
        ],
        if (c.conflict) ...[
          const SizedBox(height: 8),
          const Text(
            'Saved settings or the service changed. Your draft is preserved against its earlier revision. Preview changes to compare it with current settings before explicitly rebinding.',
          ),
        ],
        field(
          'Global instructions',
          'global-instructions',
          instructions,
          editable,
          (value) => c.edit(c.draft.copyWith(instructions: value)),
        ),
        field(
          'Personality',
          'global-personality',
          personality,
          editable,
          (value) => c.edit(c.draft.copyWith(personality: value)),
        ),
        field(
          'Additional guardrails',
          'global-guardrails',
          guardrails,
          editable,
          (value) => c.edit(c.draft.copyWith(guardrails: value)),
        ),
        const SizedBox(height: 8),
        Text(
          '${draft.personalBytes} / $maxPersonalInstructionBytes combined UTF-8 bytes',
        ),
        if (draft.validationError != null)
          Semantics(
            liveRegion: true,
            child: Text(
              draft.validationError!,
              key: const ValueKey('instructions-validation'),
              style: const TextStyle(color: SmithColors.danger),
            ),
          ),
        ConstrainedBox(
          constraints: const BoxConstraints(minHeight: 48),
          child: CheckboxListTile(
            key: const ValueKey('instructions-cloud-sharing'),
            contentPadding: EdgeInsets.zero,
            controlAffinity: ListTileControlAffinity.leading,
            value: draft.cloudEnabled,
            onChanged: editable
                ? (value) => c.edit(c.draft.copyWith(cloudEnabled: value))
                : null,
            title: const Text('Allow personal instructions in cloud prompts'),
          ),
        ),
        Text(_privacy(draft)),
        const SizedBox(height: 8),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            OutlinedButton(
              key: const ValueKey('instructions-review'),
              style: _actionStyle,
              onPressed:
                  editable &&
                      !reviewing &&
                      (c.dirty || c.conflict) &&
                      draft.validationError == null
                  ? reviewChanges
                  : null,
              child: const Text('Preview changes'),
            ),
            TextButton(
              style: _actionStyle,
              onPressed: editable
                  ? () => c.edit(const InstructionSettings())
                  : null,
              child: const Text('Clear draft'),
            ),
            TextButton(
              style: _actionStyle,
              onPressed: editable && (c.dirty || c.conflict) ? c.discard : null,
              child: const Text('Discard edits'),
            ),
            TextButton(
              style: _actionStyle,
              onPressed: !c.loading && !c.busy
                  ? () => c.refresh(route: widget.route)
                  : null,
              child: const Text('Refresh'),
            ),
            if (c.uncertain)
              TextButton(
                style: _actionStyle,
                onPressed: !c.loading && !c.busy ? c.resolvePending : null,
                child: const Text('Check apply status'),
              ),
          ],
        ),
        if (saved != null) ...[
          const SizedBox(height: 16),
          ExpansionTile(
            key: const ValueKey('instructions-inspector'),
            tilePadding: EdgeInsets.zero,
            title: const Text('Applied preamble inspector'),
            children: [
              if (c.snapshotRoute != widget.route)
                const Text(
                  'This observation is for a different route than this pane. Refresh to inspect the current selection; the labels below describe the observed preamble.',
                  key: ValueKey('instructions-route-mismatch'),
                ),
              _InstructionInspector(snapshot: saved, route: c.snapshotRoute),
            ],
          ),
        ],
      ],
    );
  }
}

final _actionStyle = TextButton.styleFrom(minimumSize: const Size(48, 48));

String _privacy(InstructionSettings settings) =>
    settings.hasPersonalText && !settings.cloudEnabled
    ? 'Local-only personal text: cloud sends are blocked, not stripped of your instructions. Choose a local model or explicitly allow sharing and review Apply.'
    : settings.cloudEnabled
    ? 'Personal text may be included in future cloud prompts. Allowing sharing is not Send consent. A new effective bundle requires destination-bound context review; provider processing, retention and possible charges apply.'
    : 'Cloud sharing is off by default. Empty personal fields do not block cloud sends. Added local-only text will block cloud sends rather than being omitted.';

class _SettingsValues extends StatelessWidget {
  const _SettingsValues({required this.heading, required this.settings});
  final String heading;
  final InstructionSettings settings;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    mainAxisSize: MainAxisSize.min,
    children: [
      Text(heading, style: Theme.of(context).textTheme.titleSmall),
      for (final (label, value) in [
        ('Global instructions', settings.instructions),
        ('Personality', settings.personality),
        ('Additional guardrails', settings.guardrails),
      ]) ...[
        const SizedBox(height: 12),
        Text(label),
        SelectableText(value.isEmpty ? '(empty)' : value),
      ],
      const SizedBox(height: 12),
      Text(
        'Allow personal text in cloud: ${settings.cloudEnabled ? 'Yes' : 'No'}',
      ),
      Text('${settings.personalBytes} combined UTF-8 bytes'),
    ],
  );
}

class _InstructionInspector extends StatelessWidget {
  const _InstructionInspector({required this.snapshot, required this.route});
  final InstructionSnapshot snapshot;
  final ChatRoute? route;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    mainAxisSize: MainAxisSize.min,
    children: [
      const Text(
        'Read-only source: service instructions_get. This is the last observed applied preamble only, not your draft, the exact future conversation, live host readiness or a tokenizer-fit guarantee. Preview runs no inference.',
      ),
      const SizedBox(height: 12),
      SelectableText('Applied revision: ${snapshot.configRevision}'),
      SelectableText('Renderer source/version: ${snapshot.rendererVersion}'),
      SelectableText(
        'Preamble vessel: ${snapshot.vessel ?? 'Shared prefix only — no route requested'}',
      ),
      SelectableText('Provider: ${route?.provider ?? 'Not selected'}'),
      SelectableText('Model: ${route?.model ?? 'Not selected'}'),
      const SizedBox(height: 12),
      SelectableText('Core identity/safety source: ${snapshot.core.version}'),
      SelectableText(snapshot.core.text),
      const SizedBox(height: 12),
      SelectableText('Product knowledge source: ${snapshot.product.version}'),
      SelectableText(snapshot.product.text),
      const SizedBox(height: 12),
      const Text('Exact applied local/cloud preamble text'),
      SelectableText(
        snapshot.local.text,
        key: const ValueKey('instructions-preamble'),
      ),
      Text(
        '${utf8.encode(snapshot.local.text).length} / ${snapshot.maxPreambleBytes} preamble UTF-8 bytes',
      ),
      SelectableText('Preamble SHA-256: ${snapshot.local.sha256}'),
      SelectableText('Cloud bundle SHA-256: ${snapshot.cloud.bundleSha256}'),
      Text(
        'Personal text included: ${snapshot.cloud.personalIncluded == true ? 'Yes' : 'No'}',
      ),
      Text(
        'Cloud export eligible: ${snapshot.cloud.eligible == true ? 'Yes; destination consent still applies' : 'No — cloud sends blocked, personal text not omitted'}',
      ),
      const SizedBox(height: 16),
    ],
  );
}
