"""Authorized one-shot CPU startup/template validation; NO completion endpoint.

--prepare only compiles a private fixture from byte-extracted production validators.
--probe must be detached with a durable PID/log; unknown cleanup retains both leases.
No service lifecycle, models inventory, Dev operation, image change or generation.
"""
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import socket
import stat
import subprocess
import sys
import threading
import time
import tomllib
import uuid

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('v5_no_generation_support', HERE / 'pipeline-qa-v5.py')
Q = importlib.util.module_from_spec(spec)
spec.loader.exec_module(Q)
M, C, D = Q.M, Q.C, Q.D
BASE = Q.PRIVATE / 'inspection-diag-v5/native'
TOOL = Q.ROOT / '.smith-inspection-diag-v5/native-helper/debug/qa-native-v5'
TESTS = HERE / 'test_native_v5_nogeneration.py'
REVIEW_SCOPE = 'one-owned-no-generation-startup-template-tokenize-probe'


def function(source, name):
    start = source.index('fn ' + name + '(')
    # Include a visibility qualifier when present; functions are rustfmt top-level.
    start = source.rfind('\n', 0, start) + 1
    return source[start:source.index('\n}', start) + 2] + '\n'


def tool_hash():
    fd = os.open(TOOL, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        meta = os.fstat(stream.fileno())
        Q.require(stat.S_ISREG(meta.st_mode) and meta.st_uid == os.geteuid()
                  and meta.st_size <= 128 * 1024 * 1024, 'fixture-artifact-refused')
        digest = hashlib.sha256()
        while True:
            C.checkpoint()
            block = stream.read(1024 * 1024)
            if not block:
                break
            digest.update(block)
        Q.require(M.file_identity(os.fstat(stream.fileno())) == M.file_identity(meta)
                  == M.file_identity(TOOL.lstat()), 'fixture-artifact-drift')
        return digest.hexdigest()


def prepare():
    Q.static_check()
    BASE.mkdir(mode=0o700)
    M.sync_dir(BASE.parent)
    work = BASE / 'helper'
    work.mkdir(mode=0o700)
    source = M.read(Q.ROOT / 'service/src/runner.rs', source=True).decode()
    cloud = M.read(Q.ROOT / 'service/src/cloud_http.rs', source=True).decode()
    unique = cloud[cloud.index('struct Unique(Value);'):cloud.index('\nfn protocol()', cloud.index('struct Unique(Value);'))]
    cloud_error = function(M.read(Q.ROOT / 'service/src/cloud.rs', source=True).decode(), 'error')
    digest = function(M.read(Q.ROOT / 'service/src/filesystem.rs', source=True).decode(), 'digest')
    probe = source[source.index('enum ProbeEvidence {'):source.index('fn inspection_support(')]
    probe = probe.replace('enum ProbeEvidence {', '#[derive(serde::Serialize, serde::Deserialize)]\nenum ProbeEvidence {', 1)
    probe = probe.replace('struct InspectionProbe {', '#[derive(serde::Serialize, serde::Deserialize)]\nstruct InspectionProbe {', 1)
    declarations = '''#![allow(dead_code)]
use smith_service::app::{new_id,AppError,AppResult};
use smith_service::acceleration::Backend;
use serde_json::{json,Value};
use std::collections::BTreeMap;
mod app { pub use smith_service::app::{AppError,AppResult}; }
mod dev_local { pub use smith_service::dev_local::SYSTEM_PROMPT; }
mod dev_completion;
#[derive(Clone,Copy,PartialEq,Eq,serde::Serialize,serde::Deserialize)]
enum RuntimeProfile { Unknown, B11429 }
#[derive(serde::Serialize,serde::Deserialize)]
struct State { profile:RuntimeProfile, probe:InspectionProbe }
const CONTEXT:u32=4096; const MAX_TOKENS:u32=1024; const TOKEN_MARGIN:u32=32;
'''
    # These constants are copied literally, not inferred or relaxed by the fixture.
    for name, value in (('CONTEXT', 4096), ('MAX_TOKENS', 1024), ('TOKEN_MARGIN', 32)):
        Q.require(Q.re.search(r'const ' + name + r': u32 = ' + str(value) + ';', source), 'constant-drift')
    generated = declarations + 'mod filesystem { use sha2::{Digest,Sha256};\n' + digest + '}\n'
    generated += 'mod cloud { use super::*; use serde::Deserialize; use serde::de::{Visitor,MapAccess,SeqAccess}; use std::fmt;\n' + cloud_error + unique + '}\n'
    generated += ''.join(function(source, name) for name in ('error', 'protocol', 'unsupported_template',
                            'runtime_profile', 'missing_response_capability', 'inspection_template', 'check_inspection_tokens'))
    generated += probe
    generated += r'''
fn read(path:&str)->AppResult<Value>{let b=std::fs::read(path).map_err(|_|protocol("fixture read"))?;cloud::unique_json(&b)}
fn write(path:&str,v:&impl serde::Serialize)->AppResult<()>{std::fs::write(path,serde_json::to_vec(v).map_err(|_|protocol("fixture encode"))?).map_err(|_|protocol("fixture write"))}
fn run(a:&[String])->AppResult<Value>{
 match a[1].as_str(){
 "make"=>{
  let r=read(&a[2])?;
  let roots=smith_service::Roots{data:r["--data-root"].as_str().ok_or_else(||protocol("root"))?.into(),config:r["--config-root"].as_str().ok_or_else(||protocol("root"))?.into(),runtime:r["--runtime-root"].as_str().ok_or_else(||protocol("root"))?.into()};
  let verified=smith_service::runner::verify_runtime_backend_bounded(&roots,Backend::Cpu,std::time::Instant::now()+std::time::Duration::from_secs(60),&||false)?;
  let _config=smith_service::runner::RunnerConfig::production(&roots,verified);
  let manifest=read(roots.data.join("smith-next/client/current/.smith-next-manifest.json").to_str().ok_or_else(||protocol("manifest"))?)?;
  let mut inventory=BTreeMap::new();
  for (name,entry) in manifest["entries"].as_object().ok_or_else(||protocol("entries"))? {
   if let Some(name)=name.strip_prefix("smith_backend/runtime/cpu/") { inventory.insert(name.to_owned(),(entry["mode"].as_u64().ok_or_else(||protocol("mode"))? as u32,entry["sha256"].as_str().ok_or_else(||protocol("digest"))?.to_owned())); }
  }
  let profile=runtime_profile(&inventory,Backend::Cpu);
  if profile!=RuntimeProfile::B11429{return Err(unsupported_template());}
  let state=State{profile,probe:InspectionProbe::new()?};
  write(&a[3],&state)?; write(&a[4],&state.probe.request)?;
  Ok(json!({"phase":"verified_runtime_and_probe","code":"OK","profile":"B11429","threads":smith_service::runner::default_threads()}))
 },
 "props"=>{let state:State=serde_json::from_value(read(&a[2])?).map_err(|_|protocol("state"))?; let missing=missing_response_capability(&read(&a[3])?,state.profile)?;Ok(json!({"phase":"props","code":"OK","missing_response_capability":missing}))},
 "verify"=>{let state:State=serde_json::from_value(read(&a[2])?).map_err(|_|protocol("state"))?;state.probe.verify(&read(&a[3])?)?;Ok(json!({"phase":"semantic_probe_verify","code":"OK"}))},
 "template"=>{let request=dev_completion::validate_request(&read(&a[2])?)?;write(&a[3],&inspection_template(&request))?;Ok(json!({"phase":"actual_template_input","code":"OK"}))},
 "context"=>{check_inspection_tokens(&read(&a[2])?,CONTEXT)?;Ok(json!({"phase":"context","code":"OK"}))},
 _=>Err(protocol("fixture mode"))
 }
}
fn main(){let a:Vec<String>=std::env::args().collect();let output=match run(&a){Ok(v)=>v,Err(e)=>{let code=match e.code.as_str(){"UNSUPPORTED"|"PROVIDER_PROTOCOL"|"CLOUD_PROTOCOL"|"CONTEXT_LIMIT"|"RUNTIME_UNSAFE"|"RUNTIME_MISSING"=>e.code.as_str(),_=>"OTHER"};json!({"phase":"fixture_refusal","code":code})}};println!("{}",output);}
'''.replace('\\"', '"')
    M.write(work / 'main.rs', generated.encode())
    completion = M.read(Q.ROOT / 'service/src/dev_completion.rs', source=True)
    M.write(work / 'dev_completion.rs', completion)
    manifest = '[package]\nname="qa-native-v5"\nversion="0.0.0"\nedition="2021"\n[workspace]\n[[bin]]\nname="qa-native-v5"\npath="main.rs"\n[dependencies]\nserde={version="1",features=["derive"]}\nserde_json="1"\nsha2="0.10"\nsmith-service={path=' + json.dumps(str(Q.ROOT / 'service')) + ',features=["inspection-diagnostics"]}\n'
    M.write(work / 'Cargo.toml', manifest.encode())
    original_lock = M.read(Q.ROOT / 'service/Cargo.lock', source=True)
    M.write(work / 'Cargo.lock', original_lock)
    command = ['cargo', 'build', '--offline', '--manifest-path', str(work / 'Cargo.toml'),
               '--target-dir', str(TOOL.parents[1])]
    built = subprocess.run(command, capture_output=True, timeout=240)
    M.write(BASE / 'compile.log', built.stdout + built.stderr)
    Q.require(built.returncode == 0, 'validator-fixture-compile-failed')
    original = {(p['name'], p['version'], p.get('source'), p.get('checksum'))
                for p in tomllib.loads(original_lock.decode())['package']}
    actual = tomllib.loads(M.read(work / 'Cargo.lock').decode())['package']
    Q.require(all(p['name'] == 'qa-native-v5' or (p['name'], p['version'], p.get('source'), p.get('checksum')) in original
                  for p in actual), 'fixture-dependency-drift')
    M.write(BASE / 'prepared.json', M.encoded({'tool_sha256': tool_hash(),
        'fixture_source_sha256': Q.sha(generated.encode()), 'completion_source_sha256': Q.sha(completion),
        'source_snapshot': Q.SOURCE_SNAPSHOT, 'candidate_sha256': Q.CANDIDATE_SHA,
        'validator_function_bodies': 'byte-extracted-production; serde derives added only to private probe state'}))
    print('{"status":"private-production-validator-fixture-prepared","generation_requests":0}')


def reviewed_probe(prepared):
    raw = M.read(BASE / 'review.json', 16384)
    receipt = M.decode(raw)
    expected = {'scope': REVIEW_SCOPE, 'source_snapshot': Q.SOURCE_SNAPSHOT,
                'candidate_sha256': Q.CANDIDATE_SHA,
                'driver_sha256': Q.sha(M.read(Path(__file__), source=True)),
                'tests_sha256': Q.sha(M.read(TESTS, source=True)),
                'harness_sha256': Q.sha(M.read(Path(Q.__file__), source=True)),
                'harness_tests_sha256': Q.sha(M.read(Q.TESTS, source=True)),
                'prepared_sha256': Q.sha(M.read(BASE / 'prepared.json')),
                'tool_sha256': prepared['tool_sha256']}
    Q.require(receipt.get('verdict') == 'pass' and receipt.get('independent_review') is True
              and isinstance(receipt.get('reviewer'), str) and bool(receipt['reviewer'])
              and type(receipt.get('expires_unix')) in (int, float)
              and time.time() < receipt['expires_unix'] <= time.time() + 86400
              and all(receipt.get(k) == v for k, v in expected.items()), 'independent-review-required')
    Q.require(os.getsid(0) == os.getpid(), 'detached-owned-supervisor-required')
    owner = M.decode(M.read(BASE / 'supervisor-owner.json'))
    Q.require(owner == {'pid': os.getpid(), 'sid': os.getpid(),
              'start': Q.process_stat(os.getpid())['start'],
              'driver_sha256': expected['driver_sha256'], 'scope': REVIEW_SCOPE},
              'durable-supervisor-owner-conflict')


def probe():
    os.umask(0o077)
    prepared = M.decode(M.read(BASE / 'prepared.json'))
    reviewed_probe(prepared)
    Q.require(tool_hash() == prepared['tool_sha256']
              and prepared['source_snapshot'] == Q.SOURCE_SNAPSHOT
              and prepared['candidate_sha256'] == Q.CANDIDATE_SHA
              and Q.sha(M.read(BASE / 'helper/main.rs')) == prepared['fixture_source_sha256']
              and Q.sha(M.read(BASE / 'helper/dev_completion.rs')) == prepared['completion_source_sha256']
              == Q.sha(M.read(Q.ROOT / 'service/src/dev_completion.rs', source=True)))
    preserved_raw = M.read(Q.PRIVATE / 'pipeline-qa-v4-retained-containers.json')
    Q.require(Q.sha(preserved_raw) == 'a1d84425437ffde6e39272a28773efaa30621622862ffd836bd3eba104067afb')
    preserved = M.decode(preserved_raw)
    before = Q.static_check()
    result = {'status': 'blocked', 'backend': 'cpu', 'warmup_disabled': True, 'generation_requests': 0,
              'candidate_sha256': Q.CANDIDATE_SHA, 'source_snapshot': Q.SOURCE_SNAPSHOT,
              'endpoint_counts': {}, 'steps': []}
    child = model = global_lock = local_lock = None
    drains = []
    stderr_errors = []
    with C.cancellation_owner():
        def phase(name):
            if not name.startswith('finished-'):
                result['last_phase'] = name
            M.publish(BASE / 'progress.json', {'phase': name, 'supervisor_pid': os.getpid()})
        def helper(*args):
            C.checkpoint()
            Q.require(tool_hash() == prepared['tool_sha256'])
            output = C.owned_command([str(TOOL), *map(str, args)], Q.environment(), timeout=60, limit=4096)
            value = M.decode(output)
            value['operation'] = args[0]
            result['steps'].append(value)
            Q.require(value.get('code') == 'OK', 'production-validator-refused')
            return value
        try:
            M.write(BASE / 'attempted.json', M.encoded({'scope': 'startup-and-template-only-no-generation'}))
            roots, installed, _ = Q.live_check(before)
            Q.require(Q.retained_containers() == preserved, 'retained-container-drift')
            global_path = roots['--runtime-root'] / 'smith-next/runner.lock'
            global_meta = global_path.lstat()
            Q.require(stat.S_ISREG(global_meta.st_mode) and global_meta.st_uid == os.geteuid()
                      and stat.S_IMODE(global_meta.st_mode) == 0o600 and global_meta.st_nlink == 1)
            global_lock = os.open(global_path, os.O_RDWR | os.O_NOFOLLOW | os.O_NONBLOCK)
            fcntl.flock(global_lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            Q.require(M.identity(os.fstat(global_lock)) == M.identity(global_path.lstat()))
            Q.live_check(before)
            # Read-only reuse of v3's verified copy; never rewrite its state/package/model.
            copies = Q.isolated_roots(roots, '159361a5dbe74646a23f2878d51de89a')
            identity = uuid.uuid4().hex
            # Use the standard v5 mapping and full-ID journal for this diagnostic owner.
            copies['--runtime-root'] = Q.isolated_roots(roots, identity)['--runtime-root']
            Q.claim_runtime(copies, identity, 'native-startup-no-generation')
            result['validation_id'] = identity
            result['socket_bytes'] = Q.socket_budget(copies)
            namespace = copies['--data-root'] / 'smith-next'
            saved_states = D.hashes(namespace)
            M.write(BASE / 'roots.json', M.encoded({k: str(v) for k, v in copies.items()}))
            generated = helper('make', BASE / 'roots.json', BASE / 'probe-state.json', BASE / 'probe-request.json')
            state_sha = Q.sha(M.read(BASE / 'probe-state.json'))
            current = os.readlink(namespace / 'client/current')
            cpu = namespace / 'client' / current / 'smith_backend/runtime/cpu'
            executable = cpu / 'llama-server'
            model_path = namespace / 'models' / (Q.MODEL_SHA + '.gguf')
            model = os.open(model_path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
            meta = os.fstat(model)
            Q.require(stat.S_ISREG(meta.st_mode) and meta.st_uid == os.geteuid() and meta.st_nlink == 1
                      and stat.S_IMODE(meta.st_mode) == 0o600 and meta.st_size == Q.MODEL_SIZE)
            digest = hashlib.sha256()
            until = time.monotonic() + 60
            while True:
                C.checkpoint()
                Q.require(time.monotonic() < until, 'model-verification-deadline')
                block = os.read(model, 1024 * 1024)
                if not block:
                    break
                digest.update(block)
            Q.require(digest.hexdigest() == Q.MODEL_SHA and M.file_identity(os.fstat(model)) == M.file_identity(meta))
            available = next(int(v.split()[1]) * 1024 for v in Path('/proc/meminfo').read_text().splitlines()
                             if v.startswith('MemAvailable:'))
            Q.require(available > Q.MODEL_SIZE + 1024 * 1024 * 1024, 'memory-headroom-refused')
            parent = copies['--runtime-root'] / 'smith-next'
            Q.make_directory(parent)
            M.write(parent / 'runner.lock', b'')
            local_lock = os.open(parent / 'runner.lock', os.O_RDWR | os.O_NOFOLLOW)
            fcntl.flock(local_lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            runner_dir = parent / ('runner-' + uuid.uuid4().hex)
            Q.make_directory(runner_dir)
            endpoint = runner_dir / 'llama.sock'
            Q.require(len(os.fsencode(endpoint)) <= Q.SOCKET_LIMIT)
            key = uuid.uuid4().hex
            os.lseek(model, 0, os.SEEK_SET)
            parent_pid = os.getpid()
            argv = [str(executable), '--model', '/proc/self/fd/' + str(model), '--host', str(endpoint),
                '--offline', '--no-webui', '--no-slots', '--no-agent', '--no-mmproj-auto', '--no-cache-idle-slots',
                '--cache-ram', '0', '--ctx-checkpoints', '0', '--ctx-size', '4096', '--parallel', '1',
                '--threads', str(generated['threads']), '--threads-http', '2', '--timeout', '180',
                '--alias', Q.MODEL_SHA, '--device', 'none', '--gpu-layers', '0', '--fit', 'off', '--jinja', '--no-warmup']
            phase('native-startup')
            C.checkpoint()
            child = subprocess.Popen(argv, cwd=cpu, env={'LLAMA_API_KEY': key}, stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, pass_fds=(global_lock, local_lock, model),
                start_new_session=True, preexec_fn=lambda: M.parent_death_guard(parent_pid))
            M.write(BASE / 'native-owner.json', M.encoded({'pid': child.pid, 'sid': child.pid,
                'executable_sha256': Q.sha(M.read(executable)), 'runtime_root': str(copies['--runtime-root'])}))
            def drain_body():
                with os.fdopen(os.open(BASE / 'native.log', os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), 'wb') as out:
                    total = 0
                    discarding = False
                    while True:
                        raw = child.stderr.readline(65536)
                        if not raw:
                            break
                        if discarding or (len(raw) == 65536 and not raw.endswith(b'\n')):
                            discarding = not raw.endswith(b'\n')
                            continue
                        data = (D.redact(raw.decode(errors='replace'), key, [str(Q.PRIVATE)]) + '\n').encode()
                        if total + len(data) <= 128 * 1024:
                            out.write(data); total += len(data)
                    out.flush(); os.fsync(out.fileno())
            def drain():
                try:
                    drain_body()
                except BaseException:
                    stderr_errors.append('capture-failed')
                finally:
                    child.stderr.close()
            thread = threading.Thread(target=drain, daemon=True)
            thread.start(); drains.append(thread)
            Q.require(Q.sha((Path('/proc') / str(child.pid) / 'exe').read_bytes()) == D.EXE_SHA,
                      'native-pid-executable-conflict')
            deadline = time.monotonic() + 90
            def check():
                C.checkpoint()
                Q.require(time.monotonic() < deadline, 'native-validation-deadline')
                Q.validate_runtime_binding(copies, identity, 'native-startup-no-generation')
                Q.require(D.hashes(installed) == before['states'] and D.hashes(namespace) == saved_states)
                Q.require(os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT) is None,
                          'native-exited-before-validation')
            def http(method, route, body=None):
                check()
                Q.require((method, route) in D.ALLOWED, 'generation-endpoint-forbidden')
                result['endpoint_counts'][route] = result['endpoint_counts'].get(route, 0) + 1
                code, value = D.http(endpoint, child.pid, key, method, route, body)
                result.setdefault('http_statuses', []).append({'method': method, 'path': route, 'status': code})
                return code, value
            startup = time.monotonic() + 60
            while True:
                check()
                Q.require(time.monotonic() < startup, 'startup-deadline')
                if endpoint.is_socket():
                    try:
                        code, _ = http('GET', '/health')
                    except (socket.timeout, ConnectionRefusedError, FileNotFoundError):
                        code = None
                    if code == 200:
                        break
                time.sleep(0.1)
            result['startup_verified'] = True
            phase('props')
            code, props = http('GET', '/props')
            Q.require(code == 200)
            result['caps'] = D.caps_summary(props)['actual_chat_template_caps']
            M.write(BASE / 'props.json', M.encoded(props))
            helper('props', BASE / 'probe-state.json', BASE / 'props.json')
            phase('semantic-probe')
            Q.require(Q.sha(M.read(BASE / 'probe-state.json')) == state_sha)
            code, rendered = http('POST', '/apply-template', M.decode(M.read(BASE / 'probe-request.json')))
            Q.require(code == 200)
            M.write(BASE / 'semantic-rendered.json', M.encoded(rendered))
            helper('verify', BASE / 'probe-state.json', BASE / 'semantic-rendered.json')
            result['semantic_probe_verified'] = True
            phase('actual-template-and-context')
            frames = M.read(Q.PRIVATE / 'pipeline-qa-159361a5dbe74646a23f2878d51de89a/qa-worker-frames.log')
            request = next(M.decode(line)['body'] for line in frames.splitlines() if M.decode(line)['type'] == 'model_request')
            M.write(BASE / 'actual-request.json', M.encoded(request))
            helper('template', BASE / 'actual-request.json', BASE / 'actual-template-input.json')
            code, rendered = http('POST', '/apply-template', M.decode(M.read(BASE / 'actual-template-input.json')))
            Q.require(code == 200 and isinstance(rendered.get('prompt'), str))
            M.write(BASE / 'actual-rendered.json', M.encoded(rendered))
            code, tokens = http('POST', '/tokenize', {'content': rendered['prompt'], 'add_special': True})
            Q.require(code == 200)
            M.write(BASE / 'tokens.json', M.encoded(tokens))
            helper('context', BASE / 'tokens.json')
            result['actual_context_verified'] = True
            result['actual_token_count'] = len(tokens['tokens'])
            result['status'] = 'pass'
        except BaseException:
            result['status'] = 'blocked'
        finally:
            if child is not None:
                Q.end_session(child, lambda name: None)
                for thread in drains:
                    while thread.is_alive():
                        thread.join(0.2)
                if not child.stderr.closed:
                    child.stderr.close()
                while True:
                    try:
                        if not any(line.split() and line.split()[-1] == str(endpoint)
                                   for line in Path('/proc/net/unix').read_text().splitlines()[1:]):
                            break
                    except Exception:
                        pass
                    time.sleep(0.1)  # Unknown kernel socket cleanup retains both leases.
                result['kernel_socket_absent'] = True
                result['native_cleanup_verified'] = True
                result['native_exit_code'] = child.returncode
                result['stderr_capture_ok'] = not stderr_errors
                if stderr_errors:
                    result['status'] = 'blocked'
            try:
                result['installed_states_exact'] = D.hashes(installed) == before['states']
                result['reused_profile_states_exact'] = D.hashes(namespace) == saved_states
                result['retained_containers_exact'] = Q.retained_containers(Q.cleanup_docker, cleanup=True) == preserved
                result['retained_containers_sha256'] = Q.sha(preserved_raw)
                result['retained_container_count'] = len(preserved)
                # Cleanup observations may run after cancellation, but never authorize forward progress.
                if (not result['installed_states_exact'] or not result['reused_profile_states_exact']
                        or not result['retained_containers_exact']):
                    result['status'] = 'blocked'
            except BaseException:
                result['status'] = 'blocked'
            for fd in (model, local_lock, global_lock):
                if fd is not None:
                    os.close(fd)
            M.write(BASE / 'native-proof.json', M.encoded(result))
            phase('finished-' + result['status'])


if __name__ == '__main__':
    try:
        if sys.argv[1:] == ['--prepare']:
            prepare()
        elif sys.argv[1:] == ['--probe']:
            probe()
        else:
            raise RuntimeError('explicit-mode-required')
    except BaseException:
        print('{"status":"refused-private-evidence-only"}', file=sys.stderr)
        sys.exit(1)
