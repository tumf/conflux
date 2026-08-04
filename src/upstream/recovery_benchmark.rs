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
//! `PATH` is process-global and the test harness runs tests on many threads, so
//! the shim is never installed into this process. Each measurement re-executes
//! this same test binary for [`CHILD_TEST`] with the shimmed `PATH` in the
//! child's own environment, which keeps the sibling tests that shell out to Git
//! on the untouched real binary. Measurements are additionally serialized under
//! a mutex so two shims can never be in flight at once. The shim also records
//! only invocations whose working directory is the fixture repository, so
//! nothing else the child runs can pollute the measurement.

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

/// Full test path of the child-side scan re-executed under the shim.
const CHILD_TEST: &str = "upstream::recovery_benchmark::recovery_benchmark_child_scan";
/// Fixture repository the child must scan. Its absence makes the child a no-op.
const REPO_ENV: &str = "CFLX_RECOVERY_BENCHMARK_REPO";
/// Shim log the child truncates between its warm-up and measured passes.
const LOG_ENV: &str = "CFLX_RECOVERY_BENCHMARK_LOG";
/// Prefix the child prints its own scan duration behind.
const ELAPSED_MARKER: &str = "cflx-recovery-benchmark-elapsed-ms=";

/// Only one shimmed measurement runs at a time.
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

/// The measured side of one scan, run in a child process so the shimmed `PATH`
/// stays inside that child.
///
/// Always `#[ignore]`: it is driven by [`measure_recovery_scan`] through
/// `--exact --ignored`, never by an ordinary test run. Without [`REPO_ENV`] it
/// does nothing, so an operator who runs it by hand cannot get a false pass.
#[test]
#[ignore = "driven as a child process by the recovery benchmark"]
fn recovery_benchmark_child_scan() {
    let Some(repo_root) = std::env::var_os(REPO_ENV) else {
        return;
    };
    let repo_root = Path::new(&repo_root);
    let log = PathBuf::from(std::env::var_os(LOG_ENV).expect("shim log path"));

    let scan = || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(ensure_no_unpushed_upstream_recovery(repo_root))
            .expect("a history with no recovery evidence must not refuse startup");
    };

    // A cold child pays for demand-paging this test binary and warming Git's
    // object cache, which would otherwise swamp the reported duration. Discard
    // that pass and the commands it logged, then measure and count the warm one.
    scan();
    std::fs::write(&log, "").expect("reset the shim log between passes");

    let started = Instant::now();
    scan();
    let elapsed = started.elapsed();

    println!("{}{}", ELAPSED_MARKER, elapsed.as_millis());
}

/// Run the production option-less recovery check against `repo_root` in a child
/// process whose `PATH` leads with a recording `git` shim.
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

    let shimmed = match std::env::var_os("PATH") {
        Some(existing) => format!(
            "{}:{}",
            shim_dir.path().display(),
            existing.to_string_lossy()
        ),
        None => shim_dir.path().display().to_string(),
    };

    let guard = SHIM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let output = SyncCommand::new(std::env::current_exe().expect("test binary path"))
        .args([CHILD_TEST, "--exact", "--ignored", "--nocapture"])
        .env("PATH", shimmed)
        .env(REPO_ENV, repo_root)
        .env(LOG_ENV, &log)
        .output()
        .expect("re-execute the test binary for the shimmed scan");
    drop(guard);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "shimmed child scan failed:\n{}\n{}",
        stdout,
        String::from_utf8_lossy(&output.stderr)
    );

    // The child times its own warm pass, so neither process startup nor a cold
    // page cache is charged to the reported duration.
    let elapsed = stdout
        .lines()
        .find_map(|line| line.strip_prefix(ELAPSED_MARKER))
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or_else(|| panic!("child did not report its elapsed time:\n{}", stdout));

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
