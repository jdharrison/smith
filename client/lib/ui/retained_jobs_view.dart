import 'dart:async';

import 'package:flutter/material.dart';

import '../projects.dart';
import '../retained_jobs.dart';
import '../retained_jobs_controller.dart';

class RetainedJobsView extends StatefulWidget {
  const RetainedJobsView({super.key, required this.controller});
  final RetainedJobsController controller;
  @override
  State<RetainedJobsView> createState() => _RetainedJobsViewState();
}

class _RetainedJobsViewState extends State<RetainedJobsView> {
  DialogRoute<void>? _route;
  RetainedJobsController get jobs => widget.controller;
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) unawaited(jobs.refresh());
    });
  }

  void _retire() {
    final route = _route;
    _route = null;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (route?.isActive == true) route!.navigator?.removeRoute(route);
    });
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (!TickerMode.valuesOf(context).enabled) _retire();
  }

  @override
  void didUpdateWidget(covariant RetainedJobsView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != jobs) {
      _retire();
      unawaited(jobs.refresh());
    }
  }

  @override
  void dispose() {
    _retire();
    super.dispose();
  }

  void _open([String? id]) {
    if (_route != null ||
        !TickerMode.valuesOf(context).enabled ||
        ModalRoute.of(context)?.isCurrent != true) {
      return;
    }
    late final DialogRoute<void> route;
    route = DialogRoute<void>(
      context: context,
      barrierDismissible: false,
      builder: (_) => _RetainedDialog(
        controller: jobs,
        id: id,
        owns: () =>
            mounted &&
            identical(_route, route) &&
            route.isCurrent &&
            TickerMode.valuesOf(context).enabled,
        close: () {
          if (identical(_route, route)) _retire();
        },
      ),
    );
    _route = route;
    unawaited(
      Navigator.of(context, rootNavigator: true).push(route).whenComplete(() {
        if (identical(_route, route)) _route = null;
      }),
    );
    if (id != null) unawaited(jobs.loadDetail(id));
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: Listenable.merge([jobs, jobs.projects]),
    builder: (context, _) => Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Wrap(
          spacing: 12,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            FilledButton(
              onPressed: jobs.canStart ? () => _open() : null,
              child: const Text('New read/write job'),
            ),
            if (jobs.pending != null ||
                jobs.error != null ||
                jobs.stopped ||
                (jobs.state != null && !jobs.fresh))
              TextButton(
                onPressed: jobs.loading || jobs.busy ? null : jobs.refresh,
                child: Text(
                  jobs.pending != null
                      ? 'Check acknowledgement'
                      : 'Refresh retained jobs',
                ),
              ),
          ],
        ),
        if (jobs.state == null && jobs.error == null)
          const Text('Loading retained jobs…'),
        if (jobs.state?.available == false)
          const Text('Read/write jobs unavailable: runtime not prepared.'),
        if (jobs.state != null && !jobs.fresh)
          const Text('Retained jobs may be out of date.'),
        if (jobs.error != null) Text(jobs.error!),
        if (jobs.pending != null)
          Semantics(
            liveRegion: true,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  jobs.busy ? 'Awaiting acknowledgement…' : 'Acknowledgement unknown. Observation only; do not resubmit.',
                ),
                ExpansionTile(
                  title: const Text('Details'),
                  children: [
                    SelectableText(
                      'Operation: ${jobs.pending!['operation_id']}',
                    ),
                    const Text(
                      'Checks query the original receipt and saved job. Missing proof never resubmits work.',
                    ),
                  ],
                ),
              ],
            ),
          ),
        if (jobs.stopped)
          const Text('Auto-refresh paused. Refresh to observe.'),
        if (jobs.fresh &&
            jobs.state?.available == true &&
            jobs.state?.jobs.isEmpty == true)
          const Text('No retained jobs.'),
        for (final job in jobs.state?.jobs ?? <RetainedJob>[])
          ListTile(
            contentPadding: EdgeInsets.zero,
            title: Text('${job.projectKey} · ${job.jobType}'),
            subtitle: Text('${job.status} · ${job.sessionId}'),
            onTap: () => _open(job.sessionId),
          ),
        const SizedBox(height: 24),
      ],
    ),
  );
}

class _RetainedDialog extends StatefulWidget {
  const _RetainedDialog({
    required this.controller,
    required this.id,
    required this.owns,
    required this.close,
  });
  final RetainedJobsController controller;
  final String? id;
  final bool Function() owns;
  final VoidCallback close;
  @override
  State<_RetainedDialog> createState() => _RetainedDialogState();
}

class _RetainedDialogState extends State<_RetainedDialog> {
  final _prompt = TextEditingController();
  String? _project;
  String _type = 'read';
  Map<String, dynamic>? _review;
  ProjectRecord? _reviewProject;
  String? _confirmation;
  int? _confirmRevision;
  String? _confirmInstance;
  RetainedJobsController get jobs => widget.controller;
  @override
  void dispose() {
    _prompt.dispose();
    super.dispose();
  }

  bool get _validPrompt =>
      retainedPromptValid(_prompt.text) && _prompt.value.composing.isCollapsed;
  Future<void> _send() async {
    if (!widget.owns() || !_validPrompt) return;
    final value = _prompt.value;
    await jobs.act(
      widget.id!,
      'prompt',
      prompt: value.text,
      onAdmitted: (_) {
        if (mounted && _prompt.value == value) _prompt.clear();
      },
    );
    if (mounted) await jobs.loadDetail(widget.id!);
  }

  Future<void> _confirm() async {
    if (_confirmation == null) return;
    if (!widget.owns() ||
        _confirmRevision != jobs.state?.revision ||
        _confirmInstance != jobs.instanceId) {
      setState(() => _confirmation = null);
      return;
    }
    final action = _confirmation!;
    setState(() => _confirmation = null);
    await jobs.act(widget.id!, action);
    if (mounted) await jobs.loadDetail(widget.id!);
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: Listenable.merge([jobs, jobs.projects, _prompt]),
    builder: (context, _) {
      final id = widget.id;
      final review = _review;
      final j = id == null ? null : jobs.job(id);
      final d = id == null ? null : jobs.details[id];
      final currentDetail =
          d != null &&
          d.revision == jobs.state?.revision &&
          d.job.status == j?.status &&
          d.job.retained == j?.retained;
      return AlertDialog(
        title: Text(
          id == null
              ? (review == null ? 'New read/write job' : 'Review job')
              : 'Retained job',
        ),
        content: SizedBox(
          width: 560,
          child: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                if (id == null && review == null) ...[
                  DropdownButtonFormField<String>(
                    initialValue:
                        jobs.projects.projects.any(
                          (p) => p.projectId == _project,
                        )
                        ? _project
                        : null,
                    decoration: const InputDecoration(labelText: 'Project'),
                    items: [
                      for (final p in jobs.projects.projects)
                        DropdownMenuItem(
                          value: p.projectId,
                          child: Text(p.key),
                        ),
                    ],
                    onChanged: jobs.busy
                        ? null
                        : (v) => setState(() => _project = v),
                  ),
                  DropdownButtonFormField<String>(
                    initialValue: _type,
                    decoration: const InputDecoration(labelText: 'Job type'),
                    items: const [
                      DropdownMenuItem(value: 'read', child: Text('Read')),
                      DropdownMenuItem(value: 'write', child: Text('Write')),
                    ],
                    onChanged: jobs.busy
                        ? null
                        : (v) => setState(() => _type = v!),
                  ),
                  TextField(
                    controller: _prompt,
                    minLines: 3,
                    maxLines: 8,
                    decoration: const InputDecoration(labelText: 'Prompt'),
                  ),
                ],
                if (id == null && review != null) ...[
                  Text(
                    'Project: ${jobs.projects.projects.where((p) => p.projectId == review['project_id']).map((p) => p.key).firstOrNull ?? 'changed'} · ${review['job_type']}',
                  ),
                  SelectableText(review['prompt'] as String),
                  if (_reviewProject != null)
                    ExpansionTile(
                      title: const Text('Context and repository access'),
                      children: [
                        SelectableText(_reviewProject!.prompt),
                        for (final repository in _reviewProject!.repositories)
                          ListTile(
                            title: Text(repository.url),
                            subtitle: Text(
                              '${repository.branch.base ?? 'default'} · ${review['job_type'] == 'read' || repository.readOnly ? 'read-only' : 'writable'}',
                            ),
                          ),
                      ],
                    ),
                  const Text(
                    'Start sends the prompt and saved project context to a retained environment. Repository preparation may use the network. Write cannot override read-only repositories. Finish releases the environment; closing this dialog does not.',
                  ),
                  if (!jobs.validReview(review))
                    const Text('Saved state changed. Edit and review again.'),
                ],
                if (id != null) ...[
                  SelectableText('Session: $id'),
                  Text(
                    j == null
                        ? 'Job no longer listed.'
                        : '${j.projectKey} · ${j.jobType} · ${j.status}',
                  ),
                  if (!currentDetail) const Text('History may be out of date.'),
                  if (jobs.detailErrors[id] != null)
                    Text(jobs.detailErrors[id]!),
                  TextButton(
                    onPressed:
                        jobs.loading ||
                            jobs.busy ||
                            jobs.readingDetails.contains(id)
                        ? null
                        : () async {
                            await jobs.refresh();
                            await jobs.loadDetail(id);
                          },
                    child: Text(
                      jobs.pending != null
                          ? 'Check acknowledgement'
                          : 'Refresh history',
                    ),
                  ),
                  if (d != null) ...[
                    for (final t in d.turns)
                      Padding(
                        padding: const EdgeInsets.symmetric(vertical: 8),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            SelectableText(t.prompt),
                            Text(t.outcome),
                            if (t.reply != null) SelectableText(t.reply!),
                          ],
                        ),
                      ),
                    ExpansionTile(
                      title: const Text('Repositories'),
                      children: [
                        for (final r in d.repositories)
                          ListTile(
                            title: Text(r.url),
                            subtitle: Text(
                              '${r.reference ?? 'default'} · ${r.readOnly ? 'read-only' : 'writable'}',
                            ),
                          ),
                      ],
                    ),
                  ],
                  TextField(
                    controller: _prompt,
                    minLines: 2,
                    maxLines: 6,
                    decoration: const InputDecoration(labelText: 'Follow-up'),
                  ),
                  if (_confirmation != null)
                    Text(
                      _confirmation == 'delete'
                          ? 'Delete this job’s history? A retained environment must be cleaned up first.'
                          : 'Finish this job and release its retained environment?',
                    ),
                ],
                if (jobs.state?.available == false)
                  const Text('Runtime not prepared.'),
                if (jobs.pending != null)
                  const Text(
                    'Awaiting a proven acknowledgement. No resubmission.',
                  ),
                if (jobs.error != null) Text(jobs.error!),
              ],
            ),
          ),
        ),
        actions: [
          TextButton(onPressed: widget.close, child: const Text('Close')),
          if (id == null && review == null)
            TextButton(
              onPressed:
                  jobs.canStart &&
                      jobs.projects.projects.any(
                        (p) => p.projectId == _project,
                      ) &&
                      _validPrompt
                  ? () {
                      if (widget.owns()) {
                        setState(() {
                          _reviewProject = jobs.projects.projects.firstWhere(
                            (p) => p.projectId == _project,
                          );
                          _review = jobs.captureStart(
                            _project!,
                            _type,
                            _prompt.text,
                          );
                        });
                      }
                    }
                  : null,
              child: const Text('Review'),
            ),
          if (id == null && review != null) ...[
            TextButton(
              onPressed: jobs.busy
                  ? null
                  : () => setState(() => _review = null),
              child: const Text('Edit'),
            ),
            FilledButton(
              onPressed: jobs.validReview(review)
                  ? () async {
                      if (!widget.owns() || !identical(_review, review)) return;
                      final ok = await jobs.start(review);
                      if (mounted && ok && widget.owns()) widget.close();
                    }
                  : null,
              child: const Text('Start'),
            ),
          ],
          if (id != null && _confirmation == null) ...[
            FilledButton(
              onPressed:
                  currentDetail && jobs.canAct(id, 'prompt') && _validPrompt
                  ? _send
                  : null,
              child: const Text('Send'),
            ),
            for (final action in ['finish', 'delete'])
              TextButton(
                onPressed: jobs.canAct(id, action)
                    ? () {
                        if (widget.owns()) {
                          setState(() {
                            _confirmation = action;
                            _confirmRevision = jobs.state!.revision;
                            _confirmInstance = jobs.instanceId;
                          });
                        }
                      }
                    : null,
                child: Text(action == 'finish' ? 'Finish' : 'Delete'),
              ),
          ],
          if (id != null && _confirmation != null) ...[
            TextButton(
              autofocus: true,
              onPressed: () => setState(() => _confirmation = null),
              child: const Text('Cancel'),
            ),
            FilledButton(
              onPressed:
                  jobs.canAct(id, _confirmation!) &&
                      _confirmRevision == jobs.state?.revision &&
                      _confirmInstance == jobs.instanceId
                  ? _confirm
                  : null,
              child: Text(
                _confirmation == 'delete' ? 'Delete job' : 'Finish job',
              ),
            ),
          ],
        ],
      );
    },
  );
}
