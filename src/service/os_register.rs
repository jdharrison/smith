//! OS registration for the smith supervisor (`smith serve` at login).

use std::io::Write;
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::fs;

const WINDOWS_TASK_NAME: &str = "SmithServe";
#[cfg(target_os = "linux")]
const SYSTEMD_UNIT: &str = "smith.service";
#[cfg(target_os = "macos")]
const LAUNCHD_LABEL: &str = "com.agent.smith";

fn smith_bin() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("current_exe: {e}"))
}

/// True when the loopback bridge appears to be accepting connections.
pub fn bridge_is_listening() -> bool {
    let port = crate::commands::serve::BRIDGE_DEFAULT_PORT;
    TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(400),
    )
    .is_ok()
}

/// True if an OS unit/task appears registered (best-effort).
pub fn is_registered() -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut cmd = Command::new("schtasks");
        cmd.args(["/Query", "/TN", WINDOWS_TASK_NAME])
            .creation_flags(CREATE_NO_WINDOW);
        cmd.output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(target_os = "linux")]
    {
        systemd_unit_path()
            .map(|p| p.is_file())
            .unwrap_or(false)
    }
    #[cfg(target_os = "macos")]
    {
        launchd_plist_path()
            .map(|p| p.is_file())
            .unwrap_or(false)
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        false
    }
}

/// Write unit/task and enable (start at login). Best-effort start now if not already up.
pub fn register_and_enable() -> Result<(), String> {
    #[cfg(windows)]
    {
        register_windows()
    }
    #[cfg(target_os = "linux")]
    {
        register_linux()
    }
    #[cfg(target_os = "macos")]
    {
        register_macos()
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        Err("OS service registration not supported on this platform".into())
    }
}

#[cfg(windows)]
fn windows_runner_path() -> Result<PathBuf, String> {
    crate::config_dir().map(|d| d.join("serve_background.vbs"))
}

/// Disable autostart (leave binary/config; remove or disable unit).
pub fn disable() -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut ps_cmd = Command::new("powershell");
        ps_cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &format!(
                "Unregister-ScheduledTask -TaskName '{}' -Confirm:$false -ErrorAction SilentlyContinue",
                WINDOWS_TASK_NAME
            ),
        ])
        .creation_flags(CREATE_NO_WINDOW);
        let _ = ps_cmd.output();
        if let Ok(path) = windows_runner_path() {
            let _ = std::fs::remove_file(path);
        }
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("systemctl")
            .args(["--user", "disable", "--now", SYSTEMD_UNIT])
            .output();
        if let Ok(path) = systemd_unit_path() {
            let _ = fs::remove_file(path);
        }
        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output();
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(path) = launchd_plist_path() {
            let _ = Command::new("launchctl")
                .args(["unload", &path.display().to_string()])
                .output();
            let _ = fs::remove_file(&path);
        }
        Ok(())
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        Ok(())
    }
}

/// Remove registration (uninstall).
pub fn uninstall_registration() -> Result<(), String> {
    disable()
}

/// Ask a running bridge to shut down (loopback). No-op if not listening.
pub fn request_bridge_shutdown() -> Result<(), String> {
    if !bridge_is_listening() {
        return Ok(());
    }
    let port = crate::commands::serve::BRIDGE_DEFAULT_PORT;
    let mut stream = TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_secs(2),
    )
    .map_err(|e| format!("connect bridge for shutdown: {e}"))?;
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let req = format!(
        "POST /v1/shutdown HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("shutdown request: {e}"))?;
    // Give the process a moment to exit.
    std::thread::sleep(Duration::from_millis(600));
    Ok(())
}

/// Best-effort: start supervisor now only if not already listening.
pub fn start_supervisor_now() -> Result<(), String> {
    if bridge_is_listening() {
        return Ok(());
    }
    let bin = smith_bin()?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        Command::new(&bin)
            .args(["serve"])
            .creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS | CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("failed to spawn smith serve: {e}"))?;
        Ok(())
    }
    #[cfg(unix)]
    {
        Command::new(&bin)
            .args(["serve"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to spawn smith serve: {e}"))?;
        Ok(())
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = bin;
        Err("cannot start supervisor on this platform".into())
    }
}

/// Ask a running bridge to ensure Whisper/TTS sidecars (no-op if bridge down).
pub fn request_sidecars_ensure() -> Result<(), String> {
    if !bridge_is_listening() {
        return Ok(());
    }
    let port = crate::commands::serve::BRIDGE_DEFAULT_PORT;
    let mut stream = TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_secs(2),
    )
    .map_err(|e| format!("connect bridge for sidecars: {e}"))?;
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_read_timeout(Some(Duration::from_secs(120)));
    let req = format!(
        "POST /v1/sidecars/ensure HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("sidecars ensure request: {e}"))?;
    // Drain briefly so the server finishes the handler before we drop.
    let mut buf = [0u8; 256];
    let _ = std::io::Read::read(&mut stream, &mut buf);
    Ok(())
}

#[cfg(windows)]
fn register_windows() -> Result<(), String> {
    let bin = smith_bin()?;
    let vbs_path = windows_runner_path()?;
    if let Some(parent) = vbs_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let bin_q = bin.display().to_string().replace('"', "\"\"");
    let vbs_content = format!(
        "Set WshShell = CreateObject(\"WScript.Shell\")\r\nWshShell.Run \"\"\"{bin}\"\" serve\", 0, False\r\n",
        bin = bin_q
    );
    std::fs::write(&vbs_path, vbs_content).map_err(|e| format!("write vbs runner: {e}"))?;

    let ps_script = format!(
        "$act = New-ScheduledTaskAction -Execute 'wscript.exe' -Argument ('//B //Nologo \"' + '{}' + '\"'); \
         $trig = New-ScheduledTaskTrigger -AtLogOn; \
         Register-ScheduledTask -TaskName '{}' -Action $act -Trigger $trig -Force",
        vbs_path.display().to_string().replace('\'', "''"),
        WINDOWS_TASK_NAME
    );

    let mut ps_cmd = Command::new("powershell");
    ps_cmd.args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &ps_script]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        ps_cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = ps_cmd
        .output()
        .map_err(|e| format!("powershell register scheduled task: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "failed to register Windows scheduled task: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    start_supervisor_now()?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn systemd_unit_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "HOME not set".to_string())?;
    Ok(home
        .join(".config")
        .join("systemd")
        .join("user")
        .join(SYSTEMD_UNIT))
}

#[cfg(target_os = "linux")]
fn register_linux() -> Result<(), String> {
    let bin = smith_bin()?;
    let path = systemd_unit_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("systemd dir: {e}"))?;
    }
    let bin_q = bin.display().to_string().replace('"', "\\\"");
    let body = format!(
        r#"[Unit]
Description=smith local supervisor (bridge + warm services)
After=default.target

[Service]
Type=simple
ExecStart="{bin}" serve
Restart=on-failure
RestartSec=15

[Install]
WantedBy=default.target
"#,
        bin = bin_q
    );
    fs::write(&path, body).map_err(|e| format!("write unit: {e}"))?;
    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .output();
    let enable = Command::new("systemctl")
        .args(["--user", "enable", "--now", SYSTEMD_UNIT])
        .output()
        .map_err(|e| format!("systemctl: {e}"))?;
    if !enable.status.success() {
        let _ = start_supervisor_now();
        return Err(format!(
            "systemctl enable failed (unit written at {}): {}",
            path.display(),
            String::from_utf8_lossy(&enable.stderr).trim()
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn launchd_plist_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "HOME not set".to_string())?;
    Ok(home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{LAUNCHD_LABEL}.plist")))
}

#[cfg(target_os = "macos")]
fn register_macos() -> Result<(), String> {
    let bin = smith_bin()?;
    let path = launchd_plist_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("LaunchAgents dir: {e}"))?;
    }
    // KeepAlive=false avoids crash loops when bind fails; RunAtLoad starts once at login.
    let body = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{label}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{bin}</string>
    <string>serve</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <false/>
</dict>
</plist>
"#,
        label = LAUNCHD_LABEL,
        bin = bin.display()
    );
    fs::write(&path, &body).map_err(|e| format!("write plist: {e}"))?;
    let _ = Command::new("launchctl")
        .args(["unload", &path.display().to_string()])
        .output();
    let load = Command::new("launchctl")
        .args(["load", &path.display().to_string()])
        .output()
        .map_err(|e| format!("launchctl: {e}"))?;
    if !load.status.success() {
        let _ = start_supervisor_now();
        return Err(format!(
            "launchctl load failed: {}",
            String::from_utf8_lossy(&load.stderr).trim()
        ));
    }
    Ok(())
}
