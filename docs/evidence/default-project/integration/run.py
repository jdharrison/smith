import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[3]
out = Path(__file__).resolve().parent
label, cwd, *command = sys.argv[1:]
record = {'command': command, 'cwd': cwd, 'started_utc': datetime.datetime.now(datetime.timezone.utc).isoformat()}
inputs = []
for folder in ['service/src', 'service/tests', 'client/lib', 'client/test']:
    inputs.extend(p for p in (root / folder).rglob('*') if p.is_file())
for name in ['service/Cargo.toml', 'service/Cargo.lock', 'client/pubspec.yaml', 'client/pubspec.lock', 'client/.dart_tool/package_config.json']:
    p = root / name
    if p.exists():
        inputs.append(p)
def identity():
    return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(inputs)}
record['source_before'] = identity()
if 'SMITH_TEST_CLOUD_SERVICE' in os.environ:
    binary = Path(os.environ['SMITH_TEST_CLOUD_SERVICE'])
    record['fixture_binary'] = str(binary.relative_to(root))
    record['fixture_sha256'] = hashlib.sha256(binary.read_bytes()).hexdigest()
with (out / (label + '.log')).open('x') as log:
    try:
        result = subprocess.run(command, cwd=root / cwd, stdout=log, stderr=subprocess.STDOUT, timeout=240)
        record['exit_code'] = result.returncode
    except subprocess.TimeoutExpired:
        record['timeout_seconds'] = 240
record['source_after'] = identity()
record['finished_utc'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
with (out / (label + '.json')).open('x') as f:
    json.dump(record, f, indent=2)
print(json.dumps({k: v for k, v in record.items() if not k.startswith('source_')}, indent=2))
print((out / (label + '.log')).read_text()[-14000:])
sys.exit(record.get('exit_code', 124))
