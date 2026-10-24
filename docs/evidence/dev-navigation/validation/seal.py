import shutil
from recorder import *
for p in sorted(OUT.glob('*.py')):
    shutil.copyfile(p,EVIDENCE/p.name)
# Manifests/artifact inventories cover expanded source and Linux export; avoid duplicating the bundle.
entries=[file_record(p) for p in sorted(OUT.iterdir()) if p.is_file() and p.name!='evidence-index.json']
entries.extend(file_record(p) for p in sorted(EVIDENCE.iterdir()) if p.is_file() and p.name!='evidence-index.json')
final=manifest()
save('seal-core-binding.json',{'recorded_at':now(),'core_sha256':final['sha256'],'count':final['count'],'untracked_count':final['untracked_count'],'stable_since_initial':final['sha256']==__import__('json').loads((OUT/'core-manifest-before.json').read_text())['sha256']})
entries.extend(file_record(p) for p in [OUT/'seal-core-binding.json',EVIDENCE/'seal-core-binding.json'])
save('evidence-index.json',{'recorded_at':now(),'entries':entries,'entries_sha256':canonical_sha(entries),'scope':'New validation files, recorders and archives only; expanded source/export contents bound by their manifests. Index itself excluded to avoid self-reference. Parent-owned digests, not independent assurance.','core_sha256':final['sha256']})
print('SEALED',final['sha256'],len(entries),'artifacts')
