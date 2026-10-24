import difflib
import hashlib
import json
from pathlib import Path
import re
import shutil
import tarfile

root = Path(__file__).resolve().parents[3]
out = Path(__file__).resolve().parent
docs = root / 'docs/evidence/default-project/integration'
sha = lambda data: hashlib.sha256(data).hexdigest()
base = json.loads((root / 'target/default-project/baseline-manifest.json').read_text())
compiled = json.loads((out / '12-compiled-projects.json').read_text())
binary = root / compiled['fixture_binary']
assert sha(binary.read_bytes()) == compiled['fixture_sha256']
with (out / 'debug_service-fixture').open('xb') as f:
    f.write(binary.read_bytes())
(out / 'debug_service-fixture').chmod(0o755)

snapshots = {}
for name in ['build-source', 'build-source-2']:
    folder = out / name / 'service'
    files = [p for p in folder.rglob('*') if p.is_file() and 'target' not in p.relative_to(folder).parts]
    snapshots[name] = {str(p.relative_to(folder)): sha(p.read_bytes()) for p in sorted(files)}
    with (out / (name + '-manifest.json')).open('x') as f:
        json.dump(snapshots[name], f, indent=2)

with tarfile.open(root / 'target/default-project/baseline-source.tar.gz') as archive:
    paths = ['client/test/rust_projects_integration_test.dart', 'service/tests/debug_service.rs']
    old = {p: archive.extractfile(p).read().decode() for p in paths}
    current = {p: (root / p).read_text() for p in paths}
    patch = ''.join(''.join(difflib.unified_diff(old[p].splitlines(True), current[p].splitlines(True), fromfile='baseline/' + p, tofile='current/' + p)) for p in paths)
    with (out / 'final-fixture-from-baseline.patch').open('x') as f:
        f.write(patch)
    pattern = r"  test\(\n    '([^']+)'"
    old_tests = re.findall(pattern, old[paths[0]])
    new_tests = re.findall(pattern, current[paths[0]])
    assert len(old_tests) == 7 and len(new_tests) == 8
    assert new_tests[1:] == old_tests

service_drift = []
for p, digest in snapshots['build-source-2'].items():
    live = root / 'service' / p
    if not live.exists() or sha(live.read_bytes()) != digest:
        service_drift.append(p)

with tarfile.open(out / 'successful-service-source.tar.gz', 'x:gz') as archive:
    for p in snapshots['build-source-2']:
        archive.add(out / 'build-source-2/service' / p, arcname='service/' + p, recursive=False)
with tarfile.open(out / 'successful-dart-source.tar.gz', 'x:gz') as archive:
    for p in compiled['source_before']:
        if p.startswith('client/') and p.endswith(('.dart', '.yaml', '.lock')):
            assert sha((root / p).read_bytes()) == compiled['source_before'][p], p
            archive.add(root / p, arcname=p, recursive=False)

report = {
    'baseline_embedded_sha256': base['sha256'],
    'baseline_json_file_sha256': sha((root / 'target/default-project/baseline-manifest.json').read_bytes()),
    'baseline_count': base['count'],
    'old_pre_seed_binary_sha256_not_used': '7d010513ac31621c1d928352f70b986f92d3a777ecf75470bc9a3946dcf4ab92',
    'successful_binary_sha256': compiled['fixture_sha256'],
    'successful_compile_record': '11-compile-offline.json',
    'successful_compile_target_dir': 'target/default-project/integration/build-source/service/target',
    'failed_compile_source': 'build-source-manifest.json',
    'successful_compile_source': 'build-source-2-manifest.json',
    'compiled_source_stable_during_run': compiled['source_before'] == compiled['source_after'],
    'service_drift_after_successful_snapshot': service_drift,
    'seven_original_test_names_preserved_in_order': old_tests,
    'added_test': new_tests[0],
    'fixture_files': {p: sha((root / p).read_bytes()) for p in paths},
    'scope': 'Compiled fixture slice only; not full root gates, independent review, installed acceptance, or approval.',
}
with (out / 'capture-summary.json').open('x') as f:
    json.dump(report, f, indent=2)

# Keep raw original logs locally. Publish new redacted copies; do not overwrite
# any original evidence or include local absolute workspace/toolchain paths.
for p in sorted(out.iterdir()):
    if p.is_file() and p.suffix in {'.json', '.log', '.patch', '.py'}:
        text = p.read_text().replace(str(root), '<workspace>')
        text = text.replace(str(Path.home()), '<home>')
        with (docs / p.name).open('x') as f:
            f.write(text)
for folder in [out, docs]:
    files = [p for p in folder.iterdir() if p.is_file()]
    with (folder / 'SHA256SUMS').open('x') as f:
        for p in sorted(files):
            f.write(sha(p.read_bytes()) + '  ' + p.name + '\n')
print(json.dumps(report, indent=2))
