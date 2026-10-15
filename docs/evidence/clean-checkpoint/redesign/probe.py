import datetime
import hashlib
import json
from pathlib import Path
import runpy

root = Path.cwd()
logs = root / 'installer/build/redesign-final-validation'
graphql = runpy.run_path('installer/scripts/capture-ui.py')['graphql']
q = json.dumps
started = datetime.datetime.now(datetime.timezone.utc).isoformat()
files = {}
for name in ('smith-installer', 'smith-installer-ui'):
    data = graphql('{ host { file(path: ' + q(str(root / 'installer/dist' / name)) + ') { id } } }')
    files[name] = data['host']['file']['id']
results = []
for name, flags in [('smith-installer', ['--help', '--version']), ('smith-installer-ui', ['--help', '--version', '--licenses'])]:
    chain = '{ container { from(address: "ubuntu:24.04") {'
    depth = 3
    if name.endswith('-ui'):
        for args in [['apt-get', 'update'], ['apt-get', 'install', '--yes', '--no-install-recommends', 'libgtk-3-0t64']]:
            chain += 'withExec(args: ' + q(args) + ') {'
            depth += 1
    chain += 'withFile(path: "/fixture/' + name + '", source: ' + q(files[name]) + ', permissions: 493) {'
    depth += 1
    setup = ['sh', '-c', 'set -e; mkdir -p [REDACTED_HOME_PATH] chown 1000:1000 [REDACTED_HOME_PATH] test ! -e /usr/bin/python3; test ! -e /usr/local/bin/python3']
    if not name.endswith('-ui'):
        setup[2] += '; test ! -e /usr/lib/x86_64-linux-gnu/libgtk-3.so.0'
    chain += 'withExec(args: ' + q(setup) + ') { withUser(name: "1000:1000") {'
    depth += 2
    for flag in flags:
        args = ['/usr/bin/env', '-i', 'HOME=[REDACTED_HOME_PATH]', 'XDG_DATA_HOME=[REDACTED_HOME_PATH]', 'XDG_CONFIG_HOME=[REDACTED_HOME_PATH]', 'XDG_RUNTIME_DIR=[REDACTED_HOME_PATH]', 'PATH=/unavailable', 'DISPLAY=', 'WAYLAND_DISPLAY=', '/fixture/' + name, flag]
        result = graphql(chain + 'withExec(args: ' + q(args) + ', expect: ANY) { stdout stderr exitCode }' + '}' * depth)
        while 'exitCode' not in result:
            result = next(iter(result.values()))
        (logs / (name + flag + '.stdout')).write_text(result['stdout'])
        (logs / (name + flag + '.stderr')).write_text(result['stderr'])
        results.append({'binary': 'next/installer/dist/' + name, 'sha256': hashlib.sha256((root / 'installer/dist' / name).read_bytes()).hexdigest(), 'argv': args, **result})
        assert result['exitCode'] == 0, result
        if flag == '--licenses':
            for notice in (root / 'installer/assets').rglob('*'):
                if notice.is_file() and (notice.name == 'OFL.txt' or 'NOTICE' in notice.name):
                    assert notice.read_text().strip() in result['stdout'], str(notice)

record = {'started_utc': started, 'finished_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'environment': 'Isolated Dagger Ubuntu 24.04 UID/GID 1000; empty environment except private HOME/XDG, unusable PATH, empty displays; no package payload copied. GUI GTK shared libraries installed in container only; CLI separate base without GTK or Python.', 'results': results, 'status': 'PASS'}
(logs / 'release-elf-probes.json').write_text(json.dumps(record, indent=2) + '\n')
print('PASS: five actual exported ELF documentation invocations, including embedded notices; CLI image lacks GTK/Python.')
