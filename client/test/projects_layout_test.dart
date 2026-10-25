import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/projects.dart';
import 'package:smith_client/ui/projects_view.dart';

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
        projectTap;

ProjectRepository _repository() => ProjectRepository(
  readOnly: false,
  name: 'backend',
  url: 'git@example.invalid:team/backend.git',
  branch: ProjectBranches(
    base: 'main',
    work: 'work/next/*',
    types: ProjectBranchTypes(
      feature: 'custom/features/*',
      fix: 'custom/fixes/*',
      release: 'custom/releases/*',
      hotfix: 'urgent/*',
    ),
  ),
);

ProjectsUiApi _api() {
  final api = ProjectsUiApi();
  api.records[alphaProject] = api.record(
    alphaProject,
    'alpha',
    [_repository()],
    'alpha',
    'Saved description',
  );
  return api;
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(loadProjectTestFonts);

  for (final (size, scale) in [
    (const Size(360, 640), 1.0),
    (const Size(400, 400), 2.0),
    (const Size(4200, 2400), 1.0),
  ]) {
    testWidgets('list and editor remain reachable at $size scale $scale', (
      tester,
    ) async {
      final api = _api();
      await mountProjects(tester, api, size: size, scale: scale);
      await chooseProject(tester, alphaProject);
      expect(projectKey('project-key'), findsOneWidget);
      expect(projectKey('project-description'), findsOneWidget);
      expect(projectKey('add-project-repository'), findsOneWidget);
      await expandProjectRepository(tester, 0);
      expect(projectKey('project-repository-0-name'), findsOneWidget);
      expect(projectKey('project-repository-0'), findsOneWidget);
      await tester.enterText(projectKey('project-key'), 'layout-draft');
      await tester.pumpAndSettle();
      expect(tester.getRect(projectKey('project-key')).width, greaterThan(0));
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    });
  }

  testWidgets('compact Back retains the editor state and draft', (
    tester,
  ) async {
    final api = _api();
    await mountProjects(tester, api, size: const Size(400, 400));
    await chooseProject(tester, alphaProject);
    final state = tester.state(find.byType(ProjectsView));
    await tester.enterText(projectKey('project-key'), 'retained-layout-draft');
    await tester.pumpAndSettle();
    await projectTap(tester, 'back-to-projects');
    expect(projectKey('project-key'), findsNothing);
    await projectTap(tester, 'project-$alphaProject');
    expect(tester.state(find.byType(ProjectsView)), same(state));
    expect(
      projectField(tester, 'project-key').controller!.text,
      'retained-layout-draft',
    );
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('resizing keeps the same editor and repository rows', (
    tester,
  ) async {
    final api = _api();
    await mountProjects(tester, api);
    await chooseProject(tester, alphaProject);
    final state = tester.state(find.byType(ProjectsView));
    await tester.enterText(projectKey('project-key'), 'resize-draft');
    await expandProjectRepository(tester, 0);
    final idController = projectField(
      tester,
      'project-repository-0-name',
    ).controller;
    tester.view.physicalSize = const Size(400, 400);
    await tester.pumpAndSettle();
    expect(tester.state(find.byType(ProjectsView)), same(state));
    expect(projectKey('back-to-projects'), findsOneWidget);
    await projectTap(tester, 'back-to-projects');
    await projectTap(tester, 'project-$alphaProject');
    expect(
      projectField(tester, 'project-key').controller!.text,
      'resize-draft',
    );
    expect(
      projectField(tester, 'project-repository-0-name').controller,
      same(idController),
    );
    tester.view.physicalSize = const Size(1280, 900);
    await tester.pumpAndSettle();
    expect(tester.state(find.byType(ProjectsView)), same(state));
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
    'keyboard focus reaches Save while Enter in a field never saves',
    (tester) async {
      final api = _api();
      await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      final key = projectKey('project-key');
      final editable = find.descendant(
        of: key,
        matching: find.byType(EditableText),
      );
      Focus.of(tester.element(editable)).requestFocus();
      await tester.pump();
      await tester.enterText(key, 'keyboard-draft');
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(api.writes, isEmpty);
      final saveLabel = find.text('Save Changes');
      Focus.of(tester.element(saveLabel)).requestFocus();
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(api.writes, ['project_update_v3']);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets('repository row layout stays inside the editor at both widths', (
    tester,
  ) async {
    final api = _api();
    for (final size in [const Size(400, 400), const Size(1536, 1024)]) {
      await mountProjects(tester, api, size: size);
      await chooseProject(tester, alphaProject);
      await expandProjectRepository(tester, 0);
      final editor = tester.getRect(projectKey('project-editor-scroll'));
      final id = tester.getRect(projectKey('project-repository-0-name'));
      final url = tester.getRect(projectKey('project-repository-0'));
      expect(id.left, greaterThanOrEqualTo(editor.left - 1));
      expect(id.right, lessThanOrEqualTo(editor.right + 1));
      expect(url.left, greaterThanOrEqualTo(editor.left - 1));
      expect(url.right, lessThanOrEqualTo(editor.right + 1));
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    }
  });
}
