import 'dart:async';

import 'package:flutter/material.dart';

import '../app_api.dart';
import '../dev_jobs.dart';
import '../dev_jobs_controller.dart';
import '../projects.dart';
import 'retained_jobs_view.dart';

// Resolve is query-only, but its pending UI state must be shared by every pane.
final _deletionResolutions = Expando<ValueNotifier<bool>>();

class DevJobsView extends StatefulWidget {
  const DevJobsView({super.key, required this.controller});
  final DevJobsController controller;
  @override
  State<DevJobsView> createState() => _DevJobsViewState();
}

class _DevJobsViewState extends State<DevJobsView> {
  final _prompt = TextEditingController();
  final _modalChanges = ValueNotifier(0);
  final _newJobFocus = FocusNode();
  final _rowFocus = <String, FocusNode>{};
  final _viewId = appId();
  TextEditingValue _draft = TextEditingValue.empty;
  String? _projectId, _error, _selectedId;
  DialogRoute<void>? _modal;
  _JobReview? _review;
  _JobDeletionReview? _deletionReview;
  bool _newJobOpen = false;
  bool _originVisible = true;
  int _visibilityEpoch = 0, _draftVersion = 0, _modalSerial = 0;
  DevJobsController get jobs => widget.controller;
  ValueNotifier<bool> get _resolvingDeletion =>
      _deletionResolutions[jobs] ??= ValueNotifier(false);

  @override
  void initState() {
    super.initState();
    _prompt.addListener(_draftChanged);
    jobs.addListener(_originChanged);
    jobs.projects.addListener(_originChanged);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) unawaited(_refresh());
    });
  }

  @override
  void didUpdateWidget(covariant DevJobsView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != jobs) {
      oldWidget.controller.removeListener(_originChanged);
      oldWidget.controller.projects.removeListener(_originChanged);
      jobs.addListener(_originChanged);
      jobs.projects.addListener(_originChanged);
      _visibilityEpoch++;
      _retireModal(afterFrame: true);
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) unawaited(_refresh());
      });
    }
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final visible = TickerMode.valuesOf(context).enabled;
    if (_originVisible && !visible) {
      _visibilityEpoch++;
      _retireModal(afterFrame: true);
    }
    _originVisible = visible;
  }

  bool get _visible =>
      mounted && _originVisible && TickerMode.valuesOf(context).enabled;

  bool _owns(DialogRoute<void> route) =>
      _visible && identical(_modal, route) && route.isActive;

  void _changed() => _modalChanges.value++;

  void _draftChanged() {
    // EditableText can clear composition while its route is being removed.
    // Only the live originating editor may replace the retained editing value.
    if (!_newJobOpen || _modal == null) return;
    _draft = _prompt.value;
    _draftVersion++;
    if (_review != null) {
      _review = null;
      _error = 'Draft changed. Review again; nothing was submitted.';
    }
  }

  bool _sameOrigin(_JobReview review) =>
      review.epoch == _visibilityEpoch &&
      review.draftVersion == _draftVersion &&
      review.project.projectId == _projectId &&
      review.args['prompt'] == _draft.text &&
      _draft.composing.isCollapsed &&
      jobs.projects.instanceId == review.args['instance_id'] &&
      jobs.projects.revision == review.args['expected_projects_revision'] &&
      jobs.projects.projects.any((p) => p.projectId == _projectId) &&
      jobs.model?.configRevision == review.args['expected_config_revision'] &&
      jobs.model?.name == review.model.name &&
      jobs.model?.sha256 == review.model.sha256 &&
      jobs.model?.ready == true;

  void _originChanged() {
    final review = _review;
    if (review != null && !_sameOrigin(review)) {
      _review = null;
      _error = 'Saved origin changed. Review again; draft retained.';
      _changed();
    }
  }

  Future<void> _refresh() =>
      Future.wait([jobs.projects.refresh(), jobs.refresh()]);

  // Retire exactly this named route, including when another route covers it.
  // Never pop the navigator's current route on a late acknowledgement/hide.
  void _retireModal({bool afterFrame = false}) {
    final route = _modal;
    _modal = null;
    _newJobOpen = false;
    _review = null;
    _deletionReview = null;
    if (route == null) return;
    final name = route.settings.name;
    void remove() {
      final navigator = route.navigator;
      if (navigator != null && route.isActive && route.settings.name == name) {
        navigator.removeRoute(route);
      }
    }

    if (afterFrame) {
      WidgetsBinding.instance.addPostFrameCallback((_) => remove());
    } else {
      remove();
    }
  }

  void _close(DialogRoute<void> route, FocusNode originFocus) {
    if (!identical(_modal, route)) return;
    _retireModal();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (_visible &&
          _modal == null &&
          ModalRoute.of(context)?.isCurrent == true) {
        originFocus.requestFocus();
      }
    });
  }

  void _openModal({required bool newJob, String? requestId}) {
    if (!_visible ||
        _modal != null ||
        ModalRoute.of(context)?.isCurrent != true) {
      return;
    }
    _deletionReview = null;
    _newJobOpen = newJob;
    _selectedId = requestId ?? _selectedId;
    _review = null;
    if (newJob) {
      // Restore selection and composition as well as exact text after detaching.
      _prompt.value = _draft;
      _error = null;
    }
    final originFocus = newJob
        ? _newJobFocus
        : _rowFocus.putIfAbsent(requestId!, FocusNode.new);
    late final DialogRoute<void> route;
    route = DialogRoute<void>(
      context: context,
      barrierDismissible: false,
      settings: RouteSettings(
        name:
            'dev-jobs/$_viewId/${++_modalSerial}/${newJob ? 'new' : requestId}',
      ),
      builder: (dialogContext) => ListenableBuilder(
        listenable: Listenable.merge([
          jobs,
          jobs.projects,
          _prompt,
          _modalChanges,
          _resolvingDeletion,
        ]),
        builder: (context, _) {
          if (!mounted) return const SizedBox.shrink();
          return newJob
              ? _newJobDialog(route, originFocus)
              : _detailDialog(route, requestId!, originFocus);
        },
      ),
    );
    _modal = route;
    unawaited(
      Navigator.of(context, rootNavigator: true).push(route).whenComplete(() {
        if (mounted && identical(_modal, route)) {
          _deletionReview = null;
          _modal = null;
          _newJobOpen = false;
          _review = null;
        }
      }),
    );
    if (!newJob) unawaited(jobs.loadDetail(requestId!));
  }

  void _reviewDraft(DialogRoute<void> route) {
    if (!_owns(route) ||
        !route.isCurrent ||
        !jobs.canStart ||
        _projectId == null ||
        !_draft.composing.isCollapsed ||
        devPromptError(_draft.text) != null) {
      return;
    }
    try {
      final args = jobs.capture(_projectId!, _draft.text);
      _review = _JobReview(
        args: args,
        project: jobs.projects.projects.firstWhere(
          (p) => p.projectId == _projectId,
        ),
        model: jobs.model!,
        epoch: _visibilityEpoch,
        draftVersion: _draftVersion,
      );
      _error = null;
    } on AppFailure catch (e) {
      _error = e.message;
    }
    _changed();
  }

  Future<void> _startReviewed(
    DialogRoute<void> route,
    _JobReview review,
  ) async {
    if (!identical(_review, review) || !_owns(route) || !route.isCurrent) {
      return;
    }
    if (!_sameOrigin(review) || !jobs.canStart) {
      _review = null;
      _error = 'Start unavailable or saved origin changed. Review again; draft retained.';
      _changed();
      return;
    }
    // Remove the review before dispatch: even a repeated callback cannot resend.
    // The draft remains editable while acknowledgement is pending.
    _review = null;
    _changed();
    final accepted = await jobs.start(review.args);
    if (!mounted || !_owns(route) || review.epoch != _visibilityEpoch) return;
    if (accepted &&
        review.draftVersion == _draftVersion &&
        review.project.projectId == _projectId) {
      _prompt.clear();
      _close(route, _newJobFocus);
    } else if (accepted) {
      _error = 'Job accepted. Newer draft retained.';
      _changed();
    }
  }

  ButtonStyle get _textButtonStyle =>
      TextButton.styleFrom(minimumSize: const Size(48, 48));
  ButtonStyle get _filledButtonStyle =>
      FilledButton.styleFrom(minimumSize: const Size(48, 48));

  Widget _actions(List<Widget> children) => SizedBox(
    width: 640,
    child: Wrap(
      alignment: WrapAlignment.end,
      spacing: 8,
      runSpacing: 8,
      children: children,
    ),
  );

  Widget _newJobDialog(DialogRoute<void> route, FocusNode originFocus) {
    final review = _review;
    final projectPresent = jobs.projects.projects.any(
      (p) => p.projectId == _projectId,
    );
    final promptError = devPromptError(_draft.text);
    final canReview =
        _owns(route) &&
        jobs.canStart &&
        projectPresent &&
        _draft.composing.isCollapsed &&
        promptError == null;
    return AlertDialog(
      key: const ValueKey('dev-new-job-dialog'),
      insetPadding: const EdgeInsets.all(12),
      constraints: const BoxConstraints(maxWidth: 720),
      scrollable: true,
      title: Text(review == null ? 'New job' : 'Start local Dev job?'),
      content: SizedBox(
        width: 640,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            if (review == null) ...[
              DropdownButtonFormField<String>(
                key: ValueKey(
                  'dev-project-${projectPresent ? _projectId : 'none'}',
                ),
                initialValue: projectPresent ? _projectId : null,
                isExpanded: true,
                decoration: const InputDecoration(
                  labelText: 'Project',
                  constraints: BoxConstraints(minHeight: 48),
                ),
                items: [
                  for (final project in jobs.projects.projects)
                    DropdownMenuItem(
                      value: project.projectId,
                      child: Text(project.key, overflow: TextOverflow.ellipsis),
                    ),
                ],
                onChanged: (value) {
                  if (!_owns(route) || !route.isCurrent) return;
                  _projectId = value;
                  _draftVersion++;
                  _error = null;
                  _changed();
                },
              ),
              const SizedBox(height: 16),
              Text(
                'LOCAL model: ${jobs.model?.name.isNotEmpty == true ? jobs.model!.name : 'not selected'}',
              ),

              const SizedBox(height: 16),
              TextField(
                controller: _prompt,
                minLines: 3,
                maxLines: 6,
                keyboardType: TextInputType.multiline,
                textInputAction: TextInputAction.newline,
                decoration: InputDecoration(
                  labelText: 'Task',
                  errorMaxLines: 5,
                  errorText: _draft.text.isEmpty ? null : promptError,
                ),
              ),
            ] else
              ..._reviewContent(review),
            ..._deletionStatus(),
            if (review == null) ...[
              ..._newJobStatus(),
              if (_error != null)
                Semantics(liveRegion: true, child: Text(_error!)),
            ] else ...[
              const SizedBox(height: 12),
              ..._admissionMessages(),
              for (final error in [_error, jobs.error, jobs.projects.error])
                if (error != null)
                  Semantics(liveRegion: true, child: Text(error)),
            ],
          ],
        ),
      ),
      actions: [
        _actions([
          TextButton(
            style: _textButtonStyle,
            onPressed: () => _close(route, originFocus),
            child: const Text('Close'),
          ),
          if (review != null)
            TextButton(
              style: _textButtonStyle,
              onPressed: () {
                if (!_owns(route) || !route.isCurrent) return;
                _review = null;
                _changed();
              },
              child: const Text('Back to edit'),
            ),
          if (review == null)
            FilledButton(
              style: _filledButtonStyle,
              onPressed: canReview ? () => _reviewDraft(route) : null,
              child: const Text('Review start'),
            )
          else
            FilledButton(
              style: _filledButtonStyle,
              onPressed: _owns(route) && _sameOrigin(review) && jobs.canStart
                  ? () => _startReviewed(route, review)
                  : null,
              child: const Text('Start job'),
            ),
        ]),
      ],
    );
  }

  List<Widget> _newJobStatus() {
    final reasons = <String>[
      if (jobs.busy)
        'Submitting…'
      else if (jobs.pendingRequestId != null)
        'Start unconfirmed. Resolve the original request.'
      else if (jobs.protocolBlocked)
        'Job state could not be verified.'
      else if (jobs.scheduler?.accepting == false)
        'Queue admission unavailable.',
      if (jobs.model?.ready != true) 'Local model not ready.',
      if (!jobs.loaded || jobs.jobsStale || !jobs.projects.canMutate)
        'Job data unavailable. Refresh Dev.',
      if (jobs.error != null || jobs.projects.error != null)
        'Could not complete this action.',
    ];
    if (reasons.isEmpty &&
        !jobs.canStart &&
        jobs.pendingDeletionRequestId == null &&
        jobs.deletingRequestId == null) {
      reasons.add('Job data unavailable. Refresh Dev.');
    }
    if (reasons.isEmpty) return [];
    return [
      const SizedBox(height: 12),
      ExpansionTile(
        key: const ValueKey('dev-start-diagnostics'),
        tilePadding: EdgeInsets.zero,
        childrenPadding: const EdgeInsets.only(bottom: 8),
        title: Semantics(
          liveRegion: true,
          child: Text(
            reasons.take(2).join('\n'),
            style: Theme.of(context).textTheme.bodySmall,
          ),
        ),
        children: [
          if (jobs.model?.sha256 != null)
            SelectableText('SHA-256: ${jobs.model!.sha256}'),
          if (jobs.model?.configRevision != null)
            Text('Configuration revision: ${jobs.model!.configRevision}'),
          if (jobs.model?.ready != true)
            const Text(
              'Check the local model in the installer, then Refresh Dev.',
            ),
          ..._admissionMessages(),
          for (final error in [jobs.error, jobs.projects.error])
            if (error != null) Text(error),
        ],
      ),
    ];
  }

  List<Widget> _reviewContent(_JobReview review) => [
    Text(
      'Saved project: ${review.project.key}\nProject ID: ${review.project.projectId}\nExecution harness: OpenCode\nLOCAL model: ${review.model.name}\nSHA-256: ${review.model.sha256}\nConfiguration revision: ${review.model.configRevision}',
    ),
    const SizedBox(height: 12),
    const Text('Public repository downloads (ordered):'),
    if (review.project.repositories.isEmpty)
      const Text('No repositories configured.'),
    for (final repo in review.project.repositories)
      Text('${repo.url}\nCheckout ref: ${repo.branch.base ?? 'HEAD'}'),
    const SizedBox(height: 12),
    const Text(
      'Included context: saved global instructions and exact saved project context below. No Smith Agent text is included.',
    ),
    SelectableText(
      review.project.prompt.isEmpty
          ? '(Project context is empty.)'
          : review.project.prompt,
    ),
    const SizedBox(height: 12),
    const Text('Exact task:'),
    SelectableText(review.args['prompt'] as String),
    const SizedBox(height: 12),
    const Text(
      'Start downloads the named PUBLIC HTTPS repositories with full Git history and normal branches/tags, then performs local inference and bounded read-only inspection. Downloads and checkouts remain subject to existing resource and time limits; repositories that exceed them fail the job. No cloud inference, scripts, code writes, installs, builds, commits or pushes. Repository text grants no permissions. Saved global instructions, saved project context and the exact task are frozen by the service. Job records and stopped containers are retained. Closing the window detaches; accepted work remains service-owned. No automatic setup or model download.',
    ),
  ];

  List<Widget> _admissionMessages() => [
    if (jobs.busy)
      const Text(
        'Awaiting admission acknowledgement… Close detaches, not cancels.',
      ),
    if (jobs.pendingRequestId != null)
      Text(
        'Uncertain admission: ${jobs.pendingRequestId}. New starts blocked; Resolve queries only the original request.',
      ),
    if (jobs.protocolBlocked)
      const Text(
        'Protocol blocked: a previously accepted identity could not be verified. New starts remain blocked; observation only.',
      ),

    if (jobs.scheduler?.accepting == false)
      const Text('Queue admission unavailable. Refresh observes only.'),
    if (!jobs.canStart && !jobs.busy)
      const Text(
        'Review and Start require fresh saved project metadata, a ready LOCAL model and available admission. Refresh is read-only.',
      ),
  ];

  Future<void> _resolveDeletion() async {
    final pending = _resolvingDeletion;
    if (pending.value ||
        jobs.deletingRequestId != null ||
        jobs.pendingDeletionRequestId == null) {
      return;
    }
    pending.value = true;
    try {
      await jobs.resolveDeletion();
    } finally {
      pending.value = false;
    }
  }

  List<Widget> _deletionStatus() => [
    if (jobs.deletingRequestId != null) const Text('Deleting from history…'),
    if (jobs.pendingDeletionRequestId != null &&
        jobs.deletingRequestId == null) ...[
      const Text('Deletion unconfirmed.'),
      TextButton(
        style: _textButtonStyle,
        onPressed: !_resolvingDeletion.value ? _resolveDeletion : null,
        child: const Text('Resolve'),
      ),
    ],
    if (jobs.deletionError != null)
      ExpansionTile(
        tilePadding: EdgeInsets.zero,
        title: const Text('Details'),
        children: [Text(jobs.deletionError!)],
      ),
  ];

  void _reviewDeletion(DialogRoute<void> route, DevJob job) {
    if (!_owns(route) ||
        !route.isCurrent ||
        !jobs.canDeleteJob(job.requestId)) {
      return;
    }
    _deletionReview = _JobDeletionReview(
      job: job,
      instanceId: jobs.projects.instanceId,
      epoch: _visibilityEpoch,
    );
    _changed();
  }

  bool _canConfirmDeletion(
    DialogRoute<void> route,
    _JobDeletionReview review,
  ) =>
      identical(_deletionReview, review) &&
      _owns(route) &&
      route.isCurrent &&
      review.epoch == _visibilityEpoch &&
      review.instanceId == jobs.projects.instanceId &&
      review.instanceId != null &&
      jobs.canDeleteJob(review.job.requestId);

  Future<void> _confirmDeletion(
    DialogRoute<void> route,
    _JobDeletionReview review,
  ) async {
    if (!_canConfirmDeletion(route, review)) return;
    // Consume this exact confirmation before dispatch; old callbacks cannot resend.
    _deletionReview = null;
    _changed();
    await jobs.deleteJob(review.job.requestId);
    // Shared state updates the surviving detail. Never pop a route on completion.
  }

  Widget _deleteConfirmation(
    DialogRoute<void> route,
    _JobDeletionReview review,
  ) => AlertDialog(
    key: ValueKey('dev-delete-confirm-${review.job.requestId}'),
    insetPadding: const EdgeInsets.all(12),
    constraints: const BoxConstraints(maxWidth: 720),
    scrollable: true,
    title: const Text('Delete job?'),
    content: SizedBox(
      width: 640,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text('Project: ${review.job.projectKey}'),
          SelectableText('Request: ${review.job.requestId}'),
          const SizedBox(height: 12),
          const Text(
            'Remove from history? Execution records and containers remain.',
          ),
        ],
      ),
    ),
    actions: [
      _actions([
        TextButton(
          autofocus: true,
          style: _textButtonStyle,
          onPressed: () {
            if (!_owns(route) ||
                !route.isCurrent ||
                !identical(_deletionReview, review)) {
              return;
            }
            _deletionReview = null;
            _changed();
          },
          child: const Text('Cancel'),
        ),
        FilledButton(
          style: _filledButtonStyle,
          onPressed: _canConfirmDeletion(route, review)
              ? () => _confirmDeletion(route, review)
              : null,
          child: const Text('Delete'),
        ),
      ]),
    ],
  );

  Widget _detailDialog(
    DialogRoute<void> route,
    String id,
    FocusNode originFocus,
  ) {
    final state = jobs.detailState(id);
    final data = state.data;
    final deleted = jobs.isDeleted(id);
    final review = _deletionReview;
    if (!deleted && review != null && review.job.requestId == id) {
      return _deleteConfirmation(route, review);
    }
    return AlertDialog(
      key: ValueKey('dev-job-detail-$id'),
      insetPadding: const EdgeInsets.all(12),
      constraints: const BoxConstraints(maxWidth: 720),
      scrollable: true,
      title: const Text('Job details'),
      content: SizedBox(
        width: 640,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            SelectableText('Request: $id'),
            if (deleted)
              Semantics(liveRegion: true, child: Text('Deleted from history')),
            if (!deleted) ...[
              ..._deletionStatus(),
              if (state.loading) const Text('Loading saved job details…'),
              if (state.stale)
                const Text(
                  'Saved details are stale; Refresh reads this request only.',
                ),
              if (state.error != null)
                Semantics(liveRegion: true, child: Text(state.error!)),
              if (data == null && !state.loading)
                const Text(
                  'Job details unavailable. No missing evidence is inferred.',
                ),
              // All result fields come from one keyed detail snapshot, never a
              // newer summary combined with an older model, message or reply.
              if (data != null) ..._detailContent(data),
            ],
          ],
        ),
      ),
      actions: [
        _actions([
          TextButton(
            style: _textButtonStyle,
            onPressed: () =>
                _close(route, deleted ? _newJobFocus : originFocus),
            child: const Text('Close'),
          ),
          if (!deleted) ...[
            TextButton(
              style: _textButtonStyle,
              onPressed: _owns(route) && route.isCurrent && !state.loading
                  ? () => jobs.loadDetail(id, force: true)
                  : null,
              child: const Text('Refresh details (read-only)'),
            ),
            if (data?.job.canDelete == true)
              TextButton(
                style: _textButtonStyle,
                onPressed:
                    _owns(route) && route.isCurrent && jobs.canDeleteJob(id)
                    ? () => _reviewDeletion(route, data!.job)
                    : null,
                child: const Text('Delete'),
              ),
          ],
        ]),
      ],
    );
  }

  List<Widget> _detailContent(DevJobDetail data) => [
    Text('Project: ${data.job.projectKey}'),
    SelectableText('Project ID: ${data.job.projectId}'),
    Text(_phase(data.job)),
    Text(data.job.message),
    Text('Created UTC: ${_utc(data.job)}'),
    if (data.job.sessionId != null)
      SelectableText('Session: ${data.job.sessionId}'),
    Text(
      data.job.replyAvailable
          ? 'Reply: saved final reply available.'
          : 'Reply: no saved final reply.',
    ),
    Text(
      data.job.status == 'queued'
          ? 'Not started.'
          : data.job.status == 'reconciled'
          ? 'Operator verified all execution quiescent; historical task outcome remains unknown.'
          : data.job.cleanupVerified
          ? 'Owned container stop verified.'
          : 'Cleanup not verified; reply availability is separate.',
    ),
    if (data.job.active || !data.job.permitsNewAttempt)
      const Text(
        'Stop is not available in this client. Closing only detaches; it does not cancel service-owned work.',
      ),
    if (!data.job.active && !data.job.cleanupVerified)
      const Text(
        'Cleanup uncertain. Separate operator reconciliation is required.',
      ),
    const SizedBox(height: 12),
    Text(
      'Run model: ${data.model.name.isEmpty ? 'unavailable' : data.model.name}',
    ),
    SelectableText('SHA-256: ${data.model.sha256 ?? 'unavailable'}'),
    if (data.failedStage != null) Text('Failed stage: ${data.failedStage}'),
    const SizedBox(height: 12),
    const Text('Recorded repository checkout evidence:'),
    if (data.repositories.isEmpty)
      const Text(
        'No recorded checkout evidence. This does not mean no repositories were configured.',
      ),
    for (final repo in data.repositories)
      SelectableText('${repo.url}\nRef: ${repo.ref}\nCommit: ${repo.commit}'),
    if (data.reply != null) ...[
      const SizedBox(height: 12),
      const Text(
        'Saved final reply — untrusted text, not execution or delivery approval:',
      ),
      SelectableText(data.reply!),
    ],
  ];

  String _phase(DevJob job) => job.status == 'reconciled'
      ? 'Operator-reconciled — historical task outcome unknown'
      : 'Phase: ${job.status}';

  String _utc(DevJob job) =>
      DateTime.fromMillisecondsSinceEpoch(job.createdMs)
          .toUtc()
          .toIso8601String();

  Widget _jobRow(DevJob job) => Padding(
    key: ValueKey('dev-job-${job.requestId}'),
    padding: const EdgeInsets.only(top: 8),
    child: Material(
      shape: RoundedRectangleBorder(
        side: BorderSide(color: Theme.of(context).dividerColor),
        borderRadius: BorderRadius.circular(4),
      ),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        focusNode: _rowFocus.putIfAbsent(job.requestId, FocusNode.new),
        onTap: () => _openModal(newJob: false, requestId: job.requestId),
        child: Semantics(
          button: true,
          label: 'Read saved job details: ${job.requestId}, ${job.projectKey}',
          child: ConstrainedBox(
            constraints: const BoxConstraints(minHeight: 48),
            child: Padding(
              padding: const EdgeInsets.all(12),
              child: LayoutBuilder(
                builder: (context, constraints) {
                  final identity = Text(
                    '#${job.requestId.substring(0, 8)}  ${job.projectKey}',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  );
                  final status = Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Icon(
                        job.active
                            ? Icons.radio_button_unchecked
                            : job.status == 'completed'
                            ? Icons.check_circle_outline
                            : Icons.info_outline,
                        size: 16,
                      ),
                      const SizedBox(width: 6),
                      Flexible(
                        child: Text(
                          job.status.replaceAll('_', ' ').toUpperCase(),
                        ),
                      ),
                    ],
                  );
                  return constraints.maxWidth >=
                          400 * MediaQuery.textScalerOf(context).scale(1)
                      ? Row(
                          children: [
                            Expanded(child: identity),
                            const SizedBox(width: 16),
                            status,
                          ],
                        )
                      : Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            identity,
                            const SizedBox(height: 8),
                            status,
                          ],
                        );
                },
              ),
            ),
          ),
        ),
      ),
    ),
  );

  @override
  void dispose() {
    jobs.removeListener(_originChanged);
    jobs.projects.removeListener(_originChanged);
    _visibilityEpoch++;
    _retireModal(afterFrame: true);
    _prompt.removeListener(_draftChanged);
    _prompt.dispose();
    _modalChanges.dispose();
    _newJobFocus.dispose();
    for (final focus in _rowFocus.values) {
      focus.dispose();
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: Listenable.merge([jobs, jobs.projects, _resolvingDeletion]),
    builder: (context, _) => SingleChildScrollView(
      padding: const EdgeInsets.all(16),
      child: Align(
        alignment: Alignment.topLeft,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 1100),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              RetainedJobsView(controller: jobs.retainedJobs),
              Wrap(
                spacing: 12,
                runSpacing: 8,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: [
                  const Text('Pipeline jobs'),
                  FilledButton(
                    focusNode: _newJobFocus,
                    style: _filledButtonStyle,
                    onPressed: () => _openModal(newJob: true),
                    child: const Text('New job'),
                  ),
                  TextButton(
                    style: _textButtonStyle,
                    onPressed: jobs.busy || jobs.refreshing ? null : _refresh,
                    child: const Text('Refresh'),
                  ),
                  if (jobs.pendingRequestId != null)
                    TextButton(
                      style: _textButtonStyle,
                      onPressed: jobs.busy || jobs.refreshing
                          ? null
                          : jobs.refresh,
                      child: const Text('Resolve original request'),
                    ),
                ],
              ),
              const SizedBox(height: 12),
              if (jobs.refreshing) const Text('Refreshing…'),
              if (!jobs.loaded && jobs.error == null)
                const Text('Loading jobs…'),
              if (!jobs.loaded && jobs.error != null)
                const Text('Jobs unavailable. Refresh to try again.'),
              if (jobs.jobsStale)
                const Text('Jobs may be out of date. Refresh to update.'),
              if (jobs.loaded && !jobs.jobsStale && jobs.jobs.isEmpty)
                const Text('No jobs yet.'),
              ..._deletionStatus(),
              ..._admissionMessages(),
              if (jobs.observationStopped)
                const Text('Auto-refresh paused. Refresh to update.'),
              for (final error in [jobs.error, jobs.projects.error])
                if (error != null)
                  Semantics(liveRegion: true, child: Text(error)),
              for (final job in jobs.jobs) _jobRow(job),
            ],
          ),
        ),
      ),
    ),
  );
}

final class _JobDeletionReview {
  const _JobDeletionReview({
    required this.job,
    required this.instanceId,
    required this.epoch,
  });
  final DevJob job;
  final String? instanceId;
  final int epoch;
}

class _JobReview {
  const _JobReview({
    required this.args,
    required this.project,
    required this.model,
    required this.epoch,
    required this.draftVersion,
  });
  final Map<String, dynamic> args;
  final ProjectRecord project;
  final DevLocalModel model;
  final int epoch, draftVersion;
}
