"""Offline native-Git shallow mismatch reproduction. Loopback fixture only."""
from pathlib import Path
from http.server import BaseHTTPRequestHandler, HTTPServer
import hashlib
import json
import subprocess
import tempfile
import threading

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SOURCE = ROOT / 'service/src/dev_fetch.rs'
original = SOURCE.read_bytes()
wrapper = b'''
// Diagnostic visibility adapter only; production parser above is unchanged.
pub(crate) fn sanitize_fixture(bytes: &[u8]) -> LocalResult<Vec<u8>> {
    let repo = Repository { url: "https://github.com/jdharrison/smith".into(), reference: Some("main".into()) };
    parse_advertisement(&repo, bytes).map(|ad| ad.bytes)
}
'''
(HERE / 'repro-dev-fetch.rs').write_bytes(original + wrapper)
main = (HERE / 'src/main.rs').read_text()
stubs = main[:main.index('#[path =')]
main = stubs + '''#[path = "../../repro-dev-fetch.rs"]
mod dev_fetch;
fn main() {
    let input = std::fs::read("synthetic-advertisement.bin").unwrap();
    let output = dev_fetch::sanitize_fixture(&input).expect("production advertisement sanitizer");
    std::fs::write("sanitized-advertisement.bin", &output).unwrap();
    println!("sanitizer=ok input_bytes={} output_bytes={}", input.len(), output.len());
}
'''
(HERE / 'src/bin/shallow_repro.rs').write_text(main)

def packet(payload):
    return f'{len(payload)+4:04x}'.encode() + payload

commit = b'1' * 40
caps = b'multi_ack thin-pack side-band side-band-64k ofs-delta shallow no-progress multi_ack_detailed symref=HEAD:refs/heads/main object-format=sha1 agent=git/2.43.0'
ad = packet(b'# service=git-upload-pack\n') + b'0000'
ad += packet(commit + b' HEAD\0' + caps + b'\n')
ad += packet(commit + b' refs/heads/main\n') + b'0000'
(HERE / 'synthetic-advertisement.bin').write_bytes(ad)

build = subprocess.run(['cargo', 'build', '--offline', '--locked', '--bin', 'shallow_repro'], cwd=HERE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=120)
text = build.stdout.decode(errors='replace').replace(str(ROOT), '<workspace>')
(HERE / 'shallow-repro-build.log').write_text(text)
assert build.returncode == 0, text[:8000]
sanitize = subprocess.run([str(HERE / 'target/debug/shallow_repro')], cwd=HERE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=10)
assert sanitize.returncode == 0, sanitize.stdout.decode(errors='replace')[:2000]
sanitized = (HERE / 'sanitized-advertisement.bin').read_bytes()
sanitized_caps = sanitized.split(b'\0', 1)[1].split(b'\n', 1)[0].split(b' ')
assert b'shallow' not in sanitized_caps
assert b'side-band-64k' in sanitized_caps
records = []

class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        records.append({'method': 'GET', 'path': self.path})
        if self.path != '/repo.git/info/refs?service=git-upload-pack':
            self.send_error(403)
            return
        self.send_response(200)
        self.send_header('Content-Type', 'application/x-git-upload-pack-advertisement')
        self.send_header('Content-Length', str(len(sanitized)))
        self.end_headers()
        self.wfile.write(sanitized)

    def do_POST(self):
        records.append({'method': 'POST', 'path': self.path})
        # This is only a request-sequence control, never a pack/clone server.
        self.send_error(403, 'Diagnostic fixture deliberately supplies no pack')

server = HTTPServer(('127.0.0.1', 0), Handler)
server.timeout = 1
thread = threading.Thread(target=server.serve_forever, daemon=True)
thread.start()
url = f'http://127.0.0.1:{server.server_port}/repo.git'
results = []
try:
    with tempfile.TemporaryDirectory(prefix='native-git-', dir=HERE) as tmp:
        env = {'PATH': '/usr/bin:/bin', 'HOME': tmp, 'XDG_CONFIG_HOME': tmp, 'LANG': 'C', 'LC_ALL': 'C', 'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': '/dev/null', 'GIT_TERMINAL_PROMPT': '0', 'GIT_ALLOW_PROTOCOL': 'http'}
        version = subprocess.run(['/usr/bin/git', '--version'], env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=5).stdout.decode().strip()
        base = ['/usr/bin/git', '-c', 'protocol.version=0', '-c', 'http.followRedirects=false', '-c', 'credential.helper=', '-c', 'core.hooksPath=/dev/null', 'clone', '--no-recurse-submodules', '--template=']
        for name, flags in [('old_worker', ['--depth=1', '--single-branch', '--no-tags']), ('control_without_depth', ['--single-branch', '--no-tags'])]:
            begin = len(records)
            args = base + flags + ['--branch', 'main', '--', url, str(Path(tmp) / name)]
            run = subprocess.run(args, cwd=HERE, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=15)
            def safe(data):
                return data.decode(errors='replace').replace(str(Path(tmp)), '<fixture>').replace(url, '<loopback-repository>')
            result = {'case': name, 'flags': flags, 'exit': run.returncode, 'stdout': safe(run.stdout), 'stderr': safe(run.stderr), 'requests': records[begin:]}
            result['pack_posts'] = sum(x['method'] == 'POST' for x in result['requests'])
            results.append(result)
finally:
    server.shutdown()
    thread.join(timeout=3)
    server.server_close()

assert SOURCE.read_bytes() == original, 'production source changed concurrently'
report = {'source': 'service/src/dev_fetch.rs', 'source_sha256': hashlib.sha256(original).hexdigest(), 'production_parser_copy_byte_identical_before_visibility_adapter': True, 'sanitizer_exit': sanitize.returncode, 'sanitizer_output': sanitize.stdout.decode().strip(), 'input_has_shallow': b'shallow' in caps.split(), 'sanitized_has_shallow': b'shallow' in sanitized_caps, 'git_version': version, 'network_scope': 'loopback only; static synthetic advertisement; no pack served; no external DNS/TLS/HTTP', 'cases': results}
(HERE / 'shallow-repro-result.json').write_text(json.dumps(report, indent=2) + '\n')
passed = results[0]['exit'] != 0 and results[0]['pack_posts'] == 0 and len(results[0]['requests']) == 1 and 'does not support shallow' in results[0]['stderr'] and results[1]['pack_posts'] == 1
report_summary = {'repro_pass': passed, **report}
print(json.dumps(report_summary, indent=2)[:18000])
raise SystemExit(0 if passed else 1)
