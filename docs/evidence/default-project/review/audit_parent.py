#!/usr/bin/env python3
import importlib.util,json,pathlib,re,tarfile
s=importlib.util.spec_from_file_location('audit_inspect',pathlib.Path('docs/evidence/default-project/review/inspect.py')); a=importlib.util.module_from_spec(s); s.loader.exec_module(a)
P=a.R/'docs/evidence/default-project/validation'; rows=[]; mismatches=[]
for line in (P/'SHA256SUMS').read_text().splitlines():
    h,n=line.split('  ',1); actual=a.sha((P/n).read_bytes()); rows.append({'path':n,'sha256':actual});
    if actual!=h: mismatches.append(n)
commands=[]
for f in sorted(P.glob('*.json')):
    v=json.loads(f.read_text())
    if isinstance(v,dict) and 'log_sha256' in v and 'argv' in v:
        log=P/(v['name']+'.log'); actual=a.sha(log.read_bytes())
        commands.append({'name':v['name'],'exit':v.get('exit_code'),'status':v.get('status'),'log_matches':actual==v['log_sha256'],'core_before':v.get('core_before_sha256'),'core_after':v.get('core_after_sha256')})
def sig(f):
    text=f.read_text(); parts=re.split(r'^---- (.*?) stdout ----\n',text,flags=re.M); found={}
    for i in range(1,len(parts),2):
        body=parts[i+1].split('\nfailures:\n')[0]
        body=re.sub(r"thread '.*?' \(\d+\) panicked at [^\n]+:\n",'',body)
        found[parts[i]]='\n'.join(l for l in body.strip().splitlines() if not l.startswith('note: run with'))
    return found
ours=sig(a.O/'projects-final-sandbox.log'); candidate=sig(P/'rust-projects.log'); control=sig(P/'negative-control-isolated-projects.log')
control_root=a.R/'target/default-project/validation/negative-control'
base=tarfile.open(a.R/'target/default-project/baseline-source.tar.gz')
overlay=['service/src/app.rs','service/src/app_projects.rs','service/src/app_projects_tests.rs']
core=json.loads((a.O/'core-before.json').read_text()); control_mismatches=[]
for x in core['files']:
    rel=x['path']
    if x['type']!='file' or not rel.startswith('service/'): continue
    expected=base.extractfile(rel).read() if rel in overlay else (a.R/rel).read_bytes()
    if a.sha((control_root/rel).read_bytes())!=a.sha(expected): control_mismatches.append(rel)
artifacts={}
for rel in ['target/default-project/validation/debug_service-fixture','target/default-project/validation/candidate-core.tar.gz']:
    artifacts[rel]=a.sha((a.R/rel).read_bytes())
a.save('parent-audit.json',{'checksummed_files':len(rows),'checksum_mismatches':mismatches,'commands':commands,'independent_failure_names':list(ours),'independent_equals_parent_failure_bodies':ours==candidate,'candidate_equals_valid_control_failure_bodies':candidate==control,'current_control_overlay_mismatches':control_mismatches,'artifacts':artifacts,'retained_checksum_index_sha256':a.sha((P/'SHA256SUMS').read_bytes())})
a.save('core-final.json',a.capture())
print(json.dumps({'checksummed_files':len(rows),'checksum_mismatches':mismatches,'bad_command_logs':[x['name'] for x in commands if not x['log_matches']],'independent_equals_parent':ours==candidate,'candidate_equals_control':candidate==control,'control_overlay_mismatches':control_mismatches,'final_core':a.capture()['sha256']},indent=2))
