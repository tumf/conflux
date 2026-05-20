use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Scope selector for OpenSpec change ID completion candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeIdCandidateScope {
    pub active: bool,
    pub archived: bool,
}

impl ChangeIdCandidateScope {
    /// Default to active changes when no explicit scope flag is supplied.
    pub const fn from_flags(active: bool, archived: bool) -> Self {
        if !active && !archived {
            Self {
                active: true,
                archived: false,
            }
        } else {
            Self { active, archived }
        }
    }
}

/// Discover logical OpenSpec change IDs for shell completion.
///
/// The function intentionally reads only workspace-local `openspec/changes` state,
/// performs no logging, and treats absent workspace structures as an empty success.
pub fn discover_change_id_candidates(
    workspace_root: &Path,
    scope: ChangeIdCandidateScope,
    prefix: Option<&str>,
) -> Vec<String> {
    let changes_dir = workspace_root.join("openspec").join("changes");
    let mut candidates = BTreeSet::new();

    if scope.active {
        collect_candidate_dirs(&changes_dir, false, &mut candidates);
    }

    if scope.archived {
        collect_candidate_dirs(&changes_dir.join("archive"), true, &mut candidates);
    }

    candidates
        .into_iter()
        .filter(|candidate| prefix.is_none_or(|prefix| candidate.starts_with(prefix)))
        .collect()
}

fn collect_candidate_dirs(
    dir: &Path,
    normalize_dated_archive: bool,
    candidates: &mut BTreeSet<String>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() || !path.join("proposal.md").is_file() {
            continue;
        }

        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };

        if name.starts_with('.') || name == "archive" {
            continue;
        }

        let logical_id = if normalize_dated_archive {
            normalize_archived_change_id(&name)
        } else {
            name
        };
        candidates.insert(logical_id);
    }
}

fn normalize_archived_change_id(name: &str) -> String {
    let bytes = name.as_bytes();
    let has_date_prefix = bytes.len() > 11
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit)
        && bytes[10] == b'-';

    if has_date_prefix {
        name[11..].to_string()
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_proposal(root: &Path, relative_dir: &str) {
        let dir = root.join(relative_dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("proposal.md"), "# Proposal\n").unwrap();
    }

    #[test]
    fn default_scope_is_active_only() {
        assert_eq!(
            ChangeIdCandidateScope::from_flags(false, false),
            ChangeIdCandidateScope {
                active: true,
                archived: false
            }
        );
    }

    #[test]
    fn explicit_scope_flags_are_preserved() {
        assert_eq!(
            ChangeIdCandidateScope::from_flags(true, true),
            ChangeIdCandidateScope {
                active: true,
                archived: true
            }
        );
        assert_eq!(
            ChangeIdCandidateScope::from_flags(false, true),
            ChangeIdCandidateScope {
                active: false,
                archived: true
            }
        );
    }

    #[test]
    fn discovers_active_entries_and_ignores_invalid_dirs() {
        let tmp = TempDir::new().unwrap();
        write_proposal(tmp.path(), "openspec/changes/add-shell-completion");
        write_proposal(tmp.path(), "openspec/changes/bugfix");
        fs::create_dir_all(tmp.path().join("openspec/changes/no-proposal")).unwrap();
        write_proposal(tmp.path(), "openspec/changes/archive/old-change");
        write_proposal(tmp.path(), "openspec/changes/.hidden");

        let candidates = discover_change_id_candidates(
            tmp.path(),
            ChangeIdCandidateScope::from_flags(false, false),
            None,
        );

        assert_eq!(candidates, vec!["add-shell-completion", "bugfix"]);
    }

    #[test]
    fn discovers_archived_entries_and_normalizes_dated_archives() {
        let tmp = TempDir::new().unwrap();
        write_proposal(tmp.path(), "openspec/changes/archive/direct-archive");
        write_proposal(
            tmp.path(),
            "openspec/changes/archive/2026-05-20-dated-archive",
        );
        write_proposal(tmp.path(), "openspec/changes/archive/2026-no-normalize");

        let candidates = discover_change_id_candidates(
            tmp.path(),
            ChangeIdCandidateScope::from_flags(false, true),
            None,
        );

        assert_eq!(
            candidates,
            vec!["2026-no-normalize", "dated-archive", "direct-archive"]
        );
    }

    #[test]
    fn filters_by_prefix_and_deduplicates_logical_ids() {
        let tmp = TempDir::new().unwrap();
        write_proposal(tmp.path(), "openspec/changes/add-one");
        write_proposal(tmp.path(), "openspec/changes/add-two");
        write_proposal(tmp.path(), "openspec/changes/other");
        write_proposal(tmp.path(), "openspec/changes/archive/2026-05-20-add-one");

        let candidates = discover_change_id_candidates(
            tmp.path(),
            ChangeIdCandidateScope::from_flags(true, true),
            Some("add-"),
        );

        assert_eq!(candidates, vec!["add-one", "add-two"]);
    }

    #[test]
    fn missing_changes_directory_is_empty_success() {
        let tmp = TempDir::new().unwrap();
        let candidates = discover_change_id_candidates(
            tmp.path(),
            ChangeIdCandidateScope::from_flags(true, true),
            None,
        );

        assert!(candidates.is_empty());
    }
}
