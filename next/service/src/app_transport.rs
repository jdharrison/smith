use crate::app::{new_id, valid_id, AppError, AppReply, AppRequest, Application, FRAME_LIMIT};
use crate::filesystem;
use crate::transport::{connect, peer, socket_meta};
use crate::{Result, Roots};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
const DEADLINE: Duration = Duration::from_secs(2);

pub(crate) fn read_frame(stream: &mut UnixStream) -> Result<Vec<u8>> {
    read_frame_correlated(stream, &mut 1, &mut String::new())
}
fn read_frame_correlated(
    stream: &mut UnixStream,
    version: &mut u32,
    call: &mut String,
) -> Result<Vec<u8>> {
    let deadline = Instant::now() + DEADLINE;
    let mut bytes = Vec::new();
    let mut chunk = [0; 8192];
    loop {
        stream
            .set_read_timeout(Some(
                deadline
                    .checked_duration_since(Instant::now())
                    .ok_or("App frame deadline exceeded")?,
            ))
            .map_err(|_| "Cannot bound app read")?;
        let n = stream
            .read(&mut chunk)
            .map_err(|_| "App frame unavailable or timed out")?;
        if n == 0 {
            return Err("Truncated app frame".into());
        }
        if let Some(end) = chunk[..n].iter().position(|b| *b == b'\n') {
            if bytes.len() + end > FRAME_LIMIT {
                return Err("App frame limit rejected".into());
            }
            bytes.extend_from_slice(&chunk[..end]);
            correlate(&bytes, version, call);
            if end + 1 != n {
                return Err("Multiple app frames rejected".into());
            }
            return Ok(bytes);
        }
        if bytes.len() + n > FRAME_LIMIT {
            return Err("App frame exceeds limit".into());
        }
        bytes.extend_from_slice(&chunk[..n]);
    }
}
fn send<T: serde::Serialize>(stream: &mut UnixStream, value: &T) -> Result<()> {
    let mut bytes = serde_json::to_vec(value).map_err(|_| "Cannot encode app frame")?;
    if bytes.len() > FRAME_LIMIT {
        return Err("App reply exceeds limit".into());
    }
    bytes.push(b'\n');
    let deadline = Instant::now() + DEADLINE;
    let mut offset = 0;
    while offset < bytes.len() {
        stream
            .set_write_timeout(Some(
                deadline
                    .checked_duration_since(Instant::now())
                    .ok_or("App write deadline exceeded")?,
            ))
            .map_err(|_| "Cannot bound app write")?;
        let n = stream
            .write(&bytes[offset..])
            .map_err(|_| "App write failed")?;
        if n == 0 {
            return Err("App write closed".into());
        }
        offset += n;
    }
    Ok(())
}
pub(crate) fn request(roots: &Roots, pid: u32, request: &AppRequest) -> Result<AppReply> {
    roots.validate()?;
    request.validate().map_err(|e| e.message)?;
    if pid == 0 {
        return Err("Missing managed MainPID".into());
    }
    let path = roots.app_socket();
    let before = socket_meta(&path)?;
    let mut stream = connect(&path)?;
    let after = socket_meta(&path)?;
    let cred = peer(&stream)?;
    if before.dev() != after.dev()
        || before.ino() != after.ino()
        || cred.uid != filesystem::uid()
        || cred.pid <= 0
        || cred.pid as u32 != pid
    {
        return Err("App socket inode or peer conflicts with managed MainPID".into());
    }
    send(&mut stream, request)?;
    let reply: AppReply =
        serde_json::from_slice(&read_frame(&mut stream)?).map_err(|_| "Invalid app reply")?;
    if reply.schema_version != request.schema_version
        || reply.service != "smith-next"
        || reply.protocol != "app"
        || reply.pid != pid
        || reply.call_id != request.call_id
        || !valid_id(&reply.instance_id)
        || reply.ok != reply.error.is_none()
        || reply.ok != reply.result.is_some()
    {
        return Err("App reply identity or schema conflict".into());
    }
    Ok(reply)
}
struct Guard {
    path: std::path::PathBuf,
    dev: u64,
    ino: u64,
}
impl Drop for Guard {
    fn drop(&mut self) {
        if socket_meta(&self.path).is_ok_and(|m| m.dev() == self.dev && m.ino() == self.ino) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
pub(crate) struct Listener {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Listener {
    pub fn start(roots: &Roots) -> Result<Self> {
        let path = roots.app_socket();
        if std::fs::symlink_metadata(&path).is_ok() {
            return Err("Existing app socket/path preserved; explicit recovery required".into());
        }
        let listener = UnixListener::bind(&path).map_err(|_| "Cannot bind app socket")?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|_| "Cannot secure app socket")?;
        let meta = socket_meta(&path)?;
        let guard = Guard {
            path,
            dev: meta.dev(),
            ino: meta.ino(),
        };
        listener
            .set_nonblocking(true)
            .map_err(|_| "Cannot bound app listener")?;
        // Corrupt app state does not reinterpret or disable debug Core state.
        let app = Application::production(roots);
        let instance = app
            .as_ref()
            .map(|a| a.instance_id.clone())
            .unwrap_or(new_id()?);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();
        let thread=std::thread::Builder::new().name("smith-app-listener".into()).spawn(move || {
            let _guard=guard;
            while !stop_thread.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream,_))=> {
                        if !peer(&stream).is_ok_and(|c|c.uid==filesystem::uid()) { continue; }
                        let mut version=1; let mut call=String::new();
                                                let req=read_frame_correlated(&mut stream, &mut version, &mut call).and_then(|b| { serde_json::from_slice::<AppRequest>(&b).map_err(|_|"Invalid app request".into()) });
                        let (version,call,result)=match req {
                            Ok(req)=> { let version=req.schema_version; let call=req.call_id.clone(); let result=match &app { Ok(app)=>app.call(req),Err(_)=>Err(AppError::new("STATE_UNAVAILABLE","Application state is corrupt, unsupported or unsafe; preserved without reset")) }; (version,call,result) },
                            Err(_)=>(version,call,Err(AppError::new("INVALID_REQUEST","Invalid, oversized or incomplete app frame"))),
                        };
                        let _=send(&mut stream,&AppReply::from_versioned_result(version,&instance,&call,result));
                    }
                    Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>std::thread::sleep(Duration::from_millis(10)),
                    Err(e) if e.kind()==std::io::ErrorKind::Interrupted=>{},
                    Err(_)=>break,
                }
            }
        }).map_err(|_|"Cannot start app listener")?;
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
}
impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
/// Read one stdin JSON line with a finite deadline and a 4 MiB payload bound.
pub fn stdin_request() -> Result<AppRequest> {
    stdin_request_correlated(&mut 1, &mut String::new())
}
fn correlate(bytes: &[u8], version: &mut u32, call: &mut String) {
    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(bytes) {
        if let (Some(schema @ (1 | 2)), Some(id)) = (
            v.get("schema_version").and_then(serde_json::Value::as_u64),
            v.get("call_id").and_then(serde_json::Value::as_str),
        ) {
            if valid_id(id) {
                *version = schema as u32;
                *call = id.to_owned();
            }
        }
    }
}
/// Preserve recognized envelope correlation even when strict request decoding fails.
pub fn stdin_request_correlated(version: &mut u32, call: &mut String) -> Result<AppRequest> {
    use std::os::fd::AsRawFd;
    let stdin = std::io::stdin();
    let fd = stdin.as_raw_fd();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut bytes = Vec::new();
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or("App stdin deadline exceeded")?;
        let mut poll = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: poll addresses one valid stdin descriptor for a finite budget.
        if unsafe {
            libc::poll(
                &mut poll,
                1,
                remaining.as_millis().min(i32::MAX as u128) as i32,
            )
        } <= 0
        {
            return Err("App stdin unavailable or timed out".into());
        }
        let mut chunk = [0; 8192];
        // SAFETY: read writes at most the stack buffer size from the valid descriptor.
        let n = unsafe { libc::read(fd, chunk.as_mut_ptr().cast(), chunk.len()) };
        if n <= 0 {
            return Err("Truncated app stdin frame".into());
        }
        let n = n as usize;
        if let Some(end) = chunk[..n].iter().position(|b| *b == b'\n') {
            if bytes.len() + end > FRAME_LIMIT {
                return Err("App stdin frame limit rejected".into());
            }
            bytes.extend_from_slice(&chunk[..end]);
            correlate(&bytes, version, call);
            if end + 1 != n {
                return Err("Multiple app stdin frames rejected".into());
            }
            return serde_json::from_slice(&bytes).map_err(|_| "Invalid app stdin JSON".into());
        }
        if bytes.len() + n > FRAME_LIMIT {
            return Err("App stdin exceeds limit".into());
        }
        bytes.extend_from_slice(&chunk[..n]);
    }
}

/// Pipe-backed helper output is bounded too, not just its socket exchange.
pub fn stdout_reply(reply: &AppReply) -> Result<()> {
    let mut bytes = serde_json::to_vec(reply).map_err(|_| "Cannot encode app stdout")?;
    if bytes.len() > FRAME_LIMIT {
        return Err("App stdout exceeds limit".into());
    }
    bytes.push(b'\n');
    write_descriptor(libc::STDOUT_FILENO, &bytes)
}
fn write_descriptor(fd: i32, bytes: &[u8]) -> Result<()> {
    // SAFETY: fcntl operates on the inherited stdout descriptor without pointer arguments.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err("Cannot bound app stdout".into());
    }
    struct Restore(i32, i32);
    impl Drop for Restore {
        fn drop(&mut self) {
            // SAFETY: restore flags on the same inherited descriptor before returning.
            unsafe {
                libc::fcntl(self.0, libc::F_SETFL, self.1);
            }
        }
    }
    let _restore = Restore(fd, flags);
    let deadline = Instant::now() + DEADLINE;
    let mut offset = 0;
    while offset < bytes.len() {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or("App stdout deadline exceeded")?;
        let mut poll = libc::pollfd {
            fd,
            events: libc::POLLOUT,
            revents: 0,
        };
        // SAFETY: finite poll of one valid descriptor.
        if unsafe {
            libc::poll(
                &mut poll,
                1,
                remaining.as_millis().min(i32::MAX as u128) as i32,
            )
        } <= 0
        {
            return Err("App stdout timed out".into());
        }
        // SAFETY: the pointer covers precisely the remaining initialized bytes.
        let n = unsafe { libc::write(fd, bytes[offset..].as_ptr().cast(), bytes.len() - offset) };
        if n < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::WouldBlock {
            continue;
        }
        if n <= 0 {
            return Err("App stdout unavailable".into());
        }
        offset += n as usize;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn app_frames_accept_exact_4mib_payload_but_reject_overflow_and_multiple() {
        for (size, accepted) in [(FRAME_LIMIT, true), (FRAME_LIMIT + 1, false)] {
            let (mut writer, mut reader) = UnixStream::pair().unwrap();
            let thread = std::thread::spawn(move || {
                let mut bytes = vec![b'x'; size];
                bytes.push(b'\n');
                let _ = writer.write_all(&bytes);
            });
            let result = read_frame(&mut reader);
            assert_eq!(result.is_ok(), accepted);
            if accepted {
                assert_eq!(result.unwrap().len(), FRAME_LIMIT);
            }
            drop(reader);
            thread.join().unwrap();
        }
        let (mut writer, mut reader) = UnixStream::pair().unwrap();
        writer.write_all(b"{}\n{}\n").unwrap();
        assert!(read_frame(&mut reader).is_err());
    }
    #[test]
    fn blocked_helper_output_has_finite_budget_and_restores_flags() {
        use std::os::fd::AsRawFd;
        let (writer, _unread) = UnixStream::pair().unwrap();
        let fd = writer.as_raw_fd();
        // SAFETY: inspect flags on the owned test stream descriptor.
        let before = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        let started = Instant::now();
        assert!(write_descriptor(fd, &vec![b'x'; FRAME_LIMIT]).is_err());
        assert!(started.elapsed() < Duration::from_secs(3));
        // SAFETY: descriptor is still owned and open after the finite write attempt.
        assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFL) }, before);
    }
    #[test]
    fn complete_first_envelope_correlates_rejected_trailing_frames() {
        for suffix in ["\n", "{}\n"] {
            let (mut writer, mut reader) = UnixStream::pair().unwrap();
            let id = "1".repeat(32);
            let bytes = format!(
                "{}\n{suffix}",
                serde_json::json!({"schema_version":2,"service":"smith-next","protocol":"app","call_id":id,"op":"submit_new","args":{}})
            );
            writer.write_all(bytes.as_bytes()).unwrap();
            let mut version = 1;
            let mut call = String::new();
            assert!(read_frame_correlated(&mut reader, &mut version, &mut call).is_err());
            assert_eq!(version, 2);
            assert_eq!(call, id);
        }
    }
    #[test]
    fn truncated_app_frame_is_not_a_request() {
        let (mut writer, mut reader) = UnixStream::pair().unwrap();
        writer.write_all(b"{\"op\":\"models\"").unwrap();
        drop(writer);
        assert!(read_frame(&mut reader).unwrap_err().contains("Truncated"));
    }
}
