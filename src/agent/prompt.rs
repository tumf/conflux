//! Prompt building functions for agent commands.

use crate::config::defaults::ACCEPTANCE_SYSTEM_PROMPT;

/// Legacy hardcoded system prompt for apply commands.
/// Kept only for compatibility in tests; actual prompt is sourced from OpenCode command files.
pub const APPLY_SYSTEM_PROMPT: &str = "";

/// Build apply prompt from user prompt and history context
/// Format: user_prompt + APPLY_SYSTEM_PROMPT + history_context
pub fn build_apply_prompt(user_prompt: &str, history_context: &str) -> String {
    let mut parts = Vec::new();

    if !user_prompt.is_empty() {
        parts.push(user_prompt.to_string());
    }

    // System prompt is always included
    parts.push(APPLY_SYSTEM_PROMPT.to_string());

    if !history_context.is_empty() {
        parts.push(history_context.to_string());
    }

    parts.join("\n\n")
}

/// Build archive prompt from user prompt and history context
/// Format: user_prompt + history_context
pub fn build_archive_prompt(user_prompt: &str, history_context: &str) -> String {
    let mut parts = Vec::new();

    if !user_prompt.is_empty() {
        parts.push(user_prompt.to_string());
    }

    if !history_context.is_empty() {
        parts.push(history_context.to_string());
    }

    parts.join("\n\n")
}

/// Build acceptance prompt from user prompt and history context
///
/// The prompt is constructed as:
/// 1. ACCEPTANCE_SYSTEM_PROMPT with {change_id} expanded (always included)
/// 2. user_prompt (if not empty)
/// 3. history_context (if not empty)
pub fn build_acceptance_prompt(
    change_id: &str,
    user_prompt: &str,
    history_context: &str,
) -> String {
    let mut parts = Vec::new();

    // System prompt is always included first, with change_id expanded
    let system_prompt = ACCEPTANCE_SYSTEM_PROMPT.replace("{change_id}", change_id);
    parts.push(system_prompt);

    if !user_prompt.is_empty() {
        parts.push(user_prompt.to_string());
    }

    if !history_context.is_empty() {
        parts.push(history_context.to_string());
    }

    parts.join("\n\n")
}
