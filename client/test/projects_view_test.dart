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
const restartedProjectInstance = 'fedcba9876543210fedcba9876543210';

/// No sockets, repository access, providers, processes or inference.
class ProjectsUiApi extends FakeAppApi {
  ProjectsUiApi({bool empty = false}) {
    if (!empty) {
      records[alphaProject] = record(alphaProject, 'alpha');
      records[betaProject] = record(betaProject, 'beta');
    }
  }

  final records = <String, Map<String, dynamic>>{};
  final receipts = <String, Map<String, dynamic>>{};
  int projectRevision = 0;
  int nextId = 100;
  bool listFailure = false;
  bool rejectMutation = false;
  bool loseAcknowledgement = false;
  String missingReceiptCode = 'REQUEST_UNKNOWN';
  Completer<AppReply>? listGate;

  Map<String, dynamic> record(
    String id,
    String name, [
    List<ProjectRepository> repos = const [],
    String? key,
    String description = '',
    String prompt = '',
  ]) => {
    'project_id': id,
    'name': name,
    'key': key ?? name.toLowerCase().replaceAll(' ', '-'),
    'description': description,
    'prompt': prompt,
    'repositories': repos.map((r) => r.toJson()).toList(),
    'agent': 'opencode',
  };

  List<String> get projectCalls =>
      calls.where((op) => op.startsWith('project')).toList();
  List<String> get writes => projectCalls
      .where((op) => !['projects_list_v3', 'project_receipt'].contains(op))
      .toList();

  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    if (!op.startsWith('project')) return super.call(op, args);
    calls.add(op);
    arguments.add(Map.of(args));
    validateProjectArguments(op, args);
    if (op == 'projects_list_v3') {
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
        throw AppFailure(missingReceiptCode, 'No saved acknowledgement.');
      }
      return AppReply(instance, Map.of(receipt));
    }
    if (rejectMutation) {
      throw const AppFailure(
        'CAPACITY',
        'Project metadata could not be saved.',
      );
    }
    if (args['instance_id'] != instance) {
      throw const AppFailure('INSTANCE_MISMATCH', 'Refresh project metadata.');
    }
    if (args['expected_revision'] != projectRevision) {
      throw const AppFailure('STALE_REVISION', 'Refresh project metadata.');
    }
    final id = op == 'project_create_v3'
        ? (++nextId).toRadixString(16).padLeft(32, '0')
        : args['project_id'] as String;
    if (op != 'project_create_v3' && !records.containsKey(id)) {
      throw const AppFailure('NOT_FOUND', 'Project not found.');
    }
    if (op == 'project_update_v3' && args['name'] != records[id]!['name']) {
      throw const AppFailure(
        'INVALID_REQUEST',
        'Preserve the existing project compatibility name',
      );
    }
    if (op == 'project_delete_v3') {
      records.remove(id);
    } else {
      records[id] = record(
        id,
        args['name'],
        (args['repositories'] as List).map(ProjectRepository.parse).toList(),
        args['key'],
        args['description'],
        args['prompt'],
      );
    }
    final receipt = {
      'request_id': args['request_id'],
      'project_id': id,
      'revision': ++projectRevision,
      'deleted': op == 'project_delete_v3',
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

Future<void> loadProjectTestFonts() async {
  await (FontLoader(
    'MaterialIcons',
  )..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'))).load();
  await (FontLoader(SmithTheme.defaultFontFamily)..addFont(
        rootBundle.load(
          'assets/fonts/jetbrains-mono/JetBrainsMono-Regular.ttf',
        ),
      ))
      .load();
}

Finder projectKey(String key) => find.byKey(ValueKey(key));
Finder projectRefresh() => find.widgetWithText(TextButton, 'Refresh projects');
TextField projectField(WidgetTester tester, String key) =>
    tester.widget<TextField>(projectKey(key));

Future<void> projectTap(WidgetTester tester, String key) async {
  await tester.pumpAndSettle();
  if (key == 'refresh-projects') {
    final view = tester.widget<ProjectsView>(find.byType(ProjectsView));
    await view.controller.refresh();
    await tester.pumpAndSettle();
    return;
  }
  if (projectKey(key).evaluate().isEmpty) {
    if (key == 'delete-project' || key == 'duplicate-project') {
      await projectTap(tester, 'project-editor-menu');
    } else if (key.startsWith('remove-project-repository-')) {
      final index = key.substring('remove-project-repository-'.length);
      await projectTap(tester, 'project-repository-$index-menu');
    }
  }
  final target = key == 'refresh-projects' ? projectRefresh() : projectKey(key);
  await tester.ensureVisible(target);
  await tester.pumpAndSettle();
  await tester.tap(target);
  await tester.pumpAndSettle();
}

Future<void> expandProjectRepository(WidgetTester tester, int index) async {
  final key = 'expand-project-repository-$index';
  if (tester
      .widget<IconButton>(projectKey(key))
      .tooltip!
      .startsWith('Configure')) {
    await projectTap(tester, key);
  }
}

Future<void> writableProjectRepository(WidgetTester tester, int index) async {
  await expandProjectRepository(tester, index);
  final key = 'project-repository-$index-read-only';
  if (tester.widget<SwitchListTile>(projectKey(key)).value) {
    await projectTap(tester, key);
  }
}

Future<void> selectProjectType(
  WidgetTester tester,
  int index,
  String type,
) async {
  await writableProjectRepository(tester, index);
  final key = 'project-repository-$index-$type-selected';
  if (!tester.widget<CheckboxListTile>(projectKey(key)).value!) {
    await projectTap(tester, key);
  }
}

Future<void> discardProjectChanges(WidgetTester tester) async {
  await projectTap(tester, 'cancel-project');
  expect(find.text('Discard unsaved project changes?'), findsOneWidget);
  await tester.tap(find.text('Discard changes'));
  await tester.pumpAndSettle();
}

Future<void> chooseProject(WidgetTester tester, String id) async {
  if (projectKey('back-to-projects').evaluate().isNotEmpty) {
    await projectTap(tester, 'back-to-projects');
  }
  await projectTap(tester, 'project-$id');
}

Future<void> acceptProjectOriginReview(WidgetTester tester) async {
  await projectTap(tester, 'review-project-revision');
  await tester.tap(
    find.widgetWithText(TextButton, 'Keep edits at this revision'),
  );
  await tester.pumpAndSettle();
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
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(loadProjectTestFonts);

  ProjectRepository richRepository() => ProjectRepository(
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

  ProjectsUiApi richApi() {
    final api = ProjectsUiApi();
    api.records[alphaProject] = api.record(
      alphaProject,
      'Alpha compatibility',
      [richRepository()],
      'alpha',
      'Saved description',
      'Saved hidden prompt\n\tcontext',
    );
    return api;
  }

  testWidgets(
    'visible edits save while hidden prompt and branch values are preserved',
    (tester) async {
      final api = richApi();
      await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      await expandProjectRepository(tester, 0);
      expect(projectKey('project-prompt'), findsNothing);
      expect(projectKey('project-repository-0-base'), findsNothing);
      expect(projectKey('project-repository-0-feature'), findsNothing);
      await tester.enterText(projectKey('project-key'), 'alpha-updated');
      await tester.enterText(
        projectKey('project-description'),
        'Updated description',
      );
      await tester.enterText(
        projectKey('project-repository-0-name'),
        'backend-two',
      );
      await tester.enterText(
        projectKey('project-repository-0'),
        'https://example.invalid/updated.git',
      );
      await projectTap(tester, 'save-project');
      expect(api.writes, ['project_update_v3']);
      final saved = api.records[alphaProject]!;
      expect(saved['key'], 'alpha-updated');
      expect(saved['description'], 'Updated description');
      expect(saved['prompt'], 'Saved hidden prompt\n\tcontext');
      final repo = (saved['repositories'] as List).single as Map;
      expect(repo['name'], 'backend-two');
      expect(repo['url'], 'https://example.invalid/updated.git');
      expect(repo['branch']['base'], 'main');
      expect(repo['branch']['work'], 'work/next/*');
      expect(repo['branch']['types']['feature'], 'custom/features/*');
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets('invalid repository IDs and locators block Save without writes', (
    tester,
  ) async {
    final api = richApi();
    await mountProjects(tester, api);
    await chooseProject(tester, alphaProject);
    await expandProjectRepository(tester, 0);
    await tester.enterText(projectKey('project-repository-0-name'), '');
    await tester.pumpAndSettle();
    final save = tester.widget<FilledButton>(projectKey('save-project'));
    expect(save.onPressed, isNotNull);
    save.onPressed!();
    await tester.pumpAndSettle();
    expect(api.writes, isEmpty);
    expect(
      projectField(tester, 'project-repository-0-name').decoration!.errorText,
      contains('Enter a repository ID'),
    );
    await tester.enterText(
      projectKey('project-repository-0-name'),
      'backend-two',
    );
    await tester.enterText(projectKey('project-repository-0'), 'not a locator');
    await projectTap(tester, 'save-project');
    expect(api.writes, isEmpty);
    expect(
      projectField(tester, 'project-repository-0').decoration!.errorText,
      contains('https://host/path'),
    );
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('read-only is dirty, reviewed and saved', (tester) async {
    final api = richApi();
    await mountProjects(tester, api);
    await chooseProject(tester, alphaProject);
    await expandProjectRepository(tester, 0);
    final toggle = projectKey('project-repository-0-read-only');
    expect(tester.widget<SwitchListTile>(toggle).value, isFalse);
    await projectTap(tester, 'project-repository-0-read-only');
    await projectTap(tester, 'save-project');
    expect(api.writes, ['project_update_v3']);
    final repo = (api.records[alphaProject]!['repositories'] as List).single;
    expect(repo['read_only'], isTrue);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('external refresh requires explicit origin review before Save', (
    tester,
  ) async {
    final api = richApi();
    await mountProjects(tester, api);
    await chooseProject(tester, alphaProject);
    await tester.enterText(projectKey('project-key'), 'dirty-key');
    await tester.pumpAndSettle();
    final capturedSave = tester
        .widget<FilledButton>(projectKey('save-project'))
        .onPressed!;
    api.instance = restartedProjectInstance;
    api.projectRevision = 1;
    await tester
        .widget<ProjectsView>(find.byType(ProjectsView))
        .controller
        .refresh();
    await tester.pumpAndSettle();
    capturedSave();
    await tester.pumpAndSettle();
    expect(api.writes, isEmpty);
    expect(find.text('Unsaved changes — this view only'), findsOneWidget);
    expect(projectKey('review-project-revision'), findsOneWidget);
    await acceptProjectOriginReview(tester);
    await projectTap(tester, 'save-project');
    expect(api.writes, ['project_update_v3']);
    expect(api.records[alphaProject]!['key'], 'dirty-key');
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('delete binds the exact project and preserves the other', (
    tester,
  ) async {
    final api = richApi();
    await mountProjects(tester, api);
    await chooseProject(tester, alphaProject);
    await projectTap(tester, 'delete-project');
    expect(find.text('Delete project metadata?'), findsOneWidget);
    await tester.tap(find.text('Delete metadata'));
    await tester.pumpAndSettle();
    expect(api.records.containsKey(alphaProject), isFalse);
    expect(api.records.containsKey(betaProject), isTrue);
    expect(api.writes, ['project_delete_v3']);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('compact drill-in retains the same editor and draft', (
    tester,
  ) async {
    final api = richApi();
    await mountProjects(tester, api, size: const Size(400, 400));
    await chooseProject(tester, alphaProject);
    await tester.enterText(projectKey('project-key'), 'compact-draft');
    final state = tester.state(find.byType(ProjectsView));
    await projectTap(tester, 'back-to-projects');
    expect(projectKey('project-key'), findsNothing);
    await projectTap(tester, 'project-$alphaProject');
    expect(tester.state(find.byType(ProjectsView)), same(state));
    expect(
      projectField(tester, 'project-key').controller!.text,
      'compact-draft',
    );
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('new project stays blank and Cancel writes nothing', (
    tester,
  ) async {
    final api = richApi();
    await mountProjects(tester, api);
    await projectTap(tester, 'new-project');
    expect(find.text('New project · not saved'), findsOneWidget);
    expect(projectField(tester, 'project-key').controller!.text, isEmpty);
    await projectTap(tester, 'cancel-project');
    expect(api.writes, isEmpty);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('repository rows add and remove without touching hidden values', (
    tester,
  ) async {
    final api = richApi();
    await mountProjects(tester, api);
    await chooseProject(tester, alphaProject);
    await projectTap(tester, 'add-project-repository');
    expect(projectKey('expand-project-repository-1'), findsOneWidget);
    await projectTap(tester, 'project-repository-0-menu');
    await projectTap(tester, 'remove-project-repository-0');
    expect(projectKey('expand-project-repository-0'), findsOneWidget);
    expect(api.writes, isEmpty);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
    'lost create acknowledgement resolves query-only without a duplicate write',
    (tester) async {
      final api = ProjectsUiApi(empty: true)..loseAcknowledgement = true;
      final controller = await mountProjects(tester, api);
      await projectTap(tester, 'new-project');
      await tester.enterText(projectKey('project-key'), 'original-create');
      await tester.enterText(
        projectKey('project-description'),
        'Original context',
      );
      await projectTap(tester, 'save-project');
      expect(controller.uncertain, isTrue);
      final createdId = api.records.keys.single;
      await tester.enterText(projectKey('project-key'), 'newer-unsaved-edits');
      await tester.enterText(
        projectKey('project-description'),
        'Newer context',
      );
      await controller.resolvePending();
      await tester.pumpAndSettle();
      expect(controller.uncertain, isFalse);
      expect(
        projectField(tester, 'project-key').controller!.text,
        'newer-unsaved-edits',
      );
      expect(
        tester.widget<FilledButton>(projectKey('save-project')).onPressed,
        isNull,
      );
      expect(api.records.values.single['name'], 'original-create');
      expect(api.writes, ['project_create_v3']);
      expect(projectKey('review-project-revision'), findsOneWidget);
      await acceptProjectOriginReview(tester);
      api.loseAcknowledgement = false;
      await projectTap(tester, 'save-project');
      expect(controller.error, isNull);
      expect(controller.uncertain, isFalse);
      expect(api.records.keys.single, createdId);
      final saved = api.records.values.single;
      expect(saved['name'], 'original-create');
      expect(saved['key'], 'newer-unsaved-edits');
      expect(saved['description'], 'Newer context');
      expect(api.writes, ['project_create_v3', 'project_update_v3']);
      expect(find.text('Unsaved changes — this view only'), findsNothing);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'failed refresh preserves input and blocks writes until recovery',
    (tester) async {
      final api = ProjectsUiApi();
      final controller = await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      await tester.enterText(
        projectKey('project-key'),
        'preserved-through-failure',
      );
      api.listFailure = true;
      await projectTap(tester, 'refresh-projects');
      expect(
        projectField(tester, 'project-key').controller!.text,
        'preserved-through-failure',
      );
      expect(controller.canMutate, isFalse);
      api.listFailure = false;
      await projectTap(tester, 'refresh-projects');
      api.rejectMutation = true;
      await projectTap(tester, 'save-project');
      expect(
        projectField(tester, 'project-key').controller!.text,
        'preserved-through-failure',
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
      final writes = List<String>.of(api.writes);
      await projectTap(tester, 'resolve-project-change');
      expect(api.writes, writes);
      expect(api.projectCalls, contains('project_receipt'));
      expect(
        projectField(tester, 'project-key').controller!.text,
        'preserved-through-failure',
      );
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );
}
