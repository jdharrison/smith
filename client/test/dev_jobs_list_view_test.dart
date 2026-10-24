import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/dev_jobs.dart';
import 'package:smith_client/dev_jobs_controller.dart';
import 'package:smith_client/projects_controller.dart';
import 'package:smith_client/ui/dev_jobs_view.dart';
import 'package:smith_client/ui/smith_theme.dart';

import 'dev_jobs_test.dart' as fixture;

const _secondId = '44444444444444444444444444444444';
const _thirdId = '55555555555555555555555555555555';
const _otherProject = '66666666666666666666666666666666';

class _ListApi extends fixture.JobsApi {
  AppFailure? stateFailure;
  AppFailure? deletionFailure;
  Completer<void>? deletionGate;
  Completer<void>? resolutionGate;
  final deletions = <Map<String, dynamic>>[];
  final queryArguments = <Map<String, dynamic>>[];

  final detailFailures = <String, AppFailure>{};
  final detailGates = <String, Completer<AppReply>>{};
  final savedDetails = <String, Map<String, dynamic>>{};

  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    if (op == 'dev_job_delete_v3' || op == 'dev_job_deletion_v3') {
      validateDevArguments(op, args);
      calls.add(op);
      final id = args['request_id'] as String;
      if (op == 'dev_job_delete_v3') {
        deletions.add(Map.of(args));
        await deletionGate?.future;
        if (deletionFailure != null) throw deletionFailure!;
        deletedJobs[id] = jobs.firstWhere((job) => job['request_id'] == id);
        jobs.removeWhere((job) => job['request_id'] == id);
      } else {
        queryArguments.add(Map.of(args));
        await resolutionGate?.future;
      }
      return AppReply(serviceInstance, {
        'job':
            deletedJobs[id] ??
            jobs.firstWhere((job) => job['request_id'] == id),
        'deleted': deletedJobs.containsKey(id),
      });
    }
    if (op == 'dev_jobs_state_v4' && stateFailure != null) {
      calls.add(op);
      throw stateFailure!;
    }
    if (op == 'dev_job_get_v4') {
      final id = args['request_id'] as String;
      if (detailGates.containsKey(id) ||
          detailFailures.containsKey(id) ||
          savedDetails.containsKey(id)) {
        validateDevArguments(op, args);
        calls.add(op);
        gets.add(id);
        if (detailGates[id] case final gate?) return gate.future;
        if (detailFailures[id] case final failure?) throw failure;
        return AppReply(serviceInstance, savedDetails[id]!);
      }
    }
    return super.call(op, args);
  }
}

class _Routes extends NavigatorObserver {
  final pushed = <Route<dynamic>>[];
  final removed = <Route<dynamic>>[];
  @override
  void didPush(Route<dynamic> route, Route<dynamic>? previousRoute) {
    if (route.settings.name?.startsWith('dev-jobs/') == true) {
      pushed.add(route);
    }
  }

  @override
  void didRemove(Route<dynamic> route, Route<dynamic>? previousRoute) {
    removed.add(route);
  }
}

class _ControllerLifetime extends StatefulWidget {
  const _ControllerLifetime({required this.jobs, required this.child});
  final DevJobsController jobs;
  final Widget child;
  @override
  State<_ControllerLifetime> createState() => _ControllerLifetimeState();
}

class _ControllerLifetimeState extends State<_ControllerLifetime> {
  @override
  void dispose() {
    widget.jobs.dispose();
    widget.jobs.projects.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => widget.child;
}

Future<DevJobsController> _mount(
  WidgetTester tester,
  fixture.JobsApi api, {
  ValueNotifier<bool>? visible,
  ValueNotifier<bool>? alive,
  GlobalKey<NavigatorState>? navigatorKey,
  _Routes? routes,
  double scale = 1,
  int panes = 1,
}) async {
  final projects = ProjectsController(api);
  final jobs = DevJobsController(api, projects);
  addTearDown(() async {
    await tester.pumpWidget(const SizedBox());
    await tester.pump();
  });
  Widget view = panes == 1
      ? DevJobsView(controller: jobs)
      : Row(
          children: [
            for (var pane = 0; pane < panes; pane++)
              Expanded(
                child: DevJobsView(key: ValueKey(pane), controller: jobs),
              ),
          ],
        );
  if (visible != null) {
    view = ValueListenableBuilder<bool>(
      valueListenable: visible,
      child: view,
      builder: (context, enabled, child) => ExcludeFocus(
        excluding: !enabled,
        child: TickerMode(enabled: enabled, child: child!),
      ),
    );
  }
  if (alive != null) {
    view = ValueListenableBuilder<bool>(
      valueListenable: alive,
      child: view,
      builder: (context, present, child) => present ? child! : const SizedBox(),
    );
  }
  await tester.pumpWidget(
    _ControllerLifetime(
      jobs: jobs,
      child: MaterialApp(
        navigatorKey: navigatorKey,
        navigatorObservers: [?routes],
        theme: SmithTheme.dark(),
        builder: (context, child) => MediaQuery(
          data: MediaQuery.of(context)
              .copyWith(textScaler: TextScaler.linear(scale)),
          child: child!,
        ),
        home: Scaffold(body: view),
      ),
    ),
  );
  await tester.pumpAndSettle();
  return jobs;
}

Finder _row(String id) => find.byKey(ValueKey('dev-job-$id'));
Finder _filled(String label) => find.widgetWithText(FilledButton, label);
Finder _textButton(String label) => find.widgetWithText(TextButton, label);
Finder _inDialog(Finder finder) =>
    find.descendant(of: find.byType(AlertDialog), matching: finder);

Future<void> _openNew(WidgetTester tester) async {
  await tester.ensureVisible(_filled('New job'));
  await tester.tap(_filled('New job'));
  await tester.pumpAndSettle();
}

Future<void> _draft(
  WidgetTester tester, [
  String text = ' exact\n\t task ',
]) async {
  final project = find.byType(DropdownButtonFormField<String>);
  await tester.ensureVisible(project);
  await tester.tap(project);
  await tester.pumpAndSettle();
  await tester.tap(find.text('smith').last);
  await tester.pumpAndSettle();
  await tester.ensureVisible(find.byType(TextField));
  await tester.enterText(find.byType(TextField), text);
  await tester.pump();
}

Future<void> _review(WidgetTester tester) async {
  await tester.ensureVisible(_filled('Review start'));
  await tester.tap(_filled('Review start'));
  await tester.pumpAndSettle();
}

Future<void> _close(WidgetTester tester) async {
  await tester.ensureVisible(_textButton('Close'));
  await tester.tap(_textButton('Close'));
  await tester.pumpAndSettle();
}

Future<void> _openDetail(WidgetTester tester, String id) async {
  await tester.ensureVisible(_row(id));
  await tester.tap(_row(id));
  await tester.pumpAndSettle();
}

void _queriesOnly(fixture.JobsApi api) {
  expect(api.starts, isEmpty);
  expect(
    api.calls.every(
      {
        'projects_list_v3',
        'dev_jobs_state_v4',
        'dev_job_get_v4',
        'dev_sessions_state_v1',
      }.contains,
    ),
    isTrue,
  );
}

void main() {
  testWidgets(
    'queued list and detail stay observable while New job remains available',
    (tester) async {
      final api = _ListApi()
        ..jobs.addAll([
          fixture.summary(fixture.requestId, 'queued'),
          fixture.summary(_secondId, 'running'),
        ]);
      final jobs = await _mount(tester, api);
      expect(find.text('QUEUED'), findsOneWidget);
      expect(jobs.canStart, isTrue);
      await _openDetail(tester, fixture.requestId);
      expect(find.text('Phase: queued'), findsOneWidget);
      expect(find.text('Not started.'), findsOneWidget);
      expect(find.text('Owned container stop verified.'), findsNothing);
      expect(_textButton('Delete'), findsNothing);
      expect(find.textContaining('ETA'), findsNothing);
      await _close(tester);
      await _openNew(tester);
      await _draft(tester);
      expect(
        tester.widget<FilledButton>(_filled('Review start')).onPressed,
        isNotNull,
      );
      expect(find.text('Another job is running.'), findsNothing);
      _queriesOnly(api);
    },
  );

  for (final phase in ['completed', 'failed', 'reconciled']) {
    testWidgets('$phase Delete is detail-only and Cancel never mutates', (
      tester,
    ) async {
      final api = _ListApi()
        ..jobs.add(fixture.summary(fixture.requestId, phase));
      await _mount(tester, api);
      expect(_textButton('Delete'), findsNothing);
      await _openDetail(tester, fixture.requestId);
      await tester.tap(_textButton('Delete'));
      await tester.pumpAndSettle();
      expect(find.text('Delete job?'), findsOneWidget);
      expect(_inDialog(find.text('Project: smith')), findsOneWidget);
      expect(
        _inDialog(find.text('Request: ${fixture.requestId}')),
        findsOneWidget,
      );
      expect(
        find.text(
          'Remove from history? Execution records and containers remain.',
        ),
        findsOneWidget,
      );
      expect(
        tester.widget<TextButton>(_textButton('Cancel')).autofocus,
        isTrue,
      );
      await tester.tap(_textButton('Cancel'));
      await tester.pumpAndSettle();
      expect(find.text('Job details'), findsOneWidget);
      expect(api.deletions, isEmpty);
      _queriesOnly(api);
    });
  }

  for (final phase in ['running', 'unknown', 'cleanup_unknown']) {
    testWidgets('$phase has no Delete action', (tester) async {
      final api = _ListApi()
        ..jobs.add(fixture.summary(fixture.requestId, phase));
      await _mount(tester, api);
      await _openDetail(tester, fixture.requestId);
      expect(_textButton('Delete'), findsNothing);
      expect(_filled('Delete'), findsNothing);
      _queriesOnly(api);
    });
  }

  testWidgets('unverified failed detail has no Delete action', (tester) async {
    final api = _ListApi()
      ..jobs.add({
        ...fixture.summary(fixture.requestId, 'failed'),
        'cleanup_verified': false,
      });
    final jobs = await _mount(tester, api);
    await _openDetail(tester, fixture.requestId);
    expect(_textButton('Delete'), findsNothing);
    expect(jobs.canStart, isFalse);
    _queriesOnly(api);
  });

  testWidgets(
    'Resolve in-flight guard disables every shared pane and duplicate callback',
    (tester) async {
      final api = _ListApi()
        ..jobs.add(fixture.summary())
        ..deletionFailure = const AppFailure(
          'TRANSPORT',
          'Lost acknowledgement',
          unknown: true,
        )
        ..resolutionGate = Completer<void>();
      final jobs = await _mount(tester, api, panes: 2);
      await jobs.deleteJob(fixture.requestId);
      await tester.pumpAndSettle();
      expect(_textButton('Resolve'), findsNWidgets(2));
      final first = tester
          .widget<TextButton>(_textButton('Resolve').first)
          .onPressed!;
      final second = tester
          .widget<TextButton>(_textButton('Resolve').last)
          .onPressed!;
      first();
      second();
      await tester.pumpAndSettle();
      for (final button in tester.widgetList<TextButton>(
        _textButton('Resolve'),
      )) {
        expect(button.onPressed, isNull);
      }
      expect(api.queryArguments, hasLength(1));
      expect(api.deletions, hasLength(1));
      api.resolutionGate!.complete();
      await tester.pumpAndSettle();
      expect(api.deletions, hasLength(1));
      expect(api.starts, isEmpty);
    },
  );

  testWidgets(
    'delete binds exact target and consumes duplicate callbacks; detail becomes deleted',
    (tester) async {
      final api = _ListApi()
        ..deletionGate = Completer<void>()
        ..jobs.addAll([fixture.summary(), fixture.summary(_secondId)]);
      final jobs = await _mount(tester, api);
      await _openDetail(tester, _secondId);
      await tester.tap(_textButton('Delete'));
      await tester.pumpAndSettle();
      expect(_inDialog(find.text('Request: $_secondId')), findsOneWidget);
      final confirm = tester.widget<FilledButton>(_filled('Delete')).onPressed!;
      confirm();
      confirm();
      await tester.pumpAndSettle();
      expect(api.deletions, [
        {'request_id': _secondId, 'instance_id': fixture.instance},
      ]);
      expect(_inDialog(find.text('Deleting from history…')), findsOneWidget);
      expect(
        tester.widget<TextButton>(_textButton('Delete')).onPressed,
        isNull,
      );
      api.deletionGate!.complete();
      await tester.pumpAndSettle();
      expect(jobs.isDeleted(_secondId), isTrue);
      expect(find.text('Deleted from history'), findsOneWidget);
      expect(find.text('A saved final reply.'), findsNothing);
      expect(find.text('Loading saved job details…'), findsNothing);
      expect(_textButton('Refresh details (read-only)'), findsNothing);
      expect(_textButton('Delete'), findsNothing);
      await _close(tester);
      expect(_row(_secondId), findsNothing);
      expect(_row(fixture.requestId), findsOneWidget);
      expect(
        FocusManager.instance.primaryFocus,
        tester.widget<FilledButton>(_filled('New job')).focusNode,
      );
      final calls = api.calls.length;
      await tester.pump(const Duration(minutes: 1));
      expect(api.calls, hasLength(calls));
      expect(api.starts, isEmpty);
    },
  );

  testWidgets(
    'unknown deletion Resolve only queries; other unknown job stays blocking',
    (tester) async {
      final api = _ListApi()
        ..deletionFailure = const AppFailure(
          'TRANSPORT',
          'Lost acknowledgement',
          unknown: true,
        )
        ..jobs.addAll([
          fixture.summary(),
          fixture.summary(_secondId, 'unknown'),
        ]);
      final jobs = await _mount(tester, api);
      await _openDetail(tester, fixture.requestId);
      await tester.tap(_textButton('Delete'));
      await tester.pumpAndSettle();
      await tester.tap(_filled('Delete'));
      await tester.pumpAndSettle();
      expect(_inDialog(find.text('Deletion unconfirmed.')), findsOneWidget);
      expect(
        tester.widget<TextButton>(_textButton('Delete')).onPressed,
        isNull,
      );
      final deleted = api.jobs.firstWhere(
        (job) => job['request_id'] == fixture.requestId,
      );
      api.deletedJobs[fixture.requestId] = deleted;
      api.jobs.remove(deleted);
      api.resolutionGate = Completer<void>();
      final resolve = tester
          .widget<TextButton>(_inDialog(_textButton('Resolve')))
          .onPressed!;
      resolve();
      resolve();
      await tester.pumpAndSettle();
      expect(api.queryArguments, [
        {'request_id': fixture.requestId},
      ]);
      expect(api.deletions, hasLength(1));
      expect(
        tester.widget<TextButton>(_inDialog(_textButton('Resolve'))).onPressed,
        isNull,
      );
      api.resolutionGate!.complete();
      await tester.pumpAndSettle();
      expect(find.text('Deleted from history'), findsOneWidget);
      expect(jobs.canStart, isFalse);
      expect(jobs.jobs.any((job) => job.requestId == _secondId), isTrue);
      expect(api.starts, isEmpty);
    },
  );

  testWidgets(
    'stale confirmation cannot dispatch after hide or instance change',
    (tester) async {
      final visible = ValueNotifier(true);
      addTearDown(visible.dispose);
      final api = _ListApi()..jobs.add(fixture.summary());
      final jobs = await _mount(tester, api, visible: visible);
      await _openDetail(tester, fixture.requestId);
      await tester.tap(_textButton('Delete'));
      await tester.pumpAndSettle();
      final confirm = tester.widget<FilledButton>(_filled('Delete')).onPressed!;
      api.serviceInstance = _thirdId;
      await jobs.projects.refresh();
      confirm();
      expect(api.deletions, isEmpty);
      visible.value = false;
      await tester.pumpAndSettle();
      confirm();
      expect(api.deletions, isEmpty);
      expect(find.text('Delete job?'), findsNothing);
    },
  );

  testWidgets('late delete never pops replacement New job or loses draft', (
    tester,
  ) async {
    final api = _ListApi()
      ..deletionGate = Completer<void>()
      ..jobs.add(fixture.summary());
    await _mount(tester, api);
    await _openNew(tester);
    await _draft(tester, 'retained exact draft');
    await _close(tester);
    await _openDetail(tester, fixture.requestId);
    await tester.tap(_textButton('Delete'));
    await tester.pumpAndSettle();
    await tester.tap(_filled('Delete'));
    await tester.pumpAndSettle();
    await _close(tester);
    await _openNew(tester);
    api.deletionGate!.complete();
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('dev-new-job-dialog')), findsOneWidget);
    expect(
      tester.widget<TextField>(find.byType(TextField)).controller!.text,
      'retained exact draft',
    );
    expect(api.deletions, hasLength(1));
    expect(api.starts, isEmpty);
  });

  testWidgets(
    'shared pane deletion updates an already open detail and both lists',
    (tester) async {
      final api = _ListApi()
        ..jobs.addAll([fixture.summary(), fixture.summary(_secondId)]);
      final projects = ProjectsController(api);
      final jobs = DevJobsController(api, projects);
      addTearDown(() {
        jobs.dispose();
        projects.dispose();
      });
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: Row(
              children: [
                Expanded(
                  child: DevJobsView(
                    key: const ValueKey('left-delete'),
                    controller: jobs,
                  ),
                ),
                Expanded(
                  child: DevJobsView(
                    key: const ValueKey('right-delete'),
                    controller: jobs,
                  ),
                ),
              ],
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(_row(fixture.requestId).last);
      await tester.pumpAndSettle();
      expect(find.text('A saved final reply.'), findsOneWidget);
      // A second pane/window observes the same controller proof while this detail is open.
      await jobs.deleteJob(fixture.requestId);
      await tester.pumpAndSettle();
      expect(find.text('Deleted from history'), findsOneWidget);
      expect(find.text('A saved final reply.'), findsNothing);
      expect(find.text('Loading saved job details…'), findsNothing);
      await _close(tester);
      expect(_row(fixture.requestId), findsNothing);
      expect(_row(_secondId), findsNWidgets(2));
      expect(api.deletions, hasLength(1));
      await tester.pumpWidget(const SizedBox());
      await tester.pump();
    },
  );

  testWidgets(
    'confirmation revalidates target eligibility and cannot delete a replacement detail',
    (tester) async {
      final api = _ListApi()
        ..jobs.addAll([fixture.summary(), fixture.summary(_secondId)]);
      final jobs = await _mount(tester, api);
      await _openDetail(tester, fixture.requestId);
      await tester.tap(_textButton('Delete'));
      await tester.pumpAndSettle();
      final obsolete = tester
          .widget<FilledButton>(_filled('Delete'))
          .onPressed!;
      await tester.tap(_textButton('Cancel'));
      await tester.pumpAndSettle();
      await _close(tester);
      await _openDetail(tester, _secondId);
      await tester.tap(_textButton('Delete'));
      await tester.pumpAndSettle();
      obsolete();
      await tester.pumpAndSettle();
      expect(_inDialog(find.text('Request: $_secondId')), findsOneWidget);
      expect(find.text('Delete job?'), findsOneWidget);
      expect(api.deletions, isEmpty);
      final current = tester.widget<FilledButton>(_filled('Delete')).onPressed!;
      api.jobs[1] = fixture.summary(_secondId, 'unknown');
      await jobs.refresh();
      await tester.pumpAndSettle();
      current();
      expect(api.deletions, isEmpty);
      expect(tester.widget<FilledButton>(_filled('Delete')).onPressed, isNull);
    },
  );

  testWidgets(
    'covered hidden delete origin cannot pop another route on late acknowledgement',
    (tester) async {
      final api = _ListApi()
        ..deletionGate = Completer<void>()
        ..jobs.add(fixture.summary());
      final visible = ValueNotifier(true);
      addTearDown(visible.dispose);
      final navigator = GlobalKey<NavigatorState>();
      final routes = _Routes();
      await _mount(
        tester,
        api,
        visible: visible,
        navigatorKey: navigator,
        routes: routes,
      );
      await _openDetail(tester, fixture.requestId);
      await tester.tap(_textButton('Delete'));
      await tester.pumpAndSettle();
      final confirm = tester.widget<FilledButton>(_filled('Delete')).onPressed!;
      confirm();
      await tester.pumpAndSettle();
      final origin = routes.pushed.last;
      final other = DialogRoute<void>(
        context: navigator.currentContext!,
        builder: (_) => const AlertDialog(title: Text('Other deletion route')),
      );
      unawaited(navigator.currentState!.push(other));
      await tester.pumpAndSettle();
      visible.value = false;
      await tester.pumpAndSettle();
      confirm();
      api.deletionGate!.complete();
      await tester.pumpAndSettle();
      expect(routes.removed, contains(origin));
      expect(routes.removed, isNot(contains(other)));
      expect(find.text('Other deletion route'), findsOneWidget);
      expect(api.deletions, hasLength(1));
      expect(api.starts, isEmpty);
    },
  );

  testWidgets(
    'more than 32 retained rows have no count warning and allow Review',
    (tester) async {
      final api = _ListApi()
        ..jobs.addAll(
          List.generate(
            40,
            (index) => fixture.summary(
              (index + 100).toRadixString(16).padLeft(32, '0'),
            ),
          ),
        );
      final jobs = await _mount(tester, api);
      expect(jobs.jobs, hasLength(40));
      expect(find.textContaining('maximum'), findsNothing);
      expect(find.textContaining('capacity'), findsNothing);
      expect(find.textContaining('history is full'), findsNothing);
      await _openNew(tester);
      await _draft(tester);
      expect(
        tester.widget<FilledButton>(_filled('Review start')).onPressed,
        isNotNull,
      );
      _queriesOnly(api);
    },
  );

  testWidgets(
    'narrow text2 confirmation wraps with 48px keyboard-safe Cancel default',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(400, 500));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final api = _ListApi()..jobs.add(fixture.summary());
      await _mount(tester, api, scale: 2);
      await _openDetail(tester, fixture.requestId);
      await tester.ensureVisible(_textButton('Delete'));
      await tester.tap(_textButton('Delete'));
      await tester.pumpAndSettle();
      for (final button in [_textButton('Cancel'), _filled('Delete')]) {
        await tester.ensureVisible(button);
        expect(tester.getSize(button).height, greaterThanOrEqualTo(48));
        expect(tester.getSize(button).width, greaterThanOrEqualTo(48));
      }
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(find.text('Job details'), findsOneWidget);
      expect(api.deletions, isEmpty);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'new job edit shows fields without technical or consent boilerplate',
    (tester) async {
      final api = _ListApi();
      await _mount(tester, api);
      await _openNew(tester);
      expect(find.text('LOCAL model: installed-local'), findsOneWidget);
      expect(_inDialog(find.textContaining('SHA-256:')), findsNothing);
      expect(
        _inDialog(find.textContaining('Configuration revision:')),
        findsNothing,
      );
      expect(_inDialog(find.textContaining('OpenCode •')), findsNothing);
      expect(_inDialog(find.textContaining('Close retains')), findsNothing);
      expect(
        _inDialog(find.textContaining('Review and Start require')),
        findsNothing,
      );
      final field = tester.widget<TextField>(find.byType(TextField));
      expect(field.decoration!.labelText, 'Task');
      expect(field.decoration!.helperText, isNull);
      expect(find.byKey(const ValueKey('dev-start-diagnostics')), findsNothing);
      await _draft(tester);
      await _review(tester);
      expect(_inDialog(find.textContaining('SHA-256:')), findsOneWidget);
      expect(_inDialog(find.textContaining('PUBLIC HTTPS')), findsOneWidget);
      expect(_filled('Start job'), findsOneWidget);
      _queriesOnly(api);
    },
  );

  for (final scale in [1.0, 2.0]) {
    testWidgets(
      'new job blockers are concise with expandable diagnostics at $scale',
      (tester) async {
        await tester.binding.setSurfaceSize(const Size(400, 500));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final api = _ListApi()
          ..localModel = fixture.model(ready: false)
          ..jobs.add(fixture.summary(fixture.requestId, 'unknown'));
        await _mount(tester, api, scale: scale);
        await _openNew(tester);
        await _draft(tester);
        const summary = 'Queue admission unavailable.\nLocal model not ready.';
        expect(_inDialog(find.text(summary)), findsOneWidget);
        expect(
          _inDialog(find.textContaining('Refresh observes only.')),
          findsNothing,
        );
        expect(
          _inDialog(
            find.textContaining('Check the local model in the installer'),
          ),
          findsNothing,
        );
        expect(
          tester.widget<FilledButton>(_filled('Review start')).onPressed,
          isNull,
        );
        final diagnostics = find.byKey(const ValueKey('dev-start-diagnostics'));
        await tester.ensureVisible(diagnostics);
        await tester.tap(_inDialog(find.text(summary)));
        await tester.pumpAndSettle();
        expect(
          _inDialog(find.textContaining('Refresh observes only.')),
          findsOneWidget,
        );
        expect(
          _inDialog(
            find.textContaining('Check the local model in the installer'),
          ),
          findsOneWidget,
        );
        expect(tester.takeException(), isNull);
        await _close(tester);
        _queriesOnly(api);
      },
    );
  }

  for (final hasReply in [false, true]) {
    for (final compact in [false, true]) {
      testWidgets(
        'reconciled list/details reply=$hasReply narrow-text2=$compact remain read-only',
        (tester) async {
          if (compact) {
            await tester.binding.setSurfaceSize(const Size(400, 400));
            addTearDown(() => tester.binding.setSurfaceSize(null));
          }
          final job = {
            ...fixture.summary(fixture.requestId, 'reconciled'),
            'message': 'Original worker failure; no automatic replay.',
            'reply_available': hasReply,
          };
          final api = _ListApi()..jobs.add(job);
          final jobs = await _mount(tester, api, scale: compact ? 2 : 1);
          const label = 'Operator-reconciled — historical task outcome unknown';
          expect(find.text('RECONCILED'), findsOneWidget);
          expect(find.text(label), findsNothing);
          expect(
            find.text('Execution quiescence: operator verified'),
            findsNothing,
          );
          expect(find.text('Cleanup: stop verified'), findsNothing);
          expect(jobs.canStart, isTrue);
          final ink = find.descendant(
            of: _row(fixture.requestId),
            matching: find.byType(InkWell),
          );
          final focus = tester.widget<InkWell>(ink).focusNode!;
          await tester.ensureVisible(ink);
          focus.requestFocus();
          await tester.pump();
          await tester.sendKeyEvent(LogicalKeyboardKey.enter);
          await tester.pumpAndSettle();
          expect(_inDialog(find.text(label)), findsOneWidget);
          expect(find.text(job['message'] as String), findsOneWidget);
          expect(find.text('Failed stage: running'), findsOneWidget);
          expect(find.text('Session: ses_test'), findsOneWidget);
          expect(
            find.text(
              'Operator verified all execution quiescent; historical task outcome remains unknown.',
            ),
            findsOneWidget,
          );
          expect(find.text('Owned container stop verified.'), findsNothing);
          expect(
            find.textContaining('Commit: ${fixture.commit}'),
            findsOneWidget,
          );
          expect(
            find.text(
              hasReply
                  ? 'Reply: saved final reply available.'
                  : 'Reply: no saved final reply.',
            ),
            findsOneWidget,
          );
          expect(
            find.text('A saved final reply.'),
            hasReply ? findsOneWidget : findsNothing,
          );
          expect(
            find.text(
              'Saved final reply — untrusted text, not execution or delivery approval:',
            ),
            hasReply ? findsOneWidget : findsNothing,
          );
          for (final label in ['Reconcile', 'Stop', 'Retry', 'Re-run']) {
            expect(find.widgetWithText(ButtonStyleButton, label), findsNothing);
          }
          expect(tester.takeException(), isNull);
          await _close(tester);
          expect(focus.hasFocus, isTrue);
          final calls = api.calls.toList();
          await tester.pump(const Duration(seconds: 20));
          expect(api.calls, calls);
          expect(jobs.canStart, isTrue);
          _queriesOnly(api);
          expect(tester.takeException(), isNull);
        },
      );
    }
  }

  testWidgets(
    'compact keyed rows keep full identities and metadata in details',
    (tester) async {
      final first = fixture.summary();
      final second = {
        ...fixture.summary(_secondId, 'failed'),
        'project_id': _otherProject,
        'project_key': 'other-project',
        'created_ms': 1700000001000,
      };
      final third = {...fixture.summary(_thirdId), 'created_ms': 1700000001000};
      final api = _ListApi()..jobs.addAll([third, first, second]);
      api.savedDetails[_secondId] = {
        ...fixture.detail(second),
        'model': {'name': 'frozen-other-model', 'sha256': fixture.digest},
      };
      final jobs = await _mount(tester, api);
      final current = jobs.current;
      expect(find.text('Pipeline jobs'), findsOneWidget);
      expect(find.byType(TextField), findsNothing);
      for (final id in [fixture.requestId, _secondId, _thirdId]) {
        expect(_row(id), findsOneWidget);
        expect(find.text('Request: $id'), findsNothing);
        expect(
          find.descendant(
            of: _row(id),
            matching: find.textContaining('#${id.substring(0, 8)}'),
          ),
          findsOneWidget,
        );
        expect(tester.getSize(_row(id)).height, lessThanOrEqualTo(56));
      }
      expect(
        tester.getTopLeft(_row(_secondId)).dy,
        lessThan(tester.getTopLeft(_row(_thirdId)).dy),
      );
      expect(
        tester.getTopLeft(_row(_thirdId)).dy,
        lessThan(tester.getTopLeft(_row(fixture.requestId)).dy),
      );
      expect(find.text('Created UTC: 2023-11-14T22:13:20.000Z'), findsNothing);
      await _openDetail(tester, _secondId);
      expect(_inDialog(find.text('Request: $_secondId')), findsOneWidget);
      expect(
        _inDialog(find.text('Created UTC: 2023-11-14T22:13:21.000Z')),
        findsOneWidget,
      );
      expect(_inDialog(find.text('Project: other-project')), findsOneWidget);
      expect(_inDialog(find.text('Phase: failed')), findsOneWidget);
      expect(find.text('Run model: frozen-other-model'), findsOneWidget);
      expect(find.textContaining('Commit: ${fixture.commit}'), findsOneWidget);
      expect(jobs.current, same(current));
      expect(find.text('Exact task:'), findsNothing);
      for (final label in ['Stop', 'Cancel', 'Retry', 'Re-run', 'Reset']) {
        expect(
          find.byWidgetPredicate(
            (w) =>
                w is ButtonStyleButton &&
                w.child is Text &&
                (w.child! as Text).data == label,
          ),
          findsNothing,
        );
      }
      expect(_inDialog(_textButton('Delete')), findsOneWidget);
      expect(api.deletions, isEmpty);
      _queriesOnly(api);
    },
  );

  testWidgets(
    'row keyboard activation, cached reopen, read-only force refresh and focus return',
    (tester) async {
      final api = _ListApi()
        ..jobs.addAll([fixture.summary(), fixture.summary(_secondId)]);
      final jobs = await _mount(tester, api);
      final ink = find.descendant(
        of: _row(_secondId),
        matching: find.byType(InkWell),
      );
      final focus = tester.widget<InkWell>(ink).focusNode!;
      focus.requestFocus();
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(
        find.byKey(
          const ValueKey('dev-job-detail-44444444444444444444444444444444'),
        ),
        findsOneWidget,
      );
      final reads = api.gets.where((id) => id == _secondId).length;
      expect(reads, 1);
      await _close(tester);
      expect(focus.hasFocus, isTrue);
      await tester.sendKeyEvent(LogicalKeyboardKey.space);
      await tester.pumpAndSettle();
      expect(api.gets.where((id) => id == _secondId), hasLength(reads));
      await tester.tap(_textButton('Refresh details (read-only)'));
      await tester.pumpAndSettle();
      expect(api.gets.where((id) => id == _secondId), hasLength(reads + 1));
      expect(jobs.current!.requestId, fixture.requestId);
      _queriesOnly(api);
    },
  );

  testWidgets(
    'empty is distinct from unavailable without legacy count admission cap',
    (tester) async {
      final api = _ListApi()..used = 32;
      final jobs = await _mount(tester, api);
      expect(find.text('No jobs yet.'), findsOneWidget);
      expect(find.textContaining('including legacy records'), findsNothing);
      expect(find.textContaining('32'), findsNothing);
      expect(find.textContaining('capacity'), findsNothing);
      expect(
        find.byWidgetPredicate(
          (w) =>
              w.key is ValueKey<String> &&
              (w.key! as ValueKey<String>).value.startsWith('dev-job-'),
        ),
        findsNothing,
      );
      await _openNew(tester);
      await _draft(tester);
      expect(
        tester.widget<FilledButton>(_filled('Review start')).onPressed,
        isNotNull,
      );
      expect(jobs.canStart, isTrue);
      await _close(tester);
      api.stateFailure = const AppFailure(
        'APP_UNAVAILABLE',
        'Read unavailable',
      );
      await jobs.refresh();
      await tester.pumpAndSettle();
      expect(find.textContaining('Jobs may be out of date'), findsOneWidget);
      expect(find.text('No jobs yet.'), findsNothing);
      _queriesOnly(api);
    },
  );

  testWidgets('initial list failure is unavailable, never an empty list', (
    tester,
  ) async {
    final api = _ListApi()
      ..stateFailure = const AppFailure('APP_UNAVAILABLE', 'Read unavailable');
    await _mount(tester, api);
    expect(find.textContaining('Jobs unavailable.'), findsOneWidget);
    expect(find.text('No jobs yet.'), findsNothing);
    expect(_filled('New job'), findsOneWidget);
    _queriesOnly(api);
  });

  testWidgets(
    'failed refresh retains labelled stale rows and their readable details',
    (tester) async {
      final api = _ListApi()..jobs.add(fixture.summary());
      final jobs = await _mount(tester, api);
      api.stateFailure = const AppFailure(
        'APP_UNAVAILABLE',
        'Read unavailable',
      );
      await jobs.refresh();
      await tester.pumpAndSettle();
      expect(_row(fixture.requestId), findsOneWidget);
      expect(find.textContaining('Jobs may be out of date'), findsOneWidget);
      expect(find.text('No jobs yet.'), findsNothing);
      await _openDetail(tester, fixture.requestId);
      expect(find.text('A saved final reply.'), findsOneWidget);
      expect(jobs.canStart, isFalse);
      _queriesOnly(api);
    },
  );

  testWidgets(
    'late detail A cannot replace B; close/reopen coalesces A without new timers',
    (tester) async {
      final a = fixture.summary();
      final b = {...fixture.summary(_secondId), 'created_ms': 1700000001000};
      final api = _ListApi()..jobs.addAll([a, b]);
      api.savedDetails[_secondId] = {
        ...fixture.detail(b),
        'reply': 'Reply for B',
      };
      final jobs = await _mount(tester, api);
      final current = jobs.current;
      final gate = Completer<AppReply>();
      api.detailGates[fixture.requestId] = gate;
      await _openDetail(tester, fixture.requestId);
      expect(find.text('Loading saved job details…'), findsOneWidget);
      await _close(tester);
      await _openDetail(tester, fixture.requestId);
      expect(api.gets.where((id) => id == fixture.requestId), hasLength(1));
      await _close(tester);
      await _openDetail(tester, _secondId);
      expect(find.text('Reply for B'), findsOneWidget);
      gate.complete(
        AppReply(fixture.instance, {
          ...fixture.detail(a),
          'reply': 'Late reply for A',
        }),
      );
      await tester.pumpAndSettle();
      expect(find.text('Reply for B'), findsOneWidget);
      expect(find.text('Late reply for A'), findsNothing);
      expect(jobs.current, same(current));
      final calls = api.calls.length;
      await tester.pump(const Duration(seconds: 20));
      expect(api.calls, hasLength(calls));
      await _close(tester);
      await _openDetail(tester, fixture.requestId);
      expect(find.text('Late reply for A'), findsOneWidget);
      expect(api.gets.where((id) => id == fixture.requestId), hasLength(1));
      _queriesOnly(api);
    },
  );

  testWidgets(
    'keyed detail failure keeps one coherent stale snapshot and no invented repository absence',
    (tester) async {
      final a = fixture.summary();
      final b = {...fixture.summary(_secondId), 'created_ms': 1700000001000};
      final api = _ListApi()..jobs.addAll([a, b]);
      api.savedDetails[fixture.requestId] = {
        ...fixture.detail(a),
        'repositories': [],
      };
      final jobs = await _mount(tester, api);
      await _openDetail(tester, fixture.requestId);
      expect(
        find.textContaining('No recorded checkout evidence.'),
        findsOneWidget,
      );
      api.detailFailures[fixture.requestId] = const AppFailure(
        'APP_UNAVAILABLE',
        'Detail unavailable',
      );
      await tester.tap(_textButton('Refresh details (read-only)'));
      await tester.pumpAndSettle();
      expect(find.textContaining('Saved details are stale'), findsOneWidget);
      expect(find.textContaining('Detail unavailable'), findsOneWidget);
      expect(_inDialog(find.text('Phase: completed')), findsOneWidget);
      expect(find.text('A saved final reply.'), findsOneWidget);
      expect(jobs.detailState(_secondId).error, isNull);
      await _close(tester);
      await _openDetail(tester, _secondId);
      expect(find.textContaining('Detail unavailable'), findsNothing);
      _queriesOnly(api);
    },
  );

  for (final phase in ['running', 'unknown', 'cleanup_unknown']) {
    testWidgets(
      '$phase detail explains no stop, cleanup certainty and scheduler admission',
      (tester) async {
        final api = _ListApi()
          ..jobs.add(fixture.summary(fixture.requestId, phase));
        final jobs = await _mount(tester, api);
        await _openDetail(tester, fixture.requestId);
        expect(_inDialog(find.text('Phase: $phase')), findsOneWidget);
        expect(
          find.textContaining('Stop is not available in this client. Closing'),
          findsOneWidget,
        );
        expect(
          find.textContaining(
            'Cleanup not verified; reply availability is separate.',
          ),
          findsOneWidget,
        );
        if (phase == 'cleanup_unknown') {
          expect(find.text('A saved final reply.'), findsOneWidget);
        }
        expect(jobs.canStart, phase == 'running');
        await _close(tester);
        await _openNew(tester);
        await _draft(tester);
        expect(
          tester.widget<FilledButton>(_filled('Review start')).onPressed,
          phase == 'running' ? isNotNull : isNull,
        );
        _queriesOnly(api);
      },
    );
  }

  testWidgets(
    'active observation updates the open keyed detail and stops without modal polling',
    (tester) async {
      final api = _ListApi()
        ..jobs.add(fixture.summary(fixture.requestId, 'running'));
      await _mount(tester, api);
      await _openDetail(tester, fixture.requestId);
      expect(_inDialog(find.text('Phase: running')), findsOneWidget);
      expect(find.text('A saved final reply.'), findsNothing);
      api.jobs[0] = {...fixture.summary(), 'message': 'Final service message.'};
      await tester.pump(const Duration(seconds: 2));
      await tester.pumpAndSettle();
      expect(_inDialog(find.text('Phase: completed')), findsOneWidget);
      expect(find.text('Final service message.'), findsOneWidget);
      expect(find.text('A saved final reply.'), findsOneWidget);
      expect(find.text('Owned container stop verified.'), findsOneWidget);
      final reads = api.calls.length;
      await tester.pump(const Duration(seconds: 20));
      expect(api.calls, hasLength(reads));
      _queriesOnly(api);
    },
  );

  testWidgets(
    'IME/draft changes invalidate review; an obsolete Start cannot consume a replacement review',
    (tester) async {
      final api = _ListApi();
      await _mount(tester, api);
      await _openNew(tester);
      await _draft(tester);
      final editor = tester
          .widget<TextField>(find.byType(TextField))
          .controller!;
      await _review(tester);
      final obsoleteStart = tester
          .widget<FilledButton>(_filled('Start job'))
          .onPressed!;
      editor.value = const TextEditingValue(
        text: 'new exact\n task',
        selection: TextSelection.collapsed(offset: 3),
        composing: TextRange(start: 0, end: 3),
      );
      await tester.pumpAndSettle();
      obsoleteStart();
      await tester.pumpAndSettle();
      expect(api.starts, isEmpty);
      expect(
        tester.widget<FilledButton>(_filled('Review start')).onPressed,
        isNull,
      );
      editor.value = editor.value.copyWith(composing: TextRange.empty);
      await tester.pump();
      await _review(tester);
      obsoleteStart();
      await tester.pumpAndSettle();
      expect(api.starts, isEmpty);
      expect(_filled('Start job'), findsOneWidget);
      await tester.tap(_filled('Start job'));
      await tester.pumpAndSettle();
      expect(api.starts.single['prompt'], 'new exact\n task');
    },
  );

  testWidgets(
    'one named modal Edit -> Review -> Start; duplicate callback submits exactly once',
    (tester) async {
      final api = _ListApi()..startGate = Completer<void>();
      final routes = _Routes();
      await _mount(tester, api, routes: routes);
      await _openNew(tester);
      final route = routes.pushed.last;
      await _draft(tester);
      await tester.testTextInput.receiveAction(TextInputAction.newline);
      await tester.pump();
      expect(api.starts, isEmpty);
      await _review(tester);
      expect(routes.pushed.last, same(route));
      expect(routes.pushed, hasLength(1));
      expect(find.byType(AlertDialog), findsOneWidget);
      expect(find.textContaining('PUBLIC HTTPS'), findsOneWidget);
      expect(
        find.textContaining('full Git history and normal branches/tags'),
        findsOneWidget,
      );
      expect(
        find.textContaining('repositories that exceed them fail the job'),
        findsOneWidget,
      );
      expect(find.textContaining('Checkout ref: main'), findsOneWidget);
      expect(
        find.textContaining('No Smith Agent text is included'),
        findsOneWidget,
      );
      expect(
        find.textContaining('Job records and stopped containers are retained'),
        findsOneWidget,
      );
      final start = tester
          .widget<FilledButton>(_filled('Start job'))
          .onPressed!;
      start();
      start();
      await tester.pumpAndSettle();
      expect(api.starts, hasLength(1));
      expect(api.starts.single['prompt'], ' exact\n\t task ');
      expect(
        api.starts.single['expected_config_revision'],
        fixture.configRevision,
      );
      expect(routes.pushed.last, same(route));
      expect(_textButton('Close'), findsOneWidget);
      api.startGate!.complete();
      await tester.pumpAndSettle();
      expect(find.byType(AlertDialog), findsNothing);
      expect(_row(api.starts.single['request_id']), findsOneWidget);
      await _openNew(tester);
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text,
        isEmpty,
      );
      expect(
        tester
            .widget<DropdownButtonFormField<String>>(
              find.byType(DropdownButtonFormField<String>),
            )
            .initialValue,
        fixture.projectId,
      );
    },
  );

  testWidgets(
    'closed/reopened pending modal retains full newer editing value after late ack',
    (tester) async {
      final api = _ListApi()..startGate = Completer<void>();
      final routes = _Routes();
      await _mount(tester, api, routes: routes);
      await _openNew(tester);
      await _draft(tester);
      await _review(tester);
      await tester.tap(_filled('Start job'));
      await tester.pumpAndSettle();
      await _close(tester);
      await _openNew(tester);
      final reopened = routes.pushed.last;
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text,
        ' exact\n\t task ',
      );
      final editor = tester
          .widget<TextField>(find.byType(TextField))
          .controller!;
      const newer = TextEditingValue(
        text: 'newer\ncomposition',
        selection: TextSelection(baseOffset: 2, extentOffset: 5),
        composing: TextRange(start: 0, end: 5),
      );
      editor.value = newer;
      await tester.pump();
      api.startGate!.complete();
      await tester.pumpAndSettle();
      expect(routes.removed, isNot(contains(reopened)));
      expect(find.byType(AlertDialog), findsOneWidget);
      expect(editor.value, newer);
      expect(api.starts, hasLength(1));
      await _close(tester);
      await _openNew(tester);
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.value,
        newer,
      );
      expect(
        tester.widget<FilledButton>(_filled('Review start')).onPressed,
        isNull,
      );
    },
  );

  testWidgets(
    'known refusal and unknown admission keep exact drafts; no automatic resubmission',
    (tester) async {
      final api = _ListApi()..startFailure = const AppFailure('BUSY', 'Busy');
      final jobs = await _mount(tester, api);
      await _openNew(tester);
      await _draft(tester);
      await _review(tester);
      await tester.tap(_filled('Start job'));
      await tester.pumpAndSettle();
      expect(jobs.pendingRequestId, isNull);
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text,
        ' exact\n\t task ',
      );
      await _close(tester);
      await fixture.load(jobs.projects, jobs);
      await tester.pumpAndSettle();
      api.startFailure = const AppFailure(
        'TRANSPORT',
        'Lost ack',
        unknown: true,
      );
      await _openNew(tester);
      await _review(tester);
      await tester.tap(_filled('Start job'));
      await tester.pumpAndSettle();
      expect(jobs.pendingRequestId, api.starts.last['request_id']);
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text,
        ' exact\n\t task ',
      );
      expect(
        tester.widget<FilledButton>(_filled('Review start')).onPressed,
        isNull,
      );
      await _close(tester);
      await tester.tap(_textButton('Resolve original request'));
      await tester.pumpAndSettle();
      expect(api.starts, hasLength(2));
      expect(api.gets.last, api.starts.last['request_id']);
      expect(jobs.pendingRequestId, isNotNull);
      expect(
        find.byWidgetPredicate(
          (w) =>
              w.key is ValueKey<String> &&
              (w.key! as ValueKey<String>).value.startsWith('dev-job-'),
        ),
        findsNothing,
      );
    },
  );

  testWidgets(
    'project and model revision drift invalidate review and obsolete callbacks',
    (tester) async {
      final api = _ListApi();
      final jobs = await _mount(tester, api);
      await _openNew(tester);
      await _draft(tester);
      await _review(tester);
      final oldStart = tester
          .widget<FilledButton>(_filled('Start job'))
          .onPressed!;
      api.projectRevision++;
      await jobs.projects.refresh();
      await tester.pumpAndSettle();
      oldStart();
      await tester.pumpAndSettle();
      expect(api.starts, isEmpty);
      expect(_filled('Start job'), findsNothing);
      expect(find.textContaining('Saved origin changed'), findsOneWidget);
      await _review(tester);
      final modelStart = tester
          .widget<FilledButton>(_filled('Start job'))
          .onPressed!;
      api.localModel = fixture.model(revision: '2');
      await jobs.refresh();
      await tester.pumpAndSettle();
      modelStart();
      await tester.pumpAndSettle();
      expect(api.starts, isEmpty);
      expect(_filled('Start job'), findsNothing);
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text,
        ' exact\n\t task ',
      );
    },
  );

  testWidgets(
    'hiding origin removes its covered named modal, never the unrelated top route',
    (tester) async {
      final api = _ListApi();
      final visible = ValueNotifier(true);
      final navigator = GlobalKey<NavigatorState>();
      final routes = _Routes();
      addTearDown(visible.dispose);
      await _mount(
        tester,
        api,
        visible: visible,
        navigatorKey: navigator,
        routes: routes,
      );
      await _openNew(tester);
      await _draft(tester);
      await _review(tester);
      final origin = routes.pushed.last;
      final obsoleteStart = tester
          .widget<FilledButton>(_filled('Start job'))
          .onPressed!;
      final other = DialogRoute<void>(
        context: navigator.currentContext!,
        settings: const RouteSettings(name: 'unrelated-dialog'),
        builder: (context) => const AlertDialog(title: Text('Unrelated route')),
      );
      unawaited(navigator.currentState!.push(other));
      await tester.pumpAndSettle();
      visible.value = false;
      await tester.pumpAndSettle();
      obsoleteStart();
      await tester.pumpAndSettle();
      expect(routes.removed, contains(origin));
      expect(routes.removed, isNot(contains(other)));
      expect(find.text('Unrelated route'), findsOneWidget);
      expect(api.starts, isEmpty);
      visible.value = true;
      await tester.pumpAndSettle();
      navigator.currentState!.pop();
      await tester.pumpAndSettle();
      await _openNew(tester);
      expect(_filled('Start job'), findsNothing);
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text,
        ' exact\n\t task ',
      );
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'hiding a pending source detaches and late acknowledgement retains its draft',
    (tester) async {
      final api = _ListApi()..startGate = Completer<void>();
      final visible = ValueNotifier(true);
      addTearDown(visible.dispose);
      await _mount(tester, api, visible: visible);
      await _openNew(tester);
      await _draft(tester);
      await _review(tester);
      await tester.tap(_filled('Start job'));
      await tester.pumpAndSettle();
      visible.value = false;
      await tester.pumpAndSettle();
      expect(find.byType(AlertDialog), findsNothing);
      api.startGate!.complete();
      await tester.pumpAndSettle();
      visible.value = true;
      await tester.pumpAndSettle();
      await _openNew(tester);
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text,
        ' exact\n\t task ',
      );
      expect(_filled('Start job'), findsNothing);
      expect(api.starts, hasLength(1));
    },
  );

  testWidgets(
    'disposing a pending origin cannot close another route on late acknowledgement',
    (tester) async {
      final api = _ListApi()..startGate = Completer<void>();
      final alive = ValueNotifier(true);
      final navigator = GlobalKey<NavigatorState>();
      addTearDown(alive.dispose);
      await _mount(tester, api, alive: alive, navigatorKey: navigator);
      await _openNew(tester);
      await _draft(tester);
      await _review(tester);
      await tester.tap(_filled('Start job'));
      await tester.pumpAndSettle();
      unawaited(
        navigator.currentState!.push(
          DialogRoute<void>(
            context: navigator.currentContext!,
            builder: (context) =>
                const AlertDialog(title: Text('Unrelated route')),
          ),
        ),
      );
      await tester.pumpAndSettle();
      alive.value = false;
      await tester.pumpAndSettle();
      api.startGate!.complete();
      await tester.pumpAndSettle();
      expect(find.text('Unrelated route'), findsOneWidget);
      expect(api.starts, hasLength(1));
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('two views retain independent modal projects and exact drafts', (
    tester,
  ) async {
    final api = _ListApi();
    final projects = ProjectsController(api);
    final jobs = DevJobsController(api, projects);
    addTearDown(() {
      jobs.dispose();
      projects.dispose();
    });
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Row(
            children: [
              Expanded(
                child: DevJobsView(
                  key: const ValueKey('left'),
                  controller: jobs,
                ),
              ),
              Expanded(
                child: DevJobsView(
                  key: const ValueKey('right'),
                  controller: jobs,
                ),
              ),
            ],
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(_filled('New job').first);
    await tester.pumpAndSettle();
    await _draft(tester, 'left draft');
    await _close(tester);
    await tester.tap(_filled('New job').last);
    await tester.pumpAndSettle();
    expect(
      tester.widget<TextField>(find.byType(TextField)).controller!.text,
      isEmpty,
    );
    expect(
      tester
          .widget<DropdownButtonFormField<String>>(
            find.byType(DropdownButtonFormField<String>),
          )
          .initialValue,
      isNull,
    );
    await _draft(tester, 'right draft');
    await _close(tester);
    await tester.tap(_filled('New job').first);
    await tester.pumpAndSettle();
    expect(
      tester.widget<TextField>(find.byType(TextField)).controller!.text,
      'left draft',
    );
    expect(
      tester
          .widget<DropdownButtonFormField<String>>(
            find.byType(DropdownButtonFormField<String>),
          )
          .initialValue,
      fixture.projectId,
    );
    _queriesOnly(api);
    await tester.pumpWidget(const SizedBox());
    await tester.pump();
  });

  for (final width in [360.0, 400.0]) {
    testWidgets(
      '${width.toInt()}x400 text2 rows and short modals scroll with 48px actions',
      (tester) async {
        await tester.binding.setSurfaceSize(Size(width, 400));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final api = _ListApi()..jobs.add(fixture.summary());
        api.savedDetails[fixture.requestId] = {
          ...fixture.detail(fixture.summary()),
          'reply': List.filled(60, 'Selectable inert saved reply.\n').join(),
        };
        await _mount(tester, api, scale: 2);
        expect(
          tester.getSize(_filled('New job')).height,
          greaterThanOrEqualTo(48),
        );
        await _openDetail(tester, fixture.requestId);
        final reply = find.byWidgetPredicate(
          (w) =>
              w is SelectableText &&
              w.data?.startsWith('Selectable inert saved reply.') == true,
        );
        await tester.ensureVisible(reply);
        expect(
          tester.getSize(_textButton('Close')).height,
          greaterThanOrEqualTo(48),
        );
        expect(
          tester.getSize(_textButton('Refresh details (read-only)')).height,
          greaterThanOrEqualTo(48),
        );
        expect(tester.takeException(), isNull);
        await _close(tester);
        await _openNew(tester);
        await _draft(tester);
        expect(
          tester.getSize(_filled('Review start')).height,
          greaterThanOrEqualTo(48),
        );
        await _review(tester);
        expect(find.text('Start local Dev job?'), findsOneWidget);
        await tester.ensureVisible(find.text('Exact task:'));
        final disclosure = find.textContaining(
          'full Git history and normal branches/tags',
        );
        await tester.ensureVisible(disclosure);
        expect(disclosure, findsOneWidget);
        expect(tester.getSize(disclosure).width, lessThan(width));
        expect(
          tester.getSize(_filled('Start job')).height,
          greaterThanOrEqualTo(48),
        );
        expect(tester.takeException(), isNull);
        await _close(tester);
        _queriesOnly(api);
      },
    );
  }
}
