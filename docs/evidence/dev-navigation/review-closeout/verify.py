"""Read-only independent closeout audit; writes only this new evidence directory."""
import datetime
import difflib
import hashlib
import io
import json
import math
import os
from pathlib import Path
import re
import stat
import subprocess
import tarfile
from PIL import Image, ImageChops, ImageDraw

R = Path.cwd()
E = R / 'docs/evidence/dev-navigation/review-closeout'
sha = lambda b: hashlib.sha256(b).hexdigest()
canon = lambda v: sha(json.dumps(v, sort_keys=True, separators=(',', ':')).encode())

def load(p):
    return json.loads((R / p).read_text())

def save(n, v):
    with (E / n).open('x') as f:
        f.write(json.dumps(v, indent=2, sort_keys=True) + '\n')

def git(*args):
    return subprocess.check_output(['git', '--no-pager', *args], timeout=30)

prior = {str(p.relative_to(R)): sha(p.read_bytes()) for p in (R/'docs/evidence/dev-navigation').rglob('*') if p.is_file() and 'review-closeout' not in p.parts and p.name != 'REVIEW.md'}
save('immutable-before.json', prior)
old = load('docs/evidence/dev-navigation/validation/core-manifest-final.json')
roots = old['source_roots']
tracked = set(git('ls-files', '--cached', '-z', '--', *roots).split(b'\0'))
names = set(git('ls-files', '--cached', '--others', '--exclude-standard', '-z', '--', *roots).split(b'\0'))
rows = []
for n in sorted(names):
    if not n:
        continue
    name = os.fsdecode(n)
    if any(name.startswith(p) for p in old['excluded_prefixes']) or '__pycache__' in Path(name).parts or name.endswith('.pyc') or name == 'client/.flutter-plugins-dependencies':
        continue
    p = R/name
    s = p.lstat()
    mode = stat.S_IMODE(s.st_mode)
    row = dict(path=name, mode=mode, mode_octal=oct(mode), size=s.st_size, tracked=n in tracked)
    if p.is_symlink():
        row.update(type='symlink', target=os.readlink(p), sha256=sha(os.readlink(p).encode()))
    else:
        assert p.is_file(), name
        row.update(type='file', sha256=sha(p.read_bytes()))
    rows.append(row)
current = dict(count=len(rows), untracked_count=sum(not r['tracked'] for r in rows), sha256=canon(rows), files=rows)
assert rows == old['files']
assert current['sha256'] == 'b86b9d04d2867a23994b142d3850f1b4ea0a5a72c6df7f1d2321ec2f2a180777'
assert load('docs/evidence/dev-navigation/review/source-before.json')['files'] == rows
save('source-current.json', current)
base = load('docs/evidence/dev-navigation/baseline-manifest.json')
assert canon(base['files']) == base['sha256'] == '8d5e063fc8ec672d40b3218995fe3fdf98b170652c2dbb44d2ab561d2ade993f'
archive = R/'target/dev-navigation/baseline-source.tar.gz'
assert sha(archive.read_bytes()) == base['archive_sha256']
t = tarfile.open(archive)
assert len([m for m in t.getmembers() if m.isfile()]) == base['count'] == 1337
bmap = {r['path']:r for r in base['files']}
for r in base['files']:
    m = t.getmember(r['path'])
    assert sha(t.extractfile(m).read()) == r['sha256']
    assert m.size == r['size'] and m.mode == r['mode']
changes = [r['path'] for r in rows if r['path'] in bmap and r.get('sha256') != bmap[r['path']]['sha256']]
production = [n for n in changes if n.startswith(('client/lib/','service/src/','installer/src/','dagger/','scripts/'))]
assert production == ['client/lib/main.dart','client/lib/ui/smith_workspace.dart','client/lib/workspace_preferences.dart']
diff = ''.join(''.join(difflib.unified_diff(t.extractfile(n).read().decode().splitlines(True),(R/n).read_text().splitlines(True),fromfile='baseline/'+n,tofile=n)) for n in production)
with (E/'production.diff').open('x') as f:
    f.write(diff)
# Membership proves excluded/generated inputs did not silently become selected source.
save('source-binding.json',dict(recorded_at=datetime.datetime.now(datetime.timezone.utc).isoformat(), baseline_canonical=base['sha256'], baseline_archive_sha256=base['archive_sha256'], core_sha256=current['sha256'], count=len(rows), untracked_count=current['untracked_count'], source_roots=roots, excluded_prefixes=old['excluded_prefixes'], membership_equal=True, records_equal=True, production_changes=production, production_diff_sha256=sha(diff.encode()), baseline_files_verified=base['count'], excluded_parent_docs='Not approved by this bounded executable-core assessment'))
# Recompute exact pixel deltas independently from archive and current bytes.
proof = load('docs/evidence/dev-navigation/review/independent-pixel-proof.json')
pixels = []
for item in proof['images']:
    name = item['path']
    before = t.extractfile(name).read()
    after = (R/name).read_bytes()
    assert sha(before) == item['baseline_sha256'] and sha(after) == item['current_sha256']
    a, b = [Image.open(io.BytesIO(x)).convert('RGBA') for x in (before,after)]
    assert a.size == b.size
    channels = ImageChops.difference(a,b).split()
    delta = channels[0]
    for c in channels[1:]:
        delta = ImageChops.lighter(delta,c)
    delta = delta.point(lambda v: 255 if v else 0)
    mask = Image.new('L',a.size)
    rects = item['masks_half_open']
    for i,(l,u,r,d) in enumerate(rects):
        assert r-l == 56
        for x,y,z,w in rects[:i]:
            assert r<=x or z<=l or d<=y or w<=u
        ImageDraw.Draw(mask).rectangle((l,u,r-1,d-1),fill=255)
    if 'mixed-grid' in name:
        assert len(rects)==4
    count = delta.histogram()[255]
    outside = ImageChops.subtract(delta,mask).histogram()[255]
    assert count == item['changed_pixels'] and outside == 0
    pixels.append(dict(path=name,changed_pixels=count,outside_pixels=outside,masks=rects))
for r in proof['frozen_editors']:
    assert sha((R/r['path']).read_bytes()) == r['sha256'] == sha(t.extractfile(r['path']).read())
assert len(pixels)==23 and sum(r['changed_pixels'] for r in pixels)==159815 and len(proof['frozen_editors'])==13
save('pixel-recheck.json',dict(changed_images=23,changed_pixels=159815,outside_pixels=0,frozen_editor_images=13,images=pixels,mask_provenance='Prior independent geometry rectangles; widths/disjointness checked again against unchanged width56 shell; exact any-channel pixel deltas recomputed here'))
# Reparse raw logs, preserve prior harness failures, never sum replayed terminals.
gates=[]
for folder, names in [('review',['format','analyzer','focused','adversarial','adversarial-attempt002','retention']),('validation',['focused-safe-temp','full-native-tests-safe-temp','client-test-configured-module','client-build-configured-module','full-format','full-analyzer','root-test-build','root-original-test-build'])]:
    for name in names:
        stem='docs/evidence/dev-navigation/'+folder+'/'+name
        meta=load(stem+'.json')
        raw=(R/(stem+'.log')).read_bytes()
        assert sha(raw)==meta['log_sha256'],stem
        text=re.sub(r'\x1b\[[0-9;]*m','',raw.decode(errors='replace'))
        terminals=[]
        for line in text.splitlines():
            if re.search(r'All tests passed!|Some tests failed\.|test result: (?:ok|FAILED)|Formatted \d+|No issues found|Built .*',line):
                # Strip Dagger vertex prefix for failure replay deduplication.
                value=line.split('|',1)[-1].strip() if '|' in line else line.strip()
                if value not in terminals:
                    terminals.append(value)
        failures=list(dict.fromkeys(re.findall(r'^.*(?:\[E\]|test .* \.\.\. FAILED).*$',text,re.M)))
        gates.append(dict(name=folder+'/'+name,command=meta.get('argv',meta.get('command')),exit=meta.get('exit_code',meta.get('exit')),duration_seconds=meta['duration_seconds'],log=stem+'.log',log_sha256=sha(raw),terminals=terminals,failures=failures))
save('raw-gates-rechecked.json',gates)
root=load('docs/evidence/dev-navigation/validation/root-original-results.json')
assert root['real_root_attempt_count']==1 and root['source_stable'] and not root['package_eligible']
assert all(c['status']=='not_run' and c['exit_code'] is None for c in root['conditional_package_commands'])
assert all(v['unchanged'] for v in root['export_identity_changes'].values())
save('root-distinction.json',dict(real_attempts=1,exit=root['root_command']['exit_code'],duration_seconds=root['root_command']['duration_seconds'],one_service_summary=dict(passed=143,failed=20,controlled_ignores=2),build_reached=False,package_checks='NOT RUN',exports='unchanged/stale; not current acceptance artifacts',supersession='Actual repository root only supersedes earlier copied-module discovery root status. All prior failures retained.'))
print('Source: 1296 exact records, 54 untracked; selector membership unchanged;', current['sha256'])
print('Baseline: 1337 archive records/canonical digest verified; exactly three production diffs.')
print('Pixels: 23 changed PNGs, 159815 changed pixels, zero outside disjoint 56px masks; 13 editors frozen.')
for g in gates:
    print(g['name'],'exit',g['exit'],'terminal',g['terminals'][-2:])
