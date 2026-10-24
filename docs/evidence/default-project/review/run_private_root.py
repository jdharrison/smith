#!/usr/bin/env python3
import importlib.util, pathlib
s=importlib.util.spec_from_file_location('checks',pathlib.Path('docs/evidence/default-project/review/run_checks.py')); m=importlib.util.module_from_spec(s); s.loader.exec_module(m)
m.env['TMPDIR']='/tmp'
# Root-owned host / becomes unmapped uid65534 in bwrap's user namespace.
# A namespace-local tmpfs / has safe ownership; only allowed host output is writable.
prefix=['bwrap','--tmpfs','/','--ro-bind','/usr','/usr','--ro-bind','/bin','/bin','--ro-bind','/lib','/lib','--ro-bind','/lib64','/lib64','--ro-bind','/etc','/etc','--ro-bind','/home','/home','--dev','/dev','--bind',str(m.T),str(m.T),'--bind',str(m.T/'tmp'),'/tmp','--unshare-net','--die-with-parent']
if __name__=='__main__':
    m.run('bootstrap-private-root',prefix+['cargo','test','--locked','--offline','--lib','projects_bootstrap','--','--test-threads=1'])
    m.run('projects-private-root',prefix+['cargo','test','--locked','--offline','--lib','projects_','--','--test-threads=1'])
    m.a.save('core-after-private-root.json',m.a.capture())
