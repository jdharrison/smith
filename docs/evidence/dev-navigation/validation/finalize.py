import hashlib, io, json, re, shutil, tarfile
from PIL import Image, ImageChops
from recorder import *
from validate import WORK

before=json.loads((OUT/'core-manifest-before.json').read_text())
after=manifest(); save('core-manifest-final.json',after)
baseline=json.loads((ROOT/'target/dev-navigation/baseline-manifest.json').read_text())
bmap={r['path']:r for r in baseline['files']}
changed=[r['path'] for r in before['files'] if r['path'] in bmap and r.get('sha256')!=bmap[r['path']]['sha256']]
prod=[n for n in changed if n.startswith(('client/lib/','service/src/','installer/src/','dagger/src/'))]
snapshot_mismatches=[r['path'] for r in before['files'] if file_record(WORK/r['path']).get('sha256')!=r.get('sha256')]
save('final-source-binding.json',{'recorded_at':now(),'head':git('rev-parse','HEAD').decode().strip(),'branch':git('branch','--show-current').decode().strip(),'baseline_canonical_sha256':canonical_sha(baseline['files']),'baseline_archive':file_record(ROOT/'target/dev-navigation/baseline-source.tar.gz'),'source_snapshot_archive':file_record(OUT/'source-snapshot.tar.gz'),'core_before':before['sha256'],'core_final':after['sha256'],'core_count':after['count'],'untracked_count':after['untracked_count'],'core_stable':before==after,'core_changes':source_delta(before,after),'private_snapshot_core_mismatches':snapshot_mismatches,'production_changes_from_baseline':prod,'all_core_changes_from_baseline':changed,'policy_note':'Parent-owned docs may change concurrently and are excluded from executable core; copied policy bytes bound in snapshot-binding.json. No production/test/golden/policy fix or original evidence overwrite performed.'})

frozen=[]
for r in baseline['files']:
    if r['path'].endswith('.png') and r['path'].startswith(('client/test/goldens/agents/','client/test/goldens/projects-layout/')):
        frozen.append({'path':r['path'],'baseline_sha256':r['sha256'],'current_sha256':digest(ROOT/r['path']),'unchanged':digest(ROOT/r['path'])==r['sha256']})
visual=[]
prior=tarfile.open(ROOT/'target/dev-navigation/ui/editor-golden-failures.tar.gz')
for stem in ['wide-1536x1024','compact-editor-400x400','ultrawide-4200x2400']:
    expected=WORK/'client/test/failures'/ (stem+'_masterImage.png')
    actual=WORK/'client/test/failures'/ (stem+'_testImage.png')
    a=Image.open(expected).convert('RGBA'); b=Image.open(actual).convert('RGBA')
    mask=ImageChops.difference(a,b); bands=mask.split(); merged=bands[0]
    for band in bands[1:]: merged=ImageChops.lighter(merged,band)
    pixels=sum(1 for v in merged.getdata() if v)
    old=next((m for m in prior.getmembers() if Path(m.name).name==actual.name),None)
    old_sha=hashlib.sha256(prior.extractfile(old).read()).hexdigest() if old else None
    name='client/test/goldens/projects-layout/'+stem+'.png'
    visual.append({'golden':name,'baseline_sha256':bmap[name]['sha256'],'current_sha256':digest(ROOT/name),'expected':file_record(expected),'actual':file_record(actual),'changed_pixels':pixels,'changed_bbox_half_open':merged.getbbox(),'prior_failure_actual_sha256':old_sha,'matches_preserved_prevalidation_failure_actual':old_sha==digest(actual),'note':'No golden regenerated. Baseline expected bytes proven unchanged; prior actual equality, when true, proves failure render was present in UI handoff, not arbitrary historical baseline blame.'})
with tarfile.open(OUT/'current-visual-failures.tar.gz','w:gz') as t:
    for p in sorted((WORK/'client/test/failures').rglob('*')):
        if p.is_file():t.add(p,arcname=p.name)
ledger_path=ROOT/'docs/evidence/dev-navigation/ui/visual-rebase/final-golden-ledger.json'
ledger=json.loads(ledger_path.read_text())
shell=[{'path':r['golden'],'baseline_sha_matches':bmap[r['golden']]['sha256']==r['old_sha256'],'current_sha_matches':digest(ROOT/r['golden'])==r['actual_sha256'],'claimed_outside_rail_changed_pixels':r['outside_rail_changed_pixels']} for r in ledger['images']]
save('visual-binding.json',{'frozen_editor_count':len(frozen),'all_frozen_unchanged':all(r['unchanged'] for r in frozen),'frozen_editor_images':frozen,'current_failures':visual,'failure_archive':file_record(OUT/'current-visual-failures.tar.gz'),'prior_failure_archive':file_record(ROOT/'target/dev-navigation/ui/editor-golden-failures.tar.gz'),'shell_ledger':file_record(ledger_path),'shell_current_bindings':shell,'note':'Owner pixel geometry proof is bound by hash, not relabelled as independent assurance. Current normal-comparator shell suites passed within focused305.'})

artifact=record_tree('linux-client-export.json','target/dev-navigation/validation/source/client/dist')
critical=[p for p in (WORK/'client/dist').rglob('*') if p.is_file() and (p.name=='smith_client' or p.name=='smith-service')]
run('linux-artifact-file',['file',*[str(p) for p in critical]],15)
run('diff-check',['git','--no-pager','diff','--check'],15)
records=[]
for p in sorted(OUT.glob('*.json')):
    value=json.loads(p.read_text())
    if isinstance(value,dict) and 'name' in value and 'exit_code' in value:records.append(value)

summaries={}
for name in ['focused-safe-temp','full-native-tests-safe-temp','client-test-configured-module']:
    text=(OUT/(name+'.log')).read_text()
    # Failure output is replayed by Dagger; only the final terminal summary is authoritative for this invocation.
    found=re.findall(r'\+(\d+)(?:\s+~(\d+))?(?:\s+-(\d+))?:\s+(All tests passed!|Some tests failed\.)',text)
    result=found[-1]
    summaries[name]={'passed':int(result[0]),'skipped':int(result[1] or 0),'failed':int(result[2] or 0),'terminal':result[3],'count_policy':'One final Flutter terminal summary for this invocation; no addition across reruns or Dagger failure replay.'}
    failures={}
    for line in text.splitlines():
        m=re.search(r'(/src/test/|<repo>/target/dev-navigation/validation/source/client/test/)(.+?\.dart): (.+?) \[E\]',line)
        if m:failures[m.group(2)+': '+m.group(3)]=True
    summaries[name]['failures']=sorted(failures)
    summaries[name]['explicit_skip_reasons']=list(dict.fromkeys(line.strip() for line in text.splitlines() if 'Skip:' in line))
log=(OUT/'client-test-configured-module.log').read_text()
cache_lines=[line for line in log.splitlines() if re.search(r'^\d+\s+:',line) and any(s in line for s in ['CACHED','withExec cargo test','withExec sh scripts/build.sh','withExec sh scripts/test.sh','Downloading crates','Compiling smith','Finished `test`'])]
save('observed-gates.json',{'test_invocations':summaries,'cache_observations':cache_lines,'debug_fixture':'Configured current-source cargo test --locked --test debug_service --no-run executed (vertex15); selected private executable injected as /opt/smith-cloud-fixture, not an old host fixture. Internal fixture not exported/hash unavailable; raw execution and source bindings retained.','release_helper':'Configured current-source build dependency uses Dagger content-addressed cache where logged CACHED. Exported helper SHA256 recorded in linux-client-export.json. Not a cold/rebuilt-every-vertex claim.','root_once':{'invocations':1,'status':'BLOCKED at private copied module discovery, unknown command test','not_reached':['service/test','client/test within aggregate','installer/test','package/test','all root build prerequisites'],'rerun':False},'package_verify_and_actual_helpers':'NOT RUN: no completed current full package; no old exports validated','timeouts':[r['name'] for r in records if r['timed_out']],'canceled':[],'not_run':[{'name':r['name'],'reason':r['skip_reason']} for r in records if r['status']=='not_run']})
save('final-results.json',{'recorded_at':now(),'work_id':'LOCAL-SMITH-DEV-NAVIGATION','implemented_validation_only':True,'focused':'PASS:305 passed,0 failed,0 skipped in one invocation','format':'PASS:111 files,0 changed','analyzer':'PASS:no issues','linux_build':'PASS:current configured module/client/build,private exported bundle; not full package','combined':'BLOCKED','full_native':summaries['full-native-tests-safe-temp'],'full_compiled_client':summaries['client-test-configured-module'],'source_stable':before==after,'core_sha256':after['sha256'],'core_count':after['count'],'untracked_count':after['untracked_count'],'source_snapshot_core_matches':not snapshot_mismatches,'production_changes_from_baseline':prod,'warnings':'Analyzer clean; required test failures remain','not_claimed':['acceptance','waiver','independent assurance','full root success','full package rebuild/verify','installed/GUI/native IME acceptance','cold cache/hermeticity','historical baseline blame outside frozen byte proof'],'commands':[{'name':r['name'],'status':r['status'],'exit_code':r['exit_code'],'timeout_seconds':r['timeout_seconds'],'timed_out':r['timed_out'],'stable_inputs':r['stable_inputs'],'log_sha256':r['log_sha256']} for r in records]})
print('FINAL',json.dumps({'core_stable':before==after,'core_sha256':after['sha256'],'count':after['count'],'untracked':after['untracked_count'],'production_changes':prod,'snapshot_mismatches':snapshot_mismatches,'counts':{k:{n:v[n] for n in ('passed','skipped','failed')} for k,v in summaries.items()},'visual':[{k:r[k] for k in ('golden','changed_pixels','matches_preserved_prevalidation_failure_actual')} for r in visual]}),flush=True)
