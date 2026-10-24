import json, shutil
from recorder import *
from validate import WORK
p=ROOT/'client/.dart_tool/package_graph.json'
shutil.copy2(p,WORK/'client/.dart_tool/package_graph.json')
save('private-config-correction.json',{'recorded_at':now(),'copied':file_record(p),'private_copy':file_record(WORK/'client/.dart_tool/package_graph.json'),'reason':'Initial native tests stopped before execution: missing generated package_graph.json. No product source changed. Failed attempts retained; explicit separately named retries.'})
metadata=json.loads((OUT/'focused.started.json').read_text())
def restore(v):
    return v.replace('<repo>',str(ROOT)).replace('<home>',str(Path.home()))
argv=[restore(v) for v in metadata['argv']]
env={k:restore(v) for k,v in metadata['environment'].items()}
results=[run('focused-configured',argv,300,cwd=WORK/'client',extra=env)]
base=argv[:argv.index('test')]
results.append(run('full-native-tests-configured',base+['test','--no-pub','--reporter','expanded'],600,cwd=WORK/'client',extra=env))
save('native-configured-results.json',{'commands':results,'summaries':{r['name']:test_summary(r['name']) for r in results},'note':'No compiled fixture env: integration skips must not be counted as acceptance. Configured Dagger client/test separately supplies current-source compiled fixtures.'})
