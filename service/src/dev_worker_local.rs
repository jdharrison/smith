//! Native, network-isolated local inspection worker. Never used by the receipt-only worker.
use crate::dev_fetch::{branch_name, checkout_reference};
use crate::dev_local::{self as protocol, Checkout, Frame, Repository};
use crate::dev_read::{self, Reader};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, Read};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

type WResult<T> = std::result::Result<T, &'static str>;
const GUARD: &[u8] = include_bytes!("../dev-worker/guard.mjs");
/// The host can bind Ready to the same compiled-in plugin without reading worker-controlled files.
pub(crate) fn guard_sha256() -> String {
    format!("{:x}", Sha256::digest(GUARD))
}
const MAX_HTTP: usize = 128 * 1024;
const MAX_PACK: usize = 64 * 1024 * 1024;
const MAX_AD: usize = 256 * 1024;
const AD_TYPE: &str = "application/x-git-upload-pack-advertisement";
const PACK_TYPE: &str = "application/x-git-upload-pack-result";

struct Response {
    content_type: String,
    body: Vec<u8>,
}
struct Pending {
    tx: mpsc::SyncSender<WResult<Response>>,
    cap: usize,
    content_type: Option<String>,
    body: Vec<u8>,
}
struct Shared {
    job: String,
    until: Instant,
    prep: Instant,
    cancelled: AtomicBool,
    sealed: AtomicBool,
    fetch_closed: AtomicBool,
    guard_ready: AtomicBool,
    dispatched: AtomicBool,
    inference: Mutex<Option<Instant>>,
    output: Mutex<File>,
    pending: Mutex<HashMap<u32, Pending>>,
    sequence: AtomicU32,
    sha: String,
    reader: Mutex<Option<Reader>>,
    active_reads: Mutex<HashSet<String>>,
}
impl Shared {
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
    fn stopped(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst) || Instant::now() >= self.deadline()
    }
    fn deadline(&self) -> Instant {
        let deadline = if self.sealed.load(Ordering::SeqCst) {
            self.until
        } else {
            self.prep
        };
        match self.inference.lock() {
            Ok(t) => t.map_or(deadline, |i| i.min(deadline)),
            Err(_) => Instant::now(),
        }
    }
    fn send(&self, frame: &Frame) -> WResult<()> {
        let mut output = self.output.lock().map_err(|_| "protocol")?;
        protocol::send(&mut *output, frame, self.deadline(), &|| self.stopped())
            .map_err(|_| "protocol")
    }
    fn exchange(&self, frame: impl FnOnce(u32) -> Frame, cap: usize) -> WResult<Response> {
        if self.stopped() {
            return Err("cancelled");
        }
        let sequence = self.sequence.fetch_add(1, Ordering::SeqCst);
        if sequence == 0 || sequence > 1024 {
            return Err("protocol");
        }
        let (tx, rx) = mpsc::sync_channel(1);
        {
            let mut pending = self.pending.lock().map_err(|_| "protocol")?;
            if pending.len() >= 16 {
                return Err("protocol");
            }
            pending.insert(
                sequence,
                Pending {
                    tx,
                    cap,
                    content_type: None,
                    body: Vec::new(),
                },
            );
        }
        self.send(&frame(sequence))?;
        self.await_response(rx)
    }
    fn await_response(&self, rx: mpsc::Receiver<WResult<Response>>) -> WResult<Response> {
        loop {
            if self.stopped() {
                return Err("cancelled");
            }
            match rx.recv_timeout(Duration::from_millis(25)) {
                Ok(result) => return result,
                Err(mpsc::RecvTimeoutError::Timeout) => (),
                Err(_) => return Err("protocol"),
            }
        }
    }
    fn acknowledge(&self, response: Response) -> WResult<()> {
        if response.content_type != "application/json" || response.body != b"true" {
            return Err("protocol");
        }
        Ok(())
    }
}
fn duplicate(fd: i32) -> WResult<File> {
    let raw = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
    if raw < 0 {
        return Err("runtime");
    }
    let owned = unsafe { File::from_raw_fd(raw) };
    crate::dev_worker::nonblocking(raw).map_err(|_| "runtime")?;
    Ok(owned)
}

/// Exactly one input reader after Initialize. Unknown, duplicate and out-of-phase frames cancel.
fn demux(mut input: BufReader<File>, shared: Arc<Shared>, prompts: mpsc::SyncSender<Frame>) {
    let result = (|| -> WResult<()> {
        let mut prompt_seen = false;
        loop {
            let frame = protocol::receive(&mut input, shared.until, &|| shared.stopped())
                .map_err(|_| "protocol")?;
            if frame.request_id() != shared.job {
                return Err("protocol");
            }
            match frame {
                Frame::ResponseStart {
                    sequence,
                    success,
                    content_type,
                    ..
                } => {
                    let mut pending = shared.pending.lock().map_err(|_| "protocol")?;
                    let p = pending.get_mut(&sequence).ok_or("protocol")?;
                    if !success
                        || p.content_type.is_some()
                        || !["application/json", AD_TYPE, PACK_TYPE]
                            .contains(&content_type.as_str())
                    {
                        return Err("protocol");
                    }
                    p.content_type = Some(content_type);
                }
                Frame::ResponseChunk {
                    sequence,
                    bytes,
                    last,
                    ..
                } => {
                    let mut pending = shared.pending.lock().map_err(|_| "protocol")?;
                    let p = pending.get_mut(&sequence).ok_or("protocol")?;
                    if p.content_type.is_none()
                        || bytes.len() > protocol::CHUNK
                        || p.body.len().saturating_add(bytes.len()) > p.cap
                    {
                        return Err("protocol");
                    }
                    p.body.extend_from_slice(&bytes);
                    if last {
                        let p = pending.remove(&sequence).ok_or("protocol")?;
                        p.tx.try_send(Ok(Response {
                            content_type: p.content_type.ok_or("protocol")?,
                            body: p.body,
                        }))
                        .map_err(|_| "protocol")?;
                    }
                }
                frame @ Frame::Prompt { .. } => {
                    if prompt_seen
                        || !shared.guard_ready.load(Ordering::SeqCst)
                        || !shared.sealed.load(Ordering::SeqCst)
                    {
                        return Err("protocol");
                    }
                    prompt_seen = true;
                    prompts.try_send(frame).map_err(|_| "protocol")?;
                }
                _ => return Err("protocol"),
            }
        }
    })();
    if result.is_err() {
        shared.cancel();
    }
}

struct Http {
    method: String,
    path: String,
    content_type: String,
    body: Vec<u8>,
}
fn io_read(
    stream: &mut TcpStream,
    bytes: &mut [u8],
    shared: &Shared,
    until: Instant,
) -> WResult<usize> {
    loop {
        crate::dev_worker::wait_io(stream.as_raw_fd(), libc::POLLIN, until, &|| {
            shared.stopped()
        })
        .map_err(|_| "http")?;
        match stream.read(bytes) {
            Ok(n) => return Ok(n),
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) => {}
            Err(_) => return Err("http"),
        }
    }
}
fn exact(stream: &mut TcpStream, size: usize, shared: &Shared, until: Instant) -> WResult<Vec<u8>> {
    let mut bytes = vec![0; size];
    let mut pos = 0;
    while pos < size {
        let n = io_read(stream, &mut bytes[pos..], shared, until)?;
        if n == 0 {
            return Err("http");
        }
        pos += n;
    }
    Ok(bytes)
}
fn head(stream: &mut TcpStream, shared: &Shared, until: Instant) -> WResult<String> {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        if bytes.len() >= 16 * 1024 {
            return Err("http");
        }
        let byte = exact(stream, 1, shared, until)?;
        bytes.push(byte[0]);
    }
    String::from_utf8(bytes).map_err(|_| "http")
}
fn headers(text: &str) -> WResult<HashMap<String, String>> {
    let mut result = HashMap::new();
    for line in text.split("\r\n").skip(1).filter(|l| !l.is_empty()) {
        let (key, value) = line.split_once(':').ok_or("http")?;
        if key.is_empty()
            || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || value.chars().any(|c| c.is_control() && c != '\t')
        {
            return Err("http");
        }
        if result
            .insert(key.to_ascii_lowercase(), value.trim().into())
            .is_some()
        {
            return Err("http");
        }
    }
    Ok(result)
}
fn incoming(stream: &mut TcpStream, shared: &Shared) -> WResult<Http> {
    let until = shared
        .deadline()
        .min(Instant::now() + Duration::from_secs(3));
    let text = head(stream, shared, until)?;
    let first: Vec<_> = text
        .split("\r\n")
        .next()
        .ok_or("http")?
        .split(' ')
        .collect();
    if first.len() != 3 || first[2] != "HTTP/1.1" || !["GET", "POST"].contains(&first[0]) {
        return Err("http");
    }
    let headers = headers(&text)?;
    if headers.contains_key("transfer-encoding")
        || headers.contains_key("content-encoding")
        || headers.contains_key("expect")
    {
        return Err("http");
    }
    let size = headers
        .get("content-length")
        .map(|v| v.parse::<usize>().map_err(|_| "http"))
        .transpose()?
        .unwrap_or(0);
    if size > MAX_HTTP || (first[0] == "GET" && size != 0) {
        return Err("http");
    }
    Ok(Http {
        method: first[0].into(),
        path: first[1].into(),
        content_type: headers.get("content-type").cloned().unwrap_or_default(),
        body: exact(stream, size, shared, until)?,
    })
}
fn write_http(
    stream: &mut TcpStream,
    content_type: &str,
    body: &[u8],
    shared: &Shared,
) -> WResult<()> {
    let header = format!("HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n", body.len());
    for bytes in [header.as_bytes(), body] {
        crate::dev_worker::write_bytes(stream, bytes, shared.deadline(), &|| shared.stopped())
            .map_err(|_| "http")?;
    }
    Ok(())
}
fn parsed(body: &[u8]) -> WResult<Value> {
    crate::cloud::unique_json(body).map_err(|_| "protocol")
}
fn json_bytes(value: &Value) -> WResult<Vec<u8>> {
    serde_json::to_vec(value).map_err(|_| "protocol")
}
fn keys(value: &Value, allowed: &[&str]) -> WResult<()> {
    if value.as_object().is_none_or(|o| {
        o.len() != allowed.len() || o.keys().any(|k| !allowed.contains(&k.as_str()))
    }) {
        return Err("protocol");
    }
    Ok(())
}

fn sse(value: &Value) -> WResult<Vec<u8>> {
    let choices = value["choices"]
        .as_array()
        .filter(|v| v.len() == 1)
        .ok_or("model_protocol")?;
    let choice = &choices[0];
    let message = &choice["message"];
    if message["role"] != "assistant"
        || !["stop", "tool_calls"].contains(&choice["finish_reason"].as_str().unwrap_or(""))
    {
        return Err("model_protocol");
    }
    let mut delta = json!({"role":"assistant"});
    if let Some(text) = message["content"].as_str() {
        delta["content"] = json!(text);
    }
    if let Some(calls) = message.get("tool_calls") {
        let mut calls = calls.as_array().ok_or("model_protocol")?.clone();
        if calls.is_empty() || calls.len() > 4 {
            return Err("model_protocol");
        }
        for (i, call) in calls.iter_mut().enumerate() {
            let object = call.as_object_mut().ok_or("model_protocol")?;
            if object.get("type").and_then(Value::as_str) != Some("function")
                || object.get("id").and_then(Value::as_str).is_none()
                || object
                    .get("function")
                    .is_none_or(|f| f["name"] != "read" || f["arguments"].as_str().is_none())
            {
                return Err("model_protocol");
            }
            object.insert("index".into(), json!(i));
        }
        delta["tool_calls"] = json!(calls);
    }
    let mut result = Vec::new();
    for (delta, finish, usage) in [
        (delta, Value::Null, Value::Null),
        (
            json!({}),
            choice["finish_reason"].clone(),
            value["usage"].clone(),
        ),
    ] {
        let mut chunk = json!({"id":value["id"].as_str().unwrap_or("chatcmpl-smith-local"),"object":"chat.completion.chunk","created":0,"model":"selected","choices":[{"index":0,"delta":delta,"finish_reason":finish}]});
        // The shared sanitizer may omit usage; null means unknown, never invented token counts.
        chunk["usage"] = usage;
        result.extend_from_slice(b"data: ");
        result.extend(json_bytes(&chunk)?);
        result.extend_from_slice(b"\n\n");
    }
    result.extend_from_slice(b"data: [DONE]\n\n");
    Ok(result)
}

fn facade(listener: TcpListener, shared: Arc<Shared>, repositories: usize) {
    let mut downloaded = 0usize;
    let mut advertised = 0usize;
    while !shared.stopped() {
        match listener.accept() {
            Ok((mut stream, peer)) => {
                let result = (|| -> WResult<()> {
                    if !peer.ip().is_loopback() {
                        return Err("http");
                    }
                    crate::dev_worker::nonblocking(stream.as_raw_fd()).map_err(|_| "http")?;
                    let request = incoming(&mut stream, &shared)?;
                    let (ctype, body) = route(
                        request,
                        &shared,
                        repositories,
                        &mut downloaded,
                        &mut advertised,
                    )?;
                    write_http(&mut stream, ctype, &body, &shared)
                })();
                if result.is_err() {
                    shared.cancel();
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5))
            }
            Err(_) => {
                shared.cancel();
                break;
            }
        }
    }
}
fn route(
    request: Http,
    s: &Shared,
    repositories: usize,
    downloaded: &mut usize,
    advertised: &mut usize,
) -> WResult<(&'static str, Vec<u8>)> {
    if let Some(suffix) = request.path.strip_prefix("/repo/") {
        if s.fetch_closed.load(Ordering::SeqCst) || s.dispatched.load(Ordering::SeqCst) {
            return Err("repository_sealed");
        }
        let (index, endpoint) = suffix.split_once(".git/").ok_or("repository")?;
        let index_num: usize = index.parse().map_err(|_| "repository")?;
        if index_num >= repositories || index != index_num.to_string() {
            return Err("repository");
        }
        let advertisement = match (request.method.as_str(), endpoint) {
            ("GET", "info/refs?service=git-upload-pack") if request.body.is_empty() => true,
            ("POST", "git-upload-pack")
                if request.content_type == "application/x-git-upload-pack-request"
                    && request.body.len() <= 16 * 1024 =>
            {
                false
            }
            _ => return Err("repository"),
        };
        let cap = if advertisement {
            MAX_AD.saturating_sub(*advertised)
        } else {
            MAX_PACK.saturating_sub(*downloaded)
        };
        let response = s.exchange(
            |sequence| Frame::RepositoryRequest {
                request_id: s.job.clone(),
                sequence,
                repository: index_num,
                advertisement,
                body: request.body,
            },
            cap,
        )?;
        let expected = if advertisement { AD_TYPE } else { PACK_TYPE };
        if response.content_type != expected {
            return Err("repository");
        }
        if advertisement {
            *advertised += response.body.len();
        } else {
            *downloaded += response.body.len();
        }
        return Ok((expected, response.body));
    }
    if request.method != "POST" || request.content_type != "application/json" {
        return Err("http");
    }
    let body = parsed(&request.body)?;
    match request.path.as_str() {
        "/abort" => {
            s.cancel();
            Err("guard")
        }
        "/guard-ready" => {
            keys(&body, &["job", "policy", "guard_sha256"])?;
            if body["job"] != s.job
                || body["policy"] != protocol::POLICY
                || body["guard_sha256"] != s.sha
                || !s.sealed.load(Ordering::SeqCst)
                || s.guard_ready.swap(true, Ordering::SeqCst)
            {
                return Err("guard");
            }
            Ok(("application/json", b"true".to_vec()))
        }
        "/v1/chat/completions" => {
            if !s.dispatched.load(Ordering::SeqCst)
                || !s.guard_ready.load(Ordering::SeqCst)
                || !s.active_reads.lock().map_err(|_| "guard")?.is_empty()
            {
                return Err("guard");
            }
            // Host owns transcript validation, duplicate latching and the original round/token budget.
            let response = s.exchange(
                |sequence| Frame::ModelRequest {
                    request_id: s.job.clone(),
                    sequence,
                    body,
                },
                MAX_HTTP,
            )?;
            if response.content_type != "application/json" {
                return Err("model_protocol");
            }
            Ok(("text/event-stream", sse(&parsed(&response.body)?)?))
        }
        "/read" => {
            keys(&body, &["call_id", "args"])?;
            if !s.dispatched.load(Ordering::SeqCst) || !s.guard_ready.load(Ordering::SeqCst) {
                return Err("guard");
            }
            let call = body["call_id"].as_str().ok_or("read_denied")?.to_owned();
            let response = s.exchange(
                |sequence| Frame::ReadRequest {
                    request_id: s.job.clone(),
                    sequence,
                    call_id: call.clone(),
                    arguments: body["args"].clone(),
                },
                16,
            )?;
            s.acknowledge(response)?;
            let reader = s.reader.lock().map_err(|_| "read_denied")?;
            let result = reader
                .as_ref()
                .ok_or("read_denied")?
                .read(&call, &body["args"])?;
            drop(reader);
            let response = s.exchange(
                |sequence| Frame::ReadResult {
                    request_id: s.job.clone(),
                    sequence,
                    call_id: call.clone(),
                    text: result.text.clone(),
                },
                16,
            )?;
            s.acknowledge(response)?;
            if !s.active_reads.lock().map_err(|_| "guard")?.insert(call) {
                return Err("guard");
            }
            Ok((
                "application/json",
                json_bytes(&json!({"path":result.path,"text":result.text,"title":result.title}))?,
            ))
        }
        "/read-complete" => {
            keys(&body, &["call_id"])?;
            let call = body["call_id"].as_str().ok_or("guard")?;
            if !s.active_reads.lock().map_err(|_| "guard")?.remove(call) {
                return Err("guard");
            }
            Ok(("application/json", b"true".to_vec()))
        }
        _ => Err("http"),
    }
}

struct Owned(Child, bool, Instant);
impl Owned {
    fn stop(&mut self, until: Instant) -> WResult<()> {
        let until = until.min(self.2);
        if self.1 {
            return Ok(());
        }
        // Kill the owned process group, including Git helpers, rather than only its leader.
        unsafe {
            libc::kill(-(self.0.id() as i32), libc::SIGKILL);
        }
        loop {
            if self.0.try_wait().map_err(|_| "shutdown")?.is_some() {
                self.1 = true;
                return Ok(());
            }
            if Instant::now() >= until {
                return Err("shutdown");
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.stop(Instant::now() + Duration::from_millis(500));
    }
}
fn command_base(binary: &str) -> Command {
    let mut c = Command::new(binary);
    c.env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", "/tmp/empty-home")
        .env("LANG", "C.UTF-8")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    c
}
fn git() -> Command {
    let mut c = command_base("/usr/bin/git");
    c.current_dir("/workspace")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "/bin/false")
        .env("SSH_ASKPASS", "/bin/false")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_LFS_SKIP_SMUDGE", "1")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_PROTOCOL_FROM_USER", "0");
    for setting in [
        "protocol.version=0",
        "protocol.allow=never",
        "protocol.http.allow=always",
        "credential.helper=",
        "core.askPass=/bin/false",
        "core.hooksPath=/dev/null",
        "core.fsmonitor=false",
        "core.attributesFile=/dev/null",
        "core.autocrlf=false",
        "core.quotePath=true",
        "http.followRedirects=false",
        "http.proxy=",
        "http.extraHeader=",
        "http.cookieFile=",
        "http.saveCookies=false",
        "http.sslVerify=true",
        "fetch.recurseSubmodules=false",
        "submodule.recurse=false",
        "filter.lfs.required=false",
        "filter.lfs.smudge=",
        "filter.lfs.clean=",
        "filter.lfs.process=",
    ] {
        c.arg("-c").arg(setting);
    }
    c
}
/// Drains currently readable output. Before exit, WouldBlock just yields; after exit,
/// the pipe must reach EOF (a lingering helper holding it is a failure).
fn drain(output: &mut ChildStdout, bytes: &mut Vec<u8>, limit: usize, exited: bool) -> WResult<()> {
    let mut buf = [0; 65536];
    loop {
        match output.read(&mut buf) {
            Ok(0) => return Ok(()),
            Ok(n) => {
                bytes.extend_from_slice(&buf[..n]);
                if bytes.len() > limit {
                    return Err("repository");
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) if !exited && e.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
            Err(_) => return Err("repository"),
        }
    }
}
fn execute(mut command: Command, s: &Shared, capture: Option<usize>) -> WResult<Vec<u8>> {
    if capture.is_some() {
        command.stdout(Stdio::piped());
    }
    let limit = capture.unwrap_or(0);
    let mut child = Owned(
        command.spawn().map_err(|_| "runtime")?,
        false,
        s.until + Duration::from_secs(protocol::CLEANUP_SECONDS),
    );
    let mut output = child.0.stdout.take();
    if let Some(ref output) = output {
        crate::dev_worker::nonblocking(output.as_raw_fd()).map_err(|_| "runtime")?;
    }
    let mut bytes = Vec::new();
    loop {
        if s.stopped() {
            return Err("cancelled");
        }
        if let Some(ref mut output) = output {
            drain(output, &mut bytes, limit, false)?;
        }
        if let Some(status) = child.0.try_wait().map_err(|_| "runtime")? {
            if !status.success() {
                return Err("repository");
            }
            if let Some(ref mut output) = output {
                drain(output, &mut bytes, limit, true)?;
            }
            child.stop(s.deadline())?;
            return Ok(bytes);
        }
        thread::sleep(Duration::from_millis(10));
    }
}
fn repository_path(index: usize) -> String {
    format!("/workspace/repos/r{index}")
}
/// Clones every repository through the loopback relay, detached at its fetched HEAD.
/// Export reads objects only, so it skips materializing a working tree.
fn clone_all(
    repositories: &[Repository],
    addr: SocketAddr,
    s: &Shared,
    working_tree: bool,
) -> WResult<Vec<Checkout>> {
    if repositories.is_empty() || repositories.len() > 10 {
        return Err("repository");
    }
    std::fs::create_dir("/workspace/repos").map_err(|_| "repository")?;
    let mut checkouts = Vec::new();
    for (index, repo) in repositories.iter().enumerate() {
        if repo.url.len() > 2048
            || !repo.url.starts_with("https://")
            || repo.url.chars().any(char::is_control)
        {
            return Err("repository");
        }
        let path = repository_path(index);
        let mut clone = git();
        clone.args(["clone", "--no-recurse-submodules", "--template="]);
        if !working_tree {
            clone.arg("--no-checkout");
        }
        if let Some(reference) = &repo.reference {
            let branch = branch_name(reference);
            if branch.is_empty()
                || branch.starts_with('-')
                || branch.len() > 256
                || branch.chars().any(char::is_control)
            {
                return Err("repository");
            }
            clone.arg("--branch").arg(branch);
        }
        clone
            .arg("--")
            .arg(format!("http://{addr}/repo/{index}.git"))
            .arg(&path);
        execute(clone, s, None)?;
        if working_tree {
            let mut detach = git();
            detach.args([
                "-C",
                &path,
                "checkout",
                "--detach",
                "--no-recurse-submodules",
                "HEAD",
                "--",
            ]);
            execute(detach, s, None)?;
        }
        let mut rev = git();
        rev.args(["-C", &path, "rev-parse", "--verify", "HEAD^{commit}"]);
        let bytes = execute(rev, s, Some(256))?;
        let commit = std::str::from_utf8(&bytes)
            .map_err(|_| "repository")?
            .trim_end_matches('\n');
        if !protocol::valid_hash(commit, 40) {
            return Err("repository");
        }
        let reference = checkout_reference(repo.reference.as_deref());
        checkouts.push(Checkout {
            url: repo.url.clone(),
            reference,
            commit: commit.into(),
        });
    }
    Ok(checkouts)
}
fn checkouts(repositories: &[Repository], addr: SocketAddr, s: &Shared) -> WResult<Vec<Checkout>> {
    let checkouts = clone_all(repositories, addr, s, true)?;
    let roots = (0..checkouts.len())
        .map(|index| dev_read::directory(Path::new(&repository_path(index))))
        .collect::<WResult<Vec<_>>>()?;
    s.fetch_closed.store(true, Ordering::SeqCst);
    dev_read::seal(&roots)?;
    *s.reader.lock().map_err(|_| "read_denied")? = Some(Reader::new(roots)?);
    publish_checkouts(s, &checkouts)?;
    s.sealed.store(true, Ordering::SeqCst);
    Ok(checkouts)
}
/// Host verifies the pins against its own fetch before acknowledging.
fn publish_checkouts(s: &Shared, checkouts: &[Checkout]) -> WResult<()> {
    // Register the reserved phase ACK before publishing checkouts, avoiding a fast-host race.
    let (tx, rx) = mpsc::sync_channel(1);
    s.pending.lock().map_err(|_| "protocol")?.insert(
        0,
        Pending {
            tx,
            cap: 16,
            content_type: None,
            body: Vec::new(),
        },
    );
    s.send(&Frame::Repositories {
        request_id: s.job.clone(),
        repositories: checkouts.to_vec(),
    })?;
    s.acknowledge(s.await_response(rx)?)
}

/// Fits one SourceCommit frame even when every byte JSON-encodes as up to four characters.
const EXPORT_COMMIT: usize = 48 * 1024;
const EXPORT_LIST: usize = 8 * 1024 * 1024;
/// Received-entry bounds. The host applies its own, stricter snapshot limits.
pub(crate) const EXPORT_ENTRIES: usize = 20_000;
pub(crate) const EXPORT_BYTES: u64 = 128 * 1024 * 1024;

struct TreeEntry {
    mode: u32,
    oid: String,
    path: String,
}
/// Parses `git ls-tree -r -z --full-tree`: `<mode> SP <type> SP <oid> TAB <path> NUL`.
fn tree_entries(list: &[u8]) -> WResult<Vec<TreeEntry>> {
    let records = list.strip_suffix(b"\0").unwrap_or(list);
    if records.is_empty() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for record in records.split(|b| *b == 0) {
        let tab = record
            .iter()
            .position(|b| *b == b'\t')
            .ok_or("repository")?;
        let meta = std::str::from_utf8(&record[..tab]).map_err(|_| "repository")?;
        let path = std::str::from_utf8(&record[tab + 1..]).map_err(|_| "repository")?;
        let mut fields = meta.split(' ');
        let (mode, kind, oid) = match (fields.next(), fields.next(), fields.next(), fields.next()) {
            (Some(m), Some(k), Some(o), None) => (m, k, o),
            _ => return Err("repository"),
        };
        let mode = match (mode, kind) {
            ("100644", "blob") => 0o100644,
            ("100755", "blob") => 0o100755,
            ("120000", "blob") => 0o120000,
            ("160000", "commit") => 0o160000,
            _ => return Err("repository"),
        };
        if !protocol::valid_hash(oid, 40) || path.is_empty() || path.len() > 4096 {
            return Err("repository");
        }
        entries.push(TreeEntry {
            mode,
            oid: oid.into(),
            path: path.into(),
        });
        if entries.len() > EXPORT_ENTRIES {
            return Err("repository");
        }
    }
    Ok(entries)
}
/// Nonblocking, deadline-bound reader over `git cat-file --batch` output.
struct BatchOutput<'a> {
    output: ChildStdout,
    buffer: Vec<u8>,
    s: &'a Shared,
}
impl BatchOutput<'_> {
    fn fill(&mut self) -> WResult<()> {
        let mut buf = [0; 65536];
        loop {
            if self.s.stopped() {
                return Err("cancelled");
            }
            match self.output.read(&mut buf) {
                Ok(0) => return Err("repository"),
                Ok(n) => {
                    self.buffer.extend_from_slice(&buf[..n]);
                    return Ok(());
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    crate::dev_worker::wait_io(
                        self.output.as_raw_fd(),
                        libc::POLLIN,
                        self.s.deadline(),
                        &|| self.s.stopped(),
                    )
                    .map_err(|_| "cancelled")?;
                }
                Err(_) => return Err("repository"),
            }
        }
    }
    fn line(&mut self) -> WResult<Vec<u8>> {
        loop {
            if let Some(end) = self.buffer.iter().position(|b| *b == b'\n') {
                let line = self.buffer[..end].to_vec();
                self.buffer.drain(..=end);
                return Ok(line);
            }
            if self.buffer.len() > 256 {
                return Err("repository");
            }
            self.fill()?;
        }
    }
    /// Returns between 1 and `max` bytes of object content.
    fn take(&mut self, max: usize) -> WResult<Vec<u8>> {
        if self.buffer.is_empty() {
            self.fill()?;
        }
        let n = self.buffer.len().min(max);
        Ok(self.buffer.drain(..n).collect())
    }
}
fn send_entry(
    s: &Shared,
    index: usize,
    entry: &TreeEntry,
    bytes: Vec<u8>,
    last: bool,
) -> WResult<()> {
    s.send(&Frame::SourceFile {
        request_id: s.job.clone(),
        repository: index,
        path: entry.path.clone(),
        mode: entry.mode,
        bytes,
        last,
    })
}
/// Streams exact object-database content, never working-tree bytes, so attributes,
/// filters and line-ending conversion cannot alter what the host verifies.
fn export_repository(index: usize, path: &Path, s: &Shared, received: &mut u64) -> WResult<()> {
    let git_at = || {
        let mut command = git();
        command.current_dir(path);
        command
    };
    let mut commit = git_at();
    commit.args(["cat-file", "commit", "HEAD"]);
    let object = execute(commit, s, Some(EXPORT_COMMIT))?;
    s.send(&Frame::SourceCommit {
        request_id: s.job.clone(),
        repository: index,
        object,
    })?;
    let mut list = git_at();
    list.args(["ls-tree", "-r", "-z", "--full-tree", "HEAD"]);
    let entries = tree_entries(&execute(list, s, Some(EXPORT_LIST))?)?;
    let (gitlinks, blobs): (Vec<_>, Vec<_>) = entries.iter().partition(|e| e.mode == 0o160000);
    for entry in &gitlinks {
        send_entry(s, index, entry, entry.oid.as_bytes().to_vec(), true)?;
    }
    if !blobs.is_empty() {
        let mut batch = git_at();
        batch
            .args(["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
        let mut child = Owned(
            batch.spawn().map_err(|_| "runtime")?,
            false,
            s.until + Duration::from_secs(protocol::CLEANUP_SECONDS),
        );
        let mut input = child.0.stdin.take().ok_or("runtime")?;
        let output = child.0.stdout.take().ok_or("runtime")?;
        crate::dev_worker::nonblocking(output.as_raw_fd()).map_err(|_| "runtime")?;
        let mut reader = BatchOutput {
            output,
            buffer: Vec::new(),
            s,
        };
        let request: Vec<u8> = blobs
            .iter()
            .flat_map(|e| format!("{}\n", e.oid).into_bytes())
            .collect();
        let result = thread::scope(|scope| {
            // Blocking writer; killing the batch process on failure releases it with EPIPE.
            let writer = scope.spawn(move || {
                use std::io::Write;
                input.write_all(&request).is_ok()
            });
            let result = (|| -> WResult<()> {
                for entry in &blobs {
                    let header = reader.line()?;
                    let header = std::str::from_utf8(&header).map_err(|_| "repository")?;
                    let size: u64 = header
                        .strip_prefix(&format!("{} blob ", entry.oid))
                        .and_then(|size| size.parse().ok())
                        .ok_or("repository")?;
                    *received = received.saturating_add(size);
                    if *received > EXPORT_BYTES {
                        return Err("repository");
                    }
                    let mut remaining = size as usize;
                    if remaining == 0 {
                        send_entry(s, index, entry, Vec::new(), true)?;
                    }
                    while remaining > 0 {
                        let bytes = reader.take(remaining.min(protocol::CHUNK))?;
                        remaining -= bytes.len();
                        send_entry(s, index, entry, bytes, remaining == 0)?;
                    }
                    if reader.take(1)? != b"\n" {
                        return Err("repository");
                    }
                }
                Ok(())
            })();
            if result.is_err() {
                let _ = child.stop(Instant::now() + Duration::from_millis(500));
            }
            let written = writer.join().unwrap_or(false);
            result.and(if written { Ok(()) } else { Err("repository") })
        });
        result?;
        // Writer dropped stdin; the batch process must now exit cleanly on its own.
        loop {
            if s.stopped() {
                return Err("cancelled");
            }
            if let Some(status) = child.0.try_wait().map_err(|_| "runtime")? {
                if !status.success() || !reader.buffer.is_empty() {
                    return Err("repository");
                }
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        child.stop(s.deadline())?;
    }
    s.send(&Frame::SourceEnd {
        request_id: s.job.clone(),
        repository: index,
        files: entries.len(),
    })
}
fn export_environment() -> WResult<()> {
    if unsafe { libc::geteuid() } != 1000
        || std::env::current_dir().map_err(|_| "runtime")? != Path::new("/workspace")
        || std::env::var("SMITH_DEV_SOURCE_EXPORT").ok().as_deref() != Some(protocol::EXPORT_POLICY)
    {
        return Err("runtime");
    }
    // Same baked worker image; OpenCode is present but never started in export mode.
    for path in ["/etc/smith-dev-local-worker", "/usr/bin/git"] {
        immutable(Path::new(path), true)?;
    }
    if std::fs::read("/etc/smith-dev-local-worker").map_err(|_| "runtime")?
        != format!("{}\n", protocol::POLICY).as_bytes()
    {
        return Err("runtime");
    }
    Ok(())
}

fn immutable(path: &Path, regular: bool) -> WResult<()> {
    let mut prefix = std::path::PathBuf::from("/");
    for component in path.components().skip(1) {
        prefix.push(component);
        let meta = std::fs::symlink_metadata(&prefix).map_err(|_| "guard")?;
        if meta.uid() != 0
            || meta.mode() & 0o022 != 0
            || meta.file_type().is_symlink()
            || (prefix != path && !meta.is_dir())
        {
            return Err("guard");
        }
        if prefix == path && regular && (!meta.is_file() || meta.mode() & 0o222 != 0) {
            return Err("guard");
        }
    }
    Ok(())
}
fn environment() -> WResult<String> {
    if unsafe { libc::geteuid() } != 1000
        || std::env::current_dir().map_err(|_| "runtime")? != Path::new("/workspace")
        || std::env::var("SMITH_DEV_LOCAL_WORKER").ok().as_deref() != Some(protocol::POLICY)
    {
        return Err("runtime");
    }
    for path in [
        "/etc/smith-dev-local-worker",
        "/etc/smith-opencode/guard.mjs",
        "/usr/bin/git",
        "/usr/local/bin/opencode",
    ] {
        immutable(Path::new(path), true)?;
    }
    immutable(Path::new("/etc/smith-opencode/opencode"), false)?;
    if std::fs::read("/etc/smith-dev-local-worker").map_err(|_| "runtime")?
        != format!("{}\n", protocol::POLICY).as_bytes()
        || std::fs::read("/etc/smith-opencode/guard.mjs").map_err(|_| "guard")? != GUARD
    {
        return Err("guard");
    }
    Ok(guard_sha256())
}
fn configuration(addr: SocketAddr) -> Value {
    json!({
        "$schema":"https://opencode.ai/config.json", "model":"smith-local/selected", "small_model":"smith-local/selected", "default_agent":"inspect",
        "enabled_providers":["smith-local"], "plugin":["file:///etc/smith-opencode/guard.mjs"],
        "provider":{"smith-local":{"name":"Smith local", "npm":"@ai-sdk/openai-compatible", "options":{"baseURL":format!("http://{addr}/v1"),"apiKey":"smith-local-not-a-secret"}, "models":{"selected":{"id":"selected","name":"Selected Smith local model", "limit":{"context":4096,"output":1024},"tool_call":true}}}},
        "share":"disabled", "snapshot":false, "autoupdate":false, "lsp":false,"mcp":{},"instructions":[],"skills":{"paths":[],"urls":[]},
        "compaction":{"auto":false,"prune":false}, "permission":{"*":"deny","read":"allow","external_directory":{"*":"deny","/tmp/smith-read-results/*":"allow"}},
        "agent":{"inspect":{"mode":"primary","model":"smith-local/selected","prompt":protocol::SYSTEM_PROMPT,"permission":{"*":"deny","read":"allow","external_directory":{"*":"deny","/tmp/smith-read-results/*":"allow"}}}, "build":{"disable":true},"plan":{"disable":true},"general":{"disable":true},"explore":{"disable":true},"title":{"disable":true},"summary":{"disable":true},"compaction":{"disable":true}}
    })
}
fn opencode_command(addr: SocketAddr, shared: &Shared) -> Command {
    let mut c = command_base("/usr/local/bin/opencode");
    c.args(["serve", "--hostname", "127.0.0.1", "--port", "4096"])
        .current_dir("/workspace")
        .env("XDG_CONFIG_HOME", "/etc/smith-opencode")
        .env("XDG_DATA_HOME", "/tmp/data")
        .env("XDG_CACHE_HOME", "/tmp/cache")
        .env("XDG_STATE_HOME", "/tmp/state")
        .env("TMPDIR", "/tmp")
        .env("OPENCODE_CONFIG_CONTENT", configuration(addr).to_string())
        .env("SMITH_LOCAL_JOB", &shared.job)
        .env("SMITH_LOCAL_FACADE", format!("http://{addr}"));
    // PURE must be absent: it suppresses even the explicitly pinned file plugin.
    for flag in [
        "OPENCODE_DISABLE_DEFAULT_PLUGINS",
        "OPENCODE_DISABLE_PROJECT_CONFIG",
        "OPENCODE_DISABLE_AUTOUPDATE",
        "OPENCODE_DISABLE_PRUNE",
        "OPENCODE_DISABLE_AUTOCOMPACT",
        "OPENCODE_DISABLE_MODELS_FETCH",
        "OPENCODE_DISABLE_FFF",
        "OPENCODE_EXPERIMENTAL_DISABLE_FILEWATCHER",
        "OPENCODE_DISABLE_LSP_DOWNLOAD",
        "OPENCODE_DISABLE_EXTERNAL_SKILLS",
        "OPENCODE_DISABLE_CLAUDE_CODE",
        "OPENCODE_DISABLE_EMBEDDED_WEB_UI",
    ] {
        c.env(flag, "true");
    }
    c
}
fn opencode(addr: SocketAddr, shared: &Shared) -> WResult<Owned> {
    Ok(Owned(
        opencode_command(addr, shared)
            .spawn()
            .map_err(|_| "runtime")?,
        false,
        shared.until + Duration::from_secs(protocol::CLEANUP_SECONDS),
    ))
}

/// Outbound HTTP is only to the fixed owned loopback OpenCode listener, with no retry/redirect.
fn api(
    method: &str,
    path: &str,
    body: Option<Value>,
    s: &Shared,
    until: Instant,
) -> WResult<Value> {
    if s.stopped() || Instant::now() >= until {
        return Err("cancelled");
    }
    let addr: SocketAddr = "127.0.0.1:4096".parse().map_err(|_| "http")?;
    let timeout = until
        .saturating_duration_since(Instant::now())
        .min(Duration::from_millis(200));
    let mut stream = TcpStream::connect_timeout(&addr, timeout).map_err(|_| "http")?;
    crate::dev_worker::nonblocking(stream.as_raw_fd()).map_err(|_| "http")?;
    let body = body
        .map(|v| json_bytes(&v))
        .transpose()?
        .unwrap_or_default();
    if body.len() > MAX_HTTP {
        return Err("http");
    }
    let header = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:4096\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
    for bytes in [header.as_bytes(), body.as_slice()] {
        crate::dev_worker::write_bytes(&mut stream, bytes, until, &|| s.stopped())
            .map_err(|_| "http")?;
    }
    let text = head(&mut stream, s, until)?;
    if !text.starts_with("HTTP/1.1 200 ") && !text.starts_with("HTTP/1.1 201 ") {
        return Err("http");
    }
    let headers = headers(&text)?;
    if headers.contains_key("content-encoding") {
        return Err("http");
    }
    let mut bytes = Vec::new();
    match (
        headers.get("transfer-encoding"),
        headers.get("content-length"),
    ) {
        (None, Some(size)) => {
            let size: usize = size.parse().map_err(|_| "http")?;
            if size > 1024 * 1024 {
                return Err("http");
            }
            bytes = exact(&mut stream, size, s, until)?;
        }
        (Some(encoding), None) if encoding == "chunked" => loop {
            let mut line = Vec::new();
            while !line.ends_with(b"\r\n") {
                if line.len() > 16 {
                    return Err("http");
                }
                line.extend(exact(&mut stream, 1, s, until)?);
            }
            let line = std::str::from_utf8(&line[..line.len() - 2]).map_err(|_| "http")?;
            let size = usize::from_str_radix(line, 16).map_err(|_| "http")?;
            if bytes.len().saturating_add(size) > 1024 * 1024 {
                return Err("http");
            }
            if size == 0 {
                if exact(&mut stream, 2, s, until)? != b"\r\n" {
                    return Err("http");
                }
                break;
            }
            bytes.extend(exact(&mut stream, size, s, until)?);
            if exact(&mut stream, 2, s, until)? != b"\r\n" {
                return Err("http");
            }
        },
        _ => return Err("http"),
    }
    parsed(&bytes)
}
fn final_reply(value: &Value, session: &str, parent: &str) -> WResult<(String, String)> {
    let info = &value["info"];
    let message = info["id"].as_str().ok_or("reply")?;
    if message.len() > 128
        || !message.starts_with("msg_")
        || !message
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        || info["role"] != "assistant"
        || info["sessionID"] != session
        || info["parentID"] != parent
        || info["finish"] != "stop"
        || info.get("error").is_some_and(|v| !v.is_null())
    {
        return Err("reply");
    }
    let parts = value["parts"].as_array().ok_or("reply")?;
    let mut text = String::new();
    for part in parts {
        if part["sessionID"] != session || part["messageID"] != message {
            return Err("reply");
        }
        if part["type"] == "text" {
            if part["synthetic"] == true || part["ignored"] == true {
                return Err("reply");
            }
            text.push_str(part["text"].as_str().ok_or("reply")?);
            if text.len() > protocol::MAX_REPLY {
                return Err("reply");
            }
        } else if !["step-start", "step-finish", "reasoning"]
            .contains(&part["type"].as_str().unwrap_or(""))
        {
            return Err("reply");
        }
    }
    if text.trim().is_empty() {
        return Err("reply");
    }
    Ok((message.into(), text))
}
fn inspect(s: &Shared, addr: SocketAddr, prompts: mpsc::Receiver<Frame>) -> WResult<()> {
    let mut child = opencode(addr, s)?;
    loop {
        if s.stopped()
            || Instant::now() >= s.prep
            || child.0.try_wait().map_err(|_| "runtime")?.is_some()
        {
            return Err("runtime");
        }
        if let Ok(health) = api(
            "GET",
            "/global/health",
            None,
            s,
            s.prep.min(Instant::now() + Duration::from_secs(1)),
        ) {
            if health["healthy"] != true || health["version"] != "1.19.3" {
                return Err("runtime");
            }
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    // The pinned provider query calls Plugin.list; health alone does not initialize plugins.
    api("GET", "/provider", None, s, s.prep)?;
    if !s.guard_ready.load(Ordering::SeqCst) {
        return Err("guard");
    }
    let session = api(
        "POST",
        "/session",
        Some(
            json!({"title":"Smith local inspection", "permission":[{"permission":"*","pattern":"*","action":"deny"},{"permission":"read","pattern":"*","action":"allow"},{"permission":"external_directory","pattern":"/tmp/smith-read-results/*","action":"allow"}]}),
        ),
        s,
        s.prep,
    )?;
    let session = session["id"].as_str().ok_or("protocol")?.to_owned();
    crate::dev_worker::session_id(&session).map_err(|_| "protocol")?;
    s.send(&Frame::Ready {
        request_id: s.job.clone(),
        session_id: session.clone(),
        policy: protocol::POLICY.into(),
        guard_sha256: s.sha.clone(),
    })?;
    let frame = loop {
        if s.stopped() {
            return Err("cancelled");
        }
        match prompts.recv_timeout(Duration::from_millis(25)) {
            Ok(frame) => break frame,
            Err(mpsc::RecvTimeoutError::Timeout) => (),
            Err(_) => return Err("protocol"),
        }
    };
    let Frame::Prompt {
        request_id,
        session_id,
        text,
    } = frame
    else {
        return Err("protocol");
    };
    if request_id != s.job
        || session_id != session
        || text.trim().is_empty()
        || text.len() > protocol::MAX_PROMPT
    {
        return Err("protocol");
    }
    *s.inference.lock().map_err(|_| "protocol")? =
        Some((Instant::now() + Duration::from_secs(protocol::INFERENCE_SECONDS)).min(s.until));
    s.dispatched.store(true, Ordering::SeqCst);
    let parent = format!("msg_{}", s.job);
    // ONE POST. Even a missing acknowledgement can only be followed by GET observation.
    let posted = api(
        "POST",
        &format!("/session/{session}/message"),
        Some(
            json!({"messageID":parent,"noReply":false,"agent":"inspect","model":{"providerID":"smith-local","modelID":"selected"},"parts":[{"type":"text","text":text}]}),
        ),
        s,
        s.deadline(),
    );
    let reply = match posted {
        Ok(value) => final_reply(&value, &session, &parent)?,
        Err(_) if !s.stopped() => {
            let values = api(
                "GET",
                &format!("/session/{session}/message"),
                None,
                s,
                s.deadline(),
            )?;
            let messages = values.as_array().ok_or("reply")?;
            let mut finals = messages
                .iter()
                .filter_map(|v| final_reply(v, &session, &parent).ok());
            let reply = finals.next().ok_or("reply")?;
            if finals.next().is_some() {
                return Err("reply");
            }
            reply
        }
        Err(_) => return Err("reply"),
    };
    if !s.active_reads.lock().map_err(|_| "guard")?.is_empty() {
        return Err("guard");
    }
    // Publish verified text before teardown so the host can retain it if cleanup is uncertain.
    // Receipt of this frame alone is not proof of worker/container shutdown.
    s.send(&Frame::Reply {
        request_id: s.job.clone(),
        session_id: session,
        message_id: reply.0,
        text: reply.1,
    })?;
    child.stop(s.until + Duration::from_secs(protocol::CLEANUP_SECONDS))
}

/// Registered by the main owner as `dev-local-worker`; no ambient setup or network fallback.
pub fn run(request_id: &str) -> crate::Result<()> {
    // No raw errors escape to the CLI's stderr. Only a fixed Failed kind is emitted.
    if !crate::dev_worker::valid_id(request_id) {
        return Ok(());
    }
    let overall = Instant::now() + Duration::from_secs(protocol::JOB_SECONDS);
    let result = run_inner(request_id);
    if result.is_err() {
        if let Ok(mut output) = duplicate(libc::STDOUT_FILENO) {
            let _ = protocol::send(
                &mut output,
                &Frame::Failed {
                    request_id: request_id.into(),
                    kind: "local_worker_failed".into(),
                },
                overall.min(Instant::now() + Duration::from_millis(200)),
                &|| false,
            );
        }
    }
    Ok(())
}
/// Registered as `dev-source-export`: clone through the relay, then stream pinned
/// commit/tree content. No OpenCode, model, prompt or task text exists in this mode.
pub fn run_export(request_id: &str) -> crate::Result<()> {
    if !crate::dev_worker::valid_id(request_id) {
        return Ok(());
    }
    let overall = Instant::now() + Duration::from_secs(protocol::JOB_SECONDS);
    if run_export_inner(request_id).is_err() {
        if let Ok(mut output) = duplicate(libc::STDOUT_FILENO) {
            let _ = protocol::send(
                &mut output,
                &Frame::Failed {
                    request_id: request_id.into(),
                    kind: "source_export_failed".into(),
                },
                overall.min(Instant::now() + Duration::from_millis(200)),
                &|| false,
            );
        }
    }
    Ok(())
}
fn run_export_inner(request_id: &str) -> WResult<()> {
    let started = Instant::now();
    export_environment()?;
    unsafe {
        libc::umask(0o077);
    }
    let mut input = BufReader::new(duplicate(libc::STDIN_FILENO)?);
    let output = duplicate(libc::STDOUT_FILENO)?;
    let init = protocol::receive(
        &mut input,
        started + Duration::from_secs(protocol::PREP_SECONDS),
        &|| false,
    )
    .map_err(|_| "protocol")?;
    let Frame::Initialize {
        request_id: id,
        version: 1,
        repositories,
    } = init
    else {
        return Err("protocol");
    };
    if id != request_id {
        return Err("protocol");
    }
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|_| "http")?;
    listener.set_nonblocking(true).map_err(|_| "http")?;
    let addr = listener.local_addr().map_err(|_| "http")?;
    let s = Arc::new(Shared {
        job: request_id.into(),
        until: started + Duration::from_secs(protocol::JOB_SECONDS - protocol::CLEANUP_SECONDS),
        prep: started + Duration::from_secs(protocol::PREP_SECONDS),
        cancelled: AtomicBool::new(false),
        sealed: AtomicBool::new(false),
        fetch_closed: AtomicBool::new(false),
        guard_ready: AtomicBool::new(false),
        dispatched: AtomicBool::new(false),
        inference: Mutex::new(None),
        output: Mutex::new(output),
        pending: Mutex::new(HashMap::new()),
        sequence: AtomicU32::new(1),
        sha: String::new(),
        reader: Mutex::new(None),
        active_reads: Mutex::new(HashSet::new()),
    });
    // guard_ready is never set, so demux refuses any Prompt frame in this mode.
    let (tx, _prompts) = mpsc::sync_channel(1);
    thread::scope(|scope| {
        let input_shared = s.clone();
        scope.spawn(move || demux(input, input_shared, tx));
        let facade_shared = s.clone();
        let count = repositories.len();
        scope.spawn(move || facade(listener, facade_shared, count));
        let result = clone_all(&repositories, addr, &s, false).and_then(|checkouts| {
            s.fetch_closed.store(true, Ordering::SeqCst);
            publish_checkouts(&s, &checkouts)?;
            s.sealed.store(true, Ordering::SeqCst);
            let mut received = 0;
            (0..checkouts.len()).try_for_each(|index| {
                export_repository(index, Path::new(&repository_path(index)), &s, &mut received)
            })
        });
        s.cancel();
        result
    })
}
fn run_inner(request_id: &str) -> WResult<()> {
    if !crate::dev_worker::valid_id(request_id) {
        return Err("protocol");
    }
    let started = Instant::now();
    let sha = environment()?;
    // This is a dedicated worker process; set the mask before starting either scoped thread.
    unsafe {
        libc::umask(0o077);
    }
    let mut input = BufReader::new(duplicate(libc::STDIN_FILENO)?);
    let output = duplicate(libc::STDOUT_FILENO)?;
    let init = protocol::receive(
        &mut input,
        started + Duration::from_secs(protocol::PREP_SECONDS),
        &|| false,
    )
    .map_err(|_| "protocol")?;
    let Frame::Initialize {
        request_id: id,
        version: 1,
        repositories,
    } = init
    else {
        return Err("protocol");
    };
    if id != request_id {
        return Err("protocol");
    }
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|_| "http")?;
    listener.set_nonblocking(true).map_err(|_| "http")?;
    let addr = listener.local_addr().map_err(|_| "http")?;
    let s = Arc::new(Shared {
        job: request_id.into(),
        until: started + Duration::from_secs(protocol::JOB_SECONDS - protocol::CLEANUP_SECONDS),
        prep: started + Duration::from_secs(protocol::PREP_SECONDS),
        cancelled: AtomicBool::new(false),
        sealed: AtomicBool::new(false),
        fetch_closed: AtomicBool::new(false),
        guard_ready: AtomicBool::new(false),
        dispatched: AtomicBool::new(false),
        inference: Mutex::new(None),
        output: Mutex::new(output),
        pending: Mutex::new(HashMap::new()),
        sequence: AtomicU32::new(1),
        sha,
        reader: Mutex::new(None),
        active_reads: Mutex::new(HashSet::new()),
    });
    let (tx, rx) = mpsc::sync_channel(1);
    // Scoped threads always join. All I/O polls cancellation; no detached worker survives return.
    thread::scope(|scope| {
        let input_shared = s.clone();
        scope.spawn(move || demux(input, input_shared, tx));
        let facade_shared = s.clone();
        let count = repositories.len();
        scope.spawn(move || facade(listener, facade_shared, count));
        let result = checkouts(&repositories, addr, &s).and_then(|_| inspect(&s, addr, rx));
        s.cancel();
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_streams_object_bytes_that_reproduce_the_pinned_tree() {
        use crate::retained_jobs::retained_acquire::{Budget, Receiver};
        use std::os::fd::IntoRawFd;
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::net::UnixStream;
        let base =
            std::env::temp_dir().join(format!("smith-export-{}", crate::app::new_id().unwrap()));
        let repo = base.join("repo");
        let root = base.join("root");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let fixture = |args: &[&str]| {
            let mut c = git();
            c.current_dir(&repo)
                .env("GIT_AUTHOR_NAME", "S")
                .env("GIT_AUTHOR_EMAIL", "s@invalid")
                .env("GIT_COMMITTER_NAME", "S")
                .env("GIT_COMMITTER_EMAIL", "s@invalid")
                .args(args)
                .stdout(Stdio::piped());
            let out = c.output().unwrap();
            assert!(out.status.success());
            out.stdout
        };
        fixture(&["init", "--template=", "--initial-branch=main"]);
        std::fs::write(repo.join(".gitattributes"), b"*.txt text eol=crlf\n").unwrap();
        std::fs::write(repo.join("a.txt"), b"one\ntwo\n").unwrap();
        std::fs::write(repo.join("empty"), b"").unwrap();
        let large: Vec<u8> = (0..(protocol::CHUNK * 3 + 17))
            .map(|i| (i % 251) as u8)
            .collect();
        std::fs::create_dir(repo.join("dir")).unwrap();
        std::fs::write(repo.join("dir/large.bin"), &large).unwrap();
        std::os::unix::fs::symlink("a.txt", repo.join("link")).unwrap();
        fixture(&["add", "-A"]);
        let gitlink = format!("160000,{},sub", "1".repeat(40));
        fixture(&["update-index", "--add", "--cacheinfo", &gitlink]);
        fixture(&["commit", "-q", "-m", "fixture"]);
        let pin = String::from_utf8(fixture(&["rev-parse", "HEAD"]))
            .unwrap()
            .trim()
            .to_owned();

        let (writer, reader) = UnixStream::pair().unwrap();
        let now = Instant::now();
        let s = Arc::new(Shared {
            job: "0123456789abcdef0123456789abcdef".into(),
            until: now + Duration::from_secs(30),
            prep: now + Duration::from_secs(30),
            cancelled: AtomicBool::new(false),
            sealed: AtomicBool::new(true),
            fetch_closed: AtomicBool::new(true),
            guard_ready: AtomicBool::new(false),
            dispatched: AtomicBool::new(false),
            inference: Mutex::new(None),
            output: Mutex::new(unsafe { File::from_raw_fd(writer.into_raw_fd()) }),
            pending: Mutex::new(HashMap::new()),
            sequence: AtomicU32::new(1),
            sha: String::new(),
            reader: Mutex::new(None),
            active_reads: Mutex::new(HashSet::new()),
        });
        let worker = {
            let s = s.clone();
            let repo = repo.clone();
            thread::spawn(move || {
                let result = export_repository(0, &repo, &s, &mut 0);
                *s.output.lock().unwrap() = std::fs::File::open("/dev/null").unwrap();
                result
            })
        };
        let mut input = BufReader::new(reader);
        let mut receiver = Receiver::new(root.clone(), pin);
        let mut budget = Budget::default();
        let until = now + Duration::from_secs(30);
        let prepared = loop {
            match protocol::receive(&mut input, until, &|| false).unwrap() {
                Frame::SourceCommit { object, .. } => receiver.commit(&object).unwrap(),
                Frame::SourceFile {
                    path,
                    mode,
                    bytes,
                    last,
                    ..
                } => {
                    assert!(bytes.len() <= protocol::CHUNK);
                    receiver
                        .entry(path, mode, bytes, last, &mut budget)
                        .unwrap();
                }
                Frame::SourceEnd { files, .. } => break receiver.end(files).unwrap(),
                other => panic!("unexpected {:?}", other.request_id()),
            }
        };
        worker.join().unwrap().unwrap();
        assert_eq!(
            prepared.files.keys().collect::<Vec<_>>(),
            ["a.txt", "dir/large.bin", "empty"]
        );
        assert_eq!(std::fs::read(root.join("a.txt")).unwrap(), b"one\ntwo\n");
        assert_eq!(std::fs::read(root.join("dir/large.bin")).unwrap(), large);
        assert!(!root.join("link").exists() && !root.join("sub").exists());
        std::fs::remove_dir_all(&base).unwrap();
    }
    #[test]
    fn export_tree_listing_accepts_only_known_git_modes() {
        let oid = "a".repeat(40);
        let list = format!(
            "100644 blob {oid}\tsrc/a b.rs\x00100755 blob {oid}\ttool\x00120000 blob {oid}\tlink\x00160000 commit {oid}\tsub\x00"
        );
        let entries = tree_entries(list.as_bytes()).unwrap();
        assert_eq!(
            entries
                .iter()
                .map(|e| (e.mode, e.path.as_str()))
                .collect::<Vec<_>>(),
            [
                (0o100644, "src/a b.rs"),
                (0o100755, "tool"),
                (0o120000, "link"),
                (0o160000, "sub")
            ]
        );
        assert!(tree_entries(b"").unwrap().is_empty());
        for bad in [
            format!("040000 tree {oid}\td\x00"),
            format!("100644 commit {oid}\tf\x00"),
            format!("100644 blob {}\tf\x00", "A".repeat(40)),
            format!("100644 blob {oid}\t\x00"),
            format!("100644 blob {oid} extra\tf\x00"),
            format!("100644 blob {oid}f\x00"),
        ] {
            assert!(tree_entries(bad.as_bytes()).is_err(), "{bad:?}");
        }
        let mut invalid = format!("100644 blob {oid}\t").into_bytes();
        invalid.extend_from_slice(b"\xff\x00");
        assert!(tree_entries(&invalid).is_err());
        let many: String = (0..=EXPORT_ENTRIES)
            .map(|i| format!("100644 blob {oid}\tf{i}\x00"))
            .collect();
        assert!(tree_entries(many.as_bytes()).is_err());
    }
    fn shared() -> Arc<Shared> {
        let now = Instant::now();
        Arc::new(Shared {
            job: "0123456789abcdef0123456789abcdef".into(),
            until: now + Duration::from_secs(2),
            prep: now + Duration::from_secs(1),
            cancelled: AtomicBool::new(false),
            sealed: AtomicBool::new(false),
            fetch_closed: AtomicBool::new(false),
            guard_ready: AtomicBool::new(false),
            dispatched: AtomicBool::new(false),
            inference: Mutex::new(None),
            output: Mutex::new(
                std::fs::OpenOptions::new()
                    .write(true)
                    .open("/dev/null")
                    .unwrap(),
            ),
            pending: Mutex::new(HashMap::new()),
            sequence: AtomicU32::new(1),
            sha: "a".repeat(64),
            reader: Mutex::new(None),
            active_reads: Mutex::new(HashSet::new()),
        })
    }
    fn pipe() -> (File, File) {
        let mut fds = [-1; 2];
        assert_eq!(
            unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) },
            0
        );
        unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) }
    }
    #[test]
    fn initialize_overread_is_preserved_when_same_reader_moves_into_demux() {
        use std::io::{BufRead, Write};
        let s = shared();
        let (input, mut writer) = pipe();
        let (prompt_tx, _) = mpsc::sync_channel(1);
        let (tx, rx) = mpsc::sync_channel(1);
        s.pending.lock().unwrap().insert(
            0,
            Pending {
                tx,
                cap: 16,
                content_type: None,
                body: Vec::new(),
            },
        );
        let mut bytes = Vec::new();
        for frame in [
            Frame::Initialize {
                request_id: s.job.clone(),
                version: 1,
                repositories: Vec::new(),
            },
            Frame::ResponseStart {
                request_id: s.job.clone(),
                sequence: 0,
                success: true,
                content_type: "application/json".into(),
            },
            Frame::ResponseChunk {
                request_id: s.job.clone(),
                sequence: 0,
                bytes: b"true".to_vec(),
                last: true,
            },
        ] {
            serde_json::to_writer(&mut bytes, &frame).unwrap();
            bytes.push(b'\n');
        }
        writer.write_all(&bytes).unwrap();
        let mut input = BufReader::new(input);
        assert_eq!(input.fill_buf().unwrap(), bytes);
        assert!(matches!(
            protocol::receive(&mut input, s.until, &|| false).unwrap(),
            Frame::Initialize { version: 1, .. }
        ));
        assert!(!input.buffer().is_empty());
        thread::scope(|scope| {
            let worker = s.clone();
            scope.spawn(move || demux(input, worker, prompt_tx));
            s.acknowledge(s.await_response(rx).unwrap()).unwrap();
            assert!(!s.cancelled.load(Ordering::SeqCst));
            s.cancel();
        });
        drop(writer);
    }

    #[test]
    fn central_reader_correlates_sequence_zero_and_chunks_without_eof() {
        let s = shared();
        let (input, mut writer) = pipe();
        let (prompt_tx, _) = mpsc::sync_channel(1);
        let (tx, rx) = mpsc::sync_channel(1);
        s.pending.lock().unwrap().insert(
            0,
            Pending {
                tx,
                cap: 16,
                content_type: None,
                body: Vec::new(),
            },
        );
        thread::scope(|scope| {
            let worker = s.clone();
            scope.spawn(move || demux(BufReader::new(input), worker, prompt_tx));
            for frame in [
                Frame::ResponseStart {
                    request_id: s.job.clone(),
                    sequence: 0,
                    success: true,
                    content_type: "application/json".into(),
                },
                Frame::ResponseChunk {
                    request_id: s.job.clone(),
                    sequence: 0,
                    bytes: b"tr".to_vec(),
                    last: false,
                },
                Frame::ResponseChunk {
                    request_id: s.job.clone(),
                    sequence: 0,
                    bytes: b"ue".to_vec(),
                    last: true,
                },
            ] {
                protocol::send(&mut writer, &frame, s.until, &|| false).unwrap();
            }
            s.acknowledge(s.await_response(rx).unwrap()).unwrap();
            assert!(!s.cancelled.load(Ordering::SeqCst));
            s.cancel();
        });
    }
    #[test]
    fn central_reader_rejects_oversized_uncorrelated_and_eof_frames() {
        for case in 0..3 {
            let s = shared();
            let (input, mut writer) = pipe();
            let (prompt_tx, _) = mpsc::sync_channel(1);
            let (tx, _) = mpsc::sync_channel(1);
            s.pending.lock().unwrap().insert(
                1,
                Pending {
                    tx,
                    cap: 2,
                    content_type: Some("application/json".into()),
                    body: Vec::new(),
                },
            );
            if case != 2 {
                protocol::send(
                    &mut writer,
                    &Frame::ResponseChunk {
                        request_id: if case == 1 {
                            "foreign".into()
                        } else {
                            s.job.clone()
                        },
                        sequence: 1,
                        bytes: b"true".to_vec(),
                        last: true,
                    },
                    s.until,
                    &|| false,
                )
                .unwrap();
            }
            drop(writer);
            demux(BufReader::new(input), s.clone(), prompt_tx);
            assert!(s.cancelled.load(Ordering::SeqCst));
        }
    }
    #[test]
    fn sealed_fetch_and_unguarded_model_never_emit_requests() {
        let s = shared();
        s.fetch_closed.store(true, Ordering::SeqCst);
        let mut downloaded = 0;
        let mut advertised = 0;
        assert!(route(
            Http {
                method: "GET".into(),
                path: "/repo/0.git/info/refs?service=git-upload-pack".into(),
                content_type: String::new(),
                body: vec![]
            },
            &s,
            1,
            &mut downloaded,
            &mut advertised
        )
        .is_err());
        assert!(route(
            Http {
                method: "POST".into(),
                path: "/v1/chat/completions".into(),
                content_type: "application/json".into(),
                body: b"{}".to_vec()
            },
            &s,
            1,
            &mut downloaded,
            &mut advertised
        )
        .is_err());
        assert_eq!(s.sequence.load(Ordering::SeqCst), 1);
        assert!(s.pending.lock().unwrap().is_empty());
    }
    #[test]
    fn original_inference_deadline_does_not_reset_with_phase_changes() {
        let s = shared();
        let original = Instant::now() + Duration::from_millis(100);
        *s.inference.lock().unwrap() = Some(original);
        s.sealed.store(true, Ordering::SeqCst);
        assert_eq!(s.deadline(), original);
        s.dispatched.store(true, Ordering::SeqCst);
        assert_eq!(s.deadline(), original);
    }
    #[test]
    fn buffered_completion_becomes_compatible_sse() {
        let value = json!({"id":"x","choices":[{"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call_x","type":"function","function":{"name":"read","arguments":"{}"}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":12,"completion_tokens":8,"total_tokens":20}});
        let stream = String::from_utf8(sse(&value).unwrap()).unwrap();
        assert!(stream.contains("\"index\":0"));
        assert!(stream.contains("\"finish_reason\":\"tool_calls\""));
        assert!(stream.contains("\"usage\""));
        assert!(stream.ends_with("data: [DONE]\n\n"));
        assert!(sse(&json!({"choices":[]})).is_err());
    }
    #[test]
    fn final_reply_uses_only_exact_correlated_text() {
        let mut value = json!({"info":{"id":"msg_final","sessionID":"ses_x","parentID":"msg_job","role":"assistant","finish":"stop"},"parts":[{"type":"reasoning","text":"not a reply","sessionID":"ses_x","messageID":"msg_final"},{"type":"text","text":"answer","sessionID":"ses_x","messageID":"msg_final"}]});
        assert_eq!(final_reply(&value, "ses_x", "msg_job").unwrap().1, "answer");
        value["info"]["error"] = json!({});
        assert!(final_reply(&value, "ses_x", "msg_job").is_err());
        value["info"].as_object_mut().unwrap().remove("error");
        value["parts"][1]["messageID"] = json!("msg_foreign");
        assert!(final_reply(&value, "ses_x", "msg_job").is_err());
    }
    #[test]
    fn guard_system_literal_and_no_pure() {
        assert!(std::str::from_utf8(GUARD)
            .unwrap()
            .contains(protocol::SYSTEM_PROMPT));
        let c = configuration("127.0.0.1:1234".parse().unwrap());
        assert_eq!(c["enabled_providers"], json!(["smith-local"]));
        assert_eq!(c["plugin"], json!(["file:///etc/smith-opencode/guard.mjs"]));
        assert_eq!(c["compaction"]["auto"], false);
        let shared = shared();
        let command = opencode_command("127.0.0.1:1234".parse().unwrap(), &shared);
        assert_eq!(command.get_program(), "/usr/local/bin/opencode");
        assert!(!command.get_envs().any(|(k, _)| k == "OPENCODE_PURE"));
        assert!(command
            .get_envs()
            .any(|(k, v)| k == "OPENCODE_DISABLE_DEFAULT_PLUGINS"
                && v == Some(std::ffi::OsStr::new("true"))));
    }
    #[test]
    fn clone_branch_and_checkout_evidence_share_identity() {
        for (input, branch, evidence) in [
            ("main", "main", "refs/heads/main"),
            ("refs/heads/main", "main", "refs/heads/main"),
            (
                "feature/nested",
                "feature/nested",
                "refs/heads/feature/nested",
            ),
            (
                "refs/heads/feature/nested",
                "feature/nested",
                "refs/heads/feature/nested",
            ),
        ] {
            assert_eq!(branch_name(input), branch);
            assert_eq!(checkout_reference(Some(input)), evidence);
        }
        assert_eq!(checkout_reference(None), "HEAD");
    }
    #[test]
    fn prepared_git_command_has_no_ambient_helpers_or_protocol_fallback() {
        let command = git();
        assert_eq!(command.get_program(), "/usr/bin/git");
        let args: Vec<_> = command.get_args().map(|a| a.to_str().unwrap()).collect();
        for expected in [
            "protocol.version=0",
            "protocol.allow=never",
            "protocol.http.allow=always",
            "credential.helper=",
            "core.hooksPath=/dev/null",
            "core.fsmonitor=false",
            "http.followRedirects=false",
            "filter.lfs.process=",
        ] {
            assert!(args.contains(&expected));
        }
        assert!(
            command
                .get_envs()
                .any(|(k, v)| k == "GIT_CONFIG_GLOBAL"
                    && v == Some(std::ffi::OsStr::new("/dev/null")))
        );
    }
    #[test]
    fn strict_headers_reject_duplicates_and_smuggling() {
        assert!(
            headers("HTTP/1.1 200 OK\r\nContent-Length: 1\r\ncontent-length: 2\r\n\r\n").is_err()
        );
        assert!(headers("HTTP/1.1 200 OK\r\n folded: x\r\n\r\n").is_err());
    }
}
