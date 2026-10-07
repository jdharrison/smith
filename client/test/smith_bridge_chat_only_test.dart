import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:smith_client/bridge/smith_bridge.dart';

Map<String, dynamic> textReply() => {
  'v': 1,
  'profile': 'chat-only',
  'mode': 'chat-only',
  'session': 'default',
  'text': 'Inert reply',
  'attachments': [],
  'pipeline': null,
  'task': null,
};

void main() {
  test(
    'default bridge chat preserves legacy route and permissive reply',
    () async {
      final bridge = SmithBridge(
        client: MockClient((request) async {
          expect(request.url.path, '/v1/chat');
          expect(jsonDecode(request.body), {
            'v': 1,
            'text': 'Legacy',
            'session': 'existing',
            'new': true,
          });
          return http.Response('{"text":"Legacy reply"}', 200);
        }),
      );
      addTearDown(bridge.close);
      final reply = await bridge.chat(
        'Legacy',
        session: 'existing',
        newSession: true,
      );
      expect(reply.text, 'Legacy reply');
      expect(reply.session, 'existing');
    },
  );

  test(
    'chat-only bridge uses exclusive route and existing backend schema',
    () async {
      final bridge = SmithBridge(
        client: MockClient((request) async {
          expect(request.method, 'POST');
          expect(request.url.path, '/v1/chat-only');
          expect(request.followRedirects, isFalse);
          expect(jsonDecode(request.body), {
            'v': 1,
            'text': 'Text only',
            'session': 'default',
            'new': true,
          });
          return http.Response(jsonEncode(textReply()), 200);
        }),
      );
      addTearDown(bridge.close);
      final reply = await bridge.chatOnly('Text only', newSession: true);
      expect(reply.text, 'Inert reply');
      expect(reply.mode, 'chat-only');
    },
  );

  final invalidReplies = <String, Map<String, dynamic>>{
    'missing profile': textReply()..remove('profile'),
    'legacy profile': textReply()..['profile'] = 'legacy',
    'legacy mode': textReply()..['mode'] = 'chat',
    'wrong version': textReply()..['v'] = 2,
    'wrong session': textReply()..['session'] = 'other',
    'non-text answer': textReply()..['text'] = 42,
    'attachment': textReply()
      ..['attachments'] = [
        {'kind': 'image'},
      ],
    'missing attachments': textReply()..remove('attachments'),
    'task': textReply()..['task'] = {'id': 'unexpected'},
    'pipeline': textReply()..['pipeline'] = {'job_id': 'unexpected'},
  };
  for (final entry in invalidReplies.entries) {
    test('chat-only rejects ${entry.key} without fallback', () async {
      var calls = 0;
      final bridge = SmithBridge(
        client: MockClient((request) async {
          calls++;
          expect(request.url.path, '/v1/chat-only');
          return http.Response(jsonEncode(entry.value), 200);
        }),
      );
      addTearDown(bridge.close);
      await expectLater(
        bridge.chatOnly('Explicit turn'),
        throwsA(isA<SmithBridgeException>()),
      );
      expect(calls, 1);
    });
  }

  for (final status in [404, 405, 307, 502, 504]) {
    test('chat-only HTTP $status never retries or follows redirects', () async {
      var calls = 0;
      final bridge = SmithBridge(
        client: MockClient((request) async {
          calls++;
          expect(request.url.path, '/v1/chat-only');
          expect(request.followRedirects, isFalse);
          return http.Response(
            'Not JSON',
            status,
            headers: {'location': '/v1/chat'},
          );
        }),
      );
      addTearDown(bridge.close);
      await expectLater(
        bridge.chatOnly('Explicit turn'),
        throwsA(
          isA<SmithBridgeException>().having(
            (e) => e.statusCode,
            'status',
            status,
          ),
        ),
      );
      expect(calls, 1);
    });
  }
}
