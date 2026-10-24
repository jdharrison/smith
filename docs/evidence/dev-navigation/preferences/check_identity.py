import datetime
import hashlib
import json
from pathlib import Path
import tarfile
import sys

root = Path(__file__).resolve().parents[3]
owned = ['client/lib/workspace_preferences.dart', 'client/lib/main.dart', 'client/test/workspace_preferences_test.dart', 'client/test/workspace_restore_test.dart']
before = json.loads((root / 'target/dev-navigation/preferences/before-hashes.json').read_text())
manifest = json.loads((root / 'docs/evidence/dev-navigation/baseline-manifest.json').read_text())
def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None
with tarfile.open(root / 'target/dev-navigation/preferences/before-owned.tar.gz') as archive:
    main_before = archive.extractfile('client/lib/main.dart').read()
old = b"    if (!const {'chat', 'projects', 'agents', 'settings'}.contains(page)) {"
new = b"    if (!const {\n      'chat',\n      'dev',\n      'projects',\n      'agents',\n      'settings',\n    }.contains(page)) {"
assert main_before.count(old) == 1
main_only = (root / 'client/lib/main.dart').read_bytes() == main_before.replace(old, new)
lib_drift = [path for path, sha in before.items() if path not in owned and digest(root / path) != sha]
core = [entry for entry in manifest['files'] if entry['path'].startswith(('service/', 'installer/', 'dagger/'))]
core_drift = [entry['path'] for entry in core if digest(root / entry['path']) != entry['sha256']]
record = {
    'recorded_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'baseline_canonical_sha256': manifest['sha256'],
    'baseline_file_count': manifest['count'],
    'baseline_archive_sha256': digest(root / 'target/dev-navigation/baseline-source.tar.gz'),
    'owned_hashes': {path: digest(root / path) for path in owned},
    'main_only_formatted_setPage_allowlist_changed': main_only,
    'client_lib_external_drift_from_capture': lib_drift,
    'core_baseline_files_checked': len(core),
    'core_baseline_drift': core_drift,
    'preference_io_unchanged_from_capture': digest(root / 'client/lib/workspace_preferences_io.dart') == before['client/lib/workspace_preferences_io.dart'],
}
name = sys.argv[1]
(root / 'docs/evidence/dev-navigation/preferences' / (name + '.json')).write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(record, indent=2))
assert main_only
assert not core_drift
assert record['preference_io_unchanged_from_capture']
