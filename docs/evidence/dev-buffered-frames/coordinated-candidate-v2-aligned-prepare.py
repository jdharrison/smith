"""Approved expectation-only alignment, current checks, and first v2 staging.

Preserve the blocked attempt and all prior artifacts. Reuse the previously built
AOT only when the complete production-input and build inventories still match.
"""
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('v2_staging', HERE / 'coordinated-candidate-v2-prepare.py')
p = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p)
OLD_EXPECTATION = """        for (var i = 0; i < 4; i++) ...[
          'projects_list_v3',
          'agents_list_v2',
          'dev_jobs_state',
        ],"""
NEW_EXPECTATION = """        for (var i = 0; i < 4; i++) ...[
          'projects_list_v3',
          'dev_jobs_state_v2',
        ],"""


def aligned_validation(log):
    blocked_bytes = p.read(HERE / 'coordinated-candidate-v2-result.json')
    blocked = json.loads(blocked_bytes)
    p.require(blocked['status'] == 'blocked_navigation_test_failure_no_v2_package_staged',
              'Unexpected previous attempt status')
    sources = p.hashes(p.source_paths())
    tests = p.hashes(p.test_paths())
    p.require(len(sources) == 132 and sources == blocked['source_sha256'],
              'Production inputs differ from the current Linux build; do not relabel old AOT')
    p.require(p.raw_tree(p.BUNDLE) == blocked['flutter_build_inventory'],
              'Existing current build inventory drift')
    p.require(p.digest(p.read(p.BINARY)) == p.NEW, 'Retained tested service drift')
    name = 'client/test/agents_navigation_test.dart'
    current = p.read(p.ROOT / name)
    p.require(current.count(NEW_EXPECTATION.encode()) == 1, 'Expected exact aligned operation trace')
    reconstructed = current.replace(NEW_EXPECTATION.encode(), OLD_EXPECTATION.encode(), 1)
    p.require(p.digest(reconstructed) == blocked['test_inputs_sha256'][name],
              'Changes extend beyond the single approved expectation')
    expected_tests = dict(blocked['test_inputs_sha256'])
    expected_tests[name] = p.digest(current)
    p.require(tests == expected_tests, 'Other test inputs changed')
    historical = {name: p.digest(p.read(p.ROOT / name)) for name in (
        'docs/DESIGN_DETAILS.md', 'docs/plans/LOCAL-SMITH-DEV-BUFFERED-FRAMES.md')}
    env = os.environ.copy()
    env.update({'CI': 'true', 'FLUTTER_SUPPRESS_ANALYTICS': 'true', 'DART_SUPPRESS_ANALYTICS': 'true'})
    checks = {}
    commands = {
        'tests': ['/snap/bin/flutter', '--no-version-check', 'test', '--no-pub', '--reporter', 'expanded',
                  'test/agents_navigation_test.dart', 'test/dev_jobs_test.dart',
                  'test/dev_jobs_controller_list_test.dart', 'test/dev_jobs_list_view_test.dart'],
        'analyze': ['/snap/bin/flutter', '--no-version-check', 'analyze', '--no-pub'],
    }
    for label, command in commands.items():
        path = HERE / ('coordinated-candidate-v2-aligned-' + label + '.log')
        p.require(not path.exists(), 'Existing aligned check log; no overwrite')
        try:
            run = subprocess.run(command, cwd=p.ROOT / 'client', env=env,
                                 stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                 timeout=240, check=False)
            output, code = run.stdout.decode('utf-8', 'replace'), run.returncode
        except subprocess.TimeoutExpired as error:
            output, code = (error.stdout or b'').decode('utf-8', 'replace'), 'timeout_240s'
        output = output.replace(str(p.ROOT), 'smith').replace(str(Path.home()), '<home>')
        p.write(path, ('COMMAND: ' + ' '.join(command) + '\n' + output +
                       '\nEXIT_CODE: ' + str(code) + '\n').encode(), 0o600)
        checks[label] = {'command': command, 'exit_code': code,
                         'log': path.relative_to(p.ROOT).as_posix(), 'log_sha256': p.digest(p.read(path))}
        log.write(label + ': exit ' + str(code) + '; ' + path.name + '\n')
        print(label + ': exit ' + str(code), flush=True)
        p.require(code == 0, 'Aligned current client gate failed: ' + label)
        p.require(p.hashes(p.source_paths()) == sources, 'Production inputs changed during checks')
        p.require(p.hashes(p.test_paths()) == tests, 'Test inputs changed during checks')
        p.require(p.raw_tree(p.BUNDLE) == blocked['flutter_build_inventory'], 'Current AOT inventory changed')
    build = blocked['checks']['linux-build']
    p.require(build['exit_code'] == 0 and p.digest(p.read(p.ROOT / build['log'])) == build['log_sha256'],
              'Retained current-build evidence drift')
    p.require(p.read(HERE / 'coordinated-candidate-v2-result.json') == blocked_bytes,
              'Retained blocked result changed')
    validation = {
        'status': 'aligned_navigation_and_dev_tests_analysis_pass_current_aot_freshness_verified',
        'source_sha256': sources,
        'test_inputs_sha256': tests,
        'checks': checks,
        'retained_current_linux_build_check': build,
        'linux_build_rerun': False,
        'flutter_build_inventory': blocked['flutter_build_inventory'],
        'historical_document_references_sha256_not_freshness_inputs': historical,
        'previous_blocked_result_sha256': p.digest(blocked_bytes),
        'expectation_alignment': {
            'file': name, 'before_sha256': p.digest(reconstructed), 'after_sha256': p.digest(current),
            'old_per_pane_operations': ['projects_list_v3', 'agents_list_v2', 'dev_jobs_state'],
            'new_per_pane_operations': ['projects_list_v3', 'dev_jobs_state_v2'],
            'all_other_bytes_unchanged': True,
            'reason': 'Current approved local runner has no Agent dependency and uses dev_jobs_state_v2',
        },
        'service_validation': 'Service 9bb and its production inputs unchanged; retained scoped service checks, no new service build/test',
        'retained_timestamp_fixture_failure': 'Not rerun or waived',
        'whole_client_or_installed_acceptance': False,
    }
    path = HERE / 'coordinated-candidate-v2-validation.json'
    p.write(path, (json.dumps(validation, sort_keys=True, indent=2) + '\n').encode(), 0o600)
    return validation


def main():
    os.umask(0o077)
    result_path = HERE / 'coordinated-candidate-v2-aligned-result.json'
    log_path = HERE / 'coordinated-candidate-v2-aligned-prepare.log'
    p.require(not result_path.exists() and not log_path.exists(), 'Existing aligned evidence; no overwrite/retry')
    retained_paths = {HERE / 'coordinated-candidate-v2-prepare.py',
                      HERE / 'coordinated-candidate-v2-checks.py',
                      HERE / 'coordinated-candidate-v2-result.json',
                      HERE / 'coordinated-candidate-v2-prepare.log',
                      HERE / 'coordinated-candidate-v2-tests.log',
                      HERE / 'coordinated-candidate-v2-dev-focused.log',
                      HERE / 'coordinated-candidate-v2-analyze.log',
                      HERE / 'coordinated-candidate-v2-linux-build.log'}
    retained_hashes = p.hashes(retained_paths)
    own_before = p.digest(p.read(Path(__file__)))
    p.validate_current_client = aligned_validation
    log = io.StringIO()
    try:
        result = p.prepare(log)
        p.require(p.hashes(retained_paths) == retained_hashes, 'Previous attempt inputs changed')
        p.require(p.digest(p.read(Path(__file__))) == own_before, 'Aligned preparation script changed')
        result['preparation_inputs_sha256'][Path(__file__).relative_to(p.ROOT).as_posix()] = own_before
        result['retained_previous_attempt_artifact_sha256'] = retained_hashes
        result['source_binding'] = 'All 132 production inputs and full AOT inventory match the successful current Linux build; fresh navigation+158 Dev tests and analysis after exact test-only expectation alignment; service 9bb retained from prior scoped checks'
        result['linux_build_rerun'] = False
    except Exception as error:
        if type(error) is RuntimeError:
            log.write('Precondition: ' + str(error) + '\n')
        log.write('FAIL: no host effects; any partial candidate retained; no automatic retry/cleanup.\n')
        p.write(log_path, log.getvalue().encode(), 0o600)
        print('Aligned preparation failed (' + type(error).__name__ + '); inspect safe log.')
        raise SystemExit(1) from None
    p.write(log_path, log.getvalue().encode(), 0o600)
    result['log_sha256'] = p.digest(p.read(log_path))
    p.write(result_path, (json.dumps(result, sort_keys=True, indent=2) + '\n').encode(), 0o600)
    print(json.dumps({key: result[key] for key in (
        'status', 'package', 'service_sha256', 'flutter_libapp_sha256',
        'client_manifest_sha256', 'package_manifest_sha256', 'installed_unchanged')}, indent=2))


if __name__ == '__main__':
    main()
