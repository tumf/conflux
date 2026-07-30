//! First-parent cumulative-history classification.
//!
//! Before an opted-in run may publish cumulative HEAD, every commit on the
//! first-parent integration spine between the initial-fetch merge-base and
//! starting local HEAD must be recognized. Recognition is derived from Git
//! parent evidence plus the commit's own tree, never from a message alone: a
//! `Merge change:` subject with no matching archive evidence in its tree is
//! unrelated history.
//!
//! Commits reachable only through a validated merge's non-first parents are the
//! merge's payload and are not classified independently.

use std::collections::BTreeSet;

use super::trailers::{validate_upstream_merge, UpstreamTrailers};

/// Subject prefix for a single-change cumulative integration.
const MERGE_CHANGE_PREFIX: &str = "Merge change:";
/// Subject prefix for a batched cumulative integration.
const MERGE_CHANGES_PREFIX: &str = "Merge changes:";

/// Archive/active evidence read from a commit's own tree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommitTreeEvidence {
    /// Change IDs with an archive entry under `openspec/changes/archive/`.
    pub archived_change_ids: BTreeSet<String>,
    /// Change IDs still present as active `openspec/changes/<id>/` directories.
    pub active_change_ids: BTreeSet<String>,
}

impl CommitTreeEvidence {
    pub fn new<I, J>(archived: I, active: J) -> Self
    where
        I: IntoIterator<Item = String>,
        J: IntoIterator<Item = String>,
    {
        Self {
            archived_change_ids: archived.into_iter().collect(),
            active_change_ids: active.into_iter().collect(),
        }
    }
}

/// A commit observed on the first-parent spine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpineCommit {
    pub sha: String,
    /// Raw commit message (subject plus body/trailers).
    pub message: String,
    /// Parent SHAs in Git order.
    pub parents: Vec<String>,
    /// Archive/active evidence from this commit's own tree.
    pub tree_evidence: CommitTreeEvidence,
}

impl SpineCommit {
    pub fn subject(&self) -> &str {
        self.message.lines().next().unwrap_or("").trim()
    }
}

/// Result of classifying one first-parent spine commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpineCommitClass {
    /// Cumulative change integration with matching commit-tree archive evidence.
    CumulativeChangeIntegration { change_ids: Vec<String> },
    /// Validated Conflux upstream integration merge.
    UpstreamIntegration(UpstreamTrailers),
    /// Anything else: unrelated local-only history.
    Unrelated { reason: String },
}

/// Extract change IDs from a cumulative integration subject.
///
/// Returns `None` when the subject is not a cumulative integration subject.
fn parse_change_integration_subject(subject: &str) -> Option<Vec<String>> {
    let rest = if let Some(rest) = subject.strip_prefix(MERGE_CHANGES_PREFIX) {
        rest
    } else {
        subject.strip_prefix(MERGE_CHANGE_PREFIX)?
    };

    let ids: Vec<String> = rest
        .split(',')
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();

    (!ids.is_empty()).then_some(ids)
}

/// Classify one first-parent spine commit.
pub fn classify_spine_commit(
    commit: &SpineCommit,
    expected_remote: &str,
    expected_branch: &str,
) -> SpineCommitClass {
    let subject = commit.subject();

    if let Some(change_ids) = parse_change_integration_subject(subject) {
        if commit.parents.len() < 2 {
            return SpineCommitClass::Unrelated {
                reason: format!(
                    "'{}' is not a merge commit, so it carries no cumulative integration evidence",
                    subject
                ),
            };
        }
        let missing_archive: Vec<&String> = change_ids
            .iter()
            .filter(|id| !commit.tree_evidence.archived_change_ids.contains(*id))
            .collect();
        if !missing_archive.is_empty() {
            return SpineCommitClass::Unrelated {
                reason: format!(
                    "commit tree has no archive evidence for {}",
                    missing_archive
                        .iter()
                        .map(|id| id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            };
        }
        let still_active: Vec<&String> = change_ids
            .iter()
            .filter(|id| commit.tree_evidence.active_change_ids.contains(*id))
            .collect();
        if !still_active.is_empty() {
            return SpineCommitClass::Unrelated {
                reason: format!(
                    "commit tree still contains active change directories for {}",
                    still_active
                        .iter()
                        .map(|id| id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            };
        }
        return SpineCommitClass::CumulativeChangeIntegration { change_ids };
    }

    match validate_upstream_merge(
        &commit.message,
        &commit.parents,
        expected_remote,
        expected_branch,
    ) {
        Ok(trailers) => SpineCommitClass::UpstreamIntegration(trailers),
        Err(err) => SpineCommitClass::Unrelated {
            reason: format!("unrecognized first-parent commit '{}' ({:?})", subject, err),
        },
    }
}

/// Outcome of validating the whole first-parent spine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpineValidation {
    /// Validated upstream merges, oldest first.
    pub upstream_merges: Vec<(String, UpstreamTrailers)>,
    /// Change IDs published by validated cumulative integrations.
    pub integrated_change_ids: Vec<String>,
    /// First rejected commit, when the spine is not publishable.
    pub rejected: Option<(String, String)>,
}

impl SpineValidation {
    pub fn is_publishable(&self) -> bool {
        self.rejected.is_none()
    }
}

/// Validate the first-parent spine from merge-base (exclusive) to HEAD.
///
/// `commits` MUST be first-parent ordered; ordering within the result follows
/// the input order.
pub fn validate_spine(
    commits: &[SpineCommit],
    expected_remote: &str,
    expected_branch: &str,
) -> SpineValidation {
    let mut validation = SpineValidation {
        upstream_merges: Vec::new(),
        integrated_change_ids: Vec::new(),
        rejected: None,
    };

    for commit in commits {
        match classify_spine_commit(commit, expected_remote, expected_branch) {
            SpineCommitClass::CumulativeChangeIntegration { change_ids } => {
                validation.integrated_change_ids.extend(change_ids);
            }
            SpineCommitClass::UpstreamIntegration(trailers) => {
                validation
                    .upstream_merges
                    .push((commit.sha.clone(), trailers));
            }
            SpineCommitClass::Unrelated { reason } => {
                validation.rejected = Some((commit.sha.clone(), reason));
                break;
            }
        }
    }

    validation
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::upstream::trailers::format_upstream_merge_message;

    const SHA: &str = "1111111111111111111111111111111111111111";
    const PARENT: &str = "2222222222222222222222222222222222222222";

    fn change_merge(subject: &str, evidence: CommitTreeEvidence) -> SpineCommit {
        SpineCommit {
            sha: "aaa".into(),
            message: format!("{}\n", subject),
            parents: vec![PARENT.into(), "bbb".into()],
            tree_evidence: evidence,
        }
    }

    #[test]
    fn upstream_integration_accepts_change_merge_with_archive_evidence() {
        let commit = change_merge(
            "Merge change: my-change",
            CommitTreeEvidence::new(["my-change".to_string()], []),
        );
        assert_eq!(
            classify_spine_commit(&commit, "origin", "main"),
            SpineCommitClass::CumulativeChangeIntegration {
                change_ids: vec!["my-change".to_string()]
            }
        );
    }

    #[test]
    fn upstream_integration_accepts_batched_change_merge() {
        let commit = change_merge(
            "Merge changes: a, b",
            CommitTreeEvidence::new(["a".to_string(), "b".to_string()], []),
        );
        assert_eq!(
            classify_spine_commit(&commit, "origin", "main"),
            SpineCommitClass::CumulativeChangeIntegration {
                change_ids: vec!["a".to_string(), "b".to_string()]
            }
        );
    }

    #[test]
    fn upstream_integration_rejects_change_merge_without_archive_evidence() {
        let commit = change_merge("Merge change: my-change", CommitTreeEvidence::default());
        assert!(matches!(
            classify_spine_commit(&commit, "origin", "main"),
            SpineCommitClass::Unrelated { .. }
        ));
    }

    #[test]
    fn upstream_integration_rejects_change_merge_that_left_change_active() {
        let commit = change_merge(
            "Merge change: my-change",
            CommitTreeEvidence::new(["my-change".to_string()], ["my-change".to_string()]),
        );
        assert!(matches!(
            classify_spine_commit(&commit, "origin", "main"),
            SpineCommitClass::Unrelated { .. }
        ));
    }

    #[test]
    fn upstream_integration_accepts_validated_upstream_merge() {
        let commit = SpineCommit {
            sha: "ccc".into(),
            message: format_upstream_merge_message("origin", "main", SHA),
            parents: vec![PARENT.into(), SHA.into()],
            tree_evidence: CommitTreeEvidence::default(),
        };
        assert!(matches!(
            classify_spine_commit(&commit, "origin", "main"),
            SpineCommitClass::UpstreamIntegration(_)
        ));
        // A different selected remote makes the same commit unrecognized.
        assert!(matches!(
            classify_spine_commit(&commit, "upstream", "main"),
            SpineCommitClass::Unrelated { .. }
        ));
    }

    #[test]
    fn upstream_integration_rejects_unrelated_first_parent_commit() {
        let commit = SpineCommit {
            sha: "ddd".into(),
            message: "hotfix: patch production\n".into(),
            parents: vec![PARENT.into()],
            tree_evidence: CommitTreeEvidence::default(),
        };
        assert!(matches!(
            classify_spine_commit(&commit, "origin", "main"),
            SpineCommitClass::Unrelated { .. }
        ));
    }

    #[test]
    fn upstream_integration_validates_whole_spine_and_stops_at_first_rejection() {
        let commits = vec![
            change_merge(
                "Merge change: a",
                CommitTreeEvidence::new(["a".to_string()], []),
            ),
            SpineCommit {
                sha: "ccc".into(),
                message: format_upstream_merge_message("origin", "main", SHA),
                parents: vec![PARENT.into(), SHA.into()],
                tree_evidence: CommitTreeEvidence::default(),
            },
            SpineCommit {
                sha: "eee".into(),
                message: "local hack\n".into(),
                parents: vec![PARENT.into()],
                tree_evidence: CommitTreeEvidence::default(),
            },
        ];

        let validation = validate_spine(&commits, "origin", "main");
        assert!(!validation.is_publishable());
        assert_eq!(validation.integrated_change_ids, vec!["a".to_string()]);
        assert_eq!(validation.upstream_merges.len(), 1);
        assert_eq!(validation.rejected.as_ref().unwrap().0, "eee");
    }

    #[test]
    fn upstream_integration_accepts_fully_recognized_spine() {
        let commits = vec![change_merge(
            "Merge change: a",
            CommitTreeEvidence::new(["a".to_string()], []),
        )];
        assert!(validate_spine(&commits, "origin", "main").is_publishable());
        assert!(validate_spine(&[], "origin", "main").is_publishable());
    }
}
