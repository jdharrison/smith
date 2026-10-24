"""Only permitted preference rerun, in a new private fixture."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import time
import urllib.parse

R=Path.cwd()
O=R/'target/dev-navigation/review-closeout'
E=R/'docs/evidence/dev-navigation/review-closeout'
W=O/'fixture/client'
source=R/'target/dev-navigation/assurance/fixture/client'
assert not W.exists()
shutil.copytree(source,W,ignore=shutil.ignore_patterns('build','test','failures'))
(W/'test').mkdir()
shutil.copyfile(R/'client/test/workspace_preferences_test.dart',W/'test/workspace_preferences_test.dart')
manifest=json.loads((E/'source-current.json').read_text())
bound=[]
for row in manifest['files']:
    if row['path'].startswith('client/lib/') or row['path'] in ['client/pubspec.yaml','client/pubspec.lock','client/test/workspace_preferences_test.dart']:
        p=W/Path(row['path']).relative_to('client')
        assert hashlib.sha256(p.read_bytes()).hexdigest()==row['sha256'],row['path']
        bound.append(row['path'])
pkg=json.loads((W/'.dart_tool/package_config.json').read_text())
flutter=next(p for p in pkg['packages'] if p['name']=='flutter')
SDK=Path(urllib.parse.unquote(flutter['rootUri'].removeprefix('file://'))).parents[1]
DART=SDK/'bin/cache/dart-sdk/bin/dart'
TOOL=SDK/'bin/cache/flutter_tools.snapshot'
(O/'home').mkdir()
env={'HOME':str(O/'home'),'TMPDIR':'/tmp','PATH':'/usr/bin:/bin','LANG':'C.UTF-8','LC_ALL':'C.UTF-8','CI':'true','FLUTTER_ROOT':str(SDK),'FLUTTER_ALREADY_LOCKED':'true','FLUTTER_SUPPRESS_ANALYTICS':'true','DART_SUPPRESS_ANALYTICS':'true','DO_NOT_TRACK':'1','DNT':'1','GIT_OPTIONAL_LOCKS':'0'}
def safe(s):
    return s.replace(str(R),'<repo>').replace(str(SDK),'<cached-flutter-sdk>').replace(str(Path.home()),'<home>')
cmd=[str(DART),str(TOOL),'--suppress-analytics','--no-version-check','test','--no-pub','--reporter','expanded','test/workspace_preferences_test.dart']
start=time.monotonic()
stamp=datetime.datetime.now(datetime.timezone.utc).isoformat()
p=subprocess.Popen(cmd,cwd=W,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,start_new_session=True)
try:
    output=p.communicate(timeout=120)[0]
    code=p.returncode
except subprocess.TimeoutExpired:
    os.killpg(p.pid,signal.SIGKILL)
    output=p.communicate()[0]
    code=124
text=safe(output.decode(errors='replace'))
with (E/'preferences.log').open('x') as f:
    f.write(text)
rec=dict(command=[safe(v) for v in cmd],cwd='target/dev-navigation/review-closeout/fixture/client',environment={k:safe(v) for k,v in env.items()},started_utc=stamp,duration_seconds=round(time.monotonic()-start,3),exit=code,log_sha256=hashlib.sha256(text.encode()).hexdigest(),source_bound_files=bound,core_sha256=manifest['sha256'],fixture_policy='Copied new fixture only; /tmp routine private system-temp fixtures; no host directory permission changes; no prior reviewer writes')
with (E/'preferences.json').open('x') as f:
    f.write(json.dumps(rec,indent=2,sort_keys=True)+'\n')
print('preferences rerun exit',code,'seconds',rec['duration_seconds'],'bound files',len(bound))
print('\n'.join(text.splitlines()[-3:]))
