"""Finish read-only/fake-client gates after the retained navigation failure; no staging."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('v2_preparation', HERE / 'coordinated-candidate-v2-prepare.py')
p = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p)


def main():
    os.umask(0o077)
    result_path = HERE / 'coordinated-candidate-v2-result.json'
    p.require(not result_path.exists(), 'Existing result; no overwrite')
    p.require(p.DEST.is_dir() and not any(p.DEST.iterdir()), 'Candidate must remain empty')
    sources = p.hashes(p.source_paths())
    tests = p.hashes(p.test_paths())
    prior = json.loads(p.read(HERE / 'coordinated-candidate-result.json'))
    p.require(p.digest(p.read(p.BINARY)) == p.NEW, 'Retained service identity drift')
    historical = {name: p.digest(p.read(p.ROOT / name)) for name in (
        'docs/DESIGN_DETAILS.md', 'docs/plans/LOCAL-SMITH-DEV-BUFFERED-FRAMES.md')}
    home = Path(os.environ['HOME'])
    data = Path(os.environ.get('XDG_DATA_HOME', str(home / '.local/share')))
    config = Path(os.environ.get('XDG_CONFIG_HOME', str(home / '.config')))
    namespace = data / 'smith-next'
    selector = namespace / 'client/current'
    selector_before = (p.identity(selector.lstat()), os.readlink(selector))
    p.require(selector_before[1] == 'releases/' + p.SELECTED, 'Installed selector drift')
    installed = {
        'standalone': namespace / 'bin/smith-service',
        'helper': selector / 'smith_backend/smith-service',
        'unit': config / 'systemd/user/smith-next.service',
        'desktop': data / 'applications/dev.smith.smith_client.desktop',
        'shim': home / '.local/bin/smith-next',
        'app_state': namespace / 'app-state.json',
        'local_dev_ledger': namespace / 'dev-local-jobs.json',
        'receipt_dev_ledger': namespace / 'dev-jobs.json',
    }
    def installed_snapshot():
        return {name: (p.identity(path.lstat()), p.digest(p.read(path)))
                for name, path in installed.items()}
    before = installed_snapshot()
    p.require(before['standalone'][1] == before['helper'][1] == p.LIVE, 'Installed pair drift')
    env = os.environ.copy()
    env.update({'CI': 'true', 'FLUTTER_SUPPRESS_ANALYTICS': 'true', 'DART_SUPPRESS_ANALYTICS': 'true'})
    commands = {
        'dev-focused': ['/snap/bin/flutter', '--no-version-check', 'test', '--no-pub',
                        '--reporter', 'expanded', 'test/dev_jobs_test.dart',
                        'test/dev_jobs_controller_list_test.dart', 'test/dev_jobs_list_view_test.dart'],
        'analyze': ['/snap/bin/flutter', '--no-version-check', 'analyze', '--no-pub'],
        'linux-build': ['/snap/bin/flutter', '--no-version-check', 'build', 'linux', '--release', '--no-pub'],
    }
    checks = {'dev-and-navigation': {
        'exit_code': 1, 'log': 'docs/evidence/dev-buffered-frames/coordinated-candidate-v2-tests.log',
        'log_sha256': p.digest(p.read(HERE / 'coordinated-candidate-v2-tests.log')),
        'result': '161 passed; 1 failed: four-pane Agents navigation expects obsolete Agent/Dev operation calls',
    }}
    for label, command in commands.items():
        log_path = HERE / ('coordinated-candidate-v2-' + label + '.log')
        p.require(not log_path.exists(), 'Existing check log; no overwrite')
        try:
            run = subprocess.run(command, cwd=p.ROOT / 'client', env=env,
                                 stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                 timeout=240, check=False)
            output, code = run.stdout.decode('utf-8', 'replace'), run.returncode
        except subprocess.TimeoutExpired as error:
            output, code = (error.stdout or b'').decode('utf-8', 'replace'), 'timeout_240s'
        output = output.replace(str(p.ROOT), 'smith').replace(str(home), '<home>')
        p.write(log_path, ('COMMAND: ' + ' '.join(command) + '\n' + output +
                           '\nEXIT_CODE: ' + str(code) + '\n').encode(), 0o600)
        checks[label] = {'command': command, 'exit_code': code,
                         'log': log_path.relative_to(p.ROOT).as_posix(),
                         'log_sha256': p.digest(p.read(log_path))}
        print(label + ': exit ' + str(code), flush=True)
        p.require(p.hashes(p.source_paths()) == sources, 'Production inputs changed during checks')
        p.require(p.hashes(p.test_paths()) == tests, 'Test inputs changed during checks')
    p.require(installed_snapshot() == before, 'Installed hashes or identities changed')
    p.require((p.identity(selector.lstat()), os.readlink(selector)) == selector_before, 'Selector changed')
    p.require(p.digest(p.read(p.OLD_CANDIDATE / 'manifest.json')) == prior['package_manifest_sha256'],
              'Prior candidate manifest changed')
    p.require(not any(p.DEST.iterdir()), 'Unexpected staging despite blocked gate')
    bundle = p.raw_tree(p.BUNDLE)
    result = {
        'status': 'blocked_navigation_test_failure_no_v2_package_staged',
        'source_sha256': sources,
        'source_freshness_contract': 'Production/build input membership and hashes only; docs/plans/reports/tests are excluded',
        'test_inputs_sha256': tests,
        'historical_document_references_sha256_not_freshness_inputs': historical,
        'checks': checks,
        'flutter_build_inventory': bundle,
        'flutter_executable_sha256': bundle['smith_client']['sha256'],
        'flutter_libapp_sha256': bundle['lib/libapp.so']['sha256'],
        'service_sha256': p.NEW,
        'service_validation': 'Retained scoped service checks and unchanged production inputs, not a fresh service build/test',
        'installed_before_after_sha256': {n: value[1] for n, value in before.items()},
        'installed_unchanged': True,
        'prior_candidate_manifest_sha256': prior['package_manifest_sha256'],
        'v2_package_staged': False,
        'v2_candidate_directory_empty': True,
        'retained_timestamp_fixture_failure': 'Not rerun or waived',
        'host_effects_executed': False,
        'prod_test_or_doc_edits_made': False,
        'blocker': 'Required current changed-file navigation test failed; no waiver or unauthorized fixture repair',
        'preparation_script_sha256': p.digest(p.read(HERE / 'coordinated-candidate-v2-prepare.py')),
        'checks_script_sha256': p.digest(p.read(Path(__file__))),
    }
    p.write(result_path, (json.dumps(result, sort_keys=True, indent=2) + '\n').encode(), 0o600)
    print(json.dumps({key: result[key] for key in (
        'status', 'flutter_libapp_sha256', 'service_sha256', 'installed_unchanged', 'v2_package_staged')}, indent=2))


if __name__ == '__main__':
    main()
