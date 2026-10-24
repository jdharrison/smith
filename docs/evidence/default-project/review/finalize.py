#!/usr/bin/env python3
import datetime, importlib.util, json, pathlib
s=importlib.util.spec_from_file_location('audit_inspect',pathlib.Path('docs/evidence/default-project/review/inspect.py')); a=importlib.util.module_from_spec(s); s.loader.exec_module(a)
core=a.capture(); before=json.loads((a.O/'core-before.json').read_text()); assert core==before
paths=['AGENTS.md','roles/assurance.md','docs/plans/LOCAL-SMITH-DEFAULT-PROJECT.md','docs/SCOPE.md','docs/SCOPE_DETAILS.md','docs/ARCHITECTURE.md','docs/ARCHITECTURE_DETAILS.md','docs/PROCESS.md','docs/DESIGN.md','docs/HOST_PROFILE.md','docs/HOST_PROFILE_DETAILS.md','docs/SMITH_LINUX_ARCHITECTURE_REPORT.md','service/src/app.rs','service/src/app_projects.rs','service/src/app_projects_tests.rs','service/tests/debug_service.rs','client/test/rust_projects_integration_test.dart','service/src/app_v2.rs','service/src/filesystem.rs']
records=[{'path':p,'sha256':a.sha((a.R/p).read_bytes()),'size':(a.R/p).stat().st_size} for p in paths]
a.save('source-identities.json',{'at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'core_sha256':core['sha256'],'count':core['count'],'untracked_count':core['untracked_count'],'stable_since_initial_review':True,'files':records,'selected_records_canonical_sha256':a.canonical(records)})
a.save('core-final.json',core)
entries=[]
for f in sorted(a.O.glob('*.json')):
    v=json.loads(f.read_text())
    if 'argv' not in v: continue
    entries.append('## '+f.stem+'\n\n```text\n'+json.dumps(v['argv'])+'\n```\n\n- cwd: `'+v['cwd']+'`\n- exit: `'+str(v['exit'])+'`; timeout: `'+str(v['timed_out'])+'`; duration: '+str(round(v['duration_seconds'],3))+'s\n- [Full record]('+f.name+') / [sanitized log]('+f.stem+'.log)\n')
(a.O/'COMMANDS.md').write_text('# Independent assurance command ledger\n\nExact argv/environment/time/budget/exit, log digest, and core identity are retained in linked JSON. No failed attempt is discarded. See ../REVIEW.md for harness corrections and pass/fail interpretation.\n\n'+'\n'.join(entries))
files=sorted(f for f in a.O.iterdir() if f.is_file() and f.name!='SHA256SUMS')
(a.O/'SHA256SUMS').write_text(''.join(a.sha(f.read_bytes())+'  '+f.name+'\n' for f in files)+a.sha((a.O.parent/'REVIEW.md').read_bytes())+'  ../REVIEW.md\n')
print(json.dumps({'final_core_sha256':core['sha256'],'selected_records_sha256':a.canonical(records),'review_sha256':a.sha((a.O.parent/'REVIEW.md').read_bytes()),'review_checksum_index_sha256':a.sha((a.O/'SHA256SUMS').read_bytes()),'evidence_files':len(files)},indent=2))
