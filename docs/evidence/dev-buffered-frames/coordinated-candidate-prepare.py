"""One-shot, offline package preparation only; never execute any product binary."""
import contextlib
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
EVIDENCE = Path(__file__).resolve().parent
DEST = ROOT / '.smith-dev-coordinated-candidate'
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
# Exact reviewed deltas from the independently bound reconciled-state snapshot.
DELTAS = {
    'client/lib/dev_jobs.dart': '605e8f14c2e3ab3a8d32e31e0a66d8f55b2a9041e8f270ca3a6a729c8c341ea1',
    'client/lib/dev_jobs_controller.dart': '511b5ef3dcdd6e526a1a8e083e1b87dbdcb7cba2b2f978663ee5353b9dae2dd9',
    'client/lib/ui/dev_jobs_view.dart': '91c269b0563f6a87d1bdbb9c08abe118af972ea98663105e327ef5971a54cc33',
    'client/test/dev_jobs_controller_list_test.dart': '10daaa138dac263bb6a1f392dd59d58d289ca07e6cb5c0975988d4307be77aed',
    'client/test/dev_jobs_list_view_test.dart': '051e886ead0d856c3112c15e05ea8a2ec070d19d251d516f337af5f400a5c6a0',
    'client/test/dev_jobs_test.dart': 'ebe4b1f3f918b26b6bb2e95a33b99105a91787dc13a7562f4de30100b4b56d06',
    'docs/ARCHITECTURE_DETAILS.md': '04cffc729ac4210d8546bcd79e19dad4cf40996caa401ee4075f3c4a49a31924',
    'docs/DESIGN_DETAILS.md': 'bdd03fe0376fd34e94a17d1de2247a8fcbad1fbdec90344bcb5c7b45a311aaac',
    'docs/PROCESS_DETAILS.md': 'd49669e05e6274c1bf5440f8ffa63bb167491a8e5e754e7e0256254364385235',
    'docs/plans/LOCAL-SMITH-DEV-BUFFERED-FRAMES.md': 'adcda856f94f97b038e1e3dc17b24cf9d1e36372b725902cecc88b96d7b23817',
    'service/src/app_dev_local.rs': '6431aed05381438ebe0b07e73d050e7d0edac3a8404e439bc8c1235be8acbf26',
    'service/src/app_dev_local_tests.rs': 'f628ee46b0e046bc2e6229317e3be40c150abfc7a715b26db908c794ca8dee32',
    'service/src/app_v3.rs': 'e9a2778cbb79e91a51595d40a68cfd9805f6798651eda8d9e33ca694ebd11ee9',
}


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
    paths = {BINARY, Path(__file__).resolve(), EVIDENCE / 'prepare_guard_candidate.py'}
    for folder in ('service/src', 'service/dev-worker', 'service/scripts',
                   'client/lib', 'client/test', 'client/assets', 'dagger'):
        paths.update(p for p in (ROOT / folder).rglob('*')
                     if p.is_file() and '__pycache__' not in p.parts)
    for name in ('service/Cargo.toml', 'service/Cargo.lock', 'service/.dockerignore',
                 'client/pubspec.yaml', 'client/pubspec.lock'):
        paths.add(ROOT / name)
    paths.update(ROOT / name for name in DELTAS)
    for pattern in ('delete-*', 'reconciled-*', 'guard-schema-*'):
        paths.update(p for p in EVIDENCE.glob(pattern) if p.is_file())
    paths.add(EVIDENCE / 'coordinated-live-preflight.json')
    paths.add(EVIDENCE / 'guard-candidate.json')
    return paths


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

    sources = hashes(source_paths())
    baseline = {}
    for line in read(EVIDENCE / 'reconciled-independent-source-after.sha256').decode().splitlines():
        expected, name = line.split('  ', 1)
        baseline[name] = expected
    for name, expected in baseline.items():
        require(digest(read(ROOT / name)) == DELTAS.get(name, expected),
                'Unexpected source delta: ' + name)
    require(sources['service/target/debug/smith-service'] == NEW, 'Tested service artifact drift')
    require(sources['service/dev-worker/guard.mjs'] == GUARD, 'Guard source drift')
    controller_binding = read(EVIDENCE / 'delete-controller-fix-sha256.log').decode()
    for line in controller_binding.splitlines():
        expected, name = line.split('  ', 1)
        require(digest(read(ROOT / name)) == expected, 'Final controller evidence drift')
    for name in ('client/test/rust_dev_jobs_integration_test.dart',
                 'service/target/debug/deps/debug_service-0a4be2ddda80d80f'):
        expected = json.loads(read(EVIDENCE / 'delete-compiled-identity.json'))[name]
        # Cargo test executables can be hard-linked, like the production source binary.
        if name.startswith('service/target/'):
            p = ROOT / name
            require(p.is_file() and not p.is_symlink(), 'Compiled fixture input unsafe')
            require(digest(p.read_bytes()) == expected, 'Compiled fixture artifact drift')
        else:
            require(digest(read(ROOT / name)) == expected, 'Compiled fixture source drift')
    binary = read(BINARY)
    require(read(ROOT / 'service/dev-worker/guard.mjs') in binary, 'Guard not embedded in service')

    bundle_before = raw_tree(BUNDLE)
    bundle_files = {name for name, meta in bundle_before.items() if meta['type'] == 'file'}
    omitted = {'lib/native_assets.json'}
    require(bundle_files - package.CLIENT_FILES == omitted, 'Unreviewed Flutter build file')
    native = read(BUNDLE / 'lib/native_assets.json')
    require(json.loads(native) == {'format-version': [1, 0, 0], 'native-assets': {}}
            and native == read(BUNDLE / 'data/flutter_assets/NativeAssetsManifest.json'),
            'Only exact redundant empty native-assets manifest may be omitted')
    require(bundle_before['smith_client']['sha256'] ==
            '7b382f05aeba0d226adc32922fd0a2cdbf78e2275131c18539e39bb40fc0562c', 'Flutter executable drift')
    require(bundle_before['lib/libapp.so']['sha256'] ==
            'fbda56150a15d794a0e6c39bc5159baadbd5a21c7046c7faa6731bc902156e59', 'Flutter AOT artifact drift')
    app_time = (BUNDLE / 'lib/libapp.so').stat().st_mtime_ns
    require(all(p.stat().st_mtime_ns <= app_time for p in (ROOT / 'client/lib').rglob('*.dart')),
            'Client source newer than existing AOT build')
    require((EVIDENCE / 'delete-final-linux.log').stat().st_mtime_ns >= app_time,
            'AOT build newer than final scoped build log')
    require('All tests passed!' in read(EVIDENCE / 'delete-final-tests.log').decode()
            and 'No issues found!' in read(EVIDENCE / 'delete-final-analyze.log').decode()
            and 'Built build/linux/x64/release/bundle/smith_client' in
            read(EVIDENCE / 'delete-final-linux.log').decode(), 'Missing final client evidence')
    log.write('PASS: exact source deltas, current artifact identities, embedded guard and retained scoped evidence bound. No compilation performed.\n')

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

    # Recheck every source and installed hash/identity. Never execute service,
    # helper, installer, systemctl, Docker, GUI, or any network operation.
    require({p: identity(p.lstat()) for p in build_directory_ids} == build_directory_ids,
            'Flutter build source ancestry changed')
    require(raw_tree(BUNDLE) == bundle_before, 'Flutter build inputs changed')
    require(hashes(source_paths()) == sources, 'Source/task/evidence membership or bytes changed')
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
        'client_file_provenance': provenance,
        'flutter_build_inventory': bundle_before,
        'explicitly_omitted_build_files': {'lib/native_assets.json': {
            'sha256': digest(native),
            'reason': 'Exact redundant empty manifest; packaged NativeAssetsManifest.json is byte-identical'}},
        'candidate_inventory': result_manifest['files'],
        'source_binding': 'Prior independent source snapshot plus exact Delete/minimal-UI deltas; final scoped logs and existing build hashes retained; no new compile',
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
    result_path = EVIDENCE / 'coordinated-candidate-result.json'
    log_path = EVIDENCE / 'coordinated-candidate-prepare.log'
    require(not result_path.exists(), 'Existing result; no overwrite/retry')
    previous = read(log_path) if log_path.exists() else None
    if previous is not None:
        require(previous == b'FAIL: preparation stopped; partial outputs retained; no installation or runtime action attempted.\n'
                and DEST.is_dir() and not any(DEST.iterdir()),
                'Only the exact empty-output source-location precheck failure may be resumed')
    log = io.StringIO()
    if previous is not None:
        log.write('\nScoped correction: initial attempt applied the export-location gate to existing Flutter source build (0775). No files were copied; output was empty. Source permissions remain unchanged; source ancestry/bytes are bound read-only, and the full output location gate remains mandatory.\n')
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
