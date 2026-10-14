import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/control_api.dart';
import 'package:smith_client/native_control.dart';

import 'app_fakes.dart';
import 'native_control_test.dart' show FakeProcess, flush;

Map<String, dynamic> envelope(String id, {bool ok = true}) => {
  'schema_version': 1,
  'service': 'smith-next',
  'protocol': 'app',
  'pid': 123,
  'instance_id': testInstance,
  'call_id': id,
  'ok': ok,
  'result': ok ? {'config': {}, 'history': []} : null,
  'error': ok ? null : {'code': 'BUSY', 'message': 'busy'},
};

class RecordingProcess extends FakeProcess {
  RecordingProcess() {
    frames.stream.listen(
      bytes.addAll,
      onDone: () => received.complete(utf8.decode(bytes)),
    );
  }
  final frames = StreamController<List<int>>();
  final bytes = <int>[];
  final received = Completer<String>();
  late final IOSink sink = IOSink(frames.sink);
  @override
  IOSink get stdin => sink;
}

class ReplyExecutor implements AppExecutor {
  String? op;
  Map<String, dynamic>? args;
  @override
  Future<String> exchange(List<int> frame) async {
    expect(frame.last, 10);
    expect(frame.length, lessThanOrEqualTo(appFrameLimit + 1));
    final req = jsonDecode(utf8.decode(frame));
    op = req['op'];
    args = Map<String, dynamic>.from(req['args']);
    expect(req['call_id'], matches(RegExp(r'^[0-9a-f]{32}$')));
    expect(req['protocol'], 'app');
    return jsonEncode(envelope(req['call_id']));
  }

  @override
  void dispose() {}
}

void main() {
  test(
    'app stdin frame, fixed argv, response above Core cap supported',
    () async {
      final child = RecordingProcess();
      List<String>? argv;
      final helper = NativeHelperExecutor.forTesting(
        validate: () async => '/test/native',
        launch: (path, args) async {
          argv = args;
          return child;
        },
      );
      final api = NativeAppApi(executor: NativeAppExecutor(helper: helper));
      final work = api.call('submit', {
        'request_id': testInstance,
        'text': 'private prompt',
        'output': 'chat',
      });
      final frame = await child.received.future;
      final request = jsonDecode(frame);
      expect(argv, ['app']);
      expect(frame.endsWith('\n'), true);
      expect(request['args']['text'], 'private prompt');
      final reply = envelope(request['call_id']);
      reply['result'] = {'text': 'x' * 20000};
      child.finish(jsonEncode(reply));
      expect((await work).result['text'].length, 20000);
      expect(helperOutputLimit, 16384);
      api.dispose();
      expect(child.signals, isEmpty);
    },
  );
  test('app output over4MiB kills only captured helper', () async {
    final child = FakeProcess();
    final executor = NativeAppExecutor(
      helper: NativeHelperExecutor.forTesting(
        validate: () async => '/test/native',
        launch: (_, _) async => child,
      ),
    );
    final work = executor.exchange(utf8.encode('{}\n'));
    final check = expectLater(work, throwsA(isA<ControlFailure>()));
    await flush();
    child.out.add(List.filled(appFrameLimit + 2, 32));
    await check;
    expect(child.signals, [ProcessSignal.sigterm]);
    executor.dispose();
  });
  test(
    'API bounds input and rejects wrong call or instance identity',
    () async {
      final executor = ReplyExecutor();
      final api = NativeAppApi(executor: executor);
      await api.call('state', {});
      expect(executor.op, 'state');
      await expectLater(
        api.call('submit', {'text': 'x' * appFrameLimit}),
        throwsA(isA<AppFailure>()),
      );
      final bad = envelope(testInstance)
        ..['call_id'] = 'ffffffffffffffffffffffffffffffff';
      expect(
        () => AppReply.parse(jsonEncode(bad), testInstance),
        throwsA(isA<AppFailure>()),
      );
      final wrong = envelope(testInstance)
        ..['instance_id'] = 'uuid-with-hyphens';
      expect(
        () => AppReply.parse(jsonEncode(wrong), testInstance),
        throwsA(isA<AppFailure>()),
      );
      expect(
        () => AppReply.parse(
          jsonEncode(envelope(testInstance, ok: false)),
          testInstance,
        ),
        throwsA(isA<AppFailure>().having((e) => e.code, 'code', 'BUSY')),
      );
      api.dispose();
    },
  );
}
