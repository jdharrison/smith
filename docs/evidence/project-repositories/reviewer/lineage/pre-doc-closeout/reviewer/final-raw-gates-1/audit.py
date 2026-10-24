import pathlib,json,hashlib,stat,os,re,subprocess,datetime
R=pathlib.Path(__file__).resolve().parents[4]; O=pathlib.Path(__file__).parent; A=R/'target/project-repositories/attempt-2'
sha=lambda b:hashlib.sha256(b).hexdigest()
read=lambda p:json.loads(p.read_text())
safe=lambda s:s.replace(str(R),'<repo>').replace(str(pathlib.Path.home()),'<home>')
def file_record(p):
 mode=p.lstat().st_mode; b=os.readlink(p).encode() if stat.S_ISLNK(mode) else p.read_bytes()
 return {'path':str(p.relative_to(R)),'mode':stat.S_IMODE(mode),'sha256':sha(b),'size':len(b)}
def digest(records): return sha(json.dumps(records,sort_keys=True,separators=(',',':')).encode())
before=read(A/'core-manifest-before.json'); after=read(A/'core-manifest-after.json'); result=read(A/'results.json')
current=[file_record(R/f['path']) for f in before['files']]
raw_paths=subprocess.check_output(['git','--no-pager','ls-files','--cached','--others','--exclude-standard','-z','--','service','client','installer','dagger','Makefile','rust-toolchain.toml','dagger.toml','dagger.lock'],cwd=R).split(b'\0')
paths=sorted(set(os.fsdecode(p) for p in raw_paths if p))
records={'started':datetime.datetime.now(datetime.timezone.utc).isoformat(),'parent_results_passed':result['passed'],'core':{'before_matches_after':before==after,'count':len(before['files']),'digest_recomputed':digest(before['files']),'claimed_digest':before['sha256'],'current_matches_before':current==before['files'],'current_digest':digest(current),'enumerated_paths_match':paths==[f['path'] for f in before['files']]},'raw_artifact_sha256':{str(p.relative_to(R)):sha(p.read_bytes()) for p in sorted(A.iterdir()) if p.is_file()}}
text=re.sub(r'\x1b\[[0-?]*[ -/]*[@-~]','',(A/'root-gates.log').read_text()); lines=text.splitlines(); test_phase=text.split('SmithNext.test DONE')[0]
summaries={}
for v in ['29','62','76']:
 selected=[l for l in test_phase.splitlines() if re.match(rf'{v}\s*:',l) and 'test result:' in l]
 counts=[tuple(map(int,re.search(r'(\d+) passed; (\d+) failed; (\d+) ignored',l).groups())) for l in selected]
 summaries[v]={'passed':sum(c[0] for c in counts),'failed':sum(c[1] for c in counts),'ignored':sum(c[2] for c in counts),'summary_lines':selected}
summaries['111']={'passed':int(re.search(r'111\s*:.*?\+(\d+): All tests passed!',test_phase).group(1))}
summaries['125']={'passed':int(re.search(r'125\s*:.*?Ran (\d+) tests',test_phase).group(1))}
records['counts']=summaries; records['aggregate_passed']=sum(x['passed'] for x in summaries.values()); records['test_phase_cache_lines']=[l for l in test_phase.splitlines() if 'CACHED' in l]; records['build_phase_cache_line_count']=sum('CACHED' in l for l in text[len(test_phase):].splitlines()); records['ignore_skip_lines']=[l for l in test_phase.splitlines() if re.search(r'\.\.\. ignored|\bskipped\b|\bSKIP\b',l)]; records['format_analyzer']=[l for l in lines if 'Formatted 90 files' in l or 'No issues found!' in l]
compiled=read(A/'compiled-projects.json'); records['compiled']={'passed':compiled['passed'],'before_matches_after':compiled['inputs_before']==compiled['inputs_after'],'current_hashes_match':all(sha((R/p).read_bytes())==h for p,h in compiled['inputs_before'].items()),'log_digest_matches':sha((A/'compiled-projects.log').read_bytes())==compiled['log_sha256'],'seven_pass_summary':'+7: All tests passed!' in (A/'compiled-projects.log').read_text()}
package=R/'installer/dist'; manifest=read(package/'manifest.json'); payload=[file_record(p) for p in sorted(package.rglob('*')) if p.is_file()]; records['package']={'file_count_in_manifest':len(manifest['files']),'actual_regular_file_count':len(payload),'inventory':payload,'inventory_sha256':digest(payload)}
helpers=[R/p for p in ['service/dist/smith-service','client/dist/smith_backend/smith-service','installer/dist/payload/service/smith-service','installer/dist/payload/client/smith_backend/smith-service']]; records['paired_helpers']=[file_record(p) for p in helpers]; records['paired_helpers_equal']=len({x['sha256'] for x in records['paired_helpers']})==1
records['native_headers']={p: {'elf64_little_endian':(R/p).read_bytes()[:7]==b'\x7fELF\x02\x01\x01','machine':int.from_bytes((R/p).read_bytes()[18:20],'little'),'mode':oct((R/p).stat().st_mode & 0o777)} for p in ['service/dist/smith-service','client/dist/smith_client','installer/dist/smith-installer','installer/dist/smith-installer-ui','installer/dist/payload/client/smith_client','installer/dist/payload/client/lib/libapp.so']}
exported={str(p.relative_to(R/'client/dist')):sha(p.read_bytes()) for p in (R/'client/dist').rglob('*') if p.is_file()}; packaged={str(p.relative_to(package/'payload/client')):sha(p.read_bytes()) for p in (package/'payload/client').rglob('*') if p.is_file()}; records['client_export_equals_package']=exported==packaged
supplemental_paths=subprocess.check_output(['git','--no-pager','ls-files','--cached','--others','--exclude-standard','-z','--','scripts','.github','Makefile','dagger/package.py','dagger/runtime.py'],cwd=R).split(b'\0'); supplemental=[R/os.fsdecode(p) for p in sorted(set(supplemental_paths)) if p]
source_before=[file_record(p) for p in supplemental]; artifact_before=[file_record(p) for p in sorted(package.rglob('*')) if p.is_file()]
commands=[('verify',['python3','-B','dagger/package.py','--root','installer/dist','--verify']),('helpers',['python3','-B','-m','unittest','discover','-s','scripts/tests','-p','test_*.py','-v'])]; runs=[]
for name,argv in commands:
 start=datetime.datetime.now(datetime.timezone.utc).isoformat(); env=dict(os.environ,SMITH_TEST_PACKAGE=str(package)); p=subprocess.run(argv,cwd=R,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=120); log=safe(p.stdout); (O/(name+'.log')).write_text(log); runs.append({'name':name,'argv':argv,'started':start,'finished':datetime.datetime.now(datetime.timezone.utc).isoformat(),'exit_code':p.returncode,'log_sha256':sha(log.encode())}); print(log)
records['supplemental']={'sources_before':source_before,'sources_after':[file_record(p) for p in supplemental],'runs':runs,'package_unchanged':artifact_before==[file_record(p) for p in sorted(package.rglob('*')) if p.is_file()]}
records['supplemental']['stable_sources']=records['supplemental']['sources_before']==records['supplemental']['sources_after']
records['cli_sha256']=sha((R/'target/installer-pipeline/smith-dagger-0.21.9/dagger').read_bytes()); records['cli_matches_parent']=records['cli_sha256']==result['cli_sha256']
records['core_after_independent_checks_matches']=current==[file_record(R/f['path']) for f in before['files']]
for name in ['builder-recovery.json','builder-probe.json','builder-probe.log']:
 p=R/'target/project-repositories'/name; records['raw_artifact_sha256'][str(p.relative_to(R))]=sha(p.read_bytes())
records['finished']=datetime.datetime.now(datetime.timezone.utc).isoformat(); (O/'audit.json').write_text(json.dumps(records,indent=2)+'\n'); print(json.dumps({k:v for k,v in records.items() if k in ['core','aggregate_passed','test_phase_cache_lines','build_phase_cache_line_count','compiled','paired_helpers_equal','client_export_equals_package','cli_matches_parent','core_after_independent_checks_matches']},indent=2))
