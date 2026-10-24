import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/projects.dart';
import 'package:smith_client/projects_controller.dart';
import 'package:smith_client/ui/projects_view.dart';
import 'package:smith_client/ui/smith_icon.dart';
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
  const fieldNames = [
    'name',
    'url',
    'base',
    'work',
    'feature',
    'fix',
    'release',
    'hotfix',
  ];
  const configured = [
    'backend',
    'git@example.invalid:team/backend.git',
    'main',
    'main',
    'feature/*',
    'fix/*',
    'release/*',
    'hotfix/*',
  ];
  String fieldKey(int i) =>
      i == 1 ? 'project-repository-0' : 'project-repository-0-${fieldNames[i]}';
  ProjectRepository repository(List<String> values) => ProjectRepository(
    readOnly: false,
    name: values[0],
    url: values[1],
    branch: ProjectBranches(
      base: values[2],
      work: values[3],
      types: ProjectBranchTypes(
        feature: values[4],
        fix: values[5],
        release: values[6],
        hotfix: values[7],
      ),
    ),
  );

  testWidgets(
    'ID validates required kebab syntax, duplicate IDs and 1/40/41 bounds inline',
    (tester) async {
      final api = ProjectsUiApi();
      await mountProjects(tester, api);
      await projectTap(tester, 'new-project');
      for (final value in [
        '',
        '-alpha',
        'alpha-',
        'two--parts',
        'UPPER',
        'not_valid',
        List.filled(41, 'a').join(),
      ]) {
        await tester.enterText(projectKey('project-key'), value);
        await projectTap(tester, 'save-project');
        expect(
          projectField(tester, 'project-key').decoration!.errorText,
          projectKeyError(value),
        );
        expect(projectField(tester, 'project-key').controller!.text, value);
        expect(api.writes, isEmpty);
      }
      await tester.enterText(projectKey('project-key'), 'alpha');
      await projectTap(tester, 'save-project');
      expect(
        projectField(tester, 'project-key').decoration!.errorText,
        'A project with this ID already exists.',
      );
      expect(api.writes, isEmpty);
      await tester.enterText(projectKey('project-key'), '0');
      await projectTap(tester, 'save-project');
      final created = api.records.values.singleWhere((p) => p['key'] == '0');
      final id = created['project_id'];
      final forty = List.filled(40, 'a').join();
      await tester.enterText(projectKey('project-key'), forty);
      await projectTap(tester, 'save-project');
      expect(api.records[id]!['key'], forty);
      expect(api.records[id]!['name'], '0');
      expect(api.records[id]!['description'], isEmpty);
      expect(api.records[id]!['prompt'], isEmpty);
      expect(api.writes, ['project_create_v3', 'project_update_v3']);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'Unicode scalar counters and multiline IME input preserve exact 60/1000 text and reject 61/1001',
    (tester) async {
      final api = ProjectsUiApi(empty: true);
      await mountProjects(tester, api);
      await projectTap(tester, 'new-project');
      await tester.enterText(projectKey('project-key'), 'unicode-context');
      final description = List.filled(60, '😀').join();
      final prompt = ' \n\t${List.filled(996, '😀').join()}\n';
      expect(prompt.runes.length, 1000);
      await tester.enterText(
        projectKey('project-description'),
        '$description😀',
      );
      await tester.enterText(projectKey('project-prompt'), '$prompt😀');
      await projectTap(tester, 'save-project');
      expect(
        projectField(tester, 'project-description').decoration!.counterText,
        '61/60',
      );
      expect(
        projectField(tester, 'project-prompt').decoration!.counterText,
        '1001/1000',
      );
      expect(
        projectField(tester, 'project-description').decoration!.errorText,
        isNotNull,
      );
      expect(
        projectField(tester, 'project-prompt').decoration!.errorText,
        isNotNull,
      );
      expect(
        projectField(tester, 'project-prompt').controller!.text,
        '$prompt😀',
      );
      expect(api.writes, isEmpty);
      await tester.enterText(projectKey('project-description'), description);
      await tester.enterText(projectKey('project-prompt'), prompt);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      tester.testTextInput.updateEditingValue(
        TextEditingValue(
          text: prompt,
          selection: TextSelection.collapsed(offset: prompt.length),
          composing: const TextRange(start: 0, end: 2),
        ),
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(api.writes, isEmpty);
      expect(projectField(tester, 'project-prompt').maxLines, isNull);
      expect(
        projectField(tester, 'project-prompt').textInputAction,
        TextInputAction.newline,
      );
      expect(
        projectField(tester, 'project-description').decoration!.counterText,
        '60/60',
      );
      expect(
        projectField(tester, 'project-prompt').decoration!.counterText,
        '1000/1000',
      );
      await projectTap(tester, 'save-project');
      expect(api.records.values.single['description'], description);
      expect(api.records.values.single['prompt'], prompt);
      expect(api.writes, ['project_create_v3']);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'description line separators and prompt controls remain invalid raw drafts',
    (tester) async {
      final api = ProjectsUiApi(empty: true);
      await mountProjects(tester, api);
      await projectTap(tester, 'new-project');
      await tester.enterText(projectKey('project-key'), 'invalid-context');
      for (final separator in ['\u2028', '\u2029']) {
        final description = 'one${separator}two';
        await tester.enterText(projectKey('project-description'), description);
        await projectTap(tester, 'save-project');
        expect(
          projectField(tester, 'project-description').controller!.text,
          description,
        );
        expect(
          projectField(tester, 'project-description').decoration!.errorText,
          projectDescriptionError(description),
        );
      }
      await tester.enterText(projectKey('project-description'), '');
      await tester.enterText(projectKey('project-prompt'), 'bad\u0001text');
      await projectTap(tester, 'save-project');
      expect(
        projectField(tester, 'project-prompt').decoration!.errorText,
        projectPromptError('bad\u0001text'),
      );
      expect(api.writes, isEmpty);
      await tester.enterText(projectKey('project-prompt'), '');
      await projectTap(tester, 'save-project');
      expect(api.records.values.single['description'], isEmpty);
      expect(api.records.values.single['prompt'], isEmpty);
      expect(api.writes, ['project_create_v3']);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  for (final field in ['key', 'description', 'prompt']) {
    testWidgets(
      '$field alone is dirty and participates in same-revision all-field review',
      (tester) async {
        final api = ProjectsUiApi();
        api.records[alphaProject] = api.record(
          alphaProject,
          'Hidden legacy name',
          [],
          'alpha',
          'Saved description',
          'Saved\ncontext',
        );
        await mountProjects(tester, api);
        await chooseProject(tester, alphaProject);
        final saved = {
          'key': 'alpha',
          'description': 'Saved description',
          'prompt': 'Saved\ncontext',
        };
        final changed = {
          ...saved,
          field: field == 'key' ? 'alpha-edited' : 'Edited\tcontext',
        };
        if (field == 'description') changed[field] = 'Edited description';
        await tester.enterText(projectKey('project-$field'), changed[field]!);
        await tester.pumpAndSettle();
        expect(find.text('Unsaved changes — this view only'), findsOneWidget);
        final oldSave = tester
            .widget<FilledButton>(projectKey('save-project'))
            .onPressed!;
        api.instance = restartedProjectInstance;
        await projectTap(tester, 'refresh-projects');
        oldSave();
        await tester.pumpAndSettle();
        expect(api.writes, isEmpty);
        await projectTap(tester, 'review-project-revision');
        final dialog = find.byType(AlertDialog);
        for (final (name, label) in [
          ('key', 'ID'),
          ('description', 'Description'),
          ('prompt', 'Prompt'),
        ]) {
          expect(
            find.descendant(
              of: dialog,
              matching: find.text('$label: ${saved[name]}'),
            ),
            findsNWidgets(saved[name] == changed[name] ? 2 : 1),
          );
          expect(
            find.descendant(
              of: dialog,
              matching: find.text('$label: ${changed[name]}'),
            ),
            findsNWidgets(saved[name] == changed[name] ? 2 : 1),
          );
        }
        await tester.tap(find.text('Keep edits at this revision'));
        await tester.pumpAndSettle();
        expect(api.writes, isEmpty);
        await projectTap(tester, 'save-project');
        for (final entry in changed.entries) {
          expect(api.records[alphaProject]![entry.key], entry.value);
        }
        expect(api.records[alphaProject]!['name'], 'Hidden legacy name');
        expect(find.text('Unsaved changes — this view only'), findsNothing);
        expect(api.writes, ['project_update_v3']);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox());
      },
    );
  }

  testWidgets(
    'Duplicate guards dirty replacement and copies saved metadata into an independent unsaved draft with cleared ID',
    (tester) async {
      final api = ProjectsUiApi();
      final sourceRepository = ProjectRepository(
        name: 'backend',
        url: configured[1],
        branch: ProjectBranches(
          base: 'stable',
          work: 'shared/*',
          types: ProjectBranchTypes(
            feature: 'custom/features/*',
            hotfix: 'urgent/*',
          ),
        ),
      );
      final source = api.record(
        alphaProject,
        'Preserved legacy name',
        [
          sourceRepository,
          ProjectRepository(
            url: 'https://example.invalid/team/client',
            readOnly: false,
          ),
        ],
        'alpha',
        'Saved description',
        'Saved\n\tcontext\n',
      );
      api.records[alphaProject] = source;
      await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      await tester.enterText(
        projectKey('project-description'),
        'Unsaved source edit',
      );
      await projectTap(tester, 'duplicate-project');
      expect(find.text('Discard unsaved project changes?'), findsOneWidget);
      await tester.tap(find.text('Keep editing'));
      await tester.pumpAndSettle();
      expect(
        projectField(tester, 'project-description').controller!.text,
        'Unsaved source edit',
      );
      await projectTap(tester, 'duplicate-project');
      await tester.tap(find.text('Discard changes'));
      await tester.pumpAndSettle();
      expect(find.text('New Project · not saved'), findsOneWidget);
      expect(projectField(tester, 'project-key').controller!.text, isEmpty);
      expect(
        projectField(tester, 'project-description').controller!.text,
        source['description'],
      );
      expect(
        projectField(tester, 'project-prompt').controller!.text,
        source['prompt'],
      );
      expect(api.writes, isEmpty);
      await projectTap(tester, 'save-project');
      expect(
        projectField(tester, 'project-key').decoration!.errorText,
        isNotNull,
      );
      expect(api.writes, isEmpty);
      await tester.enterText(projectKey('project-key'), 'alpha-copy');
      await expandProjectRepository(tester, 0);
      expect(
        tester
            .widget<SwitchListTile>(
              projectKey('project-repository-0-read-only'),
            )
            .value,
        isTrue,
      );
      expect(projectKey('project-repository-0-base'), findsNothing);
      await tester.enterText(
        projectKey('project-repository-0-name'),
        'copy-backend',
      );
      await projectTap(tester, 'save-project');
      final copy = api.records.values.singleWhere(
        (p) => p['key'] == 'alpha-copy',
      );
      expect(copy['project_id'], isNot(alphaProject));
      expect(copy['name'], source['name']);
      expect(copy['description'], source['description']);
      expect(copy['prompt'], source['prompt']);
      expect(copy['repositories'], [
        {...sourceRepository.toJson(), 'name': 'copy-backend'},
        (source['repositories'] as List)[1],
      ]);
      expect(api.records[alphaProject], source);
      expect(api.writes, ['project_create_v3']);
      expect(
        api.arguments.singleWhere((a) => a.containsKey('expected_revision')),
        isNot(contains('project_id')),
      );
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'Read Only hides controls but preserves custom patterns, null toggles and working prefixes through Save',
    (tester) async {
      final api = ProjectsUiApi();
      final initial = ProjectRepository(
        name: 'backend',
        url: configured[1],
        readOnly: false,
        branch: ProjectBranches(
          base: 'stable',
          work: 'shared/*',
          types: ProjectBranchTypes(
            feature: 'custom/features/*',
            release: 'custom/releases/*',
            hotfix: 'urgent/*',
          ),
        ),
      );
      api.records[alphaProject] = api.record(alphaProject, 'alpha', [initial]);
      await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      expect(projectKey('project-repository-0-name'), findsNothing);
      await expandProjectRepository(tester, 0);
      expect(
        tester
            .widget<CheckboxListTile>(
              projectKey('project-repository-0-feature-selected'),
            )
            .value,
        isTrue,
      );
      expect(
        tester
            .widget<CheckboxListTile>(
              projectKey('project-repository-0-fix-selected'),
            )
            .value,
        isFalse,
      );
      expect(projectKey('project-repository-0-fix'), findsNothing);
      expect(api.writes, isEmpty);
      expect(find.text('Unsaved changes — this view only'), findsNothing);
      final pattern = projectField(
        tester,
        'project-repository-0-feature',
      ).controller!;
      pattern.selection = const TextSelection(baseOffset: 1, extentOffset: 5);
      await tester.enterText(
        projectKey('project-repository-0-work'),
        'shared/work/*',
      );
      await projectTap(tester, 'project-repository-0-read-only');
      expect(projectKey('project-repository-0-base'), findsNothing);
      expect(projectKey('project-repository-0-feature'), findsNothing);
      expect(
        tester
            .widget<TextField>(
              find.byKey(
                const ValueKey('project-repository-0-feature'),
                skipOffstage: false,
              ),
            )
            .controller,
        same(pattern),
      );
      expect(pattern.text, 'custom/features/*');
      expect(
        pattern.selection,
        const TextSelection(baseOffset: 1, extentOffset: 5),
      );
      await projectTap(tester, 'save-project');
      final expectedBranch = {
        ...initial.branch.toJson(),
        'work': 'shared/work/*',
      };
      expect((api.records[alphaProject]!['repositories'] as List).single, {
        ...initial.toJson(),
        'read_only': true,
        'branch': expectedBranch,
      });
      await expandProjectRepository(tester, 0);
      expect(projectKey('project-repository-0-work'), findsNothing);
      await writableProjectRepository(tester, 0);
      expect(
        projectField(tester, 'project-repository-0-work').controller!.text,
        'shared/work/*',
      );
      expect(
        projectField(tester, 'project-repository-0-feature').controller!.text,
        'custom/features/*',
      );
      expect(
        tester
            .widget<CheckboxListTile>(
              projectKey('project-repository-0-fix-selected'),
            )
            .value,
        isFalse,
      );
      await projectTap(tester, 'project-repository-0-fix-selected');
      expect(
        projectField(tester, 'project-repository-0-fix').controller!.text,
        'fix/*',
      );
      await projectTap(tester, 'project-repository-0-release-selected');
      expect(projectKey('project-repository-0-release'), findsNothing);
      await projectTap(tester, 'save-project');
      final saved = (api.records[alphaProject]!['repositories'] as List).single;
      expect(saved['read_only'], isFalse);
      expect(saved['branch']['types'], {
        'feature': 'custom/features/*',
        'fix': 'fix/*',
        'release': null,
        'hotfix': 'urgent/*',
      });
      expect(api.writes, ['project_update_v3', 'project_update_v3']);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'invalid retained writable values remain reachable when Read Only and repository disclosure hide them',
    (tester) async {
      final api = ProjectsUiApi();
      api.records[alphaProject] = api.record(alphaProject, 'alpha', [
        repository(configured),
      ]);
      await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      await expandProjectRepository(tester, 0);
      await tester.enterText(projectKey('project-repository-0-base'), 'HEAD');
      await tester.enterText(
        projectKey('project-repository-0-feature'),
        'bad/**',
      );
      await projectTap(tester, 'project-repository-0-read-only');
      await projectTap(tester, 'expand-project-repository-0');
      await projectTap(tester, 'save-project');
      expect(
        find.text(
          'Retained branch settings are invalid. Turn off Read Only to correct them.',
        ),
        findsOneWidget,
      );
      expect(
        tester
            .widget<SwitchListTile>(
              projectKey('project-repository-0-read-only'),
            )
            .value,
        isTrue,
      );
      expect(projectKey('project-repository-0-base'), findsNothing);
      expect(api.writes, isEmpty);
      await writableProjectRepository(tester, 0);
      expect(
        projectField(tester, 'project-repository-0-base').controller!.text,
        'HEAD',
      );
      expect(
        projectField(tester, 'project-repository-0-feature').controller!.text,
        'bad/**',
      );
      expect(
        projectField(tester, 'project-repository-0-base').decoration!.errorText,
        isNotNull,
      );
      expect(
        projectField(
          tester,
          'project-repository-0-feature',
        ).decoration!.errorText,
        isNotNull,
      );
      expect(api.writes, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'collapsed repository overflow removes only its captured draft row and stale callbacks cannot affect another project',
    (tester) async {
      final api = ProjectsUiApi();
      final first = ProjectRepository(name: 'backend', url: configured[1]);
      final second = ProjectRepository(
        name: 'frontend',
        url: 'https://example.invalid/team/frontend',
      );
      api.records[alphaProject] = api.record(alphaProject, 'alpha', [
        first,
        second,
      ]);
      api.records[betaProject] = api.record(betaProject, 'beta', [second]);
      await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      expect(projectKey('project-repository-0-name'), findsNothing);
      final oldRemove = tester
          .widget<PopupMenuButton<String>>(
            projectKey('project-repository-0-menu'),
          )
          .onSelected!;
      await projectTap(tester, 'remove-project-repository-0');
      expect(find.text('backend'), findsNothing);
      expect(find.text('frontend'), findsOneWidget);
      expect(api.writes, isEmpty);
      expect((api.records[alphaProject]!['repositories'] as List), [
        first.toJson(),
        second.toJson(),
      ]);
      expect(
        tester
            .widget<TextButton>(projectKey('add-project-repository'))
            .focusNode!
            .hasFocus,
        isTrue,
      );
      await discardProjectChanges(tester);
      await chooseProject(tester, betaProject);
      oldRemove('remove');
      await tester.pumpAndSettle();
      expect(find.text('frontend'), findsOneWidget);
      expect(projectKey('expand-project-repository-0'), findsOneWidget);
      expect(api.writes, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'read-only alone is dirty, origin-fenced and included in explicit review',
    (tester) async {
      final api = ProjectsUiApi();
      final initial = ProjectRepository(
        url: configured[1],
        branch: ProjectBranches(
          base: 'main',
          work: 'custom/*',
          types: ProjectBranchTypes(feature: 'custom/features/*'),
        ),
      );
      api.records[alphaProject] = api.record(alphaProject, 'alpha', [initial]);
      await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      await writableProjectRepository(tester, 0);
      expect(find.text('Unsaved changes — this view only'), findsOneWidget);
      api.instance = restartedProjectInstance;
      await projectTap(tester, 'refresh-projects');
      expect(
        tester.widget<FilledButton>(projectKey('save-project')).onPressed,
        isNull,
      );
      await projectTap(tester, 'review-project-revision');
      final dialog = find.byType(AlertDialog);
      expect(
        find.descendant(of: dialog, matching: find.text('Read Only: Yes')),
        findsOneWidget,
      );
      expect(
        find.descendant(of: dialog, matching: find.text('Read Only: No')),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: dialog,
          matching: find.text('Feature pattern (optional): custom/features/*'),
        ),
        findsNWidgets(2),
      );
      await tester.tap(find.text('Keep edits at this revision'));
      await tester.pumpAndSettle();
      expect(api.writes, isEmpty);
      await projectTap(tester, 'save-project');
      expect((api.records[alphaProject]!['repositories'] as List).single, {
        ...initial.toJson(),
        'read_only': false,
      });
      expect(api.writes, ['project_update_v3']);
      expect(find.text('Unsaved changes — this view only'), findsNothing);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'unknown all-field submission is immutable and receipt-only recovery preserves newer context and writable values',
    (tester) async {
      final api = ProjectsUiApi()..loseAcknowledgement = true;
      final initial = ProjectRepository(
        name: 'backend',
        url: configured[1],
        branch: ProjectBranches(
          base: 'main',
          work: 'shared/*',
          types: ProjectBranchTypes(feature: 'custom/features/*'),
        ),
      );
      api.records[alphaProject] = api.record(
        alphaProject,
        'Hidden original name',
        [initial],
        'alpha',
        'Saved description',
        'Saved\ncontext',
      );
      final controller = await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      await tester.enterText(projectKey('project-key'), 'alpha-submitted');
      await tester.enterText(
        projectKey('project-description'),
        'Submitted description',
      );
      await tester.enterText(
        projectKey('project-prompt'),
        'Submitted\n\tcontext\n',
      );
      await projectTap(tester, 'save-project');
      expect(controller.uncertain, isTrue);
      final captured = api.arguments.singleWhere(
        (a) => a.containsKey('expected_revision'),
      );
      final request = captured['request_id'];
      final capturedRepository =
          (captured['repositories'] as List).single as Map;
      expect(
        () => capturedRepository['read_only'] = false,
        throwsUnsupportedError,
      );
      await tester.enterText(
        projectKey('project-description'),
        'Newer description',
      );
      await tester.enterText(projectKey('project-prompt'), 'Newer\ncontext\n');
      await writableProjectRepository(tester, 0);
      await tester.enterText(
        projectKey('project-repository-0-work'),
        'newer/work/*',
      );
      await tester.enterText(
        projectKey('project-repository-0-feature'),
        'newer/features/*',
      );
      expect(captured['description'], 'Submitted description');
      expect(captured['prompt'], 'Submitted\n\tcontext\n');
      expect(capturedRepository, initial.toJson());
      expect(
        tester
            .widget<PopupMenuButton<String>>(projectKey('project-editor-menu'))
            .enabled,
        isFalse,
      );
      final before = api.projectCalls.length;
      await projectTap(tester, 'resolve-project-change');
      expect(api.projectCalls.sublist(before), [
        'project_receipt',
        'projects_list_v3',
      ]);
      expect(api.arguments[api.calls.indexOf('project_receipt')], {
        'request_id': request,
      });
      expect(controller.uncertain, isFalse);
      expect(
        projectField(tester, 'project-key').controller!.text,
        'alpha-submitted',
      );
      expect(
        projectField(tester, 'project-description').controller!.text,
        'Newer description',
      );
      expect(
        projectField(tester, 'project-prompt').controller!.text,
        'Newer\ncontext\n',
      );
      expect(
        projectField(tester, 'project-repository-0-work').controller!.text,
        'newer/work/*',
      );
      expect(
        projectField(tester, 'project-repository-0-feature').controller!.text,
        'newer/features/*',
      );
      expect(
        tester.widget<FilledButton>(projectKey('save-project')).onPressed,
        isNull,
      );
      expect(api.writes, ['project_update_v3']);
      await projectTap(tester, 'review-project-revision');
      final dialog = find.byType(AlertDialog);
      for (final value in [
        'Description: Submitted description',
        'Description: Newer description',
        'Prompt: Submitted\n\tcontext\n',
        'Prompt: Newer\ncontext\n',
        'Read Only: Yes',
        'Read Only: No',
        'Work branch (optional): shared/*',
        'Work branch (optional): newer/work/*',
      ]) {
        expect(
          find.descendant(of: dialog, matching: find.text(value)),
          findsOneWidget,
        );
      }
      await tester.tap(find.text('Keep edits at this revision'));
      await tester.pumpAndSettle();
      api.loseAcknowledgement = false;
      await projectTap(tester, 'save-project');
      final saved = api.records[alphaProject]!;
      expect(saved['name'], 'Hidden original name');
      expect(saved['description'], 'Newer description');
      expect(saved['prompt'], 'Newer\ncontext\n');
      expect((saved['repositories'] as List).single['read_only'], isFalse);
      expect(
        (saved['repositories'] as List).single['branch']['work'],
        'newer/work/*',
      );
      expect(api.writes, ['project_update_v3', 'project_update_v3']);
      final requests = api.arguments
          .where((a) => a.containsKey('expected_revision'))
          .map((a) => a['request_id'])
          .toSet();
      expect(requests, hasLength(2));
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'blank defaults, independent optional settings, exact structured save and clear',
    (tester) async {
      final api = ProjectsUiApi(empty: true);
      await mountProjects(tester, api);
      await projectTap(tester, 'new-project');
      await tester.enterText(projectKey('project-key'), 'structured');
      await projectTap(tester, 'add-project-repository');
      expect(projectField(tester, fieldKey(0)).focusNode!.hasFocus, isTrue);
      expect(projectKey(fieldKey(4)), findsNothing);
      expect(
        tester
            .widget<SwitchListTile>(
              projectKey('project-repository-0-read-only'),
            )
            .value,
        isTrue,
      );
      for (var i = 0; i < 2; i++) {
        expect(projectField(tester, fieldKey(i)).controller!.text, isEmpty);
      }
      await writableProjectRepository(tester, 0);
      for (var i = 0; i < 8; i++) {
        if (i >= 4) await selectProjectType(tester, 0, fieldNames[i]);
        expect(
          projectField(tester, fieldKey(i)).controller!.text,
          i >= 4 ? configured[i] : isEmpty,
        );
        await tester.enterText(projectKey(fieldKey(i)), configured[i]);
      }
      expect(api.writes, isEmpty);
      await projectTap(tester, 'save-project');
      expect(api.records.values.single['repositories'], [
        repository(configured).toJson(),
      ]);
      expect(find.text('Unsaved changes — this view only'), findsNothing);
      expect(projectKey(fieldKey(0)), findsNothing);
      await expandProjectRepository(tester, 0);
      for (var i = 0; i < 8; i++) {
        if (i >= 4) {
          await projectTap(
            tester,
            'project-repository-0-${fieldNames[i]}-selected',
          );
        } else if (i != 1) {
          await tester.enterText(projectKey(fieldKey(i)), '');
        }
      }
      await projectTap(tester, 'save-project');
      expect(api.records.values.single['repositories'], [
        ProjectRepository(url: configured[1], readOnly: false).toJson(),
      ]);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  for (var field = 0; field < 8; field++) {
    testWidgets(
      '${fieldNames[field]} alone is dirty, origin-fenced, reviewed, saved and reconciled',
      (tester) async {
        final api = ProjectsUiApi();
        api.records[alphaProject] = api.record(alphaProject, 'alpha', [
          repository(configured),
        ]);
        await mountProjects(tester, api);
        await chooseProject(tester, alphaProject);
        await expandProjectRepository(tester, 0);
        final changed = field == 1
            ? 'https://example.invalid/changed'
            : field >= 4
            ? 'changed/*'
            : 'changed';
        await tester.enterText(projectKey(fieldKey(field)), changed);
        await tester.pumpAndSettle();
        expect(find.text('Unsaved changes — this view only'), findsOneWidget);
        final capturedSave = tester
            .widget<FilledButton>(projectKey('save-project'))
            .onPressed!;
        api.instance = restartedProjectInstance;
        await projectTap(tester, 'refresh-projects');
        capturedSave();
        await tester.pumpAndSettle();
        expect(api.writes, isEmpty);
        expect(projectField(tester, fieldKey(field)).controller!.text, changed);
        await projectTap(tester, 'review-project-revision');
        final dialog = find.byType(AlertDialog);
        for (final label in [
          'Name (optional)',
          'URL (required)',
          'Base branch (optional)',
          'Work branch (optional)',
          'Feature pattern (optional)',
          'Fix pattern (optional)',
          'Release pattern (optional)',
          'Hotfix pattern (optional)',
        ]) {
          expect(
            find.descendant(
              of: dialog,
              matching: find.textContaining('$label:'),
            ),
            findsNWidgets(2),
          );
        }
        await tester.tap(
          find.widgetWithText(TextButton, 'Keep edits at this revision'),
        );
        await tester.pumpAndSettle();
        await projectTap(tester, 'save-project');
        final expected = List<String>.of(configured)..[field] = changed;
        expect(api.records[alphaProject]!['repositories'], [
          repository(expected).toJson(),
        ]);
        expect(find.text('Unsaved changes — this view only'), findsNothing);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox());
      },
    );
  }

  testWidgets(
    'invalid raw drafts never construct models; Save reveals hidden type errors',
    (tester) async {
      final api = ProjectsUiApi();
      api.records[alphaProject] = api.record(alphaProject, 'alpha', [
        ProjectRepository(url: configured[1]),
      ]);
      await mountProjects(tester, api, size: const Size(400, 400), scale: 2);
      await chooseProject(tester, alphaProject);
      await selectProjectType(tester, 0, 'feature');
      await tester.enterText(projectKey(fieldKey(4)), 'bad/**');
      await tester.enterText(projectKey(fieldKey(2)), 'HEAD');
      await projectTap(tester, 'expand-project-repository-0');
      expect(projectKey(fieldKey(4)), findsNothing);
      await projectTap(tester, 'save-project');
      expect(projectKey(fieldKey(4)), findsOneWidget);
      expect(find.text(projectBranchTypeError('bad/**')!), findsOneWidget);
      expect(find.text(projectBranchNameError('HEAD')!), findsOneWidget);
      expect(api.writes, isEmpty);
      expect(tester.takeException(), isNull);
      await discardProjectChanges(tester);
      await writableProjectRepository(tester, 0);
      expect(projectField(tester, fieldKey(2)).controller!.text, isEmpty);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'nested uncertain submission is immutable and preserves newer type-only input',
    (tester) async {
      final api = ProjectsUiApi()..loseAcknowledgement = true;
      api.records[alphaProject] = api.record(alphaProject, 'alpha', [
        repository(configured),
      ]);
      final controller = await mountProjects(tester, api);
      await chooseProject(tester, alphaProject);
      await expandProjectRepository(tester, 0);
      await tester.enterText(projectKey(fieldKey(7)), 'submitted/*');
      await projectTap(tester, 'save-project');
      expect(controller.uncertain, isTrue);
      await tester.enterText(projectKey(fieldKey(7)), 'newer/*');
      await controller.resolvePending();
      await tester.pumpAndSettle();
      expect(projectField(tester, fieldKey(7)).controller!.text, 'newer/*');
      expect(
        (api.records[alphaProject]!['repositories'] as List)
            .single['branch']['types']['hotfix'],
        'submitted/*',
      );
      expect(
        tester.widget<FilledButton>(projectKey('save-project')).onPressed,
        isNull,
      );
      expect(api.writes, ['project_update_v3']);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'repository controls have 48px targets and keyboard-accessible disclosure',
    (tester) async {
      final api = ProjectsUiApi(empty: true);
      await mountProjects(tester, api, size: const Size(400, 400), scale: 2);
      await projectTap(tester, 'new-project');
      await projectTap(tester, 'add-project-repository');
      await writableProjectRepository(tester, 0);
      for (final key in [
        'add-project-repository',
        'project-repository-0-menu',
        'expand-project-repository-0',
        'project-repository-0-read-only',
        'project-repository-0-feature-selected',
        'save-project',
        'cancel-project',
      ]) {
        await tester.ensureVisible(projectKey(key));
        await tester.pumpAndSettle();
        final size = tester.getSize(projectKey(key));
        expect(size.width, greaterThanOrEqualTo(48));
        expect(size.height, greaterThanOrEqualTo(48));
        expect(projectKey(key).hitTestable(), findsOneWidget);
      }
      final disclosure = projectKey('expand-project-repository-0');
      await projectTap(tester, 'expand-project-repository-0');
      expect(projectKey(fieldKey(0)), findsNothing);
      Focus.of(
        tester.element(
          find.descendant(of: disclosure, matching: find.byType(SmithIcon)),
        ),
      ).requestFocus();
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(projectKey(fieldKey(0)), findsOneWidget);
      final toggle = projectKey('project-repository-0-feature-selected');
      await tester.ensureVisible(toggle);
      Focus.of(
        tester.element(
          find.descendant(of: toggle, matching: find.text('Feature')),
        ),
      ).requestFocus();
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.space);
      await tester.pumpAndSettle();
      expect(projectField(tester, fieldKey(4)).controller!.text, 'feature/*');
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pumpAndSettle();
      expect(
        tester
            .widget<EditableText>(
              find.descendant(
                of: projectKey(fieldKey(4)),
                matching: find.byType(EditableText),
              ),
            )
            .focusNode
            .hasFocus,
        isTrue,
      );
      await projectTap(tester, 'project-repository-0-menu');
      final remove = find.ancestor(
        of: projectKey('remove-project-repository-0'),
        matching: find.byType(PopupMenuItem<String>),
      );
      expect(tester.getSize(remove).height, greaterThanOrEqualTo(48));
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(api.writes, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'name duplicates are exact case-sensitive; remove and cancel are draft only',
    (tester) async {
      final api = ProjectsUiApi(empty: true);
      await mountProjects(tester, api);
      await projectTap(tester, 'new-project');
      await tester.enterText(projectKey('project-key'), 'duplicates');
      for (var i = 0; i < 2; i++) {
        await projectTap(tester, 'add-project-repository');
        await tester.enterText(
          projectKey('project-repository-$i-name'),
          'backend',
        );
        await tester.enterText(
          projectKey('project-repository-$i'),
          'https://example.invalid/repo-$i',
        );
      }
      await projectTap(tester, 'save-project');
      expect(
        find.text('Repository names must be distinct (case-sensitive).'),
        findsNWidgets(2),
      );
      expect(api.writes, isEmpty);
      await tester.enterText(
        projectKey('project-repository-1-name'),
        'Backend',
      );
      await projectTap(tester, 'save-project');
      expect(api.writes, ['project_create_v3']);
      await projectTap(tester, 'remove-project-repository-0');
      expect(
        tester
            .widget<TextButton>(projectKey('add-project-repository'))
            .focusNode!
            .hasFocus,
        isTrue,
      );
      await expandProjectRepository(tester, 0);
      expect(
        projectField(tester, 'project-repository-0-name').controller!.text,
        'Backend',
      );
      await discardProjectChanges(tester);
      await expandProjectRepository(tester, 0);
      expect(
        projectField(tester, 'project-repository-0-name').controller!.text,
        'backend',
      );
      expect(api.writes, ['project_create_v3']);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  for (final creating in [false, true]) {
    testWidgets(
      'same-revision restart preserves ${creating ? 'new' : 'existing'} dirty editor until explicit origin review',
      (tester) async {
        final api = ProjectsUiApi()..projectRevision = 2;
        final controller = await mountProjects(tester, api);
        if (creating) {
          await projectTap(tester, 'new-project');
        } else {
          await chooseProject(tester, alphaProject);
        }
        await tester.enterText(
          projectKey('project-key'),
          'local-edits-before-restart',
        );
        await tester.pumpAndSettle();
        final staleSave = tester
            .widget<FilledButton>(projectKey('save-project'))
            .onPressed!;
        api.instance = restartedProjectInstance;
        await projectTap(tester, 'refresh-projects');
        expect(controller.instanceId, restartedProjectInstance);
        expect(controller.revision, 2);
        expect(
          projectField(tester, 'project-key').controller!.text,
          'local-edits-before-restart',
        );
        expect(
          tester.widget<FilledButton>(projectKey('save-project')).onPressed,
          isNull,
        );
        if (!creating) {
          expect(
            tester
                .widget<PopupMenuButton<String>>(
                  projectKey('project-editor-menu'),
                )
                .enabled,
            isFalse,
          );
        }
        staleSave();
        await tester.pumpAndSettle();
        expect(api.writes, isEmpty);
        await acceptProjectOriginReview(tester);
        expect(api.writes, isEmpty);
        await projectTap(tester, 'save-project');
        expect(api.writes, [
          creating ? 'project_create_v3' : 'project_update_v3',
        ]);
        final args = api.arguments.singleWhere(
          (args) => args.containsKey('expected_revision'),
        );
        expect(args['instance_id'], restartedProjectInstance);
        expect(args['expected_revision'], 2);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox());
      },
    );
  }

  for (final dirty in [false, true]) {
    testWidgets(
      'same-revision restart invalidates open delete confirmation for ${dirty ? 'dirty' : 'clean'} form and captured callback',
      (tester) async {
        final api = ProjectsUiApi()..projectRevision = 2;
        final controller = await mountProjects(tester, api);
        await chooseProject(tester, alphaProject);
        if (dirty) {
          await tester.enterText(
            projectKey('project-key'),
            'preserve-local-delete-target-edits',
          );
        }
        await projectTap(tester, 'delete-project');
        final oldConfirm = tester
            .widget<TextButton>(
              find.widgetWithText(TextButton, 'Delete metadata'),
            )
            .onPressed!;
        api.instance = restartedProjectInstance;
        api.records[alphaProject] = api.record(
          alphaProject,
          'alpha-after-restart',
        );
        await controller.refresh();
        await tester.pumpAndSettle();
        expect(controller.revision, 2);
        expect(
          tester
              .widget<TextButton>(
                find.widgetWithText(TextButton, 'Delete metadata'),
              )
              .onPressed,
          isNull,
        );
        oldConfirm();
        await tester.pumpAndSettle();
        expect(api.writes, isEmpty);
        expect(
          projectField(tester, 'project-key').controller!.text,
          dirty ? 'preserve-local-delete-target-edits' : 'alpha-after-restart',
        );
        if (dirty) {
          expect(
            tester
                .widget<PopupMenuButton<String>>(
                  projectKey('project-editor-menu'),
                )
                .enabled,
            isFalse,
          );
          await acceptProjectOriginReview(tester);
        }
        await projectTap(tester, 'delete-project');
        await tester.tap(find.widgetWithText(TextButton, 'Delete metadata'));
        await tester.pumpAndSettle();
        expect(api.writes, ['project_delete_v3']);
        final args = api.arguments.singleWhere(
          (args) => args.containsKey('expected_revision'),
        );
        expect(args['instance_id'], restartedProjectInstance);
        expect(args['expected_revision'], 2);
        expect(api.records.keys, [betaProject]);
        expect(find.text('No project selected'), findsOneWidget);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox());
      },
    );
  }

  for (final restart in [true, false]) {
    testWidgets(
      'open origin review and its return callback reject ${restart ? 'same-revision instance change' : 'revision change'}',
      (tester) async {
        final api = ProjectsUiApi()..projectRevision = 2;
        final controller = await mountProjects(tester, api);
        await chooseProject(tester, alphaProject);
        await tester.enterText(
          projectKey('project-key'),
          'keep-unsaved-review-edits',
        );
        api.projectRevision++;
        await projectTap(tester, 'refresh-projects');
        await projectTap(tester, 'review-project-revision');
        final oldConfirm = tester
            .widget<TextButton>(
              find.widgetWithText(TextButton, 'Keep edits at this revision'),
            )
            .onPressed!;
        if (restart) {
          api.instance = restartedProjectInstance;
        } else {
          api.projectRevision++;
        }
        await controller.refresh();
        await tester.pumpAndSettle();
        expect(
          tester
              .widget<TextButton>(
                find.widgetWithText(TextButton, 'Keep edits at this revision'),
              )
              .onPressed,
          isNull,
        );
        oldConfirm();
        await tester.pumpAndSettle();
        expect(
          projectField(tester, 'project-key').controller!.text,
          'keep-unsaved-review-edits',
        );
        expect(
          tester.widget<FilledButton>(projectKey('save-project')).onPressed,
          isNull,
        );
        expect(api.writes, isEmpty);
        await acceptProjectOriginReview(tester);
        expect(api.writes, isEmpty);
        await projectTap(tester, 'save-project');
        expect(api.writes, ['project_update_v3']);
        final args = api.arguments.singleWhere(
          (args) => args.containsKey('expected_revision'),
        );
        expect(args['instance_id'], api.instance);
        expect(args['expected_revision'], restart ? 3 : 4);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox());
      },
    );
  }

  for (final creating in [false, true]) {
    testWidgets(
      'receipt resolution after restart never silently rebinds ${creating ? 'created identity' : 'dirty editor'}',
      (tester) async {
        final api = ProjectsUiApi(empty: creating)
          ..projectRevision = 2
          ..loseAcknowledgement = true;
        final controller = await mountProjects(tester, api);
        if (creating) {
          await projectTap(tester, 'new-project');
        } else {
          await chooseProject(tester, alphaProject);
        }
        await tester.enterText(
          projectKey('project-key'),
          'originally-submitted-edits',
        );
        await projectTap(tester, 'save-project');
        expect(controller.uncertain, isTrue);
        await tester.enterText(
          projectKey('project-key'),
          'newer-unsaved-edits',
        );
        api.instance = restartedProjectInstance;
        await projectTap(tester, 'resolve-project-change');
        expect(controller.uncertain, isFalse);
        expect(
          projectField(tester, 'project-key').controller!.text,
          'newer-unsaved-edits',
        );
        expect(
          tester.widget<FilledButton>(projectKey('save-project')).onPressed,
          isNull,
        );
        expect(api.writes, [
          creating ? 'project_create_v3' : 'project_update_v3',
        ]);
        if (creating) {
          expect(projectKey('review-project-revision'), findsNothing);
          expect(
            find.textContaining('do not repeat the creation'),
            findsOneWidget,
          );
        } else {
          await acceptProjectOriginReview(tester);
          expect(
            projectField(tester, 'project-key').controller!.text,
            'newer-unsaved-edits',
          );
          expect(
            tester.widget<FilledButton>(projectKey('save-project')).onPressed,
            isNotNull,
          );
        }
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox());
      },
    );
  }

  for (final missingCode in ['REQUEST_UNKNOWN', 'REQUEST_EXPIRED']) {
    testWidgets(
      '$missingCode retains uncertainty while serialized Refresh observes external metadata and preserves input',
      (tester) async {
        final api = ProjectsUiApi()
          ..projectRevision = 2
          ..loseAcknowledgement = true
          ..missingReceiptCode = missingCode;
        final controller = await mountProjects(tester, api);
        await chooseProject(tester, alphaProject);
        await tester.enterText(
          projectKey('project-key'),
          'submitted-before-uncertainty',
        );
        await projectTap(tester, 'save-project');
        expect(controller.uncertain, isTrue);
        final originalRequest = api.arguments.singleWhere(
          (args) => args.containsKey('expected_revision'),
        )['request_id'];
        api.receipts.clear();
        await projectTap(tester, 'resolve-project-change');
        expect(controller.uncertain, isTrue);
        expect(controller.canMutate, isFalse);
        await tester.enterText(
          projectKey('project-key'),
          'newer-preserved-input',
        );
        api.records[betaProject] = api.record(
          betaProject,
          'observed-external-metadata',
        );
        api.projectRevision++;
        api.instance = restartedProjectInstance;
        api.listGate = Completer<AppReply>();
        final callsBefore = api.projectCalls.length;
        final refresh = tester.widget<TextButton>(projectRefresh()).onPressed;
        expect(refresh, isNotNull);
        refresh!();
        refresh();
        await tester.pump();
        expect(api.projectCalls.sublist(callsBefore), ['projects_list_v3']);
        expect(tester.widget<TextButton>(projectRefresh()).onPressed, isNull);
        api.listGate!.complete(
          AppReply(api.instance, {
            'revision': api.projectRevision,
            'projects': api.records.values.toList(),
          }),
        );
        api.listGate = null;
        await tester.pumpAndSettle();
        expect(controller.instanceId, restartedProjectInstance);
        expect(
          controller.projects
              .singleWhere((p) => p.projectId == betaProject)
              .name,
          'observed-external-metadata',
        );
        expect(find.text('observed-external-metadata'), findsOneWidget);
        expect(
          projectField(tester, 'project-key').controller!.text,
          'newer-preserved-input',
        );
        expect(controller.uncertain, isTrue);
        expect(controller.canMutate, isFalse);
        expect(
          tester.widget<FilledButton>(projectKey('save-project')).onPressed,
          isNull,
        );
        expect(
          tester
              .widget<PopupMenuButton<String>>(
                projectKey('project-editor-menu'),
              )
              .enabled,
          isFalse,
        );
        expect(
          tester.widget<OutlinedButton>(projectKey('new-project')).onPressed,
          isNull,
        );
        expect(
          tester.widget<TextButton>(projectRefresh()).onPressed,
          isNotNull,
        );
        expect(api.writes, ['project_update_v3']);
        await projectTap(tester, 'resolve-project-change');
        final receiptRequests = [
          for (var i = 0; i < api.calls.length; i++)
            if (api.calls[i] == 'project_receipt')
              api.arguments[i]['request_id'],
        ];
        expect(receiptRequests, [originalRequest, originalRequest]);
        expect(controller.uncertain, isTrue);
        expect(api.writes, ['project_update_v3']);
        expect(
          api.projectCalls
              .sublist(callsBefore)
              .every(
                (op) => ['projects_list_v3', 'project_receipt'].contains(op),
              ),
          isTrue,
        );
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox());
      },
    );
  }
  testWidgets(
    'wide list and editor: new is unsaved, Cancel has no effects, empty repositories save',
    (tester) async {
      final api = ProjectsUiApi(empty: true);
      await mountProjects(tester, api);
      expect(find.text('No projects yet.'), findsOneWidget);
      expect(find.text('No project selected'), findsOneWidget);
      expect(api.calls, ['projects_list_v3']);
      await projectTap(tester, 'new-project');
      expect(find.text('New Project · not saved'), findsOneWidget);
      await tester.enterText(projectKey('project-key'), 'not-saved');
      expect(api.writes, isEmpty);
      await discardProjectChanges(tester);
      expect(find.text('No project selected'), findsOneWidget);
      expect(api.writes, isEmpty);
      await projectTap(tester, 'new-project');
      await tester.enterText(projectKey('project-key'), 'saved-foundation');
      await projectTap(tester, 'save-project');
      expect(api.records.values.single['repositories'], isEmpty);
      expect(api.records.values.single['agent'], 'opencode');
      expect(api.writes, ['project_create_v3']);
      expect(find.text('Edit Project'), findsOneWidget);
      expect(find.text('OpenCode'), findsNothing);
      expect(find.text('Execution harness'), findsNothing);
      expect(find.text('Agent'), findsNothing);
      expect(projectKey('project-name'), findsNothing);
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
        projectKey('project-key'),
        'retained-across-breakpoints',
      );
      await projectTap(tester, 'add-project-repository');
      await tester.enterText(
        projectKey('project-repository-0'),
        'git@example.invalid:team/repo',
      );
      final name = projectField(tester, 'project-key').controller;
      final repository = projectField(
        tester,
        'project-repository-0',
      ).controller;
      final state = tester.state(find.byType(ProjectsView));
      for (final size in [const Size(400, 400), const Size(1600, 1000)]) {
        tester.view.physicalSize = size;
        await tester.pumpAndSettle();
        expect(tester.state(find.byType(ProjectsView)), same(state));
        expect(projectField(tester, 'project-key').controller, same(name));
        expect(
          projectField(tester, 'project-repository-0').controller,
          same(repository),
        );
        expect(name!.text, 'retained-across-breakpoints');
        expect(repository!.text, 'git@example.invalid:team/repo');
        expect(tester.takeException(), isNull);
      }
      expect(api.projectCalls, ['projects_list_v3']);
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
      expect(api.projectCalls, ['projects_list_v3']);
      expect(api.disposedCount, 0);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'repository validators preserve invalid 41-scalar names and locators without writes',
    (tester) async {
      final api = ProjectsUiApi(empty: true);
      await mountProjects(tester, api);
      await projectTap(tester, 'new-project');
      await tester.enterText(
        projectKey('project-key'),
        'repository-validation',
      );
      await projectTap(tester, 'add-project-repository');
      final tooLong = List.filled(41, '😀').join();
      await tester.enterText(projectKey('project-repository-0-name'), tooLong);
      await tester.enterText(
        projectKey('project-repository-0'),
        'https://example.invalid/team/repo',
      );
      await projectTap(tester, 'save-project');
      expect(find.text(projectNameError(tooLong)!), findsOneWidget);
      expect(
        projectField(tester, 'project-repository-0-name').controller!.text,
        tooLong,
      );
      await tester.enterText(
        projectKey('project-repository-0-name'),
        List.filled(40, '😀').join(),
      );
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
      expect(api.writes, ['project_create_v3']);
      expect(
        (api.records.values.single['repositories'] as List).single['name'],
        List.filled(40, '😀').join(),
      );
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
      await tester.enterText(projectKey('project-key'), 'repositories');
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
      await projectTap(tester, 'remove-project-repository-0');
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
      expect(api.writes, ['project_create_v3']);
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
      'drill/list, scrollable fields and actions reachable at ${fixture.$1} scale ${fixture.$2}',
      (tester) async {
        final api = ProjectsUiApi();
        await mountProjects(tester, api, size: fixture.$1, scale: fixture.$2);
        await chooseProject(tester, alphaProject);
        await tester.enterText(
          projectKey('project-key'),
          'edited-at-this-size',
        );
        await projectTap(tester, 'add-project-repository');
        await tester.enterText(
          projectKey('project-repository-0'),
          'ssh://git@example.invalid/team/repo',
        );
        await projectTap(tester, 'save-project');
        expect(api.records[alphaProject]!['key'], 'edited-at-this-size');
        expect(api.records[alphaProject]!['name'], 'alpha');
        await projectTap(tester, 'delete-project');
        expect(find.text('Delete project metadata?'), findsOneWidget);
        await tester.ensureVisible(find.text('Keep project'));
        await tester.tap(find.text('Keep project'));
        await tester.pumpAndSettle();
        expect(api.writes, ['project_update_v3']);
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
      await tester.tap(projectKey('project-key'));
      await tester.enterText(projectKey('project-key'), 'keyboard');
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(api.writes, isEmpty);
      // Save is now in the local header, but remains in normal keyboard traversal.
      final save = find.text('Save Changes');
      for (var i = 0; i < 20 && !Focus.of(tester.element(save)).hasFocus; i++) {
        await tester.sendKeyEvent(LogicalKeyboardKey.tab);
        await tester.pumpAndSettle();
      }
      expect(Focus.of(tester.element(save)).hasFocus, isTrue);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(api.writes, ['project_create_v3']);
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
      await tester.enterText(projectKey('project-key'), 'only-in-alpha-editor');
      await chooseProject(tester, betaProject);
      expect(find.text('Discard unsaved project changes?'), findsOneWidget);
      await tester.tap(find.text('Keep editing'));
      await tester.pumpAndSettle();
      expect(
        projectField(tester, 'project-key').controller!.text,
        'only-in-alpha-editor',
      );
      final retained = projectField(tester, 'project-key').controller;
      await projectTap(tester, 'cancel-project');
      expect(find.text('Discard unsaved project changes?'), findsOneWidget);
      expect(
        tester
            .widget<TextButton>(find.widgetWithText(TextButton, 'Keep editing'))
            .autofocus,
        isTrue,
      );
      await tester.tap(find.text('Keep editing'));
      await tester.pumpAndSettle();
      expect(projectField(tester, 'project-key').controller, same(retained));
      expect(retained!.text, 'only-in-alpha-editor');
      await discardProjectChanges(tester);
      expect(projectField(tester, 'project-key').controller!.text, 'alpha');
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
      await tester.enterText(projectKey('project-key'), 'local-edits');
      api.records[alphaProject] = api.record(
        alphaProject,
        'alpha',
        [],
        'foreign-edits',
      );
      api.projectRevision++;
      await projectTap(tester, 'refresh-projects');
      expect(controller.revision, 1);
      expect(
        projectField(tester, 'project-key').controller!.text,
        'local-edits',
      );
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
      await projectTap(tester, 'review-project-revision');
      expect(find.text('foreign-edits'), findsOneWidget);
      expect(find.text('ID: foreign-edits'), findsOneWidget);
      expect(
        find.descendant(
          of: find.byType(AlertDialog),
          matching: find.text('ID: local-edits'),
        ),
        findsOneWidget,
      );
      await tester.tap(find.text('Keep edits at this revision'));
      await tester.pumpAndSettle();
      expect(api.writes, isEmpty);
      await projectTap(tester, 'save-project');
      expect(api.writes, ['project_update_v3']);
      expect(api.records[alphaProject]!['key'], 'local-edits');
      expect(api.records[alphaProject]!['name'], 'alpha');
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
        projectKey('project-key'),
        'keep-deleted-target-edits',
      );
      api.records.remove(alphaProject);
      api.projectRevision++;
      await projectTap(tester, 'refresh-projects');
      expect(
        projectField(tester, 'project-key').controller!.text,
        'keep-deleted-target-edits',
      );
      expect(find.textContaining('no longer in the registry'), findsOneWidget);
      expect(
        tester.widget<FilledButton>(projectKey('save-project')).onPressed,
        isNull,
      );
      expect(projectKey('review-project-revision'), findsNothing);
      expect(api.writes, isEmpty);
      await discardProjectChanges(tester);
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
      expect(
        find.descendant(
          of: find.byType(AlertDialog),
          matching: find.text('alpha'),
        ),
        findsOneWidget,
      );
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
      expect(api.writes, ['project_delete_v3']);
      expect(api.records.keys, [betaProject]);
      expect(find.text('No project selected'), findsOneWidget);
      expect(projectKey('project-key'), findsNothing);
      await tester.pumpWidget(const SizedBox());
    },
  );

  for (final targetPresent in [false, true]) {
    testWidgets(
      'dirty delete after restart ${targetPresent ? 'preserves edits when the target is present' : 'clears selection only after receipt and authoritative absence'} without replay',
      (tester) async {
        final api = ProjectsUiApi()
          ..projectRevision = 2
          ..loseAcknowledgement = true;
        final controller = await mountProjects(tester, api);
        await chooseProject(tester, alphaProject);
        await tester.enterText(
          projectKey('project-key'),
          'unsaved-before-deletion',
        );
        await projectTap(tester, 'delete-project');
        await tester.tap(find.widgetWithText(TextButton, 'Delete metadata'));
        await tester.pumpAndSettle();
        expect(controller.uncertain, isTrue);
        expect(api.records.keys, [betaProject]);
        final originalRequest = api.arguments.singleWhere(
          (args) => args.containsKey('expected_revision'),
        )['request_id'];
        api.instance = restartedProjectInstance;
        if (targetPresent) {
          api.records[alphaProject] = api.record(
            alphaProject,
            'Present in current state',
          );
          api.projectRevision++;
        }
        await projectTap(tester, 'refresh-projects');
        expect(controller.uncertain, isTrue);
        expect(
          projectField(tester, 'project-key').controller!.text,
          'unsaved-before-deletion',
        );
        expect(api.writes, ['project_delete_v3']);
        await projectTap(tester, 'resolve-project-change');
        expect(controller.instanceId, restartedProjectInstance);
        expect(controller.uncertain, isFalse);
        expect(controller.error, isNull);
        expect(api.writes, ['project_delete_v3']);
        expect(api.projectCalls, [
          'projects_list_v3',
          'project_delete_v3',
          'projects_list_v3',
          'project_receipt',
          'projects_list_v3',
        ]);
        expect(api.arguments[3], {'request_id': originalRequest});
        if (targetPresent) {
          expect(
            projectField(tester, 'project-key').controller!.text,
            'unsaved-before-deletion',
          );
          expect(
            tester.widget<FilledButton>(projectKey('save-project')).onPressed,
            isNull,
          );
          expect(
            tester
                .widget<PopupMenuButton<String>>(
                  projectKey('project-editor-menu'),
                )
                .enabled,
            isFalse,
          );
          expect(find.text('No project selected'), findsNothing);
        } else {
          expect(controller.projects.map((project) => project.projectId), [
            betaProject,
          ]);
          expect(find.text('No project selected'), findsOneWidget);
          expect(projectKey('project-key'), findsNothing);
          expect(find.text('Edit Project'), findsNothing);
        }
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox());
      },
    );
  }

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
      expect(projectField(tester, 'project-key').enabled, isFalse);
      await projectTap(tester, 'resolve-project-change');
      expect(controller.uncertain, isFalse);
      expect(find.text('No project selected'), findsOneWidget);
      expect(projectKey('project-key'), findsNothing);
      expect(api.records.keys, [betaProject]);
      expect(api.writes, ['project_delete_v3']);
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
      await tester.enterText(projectKey('project-key'), 'original-create');
      await tester.enterText(
        projectKey('project-description'),
        'Original context',
      );
      await tester.enterText(projectKey('project-prompt'), ' exact\n\tprompt ');
      await projectTap(tester, 'save-project');
      expect(controller.uncertain, isTrue);
      final createdId = api.records.keys.single;
      await tester.enterText(projectKey('project-key'), 'newer-unsaved-edits');
      await tester.enterText(
        projectKey('project-description'),
        'Newer context',
      );
      await tester.enterText(projectKey('project-prompt'), ' newer\n\tprompt ');
      await controller.resolvePending();
      await tester.pumpAndSettle();
      expect(controller.uncertain, isFalse);
      expect(find.text('Edit Project'), findsOneWidget);
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
      expect(saved['prompt'], ' newer\n\tprompt ');
      expect(api.writes, ['project_create_v3', 'project_update_v3']);
      expect(find.text('Unsaved changes — this view only'), findsNothing);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'later registry writes after uncertain creation never select an inferred target or enable duplicate Save',
    (tester) async {
      final api = ProjectsUiApi(empty: true)..loseAcknowledgement = true;
      final controller = await mountProjects(tester, api);
      await projectTap(tester, 'new-project');
      await tester.enterText(projectKey('project-key'), 'original-create');
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
        projectField(tester, 'project-key').controller!.text,
        'original-create',
      );
      expect(find.textContaining('do not repeat the creation'), findsOneWidget);
      expect(
        tester.widget<FilledButton>(projectKey('save-project')).onPressed,
        isNull,
      );
      expect(projectKey('review-project-revision'), findsNothing);
      expect(api.writes, ['project_create_v3']);
      await discardProjectChanges(tester);
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
      expect(
        tester
            .widget<PopupMenuButton<String>>(projectKey('project-editor-menu'))
            .enabled,
        isFalse,
      );
      final writes = List<String>.of(api.writes);
      await projectTap(tester, 'resolve-project-change');
      expect(api.writes, writes);
      expect(api.projectCalls, contains('project_receipt'));
      expect(
        projectField(tester, 'project-key').controller!.text,
        'preserved-through-failure',
      );
      expect(
        api.calls.every(
          (op) => [
            'projects_list_v3',
            'project_create_v3',
            'project_update_v3',
            'project_delete_v3',
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
