#!/usr/bin/env python3
import importlib.util,pathlib
s=importlib.util.spec_from_file_location('private',pathlib.Path('docs/evidence/default-project/review/run_private_root.py')); p=importlib.util.module_from_spec(s); s.loader.exec_module(p)
p.prefix += ['--proc','/proc']
if __name__=='__main__':
    p.m.run('bootstrap-final-sandbox',p.prefix+['cargo','test','--locked','--offline','--lib','projects_bootstrap','--','--test-threads=1'])
    p.m.run('projects-final-sandbox',p.prefix+['cargo','test','--locked','--offline','--lib','projects_','--','--test-threads=1'])
    p.m.a.save('core-after-final-sandbox.json',p.m.a.capture())
