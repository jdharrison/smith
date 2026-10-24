"""Final independent artifact, preservation and source recheck; exclusive new output."""
import hashlib
import io
import json
import os
from pathlib import Path
import stat
import subprocess
import tarfile
from PIL import Image, ImageChops
R=Path.cwd()
E=R/'docs/evidence/dev-navigation/review-closeout'
sha=lambda b:hashlib.sha256(b).hexdigest()
def load(n): return json.loads((R/n).read_text())
def canonical(v): return sha(json.dumps(v,sort_keys=True,separators=(',',':')).encode())
old=load('docs/evidence/dev-navigation/review-closeout/source-current.json')
policy=load('docs/evidence/dev-navigation/validation/core-manifest-final.json')
roots=policy['source_roots']
argv=['git','--no-pager','ls-files','--cached','--others','--exclude-standard','-z','--',*roots]
names=subprocess.check_output(argv,timeout=30).split(b'\0')
selected=set()
for b in names:
 if not b: continue
 n=os.fsdecode(b)
 if any(n.startswith(p) for p in policy['excluded_prefixes']) or '__pycache__' in Path(n).parts or n.endswith('.pyc') or n=='client/.flutter-plugins-dependencies': continue
 selected.add(n)
assert selected=={r['path'] for r in old['files']}
for r in old['files']:
 p=R/r['path']; s=p.lstat()
 assert s.st_size==r['size'] and stat.S_IMODE(s.st_mode)==r['mode']
 assert sha(os.readlink(p).encode() if p.is_symlink() else p.read_bytes())==r['sha256']
prior=load('docs/evidence/dev-navigation/review-closeout/immutable-before.json')
assert all(sha((R/n).read_bytes())==h for n,h in prior.items())
# Prior reviewer's pre-existing evidence anchors, including original target archives.
anchor=load('docs/evidence/dev-navigation/review/prior-evidence-before.json')
anchor_bad=[n for n,h in anchor.items() if not (R/n).is_file() or sha((R/n).read_bytes())!=h]
assert not anchor_bad,anchor_bad
export=load('docs/evidence/dev-navigation/validation/linux-client-export.json')
assert canonical(export['entries'])==export['sha256']
for r in export['entries']:
 p=R/r['path']; assert stat.S_IMODE(p.lstat().st_mode)==r['mode']
 if r['type']=='file': assert sha(p.read_bytes())==r['sha256']
 if r['type']=='symlink': assert sha(os.readlink(p).encode())==r['sha256']
visual=load('docs/evidence/dev-navigation/validation/visual-binding.json')
for key in ['failure_archive','prior_failure_archive']:
 r=visual[key]; assert sha((R/r['path']).read_bytes())==r['sha256']
prior_tar=tarfile.open(R/visual['prior_failure_archive']['path'])
visual_rows=[]
for r in visual['current_failures']:
 actual=(R/r['actual']['path']).read_bytes()
 expected=(R/r['expected']['path']).read_bytes()
 assert sha(actual)==r['actual']['sha256']==r['prior_failure_actual_sha256']
 assert sha(expected)==r['expected']['sha256']==sha((R/r['golden']).read_bytes())
 matches=[m for m in prior_tar.getmembers() if m.isfile() and m.name.endswith(Path(r['actual']['path']).name)]
 assert len(matches)==1 and sha(prior_tar.extractfile(matches[0]).read())==sha(actual)
 a,b=[Image.open(io.BytesIO(v)).convert('RGBA') for v in (actual,expected)]
 channels=ImageChops.difference(a,b).split(); delta=channels[0]
 for c in channels[1:]: delta=ImageChops.lighter(delta,c)
 count=delta.point(lambda x:255 if x else 0).histogram()[255]
 assert count==865
 visual_rows.append(dict(golden=r['golden'],changed_pixels=count,actual_matches_handoff=True))
snapshot=load('docs/evidence/dev-navigation/validation/final-source-binding.json')['source_snapshot_archive']
assert sha((R/snapshot['path']).read_bytes())==snapshot['sha256']
# Bind all closeout inputs and the new report; never self-hash the output seal.
files={str(p.relative_to(R)):sha(p.read_bytes()) for p in E.iterdir() if p.is_file()}
report=R/'docs/evidence/dev-navigation/REVIEW.md'
if report.exists(): files[str(report.relative_to(R))]=sha(report.read_bytes())
result=dict(core_sha256=old['sha256'],final_core_records_equal=True,final_selector_membership_equal=True,old_docs_evidence_preserved=len(prior),prior_reviewer_preexisting_anchor_files_verified=len(anchor),prior_review_records_preserved=True,linux_export_sha256=export['sha256'],linux_export_files=export['file_count'],source_snapshot_archive_sha256=snapshot['sha256'],projects_visual_failures=visual_rows,closeout_files=files,scope='No approval of concurrent parent-owned README, policy or ledger changes outside executable core; no acceptance/release authority')
with (E/'seal.json').open('x') as f: f.write(json.dumps(result,indent=2,sort_keys=True)+'\n')
print('Final core/selector unchanged; old docs evidence preserved:',len(prior),'; prior reviewer anchor files:',len(anchor))
print('Linux export files verified:',export['file_count'],'; snapshot archive verified; three 865-pixel failures match handoff.')
print('New report SHA256:',files.get('docs/evidence/dev-navigation/REVIEW.md'))
print('Seal SHA256:',sha((E/'seal.json').read_bytes()))
