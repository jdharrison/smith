#!/usr/bin/env python3
import datetime, hashlib, importlib.util, json, os, pathlib, signal, subprocess, time
spec=importlib.util.spec_from_file_location('audit_inspect',pathlib.Path('docs/evidence/default-project/review/inspect.py')); a=importlib.util.module_from_spec(spec); spec.loader.exec_module(a)
R=a.R; O=a.O; T=a.T
HOME=pathlib.Path.home()
def safe(s): return s.replace(str(R),'<repo>').replace(str(HOME),'<home>')
env={'HOME':str(HOME),'PATH':str(HOME/'.cargo/bin')+':/usr/bin:/bin','LANG':'C.UTF-8','LC_ALL':'C.UTF-8','CARGO_NET_OFFLINE':'true','CARGO_TARGET_DIR':str(T/'cargo-original'),'TMPDIR':str(T/'tmp'),'CARGO_INCREMENTAL':'0'}
def run(name,argv,seconds=240):
    before=a.capture(); start=datetime.datetime.now(datetime.timezone.utc).isoformat(); clock=time.monotonic()
    p=subprocess.Popen(argv,cwd=T/'candidate/service',env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,start_new_session=True)
    timeout=False
    try: out=p.communicate(timeout=seconds)[0]
    except subprocess.TimeoutExpired:
        timeout=True; os.killpg(p.pid,signal.SIGKILL); out=p.communicate()[0]
    text=safe(out.decode(errors='replace')); (O/(name+'.log')).write_text(text)
    after=a.capture(); a.save(name+'.json',{'argv':argv,'cwd':'<repo>/target/default-project/assurance/candidate/service','environment':{k:safe(v) for k,v in env.items()},'start':start,'duration_seconds':time.monotonic()-clock,'timeout_seconds':seconds,'timed_out':timeout,'exit':p.returncode,'log_sha256':a.sha(text.encode()),'core_before':before['sha256'],'core_after':after['sha256']})
    print(name,'exit',p.returncode,'timeout',timeout,flush=True); print(text[-4000:],flush=True)
if __name__=='__main__':
    run('rust-version',['rustc','--version','--verbose'],20)
    run('bootstrap-original',['cargo','test','--locked','--offline','--lib','projects_bootstrap','--','--test-threads=1'])
    run('projects-original',['cargo','test','--locked','--offline','--lib','projects_','--','--test-threads=1'])
    a.save('core-after-checks.json',a.capture())
