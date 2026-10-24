import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../projects.dart';
import '../projects_controller.dart';
import 'smith_icon.dart';
import 'smith_theme.dart';

/// Per-view, volatile editor state; the desktop owns the shared registry.
class ProjectsView extends StatefulWidget {
  const ProjectsView({super.key, required this.controller});

  final ProjectsController controller;

  @override
  State<ProjectsView> createState() => _ProjectsViewState();
}

class _ProjectsViewState extends State<ProjectsView> {
  final _key = TextEditingController();
  final _description = TextEditingController();
  final _prompt = TextEditingController();
  String? _compatibilityName;

  final _repositories = <_RepositoryDraft>[];
  final _addRepositoryFocus = FocusNode();
  final _listScroll = _ProjectsScrollController();
  final _editorScroll = _ProjectsScrollController();

  bool _editorOpen = false;
  ProjectRecord? _original;
  int? _editorRevision;
  String? _editorInstanceId;
  bool _creating = false;
  bool _validation = false;
  bool _saving = false;
  bool _choosing = false;
  int _editorEpoch = 0;
  String? _localError;
  _EditorSubmission? _submission;
  bool _creationAcknowledged = false;

  ProjectsController get registry => widget.controller;
  bool get _editing => _creating || _original != null;
  bool get _editorEnabled => !_saving && _submission?.deleting != true;
  List<List<String>> get _draftValues =>
      List.unmodifiable(_repositories.map((row) => row.values));
  bool get _dirty =>
      _creating ||
      (_original != null &&
          (_key.text != _original!.key ||
              _description.text != _original!.description ||
              _prompt.text != _original!.prompt ||
              !_sameRepositories(
                _draftValues,
                _original!.repositories.map(_repositoryValues).toList(),
              )));
  ProjectRecord? _record(String id) {
    for (final project in registry.projects) {
      if (project.projectId == id) return project;
    }
    return null;
  }

  bool get _missing =>
      _original != null &&
      registry.loaded &&
      _record(_original!.projectId) == null;
  bool get _stale =>
      _editing &&
      (_editorInstanceId != registry.instanceId ||
          _editorRevision != registry.revision ||
          _missing);
  bool get _writeAllowed =>
      registry.canMutate &&
      !registry.loading &&
      !_saving &&
      !_stale &&
      !_creationAcknowledged;

  @override
  void initState() {
    super.initState();
    registry.addListener(_registryChanged);
    // Construction is inert. Mounting happens only on the first Projects visit.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && !registry.loaded) unawaited(_refresh());
    });
  }

  @override
  void didUpdateWidget(covariant ProjectsView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != registry) {
      oldWidget.controller.removeListener(_registryChanged);
      registry.addListener(_registryChanged);
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && !registry.loaded) unawaited(_refresh());
      });
    }
  }

  void _registryChanged() {
    if (!mounted) return;
    if (!_saving) _reconcileSubmission();
    // Dirty forms retain both origin fences; clean reloads also invalidate dialogs.
    if (!_saving &&
        !_dirty &&
        _original != null &&
        registry.loaded &&
        !registry.loading &&
        !registry.busy &&
        registry.error == null &&
        !registry.uncertain &&
        _stale) {
      final current = _record(_original!.projectId);
      _load(current);
    }
    setState(() {});
  }

  void _load(ProjectRecord? project, {bool creating = false}) {
    _editorEpoch++;
    _submission = null;
    _creationAcknowledged = false;
    _original = project;
    _creating = creating;
    _editorRevision = registry.revision;
    _editorInstanceId = registry.instanceId;
    _compatibilityName = project?.name;
    _key.text = project?.key ?? '';
    _description.text = project?.description ?? '';
    _prompt.text = project?.prompt ?? '';
    for (final row in _repositories) {
      row.dispose();
    }
    _repositories
      ..clear()
      ..addAll([
        for (final repository in project?.repositories ?? <ProjectRepository>[])
          _RepositoryDraft(repository),
      ]);
    if (!_editing) _editorOpen = false;
    _validation = false;
    _localError = null;
  }

  void _changed(String _) => setState(() => _localError = null);

  String? get _keyError {
    final error = projectKeyError(_key.text);
    if (error != null) return error;
    if (registry.projects.any(
      (p) => p.projectId != _original?.projectId && p.key == _key.text,
    )) {
      return 'A project with this ID already exists.';
    }
    return null;
  }

  Future<void> _cancel() async {
    if (_saving || _choosing) return;
    _choosing = true;
    final epoch = _editorEpoch;
    final discard = await _discard();
    _choosing = false;
    if (!mounted || _saving || epoch != _editorEpoch || !discard) return;
    setState(
      () => _load(_original == null ? null : _record(_original!.projectId)),
    );
  }

  Future<void> _duplicate() async {
    final source = _original;
    if (source == null ||
        !_writeAllowed ||
        _choosing ||
        registry.projects.length >= maxProjects) {
      return;
    }
    final epoch = _editorEpoch;
    final instance = registry.instanceId;
    final revision = registry.revision;
    _choosing = true;
    final discard = await _discard();
    _choosing = false;
    if (!mounted ||
        epoch != _editorEpoch ||
        !discard ||
        !_writeAllowed ||
        registry.instanceId != instance ||
        registry.revision != revision ||
        _record(source.projectId) != source) {
      return;
    }
    setState(() {
      _load(null, creating: true);
      _compatibilityName = source.name;
      _description.text = source.description;
      _prompt.text = source.prompt;
      _repositories.addAll(source.repositories.map(_RepositoryDraft.new));
      _editorOpen = true;
      _editorScroll.jumpTo(0);
    });
  }

  Future<bool> _discard() async =>
      !_dirty ||
      await showDialog<bool>(
            context: context,
            builder: (context) => AlertDialog(
              scrollable: true,
              title: const Text('Discard unsaved project changes?'),
              content: const Text(
                'These edits exist only in this view. No saved project metadata will change.',
              ),
              actions: [
                TextButton(
                  style: TextButton.styleFrom(minimumSize: const Size(48, 48)),
                  autofocus: true,
                  onPressed: () => Navigator.pop(context, false),
                  child: const Text('Keep editing'),
                ),
                TextButton(
                  style: TextButton.styleFrom(minimumSize: const Size(48, 48)),
                  onPressed: () => Navigator.pop(context, true),
                  child: const Text('Discard changes'),
                ),
              ],
            ),
          ) ==
          true;

  Future<void> _select(String? id) async {
    if (_saving || _choosing) return;
    // Returning from the compact list is navigation, not a reload or a discard.
    if (id != null && id == _original?.projectId) {
      setState(() => _editorOpen = true);
      return;
    }
    _choosing = true;
    final epoch = _editorEpoch;
    final discard = await _discard();
    _choosing = false;
    if (!mounted || _saving || epoch != _editorEpoch) return;
    if (!discard) {
      if (_editing) setState(() => _editorOpen = true);
      return;
    }
    final project = id == null ? null : _record(id);
    if (id != null && project == null) {
      setState(
        () => _localError = 'That project is no longer in the registry. Your edits are preserved.',
      );
      return;
    }
    setState(() {
      _load(project, creating: id == null);
      _editorOpen = true;
      _editorScroll.jumpTo(0);
    });
  }

  Future<void> _refresh() async {
    try {
      await registry.refresh();
    } on Object {
      if (mounted) {
        setState(
          () => _localError = 'Project metadata could not be refreshed. Your edits are preserved.',
        );
      }
    }
  }

  Future<void> _resolve() async {
    try {
      await registry.resolvePending();
    } on Object {
      if (mounted) {
        setState(
          () => _localError =
              'The project change is still unresolved. No change was retried.',
        );
      }
    }
  }

  String? _repositoryIdError(int index) {
    final value = _repositories[index].optional(0);
    final error = projectRepositoryIdError(value);
    if (error != null) return error;
    if (value != null &&
        _repositories.where((row) => row.optional(0) == value).length > 1) {
      return 'Repository IDs must be distinct.';
    }
    return null;
  }

  List<Widget> _reviewRepositories(List<List<String>> repositories) => [
    for (var i = 0; i < repositories.length; i++) ...[
      Text('Repository ${i + 1}'),
      for (var j = 0; j < 8; j++)
        Text(
          '${_repositoryLabels[j]}: ${repositories[i][j].isEmpty ? 'Not configured' : repositories[i][j]}',
        ),
      Text('Read Only: ${repositories[i][8] == 'true' ? 'Yes' : 'No'}'),
    ],
  ];

  Widget _repositoryCard(int index) {
    final row = _repositories[index];
    final epoch = _editorEpoch;
    Widget field(int i) => Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: TextField(
        key: ValueKey(
          i == 1
              ? 'project-repository-$index'
              : 'project-repository-$index-${_repositoryFields[i]}',
        ),
        controller: row.fields[i],
        focusNode: i == 0 ? row.nameFocus : null,
        inputFormatters: i == 0 ? [row.idFormatter] : null,
        // Single-line fields strip pasted newlines before custom formatters.
        // Let the ID formatter reject the whole edit instead.
        maxLines: i == 0 ? null : 1,
        keyboardType: TextInputType.text,
        textInputAction: TextInputAction.done,
        enabled: _editorEnabled,
        onChanged: _changed,
        decoration: InputDecoration(
          isDense: true,
          contentPadding: const EdgeInsets.all(12),
          labelText: _repositoryLabels[i],
          hintText: _repositoryHints[i],
          errorText: !_validation
              ? null
              : i == 0
              ? _repositoryIdError(index)
              : i == 1
              ? _repositoryError(index)
              : row.error(i),
          errorMaxLines: 8,
        ),
      ),
    );
    final idError = _validation ? _repositoryIdError(index) : null;
    final hiddenErrors = [
      if (_validation)
        for (var i = 2; i < 8; i++)
          if (row.error(i) != null) '${_repositoryLabels[i]}: ${row.error(i)}',
    ];
    return Container(
      key: ObjectKey(row),
      margin: const EdgeInsets.only(bottom: 8),
      decoration: BoxDecoration(
        color: SmithColors.surface,
        border: Border.all(color: SmithColors.strokeSubtle),
        borderRadius: BorderRadius.circular(
          Theme.of(context).extension<SmithTokens>()!.radius,
        ),
      ),
      child: Material(
        color: Colors.transparent,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.only(left: 12, right: 4),
              child: Row(
                children: [
                  const SmithIcon('git-branch', size: 20),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Padding(
                      padding: const EdgeInsets.symmetric(vertical: 8),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Wrap(
                            spacing: 12,
                            children: [
                              Text(
                                row.fields[0].text.isEmpty
                                    ? 'Repository ID required'
                                    : row.fields[0].text,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                              ),
                              if (row.readOnly)
                                Text(
                                  'Read only',
                                  style: Theme.of(context).textTheme.bodySmall,
                                ),
                            ],
                          ),
                          Text(
                            row.fields[1].text.isEmpty
                                ? 'URL not configured'
                                : row.fields[1].text,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: Theme.of(context).textTheme.bodySmall,
                          ),
                        ],
                      ),
                    ),
                  ),
                  IconButton(
                    key: ValueKey('expand-project-repository-$index'),
                    tooltip: row.expanded
                        ? 'Collapse repository ${index + 1}'
                        : 'Configure repository ${index + 1}',
                    onPressed: () =>
                        setState(() => row.expanded = !row.expanded),
                    icon: SmithIcon(
                      row.expanded ? 'chevron-down' : 'chevron-right',
                      size: 20,
                    ),
                  ),
                  PopupMenuButton<String>(
                    key: ValueKey('project-repository-$index-menu'),
                    tooltip: 'Repository ${index + 1} actions',
                    enabled: _editorEnabled,
                    icon: const SmithIcon('ellipsis-vertical', size: 20),
                    itemBuilder: (_) => [
                      PopupMenuItem(
                        value: 'remove',
                        height: 48,
                        child: Text(
                          'Remove repository',
                          key: ValueKey('remove-project-repository-$index'),
                        ),
                      ),
                    ],
                    onSelected: (_) {
                      if (!mounted ||
                          epoch != _editorEpoch ||
                          !_editorEnabled ||
                          !_repositories.contains(row) ||
                          !TickerMode.valuesOf(context).enabled) {
                        return;
                      }
                      setState(() {
                        _repositories.remove(row);
                        _localError = null;
                      });
                      WidgetsBinding.instance.addPostFrameCallback((_) {
                        row.dispose();
                        if (mounted) _addRepositoryFocus.requestFocus();
                      });
                    },
                  ),
                ],
              ),
            ),
            if (!row.expanded && idError != null)
              Padding(
                padding: const EdgeInsets.fromLTRB(12, 0, 12, 12),
                child: Text(
                  idError,
                  style: const TextStyle(color: SmithColors.danger),
                ),
              ),
            if (hiddenErrors.isNotEmpty)
              Padding(
                padding: const EdgeInsets.all(12),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    for (final error in hiddenErrors)
                      Text(
                        'Retained $error',
                        style: const TextStyle(color: SmithColors.danger),
                      ),
                    const Text(
                      'These settings are not editable here. Cancel to reload saved metadata, or remove this repository from the draft.',
                    ),
                  ],
                ),
              ),
            _retainedPane(
              visible: row.expanded,
              child: Padding(
                padding: const EdgeInsets.fromLTRB(12, 8, 12, 12),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    _pairedFields(
                      field(0),
                      field(1),
                      minimumWidth: 600,
                      secondFlex: 2,
                    ),
                    SwitchListTile(
                      key: ValueKey('project-repository-$index-read-only'),
                      title: const Text('Read Only'),
                      value: row.readOnly,
                      contentPadding: EdgeInsets.zero,
                      onChanged: !_editorEnabled
                          ? null
                          : (value) => setState(() {
                              row.readOnly = value;
                              _localError = null;
                            }),
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _pairedFields(
    Widget first,
    Widget second, {
    required double minimumWidth,
    int secondFlex = 1,
  }) => LayoutBuilder(
    builder: (context, bounds) {
      final tokens = Theme.of(context).extension<SmithTokens>()!;
      final scale = MediaQuery.textScalerOf(context).scale(14) / 14;
      final paired = bounds.maxWidth >= minimumWidth * scale;
      final unit = (bounds.maxWidth - tokens.gutter) / (1 + secondFlex);
      // Stable Wrap/SizedBox slots keep TextField state and selection on reflow.
      return Wrap(
        spacing: tokens.gutter,
        runSpacing: tokens.gutter,
        children: [
          SizedBox(width: paired ? unit : bounds.maxWidth, child: first),
          SizedBox(
            width: paired ? unit * secondFlex : bounds.maxWidth,
            child: second,
          ),
        ],
      );
    },
  );

  String? _repositoryError(int index) {
    final value = _repositories[index].fields[1].text;
    final error = repositoryLocatorError(value);
    if (error != null) return error;
    if (_repositories.where((row) => row.fields[1].text == value).length > 1) {
      return 'Repository locators must be distinct.';
    }
    return null;
  }

  Future<void> _save() async {
    if (!_editing || !_writeAllowed || !_dirty) return;
    setState(() {
      _validation = true;
      for (var i = 0; i < _repositories.length; i++) {
        final row = _repositories[i];
        if (row.firstError != null ||
            _repositoryIdError(i) != null ||
            _repositoryError(i) != null) {
          row.expanded = true;
        }
      }
    });
    if (_keyError != null ||
        projectDescriptionError(_description.text) != null ||
        projectPromptError(_prompt.text) != null ||
        _repositories.length > maxProjectRepositories ||
        List.generate(
          _repositories.length,
          (i) =>
              _repositoryError(i) ??
              _repositoryIdError(i) ??
              _repositories[i].firstError,
        ).any((e) => e != null)) {
      return;
    }
    final revision = _editorRevision;
    final instanceId = _editorInstanceId;
    if (revision == null || instanceId == null) return;
    final id = _original?.projectId;
    final epoch = _editorEpoch;
    final repositories = List<ProjectRepository>.unmodifiable(
      _repositories.map((row) => row.toRepository()),
    );
    _submission = _EditorSubmission(
      epoch,
      instanceId,
      revision,
      id,
      _compatibilityName ?? _key.text,
      _draftValues,
      registry.projects.map((project) => project.projectId).toSet(),
      key: _key.text,
      description: _description.text,
      prompt: _prompt.text,
    );
    setState(() {
      _saving = true;
      _localError = null;
    });
    try {
      if (id == null) {
        await registry.create(
          _submission!.name,
          repositories,
          expectedInstanceId: instanceId,
          expectedRevision: revision,
          key: _submission!.key,
          description: _submission!.description,
          prompt: _submission!.prompt,
        );
      } else {
        await registry.update(
          id,
          _submission!.name,
          repositories,
          expectedInstanceId: instanceId,
          expectedRevision: revision,
          key: _submission!.key,
          description: _submission!.description,
          prompt: _submission!.prompt,
        );
      }
      if (!mounted || epoch != _editorEpoch) return;
      _reconcileSubmission();
    } on Object {
      if (mounted) _localError = 'Project metadata could not be saved. Your edits are preserved; nothing is retried automatically.';
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  void _reconcileSubmission() {
    final submitted = _submission;
    if (submitted == null ||
        registry.busy ||
        registry.loading ||
        registry.uncertain) {
      return;
    }
    _submission = null;
    if (submitted.epoch != _editorEpoch ||
        registry.error != null ||
        !registry.loaded) {
      return;
    }
    // A confirmed delete plus current absence completes the exact-target intent,
    // including across restart; it does not rebind a dirty editor to that instance.
    if (submitted.deleting && _record(submitted.projectId!) == null) {
      _load(null);
      return;
    }
    if (submitted.instanceId != registry.instanceId) {
      _creationAcknowledged = submitted.projectId == null;
      _localError = submitted.projectId == null
          ? 'Creation was acknowledged, but Smith restarted. Your input is preserved. Choose a saved project or Cancel this editor; do not repeat the creation.'
          : 'The earlier change was acknowledged, but Smith restarted. Your edits are preserved; review the current instance and revision before making another change.';
      return;
    }
    if (submitted.deleting) return;
    ProjectRecord? saved;
    if (submitted.projectId != null) {
      saved = _record(submitted.projectId!);
    } else if (registry.revision == submitted.revision + 1) {
      // With no intervening registry writes, exactly one new identity can be
      // attributed to this acknowledged create. Later snapshots cannot do so.
      final added = registry.projects
          .where((project) => !submitted.knownIds.contains(project.projectId))
          .toList();
      if (added.length == 1 &&
          added.single.name == submitted.name &&
          added.single.key == submitted.key &&
          added.single.description == submitted.description &&
          added.single.prompt == submitted.prompt &&
          _sameRepositories(
            added.single.repositories.map(_repositoryValues).toList(),
            submitted.repositories,
          )) {
        saved = added.single;
      }
    }
    if (saved == null) {
      _creationAcknowledged = submitted.projectId == null;
      _localError = 'The change was acknowledged, but its target is not identifiable in the current list. Your input is preserved. Choose a saved project or Cancel this editor; do not repeat the creation.';
      return;
    }
    if (_key.text == submitted.key &&
        _description.text == submitted.description &&
        _prompt.text == submitted.prompt &&
        _sameRepositories(_draftValues, submitted.repositories)) {
      _load(saved);
    } else {
      // Edits made during uncertainty are newer than the acknowledged request.
      // Bind the known target, but retain the old revision until explicit review.
      _original = saved;
      _compatibilityName = saved.name;
      _creating = false;
      _localError = 'The earlier change was acknowledged. Your newer edits are preserved; review the current revision before saving them.';
    }
  }

  Future<void> _reviewRevision() async {
    if (!_stale ||
        _creationAcknowledged ||
        _missing ||
        !registry.canMutate ||
        registry.loading ||
        _saving) {
      return;
    }
    final revision = registry.revision;
    final instanceId = registry.instanceId;
    if (revision == null || instanceId == null) return;
    final epoch = _editorEpoch;
    final editorInstanceId = _editorInstanceId;
    final editorRevision = _editorRevision;
    final current = _original == null ? null : _record(_original!.projectId);
    bool eligible() =>
        mounted &&
        epoch == _editorEpoch &&
        _editorInstanceId == editorInstanceId &&
        _editorRevision == editorRevision &&
        !_saving &&
        registry.canMutate &&
        !registry.loading &&
        registry.instanceId == instanceId &&
        registry.revision == revision &&
        !_missing;
    final accepted = await showDialog<bool>(
      context: context,
      builder: (context) => ListenableBuilder(
        listenable: registry,
        builder: (context, _) => AlertDialog(
          scrollable: true,
          title: const Text('Review current project metadata'),
          content: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('Registry revision: $revision'),
              const Text(
                'This review is bound to the currently observed Smith instance.',
              ),
              const SizedBox(height: 16),
              if (current == null)
                Text(
                  '${registry.projects.length} saved projects. This new project has not been saved.',
                )
              else ...[
                const Text('Currently saved'),
                Text('ID: ${current.key}'),
                Text('Description: ${current.description}'),
                Text('Prompt: ${current.prompt}'),
                ..._reviewRepositories(
                  current.repositories.map(_repositoryValues).toList(),
                ),
                if (current.repositories.isEmpty) const Text('No repositories'),
              ],
              const SizedBox(height: 16),
              const Text('Your unsaved edits'),
              Text('ID: ${_key.text}'),
              Text('Description: ${_description.text}'),
              Text('Prompt: ${_prompt.text}'),
              ..._reviewRepositories(_draftValues),
              if (_repositories.isEmpty) const Text('No repositories'),
              const SizedBox(height: 16),
              const Text(
                'Keep these edits against this instance and revision, then Save explicitly. Reviewing does not change saved metadata.',
              ),
              if (!eligible()) ...[
                const SizedBox(height: 16),
                const Text(
                  'Smith or the registry changed. Close this review and review current metadata again.',
                ),
              ],
            ],
          ),
          actions: [
            TextButton(
              style: TextButton.styleFrom(minimumSize: const Size(48, 48)),
              autofocus: true,
              onPressed: () => Navigator.pop(context, false),
              child: const Text('Keep blocked'),
            ),
            TextButton(
              style: TextButton.styleFrom(minimumSize: const Size(48, 48)),
              onPressed: eligible() ? () => Navigator.pop(context, true) : null,
              child: const Text('Keep edits at this revision'),
            ),
          ],
        ),
      ),
    );
    if (accepted != true || !eligible()) return;
    setState(() {
      _editorEpoch++;
      _editorInstanceId = instanceId;
      _editorRevision = revision;
      _original = current;
      _localError = null;
    });
  }

  Future<void> _delete() async {
    final project = _original;
    final revision = _editorRevision;
    final instanceId = _editorInstanceId;
    if (project == null ||
        revision == null ||
        instanceId == null ||
        !_writeAllowed) {
      return;
    }
    final epoch = _editorEpoch;
    bool eligible() =>
        mounted &&
        epoch == _editorEpoch &&
        _writeAllowed &&
        _editorInstanceId == instanceId &&
        _editorRevision == revision &&
        registry.instanceId == instanceId &&
        registry.revision == revision &&
        _record(project.projectId) != null;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => ListenableBuilder(
        listenable: registry,
        builder: (context, _) => AlertDialog(
          scrollable: true,
          title: const Text('Delete project metadata?'),
          content: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(project.key),
              const SizedBox(height: 16),
              const Text(
                'Only this project’s saved metadata will be removed. Repository files, chats, credentials and installations are not changed. This cannot be undone. Unsaved edits for this project will be discarded.',
              ),
              if (!eligible()) ...[
                const SizedBox(height: 16),
                const Text(
                  'The target or registry changed. Cancel and review current metadata before deleting.',
                ),
              ],
            ],
          ),
          actions: [
            TextButton(
              style: TextButton.styleFrom(minimumSize: const Size(48, 48)),
              autofocus: true,
              onPressed: () => Navigator.pop(context, false),
              child: const Text('Keep project'),
            ),
            TextButton(
              style: TextButton.styleFrom(minimumSize: const Size(48, 48)),
              onPressed: eligible() ? () => Navigator.pop(context, true) : null,
              child: const Text('Delete metadata'),
            ),
          ],
        ),
      ),
    );
    if (confirmed != true || !eligible()) return;
    setState(() {
      _saving = true;
      _localError = null;
      _submission = _EditorSubmission(
        epoch,
        instanceId,
        revision,
        project.projectId,
        project.name,
        _draftValues,
        const {},
        key: _key.text,
        description: _description.text,
        prompt: _prompt.text,
        deleting: true,
      );
    });
    try {
      await registry.delete(
        project.projectId,
        expectedInstanceId: instanceId,
        expectedRevision: revision,
      );
      if (mounted && epoch == _editorEpoch) _reconcileSubmission();
    } on Object {
      if (mounted) _localError = 'Project deletion could not be confirmed. Your edits are preserved; nothing is retried automatically.';
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  Widget _heading(String text) => Semantics(
    header: true,
    child: Text(text, style: Theme.of(context).textTheme.titleMedium),
  );

  Widget _newProjectButton() => OutlinedButton(
    style: OutlinedButton.styleFrom(minimumSize: const Size(48, 48)),
    key: const ValueKey('new-project'),
    onPressed:
        registry.canMutate &&
            !registry.loading &&
            !_saving &&
            registry.projects.length < maxProjects
        ? () => _select(null)
        : null,
    child: const Text('Create'),
  );

  Widget _refreshButton({bool list = false}) => TextButton(
    style: TextButton.styleFrom(minimumSize: const Size(48, 48)),
    key: ValueKey(list ? 'refresh-projects-list' : 'refresh-projects'),
    onPressed: registry.loading || registry.busy || _saving ? null : _refresh,
    child: const Text('Refresh'),
  );

  Widget _projectList(bool compact) {
    final tokens = Theme.of(context).extension<SmithTokens>()!;

    return SingleChildScrollView(
      key: const ValueKey('project-list'),
      controller: _listScroll,
      padding: EdgeInsets.all(tokens.gutter),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Expanded(child: _heading('Projects')),
              SizedBox(width: tokens.gap),
              _newProjectButton(),
            ],
          ),
          if (compact &&
              _editing &&
              !registry.projects.any(
                (project) => project.projectId == _original?.projectId,
              ))
            Align(
              alignment: Alignment.centerLeft,
              child: TextButton(
                key: const ValueKey('resume-project-editor'),
                style: TextButton.styleFrom(minimumSize: const Size(48, 48)),
                onPressed: () => setState(() => _editorOpen = true),
                child: const Text('Return to project editor'),
              ),
            ),
          SizedBox(height: tokens.gutter),
          if (compact) _statusRegion(list: true),
          if (registry.projects.isEmpty)
            Text(
              registry.loading
                  ? 'Loading projects…'
                  : !registry.loaded || registry.error != null
                  ? 'Project metadata unavailable.'
                  : 'No projects yet.',
            ),
          for (final project in registry.projects)
            Padding(
              padding: EdgeInsets.only(bottom: tokens.gap),
              child: TextButton(
                key: ValueKey('project-${project.projectId}'),
                onPressed: _saving ? null : () => _select(project.projectId),
                style:
                    TextButton.styleFrom(
                      alignment: Alignment.centerLeft,
                      minimumSize: const Size(48, 48),
                      padding: EdgeInsets.all(tokens.gap),
                      backgroundColor: _original?.projectId == project.projectId
                          ? SmithColors.surfaceSelected
                          : SmithColors.surface,
                    ).copyWith(
                      side: WidgetStateProperty.resolveWith(
                        (states) => BorderSide(
                          color:
                              states.contains(WidgetState.focused) &&
                                  !states.contains(WidgetState.disabled)
                              ? SmithColors.accentBright
                              : SmithColors.strokeSubtle,
                        ),
                      ),
                    ),
                child: Semantics(
                  selected: _original?.projectId == project.projectId,
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Container(
                        width: 2,
                        height: 40,
                        color: _original?.projectId == project.projectId
                            ? SmithColors.accent
                            : Colors.transparent,
                      ),
                      SizedBox(width: tokens.gap),
                      const SmithIcon('layers'),
                      SizedBox(width: tokens.gap),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Tooltip(
                              message: project.key,
                              child: Text(
                                project.key,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                            Text(
                              project.description.isEmpty
                                  ? 'No description'
                                  : project.description,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: Theme.of(context).textTheme.bodySmall,
                            ),
                          ],
                        ),
                      ),
                      SizedBox(width: tokens.gap),
                      Semantics(
                        label:
                            '${project.repositories.length} ${project.repositories.length == 1 ? 'repository' : 'repositories'}',
                        excludeSemantics: true,
                        child: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            const SmithIcon('git-branch', size: 16),
                            SizedBox(width: tokens.gap),
                            Text(
                              '${project.repositories.length}',
                              key: ValueKey(
                                'project-repository-count-${project.projectId}',
                              ),
                            ),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }

  List<Widget> _status({bool list = false}) => [
    if (registry.loading) const Text('Refreshing project metadata…'),
    if (registry.loaded && registry.projects.length >= maxProjects)
      const Text(
        'The project registry is full. Choose a project to edit or delete its metadata.',
      ),
    if (registry.revision == maxProjectRevision)
      const Text(
        'The project revision limit is reached. Further metadata changes are unavailable.',
      ),
    if (registry.busy || _saving) const Text('Saving project metadata…'),
    if (registry.error != null) Text(registry.error!),
    if (_localError != null) Text(_localError!),
    if (_validation && projectPromptError(_prompt.text) != null)
      Text(
        'Retained Prompt: ${projectPromptError(_prompt.text)} This field is not editable here. Cancel to reload saved metadata.',
        style: const TextStyle(color: SmithColors.danger),
      ),
    if (registry.uncertain) ...[
      const Text(
        'A project change has an unknown outcome. Further writes are blocked. Refresh observes metadata; Resolve queries the original receipt and current list. Neither retries the change.',
      ),
      Align(
        alignment: Alignment.centerLeft,
        child: TextButton(
          style: TextButton.styleFrom(minimumSize: const Size(48, 48)),
          key: const ValueKey('resolve-project-change'),
          onPressed: registry.busy || registry.loading || _saving
              ? null
              : _resolve,
          child: const Text('Resolve pending change'),
        ),
      ),
    ],
    if (_stale) ...[
      Text(
        _missing
            ? 'This project is no longer in the registry. Your edits are preserved; Save and Delete are blocked.'
            : 'The Smith instance or project metadata changed since this editor opened. Your edits are preserved; review the current instance and revision before saving.',
      ),
      if (!_missing && !_creationAcknowledged)
        Align(
          alignment: Alignment.centerLeft,
          child: TextButton(
            style: TextButton.styleFrom(minimumSize: const Size(48, 48)),
            key: const ValueKey('review-project-revision'),
            onPressed: registry.canMutate && !registry.loading && !_saving
                ? _reviewRevision
                : null,
            child: const Text('Review current revision'),
          ),
        ),
    ],
    if ((!registry.loaded && !registry.loading) ||
        registry.error != null ||
        registry.uncertain ||
        _stale ||
        _creationAcknowledged)
      Align(
        alignment: Alignment.centerLeft,
        child: _refreshButton(list: list),
      ),
  ];

  Widget _statusRegion({bool list = false}) => Semantics(
    liveRegion: true,
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: _status(list: list),
    ),
  );

  Widget _editorActions() => Wrap(
    key: const ValueKey('project-editor-actions'),
    alignment: WrapAlignment.end,
    spacing: 8,
    runSpacing: 8,
    crossAxisAlignment: WrapCrossAlignment.center,
    children: [
      TextButton(
        key: const ValueKey('cancel-project'),
        style: TextButton.styleFrom(minimumSize: const Size(48, 48)),
        onPressed: _saving ? null : _cancel,
        child: const Text('Cancel'),
      ),
      FilledButton(
        key: const ValueKey('save-project'),
        style: FilledButton.styleFrom(minimumSize: const Size(48, 48)),
        onPressed: _writeAllowed && _dirty ? _save : null,
        child: const Text('Save Changes'),
      ),
    ],
  );

  Widget _projectMenu() {
    final epoch = _editorEpoch;
    return PopupMenuButton<String>(
      key: const ValueKey('project-editor-menu'),
      tooltip: 'Project actions',
      enabled: _writeAllowed,
      icon: const SmithIcon('ellipsis-vertical', size: 20),
      itemBuilder: (_) => [
        PopupMenuItem(
          value: 'duplicate',
          enabled: registry.projects.length < maxProjects,
          height: 48,
          child: const Text('Duplicate', key: ValueKey('duplicate-project')),
        ),
        const PopupMenuItem(
          value: 'delete',
          height: 48,
          child: Text('Delete project', key: ValueKey('delete-project')),
        ),
      ],
      onSelected: (action) {
        if (!mounted ||
            epoch != _editorEpoch ||
            !_writeAllowed ||
            !TickerMode.valuesOf(context).enabled) {
          return;
        }
        if (action == 'duplicate') {
          unawaited(_duplicate());
        } else {
          unawaited(_delete());
        }
      },
    );
  }

  List<Widget> _editor() => [
    _pairedFields(
      TextField(
        key: const ValueKey('project-key'),
        controller: _key,
        enabled: _editorEnabled,
        onChanged: _changed,
        decoration: InputDecoration(
          labelText: 'ID',
          hintText: 'project-id',
          errorText: _validation ? _keyError : null,
          errorMaxLines: 5,
        ),
      ),
      TextField(
        key: const ValueKey('project-description'),
        controller: _description,
        enabled: _editorEnabled,
        onChanged: _changed,
        decoration: InputDecoration(
          labelText: 'Description',
          counterText: '${_description.text.runes.length}/60',
          errorText: _validation
              ? projectDescriptionError(_description.text)
              : null,
          errorMaxLines: 5,
        ),
      ),
      minimumWidth: 600,
      secondFlex: 2,
    ),
    const SizedBox(height: 16),

    Wrap(
      alignment: WrapAlignment.spaceBetween,
      crossAxisAlignment: WrapCrossAlignment.center,
      spacing: 8,
      runSpacing: 8,
      children: [
        _heading('Repositories'),
        TextButton(
          key: const ValueKey('add-project-repository'),
          focusNode: _addRepositoryFocus,
          style: TextButton.styleFrom(minimumSize: const Size(48, 48)),
          onPressed:
              !_editorEnabled || _repositories.length >= maxProjectRepositories
              ? null
              : () => setState(() {
                  final row = _RepositoryDraft();
                  _repositories.add(row);
                  WidgetsBinding.instance.addPostFrameCallback((_) {
                    if (mounted && _repositories.contains(row)) {
                      row.nameFocus.requestFocus();
                    }
                  });
                }),
          child: Text(
            'Add repository (${_repositories.length}/$maxProjectRepositories)',
          ),
        ),
      ],
    ),

    const SizedBox(height: 8),
    if (_repositories.isEmpty) const Text('No repositories'),
    for (var i = 0; i < _repositories.length; i++) _repositoryCard(i),
    const SizedBox(height: 16),
    _editorActions(),
  ];

  Widget _editorPane(bool compact) {
    final tokens = Theme.of(context).extension<SmithTokens>()!;
    return SingleChildScrollView(
      key: const ValueKey('project-editor-scroll'),
      controller: _editorScroll,
      padding: EdgeInsets.all(tokens.gutter),
      child: Align(
        alignment: Alignment.topLeft,
        child: ConstrainedBox(
          key: const ValueKey('project-editor-form'),
          constraints: BoxConstraints(maxWidth: tokens.readingWidth),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (compact || _editing)
                Wrap(
                  key: const ValueKey('project-editor-header'),
                  alignment: WrapAlignment.spaceBetween,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  spacing: tokens.gap,
                  runSpacing: tokens.gap,
                  children: [
                    if (compact)
                      TextButton(
                        key: const ValueKey('back-to-projects'),
                        style: TextButton.styleFrom(
                          minimumSize: const Size(48, 48),
                        ),
                        onPressed: () => setState(() => _editorOpen = false),
                        child: const Text('Back to projects'),
                      ),
                    if (_editing)
                      _heading(
                        _creating ? 'New project · not saved' : _original!.key,
                      ),
                    if (compact) _newProjectButton(),
                    if (_editing && !_creating) _projectMenu(),
                  ],
                ),
              if (_dirty) const Text('Unsaved changes — this view only'),
              _statusRegion(),
              SizedBox(height: tokens.gutter),
              if (!_editing) ...[
                _heading('No project selected'),
                SizedBox(height: tokens.gap),
                const Text(
                  'Choose a project or create one to configure its metadata and repositories. Saving configuration does not start a run.',
                ),
              ] else
                ..._editor(),
            ],
          ),
        ),
      ),
    );
  }

  Widget _retainedPane({required bool visible, required Widget child}) =>
      Offstage(
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
      final listWidth = (bounds.maxWidth / 4).clamp(256.0, 384.0);
      // Both slots stay mounted, with usable constraints even while offstage.
      return Stack(
        children: [
          Positioned(
            left: 0,
            top: 0,
            bottom: 0,
            width: compact ? bounds.maxWidth : listWidth,
            child: _retainedPane(
              visible: !compact || !_editorOpen,
              child: DecoratedBox(
                decoration: BoxDecoration(
                  border: compact
                      ? null
                      : const Border(
                          right: BorderSide(color: SmithColors.strokeSubtle),
                        ),
                ),
                child: _projectList(compact),
              ),
            ),
          ),
          Positioned.fill(
            left: compact ? 0 : listWidth,
            child: _retainedPane(
              visible: !compact || _editorOpen,
              child: _editorPane(compact),
            ),
          ),
        ],
      );
    },
  );

  @override
  void dispose() {
    registry.removeListener(_registryChanged);
    _key.dispose();
    _description.dispose();
    _prompt.dispose();

    for (final row in _repositories) {
      row.dispose();
    }
    _addRepositoryFocus.dispose();
    _listScroll.dispose();
    _editorScroll.dispose();
    super.dispose();
  }
}

/// Keep an absolute reading bookmark through temporary clamping/reflow/hiding.
/// Only actual scrolling changes the bookmark; layout corrections do not.
class _ProjectsScrollController extends ScrollController {
  double desiredOffset = 0;

  @override
  ScrollPosition createScrollPosition(
    ScrollPhysics physics,
    ScrollContext context,
    ScrollPosition? oldPosition,
  ) => _ProjectsScrollPosition(this, physics, context, oldPosition);
}

class _ProjectsScrollPosition extends ScrollPositionWithSingleContext {
  _ProjectsScrollPosition(
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

  final _ProjectsScrollController owner;

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
    // One-edge traversal reveal misses focus wrapping from the opposite edge.
    // Check both edges without moving an already-visible control; retain callers'
    // explicit alignment and the bookmark for genuine layout-only clamping.
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
    final overscroll = super.setPixels(newPixels);
    owner.desiredOffset = pixels;
    return overscroll;
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

class _EditorSubmission {
  const _EditorSubmission(
    this.epoch,
    this.instanceId,
    this.revision,
    this.projectId,
    this.name,
    this.repositories,
    this.knownIds, {
    required this.key,
    required this.description,
    required this.prompt,
    this.deleting = false,
  });
  final int epoch;
  final String instanceId;
  final int revision;
  final String? projectId;
  final String name;
  final String key;
  final String description;
  final String prompt;
  final List<List<String>> repositories;
  final Set<String> knownIds;
  final bool deleting;
}

bool _sameRepositories(List<List<String>> a, List<List<String>> b) =>
    a.length == b.length &&
    List.generate(
      a.length,
      (i) =>
          a[i].length == b[i].length &&
          List.generate(a[i].length, (j) => a[i][j] == b[i][j]).every((v) => v),
    ).every((v) => v);

const _repositoryFields = [
  'name',
  'url',
  'base',
  'work',
  'feature',
  'fix',
  'release',
  'hotfix',
];
const _repositoryLabels = [
  'ID (required)',
  'URL (required)',
  'Base branch (optional)',
  'Work branch (optional)',
  'Feature pattern (optional)',
  'Fix pattern (optional)',
  'Release pattern (optional)',
  'Hotfix pattern (optional)',
];
const _repositoryHints = [
  'Example: backend',
  'Example: git@example.invalid:team/backend.git',
  'Example: main',
  'Example: dev',
  'Example: feature/*',
  'Example: fix/*',
  'Example: release/*',
  'Example: hotfix/*',
];
List<String> _repositoryValues(ProjectRepository r) => [
  r.name ?? '',
  r.url,
  r.branch.base ?? '',
  r.branch.work ?? '',
  r.branch.types.feature ?? '',
  r.branch.types.fix ?? '',
  r.branch.types.release ?? '',
  r.branch.types.hotfix ?? '',
  r.readOnly.toString(),
  for (final value in [
    r.branch.types.feature,
    r.branch.types.fix,
    r.branch.types.release,
    r.branch.types.hotfix,
  ])
    (value != null).toString(),
];

/// Reject whole edits rather than silently filtering or renaming repository IDs.
class _RepositoryIdFormatter extends TextInputFormatter {
  static final _partial = RegExp(r'^[a-z0-9]+(?:-[a-z0-9]+)*-?$');
  TextEditingValue? _beforeComposition;

  @override
  TextEditingValue formatEditUpdate(
    TextEditingValue oldValue,
    TextEditingValue newValue,
  ) {
    if (!newValue.composing.isCollapsed) {
      _beforeComposition ??= oldValue.copyWith(composing: TextRange.empty);
      return newValue;
    }
    final previous = _beforeComposition ?? oldValue;
    _beforeComposition = null;
    final text = newValue.text;
    final match = _partial.matchAsPrefix(text);
    if (text.isEmpty ||
        (text.length <= maxProjectKeyLength && match?.end == text.length)) {
      return newValue;
    }
    // Legacy values stay exact on load, but explicit deletion can correct them.
    if (text.length < previous.text.length) {
      var prefix = 0;
      while (prefix < text.length && text[prefix] == previous.text[prefix]) {
        prefix++;
      }
      final removed = previous.text.length - text.length;
      if (previous.text.substring(prefix + removed) == text.substring(prefix)) {
        return newValue;
      }
    }
    return previous.copyWith(composing: TextRange.empty);
  }
}

/// Invalid legacy input remains raw text until explicit Save passes validation.
class _RepositoryDraft {
  _RepositoryDraft([ProjectRepository? repository])
    : fields = [
        for (final value
            in repository == null
                ? List<String>.filled(8, '')
                : _repositoryValues(repository).take(8))
          TextEditingController(text: value),
      ],
      readOnly = repository?.readOnly ?? true,
      expanded = repository == null,
      selectedTypes = [
        for (final value in [
          repository?.branch.types.feature,
          repository?.branch.types.fix,
          repository?.branch.types.release,
          repository?.branch.types.hotfix,
        ])
          value != null,
      ];
  final List<TextEditingController> fields;
  final nameFocus = FocusNode();
  final idFormatter = _RepositoryIdFormatter();
  bool expanded;
  bool readOnly;
  final List<bool> selectedTypes;
  List<String> get values => List.unmodifiable([
    ...fields.map((f) => f.text),
    readOnly.toString(),
    ...selectedTypes.map((v) => v.toString()),
  ]);
  String? optional(int i) => fields[i].text.isEmpty ? null : fields[i].text;
  String? error(int i) => switch (i) {
    0 => projectRepositoryIdError(optional(i)),
    1 => repositoryLocatorError(fields[i].text),
    2 => projectBranchNameError(optional(i)),
    3 => projectWorkBranchError(optional(i)),
    _ =>
      selectedTypes[i - 4] && fields[i].text.isEmpty
          ? 'Enter a branch pattern or deselect this type.'
          : projectBranchTypeError(optional(i)),
  };
  String? get firstError {
    for (var i = 0; i < fields.length; i++) {
      final issue = error(i);
      if (issue != null) return issue;
    }
    return null;
  }

  ProjectRepository toRepository() => ProjectRepository(
    name: optional(0),
    url: fields[1].text,
    readOnly: readOnly,
    branch: ProjectBranches(
      base: optional(2),
      work: optional(3),
      types: ProjectBranchTypes(
        feature: optional(4),
        fix: optional(5),
        release: optional(6),
        hotfix: optional(7),
      ),
    ),
  );
  void dispose() {
    for (final field in fields) {
      field.dispose();
    }
    nameFocus.dispose();
  }
}
