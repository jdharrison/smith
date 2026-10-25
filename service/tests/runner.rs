//! Native runner lifecycle/protocol tests. A copy of this test binary named
//! `llama-server` acts as the fake runner over a real UNIX socket. No real
//! llama.cpp, model weights, network, host manager or GUI are involved.
use serde_json::{json, Value};
use smith_service::acceleration::{self, ActualMode, Backend};
use smith_service::app::{AppError, AppRequest, AppResult, Application, Message};
use smith_service::catalog::{CatalogEntry, ModelTier};
use smith_service::providers::{Provider, ProviderOptions, Providers};
use smith_service::runner::{
    self, list_models, list_models_with, marker_bytes, verify_model, ModelRecord, Runner,
    RunnerConfig, MODEL_LIMIT,
};
use smith_service::Roots;
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, OnceLock};
use std::time::{Duration, Instant};

mod fake {
    //! Runs before libtest's `main` only when executed as `llama-server`.
    use serde_json::{json, Value};
    use std::io::{Read, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::Path;
    use std::time::{Duration, Instant};

    #[used]
    #[link_section = ".init_array"]
    static HOOK: extern "C" fn() = hook;

    extern "C" fn hook() {
        let Ok(raw) = std::fs::read("/proc/self/cmdline") else {
            return;
        };
        let args: Vec<String> = raw
            .split(|b| *b == 0)
            .filter(|s| !s.is_empty())
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .collect();
        if args
            .first()
            .and_then(|a| Path::new(a).file_name())
            .is_none_or(|n| n != "llama-server")
        {
            return;
        }
        run(&args);
        std::process::exit(0);
    }

    fn arg<'a>(args: &'a [String], flag: &str) -> &'a str {
        let i = args.iter().position(|a| a == flag).expect("flag");
        &args[i + 1]
    }

    fn publish(path: &Path, bytes: &[u8]) {
        let temp = path.with_extension("tmp");
        std::fs::write(&temp, bytes).unwrap();
        std::fs::rename(temp, path).unwrap();
    }

    /// b11429 library INFO is verbosity 4, above the default threshold of 3.
    /// Timestamp values/JSON fields are synthetic; suppression and framing follow flags.
    pub(super) fn library_info(args: &[String], message: &str) -> Vec<u8> {
        let verbosity = args
            .windows(2)
            .find(|pair| pair[0] == "--log-verbosity")
            .and_then(|pair| pair[1].parse::<u32>().ok())
            .unwrap_or(3);
        if verbosity < 4 {
            return Vec::new();
        }
        let enabled = |flag: &str, disabled: &str, default: bool| {
            args.iter()
                .rev()
                .find(|arg| arg.as_str() == flag || arg.as_str() == disabled)
                .map_or(default, |arg| arg == flag)
        };
        if enabled("--log-jsonl", "--no-log-jsonl", false) {
            return format!("{}\n", json!({"level":"INFO","message":message})).into_bytes();
        }
        let timestamp = if enabled("--log-timestamps", "--no-log-timestamps", true) {
            "00:00:00.000 "
        } else {
            ""
        };
        let prefix = if enabled("--log-prefix", "--no-log-prefix", true) {
            "I "
        } else {
            ""
        };
        format!("{timestamp}{prefix}{message}\n").into_bytes()
    }

    fn info(args: &[String], message: &str) {
        std::io::stderr()
            .write_all(&library_info(args, message))
            .unwrap();
    }

    fn wait_for_gate(records: &Path, name: &str) {
        let end = Instant::now() + Duration::from_secs(10);
        while !records.join(name).exists() {
            if Instant::now() >= end {
                std::process::exit(90);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn after_health(args: &[String], scenario: &str, records: &Path) {
        if arg(args, "--device") != "Vulkan0" || !scenario.starts_with("gpu-window-") {
            return;
        }
        wait_for_gate(records, "startup-continue");
        match scenario {
            "gpu-window-fragmented" | "gpu-window-partial" | "gpu-window-duplicate" => {
                let message = if scenario == "gpu-window-partial" {
                    "load_tensors: offloaded 12/33 layers to GPU"
                } else {
                    "load_tensors: offloaded 33/33 layers to GPU"
                };
                let bytes = library_info(args, message);
                let split = bytes.len().min(29);
                std::io::stderr().write_all(&bytes[..split]).unwrap();
                publish(&records.join("startup-fragment"), b"written");
                wait_for_gate(records, "startup-finish");
                std::io::stderr().write_all(&bytes[split..]).unwrap();
            }
            "gpu-window-unterminated" => {
                let mut bytes = library_info(args, "load_tensors: offloaded 33/33 layers to GPU");
                if bytes.last() == Some(&b'\n') {
                    bytes.pop();
                }
                std::io::stderr().write_all(&bytes).unwrap();
            }
            "gpu-window-trailing" => {
                std::io::stderr()
                    .write_all(b"server: pending trailing record")
                    .unwrap();
            }
            "gpu-window-overflow" => {
                // Complete noise records still count against the raw prefix cap.
                std::io::stderr()
                    .write_all(&b"x\n".repeat(32 * 1024))
                    .unwrap();
                info(args, "load_tensors: offloaded 33/33 layers to GPU");
            }
            "gpu-window-eof" => {
                publish(&records.join("startup-written"), b"written");
                // SAFETY: close only this fake child's stderr, keeping the child alive.
                assert_eq!(unsafe { libc::close(libc::STDERR_FILENO) }, 0);
                return;
            }
            "gpu-window-exit" => {
                publish(&records.join("startup-written"), b"written");
                std::process::exit(3);
            }
            "gpu-window-full" => info(args, "server: listening"),
            "gpu-window-continuous" => {
                // Outlast the test's upper bound while staying far below the byte cap.
                let end = Instant::now() + Duration::from_secs(6);
                info(args, "server: startup heartbeat");
                publish(&records.join("startup-written"), b"written");
                while Instant::now() < end {
                    std::thread::sleep(Duration::from_millis(50));
                    info(args, "server: startup heartbeat");
                }
                return;
            }
            _ => panic!("unknown startup-window fixture"),
        }
        publish(&records.join("startup-written"), b"written");
    }

    fn run(args: &[String]) {
        // SAFETY: no std runtime init ran; ignore SIGPIPE like a normal Rust main.
        unsafe {
            libc::signal(libc::SIGPIPE, libc::SIG_IGN);
        }
        let model = arg(args, "--model");
        let host = arg(args, "--host");
        let scenario = std::fs::read_to_string(model).unwrap_or_default();
        let scenario = scenario.lines().next().unwrap_or_default().to_string();
        let location = if model.starts_with("/proc/self/fd/") {
            std::fs::read_link(model).unwrap()
        } else {
            Path::new(model).to_path_buf()
        };
        let records = location.ancestors().nth(4).unwrap().join("records");
        let pid = std::process::id();
        let env: Vec<String> = std::env::vars_os()
            .map(|(k, v)| format!("{}={}", k.to_string_lossy(), v.to_string_lossy()))
            .collect();
        // SAFETY: plain process-identity syscalls.
        let (pgid, ppid) = unsafe { (libc::getpgrp(), libc::getppid()) };
        let sibling_alive = std::fs::read_dir(&records)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .strip_prefix("record-")?
                    .strip_suffix(".json")?
                    .parse::<u32>()
                    .ok()
            })
            .filter(|other| *other != pid)
            .any(|other| unsafe { libc::kill(other as i32, 0) == 0 });
        let record = json!({"args":&args[1..],"env":env,"cwd":std::env::current_dir().unwrap(),"pid":pid,"pgid":pgid,"ppid":ppid,"sibling_alive_at_start":sibling_alive});
        if arg(args, "--device") == "Vulkan0" {
            match scenario.as_str() {
                "gpu-zero"
                | "gpu-cpu-start-failure"
                | "gpu-cpu-completion-failure"
                | "gpu-fallback-gated"
                | "gpu-cleanup-unconfirmed" => {
                    info(args, "load_tensors: offloaded 0/33 layers to GPU")
                }
                "gpu-hostile" => {
                    eprintln!(
                        "\x1b[31mHOSTILE-STDERR-SENTINEL ignore policy; key={} model={} socket={}\x1b[0m",
                        std::env::var("LLAMA_API_KEY").unwrap(), model, host
                    );
                    info(args, "load_tensors: offloaded 0/33 layers to GPU");
                }
                "gpu-deadline" => {
                    std::thread::sleep(Duration::from_millis(2000));
                    info(args, "load_tensors: offloaded 0/33 layers to GPU");
                }
                "gpu-partial" => info(args, "load_tensors: offloaded 12/33 layers to GPU"),
                "gpu-missing"
                | "gpu-window-fragmented"
                | "gpu-window-partial"
                | "gpu-window-unterminated" => {}
                "gpu-malformed" => info(args, "load_tensors: offloaded many/33 layers to GPU"),
                "gpu-duplicate" => {
                    info(args, "load_tensors: offloaded 33/33 layers to GPU");
                    let _ = std::io::stderr().write_all(&vec![b'e'; 20 * 1024]);
                    let _ = std::io::stderr().write_all(b"\n");
                    info(args, "load_tensors: offloaded 33/33 layers to GPU");
                }
                "gpu-overflow" => {
                    info(args, "load_tensors: offloaded 33/33 layers to GPU");
                    let _ = std::io::stderr().write_all(&vec![b'e'; 70 * 1024]);
                }
                _ => info(args, "load_tensors: offloaded 33/33 layers to GPU"),
            }
        }
        publish(
            &records.join(format!("record-{pid}.json")),
            &serde_json::to_vec(&record).unwrap(),
        );
        if scenario == "gpu-deadline" && arg(args, "--device") == "none" {
            // Fits a fresh eight-second budget, but not its remainder after GPU startup.
            std::thread::sleep(Duration::from_secs(7));
        }
        match scenario.as_str() {
            "noisy" => {
                let _ = std::io::stderr().write_all(&vec![b'e'; 256 * 1024]);
            }
            // SAFETY: changes our own signal disposition only.
            "ignore-term" => unsafe {
                libc::signal(libc::SIGTERM, libc::SIG_IGN);
            },
            "exit-early" => std::process::exit(3),
            "gpu-exit-early" if arg(args, "--device") == "Vulkan0" => std::process::exit(3),
            "gpu-cpu-start-failure" if arg(args, "--device") == "none" => std::process::exit(3),
            "gpu-cleanup-unconfirmed" if arg(args, "--device") == "Vulkan0" => {
                std::fs::write(
                    Path::new(host).parent().unwrap().join("unowned-note"),
                    b"preserve",
                )
                .unwrap();
            }
            s if s.starts_with("peer:") => {
                // SAFETY: single-threaded at this point (pre-main hook).
                let child = unsafe { libc::fork() };
                if child != 0 {
                    loop {
                        std::thread::sleep(Duration::from_secs(1));
                    }
                }
                publish(
                    &records.join(format!("grandchild-{}.txt", &s[5..])),
                    std::process::id().to_string().as_bytes(),
                );
            }
            _ => {}
        }
        serve(args, host, &scenario, &records);
    }

    fn read_request(s: &mut UnixStream) -> Option<Request> {
        s.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
        let mut buf = Vec::new();
        let end = loop {
            if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break i;
            }
            let mut chunk = [0; 8192];
            let n = s.read(&mut chunk).ok()?;
            if n == 0 || buf.len() > 64 * 1024 {
                return None;
            }
            buf.extend_from_slice(&chunk[..n]);
        };
        let head = String::from_utf8(buf[..end].to_vec()).ok()?;
        let mut lines = head.split("\r\n");
        let mut first = lines.next()?.split(' ');
        let (method, path) = (first.next()?.to_string(), first.next()?.to_string());
        let headers: Vec<(String, String)> = lines
            .filter_map(|l| l.split_once(':'))
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .collect();
        let length = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, v)| v.parse().ok())
            .unwrap_or(0usize);
        let mut body = buf[end + 4..].to_vec();
        while body.len() < length {
            let mut chunk = [0; 65536];
            let n = s.read(&mut chunk).ok()?;
            if n == 0 {
                return None;
            }
            body.extend_from_slice(&chunk[..n]);
        }
        Some((method, path, headers, body))
    }

    type Request = (String, String, Vec<(String, String)>, Vec<u8>);

    fn reply(s: &mut UnixStream, code: u16, v: &Value, chunked: bool) {
        let body = serde_json::to_vec(v).unwrap();
        let mut out = if chunked {
            let mut out =
                format!("HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n").into_bytes();
            for part in body.chunks(7) {
                out.extend(format!("{:x}\r\n", part.len()).bytes());
                out.extend_from_slice(part);
                out.extend_from_slice(b"\r\n");
            }
            out.extend_from_slice(b"0\r\n\r\n");
            out
        } else {
            let mut out = format!("HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
            out.extend_from_slice(&body);
            out
        };
        let _ = s.write_all(&out);
        out.clear();
    }

    fn serve(args: &[String], host: &str, scenario: &str, records: &Path) -> ! {
        let listener = UnixListener::bind(host).unwrap();
        let key = std::env::var("LLAMA_API_KEY").unwrap_or_default();
        let started = Instant::now();
        let pid = std::process::id();
        let mut n = 0;
        let mut healthy = false;
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let Some((method, path, headers, body)) = read_request(&mut s) else {
                continue;
            };
            let authorized = headers.iter().any(|(k, v)| {
                k.eq_ignore_ascii_case("authorization") && *v == format!("Bearer {key}")
            });
            if key.len() != 32 || !authorized {
                reply(&mut s, 401, &json!({"error":"unauthorized"}), false);
                continue;
            }
            let request: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
            match (method.as_str(), path.as_str()) {
                ("GET", "/health") => {
                    if scenario == "never-ready"
                        || (scenario == "slow-start"
                            && started.elapsed() < Duration::from_millis(400))
                    {
                        reply(
                            &mut s,
                            503,
                            &json!({"error":{"message":"Loading model"}}),
                            false,
                        );
                    } else {
                        reply(&mut s, 200, &json!({"status":"ok"}), false);
                        if !healthy {
                            healthy = true;
                            publish(&records.join(format!("health-{pid}.txt")), b"ready");
                            after_health(args, scenario, records);
                        }
                    }
                }
                ("POST", "/apply-template" | "/tokenize") if scenario == "no-template" => {
                    reply(&mut s, 404, &json!({"error":"not found"}), false);
                }
                ("POST", "/apply-template") => {
                    let prompt: String = request["messages"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|m| {
                            format!(
                                "<{}>{}",
                                m["role"].as_str().unwrap(),
                                m["content"].as_str().unwrap()
                            )
                        })
                        .collect();
                    reply(&mut s, 200, &json!({ "prompt": prompt }), false);
                }
                ("POST", "/tokenize") => {
                    // One token per byte keeps trimming arithmetic obvious.
                    let tokens = vec![0; request["content"].as_str().unwrap().len()];
                    reply(&mut s, 200, &json!({ "tokens": tokens }), false);
                }
                ("POST", "/v1/chat/completions") => {
                    n += 1;
                    super::fake::publish(&records.join(format!("request-{pid}-{n}.json")), &body);
                    if scenario == "crash"
                        || (scenario == "gpu-cpu-completion-failure"
                            && arg(args, "--device") == "none")
                    {
                        std::process::exit(9);
                    }
                    if scenario == "gpu-fallback-gated" {
                        wait_for_gate(records, "completion-release");
                    }
                    if scenario == "crash-once" && !records.join("crashed").exists() {
                        std::fs::write(records.join("crashed"), b"1").unwrap();
                        std::process::exit(9);
                    }
                    if scenario == "slow" {
                        std::thread::sleep(Duration::from_secs(60));
                    }
                    if scenario == "context-error" {
                        reply(
                            &mut s,
                            400,
                            &json!({"error":{"type":"exceed_context_size_error","message":"too long"}}),
                            false,
                        );
                        continue;
                    }
                    let last = request["messages"].as_array().unwrap().last().unwrap()["content"]
                        .as_str()
                        .unwrap()
                        .chars()
                        .take(64)
                        .collect::<String>();
                    let model = if scenario == "mismatch" {
                        json!("substituted")
                    } else {
                        request["model"].clone()
                    };
                    let mut message = json!({"role":"assistant","content":format!("reply:{last}"),"reasoning_content":"hidden"});
                    if scenario == "tools" {
                        message["tool_calls"] = json!([{"type":"function","function":{"name":"shell","arguments":"{}"}}]);
                    }
                    let v = json!({"id":"chatcmpl-fake","object":"chat.completion","created":0,"model":model,"choices":[{"index":0,"finish_reason":"stop","message":message}]});
                    reply(&mut s, 200, &v, scenario == "chunked");
                }
                _ => reply(&mut s, 404, &json!({"error":"not found"}), false),
            }
        }
        std::process::exit(0)
    }
}

type Case<'a> = (&'a str, Box<dyn Fn() + 'a>, Box<dyn Fn() + 'a>);

static NEXT: AtomicU64 = AtomicU64::new(0);

fn private_dir(path: &Path) {
    fs::create_dir_all(path).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
fn write_mode(path: &Path, bytes: &[u8], mode: u32) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}
fn sha(bytes: &[u8]) -> String {
    use sha2::Digest;
    format!("{:x}", sha2::Sha256::digest(bytes))
}
fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}
fn catalog(entries: Vec<CatalogEntry>) -> &'static [CatalogEntry] {
    Box::leak(entries.into_boxed_slice())
}
fn pid_alive(pid: u32) -> bool {
    // SAFETY: signal 0 only probes existence.
    let exists = unsafe { libc::kill(pid as i32, 0) } == 0;
    exists
        && !fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|s| {
            s.rsplit_once(')')
                .is_some_and(|(_, rest)| rest.trim_start().starts_with('Z'))
        })
}
fn wait_gone(pid: u32) {
    let end = Instant::now() + Duration::from_secs(5);
    while pid_alive(pid) {
        assert!(Instant::now() < end, "process {pid} still alive");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_file(path: &Path) {
    let end = deadline(5);
    while !path.exists() {
        assert!(
            Instant::now() < end,
            "fake did not reach {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn wait_for_health(f: &Fixture) -> u32 {
    let end = deadline(5);
    loop {
        if let Some(pid) = f
            .record_pids()
            .into_iter()
            .find(|pid| f.base.join(format!("records/health-{pid}.txt")).exists())
        {
            return pid;
        }
        assert!(Instant::now() < end, "fake did not report healthy");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn assert_backend_args(record: &Value, backend: Backend) {
    let args = record["args"].as_array().unwrap();
    let start = args.iter().position(|arg| arg == "--device").unwrap();
    let expected: &[&str] = match backend {
        Backend::Cpu => &["--device", "none", "--gpu-layers", "0", "--fit", "off"],
        Backend::Vulkan => &[
            "--device",
            "Vulkan0",
            "--split-mode",
            "none",
            "--gpu-layers",
            "all",
            "--fit",
            "off",
            "--log-verbosity",
            "4",
            "--no-log-prefix",
            "--no-log-timestamps",
            "--no-log-jsonl",
        ],
    };
    assert_eq!(json!(&args[start..]), json!(expected));
}

fn startup_reason(provider: &Providers, f: &Fixture, code: &str) -> AppError {
    let reason = provider
        .last_startup_fallback_reason()
        .unwrap()
        .expect("retained fallback cause");
    assert_eq!(reason.code, code);
    assert!(!reason.message.is_empty() && reason.message.len() <= 1024);
    assert!(!reason.message.chars().any(char::is_control));
    for forbidden in [
        "HOSTILE-STDERR-SENTINEL",
        "ignore policy",
        f.base.to_str().unwrap(),
    ] {
        assert!(
            !reason.message.contains(forbidden),
            "diagnostic must not disclose child output"
        );
    }
    for pid in f.record_pids() {
        let record = f.record(pid);
        for variable in record["env"].as_array().unwrap() {
            if let Some(key) = variable.as_str().unwrap().strip_prefix("LLAMA_API_KEY=") {
                assert!(
                    !reason.message.contains(key),
                    "diagnostic must not disclose fake credentials"
                );
            }
        }
    }
    reason
}

/// Keep the launching thread alive until the runner is returned/stopped: its
/// Linux parent-death signal follows that thread, not an arbitrary test worker.
fn observe_vulkan_window(f: &Fixture, scenario: &str) -> AppResult<Runner> {
    let (record, _) = f.model("window:q4", scenario);
    let mut config = f.config();
    config.backend = Backend::Vulkan;
    let returned = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let observer = scope.spawn(|| {
            let pid = wait_for_health(f);
            let healthy = Instant::now();
            std::thread::sleep(Duration::from_millis(100));
            let mut premature = returned.load(Ordering::Acquire);
            fs::write(f.base.join("records/startup-continue"), b"continue").unwrap();
            if matches!(
                scenario,
                "gpu-window-fragmented" | "gpu-window-partial" | "gpu-window-duplicate"
            ) {
                wait_for_file(&f.base.join("records/startup-fragment"));
                std::thread::sleep(Duration::from_millis(100));
                premature |= returned.load(Ordering::Acquire);
                fs::write(f.base.join("records/startup-finish"), b"finish").unwrap();
            }
            wait_for_file(&f.base.join("records/startup-written"));
            (pid, healthy, premature)
        });
        let result = Runner::launch(&config, &f.model_path(&record), deadline(10));
        let completed = Instant::now();
        returned.store(true, Ordering::Release);
        let (pid, healthy, premature) = observer.join().unwrap();
        assert!(
            !premature,
            "{scenario}: startup returned before the gated evidence"
        );
        if !matches!(scenario, "gpu-window-eof" | "gpu-window-exit") {
            assert!(
                completed.saturating_duration_since(healthy) >= Duration::from_millis(900),
                "{scenario}: a candidate must remain provisional for the full post-health window"
            );
        }
        if scenario == "gpu-window-continuous" {
            assert!(
                completed.saturating_duration_since(healthy) < Duration::from_secs(4),
                "complete unrelated records must not slide the observation window"
            );
        }
        assert_backend_args(&f.record(pid), Backend::Vulkan);
        result
    })
}

/// One copied fake executable per test process (owner 0755, single link).
fn shared_exe() -> PathBuf {
    static EXE: OnceLock<PathBuf> = OnceLock::new();
    EXE.get_or_init(|| {
        for entry in fs::read_dir(std::env::temp_dir()).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(pid) = name
                .strip_prefix("srt-bin-")
                .and_then(|p| p.parse::<i32>().ok())
            {
                // SAFETY: signal 0 only probes existence.
                if unsafe { libc::kill(pid, 0) } != 0 {
                    let _ = fs::remove_dir_all(entry.path());
                }
            }
        }
        let dir = std::env::temp_dir().join(format!("srt-bin-{}", std::process::id()));
        private_dir(&dir);
        let exe = dir.join("llama-server");
        fs::copy(std::env::current_exe().unwrap(), &exe).unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();
        exe
    })
    .clone()
}

#[test]
fn carried_lease_is_consumed_once_and_loaded_runner_blocks_all_removal() {
    let f = Fixture::new();
    let (record, entry) = f.model("leased:q4", "normal");
    let (other, other_entry) = f.model("other:q4", "normal");
    let held = runner::verify_model_leased(&f.roots, &record, deadline(5), &|| false).unwrap();
    let runner =
        Runner::launch_verified(&f.config(), &held, deadline(5), Arc::new(|| false)).unwrap();
    assert!(Runner::launch_verified(&f.config(), &held, deadline(5), Arc::new(|| false)).is_err());
    assert_eq!(
        runner::remove_model_with(
            &f.roots,
            &other.name,
            &other.sha256,
            &[other_entry],
            deadline(5),
            &|| false
        )
        .unwrap_err()
        .code,
        "RUNNER_BUSY"
    );
    runner.stop().unwrap();
    // Confirmed cleanup returns the same lease for one pre-dispatch fallback.
    let fallback =
        Runner::launch_verified(&f.config(), &held, deadline(5), Arc::new(|| false)).unwrap();
    fallback.stop().unwrap();
    drop(held);
    runner::remove_model_with(
        &f.roots,
        &record.name,
        &record.sha256,
        &[entry],
        deadline(5),
        &|| false,
    )
    .unwrap();
}

#[test]
fn removal_revalidates_held_directory_files_and_install_lock_before_unlink() {
    for target in ["directory", "model", "marker", "lock"] {
        let f = Fixture::new();
        let (record, entry) = f.model("replaced:q4", "normal");
        let marker = f.models().join(format!("{}.json", record.sha256));
        let checks = AtomicU64::new(0);
        let cancelled = || {
            // The fourth budget check is after hashing this small synthetic file.
            if checks.fetch_add(1, Ordering::SeqCst) == 3 {
                match target {
                    "directory" => {
                        fs::rename(f.models(), f.models().with_extension("old")).unwrap();
                        private_dir(&f.models());
                    }
                    "model" => {
                        let path = f.model_path(&record);
                        let bytes = fs::read(&path).unwrap();
                        fs::rename(&path, path.with_extension("old")).unwrap();
                        write_mode(&path, &bytes, 0o600);
                    }
                    "marker" => {
                        let bytes = fs::read(&marker).unwrap();
                        fs::rename(&marker, marker.with_extension("old")).unwrap();
                        write_mode(&marker, &bytes, 0o600);
                    }
                    "lock" => {
                        let path = f.roots.data.join(".smith-next-install.lock");
                        fs::rename(&path, path.with_extension("old")).unwrap();
                        write_mode(&path, b"", 0o600);
                    }
                    _ => unreachable!(),
                }
            }
            false
        };
        assert_eq!(
            runner::remove_model_with(
                &f.roots,
                &record.name,
                &record.sha256,
                &[entry],
                deadline(5),
                &cancelled
            )
            .unwrap_err()
            .code,
            "MODEL_STORE_UNSAFE",
            "{target}"
        );
        if target == "directory" {
            assert!(f
                .models()
                .with_extension("old")
                .join(format!("{}.json", record.sha256))
                .exists());
        } else {
            assert!(marker.exists());
        }
    }
}

#[test]
fn removal_stopping_after_marker_unlink_reports_partial_and_preserves_orphan() {
    let f = Fixture::new();
    let (record, entry) = f.model("partial:q4", "normal");
    let marker = f.models().join(format!("{}.json", record.sha256));
    let error = runner::remove_model_with(
        &f.roots,
        &record.name,
        &record.sha256,
        &[entry],
        deadline(5),
        &|| !marker.exists(),
    )
    .unwrap_err();
    assert_eq!(error.code, "REMOVE_PARTIAL");
    assert!(!marker.exists());
    assert!(f.model_path(&record).exists());
    assert!(runner::list_models_with(&f.models(), &[entry])
        .unwrap()
        .is_empty());
    assert!(runner::remove_model_with(
        &f.roots,
        &record.name,
        &record.sha256,
        &[entry],
        deadline(5),
        &|| false
    )
    .is_err());
    assert!(f.model_path(&record).exists());
}

#[test]
fn model_removal_is_exact_safe_and_blocked_by_preverification_lease() {
    let f = Fixture::new();
    let (record, entry) = f.model("remove:q4", "normal");
    let (other, _) = f.model("keep:q4", "normal");
    let held = runner::verify_model_leased(&f.roots, &record, deadline(5), &|| false).unwrap();
    let remove = || {
        runner::remove_model_with(
            &f.roots,
            &record.name,
            &record.sha256,
            &[entry],
            deadline(5),
            &|| false,
        )
    };
    assert_eq!(remove().unwrap_err().code, "RUNNER_BUSY");
    assert!(f.model_path(&record).exists());
    drop(held);
    remove().unwrap();
    assert!(!f.model_path(&record).exists());
    assert!(!f.models().join(format!("{}.json", record.sha256)).exists());
    assert!(f.model_path(&other).exists());
    assert!(remove().is_err());
}

#[test]
fn removal_rejects_lock_content_symlink_and_hardlink_without_unpublishing() {
    use std::os::fd::AsRawFd;
    for scenario in ["lock", "tamper", "symlink", "hardlink", "marker", "cancel"] {
        let f = Fixture::new();
        let (record, entry) = f.model("unsafe:q4", "normal");
        let marker = f.models().join(format!("{}.json", record.sha256));
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(f.roots.data.join(".smith-next-install.lock"))
            .unwrap();
        fs::set_permissions(
            f.roots.data.join(".smith-next-install.lock"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        match scenario {
            "lock" => {
                assert_eq!(
                    unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
                    0
                );
            }
            "tamper" => {
                let bytes = fs::read(f.model_path(&record)).unwrap();
                fs::write(f.model_path(&record), vec![b'x'; bytes.len()]).unwrap();
            }
            "symlink" => {
                fs::remove_file(f.model_path(&record)).unwrap();
                symlink(&marker, f.model_path(&record)).unwrap();
            }
            "hardlink" => {
                fs::hard_link(f.model_path(&record), f.models().join("other")).unwrap();
            }
            "marker" => {
                fs::write(&marker, b"{}").unwrap();
            }
            _ => {}
        }
        let result = runner::remove_model_with(
            &f.roots,
            &record.name,
            &record.sha256,
            &[entry],
            deadline(5),
            &|| scenario == "cancel",
        );
        assert!(result.is_err(), "{scenario}");
        assert!(marker.exists(), "{scenario}");
        assert!(
            f.model_path(&record).symlink_metadata().is_ok(),
            "{scenario}"
        );
    }
}

struct Fixture {
    base: PathBuf,
    roots: Roots,
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "srt-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let roots = Roots {
            data: base.join("data"),
            config: base.join("config"),
            runtime: base.join("run"),
        };
        for p in [
            &base,
            &roots.data,
            &roots.config,
            &roots.runtime,
            &roots.data.join("smith-next"),
            &roots.data.join("smith-next/models"),
            &base.join("records"),
        ] {
            private_dir(p);
        }
        Self { base, roots }
    }
    fn models(&self) -> PathBuf {
        runner::models_dir(&self.roots)
    }
    /// Install one fake model whose first content line selects fake behavior.
    fn model(&self, name: &str, scenario: &str) -> (ModelRecord, CatalogEntry) {
        let content = format!("{scenario}\n{name}\n");
        let entry = CatalogEntry {
            name: leak(name.into()),
            display_name: "Fixture model",
            family: "Fixture",
            tier: ModelTier::Compact,
            parameter_label: "Fixture",
            sha256: leak(sha(content.as_bytes())),
            size: content.len() as u64,
            license: "apache-2.0",
            source: "https://huggingface.co/fixture/fake",
            url: "https://huggingface.co/fixture/fake/resolve/0/fake.gguf",
        };
        let record = entry.record();
        write_mode(
            &self.models().join(format!("{}.gguf", record.sha256)),
            content.as_bytes(),
            0o600,
        );
        write_mode(
            &self.models().join(format!("{}.json", record.sha256)),
            &marker_bytes(&record),
            0o600,
        );
        (record, entry)
    }
    fn model_path(&self, record: &ModelRecord) -> PathBuf {
        self.models().join(format!("{}.gguf", record.sha256))
    }
    fn config(&self) -> RunnerConfig {
        RunnerConfig {
            executable: shared_exe(),
            runtime_parent: runner::runner_parent(&self.roots),
            lock: runner::lock_path(&self.roots),
            context: 4096,
            threads: 2,
            backend: Backend::Cpu,
            verified_runtime: None,
        }
    }
    fn start(&self, scenario: &str) -> (Runner, ModelRecord) {
        let (record, _) = self.model(&format!("fake-{scenario}:q4"), scenario);
        let runner =
            Runner::launch(&self.config(), &self.model_path(&record), deadline(10)).unwrap();
        (runner, record)
    }
    fn record(&self, pid: u32) -> Value {
        serde_json::from_slice(
            &fs::read(self.base.join(format!("records/record-{pid}.json"))).unwrap(),
        )
        .unwrap()
    }
    fn record_pids(&self) -> Vec<u32> {
        let mut pids: Vec<u32> = fs::read_dir(self.base.join("records"))
            .unwrap()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                name.strip_prefix("record-")?
                    .strip_suffix(".json")?
                    .parse()
                    .ok()
            })
            .collect();
        pids.sort();
        pids
    }
    fn request(&self, pid: u32, n: usize) -> Value {
        serde_json::from_slice(
            &fs::read(self.base.join(format!("records/request-{pid}-{n}.json"))).unwrap(),
        )
        .unwrap()
    }
    fn runner_dirs(&self) -> Vec<PathBuf> {
        fs::read_dir(runner::runner_parent(&self.roots))
            .map(|d| {
                d.flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        p.file_name()
                            .unwrap()
                            .to_string_lossy()
                            .starts_with("runner-")
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    /// Publish a content-addressed client release and select it.
    fn release(&self, files: &[(&str, Vec<u8>, u32)]) -> PathBuf {
        self.release_backends(files, false)
    }
    fn release_backends(&self, files: &[(&str, Vec<u8>, u32)], vulkan: bool) -> PathBuf {
        let client = self.roots.data.join("smith-next/client");
        private_dir(&client);
        private_dir(&client.join("releases"));
        let mut entries = BTreeMap::new();
        entries.insert(
            "smith_backend".to_string(),
            json!({"mode":448,"type":"directory"}),
        );
        entries.insert(
            "smith_backend/runtime".to_string(),
            json!({"mode":448,"type":"directory"}),
        );
        entries.insert(
            "smith_backend/runtime/cpu".to_string(),
            json!({"mode":448,"type":"directory"}),
        );
        for (name, bytes, mode) in files {
            entries.insert(
                format!("smith_backend/runtime/cpu/{name}"),
                json!({"mode":mode,"sha256":sha(bytes),"type":"file"}),
            );
        }
        if vulkan {
            entries.insert(
                "smith_backend/runtime/vulkan".to_string(),
                json!({"mode":448,"type":"directory"}),
            );
            for (name, bytes, mode) in files {
                entries.insert(
                    format!("smith_backend/runtime/vulkan/{name}"),
                    json!({"mode":mode,"sha256":sha(bytes),"type":"file"}),
                );
            }
        }
        let mut manifest = serde_json::to_vec(&json!({"entries":entries,"schema":1})).unwrap();
        manifest.push(b'\n');
        let id = sha(&manifest);
        let release = client.join("releases").join(&id);
        let runtime = release.join("smith_backend/runtime");
        let cpu = runtime.join("cpu");
        private_dir(&release);
        private_dir(&release.join("smith_backend"));
        private_dir(&runtime);
        private_dir(&cpu);
        for (name, bytes, mode) in files {
            write_mode(&cpu.join(name), bytes, *mode);
        }
        if vulkan {
            let gpu = runtime.join("vulkan");
            private_dir(&gpu);
            for (name, bytes, mode) in files {
                write_mode(&gpu.join(name), bytes, *mode);
            }
        }
        write_mode(&release.join(".smith-next-manifest.json"), &manifest, 0o600);
        let current = client.join("current");
        let _ = fs::remove_file(&current);
        symlink(format!("releases/{id}"), &current).unwrap();
        cpu
    }
    fn real_release(&self) -> PathBuf {
        let exe = fs::read(shared_exe()).unwrap();
        self.release_backends(
            &[
                ("llama-server", exe, 0o755),
                ("libggml-fake.so", b"fake library".to_vec(), 0o644),
            ],
            true,
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}
fn deadline(secs: u64) -> Instant {
    Instant::now() + Duration::from_secs(secs)
}
fn user(content: &str) -> Message {
    Message {
        role: "user".into(),
        content: content.into(),
    }
}
fn assistant(content: &str) -> Message {
    Message {
        role: "assistant".into(),
        content: content.into(),
    }
}

#[test]
fn spawn_uses_fixed_args_cleared_env_private_socket_and_own_group() {
    let f = Fixture::new();
    let (mut runner, record) = f.start("ok");
    let pid = runner.pid();
    let r = f.record(pid);
    let sock = r["args"][3].as_str().unwrap().to_string();
    let expected: Vec<String> = [
        "--model",
        f.model_path(&record).to_str().unwrap(),
        "--host",
        &sock,
        "--offline",
        "--no-webui",
        "--no-slots",
        "--no-agent",
        "--no-mmproj-auto",
        "--no-cache-idle-slots",
        "--cache-ram",
        "0",
        "--ctx-checkpoints",
        "0",
        "--ctx-size",
        "4096",
        "--parallel",
        "1",
        "--threads",
        "2",
        "--threads-http",
        "2",
        "--timeout",
        "180",
        "--alias",
        &record.sha256,
        "--device",
        "none",
        "--gpu-layers",
        "0",
        "--fit",
        "off",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(r["args"], json!(expected));
    let env = r["env"].as_array().unwrap();
    assert_eq!(env.len(), 1, "{env:?}");
    let key = env[0]
        .as_str()
        .unwrap()
        .strip_prefix("LLAMA_API_KEY=")
        .unwrap();
    assert!(key.len() == 32 && key.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(r["cwd"], json!(shared_exe().parent().unwrap()));
    assert_eq!(r["pgid"], json!(pid));
    assert_eq!(r["ppid"], json!(std::process::id()));
    let sock = PathBuf::from(sock);
    let dir = sock.parent().unwrap();
    assert_eq!(dir.parent().unwrap(), runner::runner_parent(&f.roots));
    let name = dir.file_name().unwrap().to_str().unwrap();
    assert!(name.starts_with("runner-") && name.len() == 39);
    assert_eq!(fs::metadata(dir).unwrap().mode() & 0o7777, 0o700);
    assert_eq!(
        fs::metadata(runner::lock_path(&f.roots)).unwrap().mode() & 0o7777,
        0o600
    );
    let reply = runner.chat(&[user("hello")], 1024, deadline(10)).unwrap();
    assert_eq!(reply, "reply:hello", "reasoning_content is ignored");
    let request = f.request(pid, 1);
    assert_eq!(
        request,
        json!({"model":record.sha256,"messages":[{"role":"user","content":"hello"}],"stream":false,"max_tokens":1024})
    );
    runner.stop().unwrap();
    wait_gone(pid);
    assert!(!dir.exists());
    assert!(f.runner_dirs().is_empty());
}

#[test]
fn fake_b11429_logger_suppresses_default_info_and_models_decorations() {
    let old: Vec<String> = [
        "--device",
        "Vulkan0",
        "--split-mode",
        "none",
        "--gpu-layers",
        "all",
        "--fit",
        "off",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    let message = "load_tensors: offloaded 33/33 layers to GPU";
    assert!(
        fake::library_info(&old, message).is_empty(),
        "old launch must not confirm offload"
    );
    let mut plain = old;
    plain.extend(
        [
            "--log-verbosity",
            "4",
            "--no-log-prefix",
            "--no-log-timestamps",
            "--no-log-jsonl",
        ]
        .into_iter()
        .map(str::to_string),
    );
    let canonical = format!("{message}\n").into_bytes();
    assert_eq!(fake::library_info(&plain, message), canonical);
    for omitted in ["--no-log-prefix", "--no-log-timestamps"] {
        let decorated: Vec<String> = plain
            .iter()
            .filter(|arg| arg.as_str() != omitted)
            .cloned()
            .collect();
        let output = String::from_utf8(fake::library_info(&decorated, message)).unwrap();
        assert_ne!(
            acceleration::actual_mode(Backend::Vulkan, &output),
            Ok(ActualMode::Gpu)
        );
    }
    let mut jsonl: Vec<String> = plain
        .into_iter()
        .filter(|arg| arg != "--no-log-jsonl")
        .collect();
    jsonl.push("--log-jsonl".into());
    let output = String::from_utf8(fake::library_info(&jsonl, message)).unwrap();
    assert_ne!(
        acceleration::actual_mode(Backend::Vulkan, &output),
        Ok(ActualMode::Gpu)
    );
    jsonl.push("--no-log-jsonl".into());
    assert_eq!(fake::library_info(&jsonl, message), canonical);
}

#[test]
fn vulkan_startup_observes_full_window_and_accepts_only_complete_fragmented_evidence() {
    for scenario in ["gpu-window-full", "gpu-window-fragmented"] {
        let f = Fixture::new();
        let mut runner = observe_vulkan_window(&f, scenario).unwrap();
        assert_eq!(
            runner.chat(&[user("hello")], 32, deadline(5)).unwrap(),
            "reply:hello"
        );
        let pid = runner.pid();
        runner.stop().unwrap();
        wait_gone(pid);
        assert!(f.runner_dirs().is_empty());
    }
}

#[test]
fn vulkan_startup_continuous_complete_records_do_not_slide_window() {
    let f = Fixture::new();
    let runner = observe_vulkan_window(&f, "gpu-window-continuous").unwrap();
    assert!(
        runner
            .diagnostics()
            .matches("server: startup heartbeat\n")
            .count()
            >= 2,
        "the fake must actually emit continued complete records"
    );
    let pid = runner.pid();
    runner.stop().unwrap();
    wait_gone(pid);
    assert!(f.runner_dirs().is_empty());
}

#[test]
fn vulkan_startup_rejects_partial_unterminated_duplicate_and_overflow_during_window() {
    for scenario in [
        "gpu-window-partial",
        "gpu-window-unterminated",
        "gpu-window-trailing",
        "gpu-window-duplicate",
        "gpu-window-overflow",
    ] {
        let f = Fixture::new();
        let error = observe_vulkan_window(&f, scenario)
            .err()
            .expect("invalid startup must fail");
        assert_eq!(error.code, "GPU_PREPARATION", "{scenario}");
        assert_eq!(f.record_pids().len(), 1);
        for pid in f.record_pids() {
            wait_gone(pid);
            assert!(!f
                .base
                .join(format!("records/request-{pid}-1.json"))
                .exists());
        }
        assert!(f.runner_dirs().is_empty());
        f.start("ok").0.stop().unwrap();
    }
}

#[test]
fn vulkan_startup_rejects_child_exit_and_stderr_eof_after_health() {
    for scenario in ["gpu-window-eof", "gpu-window-exit"] {
        let f = Fixture::new();
        let error = observe_vulkan_window(&f, scenario)
            .err()
            .expect("capture/liveness failure must fail");
        assert_eq!(error.code, "GPU_PREPARATION", "{scenario}");
        assert_eq!(f.record_pids().len(), 1);
        wait_gone(f.record_pids()[0]);
        assert!(f.runner_dirs().is_empty());
    }
}

#[test]
fn vulkan_post_health_window_obeys_cancellation_and_original_deadline() {
    for cancel in [false, true] {
        let f = Fixture::new();
        let (record, _) = f.model("window-budget:q4", "ok");
        let mut config = f.config();
        config.backend = Backend::Vulkan;
        let verified =
            runner::verify_model_leased(&f.roots, &record, deadline(5), &|| false).unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let cancellation: runner::Cancellation = Arc::new(move || flag.load(Ordering::Acquire));
        std::thread::scope(|scope| {
            let trigger = scope.spawn(|| {
                let pid = wait_for_health(&f);
                std::thread::sleep(Duration::from_millis(100));
                if cancel {
                    cancelled.store(true, Ordering::Release);
                }
                pid
            });
            let started = Instant::now();
            let budget = if cancel {
                Duration::from_secs(10)
            } else {
                Duration::from_millis(700)
            };
            let result =
                Runner::launch_verified(&config, &verified, started + budget, cancellation);
            let pid = trigger.join().unwrap();
            let error = result
                .err()
                .expect("provisional evidence cannot bypass cancellation/deadline");
            assert_eq!(
                error.code,
                if cancel {
                    "RUNNER_CANCELLED"
                } else {
                    "PROVIDER_TIMEOUT"
                }
            );
            assert!(started.elapsed() < Duration::from_secs(3));
            wait_gone(pid);
            assert!(f.runner_dirs().is_empty());
        });
        drop(verified);
        f.start("ok").0.stop().unwrap();
    }
}

#[test]
fn readiness_waits_for_load_and_bounded_failures_clean_up() {
    let f = Fixture::new();
    let started = Instant::now();
    let (runner, _) = f.start("slow-start");
    assert!(started.elapsed() >= Duration::from_millis(350));
    runner.stop().unwrap();

    let (record, _) = f.model("fake-never:q4", "never-ready");
    let started = Instant::now();
    let error = Runner::launch(
        &f.config(),
        &f.model_path(&record),
        Instant::now() + Duration::from_millis(700),
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "PROVIDER_TIMEOUT");
    assert!(started.elapsed() < Duration::from_secs(4));
    for pid in f.record_pids() {
        wait_gone(pid);
    }
    assert!(f.runner_dirs().is_empty());

    let (record, _) = f.model("fake-exit:q4", "exit-early");
    let started = Instant::now();
    let error = Runner::launch(&f.config(), &f.model_path(&record), deadline(30))
        .err()
        .unwrap();
    assert_eq!(error.code, "LLM_UNAVAILABLE");
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(f.runner_dirs().is_empty());
    // The lock was released by each failure.
    f.start("ok").0.stop().unwrap();
}

#[test]
fn identity_mismatch_and_tool_calls_are_rejected_and_stop_the_runner() {
    let f = Fixture::new();
    for scenario in ["mismatch", "tools"] {
        let (mut runner, _) = f.start(scenario);
        let pid = runner.pid();
        let error = runner.chat(&[user("hi")], 1024, deadline(10)).unwrap_err();
        assert_eq!(error.code, "PROVIDER_PROTOCOL", "{scenario}");
        assert!(!runner.is_running());
        wait_gone(pid);
        assert!(f.runner_dirs().is_empty());
        assert_eq!(
            runner
                .chat(&[user("hi")], 1024, deadline(10))
                .unwrap_err()
                .code,
            "LLM_UNAVAILABLE",
            "never reused"
        );
    }
}

#[test]
fn chunked_responses_and_server_context_errors() {
    let f = Fixture::new();
    let (mut runner, _) = f.start("chunked");
    assert_eq!(
        runner.chat(&[user("chunks")], 1024, deadline(10)).unwrap(),
        "reply:chunks"
    );
    runner.stop().unwrap();
    let (mut runner, _) = f.start("context-error");
    assert_eq!(
        runner
            .chat(&[user("x")], 1024, deadline(10))
            .unwrap_err()
            .code,
        "CONTEXT_LIMIT"
    );
    assert!(runner.is_running(), "semantic refusal keeps the runner");
}

#[test]
fn context_trims_oldest_pairs_in_request_only_or_reports_limit() {
    let f = Fixture::new();
    let (mut runner, _) = f.start("ok");
    let pid = runner.pid();
    let mut messages = Vec::new();
    for i in 0..4 {
        messages.push(user(&format!("{i}{}", "u".repeat(999))));
        messages.push(assistant(&format!("{i}{}", "a".repeat(999))));
    }
    messages.push(user("final"));
    let original = serde_json::to_value(&messages).unwrap();
    runner.chat(&messages, 1024, deadline(10)).unwrap();
    assert_eq!(serde_json::to_value(&messages).unwrap(), original);
    // Budget 4096 - 1024 - 32 = 3040 one-byte tokens: only the newest pair fits.
    let sent = f.request(pid, 1);
    let sent = sent["messages"].as_array().unwrap();
    assert_eq!(sent.len(), 3);
    assert!(sent[0]["content"].as_str().unwrap().starts_with('3'));
    assert_eq!(sent[2]["content"], "final");
    let error = runner
        .chat(&[user(&"x".repeat(3100))], 1024, deadline(10))
        .unwrap_err();
    assert_eq!(error.code, "CONTEXT_LIMIT");
    assert!(runner.is_running());
    assert_eq!(
        runner
            .chat(&[assistant("x")], 1024, deadline(10))
            .unwrap_err()
            .code,
        "CONTEXT_LIMIT"
    );
    runner.stop().unwrap();

    // Without render/tokenize endpoints, a conservative byte-token estimate applies.
    let (mut runner, _) = f.start("no-template");
    let pid = runner.pid();
    let mut messages = Vec::new();
    for _ in 0..4 {
        messages.push(user(&"u".repeat(1500)));
        messages.push(assistant(&"a".repeat(1500)));
    }
    messages.push(user("final"));
    runner.chat(&messages, 1024, deadline(10)).unwrap();
    let sent = f.request(pid, 1)["messages"].as_array().unwrap().len();
    assert!(sent < messages.len() && sent % 2 == 1, "{sent}");
}

#[test]
fn smith_preamble_survives_token_trimming_and_counts_toward_limit() {
    let f = Fixture::new();
    for scenario in ["ok", "no-template"] {
        let (mut runner, _) = f.start(scenario);
        let pid = runner.pid();
        let preamble = Message {
            role: "system".into(),
            content: "Smith instructions ".repeat(50),
        };
        let messages = vec![
            preamble.clone(),
            user(&"old".repeat(500)),
            assistant(&"old".repeat(500)),
            user("new user"),
            assistant("new assistant"),
            user("final"),
        ];
        let original = serde_json::to_value(&messages).unwrap();
        runner.chat(&messages, 1024, deadline(10)).unwrap();
        assert_eq!(serde_json::to_value(&messages).unwrap(), original);
        assert_eq!(
            f.request(pid, 1)["messages"],
            serde_json::to_value(vec![
                preamble.clone(),
                user("new user"),
                assistant("new assistant"),
                user("final"),
            ])
            .unwrap()
        );
        assert_eq!(
            runner
                .chat(
                    &[preamble.clone(), user(&"x".repeat(2200))],
                    1024,
                    deadline(10)
                )
                .unwrap_err()
                .code,
            "CONTEXT_LIMIT"
        );
        assert_eq!(
            runner
                .chat(
                    &[preamble.clone(), preamble, user("final")],
                    1024,
                    deadline(10)
                )
                .unwrap_err()
                .code,
            "CONTEXT_LIMIT"
        );
        runner.stop().unwrap();
    }
}

#[test]
fn deadline_expiry_stops_runner_mid_generation() {
    let f = Fixture::new();
    let (mut runner, _) = f.start("slow");
    let pid = runner.pid();
    let started = Instant::now();
    let error = runner
        .chat(
            &[user("wait")],
            1024,
            Instant::now() + Duration::from_millis(500),
        )
        .unwrap_err();
    assert_eq!(error.code, "PROVIDER_TIMEOUT");
    assert!(started.elapsed() < Duration::from_secs(4));
    assert!(!runner.is_running());
    wait_gone(pid);
    assert!(f.runner_dirs().is_empty());
}

#[test]
fn explicit_cancellation_stops_a_blocked_completion_without_waiting_for_deadline() {
    static CANCEL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    fn cancelled() -> bool {
        CANCEL.load(Ordering::Relaxed)
    }
    CANCEL.store(false, Ordering::Relaxed);
    let f = Fixture::new();
    let (record, _) = f.model("fixture-cancel:q4", "slow");
    let mut runner =
        Runner::start_cancelled(&f.config(), &f.model_path(&record), deadline(10), cancelled)
            .unwrap();
    let pid = runner.pid();
    let trigger = std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(200));
        CANCEL.store(true, Ordering::Relaxed);
    });
    let started = Instant::now();
    let error = runner.chat(&[user("hello")], 32, deadline(10)).unwrap_err();
    trigger.join().unwrap();
    assert_eq!(error.code, "RUNNER_CANCELLED");
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(!runner.is_running());
    wait_gone(pid);
}

#[test]
fn crash_during_request_is_reported_and_runner_not_reused() {
    let f = Fixture::new();
    let (mut runner, _) = f.start("crash");
    let pid = runner.pid();
    let error = runner
        .chat(&[user("boom")], 1024, deadline(10))
        .unwrap_err();
    assert_eq!(error.code, "LLM_UNAVAILABLE");
    assert!(!runner.is_running());
    wait_gone(pid);
    assert!(f.runner_dirs().is_empty());
}

#[test]
fn single_runner_lock_contention_is_runner_busy_without_waiting() {
    let f = Fixture::new();
    let (first, _) = f.start("ok");
    let (record, _) = f.model("fake-second:q4", "ok");
    let started = Instant::now();
    let error = Runner::launch(&f.config(), &f.model_path(&record), deadline(10))
        .err()
        .unwrap();
    assert_eq!(error.code, "RUNNER_BUSY");
    assert!(started.elapsed() < Duration::from_millis(500));
    let text = Runner::start(&f.config(), &f.model_path(&record), deadline(10))
        .err()
        .unwrap();
    assert!(text.starts_with("RUNNER_BUSY: "), "{text}");
    assert_eq!(f.record_pids().len(), 1, "no second child spawned");
    first.stop().unwrap();
    Runner::launch(&f.config(), &f.model_path(&record), deadline(10))
        .unwrap()
        .stop()
        .unwrap();
}

#[test]
fn stale_runner_dirs_are_removed_only_when_dead_and_exact() {
    let f = Fixture::new();
    let parent = runner::runner_parent(&f.roots);
    private_dir(&parent);
    let dead = parent.join(format!("runner-{}", "1".repeat(32)));
    private_dir(&dead);
    drop(UnixListener::bind(dead.join("llama.sock")).unwrap());
    let empty = parent.join(format!("runner-{}", "2".repeat(32)));
    private_dir(&empty);
    let live = parent.join(format!("runner-{}", "3".repeat(32)));
    private_dir(&live);
    let _listener = UnixListener::bind(live.join("llama.sock")).unwrap();
    let foreign = parent.join(format!("runner-{}", "4".repeat(32)));
    private_dir(&foreign);
    fs::write(foreign.join("notes"), b"keep").unwrap();
    let loose = parent.join(format!("runner-{}", "5".repeat(32)));
    fs::create_dir(&loose).unwrap();
    fs::set_permissions(&loose, fs::Permissions::from_mode(0o755)).unwrap();
    let unrelated = parent.join("runner-keep");
    private_dir(&unrelated);

    let (runner, _) = f.start("ok");
    assert!(!dead.exists() && !empty.exists());
    for kept in [&live, &foreign, &loose, &unrelated] {
        assert!(kept.exists(), "{}", kept.display());
    }
    runner.stop().unwrap();
}

#[test]
fn peer_pid_mismatch_is_fatal_and_group_is_killed() {
    let f = Fixture::new();
    let nonce = format!("{}", NEXT.fetch_add(1, Ordering::Relaxed));
    let (record, _) = f.model("fake-peer:q4", &format!("peer:{nonce}"));
    let started = Instant::now();
    let error = Runner::launch(&f.config(), &f.model_path(&record), deadline(20))
        .err()
        .unwrap();
    assert_eq!(error.code, "RUNNER_UNSAFE");
    assert!(started.elapsed() < Duration::from_secs(5));
    let grandchild: u32 =
        fs::read_to_string(f.base.join(format!("records/grandchild-{nonce}.txt")))
            .unwrap()
            .parse()
            .unwrap();
    wait_gone(grandchild);
    wait_gone(*f.record_pids().last().unwrap());
    assert!(f.runner_dirs().is_empty());
}

#[test]
fn drop_and_stop_escalation_clean_up_child_dir_and_lock() {
    let f = Fixture::new();
    let (runner, _) = f.start("ok");
    let pid = runner.pid();
    drop(runner);
    wait_gone(pid);
    assert!(f.runner_dirs().is_empty());

    let (runner, _) = f.start("ignore-term");
    let pid = runner.pid();
    let started = Instant::now();
    runner.stop().unwrap();
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(1900) && elapsed < Duration::from_millis(3500),
        "{elapsed:?}"
    );
    wait_gone(pid);
    assert!(f.runner_dirs().is_empty());
    f.start("ok").0.stop().unwrap();
}

#[test]
fn stderr_capture_is_bounded() {
    let f = Fixture::new();
    let (mut runner, _) = f.start("noisy");
    assert_eq!(
        runner.chat(&[user("a")], 1024, deadline(10)).unwrap(),
        "reply:a"
    );
    let end = Instant::now() + Duration::from_secs(3);
    while runner.diagnostics().len() < 16 * 1024 && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(runner.diagnostics().len(), 16 * 1024);
}

#[test]
fn runner_rejects_unsafe_inputs_before_spawning() {
    let f = Fixture::new();
    let (record, _) = f.model("fake-ok:q4", "ok");
    let renamed = f.models().join("model.gguf");
    fs::copy(f.model_path(&record), &renamed).unwrap();
    fs::set_permissions(&renamed, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        Runner::launch(&f.config(), &renamed, deadline(5))
            .err()
            .unwrap()
            .code,
        "MODEL_UNAVAILABLE"
    );
    let mut config = f.config();
    config.threads = 0;
    assert!(Runner::launch(&config, &f.model_path(&record), deadline(5)).is_err());
    let mut config = f.config();
    let writable = f.base.join("bin");
    private_dir(&writable);
    fs::copy(shared_exe(), writable.join("llama-server")).unwrap();
    fs::set_permissions(
        writable.join("llama-server"),
        fs::Permissions::from_mode(0o775),
    )
    .unwrap();
    config.executable = writable.join("llama-server");
    assert_eq!(
        Runner::launch(&config, &f.model_path(&record), deadline(5))
            .err()
            .unwrap()
            .code,
        "RUNTIME_UNSAFE"
    );
    assert_eq!(
        Runner::launch(&f.config(), &f.model_path(&record), Instant::now())
            .err()
            .unwrap()
            .code,
        "PROVIDER_TIMEOUT"
    );
    assert!(f.record_pids().is_empty());
}

#[test]
fn list_models_accepts_only_exact_canonical_catalog_records() {
    let f = Fixture::new();
    let missing = f.base.join("absent");
    assert!(list_models_with(&missing, &[]).unwrap().is_empty());
    assert!(
        list_models(&f.models()).unwrap().is_empty(),
        "production catalog: fake files ineligible"
    );
    let (b, eb) = f.model("b-model:q4", "ok");
    let (a, ea) = f.model("a-model:q4", "ok");
    let (_, _other) = f.model("not-curated:q4", "ok");
    fs::write(f.models().join(".staging-download"), b"partial").unwrap();
    fs::write(
        f.models().join(format!(".{}.json", "c".repeat(64))),
        b"junk",
    )
    .unwrap();
    fs::write(f.models().join("README"), b"ignored").unwrap();
    let cat = catalog(vec![eb, ea]);
    assert_eq!(
        list_models_with(&f.models(), cat).unwrap(),
        vec![a.clone(), b.clone()]
    );

    let marker = f.models().join(format!("{}.json", a.sha256));
    let data = f.model_path(&a);
    let canonical = fs::read(&marker).unwrap();
    let mutations: Vec<Case> = vec![
        (
            "non-canonical marker",
            Box::new(|| {
                write_mode(
                    &marker,
                    &[&canonical[..canonical.len() - 1], b" \n"].concat(),
                    0o600,
                )
            }),
            Box::new(|| write_mode(&marker, &canonical, 0o600)),
        ),
        (
            "marker mode",
            Box::new(|| fs::set_permissions(&marker, fs::Permissions::from_mode(0o644)).unwrap()),
            Box::new(|| fs::set_permissions(&marker, fs::Permissions::from_mode(0o600)).unwrap()),
        ),
        (
            "data missing",
            Box::new(|| fs::rename(&data, f.base.join("held")).unwrap()),
            Box::new(|| fs::rename(f.base.join("held"), &data).unwrap()),
        ),
        (
            "data mode",
            Box::new(|| fs::set_permissions(&data, fs::Permissions::from_mode(0o644)).unwrap()),
            Box::new(|| fs::set_permissions(&data, fs::Permissions::from_mode(0o600)).unwrap()),
        ),
        (
            "data hard link",
            Box::new(|| fs::hard_link(&data, f.base.join("link")).unwrap()),
            Box::new(|| fs::remove_file(f.base.join("link")).unwrap()),
        ),
        (
            "data symlink",
            Box::new(|| {
                fs::rename(&data, f.base.join("held")).unwrap();
                symlink(f.base.join("held"), &data).unwrap();
            }),
            Box::new(|| {
                fs::remove_file(&data).unwrap();
                fs::rename(f.base.join("held"), &data).unwrap();
            }),
        ),
        (
            "data size",
            Box::new(|| {
                let mut bytes = fs::read(&data).unwrap();
                bytes.push(b'!');
                write_mode(&data, &bytes, 0o600);
            }),
            Box::new(|| {
                let mut bytes = fs::read(&data).unwrap();
                bytes.pop();
                write_mode(&data, &bytes, 0o600);
            }),
        ),
        (
            "marker for another sha",
            Box::new(|| {
                write_mode(
                    &f.models().join(format!("{}.json", "d".repeat(64))),
                    &canonical,
                    0o600,
                )
            }),
            Box::new(|| {
                fs::remove_file(f.models().join(format!("{}.json", "d".repeat(64)))).unwrap()
            }),
        ),
        (
            "unsafe directory",
            Box::new(|| {
                fs::set_permissions(f.models(), fs::Permissions::from_mode(0o755)).unwrap()
            }),
            Box::new(|| {
                fs::set_permissions(f.models(), fs::Permissions::from_mode(0o700)).unwrap()
            }),
        ),
    ];
    for (label, break_it, fix_it) in mutations {
        break_it();
        assert!(list_models_with(&f.models(), cat).is_err(), "{label}");
        fix_it();
        assert_eq!(
            list_models_with(&f.models(), cat).unwrap().len(),
            2,
            "{label} restored"
        );
    }
    // Wrong catalog size/license is not an exact match and is simply ineligible.
    let mut sized = ea;
    sized.size += 1;
    let mut licensed = eb;
    licensed.license = "mit";
    assert!(
        list_models_with(&f.models(), catalog(vec![sized, licensed]))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn list_models_rejects_duplicate_names_and_excess_records() {
    let f = Fixture::new();
    let (_, first) = f.model("same:q4", "ok");
    let (_, second) = f.model("same:q4", "other");
    assert!(list_models_with(&f.models(), catalog(vec![first])).is_err());
    assert!(list_models_with(&f.models(), catalog(vec![first, second])).is_err());

    let f = Fixture::new();
    let entries: Vec<CatalogEntry> = (0..=MODEL_LIMIT)
        .map(|i| f.model(&format!("m{i:03}:q4"), "ok").1)
        .collect();
    assert!(list_models_with(&f.models(), catalog(entries.clone())).is_err());
    fs::remove_file(f.models().join(format!("{}.json", entries[0].sha256))).unwrap();
    assert_eq!(
        list_models_with(&f.models(), catalog(entries))
            .unwrap()
            .len(),
        MODEL_LIMIT
    );
}

#[test]
fn verify_model_hashes_held_descriptor_and_rejects_tampering() {
    let f = Fixture::new();
    let (record, _) = f.model("v:q4", "ok");
    assert_eq!(
        verify_model(&f.models(), &record).unwrap(),
        f.model_path(&record)
    );
    let data = f.model_path(&record);
    let original = fs::read(&data).unwrap();
    let mut tampered = original.clone();
    tampered[0] ^= 1;
    write_mode(&data, &tampered, 0o600);
    assert!(
        verify_model(&f.models(), &record).is_err(),
        "same size, different content"
    );
    write_mode(&data, &original[..original.len() - 1], 0o600);
    assert!(verify_model(&f.models(), &record).is_err(), "size");
    write_mode(&data, &original, 0o600);
    let mut bad = record.clone();
    bad.sha256 = "../x".into();
    assert!(verify_model(&f.models(), &bad).is_err());
    fs::rename(&data, f.base.join("held")).unwrap();
    symlink(f.base.join("held"), &data).unwrap();
    assert!(verify_model(&f.models(), &record).is_err(), "symlink");
}

#[test]
fn runtime_executable_verifies_selected_release_against_manifest() {
    let f = Fixture::new();
    let code = |f: &Fixture| runner::verify_runtime(&f.roots).err().map(|e| e.code);
    assert_eq!(code(&f).as_deref(), Some("RUNTIME_MISSING"));
    assert!(runner::runtime_executable(&f.roots)
        .unwrap_err()
        .starts_with("RUNTIME_MISSING: "));
    let files = vec![
        ("llama-server", b"#!fake runner".to_vec(), 0o755),
        ("libllama.so.0", b"library".to_vec(), 0o644),
    ];
    let runtime = f.release(&files);
    assert_eq!(
        runner::runtime_executable(&f.roots).unwrap(),
        runtime.join("llama-server")
    );
    let lib = runtime.join("libllama.so.0");
    let current = f.roots.data.join("smith-next/client/current");
    let release = runtime
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let manifest = release.join(".smith-next-manifest.json");
    let cases: Vec<Case> = vec![
        (
            "tampered library",
            Box::new(|| write_mode(&lib, b"librarX", 0o644)),
            Box::new(|| write_mode(&lib, b"library", 0o644)),
        ),
        (
            "extra runtime file",
            Box::new(|| write_mode(&runtime.join(".hidden"), b"x", 0o644)),
            Box::new(|| fs::remove_file(runtime.join(".hidden")).unwrap()),
        ),
        (
            "mode drift",
            Box::new(|| fs::set_permissions(&lib, fs::Permissions::from_mode(0o755)).unwrap()),
            Box::new(|| fs::set_permissions(&lib, fs::Permissions::from_mode(0o644)).unwrap()),
        ),
        (
            "hard link",
            Box::new(|| fs::hard_link(&lib, f.base.join("lib-link")).unwrap()),
            Box::new(|| fs::remove_file(f.base.join("lib-link")).unwrap()),
        ),
        (
            "symlinked runtime file",
            Box::new(|| {
                fs::rename(&lib, f.base.join("lib-held")).unwrap();
                symlink(f.base.join("lib-held"), &lib).unwrap();
            }),
            Box::new(|| {
                fs::remove_file(&lib).unwrap();
                fs::rename(f.base.join("lib-held"), &lib).unwrap();
            }),
        ),
        (
            "manifest identity",
            Box::new(|| {
                let mut bytes = fs::read(&manifest).unwrap();
                bytes.insert(1, b' ');
                write_mode(&manifest, &bytes, 0o600);
            }),
            Box::new(|| {
                let mut bytes = fs::read(&manifest).unwrap();
                bytes.remove(1);
                write_mode(&manifest, &bytes, 0o600);
            }),
        ),
        (
            "unmanaged selector",
            Box::new(|| {
                fs::rename(&current, f.base.join("cur-held")).unwrap();
                symlink(&release, &current).unwrap();
            }),
            Box::new(|| {
                fs::remove_file(&current).unwrap();
                fs::rename(f.base.join("cur-held"), &current).unwrap();
            }),
        ),
        (
            "group-writable release",
            Box::new(|| fs::set_permissions(&release, fs::Permissions::from_mode(0o770)).unwrap()),
            Box::new(|| fs::set_permissions(&release, fs::Permissions::from_mode(0o700)).unwrap()),
        ),
    ];
    for (label, break_it, fix_it) in cases {
        break_it();
        assert_eq!(code(&f).as_deref(), Some("RUNTIME_UNSAFE"), "{label}");
        fix_it();
        assert!(runner::verify_runtime(&f.roots).is_ok(), "{label} restored");
    }
    // A selected client release published before the runtime existed.
    let f = Fixture::new();
    f.release(&[]);
    assert_eq!(code(&f).as_deref(), Some("RUNTIME_MISSING"));
    let f = Fixture::new();
    f.release(&[("libllama.so.0", b"library".to_vec(), 0o644)]);
    assert_eq!(code(&f).as_deref(), Some("RUNTIME_MISSING"));
}

#[test]
fn models_dir_is_part_of_private_root_validation() {
    let f = Fixture::new();
    assert!(f.roots.validate().is_ok());
    fs::set_permissions(f.models(), fs::Permissions::from_mode(0o755)).unwrap();
    assert!(f.roots.validate().is_err());
    fs::set_permissions(f.models(), fs::Permissions::from_mode(0o700)).unwrap();
    fs::remove_dir(f.models()).unwrap();
    symlink(f.base.join("records"), f.models()).unwrap();
    assert!(f.roots.validate().is_err());
}

fn options(cat: &'static [CatalogEntry]) -> ProviderOptions {
    ProviderOptions {
        catalog: cat,
        budget: Duration::from_secs(20),
        idle: Duration::from_secs(600),
        threads: 2,
    }
}

#[test]
fn provider_inventory_reports_runtime_state_and_curated_models() {
    let f = Fixture::new();
    let (record, entry) = f.model("fake-ok:q4", "ok");
    let provider = Providers::with_options(&f.roots, options(catalog(vec![entry])));
    let missing = provider.inventory().unwrap();
    assert!(!missing.providers.llm.ready);
    assert_eq!(missing.providers.llm.error.unwrap().code, "RUNTIME_MISSING");
    assert!(missing.models.is_empty());
    f.release(&[("llama-server", b"#!fake".to_vec(), 0o755)]);
    let ready = provider.inventory().unwrap();
    assert!(ready.providers.llm.ready && ready.providers.llm.error.is_none());
    assert_eq!(ready.models.len(), 1);
    assert_eq!(ready.models[0].name, "fake-ok:q4");
    assert_eq!(ready.models[0].digest, record.sha256);
    fs::set_permissions(f.model_path(&record), fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(provider.inventory().unwrap_err().code, "MODEL_STORE_UNSAFE");
    let production = Providers::new(&f.roots).unwrap().inventory().unwrap();
    assert!(
        production.models.is_empty(),
        "fake models are never in the production catalog"
    );
    assert_eq!(
        provider
            .chat("llama3:8b", &[user("old ollama tag")])
            .unwrap_err()
            .code,
        "MODEL_STORE_UNSAFE"
    );
    fs::set_permissions(f.model_path(&record), fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        provider
            .chat("llama3:8b", &[user("old ollama tag")])
            .unwrap_err()
            .code,
        "MODEL_UNAVAILABLE"
    );
    let unsafe_runtime = f.release(&[("llama-server", b"#!fake".to_vec(), 0o755)]);
    write_mode(&unsafe_runtime.join("llama-server"), b"#!evil", 0o755);
    let unsafe_inventory = provider.inventory().unwrap();
    assert_eq!(
        unsafe_inventory.providers.llm.error.unwrap().code,
        "RUNTIME_UNSAFE"
    );
    assert!(unsafe_inventory.models.is_empty());
}

#[test]
fn provider_prefers_full_vulkan_and_falls_back_once_only_before_completion_dispatch() {
    let f = Fixture::new();
    let exe = fs::read(shared_exe()).unwrap();
    let files = [
        ("llama-server", exe, 0o755),
        ("libggml-fake.so", b"fake library".to_vec(), 0o644),
    ];
    f.release_backends(&files, true);
    let (_, full) = f.model("gpu-full:q4", "ok");
    let full_provider = Providers::with_options(&f.roots, options(catalog(vec![full])));
    assert_eq!(full_provider.last_startup_fallback_reason().unwrap(), None);
    assert_eq!(
        full_provider.chat("gpu-full:q4", &[user("hello")]).unwrap(),
        "reply:hello"
    );
    let first = f.record_pids();
    assert_eq!(first.len(), 1);
    assert_backend_args(&f.record(first[0]), Backend::Vulkan);
    assert_eq!(full_provider.last_startup_fallback_reason().unwrap(), None);
    full_provider.shutdown();

    for (index, (scenario, expected_message)) in [
        (
            "gpu-zero",
            "Vulkan offload evidence confirms zero GPU layers",
        ),
        (
            "gpu-partial",
            "Vulkan runner did not confirm full model offload",
        ),
        ("gpu-missing", "Vulkan offload evidence is missing"),
        (
            "gpu-malformed",
            "Vulkan offload evidence has invalid layer counts",
        ),
        (
            "gpu-duplicate",
            "Vulkan offload evidence contains duplicate records",
        ),
        (
            "gpu-overflow",
            "Vulkan startup evidence exceeded its byte limit",
        ),
        ("gpu-exit-early", "Vulkan runner exited before readiness"),
        (
            "gpu-hostile",
            "Vulkan offload evidence confirms zero GPU layers",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let name = leak(format!("gpu-fallback-{index}:q4"));
        let before = f.record_pids().len();
        let (_, entry) = f.model(name, scenario);
        let provider = Providers::with_options(&f.roots, options(catalog(vec![entry])));
        assert_eq!(
            provider.chat(name, &[user("hello")]).unwrap(),
            "reply:hello"
        );
        let pids = f.record_pids();
        assert_eq!(pids.len(), before + 2, "exactly one GPU and one CPU launch");
        let gpu = pids[pids.len() - 2];
        let cpu = pids[pids.len() - 1];
        assert!(
            !pid_alive(gpu),
            "GPU child must be gone before CPU fallback"
        );
        let cpu_record = f.record(cpu);
        assert_eq!(cpu_record["sibling_alive_at_start"], false);
        assert_backend_args(&f.record(gpu), Backend::Vulkan);
        assert_backend_args(&cpu_record, Backend::Cpu);
        let reason = startup_reason(&provider, &f, "GPU_PREPARATION");
        assert_eq!(reason.message, expected_message, "{scenario}");
        provider.shutdown();
    }

    let before_deadline = f.record_pids().len();
    let (_, deadline_entry) = f.model("gpu-deadline:q4", "gpu-deadline");
    let mut deadline_options = options(catalog(vec![deadline_entry]));
    // Two seconds of GPU load plus the fixed one-second observation still leave
    // ample verification/spawn time, but not the fake CPU's seven-second load.
    deadline_options.budget = Duration::from_secs(8);
    let deadline_provider = Providers::with_options(&f.roots, deadline_options);
    let deadline_started = Instant::now();
    assert_eq!(
        deadline_provider
            .chat("gpu-deadline:q4", &[user("hello")])
            .unwrap_err()
            .code,
        "PROVIDER_TIMEOUT",
        "CPU fallback must retain only the original deadline remainder"
    );
    assert!(deadline_started.elapsed() < Duration::from_secs(10));
    let deadline_pids = f.record_pids();
    assert_eq!(deadline_pids.len(), before_deadline + 2);
    let deadline_cpu = *deadline_pids.last().unwrap();
    assert_eq!(f.record(deadline_cpu)["sibling_alive_at_start"], false);
    assert!(!pid_alive(deadline_cpu));
    assert_eq!(
        startup_reason(&deadline_provider, &f, "GPU_PREPARATION").message,
        "Vulkan offload evidence confirms zero GPU layers"
    );

    let before_crash = f.record_pids().len();
    let (_, crash) = f.model("gpu-crash:q4", "crash-once");
    let crash_provider = Providers::with_options(&f.roots, options(catalog(vec![crash])));
    assert!(crash_provider
        .chat("gpu-crash:q4", &[user("hello")])
        .is_err());
    assert_eq!(
        f.record_pids().len(),
        before_crash + 1,
        "completion failure must not replay on CPU"
    );
    assert_eq!(crash_provider.last_startup_fallback_reason().unwrap(), None);
    crash_provider.shutdown();

    let cpu_only = Fixture::new();
    let cpu_exe = fs::read(shared_exe()).unwrap();
    cpu_only.release(&[("llama-server", cpu_exe, 0o755)]);
    let (_, entry) = cpu_only.model("cpu-only:q4", "ok");
    let provider = Providers::with_options(&cpu_only.roots, options(catalog(vec![entry])));
    assert_eq!(
        provider.chat("cpu-only:q4", &[user("hello")]).unwrap(),
        "reply:hello"
    );
    assert_eq!(cpu_only.record_pids().len(), 1);
    assert_backend_args(&cpu_only.record(cpu_only.record_pids()[0]), Backend::Cpu);
    startup_reason(&provider, &cpu_only, "RUNTIME_MISSING");
    provider.shutdown();

    let tampered = Fixture::new();
    let tampered_exe = fs::read(shared_exe()).unwrap();
    let cpu = tampered.release_backends(&[("llama-server", tampered_exe, 0o755)], true);
    write_mode(
        &cpu.parent().unwrap().join("vulkan/llama-server"),
        b"tampered",
        0o755,
    );
    let (_, entry) = tampered.model("tampered-gpu:q4", "ok");
    let provider = Providers::with_options(&tampered.roots, options(catalog(vec![entry])));
    assert_eq!(
        provider
            .chat("tampered-gpu:q4", &[user("hello")])
            .unwrap_err()
            .code,
        "RUNTIME_UNSAFE"
    );
    assert!(
        tampered.record_pids().is_empty(),
        "tamper must not fall back"
    );
    assert_eq!(provider.last_startup_fallback_reason().unwrap(), None);
}

#[test]
fn provider_startup_fallback_reason_survives_success_reuse_and_idle_then_resets() {
    use std::os::fd::AsRawFd;

    for vulkan in [false, true] {
        let f = Fixture::new();
        f.release_backends(
            &[("llama-server", fs::read(shared_exe()).unwrap(), 0o755)],
            vulkan,
        );
        let (_, fallback) = f.model("fallback-history:q4", "gpu-hostile");
        let (fresh_record, fresh) = f.model("fresh-history:q4", "ok");
        let mut opts = options(catalog(vec![fallback, fresh]));
        opts.idle = Duration::ZERO;
        let provider = Providers::with_options(&f.roots, opts);
        assert_eq!(provider.last_startup_fallback_reason().unwrap(), None);
        provider.inventory().unwrap();
        assert_eq!(provider.last_startup_fallback_reason().unwrap(), None);
        assert!(
            f.record_pids().is_empty(),
            "diagnostics/inventory must not prepare a runner"
        );

        assert_eq!(
            provider
                .chat("fallback-history:q4", &[user("first")])
                .unwrap(),
            "reply:first"
        );
        let code = if vulkan {
            "GPU_PREPARATION"
        } else {
            "RUNTIME_MISSING"
        };
        let retained = startup_reason(&provider, &f, code);
        if vulkan {
            assert_eq!(
                retained.message,
                "Vulkan offload evidence confirms zero GPU layers"
            );
        }
        let pids = f.record_pids();
        assert_eq!(pids.len(), if vulkan { 2 } else { 1 });
        let cpu = *pids.iter().find(|pid| pid_alive(**pid)).unwrap();
        assert_backend_args(&f.record(cpu), Backend::Cpu);
        assert_eq!(f.record(cpu)["sibling_alive_at_start"], false);

        assert_eq!(
            provider
                .chat("fallback-history:q4", &[user("reuse")])
                .unwrap(),
            "reply:reuse"
        );
        assert_eq!(f.record_pids(), pids, "reused CPU must not retry Vulkan");
        assert_eq!(
            provider.last_startup_fallback_reason().unwrap(),
            Some(retained.clone())
        );
        let inventory = serde_json::to_value(provider.inventory().unwrap()).unwrap();
        assert_eq!(
            inventory
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["models", "providers", "voices"]
        );
        assert_eq!(
            inventory["providers"]["llm"],
            json!({"ready":true,"error":null})
        );
        assert_eq!(
            provider.last_startup_fallback_reason().unwrap(),
            Some(retained.clone())
        );

        provider.idle();
        wait_gone(cpu);
        assert_eq!(provider.idle_timeout(), None);
        assert!(f.runner_dirs().is_empty());
        assert_eq!(
            provider.last_startup_fallback_reason().unwrap(),
            Some(retained)
        );

        {
            // Parallel tests can fork with this fixture's carried lease still open.
            // Child exit/directory cleanup alone do not prove those inherited
            // descriptions have reached CLOEXEC; observe the actual lease first.
            let lock = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(runner::lock_path(&f.roots))
                .unwrap();
            let end = deadline(5);
            loop {
                // SAFETY: flock uses only this owned, live fixture descriptor.
                if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
                    // SAFETY: release our acquired lock explicitly so another fork
                    // inheriting the probe descriptor cannot prolong its ownership.
                    assert_eq!(unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_UN) }, 0);
                    break;
                }
                let error = std::io::Error::last_os_error();
                assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock, "{error}");
                assert!(
                    Instant::now() < end,
                    "idle fixture runner lease remained held"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        // Reset at fresh preparation, even if model verification fails before launch.
        let original = fs::read(f.model_path(&fresh_record)).unwrap();
        let mut corrupt = original.clone();
        corrupt[0] ^= 1;
        fs::write(f.model_path(&fresh_record), corrupt).unwrap();
        assert_eq!(
            provider
                .chat("fresh-history:q4", &[user("fresh")])
                .unwrap_err()
                .code,
            "MODEL_UNAVAILABLE"
        );
        assert_eq!(provider.last_startup_fallback_reason().unwrap(), None);
        assert_eq!(f.record_pids(), pids);
        fs::write(f.model_path(&fresh_record), original).unwrap();
        f.real_release();
        assert_eq!(
            provider.chat("fresh-history:q4", &[user("fresh")]).unwrap(),
            "reply:fresh"
        );
        assert_eq!(provider.last_startup_fallback_reason().unwrap(), None);
        assert_eq!(f.record_pids().len(), pids.len() + 1);
        provider.shutdown();
    }
}

#[test]
fn provider_startup_fallback_reason_survives_cpu_startup_and_completion_failure() {
    for vulkan in [false, true] {
        for scenario in ["gpu-cpu-start-failure", "gpu-cpu-completion-failure"] {
            let f = Fixture::new();
            f.release_backends(
                &[("llama-server", fs::read(shared_exe()).unwrap(), 0o755)],
                vulkan,
            );
            let (_, entry) = f.model("cpu-failure:q4", scenario);
            let provider = Providers::with_options(&f.roots, options(catalog(vec![entry])));
            let error = provider
                .chat("cpu-failure:q4", &[user("fail")])
                .unwrap_err();
            assert_eq!(
                error.code, "LLM_UNAVAILABLE",
                "original CPU error is authoritative: {scenario}"
            );
            let code = if vulkan {
                "GPU_PREPARATION"
            } else {
                "RUNTIME_MISSING"
            };
            let retained = startup_reason(&provider, &f, code);
            if vulkan {
                assert_eq!(
                    retained.message,
                    "Vulkan offload evidence confirms zero GPU layers"
                );
            }
            let pids = f.record_pids();
            assert_eq!(
                pids.len(),
                if vulkan { 2 } else { 1 },
                "one fallback, no completion replay"
            );
            let cpu = *pids
                .iter()
                .find(|pid| {
                    f.record(**pid)["args"]
                        .as_array()
                        .unwrap()
                        .windows(2)
                        .any(|pair| pair == [json!("--device"), json!("none")])
                })
                .unwrap();
            assert_eq!(f.record(cpu)["sibling_alive_at_start"], false);
            assert_eq!(
                f.base
                    .join(format!("records/request-{cpu}-1.json"))
                    .exists(),
                scenario == "gpu-cpu-completion-failure"
            );
            for pid in pids {
                wait_gone(pid);
            }
            assert!(f.runner_dirs().is_empty());
            provider.idle();
            assert_eq!(
                provider.last_startup_fallback_reason().unwrap(),
                Some(retained)
            );
            provider.shutdown();
        }
    }
}

#[test]
fn provider_startup_fallback_reason_requires_confirmed_cleanup() {
    let f = Fixture::new();
    f.real_release();
    let (_, entry) = f.model("cleanup-unconfirmed:q4", "gpu-cleanup-unconfirmed");
    let provider = Providers::with_options(&f.roots, options(catalog(vec![entry])));
    assert_eq!(
        provider
            .chat("cleanup-unconfirmed:q4", &[user("fail")])
            .unwrap_err()
            .code,
        "RUNTIME_UNSAFE"
    );
    let pids = f.record_pids();
    assert_eq!(
        pids.len(),
        1,
        "uncertain cleanup must not permit CPU fallback"
    );
    wait_gone(pids[0]);
    assert_eq!(provider.last_startup_fallback_reason().unwrap(), None);
    let dirs = f.runner_dirs();
    assert_eq!(dirs.len(), 1);
    assert_eq!(fs::read(dirs[0].join("unowned-note")).unwrap(), b"preserve");
}

#[test]
fn provider_startup_fallback_reason_accessor_is_nonblocking_during_slow_inference() {
    let f = Fixture::new();
    f.real_release();
    let (_, entry) = f.model("diagnostics-busy:q4", "gpu-fallback-gated");
    let provider = Providers::with_options(&f.roots, options(catalog(vec![entry])));
    std::thread::scope(|scope| {
        let worker = scope.spawn(|| {
            let result = provider.chat("diagnostics-busy:q4", &[user("held")]);
            // Shut down on the child-owning thread before that thread exits.
            provider.shutdown();
            result
        });
        let end = deadline(10);
        let cpu = loop {
            if let Some(pid) = f.record_pids().into_iter().find(|pid| {
                f.base
                    .join(format!("records/request-{pid}-1.json"))
                    .exists()
            }) {
                break pid;
            }
            if Instant::now() >= end {
                provider.cancel_owned();
                panic!("fake CPU did not enter gated inference");
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        let (tx, rx) = mpsc::channel();
        let accessor_provider = &provider;
        let accessor = scope.spawn(move || {
            tx.send(accessor_provider.last_startup_fallback_reason())
                .unwrap();
        });
        let started = Instant::now();
        let observed = rx.recv_timeout(Duration::from_millis(500));
        let elapsed = started.elapsed();
        // Always release/join before asserting, so a blocking-accessor regression
        // fails promptly without hanging a scoped thread or leaking the runner.
        fs::write(f.base.join("records/completion-release"), b"finish").unwrap();
        let result = worker.join().unwrap();
        accessor.join().unwrap();
        assert_backend_args(&f.record(cpu), Backend::Cpu);
        let busy = observed
            .expect("diagnostics must not wait for inference")
            .unwrap_err();
        assert_eq!(busy.code, "DIAGNOSTICS_BUSY");
        assert!(elapsed < Duration::from_millis(500));
        assert_eq!(result.unwrap(), "reply:held");
        wait_gone(cpu);
        assert_eq!(f.record_pids().len(), 2);
        assert_eq!(
            startup_reason(&provider, &f, "GPU_PREPARATION").message,
            "Vulkan offload evidence confirms zero GPU layers"
        );
        assert!(f.runner_dirs().is_empty());
    });
}

fn id(n: usize) -> String {
    format!("{n:032x}")
}
fn call(app: &Application, op: &str, args: Value) -> Value {
    app.call(AppRequest {
        schema_version: 1,
        service: "smith-next".into(),
        protocol: "app".into(),
        call_id: id(90000),
        op: op.into(),
        args,
    })
    .unwrap()
}
fn finished(app: &Application, n: usize) -> Value {
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        let v = call(
            app,
            "poll",
            json!({"request_id":id(n),"instance_id":app.instance_id}),
        );
        if v["status"] != "running" {
            return v;
        }
        assert!(Instant::now() < end, "operation deadline");
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn submit(app: &Application, n: usize, text: &str) -> Value {
    call(
        app,
        "submit",
        json!({"request_id":id(n),"text":text,"output":"chat"}),
    );
    finished(app, n)
}

#[test]
fn app_worker_owns_runner_restart_switch_idle_and_shutdown() {
    let f = Fixture::new();
    f.real_release();
    let (_, crash) = f.model("fake-crash:q4", "crash-once");
    let (_, other) = f.model("fake-other:q4", "ok");
    let mut opts = options(catalog(vec![crash, other]));
    opts.idle = Duration::from_millis(1500);
    let app =
        Application::open(&f.roots, Arc::new(Providers::with_options(&f.roots, opts))).unwrap();
    call(&app, "models", json!({"request_id":id(1)}));
    let inventory = finished(&app, 1);
    assert_eq!(inventory["status"], "succeeded", "{inventory}");
    assert_eq!(inventory["result"]["models"].as_array().unwrap().len(), 2);
    call(
        &app,
        "configure",
        json!({"model":"fake-crash:q4","voice":null}),
    );

    // Crash mid-request: reported, never retried; the next request restarts.
    let failed = submit(&app, 2, "first");
    assert_eq!(failed["error"]["code"], "LLM_UNAVAILABLE", "{failed}");
    let pids = f.record_pids();
    assert_eq!(pids.len(), 1);
    wait_gone(pids[0]);
    let ok = submit(&app, 3, "second");
    assert_eq!(ok["result"]["assistant_text"], "reply:second", "{ok}");
    let pids = f.record_pids();
    assert_eq!(pids.len(), 2);
    let running: Vec<u32> = pids.iter().copied().filter(|p| pid_alive(*p)).collect();
    assert_eq!(running.len(), 1);

    // Reuse within the idle window: no new child.
    assert_eq!(submit(&app, 4, "third")["status"], "succeeded");
    assert_eq!(f.record_pids().len(), 2);

    // Model switch stops the old runner before starting the new one.
    call(
        &app,
        "configure",
        json!({"model":"fake-other:q4","voice":null}),
    );
    let switched = submit(&app, 5, "switched");
    assert_eq!(switched["result"]["assistant_text"], "reply:switched");
    wait_gone(running[0]);
    let pids = f.record_pids();
    assert_eq!(pids.len(), 3);
    let current: Vec<u32> = pids.iter().copied().filter(|p| pid_alive(*p)).collect();
    assert_eq!(current.len(), 1);
    assert_eq!(f.record(current[0])["ppid"], json!(std::process::id()));

    // Idle stop via the worker's receive timeout.
    wait_gone(current[0]);
    let end = Instant::now() + Duration::from_secs(3);
    while !f.runner_dirs().is_empty() {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(10));
    }
    // Idle-stopped runner restarts lazily.
    assert_eq!(submit(&app, 6, "again")["status"], "succeeded");
    let latest = *f.record_pids().iter().find(|p| pid_alive(**p)).unwrap();
    drop(app);
    wait_gone(latest);
    let end = Instant::now() + Duration::from_secs(3);
    while !f.runner_dirs().is_empty() {
        assert!(Instant::now() < end, "shutdown removes per-runner dir");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn app_reports_busy_lock_saved_ollama_model_and_tampered_model() {
    let f = Fixture::new();
    f.real_release();
    let (record, entry) = f.model("fake-ok:q4", "ok");
    let cat = catalog(vec![entry]);
    // An Ollama-era preference is preserved but never migrated.
    write_mode(
        &f.roots.data.join("smith-next/app-state.json"),
        br#"{"schema_version":1,"config":{"model":"llama3:8b","voice":null,"revision":1},"history":[],"jobs":[],"expired_ids":[]}"#,
        0o600,
    );
    let app = Application::open(
        &f.roots,
        Arc::new(Providers::with_options(&f.roots, options(cat))),
    )
    .unwrap();
    let old = submit(&app, 1, "hello");
    assert_eq!(old["error"]["code"], "MODEL_UNAVAILABLE", "{old}");
    call(&app, "models", json!({"request_id":id(2)}));
    finished(&app, 2);
    call(
        &app,
        "configure",
        json!({"model":"fake-ok:q4","voice":null}),
    );

    // The installer's verify-inference (or any other owner) holds the lock.
    let holder = Runner::launch(&f.config(), &f.model_path(&record), deadline(10)).unwrap();
    let busy = submit(&app, 3, "hello");
    assert_eq!(busy["error"]["code"], "RUNNER_BUSY", "{busy}");
    holder.stop().unwrap();
    assert_eq!(
        submit(&app, 4, "hello")["result"]["assistant_text"],
        "reply:hello"
    );

    // Same-size content tampering is caught by the full hash at the next start.
    drop(app);
    let data = f.model_path(&record);
    let mut bytes = fs::read(&data).unwrap();
    bytes[0] = b'X';
    write_mode(&data, &bytes, 0o600);
    let app = Application::open(
        &f.roots,
        Arc::new(Providers::with_options(&f.roots, options(cat))),
    )
    .unwrap();
    let tampered = submit(&app, 5, "hello");
    assert_eq!(tampered["error"]["code"], "MODEL_UNAVAILABLE", "{tampered}");
    let state = call(&app, "state", json!({}));
    assert_eq!(
        state["history"].as_array().unwrap().len(),
        2,
        "failed turns are not appended"
    );
}

#[test]
fn verified_model_descriptor_survives_path_replacement() {
    let f = Fixture::new();
    let (record, _) = f.model("held:q4", "ok");
    let held = runner::verify_model_held(&f.models(), &record, deadline(10), &|| false).unwrap();
    let path = f.model_path(&record);
    fs::rename(&path, path.with_extension("original")).unwrap();
    write_mode(&path, b"exit-early\nsubstituted\n", 0o600);
    let mut child =
        Runner::launch_verified(&f.config(), &held, deadline(10), Arc::new(|| false)).unwrap();
    assert_eq!(
        child
            .chat(
                &[Message {
                    role: "user".into(),
                    content: "original".into()
                }],
                32,
                deadline(10)
            )
            .unwrap(),
        "reply:original"
    );
    child.stop().unwrap();
}

#[test]
fn verification_checks_deadline_and_cancellation_between_reads() {
    let f = Fixture::new();
    let (mut record, _) = f.model("bounded:q4", "ok");
    let bytes = vec![b'x'; 3 * 1024 * 1024];
    record.size = bytes.len() as u64;
    record.sha256 = sha(&bytes);
    write_mode(&f.model_path(&record), &bytes, 0o600);
    let calls = AtomicU64::new(0);
    let result = runner::verify_model_held(&f.models(), &record, deadline(10), &|| {
        calls.fetch_add(1, Ordering::Relaxed) >= 2
    });
    assert!(result.err().unwrap().starts_with("RUNNER_CANCELLED:"));
    assert_eq!(calls.load(Ordering::Relaxed), 3);
    assert!(
        runner::verify_model_held(&f.models(), &record, Instant::now(), &|| false)
            .err()
            .unwrap()
            .starts_with("PROVIDER_TIMEOUT:")
    );
    assert_eq!(
        runner::verify_runtime_bounded(&f.roots, Instant::now(), &|| false)
            .unwrap_err()
            .code,
        "PROVIDER_TIMEOUT"
    );
    assert_eq!(
        runner::verify_runtime_bounded(&f.roots, deadline(10), &|| true)
            .unwrap_err()
            .code,
        "RUNNER_CANCELLED"
    );
}

#[test]
fn application_teardown_cancels_active_native_inference_and_joins_cleanup() {
    let f = Fixture::new();
    f.real_release();
    let (record, entry) = f.model("slow-owned:q4", "slow");
    let app = Application::open(
        &f.roots,
        Arc::new(Providers::with_options(
            &f.roots,
            options(catalog(vec![entry])),
        )),
    )
    .unwrap();
    call(&app, "models", json!({"request_id":id(1)}));
    assert_eq!(finished(&app, 1)["status"], "succeeded");
    call(&app, "configure", json!({"model":record.name,"voice":null}));
    call(
        &app,
        "submit",
        json!({"request_id":id(2),"text":"wait","output":"chat"}),
    );
    let end = deadline(10);
    let pid = loop {
        if let Some(pid) = f.record_pids().first().copied() {
            if f.base
                .join(format!("records/request-{pid}-1.json"))
                .exists()
            {
                break pid;
            }
        }
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(10));
    };
    let start = Instant::now();
    drop(app);
    assert!(start.elapsed() < Duration::from_secs(5));
    assert!(!pid_alive(pid), "teardown must join runner cleanup");
    assert!(f.runner_dirs().is_empty());
    let holder = Runner::launch(&f.config(), &f.model_path(&record), deadline(10)).unwrap();
    holder.stop().unwrap();
}
