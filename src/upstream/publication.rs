//! `Cflx-Publish-*` publication-required identity trailers.
//!
//! An opted-in cumulative-base integration is *not* terminal `merged`: it is a
//! local step that still owes a remote-confirmed publication. Process loss
//! between local integration and remote confirmation must therefore be
//! recoverable from the repository alone, without any journal, event, or
//! in-memory flag.
//!
//! These trailers are that evidence. They are recorded on a Conflux-owned marker
//! commit created immediately after the change merges into cumulative base, and
//! they bind three facts that recovery needs and cannot otherwise derive:
//!
//! - which change the integration is attributed to,
//! - which remote was selected,
//! - which same-name base branch must contain the published revision.
//!
//! They are deliberately distinct from [`super::trailers`]
//! (`Cflx-Upstream-*`), which identify a *fetched* revision integrated into
//! cumulative base. A run whose remote never advanced produces no upstream merge
//! at all, so upstream trailers cannot represent "this local integration still
//! owes a push". Ordinary disabled-mode merges carry neither marker and keep
//! their existing terminal `merged` meaning.

/// Change ID whose cumulative-base integration requires publication.
pub const TRAILER_PUBLISH_CHANGE: &str = "Cflx-Publish-Change";
/// Selected remote that must contain the published revision.
pub const TRAILER_PUBLISH_REMOTE: &str = "Cflx-Publish-Remote";
/// Same-name base branch that must contain the published revision.
pub const TRAILER_PUBLISH_BRANCH: &str = "Cflx-Publish-Branch";

/// Subject line of the publication-required marker commit.
pub const PUBLICATION_MARKER_SUBJECT_PREFIX: &str = "Publish required:";

/// Validated publication-required identity recovered from a marker commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationTrailers {
    pub change_id: String,
    pub remote: String,
    pub branch: String,
}

/// Build the marker commit message for an opted-in cumulative-base integration.
pub fn format_publication_marker_message(change_id: &str, remote: &str, branch: &str) -> String {
    format!(
        "{} {}\n\n{}: {}\n{}: {}\n{}: {}\n",
        PUBLICATION_MARKER_SUBJECT_PREFIX,
        change_id,
        TRAILER_PUBLISH_CHANGE,
        change_id,
        TRAILER_PUBLISH_REMOTE,
        remote,
        TRAILER_PUBLISH_BRANCH,
        branch
    )
}

/// Parse publication-required trailers from a raw commit message.
///
/// Returns `None` unless all three trailers are present exactly once with
/// non-empty values. Duplicated trailers are rejected for the same reason as in
/// [`super::trailers`]: a rewritten or hand-edited message cannot establish
/// identity.
pub fn parse_publication_trailers(commit_message: &str) -> Option<PublicationTrailers> {
    let mut change_id: Option<String> = None;
    let mut remote: Option<String> = None;
    let mut branch: Option<String> = None;

    for line in commit_message.lines() {
        let line = line.trim();
        let assign = |slot: &mut Option<String>, value: &str| -> bool {
            if slot.is_some() {
                return false;
            }
            *slot = Some(value.trim().to_string());
            true
        };

        if let Some(value) = line.strip_prefix(&format!("{}:", TRAILER_PUBLISH_CHANGE)) {
            if !assign(&mut change_id, value) {
                return None;
            }
        } else if let Some(value) = line.strip_prefix(&format!("{}:", TRAILER_PUBLISH_REMOTE)) {
            if !assign(&mut remote, value) {
                return None;
            }
        } else if let Some(value) = line.strip_prefix(&format!("{}:", TRAILER_PUBLISH_BRANCH)) {
            if !assign(&mut branch, value) {
                return None;
            }
        }
    }

    let (change_id, remote, branch) = (change_id?, remote?, branch?);
    if change_id.is_empty() || remote.is_empty() || branch.is_empty() {
        return None;
    }

    Some(PublicationTrailers {
        change_id,
        remote,
        branch,
    })
}

/// A publication-required integration recovered from cumulative history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationEvidence {
    /// SHA of the marker commit carrying the identity trailers.
    pub commit: String,
    pub trailers: PublicationTrailers,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_change_upstream_formats_and_parses_publication_trailers() {
        let message = format_publication_marker_message("alpha", "origin", "main");
        assert!(message.starts_with("Publish required: alpha"));
        assert_eq!(
            parse_publication_trailers(&message).unwrap(),
            PublicationTrailers {
                change_id: "alpha".into(),
                remote: "origin".into(),
                branch: "main".into(),
            }
        );
    }

    #[test]
    fn per_change_upstream_rejects_incomplete_publication_trailers() {
        assert!(parse_publication_trailers("Publish required: alpha").is_none());
        assert!(parse_publication_trailers(&format!(
            "subject\n\n{}: alpha\n{}: origin\n",
            TRAILER_PUBLISH_CHANGE, TRAILER_PUBLISH_REMOTE
        ))
        .is_none());
        assert!(parse_publication_trailers(&format!(
            "subject\n\n{}: \n{}: origin\n{}: main\n",
            TRAILER_PUBLISH_CHANGE, TRAILER_PUBLISH_REMOTE, TRAILER_PUBLISH_BRANCH
        ))
        .is_none());
    }

    #[test]
    fn per_change_upstream_rejects_duplicated_publication_trailers() {
        let duplicated = format!(
            "{}{}: beta\n",
            format_publication_marker_message("alpha", "origin", "main"),
            TRAILER_PUBLISH_CHANGE
        );
        assert!(parse_publication_trailers(&duplicated).is_none());
    }

    #[test]
    fn per_change_upstream_ignores_ordinary_merge_history() {
        // Disabled-mode cumulative merges carry no publication marker, so they
        // can never be recovered as publication work.
        assert!(parse_publication_trailers("Merge change: alpha").is_none());
        assert!(parse_publication_trailers(
            &crate::upstream::trailers::format_upstream_merge_message(
                "origin",
                "main",
                "1111111111111111111111111111111111111111"
            )
        )
        .is_none());
    }
}
