//! Desktop setup is deliberately separate from legacy provisioning and serving.
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::{self, Write};
use std::time::Duration;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, BufReader};

const ENDPOINT: &str = "http://127.0.0.1:11434";
const FRAME_LIMIT: usize = 16 * 1024;
const TAGS_LIMIT: usize = 256 * 1024;
const PROBE: Duration = Duration::from_secs(2);
const APPROVAL: Duration = Duration::from_secs(60);
const OVERALL: Duration = Duration::from_secs(15 * 60);
const IDLE: Duration = Duration::from_secs(30);

#[derive(Subcommand)]
pub enum SetupCommands {
    /// Observe eligible local models; never starts or installs anything
    Inspect,
    /// Request a model download only after bound stdin approval
    Pull {
        #[arg(long)]
        model: String,
    },
}

#[derive(Debug, Clone, Serialize)]
struct Failure {
    code: &'static str,
    message: &'static str,
}

fn failure(code: &'static str, message: &'static str) -> Failure {
    Failure { code, message }
}

type Result<T> = std::result::Result<T, Failure>;

fn validate_model(model: &str) -> Result<()> {
    let valid = model.len() <= 200
        && model.is_ascii()
        && model.split(':').count() == 2
        && model.split(':').all(|part| {
            !part.is_empty()
                && part.split('/').all(|component| {
                    !component.is_empty()
                        && component != "."
                        && component != ".."
                        && component.as_bytes()[0].is_ascii_alphanumeric()
                        && component
                            .bytes()
                            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
                })
        })
        && !model.rsplit(':').next().unwrap_or_default().contains('/')
        && !model.to_ascii_lowercase().contains("cloud");
    if valid {
        Ok(())
    } else {
        Err(failure("invalid_model", "Use an exact local model name:tag (maximum 200 ASCII bytes), not a URL or cloud model."))
    }
}

fn platform_supported(release: &str) -> bool {
    let value = |key: &str| {
        release.lines().find_map(|line| {
            let (name, value) = line.split_once('=')?;
            (name == key).then(|| value.trim_matches('"'))
        })
    };
    cfg!(all(target_os = "linux", target_arch = "x86_64"))
        && value("ID") == Some("ubuntu")
        && value("VERSION_ID") == Some("24.04")
}

fn host_gate() -> Result<()> {
    let release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    if platform_supported(&release) {
        Ok(())
    } else {
        Err(failure(
            "unsupported_platform",
            "Setup supports Ubuntu 24.04 LTS x86_64 only.",
        ))
    }
}

#[derive(Debug, Serialize)]
struct Model {
    name: String,
    digest: String,
    size: u64,
}

fn parse_tags(body: &[u8]) -> Result<Vec<Model>> {
    let bad = || {
        failure(
            "invalid_tags",
            "Local Ollama returned invalid or ambiguous model metadata.",
        )
    };
    let root: Value = serde_json::from_slice(body).map_err(|_| bad())?;
    let entries = root
        .get("models")
        .and_then(Value::as_array)
        .ok_or_else(bad)?;
    let mut names = HashSet::new();
    let mut models = Vec::new();
    for entry in entries {
        let name = entry.get("name").and_then(Value::as_str).ok_or_else(bad)?;
        if !names.insert(name) {
            return Err(bad());
        }
        if entry
            .get("model")
            .is_some_and(|alias| alias.as_str() != Some(name))
        {
            return Err(bad());
        }
        // Cloud entries never become selectable, even if a provider supplies local-looking names.
        if name.to_ascii_lowercase().contains("cloud")
            || entry
                .as_object()
                .ok_or_else(bad)?
                .iter()
                .any(|(key, value)| {
                    (key.starts_with("remote_") || key.contains("cloud")) && !value.is_null()
                })
        {
            continue;
        }
        validate_model(name).map_err(|_| bad())?;
        let digest = entry
            .get("digest")
            .and_then(Value::as_str)
            .ok_or_else(bad)?;
        let hex = digest.strip_prefix("sha256:").unwrap_or(digest);
        if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(bad());
        }
        let size = entry
            .get("size")
            .and_then(Value::as_u64)
            .filter(|s| *s > 0)
            .ok_or_else(bad)?;
        models.push(Model {
            name: name.into(),
            digest: digest.into(),
            size,
        });
    }
    Ok(models)
}

struct Provider {
    client: reqwest::Client,
    base: String,
    probe: Duration,
    idle: Duration,
    overall: Duration,
}

impl Provider {
    fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(PROBE)
            .build()
            .map_err(|_| {
                failure(
                    "client_error",
                    "Could not initialize the fixed-loopback HTTP client.",
                )
            })?;
        Ok(Self {
            client,
            base: ENDPOINT.into(),
            probe: PROBE,
            idle: IDLE,
            overall: OVERALL,
        })
    }

    async fn tags(&self) -> Result<Vec<Model>> {
        tokio::time::timeout(self.probe, async {
            let mut response = self
                .client
                .get(format!("{}/api/tags", self.base))
                .send()
                .await
                .map_err(|_| {
                    failure(
                        "provider_unavailable",
                        "Local Ollama is unavailable; setup never installs or starts it.",
                    )
                })?;
            check_http(&response)?;
            let mut body = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| failure("provider_io", "Could not read local Ollama inventory."))?
            {
                if chunk.len() > TAGS_LIMIT.saturating_sub(body.len()) {
                    return Err(failure(
                        "tags_too_large",
                        "Local Ollama inventory exceeds 256 KiB.",
                    ));
                }
                body.extend_from_slice(&chunk);
            }
            parse_tags(&body)
        })
        .await
        .map_err(|_| {
            failure(
                "probe_timeout",
                "Local Ollama inventory probe exceeded two seconds.",
            )
        })?
    }

    async fn pull<W: Write>(&self, model: &str, events: &mut Events<W>) -> Result<()> {
        let deadline = std::time::Instant::now() + self.overall;
        let mut response = tokio::time::timeout(self.idle, self.client.post(format!("{}/api/pull", self.base))
            .json(&json!({"model": model, "stream": true})).send()).await
            .map_err(|_| failure("idle_timeout", "Download observation was idle for 30 seconds; daemon work and cache outcome are unknown."))?
            .map_err(|_| failure("provider_io", "Download request failed; daemon work and cache outcome are unknown."))?;
        check_http(&response)?;
        let mut frames = Frames::default();
        loop {
            let chunk = tokio::time::timeout(self.idle, response.chunk()).await
                .map_err(|_| failure("idle_timeout", "Download observation was idle for 30 seconds; daemon work and cache outcome are unknown."))?
                .map_err(|_| failure("provider_io", "Download observation failed; daemon work and cache outcome are unknown."))?;
            let Some(chunk) = chunk else {
                return Err(failure("early_eof", "Provider stream ended without verified success; daemon work and cache outcome are unknown."));
            };
            for byte in chunk {
                if let Some(frame) = frames.push(byte)? {
                    if std::time::Instant::now() >= deadline {
                        return Err(failure("overall_timeout", "Download observation exceeded 15 minutes; daemon work and cache outcome are unknown."));
                    }
                    if parse_progress(&frame, events)? {
                        // Provider success is not readiness: verify a fresh exact eligible local tag.
                        let tags = self.tags().await?;
                        return if tags.iter().any(|tag| tag.name == model) {
                            Ok(())
                        } else {
                            Err(failure("verification_failed", "Provider reported success, but the exact eligible local model was not found."))
                        };
                    }
                }
            }
        }
    }
}

fn check_http(response: &reqwest::Response) -> Result<()> {
    if response.status().is_redirection() {
        Err(failure(
            "redirect_refused",
            "Local Ollama redirect refused; no redirected request was sent.",
        ))
    } else if !response.status().is_success() {
        Err(failure(
            "provider_http",
            "Local Ollama returned an unsuccessful HTTP status.",
        ))
    } else {
        Ok(())
    }
}

#[derive(Default)]
struct Frames {
    bytes: Vec<u8>,
}

impl Frames {
    fn push(&mut self, byte: u8) -> Result<Option<Vec<u8>>> {
        if self.bytes.len() == FRAME_LIMIT {
            return Err(failure("frame_too_large", "Protocol frame exceeds 16 KiB."));
        }
        if byte == b'\n' {
            Ok(Some(std::mem::take(&mut self.bytes)))
        } else {
            self.bytes.push(byte);
            Ok(None)
        }
    }
}

fn parse_progress<W: Write>(frame: &[u8], events: &mut Events<W>) -> Result<bool> {
    let bad = || {
        failure("invalid_progress", "Local Ollama returned malformed download progress; daemon work and cache outcome are unknown.")
    };
    let value: Value = serde_json::from_slice(frame).map_err(|_| bad())?;
    if value.get("error").is_some() {
        return Err(failure(
            "provider_error",
            "Local Ollama reported a download error; daemon work and cache outcome are unknown.",
        ));
    }
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(bad)?;
    if status == "success" {
        return Ok(true);
    }
    // Never echo arbitrary daemon text (which may contain paths, URLs or credentials).
    let safe_status = match status {
        "pulling manifest"
        | "verifying sha256 digest"
        | "writing manifest"
        | "removing any unused layers" => status,
        stage
            if stage.starts_with("pulling ")
                && stage[8..].bytes().all(|c| c.is_ascii_hexdigit())
                && (12..=64).contains(&stage[8..].len()) =>
        {
            "pulling layer"
        }
        _ => return Err(bad()),
    };
    let mut progress = json!({"status": safe_status});
    for key in ["completed", "total"] {
        if let Some(number) = value.get(key) {
            let number = number.as_u64().ok_or_else(bad)?;
            progress[key] = json!(number);
        }
    }
    if let (Some(completed), Some(total)) =
        (progress["completed"].as_u64(), progress["total"].as_u64())
    {
        if completed > total {
            return Err(bad());
        }
    }
    events.emit("progress", progress)?;
    Ok(false)
}

struct Events<W> {
    output: W,
    operation: String,
    seq: u64,
}

impl<W: Write> Events<W> {
    fn emit(&mut self, event: &str, mut fields: Value) -> Result<()> {
        fields["schema_version"] = json!(1);
        fields["command"] = json!("pull");
        fields["operation_id"] = json!(self.operation);
        fields["seq"] = json!(self.seq);
        fields["event"] = json!(event);
        let mut frame = serde_json::to_vec(&fields)
            .map_err(|_| failure("output_closed", "Protocol output is unavailable."))?;
        frame.push(b'\n');
        if frame.len() > FRAME_LIMIT {
            return Err(failure("frame_too_large", "Protocol frame exceeds 16 KiB."));
        }
        self.output
            .write_all(&frame)
            .and_then(|_| self.output.flush())
            .map_err(|_| failure("output_closed", "Protocol output is unavailable."))?;
        self.seq += 1;
        Ok(())
    }

    fn terminal(&mut self, status: &str, error: Failure) -> Result<()> {
        self.emit(
            "terminal",
            json!({"status": status, "code": error.code, "message": error.message}),
        )
    }
}

async fn input_line<R: AsyncBufRead + Unpin>(input: &mut R) -> Result<Option<Vec<u8>>> {
    let mut frame = Frames::default();
    loop {
        let buffer = input
            .fill_buf()
            .await
            .map_err(|_| failure("input_error", "Approval input could not be read."))?;
        if buffer.is_empty() {
            return Ok(None); // A partial line is not approval.
        }
        let mut consumed = 0;
        let mut line = None;
        for byte in buffer {
            consumed += 1;
            if let Some(bytes) = frame.push(*byte)? {
                line = Some(bytes);
                break;
            }
        }
        input.consume(consumed);
        if line.is_some() {
            return Ok(line);
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Approval {
    schema_version: u32,
    operation_id: String,
    model: String,
    approved: bool,
}

async fn approve<R: AsyncBufRead + Unpin>(
    input: &mut R,
    operation: &str,
    model: &str,
    budget: Duration,
) -> Result<()> {
    let line = tokio::time::timeout(budget, input_line(input))
        .await
        .map_err(|_| {
            failure(
                "approval_expired",
                "Approval expired; no download request was sent.",
            )
        })??
        .ok_or_else(|| {
            failure(
                "approval_eof",
                "Approval input closed; no download request was sent.",
            )
        })?;
    let approval: Approval = serde_json::from_slice(&line).map_err(|_| {
        failure(
            "invalid_approval",
            "Approval must be one bounded JSON line; no download request was sent.",
        )
    })?;
    if approval.schema_version != 1 || approval.operation_id != operation || approval.model != model
    {
        return Err(failure(
            "approval_mismatch",
            "Approval does not match this operation and exact model; no download request was sent.",
        ));
    }
    if !approval.approved {
        return Err(failure(
            "approval_declined",
            "Download declined; no download request was sent.",
        ));
    }
    Ok(())
}

async fn run_pull<R: AsyncBufRead + Unpin, W: Write>(
    provider: &Provider,
    input: &mut R,
    events: &mut Events<W>,
    model: &str,
    gate: Result<()>,
) -> Result<()> {
    if let Err(error) = validate_model(model) {
        return events.terminal("error", error);
    }
    if events.seq == 0 {
        events.emit(
            "approval_required",
            json!({"model":model, "endpoint":ENDPOINT}),
        )?;
    }
    if let Err(error) = gate {
        return events.terminal("error", error);
    }
    if let Err(error) = approve(input, &events.operation, model, APPROVAL).await {
        let status = if matches!(
            error.code,
            "approval_eof" | "approval_declined" | "approval_expired"
        ) {
            "cancelled"
        } else {
            "error"
        };
        return events.terminal(status, error);
    }
    // EOF wins even when already buffered at approval. Dropping this future drops the
    // local HTTP response/request, not a promise to stop daemon work or delete cache.
    let outcome = tokio::select! {
        biased;
        _ = input_line(input) => Err(failure("cancelled", "Local download request/observation cancelled; daemon work and cache outcome are unknown.")),
        result = tokio::time::timeout(provider.overall, provider.pull(model, events)) => result.unwrap_or_else(|_| Err(failure("overall_timeout", "Download observation exceeded 15 minutes; daemon work and cache outcome are unknown."))),
    };
    match outcome {
        Ok(()) => events.terminal("success", failure("model_verified", "Provider reported success and the exact eligible local model was freshly verified.")),
        Err(error) => events.terminal(if error.code == "cancelled" { "cancelled" } else { "error" }, error),
    }
}

// Tokio's blocking stdin can prevent runtime shutdown after a timeout. A nonblocking
// descriptor lets cancellation and approval deadlines stop without a stuck reader.
#[cfg(unix)]
struct Input(InputHandle, libc::c_int);

#[cfg(unix)]
enum InputHandle {
    Pollable(tokio::io::unix::AsyncFd<std::fs::File>),
    Immediate(std::fs::File),
}

#[cfg(unix)]
impl InputHandle {
    fn file(&self) -> &std::fs::File {
        match self {
            Self::Pollable(fd) => fd.get_ref(),
            Self::Immediate(file) => file,
        }
    }
}

#[cfg(unix)]
impl Input {
    fn new() -> io::Result<Self> {
        use std::os::fd::FromRawFd;
        let fd = unsafe { libc::dup(libc::STDIN_FILENO) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let file = unsafe { std::fs::File::from_raw_fd(fd) };
        Self::from_file(file)
    }

    fn from_file(file: std::fs::File) -> io::Result<Self> {
        use std::os::fd::AsRawFd;
        let fd = file.as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::last_os_error());
        }
        let registered = file.try_clone().and_then(tokio::io::unix::AsyncFd::new);
        match registered {
            Ok(registered) => Ok(Self(InputHandle::Pollable(registered), flags)),
            // epoll does not register regular files or /dev/null. Nonblocking
            // direct reads still observe their immediate data/EOF without a worker.
            Err(error) if error.raw_os_error() == Some(libc::EPERM) => {
                Ok(Self(InputHandle::Immediate(file), flags))
            }
            Err(error) => {
                unsafe {
                    libc::fcntl(fd, libc::F_SETFL, flags);
                }
                Err(error)
            }
        }
    }
}

#[cfg(unix)]
impl Drop for Input {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        unsafe {
            libc::fcntl(self.0.file().as_raw_fd(), libc::F_SETFL, self.1);
        }
    }
}

#[cfg(unix)]
impl tokio::io::AsyncRead for Input {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        use std::io::Read;
        let InputHandle::Pollable(fd) = &self.0 else {
            let count = (&mut self.0.file()).read(buf.initialize_unfilled())?;
            buf.advance(count);
            return std::task::Poll::Ready(Ok(()));
        };
        loop {
            let mut ready = std::task::ready!(fd.poll_read_ready(cx))?;
            match ready.try_io(|fd| (&mut fd.get_ref()).read(buf.initialize_unfilled())) {
                Ok(Ok(count)) => {
                    buf.advance(count);
                    return std::task::Poll::Ready(Ok(()));
                }
                Ok(Err(error)) => return std::task::Poll::Ready(Err(error)),
                Err(_) => continue,
            }
        }
    }
}

// Stdout backpressure must not turn a bounded download observation into an
// indefinitely blocked process. No diagnostics are written if this pipe closes.
#[cfg(unix)]
struct Output {
    file: std::fs::File,
    flags: libc::c_int,
    wait: Duration,
}

#[cfg(unix)]
impl Output {
    fn new() -> io::Result<Self> {
        use std::os::fd::FromRawFd;
        let fd = unsafe { libc::dup(libc::STDOUT_FILENO) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Self::from_file(unsafe { std::fs::File::from_raw_fd(fd) }, PROBE)
    }

    fn from_file(file: std::fs::File, wait: Duration) -> io::Result<Self> {
        use std::os::fd::AsRawFd;
        let fd = file.as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { file, flags, wait })
    }
}

#[cfg(unix)]
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        use std::os::fd::AsRawFd;
        let deadline = std::time::Instant::now() + self.wait;
        loop {
            match self.file.write(bytes) {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                    if remaining.is_zero() {
                        return Err(io::ErrorKind::TimedOut.into());
                    }
                    let mut fd = libc::pollfd {
                        fd: self.file.as_raw_fd(),
                        events: libc::POLLOUT,
                        revents: 0,
                    };
                    let result = unsafe {
                        libc::poll(
                            &mut fd,
                            1,
                            remaining.as_millis().max(1).min(i32::MAX as u128) as i32,
                        )
                    };
                    if result == 0 {
                        return Err(io::ErrorKind::TimedOut.into());
                    }
                    if result < 0 {
                        let error = io::Error::last_os_error();
                        if error.kind() != io::ErrorKind::Interrupted {
                            return Err(error);
                        }
                    }
                }
                result => return result,
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(unix)]
impl Drop for Output {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        unsafe {
            libc::fcntl(self.file.as_raw_fd(), libc::F_SETFL, self.flags);
        }
    }
}

pub async fn handle(command: SetupCommands) {
    let gate = host_gate();
    let provider = Provider::new();
    match command {
        SetupCommands::Inspect => {
            let result = match (gate, provider) {
                (Ok(()), Ok(provider)) => provider.tags().await,
                (Err(error), _) | (_, Err(error)) => Err(error),
            };
            let value = match result {
                Ok(models) => {
                    json!({"schema_version":1,"command":"inspect","status":"ready","provider":"ollama","endpoint":ENDPOINT,"models":models,"error":null})
                }
                Err(error) => {
                    json!({"schema_version":1,"command":"inspect","status":if matches!(error.code,"provider_unavailable"|"probe_timeout") {"unavailable"} else {"error"},"provider":"ollama","endpoint":ENDPOINT,"models":[],"error":error})
                }
            };
            #[cfg(unix)]
            let Ok(mut output) = Output::new() else {
                return;
            };
            #[cfg(not(unix))]
            let mut output = io::stdout();
            let _ = serde_json::to_writer(&mut output, &value);
            let _ = output.write_all(b"\n");
            let _ = output.flush();
        }
        SetupCommands::Pull { model } => {
            let operation = new_operation_id();
            #[cfg(unix)]
            let Ok(output) = Output::new() else {
                return;
            };
            #[cfg(not(unix))]
            let output = io::stdout();
            let mut events = Events {
                output,
                operation: operation.as_ref().cloned().unwrap_or_default(),
                seq: 0,
            };
            if operation.is_err() {
                let _ = events.terminal("error", failure("operation_id_error", "Could not create a fresh operation identifier; no download request was sent."));
                return;
            }
            if let Err(error) = validate_model(&model) {
                let _ = events.terminal("error", error);
                return;
            }
            if events
                .emit(
                    "approval_required",
                    json!({"model":model, "endpoint":ENDPOINT}),
                )
                .is_err()
            {
                return;
            }
            let provider = match provider {
                Ok(provider) => provider,
                Err(error) => {
                    let _ = events.terminal("error", error);
                    return;
                }
            };
            if let Err(error) = gate.clone() {
                let _ = events.terminal("error", error);
                return;
            }
            #[cfg(unix)]
            match Input::new() {
                Ok(input) => {
                    let _ = run_pull(
                        &provider,
                        &mut BufReader::new(input),
                        &mut events,
                        &model,
                        gate,
                    )
                    .await;
                }
                Err(_) => {
                    let _ = events.terminal(
                        "error",
                        failure(
                            "input_error",
                            "Readable approval input is required; no download request was sent.",
                        ),
                    );
                }
            }
            #[cfg(not(unix))]
            {
                let _ = (provider, model, gate);
                let _ = events.terminal(
                    "error",
                    failure(
                        "unsupported_platform",
                        "Setup supports Ubuntu 24.04 LTS x86_64 only.",
                    ),
                );
            }
        }
    }
}

fn new_operation_id() -> io::Result<String> {
    use std::io::Read;
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        routing::{get, post},
        Router,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::io::AsyncWriteExt;

    fn inventory(name: &str) -> Value {
        json!({"models":[{"name":name,"model":name,"digest":"a".repeat(64),"size":123}]})
    }

    fn consent(approved: bool) -> Vec<u8> {
        format!("{}\n", json!({"schema_version":1,"operation_id":"op","model":"test:latest","approved":approved})).into_bytes()
    }

    fn events() -> Events<Vec<u8>> {
        Events {
            output: Vec::new(),
            operation: "op".into(),
            seq: 0,
        }
    }

    fn output(events: &Events<Vec<u8>>) -> Vec<Value> {
        std::str::from_utf8(&events.output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    async fn fixture(router: Router) -> (Provider, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut provider = Provider::new().unwrap();
        provider.base = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        (provider, task)
    }

    #[test]
    fn exact_model_and_platform_validation() {
        for model in [
            "",
            "test",
            "a:",
            ":b",
            "a:b:c",
            "https://host/a:b",
            "host:11434/a:b",
            "../a:b",
            "a/../b:c",
            "a//b:c",
            "/a:b",
            "-a:b",
            "a:b/c",
            "a:cloud",
            "a:b\n",
            "a:b\u{1b}",
            "a:☃",
            "a b:c",
        ] {
            assert!(validate_model(model).is_err(), "{model:?}");
        }
        assert!(validate_model(&format!("a:{}", "b".repeat(199))).is_err());
        assert!(validate_model(&format!("a:{}", "b".repeat(198))).is_ok());
        for model in ["test:latest", "library/test:q4_K-M", "local/model:1.2"] {
            assert!(validate_model(model).is_ok());
        }
        assert!(platform_supported("ID=ubuntu\nVERSION_ID=\"24.04\"\n"));
        assert!(!platform_supported("ID=debian\nVERSION_ID=24.04\n"));
        assert!(!platform_supported("ID=ubuntu\nVERSION_ID=22.04\n"));
    }

    #[test]
    fn inventory_requires_unambiguous_eligible_metadata() {
        let normal = inventory("test:latest");
        assert_eq!(
            parse_tags(&serde_json::to_vec(&normal).unwrap()).unwrap()[0].name,
            "test:latest"
        );
        for field in ["remote_host", "remote_model", "cloud"] {
            let mut value = normal.clone();
            value["models"][0][field] = json!("secret/path");
            assert!(parse_tags(&serde_json::to_vec(&value).unwrap())
                .unwrap()
                .is_empty());
        }
        assert!(
            parse_tags(&serde_json::to_vec(&inventory("test:cloud")).unwrap())
                .unwrap()
                .is_empty()
        );
        for (key, value) in [
            ("digest", json!("unsafe/path")),
            ("size", json!(-1)),
            ("model", json!("other:tag")),
            ("name", json!("unsafe\n:tag")),
        ] {
            let mut tags = normal.clone();
            tags["models"][0][key] = value;
            assert!(parse_tags(&serde_json::to_vec(&tags).unwrap()).is_err());
        }
        let duplicate = json!({"models":[normal["models"][0],normal["models"][0]]});
        assert!(parse_tags(&serde_json::to_vec(&duplicate).unwrap()).is_err());
        for bytes in [b"not json".as_slice(), b"{}", b"{\"models\":null}"] {
            assert!(parse_tags(bytes).is_err());
        }
    }

    #[tokio::test]
    async fn approval_decline_eof_mismatch_malformed_split_and_expiry() {
        let declined = consent(false);
        let mut input = BufReader::new(declined.as_slice());
        assert_eq!(
            approve(&mut input, "op", "test:latest", APPROVAL)
                .await
                .unwrap_err()
                .code,
            "approval_declined"
        );
        for bytes in [Vec::new(), b"{\"approved\":true}".to_vec()] {
            assert_eq!(
                approve(
                    &mut BufReader::new(bytes.as_slice()),
                    "op",
                    "test:latest",
                    APPROVAL
                )
                .await
                .unwrap_err()
                .code,
                "approval_eof"
            );
        }
        let yes = consent(true);
        for (operation, model) in [("other", "test:latest"), ("op", "other:latest")] {
            assert_eq!(
                approve(
                    &mut BufReader::new(yes.as_slice()),
                    operation,
                    model,
                    APPROVAL
                )
                .await
                .unwrap_err()
                .code,
                "approval_mismatch"
            );
        }
        for bytes in [b"broken\n".to_vec(), format!("{}\n",json!({"schema_version":2,"operation_id":"op","model":"test:latest","approved":true})).into_bytes()] {
            assert!(approve(&mut BufReader::new(bytes.as_slice()), "op", "test:latest", APPROVAL).await.is_err());
        }
        let oversized = vec![b'x'; FRAME_LIMIT + 1];
        assert_eq!(
            approve(
                &mut BufReader::new(oversized.as_slice()),
                "op",
                "test:latest",
                APPROVAL
            )
            .await
            .unwrap_err()
            .code,
            "frame_too_large"
        );
        let (mut writer, reader) = tokio::io::duplex(64);
        let task = tokio::spawn(async move {
            for byte in consent(true) {
                writer.write_all(&[byte]).await.unwrap();
            }
        });
        assert!(
            approve(&mut BufReader::new(reader), "op", "test:latest", APPROVAL)
                .await
                .is_ok()
        );
        task.await.unwrap();
        let (_writer, reader) = tokio::io::duplex(64);
        assert_eq!(
            approve(
                &mut BufReader::new(reader),
                "op",
                "test:latest",
                Duration::from_millis(5)
            )
            .await
            .unwrap_err()
            .code,
            "approval_expired"
        );
    }

    #[test]
    fn split_progress_frames_are_bounded_and_sanitized() {
        let mut decoder = Frames::default();
        let mut e = events();
        for byte in b"{\"status\":\"pulling manifest\"}\n{\"status\":\"pulling abcdef123456\",\"completed\":3,\"total\":10}\n" {
            if let Some(frame) = decoder.push(*byte).unwrap() { assert!(!parse_progress(&frame, &mut e).unwrap()); }
        }
        assert_eq!(output(&e)[1]["status"], "pulling layer");
        assert_eq!(output(&e)[1]["completed"], 3);
        assert!(output(&e)[0].get("total").is_none());
        for frame in [
            b"bad".as_slice(),
            b"{}",
            b"{\"status\":\"/secret/token\"}",
            b"{\"error\":\"/secret/token\"}",
            b"{\"status\":\"pulling manifest\",\"completed\":2,\"total\":1}",
            b"{\"status\":\"pulling manifest\",\"total\":-1}",
        ] {
            assert!(parse_progress(frame, &mut e).is_err());
        }
        assert!(!String::from_utf8(e.output).unwrap().contains("secret"));
        for _ in 0..FRAME_LIMIT {
            decoder.push(b'x').unwrap();
        }
        assert_eq!(decoder.push(b'x').unwrap_err().code, "frame_too_large");
    }

    #[tokio::test]
    async fn rejected_approval_and_unsupported_host_never_post() {
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        let (provider, task) = fixture(Router::new().route(
            "/api/pull",
            post(move || {
                c.fetch_add(1, Ordering::SeqCst);
                async { "{\"status\":\"success\"}\n" }
            }),
        ))
        .await;
        for bytes in [consent(false), Vec::new(), b"bad\n".to_vec(), consent(true)] {
            let mut e = events();
            let gate = if bytes == consent(true) {
                Err(failure("unsupported_platform", "unsupported"))
            } else {
                Ok(())
            };
            run_pull(
                &provider,
                &mut BufReader::new(bytes.as_slice()),
                &mut e,
                "test:latest",
                gate,
            )
            .await
            .unwrap();
            assert_eq!(output(&e).last().unwrap()["event"], "terminal");
            assert_ne!(output(&e).last().unwrap()["status"], "success");
        }
        assert_eq!(count.load(Ordering::SeqCst), 0);
        task.abort();
    }

    #[tokio::test]
    async fn success_requires_fresh_exact_eligible_inventory_and_bound_body() {
        for tags in [inventory("test:latest"), inventory("other:latest"), {
            let mut value = inventory("test:latest");
            value["models"][0]["remote_host"] = json!("remote");
            value
        }] {
            let verified = tags == inventory("test:latest");
            let reads = Arc::new(AtomicUsize::new(0));
            let count = reads.clone();
            let (provider, task) = fixture(
                Router::new()
                    .route(
                        "/api/tags",
                        get(move || {
                            count.fetch_add(1, Ordering::SeqCst);
                            let tags = tags.clone();
                            async move { axum::Json(tags) }
                        }),
                    )
                    .route(
                        "/api/pull",
                        post(|axum::Json(body): axum::Json<Value>| async move {
                            assert_eq!(body, json!({"model":"test:latest","stream":true}));
                            "{\"status\":\"pulling manifest\"}\n{\"status\":\"success\"}\n"
                        }),
                    ),
            )
            .await;
            let (mut writer, reader) = tokio::io::duplex(1024);
            writer.write_all(&consent(true)).await.unwrap();
            let mut e = events();
            run_pull(
                &provider,
                &mut BufReader::new(reader),
                &mut e,
                "test:latest",
                Ok(()),
            )
            .await
            .unwrap();
            let output = output(&e);
            assert_eq!(output[0]["event"], "approval_required");
            assert_eq!(output[0]["endpoint"], ENDPOINT);
            for (seq, item) in output.iter().enumerate() {
                assert_eq!(item["schema_version"], 1);
                assert_eq!(item["command"], "pull");
                assert_eq!(item["operation_id"], "op");
                assert_eq!(item["seq"], seq);
            }
            assert_eq!(
                output.last().unwrap()["status"],
                if verified { "success" } else { "error" }
            );
            assert_eq!(
                output
                    .iter()
                    .filter(|item| item["event"] == "terminal")
                    .count(),
                1
            );
            assert_eq!(reads.load(Ordering::SeqCst), 1);
            task.abort();
        }
    }

    #[tokio::test]
    async fn malformed_oversize_early_eof_and_provider_error_are_terminal() {
        for (body, code) in [
            ("bad\n".to_string(), "invalid_progress"),
            ("x".repeat(FRAME_LIMIT + 1), "frame_too_large"),
            ("{\"status\":\"pulling manifest\"}\n".into(), "early_eof"),
            ("{\"error\":\"secret\"}\n".into(), "provider_error"),
        ] {
            let (provider, task) = fixture(Router::new().route(
                "/api/pull",
                post(move || {
                    let body = body.clone();
                    async move { body }
                }),
            ))
            .await;
            let (mut writer, reader) = tokio::io::duplex(1024);
            writer.write_all(&consent(true)).await.unwrap();
            let mut e = events();
            run_pull(
                &provider,
                &mut BufReader::new(reader),
                &mut e,
                "test:latest",
                Ok(()),
            )
            .await
            .unwrap();
            assert_eq!(output(&e).last().unwrap()["code"], code);
            assert!(!String::from_utf8(e.output).unwrap().contains("secret"));
            task.abort();
        }
    }

    #[tokio::test]
    async fn eof_before_post_and_during_request_cancels_truthfully() {
        let posts = Arc::new(AtomicUsize::new(0));
        let count = posts.clone();
        let (provider, task) = fixture(Router::new().route(
            "/api/pull",
            post(move || {
                count.fetch_add(1, Ordering::SeqCst);
                async {
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    ""
                }
            }),
        ))
        .await;
        let yes = consent(true);
        let mut e = events();
        run_pull(
            &provider,
            &mut BufReader::new(yes.as_slice()),
            &mut e,
            "test:latest",
            Ok(()),
        )
        .await
        .unwrap();
        assert_eq!(posts.load(Ordering::SeqCst), 0);
        assert_eq!(output(&e).last().unwrap()["status"], "cancelled");
        let (mut writer, reader) = tokio::io::duplex(1024);
        writer.write_all(&consent(true)).await.unwrap();
        let c = posts.clone();
        let closer = tokio::spawn(async move {
            while c.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
            drop(writer);
        });
        let mut e = events();
        run_pull(
            &provider,
            &mut BufReader::new(reader),
            &mut e,
            "test:latest",
            Ok(()),
        )
        .await
        .unwrap();
        assert_eq!(output(&e).last().unwrap()["status"], "cancelled");
        assert!(output(&e).last().unwrap()["message"]
            .as_str()
            .unwrap()
            .contains("unknown"));
        closer.await.unwrap();
        task.abort();
    }

    #[tokio::test]
    async fn idle_overall_probe_and_inventory_size_limits() {
        let (mut provider, task) = fixture(
            Router::new()
                .route(
                    "/api/pull",
                    post(|| async {
                        tokio::time::sleep(Duration::from_secs(10)).await;
                        ""
                    }),
                )
                .route(
                    "/api/tags",
                    get(|| async {
                        tokio::time::sleep(Duration::from_secs(10)).await;
                        ""
                    }),
                ),
        )
        .await;
        provider.probe = Duration::from_millis(5);
        assert_eq!(provider.tags().await.unwrap_err().code, "probe_timeout");
        for overall in [false, true] {
            provider.idle = if overall {
                Duration::from_secs(1)
            } else {
                Duration::from_millis(5)
            };
            provider.overall = if overall {
                Duration::from_millis(5)
            } else {
                Duration::from_secs(1)
            };
            let (mut writer, reader) = tokio::io::duplex(1024);
            writer.write_all(&consent(true)).await.unwrap();
            let mut e = events();
            run_pull(
                &provider,
                &mut BufReader::new(reader),
                &mut e,
                "test:latest",
                Ok(()),
            )
            .await
            .unwrap();
            assert_eq!(
                output(&e).last().unwrap()["code"],
                if overall {
                    "overall_timeout"
                } else {
                    "idle_timeout"
                }
            );
        }
        task.abort();
        let (provider, task) =
            fixture(Router::new().route("/api/tags", get(|| async { "x".repeat(TAGS_LIMIT + 1) })))
                .await;
        assert_eq!(provider.tags().await.unwrap_err().code, "tags_too_large");
        task.abort();
    }

    #[tokio::test]
    async fn redirects_refused_for_inventory_and_pull() {
        use axum::http::{header, StatusCode};
        let destination_hits = Arc::new(AtomicUsize::new(0));
        let hits = destination_hits.clone();
        let (destination, dest_task) = fixture(Router::new().route(
            "/never",
            get(move || {
                hits.fetch_add(1, Ordering::SeqCst);
                async { "must not be called" }
            }),
        ))
        .await;
        let location = format!("{}/never", destination.base);
        let other_location = location.clone();
        let (provider, task) = fixture(
            Router::new()
                .route(
                    "/api/tags",
                    get(move || {
                        let location = location.clone();
                        async move {
                            (
                                StatusCode::TEMPORARY_REDIRECT,
                                [(header::LOCATION, location)],
                            )
                        }
                    }),
                )
                .route(
                    "/api/pull",
                    post(move || {
                        let location = other_location.clone();
                        async move {
                            (
                                StatusCode::TEMPORARY_REDIRECT,
                                [(header::LOCATION, location)],
                            )
                        }
                    }),
                ),
        )
        .await;
        assert_eq!(provider.tags().await.unwrap_err().code, "redirect_refused");
        assert_eq!(
            provider
                .pull("test:latest", &mut events())
                .await
                .unwrap_err()
                .code,
            "redirect_refused"
        );
        assert_eq!(destination_hits.load(Ordering::SeqCst), 0);
        task.abort();
        dest_task.abort();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn nonblocking_real_pipe_input_observes_deadline_and_eof() {
        use std::os::fd::FromRawFd;
        let mut fds = [0; 2];
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        let reader = unsafe { std::fs::File::from_raw_fd(fds[0]) };
        let mut writer = unsafe { std::fs::File::from_raw_fd(fds[1]) };
        let mut input = BufReader::new(Input::from_file(reader).unwrap());
        assert_eq!(
            approve(&mut input, "op", "test:latest", Duration::from_millis(5))
                .await
                .unwrap_err()
                .code,
            "approval_expired"
        );
        writer.write_all(&consent(true)).unwrap();
        assert!(approve(&mut input, "op", "test:latest", APPROVAL)
            .await
            .is_ok());
        drop(writer);
        assert!(input_line(&mut input).await.unwrap().is_none());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn nonpollable_stdin_eof_is_cancelled_not_input_error() {
        let input = Input::from_file(std::fs::File::open("/dev/null").unwrap()).unwrap();
        assert_eq!(
            approve(&mut BufReader::new(input), "op", "test:latest", APPROVAL)
                .await
                .unwrap_err()
                .code,
            "approval_eof"
        );
    }

    #[cfg(unix)]
    #[test]
    fn protocol_output_backpressure_is_bounded() {
        use std::os::fd::FromRawFd;
        let mut fds = [0; 2];
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        let _reader = unsafe { std::fs::File::from_raw_fd(fds[0]) };
        let writer = unsafe { std::fs::File::from_raw_fd(fds[1]) };
        let mut output = Output::from_file(writer, Duration::from_millis(5)).unwrap();
        assert_eq!(
            output
                .write_all(&vec![b'x'; 1024 * 1024])
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
    }

    #[test]
    fn ambient_proxy_is_ignored_in_isolated_test_process() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "commands::setup::tests::proxy_fixture_child"])
            .env("SMITH_SETUP_PROXY_FIXTURE", "1")
            .env("HTTP_PROXY", "http://127.0.0.1:1")
            .env("HTTPS_PROXY", "http://127.0.0.1:1")
            .env("ALL_PROXY", "http://127.0.0.1:1")
            .env("http_proxy", "http://127.0.0.1:1")
            .env("https_proxy", "http://127.0.0.1:1")
            .env("all_proxy", "http://127.0.0.1:1")
            .env("NO_PROXY", "")
            .env("no_proxy", "")
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn proxy_fixture_child() {
        if std::env::var_os("SMITH_SETUP_PROXY_FIXTURE").is_none() {
            return;
        }
        let (provider, task) = fixture(
            Router::new()
                .route(
                    "/api/tags",
                    get(|| async { axum::Json(inventory("test:latest")) }),
                )
                .route("/api/pull", post(|| async { "{\"status\":\"success\"}\n" })),
        )
        .await;
        assert_eq!(provider.tags().await.unwrap()[0].name, "test:latest");
        assert!(provider.pull("test:latest", &mut events()).await.is_ok());
        task.abort();
    }

    #[tokio::test]
    async fn split_http_chunks_and_body_idle_deadline() {
        use tokio::io::AsyncReadExt;
        for stall in [false, true] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let mut provider = Provider::new().unwrap();
            provider.base = format!("http://{}", listener.local_addr().unwrap());
            provider.idle = Duration::from_millis(20);
            let task = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = [0u8; 4096];
                assert!(socket.read(&mut request).await.unwrap() > 0);
                socket
                    .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
                    .await
                    .unwrap();
                for byte in b"{\"status\":\"pulling manifest\"}\n" {
                    let frame = [b'1', b'\r', b'\n', *byte, b'\r', b'\n'];
                    socket.write_all(&frame).await.unwrap();
                    tokio::task::yield_now().await;
                }
                if stall {
                    tokio::time::sleep(Duration::from_secs(10)).await;
                } else {
                    socket.write_all(b"0\r\n\r\n").await.unwrap();
                }
            });
            let mut e = events();
            let error = provider.pull("test:latest", &mut e).await.unwrap_err();
            assert_eq!(error.code, if stall { "idle_timeout" } else { "early_eof" });
            assert_eq!(output(&e)[0]["status"], "pulling manifest");
            task.abort();
        }
    }

    #[test]
    fn setup_cli_is_namespaced_and_has_no_unconditional_approval_option() {
        use clap::Parser;
        assert!(crate::Cli::try_parse_from(["smith", "setup", "inspect"]).is_ok());
        assert!(
            crate::Cli::try_parse_from(["smith", "setup", "pull", "--model", "test:latest"])
                .is_ok()
        );
        assert!(crate::Cli::try_parse_from(["smith", "setup", "pull"]).is_err());
        assert!(crate::Cli::try_parse_from([
            "smith",
            "setup",
            "pull",
            "--model",
            "test:latest",
            "--yes"
        ])
        .is_err());
    }
}
