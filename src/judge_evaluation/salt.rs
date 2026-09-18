//! Installation salt and directed pair pseudonymization.
//!
//! A pair identifier exists so aggregate precision/recall can be recomputed
//! per pair without storing what the pair *is*. It is local pseudonymization,
//! not an authentication primitive and not a commitment scheme: the salt only
//! prevents an unsalted dictionary of known change-ID pairs from being matched
//! across installations.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::error::{EvaluationError, EvaluationResult};
use crate::config::defaults::EVALUATION_SALT_FILE_NAME;

/// Required salt length. A short or long file is corruption, never a shorter key.
pub const SALT_LEN: usize = 32;

/// Domain separator, NUL-terminated so it can never be confused with the first
/// length-prefixed field.
const PAIR_DOMAIN: &[u8] = b"cflx-judge-eval-v1\0";

/// A loaded 32-byte installation salt.
#[derive(Clone)]
pub struct PairIdSalt(Vec<u8>);

impl std::fmt::Debug for PairIdSalt {
    /// The salt's bytes are deliberately unprintable: a `{:?}` in a diagnostic
    /// must not be the thing that leaks it into an ordinary log.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PairIdSalt(<redacted>)")
    }
}

impl PairIdSalt {
    /// Wrap exactly [`SALT_LEN`] bytes, for tests that need a deterministic salt.
    pub fn from_bytes(bytes: [u8; SALT_LEN]) -> Self {
        Self(bytes.to_vec())
    }

    /// Opaque identifier for one directed pair inside one project.
    ///
    /// Framing is unambiguous by construction: every variable-length field is
    /// preceded by its big-endian `u64` length, so no combination of project
    /// slug and change IDs can produce the same byte string as a different
    /// combination. Direction is meaningful — `(a, b)` and `(b, a)` frame
    /// differently and therefore digest differently.
    pub fn pair_id(&self, project_slug: &str, dependent: &str, dependency: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(&self.0);
        hasher.update(PAIR_DOMAIN);
        for field in [project_slug, dependent, dependency] {
            hasher.update((field.len() as u64).to_be_bytes());
            hasher.update(field.as_bytes());
        }
        let digest = hasher.finalize();

        let mut out = String::with_capacity(7 + digest.len() * 2);
        out.push_str("sha256:");
        for byte in digest {
            use std::fmt::Write as _;
            let _ = write!(out, "{byte:02x}");
        }
        out
    }
}

/// Load the installation salt, creating it exactly once if it is absent.
///
/// Creation is atomic and never replaces an installed salt: the loser of a
/// concurrent first-writer race discards its temporary file and reads the
/// winner's. That matters because replacing a salt silently re-pseudonymizes
/// every future record, which would split one pair's history into two
/// unrelatable identifiers without any error to notice.
pub fn load_or_create_salt(evaluation_root: &Path) -> EvaluationResult<PairIdSalt> {
    let path = evaluation_root.join(EVALUATION_SALT_FILE_NAME);

    if let Some(salt) = read_installed_salt(&path)? {
        return Ok(salt);
    }

    super::fsutil::create_private_dir_all(evaluation_root)?;

    let mut bytes = [0u8; SALT_LEN];
    let nonce: u64;
    {
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut bytes);
        nonce = rng.next_u64();
    }

    // The nonce is random rather than a PID or a clock reading, because two
    // threads of *one* process race here: they share a PID, and a coarse clock
    // can hand both the same tick. A collision there is not a harmless retry —
    // the loser's `create_new` fails and its cleanup unlinks the *winner's*
    // in-flight temporary, so the winner's install then fails too and neither
    // gets a salt.
    let temporary = evaluation_root.join(format!(
        ".pair-id-salt.{}.{nonce:016x}.tmp",
        std::process::id()
    ));

    let write_result = (|| -> std::io::Result<()> {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(&bytes)?;
        // Durability before the rename: a salt that is visible but empty after a
        // crash would fail every later read as corruption.
        file.sync_all()?;
        Ok(())
    })();

    if let Err(source) = write_result {
        let _ = fs::remove_file(&temporary);
        return Err(EvaluationError::Salt {
            reason: "temporary salt could not be written",
            source: Some(source),
        });
    }

    // `hard_link` + unlink rather than `rename`: rename would silently replace a
    // salt another process installed between the read above and here.
    match fs::hard_link(&temporary, &path) {
        Ok(()) => {
            let _ = fs::remove_file(&temporary);
            Ok(PairIdSalt(bytes.to_vec()))
        }
        Err(_) => {
            let _ = fs::remove_file(&temporary);
            // Lost the race, or the filesystem refuses links. Either way the
            // installed salt — if there is one — is the authority.
            read_installed_salt(&path)?.ok_or(EvaluationError::Salt {
                reason: "salt could not be installed",
                source: None,
            })
        }
    }
}

/// Read an already-installed salt, or `Ok(None)` when none exists yet.
///
/// Anything present but not a plain, correctly sized, non-symlink, privately
/// permissioned regular file is an error rather than a reason to overwrite it.
fn read_installed_salt(path: &Path) -> EvaluationResult<Option<PairIdSalt>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(EvaluationError::Salt {
                reason: "salt could not be inspected",
                source: Some(source),
            })
        }
    };

    if metadata.file_type().is_symlink() {
        return Err(EvaluationError::Salt {
            reason: "salt is a symlink",
            source: None,
        });
    }
    if !metadata.is_file() {
        return Err(EvaluationError::Salt {
            reason: "salt is not a regular file",
            source: None,
        });
    }
    if metadata.len() != SALT_LEN as u64 {
        return Err(EvaluationError::Salt {
            reason: "salt has the wrong length",
            source: None,
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(EvaluationError::Salt {
                reason: "salt is group- or world-accessible",
                source: None,
            });
        }
    }

    let bytes = fs::read(path).map_err(|source| EvaluationError::Salt {
        reason: "salt could not be read",
        source: Some(source),
    })?;
    if bytes.len() != SALT_LEN {
        return Err(EvaluationError::Salt {
            reason: "salt has the wrong length",
            source: None,
        });
    }
    Ok(Some(PairIdSalt(bytes)))
}

/// Path of the installation salt beneath an evaluation root.
pub fn salt_path(evaluation_root: &Path) -> PathBuf {
    evaluation_root.join(EVALUATION_SALT_FILE_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_salt(seed: u8) -> PairIdSalt {
        PairIdSalt::from_bytes([seed; SALT_LEN])
    }

    #[test]
    fn pair_id_is_stable_directional_and_project_scoped() {
        let salt = fixed_salt(7);

        let forward = salt.pair_id("proj-a", "alpha", "beta");
        assert_eq!(
            forward,
            salt.pair_id("proj-a", "alpha", "beta"),
            "same inputs and salt must be stable"
        );
        assert_ne!(
            forward,
            salt.pair_id("proj-a", "beta", "alpha"),
            "direction must change identity"
        );
        assert_ne!(
            forward,
            salt.pair_id("proj-b", "alpha", "beta"),
            "project scope must change identity"
        );
        assert_ne!(
            forward,
            fixed_salt(8).pair_id("proj-a", "alpha", "beta"),
            "a different installation salt must change identity"
        );
    }

    #[test]
    fn pair_id_is_opaque_lowercase_hex_and_reveals_no_input() {
        let salt = fixed_salt(1);
        let id = salt.pair_id("conflux-abcd1234", "add-secret-feature", "fix-secret-bug");

        let hex = id.strip_prefix("sha256:").expect("prefix must be present");
        assert_eq!(hex.len(), 64);
        assert!(hex
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
        for raw in ["conflux-abcd1234", "add-secret-feature", "fix-secret-bug"] {
            assert!(!id.contains(raw), "`{raw}` must not survive into `{id}`");
        }
    }

    #[test]
    fn length_framing_prevents_boundary_collisions() {
        let salt = fixed_salt(3);
        // Without length prefixes, "ab"+"c" and "a"+"bc" would concatenate to
        // the same bytes and collide into one identifier.
        assert_ne!(
            salt.pair_id("p", "ab", "c"),
            salt.pair_id("p", "a", "bc"),
            "adjacent fields must not be able to merge"
        );
        assert_ne!(
            salt.pair_id("pa", "b", "c"),
            salt.pair_id("p", "ab", "c"),
            "the project slug must not be able to bleed into the dependent"
        );
    }

    #[test]
    fn salt_is_created_once_and_reread_identically() {
        let root = tempfile::tempdir().expect("temp root");
        let root = root.path().join("evaluations");

        let first = load_or_create_salt(&root).expect("salt must be creatable");
        let installed = salt_path(&root);
        assert!(installed.is_file());
        assert_eq!(
            fs::metadata(&installed).expect("metadata").len(),
            SALT_LEN as u64
        );

        let second = load_or_create_salt(&root).expect("salt must be re-readable");
        assert_eq!(
            first.pair_id("p", "a", "b"),
            second.pair_id("p", "a", "b"),
            "re-loading must never rotate an installed salt"
        );
    }

    #[cfg(unix)]
    #[test]
    fn created_salt_and_root_are_private() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().expect("temp root");
        let root = root.path().join("evaluations");
        load_or_create_salt(&root).expect("salt must be creatable");

        assert_eq!(
            fs::metadata(&root)
                .expect("root metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(salt_path(&root))
                .expect("salt metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    fn no_temporary_salt_file_is_left_behind() {
        let root = tempfile::tempdir().expect("temp root");
        let root = root.path().join("evaluations");
        load_or_create_salt(&root).expect("salt must be creatable");

        let leftovers: Vec<_> = fs::read_dir(&root)
            .expect("root must be readable")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "leftover temporaries: {leftovers:?}");
    }

    #[test]
    fn corrupt_salt_is_an_error_rather_than_a_silent_rotation() {
        for (label, bytes) in [("short", vec![0u8; 8]), ("long", vec![0u8; 64])] {
            let root = tempfile::tempdir().expect("temp root");
            let root = root.path().join("evaluations");
            super::super::fsutil::create_private_dir_all(&root).expect("root");
            fs::write(salt_path(&root), &bytes).expect("corrupt salt");

            let error = load_or_create_salt(&root)
                .expect_err(&format!("a {label} salt must not be accepted"));
            assert!(
                matches!(error, EvaluationError::Salt { .. }),
                "unexpected error for {label}: {error:?}"
            );
            assert_eq!(
                fs::read(salt_path(&root)).expect("salt must survive"),
                bytes,
                "a corrupt salt must never be overwritten"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_salt_is_refused_without_following_it() {
        let outside = tempfile::tempdir().expect("temp outside");
        let target = outside.path().join("victim");
        fs::write(&target, [0u8; SALT_LEN]).expect("target");

        let root = tempfile::tempdir().expect("temp root");
        let root = root.path().join("evaluations");
        super::super::fsutil::create_private_dir_all(&root).expect("root");
        std::os::unix::fs::symlink(&target, salt_path(&root)).expect("symlink");

        let error = load_or_create_salt(&root).expect_err("a symlinked salt must be refused");
        assert!(matches!(error, EvaluationError::Salt { .. }), "{error:?}");
        assert!(target.exists(), "the symlink target must be untouched");
    }

    #[cfg(unix)]
    #[test]
    fn group_readable_salt_is_refused() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().expect("temp root");
        let root = root.path().join("evaluations");
        super::super::fsutil::create_private_dir_all(&root).expect("root");
        let path = salt_path(&root);
        fs::write(&path, [0u8; SALT_LEN]).expect("salt");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("permissions");

        let error = load_or_create_salt(&root).expect_err("a readable salt must be refused");
        assert!(matches!(error, EvaluationError::Salt { .. }), "{error:?}");
    }

    #[test]
    fn salt_debug_never_prints_its_bytes() {
        let rendered = format!("{:?}", fixed_salt(0xab));
        assert_eq!(rendered, "PairIdSalt(<redacted>)");
        assert!(!rendered.contains("ab"));
    }
}
