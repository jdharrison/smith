#!/usr/bin/env python3
"""Preserve late concurrent source drift; no mutation of any reviewed input."""
import datetime
import difflib
import hashlib
import json
from pathlib import Path
import stat
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
P = 'docs/evidence/projects-layout/'


def canonical(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def load(p):
    return json.loads((ROOT / p).read_text())


def record(p, full=False):
    file = ROOT / p
    info = file.lstat()
    if not stat.S_ISREG(info.st_mode) or file.is_symlink():
        raise ValueError('not a regular file: ' + p)
    value = dict(path=p, size=info.st_size, mode=stat.S_IMODE(info.st_mode), sha256=hashlib.sha256(file.read_bytes()).hexdigest())
    if full:
        value.update(type='file', mode_octal=oct(value['mode']), tracked=p in tracked)
    return value


def save(name, value):
    with (OUT / name).open('x') as handle:
        handle.write(json.dumps(value, sort_keys=True, indent=2) + '\n')


tracked = set(filter(None, subprocess.check_output(['git', '--no-pager', '--no-optional-locks', 'ls-files', '--cached', '-z'], cwd=ROOT, timeout=20).decode().split('\0')))
visible = set(filter(None, subprocess.check_output(['git', '--no-pager', '--no-optional-locks', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'], cwd=ROOT, timeout=20).decode().split('\0')))
core = load(P + 'validation-rebound/core-manifest-before.json')
roots = set(core['source_roots'])
selected = sorted(p for p in visible if (p in roots or p.split('/')[0] in roots) and not p.startswith(tuple(core['excluded_prefixes'])) and '__pycache__' not in Path(p).parts and not p.endswith('.pyc') and p != 'client/.flutter-plugins-dependencies')
actual = [record(p, True) for p in selected]
actual_map = {e['path']: e for e in actual}
changes = [dict(path=e['path'], expected=e, actual=actual_map.get(e['path'])) for e in core['files'] if actual_map.get(e['path']) != e]
new_paths = sorted(set(selected) - {e['path'] for e in core['files']})
with tarfile.open(ROOT / 'target/projects-layout/baseline-source.tar.gz', 'r:gz') as tar:
    for e in changes:
        try:
            old = tar.extractfile(e['path']).read().decode().splitlines()
            current = (ROOT / e['path']).read_text().splitlines()
            e['changed_hunks_against_pre_layout_baseline'] = [dict(kind=tag, baseline_start=i + 1, baseline_end=j, current_start=k + 1, current_end=l) for tag, i, j, k, l in difflib.SequenceMatcher(a=old, b=current, autojunk=False).get_opcodes() if tag != 'equal']
        except KeyError:
            e['baseline_hunks_available'] = False
binding = load(P + 'closeout-binding.json')
bound = [record(e['path']) for e in binding['files']]
package = load(P + 'review-docs/package-recomputed.json')
package_drift = []
for e in package['entries']:
    p = ROOT / e['path']
    mode = stat.S_IMODE(p.lstat().st_mode)
    equal = not p.is_symlink() and mode == e['mode']
    if e['type'] == 'file':
        equal = equal and p.is_file() and p.stat().st_size == e['size'] and hashlib.sha256(p.read_bytes()).hexdigest() == e['sha256']
    else:
        equal = equal and p.is_dir()
    if not equal:
        package_drift.append(e['path'])
package_members = sorted(str(p.relative_to(ROOT)) for p in (ROOT / 'installer/dist').rglob('*'))
package_equal = not package_drift and package_members == [e['path'] for e in package['entries']]
previous = load(P + 'review-docs/existing-evidence-before.json')
previous_changes = [e['path'] for e in previous['files'] if record(e['path']) != e]
after = [record(p, True) for p in selected]
save('late-current-core-observation.json', dict(files=actual, count=len(actual), canonical_files_sha256=canonical(actual), stable_during_observation=actual == after, not_a_validated_candidate=True))
value = dict(
    checked_at=datetime.datetime.now(datetime.timezone.utc).isoformat(), command=['python3', '-B', P + 'review-docs/record_final_drift.py'], cwd='<repo>',
    final_verdict='fail for current-checkout unchanged-source closeout', docs_bytes_verdict='pass' if bound == binding['files'] else 'fail',
    parent_docs_canonical_sha256=canonical(bound), parent_binding=record(P + 'closeout-binding.json'), bound_files=len(bound),
    reviewed_core_sha256=core['sha256'], reviewed_core_files=len(core['files']),
    observed_current_selector_sha256=canonical(actual), observed_current_selector_files=len(actual), observation_stable=actual == after,
    changed_previously_reviewed_inputs=changes, newly_selected_core_files=[actual_map[p] for p in new_paths],
    unchanged_package=package_equal, reviewed_package_sha256=package['sha256'], package_drift=package_drift,
    prior_layout_evidence_unchanged=not previous_changes, prior_layout_evidence_files=len(previous['files']), changed_prior_evidence=previous_changes,
    supersedes_current_checkout_pass_in='REVIEW-DOCS.md (earlier successful bound/core observation, before concurrent drift)',
    superseded_no_drift_attempt=record(P + 'review-docs/DOCS-CLOSEOUT.json'),
    initial_report_preserved=record(P + 'review-docs/REVIEW-DOCS.md'),
    source_review_status='Historical layout PASS remains attached only to f00306... and its package; it is not transferable to current concurrent work.',
    requirement='The latest request requires unchanged frozen source. Current source no longer satisfies that binding; no tests or risk waiver can be inferred.',
    blocker=dict(id='H1', severity='high', scenario='Final read-only core recheck after final docs review; another task added and changed frozen client core inputs.', remediation='Preserve concurrent work. Supply an isolated exact reviewed snapshot for layout closeout, or independently validate/review/rebind any intended combined candidate under its own scope. Do not revert files or edit the old binding to hide drift.'),
    human_approval_inferred=False, risk_waiver=False, tests_rerun=False, source_or_bound_files_edited_by_reviewer=False,
    release_gate_decision='no-go',
)
save('FINAL-DRIFT.json', value)
text = json.dumps(dict(final_verdict=value['final_verdict'], docs_bytes_verdict=value['docs_bytes_verdict'], parent_docs_canonical_sha256=value['parent_docs_canonical_sha256'], reviewed_core_sha256=core['sha256'], observed_current_selector_sha256=canonical(actual), changed_paths=[e['path'] for e in changes], added_core_paths=new_paths, package_unchanged=package_equal, existing_layout_evidence_unchanged=not previous_changes, no_tests_or_source_edits=True), sort_keys=True, indent=2) + '\n'
with (OUT / 'final-drift.log').open('x') as handle:
    handle.write(text)
print(text)
