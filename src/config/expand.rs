//! Placeholder expansion utilities for command templates.

use std::borrow::Cow;

const PLACEHOLDER_CHANGE_ID: &str = "{change_id}";
const PLACEHOLDER_PROMPT: &str = "{prompt}";
const PLACEHOLDER_CONFLICT_FILES: &str = "{conflict_files}";
#[allow(dead_code)]
const PLACEHOLDER_PROPOSAL: &str = "{proposal}";
const PLACEHOLDER_WORKSPACE_DIR: &str = "{workspace_dir}";
const PLACEHOLDER_REPO_ROOT: &str = "{repo_root}";

/// Expand `{change_id}` placeholder in a command template.
///
/// # Example
///
/// ```ignore
/// let template = "agent run --apply {change_id}";
/// let result = expand_change_id(template, "update-auth");
/// assert_eq!(result, "agent run --apply update-auth");
/// ```
pub fn expand_change_id(template: &str, change_id: &str) -> String {
    expand_placeholder(template, PLACEHOLDER_CHANGE_ID, change_id)
}

/// Expand `{prompt}` placeholder in a command template.
///
/// Prompts are shell-escaped via `shlex::try_quote()` on POSIX platforms.
/// If the placeholder is already inside single quotes in the template,
/// the outer quotes are removed to avoid double-quoting.
///
/// # Example
///
/// ```ignore
/// let template = "claude '{prompt}'";
/// let result = expand_prompt(template, "Select the next change");
/// assert_eq!(result, "claude 'Select the next change'");
/// ```
pub fn expand_prompt(template: &str, prompt: &str) -> String {
    expand_placeholder(template, PLACEHOLDER_PROMPT, prompt)
}

/// Expand `{conflict_files}` placeholder in a command template.
#[allow(dead_code)]
pub fn expand_conflict_files(template: &str, conflict_files: &str) -> String {
    expand_placeholder(template, PLACEHOLDER_CONFLICT_FILES, conflict_files)
}

/// Expand `{proposal}` placeholder in a command template for proposing new changes.
///
/// # Example
///
/// ```ignore
/// let template = "opencode run '{proposal}'";
/// let result = expand_proposal(template, "Add user authentication feature");
/// assert_eq!(result, "opencode run 'Add user authentication feature'");
/// ```
#[allow(dead_code)]
pub fn expand_proposal(template: &str, proposal: &str) -> String {
    expand_placeholder(template, PLACEHOLDER_PROPOSAL, proposal)
}

/// Expand `{workspace_dir}` and `{repo_root}` placeholders in a command template.
pub fn expand_worktree_command(template: &str, workspace_dir: &str, repo_root: &str) -> String {
    let command = expand_placeholder(template, PLACEHOLDER_WORKSPACE_DIR, workspace_dir);
    expand_placeholder(&command, PLACEHOLDER_REPO_ROOT, repo_root)
}

pub(crate) fn expand_placeholder(template: &str, placeholder: &str, value: &str) -> String {
    if !template.contains(placeholder) {
        return template.to_string();
    }

    let mut result = String::with_capacity(template.len() + value.len());
    let mut last_index = 0;

    for (index, _) in template.match_indices(placeholder) {
        let in_single_quotes = is_within_single_quotes(template, index);
        result.push_str(&template[last_index..index]);
        result.push_str(&escape_shell_value(value, in_single_quotes));
        last_index = index + placeholder.len();
    }

    result.push_str(&template[last_index..]);
    result
}

fn escape_shell_value(value: &str, in_single_quotes: bool) -> String {
    if cfg!(windows) {
        return sanitize_windows_value(value);
    }

    let sanitized = sanitize_posix_value(value);
    let quoted =
        shlex::try_quote(sanitized.as_ref()).unwrap_or_else(|_| Cow::Borrowed(sanitized.as_ref()));

    if in_single_quotes {
        if quoted.as_ref().starts_with('\'') && quoted.as_ref().ends_with('\'') {
            return strip_outer_single_quotes(quoted.as_ref()).to_string();
        }
        return escape_for_single_quoted_context(sanitized.as_ref());
    }

    quoted.to_string()
}

fn sanitize_posix_value(value: &str) -> Cow<'_, str> {
    if value.contains('\0') {
        Cow::Owned(value.replace('\0', ""))
    } else {
        Cow::Borrowed(value)
    }
}

fn sanitize_windows_value(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            '\0' | '\r' | '\n' => ' ',
            _ => c,
        })
        .collect()
}

fn escape_for_single_quoted_context(value: &str) -> String {
    let sanitized = sanitize_posix_value(value);
    sanitized.as_ref().replace('\'', r"'\''")
}

fn strip_outer_single_quotes(value: &str) -> &str {
    if value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2 {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

fn is_within_single_quotes(template: &str, index: usize) -> bool {
    let mut in_single_quotes = false;
    let mut escaped = false;

    for (position, ch) in template.char_indices() {
        if position >= index {
            break;
        }

        if escaped {
            escaped = false;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            continue;
        }

        if ch == '\'' {
            in_single_quotes = !in_single_quotes;
        }
    }

    in_single_quotes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_change_id() {
        let template = "agent run --apply {change_id}";
        let result = expand_change_id(template, "update-auth");
        assert_eq!(result, "agent run --apply update-auth");
    }

    #[test]
    fn test_expand_change_id_multiple() {
        let template = "agent --id {change_id} --name {change_id}";
        let result = expand_change_id(template, "fix-bug");
        assert_eq!(result, "agent --id fix-bug --name fix-bug");
    }

    #[test]
    fn test_expand_change_id_with_whitespace() {
        let template = "agent run --apply {change_id}";
        let result = expand_change_id(template, "fix bug");
        assert_eq!(result, "agent run --apply 'fix bug'");
    }

    #[test]
    fn test_expand_prompt_unquoted_template() {
        let template = "claude {prompt}";
        let result = expand_prompt(template, "Select the next change");
        assert_eq!(result, "claude 'Select the next change'");
    }

    #[test]
    fn test_expand_prompt_single_quoted_template() {
        let template = "claude '{prompt}'";
        let result = expand_prompt(template, "Select the next change");
        assert_eq!(result, "claude 'Select the next change'");
    }

    #[test]
    fn test_expand_prompt_in_apply_command() {
        let template = "claude -p '/openspec:apply {change_id} {prompt}'";
        let command = expand_change_id(template, "fix-bug");
        let command = expand_prompt(&command, "Custom instructions");
        assert_eq!(
            command,
            "claude -p '/openspec:apply fix-bug Custom instructions'"
        );
    }

    #[test]
    fn test_expand_prompt_with_empty_string() {
        let template = "claude -p '/openspec:archive {change_id} {prompt}'";
        let command = expand_change_id(template, "add-feature");
        let command = expand_prompt(&command, "");
        assert_eq!(command, "claude -p '/openspec:archive add-feature '");
    }

    #[test]
    fn test_backward_compatible_no_prompt_placeholder() {
        // Commands without {prompt} placeholder should continue to work
        let template = "claude -p '/openspec:apply {change_id}'";
        let command = expand_change_id(template, "fix-bug");
        let command = expand_prompt(&command, "Ignored prompt");
        // The {prompt} replacement does nothing since placeholder doesn't exist
        assert_eq!(command, "claude -p '/openspec:apply fix-bug'");
    }

    #[test]
    fn test_expand_conflict_files() {
        let template = "resolve --files {conflict_files}";
        let result = expand_conflict_files(template, "file1.rs,file2.rs");
        let expected = format!(
            "resolve --files {}",
            shlex::try_quote("file1.rs,file2.rs").unwrap()
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_expand_conflict_files_with_spaces() {
        let template = "resolve --files {conflict_files}";
        let result = expand_conflict_files(template, "file 1.rs file 2.rs");
        let expected = format!(
            "resolve --files {}",
            shlex::try_quote("file 1.rs file 2.rs").unwrap()
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_expand_proposal() {
        let template = "opencode run {proposal}";
        let result = expand_proposal(template, "Add user authentication feature");
        assert_eq!(result, "opencode run 'Add user authentication feature'");
    }

    #[test]
    fn test_expand_proposal_multiline() {
        let template = "claude {proposal}";
        let text = "Feature request:\n- Add login\n- Add logout";
        let result = expand_proposal(template, text);
        assert_eq!(
            result,
            "claude 'Feature request:\n- Add login\n- Add logout'"
        );
    }

    #[test]
    fn test_expand_worktree_command() {
        let template = "run --cwd {workspace_dir} --repo {repo_root}";
        let result = expand_worktree_command(template, "/tmp/worktree", "/repo/root");
        assert_eq!(result, "run --cwd /tmp/worktree --repo /repo/root");
    }

    #[test]
    fn test_expand_worktree_command_escaped() {
        let template = "cmd {workspace_dir} {repo_root}";
        let result = expand_worktree_command(template, "/tmp/work tree", "/repo/root path");
        assert_eq!(result, "cmd '/tmp/work tree' '/repo/root path'");
    }

    #[test]
    fn test_expand_prompt_with_single_quotes() {
        let template = "claude -p 'apply {prompt}'";
        let result = expand_prompt(template, "it's a test");
        assert_eq!(result, "claude -p 'apply it'\\''s a test'");
    }

    #[test]
    fn test_expand_prompt_multiline() {
        let template = "claude {prompt}";
        let text = "Line 1\nLine 2\nLine 3";
        let result = expand_prompt(template, text);
        assert_eq!(result, "claude 'Line 1\nLine 2\nLine 3'");
    }

    #[test]
    fn test_expand_prompt_with_special_chars() {
        let template = "claude {prompt}";
        let prompt = "$HOME `echo` ! \\\\";
        let result = expand_prompt(template, prompt);
        let expected = format!("claude {}", shlex::try_quote(prompt).unwrap());
        assert_eq!(result, expected);
    }

    #[test]
    fn test_expand_prompt_multibyte_chars() {
        let template = "claude {prompt}";
        let result = expand_prompt(template, "こんにちは 🌟");
        assert_eq!(result, "claude 'こんにちは 🌟'");
    }

    #[test]
    fn test_expand_prompt_quoted_template_no_double_quotes() {
        let template = "claude '{prompt}'";
        let result = expand_prompt(template, "Hello world");
        assert_eq!(result, "claude 'Hello world'");
    }
}
