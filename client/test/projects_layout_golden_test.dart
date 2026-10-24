// Projects interior fixtures only; no shell, live service or repository access.
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/projects.dart';
import 'package:smith_client/projects_controller.dart';
import 'package:smith_client/ui/projects_view.dart';
import 'package:smith_client/ui/smith_theme.dart';

import 'projects_view_test.dart'
    show
        ProjectsUiApi,
        alphaProject,
        betaProject,
        chooseProject,
        expandProjectRepository,
        projectKey;

const _capture = ValueKey('projects-layout-capture');

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    final material = FontLoader('MaterialIcons')
      ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'));
    await material.load();
    // Match the existing golden loader: unambiguous Regular registration, not
    // evidence of production 400/500/600 font-weight selection fidelity.
    final font = FontLoader(SmithTheme.defaultFontFamily)
      ..addFont(
        rootBundle.load(
          'assets/fonts/jetbrains-mono/JetBrainsMono-Regular.ttf',
        ),
      );
    await font.load();
  });

  for (final (name, size, scale, editor, repositories) in [
    ('wide-1536x1024', const Size(1536, 1024), 1.0, true, false),
    ('wide-collapsed-1536x1024', const Size(1536, 1024), 1.0, true, false),
    ('wide-read-only-1536x1024', const Size(1536, 1024), 1.0, true, true),
    ('compact-list-400x400', const Size(400, 400), 1.0, false, false),
    ('compact-editor-400x400', const Size(400, 400), 1.0, true, false),
    ('compact-list-400x400-text2', const Size(400, 400), 2.0, false, false),
    ('compact-editor-400x400-text2', const Size(400, 400), 2.0, true, false),
    ('compact-repositories-400x400', const Size(400, 400), 1.0, true, true),
    (
      'compact-repositories-400x400-text2',
      const Size(400, 400),
      2.0,
      true,
      true,
    ),
    ('ultrawide-4200x2400', const Size(4200, 2400), 1.0, true, false),
  ]) {
    testWidgets('Projects layout bundled-font golden $name', (tester) async {
      tester.view.physicalSize = size;
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final api = ProjectsUiApi();
      api.records[alphaProject] = api.record(
        alphaProject,
        'Workspace tools',
        [
          ProjectRepository(
            readOnly: name.contains('read-only'),
            name: 'backend',
            url: 'git@example.invalid:team/backend.git',
            branch: ProjectBranches(
              base: 'main',
              work: 'main',
              types: ProjectBranchTypes(
                feature: 'feature/*',
                fix: 'fix/*',
                release: 'release/*',
                hotfix: 'hotfix/*',
              ),
            ),
          ),
          ProjectRepository(url: 'https://example.invalid/team/client'),
        ],
        'workspace-tools',
        'Shared development workspace',
        'Keep changes scoped to this project.\nPreserve existing configuration.',
      );
      api.records[betaProject] = api.record(betaProject, 'Release notes');
      const gamma = 'cccccccccccccccccccccccccccccccc';
      api.records[gamma] = api.record(gamma, 'Shared libraries', [
        ProjectRepository(url: 'https://example.invalid/team/shared'),
      ]);
      final controller = ProjectsController(api);
      addTearDown(controller.dispose);
      await tester.pumpWidget(
        MaterialApp(
          debugShowCheckedModeBanner: false,
          theme: SmithTheme.dark(),
          builder: (context, child) => MediaQuery(
            data: MediaQuery.of(context)
                .copyWith(textScaler: TextScaler.linear(scale)),
            child: child!,
          ),
          home: RepaintBoundary(
            key: _capture,
            child: Scaffold(body: ProjectsView(controller: controller)),
          ),
        ),
      );
      await tester.pumpAndSettle();
      if (editor) {
        await chooseProject(tester, alphaProject);
        if ((size.width > 880 && !name.contains('collapsed')) || repositories) {
          await expandProjectRepository(tester, 0);
        }
        final scroll = tester
            .widget<SingleChildScrollView>(projectKey('project-editor-scroll'))
            .controller!;
        scroll.jumpTo(0);
        if (repositories) {
          await tester.ensureVisible(projectKey('expand-project-repository-0'));
        }
      }
      FocusManager.instance.primaryFocus?.unfocus();
      await tester.pumpAndSettle();
      expect(api.projectCalls, ['projects_list_v3']);
      expect(api.writes, isEmpty);
      expect(tester.takeException(), isNull);
      await expectLater(
        find.byKey(_capture),
        matchesGoldenFile('goldens/projects-layout/$name.png'),
      );
      await tester.pumpWidget(const SizedBox());
    });
  }
}
