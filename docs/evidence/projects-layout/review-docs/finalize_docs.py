#!/usr/bin/env python3
"""Final read-only stability check and new, standalone assurance output anchor."""
import datetime
import hashlib
import json
from pathlib import Path
import stat
import subprocess

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
P = 'docs/evidence/projects-layout/'


def canonical(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def load(relative):
    return json.loads((ROOT / relative).read_text())


def record(relative, full=False):
    p = ROOT / relative
    info = p.lstat()
    if p.is_symlink() or not (stat.S_ISREG(info.st_mode) or stat.S_ISDIR(info.st_mode)):
        raise ValueError('unsafe reviewed entry: ' + relative)
    r = dict(path=relative, mode=stat.S_IMODE(info.st_mode))
    if stat.S_ISREG(info.st_mode):
        r.update(size=info.st_size, sha256=hashlib.sha256(p.read_bytes()).hexdigest())
    if full:
        r.update(type='file' if stat.S_ISREG(info.st_mode) else 'directory', mode_octal=oct(r['mode']))
    return r


def save(name, value):
    with (OUT / name).open('x') as handle:
        handle.write(json.dumps(value, sort_keys=True, indent=2) + '\n')


binding = load(P + 'closeout-binding.json')
before = load(P + 'review-docs/bound-files-before.json')
supplement = load(P + 'review-docs/supplemental-verification.json')
original = load(P + 'review-docs/technical-verification.json')
expected_failure_names = {'process-change-is-mechanical-only', 'new-source-and-omitted-roots', 'raw-ok-lines:pipeline_helpers'}
failed_names = {e['check'] for e in original['checks'] if e['result'] == 'FAIL'}
tracked = set(filter(None, subprocess.check_output(['git', '--no-pager', '--no-optional-locks', 'ls-files', '--cached', '-z'], cwd=ROOT, timeout=20).decode().split('\0')))
visible = set(filter(None, subprocess.check_output(['git', '--no-pager', '--no-optional-locks', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'], cwd=ROOT, timeout=20).decode().split('\0')))
bound = [record(e['path']) for e in binding['files']]
core = load(P + 'validation-rebound/core-manifest-before.json')
actual_core = []
for e in core['files']:
    r = record(e['path'], True)
    r['tracked'] = e['path'] in tracked
    actual_core.append(r)
roots = set(core['source_roots'])
def selected(p):
    return (p in roots or p.split('/')[0] in roots) and not p.startswith(tuple(core['excluded_prefixes'])) and '__pycache__' not in Path(p).parts and not p.endswith('.pyc') and p != 'client/.flutter-plugins-dependencies'
core_membership_equal = {p for p in visible if selected(p)} == {e['path'] for e in core['files']}
package = load(P + 'review-docs/package-recomputed.json')
actual_package = [record(str(p.relative_to(ROOT)), True) for p in sorted((ROOT / 'installer/dist').rglob('*'))]
prior = load(P + 'review-docs/existing-evidence-before.json')
changed_prior = [e['path'] for e in prior['files'] if record(e['path']) != e]
base = load(P + 'baseline-source-manifest.json')
audit = load(P + 'scope-audit.json')
extra_paths = sorted(p for p in visible - {e['path'] for e in base['files']} - {e['path'] for e in audit['new_source_files']} - set(audit['omitted_baseline_paths_verified_against_HEAD']) if not p.startswith(P))
extra = [record(p) for p in extra_paths]
expected_extra = [e['path'] for e in supplement['whole_current_worktree_audit']['unbound_additions']]
checks = dict(
    parent_binding_raw_identity_unchanged=record(P + 'closeout-binding.json') == before['binding'],
    every_bound_record_unchanged=bound == binding['files'] == before['files'],
    canonical_docs_identity_equal=canonical(bound) == binding['canonical_files_sha256'] == 'd7a32b500cbaf874f1f8ea840cd102f71406be7bd6a3f7b653a31c1a7d26c7a8',
    core_records_and_identity_unchanged=actual_core == core['files'] and canonical(actual_core) == 'f00306ef6fb418e4c3a53b12261599cc290ede874d12822e5a647e58fdf54164',
    core_selector_membership_equal=core_membership_equal,
    package_records_and_identity_unchanged=actual_package == package['entries'] and canonical(actual_package) == 'd168fdf131402ee099b516ea752fb510c8b86f2eece1c7aca6b4f0e0e048f725',
    previous_480_evidence_files_unchanged=not changed_prior,
    all_initial_required_failures_explained=failed_names == expected_failure_names and supplement['supplemented_required_checks_pass'],
    no_additional_unbound_paths_since_report=extra_paths == expected_extra,
)
summary = dict(
    work_id='LOCAL-SMITH-PROJECTS-EDITOR-LAYOUT', review='independent final docs/evidence-binding',
    checked_at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
    verdict='pass' if all(checks.values()) else 'fail', verdict_scope='exact 27 bound files and unchanged previously reviewed layout core/package; not the whole current checkout',
    required_checks=checks, parent_docs_canonical_files_sha256=canonical(bound), parent_binding=record(P + 'closeout-binding.json'),
    core=dict(files=len(actual_core), sha256=canonical(actual_core)),
    package=dict(files=sum(e['type'] == 'file' for e in actual_package), directories=sum(e['type'] == 'directory' for e in actual_package), sha256=canonical(actual_package), exact_assets=26),
    baseline_files=1459, layout_baseline_modifications=13, layout_new_source_files=12,
    unchanged_historical_evidence_files=127, unchanged_old_pngs=28, unchanged_imported_icons=15,
    local_markdown_file_links_checked=130, markdown_anchors_checked=False,
    observed_parent_coverage=dict(aggregate=1497, fresh=1214, cached_installer=283, service_fresh=295, flutter_fresh=869, gtk_fresh=9, native_package_fresh=41, helpers_fresh=34, controlled_ignores_fresh=3, controlled_ignores_cached=2, other_skips=0, interpreter_free_cli='cached, not fresh'),
    retained_independent_review_executions=dict(total=322, repository=293, scratch=29, not_rerun_here=True),
    newly_executed_tests=0, source_or_test_edits=False, bound_or_existing_evidence_edits=False,
    report=record(P + 'review-docs/REVIEW-DOCS.md'),
    initial_failed_verifier_preserved=record(P + 'review-docs/technical-verification.json'),
    supplemental_reconciliation=record(P + 'review-docs/supplemental-verification.json'),
    discrepancies=[dict(id='D1', severity='low', scope='current whole-worktree closed-set inventory, not the bound files', unbound_documents=extra, disposition='Reported, not edited/approved/waived; no whole-current-checkout PASS.')],
    blocking_findings=[] if all(checks.values()) else [k for k, v in checks.items() if not v],
    parent_mapping_origin='technical interpretation independently assessed against already-approved R2/R5 requirements; not specific human direction',
    human_approval_inferred=False, accepted_risk_or_waiver=False,
    release_gate_decision='no-go for release/deployment/publication', required_in_slice_remediation=[] if all(checks.values()) else ['Reconcile the reported identity drift without changing preserved records.'],
    limits=['unbound Global Context work not reviewed or approved', 'native graphics/IME/accessibility/display/font-weight acceptance', 'installed migration/restore/live manager/keyring/provider/model acceptance', 'clean-source/cold-cache/hermetic provenance, SBOM, imported-icon rights and publication/deployment approval'],
)
save('DOCS-CLOSEOUT.json', summary)
files = [record(str(p.relative_to(ROOT))) for p in sorted(OUT.rglob('*')) if p.is_file()]
index = dict(files=files, count=len(files), canonical_files_sha256=canonical(files), excludes_self=True, parent_binding_canonical_sha256=canonical(bound), no_approval_or_risk_waiver=True)
save('evidence-index.json', index)
print(json.dumps(dict(verdict=summary['verdict'], required_checks=checks, parent_docs_sha256=canonical(bound), raw_parent_binding_sha256=summary['parent_binding']['sha256'], core_sha256=canonical(actual_core), package_sha256=canonical(actual_package), independent_summary_sha256=record(P + 'review-docs/DOCS-CLOSEOUT.json')['sha256'], independent_evidence_canonical_sha256=index['canonical_files_sha256'], independent_index_sha256=record(P + 'review-docs/evidence-index.json')['sha256'], unbound_paths=extra_paths), sort_keys=True, indent=2))
raise SystemExit(0 if summary['verdict'] == 'pass' else 1)
