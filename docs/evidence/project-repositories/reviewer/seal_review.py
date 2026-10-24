"""Seal only reviewer-owned outputs; never modify parent evidence or source."""
import datetime,hashlib,json,pathlib
R=pathlib.Path(__file__).resolve().parents[4]; D=R/'docs/evidence/project-repositories'; O=D/'reviewer'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
out=O/'seal.json'; assert not out.exists(), 'Never overwrite a review seal'
verified=json.loads((O/'bundle-verification-final.json').read_text()); assert verified['passed']
for f in verified['parent_bundle_files']:
 p=R/f['path']; assert sha(p)==f['sha256'] and p.stat().st_size==f['size'] and p.stat().st_mode & 0o777==f['mode'], f['path']
for name in ['core-source-manifest.jsonl','supporting-inputs.jsonl']:
 for line in (D/name).read_text().splitlines():
  f=json.loads(line); p=R/f['path']; assert sha(p)==f['sha256'] and p.stat().st_size==f['size'] and p.stat().st_mode & 0o777==f['mode'], f['path']
artifact=json.loads((D/'artifacts.json').read_text())
for f in artifact['files']:
 p=R/f['path']; assert sha(p)==f['sha256'] and p.stat().st_size==f['size'] and p.stat().st_mode & 0o777==f['mode'], f['path']
assert (O/'bundle-verification-final.json').is_file()
record=lambda p:{'path':str(p.relative_to(R)),'sha256':sha(p),'size':p.stat().st_size,'mode':p.stat().st_mode & 0o777}
seal={'sealed_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'verdict':'pass-with-risk: bounded configuration-only implementation/configured local source gates','release_gate_decision':'no-go','approval':'None: no accepted-risk, release, installed or external-action authority','review':record(D/'REVIEW.md'),'parent_bundle_files':verified['parent_bundle_files'],'core_canonical_sha256':verified['core_canonical_sha256'],'verification':record(O/'bundle-verification-final.json'),'reviewer_artifacts':[record(p) for p in sorted(O.rglob('*')) if p.is_file() and p!=out],'self_reference_note':'This seal excludes itself; its digest is returned separately in the final reviewer response.'}
out.write_text(json.dumps(seal,indent=2,sort_keys=True)+'\n'); print('REVIEW',seal['review']['sha256']); print('SEAL',sha(out)); print('reviewer_artifact_count',len(seal['reviewer_artifacts'])); print('parent_source_artifacts_still_match',True)
