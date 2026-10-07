import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:smith_client/bridge/smith_bridge.dart';

void main() {
  test(
    'chat-only additive health observations decode without inventing model readiness',
    () async {
      final bridge = SmithBridge(
        client: MockClient(
          (request) async => http.Response(
            '{"ok":true,"service":"smith-bridge","profile":"chat-only","provider":"ollama","model":"fixture:local","provider_available":false}',
            200,
          ),
        ),
      );
      addTearDown(bridge.close);
      final health = await bridge.health();
      expect(health.profile, 'chat-only');
      expect(health.provider, 'ollama');
      expect(health.model, 'fixture:local');
      expect(health.providerAvailable, isFalse);
      expect(health.modelAvailable, isNull);
    },
  );

  test(
    'legacy health never invents chat-only profile or provider availability',
    () async {
      final bridge = SmithBridge(
        client: MockClient(
          (request) async =>
              http.Response('{"ok":true,"service":"smith-bridge"}', 200),
        ),
      );
      addTearDown(bridge.close);
      final health = await bridge.health();
      expect(health.profile, isNull);
      expect(health.providerAvailable, isNull);
      expect(health.modelAvailable, isNull);
    },
  );

  test('chat requests do not follow redirects', () async {
    final bridge = SmithBridge(
      client: MockClient((request) async {
        expect(request.followRedirects, isFalse);
        return http.Response(
          '{"error":"redirect rejected"}',
          307,
          headers: {'location': 'http://127.0.0.1:9/v1/chat'},
        );
      }),
    );
    addTearDown(bridge.close);
    await expectLater(
      bridge.chat('Explicit message'),
      throwsA(
        isA<SmithBridgeException>().having(
          (error) => error.statusCode,
          'status',
          307,
        ),
      ),
    );
  });

  test('health identity probe never follows redirects', () async {
    final client = MockClient((request) async {
      expect(request.method, 'GET');
      expect(request.url.path, '/v1/health');
      expect(request.followRedirects, isFalse);
      return http.Response(
        '',
        302,
        headers: const {'location': 'http://127.0.0.1:9/v1/health'},
      );
    });
    final bridge = SmithBridge(client: client);
    addTearDown(bridge.close);

    await expectLater(
      bridge.health(),
      throwsA(
        isA<SmithBridgeException>()
            .having((error) => error.statusCode, 'statusCode', 302)
            .having((error) => error.message, 'message', contains('302')),
      ),
    );
  });
}
