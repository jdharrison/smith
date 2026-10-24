"""One approved, offline worker-image update; no job or service execution."""
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess

ROOT = Path(__file__).resolve().parents[3]
SCRATCH = ROOT / '.smith-dev-worker-buffered-update'
EVIDENCE = Path(__file__).resolve().parent
EXPECTED = '2500f2f114342895d4a0e8c7da3247b2ccde7c47685716dd3c32450fed32e708'
OLD_PAYLOAD = EXPECTED
NEW_PAYLOAD = 'af6f560867cbfa17b9d374a47dde95bb1f415d6c9f65de669b27e85aafd993c7'
OLD_IMAGE = 'sha256:3bb245491a8879160b2e61af480afc292ab03ee951c34278df3a139677b51d49'
TAG = 'smith-dev-worker-local:1.19.3'
BACKUP_TAG = 'smith-dev-worker-local:before-buffered-20261019'
CANDIDATE = 'smith-dev-worker-local:buffered-verified-20261019'
INPUTS = {
    'service/dev-worker/Dockerfile.local': '0009f1538f11bfd2145796ab1b2a6b893016ac7ee4614fb8dd90ba657f8f47df',
    'service/dev-worker/guard.mjs': '40504069bc2548e7d1da3c9a47cecda3ee1be77e2be3a60bbdaac737e25ef310',
    'service/.dockerignore': '6cab522c6a0c3c730943c2628d18b84e4efbe83e91138bf151371c7942101000',
}
ENV = {'PATH': '/usr/bin:/bin', 'HOME': '/nonexistent', 'LANG': 'C', 'DOCKER_BUILDKIT': '0'}
DOCKER = ['/usr/bin/docker', '--config', str(SCRATCH / 'docker-config'), '--host', 'unix:///run/docker.sock']


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(args, **kwargs):
    return subprocess.run(args, env=None if args[0] == 'systemctl' else ENV,
                          check=True, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE, timeout=20, **kwargs).stdout


def image(name):
    return json.loads(run(DOCKER + ['image', 'inspect', name]))[0]


def service():
    values = run(['systemctl', '--user', 'show', 'smith-next.service', '--property=MainPID', '--property=ActiveState'])
    props = dict(line.split('=', 1) for line in values.decode().splitlines())
    assert props['ActiveState'] == 'active'
    pid = int(props['MainPID'])
    assert pid > 0
    proc = Path('/proc') / str(pid)
    argv = [os.fsdecode(x) for x in (proc / 'cmdline').read_bytes().split(b'\0') if x]
    roots = dict(zip(argv[2::2], map(Path, argv[3::2])))
    namespace = roots['--data-root'] / 'smith-next'
    assert digest(proc / 'exe') == EXPECTED
    assert digest(namespace / 'client/current/smith_backend/smith-service') == EXPECTED
    return pid, proc, namespace


def recheck_inputs():
    manifest = EVIDENCE / '08-source-identity.log'
    assert digest(manifest) == 'ed2b1abcc7b609b36e8e4ee126a20a6d83b9e6d2aead082beb9cf6257bf1f6ba'
    for name, sha in {**json.loads(manifest.read_text())['sha256'], **INPUTS}.items():
        assert digest(ROOT / name) == sha, 'Build source drift'
    assert digest(ROOT / 'service/target/debug/smith-service') == NEW_PAYLOAD


def main():
    assert SCRATCH.is_dir() and not SCRATCH.is_symlink()
    os.chmod(SCRATCH, 0o700)
    config = SCRATCH / 'docker-config'
    config.mkdir(mode=0o700)
    pid, proc, namespace = service()
    recheck_inputs()
    old = image(TAG)
    assert old['Id'] == OLD_IMAGE
    assert old['Config']['User'] == '1000:1000'
    assert old['Config']['Entrypoint'] == ['/usr/local/bin/smith-service', 'dev-local-worker']
    assert not old['Config'].get('Volumes')
    assert not old['Config'].get('OnBuild') and not old['Config'].get('Shell')
    for tag in [BACKUP_TAG, CANDIDATE]:
        result = subprocess.run(DOCKER + ['image', 'inspect', tag], env=ENV, capture_output=True, timeout=10)
        assert result.returncode != 0, 'Reserved tag already exists; stop for inspection'

    ledger_path = namespace / 'dev-local-jobs.json'
    ledger_sha = digest(ledger_path)
    ledger = json.loads(ledger_path.read_text())
    assert all(r['summary']['status'] not in ['preparing', 'cloning', 'ready', 'running'] for r in ledger['records'])
    payload = ROOT / 'service/dist/smith-service'
    assert payload.parent.is_dir() and not payload.parent.is_symlink()
    assert stat.S_ISREG(payload.lstat().st_mode) and digest(payload) == OLD_PAYLOAD
    backup = SCRATCH / 'worker-payload.before'
    with backup.open('xb') as output:
        output.write(payload.read_bytes())
        os.fchmod(output.fileno(), 0o600)
        output.flush()
        os.fsync(output.fileno())
    assert digest(backup) == OLD_PAYLOAD
    # Preserve the immutable old image before moving its normal tag later.
    run(DOCKER + ['image', 'tag', OLD_IMAGE, BACKUP_TAG])
    assert image(BACKUP_TAG)['Id'] == OLD_IMAGE
    binary = (ROOT / 'service/target/debug/smith-service').read_bytes()
    assert hashlib.sha256(binary).hexdigest() == NEW_PAYLOAD and b'--depth=1' not in binary
    staged = payload.with_name('smith-service.buffered-staging')
    with staged.open('xb') as output:
        output.write(binary)
        os.fchmod(output.fileno(), 0o555)
        output.flush()
        os.fsync(output.fileno())
    assert service()[0] == pid
    recheck_inputs()
    assert digest(payload) == OLD_PAYLOAD
    os.replace(staged, payload)
    guard = INPUTS['service/dev-worker/guard.mjs']
    # Reuse the verified local toolchain image, not a cross-builder layer cache.
    recipe = (
        'FROM ' + BACKUP_TAG + '\n'
        'COPY --chown=0:0 dist/smith-service /usr/local/bin/smith-service\n'
        'RUN printf "' + NEW_PAYLOAD + '  /usr/local/bin/smith-service\\n'
        + guard + '  /etc/smith-opencode/guard.mjs\\n" | sha256sum --check --strict \\\n'
        '    && stat -c "%u:%g:%a" /usr/local/bin/smith-service | grep -Fx "0:0:555" \\\n'
        '    && stat -c "%u:%g:%a" /usr/local/bin/opencode | grep -Fx "0:0:555" \\\n'
        '    && /usr/local/bin/smith-service --help >/dev/null\n'
    )
    (EVIDENCE / 'Dockerfile.verified').write_text(recipe)
    assert service()[0] == pid and digest(payload) == NEW_PAYLOAD
    assert image(TAG)['Id'] == OLD_IMAGE
    recheck_inputs()
    with (EVIDENCE / 'worker-image-build.log').open('xb') as log:
        result = subprocess.run(DOCKER + ['build', '--pull=false', '--network=none', '--rm=false',
                                         '-f', '-', '-t', CANDIDATE, str(ROOT / 'service')],
                                input=recipe.encode(), env=ENV, stdout=log, stderr=subprocess.STDOUT,
                                timeout=240)
    assert result.returncode == 0, 'Offline image build failed; normal image tag unchanged'
    candidate = image(CANDIDATE)
    for field in ['User', 'Entrypoint', 'Cmd', 'Env', 'WorkingDir', 'Volumes', 'OnBuild', 'Shell']:
        assert candidate['Config'].get(field) == old['Config'].get(field), 'Image configuration drift'
    assert candidate['Id'] != OLD_IMAGE
    assert service()[0] == pid and digest(payload) == NEW_PAYLOAD
    assert digest(ledger_path) == ledger_sha, 'Dev state changed during preparation; stop before tag publication'
    recheck_inputs()
    assert image(TAG)['Id'] == OLD_IMAGE and image(BACKUP_TAG)['Id'] == OLD_IMAGE
    run(DOCKER + ['image', 'tag', candidate['Id'], TAG])
    assert image(TAG)['Id'] == candidate['Id']
    assert service()[0] == pid
    assert digest(ledger_path) == ledger_sha
    result = {
        'status': 'worker-image-updated', 'service_sha256': EXPECTED,
        'staged_payload_sha256': digest(payload), 'old_payload_sha256': digest(backup),
        'worker_artifact_sha256': NEW_PAYLOAD, 'dev_ledger_unchanged': True,
        'worker_only_qa': True, 'live_performance_verified': False,
        'known_broader_gate': 'local timestamp fixture fails before framing; not waived for overall acceptance',
        'old_image': OLD_IMAGE, 'preserved_image_tag': BACKUP_TAG,
        'new_image': candidate['Id'], 'runtime_tag': TAG, 'candidate_tag': CANDIDATE,
        'build_network': 'none', 'pull': False, 'image_build_hash_and_ownership_checks': True,
        'image_user': candidate['Config']['User'], 'entrypoint': candidate['Config']['Entrypoint'],
        'service_pid_unchanged': True, 'service_restarted': False, 'job_submitted': False,
        'inference_run': False, 'historical_record_mutated': False,
        'recipe_sha256': hashlib.sha256(recipe.encode()).hexdigest(),
        'build_log_sha256': digest(EVIDENCE / 'worker-image-build.log'),
    }
    with (EVIDENCE / 'worker-image-update.json').open('x') as output:
        json.dump(result, output, indent=2)
        output.write('\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
