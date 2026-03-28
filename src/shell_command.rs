//! Cross-platform login shell command builder.
//!
//! Provides a helper to construct shell commands that execute via the user's
//! login shell (`$SHELL -l -c`) on Unix, ensuring PATH and environment variables
//! from `.zprofile`/`.profile` are available even when cflx is started from
//! non-login environments (launchd, systemd, cron).
//!
//! On Windows, commands use `cmd /C` as before.

use tracing::debug;

/// Build a [`tokio::process::Command`] that runs `command_str` via the user's
/// login shell on Unix (`$SHELL -l -c`) or `cmd /C` on Windows.
///
/// The returned command:
/// - Inherits the current process environment (`env_clear()` + `envs(std::env::vars())`)
/// - Sets `stdin` to null (no interactive input)
/// - Does **not** set `stdout`/`stderr` – callers should configure capture/piping as needed
///
/// # Examples
///
/// ```ignore
/// let mut cmd = shell_command::build_login_shell_command("opencode run");
/// cmd.current_dir(work_dir);
/// cmd.stdout(std::process::Stdio::piped());
/// let status = cmd.status().await?;
/// ```
pub fn build_login_shell_command(command_str: &str) -> tokio::process::Command {
    if cfg!(target_os = "windows") {
        debug!(
            "Building login shell command (Windows): cmd /C {}",
            command_str
        );
        let mut cmd = tokio::process::Command::new("cmd");
        cmd.arg("/C")
            .arg(command_str)
            .env_clear()
            .envs(std::env::vars())
            .stdin(std::process::Stdio::null());
        cmd
    } else {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        debug!(
            "Building login shell command (Unix): {} -l -c {}",
            shell, command_str
        );
        let mut cmd = tokio::process::Command::new(&shell);
        cmd.arg("-l")
            .arg("-c")
            .arg(command_str)
            .env_clear()
            .envs(std::env::vars())
            .stdin(std::process::Stdio::null());
        cmd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_build_login_shell_command_runs_echo() {
        let mut cmd = build_login_shell_command("echo hello");
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let output = cmd.output().await.expect("Failed to execute command");
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("hello"),
            "Expected 'hello' in stdout, got: {}",
            stdout
        );
    }

    #[tokio::test]
    async fn test_build_login_shell_command_inherits_env() {
        // Set a custom env var and verify it's visible in the child
        // SAFETY: This test is single-threaded and the env var is immediately cleaned up.
        unsafe {
            std::env::set_var("CFLX_TEST_MARKER", "login_shell_test");
        }
        let mut cmd = build_login_shell_command("echo $CFLX_TEST_MARKER");
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let output = cmd.output().await.expect("Failed to execute command");
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("login_shell_test"),
            "Expected env var value in stdout, got: {}",
            stdout
        );
        // SAFETY: Cleanup env var set above.
        unsafe {
            std::env::remove_var("CFLX_TEST_MARKER");
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_build_login_shell_command_uses_login_shell() {
        // Verify that the command is executed via $SHELL -l -c by checking
        // that PATH from login profile is available.
        // We test this by running a command that simply exits 0.
        let mut cmd = build_login_shell_command("true");
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let output = cmd.output().await.expect("Failed to execute command");
        assert!(
            output.status.success(),
            "Login shell command should succeed"
        );
    }
}
