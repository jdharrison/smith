#!/usr/bin/env python3
"""Follow up strict verifier expectations without rewriting initial evidence."""
import datetime
import hashlib
import json
from pathlib import Path
import re
import stat
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
P = 'docs/evidence/projects-layout/'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def canon(value):
    return sha(json.dumps(value, sort_keys=True, separators=(',', ':')).encode())


def record(relative):
    p = ROOT / relative
    if p.is_symlink() or not stat.S_ISREG(p.lstat().st_mode):
        raise ValueError('not a regular file: ' + relative)
    return dict(path=relative, size=p.stat().st_size, mode=stat.S_IMODE(p.stat().st_mode), sha256=sha(p.read_bytes()))


def load(relative):
    return json.loads((ROOT / relative).read_text())


def clean(relative):
    return re.sub(r'\x1b\[[0-?]*[ -/]*[@-~]', '', (ROOT / relative).read_text()).splitlines()


initial = load(P + 'review-docs/technical-verification.json')
failed = [c for c in initial['checks'] if c['result'] == 'FAIL']
with tarfile.open(ROOT / 'target/projects-layout/baseline-source.tar.gz', 'r:gz') as tar:
    process_old = tar.extractfile('docs/PROCESS.md').read().decode()
old_clause = 'Current UI asset contract is24 exact nonempty paths, not a permissive wildcard.'
new_clause = 'Current UI asset contract is26 exact nonempty paths, including the two locally authored Projects layout glyphs, not a permissive wildcard.'
process_ok = process_old.count(old_clause) == 1 and process_old.replace(old_clause, new_clause) == (ROOT / 'docs/PROCESS.md').read_text()
helper_log = P + 'validation-rebound/pipeline-helpers.log'
helper_lines = clean(helper_log)
helper_cases, pending = {}, None
for n, line in enumerate(helper_lines, 1):
    match = re.match(r'^(test_\S+) \(', line)
    if match:
        pending = (match[1], n)
    if pending and re.search(r'\.\.\. ok\s*$', line):
        helper_cases[pending[0]] = dict(start_line=pending[1], result_line=n, text=line)
        pending = None
observed = load(P + 'validation-rebound/observed-gates.json')
helper_ok = set(helper_cases) == {e['test'] for e in observed['pipeline_helpers']['cases']} and len(helper_cases) == 34 and any(re.match(r'Ran 34 tests in ', line) for line in helper_lines) and 'OK' in helper_lines
special = helper_cases.get('test_checkout_commit_fallback_loses_annotation_but_explicit_tag_preserves_it')
parent_focused = {}
for name, expected in (('focused-projects', 169), ('focused-assets', 5), ('compiled-projects', 7)):
    relative = P + 'validation-rebound/' + name + '.log'
    matches = [m for line in clean(relative) if (m := re.search(r'\+(\d+): All tests passed!', line))]
    parent_focused[name] = dict(passed=int(matches[-1][1]) if matches else None, expected=expected, record=record(relative), observation_not_execution=True)
focused_ok = all(e['passed'] == e['expected'] for e in parent_focused.values())
quality_excerpts = []
root_lines = clean(P + 'validation-rebound/root-gates.log')
for e in observed['flutter']['analyzer'] + observed['flutter']['format']:
    quality_excerpts.append(dict(line=e['line'], expected=e['text'], actual=root_lines[e['line'] - 1], matches=e['text'] in root_lines[e['line'] - 1]))
visible = set(filter(None, subprocess.check_output(['git', '--no-pager', '--no-optional-locks', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'], cwd=ROOT, timeout=20).decode().split('\0')))
base = load(P + 'baseline-source-manifest.json')
audit = load(P + 'scope-audit.json')
allowed_new = {e['path'] for e in audit['new_source_files']}
omitted = set(audit['omitted_baseline_paths_verified_against_HEAD'])
unbound_additions = sorted(p for p in visible - {e['path'] for e in base['files']} - allowed_new - omitted if not p.startswith(P))
additions = []
for relative in unbound_additions:
    info = (ROOT / relative).stat()
    additions.append(dict(**record(relative), mtime_utc=datetime.datetime.fromtimestamp(info.st_mtime, datetime.timezone.utc).isoformat(), included_in_parent_binding=False, authority_or_approval_inferred=False))
binding = load(P + 'closeout-binding.json')
current_bound = [record(e['path']) for e in binding['files']]
bound_ok = current_bound == binding['files'] and canon(current_bound) == binding['canonical_files_sha256'] == initial['bound_docs_canonical_sha256']
core = load(P + 'validation-rebound/core-manifest-before.json')
tracked = set(filter(None, subprocess.check_output(['git', '--no-pager', '--no-optional-locks', 'ls-files', '--cached', '-z'], cwd=ROOT, timeout=20).decode().split('\0')))
core_changes = []
for entry in core['files']:
    actual = record(entry['path'])
    actual.update(mode_octal=oct(actual['mode']), type='file', tracked=entry['path'] in tracked)
    if actual != entry:
        core_changes.append(dict(path=entry['path'], current=actual, expected=entry))
prior = load(P + 'review-docs/existing-evidence-before.json')
prior_changes = [e['path'] for e in prior['files'] if record(e['path']) != e]
result = dict(
    recorded_at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
    command=['python3', '-B', P + 'review-docs/supplemental_verification.py'], cwd='<repo>',
    original_verifier_record=record(P + 'review-docs/technical-verification.json'),
    original_failures_preserved=failed,
    process_delta=dict(result='PASS' if process_ok else 'FAIL', original_expectation='count-only literal replacement; too strict to recognize the descriptive glyph clause', actual_exact_replacement=dict(old=old_clause, new=new_clause), policy_or_approval_boundary_relaxed=False),
    helper_multiline_result=dict(result='PASS' if helper_ok else 'FAIL', original_expectation='name and ... ok on the same line', actual='unittest splits this test name and its descriptive docstring/result across adjacent lines', case=special, raw_log=record(helper_log), cases=helper_cases),
    parent_focused_terminal_counts=parent_focused, quality_raw_excerpts=quality_excerpts,
    whole_current_worktree_audit=dict(matches_parent_closed_set=not unbound_additions, unbound_additions=additions, note='These paths are outside the bound layout files. Observed mtimes are later than the parent binding; mtimes alone do not authenticate task ownership or approval. Do not apply layout source assurance to them.'),
    bound_docs=dict(result='PASS' if bound_ok else 'FAIL', canonical_files_sha256=canon(current_bound), count=len(current_bound), raw_binding=record(P + 'closeout-binding.json')),
    final_core=dict(result='PASS' if not core_changes else 'FAIL', sha256=core['sha256'] if not core_changes else None, files=len(core['files']), changed=core_changes),
    previous_evidence_preservation=dict(result='PASS' if not prior_changes else 'FAIL', files=len(prior['files']), changed=prior_changes),
    supplemented_required_checks_pass=process_ok and helper_ok and focused_ok and all(e['matches'] for e in quality_excerpts) and bound_ok and not core_changes and not prior_changes,
    new_tests_executed=0, human_approval_inferred=False, accepted_risk=False,
)
with (OUT / 'supplemental-verification.json').open('x') as handle:
    handle.write(json.dumps(result, sort_keys=True, indent=2) + '\n')
summary = {k: result[k] for k in ('recorded_at', 'supplemented_required_checks_pass', 'bound_docs', 'final_core', 'previous_evidence_preservation', 'whole_current_worktree_audit')}
text = json.dumps(summary, sort_keys=True, indent=2) + '\n'
with (OUT / 'supplemental-verification.log').open('x') as handle:
    handle.write(text)
print(text)
raise SystemExit(0 if result['supplemented_required_checks_pass'] else 1)
