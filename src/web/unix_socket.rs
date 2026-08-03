//! Repository-scoped Unix-domain listener for the local `/api/v2` server.
//!
//! The default endpoint is `${GIT_COMMON_DIR}/cflx-api.sock`. That location is
//! deliberate: the Git *common* directory is the identity the repository lock
//! already excludes on, so every linked worktree of one repository advertises
//! one socket while unrelated repositories stay independent — and the lock makes
//! two default owners impossible without any extra registry.
//!
//! Nothing here is durable workflow state. The socket is an access surface for
//! local clients and reverse proxies, created at startup and removed at
//! shutdown; the next action for a workspace never depends on whether it exists.

use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// File name of the default repository-scoped API socket.
pub const DEFAULT_SOCKET_FILE_NAME: &str = "cflx-api.sock";

/// Owner-only permissions: the filesystem is the whole access control story for
/// a token-free local socket.
pub const SOCKET_MODE: u32 = 0o600;

/// `sun_path` capacity, including the terminating NUL.
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
pub const MAX_SOCKET_PATH_BYTES: usize = 104;
#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
)))]
pub const MAX_SOCKET_PATH_BYTES: usize = 108;

/// How long a liveness probe waits before it treats a socket as live.
///
/// A listener normally completes `connect` from the kernel backlog without ever
/// being scheduled, so the only way to time out here is a saturated backlog —
/// which still means somebody is listening.
const LIVENESS_PROBE_TIMEOUT: Duration = Duration::from_millis(250);

/// The Unix listener the operator selected for this process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnixSocketSelection {
    /// Bind this path before orchestration starts.
    Bind(PathBuf),
    /// The operator opted out with `--no-web-unix-socket`.
    Disabled,
}

impl UnixSocketSelection {
    /// The path to bind, if any.
    pub fn path(&self) -> Option<&Path> {
        match self {
            UnixSocketSelection::Bind(path) => Some(path),
            UnixSocketSelection::Disabled => None,
        }
    }
}

/// The default socket path for a repository's canonical Git common directory.
pub fn default_socket_path(common_dir: &Path) -> PathBuf {
    common_dir.join(DEFAULT_SOCKET_FILE_NAME)
}

/// Decide which Unix socket a local orchestration invocation owns.
///
/// `common_dir` is the canonical Git common directory, or `None` outside a Git
/// repository. Outside Git there is no repository identity to derive a
/// deterministic path from, so guessing one would give every unrelated
/// directory the same endpoint; the operator has to choose instead.
pub fn resolve_unix_socket(
    explicit: Option<&Path>,
    opt_out: bool,
    common_dir: Option<&Path>,
) -> Result<UnixSocketSelection, String> {
    // Clap already rejects the combination, but the resolver is the contract the
    // startup path depends on, so it refuses the contradiction on its own.
    if opt_out && explicit.is_some() {
        return Err(
            "--web-unix-socket and --no-web-unix-socket are mutually exclusive".to_string(),
        );
    }
    if opt_out {
        return Ok(UnixSocketSelection::Disabled);
    }
    if let Some(path) = explicit {
        return Ok(UnixSocketSelection::Bind(path.to_path_buf()));
    }
    match common_dir {
        Some(common_dir) => Ok(UnixSocketSelection::Bind(default_socket_path(common_dir))),
        None => Err(
            "the default API socket needs a Git repository: run inside one, choose a \
             path with --web-unix-socket PATH, or opt out with --no-web-unix-socket"
                .to_string(),
        ),
    }
}

/// The `unix://` form used in endpoint metadata and diagnostics.
///
/// It is discovery information for local clients and reverse proxies, never a
/// browser URL.
pub fn unix_endpoint(path: &Path) -> String {
    format!("unix://{}", path.display())
}

/// What currently occupies a target socket path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetState {
    /// Nothing is there; binding is safe.
    Absent,
    /// A socket nobody answers on. Only this state may be removed.
    StaleSocket,
    /// A socket that accepts connections — somebody else's live endpoint.
    LiveSocket,
    /// A regular file, directory, symlink, or device. Never ours to touch.
    NonSocket,
}

/// Classify a target path without modifying it.
///
/// A symlink counts as `NonSocket` even when it points at a socket: following it
/// would let an attacker-controlled link decide what gets unlinked.
pub async fn classify_target(path: &Path) -> io::Result<TargetState> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(TargetState::Absent),
        Err(err) => return Err(err),
    };
    if !metadata.file_type().is_socket() {
        return Ok(TargetState::NonSocket);
    }
    match tokio::time::timeout(
        LIVENESS_PROBE_TIMEOUT,
        tokio::net::UnixStream::connect(path),
    )
    .await
    {
        Ok(Ok(_stream)) => Ok(TargetState::LiveSocket),
        Ok(Err(_)) => Ok(TargetState::StaleSocket),
        Err(_elapsed) => Ok(TargetState::LiveSocket),
    }
}

/// Removes the socket entry this process created, and only that entry.
///
/// Device and inode are captured at bind time, so a path that was externally
/// unlinked and replaced during the run belongs to somebody else by shutdown and
/// is left alone.
#[derive(Debug)]
pub struct SocketGuard {
    path: PathBuf,
    device: u64,
    inode: u64,
}

impl SocketGuard {
    /// Record the identity of the entry at `path`, which must exist.
    fn capture(path: &Path) -> io::Result<Self> {
        let metadata = std::fs::symlink_metadata(path)?;
        Ok(Self {
            path: path.to_path_buf(),
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }

    /// True when the entry at the path is still the one this guard created.
    pub fn still_owns_path(&self) -> bool {
        let Ok(metadata) = std::fs::symlink_metadata(&self.path) else {
            return false;
        };
        metadata.file_type().is_socket()
            && metadata.dev() == self.device
            && metadata.ino() == self.inode
    }

    /// Remove the owned socket entry. Idempotent, and a no-op once the path
    /// identifies something else.
    pub fn release(&self) {
        if self.still_owns_path() {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

impl Drop for SocketGuard {
    fn drop(&mut self) {
        self.release();
    }
}

/// Reject a path the platform cannot represent in `sun_path`.
///
/// `bind` would report this as a bare `EINVAL`/`ENAMETOOLONG`, which reads as a
/// bug rather than as "your repository lives too deep".
fn check_path_length(path: &Path) -> Result<(), String> {
    let len = path.as_os_str().as_encoded_bytes().len();
    if len >= MAX_SOCKET_PATH_BYTES {
        return Err(format!(
            "Unix socket path '{}' is {len} bytes, but this platform allows at most {}; \
             choose a shorter path with --web-unix-socket PATH or opt out with \
             --no-web-unix-socket",
            path.display(),
            MAX_SOCKET_PATH_BYTES - 1
        ));
    }
    Ok(())
}

/// Prepare a target path for binding, removing only an unreachable stale socket.
///
/// Fail-closed on everything else: a live socket is somebody's working endpoint
/// and a non-socket entry is somebody's file, and neither becomes ours by being
/// in the way.
pub async fn prepare_socket_path(path: &Path) -> Result<(), String> {
    check_path_length(path)?;
    let state = classify_target(path).await.map_err(|err| {
        format!(
            "failed to inspect Unix socket path '{}': {err}",
            path.display()
        )
    })?;
    match state {
        TargetState::Absent => Ok(()),
        TargetState::NonSocket => Err(format!(
            "refusing to use Unix socket path '{}': it already exists and is not a socket",
            path.display()
        )),
        TargetState::LiveSocket => Err(format!(
            "refusing to use Unix socket path '{}': another process is listening on it",
            path.display()
        )),
        TargetState::StaleSocket => std::fs::remove_file(path).map_err(|err| {
            format!(
                "failed to remove the stale Unix socket at '{}': {err}",
                path.display()
            )
        }),
    }
}

/// Bind the Unix listener and restrict it to owner-only access.
///
/// Returns the listener together with the guard that owns cleanup. A failure to
/// apply the permissions removes the socket rather than leaving a
/// world-reachable endpoint behind.
pub async fn bind_unix_listener(
    path: &Path,
) -> Result<(tokio::net::UnixListener, SocketGuard), String> {
    prepare_socket_path(path).await?;

    let listener = tokio::net::UnixListener::bind(path)
        .map_err(|err| format!("failed to bind Unix socket '{}': {err}", path.display()))?;

    if let Err(err) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(SOCKET_MODE)) {
        drop(listener);
        let _ = std::fs::remove_file(path);
        return Err(format!(
            "failed to restrict Unix socket '{}' to mode {SOCKET_MODE:o}: {err}",
            path.display()
        ));
    }

    let guard = SocketGuard::capture(path).map_err(|err| {
        let _ = std::fs::remove_file(path);
        format!(
            "failed to record the identity of Unix socket '{}': {err}",
            path.display()
        )
    })?;

    Ok((listener, guard))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Path resolution (pure) ─────────────────────────────────────────────

    #[test]
    fn default_path_follows_the_git_common_directory() {
        let selection =
            resolve_unix_socket(None, false, Some(Path::new("/repo/.git"))).expect("resolves");
        assert_eq!(
            selection,
            UnixSocketSelection::Bind(PathBuf::from("/repo/.git/cflx-api.sock"))
        );
    }

    /// The lock and the socket share one identity, so the resolver must agree
    /// with `repo_lock` on what "this repository" means.
    #[test]
    fn linked_worktrees_resolve_one_socket_while_repositories_stay_distinct() {
        let main_git_dir =
            crate::repo_lock::resolve_git_dir(Path::new("/repo/.git"), None).unwrap();
        let main_common = crate::repo_lock::resolve_common_dir(&main_git_dir, None);

        let linked_git_dir = crate::repo_lock::resolve_git_dir(
            Path::new("/repo/wt/.git"),
            Some("gitdir: /repo/.git/worktrees/wt\n"),
        )
        .unwrap();
        let linked_common = crate::repo_lock::resolve_common_dir(&linked_git_dir, Some("../..\n"));

        let from_main = resolve_unix_socket(None, false, Some(&main_common)).unwrap();
        let from_linked = resolve_unix_socket(None, false, Some(&linked_common)).unwrap();
        assert_eq!(from_main, from_linked);
        assert_eq!(
            from_main,
            UnixSocketSelection::Bind(PathBuf::from("/repo/.git/cflx-api.sock"))
        );

        let other = resolve_unix_socket(None, false, Some(Path::new("/other/.git"))).unwrap();
        assert_ne!(from_main, other);
    }

    #[test]
    fn an_explicit_path_overrides_the_default() {
        let selection = resolve_unix_socket(
            Some(Path::new("/run/user/1000/custom.sock")),
            false,
            Some(Path::new("/repo/.git")),
        )
        .expect("resolves");
        assert_eq!(
            selection,
            UnixSocketSelection::Bind(PathBuf::from("/run/user/1000/custom.sock"))
        );
        assert_eq!(
            selection.path(),
            Some(Path::new("/run/user/1000/custom.sock"))
        );
    }

    #[test]
    fn opt_out_disables_the_listener_everywhere() {
        for common_dir in [Some(Path::new("/repo/.git")), None] {
            assert_eq!(
                resolve_unix_socket(None, true, common_dir).expect("opt-out always resolves"),
                UnixSocketSelection::Disabled
            );
        }
        assert_eq!(UnixSocketSelection::Disabled.path(), None);
    }

    #[test]
    fn an_explicit_path_still_works_outside_git() {
        assert_eq!(
            resolve_unix_socket(Some(Path::new("/tmp/cflx.sock")), false, None).unwrap(),
            UnixSocketSelection::Bind(PathBuf::from("/tmp/cflx.sock"))
        );
    }

    #[test]
    fn outside_git_the_default_is_refused_with_both_choices_named() {
        let error = resolve_unix_socket(None, false, None).expect_err("no repository identity");
        assert!(error.contains("--web-unix-socket"), "error={error}");
        assert!(error.contains("--no-web-unix-socket"), "error={error}");
    }

    #[test]
    fn the_override_and_the_opt_out_contradict_each_other() {
        let error = resolve_unix_socket(Some(Path::new("/tmp/a.sock")), true, None)
            .expect_err("contradictory selection");
        assert!(error.contains("mutually exclusive"), "error={error}");
    }

    #[test]
    fn unix_endpoints_are_scheme_qualified() {
        assert_eq!(
            unix_endpoint(Path::new("/repo/.git/cflx-api.sock")),
            "unix:///repo/.git/cflx-api.sock"
        );
    }

    #[test]
    fn an_unrepresentable_path_is_refused_with_the_platform_limit() {
        let long = PathBuf::from(format!("/tmp/{}.sock", "x".repeat(MAX_SOCKET_PATH_BYTES)));
        let error = check_path_length(&long).expect_err("path exceeds sun_path");
        assert!(error.contains("--web-unix-socket"), "error={error}");
        assert!(
            error.contains(&(MAX_SOCKET_PATH_BYTES - 1).to_string()),
            "error={error}"
        );
        check_path_length(Path::new("/tmp/cflx-api.sock")).expect("a short path is representable");
    }

    // ── Target classification and cleanup (real filesystem) ────────────────

    #[tokio::test]
    async fn an_absent_path_is_safe_to_bind() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("api.sock");
        assert_eq!(classify_target(&path).await.unwrap(), TargetState::Absent);
        prepare_socket_path(&path).await.expect("absent is safe");
    }

    #[tokio::test]
    async fn a_regular_file_or_directory_is_preserved() {
        let tmp = tempfile::tempdir().unwrap();

        let file = tmp.path().join("file.sock");
        std::fs::write(&file, b"precious").unwrap();
        assert_eq!(
            classify_target(&file).await.unwrap(),
            TargetState::NonSocket
        );
        let error = prepare_socket_path(&file).await.expect_err("file refused");
        assert!(error.contains("not a socket"), "error={error}");
        assert_eq!(std::fs::read(&file).unwrap(), b"precious");

        let dir = tmp.path().join("dir.sock");
        std::fs::create_dir(&dir).unwrap();
        assert_eq!(classify_target(&dir).await.unwrap(), TargetState::NonSocket);
        prepare_socket_path(&dir)
            .await
            .expect_err("directory refused");
        assert!(dir.is_dir(), "the directory must survive the refusal");
    }

    /// A symlink is classified by the link itself, so unlinking can never be
    /// redirected at a target the operator did not choose.
    #[tokio::test]
    async fn a_symlink_to_a_socket_is_not_treated_as_our_socket() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real.sock");
        let (_listener, _guard) = bind_unix_listener(&real).await.expect("binds");
        let link = tmp.path().join("link.sock");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        assert_eq!(
            classify_target(&link).await.unwrap(),
            TargetState::NonSocket
        );
        prepare_socket_path(&link)
            .await
            .expect_err("symlink refused");
        assert!(link.exists(), "the symlink must survive the refusal");
    }

    #[tokio::test]
    async fn a_live_socket_is_never_removed() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("api.sock");
        let (_listener, guard) = bind_unix_listener(&path).await.expect("binds");

        assert_eq!(
            classify_target(&path).await.unwrap(),
            TargetState::LiveSocket
        );
        let error = prepare_socket_path(&path)
            .await
            .expect_err("live socket refused");
        assert!(
            error.contains("another process is listening"),
            "error={error}"
        );
        assert!(guard.still_owns_path(), "the live socket must survive");
    }

    #[tokio::test]
    async fn an_unreachable_socket_is_replaced() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("api.sock");

        // Drop the listener but leave the entry behind, which is exactly what a
        // killed owner leaves on disk.
        let (listener, guard) = bind_unix_listener(&path).await.expect("binds");
        std::mem::forget(guard);
        drop(listener);
        assert!(path.exists(), "the stale entry must still be on disk");
        assert_eq!(
            classify_target(&path).await.unwrap(),
            TargetState::StaleSocket
        );

        let (_new_listener, new_guard) = bind_unix_listener(&path).await.expect("replaces stale");
        assert!(new_guard.still_owns_path());
    }

    #[tokio::test]
    async fn a_bound_socket_is_owner_only() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("api.sock");
        let (_listener, _guard) = bind_unix_listener(&path).await.expect("binds");

        let mode = std::fs::symlink_metadata(&path)
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, SOCKET_MODE, "socket mode must be 0600, got {mode:o}");
    }

    #[tokio::test]
    async fn cleanup_removes_only_the_entry_this_process_created() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("api.sock");
        let (listener, guard) = bind_unix_listener(&path).await.expect("binds");

        guard.release();
        assert!(!path.exists(), "shutdown must remove its own socket");
        drop(listener);

        // A replacement created after an external unlink belongs to somebody
        // else, so a late release must leave it alone.
        let (listener, guard) = bind_unix_listener(&path).await.expect("binds again");
        std::fs::remove_file(&path).unwrap();
        let (_replacement, _replacement_guard) =
            bind_unix_listener(&path).await.expect("replacement binds");
        assert!(!guard.still_owns_path());
        guard.release();
        assert!(path.exists(), "the replacement must survive our shutdown");
        drop(listener);
    }

    #[tokio::test]
    async fn dropping_the_guard_removes_the_socket() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("api.sock");
        let (listener, guard) = bind_unix_listener(&path).await.expect("binds");
        drop(guard);
        assert!(!path.exists(), "drop must clean up the owned socket");
        drop(listener);
    }

    #[tokio::test]
    async fn binding_into_a_missing_directory_reports_the_path() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("missing").join("api.sock");
        let error = bind_unix_listener(&path).await.expect_err("no parent dir");
        assert!(
            error.contains("failed to bind Unix socket"),
            "error={error}"
        );
        assert!(error.contains("api.sock"), "error={error}");
    }
}
