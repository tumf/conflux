//! Regression tests for `cflx run --all` exit-on-success behavior, plus
//! cross-process coverage of the repository-scoped orchestration lock.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use conflux::repo_lock::{self, InvocationMode};

/// Minimal `.cflx.jsonc` for testing — no AI commands needed when there are no changes.
const MINIMAL_CONFIG: &str = r#"{
  "apply_command": "echo apply",
  "archive_command": "echo archive",
  "analyze_command": "echo analyze",
  "acceptance_command": ""
}
"#;

/// Set up a temporary project root with:
/// - `.cflx.jsonc`  (minimal config)
/// - `openspec/`    (empty — no pending changes)
fn setup_empty_project(dir: &std::path::Path) {
    fs::write(dir.join(".cflx.jsonc"), MINIMAL_CONFIG).unwrap();
    fs::create_dir_all(dir.join("openspec/changes")).unwrap();
}

fn add_change(dir: &std::path::Path, id: &str) {
    let change_dir = dir.join("openspec/changes").join(id);
    fs::create_dir_all(&change_dir).unwrap();
    fs::write(change_dir.join("proposal.md"), format!("# {id}\n")).unwrap();
    fs::write(change_dir.join("tasks.md"), "- [ ] task\n").unwrap();
}

/// Run `cflx run` from `cwd` and assert it exits within `timeout`.
/// Returns the `ExitStatus`.
fn run_cflx_args_with_timeout(
    cwd: &std::path::Path,
    args: &[&str],
    timeout: Duration,
) -> std::process::ExitStatus {
    let bin = env!("CARGO_BIN_EXE_cflx");

    let mut child = std::process::Command::new(bin)
        .args(args)
        .current_dir(cwd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("failed to spawn cflx");

    let start = Instant::now();

    loop {
        match child.try_wait().expect("failed to check child status") {
            Some(status) => return status,
            None => {
                if start.elapsed() >= timeout {
                    child.kill().ok();
                    panic!(
                        "cflx run did not exit within {:?} — likely hung in post-success wait loop",
                        timeout
                    );
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

fn run_cflx_with_timeout(
    cwd: &std::path::Path,
    extra_args: &[&str],
    timeout: Duration,
) -> std::process::ExitStatus {
    let mut args = vec!["run"];
    args.extend_from_slice(extra_args);
    run_cflx_args_with_timeout(cwd, &args, timeout)
}

// ── Task 3: non-web success path ───────────────────────────────────────────

#[test]
fn test_run_exits_promptly_on_success_no_web() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());

    // With no pending changes the orchestrator returns Ok(()) immediately.
    // The process must exit within 10 seconds without any external signal.
    let status = run_cflx_with_timeout(tmp.path(), &["--all"], Duration::from_secs(10));
    assert!(
        status.success(),
        "cflx run --all should exit with status 0 on success, got: {:?}",
        status
    );
}

#[test]
fn test_run_rejects_bare_target_before_execution() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());

    let bare = run_cflx_with_timeout(tmp.path(), &[], Duration::from_secs(10));
    assert!(!bare.success());
}

#[test]
fn test_tui_entrypoints_reject_push_with_server_consistently_before_tui_init() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());

    let bin = env!("CARGO_BIN_EXE_cflx");
    let endpoint = "http://127.0.0.1:39876";
    let bare = Command::new(bin)
        .args(["--push", "--server", endpoint])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let explicit = Command::new(bin)
        .args(["tui", "--push", "--server", endpoint])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    assert!(!bare.status.success());
    assert_eq!(bare.status.code(), explicit.status.code());
    assert_eq!(bare.stderr, explicit.stderr);
    assert!(String::from_utf8_lossy(&bare.stderr)
        .contains("--push is not supported with TUI --server mode"));
}

#[test]
fn test_parallel_dry_run_uses_explicit_targets() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());
    add_change(tmp.path(), "a");
    add_change(tmp.path(), "c");
    Command::new("git")
        .arg("init")
        .current_dir(tmp.path())
        .output()
        .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_cflx"))
        .args(["run", "a", "--parallel", "--dry-run"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Total changes: 1"));
    assert!(stdout.contains("  - a"));
    assert!(!stdout.contains("  - c"));
}

#[test]
fn test_run_stdout_keeps_info_and_suppresses_debug_trace_noise() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());
    add_change(tmp.path(), "a");
    Command::new("git")
        .arg("init")
        .current_dir(tmp.path())
        .output()
        .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_cflx"))
        .args(["run", "a", "--parallel", "--dry-run"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Starting cflx "), "stdout={stdout}");
    assert!(stdout.contains("Total changes: 1"), "stdout={stdout}");
    assert!(
        !stdout.contains("DEBUG Executing git command"),
        "stdout={stdout}"
    );
    assert!(
        !stdout.contains("TRACE registering event source with poller"),
        "stdout={stdout}"
    );
    assert!(
        !stdout.contains("TRACE deregistering event source from poller"),
        "stdout={stdout}"
    );
}

// ── Task 4: --web success path ─────────────────────────────────────────────

#[test]
#[cfg(feature = "web-monitoring")]
fn test_run_exits_promptly_on_success_with_web() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());

    // Use port 0 so the OS auto-assigns an available port (no conflicts).
    let status = run_cflx_with_timeout(
        tmp.path(),
        &["--all", "--web", "--web-port", "0"],
        Duration::from_secs(15),
    );
    assert!(
        status.success(),
        "cflx run --web should exit with status 0 on success, got: {:?}",
        status
    );
}

// ── Repository-scoped orchestration lock ───────────────────────────────────
//
// The owner side of these tests takes the lock through the same library entry
// point the binary uses, and every competing side is a real `cflx` process, so
// exclusion is proven across process boundaries rather than in one address
// space.

const CONFLICT_HEADLINE: &str = "already orchestrating this repository";

fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("failed to run git {args:?}: {e}"));
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Create a git repository with an identity, so commits and worktrees work.
fn init_git_repo(dir: &Path) {
    git(dir, &["init", "--quiet"]);
    git(dir, &["config", "user.email", "test@example.com"]);
    git(dir, &["config", "user.name", "Conflux Test"]);
}

fn cflx_output(cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cflx"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("failed to run cflx")
}

fn take_repository_lock(workspace: &Path, mode: InvocationMode) -> repo_lock::RepositoryLock {
    repo_lock::acquire(workspace, mode)
        .unwrap_or_else(|e| panic!("owner failed to acquire repository lock: {e}"))
        .expect("test workspace must be inside a git repository")
}

fn assert_rejected_as_lock_conflict(output: &std::process::Output, context: &str) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !output.status.success(),
        "{context}: expected non-zero exit, got {:?} (stderr={stderr})",
        output.status
    );
    assert!(
        stderr.contains(CONFLICT_HEADLINE),
        "{context}: expected a lock conflict diagnostic, got stderr={stderr}"
    );
    // Logging, listeners, and orchestration all start after lock acquisition,
    // so a rejected invocation must not have announced a startup.
    assert!(
        !stdout.contains("Starting cflx"),
        "{context}: orchestration startup must not run, got stdout={stdout}"
    );
    stderr
}

#[test]
fn competing_run_in_the_same_repository_is_rejected_with_owner_diagnostics() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());
    init_git_repo(tmp.path());

    let owner = take_repository_lock(tmp.path(), InvocationMode::Run);
    let workspace = fs::canonicalize(tmp.path()).unwrap();

    let output = cflx_output(tmp.path(), &["run", "--all"]);
    let stderr = assert_rejected_as_lock_conflict(&output, "competing run");

    assert!(
        stderr.contains(&std::process::id().to_string()),
        "expected the owner pid in the diagnostic, got stderr={stderr}"
    );
    assert!(
        stderr.contains("mode:       run"),
        "expected the owner invocation mode, got stderr={stderr}"
    );
    assert!(
        stderr.contains(&workspace.display().to_string()),
        "expected the owner workspace, got stderr={stderr}"
    );
    assert!(
        stderr.contains("started at:"),
        "expected the owner start time, got stderr={stderr}"
    );
    // No listener has bound, so no endpoint may be claimed.
    assert!(
        !stderr.contains("api url"),
        "diagnostic must not claim an API URL before bind, got stderr={stderr}"
    );

    drop(owner);
}

#[test]
fn every_local_orchestration_entrypoint_is_guarded() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());
    init_git_repo(tmp.path());
    let data_dir = tmp.path().join("server-data");

    let owner = take_repository_lock(tmp.path(), InvocationMode::Tui);

    // Default TUI (no subcommand), explicit local TUI, run, and server.
    let guarded: [Vec<&str>; 4] = [
        vec![],
        vec!["tui"],
        vec!["run", "--all"],
        vec![
            "server",
            "--bind",
            "127.0.0.1",
            "--port",
            "0",
            "--data-dir",
            data_dir.to_str().unwrap(),
        ],
    ];
    for args in guarded {
        let output = cflx_output(tmp.path(), &args);
        assert_rejected_as_lock_conflict(&output, &format!("cflx {}", args.join(" ")));
    }
    assert!(
        !data_dir.exists(),
        "server must not create its data directory before winning the lock"
    );

    drop(owner);
}

#[test]
fn non_orchestration_commands_and_remote_tui_bypass_the_lock() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());
    init_git_repo(tmp.path());

    let owner = take_repository_lock(tmp.path(), InvocationMode::Run);

    for args in [vec!["completion", "bash"], vec!["openspec", "list"]] {
        let output = cflx_output(tmp.path(), &args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "cflx {} must stay usable while another process owns the lock (stderr={stderr})",
            args.join(" ")
        );
        assert!(
            !stderr.contains(CONFLICT_HEADLINE),
            "cflx {} must not attempt the orchestration lock (stderr={stderr})",
            args.join(" ")
        );
    }

    // Remote-client TUI owns no local orchestration: it must reach its own
    // argument validation instead of being rejected by the local lock.
    let remote = cflx_output(
        tmp.path(),
        &["tui", "--push", "--server", "http://127.0.0.1:39876"],
    );
    let remote_stderr = String::from_utf8_lossy(&remote.stderr);
    assert!(
        remote_stderr.contains("--push is not supported with TUI --server mode"),
        "remote TUI must bypass the lock and reach TUI validation, got stderr={remote_stderr}"
    );
    assert!(
        !remote_stderr.contains(CONFLICT_HEADLINE),
        "remote TUI must not acquire the local orchestration lock, got stderr={remote_stderr}"
    );

    drop(owner);
}

#[test]
fn linked_worktrees_share_one_repository_lock() {
    let tmp = tempfile::tempdir().unwrap();
    let main = tmp.path().join("main");
    let linked = tmp.path().join("linked");
    fs::create_dir_all(&main).unwrap();
    setup_empty_project(&main);
    init_git_repo(&main);
    git(&main, &["commit", "--allow-empty", "--quiet", "-m", "init"]);
    git(
        &main,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "linked",
            linked.to_str().unwrap(),
        ],
    );
    setup_empty_project(&linked);

    let owner = take_repository_lock(&main, InvocationMode::Run);
    assert_eq!(
        repo_lock::discover_common_dir(&linked),
        repo_lock::discover_common_dir(&main),
        "linked worktrees must resolve to one canonical git common directory"
    );

    let output = cflx_output(&linked, &["run", "--all"]);
    let stderr = assert_rejected_as_lock_conflict(&output, "run from linked worktree");
    assert!(
        stderr.contains(&fs::canonicalize(&main).unwrap().display().to_string()),
        "diagnostic must identify the owning worktree, got stderr={stderr}"
    );

    drop(owner);
}

#[test]
fn distinct_repositories_run_concurrently() {
    let tmp = tempfile::tempdir().unwrap();
    let first = tmp.path().join("first");
    let second = tmp.path().join("second");
    for repo in [&first, &second] {
        fs::create_dir_all(repo).unwrap();
        setup_empty_project(repo);
        init_git_repo(repo);
    }
    assert_ne!(
        repo_lock::discover_common_dir(&first),
        repo_lock::discover_common_dir(&second)
    );

    let owner = take_repository_lock(&first, InvocationMode::Run);

    // The unrelated repository is unaffected...
    let concurrent = cflx_output(&second, &["run", "--all", "--dry-run"]);
    assert!(
        concurrent.status.success(),
        "an unrelated repository must run concurrently, stderr={}",
        String::from_utf8_lossy(&concurrent.stderr)
    );
    // ...while the owned one still rejects a competitor, proving the guard is
    // active rather than globally disabled.
    let blocked = cflx_output(&first, &["run", "--all", "--dry-run"]);
    assert_rejected_as_lock_conflict(&blocked, "run in owned repository");

    drop(owner);
}

#[test]
fn released_lock_leaves_stale_metadata_that_never_blocks_startup() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());
    init_git_repo(tmp.path());

    let owner = take_repository_lock(tmp.path(), InvocationMode::Run);
    let owner_file = owner.common_dir().join(repo_lock::OWNER_FILE_NAME);
    let previous = repo_lock::read_owner_metadata(&owner_file).expect("owner metadata");
    assert_eq!(previous.pid, Some(std::process::id()));

    // Normal termination: closing the descriptor releases the OS lock while the
    // diagnostic metadata deliberately stays behind.
    drop(owner);
    assert!(
        owner_file.exists(),
        "metadata must not be deleted on release"
    );

    let output = cflx_output(tmp.path(), &["run", "--all", "--dry-run"]);
    assert!(
        output.status.success(),
        "stale metadata must not block startup, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let replaced = repo_lock::read_owner_metadata(&owner_file).expect("owner metadata");
    assert_ne!(
        replaced.pid, previous.pid,
        "the new owner must replace stale diagnostic metadata"
    );
}

#[test]
fn malformed_metadata_neither_grants_nor_blocks_ownership() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());
    init_git_repo(tmp.path());
    let common_dir = repo_lock::discover_common_dir(tmp.path()).expect("git common dir");
    let owner_file = common_dir.join(repo_lock::OWNER_FILE_NAME);

    // Unreadable metadata with no live lock must not block startup.
    fs::write(&owner_file, "{ not json").unwrap();
    let unlocked = cflx_output(tmp.path(), &["run", "--all", "--dry-run"]);
    assert!(
        unlocked.status.success(),
        "malformed metadata must not block startup, stderr={}",
        String::from_utf8_lossy(&unlocked.stderr)
    );

    // With the lock held, malformed metadata still yields a generic conflict.
    let owner = take_repository_lock(tmp.path(), InvocationMode::Run);
    fs::write(&owner_file, "{ not json").unwrap();
    let locked = cflx_output(tmp.path(), &["run", "--all", "--dry-run"]);
    let stderr = assert_rejected_as_lock_conflict(&locked, "malformed metadata conflict");
    assert!(
        stderr.contains("owner details are unavailable"),
        "expected a generic live-lock conflict, got stderr={stderr}"
    );

    drop(owner);
}

#[test]
fn conflict_reports_a_published_api_url() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());
    init_git_repo(tmp.path());

    let owner = take_repository_lock(tmp.path(), InvocationMode::Server);
    // Stand in for a listener that has just returned its actual bound address.
    owner.publish_api_url("http://127.0.0.1:39931");

    let output = cflx_output(tmp.path(), &["run", "--all"]);
    let stderr = assert_rejected_as_lock_conflict(&output, "conflict with published API URL");
    assert!(
        stderr.contains("api url:    http://127.0.0.1:39931"),
        "expected the published API URL, got stderr={stderr}"
    );

    drop(owner);
}

/// Poll `path` until it contains a URL line, or the timeout expires.
///
/// The daemon writes startup logs to stdout as well, so the URL is matched by
/// prefix rather than by position.
#[cfg(feature = "web-monitoring")]
fn wait_for_url_line(path: &Path, timeout: Duration) -> String {
    let start = Instant::now();
    loop {
        if let Ok(content) = fs::read_to_string(path) {
            if let Some(line) = content.lines().find(|line| line.starts_with("http://")) {
                return line.trim().to_string();
            }
        }
        assert!(
            start.elapsed() < timeout,
            "no URL line appeared in {} within {timeout:?}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// End-to-end proof that the reported endpoint comes from a real bind and that
/// forced termination releases ownership.
#[test]
#[cfg(feature = "web-monitoring")]
fn server_publishes_its_bound_api_url_and_kill_releases_the_lock() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());
    init_git_repo(tmp.path());
    let data_dir = tmp.path().join("server-data");
    let stdout_path = tmp.path().join("server-stdout.txt");
    let stdout_file = fs::File::create(&stdout_path).unwrap();

    // Port 0 forces an OS-assigned port, so a correct diagnostic can only come
    // from the address the listener actually bound.
    let mut server = Command::new(env!("CARGO_BIN_EXE_cflx"))
        .args([
            "server",
            "--bind",
            "127.0.0.1",
            "--port",
            "0",
            "--data-dir",
            data_dir.to_str().unwrap(),
        ])
        .current_dir(tmp.path())
        .stdout(std::process::Stdio::from(stdout_file))
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("failed to spawn cflx server");

    let url = wait_for_url_line(&stdout_path, Duration::from_secs(30));
    assert!(!url.ends_with(":0"), "expected an OS-assigned port: {url}");

    let output = cflx_output(tmp.path(), &["run", "--all"]);
    let stderr = assert_rejected_as_lock_conflict(&output, "conflict with a live server");
    assert!(
        stderr.contains(&url),
        "expected the actual bound API URL {url}, got stderr={stderr}"
    );
    assert!(
        stderr.contains(&server.id().to_string()),
        "expected the server pid, got stderr={stderr}"
    );
    assert!(
        stderr.contains("mode:       server"),
        "expected the server invocation mode, got stderr={stderr}"
    );

    // Abnormal termination releases the lock through file-descriptor semantics.
    server.kill().unwrap();
    server.wait().unwrap();
    let reacquired = repo_lock::acquire(tmp.path(), InvocationMode::Run)
        .expect("lock must be acquirable after the owner is killed");
    assert!(reacquired.is_some());
}
