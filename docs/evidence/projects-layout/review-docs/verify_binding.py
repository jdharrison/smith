#!/usr/bin/env python3
"""Independent read-only docs/core/artifact checks; writes only beside this file."""
import ast
import datetime
import difflib
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
PREFIX = 'docs/evidence/projects-layout/'
EXPECTED_DOCS = 'd7a32b500cbaf874f1f8ea840cd102f71406be7bd6a3f7b653a31c1a7d26c7a8'
EXPECTED_CORE = 'f00306ef6fb418e4c3a53b12261599cc290ede874d12822e5a647e58fdf54164'
EXPECTED_PACKAGE = 'd168fdf131402ee099b516ea752fb510c8b86f2eece1c7aca6b4f0e0e048f725'
EXPECTED_BASE = '740cc41d9ba4bc8265d5ea69a34fe5bf5ac8e11596ef4223357232a6756a44c3'
CHECKS, LOG, INPUTS = [], [], {}
START = datetime.datetime.now(datetime.timezone.utc).isoformat()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return digest(json.dumps(value, sort_keys=True, separators=(',', ':')).encode())


def path(relative):
    p = PurePosixPath(relative)
    if p.is_absolute() or not p.parts or '..' in p.parts or str(p) != relative:
        raise ValueError('unsafe project-relative path')
    current = ROOT
    for part in p.parts:
        current = current / part
        if current.is_symlink():
            raise ValueError('symlink in reviewed path: ' + relative)
    return current


def record(relative):
    p = path(relative)
    info = p.lstat()
    mode = stat.S_IMODE(info.st_mode)
    kind = 'file' if stat.S_ISREG(info.st_mode) else 'directory' if stat.S_ISDIR(info.st_mode) else 'unsupported'
    result = dict(path=relative, type=kind, mode=mode, mode_octal=oct(mode))
    if kind == 'file':
        result.update(size=info.st_size, sha256=digest(p.read_bytes()))
    return result


def simple(relative):
    value = record(relative)
    if value['type'] != 'file':
        raise ValueError('expected regular file: ' + relative)
    return {k: value[k] for k in ('path', 'size', 'mode', 'sha256')}


def load(relative):
    INPUTS[relative] = simple(relative)
    return json.loads(path(relative).read_text())


def save(name, value):
    with (OUT / name).open('x') as handle:
        handle.write(json.dumps(value, sort_keys=True, indent=2) + '\n')


def check(name, ok, details):
    value = dict(check=name, result='PASS' if ok else 'FAIL', details=details)
    CHECKS.append(value)
    LOG.append(value['result'] + ' ' + name + ': ' + json.dumps(details, sort_keys=True))
    print(LOG[-1], flush=True)


def differences(expected):
    current = record(expected['path'])
    current['tracked'] = expected['path'] in TRACKED
    return {k: {'expected': expected[k], 'actual': current.get(k)} for k in expected if current.get(k) != expected[k]}


def verify_entries(name, entries):
    mismatches = []
    for entry in entries:
        try:
            delta = differences(entry)
        except (OSError, ValueError) as exc:
            delta = {'error': type(exc).__name__}
        if delta:
            mismatches.append(dict(path=entry['path'], differences=delta))
    check(name, not mismatches, dict(count=len(entries), mismatches=mismatches))
    return mismatches


def git(*arguments):
    command = ['git', '--no-pager', '--no-optional-locks', *arguments]
    result = subprocess.run(command, cwd=ROOT, capture_output=True, timeout=20, check=True)
    LOG.append('READ_ONLY_COMMAND ' + json.dumps(command))
    return result.stdout


def clean_log(relative):
    INPUTS[relative] = simple(relative)
    return re.sub(r'\x1b\[[0-?]*[ -/]*[@-~]', '', path(relative).read_text()).splitlines()


TRACKED = set(filter(None, git('ls-files', '--cached', '-z').decode().split('\0')))
VISIBLE = set(filter(None, git('ls-files', '--cached', '--others', '--exclude-standard', '-z').decode().split('\0')))
branch = git('branch', '--show-current').decode().strip()
head = git('rev-parse', 'HEAD').decode().strip()
check('branch-head', branch == 'feature/autonomy' and head == '14d960975e76f906efb5e447ff3dece0e8e4385b', dict(branch=branch, head=head))
binding_path = PREFIX + 'closeout-binding.json'
binding_identity_before = simple(binding_path)
binding = load(binding_path)
bound_before = [simple(e['path']) for e in binding['files']]
check('closeout-canonical-and-path-set', canonical(binding['files']) == EXPECTED_DOCS == binding['canonical_files_sha256'] and len({e['path'] for e in binding['files']}) == len(binding['files']), dict(files=len(binding['files']), recomputed_sha256=canonical(bound_before), expected_sha256=EXPECTED_DOCS, raw_binding_sha256=binding_identity_before['sha256']))
verify_entries('every-bound-file-byte-size-mode', binding['files'])
check('current-docs-canonical', canonical(bound_before) == EXPECTED_DOCS, dict(sha256=canonical(bound_before)))
save('bound-files-before.json', dict(binding=binding_identity_before, canonical_files_sha256=canonical(bound_before), files=bound_before))
prior_paths = sorted(str(p.relative_to(ROOT)) for p in path(PREFIX.rstrip('/')).rglob('*') if p.is_file() and OUT not in p.parents)
prior_before = [simple(p) for p in prior_paths]
save('existing-evidence-before.json', dict(files=prior_before, canonical_files_sha256=canonical(prior_before)))

base = load(PREFIX + 'baseline-source-manifest.json')
check('baseline-manifest', len(base['files']) == 1459 and canonical(base['files']) == base['sha256'] == EXPECTED_BASE, dict(files=len(base['files']), sha256=canonical(base['files'])))
archive = 'target/projects-layout/baseline-source.tar.gz'
archive_identity = simple(archive)
base_map = {e['path']: e for e in base['files']}
archive_entries, doc_diff, archive_unsafe = [], [], []
allowed = {
    'VALIDATION.md', 'client/assets/README.md', 'client/lib/ui/projects_view.dart',
    'client/lib/ui/smith_icon.dart', 'client/test/projects_navigation_test.dart',
    'client/test/projects_view_test.dart', 'dagger/package.py', 'dagger/tests/test_package.py',
    'docs/DESIGN.md', 'docs/PROCESS.md', 'docs/SMITH_LINUX_ARCHITECTURE_REPORT.md',
    'docs/VALIDATION_DETAILS.md', 'docs/plans/LOCAL-SMITH-PROJECTS-EDITOR-LAYOUT.md',
}
with tarfile.open(path(archive), 'r:gz') as tar:
    for member in tar:
        p = PurePosixPath(member.name)
        if not member.isfile() or p.is_absolute() or '..' in p.parts or str(p) != member.name:
            archive_unsafe.append(member.name)
            continue
        data = tar.extractfile(member).read()
        archive_entries.append(dict(path=member.name, mode=member.mode, size=len(data), sha256=digest(data)))
        if member.name in allowed and member.name.endswith('.md'):
            old, new = data.decode(), path(member.name).read_text()
            doc_diff.extend(difflib.unified_diff(old.splitlines(True), new.splitlines(True), fromfile='baseline/' + member.name, tofile='current/' + member.name))
        if member.name == 'docs/PROCESS.md':
            process_old = data.decode()
archive_entries.sort(key=lambda e: e['path'])
check('baseline-archive-read-no-extraction', not archive_unsafe and len({e['path'] for e in archive_entries}) == 1459 and archive_entries == base['files'] and archive_identity['sha256'] == '80f9672a2707d25e27eb91c682eba4aa5064501fbbd95f7131ae9da68831ec6a', dict(files=len(archive_entries), unsafe_members=archive_unsafe, archive=archive_identity, canonical_sha256=canonical(archive_entries)))
with (OUT / 'baseline-documentation.diff').open('x') as handle:
    handle.write(''.join(doc_diff))
check('process-change-is-mechanical-only', process_old.replace('Current UI asset contract is24 exact nonempty paths', 'Current UI asset contract is26 exact nonempty paths') == path('docs/PROCESS.md').read_text(), dict(path='docs/PROCESS.md', change='24 to 26 exact UI asset count only'))
changed = [e['path'] for e in base['files'] if differences(e)]
audit = load(PREFIX + 'scope-audit.json')
check('baseline-delta-scope', set(changed) == allowed and changed == audit['changed_baseline_paths'], dict(changed_count=len(changed), changed=changed, unexpected=sorted(set(changed) - allowed)))
additions = VISIBLE - set(base_map)
omitted = {'.gitattributes', '.gitignore', '.smith/project.yaml', 'LICENSE', 'SECURITY.md'}
new_source = {e['path'] for e in audit['new_source_files']}
expected_new = {'client/assets/icons/layers.svg', 'client/assets/icons/git-branch.svg', 'client/test/projects_layout_test.dart', 'client/test/projects_layout_golden_test.dart'}
expected_new.update(e['path'] for e in audit['new_source_files'] if e['path'].startswith('client/test/goldens/projects-layout/') and e['path'].endswith('.png'))
new_unexpected = additions - omitted - expected_new - {p for p in additions if p.startswith(PREFIX)}
check('new-source-and-omitted-roots', len(new_source) == 12 and new_source == expected_new and new_source <= additions and not new_unexpected and all(path(p).read_bytes() == git('show', 'HEAD:' + p) for p in omitted), dict(new_source_files=len(new_source), new_pngs=len(new_source) - 4, unexpected=sorted(new_unexpected), head_equal_omitted=sorted(omitted)))
verify_entries('new-source-hashes-modes', audit['new_source_files'])
for name, entries, count in (
    ('immutable-historical-evidence', [e for e in base['files'] if e['path'].startswith(('docs/evidence/projects/', 'docs/evidence/project-repositories/'))], 127),
    ('immutable-old-golden-pngs', [e for e in base['files'] if e['path'].startswith('client/test/goldens/') and e['path'].endswith('.png')], 28),
    ('immutable-imported-icons', [e for e in base['files'] if e['path'].startswith('client/assets/icons/') and e['path'].endswith('.svg')], 15),
):
    verify_entries(name, entries)
    check(name + '-count', len(entries) == count, dict(count=len(entries)))

core_ref = load(PREFIX + 'validation-rebound/core-manifest-final.json')
core = load(core_ref['manifest'])
check('core-reference', simple(core_ref['manifest'])['sha256'] == core_ref['manifest_file_sha256'] and len(core['files']) == core_ref['count'] == 1265 and canonical(core['files']) == core['sha256'] == core_ref['sha256'] == EXPECTED_CORE, dict(files=len(core['files']), sha256=canonical(core['files']), referenced_file_sha256=simple(core_ref['manifest'])['sha256']))
verify_entries('every-current-core-input', core['files'])
roots = set(core['source_roots'])
def core_selected(p):
    return (p in roots or p.split('/')[0] in roots) and not p.startswith(tuple(core['excluded_prefixes'])) and '__pycache__' not in PurePosixPath(p).parts and not p.endswith('.pyc') and p != 'client/.flutter-plugins-dependencies'
selected = {p for p in VISIBLE if core_selected(p)}
check('core-selector-completeness', selected == {e['path'] for e in core['files']}, dict(current_files=len(selected), extra=sorted(selected - {e['path'] for e in core['files']}), missing=sorted({e['path'] for e in core['files']} - selected), tracked=sum(e['path'] in TRACKED for e in core['files']), untracked=sum(e['path'] not in TRACKED for e in core['files'])))
core_actual = [{k: (e['path'] in TRACKED if k == 'tracked' else record(e['path'])[k]) for k in e} for e in core['files']]
save('current-core-manifest.json', dict(files=core_actual, sha256=canonical(core_actual), shape='parent gate selector reconstructed from actual current metadata/bytes'))

index_results = []
for relative, key in ((PREFIX + 'validation-rebound/evidence-index-final.json', 'canonical_entries_sha256'), (PREFIX + 'review-closeout/evidence-manifest.json', 'canonical_files_sha256'), (PREFIX + 'review-closeout/prior-preservation-before.json', 'canonical_files_sha256')):
    index = load(relative)
    verify_entries('indexed-files:' + relative, index['files'])
    ok = canonical(index['files']) == index[key] and len({e['path'] for e in index['files']}) == len(index['files'])
    check('index-canonical:' + relative, ok, dict(files=len(index['files']), canonical_sha256=canonical(index['files'])))
    index_results.append(dict(path=relative, files=len(index['files']), canonical_sha256=canonical(index['files'])))

results = load(PREFIX + 'results.json')
gates = load(PREFIX + 'validation-rebound/final-results.json')
observed = load(PREFIX + 'validation-rebound/observed-gates.json')
review = load(PREFIX + 'review-closeout/review-summary.json')
for name in ('gate_record', 'review_record', 'review_report', 'artifact_record'):
    verify_entries('results-reference:' + name, [results[name]])
command_observations = []
for command in gates['commands']:
    verify_entries('parent-command-record:' + command['name'], [command['record']])
    value = load(command['record']['path'])
    logfile = value['log']
    check('parent-command-log:' + command['name'], value['exit_code'] == 0 and not value['timed_out'] and value['stable_inputs'] and simple(logfile)['sha256'] == value['log_sha256'], dict(record=command['record']['path'], log=logfile, log_sha256=simple(logfile)['sha256'], exit_code=value['exit_code'], observation_only=True))
    command_observations.append(dict(name=command['name'], record=command['record'], log=simple(logfile), argv=value.get('argv'), exit_code=value['exit_code']))
root_lines = clean_log(PREFIX + 'validation-rebound/root-gates.log')
check('root-raw-log-identity', simple(PREFIX + 'validation-rebound/root-gates.log')['sha256'] == observed['root_log_sha256'], dict(sha256=observed['root_log_sha256']))
excerpts = []
for component, expected in (('service', 295), ('installer_gtk', 9)):
    info = observed[component]
    suites = []
    for line in root_lines[:observed['invocations'][1]['line'] - 1]:
        match = re.match(r'\s*(\d+)\s*:', line)
        result = re.search(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', line)
        if match and result and int(match[1]) == info['vertex'] and '|' in line:
            values = tuple(map(int, result.groups()))
            if (values, line) not in suites:
                suites.append((values, line))
    total = sum(v[0][0] for v in suites)
    check('raw-fresh-suite-count:' + component, total == expected == info['totals']['passed'] and all(v[0][1] == 0 for v in suites), dict(passed=total, ignored=sum(v[0][2] for v in suites), vertex=info['vertex'], execution=info['execution']))
    excerpts.extend(dict(component=component, text=v[1]) for v in suites)
for component, field in (('flutter', 'terminal_result'), ('installer_native', 'cached_event'), ('native_interpreter_free_runtime', 'completion')):
    e = observed[component][field]
    check('raw-terminal:' + component, e['text'] in root_lines[e['line'] - 1], dict(line=e['line'], text=e['text'], execution=observed[component]['execution']))
    excerpts.append(dict(component=component, line=e['line'], text=root_lines[e['line'] - 1]))
for e in observed['native_interpreter_free_runtime']['command_events']:
    check('cached-cli-command:' + str(e['line']), e['text'] in root_lines[e['line'] - 1], e)
projection = [e for e in core_actual if e['path'].startswith(('installer/', 'service/', 'dagger/'))]
previous_core = load(PREFIX + 'validation-final/core-manifest-final.json')
if 'files' not in previous_core:
    previous_core = load(previous_core['manifest'])
previous_projection = [e for e in previous_core['files'] if e['path'].startswith(('installer/', 'service/', 'dagger/'))]
prior_obs_ref = observed['installer_native']['prior_result']
verify_entries('cached-installer-prior-record', [prior_obs_ref])
prior_obs = load(prior_obs_ref['path'])
check('cached-installer-projection-and-counts', len(projection) == 1065 and projection == previous_projection and canonical(projection) == observed['installer_native']['input_projection_sha256'] == '8993531727a8fc09fb197d3f1be83a15f485f7f39e0ad75e954befd349caba8a' and prior_obs['installer_native']['totals']['passed'] == 283 and prior_obs['installer_native']['totals']['ignored'] == 2, dict(files=len(projection), sha256=canonical(projection), native_passes=283, controlled_ignores=2, execution='cached, not fresh'))
check('accounting-cross-records', results['aggregate_coverage'] == observed['component_total_not_all_fresh'] == review['parent_observed_aggregate'] == 1497 and results['fresh_component_passes'] == gates['fresh_component_passed'] == review['parent_fresh_aggregate_cases'] == 1214 and results['cached_installer_passes'] == gates['cached_component_passed'] == 283 and gates['flutter_fresh_counts']['passed'] == 869 and gates['gtk_fresh_counts']['passed'] == 9 and gates['native_package_fresh_counts']['passed'] == 41 and gates['helper_fresh_counts']['passed'] == 34 and gates['controlled_fixture_ignores_fresh'] == 3 and gates['controlled_fixture_ignores_cached'] == 2, dict(aggregate=1497, fresh=1214, cached_installer=283, helpers=34, controlled_ignores=5, interpreter_free_cli='cached; not a fresh scenario', parent_executions_not_reviewer_executions=True))
review_counts = []
for run in review['reviewer_runs']:
    rp = PREFIX + 'review-closeout/' + run['record']
    value = load(rp)
    lp = PREFIX + 'review-closeout/' + run['log']
    lines = clean_log(lp)
    terminal = [re.search(r'\+(\d+)(?:\s+~\d+)?(?:\s+-\d+)?: All tests passed!', line) for line in lines]
    actual = [int(m[1]) for m in terminal if m]
    check('retained-review-run:' + run['name'], value['exit_code'] == run['exit_code'] == 0 and not value['timed_out'] and simple(lp)['sha256'] == run['log_sha256'] == value['log_sha256'] and actual and actual[-1] == run['passed'], dict(passed=actual[-1] if actual else None, expected=run['passed'], log=lp, observed_prior_review_execution=True, rerun=False))
    review_counts.append(dict(name=run['name'], passed=run['passed']))
check('review-execution-total', sum(e['passed'] for e in review_counts) == review['independent_flutter_executions_total'] == 322 and review['independent_repository_flutter_executions_passed'] == 293 and review['independent_scratch_executions_passed'] == 29, dict(repository=293, scratch=29, executions=322, newly_executed_tests=0))
for group, expected in (('native_package', 41), ('pipeline_helpers', 34)):
    info = observed[group]
    check('observed-case-table:' + group, len(info['cases']) == expected == info['passed'] == info['ran'] and info['skipped'] == info['failed'] == info['errors'] == 0 and all(e['result'] == 'ok' for e in info['cases']), dict(cases=len(info['cases']), skips=info['skipped']))
    log_lines = root_lines if group == 'native_package' else clean_log(PREFIX + 'validation-rebound/pipeline-helpers.log')
    missing = [e['test'] for e in info['cases'] if not any(e['test'] in line and re.search(r'\.\.\. ok\s*$', line) for line in log_lines)]
    check('raw-ok-lines:' + group, not missing, dict(checked=expected, missing=missing))
save('gate-and-review-observations.json', dict(parent_commands=command_observations, index_results=index_results, excerpts=excerpts, review_counts=review_counts, newly_executed_tests=0, classification='hash/log observations, not rerun Make/Flutter/native/helper gates'))

artifact = load(PREFIX + 'validation-rebound/final-artifact-binding.json')
verify_entries('package-tree-manifest-reference', [artifact['full_manifest']])
package_manifest = load(artifact['full_manifest']['path'])
package_entries = [record(str(p.relative_to(ROOT))) for p in sorted(path('installer/dist').rglob('*'))]
check('current-package-full-tree', package_entries == package_manifest['entries'] and canonical(package_entries) == package_manifest['sha256'] == artifact['installer_dist_tree_sha256'] == EXPECTED_PACKAGE and sum(e['type'] == 'file' for e in package_entries) == 99 and sum(e['type'] == 'directory' for e in package_entries) == 20, dict(files=sum(e['type'] == 'file' for e in package_entries), directories=sum(e['type'] == 'directory' for e in package_entries), sha256=canonical(package_entries), modes_included=True))
verify_entries('package-root-and-pairing', [artifact['installer_dist_root'], artifact['package_manifest_file'], artifact['package_checksums_file'], *artifact['paired_helpers']])
assets_node = [node.value for node in ast.parse(path('dagger/package.py').read_text()).body if isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id == 'UI_ASSET_FILES' for t in node.targets)][0]
ui_assets = ast.literal_eval(assets_node)
asset_mismatches = []
for asset_path in ui_assets:
    relative = asset_path.split('data/flutter_assets/', 1)[1]
    records = [record('client/' + relative), record('client/dist/' + asset_path), record('installer/dist/payload/client/' + asset_path)]
    if len({(e['size'], e['sha256']) for e in records}) != 1 or not records[0]['size'] or records[1]['mode'] != 0o644 or records[2]['mode'] != 0o644:
        asset_mismatches.append(asset_path)
client_entries = [record(str(p.relative_to(ROOT))) for p in sorted(path('client/dist').rglob('*'))]
client_shape = [{**e, 'path': e['path'].removeprefix('client/dist/')} for e in client_entries]
package_client_shape = [{**e, 'path': e['path'].removeprefix('installer/dist/payload/client/')} for e in package_entries if e['path'].startswith('installer/dist/payload/client/')]
check('exact-assets-and-client-pairing', len(ui_assets) == len(set(ui_assets)) == 26 and not asset_mismatches and client_shape == package_client_shape, dict(assets=len(ui_assets), mismatches=asset_mismatches, full_client_subtree_equal=client_shape == package_client_shape))
save('package-recomputed.json', dict(entries=package_entries, sha256=canonical(package_entries), assets=sorted(ui_assets), verification='read/hash only, no package program or native ELF executed'))

markdown = sorted(p for p in allowed if p.endswith('.md')) + [PREFIX + 'README.md']
links, missing = [], []
for relative in markdown:
    for target in re.findall(r'!?\[[^\]]*\]\(([^\s)]+)\)', path(relative).read_text()):
        if '://' in target or target.startswith(('#', 'mailto:')):
            continue
        local = target.split('#', 1)[0]
        if local:
            entry = dict(source=relative, target=target)
            links.append(entry)
            resolved = path(relative).parent.joinpath(local).resolve()
            if not resolved.is_relative_to(ROOT) or not resolved.exists():
                missing.append(entry)
check('parent-link-audit-reproduced', links == audit['local_markdown_file_links_checked'] and len(links) == 130 and not missing and audit['anchor_validation'] == 'not performed', dict(local_file_links=len(links), source_documents=len(markdown), missing=missing, anchors='not checked', remote_links='not fetched'))
save('links-rechecked.json', dict(links=links, missing=missing, anchors_checked=False))
correction = load(PREFIX + 'review-closeout/attribution-correction/correction.json')
verify_entries('corrected-attribution-identities', correction['corrected_records'])
check('approval-and-source-verdict-boundaries', binding['no_approval_or_risk_waiver'] and review['attribution_correction']['specific_human_mapping_direction_claimed'] is False and correction['new_human_approval_or_risk_waiver'] is False and not results['native_installed_release_acceptance'] and review['verdict'] == 'pass' and review['source_review_gate'] == 'go' and review['release_gate_decision'] == 'no-go for release/deployment/publication', dict(mapping_origin=review['attribution_correction']['mapping_origin'], verdict_basis=review['attribution_correction']['independent_verdict_basis'], human_approval_inferred=False, risk_accepted=False, release='no-go'))

bound_after = [simple(e['path']) for e in binding['files']]
prior_after = [simple(p) for p in prior_paths]
core_after = [{k: (e['path'] in TRACKED if k == 'tracked' else record(e['path'])[k]) for k in e} for e in core['files']]
check('before-after-unchanged', bound_before == bound_after and simple(binding_path) == binding_identity_before and prior_before == prior_after and core_actual == core_after, dict(bound_files=len(bound_after), existing_evidence_files=len(prior_after), core_files=len(core_after), changes=[], canonical_docs_sha256=canonical(bound_after), canonical_core_sha256=canonical(core_after)))
save('bound-files-after.json', dict(binding=simple(binding_path), canonical_files_sha256=canonical(bound_after), files=bound_after))
save('read-inputs.json', dict(files=[INPUTS[p] for p in sorted(INPUTS)], note='Read/hash identities retained; not a new source gate run.'))
result = dict(work_id='LOCAL-SMITH-PROJECTS-EDITOR-LAYOUT', review_type='final docs/evidence-binding', started_at=START, finished_at=datetime.datetime.now(datetime.timezone.utc).isoformat(), command=['python3', '-B', PREFIX + 'review-docs/verify_binding.py'], cwd='<repo>', checks=CHECKS, technical_verdict='PASS' if all(c['result'] == 'PASS' for c in CHECKS) else 'FAIL', bound_docs_canonical_sha256=canonical(bound_after), raw_binding_identity=binding_identity_before, core_sha256=canonical(core_after), package_tree_sha256=canonical(package_entries), newly_executed_tests=0, human_approval_inferred=False, source_or_bound_files_edited=False, old_evidence_unchanged=prior_before == prior_after)
save('technical-verification.json', result)
with (OUT / 'verification.log').open('x') as handle:
    handle.write('\n'.join(LOG) + '\n')
print('TECHNICAL_VERDICT ' + result['technical_verdict'], flush=True)
raise SystemExit(0 if result['technical_verdict'] == 'PASS' else 1)
