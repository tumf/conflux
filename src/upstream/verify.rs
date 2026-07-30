//! Native execution of the operator-supplied complete verification command.
//!
//! The command is explicit CLI input, runs only when upstream integration is
//! enabled, and executes from the cumulative base root through the same login
//! shell convention as the rest of Conflux's command execution. It is a native
//! Conflux operation: it never runs through the AI command harness, and its
//! process result — not any narrative output — is the gate.

use std::path::PathBuf;
use std::process::Stdio;

use async_trait::async_trait;
use tracing::{info, warn};

use super::ports::{PortResult, UpstreamPortError, UpstreamVerifier, VerificationOutcome};

/// Maximum bytes of combined output retained for bounded repair context.
const OUTPUT_TAIL_LIMIT: usize = 8 * 1024;

/// Runs `--upstream-verify-command` from the cumulative base root.
pub struct CommandVerifier {
    command: String,
    cwd: PathBuf,
}

impl CommandVerifier {
    pub fn new(command: impl Into<String>, cwd: impl Into<PathBuf>) -> Self {
        Self {
            command: command.into(),
            cwd: cwd.into(),
        }
    }
}

/// Keep the trailing `limit` bytes of `text`, on a character boundary.
fn tail(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_string();
    }
    let mut start = text.len() - limit;
    while start < text.len() && !text.is_char_boundary(start) {
        start += 1;
    }
    text[start..].to_string()
}

#[async_trait]
impl UpstreamVerifier for CommandVerifier {
    async fn verify(&self) -> PortResult<VerificationOutcome> {
        info!(
            command = %self.command,
            cwd = %self.cwd.display(),
            "Running upstream verification command"
        );

        let mut cmd = crate::shell_command::build_login_shell_command(&self.command);
        cmd.current_dir(&self.cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd
            .output()
            .await
            .map_err(|e| UpstreamPortError::new("upstream verification command", e.to_string()))?;

        if output.status.success() {
            return Ok(VerificationOutcome::passed());
        }

        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        warn!(
            command = %self.command,
            exit_code = ?output.status.code(),
            "Upstream verification command failed"
        );
        Ok(VerificationOutcome::failed(tail(
            &combined,
            OUTPUT_TAIL_LIMIT,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_integration_bounds_verification_output_tail() {
        let long = "x".repeat(OUTPUT_TAIL_LIMIT * 2);
        assert_eq!(tail(&long, OUTPUT_TAIL_LIMIT).len(), OUTPUT_TAIL_LIMIT);
        assert_eq!(tail("short", OUTPUT_TAIL_LIMIT), "short");
    }

    #[test]
    fn upstream_integration_tail_respects_char_boundaries() {
        let text = "あいうえお";
        let trimmed = tail(text, 7);
        assert!(text.ends_with(&trimmed));
        assert!(trimmed.len() <= 7);
    }
}
