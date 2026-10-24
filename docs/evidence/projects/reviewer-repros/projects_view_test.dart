import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/projects.dart';
import 'package:smith_client/projects_controller.dart';
import 'package:smith_client/ui/projects_view.dart';
import 'package:smith_client/ui/smith_theme.dart';

import 'app_fakes.dart';

const alphaProject = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
const betaProject = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';

/// No sockets, repository access, providers, processes or inference.
class ProjectsUiApi extends FakeAppApi {
  ProjectsUiApi({bool empty = false}) {
    if (!empty) {
      records[alphaProject] = record(alphaProject, 'Alpha');
      records[betaProject] = record(betaProject, 'Beta');
    }
  }

  final records = <String, Map<String, dynamic>>{};
  final receipts = <String, Map<String, dynamic>>{};
  int projectRevision = 0;
  int nextId = 100;
  bool listFailure = false;
  bool rejectMutation = false;
  bool loseAcknowledgement = false;
  Completer<AppReply>? listGate;

  Map<String, dynamic> record(
    String id,
    String name, [
    List<String> repos = const [],
  ]) => {
    'project_id': id,
    'name': name,
    'repositories': List<String>.of(repos),
    'agent': 'opencode',
  };

  List<String> get projectCalls =>
      calls.where((op) => op.startsWith('project')).toList();
  List<String> get writes => projectCalls
      .where((op) => !['projects_list', 'project_receipt'].contains(op))
      .toList();

  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    if (!op.startsWith('project')) return super.call(op, args);
    calls.add(op);
    arguments.add(Map.of(args));
    validateProjectArguments(op, args);
    if (op == 'projects_list') {
      if (listFailure) {
        throw const AppFailure('TRANSPORT', 'Project metadata unavailable.');
      }
      if (listGate != null) return listGate!.future;
      return AppReply(instance, {
        'revision': projectRevision,
        'projects': records.values.map((record) => Map.of(record)).toList(),
      });
    }
    if (op == 'project_receipt') {
      final receipt = receipts[args['request_id']];
      if (receipt == null) {
        throw const AppFailure('REQUEST_UNKNOWN', 'No saved acknowledgement.');
      }
      return AppReply(instance, Map.of(receipt));
    }
    if (rejectMutation) {
      throw const AppFailure(
        'CAPACITY',
        'Project metadata could not be saved.',
      );
    }
    if (args['expected_revision'] != projectRevision ||
        args['instance_id'] != instance) {
      throw const AppFailure('STALE_REVISION', 'Refresh project metadata.');
    }
    final id = op == 'project_create'
        ? (++nextId).toRadixString(16).padLeft(32, '0')
        : args['project_id'] as String;
    if (op != 'project_create' && !records.containsKey(id)) {
      throw const AppFailure('NOT_FOUND', 'Project not found.');
    }
    if (op == 'project_delete') {
      records.remove(id);
    } else {
      records[id] = record(
        id,
        args['name'],
        List<String>.from(args['repositories']),
      );
    }
    final receipt = {
      'request_id': args['request_id'],
      'project_id': id,
      'revision': ++projectRevision,
      'deleted': op == 'project_delete',
    };
    receipts[args['request_id']] = receipt;
    if (loseAcknowledgement) {
      throw const AppFailure(
        'TRANSPORT',
        'Acknowledgement unavailable.',
        unknown: true,
      );
    }
    return AppReply(instance, receipt);
  }
}

Finder projectKey(String key) => find.byKey(ValueKey(key));
TextField projectField(WidgetTester tester, String key) =>
    tester.widget<TextField>(projectKey(key));

Future<void> projectTap(WidgetTester tester, String key) async {
  final target = projectKey(key);
  await tester.pumpAndSettle();
  await tester.ensureVisible(target);
  await tester.pumpAndSettle();
  await tester.tap(target);
  await tester.pumpAndSettle();
}

Future<void> chooseProject(WidgetTester tester, String id) async {
  if (projectKey('choose-project').evaluate().isNotEmpty) {
    await projectTap(tester, 'choose-project');
  }
  await projectTap(tester, 'project-$id');
}

Future<ProjectsController> mountProjects(
  WidgetTester tester,
  ProjectsUiApi api, {
  Size size = const Size(1280, 900),
  double scale = 1,
}) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  final controller = ProjectsController(api);
  addTearDown(controller.dispose);
  await tester.pumpWidget(
    MaterialApp(
      theme: SmithTheme.dark(),
      builder: (context, child) => MediaQuery(
        data: MediaQuery.of(context)
            .copyWith(textScaler: TextScaler.linear(scale)),
        child: child!,
      ),
      home: Scaffold(body: ProjectsView(controller: controller)),
    ),
  );
  await tester.pumpAndSettle();
  return controller;
}

void main() {
  testWidgets(
    'wide list and editor: new is unsaved, Cancel has no effects, empty repositories save',
    (tester) async {
      final api = ProjectsUiApi(empty: true);
      await mountProjects(tester, api);
      expect(find.text('No projects yet.'), findsOneWidget);
      expect(find.text('No project selected'), findsOneWidget);
      expect(api.calls, ['projects_list']);
      await projectTap(tester, 'new-project');
      expect(find.text('New project · not saved'), findsOneWidget);
      await tester.enterText(projectKey('project-name'), 'Not saved');
      expect(api.writes, isEmpty);
      await projectTap(tester, 'cancel-project');
      expect(find.text('No project selected'), findsOneWidget);
      expect(api.writes, isEmpty);
      await projectTap(tester, 'new-project');
      await tester.enterText(projectKey('project-name'), 'Saved foundation');
      await projectTap(tester, 'save-project');
      expect(api.records.values.single['repositories'], isEmpty);
      expect(api.records.values.single['agent'], 'opencode');
      expect(api.writes, ['project_create']);
      expect(find.text('Project settings'), findsOneWidget);
      expect(find.text('OpenCode'), findsOneWidget);
      expect(find.byType(DropdownButton), findsNothing);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      expect(api.disposedCount, 0);
    },
  );

  testWidgets(
    'wide/compact resizing keeps the same editor, selection and unsaved repository rows',
    (tester) async {
      final api = ProjectsUiApi();
      await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      await tester.enterText(
        projectKey('project-name'),
        'Retained across breakpoints',
      );
      await projectTap(tester, 'add-project-repository');
      await tester.enterText(
        projectKey('project-repository-0'),
        'git@example.invalid:team/repo',
      );
      final name = projectField(tester, 'project-name').controller;
      final repository = projectField(
        tester,
        'project-repository-0',
      ).controller;
      final state = tester.state(find.byType(ProjectsView));
      for (final size in [const Size(400, 400), const Size(1600, 1000)]) {
        tester.view.physicalSize = size;
        await tester.pumpAndSettle();
        expect(tester.state(find.byType(ProjectsView)), same(state));
        expect(projectField(tester, 'project-name').controller, same(name));
        expect(
          projectField(tester, 'project-repository-0').controller,
          same(repository),
        );
        expect(name!.text, 'Retained across breakpoints');
        expect(repository!.text, 'git@example.invalid:team/repo');
        expect(tester.takeException(), isNull);
      }
      expect(api.projectCalls, ['projects_list']);
      expect(api.writes, isEmpty);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'disposing a view during a query leaves the shared controller and API alive',
    (tester) async {
      final api = ProjectsUiApi()..listGate = Completer<AppReply>();
      final controller = await mountProjects(tester, api);
      await tester.pumpWidget(const SizedBox());
      api.listGate!.complete(
        AppReply(api.instance, {
          'revision': 0,
          'projects': api.records.values.toList(),
        }),
      );
      api.listGate = null;
      await controller.refresh();
      await tester.pumpAndSettle();
      expect(controller.loaded, isTrue);
      expect(api.projectCalls, ['projects_list']);
      expect(api.disposedCount, 0);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'shared local validators preserve invalid 41-scalar names and locators without writes',
    (tester) async {
      final api = ProjectsUiApi(empty: true);
      await mountProjects(tester, api);
      await projectTap(tester, 'new-project');
      final tooLong = List.filled(41, '😀').join();
      await tester.enterText(projectKey('project-name'), tooLong);
      await projectTap(tester, 'save-project');
      expect(find.text(projectNameError(tooLong)!), findsOneWidget);
      expect(projectField(tester, 'project-name').controller!.text, tooLong);
      await tester.enterText(
        projectKey('project-name'),
        List.filled(40, '😀').join(),
      );
      await projectTap(tester, 'add-project-repository');
      const invalid = 'https://user:password@example.invalid/repo';
      await tester.enterText(projectKey('project-repository-0'), invalid);
      await projectTap(tester, 'save-project');
      expect(find.text(repositoryLocatorError(invalid)!), findsOneWidget);
      expect(
        projectField(tester, 'project-repository-0').controller!.text,
        invalid,
      );
      expect(api.writes, isEmpty);
      await tester.enterText(
        projectKey('project-repository-0'),
        'https://example.invalid/team/repo',
      );
      await projectTap(tester, 'save-project');
      expect(api.writes, ['project_create']);
      expect(api.records.values.single['name'], List.filled(40, '😀').join());
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'repeatable repository rows reject duplicates, remove safely and stop at sixteen',
    (tester) async {
      final api = ProjectsUiApi(empty: true);
      await mountProjects(tester, api);
      await projectTap(tester, 'new-project');
      await tester.enterText(projectKey('project-name'), 'Repositories');
      for (var i = 0; i < 2; i++) {
        await projectTap(tester, 'add-project-repository');
        await tester.enterText(
          projectKey('project-repository-$i'),
          'git@example.invalid:team/repo',
        );
      }
      await projectTap(tester, 'save-project');
      expect(
        find.text('Repository locators must be distinct.'),
        findsNWidgets(2),
      );
      expect(api.writes, isEmpty);
      await tester.ensureVisible(find.byTooltip('Remove repository 1'));
      await tester.tap(find.byTooltip('Remove repository 1'));
      await tester.pumpAndSettle();
      expect(
        projectField(tester, 'project-repository-0').controller!.text,
        'git@example.invalid:team/repo',
      );
      for (var i = 1; i < 16; i++) {
        await projectTap(tester, 'add-project-repository');
        await tester.enterText(
          projectKey('project-repository-$i'),
          'https://example.invalid/team/repo-$i',
        );
      }
      expect(
        tester
            .widget<TextButton>(projectKey('add-project-repository'))
            .onPressed,
        isNull,
      );
      await projectTap(tester, 'save-project');
      expect(api.records.values.single['repositories'], hasLength(16));
      expect(api.writes, ['project_create']);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  for (final fixture in [
    (const Size(400, 400), 2.0),
    (const Size(360, 640), 1.0),
    (const Size(4200, 2400), 1.0),
  ]) {
    testWidgets(
      'chooser/list, scrollable fields and actions reachable at ${fixture.$1} scale ${fixture.$2}',
      (tester) async {
        final api = ProjectsUiApi();
        await mountProjects(tester, api, size: fixture.$1, scale: fixture.$2);
        await chooseProject(tester, alphaProject);
        await tester.enterText(
          projectKey('project-name'),
          'Edited at this size',
        );
        await projectTap(tester, 'add-project-repository');
        await tester.enterText(
          projectKey('project-repository-0'),
          'ssh://git@example.invalid/team/repo',
        );
        await projectTap(tester, 'save-project');
        expect(api.records[alphaProject]!['name'], 'Edited at this size');
        await projectTap(tester, 'delete-project');
        expect(find.text('Delete project metadata?'), findsOneWidget);
        await tester.ensureVisible(find.text('Keep project'));
        await tester.tap(find.text('Keep project'));
        await tester.pumpAndSettle();
        expect(api.writes, ['project_update']);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox());
      },
    );
  }

  testWidgets(
    'keyboard traversal reaches editor and Save; Enter in a field does not mutate',
    (tester) async {
      final api = ProjectsUiApi(empty: true);
      await mountProjects(tester, api);
      await projectTap(tester, 'new-project');
      await tester.tap(projectKey('project-name'));
      await tester.enterText(projectKey('project-name'), 'Keyboard');
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(api.writes, isEmpty);
      // Name -> Add repository -> Save (buttons use the normal traversal order).
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pumpAndSettle();
      expect(Focus.of(tester.element(find.text('Save'))).hasFocus, isTrue);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(api.writes, ['project_create']);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'dirty selection switch asks before discarding and Cancel reloads saved metadata',
    (tester) async {
      final api = ProjectsUiApi();
      await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      await tester.enterText(
        projectKey('project-name'),
        'Only in Alpha editor',
      );
      await chooseProject(tester, betaProject);
      expect(find.text('Discard unsaved project changes?'), findsOneWidget);
      await tester.tap(find.text('Keep editing'));
      await tester.pumpAndSettle();
      expect(
        projectField(tester, 'project-name').controller!.text,
        'Only in Alpha editor',
      );
      await projectTap(tester, 'cancel-project');
      expect(projectField(tester, 'project-name').controller!.text, 'Alpha');
      expect(api.writes, isEmpty);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'refresh never silently rebases a dirty form; explicit review authorizes only a later Save',
    (tester) async {
      final api = ProjectsUiApi();
      final controller = await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      await tester.enterText(projectKey('project-name'), 'Local edits');
      api.records[alphaProject] = api.record(alphaProject, 'Foreign edits');
      api.projectRevision++;
      await projectTap(tester, 'refresh-projects');
      expect(controller.revision, 1);
      expect(
        projectField(tester, 'project-name').controller!.text,
        'Local edits',
      );
      expect(
        tester.widget<FilledButton>(projectKey('save-project')).onPressed,
        isNull,
      );
      expect(
        tester.widget<TextButton>(projectKey('delete-project')).onPressed,
        isNull,
      );
      await projectTap(tester, 'review-project-revision');
      expect(find.text('Foreign edits'), findsNWidgets(2));
      expect(
        find.descendant(
          of: find.byType(AlertDialog),
          matching: find.text('Local edits'),
        ),
        findsOneWidget,
      );
      await tester.tap(find.text('Keep edits at this revision'));
      await tester.pumpAndSettle();
      expect(api.writes, isEmpty);
      await projectTap(tester, 'save-project');
      expect(api.writes, ['project_update']);
      expect(api.records[alphaProject]!['name'], 'Local edits');
      expect(
        api.arguments.lastWhere(
          (args) => args.containsKey('expected_revision'),
        )['expected_revision'],
        1,
      );
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'removed dirty target keeps input blocked; never substitutes another project',
    (tester) async {
      final api = ProjectsUiApi();
      await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      await tester.enterText(
        projectKey('project-name'),
        'Keep deleted target edits',
      );
      api.records.remove(alphaProject);
      api.projectRevision++;
      await projectTap(tester, 'refresh-projects');
      expect(
        projectField(tester, 'project-name').controller!.text,
        'Keep deleted target edits',
      );
      expect(find.textContaining('no longer in the registry'), findsOneWidget);
      expect(
        tester.widget<FilledButton>(projectKey('save-project')).onPressed,
        isNull,
      );
      expect(projectKey('review-project-revision'), findsNothing);
      expect(api.writes, isEmpty);
      await projectTap(tester, 'cancel-project');
      expect(find.text('No project selected'), findsOneWidget);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'delete binds exact target and revision; a stale open dialog cannot delete',
    (tester) async {
      final api = ProjectsUiApi();
      final controller = await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      await projectTap(tester, 'delete-project');
      expect(find.text('Project ID: $alphaProject'), findsOneWidget);
      expect(
        find.textContaining(
          'Repository files, chats, credentials and installations are not changed',
        ),
        findsOneWidget,
      );
      api.projectRevision++;
      await controller.refresh();
      await tester.pumpAndSettle();
      expect(
        tester
            .widget<TextButton>(
              find.widgetWithText(TextButton, 'Delete metadata'),
            )
            .onPressed,
        isNull,
      );
      await tester.tap(find.text('Keep project'));
      await tester.pumpAndSettle();
      expect(api.writes, isEmpty);
      await projectTap(tester, 'delete-project');
      await tester.tap(find.text('Delete metadata'));
      await tester.pumpAndSettle();
      expect(api.writes, ['project_delete']);
      expect(api.records.keys, [betaProject]);
      expect(find.text('No project selected'), findsOneWidget);
      expect(projectKey('project-name'), findsNothing);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'uncertain deletion resolves by query to no selection, never to the next project',
    (tester) async {
      final api = ProjectsUiApi()..loseAcknowledgement = true;
      final controller = await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      await projectTap(tester, 'delete-project');
      await tester.tap(find.text('Delete metadata'));
      await tester.pumpAndSettle();
      expect(controller.uncertain, isTrue);
      expect(projectField(tester, 'project-name').enabled, isFalse);
      await projectTap(tester, 'resolve-project-change');
      expect(controller.uncertain, isFalse);
      expect(find.text('No project selected'), findsOneWidget);
      expect(projectKey('project-name'), findsNothing);
      expect(api.records.keys, [betaProject]);
      expect(api.writes, ['project_delete']);
      expect(
        api.projectCalls.where((op) => op == 'project_receipt'),
        hasLength(1),
      );
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'lost create acknowledgement resolves from another view without duplicate creation or losing newer edits',
    (tester) async {
      final api = ProjectsUiApi(empty: true)..loseAcknowledgement = true;
      final controller = await mountProjects(tester, api);
      await projectTap(tester, 'new-project');
      await tester.enterText(projectKey('project-name'), 'Original create');
      await projectTap(tester, 'save-project');
      expect(controller.uncertain, isTrue);
      await tester.enterText(projectKey('project-name'), 'Newer unsaved edits');
      await controller.resolvePending();
      await tester.pumpAndSettle();
      expect(controller.uncertain, isFalse);
      expect(find.text('Project settings'), findsOneWidget);
      expect(
        projectField(tester, 'project-name').controller!.text,
        'Newer unsaved edits',
      );
      expect(
        tester.widget<FilledButton>(projectKey('save-project')).onPressed,
        isNull,
      );
      expect(api.records.values.single['name'], 'Original create');
      expect(api.writes, ['project_create']);
      expect(projectKey('review-project-revision'), findsOneWidget);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'later registry writes after uncertain creation never select an inferred target or enable duplicate Save',
    (tester) async {
      final api = ProjectsUiApi(empty: true)..loseAcknowledgement = true;
      final controller = await mountProjects(tester, api);
      await projectTap(tester, 'new-project');
      await tester.enterText(projectKey('project-name'), 'Original create');
      await projectTap(tester, 'save-project');
      api.records[betaProject] = api.record(
        betaProject,
        'Another desktop created this',
      );
      api.projectRevision++;
      await controller.resolvePending();
      await tester.pumpAndSettle();
      expect(controller.uncertain, isFalse);
      expect(
        projectField(tester, 'project-name').controller!.text,
        'Original create',
      );
      expect(find.textContaining('do not repeat the creation'), findsOneWidget);
      expect(
        tester.widget<FilledButton>(projectKey('save-project')).onPressed,
        isNull,
      );
      expect(projectKey('review-project-revision'), findsNothing);
      expect(api.writes, ['project_create']);
      await projectTap(tester, 'cancel-project');
      expect(find.text('No project selected'), findsOneWidget);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'failed refresh and Save preserve text; uncertainty offers query-only resolution and blocks writes',
    (tester) async {
      final api = ProjectsUiApi();
      final controller = await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      await tester.enterText(
        projectKey('project-name'),
        'Preserved through failure',
      );
      api.listFailure = true;
      await projectTap(tester, 'refresh-projects');
      expect(
        projectField(tester, 'project-name').controller!.text,
        'Preserved through failure',
      );
      expect(controller.canMutate, isFalse);
      api.listFailure = false;
      await projectTap(tester, 'refresh-projects');
      api.rejectMutation = true;
      await projectTap(tester, 'save-project');
      expect(
        projectField(tester, 'project-name').controller!.text,
        'Preserved through failure',
      );
      api.rejectMutation = false;
      if (!controller.canMutate) await projectTap(tester, 'refresh-projects');
      api.loseAcknowledgement = true;
      await projectTap(tester, 'save-project');
      expect(controller.uncertain, isTrue);
      expect(
        tester.widget<FilledButton>(projectKey('save-project')).onPressed,
        isNull,
      );
      expect(
        tester.widget<TextButton>(projectKey('delete-project')).onPressed,
        isNull,
      );
      final writes = List<String>.of(api.writes);
      await projectTap(tester, 'resolve-project-change');
      expect(api.writes, writes);
      expect(api.projectCalls, contains('project_receipt'));
      expect(
        projectField(tester, 'project-name').controller!.text,
        'Preserved through failure',
      );
      expect(
        api.calls.every(
          (op) => [
            'projects_list',
            'project_create',
            'project_update',
            'project_delete',
            'project_receipt',
          ].contains(op),
        ),
        isTrue,
      );
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );
}
