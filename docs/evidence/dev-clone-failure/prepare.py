"""Generate an isolated, non-test build of the production fetch transport.
No production file is written. The executable has one fixed public GET only.
"""
from pathlib import Path
import hashlib
import json

here = Path(__file__).resolve().parent
root = here.parents[2]
source = root / "service/src/dev_fetch.rs"
text = source.read_text()
lines = text.splitlines(keepends=True)
# Copy production address validation, run, wire request, TLS and HTTP parser.
selected = ''.join(lines[129:186]) + '\n' + ''.join(lines[372:624])
# Preserve control flow and errors; only add local, bounded diagnostic markers.
selected = selected.replace('let runtime = tokio::runtime::Builder', 'eprintln!("stage=runtime_build");\n    let runtime = tokio::runtime::Builder')
selected = selected.replace('let mut roots = rustls::RootCertStore::empty();', 'eprintln!("stage=tls_config");\n    let mut roots = rustls::RootCertStore::empty();')
selected = selected.replace('if let Ok(tcp) = tokio::net::TcpStream::connect(*address).await {', 'eprintln!("stage=tcp_connect");\n            if let Ok(tcp) = tokio::net::TcpStream::connect(*address).await {\n                eprintln!("stage=tcp_connected");')
selected = selected.replace('.map_err(|_| UNAVAILABLE);', '.map_err(|error| { eprintln!("stage=tls_handshake_error kind={:?}", error.kind()); if let Some(tls) = error.get_ref().and_then(|e| e.downcast_ref::<rustls::Error>()) { eprintln!("tls_error={tls:?}"); } UNAVAILABLE });')
selected = selected.replace('let mut stream = stream;', 'eprintln!("stage=tls_connected");\n    let mut stream = stream;')
selected = selected.replace('read_response(stream, kind, limit).await', 'eprintln!("stage=request_written");\n    read_response(stream, kind, limit).await')
selected = selected.replace('let status = line(&mut reader, &mut budget).await?;', 'let status = line(&mut reader, &mut budget).await?;\n    eprintln!("stage=http_status status={}", String::from_utf8_lossy(&status));')
selected = selected.replace('let mut bytes = Vec::new();\n    if chunked {', 'eprintln!("stage=http_body content_type_ok={content_type} chunked={chunked} length={length:?}");\n    let mut bytes = Vec::new();\n    if chunked {')
selected = selected.replace('match reader.read_u8().await {', 'eprintln!("stage=body_complete bytes={}", bytes.len());\n    match reader.read_u8().await {')
prefix = r'''#![allow(dead_code)]
use std::{collections::HashSet, future::Future, net::{IpAddr, SocketAddr}, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::{Duration, Instant}};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio_rustls::rustls::{self, pki_types::ServerName};
#[derive(Debug, Clone, Copy)]
enum LocalError { RepositoryRejected, RepositoryUnavailable, Interrupted, Deadline }
type LocalResult<T> = Result<T, LocalError>;
const REJECT: LocalError = LocalError::RepositoryRejected;
const UNAVAILABLE: LocalError = LocalError::RepositoryUnavailable;
const HEAD_LIMIT: usize = 16 * 1024;
const AD_TYPE: &str = "application/x-git-upload-pack-advertisement";
const PACK_TYPE: &str = "application/x-git-upload-pack-result";
struct Destination { host: String, authority: String, path: String }
'''
suffix = r'''
fn main() {
    let started = Instant::now();
    let until = started + Duration::from_secs(20);
    let cancel = AtomicBool::new(false);
    let d = Destination { host: "github.com".into(), authority: "github.com".into(), path: "/jdharrison/smith".into() };
    let request = wire_request(&d, true, &[]);
    let result = run(until, &cancel, async {
        eprintln!("stage=dns_start");
        let mut answers = Vec::new();
        for address in tokio::net::lookup_host((d.host.as_str(), 443)).await.map_err(|_| UNAVAILABLE)? {
            if answers.len() == 8 { return Err(REJECT); }
            answers.push(address);
        }
        eprintln!("stage=dns_complete count={}", answers.len());
        let addresses = approve_addresses(answers)?;
        eprintln!("stage=addresses_approved count={}", addresses.len());
        transport(&d, &addresses, &request, AD_TYPE, 256 * 1024, until).await
    });
    match result {
        Ok(bytes) => {
            let marker = b"001e# service=git-upload-pack\n0000";
            println!("result=ok bytes={} git_service_marker={} elapsed_ms={}", bytes.len(), bytes.starts_with(marker), started.elapsed().as_millis());
        }
        Err(error) => { println!("result=error error={error:?} elapsed_ms={}", started.elapsed().as_millis()); std::process::exit(2); }
    }
}
'''
(here / 'src/main.rs').write_text(prefix + selected + suffix)
(here / 'source-binding.json').write_text(json.dumps({
    'source': 'service/src/dev_fetch.rs',
    'source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
    'copied_lines': ['130-186', '373-624'],
    'instrumentation': 'Stage markers and bounded TLS error taxonomy only; same production control flow/error mapping. DNS loop copied from request. No cfg(test).',
    'fixed_effect': 'One credential-free GET https://github.com/jdharrison/smith/info/refs?service=git-upload-pack; no redirects/auth/POST/clone/job/model/service/Docker.',
}, indent=2) + '\n')
print('diagnostic_source_prepared=true')
