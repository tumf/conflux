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
    init_git_repo(tmp.path());

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
    init_git_repo(tmp.path());

    let bare = run_cflx_with_timeout(tmp.path(), &[], Duration::from_secs(10));
    assert!(!bare.success());
}

/// The removed multi-project server surfaces must fail as Clap usage errors.
///
/// Rejection has to happen before anything observable: no log file, no
/// repository lock, no listener, no orchestration. The workspace is a real git
/// repository with a real config so the failure cannot be an accident of a
/// missing prerequisite.
#[test]
fn removed_server_surfaces_are_rejected_without_side_effects() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());
    init_git_repo(tmp.path());

    let state_home = tmp.path().join("state");
    let removed: [&[&str]; 10] = [
        &["--server", "http://127.0.0.1:39876"],
        &["--server-token", "mytoken"],
        &["--server-token-env", "CFLX_TEST_TOKEN"],
        &["tui", "--server", "http://127.0.0.1:39876"],
        &["tui", "--server-token", "mytoken"],
        &["tui", "--server-token-env", "CFLX_TEST_TOKEN"],
        &["server"],
        &["service", "install"],
        &["project", "status"],
        &["project", "sync", "--all"],
    ];

    for args in removed {
        let output = Command::new(env!("CARGO_BIN_EXE_cflx"))
            .args(args)
            .current_dir(tmp.path())
            .env("XDG_STATE_HOME", &state_home)
            .output()
            .expect("failed to run cflx");
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let label = format!("cflx {}", args.join(" "));

        assert!(
            !output.status.success(),
            "{label}: removed surface must exit non-zero, got {:?}",
            output.status
        );
        assert!(
            stderr.contains("error:"),
            "{label}: expected a Clap usage error, got stderr={stderr}"
        );
        assert!(
            !stdout.contains("Starting cflx"),
            "{label}: must not announce startup, got stdout={stdout}"
        );
    }

    // Clap rejects before logging is initialized, so no log file was created.
    assert!(
        !state_home.exists(),
        "removed surfaces must be rejected before any log file is written"
    );
    // ...and before the repository lock file exists.
    let common_dir = repo_lock::discover_common_dir(tmp.path()).expect("git common dir");
    assert!(
        !common_dir.join(repo_lock::LOCK_FILE_NAME).exists(),
        "removed surfaces must be rejected before the repository lock is taken"
    );
}

/// The retained local surfaces must still parse and advertise their options.
#[test]
fn retained_local_web_options_still_parse() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());

    for args in [vec!["run", "--help"], vec!["tui", "--help"], vec!["--help"]] {
        let output = cflx_output(tmp.path(), &args);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "cflx {} must succeed, got {:?}",
            args.join(" "),
            output.status
        );
        for needle in [
            "--web",
            "--web-port",
            "--web-bind",
            "--web-auth-token",
            "--web-unix-socket",
            "--no-web-unix-socket",
        ] {
            assert!(
                stdout.contains(needle),
                "cflx {} must still advertise {needle}, got stdout={stdout}",
                args.join(" ")
            );
        }
        for needle in ["--server", "cflx server", "cflx project", "cflx service"] {
            assert!(
                !stdout.contains(needle),
                "cflx {} must not advertise '{needle}', got stdout={stdout}",
                args.join(" ")
            );
        }
    }
}

/// The retained `/api/v2` bind guarantee must still be enforced by the binary.
///
/// A routable `--web-bind` without credentials has to refuse startup. A stubbed
/// or bypassed web path would silently succeed here.
#[test]
#[cfg(feature = "web-monitoring")]
fn retained_web_bind_validation_still_refuses_an_unauthenticated_routable_listener() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());
    init_git_repo(tmp.path());

    let output = cflx_output(
        tmp.path(),
        &["run", "--all", "--web", "--web-bind", "0.0.0.0"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "a routable bind without credentials must not start, got {:?}",
        output.status
    );
    assert!(
        stderr.contains("Error:"),
        "expected a startup refusal diagnostic, got stderr={stderr}"
    );

    // The same bind with a token is accepted, so the refusal above is the
    // credential rule rather than a broken `--web-bind`.
    let authenticated = cflx_output(
        tmp.path(),
        &[
            "run",
            "--all",
            "--web",
            "--web-bind",
            "127.0.0.1",
            "--web-port",
            "0",
            "--web-auth-token",
            "test-token",
        ],
    );
    assert!(
        authenticated.status.success(),
        "an authenticated local web run must still succeed, got {:?} (stderr={})",
        authenticated.status,
        String::from_utf8_lossy(&authenticated.stderr)
    );
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
    init_git_repo(tmp.path());

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

/// Run `cflx` with a deadline, capturing stdout and stderr through files.
///
/// A test that asserts a *refusal* must fail rather than hang when the refusal
/// regresses: the workspace it uses has pending changes, so a process that
/// wrongly proceeds would orchestrate indefinitely and take the test with it.
/// Redirecting to files instead of pipes keeps the deadline honest — a full pipe
/// buffer cannot stall the child while the parent waits.
fn cflx_output_bounded(cwd: &Path, args: &[&str], timeout: Duration) -> std::process::Output {
    let capture = tempfile::tempdir().expect("capture dir");
    let out_path = capture.path().join("stdout");
    let err_path = capture.path().join("stderr");
    let mut child = Command::new(env!("CARGO_BIN_EXE_cflx"))
        .args(args)
        .current_dir(cwd)
        .stdout(fs::File::create(&out_path).unwrap())
        .stderr(fs::File::create(&err_path).unwrap())
        .spawn()
        .expect("failed to run cflx");

    let start = Instant::now();
    let status = loop {
        match child.try_wait().expect("failed to check child status") {
            Some(status) => break status,
            None => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!(
                        "cflx {args:?} did not exit within {timeout:?}; it must refuse startup \
                         rather than orchestrate (stderr={})",
                        String::from_utf8_lossy(&fs::read(&err_path).unwrap_or_default())
                    );
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };

    std::process::Output {
        status,
        stdout: fs::read(&out_path).unwrap_or_default(),
        stderr: fs::read(&err_path).unwrap_or_default(),
    }
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

    let owner = take_repository_lock(tmp.path(), InvocationMode::Tui);

    // Default TUI (no subcommand), explicit local TUI, and run.
    let guarded: [Vec<&str>; 3] = [vec![], vec!["tui"], vec!["run", "--all"]];
    for args in guarded {
        let output = cflx_output(tmp.path(), &args);
        assert_rejected_as_lock_conflict(&output, &format!("cflx {}", args.join(" ")));
    }

    drop(owner);
}

#[test]
fn non_orchestration_commands_bypass_the_lock() {
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

    let owner = take_repository_lock(tmp.path(), InvocationMode::Tui);
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

/// Positive proof that local `--web` really binds and publishes a reachable URL.
///
/// A stubbed or accidentally removed web path fails here: the run must report a
/// concrete OS-assigned port, not the requested `0`.
#[test]
#[cfg(feature = "web-monitoring")]
fn local_run_web_binds_a_real_port_and_publishes_its_url() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());
    init_git_repo(tmp.path());

    let output = cflx_output(tmp.path(), &["run", "--all", "--web", "--web-port", "0"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "cflx run --web must succeed, got {:?} (stdout={stdout})",
        output.status
    );

    let url = stdout
        .split_whitespace()
        .find(|token| token.starts_with("http://"))
        .unwrap_or_else(|| panic!("no web URL reported, stdout={stdout}"))
        .trim_end_matches(|c: char| !c.is_ascii_digit())
        .to_string();
    let port: u16 = url
        .rsplit(':')
        .next()
        .and_then(|p| p.parse().ok())
        .unwrap_or_else(|| panic!("web URL has no port: {url}"));
    assert!(
        port != 0,
        "expected a real OS-assigned port from an actual bind, got {url}"
    );
}

/// Abnormal termination of the lock owner releases ownership.
///
/// The owner is a real child process holding the OS advisory lock, so this
/// proves file-descriptor release semantics rather than in-process cleanup.
#[test]
fn killing_the_lock_owner_releases_the_repository_lock() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());
    init_git_repo(tmp.path());

    // A long-running local orchestration process: `run` blocks on the apply
    // command, so it holds the repository lock until it is killed.
    add_change(tmp.path(), "slow");
    fs::write(
        tmp.path().join(".cflx.jsonc"),
        r#"{
  "apply_command": "sleep 120",
  "archive_command": "echo archive",
  "analyze_command": "echo analyze",
  "acceptance_command": "",
  "use_llm_analysis": false
}
"#,
    )
    .unwrap();

    let mut owner = KillOnDrop(
        Command::new(env!("CARGO_BIN_EXE_cflx"))
            .args(["run", "--all"])
            .current_dir(tmp.path())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("failed to spawn cflx run"),
    );

    // Wait until the child actually owns the lock, proven by a competing
    // invocation being rejected with the conflict diagnostic.
    let start = Instant::now();
    loop {
        let output = cflx_output(tmp.path(), &["run", "--all"]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success() && stderr.contains(CONFLICT_HEADLINE) {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "the spawned owner never took the repository lock (stderr={stderr})"
        );
        std::thread::sleep(Duration::from_millis(100));
    }

    owner.0.kill().unwrap();
    owner.0.wait().unwrap();

    let reacquired = repo_lock::acquire(tmp.path(), InvocationMode::Run)
        .expect("lock must be acquirable after the owner is killed");
    assert!(reacquired.is_some());
}

// ── Default repository-scoped Unix API socket ──────────────────────────────
//
// Startup ordering, the non-Git refusal, opt-out, endpoint publication, hard
// bind failure, stale replacement, and finite-run cleanup are all properties of
// the *process*, not of a function, so every test here drives a real `cflx`.

/// The default socket path a web-enabled build must choose for `dir`.
#[cfg(all(unix, feature = "web-monitoring"))]
fn default_socket_path(dir: &Path) -> std::path::PathBuf {
    repo_lock::discover_common_dir(dir)
        .expect("git common dir")
        .join("cflx-api.sock")
}

/// A project whose apply command and lifecycle adapter each leave a marker.
///
/// A refused startup must produce neither: that is how "no AI subprocess and no
/// lifecycle adapter started" is proven rather than asserted.
#[cfg(all(unix, feature = "web-monitoring"))]
fn setup_observable_project(dir: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let applied = dir.join("apply-ran");
    let adapter_ran = dir.join("adapter-ran");
    let adapter = dir.join("adapter.sh");
    fs::write(
        &adapter,
        format!(
            "#!/bin/sh\ntouch '{}'\ncat > /dev/null\n",
            adapter_ran.display()
        ),
    )
    .unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&adapter, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fs::write(
        dir.join(".cflx.jsonc"),
        serde_json::json!({
            "apply_command": format!("touch '{}'", applied.display()),
            "archive_command": "echo archive",
            "analyze_command": "echo analyze",
            "acceptance_command": "",
            "use_llm_analysis": false,
            "lifecycle_integration": { "command": [adapter.display().to_string()] },
        })
        .to_string(),
    )
    .unwrap();
    fs::create_dir_all(dir.join("openspec/changes")).unwrap();
    (applied, adapter_ran)
}

/// The default local API is served without `--web`, on the repository's socket,
/// and a finite run removes it on its way out.
#[test]
#[cfg(all(unix, feature = "web-monitoring"))]
fn default_run_serves_the_repository_socket_and_cleans_it_up() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());
    init_git_repo(tmp.path());
    let socket = default_socket_path(tmp.path());

    let output = cflx_output(tmp.path(), &["run", "--all"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "the default run must succeed, got {:?} (stderr={})",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains(&format!("unix://{}", socket.display())),
        "the default Unix endpoint must be reported, stdout={stdout}"
    );
    // No TCP listener without `--web`.
    assert!(
        !stdout.contains("http://"),
        "no TCP endpoint may be published without --web, stdout={stdout}"
    );
    assert!(
        !socket.exists(),
        "terminal completion must remove the socket without another signal"
    );
}

/// The socket is a live endpoint during the run, not just a file.
#[test]
#[cfg(all(unix, feature = "web-monitoring"))]
fn a_running_process_answers_the_default_socket_and_publishes_it() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());
    init_git_repo(tmp.path());
    add_change(tmp.path(), "slow");
    fs::write(
        tmp.path().join(".cflx.jsonc"),
        r#"{
  "apply_command": "sleep 120",
  "archive_command": "echo archive",
  "analyze_command": "echo analyze",
  "acceptance_command": "",
  "use_llm_analysis": false
}
"#,
    )
    .unwrap();
    let socket = default_socket_path(tmp.path());

    let mut owner = KillOnDrop(
        Command::new(env!("CARGO_BIN_EXE_cflx"))
            .args(["run", "--all"])
            .current_dir(tmp.path())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("failed to spawn cflx run"),
    );

    let start = Instant::now();
    while !socket.exists() {
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "the default socket never appeared at {}",
            socket.display()
        );
        std::thread::sleep(Duration::from_millis(50));
    }

    // Owner-only permissions, and a real HTTP answer over the socket.
    {
        use std::os::unix::fs::{FileTypeExt, PermissionsExt};
        let metadata = fs::symlink_metadata(&socket).unwrap();
        assert!(metadata.file_type().is_socket());
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    }
    assert!(
        socket_health_ok(&socket),
        "the published socket must answer /api/v2/health"
    );

    // The conflict diagnostic reports the endpoint the owner actually bound.
    let competing = cflx_output(tmp.path(), &["run", "--all"]);
    let stderr = assert_rejected_as_lock_conflict(&competing, "competing run with a bound socket");
    assert!(
        stderr.contains(&format!("unix://{}", socket.display())),
        "expected the bound Unix endpoint in the diagnostic, got stderr={stderr}"
    );

    owner.0.kill().unwrap();
    owner.0.wait().unwrap();
}

/// A single blocking HTTP request over the socket, used as a liveness proof.
#[cfg(all(unix, feature = "web-monitoring"))]
fn socket_health_ok(socket: &Path) -> bool {
    use std::io::{Read, Write};
    let Ok(mut stream) = std::os::unix::net::UnixStream::connect(socket) else {
        return false;
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("read timeout");
    if stream
        .write_all(b"GET /api/v2/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut raw = String::new();
    if stream.read_to_string(&mut raw).is_err() {
        return false;
    }
    raw.starts_with("HTTP/1.1 200")
}

/// Outside Git there is no repository identity to derive a socket from, so the
/// operator has to choose — and the refusal happens before any work starts.
#[test]
#[cfg(all(unix, feature = "web-monitoring"))]
fn a_non_git_invocation_refuses_before_any_orchestration_side_effect() {
    let tmp = tempfile::tempdir().unwrap();
    let (applied, adapter_ran) = setup_observable_project(tmp.path());
    add_change(tmp.path(), "a");
    assert!(
        repo_lock::discover_common_dir(tmp.path()).is_none(),
        "this workspace must be outside Git for the test to mean anything"
    );

    let output = cflx_output_bounded(tmp.path(), &["run", "--all"], Duration::from_secs(30));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "a non-Git default must exit non-zero, got {:?}",
        output.status
    );
    assert!(
        stderr.contains("--web-unix-socket") && stderr.contains("--no-web-unix-socket"),
        "the error must explain both explicit choices, got stderr={stderr}"
    );
    assert!(!applied.exists(), "no AI subprocess may run");
    assert!(!adapter_ran.exists(), "no lifecycle adapter may start");
}

/// Both explicit choices stay usable outside Git; only the *default* is refused.
#[test]
#[cfg(all(unix, feature = "web-monitoring"))]
fn outside_git_the_explicit_choices_still_start() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());
    assert!(repo_lock::discover_common_dir(tmp.path()).is_none());

    let opted_out = cflx_output(tmp.path(), &["run", "--all", "--no-web-unix-socket"]);
    assert!(
        opted_out.status.success(),
        "the opt-out must start, got {:?} (stderr={})",
        opted_out.status,
        String::from_utf8_lossy(&opted_out.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&opted_out.stdout).contains("unix://"),
        "the opt-out must publish no Unix endpoint"
    );

    let explicit_path = tmp.path().join("explicit.sock");
    let explicit = cflx_output(
        tmp.path(),
        &[
            "run",
            "--all",
            "--web-unix-socket",
            explicit_path.to_str().unwrap(),
        ],
    );
    assert!(
        explicit.status.success(),
        "an explicit path must start, got {:?} (stderr={})",
        explicit.status,
        String::from_utf8_lossy(&explicit.stderr)
    );
    assert!(
        String::from_utf8_lossy(&explicit.stdout)
            .contains(&format!("unix://{}", explicit_path.display())),
        "the explicit endpoint must be published"
    );
    assert!(
        !explicit_path.exists(),
        "the explicit socket must be removed at completion too"
    );
}

/// The opt-out really means no socket, and `--web` still adds TCP on its own.
#[test]
#[cfg(all(unix, feature = "web-monitoring"))]
fn the_opt_out_binds_no_socket_and_still_allows_the_tcp_listener() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());
    init_git_repo(tmp.path());
    let socket = default_socket_path(tmp.path());

    let output = cflx_output(
        tmp.path(),
        &[
            "run",
            "--all",
            "--no-web-unix-socket",
            "--web",
            "--web-port",
            "0",
        ],
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "opt-out plus --web must succeed, got {:?} (stderr={})",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !socket.exists(),
        "the opt-out must never create the default socket"
    );
    assert!(
        !stdout.contains("unix://"),
        "no Unix endpoint may be published after opting out, stdout={stdout}"
    );
    assert!(
        stdout.contains("http://"),
        "--web must still publish its TCP endpoint, stdout={stdout}"
    );
}

/// `--web` adds TCP to the default socket rather than replacing it, and both
/// endpoints reach the conflict diagnostic.
#[test]
#[cfg(all(unix, feature = "web-monitoring"))]
fn the_web_flag_publishes_both_endpoints() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());
    init_git_repo(tmp.path());
    let socket = default_socket_path(tmp.path());

    let output = cflx_output(tmp.path(), &["run", "--all", "--web", "--web-port", "0"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "a dual-listener run must succeed, got {:?} (stderr={})",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains(&format!("unix://{}", socket.display())),
        "stdout={stdout}"
    );

    let port: u16 = stdout
        .split_whitespace()
        .find(|token| token.starts_with("http://"))
        .and_then(|token| {
            token
                .trim_end_matches(|c: char| !c.is_ascii_digit())
                .rsplit(':')
                .next()?
                .parse()
                .ok()
        })
        .unwrap_or_else(|| panic!("no TCP URL reported, stdout={stdout}"));
    assert!(
        port != 0,
        "expected a real OS-assigned port, stdout={stdout}"
    );

    // Both endpoints were published to the owner metadata, and a legacy reader
    // that only understands `api_url` still gets a usable diagnostic shape.
    let owner_file = repo_lock::discover_common_dir(tmp.path())
        .unwrap()
        .join(repo_lock::OWNER_FILE_NAME);
    let metadata = repo_lock::read_owner_metadata(&owner_file).expect("owner metadata");
    assert_eq!(
        metadata.available_endpoints(),
        vec![
            format!("unix://{}", socket.display()),
            format!("http://localhost:{port}")
        ],
        "both bound endpoints must be published in order"
    );
}

/// A UDS bind failure is a hard startup error: nothing downstream may run, and
/// the entry that caused the failure must survive untouched.
#[test]
#[cfg(all(unix, feature = "web-monitoring"))]
fn a_blocked_socket_path_exits_before_orchestration_and_preserves_the_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let (applied, adapter_ran) = setup_observable_project(tmp.path());
    init_git_repo(tmp.path());
    add_change(tmp.path(), "a");

    // A regular file at the target path is never ours to remove.
    let socket = default_socket_path(tmp.path());
    fs::write(&socket, b"not a socket").unwrap();

    let output = cflx_output_bounded(tmp.path(), &["run", "--all"], Duration::from_secs(30));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "a blocked socket path must exit non-zero, got {:?}",
        output.status
    );
    assert!(stderr.contains("not a socket"), "stderr={stderr}");
    assert_eq!(
        fs::read(&socket).unwrap(),
        b"not a socket",
        "the blocking entry must be preserved byte for byte"
    );
    assert!(!applied.exists(), "no AI subprocess may run");
    assert!(!adapter_ran.exists(), "no lifecycle adapter may start");
}

/// A socket left behind by a dead owner must not wedge the repository forever.
#[test]
#[cfg(all(unix, feature = "web-monitoring"))]
fn a_stale_socket_is_replaced_by_the_next_run() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());
    init_git_repo(tmp.path());
    let socket = default_socket_path(tmp.path());

    // Bind and drop the listener, leaving exactly what a killed owner leaves.
    {
        let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        drop(listener);
    }
    assert!(
        fs::symlink_metadata(&socket).is_ok(),
        "the stale entry must be on disk before the run"
    );

    let output = cflx_output(tmp.path(), &["run", "--all"]);
    assert!(
        output.status.success(),
        "a stale socket must be replaced, not fatal, got {:?} (stderr={})",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains(&format!("unix://{}", socket.display())),
        "the replaced endpoint must still be published"
    );
    assert!(!socket.exists(), "the replacement is cleaned up in turn");
}

/// Linked worktrees are one repository, so they advertise one socket — and the
/// repository lock is what keeps two owners from racing for it.
#[test]
#[cfg(all(unix, feature = "web-monitoring"))]
fn linked_worktrees_advertise_one_socket() {
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

    assert_eq!(default_socket_path(&main), default_socket_path(&linked));

    let from_linked = cflx_output(&linked, &["run", "--all"]);
    assert!(
        from_linked.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&from_linked.stderr)
    );
    assert!(
        String::from_utf8_lossy(&from_linked.stdout)
            .contains(&format!("unix://{}", default_socket_path(&main).display())),
        "a linked worktree must advertise the shared repository socket"
    );
}

/// A build without `web-monitoring` has no API to serve, so the default socket
/// contract does not apply to it at all.
///
/// This test only runs in a `--no-default-features` build; that is the point —
/// it is the assertion that keeps the feature-disabled binary API-free instead
/// of inheriting a startup requirement it cannot satisfy.
#[test]
#[cfg(all(unix, not(feature = "web-monitoring")))]
fn a_feature_disabled_build_keeps_its_api_free_behavior() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());
    init_git_repo(tmp.path());

    // No socket flags, inside Git: a web-enabled build would bind a socket here.
    let output = cflx_output(tmp.path(), &["run", "--all"]);
    assert!(
        output.status.success(),
        "a feature-disabled run must not require a socket, got {:?} (stderr={})",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let common_dir = repo_lock::discover_common_dir(tmp.path()).expect("git common dir");
    assert!(
        !common_dir.join("cflx-api.sock").exists(),
        "a feature-disabled build must never create the default socket"
    );

    // Outside Git it is equally unaffected.
    let outside = tempfile::tempdir().unwrap();
    setup_empty_project(outside.path());
    assert!(
        cflx_output(outside.path(), &["run", "--all"])
            .status
            .success(),
        "a feature-disabled run must not require a repository identity"
    );
}

/// Ensures a spawned child never outlives the test, including on panic.
struct KillOnDrop(std::process::Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
