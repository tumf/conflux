//! Agent runner module for executing configurable agent commands.
//!
//! This module provides a generic agent runner that executes shell commands
//! based on configuration templates. It replaces the OpenCode-specific runner
//! with a configurable approach.

mod history_ops;
mod output;
mod prompt;
mod runner;

// Re-export public types for backward compatibility
pub use output::OutputLine;
pub use prompt::{build_apply_prompt, build_archive_prompt};
pub use runner::AgentRunner;

// Re-export for testing and potential future use
#[allow(unused_imports)]
pub use prompt::{build_acceptance_prompt, APPLY_SYSTEM_PROMPT};

#[cfg(test)]
mod tests;
