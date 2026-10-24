import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/projects.dart';

import '../../../client/test/projects_view_test.dart' as h;

void main() {
  const fields = [
    'name',
    'url',
    'base',
    'work',
    'feature',
    'fix',
    'release',
    'hotfix',
  ];
  const changes = [
    'Named',
    'https://a/changed',
    'main',
    'dev',
    'feature/*',
    'fix/*',
    'release/*',
    'hotfix/*',
  ];
  String key(int i) =>
      i == 1 ? 'project-repository-0' : 'project-repository-0-${fields[i]}';
  for (var i = 0; i < fields.length; i++) {
    testWidgets(
      'independent ${fields[i]}-only dirty restart and newer-input recovery',
      (tester) async {
        final api = h.ProjectsUiApi()..projectRevision = 2;
        api.records[h.alphaProject] = api.record(h.alphaProject, 'Alpha', [
          ProjectRepository(url: 'https://a/original'),
        ]);
        final c = await h.mountProjects(tester, api);
        await h.chooseProject(tester, h.alphaProject);
        if (i >= 4) await h.projectTap(tester, 'project-repository-0-types');
        await tester.enterText(h.projectKey(key(i)), changes[i]);
        await tester.pumpAndSettle();
        expect(
          tester.widget<FilledButton>(h.projectKey('save-project')).onPressed,
          isNotNull,
        );
        api.instance = h.restartedProjectInstance;
        await c.refresh();
        await tester.pumpAndSettle();
        expect(h.projectField(tester, key(i)).controller!.text, changes[i]);
        expect(
          tester.widget<FilledButton>(h.projectKey('save-project')).onPressed,
          isNull,
        );
        expect(api.writes, isEmpty);
        await h.acceptProjectOriginReview(tester);
        api.loseAcknowledgement = true;
        await h.projectTap(tester, 'save-project');
        expect(c.uncertain, isTrue);
        final newer = i == 1
            ? 'https://a/newer'
            : i >= 4
            ? 'newer/*'
            : 'Newer';
        await tester.enterText(h.projectKey(key(i)), newer);
        await h.projectTap(tester, 'resolve-project-change');
        expect(c.uncertain, isFalse);
        expect(api.writes, ['project_update_v2']);
        expect(h.projectField(tester, key(i)).controller!.text, newer);
        expect(
          tester.widget<FilledButton>(h.projectKey('save-project')).onPressed,
          isNull,
        );
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox());
      },
    );
  }
  testWidgets(
    'independent null metadata survives unrelated save and dirty delete recovery',
    (tester) async {
      final api = h.ProjectsUiApi()..projectRevision = 2;
      api.records[h.alphaProject] = api.record(h.alphaProject, 'Alpha', [
        ProjectRepository(url: 'https://a/original'),
      ]);
      final c = await h.mountProjects(tester, api);
      await h.chooseProject(tester, h.alphaProject);
      await tester.enterText(h.projectKey('project-name'), 'Renamed only');
      await h.projectTap(tester, 'save-project');
      expect(api.records[h.alphaProject]!['repositories'], [
        ProjectRepository(url: 'https://a/original').toJson(),
      ]);
      await tester.enterText(
        h.projectKey('project-repository-0-base'),
        'dirty',
      );
      api.loseAcknowledgement = true;
      await h.projectTap(tester, 'delete-project');
      await tester.tap(find.widgetWithText(TextButton, 'Delete metadata'));
      await tester.pumpAndSettle();
      expect(c.uncertain, isTrue);
      api.instance = h.restartedProjectInstance;
      await h.projectTap(tester, 'resolve-project-change');
      expect(c.uncertain, isFalse);
      expect(api.writes, ['project_update_v2', 'project_delete_v2']);
      expect(h.projectKey('project-name'), findsNothing);
      expect(api.records.keys, [h.betaProject]);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );
}
