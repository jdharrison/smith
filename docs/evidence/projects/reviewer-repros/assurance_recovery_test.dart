import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'projects_view_test.dart'
    show
        ProjectsUiApi,
        alphaProject,
        betaProject,
        chooseProject,
        mountProjects,
        projectKey,
        projectTap;

const restartedInstance = 'fedcba9876543210fedcba9876543210';

void main() {
  testWidgets(
    'a pre-restart dirty editor must not dispatch into a refreshed instance without review',
    (tester) async {
      final api = ProjectsUiApi()..projectRevision = 2;
      final controller = await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      await tester.enterText(projectKey('project-name'), 'Local edits');
      api.instance = restartedInstance;
      await controller.refresh();
      await tester.pumpAndSettle();
      await projectTap(tester, 'save-project');
      expect(
        api.writes,
        isEmpty,
        reason: 'The old editor must be explicitly reviewed, not silently rebound to a new service instance.',
      );
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'an old delete confirmation must not dispatch into a refreshed instance',
    (tester) async {
      final api = ProjectsUiApi()..projectRevision = 2;
      final controller = await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      await projectTap(tester, 'delete-project');
      api.instance = restartedInstance;
      await controller.refresh();
      await tester.pumpAndSettle();
      await tester.tap(find.text('Delete metadata'));
      await tester.pumpAndSettle();
      expect(
        api.writes,
        isEmpty,
        reason: 'The original confirmation must not adopt a service instance that was observed only after the dialog opened.',
      );
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'unknown original receipt must not disable all authoritative list observation',
    (tester) async {
      final api = ProjectsUiApi()
        ..projectRevision = 2
        ..loseAcknowledgement = true;
      final controller = await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      await tester.enterText(
        projectKey('project-name'),
        'Acknowledgement lost',
      );
      await projectTap(tester, 'save-project');
      expect(controller.uncertain, isTrue);
      api.receipts.clear();
      api.records[betaProject] = api.record(
        betaProject,
        'Changed in another window',
      );
      api.projectRevision++;
      await projectTap(tester, 'resolve-project-change');
      expect(controller.uncertain, isTrue);
      expect(controller.canMutate, isFalse);
      expect(api.writes, ['project_update']);
      expect(
        api.projectCalls.where((op) => op == 'projects_list').length > 1 ||
            tester
                    .widget<TextButton>(projectKey('refresh-projects'))
                    .onPressed !=
                null,
        isTrue,
        reason: 'Receipt failure must still allow query-only list refresh while retaining the original uncertainty.',
      );
      await tester.pumpWidget(const SizedBox());
    },
  );
}
