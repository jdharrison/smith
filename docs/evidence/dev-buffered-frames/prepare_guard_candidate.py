"""One-shot local QA package preparation; never executes a product or host action."""
import contextlib
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import sys

ROOT = Path(__file__).resolve().parents[3]
EVIDENCE = Path(__file__).resolve().parent
DEST = ROOT / '.smith-dev-guard-candidate'
BASE = ROOT / '.smith-installer-preview-20261017-a4c9/package'
BINARY = ROOT / 'service/target/debug/smith-service'
NEW = '8cd17bc3e5e17a0df5df5f4ce9b5860a113355fd9a9bb4611404a58f5ce9a457'
OLD = '2500f2f114342895d4a0e8c7da3247b2ccde7c47685716dd3c32450fed32e708'
SELECTED = '6f214078f9d6ae3e5d04d5aa21ef7b5a907ffc417c2ae139ee6f61499b836602'
BASE_SERVICE = '88b0a37133863c766991d8793763bb2651055391915a434cf6d99884d6c37cca'
HELPER = 'smith_backend/smith-service'
MANIFEST = '.smith-next-manifest.json'


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def identity(info):
    return (info.st_dev, info.st_ino, info.st_uid, info.st_mode,
            info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def read(path):
    before = path.lstat()
    require(stat.S_ISREG(before.st_mode)
            and (before.st_nlink == 1 or path == BINARY)
            and before.st_uid == os.geteuid() and before.st_size <= 128 * 1024 * 1024,
            'Input must be a bounded owned regular file; only the Cargo source binary may be hard-linked')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as handle:
        require(identity(os.fstat(handle.fileno())) == identity(before), 'Input replaced')
        data = handle.read(128 * 1024 * 1024 + 1)
        require(identity(os.fstat(handle.fileno())) == identity(before)
                and identity(path.lstat()) == identity(before), 'Input changed during read')
    require(len(data) == before.st_size, 'Input size changed')
    return data


def write(path, data, mode):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, mode)
    with os.fdopen(fd, 'wb') as handle:
        os.fchmod(handle.fileno(), mode)
        handle.write(data)
        handle.flush()
        os.fsync(handle.fileno())


def directory(path):
    path.mkdir(mode=0o755)
    path.chmod(0o755)  # Explicit modes inside the private output root, independent of umask.


def prepare(log):
    require(os.geteuid() != 0, 'Non-root execution required')
    require(DEST.is_dir() and not DEST.is_symlink() and not any(DEST.iterdir()),
            'Candidate directory must exist and be fresh/empty')
    require(DEST.lstat().st_uid == os.geteuid(), 'Candidate directory must be owned')
    os.chmod(DEST, 0o700)  # Only the explicitly approved fresh output directory.
    sys.path.insert(0, str(ROOT / 'dagger'))
    spec = importlib.util.spec_from_file_location('guard_candidate_package', ROOT / 'dagger/package.py')
    package = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(package)
    package.check_location(DEST)
    with contextlib.redirect_stdout(log):
        package.verify(BASE)
    base_manifest_bytes = read(BASE / 'manifest.json')
    base_manifest = json.loads(base_manifest_bytes)
    require(base_manifest['service_sha256'] == BASE_SERVICE, 'Retained package identity drift')
    base_inventory = package.inventory(BASE)

    home = Path(os.environ['HOME'])
    data = Path(os.environ.get('XDG_DATA_HOME', str(home / '.local/share')))
    config = Path(os.environ.get('XDG_CONFIG_HOME', str(home / '.config')))
    runtime = Path(os.environ['XDG_RUNTIME_DIR'])
    for root in (home, data, config, runtime):
        package.check_location(root)
    namespace = data / 'smith-next'
    selector = namespace / 'client/current'
    require(selector.is_symlink() and os.readlink(selector) == 'releases/' + SELECTED,
            'Selected client identity drift')
    release = namespace / 'client/releases' / SELECTED
    package.check_location(release)
    manifest_bytes = read(release / MANIFEST)
    require(digest(manifest_bytes) == SELECTED, 'Selected manifest hash mismatch')
    manifest = json.loads(manifest_bytes)
    require(manifest.get('schema') == 1, 'Unexpected selected manifest schema')
    entries, _ = package.tree(release)
    entries.pop(MANIFEST)
    require(entries == manifest['entries'], 'Selected full tree does not match manifest')
    require(entries[HELPER]['sha256'] == OLD, 'Installed helper drift')
    for name in entries:
        read_path = release / name
        require(read_path.lstat().st_uid == os.geteuid(), 'Foreign selected entry')
    selector_identity = identity(selector.lstat())
    manifest_identity = identity((release / MANIFEST).lstat())

    # Observe identities only; no service/helper/control command is executed.
    installed = {
        'standalone': namespace / 'bin/smith-service',
        'unit': config / 'systemd/user/smith-next.service',
        'desktop': data / 'applications/dev.smith.smith_client.desktop',
        'shim': home / '.local/bin/smith-next',
        'app_state': namespace / 'app-state.json',
        'local_dev_ledger': namespace / 'dev-local-jobs.json',
        'receipt_dev_ledger': namespace / 'dev-jobs.json',
    }
    installed_before = {name: (identity(path.lstat()), digest(read(path)))
                        for name, path in installed.items()}
    require(installed_before['standalone'][1] == OLD, 'Installed standalone drift')
    enablement = config / 'systemd/user/default.target.wants/smith-next.service'
    require(enablement.is_symlink() and enablement.resolve() == installed['unit'],
            'Enablement identity drift')
    enablement_before = (identity(enablement.lstat()), os.readlink(enablement))
    proc = Path('/proc/126935')
    running_before = (digest((proc / 'exe').read_bytes()), digest((proc / 'cmdline').read_bytes()))
    require(running_before[0] == OLD, 'Running service identity drift')
    core_state = namespace / 'state.json'
    require(not core_state.exists() and not core_state.is_symlink(), 'Core state absence drift')

    source_paths = {ROOT / 'dagger/package.py', ROOT / 'dagger/runtime.py', BINARY,
                    ROOT / 'service/Cargo.toml', ROOT / 'service/Cargo.lock',
                    ROOT / 'service/.dockerignore', Path(__file__).resolve()}
    for folder in ('service/src', 'service/dev-worker', 'service/scripts'):
        source_paths.update(p for p in (ROOT / folder).rglob('*') if p.is_file())
    source_paths.update(EVIDENCE.glob('guard-schema-*'))
    sources = {p.relative_to(ROOT).as_posix(): digest(read(p)) for p in sorted(source_paths)}
    evidence = json.loads(read(EVIDENCE / 'guard-schema-result.json'))
    for name, expected in evidence['source_sha256'].items():
        require(sources[name] == expected, 'Guard source/evidence binding drift')
    require(sources[BINARY.relative_to(ROOT).as_posix()] == NEW, 'Tested executable drift')
    binary = read(BINARY)
    require(digest(binary) == NEW, 'Tested executable changed')

    # Copy manifest-approved contents only; never copy/edit the installed manifest.
    directory(DEST / 'payload')
    directory(DEST / 'payload/client')
    directory(DEST / 'payload/service')
    for name, meta in sorted(entries.items(), key=lambda item: (item[0].count('/'), item[0])):
        target = DEST / 'payload/client' / name
        if meta['type'] == 'directory':
            directory(target)
        else:
            content = binary if name == HELPER else read(release / name)
            if name != HELPER:
                require(digest(content) == meta['sha256'], 'Selected copy input drift')
            require(meta['mode'] in (0o644, 0o755), 'Unexpected selected file mode')
            write(target, content, 0o755 if name == HELPER else meta['mode'])
    write(DEST / 'payload/service/smith-service', binary, 0o755)
    for name in ('smith-installer', 'smith-installer-ui'):
        content = read(BASE / name)
        require(digest(content) == base_inventory[name]['sha256'], 'Retained installer drift')
        write(DEST / name, content, 0o755)
    with contextlib.redirect_stdout(log):
        package.build(DEST, BASE / 'README.txt')
        package.verify(DEST)
    result_manifest = json.loads(read(DEST / 'manifest.json'))
    candidate_entries, _ = package.tree(DEST / 'payload/client')
    changed = sorted(name for name in set(entries) | set(candidate_entries)
                     if entries.get(name) != candidate_entries.get(name))
    require(changed == [HELPER], 'Candidate changed more than the paired helper')
    require(result_manifest['service_sha256'] == NEW, 'Candidate pair hash mismatch')
    require(not (DEST / 'payload/client' / MANIFEST).exists(), 'Installed manifest copied')
    for path in DEST.rglob('*'):
        mode = stat.S_IMODE(path.lstat().st_mode)
        require(not path.is_symlink(), 'Candidate contains a symlink')
        require(mode == 0o755 if path.is_dir() else mode in (0o644, 0o755),
                'Candidate mode mismatch')

    # Revalidate every retained input and installed object, not merely copied bytes.
    with contextlib.redirect_stdout(log):
        package.verify(BASE)
    require(read(BASE / 'manifest.json') == base_manifest_bytes
            and package.inventory(BASE) == base_inventory, 'Retained package changed')
    final_entries, _ = package.tree(release)
    final_entries.pop(MANIFEST)
    require(final_entries == entries and read(release / MANIFEST) == manifest_bytes
            and identity((release / MANIFEST).lstat()) == manifest_identity,
            'Installed release changed')
    require(identity(selector.lstat()) == selector_identity
            and os.readlink(selector) == 'releases/' + SELECTED, 'Installed selector changed')
    require({name: (identity(path.lstat()), digest(read(path)))
             for name, path in installed.items()} == installed_before, 'Installed object/state changed')
    require((identity(enablement.lstat()), os.readlink(enablement)) == enablement_before,
            'Enablement changed')
    require((digest((proc / 'exe').read_bytes()), digest((proc / 'cmdline').read_bytes()))
            == running_before, 'Running service changed')
    require(not core_state.exists() and not core_state.is_symlink(), 'Core state changed')
    require({name: digest(read(ROOT / name)) for name in sources} == sources, 'Source input drift')
    for folder in [DEST, *(p for p in DEST.rglob('*') if p.is_dir())]:
        fd = os.open(folder, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)
    log.write('PASS: paired QA package verified; only copied helper changed; retained and installed inputs unchanged.\n')
    return {
        'status': 'prepared_qa_candidate_not_release_or_deployment',
        'package': DEST.relative_to(ROOT).as_posix(),
        'package_manifest_sha256': digest(read(DEST / 'manifest.json')),
        'checksums_sha256': digest(read(DEST / 'SHA256SUMS')),
        'client_release_sha256': result_manifest['client_manifest_sha256'],
        'service_sha256': NEW,
        'selected_source_release_sha256': SELECTED,
        'retained_package_manifest_sha256': digest(base_manifest_bytes),
        'retained_input_inventory': base_inventory,
        'selected_source_manifest_entries': entries,
        'source_sha256': sources,
        'candidate_inventory': result_manifest['files'],
        'installed_before_after_sha256': {name: value[1] for name, value in installed_before.items()},
        'installed_unchanged': True,
        'candidate_client_changed_entries': changed,
        'source_validation_evidence': evidence,
        'broader_timestamp_fixture': 'FAIL retained; not rerun or waived',
        'product_executable_run': False,
        'deployment_performed': False,
        'worker_image_changed': False,
        'unknown_reconciled': False,
        'live_runtime_acceptance': False,
    }


def save_log(path, previous, content):
    if previous is None:
        write(path, content, 0o600)
    else:
        require(read(path) == previous, 'Previous preparation log changed')
        fd = os.open(path, os.O_WRONLY | os.O_APPEND | os.O_NOFOLLOW)
        with os.fdopen(fd, 'ab') as handle:
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())


def main():
    os.umask(0o077)
    result_path = EVIDENCE / 'guard-candidate.json'
    log_path = EVIDENCE / 'guard-candidate.log'
    require(not result_path.exists(), 'Candidate result already exists')
    previous = read(log_path) if log_path.exists() else None
    if previous is not None:
        require(not any(DEST.iterdir()), 'Failed candidate is not empty; no automatic retry')
    log = io.StringIO()
    if previous is not None:
        log.write('\nScoped rerun: accept the hash-bound read-only Cargo binary hard link; candidate files remain single-link.\n')
    try:
        result = prepare(log)
    except Exception as error:
        # Third-party exceptions may contain sensitive absolute paths: never print them.
        if type(error) is RuntimeError:
            log.write('Precondition: ' + str(error) + '\n')
        log.write('FAIL: preparation stopped; no installation or host action attempted.\n')
        save_log(log_path, previous, log.getvalue().encode())
        print('Candidate preparation failed (' + type(error).__name__ + '); outputs retained, no retry/cleanup.')
        raise SystemExit(1) from None
    log_bytes = log.getvalue().encode()
    save_log(log_path, previous, log_bytes)
    result['log_sha256'] = digest(read(log_path))
    write(result_path, (json.dumps(result, indent=2, sort_keys=True) + '\n').encode(), 0o600)
    print(json.dumps({key: result[key] for key in (
        'status', 'package', 'package_manifest_sha256', 'client_release_sha256',
        'service_sha256', 'installed_unchanged', 'candidate_client_changed_entries')}, indent=2))


if __name__ == '__main__':
    main()
