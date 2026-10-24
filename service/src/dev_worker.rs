//! Private, receipt-only container entry point. This is not an inference worker.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs::File;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub(crate) const MAX_FRAME: usize = 256 * 1024;
pub(crate) const MAX_PROMPT: usize = 32 * 1024;
pub(crate) const MARKER: &str = "smith-receipt-worker-v1";

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Frame {
    Ready {
        request_id: String,
        session_id: String,
    },
    Prompt {
        request_id: String,
        session_id: String,
        text: String,
    },
    Recorded {
        request_id: String,
        session_id: String,
    },
}

pub(crate) fn valid_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn valid_session(id: &str) -> bool {
    id.starts_with("ses")
        && id.len() <= 128
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
pub(crate) fn session_id(id: &str) -> crate::Result<()> {
    if valid_session(id) {
        Ok(())
    } else {
        Err("Invalid session identity".into())
    }
}

pub(crate) fn nonblocking(fd: i32) -> crate::Result<()> {
    // SAFETY: fcntl operates on the owned descriptor; no pointer arguments.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err("Cannot bound worker IO".into());
    }
    Ok(())
}

pub(crate) fn wait_io(
    fd: i32,
    events: i16,
    until: Instant,
    cancelled: &dyn Fn() -> bool,
) -> crate::Result<()> {
    loop {
        if cancelled() {
            return Err("Worker exchange cancelled".into());
        }
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err("Worker exchange deadline exceeded".into());
        }
        let mut pfd = libc::pollfd {
            fd,
            events,
            revents: 0,
        };
        // SAFETY: one valid pollfd, bounded timeout, no descriptor ownership transfer.
        let rc = unsafe { libc::poll(&mut pfd, 1, left.as_millis().min(50) as i32 + 1) };
        if rc > 0 {
            return Ok(());
        }
        if rc < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            return Err("Worker IO poll failed".into());
        }
    }
}

pub(crate) fn write_bytes<W: Write + AsRawFd>(
    io: &mut W,
    bytes: &[u8],
    until: Instant,
    cancel: &dyn Fn() -> bool,
) -> crate::Result<()> {
    let mut pos = 0;
    while pos < bytes.len() {
        wait_io(io.as_raw_fd(), libc::POLLOUT, until, cancel)?;
        match io.write(&bytes[pos..]) {
            Ok(0) => return Err("Worker pipe closed".into()),
            Ok(n) => pos += n,
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) => {}
            Err(_) => return Err("Worker pipe write failed".into()),
        }
    }
    Ok(())
}
pub(crate) fn send<W: Write + AsRawFd>(
    io: &mut W,
    frame: &Frame,
    until: Instant,
    cancel: &dyn Fn() -> bool,
) -> crate::Result<()> {
    let mut bytes = serde_json::to_vec(frame).map_err(|_| "Cannot encode worker frame")?;
    if bytes.len() >= MAX_FRAME {
        return Err("Worker frame too large".into());
    }
    bytes.push(b'\n');
    write_bytes(io, &bytes, until, cancel)
}
pub(crate) fn receive<R: Read + AsRawFd>(
    io: &mut R,
    until: Instant,
    cancel: &dyn Fn() -> bool,
) -> crate::Result<Frame> {
    let mut bytes = Vec::new();
    loop {
        wait_io(io.as_raw_fd(), libc::POLLIN, until, cancel)?;
        let mut b = [0];
        match io.read(&mut b) {
            Ok(0) => return Err("Worker pipe closed before receipt".into()),
            Ok(_) if b[0] == b'\n' => {
                return serde_json::from_slice(&bytes)
                    .map_err(|_| "Invalid worker control frame".into())
            }
            Ok(_) => {
                bytes.push(b[0]);
                if bytes.len() >= MAX_FRAME {
                    return Err("Worker output limit exceeded".into());
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) => {}
            Err(_) => return Err("Worker pipe read failed".into()),
        }
    }
}

pub(crate) struct OwnedChild(pub Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        // Do not wait forever even if the OS cannot reap an uninterruptible child.
        let until = Instant::now() + Duration::from_millis(500);
        while Instant::now() < until {
            if matches!(self.0.try_wait(), Ok(Some(_))) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

fn duplicate_input(fd: RawFd) -> crate::Result<File> {
    // SAFETY: fcntl duplicates the borrowed descriptor; the new descriptor is owned below.
    let duplicated = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
    if duplicated < 0 {
        return Err("Cannot own worker input".into());
    }
    // SAFETY: successful duplication returns a fresh descriptor owned only by this File.
    let input = unsafe { File::from_raw_fd(duplicated) };
    // File is unbuffered: polling before each read must not overlook prefetched Stdin bytes.
    nonblocking(input.as_raw_fd())?;
    Ok(input)
}

fn input_closed(input: &File) -> bool {
    let mut pfd = libc::pollfd {
        fd: input.as_raw_fd(),
        events: 0,
        revents: 0,
    };
    // SAFETY: input owns the descriptor; poll does not read or consume control frames.
    unsafe {
        libc::poll(&mut pfd, 1, 0);
    }
    pfd.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0
}

// A fresh loopback connection per request, no proxies, redirects, TLS, or ambient credentials.
// Connection: close bounds the response; both Content-Length and chunked HTTP/1.1 are supported.
fn http(
    method: &str,
    path: &str,
    body: Option<Value>,
    until: Instant,
    input: &File,
) -> crate::Result<Value> {
    let addr: SocketAddr = "127.0.0.1:4096"
        .parse()
        .map_err(|_| "Invalid worker endpoint")?;
    let timeout = until
        .saturating_duration_since(Instant::now())
        .min(Duration::from_secs(2));
    if timeout.is_zero() {
        return Err("OpenCode HTTP deadline exceeded".into());
    }
    let mut stream =
        TcpStream::connect_timeout(&addr, timeout).map_err(|_| "OpenCode unavailable")?;
    nonblocking(stream.as_raw_fd())?;
    let body = body.map(|b| b.to_string()).unwrap_or_default();
    let request = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:4096\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len());
    let closed = || input_closed(input);
    write_bytes(&mut stream, request.as_bytes(), until, &closed)?;
    let mut bytes = Vec::new();
    loop {
        wait_io(stream.as_raw_fd(), libc::POLLIN, until, &closed)?;
        let mut buf = [0; 4096];
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                bytes.extend_from_slice(&buf[..n]);
                if bytes.len() > MAX_FRAME {
                    return Err("OpenCode HTTP output limit exceeded".into());
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) => {}
            Err(_) => return Err("OpenCode HTTP read failed".into()),
        }
    }
    decode_http(&bytes)
}
fn decode_http(bytes: &[u8]) -> crate::Result<Value> {
    let split = bytes
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("Malformed OpenCode HTTP response")?;
    let headers = std::str::from_utf8(&bytes[..split]).map_err(|_| "Invalid HTTP headers")?;
    let status = headers
        .lines()
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .nth(1)
        .unwrap_or_default();
    if !matches!(status, "200" | "201") {
        return Err("OpenCode rejected receipt request".into());
    }
    let body = &bytes[split + 4..];
    let chunked = headers
        .lines()
        .any(|l| l.eq_ignore_ascii_case("transfer-encoding: chunked"));
    let mut decoded = Vec::new();
    if chunked {
        let mut rest = body;
        loop {
            let end = rest
                .windows(2)
                .position(|w| w == b"\r\n")
                .ok_or("Malformed HTTP chunk")?;
            let n = usize::from_str_radix(
                std::str::from_utf8(&rest[..end]).map_err(|_| "Invalid chunk size")?,
                16,
            )
            .map_err(|_| "Invalid chunk size")?;
            rest = &rest[end + 2..];
            if n == 0 {
                break;
            }
            if n > rest.len().saturating_sub(2) || &rest[n..n + 2] != b"\r\n" {
                return Err("Truncated HTTP chunk".into());
            }
            decoded.extend_from_slice(&rest[..n]);
            rest = &rest[n + 2..];
        }
    } else {
        let length = headers
            .lines()
            .find_map(|l| {
                let (name, v) = l.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| v.trim().parse::<usize>().ok())
                    .flatten()
            })
            .ok_or("Missing HTTP response length")?;
        if body.len() != length {
            return Err("Truncated HTTP body".into());
        }
        decoded.extend_from_slice(body);
    }
    serde_json::from_slice(&decoded).map_err(|_| "Invalid OpenCode JSON response".into())
}
fn recorded(value: &Value, session: &str, message: &str, prompt: &str) -> bool {
    let info = &value["info"];
    let Some(parts) = value["parts"].as_array() else {
        return false;
    };
    info["role"] == "user"
        && info["id"] == message
        && info["sessionID"] == session
        && parts.len() == 1
        && parts[0]["type"] == "text"
        && parts[0]["text"] == prompt
        && parts[0]["sessionID"] == session
        && parts[0]["messageID"] == message
}

/// Internal container entry only; never call this for normal service startup.
pub fn run(request_id: &str) -> crate::Result<()> {
    if !valid_id(request_id)
        || unsafe { libc::geteuid() } != 1000
        || unsafe { libc::getuid() } != 1000
        || std::env::var("SMITH_DEV_WORKER").as_deref() != Ok(MARKER)
        || std::env::current_dir().ok().as_deref() != Some(std::path::Path::new("/workspace"))
        || std::fs::read_to_string("/etc/smith-dev-worker")
            .ok()
            .as_deref()
            != Some("smith-receipt-worker-v1\n")
    {
        return Err("Private worker guard rejected".into());
    }
    let started = Instant::now();
    let total = started + Duration::from_secs(90);
    let startup = started + Duration::from_secs(60);
    let mut input = duplicate_input(libc::STDIN_FILENO)?;
    let mut output = std::io::stdout();
    nonblocking(output.as_raw_fd())?;
    let config = json!({"enabled_providers":[],"plugin":[],"mcp":{},"tools":{"*":false},
        "permission":"deny","share":"disabled","autoupdate":false,"snapshot":false,
        "model":"ollama/smith-receipt-only","default_agent":"build"});
    std::fs::create_dir_all("/tmp/home").map_err(|_| "Cannot initialize private worker home")?;
    let mut command = Command::new("/usr/local/bin/opencode");
    command
        .args(["serve", "--hostname", "127.0.0.1", "--port", "4096"])
        .env_clear()
        .env("PATH", "/usr/local/bin:/usr/bin:/bin")
        .env("HOME", "/tmp/home")
        .env("XDG_CONFIG_HOME", "/etc/smith-opencode")
        .env("XDG_DATA_HOME", "/tmp/data")
        .env("XDG_CACHE_HOME", "/tmp/cache")
        .env("XDG_STATE_HOME", "/tmp/state")
        .env("TMPDIR", "/tmp")
        .env("OPENCODE_CONFIG_CONTENT", config.to_string())
        .current_dir("/workspace")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for flag in [
        "OPENCODE_PURE",
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
    ] {
        command.env(flag, "true");
    }
    let mut child = OwnedChild(command.spawn().map_err(|_| "Cannot start owned OpenCode")?);
    loop {
        if child
            .0
            .try_wait()
            .map_err(|_| "Cannot observe OpenCode")?
            .is_some()
        {
            return Err("OpenCode exited during startup".into());
        }
        let attempt = startup.min(Instant::now() + Duration::from_secs(1));
        if let Ok(health) = http("GET", "/global/health", None, attempt, &input) {
            if health["healthy"] != true || health["version"] != "1.19.3" {
                return Err("OpenCode version/health mismatch".into());
            }
            break;
        }
        if Instant::now() >= startup {
            return Err("OpenCode startup deadline exceeded".into());
        }
        // Parent disconnect/cancellation must terminate startup too, without consuming a prompt.
        let mut pfd = libc::pollfd {
            fd: input.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        unsafe {
            libc::poll(&mut pfd, 1, 50);
        }
        if pfd.revents != 0 {
            return Err("Worker input closed or arrived before readiness".into());
        }
    }
    let session = http(
        "POST",
        "/session",
        Some(json!({"title":"Smith receipt-only"})),
        startup,
        &input,
    )?;
    let session = session["id"]
        .as_str()
        .ok_or("Missing session identity")?
        .to_string();
    session_id(&session)?;
    send(
        &mut output,
        &Frame::Ready {
            request_id: request_id.into(),
            session_id: session.clone(),
        },
        startup,
        &|| false,
    )?;
    let frame = receive(&mut input, total, &|| false)?;
    let Frame::Prompt {
        request_id: correlation,
        session_id: correlation_session,
        text,
    } = frame
    else {
        return Err("Expected one initial prompt frame".into());
    };
    if correlation != request_id
        || correlation_session != session
        || text.is_empty()
        || text.len() > MAX_PROMPT
    {
        return Err("Prompt correlation or size rejected".into());
    }
    let message = format!("msg_{request_id}");
    let ack = total.min(Instant::now() + Duration::from_secs(10));
    // Explicit metadata avoids default-model/provider discovery. It is not an enabled route.
    // The pinned noReply branch records before entering the inference loop.
    let _post = http(
        "POST",
        &format!("/session/{session}/message"),
        Some(json!({
            "messageID":message,"noReply":true,"model":{"providerID":"ollama","modelID":"smith-receipt-only"},
            "parts":[{"type":"text","text":text}]
        })),
        ack.min(Instant::now() + Duration::from_secs(5)),
        &input,
    );
    // Never POST again, including after a missing/rejected acknowledgement.
    let value = http(
        "GET",
        &format!("/session/{session}/message/{message}"),
        None,
        ack,
        &input,
    )?;
    if !recorded(&value, &session, &message, &text) {
        return Err("OpenCode receipt read-back mismatch".into());
    }
    // Stop the owned server before publishing a receipt. Container shutdown is separately verified by the host.
    child.0.kill().map_err(|_| "Cannot stop owned OpenCode")?;
    loop {
        if child
            .0
            .try_wait()
            .map_err(|_| "Cannot verify OpenCode stopped")?
            .is_some()
        {
            break;
        }
        if Instant::now() >= total {
            return Err("OpenCode stop unverified".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    send(
        &mut output,
        &Frame::Recorded {
            request_id: request_id.into(),
            session_id: session,
        },
        ack,
        &|| false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn framing_reads_complete_frame_without_pipe_eof() {
        let mut fds = [-1; 2];
        // SAFETY: pipe2 initializes two descriptors in the supplied two-element array.
        assert_eq!(unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) }, 0);
        // SAFETY: successful pipe2 returns fresh descriptors, each owned by one File.
        let reader = unsafe { File::from_raw_fd(fds[0]) };
        let mut writer = unsafe { File::from_raw_fd(fds[1]) };
        let mut input = duplicate_input(reader.as_raw_fd()).unwrap();
        drop(reader);
        writer.write_all(b"{\"type\":\"prompt\",\"request_id\":\"x\",\"session_id\":\"ses_x\",\"text\":\"hello\"}\n").unwrap();
        let frame = receive(&mut input, Instant::now() + Duration::from_secs(1), &|| {
            false
        })
        .unwrap();
        assert!(matches!(frame, Frame::Prompt { text, .. } if text == "hello"));
        assert!(!input_closed(&input));
        // Keep the writer open throughout receive: completing a frame must not require EOF.
        drop(writer);
    }

    #[test]
    fn protocol_rejects_uncorrelated_or_changed_receipts() {
        assert!(valid_id("0123456789abcdef0123456789abcdef"));
        assert!(!valid_id("../bad"));
        assert!(serde_json::from_str::<Frame>(
            r#"{"type":"recorded","request_id":"x","session_id":"ses_x","extra":1}"#
        )
        .is_err());
        let v = json!({"info":{"role":"user","id":"msg_x","sessionID":"ses_x"},"parts":[{"type":"text","text":"exact","sessionID":"ses_x","messageID":"msg_x"}]});
        assert!(recorded(&v, "ses_x", "msg_x", "exact"));
        assert!(!recorded(&v, "ses_x", "msg_x", "changed"));
        assert_eq!(
            decode_http(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}").unwrap(),
            json!({})
        );
        assert_eq!(
            decode_http(
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{}\r\n0\r\n\r\n"
            )
            .unwrap(),
            json!({})
        );
        assert!(decode_http(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n{}").is_err());
    }
}
