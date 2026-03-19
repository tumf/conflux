use agent_skills_rs::{
    discover_skills, install_skill, DiscoveryConfig, InstallConfig, LockManager,
    types::{Source, SourceType},
};
use anyhow::{bail, Result};
use std::path::PathBuf;

/// Options for the install-skills command.
pub struct InstallSkillsOptions {
    /// Source string: "self" or "local:<path>"
    pub source_str: String,
    /// When true, install to ~/.agents/skills; otherwise ./.agents/skills
    pub global: bool,
    /// Override the project root for project-scope installs (defaults to CWD when None).
    /// Primarily used in tests to avoid changing process-wide CWD.
    pub project_root: Option<std::path::PathBuf>,
}

/// Parse the source string into an `agent-skills-rs` Source.
///
/// Supported forms:
/// - `self`          → local source pointing at the repository's top-level `skills/` directory
/// - `local:<path>`  → local source pointing at the given path
///
/// Any other form returns an error with an allowed-schemes message.
///
/// `project_root` is used to resolve the `self` source path when provided.
fn parse_source(source_str: &str, project_root: &std::path::Path) -> Result<Source> {
    if source_str == "self" {
        let skills_path = project_root.join("skills");
        Ok(Source {
            source_type: SourceType::Local,
            url: Some(skills_path.to_string_lossy().into_owned()),
            subpath: None,
            skill_filter: None,
            ref_: None,
        })
    } else if let Some(path) = source_str.strip_prefix("local:") {
        if path.is_empty() {
            bail!("'local:<path>' requires a non-empty path after the colon");
        }
        Ok(Source {
            source_type: SourceType::Local,
            url: Some(path.to_string()),
            subpath: None,
            skill_filter: None,
            ref_: None,
        })
    } else {
        bail!(
            "Unsupported source '{}'. Only 'self' and 'local:<path>' are supported.",
            source_str
        )
    }
}

/// Resolve installation directories for the given scope.
///
/// Returns `(skills_dir, lock_path)`.
/// For project scope, paths are resolved relative to `project_root`.
fn resolve_install_paths(global: bool, project_root: &std::path::Path) -> Result<(PathBuf, PathBuf)> {
    if global {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
        Ok((
            home.join(".agents").join("skills"),
            home.join(".agents").join(".skill-lock.json"),
        ))
    } else {
        Ok((
            project_root.join(".agents").join("skills"),
            project_root.join(".agents").join(".skill-lock.json"),
        ))
    }
}

/// Execute the install-skills command.
pub fn run_install_skills(opts: InstallSkillsOptions) -> Result<()> {
    let project_root = match opts.project_root {
        Some(ref p) => p.clone(),
        None => std::env::current_dir()
            .map_err(|e| anyhow::anyhow!("Cannot determine current directory: {e}"))?,
    };
    let source = parse_source(&opts.source_str, &project_root)?;
    let (canonical_dir, lock_path) = resolve_install_paths(opts.global, &project_root)?;

    let config = DiscoveryConfig::default();
    let skills = discover_skills(&source, &config)?;

    if skills.is_empty() {
        println!("No skills found from source '{}'.", opts.source_str);
        return Ok(());
    }

    let install_config = InstallConfig::new(canonical_dir);
    let lock_manager = LockManager::new(lock_path);

    for skill in &skills {
        println!("Installing skill: {}", skill.name);
        let result = install_skill(skill, &install_config)?;
        lock_manager.update_entry(&skill.name, &source, &result.path)?;
        println!("  -> {}", result.path.display());
    }

    println!("Successfully installed {} skill(s).", skills.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_source_self() {
        let root = PathBuf::from("/project");
        let source = parse_source("self", &root).unwrap();
        assert!(matches!(source.source_type, SourceType::Local));
        assert_eq!(source.url.as_deref(), Some("/project/skills"));
    }

    #[test]
    fn test_parse_source_local_relative() {
        let root = PathBuf::from("/project");
        let source = parse_source("local:../my-skills", &root).unwrap();
        assert!(matches!(source.source_type, SourceType::Local));
        assert_eq!(source.url.as_deref(), Some("../my-skills"));
    }

    #[test]
    fn test_parse_source_local_absolute() {
        let root = PathBuf::from("/project");
        let source = parse_source("local:/tmp/skills", &root).unwrap();
        assert!(matches!(source.source_type, SourceType::Local));
        assert_eq!(source.url.as_deref(), Some("/tmp/skills"));
    }

    #[test]
    fn test_parse_source_local_empty_path_fails() {
        let root = PathBuf::from("/project");
        let result = parse_source("local:", &root);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("non-empty path"), "got: {msg}");
    }

    #[test]
    fn test_parse_source_unsupported_scheme_fails() {
        let root = PathBuf::from("/project");
        let result = parse_source("git:https://example.com/repo", &root);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("self"), "error should mention 'self': {msg}");
        assert!(
            msg.contains("local:<path>"),
            "error should mention 'local:<path>': {msg}"
        );
    }

    #[test]
    fn test_parse_source_unknown_word_fails() {
        let root = PathBuf::from("/project");
        let result = parse_source("registry:https://example.com", &root);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_install_paths_project_scope() {
        let root = PathBuf::from("/my/project");
        let (skills_dir, lock_path) = resolve_install_paths(false, &root).unwrap();
        assert_eq!(skills_dir, root.join(".agents/skills"));
        assert_eq!(lock_path, root.join(".agents/.skill-lock.json"));
    }

    #[test]
    fn test_resolve_install_paths_global_scope() {
        let root = PathBuf::from("/my/project");
        let (skills_dir, lock_path) = resolve_install_paths(true, &root).unwrap();
        let home = dirs::home_dir().unwrap();
        assert_eq!(skills_dir, home.join(".agents/skills"));
        assert_eq!(lock_path, home.join(".agents/.skill-lock.json"));
    }
}
