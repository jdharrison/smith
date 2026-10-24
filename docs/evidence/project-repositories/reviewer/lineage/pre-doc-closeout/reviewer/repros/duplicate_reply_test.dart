import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';

class DuplicateReply implements AppExecutor {
  DuplicateReply(this.field);
  final String field;
  @override
  Future<String> exchange(List<int> bytes) async {
    final req = jsonDecode(utf8.decode(bytes));
    final repository = {
      'name': null,
      'url': 'https://a/repo',
      'branch': {
        'base': null,
        'work': null,
        'types': {
          'feature': null,
          'fix': null,
          'release': null,
          'hotfix': null,
        },
      },
    };
    final text = jsonEncode({
      'schema_version': 3,
      'service': 'smith-next',
      'protocol': 'app',
      'call_id': req['call_id'],
      'pid': 123,
      'instance_id': '11111111111111111111111111111111',
      'ok': true,
      'error': null,
      'result': {
        'revision': 1,
        'projects': [
          {
            'project_id': '22222222222222222222222222222222',
            'name': 'P',
            'repositories': [repository],
            'agent': 'opencode',
          },
        ],
      },
    });
    // Two conflicting values, where only the final value meets the schema.
    return text.replaceFirst(
      '"$field":null',
      '"$field":"INVALID!","$field":null',
    );
  }

  @override
  void dispose() {}
}

void main() {
  for (final field in ['base', 'feature', 'hotfix']) {
    test(
      'strict Projects reply rejects duplicate nested $field keys',
      () async {
        final api = NativeAppApi(executor: DuplicateReply(field));
        addTearDown(api.dispose);
        await expectLater(
          api.call('projects_list_v2', {}),
          throwsA(isA<AppFailure>()),
        );
      },
    );
  }
}
