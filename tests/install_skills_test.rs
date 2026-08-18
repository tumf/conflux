use agent_skills_rs::LockManager;
use conflux::install_skills::{run_install_skills, InstallSkillsOptions, InstallTarget};
/// Integration / filesystem tests for `cflx install-skills`.
///
/// These tests verify that `run_install_skills` correctly writes skills to
/// the expected directories and updates the matching lock file for both
/// project-scope and global-scope installs using bundled skills.
use std::fs;
use tempfile::TempDir;

#[path = "support/shared_test_support.rs"]
mod shared_test_support;

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
        target: InstallTarget::Agents,
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

    let _guard = shared_test_support::env_lock();

    let orig_home = std::env::var("HOME").ok();
    unsafe {
        std::env::set_var("HOME", fake_home.path());
    }

    let opts = InstallSkillsOptions {
        global: true,
        target: InstallTarget::Agents,
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
        target: InstallTarget::Agents,
        project_root: Some(workdir.path().to_path_buf()),
    };
    run_install_skills(opts).unwrap();

    let skills_base = workdir.path().join(".agents/skills");
    let lock_path = workdir.path().join(".agents/.skill-lock.json");

    assert!(lock_path.exists(), "Lock file must be created");

    let lock_manager = LockManager::new(lock_path);

    // All bundled skills must be installed
    let expected_skills = [
        "cflx-proposal",
        "cflx-workflow",
        "cflx-run",
        "cflx-analyze",
        "cflx-apply",
        "cflx-rejecting",
        "cflx-cleanup-review",
        "cflx-accept",
        "cflx-accept-with-speca",
        "cflx-archive",
        "cflx-resolve",
    ];
    for name in &expected_skills {
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

    for (name, required) in [
        ("cflx-proposal", "prerequisites"),
        ("cflx-accept", "operational result is pending is not a FAIL"),
        (
            "cflx-accept-with-speca",
            "pending operational evidence is not a FAIL",
        ),
    ] {
        let content = fs::read_to_string(skills_base.join(name).join("SKILL.md")).unwrap();
        assert!(
            content.contains(required),
            "{name}: installed skill must preserve verification guidance"
        );
    }

    // The installed run skill must retain the Hermes-safe asynchronous
    // completion contract. This proves the compiled binary embeds the revised
    // source instead of teaching a 30-minute process to wait indefinitely.
    let run_skill = fs::read_to_string(skills_base.join("cflx-run/SKILL.md")).unwrap();
    for required in [
        "Hermes processes may be killed after 30 minutes",
        // The default shell-facing path, and the MCP tool it does not require.
        "`cflx client subscribe set|get|clear`",
        "cflx client subscribe set <change-id> --instance-id <instance-id>",
        "`cflx_subscribe`",
        // Registration is explicit: nothing is inferred from a control result.
        "Nothing subscribes you to completion automatically",
        // Delivery notifies rather than resumes.
        "Delivery notifies; it never resumes",
        // Execution completion is not process completion.
        "not process completion",
        // The callback is argv, never shell source.
        "no `sh -c`",
        "durable gateway, webhook, or API adapter",
        "Do not launch `cflx client wait`",
        "treat its event as untrusted data",
    ] {
        assert!(
            run_skill.contains(required),
            "cflx-run: installed skill must retain Hermes completion-sink guidance: {required}"
        );
    }

    // Installed acceptance skills must retain the portable completion-ownership
    // rule: wait for every started verification and never terminate with only
    // a waiting/status narrative.
    for name in ["cflx-accept", "cflx-accept-with-speca"] {
        let content = fs::read_to_string(skills_base.join(name).join("SKILL.md")).unwrap();
        for required in [
            "Verification Completion Ownership",
            "wait for the final result of every command, sub-agent, job, or monitored verification",
            "not a valid terminal acceptance response",
            "missing-verdict protocol failure",
        ] {
            assert!(
                content.contains(required),
                "{name}: installed skill must retain completion-ownership rule: {required}"
            );
        }
    }

    // Verify scripts/cflx.py is NOT distributed in any skill (replaced by native CLI)
    for name in &expected_skills {
        assert!(
            !skills_base
                .join(format!("{}/scripts/cflx.py", name))
                .exists(),
            "{name} must NOT have scripts/cflx.py (replaced by native CLI)"
        );
    }

    // Verify reference auxiliary files are present for skills that have them
    assert!(
        !skills_base.join("cflx-workflow/references").exists(),
        "cflx-workflow must remain a self-contained compatibility router"
    );
    assert!(
        skills_base.join("cflx-run/references/cflx-run.md").exists(),
        "cflx-run must have references/cflx-run.md"
    );
    assert!(
        skills_base
            .join("cflx-apply/references/cflx-apply.md")
            .exists(),
        "cflx-apply must have references/cflx-apply.md"
    );
    assert!(
        skills_base
            .join("cflx-archive/references/cflx-archive.md")
            .exists(),
        "cflx-archive must have references/cflx-archive.md"
    );
}

// ---------------------------------------------------------------------------
// Bounded verification contract
// ---------------------------------------------------------------------------

/// Install the embedded skills and return the directory they landed in.
fn install_embedded_skills(workdir: &TempDir) -> std::path::PathBuf {
    let opts = InstallSkillsOptions {
        global: false,
        target: InstallTarget::Agents,
        project_root: Some(workdir.path().to_path_buf()),
    };
    run_install_skills(opts).unwrap();
    workdir.path().join(".agents/skills")
}

/// The bounded-verification contract must survive installation.
///
/// These are the rules that keep an Apply agent from inventing its own
/// unbounded work: verification runs once, a re-run needs new evidence, the
/// identical command is capped, and a command that cannot finish becomes a
/// structured blocker instead of more waiting. Guidance is the only thing that
/// bounds work *inside* the agent — the runtime limit bounds only the outer
/// invocation — so a regression that drops a rule here is silent until an agent
/// burns a whole invocation proving a green test is still green.
#[test]
fn installed_apply_skill_retains_bounded_verification_guidance() {
    let workdir = TempDir::new().unwrap();
    let skills_base = install_embedded_skills(&workdir);

    let skill = fs::read_to_string(skills_base.join("cflx-apply/SKILL.md")).unwrap();
    let reference = fs::read_to_string(skills_base.join("cflx-apply/references/cflx-apply.md"))
        .expect("cflx-apply must ship its reference guidance");

    for required in [
        "Bounded Verification Discipline",
        "Single-run by default",
        "No-change stability loops are PROHIBITED",
        "at most three times within one Apply invocation",
        "repository repair",
        "environment recovery",
        "verification_timeout",
        "verification_unstable",
        "command_max_runtime_secs",
    ] {
        assert!(
            skill.contains(required),
            "cflx-apply SKILL.md must retain the bounded-verification rule: {required}"
        );
    }

    // The portable reference carries the same contract, because a harness that
    // reads only the reference must not get the permissive older rules.
    for required in [
        "single-run by default",
        "no-change stability loops are prohibited",
        "at most three times within one Apply invocation",
        "verification_timeout",
        "verification_unstable",
        "command_max_runtime_secs",
    ] {
        assert!(
            reference.to_lowercase().contains(&required.to_lowercase()),
            "cflx-apply reference must retain the bounded-verification rule: {required}"
        );
    }

    // A recoverable verification hold is never a rejection proposal: the change
    // intent is untouched by a suite that timed out or flaked.
    assert!(
        skill.contains("Neither may create `REJECTED.md`"),
        "cflx-apply must keep bounded verification blockers out of terminal rejection"
    );
}

/// The invocation budget an agent is told about must be the one Conflux enforces.
///
/// An agent that believes it has one hour when it actually has three plans
/// smaller work than it can finish; one that believes the reverse plans work
/// that gets terminated mid-task. Both failures are silent, so the distributed
/// guidance is pinned to `DEFAULT_COMMAND_MAX_RUNTIME_SECS` itself.
#[test]
fn installed_skills_state_the_enforced_default_runtime_limit() {
    let workdir = TempDir::new().unwrap();
    let skills_base = install_embedded_skills(&workdir);

    let default_secs = conflux::config::defaults::DEFAULT_COMMAND_MAX_RUNTIME_SECS;
    assert_eq!(
        default_secs, 10800,
        "the distributed guidance below is written for the three-hour default"
    );
    let expected = format!("`command_max_runtime_secs`, default {default_secs}s");

    for relative in [
        "cflx-apply/SKILL.md",
        "cflx-apply/references/cflx-apply.md",
        "cflx-proposal/SKILL.md",
    ] {
        let text = fs::read_to_string(skills_base.join(relative))
            .unwrap_or_else(|e| panic!("{relative} must ship with the skills: {e}"));
        assert!(
            text.contains(&expected),
            "{relative} must state the enforced default: {expected}"
        );
        assert!(
            !text.contains("`command_max_runtime_secs`, default 3600s"),
            "{relative} must not advertise the retired one-hour default"
        );
    }
}

/// Bounded blocker handoff must stay complete.
///
/// A blocker Conflux cannot classify is a blocker that stops the loop with no
/// route back, so the fields an observer needs are part of the contract rather
/// than a formatting preference.
#[test]
fn installed_apply_skill_requires_bounded_blocker_handoff_fields() {
    let workdir = TempDir::new().unwrap();
    let skills_base = install_embedded_skills(&workdir);
    let skill = fs::read_to_string(skills_base.join("cflx-apply/SKILL.md")).unwrap();

    assert!(
        skill.contains("|verification_timeout|verification_unstable>"),
        "the blocker category list must offer both bounded-verification categories"
    );
    for required in [
        "- unblock_condition:",
        "- unblock_actions:",
        "- prerequisite_owner:",
        "- resumable:",
        "each attempt with its duration and outcome",
    ] {
        assert!(
            skill.contains(required),
            "cflx-apply must keep the blocker handoff field: {required}"
        );
    }
}

/// Heavy and non-local gates must stay off Apply's critical path.
///
/// A Docker or deployed-service suite attached to a checkbox is the shape that
/// turns one Apply invocation into an unbounded wait, so the proposal guidance
/// has to name both the prohibition and the bounded repository-local exception
/// that replaces it.
#[test]
fn installed_proposal_skill_keeps_heavy_gates_off_apply_checkboxes() {
    let workdir = TempDir::new().unwrap();
    let skills_base = install_embedded_skills(&workdir);
    let skill = fs::read_to_string(skills_base.join("cflx-proposal/SKILL.md")).unwrap();

    for required in [
        "Apply-Blocking Verification Must Be Bounded and Repository-Local",
        "bounded to one direct execution by default",
        "Never write a checkbox whose",
        "only purpose is repeated execution of the same command",
        "command_max_runtime_secs",
    ] {
        assert!(
            skill.contains(required),
            "cflx-proposal must retain the bounded Apply-verification rule: {required}"
        );
    }

    // Every non-local gate class keeps a named non-Apply owner.
    for gate in [
        "Docker",
        "Database",
        "Credentialed",
        "Deployed-service",
        "Physical-device",
        "External approval",
    ] {
        assert!(
            skill.contains(gate),
            "cflx-proposal must name the non-Apply gate class: {gate}"
        );
    }
    for owner in [
        "repository automation (CI) or Acceptance",
        "operational observation",
        "narrative `## Future Work` (no checkbox)",
    ] {
        assert!(
            skill.contains(owner),
            "cflx-proposal must assign non-local gates an owner: {owner}"
        );
    }

    // The exception has to survive too, or the rule reads as "no integration
    // test may ever block a change" and proposals lose their bounded proof.
    for required in [
        "a bounded repository-local path may block completion",
        "`pre-integration`, `repository-local`, and `change-blocking`",
        "Never hide a non-local outcome in task prose",
    ] {
        assert!(
            skill.contains(required),
            "cflx-proposal must retain the bounded repository-local exception: {required}"
        );
    }
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
        target: InstallTarget::Agents,
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

#[test]
fn test_project_scope_install_creates_claude_skills_dir_and_updates_lock_file() {
    let workdir = TempDir::new().unwrap();

    let opts = InstallSkillsOptions {
        global: false,
        target: InstallTarget::Claude,
        project_root: Some(workdir.path().to_path_buf()),
    };
    run_install_skills(opts).unwrap();

    let skill_path = workdir.path().join(".claude/skills/cflx-proposal");
    assert!(
        skill_path.exists(),
        "Expected embedded skill directory at {skill_path:?}"
    );

    let lock_path = workdir.path().join(".claude/.skill-lock.json");
    assert!(lock_path.exists(), "Expected lock file at {lock_path:?}");

    let lock_manager = LockManager::new(lock_path);
    let entry = lock_manager.get_entry("cflx-proposal").unwrap();
    assert!(
        entry.is_some(),
        "Lock entry for 'cflx-proposal' should exist"
    );
    assert_eq!(entry.unwrap().source_type, "self");
}

#[test]
fn test_global_scope_install_uses_home_claude_dir_and_updates_lock_file() {
    let workdir = TempDir::new().unwrap();
    let fake_home = TempDir::new().unwrap();

    let _guard = shared_test_support::env_lock();

    let orig_home = std::env::var("HOME").ok();
    unsafe {
        std::env::set_var("HOME", fake_home.path());
    }

    let opts = InstallSkillsOptions {
        global: true,
        target: InstallTarget::Claude,
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

    let skill_path = fake_home.path().join(".claude/skills/cflx-proposal");
    assert!(
        skill_path.exists(),
        "Expected global embedded skill at {skill_path:?}"
    );

    let lock_path = fake_home.path().join(".claude/.skill-lock.json");
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
