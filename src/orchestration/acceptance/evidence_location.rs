//! Where Acceptance evidence lives — which, after constitutional law 4, is
//! never inside the thing being reviewed.
//!
//! An earlier release wrote manifests, envelopes, and gate logs into
//! `.cflx/verification-evidence/` inside each managed worktree and then excluded
//! that directory from every cleanliness check so the writes would not make the
//! target look dirty. The constitution names that arrangement directly and
//! forbids it: a Conflux-owned runtime artifact may not live in a target
//! repository or worktree, and hiding one with an ignore file or a status
//! exception does not make the write permissible.
//!
//! This module is the single place the replacement location is decided.
//! Everything about it is deliberate:
//!
//! * **One resolver, one layout.** Every writer — manifest, envelope, artifact,
//!   diagnostics — is handed a directory produced here, and neither
//!   [`EvidenceStore`] nor [`ManifestStore`] has a constructor that could derive
//!   a path from a workspace. "Acceptance never writes into a target" is
//!   therefore a property of the types rather than a rule callers must remember.
//! * **It fails closed.** An unresolvable, uncreatable, or unwritable root, and
//!   a root that resolves inside the target repository or a managed worktree,
//!   all produce a typed runtime hold *before* any declared gate is launched.
//!   There is no target-local fallback, because a fallback is exactly the
//!   pollution being removed.
//! * **Losing it costs work, never correctness.** What lives here is cache and
//!   diagnostics. A deleted store means the next attempt reruns its gates; it
//!   never implies PASS, never refuses admission, and never selects the next
//!   workflow action. Constitutional law 1 is satisfied by construction: the
//!   workflow decision is recomputed from the workspace whether this directory
//!   exists or not.
//!
//! The legacy path is handled by *not* handling it. [`legacy_target_evidence`]
//! probes for its existence and nothing more: no open, no parse, no migration,
//! no rewrite, no delete. Cleaning it up is a repository owner's explicit
//! decision, and Conflux refuses to proceed rather than making that decision for
//! them.

use std::path::{Path, PathBuf};

use crate::config::defaults::{
    acceptance_root_path, acceptance_store_path_in, ensure_acceptance_store_usable,
    AcceptanceStoreError,
};

use super::execution_manifest::ManifestStore;
use super::verification_evidence::{legacy_target_evidence, EvidenceStore};

tokio::task_local! {
    /// Test-only task-local override for the Acceptance evidence root.
    ///
    /// Scoped to the calling task rather than set as a process environment
    /// variable, for the same reason the review-budget override is: the suite
    /// runs cases concurrently, and a `set_var` here would move the store out
    /// from under whatever else is mid-invocation.
    pub(crate) static ACCEPTANCE_ROOT_OVERRIDE: PathBuf;
}

/// Run `fut` with the Acceptance evidence root overridden. Test-only.
#[cfg(test)]
pub(crate) async fn scoped_acceptance_root_for_test<F, R>(root: PathBuf, fut: F) -> R
where
    F: std::future::Future<Output = R>,
{
    ACCEPTANCE_ROOT_OVERRIDE.scope(root, fut).await
}

/// The Acceptance evidence root this invocation resolves.
fn resolved_acceptance_root(state_base_dir: Option<&str>) -> Result<PathBuf, AcceptanceStoreError> {
    if let Ok(root) = ACCEPTANCE_ROOT_OVERRIDE.try_with(Clone::clone) {
        return Ok(root);
    }
    Ok(acceptance_root_path(state_base_dir)?)
}

/// Resolve one change's external store directory without creating it.
///
/// The single resolution point every caller shares — the executor that writes,
/// the admission port that recomputes a fingerprint, and settlement cleanup that
/// deletes. Two callers resolving independently would eventually disagree, and a
/// fingerprint recomputed against a *different* store would compare artifact
/// digests that were never written by the same run.
pub fn store_path(
    state_base_dir: Option<&str>,
    project_root: &Path,
    workspace: &Path,
    change_id: &str,
) -> Result<PathBuf, AcceptanceStoreError> {
    acceptance_store_path_in(
        &resolved_acceptance_root(state_base_dir)?,
        project_root,
        workspace,
        change_id,
    )
}

/// The Conflux-owned external directory holding one change's Acceptance cache.
///
/// Layout, per the design:
///
/// ```text
/// <conflux-state-root>/acceptance/<project-slug>/<workspace-slug>/<change-id>/
///   manifest.json
///   diagnostics.json
///   gates/
///     <verification-id>.json
///     <verification-id>.log
/// ```
#[derive(Debug, Clone)]
pub struct AcceptanceStore {
    root: PathBuf,
}

impl AcceptanceStore {
    /// Resolve, validate, and create the store for one change.
    ///
    /// `project_root` is the repository the work belongs to and `workspace` the
    /// managed worktree the gates will run in; they are frequently the same
    /// directory (a main-worktree run, `cflx openspec verify`) and frequently
    /// not (a parallel managed worktree). Both are checked for containment,
    /// because either one being polluted is the failure this change exists to
    /// prevent.
    pub fn resolve(
        state_base_dir: Option<&str>,
        project_root: &Path,
        workspace: &Path,
        change_id: &str,
    ) -> Result<Self, AcceptanceStoreError> {
        let root = store_path(state_base_dir, project_root, workspace, change_id)?;
        ensure_acceptance_store_usable(&root)?;
        Ok(Self { root })
    }

    /// Bind to an already-resolved directory without creating anything.
    ///
    /// For readers — operator inspection, tests — that must not have the side
    /// effect of materializing a store for a change that has none.
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The external directory this store owns.
    pub fn root(&self) -> &Path {
        self.root.as_path()
    }

    /// Envelope and artifact storage for this change's declared gates.
    pub fn evidence(&self) -> EvidenceStore {
        EvidenceStore::new(self.root.clone())
    }

    /// Manifest and diagnostics storage for this change.
    pub fn manifests(&self) -> ManifestStore {
        ManifestStore::new(self.root.clone())
    }

    /// Discard this change's cache.
    ///
    /// Safe to call at any settlement point precisely because the contents are
    /// not authority: the worst consequence is that a later attempt reruns a
    /// gate it could have reused. Errors are ignored for the same reason — a
    /// cache that could not be deleted is a cache, not a workflow fault.
    pub fn clear(&self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Why Acceptance cannot proceed against this target's filesystem state.
#[derive(Debug)]
pub enum EvidenceLocationRefusal {
    /// The external store could not be resolved, created, or written, or it
    /// resolved inside the target.
    Store(AcceptanceStoreError),
    /// The target still carries legacy Conflux-owned evidence.
    LegacyTargetEvidence(PathBuf),
}

impl EvidenceLocationRefusal {
    /// The typed hold category this refusal settles as.
    pub fn category(&self) -> super::execution_manifest::AcceptanceHoldCategory {
        use super::execution_manifest::AcceptanceHoldCategory;
        match self {
            Self::Store(_) => AcceptanceHoldCategory::StatePathUnavailable,
            Self::LegacyTargetEvidence(_) => AcceptanceHoldCategory::LegacyTargetEvidence,
        }
    }

    /// One concrete evidence line naming exactly what to look at.
    pub fn evidence(&self) -> String {
        match self {
            Self::Store(error) => format!("{}: {error}", error.code()),
            Self::LegacyTargetEvidence(path) => format!(
                "legacy Conflux-owned evidence exists at '{}'; it was not opened, parsed, \
                 migrated, rewritten, or deleted",
                path.display()
            ),
        }
    }
}

/// Prepare the external store for one change, refusing before any gate runs.
///
/// Order matters. The legacy probe comes first: a target that still holds
/// Conflux-owned files from an earlier release is a repository-owner decision
/// nobody should be able to paper over by fixing an unrelated state root. Only
/// then is the external store resolved and proven writable, and only then may a
/// declared command be launched.
pub fn prepare_store(
    state_base_dir: Option<&str>,
    project_root: &Path,
    workspace: &Path,
    change_id: &str,
) -> Result<AcceptanceStore, EvidenceLocationRefusal> {
    if let Some(path) = legacy_target_evidence(workspace) {
        return Err(EvidenceLocationRefusal::LegacyTargetEvidence(path));
    }
    AcceptanceStore::resolve(state_base_dir, project_root, workspace, change_id)
        .map_err(EvidenceLocationRefusal::Store)
}

#[cfg(test)]
#[path = "evidence_location/tests.rs"]
mod tests;
