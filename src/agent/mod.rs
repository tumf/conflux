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
pub use prompt::{
    build_acceptance_diff_context, build_acceptance_prompt, build_acceptance_prompt_context_only,
    build_apply_prompt, build_archive_prompt, build_last_acceptance_output_context,
};
pub use runner::AgentRunner;

#[cfg(test)]
mod tests;
