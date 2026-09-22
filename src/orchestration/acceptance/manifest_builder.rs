//! Deriving the Acceptance execution manifest from workspace evidence alone.
//!
//! Every input this module reads is one of the three the constitution allows as
//! workflow-control authority: workspace file state, workspace Git state, and
//! base-tree comparison. Nothing is read from `~/.local/state/cflx/**`, no
//! global setting participates, and no value is carried over from a previous
//! invocation. That is what makes the restart property hold: delete every
//! out-of-worktree file, rebuild from the same worktree, and the fingerprint,
//! the reuse decisions, and the retry verdict are the same.
//!
//! The one identity that is *not* a repository file — the embedded skill text
//! and the executable identity — is bound under the existing tool-identity
//! precedent the verification sidecar already set. It changes the fingerprint,
//! which is a conservative direction: a new binary or a new reviewer contract
//! makes a previously refused retry admissible, never the reverse.

use std::path::Path;

use chrono::Utc;

use super::execution_manifest::{
    digest_change_inputs, digest_text, eligible_gates, partition_deadline,
    AcceptanceExecutionManifest, ManifestGate, SkillIdentity, MANIFEST_SCHEMA,
};
use super::verification_evidence::{RepositoryFacts, ToolIdentity};

/// Why a manifest could not be derived at all.
///
/// Every variant is a runtime defect rather than a verdict: not being able to
/// read the repository says nothing about whether the change is correct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestBuildError {
    /// The change ID cannot name a repository-local sidecar.
    UnstorableChangeId(String),
    /// Git state could not be observed.
    Unobservable(String),
}

impl ManifestBuildError {
    pub fn detail(&self) -> String {
        match self {
            Self::UnstorableChangeId(id) => {
                format!("change id '{id}' cannot name a repository-local manifest")
            }
            Self::Unobservable(error) => {
                format!("current repository state could not be proven: {error}")
            }
        }
    }
}

/// Canonical change inputs digested into the manifest, in repository order.
///
/// The constitution is included deliberately: it outranks the proposal, so a
/// constitutional amendment genuinely changes what Acceptance is judging
/// against, and must produce a different fingerprint.
fn collect_change_inputs(workspace: &Path, change_id: &str) -> Vec<(String, Vec<u8>)> {
    let mut inputs = Vec::new();
    let change_dir = workspace.join("openspec/changes").join(change_id);
    fn push(inputs: &mut Vec<(String, Vec<u8>)>, relative: String, absolute: &Path) {
        if let Ok(bytes) = std::fs::read(absolute) {
            inputs.push((relative, bytes));
        }
    }
    for name in ["proposal.md", "design.md", "tasks.md", "tasks.json"] {
        push(
            &mut inputs,
            format!("openspec/changes/{change_id}/{name}"),
            &change_dir.join(name),
        );
    }
    // Spec deltas: walked rather than globbed so ordering comes from the digest's
    // own sort rather than from filesystem iteration order.
    let specs_root = change_dir.join("specs");
    let mut stack = vec![specs_root.clone()];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(relative) = path.strip_prefix(workspace) {
                let relative = relative.to_string_lossy().to_string();
                if let Ok(bytes) = std::fs::read(&path) {
                    inputs.push((relative, bytes));
                }
            }
        }
    }
    push(
        &mut inputs,
        "openspec/CONSTITUTION.md".to_string(),
        &workspace.join("openspec/CONSTITUTION.md"),
    );
    inputs
}

/// Identity of the running Conflux executable.
///
/// Best effort by construction: the crate version is always available, and the
/// executable's own path is added when the OS reports one. A value that cannot
/// be observed is simply absent rather than guessed, because a guessed identity
/// would bind the manifest to a binary that never ran.
fn executable_identity() -> String {
    let version = env!("CARGO_PKG_VERSION");
    match std::env::current_exe() {
        Ok(path) => digest_text(&format!("{version}:{}", path.to_string_lossy())),
        Err(_) => digest_text(version),
    }
}

/// Whether this workspace is a Git working tree at all.
///
/// The distinction this draws is load-bearing. A repository whose facts cannot
/// be read is a runtime defect and becomes a typed hold. A directory that was
/// never a repository is not broken — it simply has no candidate revision to
/// freeze, no review range to bind, and no evidence to compare — so Acceptance
/// runs there exactly as it did before the boundary existed, with no manifest
/// supplied. Collapsing the two would turn every non-repository Acceptance into
/// a permanent hold nobody can clear.
pub async fn workspace_is_repository(workspace: &Path) -> bool {
    tokio::process::Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(workspace)
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .is_ok_and(|output| output.status.success())
}

/// Build the frozen Acceptance execution manifest for one change.
///
/// `review_base` is the resolved base revision for the candidate diff, when one
/// exists. It is bound rather than recomputed later so a base branch that moves
/// mid-review cannot silently change what "the review range" meant.
pub async fn build_manifest<F: RepositoryFacts>(
    facts: &F,
    workspace: &Path,
    change_id: &str,
    declarations: &[crate::openspec::VerificationDeclaration],
    skill_name: &str,
    review_base: Option<&str>,
    absolute_deadline_secs: u64,
) -> Result<AcceptanceExecutionManifest, ManifestBuildError> {
    use super::execution_manifest::is_storable_change_id;

    if !is_storable_change_id(change_id) {
        return Err(ManifestBuildError::UnstorableChangeId(
            change_id.to_string(),
        ));
    }
    let candidate_commit_oid = facts
        .head_commit(workspace)
        .await
        .map_err(ManifestBuildError::Unobservable)?;
    let candidate_tree_oid = facts
        .head_tree(workspace)
        .await
        .map_err(ManifestBuildError::Unobservable)?;

    // A base that cannot be resolved leaves the range empty rather than
    // inventing one: an unresolvable base is a real state (a first commit, a
    // detached candidate), and a fabricated range would bind evidence to a
    // comparison nobody performed.
    let review_base_commit = match review_base {
        Some(base) => facts
            .resolve_revision(workspace, base)
            .await
            .unwrap_or_default(),
        None => String::new(),
    };
    let review_range = if review_base_commit.is_empty() {
        String::new()
    } else {
        format!("{review_base_commit}..{candidate_commit_oid}")
    };

    let mut gates = Vec::new();
    for request in eligible_gates(declarations) {
        let automation_blob_oid = facts
            .tracked_blob_oid(workspace, &request.automation_path)
            .await
            .unwrap_or_default();
        let tool = facts
            .resolve_tool(workspace, request.argv.first().map_or("", String::as_str))
            .await
            .unwrap_or_else(|_| ToolIdentity::default());
        let artifact_path = super::verification_evidence::EvidenceStore::artifact_relative_path(
            &request.verification_id,
        );
        let artifact_digest = facts
            .hash_file(workspace, &workspace.join(&artifact_path))
            .await
            .ok();
        gates.push(ManifestGate {
            verification_id: request.verification_id,
            argv: request.argv,
            cwd: request.cwd,
            automation_path: request.automation_path,
            automation_blob_oid,
            tool,
            artifact_path,
            artifact_digest,
        });
    }

    let skill = SkillIdentity {
        name: skill_name.to_string(),
        digest: crate::embedded_skills::embedded_acceptance_skill_text(skill_name)
            .map(digest_text)
            .unwrap_or_default(),
    };
    let partition = partition_deadline(absolute_deadline_secs);

    Ok(AcceptanceExecutionManifest {
        schema: MANIFEST_SCHEMA.to_string(),
        change_id: change_id.to_string(),
        candidate_commit_oid,
        candidate_tree_oid,
        review_base_commit,
        review_range,
        review_base_ref: review_base.map(str::to_string),
        change_input_digest: digest_change_inputs(&collect_change_inputs(workspace, change_id)),
        skill,
        executable_identity: executable_identity(),
        gates,
        created_at: Utc::now(),
        absolute_deadline_secs: partition.absolute_secs,
        work_budget_secs: partition.work_secs,
        terminal: None,
    })
}

/// Build the manifest for a change from its on-disk proposal declarations.
///
/// Convenience wrapper used by the executor and by the admission guard, so both
/// derive the same fingerprint from the same files rather than from two
/// independently assembled input sets.
pub async fn build_manifest_for_workspace<F: RepositoryFacts>(
    facts: &F,
    workspace: &Path,
    change_id: &str,
    skill_name: &str,
    review_base: Option<&str>,
    absolute_deadline_secs: u64,
) -> Result<AcceptanceExecutionManifest, ManifestBuildError> {
    let proposal_path = workspace
        .join("openspec/changes")
        .join(change_id)
        .join("proposal.md");
    let declarations = if proposal_path.is_file() {
        crate::openspec::parse_proposal_metadata_from_file(&proposal_path).verifications
    } else {
        Vec::new()
    };
    build_manifest(
        facts,
        workspace,
        change_id,
        &declarations,
        skill_name,
        review_base,
        absolute_deadline_secs,
    )
    .await
}

#[cfg(test)]
#[path = "manifest_builder/tests.rs"]
mod tests;
