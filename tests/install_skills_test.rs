/// Integration / filesystem tests for `cflx install-skills`.
///
/// These tests verify that `run_install_skills` correctly writes skills to
/// the expected directories and updates the matching lock file for both
/// project-scope and global-scope installs using bundled skills.
use std::fs;
use std::sync::Mutex;

use agent_skills_rs::LockManager;
use conflux::install_skills::{run_install_skills, InstallSkillsOptions};
use tempfile::TempDir;

/// Single process-wide mutex for tests that mutate the HOME env variable.
static HOME_MUTEX: Mutex<()> = Mutex::new(());

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a minimal skills directory with one synthetic skill for testing.
fn create_test_skills_dir(base: &TempDir) {
    let skill_dir = base.path().join("skills").join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: test-skill\ndescription: A test skill\n---\n\n# Test Skill\nContent here.\n",
    )
    .unwrap();
}

// ---------------------------------------------------------------------------
// Project-scope install tests
// ---------------------------------------------------------------------------

#[test]
fn test_project_scope_install_creates_agents_skills_dir() {
    let workdir = TempDir::new().unwrap();
    create_test_skills_dir(&workdir);

    let opts = InstallSkillsOptions {
        global: false,
        project_root: Some(workdir.path().to_path_buf()),
    };
    run_install_skills(opts).unwrap();

    let skill_path = workdir.path().join(".agents/skills/test-skill");
    assert!(
        skill_path.exists(),
        "Expected skill directory at {skill_path:?}"
    );

    let lock_path = workdir.path().join(".agents/.skill-lock.json");
    assert!(lock_path.exists(), "Expected lock file at {lock_path:?}");
}

#[test]
fn test_project_scope_install_updates_lock_file() {
    let workdir = TempDir::new().unwrap();
    create_test_skills_dir(&workdir);

    let opts = InstallSkillsOptions {
        global: false,
        project_root: Some(workdir.path().to_path_buf()),
    };
    run_install_skills(opts).unwrap();

    let lock_path = workdir.path().join(".agents/.skill-lock.json");
    let lock_manager = LockManager::new(lock_path);
    let entry = lock_manager.get_entry("test-skill").unwrap();
    assert!(entry.is_some(), "Lock entry for 'test-skill' should exist");
    let entry = entry.unwrap();
    assert_eq!(entry.source_type, "local");
}

// ---------------------------------------------------------------------------
// Global-scope install tests
// ---------------------------------------------------------------------------

#[test]
fn test_global_scope_install_uses_home_agents_dir() {
    let workdir = TempDir::new().unwrap();
    let fake_home = TempDir::new().unwrap();
    create_test_skills_dir(&workdir);

    let _guard = HOME_MUTEX.lock().unwrap();

    let orig_home = std::env::var("HOME").ok();
    std::env::set_var("HOME", fake_home.path());

    let opts = InstallSkillsOptions {
        global: true,
        project_root: Some(workdir.path().to_path_buf()),
    };
    let result = run_install_skills(opts);

    match orig_home {
        Some(h) => std::env::set_var("HOME", h),
        None => std::env::remove_var("HOME"),
    }
    drop(_guard);

    result.unwrap();

    let skill_path = fake_home.path().join(".agents/skills/test-skill");
    assert!(
        skill_path.exists(),
        "Expected global skill at {skill_path:?}"
    );

    let lock_path = fake_home.path().join(".agents/.skill-lock.json");
    assert!(
        lock_path.exists(),
        "Expected global lock file at {lock_path:?}"
    );
}

#[test]
fn test_global_scope_lock_entry_exists() {
    let workdir = TempDir::new().unwrap();
    let fake_home = TempDir::new().unwrap();
    create_test_skills_dir(&workdir);

    let _guard = HOME_MUTEX.lock().unwrap();

    let orig_home = std::env::var("HOME").ok();
    std::env::set_var("HOME", fake_home.path());

    let opts = InstallSkillsOptions {
        global: true,
        project_root: Some(workdir.path().to_path_buf()),
    };
    run_install_skills(opts).unwrap();

    match orig_home {
        Some(h) => std::env::set_var("HOME", h),
        None => std::env::remove_var("HOME"),
    }
    drop(_guard);

    let lock_path = fake_home.path().join(".agents/.skill-lock.json");
    let lock_manager = LockManager::new(lock_path);
    let entry = lock_manager.get_entry("test-skill").unwrap();
    assert!(
        entry.is_some(),
        "Global lock entry for 'test-skill' should exist"
    );
}
