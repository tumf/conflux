use agent_skills_rs::{
    discover_skills, install_skill,
    types::{Source, SourceType},
    DiscoveryConfig, InstallConfig, LockManager,
};
use anyhow::Result;
use std::path::PathBuf;

/// Options for the install-skills command.
pub struct InstallSkillsOptions {
    /// When true, install to ~/.agents/skills; otherwise ./.agents/skills
    pub global: bool,
    /// Override the project root for project-scope installs (defaults to CWD when None).
    /// Primarily used in tests to avoid changing process-wide CWD.
    pub project_root: Option<std::path::PathBuf>,
}

/// Resolve installation directories for the given scope.
///
/// Returns `(skills_dir, lock_path)`.
/// For project scope, paths are resolved relative to `project_root`.
fn resolve_install_paths(
    global: bool,
    project_root: &std::path::Path,
) -> Result<(PathBuf, PathBuf)> {
    if global {
        let home =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
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
///
/// Always installs from the bundled repository `skills/` directory.
pub fn run_install_skills(opts: InstallSkillsOptions) -> Result<()> {
    let project_root = match opts.project_root {
        Some(ref p) => p.clone(),
        None => std::env::current_dir()
            .map_err(|e| anyhow::anyhow!("Cannot determine current directory: {e}"))?,
    };

    let source = Source {
        source_type: SourceType::Local,
        url: Some(project_root.join("skills").to_string_lossy().into_owned()),
        subpath: None,
        skill_filter: None,
        ref_: None,
    };

    let (canonical_dir, lock_path) = resolve_install_paths(opts.global, &project_root)?;

    let config = DiscoveryConfig::default();
    let skills = discover_skills(&source, &config)?;

    if skills.is_empty() {
        println!("No skills found in skills/ directory.");
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
