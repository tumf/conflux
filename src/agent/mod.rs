//! Agent runner module for executing configurable agent commands.
//!
//! This module provides a generic agent runner that executes shell commands
//! based on configuration templates. It replaces the OpenCode-specific runner
//! with a configurable approach.

mod history_ops;
mod output;
pub(crate) mod prompt;
mod runner;

// Re-export public types for backward compatibility
pub use output::OutputLine;
#[allow(unused_imports)]
pub use prompt::{
    append_optional_prompt, build_acceptance_diff_context, build_acceptance_findings_context,
    build_acceptance_prompt, build_acceptance_prompt_context_only,
    build_acceptance_prompt_context_only_with_skill, build_acceptance_prompt_with_skill,
    build_apply_prompt, build_apply_prompt_with_skill, build_archive_prompt,
    build_archive_prompt_with_skill, build_cleanup_review_prompt,
    build_cleanup_review_prompt_with_skill, build_last_acceptance_output_context,
    build_missing_verdict_continuation_context, build_task_format_repair_context,
    parse_cleanup_review_output,
};
pub use runner::AgentRunner;

#[cfg(test)]
mod tests;
