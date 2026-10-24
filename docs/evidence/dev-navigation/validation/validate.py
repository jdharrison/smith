#!/usr/bin/env python3
import json, os, shutil, sys, tarfile
from pathlib import Path
from urllib.parse import urljoin, urlparse, unquote
from recorder import *

WORK = OUT / 'source'

def native():
    before = manifest()
    save('core-manifest-before.json', before)
    baseline = json.loads((ROOT / 'target/dev-navigation/baseline-manifest.json').read_text())
    assert canonical_sha(baseline['files']) == '8d5e063fc8ec672d40b3218995fe3fdf98b170652c2dbb44d2ab561d2ade993f'
    assert digest(ROOT / 'target/dev-navigation/baseline-source.tar.gz') == baseline['archive_sha256']
    names = {r['path'] for r in before['files']}
    names.update(r['path'] for r in baseline['files'])
    names.update(os.fsdecode(n) for n in git('ls-files','--cached','--others','--exclude-standard','-z','--','docs','roles','AGENTS.md').split(b'\0') if n and not os.fsdecode(n).startswith('docs/evidence/'))
    copied = []
    for name in sorted(names):
        p = ROOT / name
        if p.is_file() or p.is_symlink():
            q = WORK / name
            q.parent.mkdir(parents=True, exist_ok=True)
            if p.is_symlink(): q.symlink_to(os.readlink(p))
            else: shutil.copy2(p,q)
            copied.append(file_record(p))
    save('snapshot-binding.json', {'recorded_at':now(),'files':copied,'sha256':canonical_sha(copied),'core_sha256':before['sha256'],'baseline_archive':file_record(ROOT/'target/dev-navigation/baseline-source.tar.gz'),'baseline_canonical_sha256':canonical_sha(baseline['files']),'differences_from_baseline':[r['path'] for r in baseline['files'] if not (ROOT/r['path']).is_file() or digest(ROOT/r['path']) != r['sha256']]})
    with tarfile.open(OUT/'source-snapshot.tar.gz','w:gz') as t:
        for name in sorted(names):
            p=WORK/name
            if p.is_file() or p.is_symlink(): t.add(p,arcname=name,recursive=False)
    config = ROOT/'client/.dart_tool/package_config.json'
    data=json.loads(config.read_text())
    flutter=next(p for p in data['packages'] if p['name']=='flutter')
    sdk=Path(unquote(urlparse(urljoin(config.as_uri(),flutter['rootUri'])).path)).resolve().parents[1]
    for p in data['packages']:
        absolute=urljoin(config.as_uri(),p['rootUri'])
        p['rootUri']=(WORK/'client').as_uri()+'/' if p['name']=='smith_client' else absolute
    # The root package name is read from its location, not assumed.
    for p in data['packages']:
        if Path(unquote(urlparse(p['rootUri']).path)).resolve()==(ROOT/'client').resolve(): p['rootUri']=(WORK/'client').as_uri()+'/'
    dest=WORK/'client/.dart_tool'; dest.mkdir(parents=True,exist_ok=True)
    (dest/'package_config.json').write_text(json.dumps(data))
    dart=sdk/'bin/cache/dart-sdk/bin/dart'; tools=sdk/'bin/cache/flutter_tools.snapshot'
    save('native-toolchain.json',{'dart':file_record(dart),'flutter_tools':file_record(tools),'version':json.loads((sdk/'bin/cache/flutter.version.json').read_text()),'original_package_config':file_record(config),'private_package_config':file_record(dest/'package_config.json'),'note':'Existing SDK/pub caches; no pub/download; private config rewrites only root package URI. No old compiled fixture provided.'})
    env={'CI':'true','DART_SUPPRESS_ANALYTICS':'true','FLUTTER_SUPPRESS_ANALYTICS':'true','FLUTTER_ALREADY_LOCKED':'true','FLUTTER_ROOT':str(sdk),'TMPDIR':str(OUT/'tmp')}
    (OUT/'tmp').mkdir(); (OUT/'home').mkdir()
    env['HOME']=str(OUT/'home')
    fluttercmd=[str(dart),str(tools),'--suppress-analytics','--no-version-check']
    focused=['dev_navigation_test','dev_navigation_golden_test','workspace_preferences_test','workspace_restore_test','agents_navigation_test','projects_navigation_test','compact_workspace_navigation_test','agents_golden_test','ui_redesign_golden_test','workspace_entrance_visual_test','split_workspace_widget_test','shared_header_test','shell_pure_widgets_test','shell_action_authority_test','chat_scroll_anchor_test','workspace_entrance_test','queue_exit_guard_test','new_window_test','message_queue_assurance_test','message_queue_controller_test','message_queue_remote_outcome_test','message_queue_ui_assurance_test','message_queue_widget_test']
    results=[]
    results.append(run('focused',fluttercmd+['test','--no-pub','--reporter','expanded']+['test/'+n+'.dart' for n in focused],300,cwd=WORK/'client',extra=env))
    results.append(run('full-format',[str(dart),'format','--output=none','--set-exit-if-changed','lib','test'],120,cwd=WORK/'client',extra=env))
    results.append(run('full-analyzer',fluttercmd+['analyze','--no-pub'],180,cwd=WORK/'client',extra=env))
    results.append(run('full-native-tests',fluttercmd+['test','--no-pub','--reporter','expanded'],600,cwd=WORK/'client',extra=env))
    save('native-results.json',{'commands':results,'summaries':{r['name']:test_summary(r['name']) for r in results},'note':'Native fixture-dependent skips are not acceptance; configured Dagger client/test is separate.'})

def gates():
    results=[]
    assert digest(CLI)==CLI_SHA256
    results.append(run('dagger-version',[str(CLI),'version'],15))
    results.append(run('engine-readiness',['docker','inspect','--format','{{.State.Running}} {{.Config.Image}}',ENGINE],15))
    ok=all(r['status']=='pass' for r in results) and (OUT/'engine-readiness.log').read_text().strip()=='true registry.dagger.io/engine:v0.21.9'
    reason=None if ok else 'Verified CLI / existing engine readiness failed; no fallback/engine changes authorized'
    make=['make','DAGGER='+str(CLI)]
    root=run('root-test-build',make+['test','build'],900,cwd=WORK,skip_reason=reason);results.append(root)
    # Root Make stops on a failing test, so independently reach the required Linux build and compiled client gate.
    if root['status']!='pass':
        results.append(run('client-build',make+['client/build'],600,cwd=WORK,skip_reason=reason))
        results.append(run('client-test',make+['client/test'],600,cwd=WORK,skip_reason=reason))
    package_reason=None if root['status']=='pass' else 'NOT RUN: root test/build did not complete; old host exports are stale and private standalone client export is not a full package'
    results.append(run('package-verify',['python3','-B','dagger/package.py','--root','installer/dist','--verify'],60,cwd=WORK,skip_reason=package_reason))
    results.append(run('actual-package-helpers',['python3','-B','-m','unittest','discover','-s','scripts/tests','-p','test_*.py','-v'],120,cwd=WORK,extra={'SMITH_TEST_PACKAGE':str(WORK/'installer/dist')},skip_reason=package_reason))
    save('gates-results.json',{'commands':results,'summaries':{r['name']:test_summary(r['name']) for r in results},'cache_policy':'Fresh invocation with existing approved engine/caches; internal CACHED vertices retained in raw logs; no cold-cache or hermeticity claim. Root Make invoked once, no fixes/rerun.'})

if __name__=='__main__':
    {'native':native,'gates':gates}[sys.argv[1]]()
