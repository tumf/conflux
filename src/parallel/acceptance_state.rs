//! Acceptance retry and stall state.
//!
//! Ordinary acceptance retry bookkeeping (cycle count, previous finding
//! identities, semantic baseline) lives in memory for the duration of one
//! orchestration run only. Conflux deliberately persists no acceptance
//! checkpoint: after a restart, complete but unarchived work is accepted again
//! rather than trusting an inferred prior PASS.
//!
//! A *validated* external blocker is a temporary hold that lives in the
//! in-memory `OrchestratorState` for the lifetime of one process. Per
//! constitutional law 1 nothing about it is written outside the managed
//! worktree, so a restart drops the hold and complete unarchived work is
//! accepted again rather than inferred to have passed.
//!
//! Acceptance no longer writes `APPLY_BLOCKED/marker.md`. Apply-origin and
//! unknown-origin markers keep their existing conservative handling.
//!
//! [`AcceptanceStallRecord`], [`AcceptanceStallStore`], and
//! [`migrate_legacy_acceptance_marker`] are the retired disk-persistence types.
//! No production path reads or writes them; they are scheduled for deletion
//! after one release cycle.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[allow(dead_code)] // Retired disk persistence; kept for one release cycle (see module docs).
static WRITE_NONCE: AtomicU64 = AtomicU64::new(0);

use crate::error::{OrchestratorError, Result};

const BLOCKED_MARKER_FILE: &str = "APPLY_BLOCKED/marker.md";
const MARKER_VERSION: &str = "acceptance-stalled-v1";

/// Schema version for the out-of-worktree acceptance stall record.
///
/// Reading a record with any other schema is a hard refusal: the record is
/// quarantined and cannot control routing.
#[allow(dead_code)] // Retired disk persistence; kept for one release cycle (see module docs).
pub const ACCEPTANCE_STALL_SCHEMA: &str = "acceptance-stall-v1";

/// In-memory acceptance retry context for one active orchestration run.
///
/// This is never serialized to a generated checkpoint file. It is discarded on
/// process restart, which forces a fresh acceptance sequence instead of an
/// inferred prior verdict.
///
/// Retry comparison state and the complete latest payload live in separate
/// fields.
///
/// `finding_identities` and `semantic_fingerprint` are comparison data only.
/// `findings` is the complete actionable payload the next Apply needs. Updating
/// the comparison fields can never replace the payload, which is what the
/// previous single-slot design allowed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AcceptanceRetryContext {
    pub finding_identities: Vec<String>,
    pub semantic_fingerprint: Option<String>,
    pub cycle_count: u32,
    /// Complete latest findings reported by the most recent canonical FAIL.
    pub findings: Vec<crate::acceptance::AcceptanceFinding>,
    /// Per-finding automatic repair accounting for this active run.
    pub repair_ledger: crate::orchestration::acceptance::FindingRepairLedger,
    /// Revision observed when the latest FAIL was recorded, used as the base of
    /// the remediation coverage delta.
    pub fail_revision: Option<String>,
}

impl AcceptanceRetryContext {
    pub fn previous_identities(&self) -> &[String] {
        &self.finding_identities
    }

    pub fn previous_fingerprint(&self) -> Option<&str> {
        self.semantic_fingerprint.as_deref()
    }

    /// Complete latest actionable findings.
    pub fn latest_findings(&self) -> &[crate::acceptance::AcceptanceFinding] {
        &self.findings
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockedMarkerOrigin {
    Apply,
    Acceptance,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockedMarker {
    pub origin: BlockedMarkerOrigin,
    pub reason: String,
    pub phase: String,
    pub evidence: Vec<String>,
    pub finding_identities: Vec<String>,
    pub retry_count: u32,
    pub semantic_fingerprint: Option<String>,
    pub semantic_progress: String,
    pub external_blockers: Vec<String>,
    pub resumable: bool,
    pub next_action: String,
    pub worktree_preserved: bool,
}

#[derive(Serialize, Deserialize)]
struct AcceptanceMarkerDocument {
    schema: String,
    #[serde(flatten)]
    marker: BlockedMarker,
}

fn change_dir(workspace_path: &Path, change_id: &str) -> PathBuf {
    workspace_path.join("openspec/changes").join(change_id)
}

fn marker_path(workspace_path: &Path, change_id: &str) -> PathBuf {
    change_dir(workspace_path, change_id).join(BLOCKED_MARKER_FILE)
}

#[allow(dead_code)] // Retired disk persistence; kept for one release cycle (see module docs).
fn atomic_write(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        OrchestratorError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "state path has no parent",
        ))
    })?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".{}.tmp-{}-{}",
        path.file_name().unwrap().to_string_lossy(),
        std::process::id(),
        WRITE_NONCE.fetch_add(1, Ordering::Relaxed),
    ));
    fs::write(&temporary, contents)?;
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        OrchestratorError::Io(error)
    })
}

/// Test-only helper that fabricates a legacy acceptance-origin marker.
///
/// Production acceptance never writes a marker any more; this exists so
/// migration and conservative-preservation regressions can build realistic
/// legacy fixtures.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub fn write_legacy_acceptance_marker(
    workspace_path: &Path,
    change_id: &str,
    reason: &str,
    evidence: &[String],
    retry: &AcceptanceRetryContext,
    semantic_progress: &str,
    external_blockers: &[String],
    resumable: bool,
    next_action: &str,
) -> Result<()> {
    let document = AcceptanceMarkerDocument {
        schema: MARKER_VERSION.to_string(),
        marker: BlockedMarker {
            origin: BlockedMarkerOrigin::Acceptance,
            reason: reason.to_string(),
            phase: "acceptance".to_string(),
            evidence: evidence.to_vec(),
            finding_identities: retry.finding_identities.clone(),
            retry_count: retry.cycle_count,
            semantic_fingerprint: retry.semantic_fingerprint.clone(),
            semantic_progress: semantic_progress.to_string(),
            external_blockers: external_blockers.to_vec(),
            resumable,
            next_action: next_action.to_string(),
            worktree_preserved: true,
        },
    };
    atomic_write(
        &marker_path(workspace_path, change_id),
        &serde_json::to_vec_pretty(&document)?,
    )
}

pub fn parse_blocked_marker(
    workspace_path: &Path,
    change_id: &str,
) -> Result<Option<BlockedMarker>> {
    let path = marker_path(workspace_path, change_id);
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)?;
    if content.trim_start().starts_with('{') {
        let document: AcceptanceMarkerDocument = serde_json::from_str(&content)?;
        if document.schema != MARKER_VERSION {
            return Err(OrchestratorError::AgentCommand(
                "acceptance marker has unsupported schema".to_string(),
            ));
        }
        return Ok(Some(document.marker));
    }

    let origin = if content.lines().any(|line| line.trim() == "origin: apply") {
        BlockedMarkerOrigin::Apply
    } else {
        BlockedMarkerOrigin::Unknown
    };
    let reason = content
        .lines()
        .find_map(|line| line.trim().strip_prefix("reason: ").map(str::to_string))
        .unwrap_or_else(|| "legacy blocked marker".to_string());
    Ok(Some(BlockedMarker {
        origin,
        reason,
        phase: "unknown".to_string(),
        evidence: content
            .lines()
            .filter_map(|line| line.strip_prefix("- ").map(str::to_string))
            .collect(),
        finding_identities: Vec::new(),
        retry_count: 0,
        semantic_fingerprint: None,
        semantic_progress: "unknown".to_string(),
        external_blockers: Vec::new(),
        resumable: false,
        next_action: "preserve marker and inspect evidence".to_string(),
        worktree_preserved: true,
    }))
}

/// Retired versioned, revision-bound acceptance stall record.
///
/// No production path constructs, reads, or writes this any more: the stall
/// hold is in-memory reducer state. Kept for one release cycle so an operator
/// can still inspect leftover files under `~/.local/state/cflx/acceptance-stalls/`.
#[allow(dead_code)] // Retired disk persistence; kept for one release cycle (see module docs).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceStallRecord {
    /// Schema version; must equal [`ACCEPTANCE_STALL_SCHEMA`].
    pub schema: String,
    /// Identity of the repository that owns the change.
    pub repository_id: String,
    pub change_id: String,
    /// Stable identity of the managed worktree (see [`worktree_identity`]).
    pub worktree_id: String,
    /// Canonical worktree path at the time the hold was recorded.
    pub worktree_path: String,
    /// Branch checked out in the worktree, when resolvable.
    pub branch: Option<String>,
    /// Apply revision the hold is bound to.
    pub apply_revision: String,
    /// Phase that stalled. Always `acceptance` for records written here.
    pub phase: String,
    /// Acceptance retry count observed when the hold was recorded.
    pub retry_count: u32,
    /// Explicit blocker category supplied by the reviewer.
    pub category: String,
    /// Concrete blocker evidence.
    pub evidence: Vec<String>,
    /// Whether acceptance can resume once the prerequisite is satisfied.
    pub resumable: bool,
    /// Operator-facing unblock action.
    pub next_action: String,
    /// Optional owning team/role for the prerequisite.
    pub prerequisite_owner: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[allow(dead_code)] // Retired disk persistence; kept for one release cycle (see module docs).
impl AcceptanceStallRecord {
    /// Build a record from a validated blocker and the current repository facts.
    pub fn new(
        facts: &WorkspaceFacts,
        blocker: &crate::acceptance::AcceptanceBlocker,
        apply_revision: &str,
        retry_count: u32,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            schema: ACCEPTANCE_STALL_SCHEMA.to_string(),
            repository_id: facts.repository_id.clone(),
            change_id: facts.change_id.clone(),
            worktree_id: facts.worktree_id.clone(),
            worktree_path: facts.worktree_path.clone(),
            branch: facts.branch.clone(),
            apply_revision: apply_revision.to_string(),
            phase: "acceptance".to_string(),
            retry_count,
            category: blocker.category.clone(),
            evidence: blocker.evidence.clone(),
            resumable: blocker.resumable,
            next_action: blocker.next_action.clone(),
            prerequisite_owner: blocker.prerequisite_owner.clone(),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Operator-facing blocker view for lifecycle events and status displays.
    ///
    /// The category is copied verbatim; it is never re-derived from prose.
    pub fn to_stalled_blocker(&self) -> crate::events::StalledBlocker {
        crate::events::StalledBlocker {
            category: self.category.clone(),
            phase: self.phase.clone(),
            gate: "acceptance".to_string(),
            error_summary: format!(
                "validated external acceptance blocker ({}): {}",
                self.category,
                self.evidence.join(" | ")
            ),
            evidence: self.evidence.clone(),
            next_action: self.next_action.clone(),
            resumable: self.resumable,
            worktree_preserved: true,
        }
    }
}

/// Current repository/worktree facts describing a managed workspace.
#[allow(dead_code)] // Retired disk persistence; kept for one release cycle (see module docs).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkspaceFacts {
    pub repository_id: String,
    pub change_id: String,
    pub worktree_id: String,
    pub worktree_path: String,
    pub branch: Option<String>,
    /// Whether the stored apply revision still exists in the worktree history.
    pub apply_revision_exists: bool,
    /// Whether current HEAD equals or descends from the stored apply revision.
    pub head_descends_from_apply_revision: bool,
    /// Whether the change is still active (not archived, not merged).
    pub change_active: bool,
}

/// Stable identity for a repository, used as the storage key for the retired
/// stall store.
pub fn repository_identity(repo_root: &Path) -> String {
    fs::canonicalize(repo_root)
        .unwrap_or_else(|_| repo_root.to_path_buf())
        .to_string_lossy()
        .to_string()
}

/// Stable identity for a managed worktree.
///
/// For a linked git worktree this is the `gitdir:` pointer written into
/// `<worktree>/.git`, which changes when the worktree is recreated under a
/// different name.
pub fn worktree_identity(worktree_path: &Path) -> String {
    let git_pointer = worktree_path.join(".git");
    match fs::read_to_string(&git_pointer) {
        Ok(contents) => contents.trim().to_string(),
        // A non-linked worktree (plain clone) has `.git` as a directory.
        Err(_) => format!("gitdir: {}", git_pointer.to_string_lossy()),
    }
}

/// Retired versioned, atomic store for acceptance stall records.
///
/// No production path opens this store any more. It is kept for one release
/// cycle alongside [`AcceptanceStallRecord`] so leftover files under
/// `~/.local/state/cflx/acceptance-stalls/` remain readable by an operator; they
/// are deliberately never loaded and never deleted, so a concurrently running
/// older Conflux sharing the same state directory keeps its own holds.
#[allow(dead_code)] // Retired disk persistence; kept for one release cycle (see module docs).
#[derive(Debug, Clone)]
pub struct AcceptanceStallStore {
    root: PathBuf,
}

#[allow(dead_code)] // Retired disk persistence; kept for one release cycle (see module docs).
impl AcceptanceStallStore {
    /// Build a store rooted at an explicit directory (used by tests).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Build a store rooted at Conflux's XDG state area.
    pub fn discover() -> Result<Self> {
        let base = match std::env::var_os("XDG_STATE_HOME") {
            Some(value) if !value.is_empty() => PathBuf::from(value),
            _ => dirs::home_dir()
                .ok_or_else(|| {
                    OrchestratorError::Io(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "cannot determine home directory for acceptance stall state",
                    ))
                })?
                .join(".local/state"),
        };
        Ok(Self::new(base.join("cflx/acceptance-stalls")))
    }

    /// Storage path for one repository + change hold.
    ///
    /// Crate-visible so reconciliation tests can plant a record whose *content*
    /// disagrees with the key it is stored under.
    pub(crate) fn record_path(&self, repository_id: &str, change_id: &str) -> PathBuf {
        self.root
            .join(stable_key(repository_id))
            .join(format!("{}.json", stable_key(change_id)))
    }

    /// Atomically create or update the hold for `record`'s repository + change,
    /// returning exactly what was stored.
    ///
    /// An existing record's `created_at` is preserved so retry counts and hold
    /// age stay meaningful across repeated observations of the same blocker.
    pub fn save(&self, record: &AcceptanceStallRecord) -> Result<AcceptanceStallRecord> {
        let path = self.record_path(&record.repository_id, &record.change_id);
        let mut record = record.clone();
        if let Ok(Some(existing)) = self.read_raw(&path) {
            record.created_at = existing.created_at;
        }
        record.updated_at = chrono::Utc::now().to_rfc3339();
        atomic_write(&path, &serde_json::to_vec_pretty(&record)?)?;
        Ok(record)
    }

    /// Load the hold for a repository + change.
    ///
    /// Corrupt or unsupported-schema records are quarantined and reported as
    /// absent: a record the runtime cannot understand must never control
    /// dispatch.
    pub fn load(
        &self,
        repository_id: &str,
        change_id: &str,
    ) -> Result<Option<AcceptanceStallRecord>> {
        let path = self.record_path(repository_id, change_id);
        match self.read_raw(&path) {
            Ok(record) => Ok(record),
            Err(error) => {
                self.quarantine_path(&path, &error.to_string())?;
                Ok(None)
            }
        }
    }

    fn read_raw(&self, path: &Path) -> Result<Option<AcceptanceStallRecord>> {
        if !path.exists() {
            return Ok(None);
        }
        let contents = fs::read_to_string(path)?;
        let record: AcceptanceStallRecord = serde_json::from_str(&contents).map_err(|error| {
            OrchestratorError::AgentCommand(format!(
                "corrupt acceptance stall record at {}: {error}",
                path.display()
            ))
        })?;
        if record.schema != ACCEPTANCE_STALL_SCHEMA {
            return Err(OrchestratorError::AgentCommand(format!(
                "acceptance stall record at {} has unsupported schema '{}'",
                path.display(),
                record.schema
            )));
        }
        Ok(Some(record))
    }

    /// Remove the hold. Idempotent: consuming an absent record returns `false`
    /// rather than failing, so a retry that races a restart stays safe.
    pub fn consume(&self, repository_id: &str, change_id: &str) -> Result<bool> {
        let path = self.record_path(repository_id, change_id);
        if !path.exists() {
            return Ok(false);
        }
        fs::remove_file(&path)?;
        Ok(true)
    }

    /// Move an unusable record aside with a diagnostic so an operator can still
    /// inspect it, while guaranteeing it no longer controls routing.
    pub fn quarantine(&self, repository_id: &str, change_id: &str, reason: &str) -> Result<()> {
        let path = self.record_path(repository_id, change_id);
        self.quarantine_path(&path, reason)
    }

    fn quarantine_path(&self, path: &Path, reason: &str) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }
        let quarantine_dir = self.root.join("quarantine");
        fs::create_dir_all(&quarantine_dir)?;
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "record.json".to_string());
        let target = quarantine_dir.join(format!(
            "{}-{}-{}",
            std::process::id(),
            WRITE_NONCE.fetch_add(1, Ordering::Relaxed),
            name
        ));
        fs::rename(path, &target)?;
        fs::write(target.with_extension("reason.txt"), reason)?;
        Ok(())
    }
}

/// Map an arbitrary identity string to a filesystem-safe, stable key.
#[allow(dead_code)] // Retired disk persistence; kept for one release cycle (see module docs).
fn stable_key(value: &str) -> String {
    let hash = value
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325u64, |hash, byte| {
            (hash ^ *byte as u64).wrapping_mul(0x100000001b3)
        });
    let readable = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let readable = readable.trim_matches('-');
    let tail = readable
        .char_indices()
        .rev()
        .nth(39)
        .map(|(index, _)| &readable[index..])
        .unwrap_or(readable);
    format!("{tail}-{hash:016x}")
}

/// Outcome of the one-time legacy acceptance marker migration.
#[allow(dead_code)] // Retired disk persistence; kept for one release cycle (see module docs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkerMigration {
    /// No marker was present, or nothing needed to change.
    NotApplicable,
    /// A marker was preserved unchanged because it could not be proven to be a
    /// resumable, structurally valid, acceptance-owned hold.
    Preserved { reason: String },
    /// The marker was converted into a runtime stall record and removed.
    Migrated { category: String },
}

/// Migrate one legacy acceptance-origin `APPLY_BLOCKED/marker.md` into runtime
/// stall state.
///
/// Deliberately conservative. Migration happens only when the marker proves it
/// is acceptance-owned, resumable, and carries a usable blocker payload, and
/// only when the caller can bind it to the current repository, worktree, and
/// apply revision. Apply-origin, unknown-origin, non-resumable, and malformed
/// markers are preserved untouched — this function never deletes evidence it
/// cannot prove is acceptance-generated.
///
/// The runtime record is written *before* the marker is removed, so an
/// interruption can only leave a duplicate hold (idempotently re-migrated),
/// never a lost one.
#[allow(dead_code)] // Retired disk persistence; kept for one release cycle (see module docs).
pub fn migrate_legacy_acceptance_marker(
    store: &AcceptanceStallStore,
    workspace_path: &Path,
    facts: &WorkspaceFacts,
    apply_revision: &str,
) -> Result<MarkerMigration> {
    let path = marker_path(workspace_path, &facts.change_id);
    if !path.exists() {
        return Ok(MarkerMigration::NotApplicable);
    }

    // A marker we cannot parse is malformed: preserve it for human inspection.
    let marker = match parse_blocked_marker(workspace_path, &facts.change_id) {
        Ok(Some(marker)) => marker,
        Ok(None) => return Ok(MarkerMigration::NotApplicable),
        Err(error) => {
            return Ok(MarkerMigration::Preserved {
                reason: format!("marker is malformed and was left untouched: {error}"),
            })
        }
    };

    if !matches!(marker.origin, BlockedMarkerOrigin::Acceptance) {
        return Ok(MarkerMigration::Preserved {
            reason: format!(
                "marker origin {:?} is not acceptance-owned; conservative blocked handling retained",
                marker.origin
            ),
        });
    }
    if !marker.resumable {
        return Ok(MarkerMigration::Preserved {
            reason: "acceptance marker is not resumable; conservative blocked handling retained"
                .to_string(),
        });
    }

    // A tracked marker is committed repository content, not generated residue.
    // Deleting it would dirty the managed worktree and rewrite change artifacts,
    // so it is ambiguous by definition and stays untouched.
    if marker_is_tracked(workspace_path, &facts.change_id) {
        return Ok(MarkerMigration::Preserved {
            reason: "acceptance marker is tracked by git; removing it would dirty the managed \
                     worktree, so it is treated as ambiguous and preserved"
                .to_string(),
        });
    }

    let evidence = marker
        .evidence
        .iter()
        .chain(marker.external_blockers.iter())
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
        .collect::<Vec<_>>();
    if evidence.is_empty() {
        return Ok(MarkerMigration::Preserved {
            reason: "acceptance marker carries no concrete blocker evidence to migrate".to_string(),
        });
    }
    let next_action = marker.next_action.trim();
    if next_action.is_empty() {
        return Ok(MarkerMigration::Preserved {
            reason: "acceptance marker carries no next action to migrate".to_string(),
        });
    }

    // Legacy markers predate explicit categories. Rather than infer one from
    // prose, migrate under the neutral `pending_verification` category and keep
    // the original reason as the leading evidence line.
    let mut migrated_evidence = vec![format!(
        "legacy acceptance marker reason: {}",
        marker.reason
    )];
    migrated_evidence.extend(evidence);
    let blocker = crate::acceptance::AcceptanceBlocker {
        category: "pending_verification".to_string(),
        evidence: migrated_evidence,
        next_action: next_action.to_string(),
        resumable: true,
        prerequisite_owner: None,
        evidence_ids: Vec::new(),
    };

    let record = store.save(&AcceptanceStallRecord::new(
        facts,
        &blocker,
        apply_revision,
        marker.retry_count,
    ))?;

    // Only now remove the generated marker, and only its own directory.
    remove_generated_marker(workspace_path, &facts.change_id)?;
    Ok(MarkerMigration::Migrated {
        category: record.category,
    })
}

/// Whether git tracks the marker file at `workspace_path` for `change_id`.
///
/// A non-git directory, or a git invocation that cannot run at all, reports
/// "not tracked" only because the caller has already established that the marker
/// is untracked generated residue in that case; a *failed* lookup inside a real
/// repository is conservatively reported as tracked so migration refuses.
#[allow(dead_code)] // Retired disk persistence; kept for one release cycle (see module docs).
fn marker_is_tracked(workspace_path: &Path, change_id: &str) -> bool {
    let path = marker_path(workspace_path, change_id);
    let output = std::process::Command::new("git")
        .args(["ls-files", "--error-unmatch", "--"])
        .arg(&path)
        .current_dir(workspace_path)
        .output();
    match output {
        // Exit 0 means the path is in the index: tracked repository content.
        Ok(output) => output.status.success(),
        // git is unavailable: we cannot prove the marker is generated residue.
        Err(_) => true,
    }
}

/// Remove a generated `APPLY_BLOCKED` marker and the directory Conflux created
/// for it, leaving no residue behind. Any unexpected sibling file keeps the
/// directory, so nothing a human placed there is destroyed.
///
/// The removal is verified: `git status --porcelain` scoped to the marker
/// directory must come back empty, so a migration can never silently hand back a
/// dirty managed worktree.
#[allow(dead_code)] // Retired disk persistence; kept for one release cycle (see module docs).
fn remove_generated_marker(workspace_path: &Path, change_id: &str) -> Result<()> {
    let path = marker_path(workspace_path, change_id);
    if path.exists() {
        fs::remove_file(&path)?;
    }
    if let Some(parent) = path.parent() {
        if parent.exists() && fs::read_dir(parent)?.next().is_none() {
            fs::remove_dir(parent)?;
        }
    }
    verify_marker_removal_left_worktree_clean(workspace_path, &path)
}

/// Fail loudly when removing the generated marker left git-visible residue.
#[allow(dead_code)] // Retired disk persistence; kept for one release cycle (see module docs).
fn verify_marker_removal_left_worktree_clean(workspace_path: &Path, path: &Path) -> Result<()> {
    let Some(marker_dir) = path.parent() else {
        return Ok(());
    };
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain", "--"])
        .arg(marker_dir)
        .current_dir(workspace_path)
        .output();
    let Ok(output) = output else {
        // No git available to check against; the filesystem removal above is the
        // only guarantee we can offer.
        return Ok(());
    };
    if !output.status.success() {
        return Ok(());
    }
    let residue = String::from_utf8_lossy(&output.stdout);
    if residue.trim().is_empty() {
        return Ok(());
    }
    Err(OrchestratorError::Io(std::io::Error::other(format!(
        "removing the generated acceptance marker at {} left the worktree dirty: {}",
        path.display(),
        residue.trim()
    ))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn retry_context(
        identities: &[&str],
        fingerprint: Option<&str>,
        cycle_count: u32,
    ) -> AcceptanceRetryContext {
        AcceptanceRetryContext {
            finding_identities: identities.iter().map(|value| value.to_string()).collect(),
            semantic_fingerprint: fingerprint.map(str::to_string),
            cycle_count,
            ..AcceptanceRetryContext::default()
        }
    }

    fn blocker() -> crate::acceptance::AcceptanceBlocker {
        crate::acceptance::AcceptanceBlocker {
            category: "credential".to_string(),
            evidence: vec!["STAGING_API_KEY is unset".to_string()],
            next_action: "provision STAGING_API_KEY then retry acceptance".to_string(),
            resumable: true,
            prerequisite_owner: Some("platform".to_string()),
            evidence_ids: vec!["cred-1".to_string()],
        }
    }

    fn facts() -> WorkspaceFacts {
        WorkspaceFacts {
            repository_id: "/repo".to_string(),
            change_id: "change".to_string(),
            worktree_id: "gitdir: /repo/.git/worktrees/ws-change".to_string(),
            worktree_path: "/repo/../worktrees/ws-change".to_string(),
            branch: Some("ws-change".to_string()),
            apply_revision_exists: true,
            head_descends_from_apply_revision: true,
            change_active: true,
        }
    }

    fn record() -> AcceptanceStallRecord {
        AcceptanceStallRecord::new(&facts(), &blocker(), "revision-1", 2)
    }

    #[test]
    fn acceptance_state_store_round_trips_every_bound_field() {
        let temp = TempDir::new().unwrap();
        let store = AcceptanceStallStore::new(temp.path());
        let original = record();
        store.save(&original).unwrap();

        let loaded = store.load("/repo", "change").unwrap().unwrap();
        assert_eq!(loaded.schema, ACCEPTANCE_STALL_SCHEMA);
        assert_eq!(loaded.repository_id, "/repo");
        assert_eq!(loaded.change_id, "change");
        assert_eq!(loaded.worktree_id, original.worktree_id);
        assert_eq!(loaded.worktree_path, original.worktree_path);
        assert_eq!(loaded.branch.as_deref(), Some("ws-change"));
        assert_eq!(loaded.apply_revision, "revision-1");
        assert_eq!(loaded.phase, "acceptance");
        assert_eq!(loaded.retry_count, 2);
        assert_eq!(loaded.category, "credential");
        assert_eq!(loaded.evidence, ["STAGING_API_KEY is unset"]);
        assert!(loaded.resumable);
        assert_eq!(
            loaded.next_action,
            "provision STAGING_API_KEY then retry acceptance"
        );
        assert_eq!(loaded.prerequisite_owner.as_deref(), Some("platform"));
        assert!(!loaded.created_at.is_empty());
        assert!(!loaded.updated_at.is_empty());

        // Nothing is written into any worktree-shaped location.
        assert!(!temp.path().join("openspec").exists());
        assert!(!temp.path().join(".cflx").exists());
    }

    #[test]
    fn acceptance_state_store_isolates_repositories_and_changes() {
        let temp = TempDir::new().unwrap();
        let store = AcceptanceStallStore::new(temp.path());
        store.save(&record()).unwrap();
        store
            .save(&AcceptanceStallRecord {
                repository_id: "/other-repo".to_string(),
                ..record()
            })
            .unwrap();
        store
            .save(&AcceptanceStallRecord {
                change_id: "other-change".to_string(),
                ..record()
            })
            .unwrap();

        assert!(store.consume("/repo", "change").unwrap());
        assert!(store.load("/repo", "change").unwrap().is_none());
        assert!(store.load("/other-repo", "change").unwrap().is_some());
        assert!(store.load("/repo", "other-change").unwrap().is_some());
    }

    /// Re-observing the same blocker updates the hold in place and preserves the
    /// original creation time, so hold age stays meaningful.
    #[test]
    fn acceptance_state_save_is_idempotent_and_preserves_creation_time() {
        let temp = TempDir::new().unwrap();
        let store = AcceptanceStallStore::new(temp.path());
        let first = record();
        store.save(&first).unwrap();
        let stored_first = store.load("/repo", "change").unwrap().unwrap();

        let mut second = record();
        second.created_at = "1999-01-01T00:00:00Z".to_string();
        second.retry_count = 5;
        store.save(&second).unwrap();

        let stored_second = store.load("/repo", "change").unwrap().unwrap();
        assert_eq!(stored_second.created_at, stored_first.created_at);
        assert_eq!(stored_second.retry_count, 5);
    }

    /// Corrupt and unsupported-schema records are quarantined, never obeyed.
    #[test]
    fn acceptance_state_quarantines_corrupt_and_unsupported_records() {
        for (label, contents) in [
            ("corrupt", "{ not json".to_string()),
            (
                "unsupported schema",
                serde_json::to_string(&AcceptanceStallRecord {
                    schema: "acceptance-stall-v99".to_string(),
                    ..record()
                })
                .unwrap(),
            ),
        ] {
            let temp = TempDir::new().unwrap();
            let store = AcceptanceStallStore::new(temp.path());
            store.save(&record()).unwrap();
            let path = store.record_path("/repo", "change");
            fs::write(&path, &contents).unwrap();

            assert!(
                store.load("/repo", "change").unwrap().is_none(),
                "{label} record must not control routing"
            );
            assert!(!path.exists(), "{label} record must be moved aside");
            let quarantined = fs::read_dir(temp.path().join("quarantine"))
                .unwrap()
                .filter_map(std::result::Result::ok)
                .count();
            assert!(quarantined >= 2, "{label} record and its reason are kept");
        }
    }

    #[test]
    fn acceptance_state_consume_is_idempotent_and_quarantine_tolerates_absence() {
        let temp = TempDir::new().unwrap();
        let store = AcceptanceStallStore::new(temp.path());
        store.save(&record()).unwrap();

        assert!(store.consume("/repo", "change").unwrap());
        assert!(!store.consume("/repo", "change").unwrap());
        store.quarantine("/repo", "change", "already gone").unwrap();
    }

    /// A write failure must not leave a half-written hold behind.
    #[test]
    fn acceptance_state_write_failure_leaves_no_partial_record() {
        let temp = TempDir::new().unwrap();
        let store = AcceptanceStallStore::new(temp.path());
        let path = store.record_path("/repo", "change");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        // A directory where the record file belongs makes the rename fail.
        fs::create_dir_all(&path).unwrap();

        assert!(store.save(&record()).is_err());
        assert!(path.is_dir(), "the obstruction is left untouched");
        assert!(
            fs::read_dir(path.parent().unwrap())
                .unwrap()
                .filter_map(std::result::Result::ok)
                .all(|entry| !entry.file_name().to_string_lossy().contains(".tmp-")),
            "no temporary file may survive a failed write"
        );
    }

    /// The storage key must stay stable, filesystem-safe, and collision-free
    /// across identities that only differ deep inside a long path.
    #[test]
    fn acceptance_state_keys_are_stable_and_safe() {
        let long_a = "/Users/dev/very/long/path/to/repository/alpha";
        let long_b = "/Users/dev/very/long/path/to/repository/beta";
        assert_eq!(stable_key(long_a), stable_key(long_a));
        assert_ne!(stable_key(long_a), stable_key(long_b));
        for key in [stable_key(long_a), stable_key("weird/id with spaces")] {
            assert!(!key.contains('/'), "{key} must be a single path segment");
            assert!(!key.contains(' '), "{key} must not contain spaces");
            assert!(!key.is_empty());
        }
    }

    #[test]
    fn acceptance_state_record_exposes_the_explicit_category_verbatim() {
        let blocker = crate::acceptance::AcceptanceBlocker {
            category: "human_decision".to_string(),
            // Prose full of credential words must not change the category.
            evidence: vec!["needs a credential token auth decision from the owner".to_string()],
            next_action: "owner decides".to_string(),
            resumable: false,
            prerequisite_owner: None,
            evidence_ids: Vec::new(),
        };
        let record = AcceptanceStallRecord::new(&facts(), &blocker, "revision-1", 0);
        let displayed = record.to_stalled_blocker();

        assert_eq!(record.category, "human_decision");
        assert_eq!(displayed.category, "human_decision");
        assert_eq!(displayed.phase, "acceptance");
        assert_eq!(displayed.gate, "acceptance");
        assert!(!displayed.resumable);
        assert!(displayed.worktree_preserved);
        assert!(displayed.error_summary.contains("human_decision"));
    }

    #[test]
    fn legacy_marker_round_trip_preserves_context_and_foreign_markers() {
        let temp = TempDir::new().unwrap();
        write_legacy_acceptance_marker(
            temp.path(),
            "change",
            "blocked: details\nnext line",
            &["external: detail\n- nested".to_string()],
            &retry_context(
                &["repository|evidence|verification"],
                Some("semantic baseline"),
                2,
            ),
            "no_semantic_progress",
            &["recoverable verification blocker".to_string()],
            true,
            "explicit retry",
        )
        .unwrap();

        let marker = parse_blocked_marker(temp.path(), "change")
            .unwrap()
            .unwrap();
        assert_eq!(marker.origin, BlockedMarkerOrigin::Acceptance);
        assert_eq!(
            marker.finding_identities,
            ["repository|evidence|verification"]
        );
        assert_eq!(marker.retry_count, 2);
        assert_eq!(marker.evidence, ["external: detail\n- nested"]);
        assert_eq!(
            marker.external_blockers,
            ["recoverable verification blocker"]
        );
        assert!(marker.worktree_preserved);

        // Foreign markers keep their conservative classification.
        let path = marker_path(temp.path(), "change");
        fs::write(&path, "origin: apply\nreason: blocked\n").unwrap();
        assert_eq!(
            parse_blocked_marker(temp.path(), "change")
                .unwrap()
                .unwrap()
                .origin,
            BlockedMarkerOrigin::Apply
        );

        fs::write(&path, "{not json").unwrap();
        assert!(parse_blocked_marker(temp.path(), "change").is_err());
        assert!(path.exists(), "a malformed marker must never be consumed");
    }

    /// The removed writer must stay removed: acceptance has no production path
    /// that can create a marker under a change directory.
    #[test]
    fn acceptance_state_module_has_no_production_marker_writer() {
        let source = include_str!("acceptance_state.rs");
        // Split so this assertion's own literal cannot satisfy the search.
        let removed_writer = concat!("pub fn ", "write_acceptance_blocked_marker");
        assert_eq!(
            source.matches(removed_writer).count(),
            0,
            "acceptance must not expose a blocked-marker writer"
        );
        assert!(
            source.contains("#[cfg(test)]\n#[allow(clippy::too_many_arguments)]\npub fn write_legacy_acceptance_marker"),
            "the only marker writer left must be the test-only legacy fixture helper"
        );
    }
}
