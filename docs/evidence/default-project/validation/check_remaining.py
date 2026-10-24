#!/usr/bin/env python3
from recorder import *
from run_validation import CAND, CONTROL
import json
import re
from urllib.parse import unquote,urljoin,urlparse

config=CAND/'client/.dart_tool/package_config.json'
p=next(p for p in json.loads(config.read_text())['packages'] if p['name']=='flutter')
sdk=Path(unquote(urlparse(urljoin(config.as_uri(),p['rootUri'])).path)).resolve().parents[1]
run('flutter-full-analyze',[str(sdk/'bin/cache/dart-sdk/bin/dart'),str(sdk/'bin/cache/flutter_tools.snapshot'),'--suppress-analytics','--no-version-check','analyze','--no-pub'],180,cwd=CAND/'client',extra={'DART_SUPPRESS_ANALYTICS':'true','FLUTTER_ALREADY_LOCKED':'true','FLUTTER_ROOT':str(sdk)})
run('rust-toolchain-version',['rustc','--version','--verbose'],15)
run('cargo-toolchain-version',['cargo','--version'],15)
run('engine-final-observation',['docker','inspect','--format','{"running":{{json .State.Running}},"image":{{json .Config.Image}},"started_at":{{json .State.StartedAt}}}',ENGINE],15)
initial=json.loads((OUT/'core-manifest-before.json').read_text())
changed=[]
control_changed=[]
owned={r['path']:r['control_sha256'] for r in json.loads((OUT/'negative-control-binding.json').read_text())['overlays']}
for row in initial['files']:
    if row['type']!='file':continue
    p=row['path']
    if not (CAND/p).is_file() or digest(CAND/p)!=row['sha256']:changed.append(p)
    if p.startswith('service/') and (not (CONTROL/p).is_file() or digest(CONTROL/p)!=owned.get(p,row['sha256'])):control_changed.append(p)
save('snapshot-final-integrity.json',{'candidate_original_core_file_changes':changed,'control_service_changes_since_overlay':control_changed,'original_core_unchanged':not changed,'control_unchanged':not control_changed,'fixture_final':file_record(OUT/'debug_service-fixture'),'source_archive':file_record(OUT/'candidate-core.tar.gz'),'driver_final':file_record(OUT/'run_validation.py'),'control_driver':file_record(OUT/'control_isolated.py'),'remaining_driver':file_record(Path(__file__))})
text=re.sub(r'\x1b\[[0-9;]*[A-Za-z]','',(OUT/'root-gates.log').read_text())
cached=re.findall(r'^(\d+)\s*: .* CACHED.*$',text,re.M)
canceled=re.findall(r'^(\d+)\s*: .* ! context canceled$',text,re.M)
errors=re.findall(r'^(\d+)\s*: .* ERROR.*$',text,re.M)
save('root-observed-gates.json',{'cached_vertex_ids':sorted(set(cached),key=int),'context_canceled_vertex_ids':sorted(set(canceled),key=int),'error_vertex_ids':sorted(set(errors),key=int),'root_make_invocations':1,'root_timeout_seconds':900,'observed_failure':'Service warning-denied all-target Clippy: app_instructions_tests.rs:21 field_reassign_with_default. Exit101 propagated as make exit2.', 'parallel_terminal_stdout_status':{'25/service':'failed101','70/installer-ui':'context canceled','74/installer':'context canceled','96/client':'context canceled','112/package':'context canceled','126/no-python-cli-runtime':'context canceled'},'unit_suite_totals_in_root':[],'root_service_format':'script continued past cargo fmt to clippy; no format failure observed','root_service_tests':'not reached after Clippy failure','root_client_analyzer_tests':'no completed result, canceled','root_installer_tests':'no completed result, canceled','root_package_tests':'no completed result, canceled','make_build':'not reached','exports_current':False,'network_and_cache_observations':'Existing approved engine selected with v0.21.9 client/server. Cached setup including Flutter Git clone; actual crates.io index/download and apt package traffic occurred within configured pipeline. No image/config/engine changes or authority broadened by validator. DONE graph-construction lines are not test-pass results. No cold-cache/hermeticity claim.'})
