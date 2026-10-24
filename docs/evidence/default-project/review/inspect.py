#!/usr/bin/env python3
"""Independent read-only source audit; writes confined to review/assurance."""
import datetime, difflib, hashlib, json, os, pathlib, shutil, stat, subprocess, tarfile
R = pathlib.Path.cwd()
O = R / 'docs/evidence/default-project/review'
T = R / 'target/default-project/assurance'
def sha(b): return hashlib.sha256(b).hexdigest()
def save(n,v): (O/n).write_text(json.dumps(v,sort_keys=True,indent=2)+'\n')
def canonical(v): return sha(json.dumps(v,sort_keys=True,separators=(',',':')).encode())
base=json.loads((R/'target/default-project/baseline-manifest.json').read_text())
parent=json.loads((R/'docs/evidence/default-project/validation/core-manifest-final.json').read_text())
# Reconstruct the parent's exact selection, independently querying current Git paths.
roots=parent['source_roots']
def capture():
    tracked=set(subprocess.check_output(['git','--no-pager','ls-files','--cached','-z','--',*roots]).split(b'\0'))
    paths=set(subprocess.check_output(['git','--no-pager','ls-files','--cached','--others','--exclude-standard','-z','--',*roots]).split(b'\0'))
    records=[]
    for raw in sorted(paths):
        if not raw: continue
        p=os.fsdecode(raw)
        if any(p.startswith(x) for x in parent['excluded_prefixes']) or '__pycache__' in pathlib.Path(p).parts or p.endswith('.pyc') or p=='client/.flutter-plugins-dependencies': continue
        f=R/p
        if not f.exists() and not f.is_symlink(): rec={'path':p,'type':'missing'}
        else:
            s=f.lstat(); rec={'path':p,'mode':stat.S_IMODE(s.st_mode),'mode_octal':oct(stat.S_IMODE(s.st_mode)),'size':s.st_size}
            if f.is_symlink(): rec.update(type='symlink',target=os.readlink(f),sha256=sha(os.readlink(f).encode()))
            elif f.is_file(): rec.update(type='file',sha256=sha(f.read_bytes()))
            else: raise RuntimeError(p)
        rec['tracked']=raw in tracked; records.append(rec)
    return {'sha256':canonical(records),'count':len(records),'untracked_count':sum(not x['tracked'] for x in records),'files':records}
if __name__=='__main__':
    current=capture(); save('core-before.json',current)
    save('binding.json',{'at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'baseline_keys':{k:v for k,v in base.items() if k!='files'},'baseline_records_canonical_sha256':canonical(base['files']),'baseline_archive_sha256':sha((R/'target/default-project/baseline-source.tar.gz').read_bytes()),'parent_sha256':parent['sha256'],'current_sha256':current['sha256'],'matches_parent':current['files']==parent['files']})
    baseline={x['path']:x for x in base['files']}; deltas=[]
    archive=tarfile.open(R/'target/default-project/baseline-source.tar.gz')
    members={x.name:x for x in archive.getmembers()}
    diffs=[]; archive_mismatch=[]
    for p,old in baseline.items():
        if p in members and members[p].isfile() and sha(archive.extractfile(members[p]).read())!=old['sha256']: archive_mismatch.append(p)
        f=R/p
        if not f.is_file():
            deltas.append({'path':p,'status':'missing_or_not_file'}); continue
        b=f.read_bytes(); h=sha(b)
        if h!=old['sha256'] or stat.S_IMODE(f.stat().st_mode)!=old['mode']:
            deltas.append({'path':p,'before':old['sha256'],'after':h})
            if p in members and not p.startswith('docs/evidence/'):
                try: diffs.extend(difflib.unified_diff(archive.extractfile(members[p]).read().decode().splitlines(True),b.decode().splitlines(True),fromfile='baseline/'+p,tofile='current/'+p))
                except UnicodeError: pass
    save('baseline-delta.json',{'archive_record_mismatches':archive_mismatch,'changed_existing':deltas,'new_core_paths':[x['path'] for x in current['files'] if x['path'] not in baseline]})
    (O/'baseline.diff').write_text(''.join(diffs))
    for x in current['files']:
        if x['type']=='file' and x['path'].startswith(('service/','installer/')):
            dst=T/'candidate'/x['path']; dst.parent.mkdir(parents=True,exist_ok=True); shutil.copy2(R/x['path'],dst)
    (T/'tmp').mkdir(mode=0o700); os.chmod(T,0o700)
    save('copied-source.json',{'files':[x for x in current['files'] if x['path'].startswith(('service/','installer/'))], 'mismatches':[x['path'] for x in current['files'] if x['type']=='file' and x['path'].startswith(('service/','installer/')) and sha((T/'candidate'/x['path']).read_bytes())!=x['sha256']]})
    print(json.dumps({'core':current['sha256'],'matches_parent':current['files']==parent['files'],'baseline_archive_mismatches':archive_mismatch,'changed_existing':[x['path'] for x in deltas],'new_core':[x['path'] for x in current['files'] if x['path'] not in baseline]},indent=2))
