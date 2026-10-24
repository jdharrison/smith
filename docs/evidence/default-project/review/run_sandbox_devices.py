#!/usr/bin/env python3
import importlib.util, pathlib
s=importlib.util.spec_from_file_location('checks',pathlib.Path('docs/evidence/default-project/review/run_checks.py')); m=importlib.util.module_from_spec(s); s.loader.exec_module(m)
m.env['TMPDIR']='/tmp'
prefix=['bwrap','--ro-bind','/','/','--dev-bind','/dev','/dev','--bind',str(m.T),str(m.T),'--bind',str(m.T/'tmp'),'/tmp','--unshare-net','--die-with-parent']
if __name__=='__main__':
    m.run('bootstrap-sandbox-devices',prefix+['cargo','test','--locked','--offline','--lib','projects_bootstrap','--','--test-threads=1'])
    m.run('projects-sandbox-devices',prefix+['cargo','test','--locked','--offline','--lib','projects_','--','--test-threads=1'])
    m.a.save('core-after-sandbox-devices.json',m.a.capture())
