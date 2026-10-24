#!/usr/bin/env python3
"""Correct the observed cross-snapshot Cargo artifact reuse without changing sources."""
from recorder import *
from run_validation import CAND, CONTROL
import re

save('negative-control-initial-attempt-invalid.json', {
    'reason':'Shared Cargo target reused candidate unit-test executable: control log says 29 tests and lists seven bootstrap tests absent from archive control source. Initial control run is not baseline evidence.',
    'initial_log':file_record(OUT/'negative-control-projects.log'),
    'control_source':file_record(CONTROL/'service/src/app_projects_tests.rs'),
    'control_contains_bootstrap_tests':'fn projects_bootstrap' in (CONTROL/'service/src/app_projects_tests.rs').read_text(),
    'remedy':'New recorded attempt with a distinct initially absent target directory, same test/clippy arguments and unchanged overlaid sources. No clean/delete, repairs or suppression.'})
env={'CARGO_TARGET_DIR':str(OUT/'negative-control-cargo-target')}
if (OUT/'negative-control-cargo-target').exists(): raise RuntimeError('Expected fresh control target')
rows=[]
for name,args in [('negative-control-isolated-projects',['test','--lib','projects_']),('negative-control-isolated-all-target-clippy',['clippy','--all-targets','--','-D','warnings'])]:
    rows.append(run(name,['cargo',args[0],'--manifest-path','Cargo.toml','--locked','--offline',*args[1:]],300,cwd=CONTROL/'service',extra=env))

def signatures(name):
    text=(OUT/(name+'.log')).read_text()
    parts=re.split(r'^---- (.*?) stdout ----\n',text,flags=re.M)
    found={}
    for i in range(1,len(parts),2):
        body=parts[i+1].split('\nfailures:\n')[0]
        body=re.sub(r"thread '.*?' \(\d+\) panicked at [^\n]+:\n",'',body)
        body='\n'.join(l for l in body.strip().splitlines() if not l.startswith('note: run with'))
        found[parts[i]]=body
    return found
candidate=signatures('rust-projects'); baseline=signatures('negative-control-isolated-projects')
save('negative-control-comparison.json',{'candidate':candidate,'control':baseline,'same_failure_names':candidate.keys()==baseline.keys(),'exact_normalized_failure_bodies_equal':candidate==baseline,'normalization':'Remove thread/pid/source line locations and RUST_BACKTRACE note only; retain assertion/error/projection bytes.','commands':rows,'summaries':{r['name']:test_summary(r['name']) for r in rows},'initial_shared_target_attempt_valid':False})
