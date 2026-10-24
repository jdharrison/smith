#!/usr/bin/env python3
"""Validation only: exclusive records, immutable inputs, no candidate repairs."""
import ast
import json
import os
from pathlib import Path
import re
import shutil
import sys
import tarfile
from urllib.parse import unquote, urljoin, urlparse
from recorder import *

CAND = OUT / 'candidate'
CONTROL = OUT / 'negative-control'
CARGO_ENV = {'CARGO_TARGET_DIR': str(OUT / 'cargo-target')}

def capture_snapshot():
    before = manifest()
    save('core-manifest-before.json', before)
    for row in before['files']:
        src = ROOT / row['path']; dst = CAND / row['path']
        if row['type'] == 'missing': continue
        dst.parent.mkdir(parents=True, exist_ok=True)
        if row['type'] == 'symlink': dst.symlink_to(os.readlink(src))
        elif row['type'] == 'file': shutil.copy2(src, dst)
        else: raise RuntimeError('Unexpected core input type')
    after = manifest()
    save('core-manifest-after-capture.json', after)
    mismatches = [r['path'] for r in before['files'] if r['type'] == 'file' and digest(CAND/r['path']) != r['sha256']]
    save('snapshot-binding.json', {'before': before['sha256'], 'after': after['sha256'], 'stable_capture': before == after, 'copy_mismatches': mismatches,
        'head': git('rev-parse','HEAD').decode().strip(), 'branch': git('branch','--show-current').decode().strip(),
        'recorder_lineage': [file_record(ROOT/p) for p in ['target/projects-layout/validation-rebound/recorder.py','target/projects-layout/validation-closeout/recorder.py']],
        'local_recorder': file_record(OUT/'recorder.py'), 'driver': file_record(Path(__file__)),
        'cache_policy': 'Existing offline Cargo registry and cached Flutter SDK/pub packages; fresh isolated Cargo target. No old fixture executable. Dagger vertices may be cached.'})
    if before != after or mismatches: raise SystemExit('Unstable capture; stop without pretending final-current evidence')
    archive = OUT/'candidate-core.tar.gz'
    with tarfile.open(archive,'w:gz') as t:
        for r in before['files']:
            if r['type'] != 'missing': t.add(CAND/r['path'], arcname=r['path'], recursive=False)
    save('candidate-archive.json', file_record(archive))
    baseline = ROOT/'target/default-project/baseline-manifest.json'
    bm = json.loads(baseline.read_text())
    if bm['sha256'] != 'aeb1ce67ebb10c0af866313901b7060a921018be60fd267901323a4d69d56ca2': raise RuntimeError('baseline identity mismatch')
    ba = ROOT/'target/default-project/baseline-source.tar.gz'
    if digest(ba) != bm['archive_sha256']: raise RuntimeError('baseline archive mismatch')
    shutil.copytree(CAND/'service', CONTROL/'service')
    owned = ['service/src/app.rs','service/src/app_projects.rs','service/src/app_projects_tests.rs']
    overlays=[]
    with tarfile.open(ba) as t:
        for p in owned:
            data=t.extractfile(p).read(); (CONTROL/p).write_bytes(data)
            overlays.append({'path':p,'candidate_sha256':digest(CAND/p),'archive_sha256': __import__('hashlib').sha256(data).hexdigest(),'control_sha256':digest(CONTROL/p)})
    unchanged=[r['path'] for r in before['files'] if r['path'].startswith('service/') and r['path'] not in owned and r['type']=='file' and digest(CONTROL/r['path']) != r['sha256']]
    save('negative-control-binding.json', {'baseline_manifest':file_record(baseline),'baseline_embedded_identity':bm['sha256'],'baseline_archive':file_record(ba),'overlays':overlays,'other_service_mismatches':unchanged,'fixture_kept_current':file_record(CONTROL/'service/tests/debug_service.rs'), 'scope':'Only three complete archive files overlaid. Other current GlobalContext and fixture inputs retained. Not a pristine historical-tree execution.'})
    if unchanged: raise RuntimeError('control copied inputs mismatch')
    # Test outputs/dependency metadata remain under the allowed validation tree.
    (CAND/'client/.dart_tool').mkdir(exist_ok=True)
    for p in (ROOT/'client/.dart_tool').iterdir():
        if p.is_file(): shutil.copy2(p,CAND/'client/.dart_tool'/p.name)
    save('dependency-metadata.json', [file_record(p) for p in sorted((CAND/'client/.dart_tool').iterdir())])
    record_tree('installer-dist-before.json','installer/dist')
    record_tree('client-dist-before.json','client/dist')
    record_tree('service-dist-before.json','service/dist')

def cargo(name, args, control=False, seconds=240):
    cwd=(CONTROL if control else CAND)/'service'
    return run(name,['cargo',*args[:1],'--manifest-path','Cargo.toml','--locked','--offline',*args[1:]],seconds,cwd=cwd,extra=CARGO_ENV)

def rust():
    results=[]
    results.append(cargo('rust-bootstrap',['test','--lib','projects_bootstrap']))
    results.append(cargo('rust-projects',['test','--lib','projects_']))
    results.append(cargo('rust-production-clippy',['clippy','--lib','--','-D','warnings']))
    results.append(cargo('rust-all-target-clippy',['clippy','--all-targets','--','-D','warnings']))
    results.append(run('rust-format',['cargo','fmt','--manifest-path','Cargo.toml','--all','--','--check'],60,cwd=CAND/'service'))
    results.append(cargo('fixture-rebuild',['test','--test','debug_service','--no-run']))
    fixture=OUT/'cargo-target/debug/deps'
    binaries=[p for p in fixture.glob('debug_service-*') if p.is_file() and os.access(p,os.X_OK) and p.suffix != '.d'] if fixture.exists() else []
    ok=results[-1]['status']=='pass' and len(binaries)==1
    if ok: shutil.copy2(binaries[0],OUT/'debug_service-fixture')
    save('fixture-binding.json',{'new_build_completed':results[-1]['status']=='pass','binary_candidates':[file_record(p) for p in binaries], 'fixture':file_record(OUT/'debug_service-fixture'),'eligible':ok,'service_archive':file_record(OUT/'candidate-core.tar.gz'),'old_preseed_binary_used':False})
    for label in ['disk5_projects_control_seeds_real_legacy_bytes_and_reopens_without_effects','bootstrap_modes_keep_fake_keys_and_restart_resets_only_volatile_facts']:
        results.append(cargo('fixture-control-'+label,['test','--test','debug_service','dart_cloud::'+label,'--','--exact']))
    results.append(cargo('negative-control-projects',['test','--lib','projects_'],control=True))
    results.append(cargo('negative-control-all-target-clippy',['clippy','--all-targets','--','-D','warnings'],control=True))
    save('rust-results.json',{'commands':results,'summaries':{r['name']:test_summary(r['name']) for r in results}})

def flutter():
    config=CAND/'client/.dart_tool/package_config.json'
    fp=next(p for p in json.loads(config.read_text())['packages'] if p['name']=='flutter')
    sdk=Path(unquote(urlparse(urljoin(config.as_uri(),fp['rootUri'])).path)).resolve().parents[1]
    dart=sdk/'bin/cache/dart-sdk/bin/dart'; tools=sdk/'bin/cache/flutter_tools.snapshot'
    env={'DART_SUPPRESS_ANALYTICS':'true','FLUTTER_ALREADY_LOCKED':'true','FLUTTER_ROOT':str(sdk)}
    base=[str(dart),str(tools),'--suppress-analytics','--no-version-check']
    save('flutter-toolchain.json',{'dart':file_record(dart),'tools':file_record(tools),'version':json.loads((sdk/'bin/cache/flutter.version.json').read_text()),'package_config':file_record(config)})
    results=[]
    files=['projects_view_test.dart','projects_navigation_test.dart','projects_layout_test.dart','projects_layout_golden_test.dart','projects_controller_test.dart','projects_protocol_test.dart']
    results.append(run('flutter-focused-projects',base+['test','--no-pub','--reporter','expanded']+['test/'+p for p in files],240,cwd=CAND/'client',extra=env))
    results.append(run('flutter-assets',base+['test','--no-pub','--reporter','expanded','test/ui_foundation_assets_test.dart'],120,cwd=CAND/'client',extra=env))
    fb=json.loads((OUT/'fixture-binding.json').read_text())
    results.append(run('flutter-compiled-projects',base+['test','--no-pub','--reporter','expanded','test/rust_projects_integration_test.dart'],180,cwd=CAND/'client',extra=dict(env,SMITH_TEST_CLOUD_SERVICE=str(OUT/'debug_service-fixture')),skip_reason=None if fb['eligible'] else 'Current fixture build unavailable'))
    results.append(run('flutter-scoped-analyze',base+['analyze','--no-pub','lib/projects.dart','lib/projects_controller.dart','lib/ui/projects_view.dart','test/rust_projects_integration_test.dart'],180,cwd=CAND/'client',extra=env))
    results.append(run('flutter-format', [str(dart),'format','--output=none','--set-exit-if-changed','lib','test'],90,cwd=CAND/'client',extra=env))
    save('flutter-results.json',{'commands':results,'summaries':{r['name']:test_summary(r['name']) for r in results}})

def root():
    cli_ok=CLI.is_file() and digest(CLI)==CLI_SHA256
    save('official-cli-binding.json',{'actual':file_record(CLI),'expected_sha256':CLI_SHA256,'verified':cli_ok,'engine':BASE_ENV['_EXPERIMENTAL_DAGGER_RUNNER_HOST'],'PATH_dagger_used':False,'engine_restart_or_change_performed':False})
    run('dagger-version',[str(CLI),'version'],15,skip_reason=None if cli_ok else 'CLI checksum mismatch')
    state=run('engine-observation',['docker','inspect','--format','{"running":{{json .State.Running}},"image":{{json .Config.Image}},"started_at":{{json .State.StartedAt}}}',ENGINE],15)
    engine_ok=False
    if state['status']=='pass':
        j=json.loads((OUT/'engine-observation.log').read_text()); engine_ok=j['running'] and j['image']=='registry.dagger.io/engine:v0.21.9'
    result=run('root-gates',['make','DAGGER=target/installer-pipeline/smith-dagger-0.21.9/dagger','test','build'],900,skip_reason=None if cli_ok and engine_ok else 'Environment refused: pinned CLI or approved already-running engine unavailable; no restart/change authorized')
    reason=None if result['status']=='pass' else 'Root rebuild did not succeed; stale exports are not current-candidate evidence'
    run('package-verify',['python3','-B','dagger/package.py','--root','installer/dist','--verify'],60,skip_reason=reason)
    run('pipeline-helpers',['python3','-B','-m','unittest','discover','-s','scripts/tests','-p','test_*.py','-v'],120,extra={'SMITH_TEST_PACKAGE':'installer/dist'},skip_reason=reason)
    run('diff-check',['git','--no-pager','diff','--check'],15)
    for component in ['installer','client','service']: record_tree(component+'-dist-after.json',component+'/dist')
    save('root-result.json',{'result':result,'full_gate':'PASS' if result['status']=='pass' else 'BLOCKED','rebuilt_export_current_claim':result['status']=='pass','cache_note':'Fresh command invocation; inspect raw Dagger vertices for CACHED versus executed; canceled/unreached checks are not pass.'})

def final():
    end=manifest(); save('core-manifest-final.json',end)
    begin=json.loads((OUT/'core-manifest-before.json').read_text())
    records=[]
    for p in OUT.glob('*.json'):
        j=json.loads(p.read_text())
        if isinstance(j,dict) and j.get('name') and p.name==j['name']+'.json' and 'exit_code' in j: records.append(j)
    save('final-results.json',{'work_id':'LOCAL-SMITH-DEFAULT-PROJECT','finished_at':now(),'core_before':begin['sha256'],'core_after':end['sha256'],'core_count':end['count'],'untracked_count':end['untracked_count'],'live_core_stable':begin==end,'changed_core_paths':source_delta(begin,end),'snapshot_archive':file_record(OUT/'candidate-core.tar.gz'),'commands':records,'summaries':{r['name']:test_summary(r['name']) for r in records},'full_gate':'BLOCKED' if any(r['status']!='pass' for r in records) or begin!=end else 'PASS','approval':'No accepted risk or human approval conferred.'})

if __name__=='__main__':
    {'capture':capture_snapshot,'rust':rust,'flutter':flutter,'root':root,'final':final}[sys.argv[1]]()
