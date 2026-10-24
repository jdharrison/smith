import pathlib,json,hashlib,datetime,subprocess,os,sys
R=pathlib.Path(__file__).resolve().parents[4]; O=pathlib.Path(__file__).parent; A=O.parent
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def inputs():
 return {str(p.relative_to(R)):sha(p) for tree in ['client/lib','client/test','service/src','service/tests'] for p in (R/tree).rglob('*') if p.is_file()} | {str(p.relative_to(R)):sha(p) for p in [A/'duplicate_reply_test.dart',O/'parser_adversarial_test.dart',O/'run.py']}
original={str(p.relative_to(A)):sha(p) for p in [A/'duplicates.log',A/'duplicates.json',A/'duplicate_reply_test.dart',A/'evidence-index.json']}
old=json.loads((A/'evidence-index.json').read_text())['review_artifacts']
assert all(original[p]==old[p] for p in ['duplicates.log','duplicates.json','duplicate_reply_test.dart'])
sdk=pathlib.Path.home()/'snap/flutter/common/flutter'; env=dict(os.environ,CI='true',DART_SUPPRESS_ANALYTICS='true',FLUTTER_ALREADY_LOCKED='true',FLUTTER_ROOT=str(sdk))
base=[str(sdk/'bin/cache/dart-sdk/bin/dart'),str(sdk/'bin/cache/flutter_tools.snapshot'),'--suppress-analytics','--no-version-check','test','--no-pub','--reporter','expanded']
groups=[('unchanged-repro',[str(A/'duplicate_reply_test.dart')]),('parser-edge-cases',['test/app_reply_json_test.dart',str(O/'parser_adversarial_test.dart')]),('neighbors',['test/app_api_test.dart','test/projects_protocol_test.dart','test/projects_controller_test.dart','test/projects_view_test.dart','test/projects_navigation_test.dart','test/cloud_protocol_test.dart','test/cloud_routing_test.dart'])]
records=[]
safe=lambda s:s.replace(str(R),'<repo>').replace(str(pathlib.Path.home()),'<home>')
for name,tests in groups:
 assert not (O/(name+'.json')).exists(), 'Never overwrite an evidence run'
 argv=base+tests; before=inputs(); started=datetime.datetime.now(datetime.timezone.utc).isoformat()
 try:
  p=subprocess.run(argv,cwd=R/'client',env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=150); code=p.returncode; output=p.stdout
 except subprocess.TimeoutExpired as e:
  code=124; output=e.stdout or b''; output=output.decode(errors='replace') if isinstance(output,bytes) else output
 log=safe(output); (O/(name+'.log')).write_text(log); after=inputs()
 rec={'started':started,'finished':datetime.datetime.now(datetime.timezone.utc).isoformat(),'argv':[safe(v) for v in argv],'cwd':'client','environment':{k:safe(env[k]) for k in ['CI','DART_SUPPRESS_ANALYTICS','FLUTTER_ALREADY_LOCKED','FLUTTER_ROOT']},'exit_code':code,'source_before':before,'source_after':after,'stable_inputs':before==after,'log_sha256':sha(O/(name+'.log')),'original_failure_artifacts':original,'original_artifacts_unchanged':all(sha(A/k)==v for k,v in original.items())}
 (O/(name+'.json')).write_text(json.dumps(rec,indent=2)+'\n'); records.append(rec); print(log); print('RESULT',name,code,'stable',before==after,flush=True)
index={'results':[{k:r[k] for k in ['argv','exit_code','stable_inputs','log_sha256','original_artifacts_unchanged']} for r in records],'artifact_hashes':{p.name:sha(p) for p in O.iterdir() if p.is_file()}}
(O/'index.json').write_text(json.dumps(index,indent=2)+'\n')
sys.exit(0 if all(r['exit_code']==0 and r['stable_inputs'] and r['original_artifacts_unchanged'] for r in records) else 1)
