use crate::task_file::TaskFileFormat;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArchiveLayoutError {
    pub(crate) change_id: String,
    pub(crate) path: PathBuf,
}

impl ArchiveLayoutError {
    pub(crate) fn message(&self) -> String {
        format!(
            "Invalid archive layout for '{}': found nested archive path {}. Expected openspec/changes/archive/YYYY-MM-DD-{}. Do not manually move archive directories; restore the active change and rerun cflx openspec archive {} --yes.",
            self.change_id,
            self.path.display(),
            self.change_id,
            self.change_id
        )
    }
}

pub(crate) fn find_valid_archive_entry(change_id: &str, archive_dir: &Path) -> Option<PathBuf> {
    if !archive_dir.exists() {
        return None;
    }

    let direct = archive_dir.join(change_id);
    if direct.is_dir() {
        return Some(direct);
    }

    fs::read_dir(archive_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .find_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            is_valid_archive_entry_name(&name, change_id).then_some(path)
        })
}

/// Why the archive cannot name exactly one proposal for a change.
///
/// Resolution fails closed: an ambiguous archive or an unrecognisable layout is
/// reported, never resolved by `read_dir` order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArchivedProposalError {
    /// More than one canonical entry carries a `proposal.md` for the change.
    Ambiguous {
        change_id: String,
        entries: Vec<PathBuf>,
    },
    /// No canonical entry resolved, and a nested date layout explains why.
    InvalidLayout(ArchiveLayoutError),
}

impl ArchivedProposalError {
    pub(crate) fn message(&self) -> String {
        match self {
            Self::Ambiguous { change_id, entries } => format!(
                "Ambiguous archive for '{}': {} canonical archive entries contain proposal.md ({}). Keep exactly one archived entry for this change and remove or rename the others.",
                change_id,
                entries.len(),
                entries
                    .iter()
                    .map(|entry| entry.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::InvalidLayout(error) => error.message(),
        }
    }
}

/// Resolve the sole canonical archived `proposal.md` for `change_id`.
///
/// A deliberate sibling of [`find_valid_archive_entry`] rather than a
/// tightening of it: archive completion detection and task-file resolution
/// depend on that function's direct-entry preference, first-match fallback, and
/// `proposal.md`-optional contract, while a declaration source has to be both
/// unique and readable.
///
/// `Ok(None)` means the archive holds no canonical entry with a proposal for
/// this change and no nested date layout explains the absence.
pub(crate) fn find_archived_proposal(
    change_id: &str,
    archive_dir: &Path,
) -> Result<Option<PathBuf>, ArchivedProposalError> {
    let mut entries: Vec<PathBuf> = Vec::new();
    if let Ok(dir) = fs::read_dir(archive_dir) {
        for entry in dir.filter_map(|entry| entry.ok()) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if is_valid_archive_entry_name(&name, change_id) && path.join("proposal.md").is_file() {
                entries.push(path);
            }
        }
    }
    // `read_dir` order is filesystem-defined, so sorting is what makes the
    // ambiguity diagnostic reproducible instead of arbitrarily ordered.
    entries.sort();

    match entries.len() {
        0 => match invalid_layout_error(change_id, archive_dir) {
            Some(error) => Err(ArchivedProposalError::InvalidLayout(error)),
            None => Ok(None),
        },
        1 => Ok(Some(entries.remove(0).join("proposal.md"))),
        _ => Err(ArchivedProposalError::Ambiguous {
            change_id: change_id.to_string(),
            entries,
        }),
    }
}

pub(crate) fn find_invalid_nested_archive_entry(
    change_id: &str,
    archive_dir: &Path,
) -> Option<PathBuf> {
    if !archive_dir.exists() {
        return None;
    }

    fs::read_dir(archive_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .find_map(|entry| {
            let date_dir = entry.path();
            if !date_dir.is_dir() {
                return None;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !is_yyyy_mm_dd(&name) {
                return None;
            }
            let nested = date_dir.join(change_id);
            nested.is_dir().then_some(nested)
        })
}

pub(crate) fn invalid_layout_error(
    change_id: &str,
    archive_dir: &Path,
) -> Option<ArchiveLayoutError> {
    find_invalid_nested_archive_entry(change_id, archive_dir).map(|path| ArchiveLayoutError {
        change_id: change_id.to_string(),
        path,
    })
}

/// Repository-relative prefix of the active (live) change directory.
pub(crate) const ACTIVE_CHANGES_PREFIX: &str = "openspec/changes";

/// Repository-relative prefix of the archive directory.
pub(crate) const ARCHIVE_PREFIX: &str = "openspec/changes/archive";

/// Repository-relative path proving an active live change identity.
pub(crate) fn active_change_proposal_path(change_id: &str) -> String {
    format!("{ACTIVE_CHANGES_PREFIX}/{change_id}/proposal.md")
}

/// Whether a repository-relative path belongs to the active live change subtree.
pub(crate) fn is_active_change_path(path: &str, change_id: &str) -> bool {
    let prefix = format!("{ACTIVE_CHANGES_PREFIX}/{change_id}/");
    path.starts_with(&prefix)
}

/// Repository-relative path of the active (live) change task list in `format`.
///
/// The format is always explicit: no caller may silently construct `tasks.md`
/// for a change whose artifact is `tasks.json`.
pub(crate) fn active_change_tasks_path(change_id: &str, format: TaskFileFormat) -> String {
    format!("{ACTIVE_CHANGES_PREFIX}/{change_id}/{}", format.file_name())
}

/// Every repository-relative active task path that can speak for `change_id`.
pub(crate) fn active_change_tasks_paths(change_id: &str) -> Vec<(TaskFileFormat, String)> {
    TaskFileFormat::ALL
        .into_iter()
        .map(|format| (format, active_change_tasks_path(change_id, format)))
        .collect()
}

/// The format a repository-relative path names as `change_id`'s active task
/// artifact, if it names one at all.
pub(crate) fn active_change_tasks_format(path: &str, change_id: &str) -> Option<TaskFileFormat> {
    let rest = path.strip_prefix(&format!("{ACTIVE_CHANGES_PREFIX}/{change_id}/"))?;
    TaskFileFormat::from_file_name(rest)
}

/// The format a repository-relative path names as `change_id`'s archived task
/// artifact, if it names one at all.
///
/// Uses the same entry rules as the archived proposal identity, so a nested
/// date layout, a suffix collision, or another change's entry is never
/// mistaken for this change's archived task evidence.
pub(crate) fn archive_tasks_format(path: &str, change_id: &str) -> Option<TaskFileFormat> {
    let rest = path.strip_prefix(&format!("{ARCHIVE_PREFIX}/"))?;
    let (entry, file_name) = rest.rsplit_once('/')?;
    let format = TaskFileFormat::from_file_name(file_name)?;
    is_valid_archive_entry_name(entry, change_id).then_some(format)
}

/// Why a commit's diff does not prove a single-format archive transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArchiveTaskTransitionError {
    /// No deletion of an active task artifact was found.
    MissingDeletion,
    /// No addition of an archived task artifact was found.
    MissingAddition,
    /// More than one active or archived task artifact was touched.
    Ambiguous,
    /// The deleted and added artifacts use different formats.
    CrossFormat {
        /// Format removed from the active entry.
        deleted: TaskFileFormat,
        /// Format added at the archived entry.
        added: TaskFileFormat,
    },
}

impl ArchiveTaskTransitionError {
    /// Operator-facing reason, naming the change it applies to.
    pub(crate) fn message(&self, change_id: &str) -> String {
        match self {
            Self::MissingDeletion => format!(
                "archive transition for '{change_id}' deletes no active task artifact ({} or {})",
                TaskFileFormat::Markdown.file_name(),
                TaskFileFormat::Json.file_name()
            ),
            Self::MissingAddition => format!(
                "archive transition for '{change_id}' adds no task artifact at a valid archived entry"
            ),
            Self::Ambiguous => format!(
                "archive transition for '{change_id}' touches more than one task artifact; exactly one add/delete pair is required"
            ),
            Self::CrossFormat { deleted, added } => format!(
                "archive transition for '{change_id}' deletes {} but adds {}; an archive move never changes task-file format",
                deleted.file_name(),
                added.file_name()
            ),
        }
    }
}

/// Verify that a commit's diff moves exactly one task artifact from the active
/// entry to the valid archived entry, keeping its basename.
///
/// Archive validation cannot infer the deleted active filename from filesystem
/// existence — the file is gone by then — so the transition is proven from the
/// diff itself. `entries` are `(status, repository-relative path)` pairs.
pub(crate) fn classify_archive_task_transition(
    entries: &[(char, String)],
    change_id: &str,
) -> std::result::Result<TaskFileFormat, ArchiveTaskTransitionError> {
    let mut deleted: Vec<TaskFileFormat> = Vec::new();
    let mut added: Vec<TaskFileFormat> = Vec::new();

    // Only removal from the active entry and addition at the archived entry
    // describe the move. Any other status on those paths (a modification, a
    // type change) is unrelated churn and is ignored rather than guessed at;
    // callers pre-split renames into their delete and add halves.
    for (status, path) in entries {
        if *status == 'D' {
            if let Some(format) = active_change_tasks_format(path, change_id) {
                deleted.push(format);
            }
        } else if *status == 'A' {
            if let Some(format) = archive_tasks_format(path, change_id) {
                added.push(format);
            }
        }
    }

    if deleted.len() > 1 || added.len() > 1 {
        return Err(ArchiveTaskTransitionError::Ambiguous);
    }
    let deleted = deleted
        .first()
        .copied()
        .ok_or(ArchiveTaskTransitionError::MissingDeletion)?;
    let added = added
        .first()
        .copied()
        .ok_or(ArchiveTaskTransitionError::MissingAddition)?;
    if deleted != added {
        return Err(ArchiveTaskTransitionError::CrossFormat { deleted, added });
    }
    Ok(added)
}

/// Whether a repository-relative path is `file_name` inside the valid archive
/// entry for `change_id`.
///
/// Accepts the exact `<change_id>` entry and the dated `YYYY-MM-DD-<change_id>`
/// entry only. Nested date directories, unrelated entries, and suffix
/// collisions such as `prefix-<change_id>` are rejected, so they can never
/// stand in for the change's own archived file.
pub(crate) fn is_valid_archive_file_path(path: &str, change_id: &str, file_name: &str) -> bool {
    let Some(rest) = path.strip_prefix(&format!("{ARCHIVE_PREFIX}/")) else {
        return false;
    };
    let Some(entry) = rest.strip_suffix(&format!("/{file_name}")) else {
        return false;
    };
    is_valid_archive_entry_name(entry, change_id)
}

/// Whether a repository-relative path is the archived proposal identity for `change_id`.
///
/// Accepts the exact `<change_id>` entry and the dated `YYYY-MM-DD-<change_id>`
/// entry only. Nested date directories, unrelated entries, and suffix
/// collisions such as `prefix-<change_id>` are rejected, so they can never
/// authorize deletion of the live change.
pub(crate) fn is_valid_archive_proposal_path(path: &str, change_id: &str) -> bool {
    is_valid_archive_file_path(path, change_id, "proposal.md")
}

/// Whether a repository-relative path sits under an invalid nested archive layout
/// (`openspec/changes/archive/YYYY-MM-DD/<change_id>/...`).
pub(crate) fn is_invalid_nested_archive_path(path: &str, change_id: &str) -> bool {
    let Some(rest) = path.strip_prefix(&format!("{ARCHIVE_PREFIX}/")) else {
        return false;
    };
    let mut segments = rest.split('/');
    let (Some(date), Some(entry)) = (segments.next(), segments.next()) else {
        return false;
    };
    is_yyyy_mm_dd(date) && entry == change_id
}

/// Whether the supplied committed/indexed paths prove an active live change identity.
pub(crate) fn paths_contain_active_change<S: AsRef<str>>(paths: &[S], change_id: &str) -> bool {
    let expected = active_change_proposal_path(change_id);
    paths.iter().any(|path| path.as_ref() == expected)
}

/// Whether the supplied committed/indexed paths prove a valid archived identity.
pub(crate) fn paths_contain_valid_archive<S: AsRef<str>>(paths: &[S], change_id: &str) -> bool {
    paths
        .iter()
        .any(|path| is_valid_archive_proposal_path(path.as_ref(), change_id))
}

/// Whether the supplied committed/indexed paths carry the invalid nested archive
/// layout for `change_id`.
///
/// Used to separate "not archived at all" from "archived under a layout that can
/// never be recognised", so Git-view callers can report the actionable one.
pub(crate) fn paths_contain_invalid_nested_archive<S: AsRef<str>>(
    paths: &[S],
    change_id: &str,
) -> bool {
    paths
        .iter()
        .any(|path| is_invalid_nested_archive_path(path.as_ref(), change_id))
}

pub(crate) fn is_valid_archive_entry_name(name: &str, change_id: &str) -> bool {
    name == change_id || is_valid_dated_archive_name(name, change_id)
}

fn is_valid_dated_archive_name(name: &str, change_id: &str) -> bool {
    let Some(date) = name.strip_suffix(&format!("-{change_id}")) else {
        return false;
    };
    is_yyyy_mm_dd(date)
}

fn is_yyyy_mm_dd(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[8..10].iter().all(u8::is_ascii_digit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_archive_names_are_direct_or_dated() {
        assert!(is_valid_archive_entry_name("my-change", "my-change"));
        assert!(is_valid_archive_entry_name(
            "2026-07-09-my-change",
            "my-change"
        ));
        assert!(!is_valid_archive_entry_name(
            "prefix-my-change",
            "my-change"
        ));
        assert!(!is_valid_archive_entry_name(
            "2026-7-9-my-change",
            "my-change"
        ));
    }

    #[test]
    fn path_predicates_share_the_archive_name_rules() {
        assert!(is_active_change_path(
            "openspec/changes/my-change/proposal.md",
            "my-change"
        ));
        assert!(!is_active_change_path(
            "openspec/changes/archive/my-change/proposal.md",
            "my-change"
        ));
        assert!(!is_active_change_path(
            "openspec/changes/my-change-extra/proposal.md",
            "my-change"
        ));

        assert!(is_valid_archive_proposal_path(
            "openspec/changes/archive/my-change/proposal.md",
            "my-change"
        ));
        assert!(is_valid_archive_proposal_path(
            "openspec/changes/archive/2026-07-09-my-change/proposal.md",
            "my-change"
        ));
        for invalid in [
            // nested date layout
            "openspec/changes/archive/2026-07-09/my-change/proposal.md",
            // suffix collision
            "openspec/changes/archive/prefix-my-change/proposal.md",
            // malformed date
            "openspec/changes/archive/2026-7-9-my-change/proposal.md",
            // archived without the proposal identity
            "openspec/changes/archive/my-change/design.md",
            // live path, not archive
            "openspec/changes/my-change/proposal.md",
        ] {
            assert!(
                !is_valid_archive_proposal_path(invalid, "my-change"),
                "'{}' must not count as a valid archive identity",
                invalid
            );
        }

        assert!(is_invalid_nested_archive_path(
            "openspec/changes/archive/2026-07-09/my-change/proposal.md",
            "my-change"
        ));
        assert!(!is_invalid_nested_archive_path(
            "openspec/changes/archive/2026-07-09-my-change/proposal.md",
            "my-change"
        ));
    }

    #[test]
    fn archive_task_paths_use_the_same_entry_rules_as_proposals() {
        assert!(
            archive_tasks_format("openspec/changes/archive/my-change/tasks.md", "my-change")
                .is_some()
        );
        assert!(archive_tasks_format(
            "openspec/changes/archive/2026-07-09-my-change/tasks.md",
            "my-change"
        )
        .is_some());
        for invalid in [
            // nested date layout
            "openspec/changes/archive/2026-07-09/my-change/tasks.md",
            // suffix collision
            "openspec/changes/archive/prefix-my-change/tasks.md",
            // a different change's task list
            "openspec/changes/archive/other-change/tasks.md",
            // the live task list, not the archived one
            "openspec/changes/my-change/tasks.md",
            // a different file inside the right entry
            "openspec/changes/archive/my-change/proposal.md",
        ] {
            assert!(
                archive_tasks_format(invalid, "my-change").is_none(),
                "'{}' must not count as archived task evidence",
                invalid
            );
        }
        assert_eq!(
            active_change_tasks_path("my-change", TaskFileFormat::Markdown),
            "openspec/changes/my-change/tasks.md"
        );
        assert_eq!(
            active_change_tasks_path("my-change", TaskFileFormat::Json),
            "openspec/changes/my-change/tasks.json"
        );
    }

    #[test]
    fn task_path_recognition_accepts_either_supported_basename() {
        assert_eq!(
            active_change_tasks_format("openspec/changes/my-change/tasks.json", "my-change"),
            Some(TaskFileFormat::Json)
        );
        assert_eq!(
            active_change_tasks_format("openspec/changes/my-change/tasks.md", "my-change"),
            Some(TaskFileFormat::Markdown)
        );
        assert_eq!(
            active_change_tasks_format("openspec/changes/my-change/tasks.yaml", "my-change"),
            None
        );
        assert_eq!(
            active_change_tasks_format("openspec/changes/other/tasks.json", "my-change"),
            None
        );

        assert_eq!(
            archive_tasks_format(
                "openspec/changes/archive/2026-07-09-my-change/tasks.json",
                "my-change"
            ),
            Some(TaskFileFormat::Json)
        );
        for invalid in [
            "openspec/changes/archive/2026-07-09/my-change/tasks.json",
            "openspec/changes/archive/prefix-my-change/tasks.json",
            "openspec/changes/archive/other-change/tasks.json",
            "openspec/changes/my-change/tasks.json",
        ] {
            assert_eq!(
                archive_tasks_format(invalid, "my-change"),
                None,
                "'{invalid}' must not count as archived task evidence"
            );
        }
    }

    #[test]
    fn archive_transition_requires_one_same_basename_add_delete_pair() {
        let pair = |deleted: &str, added: &str| {
            vec![
                ('D', deleted.to_string()),
                ('A', added.to_string()),
                // Unrelated churn is ignored rather than treated as ambiguity.
                ('M', "src/lib.rs".to_string()),
            ]
        };

        assert_eq!(
            classify_archive_task_transition(
                &pair(
                    "openspec/changes/my-change/tasks.json",
                    "openspec/changes/archive/2026-07-09-my-change/tasks.json"
                ),
                "my-change"
            ),
            Ok(TaskFileFormat::Json)
        );
        assert_eq!(
            classify_archive_task_transition(
                &pair(
                    "openspec/changes/my-change/tasks.md",
                    "openspec/changes/archive/my-change/tasks.md"
                ),
                "my-change"
            ),
            Ok(TaskFileFormat::Markdown)
        );

        // Cross-format move.
        assert_eq!(
            classify_archive_task_transition(
                &pair(
                    "openspec/changes/my-change/tasks.json",
                    "openspec/changes/archive/my-change/tasks.md"
                ),
                "my-change"
            ),
            Err(ArchiveTaskTransitionError::CrossFormat {
                deleted: TaskFileFormat::Json,
                added: TaskFileFormat::Markdown,
            })
        );

        // Both basenames added.
        assert_eq!(
            classify_archive_task_transition(
                &[
                    ('D', "openspec/changes/my-change/tasks.json".to_string()),
                    (
                        'A',
                        "openspec/changes/archive/my-change/tasks.json".to_string()
                    ),
                    (
                        'A',
                        "openspec/changes/archive/my-change/tasks.md".to_string()
                    ),
                ],
                "my-change"
            ),
            Err(ArchiveTaskTransitionError::Ambiguous)
        );

        // Nested archive layout is not a valid archived entry.
        assert_eq!(
            classify_archive_task_transition(
                &pair(
                    "openspec/changes/my-change/tasks.json",
                    "openspec/changes/archive/2026-07-09/my-change/tasks.json"
                ),
                "my-change"
            ),
            Err(ArchiveTaskTransitionError::MissingAddition)
        );

        // Another change's archived entry proves nothing about this one.
        assert_eq!(
            classify_archive_task_transition(
                &pair(
                    "openspec/changes/my-change/tasks.json",
                    "openspec/changes/archive/other-change/tasks.json"
                ),
                "my-change"
            ),
            Err(ArchiveTaskTransitionError::MissingAddition)
        );

        // A missing deletion is never inferred from the addition alone.
        assert_eq!(
            classify_archive_task_transition(
                &[(
                    'A',
                    "openspec/changes/archive/my-change/tasks.json".to_string()
                )],
                "my-change"
            ),
            Err(ArchiveTaskTransitionError::MissingDeletion)
        );

        assert!(ArchiveTaskTransitionError::MissingDeletion
            .message("my-change")
            .contains("my-change"));
    }

    #[test]
    fn path_set_predicates_require_the_proposal_identity() {
        let paths = [
            "openspec/changes/my-change/tasks.md".to_string(),
            "openspec/changes/archive/2026-07-09/my-change/proposal.md".to_string(),
        ];
        assert!(!paths_contain_active_change(&paths, "my-change"));
        assert!(!paths_contain_valid_archive(&paths, "my-change"));

        let paths = [
            "openspec/changes/my-change/proposal.md".to_string(),
            "openspec/changes/archive/2026-07-09-my-change/proposal.md".to_string(),
        ];
        assert!(paths_contain_active_change(&paths, "my-change"));
        assert!(paths_contain_valid_archive(&paths, "my-change"));
    }

    /// Build an archive tree, creating `proposal.md` in every entry listed with
    /// `true`, and return the archive directory.
    fn archive_with(entries: &[(&str, bool)]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let archive_dir = dir.path().join("openspec/changes/archive");
        for (name, with_proposal) in entries {
            let entry = archive_dir.join(name);
            std::fs::create_dir_all(&entry).unwrap();
            if *with_proposal {
                std::fs::write(entry.join("proposal.md"), "# archived\n").unwrap();
            }
        }
        (dir, archive_dir)
    }

    #[test]
    fn archived_proposal_resolves_direct_and_dated_entries() {
        let (_dir, archive_dir) = archive_with(&[("my-change", true)]);
        assert_eq!(
            find_archived_proposal("my-change", &archive_dir),
            Ok(Some(archive_dir.join("my-change/proposal.md")))
        );

        let (_dir, archive_dir) = archive_with(&[("2026-07-09-my-change", true)]);
        assert_eq!(
            find_archived_proposal("my-change", &archive_dir),
            Ok(Some(archive_dir.join("2026-07-09-my-change/proposal.md")))
        );
    }

    #[test]
    fn archived_proposal_requires_a_canonical_entry_that_holds_a_proposal() {
        // An entry that is canonical but carries no proposal is not a
        // declaration source, and neither is any non-canonical name.
        let (_dir, archive_dir) = archive_with(&[
            ("my-change", false),
            ("prefix-my-change", true),
            ("2026-7-9-my-change", true),
            ("other-change", true),
        ]);
        assert_eq!(find_archived_proposal("my-change", &archive_dir), Ok(None));

        // A missing archive directory resolves to nothing rather than erroring.
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(
            find_archived_proposal("my-change", &empty.path().join("archive")),
            Ok(None)
        );
    }

    #[test]
    fn archived_proposal_fails_closed_on_competing_entries() {
        let (_dir, archive_dir) = archive_with(&[
            ("2026-07-09-my-change", true),
            ("my-change", true),
            ("2026-08-01-my-change", true),
        ]);

        let error = find_archived_proposal("my-change", &archive_dir).unwrap_err();
        let ArchivedProposalError::Ambiguous { change_id, entries } = &error else {
            panic!("competing entries must be ambiguous, got {error:?}");
        };
        assert_eq!(change_id, "my-change");
        // Sorted, so the diagnostic never depends on read_dir order.
        assert_eq!(
            entries,
            &vec![
                archive_dir.join("2026-07-09-my-change"),
                archive_dir.join("2026-08-01-my-change"),
                archive_dir.join("my-change"),
            ]
        );
        let message = error.message();
        assert!(message.contains("Ambiguous archive for 'my-change'"));
        for entry in entries {
            assert!(message.contains(&entry.display().to_string()));
        }
    }

    #[test]
    fn archived_proposal_reports_the_existing_nested_layout_diagnostic() {
        let (_dir, archive_dir) = archive_with(&[("2026-07-09/my-change", true)]);

        let error = find_archived_proposal("my-change", &archive_dir).unwrap_err();
        assert_eq!(
            error,
            ArchivedProposalError::InvalidLayout(
                invalid_layout_error("my-change", &archive_dir).unwrap()
            )
        );
        assert!(error.message().contains("Invalid archive layout"));
    }

    #[test]
    fn find_valid_archive_entry_keeps_its_own_contract() {
        // The sibling resolver must not have tightened this one: an entry
        // without a proposal still resolves here, because archive completion
        // and task-file resolution depend on that.
        let (_dir, archive_dir) = archive_with(&[("2026-07-09-my-change", false)]);
        assert_eq!(
            find_valid_archive_entry("my-change", &archive_dir),
            Some(archive_dir.join("2026-07-09-my-change"))
        );
        assert_eq!(find_archived_proposal("my-change", &archive_dir), Ok(None));

        // And the direct entry still wins over a dated one without failing closed.
        let (_dir, archive_dir) =
            archive_with(&[("2026-07-09-my-change", true), ("my-change", true)]);
        assert_eq!(
            find_valid_archive_entry("my-change", &archive_dir),
            Some(archive_dir.join("my-change"))
        );
    }

    #[test]
    fn nested_archive_entry_is_invalid_layout() {
        let dir = tempfile::tempdir().unwrap();
        let archive_dir = dir.path().join("openspec/changes/archive");
        let nested = archive_dir.join("2026-07-09/my-change");
        std::fs::create_dir_all(&nested).unwrap();

        let err = invalid_layout_error("my-change", &archive_dir).unwrap();
        assert_eq!(err.path, nested);
        assert!(err.message().contains("Invalid archive layout"));
        assert!(err.message().contains("2026-07-09/my-change"));
    }
}
