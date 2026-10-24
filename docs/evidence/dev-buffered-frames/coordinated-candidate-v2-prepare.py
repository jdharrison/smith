"""Revalidate/rebuild current Flutter source, then stage v2; no host/product actions."""
import contextlib
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import sys
import subprocess

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
EVIDENCE = Path(__file__).resolve().parent
DEST = ROOT / '.smith-dev-coordinated-candidate-v2'
OLD_CANDIDATE = ROOT / '.smith-dev-coordinated-candidate'
BASE = ROOT / '.smith-installer-preview-20261017-a4c9/package'
BUNDLE = ROOT / 'client/build/linux/x64/release/bundle'
BINARY = ROOT / 'service/target/debug/smith-service'
NEW = '9bb0228c61bf762c7899f15f7105924d666b6e534102a33c6ecea8779fee77ec'
LIVE = 'b7790a585d568373c0c3207e34ea4bbbeec080f863ff9ecec0a08b60a25c9d10'
SELECTED = '9f670a310ffc1f883c5fdb6b93ebcebdf93b035de8928b8ba98e789b0c9343fb'
BASE_MANIFEST = '3844315097350f64946466a20b970b14603de07d5fbdcf0162229ae78ebf557d'
GUARD = 'b8c512c1b80382783bec41a76204815a9ddf3c234cee6c8ad6e721295e2489f0'
HELPER = 'smith_backend/smith-service'
MANIFEST = '.smith-next-manifest.json'



def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


# Reuse the prior preparation's bounded, owned, no-follow stable reads and
# exclusive/fsynced writes. Its Cargo-hardlink exception applies only to BINARY.
prior = load_module('prior_candidate', EVIDENCE / 'prepare_guard_candidate.py')
read, write, identity, directory = prior.read, prior.write, prior.identity, prior.directory


def hashes(paths):
    return {p.relative_to(ROOT).as_posix(): digest(read(p)) for p in sorted(paths)}


def source_paths():
    """Only production/build inputs belong in the live source freshness map."""
    paths = set()
    for folder in ('service/src', 'service/dev-worker', 'service/scripts',
                   'client/lib', 'client/assets', 'client/linux', 'client/scripts'):
        paths.update(p for p in (ROOT / folder).rglob('*')
                     if p.is_file() and not {'ephemeral', '__pycache__'} & set(p.parts)
                     and not p.name.endswith(('_tests.rs', '.test.mjs'))
                     and p.name != 'tests.rs')
    for name in ('service/Cargo.toml', 'service/Cargo.lock', 'service/.dockerignore',
                 'client/pubspec.yaml', 'client/pubspec.lock', 'client/.metadata',
                 'client/analysis_options.yaml'):
        paths.add(ROOT / name)
    return paths


def test_paths():
    return {p for p in (ROOT / 'client/test').rglob('*') if p.is_file()}


def validate_current_client(log):
    sources = hashes(source_paths())
    tests = hashes(test_paths())
    old = json.loads(read(EVIDENCE / 'coordinated-candidate-result.json'))
    observed_deltas = {n: value for n, value in sources.items()
                       if n in old['source_sha256'] and old['source_sha256'][n] != value}
    require(observed_deltas == {
        'client/lib/ui/smith_workspace.dart':
        '07920c8385793b504d5f9281be015be028c72ce2249350e7a58e89b2a59bc125'},
        'Unreviewed production delta since v1 preparation')
    changed_tests = {n: value for n, value in tests.items()
                     if n in old['source_sha256'] and old['source_sha256'][n] != value}
    require(changed_tests == {
        'client/test/agents_navigation_test.dart':
        '6e3fb964241cb74e958a3e4f84ce36d714c0fb2b795cef3bef0388870e0a6f7f'},
        'Unreviewed test delta since v1 preparation')
    require(digest(read(BINARY)) == NEW, 'Previously tested service binary drift')
    # Documents are historical context only. They are intentionally never
    # reread as a source-freshness gate after this initial reference snapshot.
    historical = {name: digest(read(ROOT / name)) for name in (
        'docs/DESIGN_DETAILS.md', 'docs/plans/LOCAL-SMITH-DEV-BUFFERED-FRAMES.md',
        'docs/plans/LOCAL-SMITH-SIDEBAR-EXPANSION.md')}
    env = os.environ.copy()
    env.update({'CI': 'true', 'FLUTTER_SUPPRESS_ANALYTICS': 'true',
                'DART_SUPPRESS_ANALYTICS': 'true'})
    commands = {
        'tests': ['/snap/bin/flutter', '--no-version-check', 'test', '--no-pub', '--reporter', 'expanded',
                  'test/dev_jobs_test.dart', 'test/dev_jobs_controller_list_test.dart',
                  'test/dev_jobs_list_view_test.dart', 'test/agents_navigation_test.dart'],
        'analyze': ['/snap/bin/flutter', '--no-version-check', 'analyze', '--no-pub'],
        'linux-build': ['/snap/bin/flutter', '--no-version-check', 'build', 'linux', '--release', '--no-pub'],
    }
    checks = {}
    for label, command in commands.items():
        path = EVIDENCE / ('coordinated-candidate-v2-' + label + '.log')
        require(not path.exists(), 'Existing validation log; no overwrite')
        try:
            result = subprocess.run(command, cwd=ROOT / 'client', env=env,
                                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                    timeout=240, check=False)
            output = result.stdout.decode('utf-8', 'replace')
            code = result.returncode
        except subprocess.TimeoutExpired as error:
            output = (error.stdout or b'').decode('utf-8', 'replace')
            code = 'timeout_240s'
        output = output.replace(str(ROOT), 'smith').replace(str(Path.home()), '<home>')
        write(path, ('COMMAND: ' + ' '.join(command) + '\n' + output +
                     '\nEXIT_CODE: ' + str(code) + '\n').encode(), 0o600)
        checks[label] = {'command': command, 'exit_code': code,
                         'log': path.relative_to(ROOT).as_posix(), 'log_sha256': digest(read(path))}
        log.write(label + ': exit ' + str(code) + '; ' + path.name + '\n')
        print(label + ': exit ' + str(code), flush=True)
        require(code == 0, 'Current client gate failed: ' + label)
        require(hashes(source_paths()) == sources, 'Production inputs changed during validation')
        require(hashes(test_paths()) == tests, 'Test inputs changed during validation')
    validation = {
        'status': 'current_client_focused_tests_analysis_linux_build_pass',
        'source_sha256': sources, 'test_inputs_sha256': tests,
        'checks': checks, 'flutter_build_inventory': raw_tree(BUNDLE),
        'historical_document_references_sha256_not_freshness_inputs': historical,
        'reviewed_production_delta': observed_deltas,
        'reviewed_test_delta': changed_tests,
        'classification': 'Compact navigation presentation only: 104px inline separate route/disclosure targets; no new capability or persistence',
        'retained_service_artifact_sha256': NEW,
        'service_validation': 'Retained scoped service evidence; not rebuilt or freshly service-tested',
        'known_unrun_legacy_fixtures': 'sidebar_expansion_test.dart and dev_navigation_test.dart still describe the previous compact chooser; no whole-navigation/full-suite claim',
        'retained_failure': 'Broader Rust timestamp fixture remains failing, not rerun or waived',
    }
    path = EVIDENCE / 'coordinated-candidate-v2-validation.json'
    write(path, (json.dumps(validation, sort_keys=True, indent=2) + '\n').encode(), 0o600)
    return validation


def raw_tree(root):
    result = {}
    for p in sorted(root.rglob('*')):
        info = p.lstat()
        require(info.st_uid == os.geteuid() and not p.is_symlink(), 'Foreign or linked build input')
        name = p.relative_to(root).as_posix()
        if stat.S_ISDIR(info.st_mode):
            result[name] = {'type': 'directory'}
        else:
            result[name] = {'type': 'file', 'sha256': digest(read(p))}
    return result


def prepare(log):
    require(os.geteuid() != 0, 'Non-root execution required')
    require(DEST.is_dir() and not DEST.is_symlink() and not any(DEST.iterdir()),
            'Fresh empty output directory required; no retry or cleanup')
    require(DEST.lstat().st_uid == os.geteuid(), 'Output ownership mismatch')
    DEST.chmod(0o700)
    sys.path.insert(0, str(ROOT / 'dagger'))
    package = load_module('coordinated_package', ROOT / 'dagger/package.py')
    package.check_location(DEST)
    # Build inputs are not exports: do not chmod or apply recipient modes to
    # Flutter's existing build directories. Bind their owned/no-follow ancestry.
    build_ancestors = [ROOT / Path(*BUNDLE.relative_to(ROOT).parts[:i])
                       for i in range(1, len(BUNDLE.relative_to(ROOT).parts) + 1)]
    build_directory_ids = {}
    for p in build_ancestors:
        info = p.lstat()
        require(stat.S_ISDIR(info.st_mode) and not p.is_symlink()
                and info.st_uid == os.geteuid(), 'Unsafe Flutter source ancestry')
        build_directory_ids[p] = identity(info)
    with contextlib.redirect_stdout(log):
        package.verify(BASE)
    base_manifest = read(BASE / 'manifest.json')
    require(digest(base_manifest) == BASE_MANIFEST, 'Retained package identity mismatch')
    base_inventory = package.inventory(BASE)
    retained = json.loads(read(EVIDENCE / 'guard-candidate.json'))
    require(base_inventory == retained['retained_input_inventory'], 'Retained package drift')

    old_result_bytes = read(EVIDENCE / 'coordinated-candidate-result.json')
    old_result = json.loads(old_result_bytes)
    with contextlib.redirect_stdout(log):
        package.verify(OLD_CANDIDATE)
    require(digest(read(OLD_CANDIDATE / 'manifest.json')) == old_result['package_manifest_sha256'],
            'Prior candidate manifest drift')
    old_inventory = package.inventory(OLD_CANDIDATE)
    preparation_paths = {Path(__file__).resolve(), EVIDENCE / 'prepare_guard_candidate.py',
                         ROOT / 'dagger/package.py', ROOT / 'dagger/runtime.py'}
    preparation_hashes = hashes(preparation_paths)

    home = Path(os.environ['HOME'])
    data = Path(os.environ.get('XDG_DATA_HOME', str(home / '.local/share')))
    config = Path(os.environ.get('XDG_CONFIG_HOME', str(home / '.config')))
    namespace = data / 'smith-next'
    for root in (home, data, config, namespace):
        package.check_location(root)
    selector = namespace / 'client/current'
    require(selector.is_symlink() and os.readlink(selector) == 'releases/' + SELECTED,
            'Fresh installed selector differs from preflight')
    selector_before = (identity(selector.lstat()), os.readlink(selector))
    release = namespace / 'client/releases' / SELECTED
    package.check_location(release)
    installed_manifest = read(release / MANIFEST)
    require(digest(installed_manifest) == SELECTED, 'Installed manifest digest mismatch')
    entries, _ = package.tree(release)
    entries.pop(MANIFEST)
    require(entries == json.loads(installed_manifest)['entries'], 'Installed full tree mismatch')
    require(entries[HELPER]['sha256'] == LIVE, 'Installed helper pairing mismatch')
    installed = {
        'standalone': namespace / 'bin/smith-service',
        'unit': config / 'systemd/user/smith-next.service',
        'desktop': data / 'applications/dev.smith.smith_client.desktop',
        'shim': home / '.local/bin/smith-next',
        'app_state': namespace / 'app-state.json',
        'local_dev_ledger': namespace / 'dev-local-jobs.json',
        'receipt_dev_ledger': namespace / 'dev-jobs.json',
    }
    installed_before = {n: (identity(p.lstat()), digest(read(p))) for n, p in installed.items()}
    require(installed_before['standalone'][1] == LIVE, 'Installed standalone pairing mismatch')
    enablement = config / 'systemd/user/default.target.wants/smith-next.service'
    require(enablement.is_symlink() and enablement.resolve() == installed['unit'], 'Enablement drift')
    enablement_before = (identity(enablement.lstat()), os.readlink(enablement))
    processes = []
    for p in Path('/proc').iterdir():
        if not p.name.isdigit():
            continue
        try:
            if (p / 'comm').read_text().strip() == 'smith-service':
                processes.append(p)
        except (FileNotFoundError, PermissionError, ProcessLookupError):
            continue
    require(len(processes) == 1, 'Expected one live service process')
    proc = processes[0]
    def running():
        return (digest((proc / 'exe').read_bytes()), digest((proc / 'cmdline').read_bytes()))
    running_before = running()
    require(running_before[0] == LIVE, 'Running service does not match installed pair')
    core_state = namespace / 'state.json'
    require(not core_state.exists() and not core_state.is_symlink(), 'Unexpected core-state file')
    log.write('PASS: fresh installed standalone/helper/running pair b7790a58; selected release verified read-only.\n')

    validation = validate_current_client(log)
    sources = validation['source_sha256']
    require(sources['service/dev-worker/guard.mjs'] == GUARD, 'Guard source drift')
    binary = read(BINARY)
    require(digest(binary) == NEW, 'Retained tested service artifact drift')
    require(read(ROOT / 'service/dev-worker/guard.mjs') in binary, 'Guard not embedded in service')
    bundle_before = raw_tree(BUNDLE)
    bundle_files = {name for name, meta in bundle_before.items() if meta['type'] == 'file'}
    omitted = {'lib/native_assets.json'}
    require(bundle_files - package.CLIENT_FILES == omitted, 'Unreviewed Flutter build file')
    native = read(BUNDLE / 'lib/native_assets.json')
    require(json.loads(native) == {'format-version': [1, 0, 0], 'native-assets': {}}
            and native == read(BUNDLE / 'data/flutter_assets/NativeAssetsManifest.json'),
            'Only exact redundant empty native-assets manifest may be omitted')
    require(bundle_before == validation['flutter_build_inventory'], 'Fresh build artifact drift')
    require(bundle_before['lib/libapp.so']['sha256'] != old_result['flutter_libapp_sha256'],
            'AOT artifact unchanged despite reviewed production navigation change')
    log.write('PASS: current source validated and newly built AOT inventory bound; retained tested service/guard unchanged.\n')

    runtime_files = {'smith_backend/runtime/' + name for name in package.RUNTIME_FILES}
    desktop_files = {'share/applications/' + package.APP + '.desktop',
                     'share/pixmaps/' + package.APP + '.png'}
    supplement = runtime_files | (desktop_files - bundle_files)
    copied_build = bundle_files - omitted
    require(not copied_build & supplement and HELPER not in copied_build, 'Overlapping source provenance')
    require(copied_build | supplement | {HELPER} == package.CLIENT_FILES, 'Missing/unreviewed package source')
    for name in supplement:
        require(entries[name]['sha256'] == base_inventory['payload/client/' + name]['sha256'],
                'Installed supplementary file differs from retained verified package')
    directory(DEST / 'payload')
    directory(DEST / 'payload/client')
    directory(DEST / 'payload/service')
    for name in sorted(package.CLIENT_DIRS, key=lambda n: (n.count('/'), n)):
        directory(DEST / 'payload/client' / name)
    provenance = {}
    for name in sorted(package.CLIENT_FILES):
        if name == HELPER:
            content, origin = binary, 'current_tested_service'
        elif name in copied_build:
            content, origin = read(BUNDLE / name), 'current_flutter_release_bundle'
            require(digest(content) == bundle_before[name]['sha256'], 'Flutter copy input drift')
        else:
            content, origin = read(release / name), 'verified_installed_runtime_or_missing_desktop'
            require(digest(content) == entries[name]['sha256'], 'Supplementary copy input drift')
        mode = 0o755 if name in {'smith_client', HELPER} or name.endswith('/llama-server') else 0o644
        write(DEST / 'payload/client' / name, content, mode)
        provenance[name] = {'origin': origin, 'sha256': digest(content), 'mode': mode}
    write(DEST / 'payload/service/smith-service', binary, 0o755)
    for name in ('smith-installer', 'smith-installer-ui'):
        content = read(BASE / name)
        require(digest(content) == base_inventory[name]['sha256'], 'Installer copy input drift')
        write(DEST / name, content, 0o755)
    with contextlib.redirect_stdout(log):
        package.build(DEST, BASE / 'README.txt')
        package.verify(DEST)
    result_manifest = json.loads(read(DEST / 'manifest.json'))
    require(result_manifest['service_sha256'] == NEW, 'Candidate service pair mismatch')
    candidate_entries, _ = package.tree(DEST / 'payload/client')
    for name, meta in provenance.items():
        require(candidate_entries[name]['sha256'] == meta['sha256'], 'Candidate provenance mismatch')
    require(MANIFEST not in candidate_entries, 'Installed manifest must not be copied')
    for p in DEST.rglob('*'):
        mode = stat.S_IMODE(p.lstat().st_mode)
        require(not p.is_symlink() and p.lstat().st_nlink == 1 if p.is_file() else not p.is_symlink(),
                'Linked candidate output')
        require(mode == 0o755 if p.is_dir() else mode in (0o644, 0o755), 'Output mode mismatch')

    # Build may recreate its output directories; revalidate safe ancestry rather
    # than comparing build-time directory mtimes. No source permissions change.
    for p in build_directory_ids:
        info = p.lstat()
        require(stat.S_ISDIR(info.st_mode) and not p.is_symlink()
                and info.st_uid == os.geteuid(), 'Flutter source ancestry became unsafe')
    require(raw_tree(BUNDLE) == bundle_before, 'Flutter build inputs changed')
    require(hashes(source_paths()) == sources, 'Production input membership or bytes changed')
    require(hashes(preparation_paths) == preparation_hashes, 'Preparation implementation drift')
    require(hashes(test_paths()) == validation['test_inputs_sha256'], 'Validation test inputs drift')
    require(digest(read(BINARY)) == NEW, 'Retained tested service binary drift')
    require(package.inventory(OLD_CANDIDATE) == old_inventory
            and read(EVIDENCE / 'coordinated-candidate-result.json') == old_result_bytes
            and digest(read(OLD_CANDIDATE / 'manifest.json')) == old_result['package_manifest_sha256'],
            'Previous candidate or evidence changed')
    require({n: (identity(p.lstat()), digest(read(p))) for n, p in installed.items()} == installed_before,
            'Installed object or state changed')
    final_entries, _ = package.tree(release)
    final_entries.pop(MANIFEST)
    require(final_entries == entries and read(release / MANIFEST) == installed_manifest,
            'Installed release changed')
    require((identity(selector.lstat()), os.readlink(selector)) == selector_before, 'Selector changed')
    require((identity(enablement.lstat()), os.readlink(enablement)) == enablement_before, 'Enablement changed')
    require(running() == running_before, 'Running process changed')
    require(not core_state.exists() and not core_state.is_symlink(), 'Core-state absence changed')
    with contextlib.redirect_stdout(log):
        package.verify(BASE)
    require(read(BASE / 'manifest.json') == base_manifest and package.inventory(BASE) == base_inventory,
            'Retained package changed')
    for folder in [DEST, *(p for p in DEST.rglob('*') if p.is_dir())]:
        fd = os.open(folder, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)
    log.write('PASS: fresh complete client/service candidate assembled and verified; all installed before/after identities and hashes unchanged.\n')
    return {
        'status': 'prepared_verified_candidate_only_not_installed_or_accepted',
        'package': DEST.relative_to(ROOT).as_posix(),
        'service_sha256': NEW,
        'flutter_executable_sha256': provenance['smith_client']['sha256'],
        'flutter_libapp_sha256': provenance['lib/libapp.so']['sha256'],
        'guard_sha256': GUARD,
        'client_manifest_sha256': result_manifest['client_manifest_sha256'],
        'package_manifest_sha256': digest(read(DEST / 'manifest.json')),
        'checksums_sha256': digest(read(DEST / 'SHA256SUMS')),
        'selected_installed_release_sha256': SELECTED,
        'installed_helper_before_after_sha256': LIVE,
        'running_service_pid': int(proc.name),
        'running_service_before_after_sha256': running_before[0],
        'installed_before_after_sha256': {n: item[1] for n, item in installed_before.items()},
        'installed_unchanged': True,
        'retained_package_manifest_sha256': BASE_MANIFEST,
        'source_sha256': sources,
        'source_freshness_contract': 'Exact production/build input membership and hashes only; excludes tests, mutable docs/plans and evidence reports',
        'preparation_inputs_sha256': preparation_hashes,
        'validation_record': 'docs/evidence/dev-buffered-frames/coordinated-candidate-v2-validation.json',
        'validation_record_sha256': digest(read(EVIDENCE / 'coordinated-candidate-v2-validation.json')),
        'historical_document_references_sha256_not_freshness_inputs': validation['historical_document_references_sha256_not_freshness_inputs'],
        'old_candidate_unchanged': True,
        'client_file_provenance': provenance,
        'flutter_build_inventory': bundle_before,
        'explicitly_omitted_build_files': {'lib/native_assets.json': {
            'sha256': digest(native),
            'reason': 'Exact redundant empty manifest; packaged NativeAssetsManifest.json is byte-identical'}},
        'candidate_inventory': result_manifest['files'],
        'source_binding': 'Current client inputs held stable across fresh focused tests, full analysis and Linux release build; service 9bb retained from prior scoped checks with unchanged service production inputs',
        'validation_limit': 'Retained timestamp fixture failure not waived; no installed, OpenCode/model or runtime acceptance',
        'product_executable_run': False,
        'installation_performed': False,
        'worker_image_or_tag_changed': False,
        'unknown_reconciled': False,
        'runtime_state_modified': False,
        'network_used': False,
    }


def main():
    os.umask(0o077)
    result_path = EVIDENCE / 'coordinated-candidate-v2-result.json'
    log_path = EVIDENCE / 'coordinated-candidate-v2-prepare.log'
    require(not result_path.exists() and not log_path.exists(), 'Existing v2 evidence; no overwrite/retry')
    previous = None
    log = io.StringIO()
    try:
        result = prepare(log)
    except Exception as error:
        # Never emit third-party exception messages containing sensitive paths.
        if type(error) is RuntimeError:
            log.write('Precondition: ' + str(error) + '\n')
        log.write('FAIL: preparation stopped; partial outputs retained; no installation or runtime action attempted.\n')
        prior.save_log(log_path, previous, log.getvalue().encode())
        print('Preparation failed (' + type(error).__name__ + '); inspect safe evidence log. No automatic retry/cleanup.')
        raise SystemExit(1) from None
    prior.save_log(log_path, previous, log.getvalue().encode())
    result['log_sha256'] = digest(read(log_path))
    write(result_path, (json.dumps(result, sort_keys=True, indent=2) + '\n').encode(), 0o600)
    print(json.dumps({k: result[k] for k in (
        'status', 'package', 'service_sha256', 'flutter_libapp_sha256',
        'client_manifest_sha256', 'package_manifest_sha256', 'installed_unchanged')}, indent=2))


if __name__ == '__main__':
    main()
