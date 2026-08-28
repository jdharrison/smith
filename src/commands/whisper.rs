//! Local Whisper STT sidecar lifecycle for `smith serve` and `smith install`.
//!
//! Spawns `client/sidecar/whisper_server.py` on loopback :4790 when needed,
//! and stops the child when the bridge exits (if we started it).

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::{BULLET_GREEN, BULLET_RED, BULLET_YELLOW};

pub const WHISPER_DEFAULT_PORT: u16 = 4790;
const REQS_MARKER: &str = ".smith-whisper-reqs";

/// Holds a Whisper child we spawned (so Drop / explicit stop can kill it).
pub struct WhisperGuard {
    child: Option<Child>,
}

impl WhisperGuard {
    pub fn none() -> Self {
        Self { child: None }
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for WhisperGuard {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Ensure Whisper venv + deps under `client/sidecar` (idempotent). Non-fatal.
pub fn ensure_whisper_install() {
    println!("  Whisper STT:");

    let sidecar = match resolve_sidecar_dir() {
        Some(p) => p,
        None => {
            eprintln!(
                "  {} whisper - sidecar not found (client/sidecar/whisper_server.py)",
                BULLET_YELLOW
            );
            eprintln!("       Set SMITH_WHISPER_DIR or run install from the smith repo.");
            return;
        }
    };

    let reqs = sidecar.join("requirements.txt");
    if !reqs.is_file() {
        eprintln!(
            "  {} whisper - missing {}",
            BULLET_YELLOW,
            reqs.display()
        );
        return;
    }

    let venv_py = venv_python_path(&sidecar);
    if !venv_py.is_file() {
        let host = match host_python() {
            Some(p) => p,
            None => {
                eprintln!(
                    "  {} whisper - Python not found (need python3/python for venv)",
                    BULLET_YELLOW
                );
                return;
            }
        };
        println!(
            "  Creating Whisper venv at {} ...",
            sidecar.join(".venv").display()
        );
        let status = Command::new(&host)
            .args(["-m", "venv"])
            .arg(sidecar.join(".venv"))
            .status();
        match status {
            Ok(s) if s.success() && venv_py.is_file() => {
                println!("  {} whisper - venv created", BULLET_GREEN);
            }
            Ok(s) => {
                eprintln!(
                    "  {} whisper - venv failed (exit {})",
                    BULLET_RED,
                    s.code().unwrap_or(-1)
                );
                return;
            }
            Err(e) => {
                eprintln!("  {} whisper - venv failed: {}", BULLET_RED, e);
                return;
            }
        }
    } else {
        println!("  {} whisper - venv present", BULLET_GREEN);
    }

    let reqs_bytes = match fs::read(&reqs) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("  {} whisper - read requirements: {}", BULLET_RED, e);
            return;
        }
    };
    let reqs_hash = hash_bytes(&reqs_bytes);
    let marker = sidecar.join(".venv").join(REQS_MARKER);
    let up_to_date = fs::read_to_string(&marker)
        .map(|s| s.trim() == reqs_hash)
        .unwrap_or(false);

    if up_to_date && whisper_imports_ok(&venv_py) {
        println!("  {} whisper - deps up to date", BULLET_GREEN);
        return;
    }

    println!("  Installing Whisper deps (pip install -r requirements.txt) ...");
    let status = Command::new(&venv_py)
        .args(["-m", "pip", "install", "-r"])
        .arg(&reqs)
        .current_dir(&sidecar)
        .status();
    match status {
        Ok(s) if s.success() => {
            if let Err(e) = fs::write(&marker, &reqs_hash) {
                eprintln!("  {} whisper - marker write: {}", BULLET_YELLOW, e);
            }
            if whisper_imports_ok(&venv_py) {
                println!("  {} whisper - deps ready", BULLET_GREEN);
            } else {
                eprintln!(
                    "  {} whisper - pip ok but import check failed (faster_whisper / fastapi)",
                    BULLET_YELLOW
                );
            }
        }
        Ok(s) => eprintln!(
            "  {} whisper - pip failed (exit {})",
            BULLET_RED,
            s.code().unwrap_or(-1)
        ),
        Err(e) => eprintln!("  {} whisper - pip failed: {}", BULLET_RED, e),
    }
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut h = DefaultHasher::new();
    bytes.hash(&mut h);
    format!("{:016x}", h.finish())
}

fn whisper_imports_ok(python: &Path) -> bool {
    Command::new(python)
        .args(["-c", "import faster_whisper, fastapi"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn venv_python_path(sidecar: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        sidecar.join(".venv").join("Scripts").join("python.exe")
    }
    #[cfg(not(windows))]
    {
        sidecar.join(".venv").join("bin").join("python")
    }
}

fn host_python() -> Option<PathBuf> {
    for name in ["python3", "python"] {
        if Command::new(name)
            .arg("-c")
            .arg("import sys; sys.exit(0)")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Some(PathBuf::from(name));
        }
    }
    None
}

/// Ensure Whisper is reachable on :4790. Reuses an existing healthy server.
/// On failure, prints a warning and returns an empty guard (bridge still runs).
pub fn ensure_whisper_sidecar() -> WhisperGuard {
    if std::env::var_os("SMITH_WHISPER")
        .map(|v| v == "0" || v.eq_ignore_ascii_case("false") || v.eq_ignore_ascii_case("off"))
        .unwrap_or(false)
    {
        println!("  whisper: skipped (SMITH_WHISPER=0)");
        return WhisperGuard::none();
    }

    if whisper_healthy() {
        println!(
            "  whisper: already up on http://127.0.0.1:{}",
            WHISPER_DEFAULT_PORT
        );
        return WhisperGuard::none();
    }

    let sidecar = match resolve_sidecar_dir() {
        Some(p) => p,
        None => {
            eprintln!(
                "  whisper: sidecar not found (expected client/sidecar/whisper_server.py).\n  \
                 Run from the smith repo, or set SMITH_WHISPER_DIR to that folder.\n  \
                 Or: cd client/sidecar && .venv\\Scripts\\activate && python whisper_server.py"
            );
            return WhisperGuard::none();
        }
    };

    let script = sidecar.join("whisper_server.py");
    let python = match resolve_python(&sidecar) {
        Some(p) => p,
        None => {
            eprintln!(
                "  whisper: no Python for {}.\n  \
                 Run `smith install` (Whisper STT section) or: cd client/sidecar && python -m venv .venv && pip install -r requirements.txt",
                sidecar.display()
            );
            return WhisperGuard::none();
        }
    };

    println!(
        "  whisper: starting {} ({})",
        script.display(),
        python.display()
    );

    let mut cmd = Command::new(&python);
    cmd.arg(&script)
        .current_dir(&sidecar)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("  whisper: failed to spawn: {e}");
            return WhisperGuard::none();
        }
    };

    let mut guard = WhisperGuard { child: Some(child) };

    for attempt in 1..=90 {
        if whisper_healthy() {
            println!(
                "  whisper: ready on http://127.0.0.1:{}",
                WHISPER_DEFAULT_PORT
            );
            return guard;
        }
        if let Some(ref mut c) = guard.child {
            if let Ok(Some(status)) = c.try_wait() {
                eprintln!("  whisper: exited early ({status})");
                guard.child = None;
                return WhisperGuard::none();
            }
        }
        if attempt < 90 {
            thread::sleep(Duration::from_secs(1));
        }
    }

    eprintln!(
        "  whisper: started but /v1/health not ready on :{} — voice may stay offline",
        WHISPER_DEFAULT_PORT
    );
    guard
}

fn whisper_healthy() -> bool {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let Ok(mut stream) = TcpStream::connect(("127.0.0.1", WHISPER_DEFAULT_PORT)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let req = format!(
        "GET /v1/health HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
        WHISPER_DEFAULT_PORT
    );
    if stream.write_all(req.as_bytes()).is_err() {
        return false;
    }
    let mut buf = Vec::new();
    let _ = stream.read_to_end(&mut buf);
    let body = String::from_utf8_lossy(&buf);
    body.contains("\"ok\":true") || body.contains("\"ok\": true")
}

fn resolve_sidecar_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("SMITH_WHISPER_DIR") {
        let p = PathBuf::from(dir);
        if p.join("whisper_server.py").is_file() {
            return Some(p);
        }
    }

    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(cwd) = std::env::current_dir() {
        push_ancestor_sidecars(&cwd, &mut candidates);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            push_ancestor_sidecars(parent, &mut candidates);
        }
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.push(manifest.join("client").join("sidecar"));

    candidates
        .into_iter()
        .find(|c| c.join("whisper_server.py").is_file())
}

fn push_ancestor_sidecars(start: &Path, out: &mut Vec<PathBuf>) {
    let mut cur = Some(start);
    for _ in 0..8 {
        let Some(dir) = cur else { break };
        out.push(dir.join("client").join("sidecar"));
        out.push(dir.join("sidecar"));
        cur = dir.parent();
    }
}

fn resolve_python(sidecar: &Path) -> Option<PathBuf> {
    let venv_py = venv_python_path(sidecar);
    if venv_py.is_file() {
        return Some(venv_py);
    }
    host_python()
}
