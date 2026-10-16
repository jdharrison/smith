//! Volatile installer input, never a report, argument or serialized DTO.
use crate::{consent, Result};
use std::fmt;
use std::os::fd::{AsRawFd, BorrowedFd};
use std::time::{Duration, Instant};

#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);
impl Secret {
    pub fn new(value: String) -> Result<Self> {
        let secret = Self(value);
        if secret.0.is_empty()
            || secret.0.len() > 4096
            || !secret.0.bytes().all(|b| (33..=126).contains(&b))
        {
            return Err("API key must contain 1–4096 printable ASCII bytes without whitespace; input was not saved".into());
        }
        Ok(secret)
    }
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([REDACTED])")
    }
}
impl Drop for Secret {
    fn drop(&mut self) {
        // Best effort only: GTK, JSON serialization, swap and same-UID access
        // are not covered by zeroing this owned input allocation.
        unsafe {
            for byte in self.0.as_bytes_mut() {
                std::ptr::write_volatile(byte, 0);
            }
        }
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    }
}

struct Terminal {
    fd: libc::c_int,
    original: libc::termios,
    flags: libc::c_int,
}
impl Terminal {
    fn enter(input: BorrowedFd<'_>) -> Result<Self> {
        let fd = input.as_raw_fd();
        // All pointers refer to initialized native termios objects; fd is borrowed
        // by the caller for the entire guarded read.
        unsafe {
            if libc::isatty(fd) != 1 {
                return Err("API key entry requires an interactive terminal; no pipe, flag or environment fallback".into());
            }
            let mut original: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(fd, &mut original) != 0 {
                return Err("Cannot inspect terminal for hidden API key input".into());
            }
            let flags = libc::fcntl(fd, libc::F_GETFL);
            if flags < 0 {
                return Err("Cannot bound hidden terminal input".into());
            }
            let guard = Self {
                fd,
                original,
                flags,
            };
            let mut hidden = original;
            hidden.c_lflag &= !(libc::ECHO | libc::ECHONL | libc::ICANON);
            hidden.c_cc[libc::VMIN] = 0;
            hidden.c_cc[libc::VTIME] = 0;
            if libc::tcsetattr(fd, libc::TCSAFLUSH, &hidden) != 0
                || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0
            {
                return Err("Cannot disable echo safely; API key input refused".into());
            }
            Ok(guard)
        }
    }
    fn restore(&self) -> Result<()> {
        // Flush unread secret bytes so overflow/interrupt cannot become consent.
        let (term, flags) = unsafe {
            (
                libc::tcsetattr(self.fd, libc::TCSAFLUSH, &self.original),
                libc::fcntl(self.fd, libc::F_SETFL, self.flags),
            )
        };
        if term != 0 || flags < 0 {
            Err("Cannot restore terminal; no key submission was approved".into())
        } else {
            Ok(())
        }
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

/// Separate from the ordinary echoing confirmation callback. Smaller deadlines
/// and cancellation callbacks are library-only pseudo-terminal test seams.
pub fn read_terminal(
    input: BorrowedFd<'_>,
    budget: Duration,
    stopped: &dyn Fn() -> bool,
) -> Result<Secret> {
    read_terminal_prompt(input, budget, stopped, &mut || Ok(()))
}

fn read_terminal_prompt(
    input: BorrowedFd<'_>,
    budget: Duration,
    stopped: &dyn Fn() -> bool,
    ready: &mut dyn FnMut() -> Result<()>,
) -> Result<Secret> {
    if budget > consent::INPUT_BUDGET || budget.is_zero() {
        return Err("Invalid secret input budget".into());
    }
    let terminal = Terminal::enter(input)?;
    let mut bytes = Secret(String::new());
    let deadline = Instant::now() + budget;
    // Invite input only after echo is disabled. Prompt failure restores the tty.
    ready()?;
    let result: Result<()> = (|| loop {
        if stopped() || consent::cancelled() {
            return Err("API key input interrupted; nothing submitted".into());
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or("API key input timed out; nothing submitted")?;
        let mut poll = libc::pollfd {
            fd: terminal.fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let ready = unsafe { libc::poll(&mut poll, 1, remaining.as_millis().clamp(1, 100) as i32) };
        if ready < 0 {
            if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err("Hidden API key input failed".into());
        }
        if ready == 0 {
            continue;
        }
        let mut byte = 0u8;
        let n = unsafe { libc::read(terminal.fd, std::ptr::addr_of_mut!(byte).cast(), 1) };
        if n < 0 {
            if matches!(
                std::io::Error::last_os_error().kind(),
                std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock
            ) {
                continue;
            }
            return Err("Hidden API key input failed".into());
        }
        if n == 0 {
            return Err("API key input ended; nothing submitted".into());
        }
        if stopped() || consent::cancelled() || Instant::now() >= deadline {
            return Err("API key input stopped or expired; nothing submitted".into());
        }
        match byte {
            b'\n' => {
                if bytes.0.is_empty() {
                    return Err("Empty API key; nothing submitted".into());
                }
                return Ok(());
            }
            8 | 127 => {
                bytes.0.pop();
            }
            33..=126 if bytes.0.len() < 4096 => bytes.0.push(char::from(byte)),
            _ => return Err("Invalid or oversized API key input; nothing submitted".into()),
        }
    })();
    let restored = terminal.restore();
    result?;
    restored?;
    Ok(bytes)
}

pub fn prompt() -> Result<Secret> {
    use std::os::fd::AsFd;
    let input = std::io::stdin();
    let result = read_terminal_prompt(
        input.as_fd(),
        consent::INPUT_BUDGET,
        &consent::cancelled,
        &mut || {
            consent::stdout_text(
                "API key (hidden, 60-second limit; not saved until separate approval): ",
            )
        },
    );
    let output = consent::stdout_text("\n");
    let secret = result?;
    output?;
    Ok(secret)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::os::fd::{AsFd, FromRawFd, OwnedFd};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    };

    fn pty() -> (std::fs::File, OwnedFd) {
        let (mut master, mut slave) = (-1, -1);
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null(),
                )
            },
            0
        );
        // Each successful openpty descriptor is transferred to exactly one owner.
        unsafe {
            (
                std::fs::File::from_raw_fd(master),
                OwnedFd::from_raw_fd(slave),
            )
        }
    }
    fn term(fd: BorrowedFd<'_>) -> libc::termios {
        let mut value = unsafe { std::mem::zeroed() };
        assert_eq!(unsafe { libc::tcgetattr(fd.as_raw_fd(), &mut value) }, 0);
        value
    }
    fn assert_restored(fd: BorrowedFd<'_>, before: &libc::termios, flags: i32) {
        let after = term(fd);
        assert_eq!(after.c_lflag, before.c_lflag);
        assert_eq!(after.c_cc, before.c_cc);
        assert_eq!(unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) }, flags);
    }
    fn input(bytes: Vec<u8>) -> Result<Secret> {
        let (mut master, slave) = pty();
        let before = term(slave.as_fd());
        let flags = unsafe { libc::fcntl(slave.as_raw_fd(), libc::F_GETFL) };
        let (ready, wait) = mpsc::sync_channel(1);
        std::thread::scope(|scope| {
            let writer = scope.spawn(move || {
                wait.recv_timeout(Duration::from_secs(2)).unwrap();
                master.write_all(&bytes).unwrap();
                master
            });
            let result = read_terminal_prompt(
                slave.as_fd(),
                Duration::from_secs(2),
                &|| false,
                &mut || {
                    let hidden = term(slave.as_fd());
                    assert_eq!(
                        hidden.c_lflag & (libc::ECHO | libc::ECHONL | libc::ICANON),
                        0
                    );
                    ready.send(()).unwrap();
                    Ok(())
                },
            );
            let mut master = writer.join().unwrap();
            assert_restored(slave.as_fd(), &before, flags);
            let flags = unsafe { libc::fcntl(master.as_raw_fd(), libc::F_GETFL) };
            assert_eq!(
                unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) },
                0
            );
            let mut echo = [0u8; 8192];
            assert_eq!(
                master.read(&mut echo).unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock,
                "No secret, newline or edit echo"
            );
            result
        })
    }
    #[test]
    fn hidden_input_is_bounded_redacted_and_restores_echo() {
        assert_eq!(
            input(b"synthetic-secret\x7fx\n".to_vec()).unwrap().expose(),
            "synthetic-secrex"
        );
        assert_eq!(
            input([vec![b'x'; 4096], vec![b'\n']].concat())
                .unwrap()
                .expose()
                .len(),
            4096
        );
        for bytes in [
            b"\n".to_vec(),
            b"not a key\n".to_vec(),
            b"\x04".to_vec(),
            vec![255],
            [vec![b'x'; 4097], vec![b'\n']].concat(),
        ] {
            assert!(input(bytes).is_err());
        }
        let secret = Secret::new("synthetic-secret".into()).unwrap();
        assert_eq!(format!("{secret:?}"), "Secret([REDACTED])");
        for text in ["", " key", "key\n", "nonascii-é"] {
            assert!(Secret::new(text.into()).is_err());
        }
    }
    #[test]
    fn timeout_cancel_prompt_failure_and_non_terminal_fail_closed() {
        let (_master, slave) = pty();
        let before = term(slave.as_fd());
        let flags = unsafe { libc::fcntl(slave.as_raw_fd(), libc::F_GETFL) };
        assert!(
            read_terminal(slave.as_fd(), Duration::from_millis(10), &|| false)
                .unwrap_err()
                .contains("timed out")
        );
        assert_restored(slave.as_fd(), &before, flags);
        let stop = AtomicBool::new(false);
        assert!(read_terminal_prompt(
            slave.as_fd(),
            Duration::from_secs(1),
            &|| stop.load(Ordering::Acquire),
            &mut || {
                stop.store(true, Ordering::Release);
                Ok(())
            }
        )
        .is_err());
        assert_restored(slave.as_fd(), &before, flags);
        assert!(read_terminal_prompt(
            slave.as_fd(),
            Duration::from_secs(1),
            &|| false,
            &mut || Err("Synthetic prompt write failure".into())
        )
        .is_err());
        assert_restored(slave.as_fd(), &before, flags);
        let (pipe, _peer) = std::os::unix::net::UnixStream::pair().unwrap();
        assert!(
            read_terminal(pipe.as_fd(), Duration::from_secs(1), &|| false)
                .unwrap_err()
                .contains("interactive terminal")
        );
        assert!(read_terminal(slave.as_fd(), Duration::ZERO, &|| false).is_err());
        assert!(read_terminal(slave.as_fd(), Duration::from_secs(61), &|| false).is_err());
    }
}
