//! Repository evidence for terminal completion.
//!
//! The owner's snapshot is presentation: `display_status: merged` says what the
//! owner believes it did, and a change vanishing from the snapshot says only
//! that it stopped being tracked. Neither is proof. This module answers the
//! question a caller actually asked — "is the work in the repository?" — from
//! current Git state alone, and it reuses the same base-tree oracle the
//! orchestrator's own target resolution uses, so a client and a run cannot
//! disagree about what "completed" means.
//!
//! Every command here is read-only: `rev-parse`, `ls-tree`, and `ls-remote`.
//! Nothing fetches, writes a ref, or touches the working tree.

use std::path::Path;

use tokio::process::Command;

use crate::execution::state::{classify_base_completion, BaseCompletionEvidence};
use crate::web::remote_control_api::dto::{OwnerExecutionContract, TerminalMode};

/// What current repository state says about one change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Repository evidence proves the owner's terminal mode was reached.
    Completed { evidence: String },
    /// Repository evidence does not (yet) prove it. Keep observing.
    NotCompleted { detail: String },
    /// The evidence is contradictory or unreadable, so nothing can be proven.
    ///
    /// Deliberately not folded into `NotCompleted`: "not finished" invites
    /// waiting, while a broken proof needs a human.
    Broken { detail: String },
    /// The contract names a mode or a field this build cannot verify.
    Unsupported { detail: String },
}

/// Verify one change against the owner's declared terminal mode.
pub async fn verify(
    change_id: &str,
    repo_root: &Path,
    contract: &OwnerExecutionContract,
) -> Verdict {
    match contract.terminal_mode {
        TerminalMode::Merged => verify_base(change_id, repo_root, &contract.base_branch).await,
        TerminalMode::BasePublished => {
            let Some(remote) = contract.remote.as_deref() else {
                return Verdict::Unsupported {
                    detail: "the owner declared base publication but named no remote".to_string(),
                };
            };
            match verify_base(change_id, repo_root, &contract.base_branch).await {
                Verdict::Completed { evidence } => {
                    match remote_matches_local(repo_root, remote, &contract.base_branch).await {
                        Ok(Some(oid)) => Verdict::Completed {
                            evidence: format!(
                                "{evidence}; {remote}/{} published at {oid}",
                                contract.base_branch
                            ),
                        },
                        Ok(None) => Verdict::NotCompleted {
                            detail: format!(
                                "'{}' is integrated locally but {remote}/{} does not yet match the \
                                 local base tip",
                                change_id, contract.base_branch
                            ),
                        },
                        Err(detail) => Verdict::Broken { detail },
                    }
                }
                other => other,
            }
        }
        TerminalMode::BranchPushed => {
            let (Some(remote), Some(branch)) = (
                contract.remote.as_deref(),
                contract.pushed_branch.as_deref(),
            ) else {
                return Verdict::Unsupported {
                    detail: "the owner declared branch publication but named no remote or branch"
                        .to_string(),
                };
            };
            // The change branch, not base: `branch_pushed` proves publication,
            // and claiming base integration from it would be exactly the false
            // success this mode exists to avoid.
            match verify_base(change_id, repo_root, branch).await {
                Verdict::Completed { evidence } => {
                    match remote_matches_local(repo_root, remote, branch).await {
                        Ok(Some(oid)) => Verdict::Completed {
                            evidence: format!(
                                "{evidence}; {remote}/{branch} published at {oid} (branch \
                                 publication, not base integration)"
                            ),
                        },
                        Ok(None) => Verdict::NotCompleted {
                            detail: format!(
                                "'{branch}' carries the archived proposal but {remote}/{branch} \
                                 does not yet match its local tip"
                            ),
                        },
                        Err(detail) => Verdict::Broken { detail },
                    }
                }
                other => other,
            }
        }
    }
}

/// Project the shared base-completion oracle onto a client verdict.
///
/// The four-way distinction is preserved rather than collapsed: a missing
/// branch and a contradictory tree are different problems, and only one of them
/// is worth waiting through.
async fn verify_base(change_id: &str, repo_root: &Path, branch: &str) -> Verdict {
    match classify_base_completion(change_id, repo_root, branch).await {
        BaseCompletionEvidence::Completed => Verdict::Completed {
            evidence: format!(
                "'{branch}' holds the archived '{change_id}' entry and no active change directory"
            ),
        },
        BaseCompletionEvidence::NotCompleted => Verdict::NotCompleted {
            detail: format!("'{branch}' holds no archive entry for '{change_id}'"),
        },
        BaseCompletionEvidence::Contradictory { detail } => Verdict::Broken { detail },
        BaseCompletionEvidence::EvidenceError { detail, .. } => Verdict::Broken { detail },
    }
}

/// Whether the remote ref already equals the locally verified branch tip.
///
/// `ls-remote` rather than a cached remote-tracking ref: a stale
/// `refs/remotes/<remote>/<branch>` would let a never-pushed branch read as
/// published. Returns the shared OID when they match, `None` when they do not,
/// and an error only when the comparison could not be made at all.
async fn remote_matches_local(
    repo_root: &Path,
    remote: &str,
    branch: &str,
) -> Result<Option<String>, String> {
    let local = rev_parse(repo_root, branch)
        .await?
        .ok_or_else(|| format!("local branch '{branch}' could not be resolved"))?;

    let output = Command::new("git")
        .args(["ls-remote", "--exit-code", "--heads", remote, branch])
        .current_dir(repo_root)
        .output()
        .await
        .map_err(|error| format!("failed to run git ls-remote for '{remote}': {error}"))?;

    // Exit code 2 is `--exit-code`'s "no matching refs", which is an ordinary
    // "not published yet" rather than a broken comparison.
    if !output.status.success() {
        if output.status.code() == Some(2) {
            return Ok(None);
        }
        return Err(format!(
            "git ls-remote {remote} {branch} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let remote_oid = stdout
        .lines()
        .find_map(|line| parse_ls_remote_line(line, branch));
    Ok(match remote_oid {
        Some(remote_oid) if remote_oid == local => Some(local),
        _ => None,
    })
}

/// Extract the OID from one `git ls-remote` line naming exactly `branch`.
///
/// The suffix is matched against the full `refs/heads/<branch>` form so a
/// branch named `feature` cannot be satisfied by `refs/heads/other/feature`.
pub fn parse_ls_remote_line(line: &str, branch: &str) -> Option<String> {
    let (oid, reference) = line.split_once('\t')?;
    if reference.trim() != format!("refs/heads/{branch}") {
        return None;
    }
    let oid = oid.trim();
    if oid.is_empty() {
        return None;
    }
    Some(oid.to_string())
}

/// Resolve a revision to an OID, or `None` when it does not exist.
async fn rev_parse(repo_root: &Path, revision: &str) -> Result<Option<String>, String> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", &format!("{revision}^{{commit}}")])
        .current_dir(repo_root)
        .output()
        .await
        .map_err(|error| format!("failed to run git rev-parse for '{revision}': {error}"))?;
    if !output.status.success() {
        return Ok(None);
    }
    let oid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(if oid.is_empty() { None } else { Some(oid) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ls_remote_line_matches_only_the_exact_branch_ref() {
        let line = "1111111111111111111111111111111111111111\trefs/heads/alpha";
        assert_eq!(
            parse_ls_remote_line(line, "alpha").as_deref(),
            Some("1111111111111111111111111111111111111111")
        );
        // A prefix or suffix collision must not satisfy the check.
        assert_eq!(parse_ls_remote_line(line, "pha"), None);
        assert_eq!(
            parse_ls_remote_line(
                "2222222222222222222222222222222222222222\trefs/heads/team/alpha",
                "alpha"
            ),
            None
        );
        assert_eq!(
            parse_ls_remote_line(
                "3333333333333333333333333333333333333333\trefs/tags/alpha",
                "alpha"
            ),
            None
        );
        assert_eq!(parse_ls_remote_line("garbage", "alpha"), None);
    }

    #[test]
    fn a_contract_without_its_publication_identity_is_unsupported_not_completed() {
        let contract = OwnerExecutionContract {
            base_branch: "main".to_string(),
            terminal_mode: TerminalMode::BasePublished,
            remote: None,
            pushed_branch: None,
        };
        let verdict = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(verify("alpha", Path::new("/nonexistent"), &contract));
        assert!(
            matches!(verdict, Verdict::Unsupported { .. }),
            "{verdict:?}"
        );
    }

    #[test]
    fn branch_publication_without_a_branch_is_unsupported() {
        let contract = OwnerExecutionContract {
            base_branch: "main".to_string(),
            terminal_mode: TerminalMode::BranchPushed,
            remote: Some("origin".to_string()),
            pushed_branch: None,
        };
        let verdict = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(verify("alpha", Path::new("/nonexistent"), &contract));
        assert!(
            matches!(verdict, Verdict::Unsupported { .. }),
            "{verdict:?}"
        );
    }
}
