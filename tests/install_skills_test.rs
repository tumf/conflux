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
fn test_project_scope_install_creates_agents_skills_dir_and_updates_lock_file() {
    // Embedded skills are preferred; no skills/ directory needed.
    let workdir = TempDir::new().unwrap();

    let opts = InstallSkillsOptions {
        global: false,
        project_root: Some(workdir.path().to_path_buf()),
    };
    run_install_skills(opts).unwrap();

    // Embedded cflx-proposal skill must be installed.
    let skill_path = workdir.path().join(".agents/skills/cflx-proposal");
    assert!(
        skill_path.exists(),
        "Expected embedded skill directory at {skill_path:?}"
    );

    let lock_path = workdir.path().join(".agents/.skill-lock.json");
    assert!(lock_path.exists(), "Expected lock file at {lock_path:?}");

    let lock_manager = LockManager::new(lock_path);
    let entry = lock_manager.get_entry("cflx-proposal").unwrap();
    assert!(
        entry.is_some(),
        "Lock entry for 'cflx-proposal' should exist"
    );
    let entry = entry.unwrap();
    assert_eq!(entry.source_type, "self");
}

// ---------------------------------------------------------------------------
// Global-scope install tests
// ---------------------------------------------------------------------------

#[test]
fn test_global_scope_install_uses_home_agents_dir_and_updates_lock_file() {
    let workdir = TempDir::new().unwrap();
    let fake_home = TempDir::new().unwrap();

    let _guard = HOME_MUTEX.lock().unwrap();

    let orig_home = std::env::var("HOME").ok();
    unsafe {
        std::env::set_var("HOME", fake_home.path());
    }

    let opts = InstallSkillsOptions {
        global: true,
        project_root: Some(workdir.path().to_path_buf()),
    };
    let result = run_install_skills(opts);

    unsafe {
        match orig_home {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
    }
    drop(_guard);

    result.unwrap();

    // Embedded cflx-proposal skill must be installed to the global directory.
    let skill_path = fake_home.path().join(".agents/skills/cflx-proposal");
    assert!(
        skill_path.exists(),
        "Expected global embedded skill at {skill_path:?}"
    );

    let lock_path = fake_home.path().join(".agents/.skill-lock.json");
    assert!(
        lock_path.exists(),
        "Expected global lock file at {lock_path:?}"
    );

    let lock_manager = LockManager::new(lock_path);
    let entry = lock_manager.get_entry("cflx-proposal").unwrap();
    assert!(
        entry.is_some(),
        "Global lock entry for 'cflx-proposal' should exist"
    );
}

// ---------------------------------------------------------------------------
// Embedded install tests (no skills/ directory present)
// ---------------------------------------------------------------------------

/// Verify that `run_install_skills` succeeds in a directory with no `skills/` subdirectory
/// by falling back to the skills embedded at compile time.
#[test]
fn test_embedded_install_without_skills_dir() {
    // workdir has NO skills/ directory — forces embedded path
    let workdir = TempDir::new().unwrap();
    assert!(
        !workdir.path().join("skills").exists(),
        "Precondition: skills/ must not exist"
    );

    let opts = InstallSkillsOptions {
        global: false,
        project_root: Some(workdir.path().to_path_buf()),
    };
    run_install_skills(opts).unwrap();

    let skills_base = workdir.path().join(".agents/skills");
    let lock_path = workdir.path().join(".agents/.skill-lock.json");

    assert!(lock_path.exists(), "Lock file must be created");

    let lock_manager = LockManager::new(lock_path);

    // All three bundled skills must be installed
    for name in &["cflx-proposal", "cflx-workflow", "cflx-run"] {
        let skill_dir = skills_base.join(name);
        assert!(
            skill_dir.exists(),
            "Expected embedded skill directory for {name} at {skill_dir:?}"
        );
        assert!(
            skill_dir.join("SKILL.md").exists(),
            "{name}: SKILL.md must exist"
        );

        let entry = lock_manager.get_entry(name).unwrap();
        assert!(entry.is_some(), "Lock entry for '{name}' must exist");
        assert_eq!(
            entry.unwrap().source_type,
            "self",
            "{name} lock entry source_type must be 'self'"
        );
    }

    // Verify auxiliary files are present for each skill
    assert!(
        skills_base.join("cflx-proposal/scripts/cflx.py").exists(),
        "cflx-proposal must have scripts/cflx.py"
    );
    assert!(
        skills_base.join("cflx-workflow/scripts/cflx.py").exists(),
        "cflx-workflow must have scripts/cflx.py"
    );
    assert!(
        skills_base
            .join("cflx-workflow/references/cflx-accept.md")
            .exists(),
        "cflx-workflow must have references/cflx-accept.md"
    );
    assert!(
        skills_base
            .join("cflx-workflow/references/cflx-apply.md")
            .exists(),
        "cflx-workflow must have references/cflx-apply.md"
    );
    assert!(
        skills_base
            .join("cflx-workflow/references/cflx-archive.md")
            .exists(),
        "cflx-workflow must have references/cflx-archive.md"
    );
    assert!(
        skills_base.join("cflx-run/references/cflx-run.md").exists(),
        "cflx-run must have references/cflx-run.md"
    );
}

// ---------------------------------------------------------------------------
// Regression test: embedded skills win even when a local skills/ directory exists
// ---------------------------------------------------------------------------

/// Verify that `run_install_skills` always installs embedded skills even when a
/// local `skills/` directory exists at the project root.
#[test]
fn test_embedded_wins_when_local_skills_dir_exists() {
    let workdir = TempDir::new().unwrap();
    // Create a local skills/ directory with a synthetic skill.
    create_test_skills_dir(&workdir);
    assert!(
        workdir.path().join("skills").exists(),
        "Precondition: skills/ must exist"
    );

    let opts = InstallSkillsOptions {
        global: false,
        project_root: Some(workdir.path().to_path_buf()),
    };
    run_install_skills(opts).unwrap();

    let skills_base = workdir.path().join(".agents/skills");
    let lock_path = workdir.path().join(".agents/.skill-lock.json");
    let lock_manager = LockManager::new(lock_path);

    // Embedded cflx-proposal must be installed (not the local test-skill).
    let cflx_proposal_dir = skills_base.join("cflx-proposal");
    assert!(
        cflx_proposal_dir.exists(),
        "Embedded cflx-proposal must be installed even when skills/ dir exists"
    );

    let entry = lock_manager.get_entry("cflx-proposal").unwrap();
    assert!(entry.is_some(), "Lock entry for cflx-proposal must exist");
    assert_eq!(
        entry.unwrap().source_type,
        "self",
        "cflx-proposal must have source_type 'self', not 'local'"
    );

    // The local test-skill must NOT be installed.
    let test_skill_dir = skills_base.join("test-skill");
    assert!(
        !test_skill_dir.exists(),
        "Local test-skill must NOT be installed when embedded skills are available"
    );
}
