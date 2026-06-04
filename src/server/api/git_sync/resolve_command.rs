use super::*;

/// Build a resolve command argv by parsing the template with shlex and substituting
/// `{prompt}` placeholders. Retained for test coverage.
#[cfg(test)]
pub(crate) fn build_resolve_command_argv(
    resolve_command_template: &str,
    prompt: &str,
) -> Result<Vec<String>, String> {
    let parts = shlex::split(resolve_command_template)
        .ok_or_else(|| "Failed to parse resolve_command (shlex split returned None)".to_string())?;
    if parts.is_empty() {
        return Err("resolve_command is empty".to_string());
    }

    Ok(parts
        .into_iter()
        .map(|p| p.replace("{prompt}", prompt))
        .collect())
}

pub(super) async fn run_resolve_command(
    resolve_command_template: &str,
    work_dir: &std::path::Path,
    prompt: &str,
    state: Option<&AppState>,
    project_id: Option<&str>,
) -> (bool, Option<i32>) {
    // Use the shared placeholder expansion from config::expand, which handles
    // both quoted ('{prompt}') and unquoted ({prompt}) template forms correctly,
    // avoiding double-quoting issues with multi-line prompts.
    let command_str = crate::config::expand::expand_prompt(resolve_command_template, prompt);

    info!(
        "Running resolve_command via login shell: command='{}'",
        command_str
    );

    // Send start event to project log
    if let (Some(state), Some(pid)) = (state, project_id) {
        super::route_orchestration::emit_resolve_log(
            state,
            pid,
            "info",
            format!("resolve_command started: {}", command_str),
        );
    }

    let mut cmd = crate::shell_command::build_login_shell_command(&command_str);
    cmd.current_dir(work_dir);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            error!(
                "Failed to run resolve_command '{}': {}",
                resolve_command_template, e
            );
            if let (Some(state), Some(pid)) = (state, project_id) {
                super::route_orchestration::emit_resolve_log(
                    state,
                    pid,
                    "error",
                    format!("resolve_command failed to start: {}", e),
                );
            }
            return (true, Some(-1));
        }
    };

    // Stream stdout/stderr to project log
    let output = match child.wait_with_output().await {
        Ok(output) => output,
        Err(e) => {
            error!(
                "Failed to wait for resolve_command '{}': {}",
                resolve_command_template, e
            );
            return (true, Some(-1));
        }
    };

    // Stream stdout/stderr lines and completion event to project log
    if let (Some(state), Some(pid)) = (state, project_id) {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if !line.is_empty() {
                super::route_orchestration::emit_resolve_log(state, pid, "info", line.to_string());
            }
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stderr.lines() {
            if !line.is_empty() {
                super::route_orchestration::emit_resolve_log(state, pid, "warn", line.to_string());
            }
        }

        let exit_code = output.status.code();
        let level = if output.status.success() {
            "success"
        } else {
            "error"
        };
        super::route_orchestration::emit_resolve_log(
            state,
            pid,
            level,
            format!("resolve_command finished: exit_code={:?}", exit_code),
        );
    }

    (true, output.status.code())
}
