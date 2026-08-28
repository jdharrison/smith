//! Local Kokoro TTS sidecar lifecycle for `smith serve` and `smith install`.
//!
//! Spawns `client/sidecar/tts_server.py` on loopback :4791 when needed,
//! and stops the child when the bridge exits (if we started it).

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::{BULLET_GREEN, BULLET_RED, BULLET_YELLOW};

pub const TTS_DEFAULT_PORT: u16 = 4791;
const REQS_MARKER: &str = ".smith-tts-reqs";
const DEFAULT_VOICE: &str = "am_onyx";

const ONNX_URL: &str =
    "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/kokoro-v1.0.onnx";
const VOICES_URL: &str =
    "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/voices-v1.0.bin";

/// Holds a TTS child we spawned (so Drop / explicit stop can kill it).
pub struct TtsGuard {
    child: Option<Child>,
}

impl TtsGuard {
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

impl Drop for TtsGuard {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Ensure Kokoro TTS venv + deps + ONNX models (idempotent). Non-fatal.
pub fn ensure_tts_install() {
    println!("  Kokoro TTS:");

    let sidecar = match resolve_sidecar_dir() {
        Some(p) => p,
        None => {
            eprintln!(
                "  {} tts - sidecar not found (client/sidecar/tts_server.py)",
                BULLET_YELLOW
            );
            eprintln!("       Set SMITH_TTS_DIR or run install from the smith repo.");
            return;
        }
    };

    let reqs = sidecar.join("requirements-tts.txt");
    if !reqs.is_file() {
        eprintln!(
            "  {} tts - missing {}",
            BULLET_YELLOW,
            reqs.display()
        );
        return;
    }

    let venv_dir = sidecar.join(".venv-tts");
    let venv_py = venv_python_path(&sidecar);
    if !venv_py.is_file() {
        let host = match host_python() {
            Some(p) => p,
            None => {
                eprintln!(
                    "  {} tts - Python not found (need python3/python for venv)",
                    BULLET_YELLOW
                );
                return;
            }
        };
        println!("  Creating TTS venv at {} ...", venv_dir.display());
        let status = Command::new(&host)
            .args(["-m", "venv"])
            .arg(&venv_dir)
            .status();
        match status {
            Ok(s) if s.success() && venv_py.is_file() => {
                println!("  {} tts - venv created", BULLET_GREEN);
            }
            Ok(s) => {
                eprintln!(
                    "  {} tts - venv failed (exit {})",
                    BULLET_RED,
                    s.code().unwrap_or(-1)
                );
                return;
            }
            Err(e) => {
                eprintln!("  {} tts - venv failed: {}", BULLET_RED, e);
                return;
            }
        }
    } else {
        println!("  {} tts - venv present", BULLET_GREEN);
    }

    let reqs_bytes = match fs::read(&reqs) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("  {} tts - read requirements: {}", BULLET_RED, e);
            return;
        }
    };
    let reqs_hash = hash_bytes(&reqs_bytes);
    let marker = venv_dir.join(REQS_MARKER);
    let up_to_date = fs::read_to_string(&marker)
        .map(|s| s.trim() == reqs_hash)
        .unwrap_or(false);

    if up_to_date && tts_imports_ok(&venv_py) {
        println!("  {} tts - deps up to date", BULLET_GREEN);
    } else {
        println!("  Installing TTS deps (pip install -r requirements-tts.txt) ...");
        let status = Command::new(&venv_py)
            .args(["-m", "pip", "install", "-r"])
            .arg(&reqs)
            .current_dir(&sidecar)
            .status();
        match status {
            Ok(s) if s.success() => {
                if let Err(e) = fs::write(&marker, &reqs_hash) {
                    eprintln!("  {} tts - marker write: {}", BULLET_YELLOW, e);
                }
                if tts_imports_ok(&venv_py) {
                    println!("  {} tts - deps ready", BULLET_GREEN);
                } else {
                    eprintln!(
                        "  {} tts - pip ok but import check failed (kokoro_onnx / fastapi)",
                        BULLET_YELLOW
                    );
                    eprintln!(
                        "       If errors mention espeak: install espeak-ng and re-run install"
                    );
                }
            }
            Ok(s) => {
                eprintln!(
                    "  {} tts - pip failed (exit {})",
                    BULLET_RED,
                    s.code().unwrap_or(-1)
                );
                return;
            }
            Err(e) => {
                eprintln!("  {} tts - pip failed: {}", BULLET_RED, e);
                return;
            }
        }
    }

    let models = sidecar.join("models");
    if let Err(e) = fs::create_dir_all(&models) {
        eprintln!("  {} tts - models dir: {}", BULLET_RED, e);
        return;
    }
    let onnx = models.join("kokoro-v1.0.onnx");
    let voices = models.join("voices-v1.0.bin");
    if onnx.is_file() && voices.is_file() {
        println!("  {} tts - models present", BULLET_GREEN);
    } else {
        println!("  Downloading Kokoro ONNX models (one-time, ~340MB) ...");
        if !onnx.is_file() {
            if let Err(e) = download_file(ONNX_URL, &onnx) {
                eprintln!("  {} tts - onnx download: {}", BULLET_RED, e);
                return;
            }
        }
        if !voices.is_file() {
            if let Err(e) = download_file(VOICES_URL, &voices) {
                eprintln!("  {} tts - voices download: {}", BULLET_RED, e);
                return;
            }
        }
        println!("  {} tts - models ready", BULLET_GREEN);
    }
}

fn download_file(url: &str, dest: &Path) -> Result<(), String> {
    let tmp = dest.with_extension("download");
    let status = Command::new("curl")
        .args(["-fsSL", "--retry", "3", "-o"])
        .arg(&tmp)
        .arg(url)
        .status()
        .map_err(|e| format!("curl failed: {e}"))?;
    if !status.success() {
        let _ = fs::remove_file(&tmp);
        return Err(format!("curl exit {}", status.code().unwrap_or(-1)));
    }
    fs::rename(&tmp, dest).map_err(|e| format!("rename: {e}"))?;
    Ok(())
}

/// Ensure TTS is reachable on :4791. Reuses an existing healthy server.
pub fn ensure_tts_sidecar() -> TtsGuard {
    if std::env::var_os("SMITH_TTS")
        .map(|v| v == "0" || v.eq_ignore_ascii_case("false") || v.eq_ignore_ascii_case("off"))
        .unwrap_or(false)
    {
        println!("  tts: skipped (SMITH_TTS=0)");
        return TtsGuard::none();
    }

    if tts_healthy() {
        println!(
            "  tts: already up on http://127.0.0.1:{}",
            TTS_DEFAULT_PORT
        );
        return TtsGuard::none();
    }

    let sidecar = match resolve_sidecar_dir() {
        Some(p) => p,
        None => {
            eprintln!(
                "  tts: sidecar not found (expected client/sidecar/tts_server.py).\n  \
                 Run `smith install` or set SMITH_TTS_DIR. OS TTS remains as fallback."
            );
            return TtsGuard::none();
        }
    };

    let script = sidecar.join("tts_server.py");
    let python = match resolve_python(&sidecar) {
        Some(p) => p,
        None => {
            eprintln!(
                "  tts: no .venv-tts Python — run `smith install` (Kokoro TTS section).\n  \
                 OS TTS remains as fallback."
            );
            return TtsGuard::none();
        }
    };

    let voice = std::env::var("SMITH_TTS_VOICE").unwrap_or_else(|_| DEFAULT_VOICE.to_string());

    println!(
        "  tts: starting {} ({}) voice={}",
        script.display(),
        python.display(),
        voice
    );

    let mut cmd = Command::new(&python);
    cmd.arg(&script)
        .arg("--voice")
        .arg(&voice)
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
            eprintln!("  tts: failed to spawn: {e}");
            return TtsGuard::none();
        }
    };

    let mut guard = TtsGuard { child: Some(child) };

    for attempt in 1..=120 {
        if tts_healthy() {
            println!(
                "  tts: ready on http://127.0.0.1:{}",
                TTS_DEFAULT_PORT
            );
            return guard;
        }
        if let Some(ref mut c) = guard.child {
            if let Ok(Some(status)) = c.try_wait() {
                eprintln!("  tts: exited early ({status})");
                guard.child = None;
                return TtsGuard::none();
            }
        }
        if attempt < 120 {
            thread::sleep(Duration::from_secs(1));
        }
    }

    eprintln!(
        "  tts: started but /v1/health not ready on :{} — Flutter will use OS TTS",
        TTS_DEFAULT_PORT
    );
    guard
}

fn tts_healthy() -> bool {
    let Ok(mut stream) = TcpStream::connect(("127.0.0.1", TTS_DEFAULT_PORT)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let req = format!(
        "GET /v1/health HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
        TTS_DEFAULT_PORT
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
    if let Ok(dir) = std::env::var("SMITH_TTS_DIR") {
        let p = PathBuf::from(dir);
        if p.join("tts_server.py").is_file() {
            return Some(p);
        }
    }
    if let Ok(dir) = std::env::var("SMITH_WHISPER_DIR") {
        let p = PathBuf::from(dir);
        if p.join("tts_server.py").is_file() {
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
        .find(|c| c.join("tts_server.py").is_file())
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

fn venv_python_path(sidecar: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        sidecar.join(".venv-tts").join("Scripts").join("python.exe")
    }
    #[cfg(not(windows))]
    {
        sidecar.join(".venv-tts").join("bin").join("python")
    }
}

fn resolve_python(sidecar: &Path) -> Option<PathBuf> {
    let venv_py = venv_python_path(sidecar);
    if venv_py.is_file() {
        return Some(venv_py);
    }
    None
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

fn tts_imports_ok(python: &Path) -> bool {
    Command::new(python)
        .args(["-c", "import kokoro_onnx, fastapi, soundfile"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut h = DefaultHasher::new();
    bytes.hash(&mut h);
    format!("{:016x}", h.finish())
}
