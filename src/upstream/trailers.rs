//! `Cflx-Upstream-*` identity trailers.
//!
//! An upstream merge commit is the only repository-visible evidence that a
//! Conflux checkpoint changed cumulative history. Restart identification,
//! remote/branch recovery, and fetched-SHA recovery all read these trailers, so
//! a commit whose trailers are missing, malformed, or contradicted by Git
//! parent/ancestry evidence is never classified as a Conflux upstream merge.

pub const TRAILER_REMOTE: &str = "Cflx-Upstream-Remote";
pub const TRAILER_BRANCH: &str = "Cflx-Upstream-Branch";
pub const TRAILER_SHA: &str = "Cflx-Upstream-SHA";

/// Subject line prefix used for Conflux upstream merge commits.
pub const UPSTREAM_MERGE_SUBJECT_PREFIX: &str = "Merge upstream:";

/// Validated identity recovered from an upstream merge commit message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstreamTrailers {
    pub remote: String,
    pub branch: String,
    pub sha: String,
}

fn is_full_sha(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|c| c.is_ascii_hexdigit())
}

/// Build the commit message for an upstream integration merge.
pub fn format_upstream_merge_message(remote: &str, branch: &str, sha: &str) -> String {
    format!(
        "{} {}/{}\n\n{}: {}\n{}: {}\n{}: {}\n",
        UPSTREAM_MERGE_SUBJECT_PREFIX,
        remote,
        branch,
        TRAILER_REMOTE,
        remote,
        TRAILER_BRANCH,
        branch,
        TRAILER_SHA,
        sha
    )
}

/// Parse upstream identity trailers from a raw commit message.
///
/// Returns `None` unless all three trailers are present exactly once with
/// non-empty values and a full 40-hex SHA. Duplicated trailers are rejected
/// because a rewritten or hand-edited message cannot establish identity.
pub fn parse_upstream_trailers(commit_message: &str) -> Option<UpstreamTrailers> {
    let mut remote: Option<String> = None;
    let mut branch: Option<String> = None;
    let mut sha: Option<String> = None;

    for line in commit_message.lines() {
        let line = line.trim();
        let assign = |slot: &mut Option<String>, value: &str| -> bool {
            if slot.is_some() {
                return false;
            }
            *slot = Some(value.trim().to_string());
            true
        };

        if let Some(value) = line.strip_prefix(&format!("{}:", TRAILER_REMOTE)) {
            if !assign(&mut remote, value) {
                return None;
            }
        } else if let Some(value) = line.strip_prefix(&format!("{}:", TRAILER_BRANCH)) {
            if !assign(&mut branch, value) {
                return None;
            }
        } else if let Some(value) = line.strip_prefix(&format!("{}:", TRAILER_SHA)) {
            if !assign(&mut sha, value) {
                return None;
            }
        }
    }

    let (remote, branch, sha) = (remote?, branch?, sha?);
    if remote.is_empty() || branch.is_empty() || !is_full_sha(&sha) {
        return None;
    }

    Some(UpstreamTrailers {
        remote,
        branch,
        sha,
    })
}

/// Why a candidate upstream merge commit failed identity validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrailerValidationError {
    /// Message carries no complete trailer set.
    Malformed,
    /// Trailer names a different remote than the selected one.
    RemoteMismatch { expected: String, found: String },
    /// Trailer names a different branch than the cumulative base branch.
    BranchMismatch { expected: String, found: String },
    /// Recorded SHA is not one of the merge commit's parents.
    ShaNotAParent { sha: String },
    /// Commit has fewer than two parents, so it is not a merge at all.
    NotAMergeCommit,
}

/// Validate a candidate upstream merge commit against Git parent evidence.
///
/// `parents` MUST be the commit's parent SHAs in Git order; the recorded
/// fetched SHA has to appear among the non-first parents.
pub fn validate_upstream_merge(
    commit_message: &str,
    parents: &[String],
    expected_remote: &str,
    expected_branch: &str,
) -> Result<UpstreamTrailers, TrailerValidationError> {
    let trailers =
        parse_upstream_trailers(commit_message).ok_or(TrailerValidationError::Malformed)?;

    if parents.len() < 2 {
        return Err(TrailerValidationError::NotAMergeCommit);
    }
    if trailers.remote != expected_remote {
        return Err(TrailerValidationError::RemoteMismatch {
            expected: expected_remote.to_string(),
            found: trailers.remote,
        });
    }
    if trailers.branch != expected_branch {
        return Err(TrailerValidationError::BranchMismatch {
            expected: expected_branch.to_string(),
            found: trailers.branch,
        });
    }
    if !parents[1..].contains(&trailers.sha) {
        return Err(TrailerValidationError::ShaNotAParent { sha: trailers.sha });
    }

    Ok(trailers)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "1111111111111111111111111111111111111111";
    const PARENT: &str = "2222222222222222222222222222222222222222";

    #[test]
    fn upstream_integration_formats_and_parses_trailers() {
        let message = format_upstream_merge_message("origin", "main", SHA);
        assert!(message.starts_with("Merge upstream: origin/main"));
        let parsed = parse_upstream_trailers(&message).unwrap();
        assert_eq!(
            parsed,
            UpstreamTrailers {
                remote: "origin".into(),
                branch: "main".into(),
                sha: SHA.into(),
            }
        );
    }

    #[test]
    fn upstream_integration_rejects_incomplete_or_malformed_trailers() {
        assert!(parse_upstream_trailers("Merge upstream: origin/main").is_none());
        assert!(parse_upstream_trailers(&format!(
            "subject\n\n{}: origin\n{}: main\n",
            TRAILER_REMOTE, TRAILER_BRANCH
        ))
        .is_none());
        // Short SHA is not identity evidence.
        assert!(parse_upstream_trailers(&format!(
            "subject\n\n{}: origin\n{}: main\n{}: 1111111\n",
            TRAILER_REMOTE, TRAILER_BRANCH, TRAILER_SHA
        ))
        .is_none());
        // Duplicated trailers cannot establish identity.
        let duplicated = format!(
            "{}{}: deadbeef\n",
            format_upstream_merge_message("origin", "main", SHA),
            TRAILER_SHA
        );
        assert!(parse_upstream_trailers(&duplicated).is_none());
    }

    #[test]
    fn upstream_integration_validates_trailers_against_parents() {
        let message = format_upstream_merge_message("origin", "main", SHA);
        let parents = vec![PARENT.to_string(), SHA.to_string()];
        assert_eq!(
            validate_upstream_merge(&message, &parents, "origin", "main").unwrap(),
            UpstreamTrailers {
                remote: "origin".into(),
                branch: "main".into(),
                sha: SHA.into(),
            }
        );
    }

    #[test]
    fn upstream_integration_rejects_contradicted_trailers() {
        let message = format_upstream_merge_message("origin", "main", SHA);
        let parents = vec![PARENT.to_string(), SHA.to_string()];

        assert_eq!(
            validate_upstream_merge(&message, &parents, "upstream", "main"),
            Err(TrailerValidationError::RemoteMismatch {
                expected: "upstream".into(),
                found: "origin".into()
            })
        );
        assert_eq!(
            validate_upstream_merge(&message, &parents, "origin", "develop"),
            Err(TrailerValidationError::BranchMismatch {
                expected: "develop".into(),
                found: "main".into()
            })
        );
        assert_eq!(
            validate_upstream_merge(&message, &[PARENT.to_string()], "origin", "main"),
            Err(TrailerValidationError::NotAMergeCommit)
        );
        assert_eq!(
            validate_upstream_merge(
                &message,
                &[PARENT.to_string(), PARENT.to_string()],
                "origin",
                "main"
            ),
            Err(TrailerValidationError::ShaNotAParent { sha: SHA.into() })
        );
    }
}
