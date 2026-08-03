//! Repository-derived classification of sequential resolve batch state.
//!
//! Sequential integration has two mutation locations — each change worktree and
//! the target repository — and both are owned by the resolve agent. This module
//! owns the other half: deciding, purely from Git evidence, what still has to
//! happen and whether the batch is truthfully complete.
//!
//! Everything here is side-effect free. No function commits, stages, or edits
//! anything; the worst outcome of a misclassification is a withheld action, not
//! a wrong mutation. Process-local workspace membership and
//! `workspace.base_revision` are never consulted: authority comes from the
//! ordered batch items supplied at merge admission plus worktree metadata,
//! refs, commit parentage, index stages, committed trees, and cleanliness.

use crate::archive_layout;
use crate::vcs::git::commands::{self as git_commands, CommitDiffEntry, WorktreeIdentity};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

/// Exact subject of a per-change final integration merge commit.
pub fn final_merge_subject(change_id: &str) -> String {
    format!("Merge change: {}", change_id)
}

/// Exact subject of a worktree pre-sync merge commit.
pub fn presync_subject(change_id: &str) -> String {
    format!("Pre-sync base into {}", change_id)
}

/// Exact subject of the forward post-final resurrection cleanup commit.
pub fn cleanup_subject(change_id: &str) -> String {
    format!("Cleanup resurrected change: {}", change_id)
}

/// Result of a single evidence read.
pub type EvidenceResult<T> = std::result::Result<T, String>;

/// Read-only repository evidence required to classify a sequential resolve batch.
///
/// Split out as a trait so the decision table can be exercised against
/// in-memory doubles as well as a real repository: the classifier holds all of
/// the policy, the adapter holds all of the Git access.
#[async_trait]
pub trait ResolveEvidence: Send + Sync {
    /// Validate a supplied archive worktree path against Git worktree metadata.
    async fn validate_worktree(
        &self,
        supplied_path: &Path,
        expected_branch: &str,
    ) -> WorktreeIdentity;

    /// Whether the given worktree has an unfinished merge.
    async fn worktree_merge_in_progress(&self, worktree: &Path) -> EvidenceResult<bool>;

    /// Unresolved conflict paths inside the given worktree.
    async fn worktree_conflicts(&self, worktree: &Path) -> EvidenceResult<Vec<String>>;

    /// Current target repository `HEAD` commit.
    async fn target_head(&self) -> EvidenceResult<String>;

    /// Target repository `MERGE_HEAD`, when a merge is in progress.
    async fn target_merge_head(&self) -> EvidenceResult<Option<String>>;

    /// Unmerged (stage 1/2/3) target index paths.
    async fn target_conflict_paths(&self) -> EvidenceResult<Vec<String>>;

    /// Whether the target index and worktree match `HEAD`, untracked files included.
    async fn target_is_clean(&self) -> EvidenceResult<bool>;

    /// Stage-0 target index paths under `prefix`.
    async fn target_index_paths(&self, prefix: &str) -> EvidenceResult<Vec<String>>;

    /// Complete ordered parent list of a commit.
    async fn parents_of(&self, commit: &str) -> EvidenceResult<Vec<String>>;

    /// Whether `ancestor` is reachable from `descendant`.
    async fn is_ancestor(&self, ancestor: &str, descendant: &str) -> EvidenceResult<bool>;

    /// First-parent lineage of `tip`, newest first.
    async fn first_parent_lineage(&self, tip: &str) -> EvidenceResult<Vec<String>>;

    /// Merge base of two revisions.
    async fn merge_base(&self, a: &str, b: &str) -> EvidenceResult<Option<String>>;

    /// Commits in `from..to` whose complete subject equals `subject`.
    async fn commits_with_exact_subject(
        &self,
        from: Option<&str>,
        to: &str,
        subject: &str,
    ) -> EvidenceResult<Vec<String>>;

    /// Complete tree diff of a commit against its first parent.
    async fn commit_diff_entries(&self, commit: &str) -> EvidenceResult<Vec<CommitDiffEntry>>;

    /// Committed tree paths under `prefix` at `revision`.
    async fn committed_tree_paths(
        &self,
        revision: &str,
        prefix: &str,
    ) -> EvidenceResult<Vec<String>>;
}

/// Closed classification of the batch, ordered by the action it requires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchState {
    /// Identity or topology could not be proven; no guidance and no mutation.
    UnsafeEvidence {
        /// Why the evidence is not trustworthy.
        reason: String,
    },
    /// A worktree still holds an unfinished pre-sync merge or conflicts.
    PreSyncUnfinished {
        /// Change awaiting the finished pre-sync.
        change_id: String,
        /// Validated worktree path.
        worktree: PathBuf,
        /// Concrete unfinished-state detail.
        reason: String,
    },
    /// Pre-sync evidence is missing, duplicated, or has the wrong topology.
    PreSyncInvalid {
        /// Change whose pre-sync is not provable.
        change_id: String,
        /// Validated worktree path.
        worktree: PathBuf,
        /// Required target state the pre-sync must contain.
        required_target_state: String,
        /// Concrete topology detail.
        reason: String,
    },
    /// An identity-verified final merge is in progress and must be committed.
    TargetMergeUnfinished {
        /// Change owning the in-progress target merge.
        change_id: String,
        /// Validated worktree path.
        worktree: PathBuf,
        /// Whether the merged index resurrected the live change directory.
        requires_live_removal: bool,
        /// Concrete state detail.
        reason: String,
    },
    /// The item is next in order and its final merge has not started.
    FinalMergeMissing {
        /// Change awaiting final integration.
        change_id: String,
        /// Validated worktree path.
        worktree: PathBuf,
        /// Required target state the merge's first parent must be.
        required_target_state: String,
    },
    /// Committed integration retains both active and archived identities.
    ResurrectionCleanupRequired {
        /// Change whose live directory was resurrected.
        change_id: String,
        /// Concrete evidence detail.
        reason: String,
    },
    /// Every item is integrated and the target repository is clean.
    Complete,
}

impl BatchState {
    /// Stable machine-readable phase name.
    pub fn phase(&self) -> &'static str {
        match self {
            Self::UnsafeEvidence { .. } => "unsafe_evidence",
            Self::PreSyncUnfinished { .. } => "presync_unfinished",
            Self::PreSyncInvalid { .. } => "presync_invalid",
            Self::TargetMergeUnfinished { .. } => "target_merge_unfinished",
            Self::FinalMergeMissing { .. } => "final_merge_missing",
            Self::ResurrectionCleanupRequired { .. } => "resurrection_cleanup_required",
            Self::Complete => "complete",
        }
    }

    /// Whether the batch is truthfully complete.
    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Complete)
    }

    /// Whether continuation may ask the agent to mutate the repository.
    ///
    /// `UnsafeEvidence` deliberately answers `false`: unproven identity must
    /// preserve state rather than trigger a blind commit.
    pub fn allows_agent_action(&self) -> bool {
        !matches!(self, Self::UnsafeEvidence { .. } | Self::Complete)
    }

    /// Structured, bounded phase diagnosis for resolve continuation context.
    ///
    /// Every field is bounded here, before assembly, so the fixed
    /// `<resolve_context>` wrapper plus the newest diagnosis alone can never
    /// exceed the resolve context byte budget.
    pub fn diagnosis(&self) -> String {
        /// Byte cap for one assembled diagnosis field.
        const FIELD_MAX_BYTES: usize = 384;

        let mut lines = vec![format!("phase: {}", self.phase())];
        match self {
            Self::UnsafeEvidence { reason } => {
                lines.push(format!("detail: {}", reason));
                lines.push(
                    "required_action: none; preserve repository state and do not commit"
                        .to_string(),
                );
            }
            Self::PreSyncUnfinished {
                change_id,
                worktree,
                reason,
            } => {
                lines.push(format!("change_id: {}", change_id));
                lines.push(format!("worktree: {}", worktree.display()));
                lines.push(format!("detail: {}", reason));
                lines.push(format!(
                    "required_action: finish the in-progress worktree merge and commit it with subject '{}'",
                    presync_subject(change_id)
                ));
            }
            Self::PreSyncInvalid {
                change_id,
                worktree,
                required_target_state,
                reason,
            } => {
                lines.push(format!("change_id: {}", change_id));
                lines.push(format!("worktree: {}", worktree.display()));
                lines.push(format!("required_target_state: {}", required_target_state));
                lines.push(format!("detail: {}", reason));
                lines.push(format!(
                    "required_action: in the worktree run 'git merge --no-ff -m \"{}\" {}' so the pre-sync commit has exactly two parents with non-first parent {}",
                    presync_subject(change_id),
                    required_target_state,
                    required_target_state
                ));
            }
            Self::TargetMergeUnfinished {
                change_id,
                worktree,
                requires_live_removal,
                reason,
            } => {
                lines.push(format!("change_id: {}", change_id));
                lines.push(format!("worktree: {}", worktree.display()));
                lines.push(format!("requires_live_removal: {}", requires_live_removal));
                lines.push(format!("detail: {}", reason));
                if *requires_live_removal {
                    lines.push(format!(
                        "required_action: run 'git rm -r --cached -f openspec/changes/{}' and remove the directory, then commit the in-progress merge with subject '{}'",
                        change_id,
                        final_merge_subject(change_id)
                    ));
                } else {
                    lines.push(format!(
                        "required_action: commit the in-progress target merge with subject '{}'",
                        final_merge_subject(change_id)
                    ));
                }
            }
            Self::FinalMergeMissing {
                change_id,
                worktree,
                required_target_state,
            } => {
                lines.push(format!("change_id: {}", change_id));
                lines.push(format!("worktree: {}", worktree.display()));
                lines.push(format!("required_target_state: {}", required_target_state));
                lines.push(format!(
                    "required_action: in the repo root run 'git merge --no-ff --no-commit <branch>' and commit with subject '{}'",
                    final_merge_subject(change_id)
                ));
            }
            Self::ResurrectionCleanupRequired { change_id, reason } => {
                lines.push(format!("change_id: {}", change_id));
                lines.push(format!("detail: {}", reason));
                lines.push(format!(
                    "required_action: remove only openspec/changes/{} and record a forward commit with subject '{}'; never amend or rewrite history",
                    change_id,
                    cleanup_subject(change_id)
                ));
            }
            Self::Complete => {
                lines.push(
                    "detail: every batch item is integrated and the target is clean".to_string(),
                );
                lines.push("required_action: none".to_string());
            }
        }
        lines
            .into_iter()
            .map(|line| crate::history::bounded_head(&line, FIELD_MAX_BYTES))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub use super::merge::SequentialMergeItem;

/// A batch item whose worktree identity has been proven.
#[derive(Debug, Clone)]
pub struct ValidatedItem {
    /// Change identifier.
    pub change_id: String,
    /// Workspace branch name.
    pub revision: String,
    /// Validated (possibly rediscovered) worktree path.
    pub worktree: PathBuf,
    /// Branch tip commit.
    pub tip: String,
    /// Branch base recorded at admission, when available.
    pub branch_base: Option<String>,
}

/// Per-item integration evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemIntegration {
    /// A unique exact final merge commit with proven topology.
    Exact {
        /// The final merge commit.
        commit: String,
        /// Required target state proven by the commit's first parent.
        target_state: String,
    },
    /// No exact candidate exists and the branch tip is already ancestral.
    Historical,
    /// The item still needs final integration.
    NotIntegrated,
}

fn unsafe_evidence(context: &str, error: String) -> BatchState {
    BatchState::UnsafeEvidence {
        reason: format!("{}: {}", context, error),
    }
}

/// Validate every batch item's worktree identity, in declared order.
pub async fn validate_items(
    evidence: &dyn ResolveEvidence,
    items: &[SequentialMergeItem],
) -> std::result::Result<Vec<ValidatedItem>, BatchState> {
    let mut validated = Vec::with_capacity(items.len());
    for item in items {
        match evidence
            .validate_worktree(&item.archive_path, &item.revision)
            .await
        {
            WorktreeIdentity::Supplied { path, tip }
            | WorktreeIdentity::Rediscovered { path, tip } => {
                validated.push(ValidatedItem {
                    change_id: item.change_id.clone(),
                    revision: item.revision.clone(),
                    worktree: path,
                    tip,
                    branch_base: item.branch_base.clone(),
                });
            }
            WorktreeIdentity::Unsafe { reason } => {
                return Err(BatchState::UnsafeEvidence { reason });
            }
        }
    }
    Ok(validated)
}

/// Classify one item's committed integration evidence.
///
/// This is the single final-integration policy shared by resolve retry
/// continuation and terminal merge verification, so the two can never diverge.
pub async fn classify_item_integration(
    evidence: &dyn ResolveEvidence,
    item: &ValidatedItem,
    base_revision: &str,
    target_head: &str,
) -> std::result::Result<ItemIntegration, BatchState> {
    let subject = final_merge_subject(&item.change_id);
    let candidates = evidence
        .commits_with_exact_subject(Some(base_revision), target_head, &subject)
        .await
        .map_err(|error| unsafe_evidence("Failed to enumerate final merge candidates", error))?;

    match candidates.len() {
        0 => {
            let integrated =
                evidence
                    .is_ancestor(&item.tip, target_head)
                    .await
                    .map_err(|error| {
                        unsafe_evidence("Failed to check historical integration", error)
                    })?;
            if integrated {
                Ok(ItemIntegration::Historical)
            } else {
                Ok(ItemIntegration::NotIntegrated)
            }
        }
        1 => {
            let commit = candidates[0].clone();
            let parents = evidence
                .parents_of(&commit)
                .await
                .map_err(|error| unsafe_evidence("Failed to read merge parents", error))?;
            if parents.len() != 2 {
                return Err(BatchState::UnsafeEvidence {
                    reason: format!(
                        "Final merge commit {} for '{}' has {} parent(s); exactly two are required",
                        commit,
                        item.change_id,
                        parents.len()
                    ),
                });
            }
            if parents[1] != item.tip {
                return Err(BatchState::UnsafeEvidence {
                    reason: format!(
                        "Final merge commit {} for '{}' has non-first parent {} but branch '{}' tip is {}",
                        commit, item.change_id, parents[1], item.revision, item.tip
                    ),
                });
            }
            let contained = evidence
                .is_ancestor(&commit, target_head)
                .await
                .map_err(|error| unsafe_evidence("Failed to check merge containment", error))?;
            if !contained {
                return Err(BatchState::UnsafeEvidence {
                    reason: format!(
                        "Final merge commit {} for '{}' is not contained by target HEAD",
                        commit, item.change_id
                    ),
                });
            }
            Ok(ItemIntegration::Exact {
                commit,
                target_state: parents[0].clone(),
            })
        }
        count => Err(BatchState::UnsafeEvidence {
            reason: format!(
                "Found {} commits with subject '{}'; exactly one is required",
                count, subject
            ),
        }),
    }
}

/// Validate pre-sync topology of one item against required target state `T`.
///
/// Returns `Ok(None)` when pre-sync is provable.
async fn validate_presync(
    evidence: &dyn ResolveEvidence,
    item: &ValidatedItem,
    required_target_state: &str,
) -> std::result::Result<Option<BatchState>, BatchState> {
    let lineage = evidence
        .first_parent_lineage(&item.tip)
        .await
        .map_err(|error| unsafe_evidence("Failed to read worktree first-parent lineage", error))?;
    if lineage.iter().any(|commit| commit == required_target_state) {
        return Ok(None);
    }

    let subject = presync_subject(&item.change_id);
    let from = match &item.branch_base {
        Some(base) => Some(base.clone()),
        None => evidence
            .merge_base(required_target_state, &item.tip)
            .await
            .map_err(|error| unsafe_evidence("Failed to compute pre-sync search base", error))?,
    };
    let candidates = evidence
        .commits_with_exact_subject(from.as_deref(), &item.tip, &subject)
        .await
        .map_err(|error| unsafe_evidence("Failed to enumerate pre-sync candidates", error))?;

    let invalid = |reason: String| {
        Ok(Some(BatchState::PreSyncInvalid {
            change_id: item.change_id.clone(),
            worktree: item.worktree.clone(),
            required_target_state: required_target_state.to_string(),
            reason,
        }))
    };

    match candidates.len() {
        0 => invalid(format!(
            "Required target state {} is not on branch '{}' first-parent lineage and no '{}' commit exists",
            required_target_state, item.revision, subject
        )),
        1 => {
            let commit = &candidates[0];
            let parents = evidence
                .parents_of(commit)
                .await
                .map_err(|error| unsafe_evidence("Failed to read pre-sync parents", error))?;
            if parents.len() != 2 {
                return invalid(format!(
                    "Pre-sync commit {} has {} parent(s); exactly two are required",
                    commit,
                    parents.len()
                ));
            }
            if parents[1] != required_target_state {
                return invalid(format!(
                    "Pre-sync commit {} has non-first parent {} but required target state is {}",
                    commit, parents[1], required_target_state
                ));
            }
            let contained = evidence
                .is_ancestor(commit, &item.tip)
                .await
                .map_err(|error| unsafe_evidence("Failed to check pre-sync containment", error))?;
            if !contained {
                return invalid(format!(
                    "Pre-sync commit {} is not contained by branch '{}' tip {}",
                    commit, item.revision, item.tip
                ));
            }
            Ok(None)
        }
        count => invalid(format!(
            "Found {} commits with subject '{}'; exactly one is required",
            count, subject
        )),
    }
}

/// Validate any existing forward cleanup commit for the change.
///
/// Reachability from target `HEAD` is not enough: a cleanup commit authored on a
/// side branch and merged in later is reachable while the target's own history
/// never moved forward past the resurrection. The commit must therefore sit on
/// the target first-parent lineage and its single parent must be the target
/// state that actually held the live/archive coexistence, at or after this
/// item's committed integration.
async fn validate_cleanup_commits(
    evidence: &dyn ResolveEvidence,
    item: &ValidatedItem,
    integration: &ItemIntegration,
    base_revision: &str,
    target_head: &str,
    target_lineage: &[String],
) -> std::result::Result<(), BatchState> {
    let change_id = item.change_id.as_str();
    let subject = cleanup_subject(change_id);
    let candidates = evidence
        .commits_with_exact_subject(Some(base_revision), target_head, &subject)
        .await
        .map_err(|error| unsafe_evidence("Failed to enumerate cleanup candidates", error))?;

    let commit = match candidates.as_slice() {
        [] => return Ok(()),
        [commit] => commit.clone(),
        _ => {
            return Err(BatchState::UnsafeEvidence {
                reason: format!(
                    "Found {} commits with subject '{}'; exactly one is allowed",
                    candidates.len(),
                    subject
                ),
            })
        }
    };

    if !target_lineage.iter().any(|candidate| candidate == &commit) {
        return Err(BatchState::UnsafeEvidence {
            reason: format!(
                "Cleanup commit {} for '{}' is not on target HEAD {} first-parent lineage; only forward cleanup on the target is accepted",
                commit, change_id, target_head
            ),
        });
    }

    let parents = evidence
        .parents_of(&commit)
        .await
        .map_err(|error| unsafe_evidence("Failed to read cleanup commit parents", error))?;
    if parents.len() != 1 {
        return Err(BatchState::UnsafeEvidence {
            reason: format!(
                "Cleanup commit {} has {} parent(s); a forward cleanup commit has exactly one",
                commit,
                parents.len()
            ),
        });
    }
    let predecessor = parents[0].clone();

    let entries = evidence
        .commit_diff_entries(&commit)
        .await
        .map_err(|error| unsafe_evidence("Failed to read cleanup commit diff", error))?;
    if entries.is_empty() {
        return Err(BatchState::UnsafeEvidence {
            reason: format!("Cleanup commit {} changes nothing", commit),
        });
    }
    for entry in &entries {
        if entry.status != 'D' || !archive_layout::is_active_change_path(&entry.path, change_id) {
            return Err(BatchState::UnsafeEvidence {
                reason: format!(
                    "Cleanup commit {} has entry '{}{}'; it may only delete openspec/changes/{}",
                    commit, entry.status, entry.path, change_id
                ),
            });
        }
    }

    // Bind the predecessor to this item's committed integration state.
    let integrated_at = match integration {
        ItemIntegration::Exact { commit, .. } => commit.clone(),
        ItemIntegration::Historical => item.tip.clone(),
        ItemIntegration::NotIntegrated => {
            return Err(BatchState::UnsafeEvidence {
                reason: format!(
                    "Cleanup commit {} exists for '{}' but the change has no committed integration",
                    commit, change_id
                ),
            })
        }
    };
    let after_integration = evidence
        .is_ancestor(&integrated_at, &predecessor)
        .await
        .map_err(|error| unsafe_evidence("Failed to order cleanup against integration", error))?;
    if !after_integration {
        return Err(BatchState::UnsafeEvidence {
            reason: format!(
                "Cleanup commit {} for '{}' has predecessor {} which does not contain its committed integration {}",
                commit, change_id, predecessor, integrated_at
            ),
        });
    }

    let predecessor_paths = evidence
        .committed_tree_paths(&predecessor, archive_layout::ACTIVE_CHANGES_PREFIX)
        .await
        .map_err(|error| unsafe_evidence("Failed to read cleanup predecessor tree", error))?;
    if !archive_layout::paths_contain_active_change(&predecessor_paths, change_id)
        || !archive_layout::paths_contain_valid_archive(&predecessor_paths, change_id)
    {
        return Err(BatchState::UnsafeEvidence {
            reason: format!(
                "Cleanup commit {} for '{}' has predecessor {} which does not hold the live/archive coexistence it claims to remove",
                commit, change_id, predecessor
            ),
        });
    }

    Ok(())
}

/// Classify the complete ordered batch from repository evidence.
pub async fn classify_batch(
    evidence: &dyn ResolveEvidence,
    items: &[SequentialMergeItem],
    base_revision: &str,
) -> BatchState {
    match classify_batch_inner(evidence, items, base_revision).await {
        Ok(state) | Err(state) => state,
    }
}

async fn classify_batch_inner(
    evidence: &dyn ResolveEvidence,
    items: &[SequentialMergeItem],
    base_revision: &str,
) -> std::result::Result<BatchState, BatchState> {
    if items.is_empty() {
        return Err(BatchState::UnsafeEvidence {
            reason: "Sequential resolve received an empty batch".to_string(),
        });
    }

    let validated = validate_items(evidence, items).await?;

    let target_head = evidence
        .target_head()
        .await
        .map_err(|error| unsafe_evidence("Failed to read target HEAD", error))?;
    let merge_head = evidence
        .target_merge_head()
        .await
        .map_err(|error| unsafe_evidence("Failed to read target MERGE_HEAD", error))?;

    let mut integrations = Vec::with_capacity(validated.len());
    for item in &validated {
        integrations
            .push(classify_item_integration(evidence, item, base_revision, &target_head).await?);
    }

    // Global target merge ownership is decided before any per-item guidance so
    // an in-progress merge can never be attributed to the wrong change.
    let owner = match merge_head.as_deref() {
        Some(merge_head) => {
            let matched: Vec<usize> = validated
                .iter()
                .enumerate()
                .filter(|(_, item)| item.tip == merge_head)
                .map(|(index, _)| index)
                .collect();
            match matched.as_slice() {
                [index] => Some(*index),
                [] => {
                    return Err(BatchState::UnsafeEvidence {
                        reason: format!(
                            "Target MERGE_HEAD {} does not match any batch branch tip",
                            merge_head
                        ),
                    })
                }
                _ => {
                    return Err(BatchState::UnsafeEvidence {
                        reason: format!(
                            "Target MERGE_HEAD {} matches {} batch branch tips",
                            merge_head,
                            matched.len()
                        ),
                    })
                }
            }
        }
        None => None,
    };

    let first_incomplete = integrations
        .iter()
        .position(|integration| matches!(integration, ItemIntegration::NotIntegrated));

    if let Some(first) = first_incomplete {
        if let Some((offset, _)) = integrations
            .iter()
            .enumerate()
            .skip(first + 1)
            .find(|(_, integration)| !matches!(integration, ItemIntegration::NotIntegrated))
        {
            return Err(BatchState::UnsafeEvidence {
                reason: format!(
                    "Item '{}' is integrated while earlier item '{}' is not; declared order was violated",
                    validated[offset].change_id, validated[first].change_id
                ),
            });
        }
    }

    if let Some(owner) = owner {
        if first_incomplete != Some(owner) {
            return Err(BatchState::UnsafeEvidence {
                reason: format!(
                    "Target MERGE_HEAD owner '{}' is not the first incomplete batch item",
                    validated[owner].change_id
                ),
            });
        }
    }

    // Declared order must be provable from the target's own forward history, not
    // only from the presence of a hole. When every item already has an exact
    // integration commit there is no `NotIntegrated` marker left, so a history
    // that integrated B before A has to be rejected by chaining the exact
    // commits along the target first-parent lineage.
    let target_lineage = evidence
        .first_parent_lineage(&target_head)
        .await
        .map_err(|error| unsafe_evidence("Failed to read target first-parent lineage", error))?;

    let mut previous: Option<(&str, String, usize)> = None;
    for (item, integration) in validated.iter().zip(integrations.iter()) {
        // Historical ancestry-only evidence has no exact commit to place on the
        // lineage, and is exempt from the ordering proof by design.
        let ItemIntegration::Exact {
            commit,
            target_state,
        } = integration
        else {
            continue;
        };
        let Some(position) = target_lineage
            .iter()
            .position(|candidate| candidate == commit)
        else {
            return Err(BatchState::UnsafeEvidence {
                reason: format!(
                    "Final merge commit {} for '{}' is not on target HEAD {} first-parent lineage",
                    commit, item.change_id, target_head
                ),
            });
        };
        if let Some((previous_id, previous_commit, previous_position)) = &previous {
            // The lineage is newest first, so each later declared item must sit
            // strictly closer to target HEAD than the item before it.
            if position >= *previous_position {
                return Err(BatchState::UnsafeEvidence {
                    reason: format!(
                        "Item '{}' was integrated before earlier item '{}' on the target first-parent history; declared order was violated",
                        item.change_id, previous_id
                    ),
                });
            }
            let chained = evidence
                .is_ancestor(previous_commit, target_state)
                .await
                .map_err(|error| {
                    unsafe_evidence("Failed to chain declared-order integrations", error)
                })?;
            if !chained {
                return Err(BatchState::UnsafeEvidence {
                    reason: format!(
                        "Final merge commit {} for '{}' does not build on the committed integration {} of earlier item '{}'; declared order was violated",
                        commit, item.change_id, previous_commit, previous_id
                    ),
                });
            }
        }
        previous = Some((item.change_id.as_str(), commit.clone(), position));
    }

    // Committed items: prove the pre-sync topology their merge claims.
    for (item, integration) in validated.iter().zip(integrations.iter()) {
        match integration {
            ItemIntegration::Exact { target_state, .. } => {
                if let Some(state) = validate_presync(evidence, item, target_state).await? {
                    return Ok(state);
                }
            }
            // Historical ancestry-only integration has no exact final commit
            // from which to reconstruct `T`, so pre-sync reconstruction is
            // exempt; archive/live and clean-target invariants still apply.
            ItemIntegration::Historical => {}
            ItemIntegration::NotIntegrated => break,
        }
    }

    if let Some(index) = first_incomplete {
        let item = &validated[index];

        if evidence
            .worktree_merge_in_progress(&item.worktree)
            .await
            .map_err(|error| unsafe_evidence("Failed to read worktree merge state", error))?
        {
            return Ok(BatchState::PreSyncUnfinished {
                change_id: item.change_id.clone(),
                worktree: item.worktree.clone(),
                reason: "Worktree merge is in progress (MERGE_HEAD exists)".to_string(),
            });
        }

        let worktree_conflicts = evidence
            .worktree_conflicts(&item.worktree)
            .await
            .map_err(|error| unsafe_evidence("Failed to read worktree conflicts", error))?;
        if !worktree_conflicts.is_empty() {
            return Ok(BatchState::PreSyncUnfinished {
                change_id: item.change_id.clone(),
                worktree: item.worktree.clone(),
                reason: format!(
                    "Worktree has unresolved conflicts: {}",
                    worktree_conflicts.join(", ")
                ),
            });
        }

        // Final merge has not been committed, so required target state is the
        // current cumulative target HEAD (every prior item is already complete).
        if let Some(state) = validate_presync(evidence, item, &target_head).await? {
            return Ok(state);
        }

        let target_conflicts = evidence
            .target_conflict_paths()
            .await
            .map_err(|error| unsafe_evidence("Failed to read target index stages", error))?;

        if owner == Some(index) {
            if !target_conflicts.is_empty() {
                return Ok(BatchState::TargetMergeUnfinished {
                    change_id: item.change_id.clone(),
                    worktree: item.worktree.clone(),
                    requires_live_removal: false,
                    reason: format!(
                        "Target index still has conflict stages: {}",
                        target_conflicts.join(", ")
                    ),
                });
            }

            let index_paths = evidence
                .target_index_paths(archive_layout::ACTIVE_CHANGES_PREFIX)
                .await
                .map_err(|error| unsafe_evidence("Failed to read target index entries", error))?;
            let live = archive_layout::paths_contain_active_change(&index_paths, &item.change_id);
            let archived =
                archive_layout::paths_contain_valid_archive(&index_paths, &item.change_id);

            return Ok(BatchState::TargetMergeUnfinished {
                change_id: item.change_id.clone(),
                worktree: item.worktree.clone(),
                requires_live_removal: live && archived,
                reason: if live && archived {
                    format!(
                        "Merged stage-0 index holds both openspec/changes/{} and its valid archive entry",
                        item.change_id
                    )
                } else {
                    "Target merge is conflict-free and awaiting its commit".to_string()
                },
            });
        }

        if !target_conflicts.is_empty() {
            return Err(BatchState::UnsafeEvidence {
                reason: format!(
                    "Target index has conflict stages without MERGE_HEAD: {}",
                    target_conflicts.join(", ")
                ),
            });
        }

        // Archive evidence before the final merge starts comes from the
        // validated worktree tip's committed tree — not the filesystem, and not
        // the target index, which has not seen this branch yet. The final merge
        // integrates exactly this commit, so an unarchived or invalidly archived
        // tip must never be allowed to start it.
        let worktree_tree_paths = evidence
            .committed_tree_paths(&item.tip, archive_layout::ACTIVE_CHANGES_PREFIX)
            .await
            .map_err(|error| {
                unsafe_evidence("Failed to read validated worktree committed tree", error)
            })?;
        if !archive_layout::paths_contain_valid_archive(&worktree_tree_paths, &item.change_id) {
            let nested = archive_layout::paths_contain_invalid_nested_archive(
                &worktree_tree_paths,
                &item.change_id,
            );
            return Err(BatchState::UnsafeEvidence {
                reason: if nested {
                    format!(
                        "Branch '{}' tip {} archives '{}' under an invalid nested layout; expected {}/YYYY-MM-DD-{}/proposal.md",
                        item.revision,
                        item.tip,
                        item.change_id,
                        archive_layout::ARCHIVE_PREFIX,
                        item.change_id
                    )
                } else {
                    format!(
                        "Branch '{}' tip {} has no valid archive proposal for '{}'; the final merge would integrate an unarchived change",
                        item.revision, item.tip, item.change_id
                    )
                },
            });
        }

        return Ok(BatchState::FinalMergeMissing {
            change_id: item.change_id.clone(),
            worktree: item.worktree.clone(),
            required_target_state: target_head.clone(),
        });
    }

    // Every item has committed integration evidence.
    let tree_paths = evidence
        .committed_tree_paths(&target_head, archive_layout::ACTIVE_CHANGES_PREFIX)
        .await
        .map_err(|error| unsafe_evidence("Failed to read committed target tree", error))?;

    for (item, integration) in validated.iter().zip(integrations.iter()) {
        validate_cleanup_commits(
            evidence,
            item,
            integration,
            base_revision,
            &target_head,
            &target_lineage,
        )
        .await?;

        let live = archive_layout::paths_contain_active_change(&tree_paths, &item.change_id);
        let archived = archive_layout::paths_contain_valid_archive(&tree_paths, &item.change_id);
        if live && archived {
            return Ok(BatchState::ResurrectionCleanupRequired {
                change_id: item.change_id.clone(),
                reason: format!(
                    "Committed target HEAD {} holds both openspec/changes/{} and its valid archive entry",
                    target_head, item.change_id
                ),
            });
        }
    }

    let target_conflicts = evidence
        .target_conflict_paths()
        .await
        .map_err(|error| unsafe_evidence("Failed to read target index stages", error))?;
    if !target_conflicts.is_empty() {
        return Err(BatchState::UnsafeEvidence {
            reason: format!(
                "Every item is integrated but the target index has conflict stages: {}",
                target_conflicts.join(", ")
            ),
        });
    }

    let clean = evidence
        .target_is_clean()
        .await
        .map_err(|error| unsafe_evidence("Failed to read target cleanliness", error))?;
    if !clean {
        return Err(BatchState::UnsafeEvidence {
            reason: "Every item is integrated but the target index or worktree is not clean"
                .to_string(),
        });
    }

    Ok(BatchState::Complete)
}

/// Verify final integration for every batch item.
///
/// Shares [`classify_item_integration`] with retry classification so terminal
/// verification cannot accept a topology the retry loop rejects, or vice versa.
pub async fn verify_final_integration(
    evidence: &dyn ResolveEvidence,
    items: &[SequentialMergeItem],
    base_revision: &str,
) -> std::result::Result<(), String> {
    let validated = validate_items(evidence, items)
        .await
        .map_err(|state| state.diagnosis())?;
    let target_head = evidence
        .target_head()
        .await
        .map_err(|error| format!("Failed to read target HEAD: {}", error))?;

    let mut missing = Vec::new();
    for item in &validated {
        match classify_item_integration(evidence, item, base_revision, &target_head)
            .await
            .map_err(|state| state.diagnosis())?
        {
            ItemIntegration::Exact { .. } | ItemIntegration::Historical => {}
            ItemIntegration::NotIntegrated => missing.push(item.change_id.clone()),
        }
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Missing merge commit message containing change_id(s): {}",
            missing.join(", ")
        ))
    }
}

/// Git-backed [`ResolveEvidence`] over a target repository root.
pub struct GitResolveEvidence {
    repo_root: PathBuf,
}

impl GitResolveEvidence {
    /// Build an adapter rooted at the target repository.
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }
}

#[async_trait]
impl ResolveEvidence for GitResolveEvidence {
    async fn validate_worktree(
        &self,
        supplied_path: &Path,
        expected_branch: &str,
    ) -> WorktreeIdentity {
        git_commands::validate_worktree_identity(&self.repo_root, supplied_path, expected_branch)
            .await
    }

    async fn worktree_merge_in_progress(&self, worktree: &Path) -> EvidenceResult<bool> {
        git_commands::is_merge_in_progress(worktree)
            .await
            .map_err(|error| error.to_string())
    }

    async fn worktree_conflicts(&self, worktree: &Path) -> EvidenceResult<Vec<String>> {
        git_commands::get_conflict_files(worktree)
            .await
            .map_err(|error| error.to_string())
    }

    async fn target_head(&self) -> EvidenceResult<String> {
        git_commands::rev_parse_commit(&self.repo_root, "HEAD")
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "target repository has no HEAD commit".to_string())
    }

    async fn target_merge_head(&self) -> EvidenceResult<Option<String>> {
        git_commands::merge_head(&self.repo_root)
            .await
            .map_err(|error| error.to_string())
    }

    async fn target_conflict_paths(&self) -> EvidenceResult<Vec<String>> {
        let entries = git_commands::index_conflict_entries(&self.repo_root)
            .await
            .map_err(|error| error.to_string())?;
        let mut paths: Vec<String> = entries.into_iter().map(|entry| entry.path).collect();
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    async fn target_is_clean(&self) -> EvidenceResult<bool> {
        git_commands::is_clean_including_untracked(&self.repo_root)
            .await
            .map_err(|error| error.to_string())
    }

    async fn target_index_paths(&self, prefix: &str) -> EvidenceResult<Vec<String>> {
        git_commands::index_stage0_paths(&self.repo_root, prefix)
            .await
            .map_err(|error| error.to_string())
    }

    async fn parents_of(&self, commit: &str) -> EvidenceResult<Vec<String>> {
        git_commands::parents_of(&self.repo_root, commit)
            .await
            .map_err(|error| error.to_string())
    }

    async fn is_ancestor(&self, ancestor: &str, descendant: &str) -> EvidenceResult<bool> {
        git_commands::is_ancestor(&self.repo_root, ancestor, descendant)
            .await
            .map_err(|error| error.to_string())
    }

    async fn first_parent_lineage(&self, tip: &str) -> EvidenceResult<Vec<String>> {
        git_commands::first_parent_lineage(&self.repo_root, tip)
            .await
            .map_err(|error| error.to_string())
    }

    async fn merge_base(&self, a: &str, b: &str) -> EvidenceResult<Option<String>> {
        git_commands::merge_base(&self.repo_root, a, b)
            .await
            .map_err(|error| error.to_string())
    }

    async fn commits_with_exact_subject(
        &self,
        from: Option<&str>,
        to: &str,
        subject: &str,
    ) -> EvidenceResult<Vec<String>> {
        git_commands::commits_with_exact_subject(&self.repo_root, from, to, subject)
            .await
            .map_err(|error| error.to_string())
    }

    async fn commit_diff_entries(&self, commit: &str) -> EvidenceResult<Vec<CommitDiffEntry>> {
        git_commands::commit_diff_entries(&self.repo_root, commit)
            .await
            .map_err(|error| error.to_string())
    }

    async fn committed_tree_paths(
        &self,
        revision: &str,
        prefix: &str,
    ) -> EvidenceResult<Vec<String>> {
        git_commands::committed_tree_paths(&self.repo_root, revision, prefix)
            .await
            .map_err(|error| error.to_string())
    }
}
