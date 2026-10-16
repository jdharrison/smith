use crate::filesystem::{self, Lock};
use crate::{Result, Roots};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const FRAME: usize = 4096;
const DEADLINE: Duration = Duration::from_secs(2);
static STOP: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BridgeReply {
    pub schema_version: u32,
    pub service: String,
    pub pid: u32,
    pub ok: bool,
    pub setup_complete: Option<bool>,
    pub error: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema_version: u32,
    service: String,
    command: String,
}

pub(crate) fn peer(stream: &UnixStream) -> Result<libc::ucred> {
    let mut cred = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: pointers are valid for the specified ucred size.
    let rc = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut cred as *mut libc::ucred).cast(),
            &mut len,
        )
    };
    if rc != 0 || len as usize != std::mem::size_of::<libc::ucred>() {
        return Err("Cannot verify socket peer".into());
    }
    Ok(cred)
}

fn frame<T: serde::de::DeserializeOwned>(stream: &mut UnixStream) -> Result<T> {
    let deadline = Instant::now() + DEADLINE;
    let mut bytes = Vec::new();
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or("Socket frame deadline exceeded")?;
        stream
            .set_read_timeout(Some(remaining))
            .map_err(|_| "Cannot bound socket read")?;
        let mut b = [0];
        if stream
            .read(&mut b)
            .map_err(|_| "Socket frame unavailable or timed out")?
            != 1
        {
            return Err("Truncated socket frame".into());
        }
        if b[0] == b'\n' {
            break;
        }
        bytes.push(b[0]);
        if bytes.len() >= FRAME {
            return Err("Socket frame exceeds limit".into());
        }
    }
    serde_json::from_slice(&bytes).map_err(|_| "Invalid socket frame".into())
}
fn send<T: Serialize>(stream: &mut UnixStream, value: &T) -> Result<()> {
    let mut bytes = serde_json::to_vec(value).map_err(|_| "Cannot encode frame")?;
    if bytes.len() >= FRAME {
        return Err("Socket response exceeds limit".into());
    }
    bytes.push(b'\n');
    stream
        .set_write_timeout(Some(DEADLINE))
        .map_err(|_| "Cannot bound socket write")?;
    stream
        .write_all(&bytes)
        .map_err(|_| "Socket write failed".into())
}

pub(crate) fn socket_meta(path: &Path) -> Result<fs::Metadata> {
    filesystem::check_ancestors(path)?;
    let meta = fs::symlink_metadata(path).map_err(|_| "Socket unavailable")?;
    if !meta.file_type().is_socket()
        || meta.uid() != filesystem::uid()
        || meta.mode() & 0o7777 != 0o600
    {
        return Err("Socket type, owner or permissions conflict".into());
    }
    Ok(meta)
}

pub(crate) fn connect(path: &Path) -> Result<UnixStream> {
    use std::os::unix::ffi::OsStrExt;
    let bytes = path.as_os_str().as_bytes();
    // SAFETY: zero is a valid initial sockaddr_un representation.
    let mut addr: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    if bytes.len() >= addr.sun_path.len() {
        return Err("Socket path exceeds Unix address limit".into());
    }
    addr.sun_family = libc::AF_UNIX as _;
    for (slot, b) in addr.sun_path.iter_mut().zip(bytes) {
        *slot = *b as _;
    }
    // SAFETY: socket creates a new owned descriptor, wrapped immediately below.
    let fd = unsafe {
        libc::socket(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
            0,
        )
    };
    if fd < 0 {
        return Err("Cannot create socket".into());
    }
    // SAFETY: fd is exclusively owned and represents an AF_UNIX stream.
    let stream = unsafe { UnixStream::from_raw_fd(fd) };
    // SAFETY: addr is valid for the supplied sockaddr_un length.
    let rc = unsafe {
        libc::connect(
            fd,
            (&addr as *const libc::sockaddr_un).cast(),
            std::mem::size_of_val(&addr) as _,
        )
    };
    if rc != 0 {
        let error = std::io::Error::last_os_error().raw_os_error();
        if error != Some(libc::EINPROGRESS) {
            return Err("Socket connect unavailable".into());
        }
        let mut poll = libc::pollfd {
            fd,
            events: libc::POLLOUT,
            revents: 0,
        };
        // SAFETY: poll has one valid descriptor entry and finite deadline.
        if unsafe { libc::poll(&mut poll, 1, DEADLINE.as_millis() as i32) } <= 0
            || stream
                .take_error()
                .map_err(|_| "Socket status failed")?
                .is_some()
        {
            return Err("Socket connection timed out or failed".into());
        }
    }
    stream
        .set_nonblocking(false)
        .map_err(|_| "Cannot configure socket")?;
    Ok(stream)
}

/// Caller must first verify managed unit and binary facts; PID comes from MainPID.
pub fn request(roots: &Roots, expected_pid: u32, command: &str) -> Result<BridgeReply> {
    roots.validate()?;
    if expected_pid == 0 || !["identity", "get_state", "complete_setup"].contains(&command) {
        return Err("Invalid verified bridge request".into());
    }
    let before = socket_meta(&roots.socket())?;
    let mut stream = connect(&roots.socket())?;
    let after = socket_meta(&roots.socket())?;
    let cred = peer(&stream)?;
    if before.dev() != after.dev()
        || before.ino() != after.ino()
        || cred.uid != filesystem::uid()
        || cred.pid <= 0
        || cred.pid as u32 != expected_pid
    {
        return Err("Socket peer UID/PID or inode conflicts with managed runtime".into());
    }
    send(
        &mut stream,
        &Request {
            schema_version: 1,
            service: "smith-next".into(),
            command: command.into(),
        },
    )?;
    let reply: BridgeReply = frame(&mut stream)?;
    if reply.schema_version != 1
        || reply.service != "smith-next"
        || reply.pid != expected_pid
        || reply.ok != reply.error.is_none()
        || (reply.ok && reply.setup_complete.is_none())
    {
        return Err("Bridge identity or response schema conflict".into());
    }
    if !reply.ok {
        return Err(reply.error.unwrap_or_else(|| "Bridge failed".into()));
    }
    Ok(reply)
}

extern "C" fn stop(_: libc::c_int) {
    STOP.store(true, Ordering::Relaxed);
}
struct SocketGuard {
    path: std::path::PathBuf,
    dev: u64,
    ino: u64,
}
impl Drop for SocketGuard {
    fn drop(&mut self) {
        if let Ok(meta) = socket_meta(&self.path) {
            if meta.dev() == self.dev && meta.ino() == self.ino {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}

pub fn serve(roots: &Roots) -> Result<()> {
    roots.validate()?;
    filesystem::check_directory(&roots.data.join("smith-next"), true)?;
    filesystem::state(roots)?;
    filesystem::mkdir(&roots.runtime.join("smith-next"))?;
    let _lock = Lock::acquire(&roots.runtime.join("smith-next"))?;
    if fs::symlink_metadata(roots.socket()).is_ok() {
        return Err("Existing socket/path preserved; explicit recovery required".into());
    }
    STOP.store(false, Ordering::Relaxed);
    // SAFETY: handlers only perform a lock-free atomic store; no allocation or I/O.
    unsafe {
        libc::signal(libc::SIGTERM, stop as *const () as libc::sighandler_t);
        libc::signal(libc::SIGINT, stop as *const () as libc::sighandler_t);
    }
    let listener = UnixListener::bind(roots.socket()).map_err(|_| "Cannot bind debug socket")?;
    fs::set_permissions(roots.socket(), fs::Permissions::from_mode(0o600))
        .map_err(|_| "Cannot secure socket")?;
    let meta = socket_meta(&roots.socket())?;
    let _guard = SocketGuard {
        path: roots.socket(),
        dev: meta.dev(),
        ino: meta.ino(),
    };
    listener
        .set_nonblocking(true)
        .map_err(|_| "Cannot bound listener")?;
    let _app_listener = crate::app_transport::Listener::start(roots)?;
    while !STOP.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                if !peer(&stream).is_ok_and(|c| c.uid == filesystem::uid()) {
                    continue;
                }
                let response = (|| {
                    let req: Request = frame(&mut stream)?;
                    if req.schema_version != 1 || req.service != "smith-next" {
                        return Err("Unsupported protocol identity".into());
                    }
                    match req.command.as_str() {
                        "identity" | "get_state" => {}
                        "complete_setup" => filesystem::complete(roots)?,
                        _ => return Err("Unsupported debug command".into()),
                    }
                    filesystem::state(roots)
                })();
                let (setup_complete, error) = match response {
                    Ok(v) => (Some(v), None),
                    Err(e) => (None, Some(e)),
                };
                let _ = send(
                    &mut stream,
                    &BridgeReply {
                        schema_version: 1,
                        service: "smith-next".into(),
                        pid: std::process::id(),
                        ok: error.is_none(),
                        setup_complete,
                        error,
                    },
                );
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Err("Listener failed".into()),
        }
    }
    Ok(())
}
