import 'package:flutter/material.dart';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/projects.dart';

import 'projects_view_test.dart'
    show
        ProjectsUiApi,
        alphaProject,
        chooseProject,
        expandProjectRepository,
        loadProjectTestFonts,
        mountProjects,
        projectField,
        projectKey,
        restartedProjectInstance;

const _prompt = '  exact project context\n\twith retained whitespace\n';
const _compatibilityName = 'Retained compatibility name';

ProjectRepository _richRepository() => ProjectRepository(
  name: 'backend',
  url: 'git@example.invalid:team/backend.git',
  readOnly: false,
  branch: ProjectBranches(
    base: 'main',
    work: 'work/next/*',
    types: ProjectBranchTypes(
      feature: 'custom/features/*',
      release: 'custom/releases/*',
      hotfix: 'urgent/*',
    ),
  ),
);

ProjectsUiApi _richApi() {
  final api = ProjectsUiApi();
  api.records[alphaProject] = api.record(
    alphaProject,
    _compatibilityName,
    [_richRepository()],
    'alpha',
    'Saved description',
    _prompt,
  );
  return api;
}

Future<void> _tap(WidgetTester tester, String key) async {
  await tester.ensureVisible(projectKey(key));
  await tester.pumpAndSettle();
  await tester.tap(projectKey(key));
  await tester.pumpAndSettle();
}

void _expectHiddenControls() {
  expect(
    find.byKey(const ValueKey('project-prompt'), skipOffstage: false),
    findsNothing,
  );
  for (final field in ['base', 'work', 'feature', 'fix', 'release', 'hotfix']) {
    expect(
      find.byKey(ValueKey('project-repository-0-$field'), skipOffstage: false),
      findsNothing,
    );
    expect(
      find.byKey(
        ValueKey('project-repository-0-$field-selected'),
        skipOffstage: false,
      ),
      findsNothing,
    );
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(loadProjectTestFonts);

  testWidgets(
    'headers have Projects and Create with no routine or hidden controls',
    (tester) async {
      final api = _richApi();
      await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      await expandProjectRepository(tester, 0);
      for (final heading in [
        'Edit Project',
        'Basic Information',
        'Repository Configuration',
        'Search projects',
        'Refresh projects',
        'Prompt',
        'Allowed branch types',
      ]) {
        expect(find.text(heading), findsNothing);
      }
      expect(projectKey('new-project'), findsOneWidget);
      expect(find.text('New project'), findsNothing);
      expect(find.text('Create'), findsOneWidget);
      expect(find.text('Projects'), findsOneWidget);
      expect(
        tester.getRect(find.text('Projects')).right,
        lessThan(tester.getRect(projectKey('new-project')).left),
      );
      expect(projectKey('search-projects'), findsNothing);
      expect(find.text('Refresh'), findsNothing);
      expect(
        projectField(tester, 'project-repository-0-name').decoration!.labelText,
        'ID (required)',
      );
      expect(projectKey('project-editor-menu'), findsOneWidget);
      _expectHiddenControls();
      await _tap(tester, 'project-repository-0-read-only');
      expect(
        tester
            .widget<SwitchListTile>(
              projectKey('project-repository-0-read-only'),
            )
            .value,
        isTrue,
      );
      _expectHiddenControls();
      expect(api.projectCalls, ['projects_list_v3']);
      expect(api.writes, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'Save and Duplicate retain exact Prompt, hidden Name and rich branches; New stays blank',
    (tester) async {
      final api = _richApi();
      await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      await tester.enterText(
        projectKey('project-description'),
        'Edited description',
      );
      await expandProjectRepository(tester, 0);
      await _tap(tester, 'project-repository-0-read-only');
      await _tap(tester, 'project-repository-0-read-only');
      await _tap(tester, 'save-project');
      final saved = api.records[alphaProject]!;
      expect(saved['name'], _compatibilityName);
      expect(saved['prompt'], _prompt);
      expect(saved['repositories'], [_richRepository().toJson()]);

      await _tap(tester, 'project-editor-menu');
      await _tap(tester, 'duplicate-project');
      expect(projectField(tester, 'project-key').controller!.text, isEmpty);
      expect(
        projectField(tester, 'project-description').controller!.text,
        'Edited description',
      );
      _expectHiddenControls();
      await tester.enterText(projectKey('project-key'), 'alpha-copy');
      await _tap(tester, 'save-project');
      final copy = api.records.values.singleWhere(
        (record) => record['key'] == 'alpha-copy',
      );
      expect(copy['project_id'], isNot(alphaProject));
      expect(copy['name'], _compatibilityName);
      expect(copy['prompt'], _prompt);
      expect(copy['repositories'], [_richRepository().toJson()]);

      await _tap(tester, 'new-project');
      expect(projectField(tester, 'project-key').controller!.text, isEmpty);
      expect(
        projectField(tester, 'project-description').controller!.text,
        isEmpty,
      );
      expect(find.text('No repositories'), findsOneWidget);
      await tester.enterText(projectKey('project-key'), 'blank-project');
      await _tap(tester, 'save-project');
      final blank = api.records.values.singleWhere(
        (record) => record['key'] == 'blank-project',
      );
      expect(blank['prompt'], isEmpty);
      expect(blank['repositories'], isEmpty);
      expect(api.writes, [
        'project_update_v3',
        'project_create_v3',
        'project_create_v3',
      ]);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'repository IDs require explicit legacy correction and reject whole invalid edits, including IME commits',
    (tester) async {
      final api = ProjectsUiApi();
      api.records[alphaProject] = api.record(alphaProject, 'alpha', [
        ProjectRepository(url: 'https://example.invalid/team/client'),
        ProjectRepository(
          name: 'Legacy Repo',
          url: 'https://example.invalid/team/server',
        ),
      ]);
      await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      await tester.enterText(projectKey('project-description'), 'Make dirty');
      await _tap(tester, 'save-project');
      expect(api.writes, isEmpty);
      expect(
        projectField(tester, 'project-repository-1-name').controller!.text,
        'Legacy Repo',
      );
      await _tap(tester, 'expand-project-repository-0');
      expect(find.text(projectRepositoryIdError(null)!), findsOneWidget);
      await expandProjectRepository(tester, 0);
      await tester.enterText(
        projectKey('project-repository-1-name'),
        'Legacy Rep',
      );
      expect(
        projectField(tester, 'project-repository-1-name').controller!.text,
        'Legacy Rep',
      );
      await tester.enterText(
        projectKey('project-repository-1-name'),
        'backend',
      );
      await tester.enterText(
        projectKey('project-repository-0-name'),
        'backend',
      );
      await _tap(tester, 'save-project');
      expect(find.text('Repository IDs must be distinct.'), findsWidgets);
      expect(api.writes, isEmpty);

      final id = projectField(tester, 'project-repository-0-name').controller!;
      await tester.enterText(
        projectKey('project-repository-0-name'),
        'frontend',
      );
      for (final invalid in [
        'Upper',
        'bad_id',
        'bad id',
        'bad--id',
        '-bad',
        'bad\n',
        'café',
        'a' * 41,
      ]) {
        await tester.enterText(
          projectKey('project-repository-0-name'),
          invalid,
        );
        expect(id.text, 'frontend', reason: 'Reject the entire edit: $invalid');
      }
      await tester.enterText(
        projectKey('project-repository-0-name'),
        'frontend-',
      );
      expect(id.text, 'frontend-');
      await _tap(tester, 'save-project');
      expect(
        projectField(tester, 'project-repository-0-name').decoration!.errorText,
        isNotNull,
      );
      expect(api.writes, isEmpty);
      await tester.enterText(projectKey('project-repository-0-name'), 'a' * 40);
      expect(id.text.length, 40);
      await tester.enterText(projectKey('project-repository-0-name'), '');
      expect(id.text, isEmpty);
      await tester.enterText(
        projectKey('project-repository-0-name'),
        'frontend',
      );
      tester.testTextInput.updateEditingValue(
        const TextEditingValue(
          text: '候',
          selection: TextSelection.collapsed(offset: 1),
          composing: TextRange(start: 0, end: 1),
        ),
      );
      await tester.pump();
      expect(id.text, '候');
      tester.testTextInput.updateEditingValue(
        const TextEditingValue(
          text: '候',
          selection: TextSelection.collapsed(offset: 1),
        ),
      );
      await tester.pump();
      expect(id.text, 'frontend');
      await tester.enterText(projectKey('project-repository-0-name'), 'client');
      await _tap(tester, 'save-project');
      expect(
        (api.records[alphaProject]!['repositories'] as List).map(
          (row) => row['name'],
        ),
        ['client', 'backend'],
      );
      expect(api.writes, ['project_update_v3']);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'compact enlarged header retains Back, New, overflow and blocked missing drafts',
    (tester) async {
      final api = _richApi();
      final controller = await mountProjects(
        tester,
        api,
        size: const Size(400, 400),
        scale: 2,
      );
      await chooseProject(tester, alphaProject);
      final header = projectKey('project-editor-header');
      for (final key in [
        'back-to-projects',
        'new-project',
        'project-editor-menu',
      ]) {
        expect(
          find.descendant(of: header, matching: projectKey(key)),
          findsOneWidget,
        );
        expect(
          tester.getSize(projectKey(key)).height,
          greaterThanOrEqualTo(48),
        );
      }
      expect(projectKey('new-project'), findsOneWidget);
      await tester.enterText(
        projectKey('project-description'),
        'Keep this draft',
      );
      final field = projectField(tester, 'project-description').controller!;
      final focus = tester
          .widget<EditableText>(
            find.descendant(
              of: projectKey('project-description'),
              matching: find.byType(EditableText),
            ),
          )
          .focusNode;
      await _tap(tester, 'new-project');
      await tester.tap(find.text('Keep editing'));
      await tester.pumpAndSettle();
      expect(
        projectField(tester, 'project-description').controller,
        same(field),
      );
      await _tap(tester, 'back-to-projects');
      expect(focus.hasFocus, isFalse);
      expect(focus.canRequestFocus, isFalse);
      api.records.remove(alphaProject);
      api.projectRevision++;
      await controller.refresh();
      await tester.pumpAndSettle();
      await _tap(tester, 'resume-project-editor');
      expect(
        projectField(tester, 'project-description').controller,
        same(field),
      );
      expect(field.text, 'Keep this draft');
      expect(
        tester.widget<FilledButton>(projectKey('save-project')).onPressed,
        isNull,
      );
      expect(projectKey('review-project-revision'), findsNothing);
      expect(projectKey('refresh-projects'), findsOneWidget);
      expect(api.writes, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'contextual Refresh recovers unavailable and cached drafts; restart review includes hidden values',
    (tester) async {
      final api = _richApi()..listFailure = true;
      final controller = await mountProjects(
        tester,
        api,
        size: const Size(400, 400),
      );
      expect(projectKey('refresh-projects-list'), findsOneWidget);
      expect(
        tester.widget<OutlinedButton>(projectKey('new-project')).onPressed,
        isNull,
      );
      api.listFailure = false;
      await _tap(tester, 'refresh-projects-list');
      expect(find.text('Refresh'), findsNothing);
      await chooseProject(tester, alphaProject);
      await tester.enterText(projectKey('project-description'), 'Newer draft');
      final field = projectField(tester, 'project-description').controller!;
      api.listFailure = true;
      await controller.refresh();
      await tester.pumpAndSettle();
      expect(projectKey('refresh-projects'), findsOneWidget);
      expect(field.text, 'Newer draft');
      api.listFailure = false;
      await _tap(tester, 'refresh-projects');
      expect(find.text('Refresh'), findsNothing);
      api.instance = restartedProjectInstance;
      await controller.refresh();
      await tester.pumpAndSettle();
      expect(projectKey('refresh-projects'), findsOneWidget);
      expect(
        tester.widget<FilledButton>(projectKey('save-project')).onPressed,
        isNull,
      );
      await _tap(tester, 'review-project-revision');
      final dialog = find.byType(AlertDialog);
      for (final text in [
        'Prompt: $_prompt',
        'Base branch (optional): main',
        'Work branch (optional): work/next/*',
        'Feature pattern (optional): custom/features/*',
        'Fix pattern (optional): Not configured',
        'Release pattern (optional): custom/releases/*',
        'Hotfix pattern (optional): urgent/*',
        'Read Only: No',
      ]) {
        expect(
          find.descendant(of: dialog, matching: find.text(text)),
          findsNWidgets(2),
        );
      }
      await tester.tap(find.text('Keep blocked'));
      await tester.pumpAndSettle();
      expect(
        tester.widget<FilledButton>(projectKey('save-project')).onPressed,
        isNull,
      );
      await _tap(tester, 'review-project-revision');
      await tester.tap(find.text('Keep edits at this revision'));
      await tester.pumpAndSettle();
      expect(find.text('Refresh'), findsNothing);
      expect(
        projectField(tester, 'project-description').controller,
        same(field),
      );
      await _tap(tester, 'save-project');
      expect(api.records[alphaProject]!['name'], _compatibilityName);
      expect(api.records[alphaProject]!['prompt'], _prompt);
      expect(api.records[alphaProject]!['repositories'], [
        _richRepository().toJson(),
      ]);
      expect(api.writes, ['project_update_v3']);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'uncertain create stays query-only and acknowledged missing target retains contextual recovery',
    (tester) async {
      final api = ProjectsUiApi(empty: true)..loseAcknowledgement = true;
      final controller = await mountProjects(
        tester,
        api,
        size: const Size(400, 400),
      );
      await _tap(tester, 'new-project');
      await tester.enterText(projectKey('project-key'), 'created');
      await _tap(tester, 'save-project');
      expect(controller.uncertain, isTrue);
      final receipt = Map<String, dynamic>.of(api.receipts.values.single);
      final requestId = receipt['request_id'];
      await tester.enterText(projectKey('project-key'), 'newer-key');
      await tester.enterText(
        projectKey('project-description'),
        'Newer description',
      );
      final beforeRefresh = api.projectCalls.length;
      await _tap(tester, 'refresh-projects');
      expect(api.projectCalls.sublist(beforeRefresh), ['projects_list_v3']);
      expect(controller.uncertain, isTrue);
      api.receipts.clear();
      api.missingReceiptCode = 'REQUEST_EXPIRED';
      final beforeResolve = api.projectCalls.length;
      await _tap(tester, 'resolve-project-change');
      expect(api.projectCalls.sublist(beforeResolve), [
        'project_receipt',
        'projects_list_v3',
      ]);
      expect(
        api.arguments.where((args) => args['request_id'] == requestId).length,
        2,
      );
      expect(controller.uncertain, isTrue);
      await _tap(tester, 'back-to-projects');
      expect(
        tester.widget<OutlinedButton>(projectKey('new-project')).onPressed,
        isNull,
      );
      await _tap(tester, 'resume-project-editor');
      expect(projectField(tester, 'project-key').controller!.text, 'newer-key');

      api.receipts[requestId as String] = receipt;
      api.records.clear();
      api.projectRevision++;
      await _tap(tester, 'resolve-project-change');
      expect(controller.uncertain, isFalse);
      expect(projectKey('refresh-projects'), findsOneWidget);
      expect(projectKey('review-project-revision'), findsNothing);
      expect(
        tester.widget<FilledButton>(projectKey('save-project')).onPressed,
        isNull,
      );
      await tester.enterText(
        projectKey('project-description'),
        'Still retained',
      );
      await _tap(tester, 'refresh-projects');
      await _tap(tester, 'back-to-projects');
      await _tap(tester, 'resume-project-editor');
      expect(projectField(tester, 'project-key').controller!.text, 'newer-key');
      expect(
        projectField(tester, 'project-description').controller!.text,
        'Still retained',
      );
      expect(
        tester.widget<FilledButton>(projectKey('save-project')).onPressed,
        isNull,
      );
      expect(api.writes, ['project_create_v3']);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );
}
