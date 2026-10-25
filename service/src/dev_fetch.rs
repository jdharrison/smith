//! Narrow, credential-free smart HTTP v0 acquisition. No host Git or cloud transport.
use crate::dev_local::{Checkout, LocalError, LocalResult, Repository};
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    net::{IpAddr, SocketAddr},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader};
#[cfg(not(test))]
use {
    std::sync::Arc,
    tokio::io::AsyncWriteExt,
    tokio_rustls::rustls::{self, pki_types::ServerName},
};

const AD_LIMIT: usize = 256 * 1024;
const REQUEST_LIMIT: usize = 16 * 1024;
const PACK_LIMIT: usize = 64 * 1024 * 1024;
const PROGRESS_LIMIT: usize = 64 * 1024;
const HEAD_LIMIT: usize = 16 * 1024;
const REQUESTS: usize = 32;
const AD_TYPE: &str = "application/x-git-upload-pack-advertisement";
const PACK_TYPE: &str = "application/x-git-upload-pack-result";
const REJECT: LocalError = LocalError::RepositoryRejected;
const UNAVAILABLE: LocalError = LocalError::RepositoryUnavailable;

#[derive(Clone)]
struct Destination {
    host: String,
    authority: String,
    path: String,
}
fn destination(repo: &Repository) -> LocalResult<Destination> {
    let url = &repo.url;
    if url.len() > 2048
        || !url.is_ascii()
        || url.bytes().any(|b| b <= 32 || b == 127)
        || url.contains(['%', '?', '#', '@', '\\'])
    {
        return Err(REJECT);
    }
    let rest = url.strip_prefix("https://").ok_or(REJECT)?;
    let (authority, path) = rest.split_once('/').ok_or(REJECT)?;
    let host = authority.strip_suffix(":443").unwrap_or(authority);
    if host.len() > 253 || host.contains(':') || host.parse::<IpAddr>().is_ok() {
        return Err(REJECT);
    }
    let labels: Vec<_> = host.split('.').collect();
    if labels.len() < 2
        || labels.iter().any(|l| {
            l.is_empty()
                || l.len() > 63
                || l.starts_with('-')
                || l.ends_with('-')
                || !l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
        || !labels
            .last()
            .unwrap()
            .bytes()
            .any(|b| b.is_ascii_alphabetic())
    {
        return Err(REJECT);
    }
    let lower = host.to_ascii_lowercase();
    if [
        "localhost",
        "local",
        "internal",
        "home",
        "lan",
        "test",
        "invalid",
        "example",
        "onion",
    ]
    .iter()
    .any(|suffix| lower == *suffix || lower.ends_with(&format!(".{suffix}")))
    {
        return Err(REJECT);
    }
    if path.is_empty()
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
        || !path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/-_.~".contains(&b))
    {
        return Err(REJECT);
    }
    if let Some(reference) = &repo.reference {
        if !safe_branch(reference) {
            return Err(REJECT);
        }
    }
    Ok(Destination {
        host: host.to_owned(),
        authority: authority.to_owned(),
        path: format!("/{path}"),
    })
}
pub(crate) fn validate_repository(repo: &Repository) -> LocalResult<()> {
    destination(repo).map(|_| ())
}
pub(crate) fn branch_name(reference: &str) -> &str {
    reference.strip_prefix("refs/heads/").unwrap_or(reference)
}
pub(crate) fn checkout_reference(reference: Option<&str>) -> String {
    reference
        .map(|b| format!("refs/heads/{}", branch_name(b)))
        .unwrap_or_else(|| "HEAD".to_owned())
}
fn safe_branch(s: &str) -> bool {
    !s.is_empty()
        && s != "HEAD"
        && !s.starts_with('-')
        && s.len() <= 255
        && !s.contains("..")
        && !s.contains("@{")
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/-_.".contains(&b))
        && s.split('/').all(|p| {
            !p.is_empty() && !p.starts_with('.') && !p.ends_with('.') && !p.ends_with(".lock")
        })
}

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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Advertisement,
    Pack,
    Complete,
}
struct Entry {
    repository: Repository,
    destination: Destination,
    phase: Phase,
    pin: Option<Checkout>,
    capabilities: HashSet<String>,
    advertised: HashSet<String>,
    addresses: Option<Vec<SocketAddr>>,
}
pub(crate) struct FetchSession {
    entries: Vec<Entry>,
    sealed: bool,
    requests: usize,
    pack_bytes: usize,
    #[cfg(test)]
    synthetic: std::collections::VecDeque<Synthetic>,
    #[cfg(test)]
    local_transport: Option<TestTransport>,
}
#[cfg(test)]
type TestTransport = Box<dyn FnMut(&[u8]) -> LocalResult<Vec<u8>> + Send>;
impl FetchSession {
    pub(crate) fn new(repositories: Vec<Repository>) -> LocalResult<Self> {
        if repositories.len() > 10 {
            return Err(REJECT);
        }
        let entries = repositories
            .into_iter()
            .map(|repository| {
                Ok(Entry {
                    destination: destination(&repository)?,
                    repository,
                    phase: Phase::Advertisement,
                    pin: None,
                    capabilities: HashSet::new(),
                    advertised: HashSet::new(),
                    addresses: None,
                })
            })
            .collect::<LocalResult<Vec<_>>>()?;
        Ok(Self {
            entries,
            sealed: false,
            requests: 0,
            pack_bytes: 0,
            #[cfg(test)]
            synthetic: Default::default(),
            #[cfg(test)]
            local_transport: None,
        })
    }
    pub(crate) fn request(
        &mut self,
        index: usize,
        advertisement: bool,
        body: &[u8],
        until: Instant,
        cancel: &AtomicBool,
    ) -> LocalResult<(String, Vec<u8>)> {
        if self.sealed {
            return Err(REJECT);
        }
        // Reserve and latch before validation/I/O; failure never permits a retry.
        self.sealed = true;
        check(until, cancel)?;
        if self.requests >= REQUESTS {
            return Err(REJECT);
        }
        self.requests += 1;
        let entry = self.entries.get_mut(index).ok_or(REJECT)?;
        let mut selected = None;
        let (kind, limit) = if advertisement {
            if entry.phase != Phase::Advertisement || !body.is_empty() {
                return Err(REJECT);
            }
            (AD_TYPE, AD_LIMIT)
        } else {
            let caps = validate_pack_request(
                body,
                &entry.pin.as_ref().ok_or(REJECT)?.commit,
                &entry.capabilities,
                &entry.advertised,
                entry.phase,
            )?;
            selected = Some(caps);
            (
                PACK_TYPE,
                PACK_LIMIT.checked_sub(self.pack_bytes).ok_or(REJECT)?,
            )
        };
        let request = wire_request(&entry.destination, advertisement, body);
        #[cfg(not(test))]
        let bytes = run(until, cancel, async {
            if entry.addresses.is_none() {
                // Iterate under the original deadline, never use hostname connect.
                let mut answers = Vec::new();
                for address in tokio::net::lookup_host((entry.destination.host.as_str(), 443))
                    .await
                    .map_err(|_| UNAVAILABLE)?
                {
                    if answers.len() == 8 {
                        return Err(REJECT);
                    }
                    answers.push(address);
                }
                entry.addresses = Some(approve_addresses(answers)?);
            }
            transport(
                &entry.destination,
                entry.addresses.as_ref().ok_or(REJECT)?,
                &request,
                kind,
                limit,
                until,
            )
            .await
        })?;
        #[cfg(test)]
        let bytes = {
            // Explicit fixtures only: never fall through to production DNS/TLS.
            let response = if let Some(transport) = &mut self.local_transport {
                if !self.synthetic.is_empty() {
                    return Err(REJECT);
                }
                transport(&request)?
            } else {
                let fixture = self.synthetic.pop_front().ok_or(UNAVAILABLE)?;
                if fixture.request != request {
                    return Err(REJECT);
                }
                if entry.addresses.is_none() {
                    entry.addresses = Some(approve_addresses(fixture.addresses)?);
                }
                fixture.response
            };
            run(
                until,
                cancel,
                read_response(response.as_slice(), kind, limit),
            )?
        };
        let bytes = if advertisement {
            let ad = parse_advertisement(&entry.repository, &bytes)?;
            entry.pin = Some(ad.pin);
            entry.capabilities = ad.capabilities;
            entry.advertised = ad.advertised;
            entry.phase = Phase::Pack;
            ad.bytes
        } else {
            self.pack_bytes = self.pack_bytes.checked_add(bytes.len()).ok_or(REJECT)?;
            validate_pack_response(&bytes, selected.as_ref().ok_or(REJECT)?)?;
            entry.phase = Phase::Complete;
            bytes
        };
        check(until, cancel)?;
        self.sealed = false;
        Ok((kind.to_owned(), bytes))
    }
    pub(crate) fn verify_checkouts(&mut self, checkouts: &[Checkout]) -> LocalResult<()> {
        let was_sealed = self.sealed;
        self.sealed = true;
        if was_sealed || checkouts.len() != self.entries.len() {
            return Err(REJECT);
        }
        for (entry, checkout) in self.entries.iter().zip(checkouts) {
            if entry.phase != Phase::Complete || entry.pin.as_ref() != Some(checkout) {
                return Err(REJECT);
            }
        }
        Ok(())
    }
    pub(crate) fn pinned_checkouts(&self) -> LocalResult<Vec<Checkout>> {
        self.entries
            .iter()
            .map(|entry| entry.pin.clone().ok_or(REJECT))
            .collect()
    }
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
            if let Ok(tcp) = tokio::net::TcpStream::connect(*address).await {
                return connector
                    .connect(server_name, tcp)
                    .await
                    .map_err(|_| UNAVAILABLE);
            }
        }
        Err(UNAVAILABLE)
    })
    .await
    .map_err(|_| UNAVAILABLE)??;
    let mut stream = stream;
    stream.write_all(request).await.map_err(|_| UNAVAILABLE)?;
    stream.flush().await.map_err(|_| UNAVAILABLE)?;
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

// v0 permits only data pkt-lines and flush (0000); reject v1/v2 delimiters.
fn packet<'a>(bytes: &mut &'a [u8]) -> LocalResult<Option<&'a [u8]>> {
    if bytes.len() < 4 || !bytes[..4].iter().all(u8::is_ascii_hexdigit) {
        return Err(REJECT);
    }
    let n = usize::from_str_radix(std::str::from_utf8(&bytes[..4]).map_err(|_| REJECT)?, 16)
        .map_err(|_| REJECT)?;
    if n == 0 {
        *bytes = &bytes[4..];
        return Ok(None);
    }
    if !(4..=65520).contains(&n) || n > bytes.len() {
        return Err(REJECT);
    }
    let data = &bytes[4..n];
    *bytes = &bytes[n..];
    Ok(Some(data))
}
fn oid(s: &str) -> bool {
    s.len() == 40
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && s.bytes().any(|b| b != b'0')
}
struct Advertisement {
    pin: Checkout,
    advertised: HashSet<String>,
    capabilities: HashSet<String>,
    bytes: Vec<u8>,
}
fn append_packet(out: &mut Vec<u8>, data: &[u8]) {
    out.extend_from_slice(format!("{:04x}", data.len() + 4).as_bytes());
    out.extend_from_slice(data);
}
fn valid_agent(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._-+".contains(&b))
}
fn parse_advertisement(repo: &Repository, mut bytes: &[u8]) -> LocalResult<Advertisement> {
    if bytes.len() > AD_LIMIT
        || packet(&mut bytes)? != Some(b"# service=git-upload-pack\n".as_slice())
        || packet(&mut bytes)?.is_some()
    {
        return Err(REJECT);
    }
    let desired = checkout_reference(repo.reference.as_deref());
    let mut capabilities = HashSet::new();
    let mut retained = Vec::new();
    let mut refs = HashMap::new();
    let mut commit = None;
    let mut first = true;
    let mut worker = Vec::new();
    append_packet(&mut worker, b"# service=git-upload-pack\n");
    worker.extend_from_slice(b"0000");
    while let Some(data) = packet(&mut bytes)? {
        let text = std::str::from_utf8(data).map_err(|_| REJECT)?;
        let text = text.strip_suffix('\n').unwrap_or(text);
        let (record, caps) = if let Some((r, c)) = text.split_once('\0') {
            if !first || c.is_empty() {
                return Err(REJECT);
            }
            (r, Some(c))
        } else {
            (text, None)
        };
        if let Some(caps) = caps {
            if caps.len() > 8192 {
                return Err(REJECT);
            }
            let mut names = HashSet::new();
            for cap in caps.split(' ') {
                let name = cap.split('=').next().ok_or(REJECT)?;
                if cap.is_empty()
                    || !cap.bytes().all(|b| (33..=126).contains(&b))
                    || !names.insert(name)
                {
                    return Err(REJECT);
                }
                let keep = match name {
                    "multi_ack" | "multi_ack_detailed" | "thin-pack" | "side-band"
                    | "side-band-64k" | "ofs-delta" | "no-progress" => {
                        if cap != name {
                            return Err(REJECT);
                        }
                        true
                    }
                    "agent" => {
                        if !cap.strip_prefix("agent=").is_some_and(valid_agent) {
                            return Err(REJECT);
                        }
                        true
                    }
                    "object-format" => {
                        if cap != "object-format=sha1" {
                            return Err(REJECT);
                        }
                        true
                    }
                    "symref" => {
                        let target = cap.strip_prefix("symref=HEAD:").ok_or(REJECT)?;
                        if !target.strip_prefix("refs/").is_some_and(safe_branch) {
                            return Err(REJECT);
                        }
                        true
                    }
                    _ => false,
                };
                if keep {
                    capabilities.insert(cap.to_owned());
                    retained.push(cap);
                }
            }
        }
        if first {
            let mut record = record.as_bytes().to_vec();
            record.push(0);
            record.extend_from_slice(retained.join(" ").as_bytes());
            if data.ends_with(b"\n") {
                record.push(b'\n');
            }
            append_packet(&mut worker, &record);
        } else {
            // Preserve upstream ref payloads, including peeled-tag records.
            append_packet(&mut worker, data);
        }
        first = false;
        let (hash, name) = record.split_once(' ').ok_or(REJECT)?;
        let refname = name.strip_suffix("^{}").unwrap_or(name);
        if !oid(hash)
            || !(name == "HEAD" || refname.strip_prefix("refs/").is_some_and(safe_branch))
            || refs.insert(name.to_owned(), hash.to_owned()).is_some()
        {
            return Err(REJECT);
        }
        if name == desired {
            commit = Some(hash.to_owned());
        }
    }
    if !bytes.is_empty()
        || !(capabilities.contains("side-band") || capabilities.contains("side-band-64k"))
    {
        return Err(REJECT);
    }
    if let Some(target) = retained.iter().find_map(|c| c.strip_prefix("symref=HEAD:")) {
        if !refs.contains_key(target) || refs.get("HEAD") != refs.get(target) {
            return Err(REJECT);
        }
    }
    worker.extend_from_slice(b"0000");
    Ok(Advertisement {
        pin: Checkout {
            url: repo.url.clone(),
            reference: desired,
            commit: commit.ok_or(REJECT)?,
        },
        capabilities,
        advertised: refs.into_values().collect(),
        bytes: worker,
    })
}
// Text pkt-lines permit exactly one optional terminal LF, never other controls.
fn packet_text(data: &[u8]) -> LocalResult<&str> {
    let text = data.strip_suffix(b"\n").unwrap_or(data);
    if !text.iter().all(|b| (32..=126).contains(b)) {
        return Err(REJECT);
    }
    std::str::from_utf8(text).map_err(|_| REJECT)
}
fn validate_pack_request(
    body: &[u8],
    pinned: &str,
    advertised: &HashSet<String>,
    advertised_oids: &HashSet<String>,
    phase: Phase,
) -> LocalResult<HashSet<String>> {
    if body.len() > REQUEST_LIMIT || phase != Phase::Pack {
        return Err(REJECT);
    }
    let mut bytes = body;
    let want = packet(&mut bytes)?.ok_or(REJECT)?;
    let want = packet_text(want)?;
    let mut tokens = want.split(' ');
    if tokens.next() != Some("want") {
        return Err(REJECT);
    }
    let first = tokens.next().ok_or(REJECT)?;
    if !oid(first) || !advertised_oids.contains(first) {
        return Err(REJECT);
    }
    let mut seen = HashSet::new();
    let mut selected = HashSet::new();
    let mut ack = false;
    let mut side = false;
    for cap in tokens {
        let name = cap.split('=').next().ok_or(REJECT)?;
        if !seen.insert(name) {
            return Err(REJECT);
        }
        match cap {
            "multi_ack" | "multi_ack_detailed" => {
                if ack {
                    return Err(REJECT);
                }
                ack = true;
            }
            "side-band" | "side-band-64k" => {
                if side {
                    return Err(REJECT);
                }
                side = true;
            }
            "thin-pack" | "ofs-delta" | "no-progress" | "object-format=sha1" => {}
            _ if cap.starts_with("agent=") => {
                let agent = &cap[6..];
                if !valid_agent(agent) {
                    return Err(REJECT);
                }
            }
            _ => return Err(REJECT),
        }
        // The client identifies itself, not the server; all other values are exact.
        if !advertised.contains(cap)
            && !(name == "agent" && advertised.iter().any(|a| a.starts_with("agent=")))
        {
            return Err(REJECT);
        }
        selected.insert(cap.to_owned());
    }
    if !side {
        return Err(REJECT);
    }
    let mut wants = HashSet::from([first]);
    while let Some(data) = packet(&mut bytes)? {
        let text = packet_text(data)?;
        let hash = text.strip_prefix("want ").ok_or(REJECT)?;
        if !oid(hash) || !advertised_oids.contains(hash) {
            return Err(REJECT);
        }
        // Native clone can repeat an object wanted through multiple refs.
        wants.insert(hash);
    }
    if !wants.contains(pinned) || packet_text(packet(&mut bytes)?.ok_or(REJECT)?)? != "done" {
        return Err(REJECT);
    }
    if !bytes.is_empty() && packet(&mut bytes)?.is_some() {
        return Err(REJECT);
    }
    if !bytes.is_empty() {
        return Err(REJECT);
    }
    Ok(selected)
}
fn validate_pack_response(mut bytes: &[u8], selected: &HashSet<String>) -> LocalResult<()> {
    if bytes.len() > PACK_LIMIT {
        return Err(REJECT);
    }
    let packet_limit = if selected.contains("side-band-64k") {
        65520
    } else if selected.contains("side-band") {
        1000
    } else {
        return Err(REJECT);
    };
    if packet_text(packet(&mut bytes)?.ok_or(REJECT)?)? != "NAK" {
        return Err(REJECT);
    }
    let mut pack_bytes = 0usize;
    let mut progress_bytes = 0usize;
    while let Some(data) = packet(&mut bytes)? {
        let (&channel, payload) = data.split_first().ok_or(REJECT)?;
        if data.len() + 4 > packet_limit || payload.is_empty() {
            return Err(REJECT);
        }
        match channel {
            1 => {
                // Only the framing/signature is certified here. Native Git must
                // validate pack integrity; this is not object/history exclusion.
                for (i, &b) in payload
                    .iter()
                    .take(4usize.saturating_sub(pack_bytes))
                    .enumerate()
                {
                    if b != b"PACK"[pack_bytes + i] {
                        return Err(REJECT);
                    }
                }
                pack_bytes = pack_bytes.checked_add(payload.len()).ok_or(REJECT)?;
            }
            2 => {
                progress_bytes = progress_bytes.checked_add(payload.len()).ok_or(REJECT)?;
                if progress_bytes > PROGRESS_LIMIT {
                    return Err(REJECT);
                }
            }
            _ => return Err(REJECT),
        }
    }
    if !bytes.is_empty() || pack_bytes <= 4 {
        return Err(REJECT);
    }
    Ok(())
}

#[cfg(test)]
struct Synthetic {
    addresses: Vec<SocketAddr>,
    request: Vec<u8>,
    response: Vec<u8>,
}
#[cfg(test)]
#[path = "dev_fetch_tests.rs"]
mod tests;
