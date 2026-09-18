//! Private-by-default directory and file helpers for the evaluation root.
//!
//! Evaluation records are pseudonymized, not encrypted, so the filesystem
//! permission is the only access control they have. Creating the directory
//! first and tightening it afterwards would leave a window where the umask
//! decided; these helpers set the mode at creation where the platform supports
//! it and re-assert it for a directory that already existed.

use std::fs;
use std::path::Path;

use super::error::{EvaluationError, EvaluationResult};

/// Directory mode for the evaluation root and its descendants.
#[cfg(unix)]
pub const PRIVATE_DIR_MODE: u32 = 0o700;

/// File mode for evaluation records and the installation salt.
#[cfg(unix)]
pub const PRIVATE_FILE_MODE: u32 = 0o600;

/// Create `path` and its missing ancestors with private permissions.
///
/// Ancestors created here get the same private mode: an evaluation root under a
/// state root Conflux itself had to create must not be reachable through a
/// world-executable parent it also created.
pub fn create_private_dir_all(path: &Path) -> EvaluationResult<()> {
    if path.is_dir() {
        #[cfg(unix)]
        enforce_dir_mode(path)?;
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.is_dir() {
            create_private_dir_all(parent)?;
        }
    }

    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(PRIVATE_DIR_MODE);
    }
    match builder.create(path) {
        Ok(()) => Ok(()),
        // A concurrent creator won; its mode is enforced below rather than
        // treated as a failure.
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && path.is_dir() => {
            #[cfg(unix)]
            enforce_dir_mode(path)?;
            Ok(())
        }
        Err(source) => Err(EvaluationError::Directory { source }),
    }
}

/// Re-assert the private mode on a directory that already existed.
#[cfg(unix)]
fn enforce_dir_mode(path: &Path) -> EvaluationResult<()> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path).map_err(|source| EvaluationError::Directory { source })?;
    if metadata.permissions().mode() & 0o777 == PRIVATE_DIR_MODE {
        return Ok(());
    }
    fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_DIR_MODE))
        .map_err(|source| EvaluationError::Directory { source })
}

/// Open an evaluation record file append-only, creating it privately.
///
/// `O_APPEND` is what makes a concurrent append safe: the offset is chosen by
/// the kernel under the same lock as the write, so two writers cannot land on
/// the same offset and interleave halves of two JSON objects.
pub fn open_private_append(path: &Path) -> EvaluationResult<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.append(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(PRIVATE_FILE_MODE);
    }
    options
        .open(path)
        .map_err(|source| EvaluationError::Append { source })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn nested_directories_are_created_privately() {
        use std::os::unix::fs::PermissionsExt;

        let base = tempfile::tempdir().expect("temp base");
        let nested = base
            .path()
            .join("evaluations")
            .join("parallel_dependency")
            .join("proj-1234");
        create_private_dir_all(&nested).expect("nested creation must succeed");

        let mut checked = 0;
        let mut current = nested.as_path();
        while current != base.path() {
            let mode = fs::metadata(current)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, PRIVATE_DIR_MODE, "{} is {mode:o}", current.display());
            checked += 1;
            current = current.parent().expect("parent");
        }
        assert_eq!(checked, 3, "every created ancestor must be checked");
    }

    #[cfg(unix)]
    #[test]
    fn an_existing_loose_directory_is_tightened() {
        use std::os::unix::fs::PermissionsExt;

        let base = tempfile::tempdir().expect("temp base");
        let loose = base.path().join("evaluations");
        fs::create_dir(&loose).expect("create");
        fs::set_permissions(&loose, fs::Permissions::from_mode(0o777)).expect("loosen");

        create_private_dir_all(&loose).expect("must tighten rather than fail");
        assert_eq!(
            fs::metadata(&loose).expect("metadata").permissions().mode() & 0o777,
            PRIVATE_DIR_MODE
        );
    }

    #[test]
    fn creating_an_existing_directory_twice_is_idempotent() {
        let base = tempfile::tempdir().expect("temp base");
        let path = base.path().join("evaluations").join("nested");
        create_private_dir_all(&path).expect("first");
        create_private_dir_all(&path).expect("second");
        assert!(path.is_dir());
    }

    #[test]
    fn a_directory_under_a_file_is_a_typed_error_not_a_panic() {
        let base = tempfile::tempdir().expect("temp base");
        let blocker = base.path().join("evaluations");
        fs::write(&blocker, b"not a directory").expect("write");

        let error = create_private_dir_all(&blocker.join("parallel_dependency"))
            .expect_err("a file cannot become a parent directory");
        assert!(
            matches!(error, EvaluationError::Directory { .. }),
            "{error:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn append_files_are_created_privately_and_never_truncated() {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;

        let base = tempfile::tempdir().expect("temp base");
        let path = base.path().join("2026-09-18.jsonl");

        let mut first = open_private_append(&path).expect("create");
        first.write_all(b"one\n").expect("write");
        drop(first);

        let mut second = open_private_append(&path).expect("reopen");
        second.write_all(b"two\n").expect("write");
        drop(second);

        assert_eq!(fs::read_to_string(&path).expect("read"), "one\ntwo\n");
        assert_eq!(
            fs::metadata(&path).expect("metadata").permissions().mode() & 0o777,
            PRIVATE_FILE_MODE
        );
    }
}
