use agent_skills_rs::{
    discover_skills, install_skill,
    types::{Source, SourceType},
    DiscoveryConfig, InstallConfig, LockManager,
};
use anyhow::Result;
use std::path::PathBuf;

use crate::embedded_skills::get_cflx_embedded_skills;

/// Install target family for `install-skills`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallTarget {
    Agents,
    Claude,
}

impl InstallTarget {
    fn root_dir_name(self) -> &'static str {
        match self {
            Self::Agents => ".agents",
            Self::Claude => ".claude",
        }
    }
}

/// Options for the install-skills command.
pub struct InstallSkillsOptions {
    /// When true, install to home directory scope; otherwise project scope.
    pub global: bool,
    /// Install target root (`.agents` or `.claude`).
    pub target: InstallTarget,
    /// Override the project root for project-scope installs (defaults to CWD when None).
    /// Primarily used in tests to avoid changing process-wide CWD.
    pub project_root: Option<std::path::PathBuf>,
}

/// Resolve installation directories for the given scope and target.
///
/// Returns `(skills_dir, lock_path)`.
/// For project scope, paths are resolved relative to `project_root`.
fn resolve_install_paths(
    global: bool,
    target: InstallTarget,
    project_root: &std::path::Path,
) -> Result<(PathBuf, PathBuf)> {
    let root = target.root_dir_name();
    if global {
        let home =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
        Ok((
            home.join(root).join("skills"),
            home.join(root).join(".skill-lock.json"),
        ))
    } else {
        Ok((
            project_root.join(root).join("skills"),
            project_root.join(root).join(".skill-lock.json"),
        ))
    }
}

/// Execute the install-skills command.
///
/// Prefers embedded skills compiled into the binary at build time.
/// Falls back to local `skills/` directory discovery only when embedded skills
/// are unavailable (e.g. during library-only builds or testing without embed).
pub fn run_install_skills(opts: InstallSkillsOptions) -> Result<()> {
    let project_root = match opts.project_root {
        Some(ref p) => p.clone(),
        None => std::env::current_dir()
            .map_err(|e| anyhow::anyhow!("Cannot determine current directory: {e}"))?,
    };

    let (canonical_dir, lock_path) =
        resolve_install_paths(opts.global, opts.target, &project_root)?;
    let install_config = InstallConfig::new(canonical_dir);
    let lock_manager = LockManager::new(lock_path);

    // Prefer embedded skills compiled into the binary at build time.
    let embedded_skills = get_cflx_embedded_skills()?;
    if !embedded_skills.is_empty() {
        let source = Source {
            source_type: SourceType::Self_,
            url: None,
            subpath: None,
            skill_filter: None,
            ref_: None,
        };
        for skill in &embedded_skills {
            println!("Installing skill: {}", skill.name);
            let result = install_skill(skill, &install_config)?;
            lock_manager.update_entry(&skill.name, &source, &result.path)?;
            println!("  -> {}", result.path.display());
        }
        println!("Successfully installed {} skill(s).", embedded_skills.len());
        return Ok(());
    }

    // Dev fallback: use local skills/ directory when embedded skills are unavailable.
    let local_skills_dir = project_root.join("skills");
    if local_skills_dir.is_dir() {
        let source = Source {
            source_type: SourceType::Local,
            url: Some(local_skills_dir.to_string_lossy().into_owned()),
            subpath: None,
            skill_filter: None,
            ref_: None,
        };
        let config = DiscoveryConfig::default();
        let skills = discover_skills(&source, &config)?;
        if skills.is_empty() {
            println!("No skills found in skills/ directory.");
            return Ok(());
        }
        for skill in &skills {
            println!("Installing skill: {}", skill.name);
            let result = install_skill(skill, &install_config)?;
            lock_manager.update_entry(&skill.name, &source, &result.path)?;
            println!("  -> {}", result.path.display());
        }
        println!("Successfully installed {} skill(s).", skills.len());
        return Ok(());
    }

    println!("No bundled skills available.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_install_paths_project_scope() {
        let root = PathBuf::from("/my/project");
        let (skills_dir, lock_path) =
            resolve_install_paths(false, InstallTarget::Agents, &root).unwrap();
        assert_eq!(skills_dir, root.join(".agents/skills"));
        assert_eq!(lock_path, root.join(".agents/.skill-lock.json"));
    }

    #[test]
    fn test_resolve_install_paths_global_scope() {
        let root = PathBuf::from("/my/project");
        let (skills_dir, lock_path) =
            resolve_install_paths(true, InstallTarget::Agents, &root).unwrap();
        let home = dirs::home_dir().unwrap();
        assert_eq!(skills_dir, home.join(".agents/skills"));
        assert_eq!(lock_path, home.join(".agents/.skill-lock.json"));
    }

    #[test]
    fn test_resolve_install_paths_project_scope_claude_target() {
        let root = PathBuf::from("/my/project");
        let (skills_dir, lock_path) =
            resolve_install_paths(false, InstallTarget::Claude, &root).unwrap();
        assert_eq!(skills_dir, root.join(".claude/skills"));
        assert_eq!(lock_path, root.join(".claude/.skill-lock.json"));
    }

    #[test]
    fn test_resolve_install_paths_global_scope_claude_target() {
        let root = PathBuf::from("/my/project");
        let (skills_dir, lock_path) =
            resolve_install_paths(true, InstallTarget::Claude, &root).unwrap();
        let home = dirs::home_dir().unwrap();
        assert_eq!(skills_dir, home.join(".claude/skills"));
        assert_eq!(lock_path, home.join(".claude/.skill-lock.json"));
    }
}
