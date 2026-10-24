#!/usr/bin/env python3
import difflib,importlib.util,pathlib
s=importlib.util.spec_from_file_location('sandbox',pathlib.Path('docs/evidence/default-project/review/run_final_sandbox.py')); p=importlib.util.module_from_spec(s); s.loader.exec_module(p)
m=p.p.m; a=m.a
changes=[]
for rel in ['service/src/app_projects_tests.rs','service/src/filesystem.rs']:
    f=m.T/'candidate'/rel; original=f.read_text()
    if rel.endswith('app_projects_tests.rs'):
        modified=original+(m.O/'adversarial_tests.rs').read_text()
    else:
        marker='        File::open(parent)\n'
        assert original.count(marker)==1
        modified=original.replace(marker,'        #[cfg(test)]\n        if ASSURANCE_POST_LINK_FAILURE.with(|flag| flag.get()) {\n            return Err("ASSURANCE injected directory sync failure".into());\n        }\n'+marker)
        modified+='\n#[cfg(test)]\nthread_local! { pub(crate) static ASSURANCE_POST_LINK_FAILURE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }\n'
    f.write_text(modified)
    changes.append({'path':rel,'original_sha256':a.sha(original.encode()),'probe_sha256':a.sha(modified.encode())})
    (m.O/(f.name+'.probe.diff')).write_text(''.join(difflib.unified_diff(original.splitlines(True),modified.splitlines(True),fromfile='original/'+rel,tofile='isolated-probe/'+rel)))
a.save('probe-binding.json',{'changes':changes,'note':'Only isolated private copy changed. Fault hook returns an error after successful hard-link/unlink, before directory sync. Synthetic failure, not a real disk fault.'})
# Changed copied source recompiles in the already independently populated assurance target.
m.run('adversarial',p.p.prefix+['cargo','test','--locked','--offline','--lib','assurance_','--','--test-threads=1'])
a.save('core-after-adversarial.json',a.capture())
