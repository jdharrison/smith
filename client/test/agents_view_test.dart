import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/agents.dart';
import 'package:smith_client/agents_controller.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/ui/agents_view.dart';
import 'package:smith_client/ui/smith_theme.dart';

import 'app_fakes.dart';

/// Synthetic metadata only: no processes, sockets, providers or filesystem.
class AgentsUiApi extends FakeAppApi {
  AgentsUiApi({bool custom = false}) {
    if (custom) records.add(customAgent());
  }
  static const customId = '11111111111111111111111111111111';
  static AgentRecord customAgent() => AgentRecord(
    agentId: customId,
    name: 'Custom',
    description: 'Custom saved profile',
    prompt: 'Custom prompt',
  );
  int revision = 1;
  int created = 0;
  bool loseAck = false;
  bool failList = false;
  Completer<void>? saveGate;
  final records = [
    AgentRecord(
      agentId: 'developer',
      name: 'Developer',
      description: 'Implements approved work',
      prompt: 'Purpose\nImplement and self-check.',
    ),
    AgentRecord(
      agentId: 'assurance',
      name: 'Assurance',
      description: 'Reviews evidence independently',
      prompt: '',
    ),
  ];
  final receipts = <String, Map<String, dynamic>>{};
  List<String> get agentCalls =>
      calls.where((op) => op.startsWith('agent')).toList();
  List<String> get writes => agentCalls
      .where((op) => op == 'agent_update_v2' || op == 'agent_create_v1')
      .toList();

  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    if (!op.startsWith('agent')) return super.call(op, args);
    calls.add(op);
    arguments.add(Map.of(args));
    validateAgentArguments(op, args);
    if (op == 'agents_list_v2') {
      if (failList) throw const AppFailure('TRANSPORT', 'Unavailable');
      return AppReply(instance, {
        'revision': revision,
        'agents': records.map((r) => r.toJson()).toList(),
      });
    }
    if (op == 'agent_receipt') {
      final receipt = receipts[args['request_id']];
      if (receipt == null) throw const AppFailure('REQUEST_UNKNOWN', 'Unknown');
      return AppReply(instance, receipt);
    }
    if (args['instance_id'] != instance ||
        args['expected_revision'] != revision) {
      throw const AppFailure('STALE_REVISION', 'Stale');
    }
    await saveGate?.future;
    final creating = op == 'agent_create_v1';
    if (creating && records.length >= maxAgents) {
      throw const AppFailure('CAPACITY', 'Full');
    }
    final id = creating
        ? (++created + 32).toRadixString(16).padLeft(32, '0')
        : args['agent_id'] as String;
    final index = records.indexWhere((r) => r.agentId == id);
    if (!creating && index < 0) {
      throw const AppFailure('AGENT_NOT_FOUND', 'Missing');
    }
    if (!creating &&
        records[index].isBuiltIn &&
        (records[index].name != args['name'] ||
            records[index].description != args['description'])) {
      throw const AppFailure(
        'INVALID_ARGUMENT',
        'Built-in metadata is immutable',
      );
    }
    final record = AgentRecord(
      agentId: id,
      name: args['name'],
      description: args['description'],
      prompt: args['prompt'],
    );
    if (creating) {
      records.add(record);
    } else {
      records[index] = record;
    }
    final receipt = <String, dynamic>{
      'request_id': args['request_id'],
      'agent_id': id,
      'revision': ++revision,
    };
    receipts[args['request_id']] = receipt;
    if (loseAck) {
      throw const AppFailure(
        'TRANSPORT',
        'Lost acknowledgement',
        unknown: true,
      );
    }
    return AppReply(instance, receipt);
  }
}

Finder agentKey(String key) => find.byKey(ValueKey(key));
Finder agentAction(String text) => find.widgetWithText(TextButton, text).first;
Future<void> agentTap(WidgetTester tester, Finder target) async {
  await tester.pumpAndSettle();
  await tester.ensureVisible(target);
  await tester.pumpAndSettle();
  await tester.tap(target);
  await tester.pumpAndSettle();
}

TextEditingController agentField(WidgetTester tester, String name) =>
    tester.widget<TextField>(agentKey('agent-$name')).controller!;

Future<AgentsController> mountAgents(
  WidgetTester tester,
  AgentsUiApi api, {
  Size size = const Size(1280, 900),
  double scale = 1,
  String fontFamily = SmithTheme.defaultFontFamily,
}) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  final controller = AgentsController(api);
  addTearDown(controller.dispose);
  await tester.pumpWidget(
    MaterialApp(
      theme: SmithTheme.dark(fontFamily: fontFamily),
      builder: (context, child) => MediaQuery(
        data: MediaQuery.of(context)
            .copyWith(textScaler: TextScaler.linear(scale)),
        child: child!,
      ),
      home: Scaffold(body: AgentsView(controller: controller)),
    ),
  );
  await tester.pumpAndSettle();
  return controller;
}

void main() {
  testWidgets(
    'new draft focuses Name, guards replacement and Cancel, saves actual profile',
    (tester) async {
      final api = AgentsUiApi();
      await mountAgents(tester, api);
      await agentTap(tester, agentAction('NEW AGENT'));
      expect(agentField(tester, 'name').text, '');
      expect(find.textContaining('ID:'), findsNothing);
      final nameEditable = tester.widget<EditableText>(
        find.descendant(
          of: agentKey('agent-name'),
          matching: find.byType(EditableText),
        ),
      );
      expect(nameEditable.focusNode.hasFocus, isTrue);
      expect(api.writes, isEmpty);
      await tester.enterText(agentKey('agent-name'), 'New profile');
      await tester.enterText(agentKey('agent-description'), 'New description');
      await tester.enterText(agentKey('agent-prompt'), 'Exact\n\tprompt\n');
      await agentTap(tester, agentAction('Cancel'));
      await agentTap(tester, agentAction('Keep editing'));
      expect(agentField(tester, 'name').text, 'New profile');
      await agentTap(tester, agentAction('NEW AGENT'));
      await agentTap(tester, agentAction('Keep editing'));
      expect(agentField(tester, 'prompt').text, 'Exact\n\tprompt\n');
      await agentTap(tester, agentAction('Save'));
      expect(api.records.length, 3);
      expect(api.records.last.name, 'New profile');
      expect(api.records.last.description, 'New description');
      expect(api.records.last.prompt, 'Exact\n\tprompt\n');
      final args = api.arguments.singleWhere(
        (a) => a.containsKey('expected_revision'),
      );
      expect(args.containsKey('agent_id'), isFalse);
      expect(args['request_id'], matches(RegExp(r'^[0-9a-f]{32}$')));
      expect(
        find.text('ID: ${api.records.last.agentId} (immutable)'),
        findsOneWidget,
      );
      expect(agentAction('Save Changes'), findsOneWidget);
      await agentTap(tester, agentAction('NEW AGENT'));
      await tester.enterText(agentKey('agent-name'), 'Discard me');
      await agentTap(tester, agentAction('Cancel'));
      await agentTap(tester, agentAction('Discard changes'));
      expect(find.text('New agent'), findsNothing);
      expect(api.records.length, 3);
      expect(api.writes, ['agent_create_v1']);
    },
  );

  testWidgets(
    'draft switch and saved edit New are guarded; draft restart requires review',
    (tester) async {
      final api = AgentsUiApi();
      final controller = await mountAgents(tester, api);
      await agentTap(tester, agentKey('agent-card-developer'));
      await tester.enterText(agentKey('agent-prompt'), 'Saved edit');
      await agentTap(tester, agentAction('NEW AGENT'));
      await agentTap(tester, agentAction('Keep editing'));
      expect(agentField(tester, 'prompt').text, 'Saved edit');
      await agentTap(tester, agentAction('NEW AGENT'));
      await agentTap(tester, agentAction('Discard changes'));
      await tester.enterText(agentKey('agent-name'), 'Draft');
      await agentTap(tester, agentKey('agent-card-assurance'));
      await agentTap(tester, agentAction('Keep editing'));
      expect(agentField(tester, 'name').text, 'Draft');
      api.instance = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';
      await controller.refresh();
      await tester.pumpAndSettle();
      expect(tester.widget<TextButton>(agentAction('Save')).onPressed, isNull);
      await agentTap(tester, agentAction('Review current metadata'));
      await agentTap(tester, agentAction('Keep edits at this revision'));
      expect(agentField(tester, 'name').text, 'Draft');
      await agentTap(tester, agentAction('Save'));
      expect(api.records.last.name, 'Draft');
      expect(api.writes, ['agent_create_v1']);
    },
  );

  testWidgets('32 saved profiles disable New but still allow prompt editing', (
    tester,
  ) async {
    final api = AgentsUiApi();
    for (var i = 0; i < maxAgents - 2; i++) {
      api.records.add(
        AgentRecord(
          agentId: i.toRadixString(16).padLeft(32, '0'),
          name: 'Profile $i',
          description: '',
          prompt: '',
        ),
      );
    }
    await mountAgents(tester, api);
    expect(
      tester.widget<TextButton>(agentAction('NEW AGENT')).onPressed,
      isNull,
    );
    expect(find.text('32-profile limit reached.'), findsOneWidget);
    await agentTap(tester, agentKey('agent-card-developer'));
    await tester.enterText(agentKey('agent-prompt'), 'Edited at capacity');
    await agentTap(tester, agentAction('Save Changes'));
    expect(api.records.first.prompt, 'Edited at capacity');
    expect(api.records.length, maxAgents);
    expect(api.writes, ['agent_update_v2']);
  });

  testWidgets(
    'built-in metadata is read only; legacy description allows prompt save',
    (tester) async {
      final api = AgentsUiApi();
      api.records[0] = AgentRecord(
        agentId: 'developer',
        name: 'Developer',
        description: 'L' * 256,
        prompt: 'Old prompt',
      );
      await mountAgents(tester, api);
      await agentTap(tester, agentKey('agent-card-developer'));
      expect(tester.widget<TextField>(agentKey('agent-name')).readOnly, isTrue);
      expect(
        tester.widget<TextField>(agentKey('agent-description')).readOnly,
        isTrue,
      );
      expect(
        tester.widget<TextField>(agentKey('agent-prompt')).readOnly,
        isFalse,
      );
      await tester.enterText(agentKey('agent-prompt'), 'New prompt');
      await agentTap(tester, agentAction('Save Changes'));
      expect(api.records.first.description, 'L' * 256);
      expect(api.records.first.prompt, 'New prompt');
      expect(api.writes, ['agent_update_v2']);
    },
  );

  for (final field in ['name', 'description']) {
    testWidgets(
      'custom $field validates 40/41 Unicode scalars without truncation',
      (tester) async {
        final api = AgentsUiApi();
        await mountAgents(tester, api);
        await agentTap(tester, agentAction('NEW AGENT'));
        await tester.enterText(agentKey('agent-name'), 'Profile');
        await tester.enterText(agentKey('agent-$field'), '😀' * 41);
        await agentTap(tester, agentAction('Save'));
        expect(api.writes, isEmpty);
        expect(agentField(tester, field).text.runes.length, 41);
        expect(
          tester
              .widget<TextField>(agentKey('agent-$field'))
              .decoration!
              .errorText,
          isNotNull,
        );
        await tester.enterText(agentKey('agent-$field'), '😀' * 40);
        await agentTap(tester, agentAction('Save'));
        expect(api.records.last.toJson()[field], '😀' * 40);
        expect(api.writes, ['agent_create_v1']);
      },
    );
  }

  testWidgets(
    'lost create acknowledgement binds original receipt only and retains newer typing across restart',
    (tester) async {
      final api = AgentsUiApi()..loseAck = true;
      final controller = await mountAgents(tester, api);
      await agentTap(tester, agentAction('NEW AGENT'));
      await tester.enterText(agentKey('agent-name'), 'Submitted');
      await agentTap(tester, agentAction('Save'));
      expect(controller.uncertain, isTrue);
      final requestId = api.receipts.keys.single;
      await tester.enterText(agentKey('agent-name'), 'Newer');
      expect(
        tester.widget<TextButton>(agentAction('NEW AGENT')).onPressed,
        isNull,
      );
      expect(tester.widget<TextButton>(agentAction('Save')).onPressed, isNull);
      api.instance = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';
      await agentTap(tester, agentAction('Resolve pending change'));
      expect(controller.lastReceipt!.requestId, requestId);
      expect(agentField(tester, 'name').text, 'Newer');
      expect(
        find.text('ID: ${api.receipts[requestId]!['agent_id']} (immutable)'),
        findsOneWidget,
      );
      expect(
        tester.widget<TextButton>(agentAction('Save Changes')).onPressed,
        isNull,
      );
      expect(api.writes, ['agent_create_v1']);
      await agentTap(tester, agentAction('Review current metadata'));
      await agentTap(tester, agentAction('Keep edits at this revision'));
      api.loseAck = false;
      await agentTap(tester, agentAction('Save Changes'));
      expect(api.records.length, 3);
      expect(api.records.last.name, 'Newer');
      expect(api.writes, ['agent_create_v1', 'agent_update_v2']);
    },
  );

  testWidgets(
    'matching acknowledged create after restart keeps original origin until review',
    (tester) async {
      final api = AgentsUiApi()..loseAck = true;
      final controller = await mountAgents(tester, api);
      await agentTap(tester, agentAction('NEW AGENT'));
      await tester.enterText(agentKey('agent-name'), 'Submitted');
      await agentTap(tester, agentAction('Save'));
      api.instance = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';
      await agentTap(tester, agentAction('Resolve pending change'));
      expect(agentField(tester, 'name').text, 'Submitted');
      expect(find.text('Review current metadata'), findsOneWidget);
      await controller.refresh();
      await tester.pumpAndSettle();
      expect(find.text('Review current metadata'), findsOneWidget);
      await tester.enterText(agentKey('agent-prompt'), 'Later edit');
      await tester.pump();
      expect(
        tester.widget<TextButton>(agentAction('Save Changes')).onPressed,
        isNull,
      );
      await agentTap(tester, agentAction('Review current metadata'));
      await agentTap(tester, agentAction('Keep edits at this revision'));
      expect(
        tester.widget<TextButton>(agentAction('Save Changes')).onPressed,
        isNotNull,
      );
      expect(api.writes, ['agent_create_v1']);
    },
  );

  testWidgets(
    'unknown create receipt never infers identity from matching fields',
    (tester) async {
      final api = AgentsUiApi()..loseAck = true;
      await mountAgents(tester, api);
      await agentTap(tester, agentAction('NEW AGENT'));
      await tester.enterText(agentKey('agent-name'), 'Same fields');
      await agentTap(tester, agentAction('Save'));
      api.receipts.clear();
      await agentTap(tester, agentAction('Resolve pending change'));
      expect(find.textContaining('ID:'), findsNothing);
      expect(
        tester.widget<TextButton>(agentAction('NEW AGENT')).onPressed,
        isNull,
      );
      expect(tester.widget<TextButton>(agentAction('Save')).onPressed, isNull);
      expect(api.writes, ['agent_create_v1']);
      expect(api.agentCalls.sublist(api.agentCalls.length - 2), [
        'agent_receipt',
        'agents_list_v2',
      ]);
    },
  );

  testWidgets(
    'acknowledged create missing from list stays duplicate-blocked until observed and reviewed',
    (tester) async {
      final api = AgentsUiApi()..loseAck = true;
      await mountAgents(tester, api);
      await agentTap(tester, agentAction('NEW AGENT'));
      await tester.enterText(agentKey('agent-name'), 'Submitted');
      await agentTap(tester, agentAction('Save'));
      final saved = api.records.removeLast();
      api.instance = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';
      await tester.enterText(agentKey('agent-name'), 'Newer');
      await agentTap(tester, agentAction('Resolve pending change'));
      expect(
        tester.widget<TextButton>(agentAction('NEW AGENT')).onPressed,
        isNull,
      );
      expect(tester.widget<TextButton>(agentAction('Save')).onPressed, isNull);
      expect(agentField(tester, 'name').text, 'Newer');
      api.records.add(saved);
      await agentTap(tester, agentAction('Resolve pending change'));
      expect(find.text('ID: ${saved.agentId} (immutable)'), findsOneWidget);
      expect(
        tester.widget<TextButton>(agentAction('Save Changes')).onPressed,
        isNull,
      );
      await agentTap(tester, agentAction('Review current metadata'));
      await agentTap(tester, agentAction('Keep edits at this revision'));
      expect(agentField(tester, 'name').text, 'Newer');
      expect(api.writes, ['agent_create_v1']);
    },
  );

  testWidgets(
    'typing during create preserves exact submission and requires review',
    (tester) async {
      final api = AgentsUiApi()..saveGate = Completer<void>();
      await mountAgents(tester, api);
      await agentTap(tester, agentAction('NEW AGENT'));
      await tester.enterText(agentKey('agent-name'), 'Submitted');
      await agentTap(tester, agentAction('Save'));
      await tester.enterText(agentKey('agent-name'), 'Newer');
      await tester.enterText(agentKey('agent-prompt'), 'Newer prompt');
      api.saveGate!.complete();
      await tester.pumpAndSettle();
      expect(api.records.last.name, 'Submitted');
      expect(api.records.last.prompt, '');
      expect(agentField(tester, 'name').text, 'Newer');
      expect(agentField(tester, 'prompt').text, 'Newer prompt');
      expect(
        tester.widget<TextButton>(agentAction('Save Changes')).onPressed,
        isNull,
      );
      await agentTap(tester, agentAction('Review current metadata'));
      await agentTap(tester, agentAction('Keep edits at this revision'));
      expect(
        tester.widget<TextButton>(agentAction('Save Changes')).onPressed,
        isNotNull,
      );
      expect(api.writes, ['agent_create_v1']);
    },
  );

  testWidgets(
    '400x400 text2 create retains composing selection and keyboard focus across layout and Back',
    (tester) async {
      final api = AgentsUiApi();
      await mountAgents(tester, api, size: const Size(400, 400), scale: 2);
      await agentTap(tester, agentAction('NEW AGENT'));
      await tester.enterText(agentKey('agent-name'), 'Composing');
      const value = TextEditingValue(
        text: 'Composing',
        selection: TextSelection(baseOffset: 2, extentOffset: 5),
        composing: TextRange(start: 0, end: 5),
      );
      tester.testTextInput.updateEditingValue(value);
      await tester.pump();
      final field = agentField(tester, 'name');
      final editable = find.descendant(
        of: agentKey('agent-name'),
        matching: find.byType(EditableText),
      );
      final state = tester.state<EditableTextState>(editable);
      for (final width in [2400.0, 400.0]) {
        tester.view.physicalSize = Size(width, 400);
        await tester.pumpAndSettle();
        expect(tester.state<EditableTextState>(editable), same(state));
        expect(state.widget.focusNode.hasFocus, isTrue);
        expect(field.value, value);
      }
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      expect(api.writes, isEmpty);
      await agentTap(tester, agentAction('Back to agents'));
      expect(state.widget.focusNode.canRequestFocus, isFalse);
      await agentTap(tester, agentAction('Return to agent editor'));
      expect(agentField(tester, 'name'), same(field));
      expect(field.text, 'Composing');
      expect(field.selection, value.selection);
      await agentTap(tester, agentAction('Save'));
      expect(api.records.last.name, 'Composing');
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'Retry loading exists only for unavailable observation and preserves draft',
    (tester) async {
      final api = AgentsUiApi()..failList = true;
      final controller = await mountAgents(tester, api);
      expect(find.text('Retry loading'), findsOneWidget);
      api.failList = false;
      await agentTap(tester, agentAction('Retry loading'));
      expect(find.text('Retry loading'), findsNothing);
      await agentTap(tester, agentAction('NEW AGENT'));
      await tester.enterText(agentKey('agent-name'), 'Retained');
      api.failList = true;
      await controller.refresh();
      await tester.pumpAndSettle();
      expect(find.text('Retry loading'), findsOneWidget);
      api.failList = false;
      await agentTap(tester, agentAction('Retry loading'));
      expect(agentField(tester, 'name').text, 'Retained');
      expect(api.writes, isEmpty);
    },
  );

  testWidgets('large editor is top-left anchored with bounded paired fields', (
    tester,
  ) async {
    final api = AgentsUiApi();
    await mountAgents(tester, api, size: const Size(2400, 1200));
    await agentTap(tester, agentKey('agent-card-developer'));
    final form = tester.getRect(agentKey('agent-editor-form'));
    expect(form.left, 384 + 16);
    expect(form.top, 16);
    expect(form.width, 960);
    final name = tester.getRect(agentKey('agent-name'));
    final description = tester.getRect(agentKey('agent-description'));
    expect(name.top, description.top);
    expect(description.left - name.right, 16);
    expect(find.byType(VerticalDivider), findsOneWidget);
    expect(find.text('Edit Agent'), findsNothing);
    expect(find.text('ID: developer (immutable)'), findsOneWidget);
    expect(find.text('Developer'), findsNWidgets(3));
    expect(find.text('Refresh agents'), findsNothing);
    expect(find.textContaining('Definitions only.'), findsNothing);
    for (final label in [
      'Enabled',
      'Run',
      'Test',
      'History',
      'Add',
      'Delete',
    ]) {
      expect(find.text(label), findsNothing);
    }
    expect(find.byType(Switch), findsNothing);
    expect(find.byType(TabBar), findsNothing);
    final card = tester.widget<OutlinedButton>(
      agentKey('agent-card-developer'),
    );
    expect(card.style!.side!.resolve({})!.color, SmithColors.accent);
    expect(
      card.style!.backgroundColor!.resolve({}),
      SmithColors.surfaceSelected,
    );
    expect(
      find.descendant(
        of: agentKey('agent-card-developer'),
        matching: find.text('developer'),
      ),
      findsNothing,
    );
    final icon = tester.getRect(
      find.descendant(
        of: agentKey('agent-card-developer'),
        matching: find.byIcon(Icons.code),
      ),
    );
    final title = tester.getRect(
      find.descendant(
        of: agentKey('agent-card-developer'),
        matching: find.text('Developer'),
      ),
    );
    expect(icon.right, lessThan(title.left));
    await tester.enterText(agentKey('agent-prompt'), 'Unsaved prompt');
    await tester.pump();
    expect(find.text('Developer'), findsNWidgets(3));
    final save = tester.getRect(agentAction('Save Changes'));
    final cancel = tester.getRect(agentAction('Cancel'));
    expect(save.right, form.right);
    expect(cancel.right, lessThan(save.left));
    expect(
      save.top,
      greaterThan(tester.getRect(agentKey('agent-prompt')).bottom),
    );
    expect(
      tester
          .widget<TextButton>(agentAction('Save Changes'))
          .style!
          .backgroundColor!
          .resolve({}),
      SmithColors.accent,
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'paired fields retain state selection and composing across resizing',
    (tester) async {
      await mountAgents(tester, AgentsUiApi(custom: true));
      await agentTap(tester, agentKey('agent-card-${AgentsUiApi.customId}'));
      await tester.enterText(
        agentKey('agent-description'),
        'Draft description',
      );
      const value = TextEditingValue(
        text: 'Draft description',
        selection: TextSelection(baseOffset: 2, extentOffset: 4),
        composing: TextRange(start: 0, end: 5),
      );
      tester.testTextInput.updateEditingValue(value);
      await tester.pump();
      expect(agentField(tester, 'description').value, value);
      final elements = [
        tester.element(agentKey('agent-name')),
        tester.element(agentKey('agent-description')),
      ];
      final editable = find.descendant(
        of: agentKey('agent-description'),
        matching: find.byType(EditableText),
      );
      final state = tester.state<EditableTextState>(editable);
      for (final width in [880.0, 400.0, 1280.0]) {
        tester.view.physicalSize = Size(width, 900);
        await tester.pumpAndSettle();
        expect(tester.element(agentKey('agent-name')), same(elements[0]));
        expect(
          tester.element(agentKey('agent-description')),
          same(elements[1]),
        );
        expect(tester.state<EditableTextState>(editable), same(state));
        expect(state.widget.focusNode.hasFocus, isTrue);
        expect(agentField(tester, 'description').value, value);
        final name = tester.getRect(agentKey('agent-name'));
        final description = tester.getRect(agentKey('agent-description'));
        if (width < 1000) {
          expect(description.top, greaterThan(name.bottom));
          expect(description.left, name.left);
        } else {
          expect(description.top, name.top);
        }
        expect(tester.takeException(), isNull);
      }
    },
  );

  testWidgets('exact prompt above 1000 characters saves within byte bound', (
    tester,
  ) async {
    final api = AgentsUiApi();
    await mountAgents(tester, api);
    await agentTap(tester, agentKey('agent-card-developer'));
    final prompt = '  Purpose\n\t${'é' * 1500}\n';
    await tester.enterText(agentKey('agent-prompt'), prompt);
    await tester.pump();
    expect(
      tester.widget<TextField>(agentKey('agent-prompt')).maxLength,
      isNull,
    );
    expect(
      tester
          .widget<EditableText>(
            find.descendant(
              of: agentKey('agent-prompt'),
              matching: find.byType(EditableText),
            ),
          )
          .style
          .fontFamily,
      SmithTheme.defaultFontFamily,
    );
    await agentTap(tester, agentAction('Save Changes'));
    expect(api.records.first.prompt, prompt);
    expect(api.writes.length, 1);
    expect(tester.takeException(), isNull);
  });

  testWidgets('prompt inherits the configured monospace font', (tester) async {
    await mountAgents(
      tester,
      AgentsUiApi(),
      fontFamily: SmithTheme.alternativeFontFamily,
    );
    await agentTap(tester, agentKey('agent-card-developer'));
    final editable = find.descendant(
      of: agentKey('agent-prompt'),
      matching: find.byType(EditableText),
    );
    expect(
      tester.widget<EditableText>(editable).style.fontFamily,
      SmithTheme.alternativeFontFamily,
    );
  });

  testWidgets('compact high DPI editor has no routine observation controls', (
    tester,
  ) async {
    final api = AgentsUiApi();
    await mountAgents(tester, api, size: const Size(800, 800), scale: 2);
    tester.view.devicePixelRatio = 2;
    await tester.pumpAndSettle();
    await agentTap(tester, agentKey('agent-card-developer'));
    await tester.enterText(agentKey('agent-prompt'), 'Retained');
    expect(find.text('Refresh agents'), findsNothing);
    expect(find.text('Search agents'), findsNothing);
    expect(find.text('Retry loading'), findsNothing);
    expect(
      tester.getRect(agentKey('agent-description')).top,
      greaterThan(tester.getRect(agentKey('agent-name')).bottom),
    );
    await agentTap(tester, agentAction('Save Changes'));
    expect(api.records.first.prompt, 'Retained');
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'all profiles remain visible without search; dirty switching is guarded',
    (tester) async {
      final api = AgentsUiApi();
      await mountAgents(tester, api);
      expect(api.agentCalls, ['agents_list_v2']);
      expect(find.byIcon(Icons.code), findsOneWidget);
      expect(find.byIcon(Icons.shield_outlined), findsOneWidget);
      expect(find.text('AGENTS'), findsOneWidget);
      expect(find.text('NEW AGENT'), findsOneWidget);
      expect(find.text('Agents'), findsNothing);
      expect(find.text('Search agents'), findsNothing);
      expect(find.text('Refresh agents'), findsNothing);
      expect(find.text('Prompt: Not configured'), findsOneWidget);
      await agentTap(tester, agentKey('agent-card-developer'));
      await tester.enterText(agentKey('agent-prompt'), 'Draft');
      await agentTap(tester, agentKey('agent-card-assurance'));
      await agentTap(tester, agentAction('Keep editing'));
      expect(agentField(tester, 'prompt').text, 'Draft');
      await agentTap(tester, agentKey('agent-card-assurance'));
      await agentTap(tester, agentAction('Discard changes'));
      expect(agentField(tester, 'prompt').text, '');
      expect(api.writes, isEmpty);
    },
  );

  testWidgets('all fields save exactly; multiline Enter and IME never submit', (
    tester,
  ) async {
    final api = AgentsUiApi(custom: true);
    await mountAgents(tester, api);
    await agentTap(tester, agentKey('agent-card-${AgentsUiApi.customId}'));
    await tester.enterText(agentKey('agent-name'), 'Builder');
    await tester.enterText(agentKey('agent-description'), 'Approved work only');
    await tester.enterText(agentKey('agent-prompt'), 'Exact\n\ttext\n');
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    tester.testTextInput.updateEditingValue(
      const TextEditingValue(
        text: 'Exact\n\ttext\n',
        selection: TextSelection.collapsed(offset: 12),
        composing: TextRange(start: 0, end: 5),
      ),
    );
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(api.writes, isEmpty);
    await agentTap(tester, agentAction('Save Changes'));
    expect(api.records.last.toJson(), {
      'agent_id': AgentsUiApi.customId,
      'name': 'Builder',
      'description': 'Approved work only',
      'prompt': 'Exact\n\ttext\n',
    });
    expect(find.text('Unsaved changes'), findsNothing);
    expect(api.writes.length, 1);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'same revision restart fences old callbacks; all-field review rebinds explicitly',
    (tester) async {
      final api = AgentsUiApi(custom: true);
      final controller = await mountAgents(tester, api);
      await agentTap(tester, agentKey('agent-card-${AgentsUiApi.customId}'));
      await tester.enterText(agentKey('agent-name'), 'Draft name');
      await tester.enterText(
        agentKey('agent-description'),
        'Draft description',
      );
      await tester.enterText(agentKey('agent-prompt'), 'Draft prompt');
      await tester.pump();
      final oldSave = tester
          .widget<TextButton>(agentAction('Save Changes'))
          .onPressed!;
      api.instance = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';
      await controller.refresh();
      await tester.pumpAndSettle();
      oldSave();
      await tester.pumpAndSettle();
      expect(api.writes, isEmpty);
      expect(
        tester.widget<TextButton>(agentAction('Save Changes')).onPressed,
        isNull,
      );
      await agentTap(tester, agentAction('Review current metadata'));
      for (final label in ['Name', 'Description', 'Prompt']) {
        expect(find.text('$label — currently saved'), findsOneWidget);
        expect(find.text('$label — your unsaved edits'), findsOneWidget);
      }
      // An already-open comparison cannot authorize a second drift.
      api.revision++;
      await controller.refresh();
      await tester.pumpAndSettle();
      expect(
        tester
            .widget<TextButton>(agentAction('Keep edits at this revision'))
            .onPressed,
        isNull,
      );
      await agentTap(tester, agentAction('Keep blocked'));
      await agentTap(tester, agentAction('Review current metadata'));
      await agentTap(tester, agentAction('Keep edits at this revision'));
      await agentTap(tester, agentAction('Save Changes'));
      expect(api.writes.length, 1);
      final args = api.arguments.singleWhere(
        (a) => a.containsKey('expected_revision'),
      );
      expect(args['instance_id'], api.instance);
      expect(args['expected_revision'], 2);
      expect(api.records.last.prompt, 'Draft prompt');
    },
  );

  testWidgets(
    'unknown receipt remains query-only even with matching saved values',
    (tester) async {
      final api = AgentsUiApi()..loseAck = true;
      final controller = await mountAgents(tester, api);
      await agentTap(tester, agentKey('agent-card-developer'));
      await tester.enterText(agentKey('agent-prompt'), 'Possibly saved');
      await agentTap(tester, agentAction('Save Changes'));
      expect(controller.uncertain, isTrue);
      api.receipts.clear();
      await tester.enterText(agentKey('agent-prompt'), 'Newer draft');
      await agentTap(tester, agentAction('Resolve pending change'));
      expect(api.agentCalls.sublist(api.agentCalls.length - 2), [
        'agent_receipt',
        'agents_list_v2',
      ]);
      expect(controller.uncertain, isTrue);
      expect(agentField(tester, 'prompt').text, 'Newer draft');
      await agentTap(tester, agentAction('Resolve pending change'));
      expect(controller.uncertain, isTrue);
      expect(api.writes.length, 1);
      expect(
        tester.widget<TextButton>(agentAction('Save Changes')).onPressed,
        isNull,
      );
    },
  );

  testWidgets(
    'typing during save survives acknowledgement and requires review',
    (tester) async {
      final api = AgentsUiApi(custom: true)..saveGate = Completer<void>();
      await mountAgents(tester, api);
      await agentTap(tester, agentKey('agent-card-${AgentsUiApi.customId}'));
      await tester.enterText(agentKey('agent-name'), 'Submitted');
      await agentTap(tester, agentAction('Save Changes'));
      await tester.enterText(agentKey('agent-name'), 'Newer');
      await tester.enterText(
        agentKey('agent-description'),
        'Newer description',
      );
      await tester.enterText(agentKey('agent-prompt'), 'Newer prompt');
      api.saveGate!.complete();
      await tester.pumpAndSettle();
      expect(api.records.last.name, 'Submitted');
      expect(agentField(tester, 'name').text, 'Newer');
      expect(agentField(tester, 'description').text, 'Newer description');
      expect(agentField(tester, 'prompt').text, 'Newer prompt');
      expect(
        tester.widget<TextButton>(agentAction('Save Changes')).onPressed,
        isNull,
      );
      await agentTap(tester, agentAction('Review current metadata'));
      await agentTap(tester, agentAction('Keep edits at this revision'));
      expect(
        tester.widget<TextButton>(agentAction('Save Changes')).onPressed,
        isNotNull,
      );
    },
  );

  testWidgets(
    '400x400 enlarged Back Return retains text selection, excludes hidden focus and reaches actions',
    (tester) async {
      final api = AgentsUiApi();
      await mountAgents(tester, api, size: const Size(400, 400), scale: 2);
      await agentTap(tester, agentKey('agent-card-developer'));
      await tester.enterText(agentKey('agent-prompt'), 'Retained prompt');
      final field = agentField(tester, 'prompt');
      field.selection = const TextSelection(baseOffset: 2, extentOffset: 7);
      final editable = find.descendant(
        of: agentKey('agent-prompt'),
        matching: find.byType(EditableText),
      );
      final focus = tester.widget<EditableText>(editable).focusNode;
      expect(focus.hasFocus, isTrue);
      await agentTap(tester, agentAction('Back to agents'));
      expect(focus.hasFocus, isFalse);
      expect(focus.canRequestFocus, isFalse);
      expect(find.text('Search agents'), findsNothing);
      await agentTap(tester, agentAction('Return to agent editor'));
      expect(identical(field, agentField(tester, 'prompt')), isTrue);
      expect(field.text, 'Retained prompt');
      expect(
        field.selection,
        const TextSelection(baseOffset: 2, extentOffset: 7),
      );
      await agentTap(tester, agentAction('Save Changes'));
      expect(api.writes.length, 1);
      await agentTap(tester, agentAction('Cancel'));
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'validation rejects every field without dispatch, blank prompt remains valid',
    (tester) async {
      final api = AgentsUiApi(custom: true);
      await mountAgents(tester, api);
      await agentTap(tester, agentKey('agent-card-${AgentsUiApi.customId}'));
      for (final (field, invalid) in [
        ('name', ' untrimmed'),
        ('description', 'two\u2028lines'),
        ('prompt', 'x' * 4097),
      ]) {
        final original = agentField(tester, field).text;
        await tester.enterText(agentKey('agent-$field'), invalid);
        await agentTap(tester, agentAction('Save Changes'));
        expect(api.writes, isEmpty);
        expect(
          tester
              .widget<TextField>(agentKey('agent-$field'))
              .decoration!
              .errorText,
          isNotNull,
        );
        await tester.enterText(agentKey('agent-$field'), original);
      }
      await tester.enterText(agentKey('agent-prompt'), '');
      await agentTap(tester, agentAction('Save Changes'));
      expect(api.records.last.prompt, '');
      expect(find.text('Prompt: Not configured'), findsNWidgets(2));
    },
  );
}
