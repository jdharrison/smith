"""Reseal reviewer-only documentation closeout, preserving the original seal."""
import datetime,hashlib,json,pathlib
R=pathlib.Path(__file__).resolve().parents[4]; D=R/'docs/evidence/project-repositories'; O=D/'reviewer'; A=O/'lineage/pre-doc-closeout'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
out=O/'seal.json'; versioned=O/'seal-doc-closeout.json'
assert not versioned.exists(), 'Never overwrite a documentation-closeout seal'
prior_seal='874f71db07529111a454526bf7151583d70d13c3b7d869749aa880a83c961b09'
assert sha(out)==prior_seal and sha(A/'reviewer/seal.json')==prior_seal
verified=json.loads((O/'bundle-verification-doc-closeout-final.json').read_text()); assert verified['passed']
record=lambda p:{'path':str(p.relative_to(R)),'sha256':sha(p),'size':p.stat().st_size,'mode':p.stat().st_mode & 0o777}
for f in verified['parent_bundle_files']:
 assert record(R/f['path'])==f, f['path']
for name in ['core-source-manifest.jsonl','supporting-inputs.jsonl']:
 for line in (D/name).read_text().splitlines():
  f=json.loads(line); assert record(R/f['path'])==f, f['path']
for f in json.loads((D/'artifacts.json').read_text())['files']:
 assert record(R/f['path'])==f, f['path']
seal={'sealed_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'verdict':'pass-with-risk: bounded configuration-only implementation/configured local source gates','release_gate_decision':'no-go','approval':'None: no accepted-risk, release, installed or external-action authority','documentation_closeout':'L1/L2 closed; M2 remains unexplained/open. No test/build/helper gates rerun. All gate logs, core and artifact file records unchanged.','review':record(D/'REVIEW.md'),'parent_bundle_files':verified['parent_bundle_files'],'core_canonical_sha256':verified['core_canonical_sha256'],'verification':record(O/'bundle-verification-doc-closeout-final.json'),'supersedes':{'results':record(A/'results.json'),'review':record(A/'REVIEW.md'),'seal':record(A/'reviewer/seal.json')},'reviewer_artifacts':[record(p) for p in sorted(O.rglob('*')) if p.is_file() and p not in [out,versioned]],'self_reference_note':'This seal excludes its current and versioned aliases; both have identical bytes. The prior seal is retained under lineage. The new digest is returned separately in the final response.'}
encoded=(json.dumps(seal,indent=2,sort_keys=True)+'\n').encode(); versioned.write_bytes(encoded); out.write_bytes(encoded)
assert out.read_bytes()==versioned.read_bytes(); print('REVIEW',seal['review']['sha256']); print('SEAL',sha(out)); print('reviewer_artifact_count',len(seal['reviewer_artifacts'])); print('parent_source_artifacts_still_match',True)
