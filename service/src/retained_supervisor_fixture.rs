//! Native offline executable fixture, compiled only by retained-runtime tests.
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Write};
use std::time::Duration;

fn field(raw: &str, key: &str) -> String {
    let pattern = format!("\"{key}\":\"");
    let Some(start) = raw.find(&pattern) else {
        return String::new();
    };
    let mut text = String::new();
    let mut escaped = false;
    for c in raw[start + pattern.len()..].chars() {
        if escaped {
            text.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            break;
        } else {
            text.push(c);
        }
    }
    text
}
fn main() {
    let behavior = fs::read_to_string("behavior").unwrap();
    let root = std::env::current_dir()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let mut events = OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("events"))
        .unwrap();
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    assert_eq!(args, ["-B", "-m", "smith_agent.retained"]);
    for key in [
        "OPENAI_API_KEY",
        "ANTHROPIC_API_KEY",
        "SSH_AUTH_SOCK",
        "DOCKER_HOST",
        "DAGGER_CLOUD_TOKEN",
        "PYTHONPATH",
        "LD_PRELOAD",
    ] {
        assert!(std::env::var_os(key).is_none());
    }
    writeln!(events, "pid:{} argv:{}", std::process::id(), args.join(" ")).unwrap();
    let mut original_id = String::new();
    let mut original_op = String::new();
    let mut mode = String::new();
    let mut failed = false;
    let mut polls = 0;
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let op = field(&line, "op");
        let id = field(&line, "operation_id");
        let mut job = field(&line, "job_id");
        let target = field(&line, "target_operation_id");
        writeln!(
            events,
            "pid:{} op:{op} id:{id} target:{target}",
            std::process::id()
        )
        .unwrap();
        events.flush().unwrap();
        if behavior == "slow" {
            std::thread::sleep(Duration::from_secs(5));
        }
        if behavior == "exit" {
            return;
        }
        if behavior == "malformed" {
            writeln!(stdout, "not-json PRIVATE-UPSTREAM-SECRET").unwrap();
            stdout.flush().unwrap();
            continue;
        }
        if behavior == "stdout_flood" {
            stdout.write_all(&vec![b'x'; 65537]).unwrap();
            stdout.flush().unwrap();
            continue;
        }
        if behavior == "stderr_flood" {
            let _ = io::stderr().write_all(&vec![b's'; 70000]);
        }
        if behavior == "wrong_identity" {
            job = "f".repeat(32);
        }
        if op != "observe" {
            original_id = id.clone();
            original_op = op.clone();
            polls = 0;
            if op == "create" {
                mode = field(&line, "mode");
            }
            failed = field(&line, "text").contains("known-failure");
        } else {
            assert_eq!(target, original_id);
            polls += 1;
        }
        let pending = op != "observe" || polls == 1 || behavior == "pending_forever";
        let state = if pending {
            "pending"
        } else if behavior == "unknown" || (behavior == "finish_unknown" && original_op == "finish")
        {
            "unknown"
        } else if failed {
            "failed"
        } else {
            "succeeded"
        };
        let job_state = if state == "unknown" {
            "unknown"
        } else if original_op == "finish" && !pending {
            "finished"
        } else if state == "failed" {
            "failed"
        } else {
            "active"
        };
        let retained = job_state != "finished";
        let text = if !pending && original_op == "prompt" && !failed {
            format!("pid:{}", std::process::id())
        } else {
            String::new()
        };
        let code = if state == "unknown" {
            "\"runtime_unknown\""
        } else {
            "null"
        };
        let duplicate = if behavior == "duplicate_member" {
            "\"v\":1,"
        } else {
            ""
        };
        let response = format!("{{{duplicate}\"v\":1,\"operation_id\":\"{id}\",\"job_id\":\"{job}\",\"ok\":true,\"code\":null,\"job\":{{\"state\":\"{job_state}\",\"mode\":\"{mode}\",\"retained\":{retained},\"conversation\":true,\"ready\":false,\"blockers\":[]}},\"operation\":{{\"operation_id\":\"{original_id}\",\"op\":\"{original_op}\",\"state\":\"{state}\",\"code\":{code},\"text\":\"{text}\"}}}}");
        writeln!(stdout, "{response}").unwrap();
        if behavior == "extra" {
            writeln!(stdout, "{response}").unwrap();
        }
        stdout.flush().unwrap();
    }
}
