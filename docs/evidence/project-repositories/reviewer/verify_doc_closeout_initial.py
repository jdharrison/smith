"""Read-only parent/source verification; writes only new reviewer-owned records."""
import datetime,hashlib,json,os,pathlib,re,stat,sys,tarfile,urllib.parse
R=pathlib.Path(__file__).resolve().parents[4]; D=R/'docs/evidence/project-repositories'; O=D/'reviewer'
phase=sys.argv[1] if len(sys.argv)>1 else 'initial'
output=O/('bundle-verification-doc-closeout-'+phase+'.json')
assert not output.exists(), 'Do not overwrite a prior verification'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
load=lambda p:json.loads(p.read_text())
jsonl=lambda p:[json.loads(l) for l in p.read_text().splitlines() if l]
def record(p):
 m=p.lstat().st_mode; data=os.readlink(p).encode() if stat.S_ISLNK(m) else p.read_bytes()
 return {'path':str(p.relative_to(R)),'mode':stat.S_IMODE(m),'size':len(data),'sha256':hashlib.sha256(data).hexdigest()}
def mismatches(entries): return [f['path'] for f in entries if not (R/f['path']).exists() or record(R/f['path'])!=f]
res=load(D/'results.json'); core=jsonl(D/'core-source-manifest.jsonl'); supporting=jsonl(D/'supporting-inputs.jsonl'); artifacts=load(D/'artifacts.json'); changes=load(D/'changes-from-baseline.json')
check={'started':datetime.datetime.now(datetime.timezone.utc).isoformat(),'command':['python3','-B','docs/evidence/project-repositories/reviewer/verify_doc_closeout.py',phase], 'results_sha256':sha(D/'results.json'),'expected_results_sha256':'fd4696a4d784f1d379fb44d38bc6fafcd1c35fcdba17f415acad827ca438a420','parent_bundle_files':[record(p) for p in sorted(D.iterdir()) if p.is_file() and p.name!='REVIEW.md'],'core_count':len(core),'core_mismatches':mismatches(core),'supporting_count':len(supporting),'supporting_mismatches':mismatches(supporting),'artifact_count':len(artifacts['files']),'artifact_mismatches':mismatches(artifacts['files']),'selected_raw_count':len(res['raw_evidence']),'raw_mismatches':mismatches(res['raw_evidence'])}
canonical=hashlib.sha256(json.dumps(core,sort_keys=True,separators=(',',':')).encode()).hexdigest(); check['core_canonical_sha256']=canonical
checks={'results_expected_hash':check['results_sha256']==check['expected_results_sha256'],'core_digest':canonical==res['source']['canonical_array_sha256'],'core_file_hash':sha(D/'core-source-manifest.jsonl')==res['source']['core_manifest_sha256'],'supporting_file_hash':sha(D/'supporting-inputs.jsonl')==res['source']['supporting_snapshot_sha256'],'changes_file_hash':sha(D/'changes-from-baseline.json')==res['source']['baseline_changes_sha256'],'artifacts_file_hash':sha(D/'artifacts.json')==res['artifacts']['sha256'],'parent_gates_match_raw':res['final_gates']==load(R/'target/project-repositories/attempt-2/results.json'),'compiled_match_raw':res['compiled_projects']==load(R/'target/project-repositories/attempt-2/compiled-projects.json')}
for name in ['before','after']:
 checks['core_matches_gate_'+name]=core==load(R/('target/project-repositories/attempt-2/core-manifest-'+name+'.json'))['files']
base=load(R/res['baseline']['manifest_path']); tar=tarfile.open(R/res['baseline']['archive_path']); bmap={x['path']:x for x in base['files']}; archive_bad=[]; historical=[]
for p,f in bmap.items():
 member=tar.getmember(p); data=tar.extractfile(member).read()
 if hashlib.sha256(data).hexdigest()!=f['sha256'] or len(data)!=f['size'] or stat.S_IMODE(member.mode)!=f['mode']: archive_bad.append(p)
 if p.startswith('docs/evidence/projects/'):
  historical.append(p)
check.update(baseline_archive_entries=len(tar.getmembers()),baseline_archive_mismatches=archive_bad,historical_projects_count=len(historical),historical_projects_mismatches=mismatches([bmap[p] for p in historical]))
checks['baseline_archive_hash']=sha(R/res['baseline']['archive_path'])==res['baseline']['archive_sha256']; checks['baseline_manifest_hash']=sha(R/res['baseline']['manifest_path'])==res['baseline']['manifest_file_sha256']; checks['baseline_canonical_identity']=base['sha256']==res['baseline']['canonical_manifest_sha256']
bad_changes=[]
for c in changes['changes']:
 if c.get('before')!=bmap.get(c['path']) or c.get('after')!=record(R/c['path']): bad_changes.append(c['path'])
check['selected_change_count']=len(changes['changes']); check['selected_change_mismatches']=bad_changes
actual_artifacts=[record(p) for tree in ['service/dist','client/dist','installer/dist'] for p in sorted((R/tree).rglob('*')) if p.is_file()]
checks['all_exported_artifacts_enumerated']=actual_artifacts==artifacts['files']; checks['guide_matches_source']=(R/'dagger/INSTALLER.md').read_bytes()==(R/'installer/dist/README.txt').read_bytes()
retained=[]
for name in ['parser-followup-1','final-raw-gates-1']:
 for p in (O/name).rglob('*'):
  if p.is_file(): retained.append((p,R/'target/project-repositories/assurance'/p.relative_to(O)))
for dest,src in [('duplicates-original.log','duplicates.log'),('duplicates-original.json','duplicates.json')]: retained.append((O/'records'/dest,R/'target/project-repositories/assurance'/src))
for p in (O/'records').iterdir():
 if p.is_file() and not p.name.startswith(('duplicates-original','retained-repros')): retained.append((p,R/'target/project-repositories/assurance'/p.name))
for p in (O/'repros').iterdir():
 if p.is_file() and p.suffix in ['.dart','.rs']: retained.append((p,R/'target/project-repositories/assurance'/p.name))
check['retained_copy_count']=len(retained); check['retained_copy_mismatches']=[str(p.relative_to(R)) for p,q in retained if p.read_bytes()!=q.read_bytes()]
# Verify local file destinations only. Do not fetch remote URLs. Anchors are
# separately reported for human review; this scanner does not invent slug rules.
broken=[]; pending=[]; links=[]
link_sources=supporting + ([{'path':'docs/evidence/project-repositories/REVIEW.md'}] if (D/'REVIEW.md').exists() else [])
for f in link_sources:
 p=R/f['path']
 if p.suffix!='.md': continue
 for m in re.finditer(r'(?<!!)\[[^\]\n]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)',p.read_text()):
  link=m.group(1)
  if re.match(r'[A-Za-z][A-Za-z0-9+.-]*:',link) or link.startswith('#'): continue
  target=(p.parent/urllib.parse.unquote(link.split('#')[0])).resolve(); item={'source':f['path'],'link':link,'target':str(target.relative_to(R)) if target.is_relative_to(R) else '<outside-repo>'}; links.append(item)
  # The output record itself is created after validation below; verify that
  # self-reference after publication without requiring a placeholder file.
  if not target.exists() and target != output:
   (pending if target==D/'REVIEW.md' and phase!='final' else broken).append(item)
A=O/'lineage/pre-doc-closeout'; archive_source=R/'target/project-repositories/pre-doc-closeout'; prior=load(A/'results.json'); oldseal=load(A/'reviewer/seal.json'); closeout=load(R/'target/project-repositories/documentation-closeout.json')
oldanchors={'results.json':'97b73a7e54a12ad14ae55aad168496f87863e4a274e1796b8d379f8bd0ed604a','REVIEW.md':'9c837ab84e0ce3e561fb45d311d861c5675c15ae368a71be80442d3cf2943927','reviewer/seal.json':'874f71db07529111a454526bf7151583d70d13c3b7d869749aa880a83c961b09'}
checks['prior_anchors_match']=all(sha(A/p)==h for p,h in oldanchors.items())
checks['closeout_anchors_match']=all(closeout[k]==oldanchors[p] for k,p in [('prior_results_sha256','results.json'),('prior_review_sha256','REVIEW.md'),('prior_seal_sha256','reviewer/seal.json')])
prior_members=oldseal['parent_bundle_files']+oldseal['reviewer_artifacts']+[oldseal['review']]
checks['all_prior_sealed_members_preserved']=all(sha(A/pathlib.Path(f['path']).relative_to('docs/evidence/project-repositories'))==f['sha256'] and (A/pathlib.Path(f['path']).relative_to('docs/evidence/project-repositories')).stat().st_size==f['size'] and (A/pathlib.Path(f['path']).relative_to('docs/evidence/project-repositories')).stat().st_mode & 0o777==f['mode'] for f in prior_members)
checks['lineage_copy_matches_parent_archive']=all(p.read_bytes()==(archive_source/p.relative_to(A)).read_bytes() and stat.S_IMODE(p.stat().st_mode)==stat.S_IMODE((archive_source/p.relative_to(A)).stat().st_mode) for p in A.rglob('*') if p.is_file()) and {str(p.relative_to(A)) for p in A.rglob('*') if p.is_file()}=={str(p.relative_to(archive_source)) for p in archive_source.rglob('*') if p.is_file()}
checks['core_manifest_byte_identical_to_prior']=(D/'core-source-manifest.jsonl').read_bytes()==(A/'core-source-manifest.jsonl').read_bytes()
checks['artifact_records_identical_to_prior']=artifacts['files']==load(A/'artifacts.json')['files']
oldraw={f['path']:f for f in prior['raw_evidence']}; newraw={f['path']:f for f in res['raw_evidence']}
checks['all_prior_raw_records_identical']=all(newraw.get(p)==f for p,f in oldraw.items())
checks['only_closeout_raw_record_added']=set(newraw)-set(oldraw)=={'target/project-repositories/documentation-closeout.json'}
old_support={f['path']:f for f in jsonl(A/'supporting-inputs.jsonl')}; new_support={f['path']:f for f in supporting}
changed_support=sorted(p for p in old_support if new_support.get(p)!=old_support[p]); check['changed_supporting_paths']=changed_support
checks['only_three_approved_supporting_docs_changed']=set(old_support)==set(new_support) and set(changed_support)=={'docs/SMITH_LINUX_ARCHITECTURE_REPORT.md','docs/plans/LOCAL-SMITH-PROJECT-REPOSITORIES.md','docs/evidence/project-repositories/README.md'}
check['changed_parent_results_fields']=sorted(k for k in res if res[k]!=prior.get(k)); checks['results_changes_metadata_only']=set(res)==set(prior) and set(check['changed_parent_results_fields'])<={'artifacts','source','raw_evidence','limitations'}
checks['final_gate_records_unchanged']=res['final_gates']==prior['final_gates'] and res['compiled_projects']==prior['compiled_projects'] and res['counts']==prior['counts']
check['prior_anchors']=oldanchors; check['prior_sealed_members_checked']=len(prior_members); check['lineage_file_count']=sum(p.is_file() for p in A.rglob('*'))
check.update(local_links_checked=len(links),broken_local_links=broken,pending_review_links=pending,checks=checks)
check['passed']=all(checks.values()) and all(not check[k] for k in ['core_mismatches','supporting_mismatches','artifact_mismatches','raw_mismatches','baseline_archive_mismatches','historical_projects_mismatches','selected_change_mismatches','retained_copy_mismatches','broken_local_links']) and len(core)==1243 and len(artifacts['files'])==189 and len(res['raw_evidence'])==78 and len(changes['changes'])==32
check['finished']=datetime.datetime.now(datetime.timezone.utc).isoformat(); output.write_text(json.dumps(check,indent=2,sort_keys=True)+'\n'); print(json.dumps({k:v for k,v in check.items() if k not in ['parent_bundle_files']},indent=2)); sys.exit(0 if check['passed'] else 1)
