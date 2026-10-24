import json, os, sys
from recorder import *
from validate import WORK
if sys.argv[1]=='native':
    meta=json.loads((OUT/'focused.started.json').read_text())
    restore=lambda v:v.replace('<repo>',str(ROOT)).replace('<home>',str(Path.home()))
    argv=[restore(v) for v in meta['argv']]
    env={k:restore(v) for k,v in meta['environment'].items()}
    env['TMPDIR']='/tmp'
    save('isolated-temp-policy.json',{'recorded_at':now(),'reason':'Private fixture under repository path rejected by ancestor safety checks; use system temporary root for explicitly allowed isolated tests. No source/host permission change.','tmp_root':file_record(Path('/tmp')),'prior_failure_retained':True})
    results=[run('focused-safe-temp',argv,300,cwd=WORK/'client',extra=env)]
    base=argv[:argv.index('test')]
    results.append(run('full-native-tests-safe-temp',base+['test','--no-pub','--reporter','expanded'],600,cwd=WORK/'client',extra=env))
    save('native-safe-temp-results.json',{'commands':results,'summaries':{r['name']:test_summary(r['name']) for r in results},'note':'Retain configured attempt failures; missing compiled integration fixtures explicitly skipped and not acceptance.'})
else:
    wrapper=OUT/'dagger_export.py'; wrapper.chmod(0o755)
    save('component-redirect-binding.json',{'recorded_at':now(),'wrapper':file_record(wrapper),'official_cli':file_record(CLI),'original_module':file_record(ROOT/'dagger/dagger.json'),'module_implementation':file_record(ROOT/'dagger/src/smith_next/main.py'),'reason':'Copied module discovery listed example functions; original module discovery lists Smith functions. No root rerun. Original Makefile, module, source, engine unchanged; only output redirect to private validation path.'})
    results=[]
    for name,target in [('client-build-configured-module','client/build'),('client-test-configured-module','client/test')]:
        results.append(run(name,['make','DAGGER='+str(wrapper),target],600))
    save('configured-component-results.json',{'commands':results,'summaries':{r['name']:test_summary(r['name']) for r in results},'root_gate':'Failed original attempt final; no root rerun','package':'NOT RUN; no full rebuilt current package. Standalone client export not full-package acceptance.'})
