//! Fixed-loopback installer provider. Inventory is never cached or inference evidence.
//!
//! Pull progress is part of the operation budget, not arbitrary blocking `io::Write`.
//! Native output supports nonblocking Linux pipes, sockets and terminals only; regular
//! files and other devices are rejected because O_NONBLOCK does not bound their writes.
//! These are cooperative operation bounds, not real-time scheduling or daemon-stop guarantees.
use crate::Result;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};

const ENDPOINT: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 11434));
const TAG_BYTES: usize = 256 * 1024;
const TAG_COUNT: usize = 256;
const FRAME_BYTES: usize = 16 * 1024;
const PROGRESS_BYTES: usize = 16 * 1024 * 1024;
const PROGRESS_FRAMES: usize = 65536;
const INFERENCE_BYTES: usize = 64 * 1024;
const TEXT_BYTES: usize = 4096;
const HEADER_BYTES: usize = 64 * 1024;
const POLL: Duration = Duration::from_millis(100);
const OUTCOME: &str = "external daemon work and partial cache may continue; outcome unknown; no cache was deleted; no automatic retry";

/// A progress sink whose implementation must not enter an unbounded write or flush.
///
/// Complete the bounded message, or return an error by the supplied absolute deadline
/// or on cancellation (including while the destination is backpressured). Partial
/// delivery on error is possible. Implementations must not abandon a writer thread.
/// There is deliberately no adapter/blanket implementation for arbitrary `io::Write`.
/// The provided sinks accept at most 1280 bytes per call. Split larger non-model
/// reports into bounded chunks under ONE unchanged deadline, not a budget per chunk.
pub trait ProgressOutput {
    /// Numeric telemetry is strict text output unless the observer explicitly coalesces it.
    fn numeric_until(
        &mut self,
        downloaded: u64,
        total: u64,
        deadline: Instant,
        cancelled: fn() -> bool,
    ) -> Result<()> {
        self.write_until(
            format!("Downloaded {downloaded}/{total} bytes\n").as_bytes(),
            deadline,
            cancelled,
        )
    }

    fn write_until(
        &mut self,
        bytes: &[u8],
        deadline: Instant,
        cancelled: fn() -> bool,
    ) -> Result<()>;
}

const PROGRESS_LINE_BYTES: usize = 1280;
const VECTOR_BYTES: usize = 32 * 1024 * 1024;

/// Bounded, in-memory progress for library fixtures; never calls a user-supplied writer.
/// Retains at most 32 MiB by default; `with_limit` can select a smaller byte bound.
#[derive(Default)]
pub struct VectorProgress {
    bytes: Vec<u8>,
    limit: Option<usize>,
}
impl VectorProgress {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_limit(limit: usize) -> Result<Self> {
        if limit > VECTOR_BYTES {
            return Err("Progress vector limit exceeds 32 MiB".into());
        }
        Ok(Self {
            bytes: Vec::new(),
            limit: Some(limit),
        })
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}
impl ProgressOutput for VectorProgress {
    fn write_until(
        &mut self,
        bytes: &[u8],
        deadline: Instant,
        cancelled: fn() -> bool,
    ) -> Result<()> {
        progress_message(bytes, deadline, cancelled)?;
        if bytes.len()
            > self
                .limit
                .unwrap_or(VECTOR_BYTES)
                .saturating_sub(self.bytes.len())
        {
            return Err("Progress vector bound exceeded".into());
        }
        self.bytes
            .try_reserve_exact(bytes.len())
            .map_err(|_| "Unable to reserve bounded progress output")?;
        progress_remaining(deadline, cancelled)?;
        self.bytes.extend_from_slice(bytes);
        progress_remaining(deadline, cancelled)?;
        Ok(())
    }
}

/// Constant-memory progress sink for fixtures or explicitly discarded output.
pub struct DiscardProgress;
impl ProgressOutput for DiscardProgress {
    fn write_until(
        &mut self,
        bytes: &[u8],
        deadline: Instant,
        cancelled: fn() -> bool,
    ) -> Result<()> {
        progress_message(bytes, deadline, cancelled)
    }
}

/// Unbuffered native progress. Owns a CLOEXEC duplicate, not the caller's descriptor.
///
/// Construction sets O_NONBLOCK on the shared open-file description. Scope this sink
/// to the operation; do not concurrently write through stdio buffers or change the
/// shared descriptor flags. Same-UID flag races are not a filesystem/socket CAS.
/// `finish` reports restoration errors; Drop attempts restoration on all other exits.
/// Regular files and non-terminal devices are rejected BEFORE changing any flags.
/// Socket closure/output failure does not promise external daemon/cache cancellation.
pub struct FdProgress {
    fd: OwnedFd,
    original_flags: Option<libc::c_int>,
    #[cfg(test)]
    on_backpressure: Option<fn()>,
}
impl FdProgress {
    pub fn stdout() -> Result<Self> {
        let stdout = io::stdout();
        Self::from_fd(stdout.as_fd())
    }
    pub fn from_fd(fd: BorrowedFd<'_>) -> Result<Self> {
        // SAFETY: fd is borrowed/open; a successful fcntl creates a new owned CLOEXEC fd.
        let duplicate = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
        if duplicate < 0 {
            return Err("Cannot capture progress output descriptor".into());
        }
        // SAFETY: duplicate is a newly allocated descriptor owned only by this object.
        let fd = unsafe { OwnedFd::from_raw_fd(duplicate) };
        // SAFETY: fstat initializes stat, whose contents are read only on success.
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(fd.as_raw_fd(), &mut stat) } < 0 {
            return Err("Cannot inspect progress output descriptor".into());
        }
        let kind = stat.st_mode & libc::S_IFMT;
        // SAFETY: isatty inspects the live descriptor and does not acquire a stdio lock.
        if kind != libc::S_IFIFO
            && kind != libc::S_IFSOCK
            && !(kind == libc::S_IFCHR && unsafe { libc::isatty(fd.as_raw_fd()) } == 1)
        {
            return Err("Progress output requires a pipe, socket or terminal; regular files and other devices cannot guarantee bounded writes".into());
        }
        // SAFETY: fcntl operates on our live fd and takes no pointer arguments here.
        let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
        if flags < 0 || flags & libc::O_ACCMODE == libc::O_RDONLY {
            return Err("Progress output descriptor is not writable".into());
        }
        if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err("Cannot make progress output nonblocking".into());
        }
        Ok(Self {
            fd,
            original_flags: Some(flags),
            #[cfg(test)]
            on_backpressure: None,
        })
    }
    /// Restore shared descriptor flags while reporting failure, then close our duplicate.
    pub fn finish(mut self) -> Result<()> {
        self.restore()
    }
    fn restore(&mut self) -> Result<()> {
        if let Some(flags) = self.original_flags {
            // SAFETY: our owned descriptor remains open until after restoration/Drop.
            if unsafe { libc::fcntl(self.fd.as_raw_fd(), libc::F_SETFL, flags) } < 0 {
                return Err("Cannot restore progress output flags".into());
            }
            self.original_flags = None;
        }
        Ok(())
    }
}
impl Drop for FdProgress {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}
impl ProgressOutput for FdProgress {
    fn write_until(
        &mut self,
        mut bytes: &[u8],
        deadline: Instant,
        cancelled: fn() -> bool,
    ) -> Result<()> {
        progress_message(bytes, deadline, cancelled)?;
        while !bytes.is_empty() {
            progress_remaining(deadline, cancelled)?;
            // Detect accidental shared-flag changes; exclusive flag ownership is still
            // required because another actor could race this check and the syscall.
            let flags = unsafe { libc::fcntl(self.fd.as_raw_fd(), libc::F_GETFL) };
            if flags < 0 || flags & libc::O_NONBLOCK == 0 {
                return Err("Progress output lost nonblocking descriptor flags".into());
            }
            // SAFETY: bytes is live for its bounded length; our fd is owned/nonblocking.
            // The native Rust recipient retains Rust's usual ignored-SIGPIPE behavior.
            let count =
                unsafe { libc::write(self.fd.as_raw_fd(), bytes.as_ptr().cast(), bytes.len()) };
            if count > 0 {
                bytes = &bytes[count as usize..];
                progress_remaining(deadline, cancelled)?;
                continue;
            }
            if count == 0 {
                return Err("Unable to write Ollama progress: output closed".into());
            }
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            if error.kind() != io::ErrorKind::WouldBlock {
                return Err("Unable to write Ollama progress: output failure".into());
            }
            #[cfg(test)]
            if let Some(observe) = self.on_backpressure {
                observe();
            }
            let remaining = progress_remaining(deadline, cancelled)?;
            let mut poll = libc::pollfd {
                fd: self.fd.as_raw_fd(),
                events: libc::POLLOUT,
                revents: 0,
            };
            // SAFETY: poll borrows one initialized entry for a finite cancellation slice.
            let ready = unsafe {
                libc::poll(
                    &mut poll,
                    1,
                    remaining.min(POLL).as_millis().clamp(1, 100) as i32,
                )
            };
            progress_remaining(deadline, cancelled)?;
            if ready < 0 {
                if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err("Unable to poll Ollama progress output".into());
            }
            if poll.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                return Err("Unable to write Ollama progress: output closed".into());
            }
        }
        progress_remaining(deadline, cancelled)?;
        Ok(())
    }
}
fn progress_remaining(deadline: Instant, cancelled: fn() -> bool) -> Result<Duration> {
    if cancelled() {
        return Err("Ollama operation cancelled during progress output".into());
    }
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| "Ollama overall deadline exceeded during progress output".into())
}
fn progress_message(bytes: &[u8], deadline: Instant, cancelled: fn() -> bool) -> Result<()> {
    progress_remaining(deadline, cancelled)?;
    if bytes.len() > PROGRESS_LINE_BYTES {
        return Err("Progress output message bound exceeded".into());
    }
    Ok(())
}

pub trait Provider {
    fn tags(&mut self) -> Result<Vec<String>>;
    fn pull(&mut self, model: &str, output: &mut dyn ProgressOutput) -> Result<()>;
    fn inference(&mut self, model: &str) -> Result<String>;
}

/// An exact, bounded, local tag, not an endpoint or an inferred/default model.
pub fn valid_model(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 256
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/:_.-".contains(&b))
        && name
            .rsplit_once(':')
            .is_some_and(|(base, tag)| !base.is_empty() && !tag.is_empty())
        && !name.to_ascii_lowercase().contains("cloud")
        && !name.starts_with("http:")
        && !name.starts_with("https:")
}

#[derive(Clone, Copy)]
struct Timing {
    connect: Duration,
    tags: Duration,
    tags_idle: Duration,
    pull: Duration,
    inference: Duration,
    idle: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(2),
            tags: Duration::from_secs(10),
            tags_idle: Duration::from_secs(2),
            pull: Duration::from_secs(900),
            inference: Duration::from_secs(60),
            idle: Duration::from_secs(30),
        }
    }
}

pub struct Ollama {
    endpoint: SocketAddr,
    timing: Timing,
    cancelled: fn() -> bool,
}

impl Ollama {
    /// Construction performs no discovery, network access or provider effects.
    pub fn new() -> Result<Self> {
        Ok(Self {
            endpoint: ENDPOINT,
            timing: Timing::default(),
            cancelled: crate::consent::cancelled,
        })
    }

    fn check(&self) -> Result<()> {
        if (self.cancelled)() {
            return Err(format!("Ollama operation cancelled; {OUTCOME}"));
        }
        Ok(())
    }

    fn open(
        &self,
        path: &str,
        body: Option<Value>,
        overall: Duration,
        idle: Duration,
    ) -> Result<Http> {
        self.check()?;
        let budget = Budget::new(overall, idle, self.cancelled);
        let connect = self.timing.connect.min(budget.remaining()?);
        let stream = TcpStream::connect_timeout(&self.endpoint, connect)
            .map_err(|_| format!("Ollama unavailable at numeric loopback; install/start Ollama separately; {OUTCOME}"))?;
        budget.check()?;
        let mut wire = Wire {
            stream,
            budget,
            buffer: [0; 8192],
            start: 0,
            end: 0,
        };
        let encoded = body.map(|v| v.to_string()).unwrap_or_default();
        let method = if encoded.is_empty() { "GET" } else { "POST" };
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{encoded}",
            self.endpoint, encoded.len()
        );
        wire.write_all(request.as_bytes())?;
        Http::open(wire)
    }
}

impl Provider for Ollama {
    fn tags(&mut self) -> Result<Vec<String>> {
        self.check()?;
        let mut response = self.open("/api/tags", None, self.timing.tags, self.timing.tags_idle)?;
        let value = decode(&response.body(TAG_BYTES)?)?;
        response.wire.budget.check()?;
        let models = value
            .as_object()
            .and_then(|v| v.get("models"))
            .and_then(Value::as_array)
            .filter(|v| v.len() <= TAG_COUNT)
            .ok_or("Invalid fresh Ollama inventory")?;
        let mut names = HashSet::new();
        // Ambiguity in ANY raw row blocks inventory, even one later excluded as remote/unsupported.
        for item in models {
            response.wire.budget.check()?;
            let row = item.as_object().ok_or("Invalid tag metadata")?;
            let name = row
                .get("name")
                .and_then(Value::as_str)
                .ok_or("Invalid tag metadata")?;
            row.get("digest")
                .and_then(Value::as_str)
                .ok_or("Invalid tag metadata")?;
            if !names.insert(name) {
                return Err("Duplicate installed tag identity".into());
            }
            if row.get("model").is_some_and(|v| v.as_str() != Some(name)) {
                return Err("Conflicting installed tag identity".into());
            }
        }
        let mut eligible = Vec::new();
        for item in models {
            response.wire.budget.check()?;
            let name = item["name"].as_str().ok_or("Invalid tag metadata")?;
            if !valid_model(name) || remote_metadata(item) {
                continue;
            }
            let digest = item["digest"].as_str().ok_or("Invalid tag metadata")?;
            if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err("Invalid tag digest".into());
            }
            if matches!(
                item.get("details")
                    .and_then(|v| v.get("format"))
                    .and_then(Value::as_str),
                Some("gguf" | "safetensors")
            ) {
                eligible.push(name.to_owned());
            }
        }
        response.wire.budget.check()?;
        Ok(eligible)
    }

    fn pull(&mut self, model: &str, output: &mut dyn ProgressOutput) -> Result<()> {
        self.check()?;
        if !valid_model(model) {
            return Err("An exact eligible local model tag is required".into());
        }
        let result = (|| {
            let mut response = self.open(
                "/api/pull",
                Some(json!({"model": model, "stream": true})),
                self.timing.pull,
                self.timing.idle,
            )?;
            let mut pending = Vec::new();
            let mut total = 0usize;
            let mut frames = 0usize;
            let mut success = false;
            let mut buffer = [0; 4096];
            while !success {
                self.check()?;
                let count = response.read(&mut buffer)?;
                if count == 0 {
                    return Err("Pull ended without success".into());
                }
                total += count;
                if total > PROGRESS_BYTES {
                    return Err("Pull total progress bound exceeded".into());
                }
                pending.extend_from_slice(&buffer[..count]);
                let mut consumed = 0;
                while let Some(newline) = pending[consumed..].iter().position(|&b| b == b'\n') {
                    self.check()?;
                    let end = consumed + newline;
                    frames += 1;
                    if end - consumed > FRAME_BYTES || frames > PROGRESS_FRAMES {
                        return Err("Pull progress bound exceeded".into());
                    }
                    let event = decode(&pending[consumed..end])?;
                    let event = event.as_object().ok_or("Pull error/invalid progress")?;
                    let status = event
                        .get("status")
                        .and_then(Value::as_str)
                        .filter(|s| s.len() <= 1024)
                        .ok_or("Pull error/invalid progress")?;
                    if event.contains_key("error") {
                        return Err("Pull error/invalid progress".into());
                    }
                    for key in ["total", "completed"] {
                        if event.get(key).is_some_and(|v| v.as_u64().is_none()) {
                            return Err("Invalid pull byte count".into());
                        }
                    }
                    let counts = match (event.get("completed"), event.get("total")) {
                        (Some(completed), Some(total)) => {
                            format!(" (reported layer bytes {completed}/{total})")
                        }
                        _ => String::new(),
                    };
                    // Only daemon-reported layer counts; never invent a download percentage or size.
                    response.wire.budget.check()?;
                    let line = format!("Ollama: {}{counts}\n", display(status));
                    output.write_until(
                        line.as_bytes(),
                        response.wire.budget.deadline,
                        self.cancelled,
                    )?;
                    response.wire.budget.check()?;
                    consumed = end + 1;
                    if status == "success" {
                        success = true;
                        if pending[consumed..].iter().any(|b| !b.is_ascii_whitespace()) {
                            return Err("Unexpected data after pull success".into());
                        }
                        break;
                    }
                }
                pending.drain(..consumed);
                if pending.len() > FRAME_BYTES {
                    return Err("Pull frame bound exceeded".into());
                }
                response.wire.budget.check()?;
            }
            // Close our own stream before fresh verification. This is not daemon cancellation.
            drop(response);
            if !self.tags()?.iter().any(|tag| tag == model) {
                return Err(
                    "Pull success did not freshly verify the exact eligible local tag".into(),
                );
            }
            self.check()
        })();
        result.map_err(|e: String| {
            if e.contains(OUTCOME) {
                e
            } else {
                format!("{e}; {OUTCOME}")
            }
        })
    }

    fn inference(&mut self, model: &str) -> Result<String> {
        self.check()?;
        if !valid_model(model) || !self.tags()?.iter().any(|tag| tag == model) {
            return Err("Exact local model no longer available".into());
        }
        let mut response = self.open(
            "/api/generate",
            Some(json!({
                "model": model, "prompt": "Reply with OK.", "stream": false,
                "options": {"num_predict": 8, "temperature": 0}, "keep_alive": 0,
            })),
            self.timing.inference,
            self.timing.idle,
        )?;
        let result = decode(&response.body(INFERENCE_BYTES)?)?;
        response.wire.budget.check()?;
        let result = result
            .as_object()
            .ok_or("Inference response not verified")?;
        let text = result
            .get("response")
            .and_then(Value::as_str)
            .filter(|s| s.len() <= TEXT_BYTES && !s.trim().is_empty())
            .ok_or("Inference response not verified")?;
        let tools_ok = match result.get("tool_calls") {
            None | Some(Value::Null) => true,
            Some(Value::Array(calls)) => calls.is_empty(),
            _ => false,
        };
        // Generated text is inert data, not evidence of a remote model. Inspect all OTHER metadata.
        if result.get("model").and_then(Value::as_str) != Some(model)
            || result.get("done") != Some(&Value::Bool(true))
            || result.contains_key("error")
            || !tools_ok
            || result
                .iter()
                .any(|(key, value)| key != "response" && remote_field(key, value))
        {
            return Err("Inference response not verified".into());
        }
        let safe = display(text);
        if safe.trim().is_empty() {
            return Err("Inference response not verified".into());
        }
        response.wire.budget.check()?;
        Ok(safe)
    }
}

fn remote_field(key: &str, value: &Value) -> bool {
    let key = key.to_ascii_lowercase();
    ((key.contains("remote") || key.contains("cloud"))
        && !matches!(value, Value::Null | Value::Bool(false))
        && value.as_str() != Some(""))
        || remote_metadata(value)
}

fn remote_metadata(value: &Value) -> bool {
    match value {
        Value::Object(fields) => fields.iter().any(|(k, v)| remote_field(k, v)),
        Value::Array(values) => values.iter().any(remote_metadata),
        Value::String(s) => {
            matches!(s.to_ascii_lowercase().as_str(), "cloud" | "remote")
                || s.starts_with("http://")
                || s.starts_with("https://")
        }
        _ => false,
    }
}

fn display(value: &str) -> String {
    value
        .chars()
        .filter(|c| {
            !c.is_control()
                && (!c.is_whitespace() || *c == ' ')
                && !matches!(*c,
                '\u{00ad}' | '\u{0600}'..='\u{0605}' | '\u{061c}' | '\u{06dd}' | '\u{070f}'
                | '\u{0890}'..='\u{0891}' | '\u{08e2}' | '\u{180e}' | '\u{200b}'..='\u{200f}'
                | '\u{2028}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}'
                | '\u{fff9}'..='\u{fffb}' | '\u{110bd}' | '\u{110cd}' | '\u{13430}'..='\u{13455}'
                | '\u{1bca0}'..='\u{1bca3}' | '\u{1d173}'..='\u{1d17a}' | '\u{e0000}'..='\u{e0fff}')
        })
        .collect()
}

// Value's default deserializer silently overwrites duplicate object keys. Check EVERY object,
// including unknown metadata, with the Python contract's root-at-zero nesting limit.
struct JsonSeed(usize);
impl<'de> DeserializeSeed<'de> for JsonSeed {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<Value, D::Error> {
        if self.0 > 64 {
            return Err(de::Error::custom("JSON nesting bound exceeded"));
        }
        deserializer.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for JsonSeed {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("strict bounded JSON")
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> std::result::Result<Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Value, E> {
        Ok(v.into())
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Value, E> {
        Ok(v.into())
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<Value, E> {
        serde_json::Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| E::custom("Invalid JSON number"))
    }
    fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Value, E> {
        Ok(Value::String(v.into()))
    }
    fn visit_string<E: de::Error>(self, v: String) -> std::result::Result<Value, E> {
        Ok(Value::String(v))
    }
    fn visit_unit<E: de::Error>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = seq.next_element_seed(JsonSeed(self.0 + 1))? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom("Duplicate JSON field"));
            }
            values.insert(key, map.next_value_seed(JsonSeed(self.0 + 1))?);
        }
        Ok(Value::Object(values))
    }
}
fn decode(data: &[u8]) -> Result<Value> {
    let mut deserializer = serde_json::Deserializer::from_slice(data);
    let value = JsonSeed(0)
        .deserialize(&mut deserializer)
        .map_err(|_| "Malformed bounded JSON response")?;
    deserializer
        .end()
        .map_err(|_| "Malformed bounded JSON response")?;
    Ok(value)
}

struct Budget {
    deadline: Instant,
    last_io: Instant,
    idle: Duration,
    cancelled: fn() -> bool,
}
impl Budget {
    fn new(overall: Duration, idle: Duration, cancelled: fn() -> bool) -> Self {
        let now = Instant::now();
        Self {
            deadline: now + overall,
            last_io: now,
            idle,
            cancelled,
        }
    }
    fn check(&self) -> Result<()> {
        if (self.cancelled)() {
            return Err(format!("Ollama operation cancelled; {OUTCOME}"));
        }
        if Instant::now() >= self.deadline {
            return Err(format!("Ollama overall deadline exceeded; {OUTCOME}"));
        }
        Ok(())
    }
    fn remaining(&self) -> Result<Duration> {
        self.check()?;
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| format!("Ollama overall deadline exceeded; {OUTCOME}"))
    }
    fn io_timeout(&self) -> Result<Duration> {
        let remaining = self.remaining()?;
        let idle = self
            .idle
            .checked_sub(self.last_io.elapsed())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| format!("Ollama idle timeout exceeded; {OUTCOME}"))?;
        Ok(POLL.min(remaining).min(idle))
    }
}

// Blocking reqwest does not expose per-read socket cancellation. Keep exactly one owned
// plain-HTTP socket: no DNS, proxy, redirect, pooled connection, worker thread or request retry.
// Absolute budgets apply to raw reads, so drip-fed headers/chunk framing cannot hide a wait.
struct Wire {
    stream: TcpStream,
    budget: Budget,
    buffer: [u8; 8192],
    start: usize,
    end: usize,
}
impl Wire {
    fn write_all(&mut self, mut bytes: &[u8]) -> Result<()> {
        while !bytes.is_empty() {
            let timeout = self.budget.io_timeout()?;
            self.stream
                .set_write_timeout(Some(timeout))
                .map_err(|_| "Unable to bound Ollama socket")?;
            match self.stream.write(bytes) {
                Ok(0) => {
                    return Err(format!(
                        "Ollama connection closed during request; {OUTCOME}"
                    ))
                }
                Ok(count) => {
                    self.budget.io_timeout()?;
                    self.budget.last_io = Instant::now();
                    bytes = &bytes[count..];
                }
                Err(e) if transient(&e) => continue,
                Err(_) => return Err(format!("Ollama request write failed; {OUTCOME}")),
            }
        }
        self.budget.check()
    }
    fn read(&mut self, bytes: &mut [u8]) -> Result<usize> {
        self.budget.check()?;
        if bytes.is_empty() {
            return Ok(0);
        }
        if self.start == self.end {
            loop {
                let timeout = self.budget.io_timeout()?;
                self.stream
                    .set_read_timeout(Some(timeout))
                    .map_err(|_| "Unable to bound Ollama socket")?;
                match self.stream.read(&mut self.buffer) {
                    Ok(count) => {
                        self.budget.io_timeout()?;
                        self.budget.last_io = Instant::now();
                        self.start = 0;
                        self.end = count;
                        if count == 0 {
                            return Ok(0);
                        }
                        break;
                    }
                    Err(e) if transient(&e) => continue,
                    Err(_) => return Err(format!("Ollama response read failed; {OUTCOME}")),
                }
            }
        }
        let count = bytes.len().min(self.end - self.start);
        bytes[..count].copy_from_slice(&self.buffer[self.start..self.start + count]);
        self.start += count;
        self.budget.check()?;
        Ok(count)
    }
    fn exact(&mut self, mut bytes: &mut [u8]) -> Result<()> {
        while !bytes.is_empty() {
            let count = self.read(bytes)?;
            if count == 0 {
                return Err("Truncated Ollama HTTP response".into());
            }
            bytes = &mut bytes[count..];
        }
        Ok(())
    }
    fn line(&mut self, limit: usize) -> Result<String> {
        let mut line = Vec::new();
        loop {
            let mut byte = [0];
            self.exact(&mut byte)?;
            line.push(byte[0]);
            if line.len() > limit {
                return Err("Ollama HTTP framing bound exceeded".into());
            }
            if byte[0] == b'\n' {
                if !line.ends_with(b"\r\n") {
                    return Err("Malformed Ollama HTTP framing".into());
                }
                line.truncate(line.len() - 2);
                if !line
                    .iter()
                    .all(|b| b.is_ascii() && (!b.is_ascii_control() || *b == b'\t'))
                {
                    return Err("Malformed Ollama HTTP framing".into());
                }
                return String::from_utf8(line).map_err(|_| "Malformed Ollama HTTP framing".into());
            }
        }
    }
}
fn transient(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}

enum Framing {
    Length(usize),
    Chunked {
        remaining: usize,
        crlf: bool,
        done: bool,
    },
    Close,
}
struct Http {
    wire: Wire,
    framing: Framing,
}
impl Http {
    fn open(mut wire: Wire) -> Result<Self> {
        let status = wire.line(HEADER_BYTES)?;
        let code = status
            .strip_prefix("HTTP/1.1 ")
            .or_else(|| status.strip_prefix("HTTP/1.0 "))
            .and_then(|s| s.split(' ').next());
        if code != Some("200") {
            return Err(format!("Ollama HTTP failure/redirect; {OUTCOME}"));
        }
        let mut bytes = status.len() + 2;
        let mut length = None;
        let mut chunked = false;
        let mut encoding = false;
        loop {
            let line = wire.line(HEADER_BYTES)?;
            bytes += line.len() + 2;
            if bytes > HEADER_BYTES {
                return Err("Ollama HTTP header bound exceeded".into());
            }
            if line.is_empty() {
                break;
            }
            let (name, value) = header(&line)?;
            let value = value.trim();
            if name.eq_ignore_ascii_case("content-length") {
                if length.is_some()
                    || value.is_empty()
                    || !value.bytes().all(|b| b.is_ascii_digit())
                {
                    return Err("Ambiguous Ollama HTTP content length".into());
                }
                length = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| "Invalid Ollama HTTP content length")?,
                );
            } else if name.eq_ignore_ascii_case("transfer-encoding") {
                if chunked || !value.eq_ignore_ascii_case("chunked") {
                    return Err("Unsupported Ollama HTTP transfer encoding".into());
                }
                chunked = true;
            } else if name.eq_ignore_ascii_case("content-encoding") {
                if encoding || !value.eq_ignore_ascii_case("identity") {
                    return Err("Unsupported Ollama HTTP content encoding".into());
                }
                encoding = true;
            }
        }
        if chunked && length.is_some() {
            return Err("Ambiguous Ollama HTTP framing".into());
        }
        let framing = if chunked {
            Framing::Chunked {
                remaining: 0,
                crlf: false,
                done: false,
            }
        } else if let Some(length) = length {
            Framing::Length(length)
        } else {
            Framing::Close
        };
        Ok(Self { wire, framing })
    }
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize> {
        self.wire.budget.check()?;
        if buffer.is_empty() {
            return Ok(0);
        }
        match &mut self.framing {
            Framing::Length(remaining) => {
                if *remaining == 0 {
                    return Ok(0);
                }
                let maximum = buffer.len().min(*remaining);
                let count = self.wire.read(&mut buffer[..maximum])?;
                if count == 0 {
                    return Err("Truncated Ollama HTTP body".into());
                }
                *remaining -= count;
                Ok(count)
            }
            Framing::Close => self.wire.read(buffer),
            Framing::Chunked {
                remaining,
                crlf,
                done,
            } => {
                if *done {
                    return Ok(0);
                }
                if *remaining == 0 {
                    if *crlf {
                        let mut end = [0; 2];
                        self.wire.exact(&mut end)?;
                        if end != *b"\r\n" {
                            return Err("Malformed Ollama HTTP chunk".into());
                        }
                        *crlf = false;
                    }
                    let line = self.wire.line(1024)?;
                    let size = line
                        .split(';')
                        .next()
                        .ok_or("Malformed Ollama HTTP chunk size")?;
                    if size.is_empty() || !size.bytes().all(|b| b.is_ascii_hexdigit()) {
                        return Err("Malformed Ollama HTTP chunk size".into());
                    }
                    *remaining = usize::from_str_radix(size, 16)
                        .map_err(|_| "Invalid Ollama HTTP chunk size")?;
                    if *remaining == 0 {
                        let mut bytes = 0;
                        loop {
                            let trailer = self.wire.line(HEADER_BYTES)?;
                            bytes += trailer.len() + 2;
                            if bytes > HEADER_BYTES {
                                return Err("Ollama HTTP trailer bound exceeded".into());
                            }
                            if trailer.is_empty() {
                                break;
                            }
                            header(&trailer)?;
                        }
                        *done = true;
                        return Ok(0);
                    }
                }
                let maximum = buffer.len().min(*remaining);
                let count = self.wire.read(&mut buffer[..maximum])?;
                if count == 0 {
                    return Err("Truncated Ollama HTTP chunk".into());
                }
                *remaining -= count;
                if *remaining == 0 {
                    *crlf = true;
                }
                Ok(count)
            }
        }
    }
    fn body(&mut self, limit: usize) -> Result<Vec<u8>> {
        if matches!(self.framing, Framing::Length(length) if length > limit) {
            return Err("Ollama body bound exceeded".into());
        }
        let mut body = Vec::new();
        let mut buffer = [0; 8192];
        loop {
            let maximum = buffer.len().min(limit + 1 - body.len());
            let count = self.read(&mut buffer[..maximum])?;
            if count == 0 {
                return Ok(body);
            }
            body.extend_from_slice(&buffer[..count]);
            if body.len() > limit {
                return Err("Ollama body bound exceeded".into());
            }
        }
    }
}
fn header(line: &str) -> Result<(&str, &str)> {
    let (name, value) = line.split_once(':').ok_or("Malformed Ollama HTTP header")?;
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
    {
        return Err("Malformed Ollama HTTP header".into());
    }
    Ok((name, value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Condvar, Mutex};
    use std::thread::{self, JoinHandle};

    const MODEL: &str = "tiny:tag";

    struct RecordingProgress {
        bytes: Vec<u8>,
        fail: bool,
    }
    impl ProgressOutput for RecordingProgress {
        fn write_until(
            &mut self,
            bytes: &[u8],
            _deadline: Instant,
            _cancelled: fn() -> bool,
        ) -> Result<()> {
            if self.fail {
                return Err("strict sink failure".into());
            }
            self.bytes.extend_from_slice(bytes);
            Ok(())
        }
    }

    #[test]
    fn numeric_progress_defaults_to_strict_text_forwarding() {
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut output = RecordingProgress {
            bytes: Vec::new(),
            fail: false,
        };
        output.numeric_until(4, 9, deadline, || false).unwrap();
        assert_eq!(output.bytes, b"Downloaded 4/9 bytes\n");

        let mut broken = RecordingProgress {
            bytes: Vec::new(),
            fail: true,
        };
        assert_eq!(
            broken.numeric_until(4, 9, deadline, || false).unwrap_err(),
            "strict sink failure"
        );
    }

    thread_local! {
        static CANCEL: Cell<bool> = const { Cell::new(false) };
        static CANCEL_AT: Cell<Option<Instant>> = const { Cell::new(None) };
    }
    fn cancelled() -> bool {
        CANCEL.get()
            || CANCEL_AT
                .get()
                .is_some_and(|deadline| Instant::now() >= deadline)
    }
    struct ResetCancel;
    impl Drop for ResetCancel {
        fn drop(&mut self) {
            CANCEL.set(false);
            CANCEL_AT.set(None);
        }
    }

    #[derive(Debug, Clone)]
    struct Request {
        method: String,
        path: String,
        headers: String,
        body: Option<Value>,
    }
    enum Step {
        Bytes(Vec<u8>),
        Drip(Vec<u8>, Duration),
        Closed(Arc<AtomicBool>),
    }
    struct Reply(Vec<Step>);
    impl Reply {
        fn body(body: impl AsRef<[u8]>) -> Self {
            let body = body.as_ref();
            let mut bytes =
                format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).into_bytes();
            bytes.extend_from_slice(body);
            Self::raw(bytes)
        }
        fn json(value: Value) -> Self {
            Self::body(value.to_string())
        }
        fn raw(bytes: impl AsRef<[u8]>) -> Self {
            Self(vec![Step::Bytes(bytes.as_ref().to_vec())])
        }
        fn chunks(chunks: &[&[u8]]) -> Self {
            let mut bytes = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec();
            for chunk in chunks {
                bytes.extend_from_slice(format!("{:x};ignored=yes\r\n", chunk.len()).as_bytes());
                bytes.extend_from_slice(chunk);
                bytes.extend_from_slice(b"\r\n");
            }
            bytes.extend_from_slice(b"0\r\nX-Trailer: ignored\r\n\r\n");
            Self::raw(bytes)
        }
    }
    struct Server {
        endpoint: SocketAddr,
        requests: Arc<(Mutex<Vec<Request>>, Condvar)>,
        stop: Arc<AtomicBool>,
        worker: Option<JoinHandle<()>>,
    }
    impl Server {
        fn new(replies: Vec<Reply>) -> Self {
            Self::with_request_observer(replies, || {})
        }
        fn with_request_observer(
            replies: Vec<Reply>,
            mut before_record: impl FnMut() + Send + 'static,
        ) -> Self {
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
            listener.set_nonblocking(true).unwrap();
            let endpoint = listener.local_addr().unwrap();
            assert_ne!(endpoint.port(), 11434);
            let requests = Arc::new((Mutex::new(Vec::new()), Condvar::new()));
            let captured = requests.clone();
            let stop = Arc::new(AtomicBool::new(false));
            let stopped = stop.clone();
            let worker = thread::spawn(move || {
                for reply in replies {
                    let mut stream = loop {
                        if stopped.load(Ordering::SeqCst) {
                            return;
                        }
                        match listener.accept() {
                            Ok((stream, _)) => break stream,
                            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                                thread::sleep(Duration::from_millis(2))
                            }
                            Err(e) => panic!("fake accept: {e}"),
                        }
                    };
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    stream
                        .set_write_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let Some(request) = read_request(&mut stream) else {
                        continue;
                    };
                    before_record();
                    {
                        let (requests, observed) = &*captured;
                        requests.lock().unwrap().push(request);
                        observed.notify_all();
                    }
                    for step in reply.0 {
                        match step {
                            Step::Bytes(bytes) => {
                                if stream.write_all(&bytes).is_err() {
                                    break;
                                }
                            }
                            Step::Drip(bytes, interval) => {
                                for byte in bytes {
                                    if stopped.load(Ordering::SeqCst)
                                        || stream.write_all(&[byte]).is_err()
                                    {
                                        break;
                                    }
                                    thread::sleep(interval);
                                }
                            }
                            Step::Closed(closed) => {
                                let mut byte = [0];
                                let disconnected = match stream.read(&mut byte) {
                                    Ok(0) => true,
                                    Err(e) => matches!(
                                        e.kind(),
                                        io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe
                                    ),
                                    _ => false,
                                };
                                if disconnected {
                                    let (requests, observed) = &*captured;
                                    // Both predicates use this mutex so closure notification
                                    // cannot be lost between the assertion and condvar wait.
                                    let _guard = requests.lock().unwrap();
                                    closed.store(true, Ordering::SeqCst);
                                    observed.notify_all();
                                }
                            }
                        }
                    }
                }
            });
            Self {
                endpoint,
                requests,
                stop,
                worker: Some(worker),
            }
        }
        fn provider(&self) -> Ollama {
            // Private cfg(test) seam only: an already-reserved numeric ephemeral loopback port.
            Ollama {
                endpoint: self.endpoint,
                timing: Timing::default(),
                cancelled,
            }
        }
        fn requests(&self) -> Vec<Request> {
            self.requests.0.lock().unwrap().clone()
        }
        fn wait_observed(&self, count: usize, closed: &AtomicBool) -> Vec<Request> {
            // Client timeout/return is NOT evidence that the independent observer ran.
            // Keep the fixture alive until receipt AND peer closure are observed, with
            // a separate finite evidence budget rather than changing transport timing.
            let deadline = Instant::now() + Duration::from_secs(2);
            let (requests, observed) = &*self.requests;
            let mut requests = requests.lock().unwrap();
            while requests.len() < count || !closed.load(Ordering::SeqCst) {
                let remaining = deadline.checked_duration_since(Instant::now())
                    .filter(|time| !time.is_zero())
                    .unwrap_or_else(|| panic!("HTTP fixture observation timed out: requests={}, expected>={count}, closed={}", requests.len(), closed.load(Ordering::SeqCst)));
                (requests, _) = observed.wait_timeout(requests, remaining).unwrap();
            }
            requests.clone()
        }
    }
    impl Drop for Server {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            self.worker.take().unwrap().join().unwrap();
        }
    }
    fn read_request(stream: &mut TcpStream) -> Option<Request> {
        let mut headers = Vec::new();
        while !headers.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            if stream.read_exact(&mut byte).is_err() {
                return None;
            }
            headers.push(byte[0]);
            assert!(headers.len() <= 4096);
        }
        let headers = String::from_utf8(headers).unwrap();
        let mut first = headers.lines().next().unwrap().split_whitespace();
        let method = first.next().unwrap().to_owned();
        let path = first.next().unwrap().to_owned();
        let length = headers
            .lines()
            .find_map(|line| line.strip_prefix("Content-Length: "))
            .unwrap()
            .parse::<usize>()
            .unwrap();
        assert!(length <= 4096);
        let mut body = vec![0; length];
        if stream.read_exact(&mut body).is_err() {
            return None;
        }
        Some(Request {
            method,
            path,
            headers,
            body: if body.is_empty() {
                None
            } else {
                Some(serde_json::from_slice(&body).unwrap())
            },
        })
    }
    fn tag(name: &str) -> Value {
        json!({"name": name, "model": name, "digest": "b".repeat(64), "details": {"format": "gguf"}})
    }
    fn tags() -> Reply {
        Reply::json(json!({"models": [tag(MODEL)]}))
    }
    fn generated() -> Value {
        json!({"model": MODEL, "done": true, "response": "OK"})
    }
    fn pull_reply() -> Reply {
        Reply::body(b"{\"status\":\"success\"}\n")
    }
    fn quick() -> Timing {
        Timing {
            connect: Duration::from_secs(1),
            tags: Duration::from_millis(180),
            tags_idle: Duration::from_secs(1),
            pull: Duration::from_millis(180),
            inference: Duration::from_millis(180),
            idle: Duration::from_secs(1),
        }
    }

    // Maps next/dagger/tests/test_setup.py HttpTests provider cases. Matching names below
    // retain pull/fresh-tag, raw duplicate/conflict, ambiguity and inference/tool contracts.
    // The old mocked watchdog test maps to real overall_deadline_bounds_drip_* streams;
    // overall_deadline_and_cancellation_cleanup maps to raw-boundary cancellation/idle tests.
    // Wrapper subprocess and orchestration/consent cases belong to their respective owners.
    #[test]
    fn fixed_production_endpoint_and_budgets_construction_has_no_network() {
        let provider = Ollama::new().unwrap();
        assert_eq!(provider.endpoint, ENDPOINT);
        assert_eq!(provider.endpoint.to_string(), "127.0.0.1:11434");
        assert_eq!(provider.timing.connect, Duration::from_secs(2));
        assert_eq!(provider.timing.tags, Duration::from_secs(10));
        assert_eq!(provider.timing.tags_idle, Duration::from_secs(2));
        assert_eq!(provider.timing.pull, Duration::from_secs(900));
        assert_eq!(provider.timing.inference, Duration::from_secs(60));
        assert_eq!(provider.timing.idle, Duration::from_secs(30));
    }

    #[test]
    fn tags_eligible_formats_and_exact_fresh_observations() {
        let mut safe = tag("other:exact");
        safe["details"]["format"] = json!("safetensors");
        safe["digest"] = json!("A".repeat(64));
        safe.as_object_mut().unwrap().remove("model");
        let mut unsupported = tag("unknown:tag");
        unsupported["details"] = json!({"format": "unknown"});
        let mut remote = tag("remote:tag");
        remote["remote_host"] = json!("server");
        let server = Server::new(vec![
            Reply::json(
                json!({"models": [tag(MODEL), safe, unsupported, remote, tag("CLOUD:tag"), tag("no-tag")]}),
            ),
            Reply::json(json!({"models": []})),
        ]);
        let mut provider = server.provider();
        assert_eq!(provider.tags().unwrap(), vec![MODEL, "other:exact"]);
        assert!(provider.tags().unwrap().is_empty());
        assert_eq!(server.requests().len(), 2);
        assert!(server
            .requests()
            .iter()
            .all(|r| r.method == "GET" && r.path == "/api/tags" && r.body.is_none()));
    }

    #[test]
    fn tags_check_all_duplicate_names_before_eligibility_filtering() {
        for changes in [
            json!({}),
            json!({"digest": "c".repeat(64)}),
            json!({"remote_host": "server"}),
            json!({"cloud": true}),
            json!({"details": {"format": "unknown"}}),
            json!({"model": "other:tag"}),
        ] {
            let mut conflicting = tag(MODEL);
            conflicting
                .as_object_mut()
                .unwrap()
                .extend(changes.as_object().unwrap().clone());
            for entries in [
                vec![tag(MODEL), conflicting.clone()],
                vec![conflicting.clone(), tag(MODEL)],
            ] {
                let server = Server::new(vec![Reply::json(json!({"models": entries}))]);
                assert!(server.provider().tags().is_err());
                assert_eq!(server.requests().len(), 1);
            }
        }
        let server = Server::new(vec![Reply::json(
            json!({"models": [tag("cloud:tag"), tag("cloud:tag")]}),
        )]);
        assert!(server.provider().tags().is_err());
    }

    #[test]
    fn tags_conflicting_model_identity_rejected_even_if_ineligible() {
        for changes in [
            json!({"model": "other:tag"}),
            json!({"model": null}),
            json!({"model": false}),
            json!({"model": "other:tag", "remote_host": "server"}),
            json!({"model": "other:tag", "details": {"format": "unknown"}}),
        ] {
            let mut conflicting = tag(MODEL);
            conflicting
                .as_object_mut()
                .unwrap()
                .extend(changes.as_object().unwrap().clone());
            let server = Server::new(vec![Reply::json(json!({"models": [conflicting]}))]);
            assert!(server.provider().tags().is_err());
        }
    }

    #[test]
    fn tags_nonnull_typed_shapes_and_raw_count_bound() {
        for value in [
            json!(null),
            json!([]),
            json!({}),
            json!({"models": null}),
            json!({"models": {}}),
            json!({"models": [null]}),
            json!({"models": [{}]}),
            json!({"models": [{"name": null, "digest": "b"}]}),
            json!({"models": [{"name": "cloud:tag", "digest": null}]}),
        ] {
            let server = Server::new(vec![Reply::json(value)]);
            assert!(server.provider().tags().is_err());
        }
        for count in [256, 257] {
            let rows: Vec<_> = (0..count).map(|n| tag(&format!("tiny:{n}"))).collect();
            let server = Server::new(vec![Reply::json(json!({"models": rows}))]);
            let result = server.provider().tags();
            assert_eq!(result.is_ok(), count == TAG_COUNT);
            if let Ok(names) = result {
                assert_eq!(names.len(), TAG_COUNT);
            }
        }
        let mut bad = tag(MODEL);
        bad["digest"] = json!("g".repeat(64));
        let server = Server::new(vec![Reply::json(json!({"models": [bad]}))]);
        assert!(server.provider().tags().is_err());
    }

    #[test]
    fn strict_json_duplicate_keys_escaped_keys_nonfinite_and_nesting() {
        for body in [
            b"{\"models\":[],\"models\":[]}".as_slice(),
            b"{\"models\":[],\"metadata\":{\"x\":1,\"\\u0078\":2}}",
            b"{\"models\":[],\"x\":NaN}",
            b"{\"models\":[],\"x\":Infinity}",
            b"{\"models\":[],\"x\":1e999}",
            b"{\"models\":[]} null",
            b"{\"models\":[],\"x\":\"\xff\"}",
        ] {
            let server = Server::new(vec![Reply::body(body)]);
            assert!(server.provider().tags().is_err());
        }
        for depth in [64, 65] {
            let nested = format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
            assert_eq!(decode(nested.as_bytes()).is_ok(), depth == 64);
        }
        let nested = format!(
            "{{\"models\":[],\"x\":{}0{}}}",
            "[".repeat(64),
            "]".repeat(64)
        );
        let server = Server::new(vec![Reply::body(nested)]);
        assert!(server.provider().tags().is_err());
    }

    #[test]
    fn tags_body_bound_content_length_close_and_chunked() {
        let prefix = "{\"models\":[],\"padding\":\"";
        let suffix = "\"}";
        let exact = format!(
            "{prefix}{}{suffix}",
            "x".repeat(TAG_BYTES - prefix.len() - suffix.len())
        );
        let server = Server::new(vec![Reply::body(&exact)]);
        assert!(server.provider().tags().unwrap().is_empty());
        for chunked in [false, true] {
            let body = format!("{exact} ");
            let reply = if chunked {
                Reply::chunks(&[body.as_bytes()])
            } else {
                let mut bytes = b"HTTP/1.1 200 OK\r\n\r\n".to_vec();
                bytes.extend_from_slice(body.as_bytes());
                Reply::raw(bytes)
            };
            let server = Server::new(vec![reply]);
            assert!(server.provider().tags().unwrap_err().contains("body bound"));
        }
    }

    #[test]
    fn pull_bounded_chunks_fresh_tags_and_fixed_minimal_request() {
        let server = Server::new(vec![
            Reply::chunks(&[
                b"{\"status\":\"down",
                b"loading\",\"completed\":2,\"total\":3}\n",
                b"{\"status\":\"success\"}\n",
            ]),
            tags(),
        ]);
        let mut output = VectorProgress::new();
        server.provider().pull(MODEL, &mut output).unwrap();
        assert_eq!(
            String::from_utf8(output.into_bytes()).unwrap(),
            "Ollama: downloading (reported layer bytes 2/3)\nOllama: success\n"
        );
        let requests = server.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].method, "POST");
        assert_eq!(requests[0].path, "/api/pull");
        assert_eq!(
            requests[0].body,
            Some(json!({"model": MODEL, "stream": true}))
        );
        assert!(requests[0].headers.contains("Connection: close\r\n"));
        assert!(requests[0]
            .headers
            .contains(&format!("Host: {}\r\n", server.endpoint)));
        assert_eq!(requests[1].path, "/api/tags");
    }

    #[test]
    fn pull_success_requires_exact_fresh_eligible_tag() {
        let mut remote = tag(MODEL);
        remote["remote_host"] = json!("server");
        let mut unsupported = tag(MODEL);
        unsupported["details"] = json!({"format": "unknown"});
        for rows in [
            vec![tag("other:tag")],
            vec![remote.clone()],
            vec![unsupported],
            vec![tag(MODEL), remote],
        ] {
            let server = Server::new(vec![pull_reply(), Reply::json(json!({"models": rows}))]);
            assert!(server.provider().pull(MODEL, &mut DiscardProgress).is_err());
            assert_eq!(server.requests().len(), 2);
        }
    }

    #[test]
    fn ambiguous_inventory_blocks_inference_before_generate_and_postpull() {
        let mut remote = tag(MODEL);
        remote["remote_host"] = json!("server");
        let ambiguous = json!({"models": [tag(MODEL), remote]});
        let server = Server::new(vec![Reply::json(ambiguous.clone())]);
        assert!(server.provider().inference(MODEL).is_err());
        assert_eq!(server.requests().len(), 1);
        assert_eq!(server.requests()[0].path, "/api/tags");
        let server = Server::new(vec![pull_reply(), Reply::json(ambiguous)]);
        assert!(server.provider().pull(MODEL, &mut DiscardProgress).is_err());
        assert_eq!(server.requests().len(), 2);
    }

    #[test]
    fn late_fresh_verification_never_uses_prior_success_or_cached_tags() {
        let server = Server::new(vec![
            tags(),
            pull_reply(),
            Reply::json(json!({"models": []})),
        ]);
        let mut provider = server.provider();
        assert_eq!(provider.tags().unwrap(), vec![MODEL]);
        assert!(provider
            .pull(MODEL, &mut DiscardProgress)
            .unwrap_err()
            .contains("freshly verify"));
        assert_eq!(server.requests().len(), 3);
        let server = Server::new(vec![
            pull_reply(),
            tags(),
            Reply::json(json!({"models": []})),
        ]);
        let mut provider = server.provider();
        provider.pull(MODEL, &mut DiscardProgress).unwrap();
        assert!(provider.inference(MODEL).is_err());
        assert_eq!(
            server
                .requests()
                .iter()
                .map(|r| r.path.as_str())
                .collect::<Vec<_>>(),
            vec!["/api/pull", "/api/tags", "/api/tags"]
        );
    }

    #[test]
    fn redirect_error_eof_oversize_and_invalid_progress_no_retry() {
        let mut replies = vec![
            Reply::raw(
                b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:9/\r\nContent-Length: 0\r\n\r\n",
            ),
            Reply::body(b""),
            Reply::body(b"{\"status\":\"success\"}"),
            Reply::body(b"\n"),
            Reply::body(vec![b'x'; FRAME_BYTES + 1]),
            Reply::body(b"{\"status\":\"success\"}\n{\"status\":\"extra\"}\n"),
        ];
        for value in [
            json!(null),
            json!([]),
            json!({}),
            json!({"status": null}),
            json!({"status": true}),
            json!({"status": "x".repeat(1025)}),
            json!({"status": "download", "error": null}),
            json!({"status": "download", "total": -1}),
            json!({"status": "download", "total": 1.0}),
            json!({"status": "download", "completed": false}),
            json!({"status": "download", "completed": null}),
        ] {
            replies.push(Reply::body(format!("{value}\n")));
        }
        replies.push(Reply::body(
            b"{\"status\":\"download\",\"total\":18446744073709551616}\n",
        ));
        replies.push(Reply::body(
            b"{\"status\":\"success\",\"status\":\"success\"}\n",
        ));
        replies.push(Reply::body(
            b"{\"status\":\"download\",\"metadata\":{\"x\":1,\"x\":2}}\n",
        ));
        replies.push(Reply::body(format!(
            "{{\"status\":\"download\",\"x\":{}0{}}}\n",
            "[".repeat(64),
            "]".repeat(64)
        )));
        for reply in replies {
            let server = Server::new(vec![reply]);
            assert!(server.provider().pull(MODEL, &mut DiscardProgress).is_err());
            assert_eq!(server.requests().len(), 1);
        }
        for status in [301, 307, 404, 500] {
            let server = Server::new(vec![Reply::raw(format!(
                "HTTP/1.1 {status} Untrusted\r\nContent-Length: 0\r\n\r\n"
            ))]);
            assert!(server.provider().tags().is_err());
            assert_eq!(server.requests().len(), 1);
        }
    }

    #[test]
    fn pull_exact_frame_total_and_frame_count_bounds() {
        let prefix = "{\"status\":\"work\",\"padding\":\"";
        let suffix = "\"}";
        let exact = format!(
            "{prefix}{}{suffix}",
            "x".repeat(FRAME_BYTES - prefix.len() - suffix.len())
        );
        let server = Server::new(vec![
            Reply::body(format!("{exact}\n{{\"status\":\"success\"}}\n")),
            tags(),
        ]);
        server.provider().pull(MODEL, &mut DiscardProgress).unwrap();
        let oversized = format!(
            "{prefix}{}{suffix}\n",
            "x".repeat(FRAME_BYTES + 1 - prefix.len() - suffix.len())
        );
        let server = Server::new(vec![Reply::body(oversized)]);
        assert!(server
            .provider()
            .pull(MODEL, &mut DiscardProgress)
            .unwrap_err()
            .contains("bound exceeded"));
        for count in [PROGRESS_FRAMES - 1, PROGRESS_FRAMES] {
            let body = format!(
                "{}{{\"status\":\"success\"}}\n",
                "{\"status\":\"work\"}\n".repeat(count)
            );
            let mut replies = vec![Reply::body(body)];
            if count < PROGRESS_FRAMES {
                replies.push(tags());
            }
            let server = Server::new(replies);
            let result = server.provider().pull(MODEL, &mut DiscardProgress);
            assert_eq!(result.is_ok(), count < PROGRESS_FRAMES);
        }
        let frame = format!(
            "{prefix}{}{suffix}\n",
            "x".repeat(FRAME_BYTES - 1 - prefix.len() - suffix.len())
        );
        assert_eq!(frame.len(), FRAME_BYTES);
        let mut body = frame.repeat(PROGRESS_BYTES / FRAME_BYTES);
        assert_eq!(body.len(), PROGRESS_BYTES);
        body.push_str("{\"status\":\"success\"}\n");
        let server = Server::new(vec![Reply::body(body)]);
        assert!(server
            .provider()
            .pull(MODEL, &mut DiscardProgress)
            .unwrap_err()
            .contains("total progress bound"));
    }

    #[test]
    fn terminal_text_sanitized_and_only_reported_layer_counts() {
        let status = "work\u{1b}]title\u{7}\u{202e}\u{2066}\u{200f}\u{61c}\n";
        let body = format!(
            "{}\n{{\"status\":\"success\"}}\n",
            json!({"status": status, "completed": u64::MAX, "total": 0})
        );
        let server = Server::new(vec![Reply::body(body), tags()]);
        let mut output = VectorProgress::new();
        server.provider().pull(MODEL, &mut output).unwrap();
        let output = String::from_utf8(output.into_bytes()).unwrap();
        assert_eq!(
            output,
            "Ollama: work]title (reported layer bytes 18446744073709551615/0)\nOllama: success\n"
        );
        assert!(!output.contains('%'));
        let mut result = generated();
        result["response"] = json!("O\u{202e}\u{2069}\u{9b}\u{1b}\nK");
        let server = Server::new(vec![tags(), Reply::json(result)]);
        assert_eq!(server.provider().inference(MODEL).unwrap(), "OK");
    }

    #[test]
    fn inference_fixed_minimal_no_tools_no_persisted_chat() {
        let server = Server::new(vec![tags(), Reply::json(generated())]);
        assert_eq!(server.provider().inference(MODEL).unwrap(), "OK");
        let requests = server.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].path, "/api/tags");
        assert_eq!(requests[1].method, "POST");
        assert_eq!(requests[1].path, "/api/generate");
        assert_eq!(
            requests[1].body,
            Some(
                json!({"model": MODEL, "prompt": "Reply with OK.", "stream": false, "options": {"num_predict": 8, "temperature": 0}, "keep_alive": 0})
            )
        );
    }

    #[test]
    fn inference_rejects_remote_metadata_but_not_generated_text() {
        for changes in [
            json!({"remote_host": "server"}),
            json!({"cloud": true}),
            json!({"details": {"remote_host": "server"}}),
            json!({"metadata": [{"cloud": true}]}),
            json!({"remote_host": 0}),
            json!({"cloud": {}}),
            json!({"cloud": []}),
            json!({"metadata": "https://remote.example"}),
            json!({"metadata": "remote"}),
            json!({"details": {"response": "https://remote.example"}}),
        ] {
            let mut result = generated();
            result
                .as_object_mut()
                .unwrap()
                .extend(changes.as_object().unwrap().clone());
            let server = Server::new(vec![tags(), Reply::json(result)]);
            assert!(server.provider().inference(MODEL).is_err());
            assert_eq!(server.requests().len(), 2);
        }
        for text in [
            "cloud",
            "remote",
            "http://example.test",
            "https://example.test",
        ] {
            let result = json!({"model": MODEL, "done": true, "response": text, "cloud": false, "remote_host": null, "remote": ""});
            let server = Server::new(vec![tags(), Reply::json(result)]);
            assert_eq!(server.provider().inference(MODEL).unwrap(), text);
        }
    }

    #[test]
    fn inference_tool_calls_absent_null_or_empty_list_only() {
        for tools in [
            json!({}),
            json!(false),
            json!(0),
            json!(""),
            json!([{}]),
            json!({"name": "tool"}),
            json!(null),
            json!([]),
        ] {
            let valid = tools.is_null() || tools.as_array().is_some_and(Vec::is_empty);
            let mut result = generated();
            result["tool_calls"] = tools;
            let server = Server::new(vec![tags(), Reply::json(result)]);
            assert_eq!(server.provider().inference(MODEL).is_ok(), valid);
        }
    }

    #[test]
    fn inference_strict_model_done_error_nonblank_and_utf8_text_bounds() {
        for changes in [
            json!({"model": "other:tag"}),
            json!({"model": null}),
            json!({"done": false}),
            json!({"done": "true"}),
            json!({"done": 1}),
            json!({"error": null}),
            json!({"response": null}),
            json!({"response": ""}),
            json!({"response": " \n\t"}),
            json!({"response": "\u{202e}\u{2069}"}),
            json!({"response": "x".repeat(TEXT_BYTES + 1)}),
            json!({"response": "🦀".repeat(1025)}),
        ] {
            let mut result = generated();
            result
                .as_object_mut()
                .unwrap()
                .extend(changes.as_object().unwrap().clone());
            let server = Server::new(vec![tags(), Reply::json(result)]);
            assert!(server.provider().inference(MODEL).is_err());
        }
        for field in ["model", "done", "response"] {
            let mut result = generated();
            result.as_object_mut().unwrap().remove(field);
            let server = Server::new(vec![tags(), Reply::json(result)]);
            assert!(server.provider().inference(MODEL).is_err());
        }
        for text in ["x".repeat(TEXT_BYTES), "🦀".repeat(1024)] {
            let mut result = generated();
            result["response"] = json!(text);
            let server = Server::new(vec![tags(), Reply::json(result)]);
            assert_eq!(server.provider().inference(MODEL).unwrap(), text);
        }
        for invalid in [json!(null), json!([]), json!(false)] {
            let server = Server::new(vec![tags(), Reply::json(invalid)]);
            assert!(server.provider().inference(MODEL).is_err());
        }
    }

    #[test]
    fn inference_body_and_json_bounds() {
        let prefix =
            format!("{{\"model\":\"{MODEL}\",\"done\":true,\"response\":\"OK\",\"padding\":\"");
        let suffix = "\"}";
        let body = format!(
            "{prefix}{}{suffix}",
            "x".repeat(INFERENCE_BYTES - prefix.len() - suffix.len())
        );
        let server = Server::new(vec![tags(), Reply::body(&body)]);
        assert_eq!(server.provider().inference(MODEL).unwrap(), "OK");
        for reply in [Reply::body(format!("{body} ")), Reply::chunks(&[format!("{body} ").as_bytes()]), Reply::body(format!("{{\"model\":\"{MODEL}\",\"done\":true,\"response\":\"OK\",\"metadata\":{{\"x\":1,\"x\":2}}}}")), Reply::body(format!("{{\"model\":\"{MODEL}\",\"done\":true,\"response\":\"OK\",\"x\":{}0{}}}", "[".repeat(64), "]".repeat(64)))] {
            let server = Server::new(vec![tags(), reply]);
            assert!(server.provider().inference(MODEL).is_err());
            assert_eq!(server.requests().len(), 2);
        }
    }

    #[test]
    fn http_framing_ambiguity_truncation_and_encoding_fail_closed() {
        for bytes in [
            b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\nContent-Length: 13\r\n\r\n{\"models\":[]}".as_slice(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\nTransfer-Encoding: chunked\r\n\r\n",
            b"HTTP/1.1 200 OK\r\nContent-Length: -1\r\n\r\n",
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip, chunked\r\n\r\n",
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nTransfer-Encoding: chunked\r\n\r\n",
            b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\n\r\n",
            b"HTTP/1.1 200 OK\r\n bad: folded\r\n\r\n",
            b"HTTP/1.1 200 OK\n\n{\"models\":[]}",
            b"HTTP/1.1 200 OK\r\nContent-Length: 99\r\n\r\n{\"models\":[]}",
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nz\r\n",
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nffffffffffffffffffffffff\r\n",
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nD\r\n{\"models\":[]}",
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nD\r\n{\"models\":[]}xx0\r\n\r\n",
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nD\r\n{\"models\":[]}\r\n0\r\ninvalid-trailer\r\n\r\n",
        ] {
            let server = Server::new(vec![Reply::raw(bytes)]);
            assert!(server.provider().tags().is_err());
            assert_eq!(server.requests().len(), 1);
        }
        let server = Server::new(vec![Reply::raw(format!(
            "HTTP/1.1 200 OK\r\nX-Padding: {}\r\n\r\n",
            "x".repeat(HEADER_BYTES)
        ))]);
        assert!(server
            .provider()
            .tags()
            .unwrap_err()
            .contains("bound exceeded"));
        let server = Server::new(vec![Reply::raw(format!(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nD;{}\r\n",
            "x".repeat(1024)
        ))]);
        assert!(server
            .provider()
            .tags()
            .unwrap_err()
            .contains("bound exceeded"));
    }

    #[test]
    fn chunked_json_split_at_every_byte_including_utf8() {
        let mut result = generated();
        result["response"] = json!("OK 🦀");
        let encoded = result.to_string().into_bytes();
        let chunks: Vec<&[u8]> = encoded.chunks(1).collect();
        let server = Server::new(vec![tags(), Reply::chunks(&chunks)]);
        assert_eq!(server.provider().inference(MODEL).unwrap(), "OK 🦀");
    }

    #[test]
    fn overall_deadline_bounds_drip_headers_chunk_lines_bodies_and_trailers() {
        let chunked = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n";
        let cases = [
            (
                Vec::new(),
                b"HTTP/1.1 200 OK\r\nX-Padding: drip-drip-drip-drip\r\n\r\n".to_vec(),
            ),
            (
                chunked.to_vec(),
                format!("D;{}\r\n", "x".repeat(100)).into_bytes(),
            ),
            (
                b"HTTP/1.1 200 OK\r\nContent-Length: 5000\r\n\r\n".to_vec(),
                vec![b' '; 100],
            ),
            (
                [
                    chunked.as_slice(),
                    b"D\r\n{\"models\":[]}\r\n0\r\nX-Trailer: ",
                ]
                .concat(),
                vec![b'x'; 100],
            ),
        ];
        for (prefix, drip) in cases {
            let server = Server::new(vec![Reply(vec![
                Step::Bytes(prefix),
                Step::Drip(drip, Duration::from_millis(20)),
            ])]);
            let mut provider = server.provider();
            provider.timing = quick();
            let started = Instant::now();
            assert!(provider.tags().unwrap_err().contains("overall deadline"));
            assert!(started.elapsed() < Duration::from_secs(2));
            assert_eq!(server.requests().len(), 1);
        }
        let server = Server::new(vec![Reply(vec![
            Step::Bytes(chunked.to_vec()),
            Step::Drip(
                b"2000\r\n{\"status\":\"download\",\"padding\":\"xxxxxxxxxxxxxxxxxxxxxxxx".to_vec(),
                Duration::from_millis(20),
            ),
        ])]);
        let mut provider = server.provider();
        provider.timing = quick();
        assert!(provider
            .pull(MODEL, &mut DiscardProgress)
            .unwrap_err()
            .contains("overall deadline"));
        assert_eq!(server.requests().len(), 1);
        let server = Server::new(vec![
            tags(),
            Reply(vec![Step::Drip(
                b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n".to_vec(),
                Duration::from_millis(20),
            )]),
        ]);
        let mut provider = server.provider();
        provider.timing = quick();
        assert!(provider
            .inference(MODEL)
            .unwrap_err()
            .contains("overall deadline"));
        assert_eq!(server.requests().len(), 2);
    }

    #[test]
    fn socket_idle_deadline_is_not_reset_by_polling_and_closes_owned_stream() {
        let closed = Arc::new(AtomicBool::new(false));
        let server = Server::new(vec![Reply(vec![Step::Closed(closed.clone())])]);
        let mut provider = server.provider();
        provider.timing.tags_idle = Duration::from_millis(60);
        let started = Instant::now();
        assert!(provider.tags().unwrap_err().contains("idle timeout"));
        assert!(started.elapsed() < Duration::from_secs(2));
        let requests = server.wait_observed(1, &closed);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "GET");
        assert_eq!(requests[0].path, "/api/tags");
        assert!(requests[0].body.is_none());
        assert!(closed.load(Ordering::SeqCst));
        drop(server);
    }

    #[test]
    fn idle_timeout_precedes_delayed_observer_receipt_and_closure() {
        let closed = Arc::new(AtomicBool::new(false));
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let observer_gate = gate.clone();
        let server = Server::with_request_observer(
            vec![Reply(vec![Step::Closed(closed.clone())])],
            move || {
                let (released, changed) = &*observer_gate;
                let (released, _) = changed
                    .wait_timeout_while(
                        released.lock().unwrap(),
                        Duration::from_secs(2),
                        |released| !*released,
                    )
                    .unwrap();
                assert!(*released, "test did not release the delayed observer");
                drop(released);
                // Deliberate observer fault injection, not a sleep to make assertions pass.
                // The client must finish before this callback is allowed to record receipt.
                std::thread::sleep(Duration::from_millis(120));
            },
        );
        let mut provider = server.provider();
        provider.timing.tags_idle = Duration::from_millis(60);
        let started = Instant::now();
        let result = provider.tags();
        let elapsed = started.elapsed();
        let before_receipt = server.requests();
        let before_closed = closed.load(Ordering::SeqCst);
        // Release before assertions so a failed client assertion cannot leave the
        // observer parked while fixture Drop tries to join it.
        let (released, changed) = &*gate;
        *released.lock().unwrap() = true;
        changed.notify_all();
        assert!(result.unwrap_err().contains("idle timeout"));
        assert!(elapsed < Duration::from_secs(2));
        assert!(before_receipt.is_empty());
        assert!(!before_closed);
        let requests = server.wait_observed(1, &closed);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "GET");
        assert_eq!(requests[0].path, "/api/tags");
        assert!(requests[0].body.is_none());
        assert!(closed.load(Ordering::SeqCst));
        drop(server);
    }

    #[test]
    fn cancellation_at_operation_boundary_has_no_request_or_retry() {
        let _reset = ResetCancel;
        CANCEL.set(true);
        let server = Server::new(vec![tags()]);
        let mut provider = server.provider();
        for result in [
            provider.tags().map(|_| ()),
            provider.pull(MODEL, &mut DiscardProgress),
            provider.inference(MODEL).map(|_| ()),
        ] {
            let error = result.unwrap_err();
            assert!(error.contains("cancelled"));
            assert!(error.contains("outcome unknown"));
            assert!(error.contains("no cache was deleted"));
        }
        assert!(server.requests().is_empty());
    }

    #[test]
    fn cancellation_interrupts_raw_header_chunk_framing_and_body_waits() {
        let _reset = ResetCancel;
        for prefix in [
            b"HTTP/1.1 200".as_slice(),
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n100;drip",
            b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n{",
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\n{\r",
        ] {
            let closed = Arc::new(AtomicBool::new(false));
            let server = Server::new(vec![Reply(vec![
                Step::Bytes(prefix.to_vec()),
                Step::Closed(closed.clone()),
            ])]);
            let mut provider = server.provider();
            CANCEL_AT.set(Some(Instant::now() + Duration::from_millis(50)));
            let started = Instant::now();
            assert!(provider
                .pull(MODEL, &mut DiscardProgress)
                .unwrap_err()
                .contains("cancelled"));
            assert!(started.elapsed() < Duration::from_secs(2));
            assert_eq!(server.requests().len(), 1);
            CANCEL_AT.set(None);
            drop(server);
            assert!(closed.load(Ordering::SeqCst));
        }
        let closed = Arc::new(AtomicBool::new(false));
        let server = Server::new(vec![
            tags(),
            Reply(vec![
                Step::Bytes(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n".to_vec()),
                Step::Closed(closed.clone()),
            ]),
        ]);
        CANCEL_AT.set(Some(Instant::now() + Duration::from_millis(80)));
        assert!(server
            .provider()
            .inference(MODEL)
            .unwrap_err()
            .contains("cancelled"));
        assert_eq!(server.requests().len(), 2);
        drop(server);
        assert!(closed.load(Ordering::SeqCst));
    }

    struct CancelOutput;
    impl ProgressOutput for CancelOutput {
        fn write_until(
            &mut self,
            bytes: &[u8],
            deadline: Instant,
            cancelled: fn() -> bool,
        ) -> Result<()> {
            progress_message(bytes, deadline, cancelled)?;
            CANCEL.set(true);
            progress_remaining(deadline, cancelled)?;
            Ok(())
        }
    }
    struct BrokenOutput;
    impl ProgressOutput for BrokenOutput {
        fn write_until(
            &mut self,
            bytes: &[u8],
            deadline: Instant,
            cancelled: fn() -> bool,
        ) -> Result<()> {
            progress_message(bytes, deadline, cancelled)?;
            Err("Unable to write Ollama progress: fixture failure".into())
        }
    }

    #[test]
    fn cancellation_after_reported_success_prevents_fresh_verification() {
        let _reset = ResetCancel;
        let server = Server::new(vec![pull_reply()]);
        let error = server
            .provider()
            .pull(MODEL, &mut CancelOutput)
            .unwrap_err();
        assert!(error.contains("cancelled"));
        assert!(error.contains("external daemon work and partial cache may continue"));
        assert_eq!(server.requests().len(), 1);
    }

    #[test]
    fn output_failure_stops_without_postpull_request_or_false_success() {
        let server = Server::new(vec![pull_reply()]);
        let error = server
            .provider()
            .pull(MODEL, &mut BrokenOutput)
            .unwrap_err();
        assert!(error.contains("Unable to write"));
        assert!(error.contains("outcome unknown"));
        assert_eq!(server.requests().len(), 1);
    }

    #[test]
    fn invalid_exact_model_never_connects_and_redirect_target_is_never_contacted() {
        let server = Server::new(vec![tags()]);
        let mut provider = server.provider();
        for model in ["tiny", "cloud:tag", "http://example.test:tag"] {
            assert!(provider.pull(model, &mut DiscardProgress).is_err());
            assert!(provider.inference(model).is_err());
        }
        assert!(server.requests().is_empty());
        let trap = Server::new(vec![tags()]);
        let server = Server::new(vec![Reply::raw(format!("HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{}/api/tags\r\nContent-Length: 0\r\n\r\n", trap.endpoint))]);
        assert!(server.provider().tags().is_err());
        assert_eq!(server.requests().len(), 1);
        assert!(trap.requests().is_empty());
    }

    #[test]
    fn polling_does_not_shorten_idle_budget_or_retry_request() {
        let body = b"{\"status\":\"work\"}\n{\"status\":\"success\"}\n";
        let headers = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
        let server = Server::new(vec![
            Reply(vec![
                Step::Bytes(headers.into_bytes()),
                Step::Drip(body[..1].to_vec(), Duration::from_millis(160)),
                Step::Bytes(body[1..].to_vec()),
            ]),
            tags(),
        ]);
        let mut provider = server.provider();
        provider.timing.pull = Duration::from_secs(2);
        provider.timing.idle = Duration::from_millis(600);
        provider.pull(MODEL, &mut DiscardProgress).unwrap();
        assert_eq!(server.requests().len(), 2);
    }

    #[test]
    fn cancellation_during_postpull_fresh_tags_never_reports_verified_success() {
        let _reset = ResetCancel;
        let closed = Arc::new(AtomicBool::new(false));
        let server = Server::new(vec![
            pull_reply(),
            Reply(vec![
                Step::Bytes(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n".to_vec()),
                Step::Closed(closed.clone()),
            ]),
        ]);
        CANCEL_AT.set(Some(Instant::now() + Duration::from_millis(80)));
        let error = server
            .provider()
            .pull(MODEL, &mut DiscardProgress)
            .unwrap_err();
        assert!(error.contains("cancelled"));
        assert!(error.contains("outcome unknown"));
        assert_eq!(server.requests().len(), 2);
        drop(server);
        assert!(closed.load(Ordering::SeqCst));
    }

    struct BackpressuredOutput;
    impl ProgressOutput for BackpressuredOutput {
        fn write_until(
            &mut self,
            bytes: &[u8],
            deadline: Instant,
            cancelled: fn() -> bool,
        ) -> Result<()> {
            progress_message(bytes, deadline, cancelled)?;
            loop {
                thread::sleep(progress_remaining(deadline, cancelled)?.min(POLL));
            }
        }
    }
    #[test]
    fn progress_sink_deadline_never_permits_fresh_verification() {
        let server = Server::new(vec![pull_reply()]);
        let mut provider = server.provider();
        provider.timing = quick();
        assert!(provider
            .pull(MODEL, &mut BackpressuredOutput)
            .unwrap_err()
            .contains("overall deadline"));
        assert_eq!(server.requests().len(), 1);
    }

    #[test]
    fn tags_remote_metadata_is_recursive_and_false_is_not_numeric_zero() {
        for value in [json!(0), json!({}), json!([]), json!(true), json!("server")] {
            let mut row = tag(MODEL);
            row["details"]["remote_host"] = value;
            let server = Server::new(vec![Reply::json(json!({"models": [row]}))]);
            assert!(server.provider().tags().unwrap().is_empty());
        }
        let mut row = tag(MODEL);
        row["metadata"] = json!([{"value": "https://remote.example"}]);
        let server = Server::new(vec![Reply::json(json!({"models": [row]}))]);
        assert!(server.provider().tags().unwrap().is_empty());
        let mut row = tag(MODEL);
        row["cloud"] = json!(false);
        row["remote_host"] = Value::Null;
        let server = Server::new(vec![Reply::json(json!({"models": [row]}))]);
        assert_eq!(server.provider().tags().unwrap(), vec![MODEL]);
    }

    #[test]
    fn pull_and_inference_idle_timeouts_close_stream_without_retry() {
        for inference in [false, true] {
            let closed = Arc::new(AtomicBool::new(false));
            let waiting = Reply(vec![
                Step::Bytes(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n".to_vec()),
                Step::Closed(closed.clone()),
            ]);
            let replies = if inference {
                vec![tags(), waiting]
            } else {
                vec![waiting]
            };
            let server = Server::new(replies);
            let mut provider = server.provider();
            provider.timing.idle = Duration::from_millis(60);
            let error = if inference {
                provider.inference(MODEL).unwrap_err()
            } else {
                provider.pull(MODEL, &mut DiscardProgress).unwrap_err()
            };
            assert!(error.contains("idle timeout"));
            assert_eq!(server.requests().len(), if inference { 2 } else { 1 });
            drop(server);
            assert!(closed.load(Ordering::SeqCst));
        }
    }

    fn output_pipe() -> (OwnedFd, OwnedFd) {
        let mut fds = [-1; 2];
        // SAFETY: pipe2 initializes both entries on success; each becomes singly owned.
        assert_eq!(unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) }, 0);
        let read = unsafe { OwnedFd::from_raw_fd(fds[0]) };
        let write = unsafe { OwnedFd::from_raw_fd(fds[1]) };
        // Shrink this fixture only: a small valid HTTP stream can fill the unread pipe
        // without the fake daemon itself blocking on a large TCP write.
        assert_eq!(
            unsafe { libc::fcntl(write.as_raw_fd(), libc::F_SETPIPE_SZ, 4096) },
            4096
        );
        (read, write)
    }
    fn fill_pipe(fd: BorrowedFd<'_>) {
        let raw = fd.as_raw_fd();
        // SAFETY: the descriptor stays borrowed/open. All writes use an initialized
        // stack buffer and nonblocking mode; the fixture never drains the read end.
        let flags = unsafe { libc::fcntl(raw, libc::F_GETFL) };
        assert!(flags >= 0);
        assert_eq!(
            unsafe { libc::fcntl(raw, libc::F_SETFL, flags | libc::O_NONBLOCK) },
            0
        );
        let bytes = [b'x'; 4096];
        let mut total = 0;
        loop {
            let count = unsafe { libc::write(raw, bytes.as_ptr().cast(), bytes.len()) };
            if count > 0 {
                total += count;
                assert!(total <= 1024 * 1024);
                continue;
            }
            assert!(count < 0);
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
            break;
        }
        assert_eq!(unsafe { libc::fcntl(raw, libc::F_SETFL, flags) }, 0);
    }
    fn pressure_reply(closed: Arc<AtomicBool>) -> Reply {
        let body = format!(
            "{}{{\"status\":\"success\"}}\n",
            format!("{}\n", json!({"status": "x".repeat(1024)})).repeat(32)
        );
        let mut reply = Reply::body(body);
        reply.0.push(Step::Closed(closed));
        reply
    }

    #[test]
    fn fd_progress_delivery_and_restoration_never_uses_buffered_stdio() {
        let (read, write) = output_pipe();
        let before = unsafe { libc::fcntl(write.as_raw_fd(), libc::F_GETFL) };
        let mut output = FdProgress::from_fd(write.as_fd()).unwrap();
        assert_ne!(
            unsafe { libc::fcntl(write.as_raw_fd(), libc::F_GETFL) } & libc::O_NONBLOCK,
            0
        );
        output
            .write_until(
                b"progress\n",
                Instant::now() + Duration::from_secs(1),
                cancelled,
            )
            .unwrap();
        output.finish().unwrap();
        assert_eq!(
            unsafe { libc::fcntl(write.as_raw_fd(), libc::F_GETFL) },
            before
        );
        let mut bytes = [0; 9];
        assert_eq!(
            unsafe { libc::read(read.as_raw_fd(), bytes.as_mut_ptr().cast(), bytes.len()) },
            9
        );
        assert_eq!(&bytes, b"progress\n");
        {
            let _output = FdProgress::from_fd(write.as_fd()).unwrap();
        }
        assert_eq!(
            unsafe { libc::fcntl(write.as_raw_fd(), libc::F_GETFL) },
            before
        );
        assert!(FdProgress::from_fd(read.as_fd()).is_err());
    }

    #[test]
    fn fd_progress_closed_reader_returns_error_without_retry() {
        let (read, write) = output_pipe();
        drop(read);
        let mut output = FdProgress::from_fd(write.as_fd()).unwrap();
        assert!(output
            .write_until(
                b"progress\n",
                Instant::now() + Duration::from_secs(1),
                cancelled
            )
            .unwrap_err()
            .contains("output failure"));
        output.finish().unwrap();
    }

    #[test]
    fn full_and_unread_pipe_progress_deadline_and_cancel_close_http_without_inventory() {
        let _reset = ResetCancel;
        for full in [false, true] {
            for cancel in [false, true] {
                let (read, write) = output_pipe();
                if full {
                    fill_pipe(write.as_fd());
                }
                let before = unsafe { libc::fcntl(write.as_raw_fd(), libc::F_GETFL) };
                let mut output = FdProgress::from_fd(write.as_fd()).unwrap();
                let closed = Arc::new(AtomicBool::new(false));
                let server = Server::new(vec![pressure_reply(closed.clone()), tags()]);
                let mut provider = server.provider();
                provider.timing.pull = Duration::from_millis(200);
                if cancel {
                    CANCEL_AT.set(Some(Instant::now() + Duration::from_millis(70)));
                }
                let started = Instant::now();
                let error = provider.pull(MODEL, &mut output).unwrap_err();
                assert!(error.contains(if cancel { "cancelled" } else { "deadline" }));
                assert!(error.contains("during progress output"));
                assert!(error.contains("outcome unknown"));
                assert!(started.elapsed() < Duration::from_millis(700));
                output.finish().unwrap();
                assert_eq!(
                    unsafe { libc::fcntl(write.as_raw_fd(), libc::F_GETFL) },
                    before
                );
                CANCEL_AT.set(None);
                assert_eq!(server.requests().len(), 1);
                assert_eq!(server.requests()[0].path, "/api/pull");
                drop(server);
                assert!(closed.load(Ordering::SeqCst));
                drop(read);
            }
        }
    }

    thread_local! {
        static PROBE_BLOCKED: Cell<bool> = const { Cell::new(false) };
    }
    fn probe_backpressure() {
        if !PROBE_BLOCKED.replace(true) {
            eprintln!("MODEL_PROGRESS_BLOCKED");
        }
    }
    // Invoked only by the subprocess tests below; no production endpoint override exists.
    #[test]
    #[ignore = "controlled signal-probe child only"]
    fn progress_output_probe_child() {
        if std::env::var("SMITH_TEST_MODEL_PROGRESS_CHILD").as_deref() != Ok("1") {
            return;
        }
        let result = (|| -> Result<()> {
            crate::consent::install_cancellation()?;
            let endpoint: SocketAddr = std::env::var("SMITH_TEST_MODEL_ENDPOINT")
                .map_err(|_| "missing test endpoint")?
                .parse()
                .map_err(|_| "invalid test endpoint")?;
            if endpoint.ip() != Ipv4Addr::LOCALHOST || endpoint.port() == 11434 {
                return Err("invalid reserved test endpoint".into());
            }
            let mode =
                std::env::var("SMITH_TEST_MODEL_PROGRESS_MODE").map_err(|_| "missing test mode")?;
            let mut provider = Ollama {
                endpoint,
                timing: Timing::default(),
                cancelled: crate::consent::cancelled,
            };
            provider.timing.pull = if mode == "deadline" {
                Duration::from_millis(200)
            } else {
                Duration::from_secs(5)
            };
            let mut output = FdProgress::stdout()?;
            output.on_backpressure = Some(probe_backpressure);
            eprintln!("MODEL_PROGRESS_READY");
            let mut start = [0];
            io::stdin()
                .read_exact(&mut start)
                .map_err(|_| "missing test start")?;
            if start != *b"G" {
                return Err("invalid test start".into());
            }
            let observed = provider.pull(MODEL, &mut output);
            output.finish()?;
            let error = observed.err().ok_or("unexpected pull success")?;
            let expected = if mode == "deadline" {
                "deadline"
            } else {
                "cancelled"
            };
            if !error.contains(expected)
                || !error.contains("during progress output")
                || !error.contains("outcome unknown")
            {
                return Err(format!("unexpected bounded-progress result: {error}"));
            }
            eprintln!("MODEL_PROGRESS_RESULT: {expected}");
            Ok(())
        })();
        if let Err(error) = &result {
            eprintln!("MODEL_PROGRESS_FAILURE: {error}");
        }
        // The test harness's final stdout report would itself block on the intentionally
        // unread pipe. Exit only this controlled child, AFTER provider return/socket Drop
        // and explicit output flag restoration; never bypass provider cancellation.
        std::process::exit(if result.is_ok() { 0 } else { 1 });
    }

    struct ProbeChild(std::process::Child);
    impl Drop for ProbeChild {
        fn drop(&mut self) {
            if !matches!(self.0.try_wait(), Ok(Some(_))) {
                // Failure-only fixture cleanup; passing regressions must exit without KILL.
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }
    fn probe_line(stderr: &mut std::process::ChildStderr, deadline: Instant) -> String {
        let raw = stderr.as_raw_fd();
        let flags = unsafe { libc::fcntl(raw, libc::F_GETFL) };
        assert_eq!(
            unsafe { libc::fcntl(raw, libc::F_SETFL, flags | libc::O_NONBLOCK) },
            0
        );
        let mut bytes = Vec::new();
        loop {
            assert!(Instant::now() < deadline, "signal probe did not respond");
            let mut poll = libc::pollfd {
                fd: raw,
                events: libc::POLLIN,
                revents: 0,
            };
            let ready = unsafe { libc::poll(&mut poll, 1, 20) };
            if ready <= 0 {
                continue;
            }
            let mut byte = [0];
            match stderr.read(&mut byte) {
                Ok(1) => {
                    bytes.push(byte[0]);
                    assert!(bytes.len() <= 2048);
                    if byte[0] == b'\n' {
                        return String::from_utf8(bytes).unwrap();
                    }
                }
                Err(e) if transient(&e) => continue,
                other => panic!("signal probe output ended: {other:?}"),
            }
        }
    }
    fn run_output_probe(signal: Option<libc::c_int>, full: bool) {
        use std::process::{Command, Stdio};
        let (read, write) = output_pipe();
        let closed = Arc::new(AtomicBool::new(false));
        let server = Server::new(vec![pressure_reply(closed.clone()), tags()]);
        let mut child = ProbeChild(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "model::tests::progress_output_probe_child",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env("SMITH_TEST_MODEL_PROGRESS_CHILD", "1")
                .env("SMITH_TEST_MODEL_ENDPOINT", server.endpoint.to_string())
                .env(
                    "SMITH_TEST_MODEL_PROGRESS_MODE",
                    if signal.is_some() {
                        "signal"
                    } else {
                        "deadline"
                    },
                )
                .stdin(Stdio::piped())
                .stdout(Stdio::from(write.try_clone().unwrap()))
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let limit = Instant::now() + Duration::from_secs(4);
        assert_eq!(
            probe_line(child.0.stderr.as_mut().unwrap(), limit),
            "MODEL_PROGRESS_READY\n"
        );
        // Harness startup precedes filling; no stdout reader is ever started/drained.
        if full {
            fill_pipe(write.as_fd());
        }
        child.0.stdin.take().unwrap().write_all(b"G").unwrap();
        assert_eq!(
            probe_line(child.0.stderr.as_mut().unwrap(), limit),
            "MODEL_PROGRESS_BLOCKED\n"
        );
        let started = Instant::now();
        if let Some(signal) = signal {
            // SAFETY: this PID is our captured, live child, not a host manager/group.
            assert_eq!(
                unsafe { libc::kill(child.0.id() as libc::pid_t, signal) },
                0
            );
        }
        let expected = if signal.is_some() {
            "cancelled"
        } else {
            "deadline"
        };
        assert_eq!(
            probe_line(child.0.stderr.as_mut().unwrap(), limit),
            format!("MODEL_PROGRESS_RESULT: {expected}\n")
        );
        let status = loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < limit, "bounded progress required KILL");
            thread::sleep(Duration::from_millis(5));
        };
        assert!(status.success(), "controlled progress child: {status}");
        assert!(started.elapsed() < Duration::from_millis(700));
        assert_eq!(server.requests().len(), 1);
        assert_eq!(server.requests()[0].path, "/api/pull");
        drop(server);
        assert!(closed.load(Ordering::SeqCst));
        drop(read);
    }
    #[test]
    fn full_pipe_progress_deadline_exits_child_without_kill_or_inventory() {
        run_output_probe(None, true);
    }
    #[test]
    fn sigint_full_pipe_progress_closes_http_without_kill_or_inventory() {
        run_output_probe(Some(libc::SIGINT), true);
    }
    #[test]
    fn sigterm_unread_pipe_progress_closes_http_without_kill_or_inventory() {
        run_output_probe(Some(libc::SIGTERM), false);
    }
}
