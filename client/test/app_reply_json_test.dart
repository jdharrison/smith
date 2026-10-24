import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/projects_controller.dart';
import 'package:smith_client/projects.dart';

const _instance = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
const _call = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';
const _project = 'cccccccccccccccccccccccccccccccc';

String _reply(String result, {int schema = 3, String call = _call}) =>
    '{"schema_version":$schema,"service":"smith-next","protocol":"app",'
    '"pid":123,"instance_id":"$_instance","call_id":"$call",'
    '"ok":true,"error":null,"result":$result}';

Map<String, dynamic> _repository() => {
  'name': null,
  'url': 'https://a/repo',
  'read_only': true,
  'branch': {
    'base': null,
    'work': null,
    'types': {'feature': null, 'fix': null, 'release': null, 'hotfix': null},
  },
};

class _RawReplyExecutor implements AppExecutor {
  _RawReplyExecutor({
    this.duplicateField,
    this.escaped = false,
    this.badReceipt = false,
    this.populated = false,
  });

  final String? duplicateField;
  final bool escaped;
  final bool badReceipt;
  final bool populated;
  final calls = <Map<String, dynamic>>[];

  @override
  Future<String> exchange(List<int> bytes) async {
    final request = jsonDecode(utf8.decode(bytes)) as Map<String, dynamic>;
    calls.add(request);
    final op = request['op'];
    String result;
    if (op == 'projects_list_v3') {
      result = jsonEncode({
        'revision': 0,
        'projects': duplicateField == null && !populated
            ? []
            : [
                {
                  'project_id': _project,
                  'name': 'P',
                  'key': 'project-p',
                  'description': ' exact description 🛠 ',
                  'prompt': ' exact\n\tprompt 🛠 ',
                  'repositories': [_repository()],
                  'agent': 'opencode',
                },
              ],
      });
      if (duplicateField != null) {
        final key = duplicateField!;
        final duplicate = escaped
            ? '\\u${key.codeUnitAt(0).toRadixString(16).padLeft(4, '0')}${key.substring(1)}'
            : key;
        final value = switch (key) {
          'key' => jsonEncode('project-p'),
          'description' => jsonEncode(' exact description 🛠 '),
          'prompt' => jsonEncode(' exact\n\tprompt 🛠 '),
          'read_only' => 'true',
          _ => 'null',
        };
        final original = '"$key":$value';
        final replacement = '"$key":"INVALID!","$duplicate":$value';
        final malformed = result.replaceFirst(original, replacement);
        expect(malformed, isNot(result), reason: 'Must insert duplicate $key');
        result = malformed;
      }
    } else if (op == 'project_create_v3') {
      result = jsonEncode({
        'request_id': request['args']['request_id'],
        'project_id': _project,
        'revision': 1,
        'deleted': false,
      });
      if (badReceipt) {
        result = result.replaceFirst(
          '"revision":1',
          r'"revision":0,"revi\u0073ion":1',
        );
      }
    } else {
      throw const AppFailure('REQUEST_UNKNOWN', 'Unknown original request');
    }
    return _reply(result, call: request['call_id']);
  }

  @override
  void dispose() {}
}

void main() {
  final protocol = throwsA(
    isA<AppFailure>().having((e) => e.code, 'code', 'PROTOCOL'),
  );

  for (final schema in [1, 2, 3]) {
    test(
      'app$schema rejects duplicate envelope, result and error members before collapse',
      () {
        final valid = _reply('{}', schema: schema);
        for (final member in <String, dynamic>{
          'schema_version': schema,
          'service': 'smith-next',
          'protocol': 'app',
          'pid': 123,
          'instance_id': _instance,
          'call_id': _call,
          'ok': true,
          'error': null,
          'result': <String, dynamic>{},
        }.entries) {
          final key = jsonEncode(member.key);
          final value = jsonEncode(member.value);
          final duplicate = valid.replaceFirst(
            '$key:$value',
            '$key:$value,$key:$value',
          );
          expect(
            () => AppReply.parse(duplicate, _call, schemaVersion: schema),
            protocol,
            reason: member.key,
          );
        }
        for (final raw in [
          r'{"base":"INVALID!","base":null}',
          r'{"outer":{"base":"INVALID!","base":null}}',
          r'{"rows":[{"a":1},[{"feature":"INVALID!","feature":null}]]}',
          r'{"outer":[[],{"a":[0,null,{"hotfix":"INVALID!","hotfix":null}]}]}',
        ]) {
          expect(
            () => AppReply.parse(
              _reply(raw, schema: schema),
              _call,
              schemaVersion: schema,
            ),
            protocol,
          );
        }
        final error = valid
            .replaceFirst('"ok":true', '"ok":false')
            .replaceFirst('"result":{}', '"result":null')
            .replaceFirst(
              '"error":null',
              r'"error":{"code":"BUSY","message":"bad","message":"busy"}',
            );
        expect(
          () => AppReply.parse(error, _call, schemaVersion: schema),
          protocol,
        );
        expect(
          AppReply.parse(valid, _call, schemaVersion: schema).result,
          isEmpty,
        );
      },
    );
  }

  test(
    'decoded member identity includes Unicode escapes and JSON string escapes',
    () {
      for (final pair in [
        [r'"base"', r'"b\u0061se"'],
        [r'"\u0062ase"', r'"base"'],
        [r'"feature"', r'"\u0066eature"'],
        [r'"hotfix"', r'"hotfi\u0078"'],
        [r'"é"', r'"\u00e9"'],
        [r'"🛠"', r'"\ud83d\udee0"'],
        [r'"a/b"', r'"a\/b"'],
        [r'"a\"b"', r'"a\u0022b"'],
        [r'"a\\b"', r'"a\u005cb"'],
        [r'"a\nb"', r'"a\u000ab"'],
        [r'""', r'""'],
      ]) {
        final raw = '{${pair[0]}:null,${pair[1]}:null}';
        expect(
          () => AppReply.parse(_reply(raw), _call, schemaVersion: 3),
          protocol,
          reason: raw,
        );
      }
    },
  );

  test('repeated keys in independent objects, case differences and punctuation strings stay valid', () {
    final result = {
      'base': 'base',
      'Base': 'Base',
      'rows': [
        {'base': null},
        [
          {'base': 'main'},
          {'base': 'main'},
        ],
      ],
      'strings': [
        r'"base":null,"base":null',
        r'{}[],:"\\\"',
        r'"base":null}],[{"base":null',
        r'\u0062ase',
        'quote " backslash \\ newline\n tab\t',
      ],
      r'key"{}[],:\\': 'punctuation in a member name',
      'é': 'accented',
      '🛠': 'supplementary scalar',
    };
    expect(
      AppReply.parse(
        _reply(jsonEncode(result)),
        _call,
        schemaVersion: 3,
      ).result,
      result,
    );
    final escaped =
        r'{"rows":[{"b\u0061se":null},{"base":null}],"a\/b":"{}[],:"}';
    expect(
      AppReply.parse(_reply(escaped), _call, schemaVersion: 3).result,
      jsonDecode(escaped),
    );
    final whitespace = '{"base" \t\r\n : null,"work": [ {"base":null} ]}';
    expect(
      AppReply.parse(_reply(whitespace), _call, schemaVersion: 3).result,
      jsonDecode(whitespace),
    );
  });

  test('malformed JSON remains rejected, including unclosed strings and containers', () {
    for (final raw in [
      r'{"x":"unterminated}',
      '{"x":"trailing\\',
      r'{"x":"bad\q"}',
      r'{"x":"bad\u00zz"}',
      r'{"x":[1,2}',
      r'{"x":{]}',
      r'{"x":1,}',
      r'{"x":01}',
      r'{"x":true false}',
      r'{"x":}',
      r'{"x" 1}',
      r'{"x":[1,]}',
      r'{"x":1} {}',
    ]) {
      expect(
        () => AppReply.parse(_reply(raw), _call, schemaVersion: 3),
        protocol,
        reason: raw,
      );
    }
  });

  test('preflight bounds nesting before decoding and retains the 4 MiB frame bound', () {
    String nested(int depth) => '{"rows":${'[' * depth}0${']' * depth}}';
    final atLimit = _reply(nested(62)); // Envelope + result + 62 arrays = 64.
    expect(AppReply.parse(atLimit, _call, schemaVersion: 3).result, isNotEmpty);
    for (final depth in [63, 10000]) {
      expect(
        () => AppReply.parse(_reply(nested(depth)), _call, schemaVersion: 3),
        protocol,
      );
    }
    final empty = _reply('{"text":""}');
    final padding = appFrameLimit + 1 - utf8.encode(empty).length;
    final maximal = _reply('{"text":"${'x' * padding}"}');
    expect(
      AppReply.parse(maximal, _call, schemaVersion: 3).result['text'],
      hasLength(padding),
    );
    expect(
      () => AppReply.parse('$maximal ', _call, schemaVersion: 3),
      protocol,
    );
  });

  test('wide objects and sibling arrays do not confuse member ownership', () {
    final result = {
      for (var i = 0; i < 2000; i++)
        'key$i': [
          {'base': i},
          {'base': i},
        ],
    };
    expect(
      AppReply.parse(
        _reply(jsonEncode(result)),
        _call,
        schemaVersion: 3,
      ).result,
      result,
    );
  });

  test(
    'native Projects companion accepts complete v3 records and receipts',
    () async {
      final executor = _RawReplyExecutor(populated: true);
      final api = NativeAppApi(executor: executor);
      addTearDown(api.dispose);
      final reply = await api.call('projects_list_v3', {});
      final saved = ProjectSnapshot.parse(reply.result).projects.single;
      expect(saved.key, 'project-p');
      expect(saved.description, ' exact description 🛠 ');
      expect(saved.prompt, ' exact\n\tprompt 🛠 ');
      expect(saved.repositories.single.toJson(), _repository());
      final created = await api.call('project_create_v3', {
        'request_id': _call,
        'instance_id': _instance,
        'expected_revision': 0,
        'name': 'P',
        'key': 'another-p',
        'description': '',
        'prompt': '',
        'repositories': [_repository()],
        'agent': 'opencode',
      });
      expect(ProjectReceipt.parse(created.result).revision, 1);
    },
  );

  for (final field in [
    'key',
    'description',
    'prompt',
    'read_only',
    'name',
    'base',
    'work',
    'feature',
    'fix',
    'release',
    'hotfix',
  ]) {
    for (final escaped in [false, true]) {
      test(
        'native Projects list rejects duplicate $field (escaped=$escaped)',
        () async {
          final executor = _RawReplyExecutor(
            duplicateField: field,
            escaped: escaped,
          );
          final api = NativeAppApi(executor: executor);
          addTearDown(api.dispose);
          await expectLater(api.call('projects_list_v3', {}), protocol);
          expect(executor.calls, hasLength(1));
        },
      );
    }
  }

  test('duplicate mutation receipt remains uncertain with original-ID query-only recovery', () async {
    final executor = _RawReplyExecutor(badReceipt: true);
    final api = NativeAppApi(executor: executor);
    final controller = ProjectsController(api);
    addTearDown(() {
      controller.dispose();
      api.dispose();
    });
    await controller.refresh();
    await controller.create(
      'P',
      [],
      expectedInstanceId: _instance,
      expectedRevision: 0,
    );
    expect(controller.uncertain, isTrue);
    expect(controller.canMutate, isFalse);
    final original = executor.calls[1]['args']['request_id'];
    await controller.resolvePending();
    expect(executor.calls.map((r) => r['op']), [
      'projects_list_v3',
      'project_create_v3',
      'project_receipt',
      'projects_list_v3',
    ]);
    expect(executor.calls[2]['args'], {'request_id': original});
    expect(controller.uncertain, isTrue);
    expect(controller.canMutate, isFalse);
  });
}
