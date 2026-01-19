//! Output line types for agent command execution.

/// Output line from a child process
#[derive(Debug, Clone)]
pub enum OutputLine {
    Stdout(String),
    Stderr(String),
}
