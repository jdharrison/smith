"""Run retained, unchanged reviewer repros against the bound current client."""
import datetime,hashlib,json,os,pathlib,subprocess,sys
R=pathlib.Path(__file__).resolve().parents[4]; O=pathlib.Path(__file__).parent
output=O/'records/retained-repros.json'; assert not output.exists(), 'Do not overwrite an evidence run'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
parents=[p for p in O.parent.iterdir() if p.is_file() and p.name!='REVIEW.md']
paths=[R/json.loads(line)['path'] for file in ['core-source-manifest.jsonl','supporting-inputs.jsonl'] for line in (O.parent/file).read_text().splitlines() if line]
tests=[O/'repros/duplicate_reply_test.dart',O/'parser-followup-1/parser_adversarial_test.dart']
inputs=lambda:{str(p.relative_to(R)):sha(p) for p in paths+parents+tests}
before=inputs(); sdk=pathlib.Path.home()/'snap/flutter/common/flutter'; env=dict(os.environ,CI='true',DART_SUPPRESS_ANALYTICS='true',FLUTTER_ALREADY_LOCKED='true',FLUTTER_ROOT=str(sdk))
argv=[str(sdk/'bin/cache/dart-sdk/bin/dart'),str(sdk/'bin/cache/flutter_tools.snapshot'),'--suppress-analytics','--no-version-check','test','--no-pub','--reporter','expanded',*[str(p) for p in tests]]
start=datetime.datetime.now(datetime.timezone.utc).isoformat()
try:
 p=subprocess.run(argv,cwd=R/'client',env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=120); code=p.returncode; text=p.stdout
except subprocess.TimeoutExpired as e:
 code=124; text=e.stdout or b''; text=text.decode(errors='replace') if isinstance(text,bytes) else text
safe=lambda s:s.replace(str(R),'<repo>').replace(str(pathlib.Path.home()),'<home>')
log=safe(text); (O/'records/retained-repros.log').write_text(log); after=inputs(); rec={'command':[safe(v) for v in argv],'cwd':'client','started':start,'finished':datetime.datetime.now(datetime.timezone.utc).isoformat(),'exit_code':code,'inputs_before':before,'inputs_after':after,'stable_inputs':before==after,'log_sha256':sha(O/'records/retained-repros.log')}; output.write_text(json.dumps(rec,indent=2)+'\n'); print(log); print('exit',code,'stable_inputs',before==after); sys.exit(0 if code==0 and before==after else 1)
