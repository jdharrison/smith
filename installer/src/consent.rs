//! Bounded operator input and cooperative cancellation shared with Core control.
use crate::model::{FdProgress, ProgressOutput};
use crate::Result;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

pub const INPUT_LIMIT: usize = 4096;
pub const INPUT_BUDGET: Duration = Duration::from_secs(60);
pub const CONSOLE_BUDGET: Duration = Duration::from_secs(2);
pub const REPORT_BUDGET: Duration = Duration::from_millis(250);
const CONSOLE_LIMIT: usize = 128 * 1024;
// Smaller than the model progress sink's bounded message size. Normal console
// text may be longer; all chunks share ONE deadline, not one budget per chunk.
const CONSOLE_CHUNK: usize = 1024;
static CANCELLED: AtomicBool = AtomicBool::new(false);
static CORE_INT: AtomicUsize = AtomicUsize::new(0);
static CORE_TERM: AtomicUsize = AtomicUsize::new(0);
static HANDLERS: OnceLock<Result<()>> = OnceLock::new();

extern "C" fn cancel(signal: libc::c_int) {
    CANCELLED.store(true, Ordering::Relaxed);
    let address = match signal {
        libc::SIGINT => CORE_INT.load(Ordering::Acquire),
        libc::SIGTERM => CORE_TERM.load(Ordering::Acquire),
        _ => return,
    };
    if address > 1 {
        // SAFETY: installation captures only Core's one-argument, atomic-only
        // cancellation handler, not an arbitrary prior handler or SA_SIGINFO.
        let handler: extern "C" fn(libc::c_int) = unsafe { std::mem::transmute(address) };
        handler(signal);
    }
}

/// Install once, on the main thread BEFORE any provider/manager threads exist.
/// Core keeps its own cancellation flag private. Capturing its handlers while
/// both signals are blocked lets one signal notify both flags without changing
/// Core, overwriting its cleanup behavior, or starting an effect-capable thread.
pub fn install_cancellation() -> Result<()> {
    HANDLERS.get_or_init(install_handlers).clone()
}

fn install_handlers() -> Result<()> {
    // SAFETY: every sigset/sigaction is initialized before use. Signals remain
    // blocked on this sole startup thread until both captured pointers and both
    // composed handlers are installed. The handlers use only lock-free atomics.
    unsafe {
        let mut blocked: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut blocked);
        libc::sigaddset(&mut blocked, libc::SIGINT);
        libc::sigaddset(&mut blocked, libc::SIGTERM);
        let mut previous_mask: libc::sigset_t = std::mem::zeroed();
        if libc::pthread_sigmask(libc::SIG_BLOCK, &blocked, &mut previous_mask) != 0 {
            return Err("Cannot block signals for cancellation installation".into());
        }
        let result = (|| {
            smith_service::install_control_cancellation()?;
            for (signal, slot) in [(libc::SIGINT, &CORE_INT), (libc::SIGTERM, &CORE_TERM)] {
                let mut core: libc::sigaction = std::mem::zeroed();
                if libc::sigaction(signal, std::ptr::null(), &mut core) != 0
                    || core.sa_flags & libc::SA_SIGINFO != 0
                    || core.sa_sigaction <= 1
                {
                    return Err("Cannot capture Core cancellation handler".into());
                }
                slot.store(core.sa_sigaction, Ordering::Release);
            }
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = cancel as *const () as libc::sighandler_t;
            action.sa_mask = blocked;
            for signal in [libc::SIGINT, libc::SIGTERM] {
                if libc::sigaction(signal, &action, std::ptr::null_mut()) != 0 {
                    return Err("Cannot install installer cancellation handler".into());
                }
            }
            Ok(())
        })();
        if result.is_err() {
            CANCELLED.store(true, Ordering::Relaxed);
        }
        if libc::pthread_sigmask(libc::SIG_SETMASK, &previous_mask, std::ptr::null_mut()) != 0 {
            CANCELLED.store(true, Ordering::Relaxed);
            return Err("Cannot restore signal mask; installation stopped".into());
        }
        result
    }
}

/// Request the SAME composed installer/Core cancellation as SIGTERM, without
/// signalling an unrelated process or starting a helper. Install the handlers on
/// the main thread before GTK/workers (Session::spawn also checks installation).
/// Cancellation is terminal for this process and is never reset for a retry.
pub fn request_cancel() {
    cancel(libc::SIGTERM);
}

/// Also used by the model transport's bounded polling loops.
pub fn cancelled() -> bool {
    CANCELLED.load(Ordering::Relaxed)
}

pub fn check() -> Result<()> {
    if cancelled() {
        Err("Installer interrupted; accepted effects may require fresh inspection".into())
    } else {
        Ok(())
    }
}

pub fn matches(answer: &str, exact: &str) -> Result<bool> {
    check()?;
    if answer.len() >= INPUT_LIMIT || answer.contains(['\n', '\0', '\u{3}']) {
        return Err("Invalid or oversized consent line".into());
    }
    Ok(answer.trim() == exact)
}

/// Production stdout: use only the peer's nonblocking descriptor implementation.
/// Each write owns the shared descriptor flags only for that write. In particular
/// no FdProgress stays alive across the separately owned publisher's stdio calls.
/// There is no buffered flush, arbitrary Write adapter, or background writer.
pub struct StdoutConsole;
impl ProgressOutput for StdoutConsole {
    fn write_until(
        &mut self,
        bytes: &[u8],
        deadline: Instant,
        cancelled: fn() -> bool,
    ) -> Result<()> {
        let mut output = FdProgress::stdout()?;
        let result = output.write_until(bytes, deadline, cancelled);
        finish_output(result, output.finish())
    }
}

fn finish_output(result: Result<()>, restored: Result<()>) -> Result<()> {
    match (result, restored) {
        (Err(error), Err(restoration)) => Err(format!("{error}; {restoration}")),
        (Err(error), _) => Err(error),
        (Ok(()), restored) => restored,
    }
}

/// One setup call owns this console. A failed model-progress write poisons the
/// sink so terminal reporting cannot re-enter the same full/failed stdout pipe.
/// Other model failures get a single short reporting deadline, owned by setup.
pub struct Console<'a> {
    output: &'a mut dyn ProgressOutput,
    failed: bool,
}
impl<'a> Console<'a> {
    pub fn new(output: &'a mut dyn ProgressOutput) -> Self {
        Self {
            output,
            failed: false,
        }
    }
    pub fn text_until(&mut self, text: &str, deadline: Instant) -> Result<()> {
        if text.len() > CONSOLE_LIMIT {
            self.failed = true;
            return Err("Console message exceeds bound".into());
        }
        for bytes in text.as_bytes().chunks(CONSOLE_CHUNK) {
            self.write_until(bytes, deadline, cancelled)?;
        }
        Ok(())
    }
    pub fn line_until(&mut self, text: &str, deadline: Instant) -> Result<()> {
        self.text_until(text, deadline)?;
        self.write_until(b"\n", deadline, cancelled)
    }
    pub fn line(&mut self, text: &str) -> Result<()> {
        self.line_until(text, Instant::now() + CONSOLE_BUDGET)
    }
}
impl ProgressOutput for Console<'_> {
    fn write_until(
        &mut self,
        bytes: &[u8],
        deadline: Instant,
        cancelled: fn() -> bool,
    ) -> Result<()> {
        if self.failed {
            return Err(
                "Console output unavailable; report may be incomplete; no rollback inferred".into(),
            );
        }
        // Preserve the model's exact absolute deadline and cancellation function.
        // Do not renew a failed stage's budget or weaken its progress contract.
        let result = self.output.write_until(bytes, deadline, cancelled);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
}

pub fn stdout_text(text: &str) -> Result<()> {
    Console::new(&mut StdoutConsole).text_until(text, Instant::now() + CONSOLE_BUDGET)
}

/// Best-effort production stderr, also bounded if redirected to a full pipe.
/// Failure/partial delivery cannot change the caller's nonzero exit status.
pub fn diagnostic(text: &str) -> Result<()> {
    let deadline = Instant::now() + REPORT_BUDGET;
    check()?;
    let stderr = std::io::stderr();
    let mut output = FdProgress::from_fd(stderr.as_fd())?;
    let result = Console::new(&mut output).line_until(text, deadline);
    finish_output(result, output.finish())
}

pub fn prompt(message: &str) -> Result<String> {
    check()?;
    stdout_text(message).map_err(|_| "Cannot display consent; no approval accepted")?;
    let stdin = std::io::stdin();
    read_line(stdin.as_fd(), INPUT_BUDGET)
}

/// Read one line, including its newline, in at most 4096 bytes/60 seconds.
/// Smaller budgets are a library test seam, never an executable override. Raw
/// nonblocking reads avoid stdio read-ahead and bound an actual slow-drip read.
/// No background reader survives EOF, cancellation, overflow or timeout.
pub fn read_line(input: BorrowedFd<'_>, budget: Duration) -> Result<String> {
    check()?;
    if budget > INPUT_BUDGET {
        return Err("Consent budget cannot exceed 60 seconds".into());
    }
    let fd = input.as_raw_fd();
    // SAFETY: input remains borrowed/open for this call; fcntl has no pointers.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err("Cannot bound consent input".into());
    }
    let result = read_nonblocking(fd, Instant::now() + budget);
    // SAFETY: the same borrowed input is still open; restore its original flags.
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags) } < 0 {
        return Err("Cannot restore consent input; no approval accepted".into());
    }
    result
}

fn read_nonblocking(fd: libc::c_int, deadline: Instant) -> Result<String> {
    let mut bytes = Vec::new();
    loop {
        check()?;
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or("Consent timed out; no approval accepted")?;
        let mut poll = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: poll addresses one valid descriptor for a finite slice. Short
        // slices let cancellation be noticed even when delivered on another thread.
        let ready = unsafe { libc::poll(&mut poll, 1, remaining.as_millis().clamp(1, 100) as i32) };
        if ready < 0 {
            if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err("Consent input polling failed".into());
        }
        if ready == 0 {
            continue;
        }
        check()?;
        if Instant::now() >= deadline {
            return Err("Consent timed out; no approval accepted".into());
        }
        // Read exactly one byte: a piped second answer must remain for its own
        // separate prompt, rather than being consumed or implicitly accepted here.
        let mut byte = 0u8;
        // SAFETY: read writes at most one byte into the initialized stack object.
        let n = unsafe { libc::read(fd, std::ptr::addr_of_mut!(byte).cast(), 1) };
        if n < 0 {
            let kind = std::io::Error::last_os_error().kind();
            if matches!(
                kind,
                std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock
            ) {
                continue;
            }
            return Err("Consent input failed".into());
        }
        if n == 0 {
            return Err("Consent input ended before newline; no approval accepted".into());
        }
        check()?;
        if Instant::now() >= deadline {
            return Err("Consent timed out; no approval accepted".into());
        }
        if byte == 3 {
            return Err("Consent interrupted; no approval accepted".into());
        }
        if byte == b'\n' {
            return String::from_utf8(bytes)
                .map_err(|_| "Consent is not UTF-8; no approval accepted".into());
        }
        bytes.push(byte);
        // Leave room for the required terminating newline.
        if bytes.len() >= INPUT_LIMIT {
            return Err("Consent exceeds 4096 bytes; no approval accepted".into());
        }
    }
}

/// Remove terminal controls and bidi direction controls from untrusted output.
/// This is display sanitization, not a general credential-redaction guarantee.
pub fn display(value: &str) -> String {
    value
        .chars()
        .filter(|c| {
            !c.is_control()
                && !matches!(c,
                    '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
                )
        })
        .collect()
}
