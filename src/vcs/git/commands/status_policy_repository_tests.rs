//! Repository-scoped regressions for the read-only `git status` policy.
//!
//! These run real `git` subprocesses against throwaway repositories and read
//! this crate's own sources, so they are integration-scoped rather than
//! unit-scoped. The command-shape unit coverage lives beside the policy itself.

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

use crate::vcs::git::commands::status_policy::{
    read_only_status_argv, NO_OPTIONAL_LOCKS, PORCELAIN_STATUS_ARGS, STATUS_SUBCOMMAND,
};

// ── Fixture helpers ────────────────────────────────────────────────────────

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "cflx")
        .env("GIT_AUTHOR_EMAIL", "cflx@example.com")
        .env("GIT_COMMITTER_NAME", "cflx")
        .env("GIT_COMMITTER_EMAIL", "cflx@example.com")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("fixture path has a parent")).unwrap();
    std::fs::write(path, contents).unwrap();
}

/// A committed repository with one ignored directory configured.
fn baseline_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "commit.gpgsign", "false"]);
    write(root, ".gitignore", "generated/\n");
    write(root, "keep.txt", "kept\n");
    write(root, "unstaged.txt", "base\n");
    write(root, "deleted.txt", "doomed\n");
    write(
        root,
        "old.txt",
        "renamed content that is long enough to match\n",
    );
    write(root, "stale.txt", "stat cache subject\n");
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "baseline"]);
    dir
}

/// Backdate one content-identical file so an ordinary `git status` has a reason
/// to rewrite the index, and return the index bytes in that stale state.
fn make_index_stat_cache_stale(root: &Path) -> Vec<u8> {
    let stale_time = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    std::fs::File::options()
        .write(true)
        .open(root.join("stale.txt"))
        .expect("open fixture file")
        .set_times(std::fs::FileTimes::new().set_modified(stale_time))
        .expect("backdate fixture mtime");
    std::fs::read(root.join(".git/index")).expect("read index")
}

fn index_bytes(root: &Path) -> Vec<u8> {
    std::fs::read(root.join(".git/index")).expect("read index")
}

fn restore_index(root: &Path, bytes: &[u8]) {
    std::fs::write(root.join(".git/index"), bytes).expect("restore stale index");
}

// ── Index-byte safety across production status paths ───────────────────────

/// The positive control first proves the fixture can detect an optional index
/// refresh; without it every assertion below would be vacuous. Then each
/// representative production path — shared helper, direct execution adapter,
/// and upstream adapter — observes the same restored stale index and must both
/// report current state and leave the complete index bytes untouched.
#[tokio::test]
async fn production_status_paths_do_not_persist_an_optional_index_refresh() {
    let dir = baseline_repo();
    let root = dir.path();

    // Current worktree state every observation below must still see. The size
    // changes too, so detection cannot depend on mtime granularity alone.
    write(root, "unstaged.txt", "changed content\n");

    let stale_index = make_index_stat_cache_stale(root);

    // Positive control: a normal `git status` demonstrably rewrites the index.
    git(root, &["status", "--porcelain"]);
    assert_ne!(
        stale_index,
        index_bytes(root),
        "fixture cannot detect an optional index refresh; every assertion below would be vacuous"
    );

    // Shared trimmed dirty check.
    restore_index(root, &stale_index);
    let (has_changes, status) = crate::vcs::git::commands::has_uncommitted_changes(root)
        .await
        .unwrap();
    assert!(has_changes, "the modified file must still be observed");
    assert!(status.contains("unstaged.txt"), "{status:?}");
    assert_eq!(
        stale_index,
        index_bytes(root),
        "has_uncommitted_changes must leave the complete index bytes unchanged"
    );

    // Shared untrimmed porcelain read (the Apply finalization stage gate).
    restore_index(root, &stale_index);
    let porcelain = crate::vcs::git::commands::porcelain_status(root)
        .await
        .unwrap();
    assert!(porcelain.starts_with(" M unstaged.txt"), "{porcelain:?}");
    assert_eq!(
        stale_index,
        index_bytes(root),
        "porcelain_status must leave the complete index bytes unchanged"
    );

    // Shared human-readable capture used as conflict-resolution context.
    restore_index(root, &stale_index);
    let human = crate::vcs::git::commands::get_status(root).await.unwrap();
    assert!(
        human.contains("unstaged.txt") && human.contains("modified:"),
        "the resolve prompt still needs human-readable status text: {human:?}"
    );
    assert_eq!(
        stale_index,
        index_bytes(root),
        "get_status must leave the complete index bytes unchanged"
    );

    // Direct execution adapter: Archive phase classification.
    restore_index(root, &stale_index);
    assert!(
        !crate::execution::archive::is_archive_commit_complete("change-a", Some(root))
            .await
            .unwrap(),
        "a dirty worktree is still not a completed archive commit"
    );
    assert_eq!(
        stale_index,
        index_bytes(root),
        "archive classification must leave the complete index bytes unchanged"
    );

    // Upstream adapter cleanliness and porcelain-v2 observations.
    restore_index(root, &stale_index);
    let ops = crate::upstream::git_ops::GitUpstreamOps::new(root);
    {
        use crate::upstream::ports::UpstreamGit;
        assert!(
            !ops.is_working_tree_clean().await.unwrap(),
            "the upstream adapter must still observe the dirty worktree"
        );
        let v2 = ops.status_porcelain_v2().await.unwrap();
        assert!(
            v2.lines().any(|line| line.starts_with("1 ")),
            "porcelain v2 must stay v2: {v2:?}"
        );
    }
    assert_eq!(
        stale_index,
        index_bytes(root),
        "upstream status observations must leave the complete index bytes unchanged"
    );
}

// ── Per-helper command shape ───────────────────────────────────────────────

/// The exact command line a shared helper issued, taken from the error it
/// reports when Git fails.
///
/// Outside a repository every read-only status observation fails, and the
/// preserved `command` field is the argv the helper actually ran — which is the
/// only way to assert one helper's command shape without spawning a fake `git`.
fn issued_command(error: crate::vcs::VcsError) -> String {
    match error {
        crate::vcs::VcsError::Command { command, .. } => {
            command.expect("a failed Git command preserves its command line")
        }
        other => panic!("expected a Git command failure, got {other:?}"),
    }
}

/// Every shared helper issues its own exact read-only status command, with the
/// global option before the subcommand and its own arguments intact.
#[tokio::test]
async fn every_shared_helper_issues_the_exact_read_only_status_command() {
    // Not a Git repository, so each observation fails and reports its argv.
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    let observed: Vec<(&str, String)> = vec![
        (
            "has_uncommitted_changes",
            issued_command(
                crate::vcs::git::commands::has_uncommitted_changes(root)
                    .await
                    .expect_err("outside a repository this must fail"),
            ),
        ),
        (
            "porcelain_status",
            issued_command(
                crate::vcs::git::commands::porcelain_status(root)
                    .await
                    .expect_err("outside a repository this must fail"),
            ),
        ),
        (
            "get_status",
            issued_command(
                crate::vcs::git::commands::get_status(root)
                    .await
                    .expect_err("outside a repository this must fail"),
            ),
        ),
        (
            "is_working_directory_clean",
            issued_command(
                crate::vcs::git::commands::basic::is_working_directory_clean(root)
                    .await
                    .expect_err("outside a repository this must fail"),
            ),
        ),
        (
            "has_changes_to_commit",
            issued_command(
                crate::vcs::git::commands::commit::has_changes_to_commit(root)
                    .await
                    .expect_err("outside a repository this must fail"),
            ),
        ),
        (
            "is_clean_including_untracked",
            issued_command(
                crate::vcs::git::commands::is_clean_including_untracked(root)
                    .await
                    .expect_err("outside a repository this must fail"),
            ),
        ),
        (
            "list_changes_with_uncommitted_files",
            issued_command(
                crate::vcs::git::commands::list_changes_with_uncommitted_files(root)
                    .await
                    .expect_err("outside a repository this must fail"),
            ),
        ),
    ];

    let expected = [
        (
            "has_uncommitted_changes",
            "git --no-optional-locks status --porcelain --untracked-files=normal --ignored=no",
        ),
        (
            "porcelain_status",
            "git --no-optional-locks status --porcelain --untracked-files=normal --ignored=no",
        ),
        ("get_status", "git --no-optional-locks status"),
        (
            "is_working_directory_clean",
            "git --no-optional-locks status --porcelain",
        ),
        (
            "has_changes_to_commit",
            "git --no-optional-locks status --porcelain",
        ),
        (
            "is_clean_including_untracked",
            "git --no-optional-locks status --porcelain --untracked-files=normal",
        ),
        (
            "list_changes_with_uncommitted_files",
            "git --no-optional-locks status --porcelain -u",
        ),
    ];

    for ((name, issued), (expected_name, expected_command)) in observed.iter().zip(expected) {
        assert_eq!(name, &expected_name);
        assert_eq!(
            issued, expected_command,
            "{name} issued the wrong read-only status command"
        );
    }
}

// ── Classification fidelity ────────────────────────────────────────────────

/// Staged, unstaged, deleted, renamed, untracked, and ignored states keep the
/// classifications their callers already depend on, and the untrimmed reader
/// keeps both status columns.
#[tokio::test]
async fn production_status_paths_preserve_every_state_classification() {
    let dir = baseline_repo();
    let root = dir.path();

    // With only an unstaged modification present, it is the first line, which is
    // where a trimming reader would silently drop the leading worktree column.
    write(root, "unstaged.txt", "changed content\n");
    let first_only = crate::vcs::git::commands::porcelain_status(root)
        .await
        .unwrap();
    assert!(
        first_only.starts_with(" M unstaged.txt"),
        "the untrimmed reader must keep both status columns on the first line: {first_only:?}"
    );

    write(root, "staged.txt", "new\n");
    git(root, &["add", "staged.txt"]);
    std::fs::remove_file(root.join("deleted.txt")).unwrap();
    git(root, &["mv", "old.txt", "new.txt"]);
    write(root, "untracked.txt", "stray\n");
    write(root, "generated/artifact.txt", "generated\n");

    let porcelain = crate::vcs::git::commands::porcelain_status(root)
        .await
        .unwrap();
    let line_for = |suffix: &str| {
        porcelain
            .lines()
            .find(|line| line.ends_with(suffix))
            .unwrap_or_else(|| panic!("no status line for {suffix}: {porcelain:?}"))
            .to_string()
    };

    assert_eq!(&line_for("staged.txt")[..3], "A  ", "{porcelain:?}");
    assert_eq!(&line_for("unstaged.txt")[..3], " M ", "{porcelain:?}");
    assert_eq!(&line_for("deleted.txt")[..3], " D ", "{porcelain:?}");
    assert_eq!(&line_for("untracked.txt")[..3], "?? ", "{porcelain:?}");
    assert!(
        line_for("old.txt -> new.txt").starts_with("R  "),
        "{porcelain:?}"
    );
    assert!(
        !porcelain.contains("generated/"),
        "`--ignored=no` must keep generated content out: {porcelain:?}"
    );
    assert!(
        !porcelain.contains("keep.txt"),
        "a clean committed path is not an uncommitted change: {porcelain:?}"
    );

    let (has_changes, trimmed) = crate::vcs::git::commands::has_uncommitted_changes(root)
        .await
        .unwrap();
    assert!(has_changes);
    assert!(!trimmed.starts_with(' '), "this reader trims on purpose");
    assert!(
        !crate::vcs::git::commands::basic::is_working_directory_clean(root)
            .await
            .unwrap()
    );
    assert!(
        !crate::vcs::git::commands::is_clean_including_untracked(root)
            .await
            .unwrap()
    );
}

/// A conflicted index keeps its `UU` classification and its dirty verdict.
#[tokio::test]
async fn production_status_paths_preserve_conflicted_classification() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "commit.gpgsign", "false"]);
    write(root, "conflict.txt", "base\n");
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "base"]);
    git(root, &["checkout", "-q", "-b", "side"]);
    write(root, "conflict.txt", "side\n");
    git(root, &["commit", "-q", "-am", "side"]);
    git(root, &["checkout", "-q", "main"]);
    write(root, "conflict.txt", "main\n");
    git(root, &["commit", "-q", "-am", "main"]);

    // The merge is expected to fail; only the resulting conflicted index matters.
    let merged = Command::new("git")
        .args(["merge", "--no-edit", "side"])
        .current_dir(root)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .unwrap();
    assert!(!merged.status.success(), "the fixture merge must conflict");

    let porcelain = crate::vcs::git::commands::porcelain_status(root)
        .await
        .unwrap();
    assert!(
        porcelain.contains("UU conflict.txt"),
        "the conflicted classification must survive: {porcelain:?}"
    );
    assert!(
        !crate::vcs::git::commands::basic::is_working_directory_clean(root)
            .await
            .unwrap()
    );
}

// ── Production command inventory ───────────────────────────────────────────

/// Remove `#[cfg(test)]` items so the inventory sees production code only.
///
/// Test fixtures legitimately run plain `git status` — the positive control in
/// this very file does — so scanning them would make the invariant unstatable.
fn strip_cfg_test_items(source: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let mut kept: Vec<&str> = Vec::with_capacity(lines.len());
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        if line.trim() != "#[cfg(test)]" {
            kept.push(line);
            index += 1;
            continue;
        }

        let indent = line.len() - line.trim_start().len();
        // Skip any further attributes or doc comments on the same item.
        let mut item = index + 1;
        while item < lines.len() {
            let trimmed = lines[item].trim_start();
            if trimmed.starts_with("#[") || trimmed.starts_with("//") {
                item += 1;
            } else {
                break;
            }
        }

        if item < lines.len() && lines[item].trim_end().ends_with('{') {
            // A block item: drop through its closing brace, which rustfmt puts
            // back at the item's own indentation.
            let closing = format!("{}}}", " ".repeat(indent));
            let mut end = item + 1;
            while end < lines.len() && lines[end].trim_end() != closing {
                end += 1;
            }
            index = end + 1;
        } else {
            // A single-line item: a `mod x;` declaration or a struct field.
            index = item + 1;
        }
    }
    kept.join("\n")
}

/// Every production Rust source in this crate, with `#[cfg(test)]` removed.
fn production_sources() -> Vec<(PathBuf, String)> {
    fn visit(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
        for entry in std::fs::read_dir(dir).expect("read source directory") {
            let path = entry.expect("read source entry").path();
            if path.is_dir() {
                // Whole directories of test modules are not production code.
                if path.file_name().and_then(|n| n.to_str()) == Some("tests") {
                    continue;
                }
                visit(&path, out);
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.ends_with(".rs") || name.ends_with("_tests.rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("read source file");
            out.push((path.clone(), strip_cfg_test_items(&source)));
        }
    }

    let mut sources = Vec::new();
    visit(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut sources,
    );
    assert!(
        sources.len() > 50,
        "the production corpus looks truncated: {} files",
        sources.len()
    );
    sources
}

/// One Git argv literal found in source text, as its element strings.
fn argv_literals_containing(source: &str, needle: &str) -> Vec<Vec<String>> {
    let bytes = source.as_bytes();
    let quoted = format!("\"{needle}\"");
    let mut found = Vec::new();
    let mut search = 0;

    while let Some(offset) = source[search..].find(&quoted) {
        let start = search + offset;
        let end = start + quoted.len();
        search = end;

        // Argv shape: the literal is an element of a slice or array literal.
        let before = source[..start].trim_end().chars().next_back();
        let after = source[end..].trim_start().chars().next();
        if !matches!(before, Some('[') | Some(',')) || !matches!(after, Some(',') | Some(']')) {
            continue;
        }

        // Walk back to the opening bracket of the enclosing literal, then
        // forward to its match, and split the elements it holds. A call
        // argument list or a statement boundary means this is not argv at all.
        let mut depth = 0usize;
        let mut parens = 0usize;
        let mut open = None;
        for position in (0..start).rev() {
            match bytes[position] {
                b']' => depth += 1,
                b'[' if depth == 0 => {
                    open = Some(position);
                    break;
                }
                b'[' => depth -= 1,
                b')' if depth == 0 => parens += 1,
                b'(' if depth == 0 && parens > 0 => parens -= 1,
                b'(' | b';' | b'{' | b'}' if depth == 0 => break,
                _ => {}
            }
        }
        let Some(open) = open else { continue };

        let mut depth = 0usize;
        let mut close = None;
        for (position, byte) in bytes.iter().enumerate().skip(open + 1) {
            match byte {
                b'[' => depth += 1,
                b']' if depth == 0 => {
                    close = Some(position);
                    break;
                }
                b']' => depth -= 1,
                _ => {}
            }
        }
        let Some(close) = close else { continue };

        found.push(
            source[open + 1..close]
                .split(',')
                .map(|element| element.trim().trim_matches('"').to_string())
                .filter(|element| !element.is_empty())
                .collect(),
        );
    }

    found
}

/// The detector itself must reject the two ways this invariant can be broken.
#[test]
fn inventory_detector_rejects_omitted_and_misplaced_global_options() {
    let omitted = argv_literals_containing(r#"run_git(&["status", "--porcelain"], cwd)"#, "status");
    assert_eq!(omitted, vec![vec!["status", "--porcelain"]]);
    assert!(
        !omitted[0].starts_with(&[NO_OPTIONAL_LOCKS.to_string()]),
        "an omitted global option must be reported"
    );

    let misplaced = argv_literals_containing(
        r#"run_git(&["status", "--no-optional-locks"], cwd)"#,
        "status",
    );
    assert_eq!(misplaced, vec![vec!["status", "--no-optional-locks"]]);
    assert_ne!(
        misplaced[0][0], NO_OPTIONAL_LOCKS,
        "a global option after the subcommand must be reported"
    );

    // Policy use and non-argv `status` text must not be reported at all.
    assert!(argv_literals_containing(
        r#"run_git(&read_only_status_argv(PORCELAIN_STATUS_ARGS), cwd)"#,
        "status",
    )
    .is_empty());
    assert!(argv_literals_containing(r#"value.get("status")"#, "status").is_empty());
    assert!(argv_literals_containing(r#"json!({"status": "ok"})"#, "status").is_empty());
    assert!(
        argv_literals_containing(
            r#"build_prompt(&["rev1"], "err", "status", "log")"#,
            "status"
        )
        .is_empty(),
        "a plain call argument that happens to be the word `status` is not argv"
    );
}

/// The stripper must remove test modules without eating production code.
#[test]
fn inventory_stripper_removes_only_test_items() {
    let source = "pub fn keep() {}\n\
                  #[cfg(test)]\n\
                  mod tests {\n\
                      fn drop_me() {}\n\
                  }\n\
                  pub fn also_keep() {}\n\
                  #[cfg(test)]\n\
                  mod declared;\n\
                  pub fn last() {}\n";
    let stripped = strip_cfg_test_items(source);
    assert!(stripped.contains("keep()"));
    assert!(stripped.contains("also_keep()"));
    assert!(stripped.contains("last()"));
    assert!(!stripped.contains("drop_me"));
    assert!(!stripped.contains("mod declared"));
}

/// No production Git argv builder may construct a read-only `git status`
/// without the shared policy, and no mutating command may carry the policy.
#[test]
fn production_status_argv_construction_uses_the_shared_policy() {
    let policy_module = Path::new("status_policy.rs");
    let mut policy_uses = 0usize;
    let mut offenders: Vec<String> = Vec::new();

    for (path, source) in production_sources() {
        policy_uses += source.matches("read_only_status_argv(").count();

        if path.file_name() == Some(policy_module.as_os_str()) {
            continue;
        }

        for argv in argv_literals_containing(&source, STATUS_SUBCOMMAND) {
            let subcommand = argv.iter().position(|arg| arg == STATUS_SUBCOMMAND);
            let option = argv.iter().position(|arg| arg == NO_OPTIONAL_LOCKS);
            match (option, subcommand) {
                (Some(option), Some(subcommand)) if option < subcommand => {}
                _ => offenders.push(format!("{}: {argv:?}", path.display())),
            }
        }

        for argv in argv_literals_containing(&source, NO_OPTIONAL_LOCKS) {
            let option = argv
                .iter()
                .position(|arg| arg == NO_OPTIONAL_LOCKS)
                .expect("the literal contains the option it was found by");
            if argv.get(option + 1).map(String::as_str) != Some(STATUS_SUBCOMMAND) {
                offenders.push(format!(
                    "{}: optional-lock suppression outside a read-only status command: {argv:?}",
                    path.display()
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "native read-only `git status` argv must be built by \
         `crate::vcs::git::commands::status_policy`:\n{}",
        offenders.join("\n")
    );

    // Non-vacuity control: the corpus must still contain the production call
    // sites, so an over-eager stripper cannot make the scan pass by emptiness.
    assert!(
        policy_uses >= 10,
        "expected the production corpus to keep every policy call site, found {policy_uses}"
    );
}

/// Index-mutating commands keep Git's normal lock semantics.
#[test]
fn mutating_command_builders_never_suppress_optional_locks() {
    use crate::vcs::git::commands::commit::{verified_commit_args, VerifiedCommitMode};

    for mode in [VerifiedCommitMode::AddAndCommit, VerifiedCommitMode::Amend] {
        let args = verified_commit_args(mode, "Apply: change-a");
        assert!(
            !args.iter().any(|arg| arg == NO_OPTIONAL_LOCKS),
            "the final Apply commit must retain normal lock behavior: {args:?}"
        );
    }

    // The policy builder itself can only ever produce a status command.
    let argv = read_only_status_argv(PORCELAIN_STATUS_ARGS);
    assert_eq!(argv[1], STATUS_SUBCOMMAND);
}

/// A real mutating command run beside a read-only observation is unaffected:
/// the suppression is child-command-local, not process-wide.
#[tokio::test]
async fn optional_lock_suppression_stays_local_to_the_status_child() {
    let dir = baseline_repo();
    let root = dir.path();

    write(root, "unstaged.txt", "changed\n");
    assert!(
        crate::vcs::git::commands::has_uncommitted_changes(root)
            .await
            .unwrap()
            .0
    );

    // The mutating command that follows must still take and release the index
    // lock normally, and must still commit.
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "authorized mutation"]);
    assert!(
        crate::vcs::git::commands::basic::is_working_directory_clean(root)
            .await
            .unwrap(),
        "the authorized commit must have succeeded"
    );
    assert!(
        std::env::var_os("GIT_OPTIONAL_LOCKS").is_none(),
        "the policy must never set process-wide optional-lock state"
    );
}

// ── Direct execution adapters ──────────────────────────────────────────────

/// Apply/Archive phase classification keeps its outputs and its fail-closed
/// behavior after moving onto the shared policy.
#[tokio::test]
async fn direct_phase_classification_keeps_outputs_and_fails_closed() {
    let dir = baseline_repo();
    let root = dir.path();

    // Archiving state: dirty worktree, no active change directory, and an
    // archive entry for the change.
    write(
        root,
        "openspec/changes/archive/2026-01-01-change-a/proposal.md",
        "archived\n",
    );
    let stale_index = make_index_stat_cache_stale(root);

    assert!(
        crate::execution::state::has_archive_files("change-a", root)
            .await
            .unwrap(),
        "archiving detection must still recognize the dirty-plus-archived shape"
    );
    assert_eq!(
        stale_index,
        index_bytes(root),
        "archiving detection must leave the complete index bytes unchanged"
    );

    // An unreadable index is not evidence of a clean worktree: both direct
    // classifiers must fail rather than degrade to a verdict.
    std::fs::write(root.join(".git/index"), b"not an index").unwrap();
    assert!(
        crate::execution::state::has_archive_files("change-a", root)
            .await
            .is_err(),
        "an unreadable status must fail closed"
    );
    assert!(
        crate::execution::archive::is_archive_commit_complete("change-a", Some(root))
            .await
            .is_err(),
        "an unreadable status must fail closed"
    );
}
