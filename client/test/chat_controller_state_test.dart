import 'dart:async';
import 'dart:collection';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/bridge/smith_bridge.dart';
import 'package:smith_client/chat/chat_controller.dart';

BoardTask boardTask(String id, String status) => BoardTask(
  id: id,
  title: 'Task $id',
  kind: 'manual',
  stage: status == 'running' ? 'production' : 'new',
  status: status,
);

class FakeSmithBridge extends SmithBridge {
  FakeSmithBridge({this.healthService = 'smith-bridge'});

  final String healthService;
  final List<String> operations = [];
  final List<bool> chatNewSessionValues = [];
  final List<Completer<List<BoardTask>>> taskRequests = [];
  final Map<String, Completer<BoardTask>> detailRequests = {};
  final Queue<Future<List<BoardTask>>> scriptedTaskResults = Queue();
  List<BoardTask>? immediateTasks;

  @override
  Future<HealthInfo> health({
    Duration timeout = const Duration(seconds: 2),
  }) async {
    operations.add('health');
    return HealthInfo(
      ok: true,
      service: healthService,
      echo: false,
      isolation: 'sealed',
    );
  }

  @override
  Future<List<BoardTask>> listTasks({
    bool activeOnly = false,
    int? since,
    Duration timeout = const Duration(seconds: 5),
  }) {
    operations.add('listTasks');
    if (scriptedTaskResults.isNotEmpty) {
      return scriptedTaskResults.removeFirst();
    }
    if (immediateTasks != null) {
      return Future.value(immediateTasks);
    }
    final request = Completer<List<BoardTask>>();
    taskRequests.add(request);
    return request.future;
  }

  @override
  Future<BoardTask> getTask(
    String id, {
    Duration timeout = const Duration(seconds: 5),
  }) {
    final request = Completer<BoardTask>();
    detailRequests[id] = request;
    return request.future;
  }

  @override
  Future<ChatReply> chat(
    String text, {
    bool newSession = false,
    String session = 'default',
    Duration timeout = const Duration(minutes: 5),
  }) async {
    operations.add('chat');
    chatNewSessionValues.add(newSession);
    return ChatReply(text: 'Ready', session: session);
  }

  @override
  Future<void> ensureSidecars({
    Duration timeout = const Duration(seconds: 120),
  }) async {
    operations.add('ensureSidecars');
  }

  @override
  void close() {}
}

void main() {
  test('startup performs bridge health and task reads only', () async {
    final bridge = FakeSmithBridge()..immediateTasks = [];
    final controller = ChatController(bridge: bridge);
    addTearDown(controller.dispose);

    await controller.start();

    expect(bridge.operations, ['health', 'listTasks']);
    expect(controller.messages, isEmpty);
  });

  test('healthy non-Smith listener is not treated as the bridge', () async {
    final bridge = FakeSmithBridge(healthService: 'other-service')
      ..immediateTasks = [];
    final controller = ChatController(bridge: bridge);
    addTearDown(controller.dispose);

    await controller.start();

    expect(controller.bridgeStatus, BridgeStatus.offline);
    expect(bridge.operations, ['health', 'health']);
  });

  test('new session reset is local until the next explicit send', () async {
    final bridge = FakeSmithBridge()..immediateTasks = [];
    final controller = ChatController(bridge: bridge);
    addTearDown(controller.dispose);
    await controller.refreshHealth();
    controller.beginVoiceUserMessage();
    bridge.operations.clear();

    await controller.requestNewSession();

    expect(bridge.operations, isEmpty);
    expect(controller.messages, isEmpty);

    await controller.send('Explicit first message');
    expect(bridge.operations, ['chat']);
    expect(bridge.chatNewSessionValues, [true]);
  });

  test('latest task refresh wins when responses arrive out of order', () async {
    final bridge = FakeSmithBridge();
    final controller = ChatController(bridge: bridge);
    addTearDown(controller.dispose);
    await controller.refreshHealth();

    final first = controller.refreshTasks();
    final second = controller.refreshTasks();
    expect(bridge.taskRequests, hasLength(2));

    bridge.taskRequests[1].complete([boardTask('newer', 'completed')]);
    await second;
    bridge.taskRequests[0].complete([boardTask('older', 'running')]);
    await first;

    expect(controller.tasks.single.id, 'newer');
    expect(controller.pollingTasks, isFalse);
  });

  test(
    'latest selected task detail wins when responses arrive out of order',
    () async {
      final bridge = FakeSmithBridge();
      final controller = ChatController(bridge: bridge);
      addTearDown(controller.dispose);
      final taskA = boardTask('a', 'running');
      final taskB = boardTask('b', 'queued');

      controller.selectTask(taskA);
      final first = controller.openTaskDetail('a');
      controller.selectTask(taskB);
      final second = controller.openTaskDetail('b');

      bridge.detailRequests['b']!.complete(boardTask('b', 'completed'));
      await second;
      bridge.detailRequests['a']!.complete(boardTask('a', 'failed'));
      await first;

      expect(controller.selectedTask?.id, 'b');
      expect(controller.selectedTask?.status, 'completed');
    },
  );

  test('startup snapshot with running work enables polling', () async {
    final bridge = FakeSmithBridge()
      ..immediateTasks = [boardTask('running', 'running')];
    final controller = ChatController(bridge: bridge);
    addTearDown(controller.dispose);

    await controller.start();

    expect(controller.pollingTasks, isTrue);
    expect(controller.tasks.single.status, 'running');
  });

  test('failed background poll marks running snapshot stale', () async {
    final pollFailure = Completer<List<BoardTask>>();
    final bridge = FakeSmithBridge()
      ..scriptedTaskResults.add(Future.value([boardTask('running', 'running')]))
      ..scriptedTaskResults.add(pollFailure.future);
    final controller = ChatController(bridge: bridge);
    addTearDown(controller.dispose);

    await controller.start();
    pollFailure.completeError(Exception('poll unavailable'));
    await Future<void>.delayed(const Duration(milliseconds: 20));

    expect(controller.tasks.single.status, 'running');
    expect(controller.tasksError, contains('Background task refresh failed'));
  });

  test('startup background poll reconciles without speech callback', () async {
    final bridge = FakeSmithBridge()
      ..scriptedTaskResults.add(Future.value([boardTask('running', 'running')]))
      ..scriptedTaskResults.add(
        Future.value([boardTask('running', 'completed')]),
      )
      ..scriptedTaskResults.add(
        Future.value([boardTask('running', 'completed')]),
      );
    final spoken = <String>[];
    final controller = ChatController(bridge: bridge)
      ..onSpeakAssistant = (text) async => spoken.add(text);
    addTearDown(controller.dispose);

    await controller.start();
    await Future<void>.delayed(const Duration(milliseconds: 20));

    expect(controller.tasks.single.status, 'completed');
    expect(controller.tasksError, isNull);
    expect(controller.pollingTasks, isFalse);
    expect(spoken, isEmpty);
  });
}
