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

void main() {
  for (final dirty in [false, true]) {
    testWidgets(
      'confirmed delete after restart clears the ${dirty ? 'dirty' : 'clean'} original selection without replay',
      (tester) async {
        final api = ProjectsUiApi()
          ..projectRevision = 2
          ..loseAcknowledgement = true;
        final controller = await mountProjects(tester, api);
        await chooseProject(tester, alphaProject);
        if (dirty) {
          await tester.enterText(
            projectKey('project-name'),
            'Unsaved before deletion',
          );
        }
        await projectTap(tester, 'delete-project');
        await tester.tap(find.text('Delete metadata'));
        await tester.pumpAndSettle();
        expect(controller.uncertain, isTrue);
        expect(api.records.keys, [betaProject]);
        api.instance = 'fedcba9876543210fedcba9876543210';
        await projectTap(tester, 'resolve-project-change');
        expect(controller.uncertain, isFalse);
        expect(controller.error, isNull);
        expect(controller.projects.map((p) => p.projectId), [betaProject]);
        expect(api.writes, ['project_delete']);
        expect(find.text('No project selected'), findsOneWidget);
        expect(projectKey('project-name'), findsNothing);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox());
      },
    );
  }
}
