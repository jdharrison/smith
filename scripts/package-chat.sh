#!/usr/bin/env bash
set -euo pipefail
exec python3 - "$@" <<'PY'
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile


def fail(message):
    raise ValueError(message)


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def run(*command):
    return subprocess.check_output(command, stderr=subprocess.PIPE, timeout=30).decode().strip()


def source_identity(repo):
    names = subprocess.check_output(['git', '--no-optional-locks', 'ls-files', '-z', '--cached', '--others', '--exclude-standard'], cwd=repo, timeout=30).split(b'\0')
    records = []
    for raw in sorted(set(names) - {b''}):
        name = os.fsdecode(raw)
        if name.startswith(('target/', 'client/build/', 'client/.dart_tool/')):
            continue
        path = repo / name
        if path.is_symlink():
            checksum = hashlib.sha256(os.readlink(path).encode()).hexdigest()
            kind = 'symlink'
        elif path.is_file():
            checksum, kind = sha(path), 'file'
        else:
            checksum, kind = None, 'missing'
        mode = stat.S_IMODE(path.lstat().st_mode) if path.exists() or path.is_symlink() else None
        records.append({'path': name, 'kind': kind, 'sha256': checksum, 'mode': mode})
    encoded = json.dumps(records, sort_keys=True, separators=(',', ':')).encode()
    return {'sha256': hashlib.sha256(encoded).hexdigest(), 'files': records}


def safe_tree(root):
    if root.is_symlink() or not root.is_dir():
        fail('payload must be a real directory')
    for path in [root, *root.rglob('*')]:
        info = path.lstat()
        if not (stat.S_ISREG(info.st_mode) or stat.S_ISDIR(info.st_mode)) or info.st_mode & 0o7022:
            fail('payload contains symlink, special file or unsafe permissions')
        if stat.S_ISREG(info.st_mode) and info.st_nlink != 1:
            fail('payload contains hardlinks')
        name = path.relative_to(root).as_posix()
        if name != '.' and not re.fullmatch(r'[A-Za-z0-9_./+\-]+', name):
            fail('payload filename is not portable/checksum-safe')


def inventory(root):
    safe_tree(root)
    return {path.relative_to(root).as_posix(): {
        'mode': stat.S_IMODE(path.stat().st_mode),
        'sha256': sha(path) if path.is_file() else None,
    } for path in [root, *root.rglob('*')]}


def real_input(path):
    if path.absolute() != path.resolve():
        fail('assembly input has a symlink component')
    if not path.is_file() or path.stat().st_nlink != 1 or path.stat().st_mode & 0o7022:
        fail('missing/unsafe assembly input')


def native_elf(path):
    with path.open('rb') as stream:
        header = stream.read(20)
    if len(header) != 20 or header[:6] != b'\x7fELF\x02\x01' or header[18:20] != b'\x3e\x00':
        fail('expected native Linux x86_64 ELF executable')
    if not path.stat().st_mode & 0o111:
        fail('payload executable is not executable')


def dependencies(payload):
    needed = {}
    for path in sorted(payload.rglob('*')):
        if not path.is_file():
            continue
        with path.open('rb') as stream:
            magic = stream.read(4)
        if magic == b'\x7fELF':
            output = run('readelf', '--wide', '--dynamic', str(path))
            needed[path.relative_to(payload).as_posix()] = re.findall(r'\(NEEDED\).*?\[([^\]]+)\]', output)
    return needed


def main():
    parser = argparse.ArgumentParser(description='Assemble existing built artifacts only; never builds, launches, downloads or publishes.')
    parser.add_argument('--output', help='new absolute output directory (must not exist)')
    parser.add_argument('--build-info', help='compiler metadata JSON supplied by build coordinator; see docs/CHAT_RELEASE.md')
    parser.add_argument('--source-identity', action='store_true', help='print current source identity only; no output files')
    args = parser.parse_args()
    os.umask(0o022)
    repo = Path.cwd()
    if not (repo / 'Cargo.toml').is_file() or not (repo / 'client/pubspec.yaml').is_file():
        fail('run from the Smith repository root')
    before = source_identity(repo)
    if args.source_identity:
        print(json.dumps(before, indent=2, sort_keys=True))
        return
    if not args.output or not args.build_info:
        fail('--output and --build-info are required')
    output = Path(args.output)
    if not output.is_absolute() or str(output) != args.output or '..' in output.parts:
        fail('output must be absolute and normalized')
    if output.exists() or output.is_symlink():
        fail('output already exists; candidates are never overwritten')
    if not output.parent.is_dir() or output.parent.resolve() != output.parent:
        fail('output parent must already exist and have no symlink components')
    release = dict(line.split('=', 1) for line in Path('/etc/os-release').read_text().splitlines() if '=' in line)
    if release.get('ID', '').strip('"') != 'ubuntu' or release.get('VERSION_ID', '').strip('"') != '24.04' or os.uname().machine != 'x86_64':
        fail('assemble only on Ubuntu 24.04 x86_64')
    for name in ('LICENSE', 'docs/CHAT_RELEASE.md', 'scripts/install-prebuilt.sh', 'Cargo.toml', 'Cargo.lock', 'client/pubspec.yaml', 'client/pubspec.lock'):
        real_input(repo / name)
    real_input(Path(args.build_info))
    info = json.loads(Path(args.build_info).read_text())
    required = {'rustc', 'cargo', 'flutter', 'dart', 'source_sha256'}
    if not isinstance(info, dict) or set(info) != required or any(not isinstance(value, str) or not value.strip() or any(c in value for c in ('\n', '\r', '\0')) for value in info.values()):
        fail('build-info needs only nonempty single-line rustc/cargo/flutter/dart/source_sha256 strings')
    if info['source_sha256'] != before['sha256']:
        fail('build coordinator source identity does not match current source; reconcile before assembly')
    backend_version = re.search(r'^version = "([^"]+)"', (repo / 'Cargo.toml').read_text(), re.M).group(1)
    halo_version = re.search(r'^version: (\S+)', (repo / 'client/pubspec.yaml').read_text(), re.M).group(1)
    for version in (backend_version, halo_version):
        if not re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+(?:\+[0-9]+)?', version):
            fail('unsupported version syntax')
    backend = repo / 'target/release/smith'
    halo = repo / 'client/build/linux/x64/release/bundle'
    real_input(backend)
    if halo.absolute() != halo.resolve():
        fail('Halo bundle path has a symlink component')
    safe_tree(halo)
    if not (halo / 'lib').is_dir() or not (halo / 'data').is_dir():
        fail('Halo bundle must include lib/ and data/')
    native_elf(backend)
    native_elf(halo / 'smith_halo')
    backend_digest = sha(backend)
    backend_mode = stat.S_IMODE(backend.stat().st_mode)
    halo_inventory = inventory(halo)
    bundled_backend = halo / 'smith_backend'
    if bundled_backend.exists():
        if not bundled_backend.is_dir() or set(bundled_backend.iterdir()) != {bundled_backend / 'smith'}:
            fail('existing smith_backend must contain exactly smith; unknown extras refused')
        native_elf(bundled_backend / 'smith')
        if halo_inventory.get('smith_backend/smith', {}).get('sha256') != backend_digest:
            fail('existing bundled backend differs from target/release/smith; rebuild a consistent bundle')
    git_head = run('git', '--no-pager', 'rev-parse', 'HEAD')
    branch = run('git', '--no-pager', 'rev-parse', '--abbrev-ref', 'HEAD')
    dirty = bool(run('git', '--no-optional-locks', '--no-pager', 'status', '--porcelain'))
    suffix = 'candidate-' + git_head[:12] + '-' + before['sha256'][:12]
    # Private sibling staging keeps failed assembly from looking like a complete candidate.
    with tempfile.TemporaryDirectory(prefix='.chat-package-', dir=output.parent) as temporary:
        stage = Path(temporary)
        artifacts = stage / 'artifacts'
        artifacts.mkdir(mode=0o700)
        for kind, version in (('backend', backend_version), ('halo', halo_version)):
            name = 'smith-' + kind + '-' + version + '-ubuntu-24.04-x86_64-' + suffix
            package = stage / name
            package.mkdir(mode=0o755)
            payload = package / 'payload'
            if kind == 'backend':
                payload.mkdir(mode=0o755)
                shutil.copy2(backend, payload / 'smith')
            else:
                shutil.copytree(halo, payload)
                paired = payload / 'smith_backend'
                paired.mkdir(mode=0o755, exist_ok=True)
                # Only the already checked identical backend may be replaced in staging.
                # Always package the explicit release output, never an incidental bundle copy.
                if (paired / 'smith').exists():
                    (paired / 'smith').unlink()
                shutil.copy2(backend, paired / 'smith')
            safe_tree(payload)
            packaged_backend = payload / ('smith' if kind == 'backend' else 'smith_backend/smith')
            native_elf(packaged_backend)
            if sha(packaged_backend) != backend_digest or stat.S_IMODE(packaged_backend.stat().st_mode) != backend_mode:
                fail('backend changed during assembly')
            shutil.copyfile(repo / 'LICENSE', package / 'LICENSE')
            shutil.copyfile(repo / 'docs/CHAT_RELEASE.md', package / 'CHAT_RELEASE.md')
            shutil.copyfile(repo / 'scripts/install-prebuilt.sh', package / 'install-prebuilt.sh')
            (package / 'install-prebuilt.sh').chmod(0o755)
            manifest = {
                'format': 'smith-chat-candidate-v1', 'kind': kind, 'version': version,
                'backend_version': backend_version, 'halo_version': halo_version,
                'target': 'ubuntu-24.04-x86_64', 'status': 'LOCAL CANDIDATE — NOT A PRODUCTION RELEASE',
                'source_dirty': dirty, 'head': git_head, 'branch': branch,
                'source': before, 'declared_build_toolchains': info,
                'build_binding': 'Coordinator declaration only; assembly does not prove build inputs or checks passed.',
                'packaged_backend': {
                    'path': packaged_backend.relative_to(package).as_posix(),
                    'version': backend_version, 'sha256': backend_digest,
                    'source_input': 'target/release/smith', 'source_sha256': before['sha256'],
                    'consistency': 'Existing bundle backend accepted only with exact digest and no extra entries; copied explicitly from release output.',
                },
                'created_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
                'elf_needed': dependencies(payload),
                'runtime': {
                    'installer': ['bash', 'python3 >= 3.11 (Ubuntu 24.04 provides 3.12)'],
                    'backend': ['Ubuntu libc/runtime ELF libraries listed above', 'preprovisioned loopback Ollama', 'explicit preprovisioned model'],
                    'halo': ['Ubuntu graphical session', 'GTK3 and system/plugin ELF libraries listed above', 'bundled Smith backend at smith_backend/smith; compatible external chat-only bridge may attach'],
                    'desktop_backend_discovery': 'Resolve smith_backend/smith only relative to the actual smith_halo executable directory (installed ROOT/halo); not cwd or PATH. Packaging does not execute or verify the client resolver.',
                    'installation': 'Full payload copied install-only; no OS/runtime installation, model weights/downloads, application launch or service registration.',
                    'excluded': ['Rust/Cargo and Flutter/Dart build tools', 'Ollama/model weights', 'automatic dependency installation', 'OpenCode for chat-only'],
                },
                'validation': 'Assembly/checksums only; real-model offline, clean-host desktop, SBOM/license review and independent assurance remain gates.',
                'lockfiles': {name: sha(repo / name) for name in ('Cargo.lock', 'client/pubspec.lock')},
            }
            (package / 'manifest.json').write_text(json.dumps(manifest, indent=2, sort_keys=True) + '\n')
            checks = []
            for path in sorted(package.rglob('*')):
                if path.is_file():
                    checks.append(sha(path) + '  ' + path.relative_to(package).as_posix() + '\n')
            (package / 'SHA256SUMS').write_text(''.join(checks))
            def archive_metadata(member):
                member.uid = member.gid = 0
                member.uname = member.gname = ''
                return member

            with tarfile.open(artifacts / (name + '.tar.gz'), 'w:gz', format=tarfile.PAX_FORMAT) as archive:
                archive.add(package, arcname=name, filter=archive_metadata)
            shutil.copyfile(package / 'manifest.json', artifacts / (name + '.provenance.json'))
            if sha(backend) != backend_digest or stat.S_IMODE(backend.stat().st_mode) != backend_mode:
                fail('backend changed during assembly')
            if kind == 'halo':
                if inventory(halo) != halo_inventory:
                    fail('Halo bundle changed during assembly')
                expected = dict(halo_inventory)
                expected.setdefault('smith_backend', {'mode': 0o755, 'sha256': None})
                expected['smith_backend/smith'] = {'mode': backend_mode, 'sha256': backend_digest}
                if inventory(payload) != expected:
                    fail('paired payload differs from full Halo bundle plus release backend')
        if source_identity(repo) != before:
            fail('source changed during assembly; coordinate a stable snapshot')
        (artifacts / 'SHA256SUMS').write_text(''.join(sha(p) + '  ' + p.name + '\n' for p in sorted(artifacts.iterdir())))
        output.mkdir(mode=0o700)
        for path in artifacts.iterdir():
            path.rename(output / path.name)
    print('Local candidate artifacts assembled; NOT a production release. No builds or applications ran.')
    print('Retain the outer SHA256SUMS digest independently; see docs/CHAT_RELEASE.md.')


try:
    main()
except (OSError, ValueError, AttributeError, subprocess.SubprocessError) as error:
    # Avoid dumping commands/absolute build paths from tool stderr into distributable evidence.
    print('package-chat: ' + (str(error) if not isinstance(error, subprocess.SubprocessError) else 'required metadata tool failed'), file=sys.stderr)
    sys.exit(1)
PY
