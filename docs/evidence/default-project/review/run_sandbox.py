#!/usr/bin/env python3
import importlib.util, pathlib
s=importlib.util.spec_from_file_location('checks',pathlib.Path('docs/evidence/default-project/review/run_checks.py')); m=importlib.util.module_from_spec(s); s.loader.exec_module(m)
m.env['TMPDIR']='/tmp'
prefix=['bwrap','--ro-bind','/','/','--bind',str(m.T),str(m.T),'--bind',str(m.T/'tmp'),'/tmp','--unshare-net','--die-with-parent']
m.run('bootstrap-sandbox',prefix+['cargo','test','--locked','--offline','--lib','projects_bootstrap','--','--test-threads=1'])
m.run('projects-sandbox',prefix+['cargo','test','--locked','--offline','--lib','projects_','--','--test-threads=1'])
m.a.save('core-after-sandbox.json',m.a.capture())
