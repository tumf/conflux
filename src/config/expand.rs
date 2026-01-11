//! Placeholder expansion utilities for command templates.

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
    template.replace("{change_id}", change_id)
}

/// Expand `{prompt}` placeholder in a command template.
///
/// # Example
///
/// ```ignore
/// let template = "claude '{prompt}'";
/// let result = expand_prompt(template, "Select the next change");
/// assert_eq!(result, "claude 'Select the next change'");
/// ```
pub fn expand_prompt(template: &str, prompt: &str) -> String {
    template.replace("{prompt}", prompt)
}

/// Expand `{conflict_files}` placeholder in a command template.
#[allow(dead_code)]
pub fn expand_conflict_files(template: &str, conflict_files: &str) -> String {
    template.replace("{conflict_files}", conflict_files)
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
    fn test_expand_prompt() {
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
        assert_eq!(result, "resolve --files file1.rs,file2.rs");
    }
}
