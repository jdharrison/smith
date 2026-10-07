#!/usr/bin/env bash
set -euo pipefail
exec python3 - "$@" <<'PY'
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
from unittest import mock

os.umask(0o077)
repo = Path.cwd()
installer = repo / 'scripts/install-prebuilt.sh'
packager = repo / 'scripts/package-chat.sh'
if not installer.is_file() or not packager.is_file():
    sys.exit('run from the Smith repository root')
if os.geteuid() == 0:
    sys.exit('run this isolated smoke as a non-root user')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def native(path):
    header = path.read_bytes()[:20]
    assert len(header) == 20 and header[:6] == b'\x7fELF\x02\x01' and header[18:20] == b'\x3e\x00'
    assert path.stat().st_mode & 0o111


def checks(package):
    (package / 'SHA256SUMS').write_text(''.join(digest(p) + '  ' + p.relative_to(package).as_posix() + '\n' for p in sorted(package.rglob('*')) if p.is_file() and p.name != 'SHA256SUMS'))


with tempfile.TemporaryDirectory(prefix='smith-prebuilt-smoke-') as temporary:
    base = Path(temporary)
    mocks = base / 'mocks'
    mocks.mkdir()
    calls = base / 'forbidden-calls'
    for command in ('systemctl', 'loginctl', 'docker', 'ollama', 'curl', 'wget', 'cargo', 'flutter', 'dart'):
        path = mocks / command
        path.write_text('#!/bin/sh\nprintf "%s\\n" forbidden >> "' + str(calls) + '"\nexit 99\n')
        path.chmod(0o755)
    home = base / 'home'
    home.mkdir()
    env = dict(os.environ, HOME=str(home), XDG_CONFIG_HOME=str(home / 'config'), XDG_DATA_HOME=str(home / 'data'), XDG_RUNTIME_DIR=str(home / 'run'), DBUS_SESSION_BUS_ADDRESS='unix:path=/nonexistent-smith-smoke', PATH=str(mocks) + ':' + os.environ['PATH'])

    def command(args, success=True, cwd=repo):
        result = subprocess.run(args, cwd=cwd, env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
        if (result.returncode == 0) != success:
            raise AssertionError(f'unexpected exit {result.returncode}: {result.stdout}{result.stderr}')
        return result

    def root(name):
        path = base / name
        path.mkdir(mode=0o700)
        return path

    def install(package, destination, success=True):
        return command(['bash', str(installer), '--package', str(package), '--root', str(destination)], success)

    package = root('backend-package')
    (package / 'payload').mkdir()
    binary = package / 'payload/smith'
    binary.write_text('#!/bin/sh\nprintf executed >> "' + str(calls) + '"\nexit 99\n')
    binary.chmod(0o755)
    (package / 'manifest.json').write_text(json.dumps({'format': 'smith-chat-candidate-v1', 'target': 'ubuntu-24.04-x86_64', 'kind': 'backend', 'version': '0.4.0'}))
    (package / 'LICENSE').write_text('fixture license\n')
    checks(package)
    installed = root('installed')
    (installed / 'existing-data').write_text('preserved')
    install(package, installed)
    assert digest(installed / 'bin/smith') == digest(binary)
    assert (installed / '.smith-install-root').read_text() == 'smith-install-root-v1\n'
    assert (installed / '.smith-install-root').stat().st_mode & 0o777 == 0o600
    assert (installed / 'bin/smith').stat().st_mode & 0o777 == 0o755
    install(package, installed, False)
    rollback = root('rollback-root')
    install(package, rollback)
    assert digest(installed / 'bin/smith') == digest(rollback / 'bin/smith')
    assert (installed / 'existing-data').read_text() == 'preserved'
    print('PASS backend copy/marker/modes/data preservation/existing refusal/fresh-root rollback')

    binary.write_text(binary.read_text() + '# corruption\n')
    destination = root('corrupt-target')
    install(package, destination, False)
    assert list(destination.iterdir()) == []
    checks(package)
    binary.chmod(0o777)
    install(package, destination, False)
    binary.chmod(0o755)
    (package / 'extra').write_text('unlisted')
    install(package, destination, False)
    (package / 'extra').unlink()
    os.link(binary, package / 'hardlink')
    install(package, destination, False)
    (package / 'hardlink').unlink()
    (package / 'escape').symlink_to(home)
    install(package, destination, False)
    (package / 'escape').unlink()
    (package / 'SHA256SUMS').write_text(digest(binary) + '  ../escape\n')
    install(package, destination, False)
    checks(package)
    print('PASS corrupt bytes/unlisted file/hardlink/symlink/mode/checksum traversal rejection')

    destination.chmod(0o777)
    install(package, destination, False)
    destination.chmod(0o700)
    alias = base / 'alias'
    alias.symlink_to(destination, target_is_directory=True)
    install(package, alias, False)
    install(package, base / 'alias/child', False)
    attack = root('bin-symlink-target')
    (attack / 'bin').symlink_to(home, target_is_directory=True)
    install(package, attack, False)
    bad_marker = root('marker-symlink-target')
    (bad_marker / '.smith-install-root').symlink_to(home / 'victim')
    install(package, bad_marker, False)
    assert not (home / 'victim').exists() and not (home / 'smith').exists()
    command(['bash', str(installer), '--package', str(package), '--root', str(destination) + '/../corrupt-target'], False)
    print('PASS unsafe roots/ancestors/destination and marker symlinks/unnormalized paths rejected')

    # Exercise root/ownership/unsupported-host preflight without changing real IDs or host files.
    source = installer.read_text().split("<<'PY'\n", 1)[1].rsplit('\nPY', 1)[0]
    definitions = source.rsplit('\ntry:\n    main()', 1)[0]
    namespace = {'__name__': 'smoke_installer'}
    exec(compile(definitions, 'isolated-installer', 'exec'), namespace)
    with mock.patch.object(sys, 'argv', ['install', '--package', str(package), '--root', str(destination)]):
        for uid in (0, os.geteuid() + 1):
            with mock.patch.object(os, 'geteuid', return_value=uid):
                try:
                    namespace['main']()
                except ValueError:
                    pass
                else:
                    raise AssertionError('root/foreign ownership accepted')
        with mock.patch.object(Path, 'read_text', return_value='ID=unsupported\nVERSION_ID=24.04\n'):
            try:
                namespace['main']()
            except ValueError:
                pass
            else:
                raise AssertionError('unsupported host accepted')
    print('PASS root/ownership/unsupported-host preflight (simulated only)')

    halo = root('halo-package')
    (halo / 'payload/lib').mkdir(parents=True)
    (halo / 'payload/data/flutter_assets').mkdir(parents=True)
    shutil.copyfile(binary, halo / 'payload/smith_halo')
    (halo / 'payload/smith_halo').chmod(0o755)
    (halo / 'payload/smith_backend').mkdir()
    shutil.copy2(binary, halo / 'payload/smith_backend/smith')
    (halo / 'payload/lib/plugin.so').write_text('plugin fixture')
    (halo / 'payload/data/flutter_assets/asset').write_text('asset fixture')
    (halo / 'manifest.json').write_text(json.dumps({'format': 'smith-chat-candidate-v1', 'target': 'ubuntu-24.04-x86_64', 'kind': 'halo', 'version': '0.4.0+1'}))
    checks(halo)
    halo_root = root('halo-root')
    install(halo, halo_root)
    assert (halo_root / 'halo/lib/plugin.so').read_text() == 'plugin fixture'
    assert (halo_root / 'halo/data/flutter_assets/asset').read_text() == 'asset fixture'
    assert digest(halo_root / 'halo/smith_backend/smith') == digest(binary)
    assert digest(halo_root / 'halo/smith_halo') == digest(halo / 'payload/smith_halo')
    assert (halo_root / 'halo/smith_backend/smith').stat().st_mode & 0o777 == 0o755
    assert not (halo_root / 'bin').exists() and not (halo_root / '.smith-install-root').exists()
    assert not (home / 'data').exists()
    install(halo, halo_root, False)
    paired_target = root('paired-corrupt-target')
    paired_binary = halo / 'payload/smith_backend/smith'
    paired_binary.write_text(paired_binary.read_text() + '# corrupt paired backend\n')
    install(halo, paired_target, False)
    assert list(paired_target.iterdir()) == []
    shutil.copy2(binary, paired_binary)
    checks(halo)
    paired_binary.unlink()
    paired_binary.symlink_to(binary)
    install(halo, paired_target, False)
    paired_binary.unlink()
    shutil.copy2(binary, paired_binary)
    checks(halo)
    print('PASS paired whole Halo layout/byte equality/modes/no standalone install or registration/existing refusal/corruption and symlink rejection')

    # Test assembly with inert ELF copies; no compiler, application or real Git operation.
    fixture = root('assembly-fixture')
    for directory in ('scripts', 'docs', 'client', 'target/release', 'client/build/linux/x64/release/bundle/lib', 'client/build/linux/x64/release/bundle/data'):
        (fixture / directory).mkdir(parents=True, exist_ok=True)
    for name in ('scripts/package-chat.sh', 'scripts/install-prebuilt.sh', 'docs/CHAT_RELEASE.md', 'LICENSE', 'Cargo.toml', 'Cargo.lock', 'client/pubspec.yaml', 'client/pubspec.lock'):
        shutil.copyfile(repo / name, fixture / name)
    shutil.copyfile('/usr/bin/true', fixture / 'target/release/smith')
    shutil.copyfile('/usr/bin/true', fixture / 'client/build/linux/x64/release/bundle/smith_halo')
    (fixture / 'target/release/smith').chmod(0o755)
    (fixture / 'client/build/linux/x64/release/bundle/smith_halo').chmod(0o755)
    bundle = fixture / 'client/build/linux/x64/release/bundle'
    release_backend = fixture / 'target/release/smith'
    shutil.copyfile('/usr/bin/true', bundle / 'lib/plugin-fixture.so')
    (bundle / 'lib/plugin-fixture.so').chmod(0o644)
    (fixture / 'client/build/linux/x64/release/bundle/data/asset').write_text('asset')
    fake_git = mocks / 'git'
    fake_git.write_text('#!/usr/bin/env python3\nimport sys\na = sys.argv\nif "ls-files" in a:\n    sys.stdout.buffer.write(b"Cargo.toml\\0Cargo.lock\\0LICENSE\\0client/pubspec.yaml\\0client/pubspec.lock\\0scripts/package-chat.sh\\0scripts/install-prebuilt.sh\\0docs/CHAT_RELEASE.md\\0")\nelif "--abbrev-ref" in a: print("feature/autonomy")\nelif "rev-parse" in a: print("a" * 40)\nelif "status" in a: print(" M docs/CHAT_RELEASE.md")\nelse: sys.exit(99)\n')
    fake_git.chmod(0o755)
    identity = json.loads(command(['bash', str(packager), '--source-identity'], cwd=fixture).stdout)
    metadata = fixture / 'target/build-info.json'
    metadata.write_text(json.dumps({'rustc': 'fixture', 'cargo': 'fixture', 'flutter': 'fixture', 'dart': 'fixture', 'source_sha256': identity['sha256']}))
    artifacts = base / 'artifacts'
    assembly_args = ['bash', str(packager), '--output', str(artifacts), '--build-info', str(metadata)]
    command(assembly_args, cwd=fixture)
    assert len(list(artifacts.glob('*.tar.gz'))) == 2
    for index, archive_path in enumerate(artifacts.glob('*.tar.gz')):
        with tarfile.open(archive_path) as archive:
            members = archive.getnames()
            assert any(name.endswith('/install-prebuilt.sh') for name in members)
            assert any(name.endswith('/LICENSE') for name in members)
            manifest = json.load(archive.extractfile(next(m for m in archive.getmembers() if m.name.endswith('/manifest.json'))))
            assert manifest['source_dirty'] and 'NOT A PRODUCTION RELEASE' in manifest['status']
            assert manifest['source']['sha256'] == identity['sha256']
            kind = manifest['kind']
            backend_path = 'smith' if kind == 'backend' else 'smith_backend/smith'
            packaged = manifest['packaged_backend']
            assert packaged['path'] == 'payload/' + backend_path
            assert packaged['version'] == manifest['backend_version']
            assert packaged['sha256'] == digest(release_backend)
            assert packaged['source_input'] == 'target/release/smith'
            assert packaged['source_sha256'] == identity['sha256']
            assert backend_path in manifest['elf_needed']
            if kind == 'halo':
                assert 'smith_halo' in manifest['elf_needed'] and 'lib/plugin-fixture.so' in manifest['elf_needed']
                assert 'actual smith_halo executable directory' in manifest['runtime']['desktop_backend_discovery']
                assert 'no OS/runtime installation' in manifest['runtime']['installation']
            unpacked = root('unpacked-' + str(index))
            archive.extractall(unpacked, filter='data')
        extracted = next(unpacked.iterdir())
        installed_archive = root('archive-install-' + str(index))
        install(extracted, installed_archive)
        installed_payload = installed_archive / ('bin' if kind == 'backend' else 'halo')
        native(extracted / 'payload' / backend_path)
        native(installed_payload / backend_path)
        assert digest(installed_payload / backend_path) == digest(release_backend)
        for path in (extracted / 'payload').rglob('*'):
            if path.is_file():
                assert digest(path) == digest(installed_payload / path.relative_to(extracted / 'payload'))
        for line in (extracted / 'SHA256SUMS').read_text().splitlines():
            expected, name = line.split('  ')
            assert digest(extracted / name) == expected
        provenance = json.loads(archive_path.with_name(archive_path.name.removesuffix('.tar.gz') + '.provenance.json').read_text())
        assert provenance == manifest
        if kind == 'halo':
            for path in bundle.rglob('*'):
                if path.is_file():
                    assert digest(path) == digest(installed_payload / path.relative_to(bundle))
            native(installed_payload / 'smith_halo')
            assert not (installed_archive / 'bin').exists()
            assert not (installed_archive / '.smith-install-root').exists()
    for line in (artifacts / 'SHA256SUMS').read_text().splitlines():
        expected, name = line.split('  ')
        assert digest(artifacts / name) == expected
    command(assembly_args, False, cwd=fixture)
    metadata.write_text(metadata.read_text().replace(identity['sha256'], '0' * 64))
    command(['bash', str(packager), '--output', str(base / 'bad-artifacts'), '--build-info', str(metadata)], False, cwd=fixture)
    assert not (base / 'bad-artifacts').exists()
    metadata.write_text(json.dumps({'rustc': 'fixture', 'cargo': 'fixture', 'flutter': 'fixture', 'dart': 'fixture', 'source_sha256': identity['sha256']}))
    # Exact pre-existing backend is accepted, never modified in the release bundle.
    paired = bundle / 'smith_backend'
    paired.mkdir(mode=0o755)
    shutil.copy2(release_backend, paired / 'smith')
    matching_digest = digest(paired / 'smith')
    matching_artifacts = base / 'matching-artifacts'
    command(['bash', str(packager), '--output', str(matching_artifacts), '--build-info', str(metadata)], cwd=fixture)
    assert digest(paired / 'smith') == matching_digest
    with tarfile.open(next(matching_artifacts.glob('smith-halo-*.tar.gz'))) as archive:
        member = next(m for m in archive.getmembers() if m.name.endswith('/payload/smith_backend/smith'))
        assert hashlib.sha256(archive.extractfile(member).read()).hexdigest() == digest(release_backend)
    print('PASS identical pre-existing bundle backend accepted and explicitly packaged from release output')

    rejection_index = 0

    def reject_assembly(expected_error):
        global rejection_index
        rejection_index += 1
        output = base / ('rejected-artifacts-' + str(rejection_index))
        result = command(['bash', str(packager), '--output', str(output), '--build-info', str(metadata)], False, cwd=fixture)
        assert expected_error in result.stderr, result.stderr
        assert not output.exists()

    with (paired / 'smith').open('ab') as stream:
        stream.write(b'changed backend')
    reject_assembly('existing bundled backend differs')
    shutil.copy2(release_backend, paired / 'smith')
    (paired / 'unknown').write_text('unknown extra')
    reject_assembly('unknown extras refused')
    (paired / 'unknown').unlink()
    (paired / 'smith').unlink()
    reject_assembly('unknown extras refused')
    (paired / 'smith').symlink_to(release_backend)
    reject_assembly('symlink, special file or unsafe permissions')
    (paired / 'smith').unlink()
    paired.rmdir()
    paired.write_text('not a backend directory')
    reject_assembly('unknown extras refused')
    paired.unlink()

    original_backend = release_backend.read_bytes()
    release_backend.write_text('#!/bin/sh\nexit 99\n')
    reject_assembly('expected native Linux x86_64 ELF')
    foreign = bytearray(original_backend)
    foreign[18:20] = b'\xb7\x00'  # AArch64 is not a supported target.
    release_backend.write_bytes(foreign)
    reject_assembly('expected native Linux x86_64 ELF')
    release_backend.write_bytes(original_backend)
    release_backend.chmod(0o644)
    reject_assembly('payload executable is not executable')
    release_backend.chmod(0o755)
    paired.mkdir(mode=0o755)
    shutil.copy2(release_backend, paired / 'smith')
    (paired / 'smith').chmod(0o644)
    reject_assembly('payload executable is not executable')
    (paired / 'smith').chmod(0o755)
    (paired / 'smith').write_bytes(foreign)
    reject_assembly('expected native Linux x86_64 ELF')
    (paired / 'smith').unlink()
    os.link(release_backend, paired / 'smith')
    reject_assembly('missing/unsafe assembly input')
    (paired / 'smith').unlink()
    paired.rmdir()
    launcher = bundle / 'smith_halo'
    original_launcher = launcher.read_bytes()
    launcher.write_text('#!/bin/sh\nexit 99\n')
    reject_assembly('expected native Linux x86_64 ELF')
    launcher.write_bytes(original_launcher)
    print('PASS changed/extra/missing/symlink/hardlink/non-directory paired backend and non-native/non-executable backend/launcher rejection; no candidate publication')
    bundle_link = fixture / 'client/build/linux/x64/release/bundle/escape'
    bundle_link.symlink_to(home)
    command(['bash', str(packager), '--output', str(base / 'symlink-artifacts'), '--build-info', str(metadata)], False, cwd=fixture)
    assert not (base / 'symlink-artifacts').exists()
    bundle_link.unlink()
    license_path = fixture / 'LICENSE'
    license_path.unlink()
    license_path.symlink_to(repo / 'LICENSE')
    changed_identity = json.loads(command(['bash', str(packager), '--source-identity'], cwd=fixture).stdout)
    metadata.write_text(metadata.read_text().replace(identity['sha256'], changed_identity['sha256']))
    command(['bash', str(packager), '--output', str(base / 'source-link-artifacts'), '--build-info', str(metadata)], False, cwd=fixture)
    assert not (base / 'source-link-artifacts').exists()
    print('PASS mock assembly/primary paired Halo plus secondary backend archives/full byte-equal round-trip/native executable types/native dependencies/provenance/checksums/source binding/no overwrite/unsafe input refusal')
    assert not calls.exists(), 'a forbidden tool or packaged executable was invoked'
    print('PASS no application/compiler/network/dependency/live-manager calls; all fixtures temporary')

print('Isolated mock smoke passed. NOT evidence of real offline inference or clean-host desktop compatibility.')
PY
