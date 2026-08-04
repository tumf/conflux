//! Repository-local benchmark for bounded recovery discovery.
//!
//! This is the regression that keeps option-less startup cheap. It observes the
//! *actual* native Git commands the production entry point issues, by putting a
//! recording `git` shim ahead of the real binary on `PATH`, and asserts the
//! no-match recovery scan costs a constant number of subprocesses regardless of
//! how deep the first-parent history is.
//!
//! Two properties are deliberately separated:
//!
//! - The **pass condition is a subprocess count**, which is hardware independent.
//! - The **elapsed time is diagnostic output only**. A wall-clock threshold would
//!   fail on a loaded or slow machine without proving anything the count does not.
//!
//! `PATH` is process-global, so the shim is installed under a mutex and removed
//! before the lock is released. The shim additionally records only invocations
//! whose working directory is the fixture repository, so a concurrently running
//! test that happens to spawn Git cannot pollute the measurement.

use std::path::{Path, PathBuf};
use std::process::Command as SyncCommand;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tempfile::TempDir;

use super::startup::ensure_no_unpushed_upstream_recovery;

/// Short history used as the low end of the comparison.
const SHORT_HISTORY: usize = 5;
/// Deep history at the bounded recovery limit.
const DEEP_HISTORY: usize = 500;

/// `PATH` mutation is process-global; only one shim may be installed at a time.
static SHIM_LOCK: Mutex<()> = Mutex::new(());

fn git(root: &Path, args: &[&str]) {
    let output = SyncCommand::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "cflx")
        .env("GIT_AUTHOR_EMAIL", "cflx@example.com")
        .env("GIT_COMMITTER_NAME", "cflx")
        .env("GIT_COMMITTER_EMAIL", "cflx@example.com")
        .output()
        .unwrap_or_else(|e| panic!("git {:?}: {}", args, e));
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A repository whose first-parent history has `commits` ordinary commits and no
/// recovery evidence at all, so the scan takes the ordinary no-match path.
fn repository_with_history(commits: usize) -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    git(root, &["init", "-q", "-b", "main"]);
    std::fs::write(root.join("README.md"), "root\n").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "root"]);
    for n in 1..commits {
        git(
            root,
            &[
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                &format!("ordinary work {}", n),
            ],
        );
    }
    dir
}

/// Absolute path of the real `git` binary, resolved before the shim is installed.
fn real_git() -> PathBuf {
    let output = SyncCommand::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .expect("resolve git");
    assert!(output.status.success(), "git is not on PATH");
    PathBuf::from(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// One measured no-match recovery scan: the Git commands it issued, and how long
/// the whole scan took.
struct Measurement {
    commands: Vec<String>,
    elapsed: Duration,
}

impl Measurement {
    fn tree_reads(&self) -> usize {
        self.commands
            .iter()
            .filter(|line| line.split('\t').next() == Some("ls-tree"))
            .count()
    }
}

/// Run the production option-less recovery check against `repo_root` with a
/// recording `git` shim installed for the duration of the scan.
fn measure_recovery_scan(repo_root: &Path) -> Measurement {
    let shim_dir = TempDir::new().unwrap();
    let log = shim_dir.path().join("git-commands.log");
    let shim = shim_dir.path().join("git");
    // The fixture root is compared against the child's resolved working
    // directory, so only this repository's invocations are recorded.
    let canonical_root = std::fs::canonicalize(repo_root).unwrap();

    std::fs::write(
        &shim,
        format!(
            "#!/bin/sh\n\
             if [ \"$(pwd -P)\" = \"{root}\" ]; then\n\
             \tprintf '%s\\t' \"$@\" >> \"{log}\"\n\
             \tprintf '\\n' >> \"{log}\"\n\
             fi\n\
             exec \"{git}\" \"$@\"\n",
            root = canonical_root.display(),
            log = log.display(),
            git = real_git().display(),
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let guard = SHIM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let original_path = std::env::var_os("PATH");
    let shimmed = match &original_path {
        Some(existing) => format!(
            "{}:{}",
            shim_dir.path().display(),
            existing.to_string_lossy()
        ),
        None => shim_dir.path().display().to_string(),
    };
    std::env::set_var("PATH", shimmed);

    let started = Instant::now();
    let outcome = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(ensure_no_unpushed_upstream_recovery(repo_root));
    let elapsed = started.elapsed();

    match original_path {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }
    drop(guard);

    outcome.expect("a history with no recovery evidence must not refuse startup");

    let commands = std::fs::read_to_string(&log)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    Measurement { commands, elapsed }
}

#[test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
fn upstream_integration_recovery_scan_subprocess_count_does_not_grow_with_history() {
    let short = repository_with_history(SHORT_HISTORY);
    let deep = repository_with_history(DEEP_HISTORY);

    let short_scan = measure_recovery_scan(short.path());
    let deep_scan = measure_recovery_scan(deep.path());

    // Machine-readable diagnostic. The elapsed values are context for an
    // operator reading the output; only the counts gate the test.
    println!(
        "cflx-recovery-benchmark commits_short={} commits_deep={} \
         subprocesses_short={} subprocesses_deep={} tree_reads_short={} tree_reads_deep={} \
         elapsed_ms_short={} elapsed_ms_deep={}",
        SHORT_HISTORY,
        DEEP_HISTORY,
        short_scan.commands.len(),
        deep_scan.commands.len(),
        short_scan.tree_reads(),
        deep_scan.tree_reads(),
        short_scan.elapsed.as_millis(),
        deep_scan.elapsed.as_millis(),
    );

    assert!(
        !short_scan.commands.is_empty(),
        "the shim recorded nothing, so the measurement proves nothing"
    );
    assert_eq!(
        short_scan.commands.len(),
        deep_scan.commands.len(),
        "no-match recovery discovery must cost a constant number of Git subprocesses;\n\
         short: {:?}\ndeep: {:?}",
        short_scan.commands,
        deep_scan.commands
    );
    assert_eq!(
        deep_scan.tree_reads(),
        0,
        "recovery discovery must read no commit trees: {:?}",
        deep_scan.commands
    );
}
