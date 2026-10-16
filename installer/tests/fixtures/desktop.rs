//! Standalone finite test desktop, compiled only by launch unit tests.
use std::{
    fs, thread,
    time::{Duration, Instant},
};
unsafe extern "C" {
    fn getsid(pid: i32) -> i32;
    fn getpgrp() -> i32;
    fn getppid() -> i32;
}
fn main() {
    let pid = std::process::id();
    let cwd = std::env::current_dir().unwrap();
    let stdio: Vec<_> = (0..3)
        .map(|fd| fs::read_link(format!("/proc/self/fd/{fd}")).unwrap())
        .collect();
    // SAFETY: these process identity calls have no pointer arguments.
    let (sid, group, parent) = unsafe { (getsid(0), getpgrp(), getppid()) };
    let probe = format!(
        "{pid}\n{sid}\n{group}\n{parent}\n{}\n{}\n{}\n{}\n{}\n",
        std::env::args_os().count(),
        cwd.display(),
        stdio[0].display(),
        stdio[1].display(),
        stdio[2].display()
    );
    fs::write("launch-probe.tmp", probe).unwrap();
    fs::rename("launch-probe.tmp", "launch-probe").unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    while !cwd.join("launch-exit").exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    fs::write("launch-done", b"done").unwrap();
}
