//! Explicit curated GGUF installation. Large files never enter the artifact
//! publisher's in-memory tree; all model publication is descriptor-relative.
use crate::{
    consent, fs,
    model::{ProgressOutput, Provider},
    Result,
};
use sha2::{Digest, Sha256};
use smith_service::{runner, Roots};
use std::cell::RefCell;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, RawFd};

use std::rc::Rc;
use std::time::{Duration, Instant};

const DOWNLOAD_BUDGET: Duration = Duration::from_secs(1800);
const IDLE: Duration = Duration::from_secs(30);
const CHUNK: usize = 64 * 1024;

pub struct Native;
impl Native {
    /// No filesystem, manager or network discovery on construction.
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}
impl Provider for Native {
    fn tags(&mut self) -> Result<Vec<String>> {
        let roots = Roots::environment()?;
        Ok(runner::list_models(&runner::models_dir(&roots))?
            .into_iter()
            .map(|m| m.name)
            .collect())
    }
    fn pull(&mut self, name: &str, output: &mut dyn ProgressOutput) -> Result<()> {
        let entry = entry(name)?;
        let roots = Roots::environment()?;
        let deadline = Instant::now() + DOWNLOAD_BUDGET;
        install_with(&roots, entry, output, deadline, |deadline| {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let executor = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| "Cannot create download executor")?;
            let client = reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .retry(reqwest::retry::never())
                .connect_timeout(Duration::from_secs(10))
                .timeout(remaining)
                .read_timeout(IDLE)
                .build()
                .map_err(|_| "Cannot create bounded model downloader")?;
            let mut url =
                reqwest::Url::parse(entry.url).map_err(|_| "Invalid curated model URL")?;
            for hop in 0..=3 {
                consent::check()?;
                check_url(&url)?;
                let response = await_until(&executor, deadline, || {
                    client
                        .get(url.clone())
                        .timeout(deadline.saturating_duration_since(Instant::now()))
                        .send()
                })?
                .map_err(|_| "Model download failed or timed out; no retry")?;
                if response.status().is_redirection() {
                    if hop == 3 {
                        return Err("Model redirect limit exceeded".into());
                    }
                    let location = response
                        .headers()
                        .get(reqwest::header::LOCATION)
                        .and_then(|v| v.to_str().ok())
                        .filter(|s| s.len() <= 16384)
                        .ok_or("Invalid model redirect")?;
                    url = url.join(location).map_err(|_| "Invalid model redirect")?;
                    continue;
                }
                if !response.status().is_success() {
                    return Err("Model server refused download".into());
                }
                if response
                    .content_length()
                    .is_some_and(|size| size != entry.size)
                {
                    return Err("Model download size differs from the curated catalog".into());
                }
                return Ok(Box::new(DownloadBody {
                    executor,
                    response,
                    pending: Vec::new(),
                    offset: 0,
                    deadline,
                }) as Box<dyn Read>);
            }
            Err("Model redirect limit exceeded".into())
        })
    }
    fn inference(&mut self, name: &str) -> Result<String> {
        consent::check()?;
        let entry = entry(name)?;
        let roots = Roots::environment()?;
        let deadline = Instant::now() + runner::BUDGET;
        let file =
            runner::verify_model_leased(&roots, &entry.record(), deadline, &consent::cancelled)?;
        let config = runner::RunnerConfig::production(
            &roots,
            runner::verify_runtime_bounded(&roots, deadline, &consent::cancelled)
                .map_err(|e| format!("{}: {}", e.code, e.message))?,
        );
        let mut child = runner::Runner::start_verified(
            &config,
            &file,
            deadline,
            std::sync::Arc::new(consent::cancelled),
        )?;
        let response = child
            .chat(
                &[smith_service::app::Message {
                    role: "user".into(),
                    content: "Reply with one short greeting.".into(),
                }],
                32,
                deadline,
            )
            .map_err(|e| format!("{}: {}", e.code, e.message));
        let stopped = child.stop();
        consent::check()?;
        let answer = response?;
        stopped?;
        if answer.len() > 4096 {
            return Err("Inference text exceeds installer limit".into());
        }
        Ok(answer)
    }
}

fn await_until<T, F>(
    executor: &tokio::runtime::Runtime,
    deadline: Instant,
    make_future: impl FnOnce() -> F,
) -> Result<T>
where
    F: std::future::Future<Output = T>,
{
    use std::future::Future;
    use std::task::Poll;
    executor.block_on(async {
        // reqwest::send creates Tokio timeout timers before its first poll.
        // Construct the future here, after block_on has entered the owned runtime.
        within(deadline)?;
        let mut task = Box::pin(make_future());
        let mut tick = Box::pin(tokio::time::sleep(Duration::from_millis(100)));
        std::future::poll_fn(|cx| {
            if let Err(error) = within(deadline) {
                return Poll::Ready(Err(error));
            }
            if let Poll::Ready(value) = task.as_mut().poll(cx) {
                return Poll::Ready(Ok(value));
            }
            if tick.as_mut().poll(cx).is_ready() {
                tick.as_mut()
                    .reset(tokio::time::Instant::now() + Duration::from_millis(100));
            }
            Poll::Pending
        })
        .await
    })
}
struct DownloadBody {
    executor: tokio::runtime::Runtime,
    response: reqwest::Response,
    pending: Vec<u8>,
    offset: usize,
    deadline: Instant,
}
impl Read for DownloadBody {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        if self.offset == self.pending.len() {
            let chunk = await_until(&self.executor, self.deadline, || self.response.chunk())
                .map_err(std::io::Error::other)?
                .map_err(std::io::Error::other)?;
            let Some(chunk) = chunk else {
                return Ok(0);
            };
            if chunk.len() > 1024 * 1024 {
                return Err(std::io::Error::other("Transfer chunk exceeds bound"));
            }
            self.pending = chunk.to_vec();
            self.offset = 0;
        }
        let n = output.len().min(self.pending.len() - self.offset);
        output[..n].copy_from_slice(&self.pending[self.offset..self.offset + n]);
        self.offset += n;
        Ok(n)
    }
}

pub fn entry(name: &str) -> Result<&'static runner::CatalogEntry> {
    runner::CATALOG
        .iter()
        .find(|entry| entry.name == name)
        .ok_or_else(|| "Choose an exact curated model from --models".into())
}
fn check_url(url: &reqwest::Url) -> Result<()> {
    let host = url.host_str().ok_or("Model URL has no host")?;
    fs::require(
        url.scheme() == "https"
            && url.port_or_known_default() == Some(443)
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
            && (host == "huggingface.co" || host.ends_with(".hf.co")),
        "Model download endpoint is not an approved HTTPS host",
    )
}
fn within(deadline: Instant) -> Result<()> {
    consent::check()?;
    fs::require(
        Instant::now() < deadline,
        "Model download deadline exceeded; no retry",
    )
}
fn safe_file(fd: RawFd, size: u64, mode: u32) -> Result<libc::stat> {
    let info = fs::fstat(fd)?;
    fs::require(
        info.st_mode & libc::S_IFMT == libc::S_IFREG
            && info.st_uid == fs::effective_uid()
            && info.st_nlink == 1
            && info.st_mode & 0o7777 == mode
            && info.st_size >= 0
            && info.st_size as u64 == size,
        "Unsafe model file type, owner, mode, link count or size",
    )?;
    Ok(info)
}
fn verify_at(
    dir: RawFd,
    name: &str,
    entry: &runner::CatalogEntry,
    deadline: Instant,
) -> Result<bool> {
    let fd = match fs::open_at(
        dir,
        name,
        libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        0,
    ) {
        Ok(fd) => fd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err("Existing model is unsafe; no overwrite".into()),
    };
    let before = safe_file(fd.as_raw_fd(), entry.size, 0o600)?;
    let mut file = File::from(fd);
    let mut hash = Sha256::new();
    let mut buffer = vec![0; CHUNK];
    let mut size = 0u64;
    loop {
        within(deadline)?;
        let n = file
            .read(&mut buffer)
            .map_err(|_| "Cannot verify existing model")?;
        if n == 0 {
            break;
        }
        size += n as u64;
        fs::require(size <= entry.size, "Model grew during verification")?;
        hash.update(&buffer[..n]);
    }
    let after = safe_file(file.as_raw_fd(), entry.size, 0o600)?;
    let named = fs::open_at(
        dir,
        name,
        libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        0,
    )
    .map_err(|_| "Model path changed during verification")?;
    let current = safe_file(named.as_raw_fd(), entry.size, 0o600)?;
    let same = |a: &libc::stat, b: &libc::stat| {
        a.st_dev == b.st_dev
            && a.st_ino == b.st_ino
            && a.st_mtime == b.st_mtime
            && a.st_mtime_nsec == b.st_mtime_nsec
            && a.st_ctime == b.st_ctime
            && a.st_ctime_nsec == b.st_ctime_nsec
    };
    fs::require(
        same(&before, &after)
            && same(&before, &current)
            && size == entry.size
            && format!("{:x}", hash.finalize()) == entry.sha256,
        "Existing model differs from catalog; preserved, no overwrite",
    )?;
    Ok(true)
}

/// Library-only transport/catalog seam. The executable cannot override URLs or hashes.
pub fn install_with<F>(
    roots: &Roots,
    entry: &runner::CatalogEntry,
    output: &mut dyn ProgressOutput,
    deadline: Instant,
    mut fetch: F,
) -> Result<()>
where
    F: FnMut(Instant) -> Result<Box<dyn Read>>,
{
    within(deadline)?;
    fs::require(
        entry.sha256.len() == 64
            && entry
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            && entry.size > 0
            && entry.size <= 16 * 1024 * 1024 * 1024,
        "Invalid model publication identity or size",
    )?;
    roots.validate()?;
    let effects = Rc::new(RefCell::new(Vec::new()));
    let mut filesystem = fs::Filesystem::new(effects);
    let data = filesystem
        .directory(&roots.data, false, false)?
        .ok_or("Existing data root required")?;
    let lock_fd = fs::open_at(
        data.as_raw_fd(),
        ".smith-next-install.lock",
        libc::O_RDWR | libc::O_CREAT | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
        0o600,
    )
    .map_err(|_| "Cannot open installation lock")?;
    let captured_lock = fs::verify_lock(lock_fd.as_raw_fd())?;
    // SAFETY: the held descriptor outlives all publication. No waiting/retry.
    fs::require(
        unsafe { libc::flock(lock_fd.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
        "Another installation is active; no wait or retry",
    )?;
    struct Lock(std::os::fd::OwnedFd);
    impl Drop for Lock {
        fn drop(&mut self) {
            unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
        }
    }
    let lock = Lock(lock_fd);
    let models = runner::models_dir(roots);
    let dir = filesystem
        .directory(&models, true, true)?
        .ok_or("Cannot create private model store")?;
    let name = format!("{}.gguf", entry.sha256);
    let marker = models.join(format!("{}.json", entry.sha256));
    let expected_marker = fs::FileData {
        bytes: runner::marker_bytes(&entry.record()),
        mode: 0o600,
    };
    if let Some(existing) = filesystem.file(&marker, Some(0o600))? {
        fs::require(
            existing == expected_marker,
            "Existing model marker conflicts; preserved",
        )?;
        fs::require(
            verify_at(dir.as_raw_fd(), &name, entry, deadline)?,
            "Model marker has no file; preserved",
        )?;
        return Ok(());
    }
    if !verify_at(dir.as_raw_fd(), &name, entry, deadline)? {
        let mut space = std::mem::MaybeUninit::<libc::statvfs>::uninit();
        // SAFETY: statvfs initializes the correctly sized output on success.
        fs::require(
            unsafe { libc::fstatvfs(dir.as_raw_fd(), space.as_mut_ptr()) } == 0,
            "Cannot check model storage capacity",
        )?;
        let space = unsafe { space.assume_init() };
        let available = (space.f_bavail as u128) * (space.f_frsize as u128);
        fs::require(
            available >= entry.size as u128 + 64 * 1024 * 1024,
            "Insufficient model storage; allow model size plus 64 MiB",
        )?;
        let stage = fs::stage_name(".smith-model-stage-")?;
        let staged_path = models.join(&stage);
        let result = (|| {
            let fd = fs::open_at(
                dir.as_raw_fd(),
                &stage,
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
            .map_err(|_| "Cannot create private model staging file")?;
            let mut file = File::from(fd);
            let mut source = fetch(deadline)?;
            let mut hash = Sha256::new();
            let mut buffer = vec![0; CHUNK];
            let mut total = 0u64;
            let mut reported = 0u64;
            loop {
                within(deadline)?;
                let n = source
                    .read(&mut buffer)
                    .map_err(|_| "Model transfer failed or timed out")?;
                within(deadline)?;
                if n == 0 {
                    break;
                }
                total = total
                    .checked_add(n as u64)
                    .ok_or("Model transfer size overflow")?;
                fs::require(
                    total <= entry.size,
                    "Model transfer exceeds exact catalog size",
                )?;
                file.write_all(&buffer[..n])
                    .map_err(|_| "Model staging write failed")?;
                hash.update(&buffer[..n]);
                if total - reported >= 4 * 1024 * 1024 || total == entry.size {
                    output.numeric_until(total, entry.size, deadline, consent::cancelled)?;
                    reported = total;
                }
            }
            fs::require(
                total == entry.size && format!("{:x}", hash.finalize()) == entry.sha256,
                "Model checksum or size mismatch; staging preserved",
            )?;
            safe_file(file.as_raw_fd(), entry.size, 0o600)?;
            file.sync_all().map_err(|_| "Cannot sync staged model")?;
            within(deadline)?;
            fs::require(
                fs::verify_lock(lock.0.as_raw_fd())? == captured_lock,
                "Installation lock changed",
            )?;
            let named_lock = fs::open_at(
                data.as_raw_fd(),
                ".smith-next-install.lock",
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0,
            )
            .map_err(|_| "Installation lock path changed")?;
            fs::require(
                fs::verify_lock(named_lock.as_raw_fd())? == captured_lock,
                "Installation lock path changed",
            )?;
            let named_dir = filesystem
                .directory(&models, false, true)?
                .ok_or("Model store disappeared")?;
            let a = fs::fstat(dir.as_raw_fd())?;
            let b = fs::fstat(named_dir.as_raw_fd())?;
            fs::require(
                a.st_dev == b.st_dev && a.st_ino == b.st_ino,
                "Model store path changed",
            )?;
            let staged = fs::open_at(
                dir.as_raw_fd(),
                &stage,
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0,
            )
            .map_err(|_| "Model staging path changed")?;
            let a = fs::fstat(file.as_raw_fd())?;
            let b = safe_file(staged.as_raw_fd(), entry.size, 0o600)?;
            fs::require(
                a.st_dev == b.st_dev && a.st_ino == b.st_ino,
                "Model staging path changed",
            )?;
            fs::rename_at(
                dir.as_raw_fd(),
                &stage,
                dir.as_raw_fd(),
                &name,
                libc::RENAME_NOREPLACE,
                &models.join(&name),
            )?;
            fs::sync(dir.as_raw_fd(), &models)?;
            Ok(())
        })();
        result.map_err(|e: String| {
            format!(
                "{e}; staging/accepted files may remain at {}; no rollback",
                staged_path.display()
            )
        })?;
    }
    within(deadline)?;
    // Completion marker comes last; a failed marker publication cannot make a
    // partially transferred model eligible. Valid orphan files can be adopted on
    // a later separately consented download, without downloading again.
    fs::require(
        verify_at(dir.as_raw_fd(), &name, entry, deadline)?,
        "Verified model file disappeared before completion",
    )?;
    filesystem.publish(&marker, &expected_marker.bytes, expected_marker.mode)?;
    fs::sync(dir.as_raw_fd(), &models)?;
    output.write_until(
        b"Model verified and installed; inference not yet tested.\n",
        deadline,
        consent::cancelled,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_redirect_hop_requires_exact_https_host_policy() {
        for url in [
            "https://huggingface.co/path",
            "https://us.aws.cdn.hf.co/path?public=token",
            "https://cdn-lfs.hf.co/path",
        ] {
            assert!(check_url(&reqwest::Url::parse(url).unwrap()).is_ok());
        }
        for url in [
            "http://huggingface.co/path",
            "https://huggingface.co:444/path",
            "https://huggingface.co.attacker.example/path",
            "https://evil-hf.co/path",
            "https://user:password@huggingface.co/path",
            "https://127.0.0.1/path",
            "https://huggingface.co/path#fragment",
        ] {
            assert!(
                check_url(&reqwest::Url::parse(url).unwrap()).is_err(),
                "{url}"
            );
        }
    }
    #[test]
    fn synchronous_worker_downloads_synthetic_http_body_without_ambient_runtime() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        assert!(tokio::runtime::Handle::try_current().is_err());
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/fixture", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let end = Instant::now() + Duration::from_secs(3);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if Instant::now() >= end {
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(e) => panic!("Synthetic server failed: {e}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let mut chunk = [0; 1024];
                let n = stream.read(&mut chunk).unwrap();
                assert!(n > 0 && request.len() + n <= 8192);
                request.extend_from_slice(&chunk[..n]);
            }
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 18\r\nConnection: close\r\n\r\nsynthetic transfer").unwrap();
        });
        let executor = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(2))
            .read_timeout(IDLE)
            .build()
            .unwrap();
        let response = await_until(&executor, deadline, || {
            client.get(url).timeout(Duration::from_secs(2)).send()
        })
        .unwrap()
        .unwrap();
        let mut body = DownloadBody {
            executor,
            response,
            pending: Vec::new(),
            offset: 0,
            deadline,
        };
        let mut bytes = Vec::new();
        body.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"synthetic transfer");
        server.join().unwrap();
        assert!(tokio::runtime::Handle::try_current().is_err());
    }

    #[test]
    fn expired_download_wait_does_not_construct_a_request() {
        let executor = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let constructed = std::cell::Cell::new(false);
        assert!(await_until(&executor, Instant::now(), || {
            constructed.set(true);
            std::future::ready(())
        })
        .is_err());
        assert!(!constructed.get());
    }

    #[test]
    fn async_download_wait_has_absolute_deadline_and_no_abandoned_task() {
        let executor = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let start = Instant::now();
        assert!(await_until(
            &executor,
            start + Duration::from_millis(20),
            std::future::pending::<()>
        )
        .is_err());
        assert!(start.elapsed() < Duration::from_secs(1));
    }
}
