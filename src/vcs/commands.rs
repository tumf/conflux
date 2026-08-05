//! Common command execution helpers for VCS operations.
//!
//! This module provides shared utilities for running VCS commands,
//! reducing code duplication in Git implementations.

use super::{VcsBackend, VcsError, VcsResult};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::debug;

/// Execute a VCS command and return the trimmed stdout output.
///
/// # Arguments
/// * `program` - The VCS program to run (e.g., "git")
/// * `args` - Arguments to pass to the program
/// * `cwd` - Working directory for the command
/// * `backend` - VCS backend type for error context
///
/// # Returns
/// The trimmed stdout output on success, or an error if the command fails.
pub async fn run_vcs_command<P: AsRef<Path>>(
    program: &str,
    args: &[&str],
    cwd: P,
    backend: VcsBackend,
) -> VcsResult<String> {
    let cwd_path = cwd.as_ref();
    let command_str = format!("{} {}", program, args.join(" "));

    debug!(
        module = module_path!(),
        "Executing {} command: {} (cwd: {:?})",
        program,
        args.join(" "),
        cwd_path
    );
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd_path)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| VcsError::Command {
            backend,
            message: format!("Failed to execute {}: {}", program, e),
            command: Some(command_str.clone()),
            working_dir: Some(cwd_path.to_path_buf()),
            stderr: None,
            stdout: None,
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        return Err(VcsError::Command {
            backend,
            message: format!("{} {} failed: {}", program, args.join(" "), stderr),
            command: Some(command_str),
            working_dir: Some(cwd_path.to_path_buf()),
            stderr: Some(stderr),
            stdout: Some(stdout),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if args.contains(&"-z") {
        Ok(stdout.into_owned())
    } else {
        Ok(stdout.trim().to_string())
    }
}

/// Result of a VCS command invocation that spawned successfully.
///
/// Unlike [`run_vcs_command`], a non-zero exit is not an error here. The caller
/// classifies the failure itself, so the actual process exit code survives
/// instead of being folded into a rendered error string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VcsCommandOutput {
    /// Rendered command line, for diagnostics.
    pub command: String,
    /// Process exit code, or `None` when the process was killed by a signal.
    pub exit_code: Option<i32>,
    /// Whether the process exited successfully.
    pub success: bool,
    /// Captured stdout.
    pub stdout: String,
    /// Captured stderr.
    pub stderr: String,
}

/// Execute a VCS command and return its captured result without treating a
/// non-zero exit as an error.
///
/// Only a spawn failure yields `Err`; every process that actually ran is
/// reported through [`VcsCommandOutput`] so callers can distinguish a
/// repository-level rejection from a fatal VCS failure by exit code.
pub async fn run_vcs_command_captured<P: AsRef<Path>>(
    program: &str,
    args: &[&str],
    cwd: P,
    backend: VcsBackend,
) -> VcsResult<VcsCommandOutput> {
    let cwd_path = cwd.as_ref();
    let command_str = format!("{} {}", program, args.join(" "));

    debug!(
        module = module_path!(),
        "Executing {} command (captured): {} (cwd: {:?})",
        program,
        args.join(" "),
        cwd_path
    );
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd_path)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| VcsError::Command {
            backend,
            message: format!("Failed to execute {}: {}", program, e),
            command: Some(command_str.clone()),
            working_dir: Some(cwd_path.to_path_buf()),
            stderr: None,
            stdout: None,
        })?;

    Ok(VcsCommandOutput {
        command: command_str,
        exit_code: output.status.code(),
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

/// One raw chunk read from a streamed command's standard streams.
///
/// Bytes rather than lines, so the accumulated buffers stay byte-identical to
/// what [`run_vcs_command_captured`] would have produced.
enum StreamChunk {
    Stdout(Vec<u8>),
    Stderr(Vec<u8>),
}

/// Accumulates one stream's raw bytes while handing out complete lines.
#[derive(Default)]
struct StreamTee {
    raw: Vec<u8>,
    emitted_upto: usize,
}

impl StreamTee {
    /// Append `chunk` and return every line completed by it.
    fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.raw.extend_from_slice(chunk);
        let mut lines = Vec::new();
        while let Some(offset) = self.raw[self.emitted_upto..]
            .iter()
            .position(|byte| *byte == b'\n')
        {
            let end = self.emitted_upto + offset;
            lines.push(render_line(&self.raw[self.emitted_upto..end]));
            self.emitted_upto = end + 1;
        }
        lines
    }

    /// Return the trailing content that never ended with a newline.
    fn finish(&mut self) -> Option<String> {
        if self.emitted_upto >= self.raw.len() {
            return None;
        }
        let line = render_line(&self.raw[self.emitted_upto..]);
        self.emitted_upto = self.raw.len();
        (!line.is_empty()).then_some(line)
    }

    /// The complete raw stream, exactly as captured.
    fn into_text(self) -> String {
        String::from_utf8_lossy(&self.raw).to_string()
    }
}

/// Render one captured line for presentation.
///
/// Only the line terminator is normalised. ANSI control sequences are left
/// intact: stripping them is a rendering decision that belongs to whichever
/// sink displays the line, and the raw buffer this line came from is what
/// classification reads.
fn render_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_end_matches('\r')
        .to_string()
}

/// Execute a VCS command, emitting each output line as it arrives while still
/// capturing the complete raw streams.
///
/// The returned [`VcsCommandOutput`] carries the same rendered command, exit
/// code, success flag, and raw stdout/stderr text that
/// [`run_vcs_command_captured`] would have produced for the same process, so
/// every existing structural classifier keeps working unchanged. `on_line` is a
/// tee for presentation only: it never sees a filtered, reordered, or truncated
/// view, and nothing it does can change what the classifier reads.
///
/// Both streams are drained concurrently by dedicated tasks, so a hook that
/// fills one pipe cannot deadlock the other.
pub async fn run_vcs_command_streamed<P, F>(
    program: &str,
    args: &[&str],
    cwd: P,
    backend: VcsBackend,
    mut on_line: F,
) -> VcsResult<VcsCommandOutput>
where
    P: AsRef<Path>,
    F: FnMut(crate::events::CommitOutputStream, &str) + Send,
{
    use crate::events::CommitOutputStream;
    use tokio::io::AsyncReadExt;

    let cwd_path = cwd.as_ref();
    let command_str = format!("{} {}", program, args.join(" "));

    debug!(
        module = module_path!(),
        "Executing {} command (streamed): {} (cwd: {:?})",
        program,
        args.join(" "),
        cwd_path
    );

    let spawn_error = |error: std::io::Error| VcsError::Command {
        backend,
        message: format!("Failed to execute {}: {}", program, error),
        command: Some(command_str.clone()),
        working_dir: Some(cwd_path.to_path_buf()),
        stderr: None,
        stdout: None,
    };

    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(spawn_error)?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamChunk>(64);

    async fn pump<R>(mut reader: R, tx: tokio::sync::mpsc::Sender<StreamChunk>, is_stdout: bool)
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        let mut buffer = [0_u8; 8192];
        loop {
            match reader.read(&mut buffer).await {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    let chunk = buffer[..read].to_vec();
                    let message = if is_stdout {
                        StreamChunk::Stdout(chunk)
                    } else {
                        StreamChunk::Stderr(chunk)
                    };
                    if tx.send(message).await.is_err() {
                        break;
                    }
                }
            }
        }
    }

    let stdout_pump = stdout.map(|stream| tokio::spawn(pump(stream, tx.clone(), true)));
    let stderr_pump = stderr.map(|stream| tokio::spawn(pump(stream, tx.clone(), false)));
    drop(tx);

    let mut stdout_tee = StreamTee::default();
    let mut stderr_tee = StreamTee::default();

    while let Some(chunk) = rx.recv().await {
        let (stream, bytes) = match &chunk {
            StreamChunk::Stdout(bytes) => (CommitOutputStream::Stdout, bytes),
            StreamChunk::Stderr(bytes) => (CommitOutputStream::Stderr, bytes),
        };
        let tee = match stream {
            CommitOutputStream::Stdout => &mut stdout_tee,
            CommitOutputStream::Stderr => &mut stderr_tee,
        };
        for line in tee.push(bytes) {
            on_line(stream, &line);
        }
    }

    if let Some(line) = stdout_tee.finish() {
        on_line(CommitOutputStream::Stdout, &line);
    }
    if let Some(line) = stderr_tee.finish() {
        on_line(CommitOutputStream::Stderr, &line);
    }

    if let Some(handle) = stdout_pump {
        let _ = handle.await;
    }
    if let Some(handle) = stderr_pump {
        let _ = handle.await;
    }

    let status = child.wait().await.map_err(spawn_error)?;

    Ok(VcsCommandOutput {
        command: command_str,
        exit_code: status.code(),
        success: status.success(),
        stdout: stdout_tee.into_text(),
        stderr: stderr_tee.into_text(),
    })
}

/// Execute a VCS command without capturing output (fire-and-forget).
///
/// Returns Ok(()) on success, error on failure.
#[allow(dead_code)]
pub async fn run_vcs_command_silent<P: AsRef<Path>>(
    program: &str,
    args: &[&str],
    cwd: P,
    backend: VcsBackend,
) -> VcsResult<()> {
    let cwd_path = cwd.as_ref();
    let command_str = format!("{} {}", program, args.join(" "));

    debug!(
        module = module_path!(),
        "Executing {} command (silent): {} (cwd: {:?})",
        program,
        args.join(" "),
        cwd_path
    );
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .await
        .map_err(|e| VcsError::Command {
            backend,
            message: format!("Failed to execute {}: {}", program, e),
            command: Some(command_str.clone()),
            working_dir: Some(cwd_path.to_path_buf()),
            stderr: None,
            stdout: None,
        })?;

    if !output.status.success() {
        return Err(VcsError::Command {
            backend,
            message: format!("{} {} failed", program, args.join(" ")),
            command: Some(command_str),
            working_dir: Some(cwd_path.to_path_buf()),
            stderr: None,
            stdout: None,
        });
    }

    Ok(())
}

/// Execute a VCS command, ignoring errors.
///
/// Useful for cleanup operations where failure is acceptable.
#[allow(dead_code)]
pub async fn run_vcs_command_ignore_error<P: AsRef<Path>>(program: &str, args: &[&str], cwd: P) {
    debug!(
        module = module_path!(),
        "Executing {} command (ignore errors): {} (cwd: {:?})",
        program,
        args.join(" "),
        cwd.as_ref()
    );
    let _ = Command::new(program)
        .args(args)
        .current_dir(cwd.as_ref())
        .stdin(Stdio::null())
        .output()
        .await;
}

/// Check if a VCS program is available.
///
/// Returns true if the program can be executed with --version.
pub async fn check_vcs_available<P: AsRef<Path>>(program: &str, cwd: P) -> VcsResult<bool> {
    debug!(
        module = module_path!(),
        "Executing {} command: {} (cwd: {:?})",
        program,
        "--version",
        cwd.as_ref()
    );
    let version_result = Command::new(program)
        .arg("--version")
        .current_dir(cwd.as_ref())
        .stdin(Stdio::null())
        .output()
        .await;

    match version_result {
        Ok(out) if out.status.success() => Ok(true),
        _ => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_check_vcs_available_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        // Non-existent program should return false
        let result = check_vcs_available("nonexistent-vcs-program", temp_dir.path()).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_vcs_error_includes_command_context() {
        let temp_dir = TempDir::new().unwrap();

        // Run a git command that will fail (invalid subcommand)
        let result = run_vcs_command(
            "git",
            &["invalid-subcommand-xyz"],
            temp_dir.path(),
            VcsBackend::Git,
        )
        .await;

        // Verify the command failed
        assert!(result.is_err());

        let err = result.unwrap_err();
        match err {
            VcsError::Command {
                command,
                working_dir,
                stderr,
                stdout,
                ..
            } => {
                // Verify command context is included
                assert!(command.is_some());
                let cmd = command.unwrap();
                assert!(cmd.contains("git"));
                assert!(cmd.contains("invalid-subcommand-xyz"));

                // Verify working directory is included
                assert!(working_dir.is_some());
                assert_eq!(working_dir.unwrap(), temp_dir.path());

                // Verify stderr is captured
                assert!(stderr.is_some());
                let stderr_str = stderr.unwrap();
                assert!(!stderr_str.is_empty());

                // stdout may or may not be present
                assert!(stdout.is_some());
            }
            _ => panic!("Expected VcsError::Command variant"),
        }
    }

    // === Streamed capture ===

    /// Run `sh -c body` through the streaming runner, collecting every emitted
    /// line alongside the captured result.
    async fn stream_shell(
        body: &str,
        cwd: &Path,
    ) -> (VcsCommandOutput, Vec<(&'static str, String)>) {
        let mut lines = Vec::new();
        let output =
            run_vcs_command_streamed("sh", &["-c", body], cwd, VcsBackend::Git, |stream, line| {
                lines.push((stream.as_str(), line.to_string()))
            })
            .await
            .expect("a spawnable command must not error");
        (output, lines)
    }

    /// The streamed runner must be a drop-in for the captured one: same command
    /// string, same exit code, same raw buffers. Classification reads those, so
    /// anything less would change what a hook rejection looks like.
    #[tokio::test]
    async fn streamed_capture_matches_the_captured_contract() {
        let temp_dir = TempDir::new().unwrap();
        let body = "printf 'out1\\nout2\\n'; printf 'err1\\n' >&2; exit 3";

        let captured =
            run_vcs_command_captured("sh", &["-c", body], temp_dir.path(), VcsBackend::Git)
                .await
                .unwrap();
        let (streamed, lines) = stream_shell(body, temp_dir.path()).await;

        assert_eq!(streamed.command, captured.command);
        assert_eq!(streamed.exit_code, captured.exit_code);
        assert_eq!(streamed.success, captured.success);
        assert_eq!(streamed.stdout, captured.stdout);
        assert_eq!(streamed.stderr, captured.stderr);
        assert_eq!(streamed.exit_code, Some(3));

        assert_eq!(
            lines,
            vec![
                ("stdout", "out1".to_string()),
                ("stdout", "out2".to_string()),
                ("stderr", "err1".to_string()),
            ]
        );
    }

    /// Classification buffers keep raw bytes. Only whichever sink renders a line
    /// may strip ANSI, so the tee must hand the escape sequences through intact.
    #[tokio::test]
    async fn streamed_lines_and_buffers_preserve_ansi_sequences() {
        let temp_dir = TempDir::new().unwrap();
        let (output, lines) =
            stream_shell("printf '\\033[31mred\\033[0m\\n'", temp_dir.path()).await;

        assert!(
            output.stdout.contains('\u{1b}'),
            "the classification buffer must stay raw: {:?}",
            output.stdout
        );
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].1.contains('\u{1b}'),
            "the emitted line must stay raw too: {:?}",
            lines[0].1
        );
    }

    /// Output that never ends with a newline is still emitted, and still lands
    /// in the raw buffer byte-for-byte.
    #[tokio::test]
    async fn a_trailing_partial_line_is_emitted_once() {
        let temp_dir = TempDir::new().unwrap();
        let (output, lines) = stream_shell("printf 'no-newline'", temp_dir.path()).await;

        assert_eq!(output.stdout, "no-newline");
        assert_eq!(lines, vec![("stdout", "no-newline".to_string())]);
    }

    /// CRLF is a line terminator, not content.
    #[tokio::test]
    async fn carriage_returns_are_trimmed_from_emitted_lines_only() {
        let temp_dir = TempDir::new().unwrap();
        let (output, lines) = stream_shell("printf 'crlf\\r\\n'", temp_dir.path()).await;

        assert_eq!(output.stdout, "crlf\r\n", "the raw buffer is untouched");
        assert_eq!(lines, vec![("stdout", "crlf".to_string())]);
    }

    /// A hook that writes far more than one pipe buffer must not deadlock, and
    /// every line must arrive.
    #[tokio::test]
    async fn a_large_stream_is_fully_drained_without_deadlocking() {
        let temp_dir = TempDir::new().unwrap();
        let (output, lines) = stream_shell(
            "i=0; while [ $i -lt 2000 ]; do echo \"line $i\"; i=$((i+1)); done",
            temp_dir.path(),
        )
        .await;

        assert!(output.success);
        assert_eq!(lines.len(), 2000);
        assert_eq!(lines[0].1, "line 0");
        assert_eq!(lines[1999].1, "line 1999");
    }

    /// A spawn failure must still be a terminal error rather than an empty
    /// "successful" capture.
    #[tokio::test]
    async fn a_spawn_failure_is_reported_as_a_command_error() {
        let temp_dir = TempDir::new().unwrap();
        let error = run_vcs_command_streamed(
            "nonexistent-vcs-program",
            &["--version"],
            temp_dir.path(),
            VcsBackend::Git,
            |_, _| {},
        )
        .await
        .expect_err("a program that cannot be spawned must not report success");

        assert!(matches!(error, VcsError::Command { .. }), "{error:?}");
    }
}
