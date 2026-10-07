import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/bridge/smith_bridge.dart';
import 'package:smith_client/chat/chat_controller.dart';
import 'package:smith_client/home/chat_view.dart';
import 'package:smith_client/home/halo_presentation.dart';
import 'package:smith_client/home/halo_theme.dart';
import 'package:smith_client/home/halo_widget.dart';
import 'package:smith_client/home/home_screen.dart';
import 'package:smith_client/home/tasks_tab.dart';

Widget testApp(Widget child) => MaterialApp(
  theme: buildHaloTheme(),
  home: Scaffold(body: Center(child: child)),
);

class RecordingHarness extends StatefulWidget {
  const RecordingHarness({super.key, required this.focusNode});

  final FocusNode focusNode;

  @override
  State<RecordingHarness> createState() => _RecordingHarnessState();
}

class _RecordingHarnessState extends State<RecordingHarness> {
  bool active = false;

  @override
  Widget build(BuildContext context) {
    return RecordingControlButton(
      active: active,
      enabled: true,
      focusNode: widget.focusNode,
      onStart: () => setState(() => active = true),
      onStop: () => setState(() => active = false),
    );
  }
}

class DestinationHarness extends StatefulWidget {
  const DestinationHarness({super.key});

  @override
  State<DestinationHarness> createState() => _DestinationHarnessState();
}

class _DestinationHarnessState extends State<DestinationHarness> {
  int index = 0;

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        TextButton(
          key: const Key('switch-destination'),
          onPressed: () => setState(() => index = index == 0 ? 1 : 0),
          child: const Text('Switch'),
        ),
        Expanded(
          child: PersistentDestinationStack(
            index: index,
            children: const [
              TextField(key: Key('persistent-draft')),
              Text('Second destination'),
            ],
          ),
        ),
      ],
    );
  }
}

void main() {
  testWidgets('production composer reflows on compact Home without overflow', (
    tester,
  ) async {
    final controller = ChatController();
    final draft = TextEditingController(text: 'preserved draft');
    addTearDown(controller.dispose);
    addTearDown(draft.dispose);
    await tester.pumpWidget(
      testApp(
        SizedBox(
          width: 312,
          child: HomeControlCluster(
            speakerControl: const SizedBox.square(dimension: 48),
            composer: SmithComposer(
              chat: controller,
              voice: null,
              draft: draft,
              expanded: true,
              onExpandedChanged: (_) {},
              recordingControl: RecordingControlButton(
                active: false,
                enabled: false,
                onStart: () {},
                onStop: () {},
              ),
            ),
          ),
        ),
      ),
    );

    expect(tester.takeException(), isNull);
    expect(find.byType(RecordingControlButton), findsOneWidget);
    expect(find.text('preserved draft'), findsOneWidget);
  });

  testWidgets('recording control supports pointer and keyboard start-stop', (
    tester,
  ) async {
    final focusNode = FocusNode();
    addTearDown(focusNode.dispose);
    await tester.pumpWidget(testApp(RecordingHarness(focusNode: focusNode)));

    expect(find.byTooltip('Start recording'), findsOneWidget);
    focusNode.requestFocus();
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(find.byTooltip('Stop recording'), findsOneWidget);

    await tester.tap(find.byType(RecordingControlButton));
    await tester.pump();
    expect(find.byTooltip('Start recording'), findsOneWidget);
  });

  testWidgets('destination stack preserves draft state', (tester) async {
    await tester.pumpWidget(testApp(const DestinationHarness()));
    await tester.enterText(
      find.byKey(const Key('persistent-draft')),
      'kept draft',
    );

    await tester.tap(find.byKey(const Key('switch-destination')));
    await tester.pump();
    expect(find.text('Second destination'), findsOneWidget);

    await tester.tap(find.byKey(const Key('switch-destination')));
    await tester.pump();
    expect(find.text('kept draft'), findsOneWidget);
  });

  testWidgets('decorative Halo does not announce a status', (tester) async {
    final semantics = tester.ensureSemantics();
    await tester.pumpWidget(
      testApp(
        const Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            SmithHalo(state: HaloState.failed, size: 80),
            SmithHalo(state: HaloState.idle, size: 32, semanticStatus: false),
          ],
        ),
      ),
    );

    expect(
      find.bySemanticsLabel('Smith status: Recovery needed'),
      findsOneWidget,
    );
    expect(find.bySemanticsLabel('Smith status: Ready'), findsNothing);
    semantics.dispose();
  });

  testWidgets('voice error is visible and retryable', (tester) async {
    var retries = 0;
    await tester.pumpWidget(
      testApp(
        VoiceErrorNotice(
          message: 'Microphone permission denied',
          onRetry: () => retries += 1,
        ),
      ),
    );

    expect(find.text('Microphone permission denied'), findsOneWidget);
    await tester.tap(find.text('Retry'));
    expect(retries, 1);
  });

  testWidgets('compact task detail exposes stale and detail errors', (
    tester,
  ) async {
    final item = BoardTask(
      id: 'task-1',
      title: 'Task',
      kind: 'manual',
      stage: 'production',
      status: 'running',
    );
    await tester.pumpWidget(
      testApp(
        SizedBox(
          width: 360,
          height: 700,
          child: TaskDetail(
            task: item,
            refreshError: 'poll unavailable',
            detailError: 'detail unavailable',
          ),
        ),
      ),
    );

    expect(
      find.textContaining('Task details could not be refreshed'),
      findsOneWidget,
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('task row remains usable with enlarged text', (tester) async {
    final item = BoardTask(
      id: 'task-1',
      title: 'A long task title that remains readable at enlarged text sizes',
      kind: 'manual',
      stage: 'production',
      status: 'running',
    );
    await tester.pumpWidget(
      MediaQuery(
        data: const MediaQueryData(textScaler: TextScaler.linear(2)),
        child: testApp(
          SizedBox(
            width: 360,
            child: TaskListRow(task: item, selected: true, onTap: () {}),
          ),
        ),
      ),
    );

    expect(tester.takeException(), isNull);
    expect(find.text('Running · Production'), findsOneWidget);
  });

  test('chat auto-scroll requires a new message and near-latest position', () {
    expect(
      shouldAutoScrollChat(
        previousMessageCount: 3,
        currentMessageCount: 3,
        extentAfter: 0,
      ),
      isFalse,
    );
    expect(
      shouldAutoScrollChat(
        previousMessageCount: 3,
        currentMessageCount: 4,
        extentAfter: 500,
      ),
      isFalse,
    );
    expect(
      shouldAutoScrollChat(
        previousMessageCount: 3,
        currentMessageCount: 4,
        extentAfter: 40,
      ),
      isTrue,
    );
  });
}
