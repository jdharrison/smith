import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/instructions.dart';
import 'package:smith_client/main.dart';
import 'package:smith_client/ui/instructions_view.dart';
import 'package:smith_client/ui/settings_view.dart';
import 'package:smith_client/ui/smith_workspace.dart';
import 'package:smith_client/ui/split_workspace.dart';

import 'app_fakes.dart';
import 'instructions_controller_test.dart' show InstructionApi;
import 'instructions_view_test.dart'
    show
        instructionField,
        reviewAction,
        applyAction,
        tapInstructionAction,
        editInstructionField;
import 'widget_test.dart' show FakeControlApi, profile;

class _GlobalSettingsApi extends FakeAppApi {
  final instructions = InstructionApi();

  @override
  Future<AppReply> call(String op, Map<String, dynamic> args) async {
    if (op.startsWith('instructions_')) {
      calls.add(op);
      arguments.add(Map.of(args));
      final reply = await instructions.call(op, args);
      config['revision'] = instructions.revision;
      return reply;
    }
    return super.call(op, args);
  }
}

Finder inTile(int index, Finder target) => find.descendant(
  of: find.byKey(ValueKey('workspace-tile-$index')),
  matching: target,
);

Future<void> _mountShell(WidgetTester tester, _GlobalSettingsApi api) async {
  await tester.binding.setSurfaceSize(const Size(1800, 1100));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    SmithApp(appApi: api, controlApi: FakeControlApi(profile())),
  );
  await tester.pumpAndSettle();
}

void main() {
  testWidgets(
    'optional Settings and shell content preserve their read-only defaults',
    (tester) async {
      await tester.pumpWidget(
        const MaterialApp(
          home: Scaffold(
            body: SettingsView(
              connectionStatus: 'Connection',
              modelStatus: 'Model',
              version: 'Fixture',
            ),
          ),
        ),
      );
      expect(find.text('Connection & About'), findsOneWidget);
      expect(find.byType(TextField), findsNothing);
      expect(find.byType(InstructionsView), findsNothing);
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SmithWorkspace(
              title: 'Chat',
              version: 'Fixture',
              connectionStatus: 'Connection',
              modelStatus: 'Model',
              chats: const [],
              child: const Text('Retained chat'),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byTooltip('Settings'));
      await tester.pumpAndSettle();
      expect(find.text('Connection & About'), findsOneWidget);
      expect(find.byType(InstructionsView), findsNothing);
      expect(find.byType(TextField), findsNothing);
    },
  );

  testWidgets(
    'production Settings lazily reads and keeps controller draft across unmount',
    (tester) async {
      final api = _GlobalSettingsApi();
      await _mountShell(tester, api);
      expect(api.instructions.calls, isEmpty);
      expect(find.byType(InstructionsView), findsNothing);
      await tester.tap(find.byTooltip('Settings'));
      await tester.pumpAndSettle();
      expect(api.instructions.calls, ['instructions_get']);
      final controller = tester
          .widget<InstructionsView>(find.byType(InstructionsView))
          .controller;
      await editInstructionField(
        tester,
        'global-personality',
        'retained personality',
      );
      await editInstructionField(
        tester,
        'global-guardrails',
        'retained caution',
      );
      await tester.tap(find.byTooltip('Return to Chat'));
      await tester.pumpAndSettle();
      expect(find.byType(InstructionsView), findsNothing);
      expect(controller.draft.personality, 'retained personality');
      expect(api.disposedCount, 0);
      await tester.tap(find.byTooltip('Settings'));
      await tester.pumpAndSettle();
      expect(
        tester
            .widget<InstructionsView>(find.byType(InstructionsView))
            .controller,
        same(controller),
      );
      expect(
        tester
            .widget<TextField>(instructionField('global-personality'))
            .controller!
            .text,
        'retained personality',
      );
      expect(
        tester
            .widget<TextField>(instructionField('global-guardrails'))
            .controller!
            .text,
        'retained caution',
      );
      expect(api.instructions.calls, ['instructions_get', 'instructions_get']);
      expect(api.disposedCount, 0);
      await tester.pumpWidget(const SizedBox());
      expect(api.disposedCount, 1);
      expect(api.instructions.disposals, 0);
    },
  );

  testWidgets(
    'all panes share one global draft and Apply synchronizes chat state',
    (tester) async {
      final api = _GlobalSettingsApi();
      await _mountShell(tester, api);
      await tester.tap(find.byKey(const ValueKey('workspace-layout')));
      await tester.pumpAndSettle();
      await tester.tap(
        find.byWidgetPredicate(
          (w) =>
              w is CheckedPopupMenuItem<WorkspaceLayout> &&
              w.value == WorkspaceLayout.sideBySide,
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(inTile(0, find.byTooltip('Settings')));
      await tester.pumpAndSettle();
      await editInstructionField(
        tester,
        'global-instructions',
        'one shared draft',
      );
      await tester.tap(inTile(1, find.byTooltip('Settings')));
      await tester.pumpAndSettle();
      final views = tester
          .widgetList<InstructionsView>(find.byType(InstructionsView))
          .toList();
      expect(views, hasLength(2));
      expect(views[0].controller, same(views[1].controller));
      for (var i = 0; i < 2; i++) {
        expect(
          tester
              .widget<TextField>(
                inTile(i, instructionField('global-instructions')),
              )
              .controller!
              .text,
          'one shared draft',
        );
      }
      final stateReads = api.calls.where((op) => op == 'state').length;
      await tapInstructionAction(tester, inTile(1, reviewAction));
      await tapInstructionAction(tester, applyAction);
      expect(
        api.instructions.settings,
        const InstructionSettings(instructions: 'one shared draft'),
      );
      expect(views[0].controller.snapshot!.configRevision, 2);
      expect(views[0].controller.dirty, false);
      expect(
        api.calls.where((op) => op == 'state').length,
        greaterThan(stateReads),
      );
      expect(
        api.instructions.calls.where((op) => op == 'instructions_update'),
        hasLength(1),
      );
      expect(
        api.calls.where(
          (op) =>
              op.startsWith('submit') ||
              op == 'configure' ||
              op == 'provider_probe',
        ),
        isEmpty,
      );
      expect(api.disposedCount, 0);
      await tester.pumpWidget(const SizedBox());
      expect(api.disposedCount, 1);
    },
  );
}
