import 'dart:convert';
import 'dart:math';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';

const call = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';
String reply(String result) =>
    '{"schema_version":3,"service":"smith-next","protocol":"app","call_id":"$call","pid":1,"instance_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","ok":true,"error":null,"result":$result}';
AppReply parse(String raw) =>
    AppReply.parse(reply(raw), call, schemaVersion: 3);
final protocol = throwsA(
  isA<AppFailure>().having((e) => e.code, 'code', 'PROTOCOL'),
);

void main() {
  test('independent escaped key aliases in all container positions', () {
    const pairs = [
      [r'"a"', r'"\u0061"'],
      [r'"/"', r'"\/"'],
      [r'"\\"', r'"\u005C"'],
      [r'"\""', r'"\u0022"'],
      [r'"\t"', r'"\u0009"'],
      [r'"\r\n"', r'"\u000D\u000A"'],
      [r'"🛠"', r'"\uD83D\uDEE0"'],
      [r'"\ud800"', r'"\uD800"'],
      [r'""', r'""'],
    ];
    for (final pair in pairs) {
      for (var depth = 0; depth < 20; depth++) {
        var raw = '{${pair[0]}:false,${pair[1]}:null}';
        for (var j = 0; j < depth; j++) {
          raw = j.isEven ? '[$raw]' : '{"wrapper":$raw}';
        }
        expect(
          () => parse('{"value":$raw}'),
          protocol,
          reason: '$pair at $depth',
        );
      }
    }
  });

  test('independent scopes and non-normalized Unicode identities remain distinct', () {
    const raw =
        r'{"é":1,"e\u0301":2,"Base":3,"base":4,"rows":[{"\u0061":1},{"a":2},[{"a":3}]],"x":{"a":4}}';
    expect(parse(raw).result, jsonDecode(raw));
  });

  test(
    'independent strings tokenize atomically for every escape and slash parity',
    () {
      final strings = <String>[
        for (var n = 0; n < 32; n++)
          '${'\\' * n}" : { [ ] } , "base":1,"base":2',
        for (var n = 0; n < 32; n++) String.fromCharCode(n),
        '\u2028\u2029\uFEFF',
        'é🛠',
        r'\u0061',
        r'\"[{}]:',
      ];
      for (final s in strings) {
        final raw = jsonEncode({
          s: s,
          'rows': [
            {'same': s},
            {'same': s},
          ],
        });
        expect(parse(raw).result, jsonDecode(raw));
      }
    },
  );

  test(
    'independent deterministic generated valid JSON does not false-positive',
    () {
      final rng = Random(92717);
      dynamic value(int depth) {
        if (depth == 0)
          return ['é', '🛠', '"\\{}[]:', null, true, 42][rng.nextInt(6)];
        if (rng.nextBool())
          return List.generate(rng.nextInt(5), (_) => value(depth - 1));
        return {
          for (var i = 0; i < rng.nextInt(5); i++)
            'k$i/"\\🛠': value(depth - 1),
        };
      }

      for (var i = 0; i < 400; i++) {
        final raw = jsonEncode({'payload': value(5)});
        expect(parse(raw).result, jsonDecode(raw), reason: 'sample $i');
      }
    },
  );

  test('independent exact mixed/object/array depth boundary', () {
    for (var style = 0; style < 3; style++) {
      for (var n = 0; n <= 66; n++) {
        var raw = 'null';
        for (var i = 0; i < n; i++) {
          raw = style == 0 || (style == 2 && i.isEven)
              ? '[$raw]'
              : '{"k":$raw}';
        }
        raw = '{"payload":$raw}';
        if (n <= 62) {
          expect(parse(raw).result, jsonDecode(raw));
        } else {
          expect(
            () => parse(raw),
            protocol,
            reason: 'style $style depth ${n + 2}',
          );
        }
      }
    }
    expect(() => parse('{"x":${'[' * 20000}0${']' * 20000}}'), protocol);
    expect(
      parse(jsonEncode({'x': '[' * 10000 + ']' * 10000})).result['x'],
      hasLength(20000),
    );
  });

  test(
    'independent malformed grammar cannot bypass preflight plus jsonDecode',
    () {
      for (final raw in [
        r'{"x":"bad\x20"}',
        r'{"x":"bad\u123"}',
        r'{"x":"bad\uZZZZ"}',
        r'{"x":NaN}',
        r'{"x":Infinity}',
        r'{"x":+1}',
        r'{"x":.1}',
        r'{"x":1.}',
        r'{"x":1e}',
        r'{"x":00}',
        r'{"x":true,}',
        r'{"x":[,]}',
        r'{"x":/*comment*/0}',
        r'{"x":"a" "b"}',
        r'{"x":[]]}',
        r'{"x":{"a":1]}}',
        r'{"x":["a":1]}',
        '{"x":"raw\nline"}',
        '{"x":"raw\u0000null"}',
        '{"x"\u00a0:1}',
        r'{"x":0}{"y":1}',
        r'{"x":"unterminated}',
        '{"x":"trailing\\',
      ]) {
        expect(() => parse(raw), protocol, reason: raw);
      }
      final valid = reply('{}');
      expect(
        () => AppReply.parse('$valid null', call, schemaVersion: 3),
        protocol,
      );
      expect(
        () => AppReply.parse('\uFEFF$valid', call, schemaVersion: 3),
        protocol,
      );
    },
  );

  test('independent UTF8 byte bound unchanged for multibyte strings', () {
    final empty = reply('{"x":""}');
    final room = appFrameLimit + 1 - utf8.encode(empty).length;
    final payload = '🛠' * (room ~/ 4) + 'x' * (room % 4);
    final raw = reply(jsonEncode({'x': payload}));
    expect(utf8.encode(raw).length, appFrameLimit + 1);
    expect(AppReply.parse(raw, call, schemaVersion: 3).result['x'], payload);
    expect(() => AppReply.parse('$raw ', call, schemaVersion: 3), protocol);
  });
}
