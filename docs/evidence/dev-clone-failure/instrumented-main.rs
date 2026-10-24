#![allow(dead_code)]
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
// A conservative public-unicast allowlist: reject special-use/transition space,
// even where an individual special-purpose address may be globally reachable.
fn public_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let n = u32::from(ip);
            ![
                (0x00000000, 8),
                (0x0a000000, 8),
                (0x64400000, 10),
                (0x7f000000, 8),
                (0xa9fe0000, 16),
                (0xac100000, 12),
                (0xc0000000, 24),
                (0xc0000200, 24),
                (0xc034c100, 24), // AMT relay anycast
                (0xc0586300, 24),
                (0xc0a80000, 16),
                (0xc6120000, 15),
                (0xc6336400, 24),
                (0xcb007100, 24),
                (0xe0000000, 3),
            ]
            .iter()
            .any(|&(base, bits)| n >> (32 - bits) == base >> (32 - bits))
        }
        IpAddr::V6(ip) => {
            let s = ip.segments();
            s[0] & 0xe000 == 0x2000
                && !(s[0] == 0x2001 && s[1] < 0x0200)
                && !(s[0] == 0x2001 && s[1] == 0x0db8)
                && s[0] != 0x2002
                && s[0] != 0x3ffe // retired 6bone transition network
                && !((s[4] == 0 || s[4] == 0x0200) && s[5] == 0x5efe) // ISATAP
                && !(s[0] == 0x3fff && s[1] & 0xf000 == 0)
        }
    }
}
fn approve_addresses(addresses: Vec<SocketAddr>) -> LocalResult<Vec<SocketAddr>> {
    if addresses.is_empty()
        || addresses.len() > 8
        || addresses.iter().any(|a| {
            a.port() != 443
                || !public_address(a.ip())
                || matches!(a, SocketAddr::V6(v) if v.scope_id() != 0 || v.flowinfo() != 0)
        })
    {
        return Err(REJECT);
    }
    let mut unique = Vec::new();
    for a in addresses {
        if !unique.contains(&a) {
            unique.push(a);
        }
    }
    Ok(unique)
}

fn check(until: Instant, cancel: &AtomicBool) -> LocalResult<()> {
    if cancel.load(Ordering::SeqCst) {
        Err(LocalError::Interrupted)
    } else if Instant::now() >= until {
        Err(LocalError::Deadline)
    } else {
        Ok(())
    }
}
fn run<T>(
    until: Instant,
    cancel: &AtomicBool,
    future: impl Future<Output = LocalResult<T>>,
) -> LocalResult<T> {
    check(until, cancel)?;
    eprintln!("stage=runtime_build");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| UNAVAILABLE)?;
    let result = runtime.block_on(async {
        tokio::pin!(future);
        loop {
            check(until, cancel)?;
            tokio::select! {
                biased;
                _ = tokio::time::sleep_until(until.into()) => return Err(LocalError::Deadline),
                _ = tokio::time::sleep(Duration::from_millis(25)) => {},
                result = &mut future => { check(until, cancel)?; return result; }
            }
        }
    });
    // A stalled OS resolver must not make runtime destruction wait past deadline.
    runtime.shutdown_background();
    result
}
fn wire_request(d: &Destination, advertisement: bool, body: &[u8]) -> Vec<u8> {
    let (method, suffix, kind) = if advertisement {
        ("GET", "/info/refs?service=git-upload-pack", AD_TYPE)
    } else {
        ("POST", "/git-upload-pack", PACK_TYPE)
    };
    let content = if advertisement {
        ""
    } else {
        "Content-Type: application/x-git-upload-pack-request\r\n"
    };
    let mut bytes = format!("{method} {}{suffix} HTTP/1.1\r\nHost: {}\r\nAccept: {kind}\r\n{content}Accept-Encoding: identity\r\nUser-Agent: smith-dev-fetch/1\r\nConnection: close\r\nContent-Length: {}\r\n\r\n", d.path, d.authority, body.len()).into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

// Private production transport: no client pool, environment, credentials, proxy,
// redirects, authentication negotiation, or protocol retries.
#[cfg(not(test))]
async fn transport(
    d: &Destination,
    addresses: &[SocketAddr],
    request: &[u8],
    kind: &str,
    limit: usize,
    until: Instant,
) -> LocalResult<Vec<u8>> {
    let connect_until = until.min(Instant::now() + Duration::from_secs(10));
    eprintln!("stage=tls_config");
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let mut config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
    let server_name = ServerName::try_from(d.host.clone()).map_err(|_| REJECT)?;
    let stream = tokio::time::timeout_at(connect_until.into(), async {
        // Only prevalidated IP literals are supplied to connect. At most 8 TCP
        // attempts, one TLS handshake, and one HTTP send; never retry a request.
        for address in addresses {
            eprintln!("stage=tcp_connect");
            if let Ok(tcp) = tokio::net::TcpStream::connect(*address).await {
                eprintln!("stage=tcp_connected");
                return connector
                    .connect(server_name, tcp)
                    .await
                    .map_err(|error| { eprintln!("stage=tls_handshake_error kind={:?}", error.kind()); if let Some(tls) = error.get_ref().and_then(|e| e.downcast_ref::<rustls::Error>()) { eprintln!("tls_error={tls:?}"); } UNAVAILABLE });
            }
        }
        Err(UNAVAILABLE)
    })
    .await
    .map_err(|_| UNAVAILABLE)??;
    eprintln!("stage=tls_connected");
    let mut stream = stream;
    stream.write_all(request).await.map_err(|_| UNAVAILABLE)?;
    stream.flush().await.map_err(|_| UNAVAILABLE)?;
    eprintln!("stage=request_written");
    read_response(stream, kind, limit).await
}

// Parse the raw wire head before any body collection. No normalization can hide
// duplicate framing, obs-fold, excessive whitespace, or informational responses.
async fn line<R: AsyncBufRead + Unpin>(reader: &mut R, budget: &mut usize) -> LocalResult<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let available = reader.fill_buf().await.map_err(|_| UNAVAILABLE)?;
        if available.is_empty() {
            return Err(REJECT);
        }
        let count = available
            .iter()
            .position(|b| *b == b'\n')
            .map_or(available.len(), |p| p + 1);
        if count > *budget {
            return Err(REJECT);
        }
        *budget -= count;
        out.extend_from_slice(&available[..count]);
        reader.consume(count);
        if out.last() == Some(&b'\n') {
            if !out.ends_with(b"\r\n") {
                return Err(REJECT);
            }
            out.truncate(out.len() - 2);
            if out.iter().any(|b| *b < 32 || *b > 126) {
                return Err(REJECT);
            }
            return Ok(out);
        }
    }
}
async fn read_response<R: AsyncRead + Unpin>(
    stream: R,
    kind: &str,
    limit: usize,
) -> LocalResult<Vec<u8>> {
    let mut reader = BufReader::with_capacity(8192, stream);
    let mut budget = HEAD_LIMIT;
    let status = line(&mut reader, &mut budget).await?;
    eprintln!("stage=http_status status={}", String::from_utf8_lossy(&status));
    if !status.starts_with(b"HTTP/1.1 200 ") && !status.starts_with(b"HTTP/1.0 200 ") {
        return Err(REJECT);
    }
    let mut names = HashSet::new();
    let mut length = None;
    let mut chunked = false;
    let mut content_type = false;
    loop {
        let raw = line(&mut reader, &mut budget).await?;
        if raw.is_empty() {
            break;
        }
        let text = std::str::from_utf8(&raw).map_err(|_| REJECT)?;
        let (name, value) = text.split_once(':').ok_or(REJECT)?;
        hyper::header::HeaderName::from_bytes(name.as_bytes()).map_err(|_| REJECT)?;
        let name = name.to_ascii_lowercase();
        let value = value.trim_matches(' ');
        if !names.insert(name.clone()) {
            return Err(REJECT);
        }
        match name.as_str() {
            "content-encoding" | "trailer" => return Err(REJECT),
            "content-type" => {
                if value != kind {
                    return Err(REJECT);
                }
                content_type = true;
            }
            "content-length" => {
                if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(REJECT);
                }
                let n = value.parse::<usize>().map_err(|_| REJECT)?;
                if n > limit {
                    return Err(REJECT);
                }
                length = Some(n);
            }
            "transfer-encoding" => {
                if value != "chunked" {
                    return Err(REJECT);
                }
                chunked = true;
            }
            _ => {}
        }
    }
    if !content_type || (chunked && length.is_some()) {
        return Err(REJECT);
    }
    eprintln!("stage=http_body content_type_ok={content_type} chunked={chunked} length={length:?}");
    let mut bytes = Vec::new();
    if chunked {
        let mut chunks = 0;
        loop {
            chunks += 1;
            if chunks > 65536 {
                return Err(REJECT);
            }
            let size = line(&mut reader, &mut 32).await?;
            if size.is_empty() || size.len() > 16 || !size.iter().all(u8::is_ascii_hexdigit) {
                return Err(REJECT);
            }
            let size = usize::from_str_radix(std::str::from_utf8(&size).map_err(|_| REJECT)?, 16)
                .map_err(|_| REJECT)?;
            if size == 0 {
                if !line(&mut reader, &mut 2).await?.is_empty() {
                    return Err(REJECT);
                }
                break;
            }
            append_exact(&mut reader, &mut bytes, size, limit).await?;
            let mut end = [0; 2];
            reader.read_exact(&mut end).await.map_err(|_| REJECT)?;
            if end != *b"\r\n" {
                return Err(REJECT);
            }
        }
    } else if let Some(n) = length {
        append_exact(&mut reader, &mut bytes, n, limit).await?;
    } else {
        loop {
            let mut chunk = [0; 8192];
            let n = reader.read(&mut chunk).await.map_err(|_| UNAVAILABLE)?;
            if n == 0 {
                break;
            }
            if n > limit.saturating_sub(bytes.len()) {
                return Err(REJECT);
            }
            bytes.extend_from_slice(&chunk[..n]);
        }
    }
    // Static Connection: close forbids a second message; do not ignore trailing
    // bytes, including an undeclared trailer or conflicting Content-Length.
    eprintln!("stage=body_complete bytes={}", bytes.len());
    match reader.read_u8().await {
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => Ok(bytes),
        _ => Err(REJECT),
    }
}
async fn append_exact<R: AsyncRead + Unpin>(
    reader: &mut R,
    out: &mut Vec<u8>,
    n: usize,
    limit: usize,
) -> LocalResult<()> {
    if n > limit.saturating_sub(out.len()) {
        return Err(REJECT);
    }
    let mut remaining = n;
    while remaining != 0 {
        let mut chunk = [0; 8192];
        let count = remaining.min(chunk.len());
        reader
            .read_exact(&mut chunk[..count])
            .await
            .map_err(|_| REJECT)?;
        out.extend_from_slice(&chunk[..count]);
        remaining -= count;
    }
    Ok(())
}

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
