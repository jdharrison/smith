import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/projects.dart';

import 'package:smith_client/ui/smith_icon.dart';

import 'projects_view_test.dart'
    show
        ProjectsUiApi,
        alphaProject,
        betaProject,
        chooseProject,
        expandProjectRepository,
        selectProjectType,
        loadProjectTestFonts,
        mountProjects,
        projectField,
        projectKey,
        projectRefresh,
        projectTap;

final _repository = ProjectRepository(
  readOnly: false,
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
);

ScrollController _scroll(WidgetTester tester, String key) => tester
    .widget<SingleChildScrollView>(
      find.byKey(ValueKey(key), skipOffstage: false),
    )
    .controller!;

TextField _retainedField(WidgetTester tester, String key) =>
    tester.widget<TextField>(find.byKey(ValueKey(key), skipOffstage: false));

EditableText _editable(WidgetTester tester, String key) =>
    tester.widget<EditableText>(
      find.descendant(of: projectKey(key), matching: find.byType(EditableText)),
    );

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(loadProjectTestFonts);
  testWidgets(
    'wide interior composition uses only real fields and derived cards',
    (tester) async {
      final api = ProjectsUiApi();
      api.records[alphaProject] = api.record(alphaProject, 'alpha', [
        _repository,
      ]);
      const gamma = 'cccccccccccccccccccccccccccccccc';
      api.records[gamma] = api.record(gamma, 'gamma', [
        _repository,
        ProjectRepository(url: 'https://example.invalid/team/client'),
        ProjectRepository(url: 'https://example.invalid/team/tools'),
      ]);
      await mountProjects(tester, api, size: const Size(1536, 1024));
      await chooseProject(tester, alphaProject);

      final list = tester.getRect(projectKey('project-list'));
      expect(list.width, inInclusiveRange(256, 384));
      final form = tester.getRect(projectKey('project-editor-form'));
      expect(form.width, 960);
      expect(form.left, list.right + 16);
      expect(form.top, 16);
      expect(projectRefresh(), findsOneWidget);
      expect(
        tester.getRect(projectKey('new-project')).right,
        lessThan(list.right),
      );
      expect(
        tester.getRect(projectKey('search-projects')).top,
        greaterThan(tester.getRect(find.text('Projects')).bottom),
      );
      for (final (id, count) in [
        (alphaProject, 1),
        (betaProject, 0),
        (gamma, 3),
      ]) {
        final card = projectKey('project-$id');
        final glyphs = tester
            .widgetList<SmithIcon>(
              find.descendant(of: card, matching: find.byType(SmithIcon)),
            )
            .map((icon) => icon.name);
        expect(glyphs, ['layers', 'git-branch']);
        final counter = projectKey('project-repository-count-$id');
        expect(tester.widget<Text>(counter).data, '$count');
        expect(
          tester.getRect(counter).right,
          closeTo(tester.getRect(card).right - 9, 2),
        );
        expect(
          tester
              .widget<Semantics>(
                find.descendant(
                  of: card,
                  matching: find.byWidgetPredicate(
                    (w) => w is Semantics && w.properties.selected != null,
                  ),
                ),
              )
              .properties
              .selected,
          id == alphaProject,
        );
      }
      final selectedCard = projectKey('project-$alphaProject');
      final selectedName = find.descendant(
        of: selectedCard,
        matching: find.text('alpha'),
      );
      final cardFocus = Focus.of(tester.element(selectedName));
      cardFocus.requestFocus();
      await tester.pumpAndSettle();
      expect(cardFocus.hasFocus, isTrue);
      expect(
        tester.widget<TextButton>(selectedCard).style!.side!.resolve({
          WidgetState.focused,
        })!.color,
        Theme.of(tester.element(selectedCard)).colorScheme.secondary,
      );
      final basic = tester.getRect(find.text('Basic Information'));
      final repositories = tester.getRect(find.text('Repositories'));
      expect(
        tester.getRect(projectKey('project-editor-menu')).bottom,
        lessThan(basic.top),
      );
      final actions = tester.getRect(projectKey('project-editor-actions'));
      expect(
        actions.top,
        greaterThan(
          tester.getRect(projectKey('expand-project-repository-0')).bottom,
        ),
      );
      final save = tester.getRect(projectKey('save-project'));
      final cancel = tester.getRect(projectKey('cancel-project'));
      expect(save.top, greaterThanOrEqualTo(actions.top));
      expect(cancel.top, greaterThanOrEqualTo(actions.top));
      expect(save.right, form.right);
      expect(cancel.right, lessThan(save.left));
      expect(
        tester.getRect(find.text('Edit Project')).bottom,
        lessThan(basic.top),
      );
      expect(
        basic.bottom,
        lessThan(tester.getRect(projectKey('project-key')).top),
      );
      final id = tester.getRect(projectKey('project-key'));
      final description = tester.getRect(projectKey('project-description'));
      final prompt = tester.getRect(projectKey('project-prompt'));
      expect(id.top, description.top);
      expect(description.width, closeTo(id.width * 2, .01));
      expect(prompt.top, greaterThan(description.bottom));
      expect(prompt.left, id.left);
      expect(prompt.right, description.right);
      expect(repositories.top, greaterThan(prompt.bottom));
      for (final label in ['Name', 'Agent', 'Execution harness', 'OpenCode']) {
        expect(find.text(label), findsNothing);
      }
      expect(projectKey('project-name'), findsNothing);
      final add = tester.getRect(projectKey('add-project-repository'));
      expect(add.center.dy, closeTo(repositories.center.dy, 1));
      expect(
        add.right,
        tester.getRect(projectKey('project-editor-form')).right,
      );
      expect(projectKey('project-repository-0-name'), findsNothing);
      expect(projectKey('project-repository-0-feature'), findsNothing);
      expect(projectKey('delete-project'), findsNothing);
      expect(projectKey('duplicate-project'), findsNothing);
      expect(find.byType(DropdownButton), findsNothing);
      for (final label in ['ID', 'Description', 'Prompt']) {
        expect(find.text(label), findsWidgets);
      }
      await expandProjectRepository(tester, 0);
      expect(
        projectField(tester, 'project-repository-0-work').controller!.text,
        'main',
      );
      expect(find.text('Read Only'), findsOneWidget);
      expect(find.byType(CheckboxListTile), findsNWidgets(4));
      final feature = tester.getRect(
        projectKey('project-repository-0-feature'),
      );
      final fix = tester.getRect(projectKey('project-repository-0-fix'));
      final release = tester.getRect(
        projectKey('project-repository-0-release'),
      );
      final hotfix = tester.getRect(projectKey('project-repository-0-hotfix'));
      expect(feature.top, fix.top);
      expect(release.top, hotfix.top);
      expect(release.top, greaterThan(feature.bottom));
      expect(feature.left, release.left);
      expect(fix.left, hotfix.left);
      expect(api.projectCalls, ['projects_list_v3']);
      expect(api.writes, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  for (final (width, namePaired, branchPaired) in [
    (879.0, true, true),
    (880.0, false, true),
    (900.0, false, true),
    (920.0, true, true),
    (1000.0, true, true),
    (1536.0, true, true),
    (400.0, false, false),
  ]) {
    testWidgets('repository pairs fit independently at local width $width', (
      tester,
    ) async {
      final api = ProjectsUiApi();
      api.records[alphaProject] = api.record(alphaProject, 'alpha', [
        _repository,
      ]);
      await mountProjects(tester, api, size: Size(width, 1024));
      await chooseProject(tester, alphaProject);
      await expandProjectRepository(tester, 0);
      final id = tester.getRect(projectKey('project-key'));
      final description = tester.getRect(projectKey('project-description'));
      final form = tester.getRect(projectKey('project-editor-form'));
      if (form.width >= 600) {
        expect(id.top, description.top);
        expect(description.width, closeTo(id.width * 2, .01));
      } else {
        expect(description.top - id.bottom, 16);
      }
      expect(tester.getRect(projectKey('project-prompt')).width, form.width);
      final name = tester.getRect(projectKey('project-repository-0-name'));
      final url = tester.getRect(projectKey('project-repository-0'));
      final base = tester.getRect(projectKey('project-repository-0-base'));
      final work = tester.getRect(projectKey('project-repository-0-work'));
      if (namePaired) {
        expect(name.top, url.top);
        expect(url.width, closeTo(name.width * 2, .01));
      } else {
        expect(url.top - name.bottom, greaterThanOrEqualTo(16));
        expect(url.width, name.width);
      }
      if (branchPaired) {
        expect(base.top, work.top);
        expect(base.width, work.width);
      } else {
        expect(work.top - base.bottom, greaterThanOrEqualTo(16));
      }
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    });
  }

  for (final scale in [1.0, 2.0]) {
    testWidgets('stacked fields keep a readable gutter at ${scale}x', (
      tester,
    ) async {
      final api = ProjectsUiApi();
      await mountProjects(
        tester,
        api,
        size: const Size(400, 400),
        scale: scale,
      );
      await chooseProject(tester, alphaProject);
      final id = tester.getRect(projectKey('project-key'));
      final description = tester.getRect(projectKey('project-description'));
      expect(description.left, id.left);
      expect(description.top - id.bottom, 16);
      expect(tester.takeException(), isNull);
      expect(api.writes, isEmpty);
      await tester.pumpWidget(const SizedBox());
    });
  }

  testWidgets(
    'focused type editor keeps its state and text selection across paired and stacked reflow',
    (tester) async {
      final api = ProjectsUiApi();
      api.records[alphaProject] = api.record(alphaProject, 'alpha', [
        _repository,
      ]);
      await mountProjects(tester, api, size: const Size(1536, 1024));
      await chooseProject(tester, alphaProject);
      await selectProjectType(tester, 0, 'feature');
      const key = 'project-repository-0-feature';
      await tester.enterText(projectKey(key), 'retained/*');
      final controller = projectField(tester, key).controller!;
      controller.selection = const TextSelection(
        baseOffset: 1,
        extentOffset: 5,
      );
      await tester.pumpAndSettle();
      final editable = find.descendant(
        of: projectKey(key),
        matching: find.byType(EditableText),
      );
      final state = tester.state(editable);
      final focus = _editable(tester, key).focusNode;
      expect(focus.hasFocus, isTrue);
      for (final width in [1000.0, 880.0, 400.0, 1536.0]) {
        tester.view.physicalSize = Size(width, 1024);
        await tester.pumpAndSettle();
        expect(tester.state(editable), same(state));
        expect(projectField(tester, key).controller, same(controller));
        expect(_editable(tester, key).focusNode, same(focus));
        expect(focus.hasFocus, isTrue);
        expect(
          controller.selection,
          const TextSelection(baseOffset: 1, extentOffset: 5),
        );
        expect(controller.text, 'retained/*');
      }
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(api.projectCalls, ['projects_list_v3']);
      expect(api.writes, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'search is saved key/description only, local, and never replaces the selected draft',
    (tester) async {
      final api = ProjectsUiApi();
      api.records[alphaProject] = api.record(
        alphaProject,
        'Hidden compatibility name',
        [_repository],
        'alpha',
        'Shared utilities',
        'Unsearchable project context',
      );
      await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      await tester.enterText(projectKey('project-key'), 'unsaved-key');
      await selectProjectType(tester, 0, 'feature');
      final name = projectField(tester, 'project-key').controller!;
      name.selection = const TextSelection(baseOffset: 1, extentOffset: 5);
      final selection = name.selection;
      for (final (query, alpha, beta) in [
        ('  bEtA  ', false, true),
        ('uTiLiTiEs', true, false),
        ('Hidden compatibility', false, false),
        ('Unsearchable project context', false, false),
        ('backend', false, false),
        ('opencode', false, false),
        ('unsaved-key', false, false),
        ('', true, true),
      ]) {
        await tester.enterText(projectKey('search-projects'), query);
        await tester.pumpAndSettle();
        expect(
          projectKey('project-$alphaProject'),
          alpha ? findsOneWidget : findsNothing,
        );
        expect(
          projectKey('project-$betaProject'),
          beta ? findsOneWidget : findsNothing,
        );
        expect(
          find.text('No matching projects.'),
          !alpha && !beta ? findsOneWidget : findsNothing,
        );
        expect(find.text('No projects yet.'), findsNothing);
        expect(projectField(tester, 'project-key').controller, same(name));
        expect(name.text, 'unsaved-key');
        expect(name.selection, selection);
        expect(projectKey('project-repository-0-feature'), findsOneWidget);
      }
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(api.projectCalls, ['projects_list_v3']);
      expect(api.writes, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  for (final state in ['loading', 'empty', 'unavailable']) {
    testWidgets('list distinguishes $state from a local search miss', (
      tester,
    ) async {
      final api = ProjectsUiApi(empty: true)
        ..listFailure = state == 'unavailable';
      if (state == 'loading') api.listGate = Completer<AppReply>();
      await mountProjects(tester, api, size: const Size(400, 400));
      await tester.enterText(projectKey('search-projects'), 'not a project');
      await tester.pumpAndSettle();
      expect(find.text('No matching projects.'), findsNothing);
      final message = switch (state) {
        'loading' => 'Loading projects…',
        'empty' => 'No projects yet.',
        _ => 'Project metadata unavailable.',
      };
      expect(find.text(message), findsOneWidget);
      expect(projectKey('refresh-projects-list'), findsOneWidget);
      expect(projectKey('project-key'), findsNothing);
      expect(api.projectCalls, ['projects_list_v3']);
      if (api.listGate != null) {
        api.listGate!.complete(
          AppReply(api.instance, {'revision': 0, 'projects': []}),
        );
        await tester.pumpAndSettle();
      }
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    });
  }

  for (final scale in [1.0, 2.0]) {
    testWidgets(
      'compact Back/return/resize retains both scrolls, selection, disclosure and focus exclusion at ${scale}x',
      (tester) async {
        final api = ProjectsUiApi(empty: true);
        for (var i = 0; i < 24; i++) {
          final id = i.toRadixString(16).padLeft(32, '0');
          api.records[id] = api.record(id, 'Project $i', [
            _repository,
            ProjectRepository(url: 'https://example.invalid/team/client'),
            ProjectRepository(url: 'https://example.invalid/team/tools'),
          ]);
        }
        const id = '00000000000000000000000000000008';
        await mountProjects(
          tester,
          api,
          size: const Size(400, 400),
          scale: scale,
        );
        expect(projectKey('project-key'), findsNothing);
        expect(projectKey('choose-project'), findsNothing);
        final list = _scroll(tester, 'project-list');
        await chooseProject(tester, id);
        final listBookmark = list.offset;
        expect(listBookmark, greaterThan(0));
        await tester.enterText(projectKey('project-key'), 'retained-draft');
        await selectProjectType(tester, 0, 'feature');
        await tester.enterText(
          projectKey('project-repository-0-feature'),
          'retained/*',
        );
        final name = projectField(tester, 'project-key').controller!;
        final field = projectField(
          tester,
          'project-repository-0-feature',
        ).controller!;
        field.selection = const TextSelection(baseOffset: 1, extentOffset: 5);
        final selection = field.selection;
        final focus = _editable(
          tester,
          'project-repository-0-feature',
        ).focusNode;
        final editor = _scroll(tester, 'project-editor-scroll');
        expect(editor, isNot(same(list)));
        final back = tester
            .widget<TextButton>(projectKey('back-to-projects'))
            .onPressed!;
        await tester.pumpAndSettle();
        editor.jumpTo(700);
        await tester.pumpAndSettle();
        final bookmark = editor.offset;
        expect(bookmark, 700);
        // Invoke the current callback without ensureVisible changing the bookmark.
        back();
        await tester.pumpAndSettle();
        expect(projectKey('project-key'), findsNothing);
        expect(_retainedField(tester, 'project-key').controller, same(name));
        expect(list.offset, listBookmark);
        expect(editor.offset, bookmark);
        expect(focus.hasFocus, isFalse);
        expect(focus.canRequestFocus, isFalse);
        focus.requestFocus();
        await tester.pump();
        expect(focus.hasFocus, isFalse);
        await projectTap(tester, 'project-$id');
        expect(find.text('Discard unsaved project changes?'), findsNothing);
        expect(projectField(tester, 'project-key').controller, same(name));
        expect(
          projectField(tester, 'project-repository-0-feature').controller,
          same(field),
        );
        expect(name.text, 'retained-draft');
        expect(field.text, 'retained/*');
        expect(field.selection, selection);
        expect(editor.offset, bookmark);
        expect(focus.hasFocus, isFalse);
        tester.view.physicalSize = const Size(4200, 2400);
        await tester.pumpAndSettle();
        expect(_scroll(tester, 'project-list'), same(list));
        expect(_scroll(tester, 'project-editor-scroll'), same(editor));
        expect(tester.getSize(projectKey('project-editor-form')).width, 960);
        expect(editor.offset, lessThanOrEqualTo(bookmark));
        tester.view.physicalSize = const Size(400, 400);
        await tester.pumpAndSettle();
        expect(projectKey('back-to-projects'), findsOneWidget);
        expect(editor.offset, bookmark);
        expect(list.offset, listBookmark);
        expect(field.selection, selection);
        expect(projectKey('project-repository-0-feature'), findsOneWidget);
        for (final key in [
          'back-to-projects',
          'save-project',
          'cancel-project',
          'project-editor-menu',
          'refresh-projects',
        ]) {
          await tester.ensureVisible(projectKey(key));
          await tester.pumpAndSettle();
          final size = tester.getSize(projectKey(key));
          expect(size.width, greaterThanOrEqualTo(48));
          expect(size.height, greaterThanOrEqualTo(48));
          expect(projectKey(key).hitTestable(), findsOneWidget);
        }
        expect(api.projectCalls, ['projects_list_v3']);
        expect(api.writes, isEmpty);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox());
      },
    );
  }

  testWidgets(
    'compact other-project and New retain the discard guard; current callback does not reload',
    (tester) async {
      final api = ProjectsUiApi();
      await mountProjects(tester, api, size: const Size(400, 400));
      await chooseProject(tester, alphaProject);
      await tester.enterText(projectKey('project-key'), 'keep-alpha-draft');
      final name = projectField(tester, 'project-key').controller!;
      for (final target in ['project-$betaProject', 'new-project']) {
        await projectTap(tester, 'back-to-projects');
        await projectTap(tester, target);
        expect(find.text('Discard unsaved project changes?'), findsOneWidget);
        await tester.tap(find.text('Keep editing'));
        await tester.pumpAndSettle();
        if (projectKey('project-key').evaluate().isEmpty) {
          await projectTap(tester, 'project-$alphaProject');
        }
        expect(projectField(tester, 'project-key').controller, same(name));
        expect(name.text, 'keep-alpha-draft');
      }
      await projectTap(tester, 'back-to-projects');
      await projectTap(tester, 'project-$betaProject');
      await tester.tap(find.text('Discard changes'));
      await tester.pumpAndSettle();
      expect(projectField(tester, 'project-key').controller!.text, 'beta');
      expect(api.projectCalls, ['projects_list_v3']);
      expect(api.writes, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  for (final scale in [1.0, 2.0]) {
    for (final state in ['new', 'missing', 'uncertain', 'filtered']) {
      testWidgets(
        'compact return reaches the retained $state editor without rebinding at ${scale}x',
        (tester) async {
          final api = ProjectsUiApi(
            empty: state == 'new' || state == 'uncertain',
          );
          final controller = await mountProjects(
            tester,
            api,
            size: const Size(400, 400),
            scale: scale,
          );
          final creating = state == 'new' || state == 'uncertain';
          if (creating) {
            await projectTap(tester, 'new-project');
          } else {
            await chooseProject(tester, alphaProject);
          }
          await tester.enterText(projectKey('project-key'), 'retained-draft');
          await projectTap(tester, 'add-project-repository');
          await tester.enterText(
            projectKey('project-repository-0'),
            'https://example.invalid/team/retained',
          );
          await selectProjectType(tester, 0, 'feature');
          await tester.enterText(
            projectKey('project-repository-0-feature'),
            'retained/*',
          );
          final name = projectField(tester, 'project-key').controller!;
          final field = projectField(
            tester,
            'project-repository-0-feature',
          ).controller!;
          field.selection = const TextSelection(baseOffset: 1, extentOffset: 5);
          final focus = _editable(
            tester,
            'project-repository-0-feature',
          ).focusNode;
          final oldSave = tester
              .widget<FilledButton>(projectKey('save-project'))
              .onPressed!;
          if (state == 'missing') {
            api.records.remove(alphaProject);
            api.projectRevision++;
            await controller.refresh();
          } else if (state == 'uncertain') {
            api.loseAcknowledgement = true;
            await projectTap(tester, 'save-project');
            api.records.clear();
            api.receipts.clear();
            api.missingReceiptCode = 'REQUEST_EXPIRED';
            await controller.resolvePending();
            expect(controller.uncertain, isTrue);
          }
          await tester.pumpAndSettle();
          final calls = List<String>.of(api.projectCalls);
          await projectTap(tester, 'back-to-projects');
          final editor = _scroll(tester, 'project-editor-scroll');
          final bookmark = editor.offset;
          expect(projectKey('project-key'), findsNothing);
          expect(focus.hasFocus, isFalse);
          expect(focus.canRequestFocus, isFalse);
          if (state == 'new') {
            await projectTap(tester, 'new-project');
            await tester.tap(find.text('Keep editing'));
            await tester.pumpAndSettle();
            expect(projectField(tester, 'project-key').controller, same(name));
            expect(name.text, 'retained-draft');
            await projectTap(tester, 'back-to-projects');
          } else if (state == 'filtered') {
            await tester.enterText(projectKey('search-projects'), 'beta');
            await tester.pumpAndSettle();
            expect(projectKey('project-$alphaProject'), findsNothing);
          } else if (state == 'uncertain') {
            expect(
              tester
                  .widget<OutlinedButton>(projectKey('new-project'))
                  .onPressed,
              isNull,
            );
          }
          final resume = projectKey('resume-project-editor');
          await tester.ensureVisible(resume);
          await tester.pumpAndSettle();
          expect(resume.hitTestable(), findsOneWidget);
          expect(tester.getSize(resume).width, greaterThanOrEqualTo(48));
          expect(tester.getSize(resume).height, greaterThanOrEqualTo(48));
          await tester.tap(resume);
          await tester.pumpAndSettle();
          expect(projectField(tester, 'project-key').controller, same(name));
          expect(
            projectField(tester, 'project-repository-0-feature').controller,
            same(field),
          );
          expect(name.text, 'retained-draft');
          expect(field.text, 'retained/*');
          expect(
            field.selection,
            const TextSelection(baseOffset: 1, extentOffset: 5),
          );
          expect(editor.offset, bookmark);
          expect(focus.hasFocus, isFalse);
          if (state == 'missing' || state == 'uncertain') {
            expect(
              tester.widget<FilledButton>(projectKey('save-project')).onPressed,
              isNull,
            );
            oldSave();
            await tester.pumpAndSettle();
          }
          expect(api.projectCalls, calls);
          expect(
            api.writes,
            state == 'uncertain' ? ['project_create_v3'] : isEmpty,
          );
          expect(tester.takeException(), isNull);
          await tester.pumpWidget(const SizedBox());
        },
      );
    }
  }

  group('bundled-font compact keyboard wrap', () {
    for (final scale in [1.0, 2.0]) {
      for (final reverse in [false, true]) {
        testWidgets(
          'reveals every focused control through ${reverse ? 'reverse' : 'forward'} wrap at ${scale}x',
          (tester) async {
            final api = ProjectsUiApi(empty: true);
            for (var i = 0; i < 20; i++) {
              final id = i.toRadixString(16).padLeft(32, '0');
              api.records[id] = api.record(id, 'Project $i');
            }
            await mountProjects(
              tester,
              api,
              size: const Size(400, 400),
              scale: scale,
            );
            await projectTap(tester, 'new-project');
            await tester.enterText(
              projectKey('project-key'),
              'retained-keyboard-draft',
            );
            final name = projectField(tester, 'project-key').controller!;
            name.selection = const TextSelection(
              baseOffset: 1,
              extentOffset: 5,
            );
            await projectTap(tester, 'back-to-projects');
            await tester.tap(projectKey('search-projects'));
            await tester.pumpAndSettle();
            final resume = projectKey('resume-project-editor');
            final focus = Focus.of(
              tester.element(
                find.descendant(
                  of: resume,
                  matching: find.text('Return to project editor'),
                ),
              ),
            );
            final lastCardFocus = Focus.of(
              tester.element(find.text('project-19')),
            );
            var visitedBottom = false;
            for (var i = 0; i < 60; i++) {
              if (reverse) {
                await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
              }
              await tester.sendKeyEvent(LogicalKeyboardKey.tab);
              if (reverse) {
                await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
              }
              await tester.pumpAndSettle();
              final rect = FocusManager.instance.primaryFocus!.rect;
              expect(rect.top, greaterThanOrEqualTo(0));
              expect(rect.bottom, lessThanOrEqualTo(400));
              visitedBottom = visitedBottom || lastCardFocus.hasPrimaryFocus;
              if (focus.hasPrimaryFocus && visitedBottom) break;
            }
            expect(visitedBottom, isTrue);
            expect(focus.hasPrimaryFocus, isTrue);
            expect(resume.hitTestable(), findsOneWidget);
            expect(tester.getSize(resume).height, greaterThanOrEqualTo(48));
            expect(tester.getSize(resume).width, greaterThanOrEqualTo(48));
            await tester.sendKeyEvent(LogicalKeyboardKey.enter);
            await tester.pumpAndSettle();
            expect(projectField(tester, 'project-key').controller, same(name));
            expect(name.text, 'retained-keyboard-draft');
            expect(
              name.selection,
              const TextSelection(baseOffset: 1, extentOffset: 5),
            );
            expect(api.projectCalls, ['projects_list_v3']);
            expect(api.writes, isEmpty);
            expect(tester.takeException(), isNull);
            await tester.pumpWidget(const SizedBox());
          },
        );
      }
    }
  });

  testWidgets(
    'compact list keeps Refresh and receipt-only recovery reachable without rebinding the draft',
    (tester) async {
      final api = ProjectsUiApi()..loseAcknowledgement = true;
      final controller = await mountProjects(
        tester,
        api,
        size: const Size(400, 400),
        scale: 2,
      );
      await chooseProject(tester, alphaProject);
      await tester.enterText(projectKey('project-key'), 'uncertain-draft');
      await projectTap(tester, 'save-project');
      expect(controller.uncertain, isTrue);
      final name = projectField(tester, 'project-key').controller!;
      final request = api.arguments.singleWhere(
        (a) => a.containsKey('expected_revision'),
      )['request_id'];
      api.receipts.clear();
      api.missingReceiptCode = 'REQUEST_EXPIRED';
      await projectTap(tester, 'back-to-projects');
      await projectTap(tester, 'refresh-projects-list');
      await projectTap(tester, 'resolve-project-change');
      expect(controller.uncertain, isTrue);
      expect(api.arguments[api.calls.indexOf('project_receipt')], {
        'request_id': request,
      });
      expect(api.writes, ['project_update_v3']);
      await projectTap(tester, 'project-$alphaProject');
      expect(projectField(tester, 'project-key').controller, same(name));
      expect(name.text, 'uncertain-draft');
      expect(
        tester.widget<FilledButton>(projectKey('save-project')).onPressed,
        isNull,
      );
      expect(
        tester
            .widget<PopupMenuButton<String>>(projectKey('project-editor-menu'))
            .enabled,
        isFalse,
      );
      expect(
        tester.widget<TextButton>(projectKey('refresh-projects')).onPressed,
        isNotNull,
      );
      expect(api.projectCalls, [
        'projects_list_v3',
        'project_update_v3',
        'projects_list_v3',
        'project_receipt',
        'projects_list_v3',
      ]);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );
}
