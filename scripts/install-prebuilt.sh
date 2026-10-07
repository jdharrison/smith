#!/usr/bin/env bash
set -euo pipefail
# Python is an existing Ubuntu host prerequisite, not a build toolchain.
exec python3 - "$@" <<'PY'
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import sys
import tempfile


def fail(message):
    raise ValueError(message)


def safe_path(value, owned=False):
    path = Path(value)
    if not path.is_absolute() or str(path) != value or '..' in path.parts:
        fail('path must be absolute and normalized')
    for part in [*reversed(path.parents), path]:
        info = part.lstat()
        if not stat.S_ISDIR(info.st_mode):
            fail('path component is not a real directory (symlinks refused)')
        sticky_temp = info.st_uid == 0 and bool(info.st_mode & stat.S_ISVTX)
        if info.st_uid not in (0, os.geteuid()) or (info.st_mode & 0o022 and not sticky_temp):
            fail('unsafe path ancestor ownership or permissions')
    info = path.lstat()
    if owned and (info.st_uid != os.geteuid() or info.st_mode & 0o022):
        fail('root must be owned by invoking user and not group/world writable')
    return path


def regular(path):
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or info.st_mode & 0o7022:
        fail('unsafe package/destination file: symlink, hardlink, special or writable file')
    return info


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def verify(package):
    files = set()
    for directory, dirs, names in os.walk(package, followlinks=False):
        for name in dirs:
            info = (Path(directory) / name).lstat()
            if not stat.S_ISDIR(info.st_mode) or info.st_mode & 0o7022:
                fail('unsafe package directory')
        for name in names:
            path = Path(directory) / name
            regular(path)
            files.add(path.relative_to(package).as_posix())
    expected = {}
    for line in (package / 'SHA256SUMS').read_text().splitlines():
        match = re.fullmatch(r'([0-9a-f]{64})  ([A-Za-z0-9_./+\-]+)', line)
        if not match:
            fail('malformed checksum entry')
        checksum, name = match.groups()
        if name.startswith('/') or any(p in ('', '.', '..') for p in name.split('/')):
            fail('unsafe checksum path')
        if name in expected or name == 'SHA256SUMS':
            fail('duplicate/self checksum entry')
        expected[name] = checksum
    if set(expected) != files - {'SHA256SUMS'}:
        fail('checksum inventory does not exactly match package')
    for name, checksum in expected.items():
        if digest(package / name) != checksum:
            fail('checksum mismatch: ' + name)
    manifest = json.loads((package / 'manifest.json').read_text())
    if not isinstance(manifest, dict):
        fail('manifest must be an object')
    if manifest.get('format') != 'smith-chat-candidate-v1' or manifest.get('target') != 'ubuntu-24.04-x86_64':
        fail('unsupported package format/target')
    if manifest.get('kind') not in ('backend', 'halo'):
        fail('unsupported package kind')
    if not isinstance(manifest.get('version'), str) or not re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+(?:\+[0-9]+)?', manifest['version']):
        fail('invalid package version')
    return manifest


def main():
    parser = argparse.ArgumentParser(description='Network-free, install-only prebuilt candidate installer. Never starts applications/services. Existing destinations are refused.')
    parser.add_argument('--package', required=True, help='absolute extracted package directory')
    parser.add_argument('--root', required=True, help='absolute existing user-owned safe directory; create it explicitly first')
    args = parser.parse_args()
    os.umask(0o077)
    if os.geteuid() == 0:
        fail('refusing effective root')
    release = dict(line.split('=', 1) for line in Path('/etc/os-release').read_text().splitlines() if '=' in line)
    if release.get('ID', '').strip('"') != 'ubuntu' or release.get('VERSION_ID', '').strip('"') != '24.04' or os.uname().machine != 'x86_64':
        fail('only Ubuntu 24.04 x86_64 is supported')
    package = safe_path(args.package, owned=True)
    root = safe_path(args.root, owned=True)
    if root == package or package in root.parents or root in package.parents:
        fail('package and installation roots must be separate')
    manifest = verify(package)
    kind = manifest['kind']
    payload = package / 'payload'
    executable = payload / ('smith' if kind == 'backend' else 'smith_halo')
    if not regular(executable).st_mode & 0o111:
        fail('payload executable has no execute permission')
    if kind == 'backend' and set(payload.iterdir()) != {executable}:
        fail('backend payload must contain only smith')
    if kind == 'halo' and (not (payload / 'lib').is_dir() or not (payload / 'data').is_dir()):
        fail('Halo payload must include lib/ and data/')
    marker = root / '.smith-install-root'
    if kind == 'backend':
        bindir = root / 'bin'
        if bindir.exists() or bindir.is_symlink():
            safe_path(str(bindir), owned=True)
        destination = bindir / 'smith'
        if destination.exists() or destination.is_symlink():
            fail('existing backend refused; use a fresh root for upgrade/rollback')
        if marker.exists() or marker.is_symlink():
            info = regular(marker)
            if info.st_uid != os.geteuid() or stat.S_IMODE(info.st_mode) != 0o600 or marker.read_text() != 'smith-install-root-v1\n':
                fail('unsafe/incompatible install marker')
    else:
        destination = root / 'halo'
        if destination.exists() or destination.is_symlink():
            fail('existing Halo refused; use a fresh root for upgrade/rollback')
    # Stage and verify copied bytes before touching final destinations. No binary probes.
    with tempfile.TemporaryDirectory(prefix='.smith-prebuilt-', dir=root) as temporary:
        stage = Path(temporary)
        shutil.copytree(payload, stage / 'payload', copy_function=shutil.copy2)
        for path in (stage / 'payload').rglob('*'):
            if path.is_file() and digest(path) != digest(payload / path.relative_to(stage / 'payload')):
                fail('payload changed during copy')
        verify(package)
        if kind == 'backend':
            staged_binary = stage / 'payload' / 'smith'
            staged_binary.chmod(0o755)
            # O_EXCL-style hardlink publication refuses a concurrently created destination.
            bindir.mkdir(mode=0o755, exist_ok=True)
            safe_path(str(bindir), owned=True)
            staged_marker = stage / 'marker'
            staged_marker.write_text('smith-install-root-v1\n')
            staged_marker.chmod(0o600)
            os.link(staged_binary, destination, follow_symlinks=False)
            try:
                if not marker.exists():
                    os.link(staged_marker, marker, follow_symlinks=False)
            except Exception:
                destination.unlink()
                raise
        else:
            # A dedicated new directory avoids replacing any existing bundle or data.
            destination.mkdir(mode=0o755)
            try:
                for child in (stage / 'payload').iterdir():
                    child.rename(destination / child.name)
            except Exception:
                shutil.rmtree(destination)
                raise
    launcher = destination if kind == 'backend' else destination / 'smith_halo'
    print('Installed local candidate (NOT a production release): ' + str(launcher))
    print('No application, service, Docker, dependency, desktop, login or lingering effect.')
    if kind == 'backend':
        print('Do NOT use legacy service registration for chat-only. See CHAT_RELEASE.md.')
    else:
        print('Launch this absolute executable explicitly; no desktop registration was written.')


try:
    main()
except (OSError, ValueError, KeyError) as error:
    print('install-prebuilt: ' + str(error), file=sys.stderr)
    sys.exit(1)
PY
