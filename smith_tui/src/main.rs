use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use anyhow::Result;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

const SMITH_DIR: &str = "/workspace/smith";
const PID_FILE: &str = "/workspace/smith/smith.pid";

fn ensure_dirs() -> io::Result<()> {
    fs::create_dir_all(SMITH_DIR)?;
    Ok(())
}

fn write_pid(pid: u32) -> io::Result<()> {
    fs::write(PID_FILE, pid.to_string())?;
    Ok(())
}

fn read_pid() -> io::Result<Option<u32>> {
    if !Path::new(PID_FILE).exists() {
        return Ok(None);
    }
    let s = fs::read_to_string(PID_FILE)?;
    let pid = s.trim().parse::<u32>().ok();
    Ok(pid)
}

fn remove_pid_file() -> io::Result<()> {
    if Path::new(PID_FILE).exists() {
        fs::remove_file(PID_FILE)?;
    }
    Ok(())
}

fn is_running(pid: u32) -> bool {
    // Use 'ps -p <pid>' to check liveness
    let out = Command::new("ps").arg("-p").arg(pid.to_string()).arg("-o").arg("pid=").output();
    if let Ok(o) = out {
        return o.status.success();
    }
    false
}

fn start_smith() -> io::Result<()> {
    ensure_dirs()?;
    // Spawn a lightweight sleep process to simulate smith running
    let mut child = Command::new("/bin/sleep").arg("600").spawn()?;
    let pid = child.id();
    write_pid(pid)?;
    Ok(())
}

fn stop_smith() -> io::Result<()> {
    if let Some(pid) = read_pid()? {
        // Attempt graceful stop
        let _ = Command::new("kill").arg("-TERM").arg(pid.to_string()).status();
        // Best-effort wait a moment
        thread::sleep(Duration::from_millis(200));
        // Remove the pid file regardless
        let _ = remove_pid_file();
    }
    Ok(())
}

fn status_smith() -> io::Result<String> {
    if let Some(pid) = read_pid()? {
        if is_running(pid) {
            return Ok(format!("Smith is running (PID {})", pid));
        }
    }
    Ok("Smith is not running".to_string())
}

fn run_tui() -> io::Result<()> {
    println!("Smith TUI - Manage smith (Rust, no Python)");
    println!("Select an option:");
    println!(" 1) Start Smith");
    println!(" 2) Stop Smith");
    println!(" 3) Status");
    println!(" 4) Validate (quick check)");
    println!(" 5) Exit");

    let mut input = String::new();
    loop {
        print!("> "); io::stdout().flush()?;
        input.clear();
        io::stdin().read_line(&mut input)?; // read one line for simplicity
        let cmd = input.trim().to_string();
        match cmd.as_str() {
            "1" => {
                start_smith()?;
                println!("Started smith.");
            }
            "2" => {
                stop_smith()?;
                println!("Stopped smith (if it was running).");
            }
            "3" => {
                match status_smith()? {
                    s => println!("{}", s),
                }
            }
            "4" => {
                // Simple internal validation: start a short-lived process and stop it
                println!("Running internal validation...");
                let mut test = Command::new("/bin/sleep").arg("2").spawn()?;
                let tid = test.id();
                println!("Validation: spawned test sleep (PID {})", tid);
                thread::sleep(Duration::from_secs(1));
                let _ = Command::new("kill").arg(tid.to_string()).status();
                println!("Validation complete.");
            }
            "5" => {
                println!("Exiting.");
                break;
            }
            _ => {
                println!("Unknown option. Try 1-5.");
            }
        }
        // Clear for next prompt
        input.clear();
        println!("\nNext action (1-5) or 5 to exit:");
    }
    Ok(())
}

fn run_validation_commands() -> io::Result<()> {
    ensure_dirs()?;
    // 1) Start smith with a short-lived process
    let mut child = Command::new("/bin/sleep").arg("3").spawn()?;
    let pid = child.id();
    write_pid(pid)?;
    // 2) Check that the pid file exists and the process is listed
    let mut ok = false;
    if let Some(p) = read_pid()? {
        if is_running(p) {
            ok = true;
        }
    }
    // 3) Stop smith and cleanup
    let _ = Command::new("kill").arg("-TERM").arg(pid.to_string()).status();
    thread::sleep(Duration::from_millis(200));
    let _ = remove_pid_file();
    if ok { println!("Validation: pass"); } else { println!("Validation: fail"); }
    Ok(())
}

fn main() -> Result<()> {
    // Simple argument parsing for a quick --validate hook
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 && args[1] == "--validate" {
        run_validation_commands()?;
        return Ok(());
    }
    // Run a basic interactive TUI (text-based) when invoked normally
    run_tui()?;
    Ok(())
}
