//! Service management for `cflx server` as a background service.
//!
//! Provides install/uninstall/start/stop/restart/status operations using
//! the native service manager for the current platform:
//!   - macOS:   launchd user agent  (~/.../LaunchAgents/com.conflux.cflx-server.plist)
//!   - Linux:   systemd user service (~/.config/systemd/user/cflx-server.service)
//!   - Windows: Scheduled Task       (schtasks "CflxServer")
//!
//! Security: install/start/restart validate the effective `ServerConfig` before
//! touching the service manager, enforcing the same policy as `cflx server`.

use std::path::PathBuf;
use std::process::Command;

use crate::error::{OrchestratorError, Result};

const SERVICE_LABEL: &str = "com.conflux.cflx-server";

/// Validate the effective global ServerConfig (same policy as `cflx server`).
/// Returns the validated config on success.
fn validate_server_config() -> Result<crate::config::ServerConfig> {
    let config = crate::config::OrchestratorConfig::load_server_config_from_global();
    config.validate()?;
    Ok(config)
}

/// Return the path of the running `cflx` executable.
fn cflx_executable() -> Result<PathBuf> {
    std::env::current_exe().map_err(|e| {
        OrchestratorError::Io(std::io::Error::other(format!(
            "Failed to determine cflx executable path: {e}"
        )))
    })
}

// ─── macOS: launchd user agent ────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod platform {
    use std::path::Path;

    use super::*;

    fn plist_path() -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| {
            OrchestratorError::ConfigLoad("Cannot determine home directory".to_string())
        })?;
        Ok(home
            .join("Library")
            .join("LaunchAgents")
            .join(format!("{SERVICE_LABEL}.plist")))
    }

    fn generate_plist(exe: &Path) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
    "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
        <string>server</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/cflx-server.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/cflx-server.log</string>
</dict>
</plist>
"#,
            label = SERVICE_LABEL,
            exe = exe.display()
        )
    }

    pub fn install() -> Result<()> {
        validate_server_config()?;
        let exe = cflx_executable()?;
        let path = plist_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, generate_plist(&exe))?;
        println!("Service plist written: {}", path.display());
        let status = Command::new("launchctl")
            .args(["load", "-w"])
            .arg(&path)
            .status()?;
        if !status.success() {
            eprintln!("Warning: launchctl load returned non-zero exit code");
        } else {
            println!("Service loaded and enabled at login.");
        }
        Ok(())
    }

    pub fn uninstall() -> Result<()> {
        let path = plist_path()?;
        if path.exists() {
            let _ = Command::new("launchctl")
                .args(["unload", "-w"])
                .arg(&path)
                .status();
            std::fs::remove_file(&path)?;
            println!("Service uninstalled.");
        } else {
            println!("Service not installed (plist not found).");
        }
        Ok(())
    }

    pub fn start() -> Result<()> {
        validate_server_config()?;
        let path = plist_path()?;
        if !path.exists() {
            return Err(OrchestratorError::ConfigLoad(
                "Service not installed. Run `cflx service install` first.".to_string(),
            ));
        }
        let status = Command::new("launchctl")
            .args(["load", "-w"])
            .arg(&path)
            .status()?;
        if !status.success() {
            return Err(OrchestratorError::ConfigLoad(
                "launchctl load failed".to_string(),
            ));
        }
        println!("Service started.");
        Ok(())
    }

    pub fn stop() -> Result<()> {
        let path = plist_path()?;
        if !path.exists() {
            println!("Service not installed.");
            return Ok(());
        }
        let _ = Command::new("launchctl")
            .args(["unload"])
            .arg(&path)
            .status();
        println!("Service stopped.");
        Ok(())
    }

    pub fn restart() -> Result<()> {
        stop()?;
        start()
    }

    pub fn status() -> Result<()> {
        let output = Command::new("launchctl")
            .args(["list", SERVICE_LABEL])
            .output()?;
        if output.status.success() {
            print!("{}", String::from_utf8_lossy(&output.stdout));
        } else {
            println!("Service not running (not found in launchctl list).");
        }
        Ok(())
    }
}

// ─── Linux: systemd user service ──────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod platform {
    use super::*;

    const SERVICE_NAME: &str = "cflx-server";

    fn unit_path() -> Result<PathBuf> {
        let config_home = dirs::config_dir().ok_or_else(|| {
            OrchestratorError::ConfigLoad("Cannot determine config directory".to_string())
        })?;
        Ok(config_home
            .join("systemd")
            .join("user")
            .join(format!("{SERVICE_NAME}.service")))
    }

    fn generate_unit(exe: &PathBuf) -> String {
        format!(
            "[Unit]\n\
             Description=Conflux Server Daemon\n\
             After=network.target\n\
             \n\
             [Service]\n\
             ExecStart={exe} server\n\
             Restart=on-failure\n\
             RestartSec=5\n\
             \n\
             [Install]\n\
             WantedBy=default.target\n",
            exe = exe.display()
        )
    }

    pub fn install() -> Result<()> {
        validate_server_config()?;
        let exe = cflx_executable()?;
        let path = unit_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, generate_unit(&exe))?;
        println!("Service unit written: {}", path.display());
        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();
        let status = Command::new("systemctl")
            .args(["--user", "enable", SERVICE_NAME])
            .status()?;
        if !status.success() {
            eprintln!("Warning: systemctl enable returned non-zero exit code");
        } else {
            println!("Service enabled to start at login.");
        }
        Ok(())
    }

    pub fn uninstall() -> Result<()> {
        let path = unit_path()?;
        let _ = Command::new("systemctl")
            .args(["--user", "disable", "--now", SERVICE_NAME])
            .status();
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();
        println!("Service uninstalled.");
        Ok(())
    }

    pub fn start() -> Result<()> {
        validate_server_config()?;
        let status = Command::new("systemctl")
            .args(["--user", "start", SERVICE_NAME])
            .status()?;
        if !status.success() {
            return Err(OrchestratorError::ConfigLoad(
                "systemctl start failed".to_string(),
            ));
        }
        println!("Service started.");
        Ok(())
    }

    pub fn stop() -> Result<()> {
        let status = Command::new("systemctl")
            .args(["--user", "stop", SERVICE_NAME])
            .status()?;
        if !status.success() {
            eprintln!("Warning: systemctl stop returned non-zero exit code");
        }
        println!("Service stopped.");
        Ok(())
    }

    pub fn restart() -> Result<()> {
        validate_server_config()?;
        let status = Command::new("systemctl")
            .args(["--user", "restart", SERVICE_NAME])
            .status()?;
        if !status.success() {
            return Err(OrchestratorError::ConfigLoad(
                "systemctl restart failed".to_string(),
            ));
        }
        println!("Service restarted.");
        Ok(())
    }

    pub fn status() -> Result<()> {
        let output = Command::new("systemctl")
            .args(["--user", "status", SERVICE_NAME])
            .output()?;
        print!("{}", String::from_utf8_lossy(&output.stdout));
        if !output.status.success() {
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
        }
        Ok(())
    }
}

// ─── Windows: Scheduled Task ──────────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod platform {
    use super::*;

    const TASK_NAME: &str = "CflxServer";

    pub fn install() -> Result<()> {
        validate_server_config()?;
        let exe = cflx_executable()?;
        let tr = format!("{} server", exe.display());
        let status = Command::new("schtasks")
            .args([
                "/create", "/tn", TASK_NAME, "/tr", &tr, "/sc", "onlogon", "/f",
            ])
            .status()?;
        if !status.success() {
            return Err(OrchestratorError::ConfigLoad(
                "schtasks /create failed".to_string(),
            ));
        }
        println!("Service installed as Scheduled Task '{TASK_NAME}'.");
        Ok(())
    }

    pub fn uninstall() -> Result<()> {
        let status = Command::new("schtasks")
            .args(["/delete", "/tn", TASK_NAME, "/f"])
            .status()?;
        if !status.success() {
            eprintln!("Warning: schtasks /delete returned non-zero exit code");
        }
        println!("Service uninstalled.");
        Ok(())
    }

    pub fn start() -> Result<()> {
        validate_server_config()?;
        let status = Command::new("schtasks")
            .args(["/run", "/tn", TASK_NAME])
            .status()?;
        if !status.success() {
            return Err(OrchestratorError::ConfigLoad(
                "schtasks /run failed".to_string(),
            ));
        }
        println!("Service started.");
        Ok(())
    }

    pub fn stop() -> Result<()> {
        let status = Command::new("schtasks")
            .args(["/end", "/tn", TASK_NAME])
            .status()?;
        if !status.success() {
            eprintln!("Warning: schtasks /end returned non-zero exit code");
        }
        println!("Service stopped.");
        Ok(())
    }

    pub fn restart() -> Result<()> {
        stop()?;
        start()
    }

    pub fn status() -> Result<()> {
        let output = Command::new("schtasks")
            .args(["/query", "/tn", TASK_NAME, "/fo", "LIST"])
            .output()?;
        if output.status.success() {
            print!("{}", String::from_utf8_lossy(&output.stdout));
        } else {
            println!("Service '{TASK_NAME}' not found.");
        }
        Ok(())
    }
}

// ─── Unsupported platform ─────────────────────────────────────────────────────

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
mod platform {
    use super::*;

    fn unsupported() -> Result<()> {
        Err(OrchestratorError::ConfigLoad(
            "Service management is not supported on this platform.".to_string(),
        ))
    }

    pub fn install() -> Result<()> {
        unsupported()
    }
    pub fn uninstall() -> Result<()> {
        unsupported()
    }
    pub fn start() -> Result<()> {
        unsupported()
    }
    pub fn stop() -> Result<()> {
        unsupported()
    }
    pub fn restart() -> Result<()> {
        unsupported()
    }
    pub fn status() -> Result<()> {
        unsupported()
    }
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Install `cflx server` as a background service.
///
/// Validates the effective server configuration before writing service files.
pub fn install() -> Result<()> {
    platform::install()
}

/// Uninstall the `cflx server` background service.
pub fn uninstall() -> Result<()> {
    platform::uninstall()
}

/// Start the `cflx server` background service.
///
/// Validates the effective server configuration before starting.
pub fn start() -> Result<()> {
    platform::start()
}

/// Stop the `cflx server` background service.
pub fn stop() -> Result<()> {
    platform::stop()
}

/// Restart the `cflx server` background service.
///
/// Validates the effective server configuration before restarting.
pub fn restart() -> Result<()> {
    platform::restart()
}

/// Show the current status of the `cflx server` background service.
pub fn status() -> Result<()> {
    platform::status()
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::config::{ServerAuthConfig, ServerAuthMode, ServerConfig};

    /// Verify that a loopback ServerConfig passes validation (service start is allowed).
    #[test]
    fn test_loopback_config_validates_ok() {
        let cfg = ServerConfig {
            bind: "127.0.0.1".to_string(),
            ..ServerConfig::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "Loopback bind should pass validation"
        );
    }

    /// Verify that a non-loopback config without a token fails validation.
    #[test]
    fn test_non_loopback_no_token_fails_validation() {
        let cfg = ServerConfig {
            bind: "0.0.0.0".to_string(),
            auth: ServerAuthConfig {
                mode: ServerAuthMode::None,
                token: None,
                token_env: None,
            },
            ..ServerConfig::default()
        };
        assert!(
            cfg.validate().is_err(),
            "Non-loopback bind without token should fail validation"
        );
    }

    /// Verify that a non-loopback config with a valid bearer token passes validation.
    #[test]
    fn test_non_loopback_with_token_validates_ok() {
        let cfg = ServerConfig {
            bind: "0.0.0.0".to_string(),
            auth: ServerAuthConfig {
                mode: ServerAuthMode::BearerToken,
                token: Some("secret".to_string()),
                token_env: None,
            },
            ..ServerConfig::default()
        };
        assert!(
            cfg.validate().is_ok(),
            "Non-loopback bind with valid bearer token should pass validation"
        );
    }
}
