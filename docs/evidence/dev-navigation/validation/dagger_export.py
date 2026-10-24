#!/usr/bin/python3
"""Evidence-only export redirect; leaves module/source/engine configuration unchanged."""
import json, os, sys
from pathlib import Path
root=Path(__file__).resolve().parents[3]
args=sys.argv[1:]
if '-o' in args:
    i=args.index('-o')+1
    allowed={'client/dist':root/'target/dev-navigation/validation/source/client/dist'}
    if args[i] not in allowed: raise SystemExit('Unapproved export destination')
    args[i]=str(allowed[args[i]])
argv=[str(root/'target/installer-pipeline/smith-dagger-0.21.9/dagger'),*args]
print('EXPORT-REDIRECT EXEC '+json.dumps(argv).replace(str(root),'<repo>').replace(str(Path.home()),'<home>'),flush=True)
os.execv(argv[0],argv)
