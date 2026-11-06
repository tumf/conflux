//! Consent-bound destructive deletion on top of the ordinary delete steps.
//!
//! [`WorktreeService::delete_worktree`] is untouched. This path *adds* checks
//! before and after the ones it shares with it: every consented fact and both
//! recomputed content identities are compared under the root guard
//! immediately before teardown and again immediately before removal, and any
//! difference, unknown, or unbounded fact refuses with nothing removed.
//!
//! The permissions map onto [`DeleteOptions`] one-to-one, so the eligibility,
//! teardown, drift, branch-ref, removal, and branch-cleanup steps are the same
//! ones the TUI confirmations run.

use std::path::Path;

use tracing::warn;

use super::{
    classify_delete_drift, classify_delete_eligibility, DeleteOptions, DirtyState, DirtyTarget,
    ExpectedTarget, SafetyFact, WorktreeFacts, WorktreeOpError, WorktreeOpOutcome,
    WorktreeOperationEvent, WorktreeService, SESSION_RUNNING_REFUSAL,
};
use crate::worktree_ops::consent::{
    empty_content_id, ConfirmedDeleteReason, ConsentObservation, ContentObservation, DeleteConsent,
    TeardownDisposition,
};

// ============================================================================
// Pure classification
// ============================================================================

/// Whether the worktree has no branch: a detached HEAD, or an empty branch name.
fn is_detached(facts: &WorktreeFacts) -> bool {
    facts.is_detached || facts.branch.is_empty()
}

/// Decide whether the observed facts admit a confirmed delete under `options`.
///
/// The permission half of the decision, shared by the preview's per-option
/// eligibility and the command, so the two cannot disagree. Consent equality is
/// checked separately: this answers "would these permissions be enough", not
/// "is this still the worktree the client saw".
///
/// A content observation that bound nothing refuses whatever was granted,
/// ahead of every permission check, because no permission can authorize
/// discarding content nobody could name.
pub fn classify_confirmed_delete(
    facts: &WorktreeFacts,
    content: &ContentObservation,
    options: DeleteOptions,
) -> Result<(), ConfirmedDeleteReason> {
    // A detached HEAD has no branch to consent to, so no option is offered
    // remotely, whatever else is true of the worktree.
    if is_detached(facts) {
        return Err(ConfirmedDeleteReason::DetachedHead);
    }
    if facts.is_main {
        return Err(ConfirmedDeleteReason::MainWorktree);
    }
    // An unreadable base merge state is reported as `root_busy` too: the
    // vocabulary has no separate "unknown root" reason, and either way the
    // root is not in a state anything may be removed under.
    if facts.base_merge_in_progress != SafetyFact::No {
        return Err(ConfirmedDeleteReason::RootBusy);
    }
    if let Some(reason) = content.refusal_reason() {
        return Err(reason);
    }
    if facts.has_commits_ahead == SafetyFact::Unknown {
        return Err(ConfirmedDeleteReason::AheadUnknown);
    }
    if facts.dirty == DirtyState::Unknown {
        return Err(ConfirmedDeleteReason::DirtyUnknown);
    }
    if facts.has_commits_ahead == SafetyFact::Yes && !options.allow_commits_ahead {
        return Err(ConfirmedDeleteReason::CommitsAhead);
    }
    if facts.dirty == DirtyState::Dirty && !options.allow_known_dirty {
        return Err(ConfirmedDeleteReason::Dirty);
    }
    let has_ignored = content.observed().is_some_and(|c| c.has_ignored_content);
    if has_ignored && !options.allow_known_dirty {
        return Err(ConfirmedDeleteReason::IgnoredContent);
    }
    Ok(())
}

/// Compare a fresh observation against what the client consented to.
///
/// `after_teardown` grants the one allowance teardown has: it exists to clean
/// build output, so the ignored set may afterwards be the consented one *or*
/// empty. Anything else it leaves behind is content nobody consented to. The
/// discardable identity has no such allowance.
pub fn classify_consent(
    facts: &WorktreeFacts,
    content: &ContentObservation,
    merge_base: Option<&str>,
    consent: &DeleteConsent,
    options: DeleteOptions,
    after_teardown: bool,
) -> Result<(), ConfirmedDeleteReason> {
    // Checked before the branch comparison: a worktree detached after an
    // attached preview is refused for what it is, not as a branch change.
    if is_detached(facts) {
        return Err(ConfirmedDeleteReason::DetachedHead);
    }
    if facts.branch != consent.branch {
        return Err(ConfirmedDeleteReason::BranchChanged);
    }
    if facts.head != consent.head {
        return Err(ConfirmedDeleteReason::HeadMoved);
    }
    if facts.is_main {
        return Err(ConfirmedDeleteReason::MainWorktree);
    }
    if facts.base_merge_in_progress != SafetyFact::No {
        return Err(ConfirmedDeleteReason::RootBusy);
    }
    // Unknown is never "safe", and it is never "equal" either: an unobservable
    // fact refuses as unknown before any comparison could treat it as a match.
    if facts.dirty == DirtyState::Unknown {
        return Err(ConfirmedDeleteReason::DirtyUnknown);
    }
    if facts.has_commits_ahead == SafetyFact::Unknown {
        return Err(ConfirmedDeleteReason::AheadUnknown);
    }
    if facts.dirty.as_option() != consent.dirty {
        return Err(ConfirmedDeleteReason::DirtyChanged);
    }
    if crate::worktree_ops::consent::fact_option(facts.has_commits_ahead)
        != consent.has_commits_ahead
    {
        return Err(ConfirmedDeleteReason::AheadChanged);
    }
    let observed = match content {
        ContentObservation::Observed(observed) => observed,
        other => {
            return Err(other
                .refusal_reason()
                .unwrap_or(ConfirmedDeleteReason::ContentUnobservable))
        }
    };
    let cleaned_by_teardown = after_teardown && observed.ignored_content_id == empty_content_id();
    if !cleaned_by_teardown {
        // A `null` consent value never equals an observed one.
        if consent.has_ignored_content != Some(observed.has_ignored_content) {
            return Err(ConfirmedDeleteReason::IgnoredChanged);
        }
        if let Some(consented) = consent.ignored_content_id.as_deref() {
            if consented != observed.ignored_content_id {
                return Err(ConfirmedDeleteReason::ContentChanged);
            }
        }
    }
    if let Some(consented) = consent.discardable_content_id.as_deref() {
        if consented != observed.discardable_content_id {
            return Err(ConfirmedDeleteReason::ContentChanged);
        }
    }
    if options.allow_commits_ahead {
        match merge_base {
            None => return Err(ConfirmedDeleteReason::AheadUnknown),
            Some(base) if consent.merge_base.as_deref() != Some(base) => {
                return Err(ConfirmedDeleteReason::BaseMoved)
            }
            Some(_) => {}
        }
    }
    classify_confirmed_delete(facts, content, options)
}

// ============================================================================
// Results
// ============================================================================

/// A refused confirmed delete, with everything a client needs to re-confirm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedDeleteRefusal {
    /// The service classification, for the v2 error code of non-stale refusals.
    pub error: WorktreeOpError,
    /// True when the consent no longer describes the target (`delete_consent_stale`).
    pub stale: bool,
    /// The typed reason; `None` for a refusal outside the consent vocabulary
    /// (an unknown worktree, a removal failure).
    pub reason: Option<ConfirmedDeleteReason>,
    /// How far teardown got. Teardown effects are never reversed.
    pub teardown: TeardownDisposition,
    /// The freshest facts the service observed, when it observed any.
    pub observed: Option<Box<ConsentObservation>>,
}

impl ConfirmedDeleteRefusal {
    /// True when `.wt/teardown` ran before the refusal.
    pub fn teardown_ran(&self) -> bool {
        self.teardown == TeardownDisposition::Ran
    }

    fn from_error(error: WorktreeOpError, teardown: TeardownDisposition) -> Self {
        Self {
            error,
            stale: false,
            reason: None,
            teardown,
            observed: None,
        }
    }

    fn with_observed(mut self, observed: ConsentObservation) -> Self {
        self.observed = Some(Box::new(observed));
        self
    }
}

impl std::fmt::Display for ConfirmedDeleteRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)
    }
}

/// Build the refusal a typed reason maps to.
fn refusal(
    reason: ConfirmedDeleteReason,
    facts: &WorktreeFacts,
    teardown: TeardownDisposition,
) -> ConfirmedDeleteRefusal {
    let suffix = if teardown == TeardownDisposition::Ran {
        "; teardown already ran and was not reversed, nothing was removed"
    } else {
        "; nothing was removed"
    };
    let message = format!("{}{suffix}", reason_message(reason));
    let error = match reason {
        ConfirmedDeleteReason::DirtyUnknown
        | ConfirmedDeleteReason::ContentUnobservable
        | ConfirmedDeleteReason::ContentUnbounded => WorktreeOpError::DirtyUnknown(message),
        ConfirmedDeleteReason::RootBusy => WorktreeOpError::RootBusy(message),
        ConfirmedDeleteReason::Dirty | ConfirmedDeleteReason::IgnoredContent => {
            WorktreeOpError::Dirty {
                message,
                target: Box::new(DirtyTarget::from_facts(facts)),
            }
        }
        ConfirmedDeleteReason::TeardownFailed => WorktreeOpError::Internal(message),
        // Stale reasons and the ineligible ones share the `Ineligible` carrier;
        // `stale` is what decides the v2 code for the former.
        _ => WorktreeOpError::Ineligible(message),
    };
    ConfirmedDeleteRefusal {
        error,
        stale: reason.is_stale(),
        reason: Some(reason),
        teardown,
        observed: None,
    }
}

/// Operator-facing text for each reason.
fn reason_message(reason: ConfirmedDeleteReason) -> &'static str {
    match reason {
        ConfirmedDeleteReason::BranchChanged => {
            "the worktree is no longer on the consented branch; preview again"
        }
        ConfirmedDeleteReason::IdentityChanged => {
            "the path is now occupied by a different Git worktree; preview again"
        }
        ConfirmedDeleteReason::HeadMoved => "the worktree's HEAD or branch ref moved; preview again",
        ConfirmedDeleteReason::BaseMoved => {
            "the merge base with base moved since the consent; preview again"
        }
        ConfirmedDeleteReason::DirtyChanged => {
            "the worktree's uncommitted-change state differs from the consent; preview again"
        }
        ConfirmedDeleteReason::AheadChanged => {
            "the worktree's commits-ahead state differs from the consent; preview again"
        }
        ConfirmedDeleteReason::IgnoredChanged => {
            "the worktree's ignored-content state differs from the consent; preview again"
        }
        ConfirmedDeleteReason::ContentChanged => {
            "the worktree's content differs from the consented content identity; preview again"
        }
        ConfirmedDeleteReason::DirtyUnknown => {
            "the worktree's uncommitted-change state could not be determined"
        }
        ConfirmedDeleteReason::AheadUnknown => {
            "the worktree's commits-ahead state or merge base could not be determined"
        }
        ConfirmedDeleteReason::ContentUnobservable => {
            "the worktree's content could not be observed"
        }
        ConfirmedDeleteReason::ContentUnbounded => {
            "the worktree's content exceeds the confirmed-delete observation budget; reduce it locally or delete through the TUI"
        }
        ConfirmedDeleteReason::NestedRepository => {
            "the worktree contains a nested repository whose content cannot be bound; delete it locally"
        }
        ConfirmedDeleteReason::MainWorktree => "the main worktree cannot be deleted",
        ConfirmedDeleteReason::DetachedHead => {
            "the worktree has a detached HEAD and no branch to consent to; delete it locally or through the TUI"
        }
        ConfirmedDeleteReason::RootBusy => {
            "the repository root is busy or holding an unresolved merge"
        }
        ConfirmedDeleteReason::CommitsAhead => {
            "the worktree has unmerged commits ahead of base and discard_commits_ahead was not granted"
        }
        ConfirmedDeleteReason::Dirty => {
            "the worktree has uncommitted changes and discard_known_dirty was not granted"
        }
        ConfirmedDeleteReason::IgnoredContent => {
            "the worktree holds ignored content that forced removal deletes and discard_known_dirty was not granted"
        }
        ConfirmedDeleteReason::TeardownFailed => "worktree teardown failed",
    }
}

/// A completed confirmed delete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedDeleteOutcome {
    /// What the shared delete steps did.
    pub outcome: WorktreeOpOutcome,
    /// HEAD the worktree was removed at.
    pub head: String,
    /// What happened to teardown.
    pub teardown: TeardownDisposition,
    /// True when known uncommitted or ignored content was discarded.
    pub dirty_discarded: bool,
    /// True when commits ahead of base were discarded with their branch.
    pub commits_discarded: bool,
    /// True when the branch no longer exists.
    pub branch_deleted: bool,
    /// Why the branch was kept, when it was.
    pub branch_retained_reason: Option<String>,
}

/// One option's eligibility on a delete preview.
pub type OptionEligibility = Result<(), ConfirmedDeleteReason>;

/// Everything a client must see before consenting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletePreview {
    /// The fresh targeted observation.
    pub facts: WorktreeFacts,
    /// The content observation.
    pub content: ContentObservation,
    /// The facts and identities a consent must echo.
    pub observation: ConsentObservation,
    /// Commits between the merge base and HEAD, when countable.
    pub commits_ahead: Option<usize>,
    /// True exactly when teardown would execute `.wt/teardown`.
    pub teardown_script_present: bool,
    /// `delete_worktree` itself (fail-closed).
    pub ordinary: OptionEligibility,
    /// `skip_teardown` alone.
    pub skip_teardown: OptionEligibility,
    /// `discard_known_dirty` alone.
    pub discard_known_dirty: OptionEligibility,
    /// `discard_commits_ahead`, with `discard_known_dirty` when anything is dirty.
    pub discard_commits_ahead: OptionEligibility,
    /// True when the ahead option also discards known dirty or ignored content.
    pub also_discards_dirty: bool,
}

// ============================================================================
// Service
// ============================================================================

impl WorktreeService {
    /// Observe what a confirmed delete of `path` would act on. A read.
    ///
    /// Takes no mutation guard and changes nothing; it runs the same targeted
    /// observation and the same content oracle the command runs, so the
    /// per-option answers agree with what the command would decide.
    pub async fn delete_preview(
        &self,
        path: &Path,
        identity: &str,
    ) -> Result<DeletePreview, WorktreeOpError> {
        let facts = self.locate(path).await?;
        if facts.identity != identity {
            return Err(WorktreeOpError::NotFound(
                "the addressed worktree's Git identity no longer matches this path".to_string(),
            ));
        }
        let content = self
            .backend
            .observe_content(&facts.path, self.consent_limits)
            .await;
        let merge_base = self.backend.merge_base(&facts.head).await.ok().flatten();
        let commits_ahead = match merge_base.as_deref() {
            Some(base) => self.backend.commits_between(base, &facts.head).await.ok(),
            None => None,
        };
        let teardown_script_present = self.backend.teardown_script_present(&facts.path).await;
        let has_ignored = content.observed().is_some_and(|c| c.has_ignored_content);
        let also_discards_dirty = facts.dirty == DirtyState::Dirty || has_ignored;

        let ordinary = classify_confirmed_delete(&facts, &content, DeleteOptions::fail_closed());
        let skip_teardown = classify_confirmed_delete(&facts, &content, DeleteOptions::local(true));
        let discard_known_dirty = classify_confirmed_delete(
            &facts,
            &content,
            DeleteOptions::local_discarding_dirty(false),
        );
        // The command refuses this option without a merge base to bind, so
        // the preview must not advertise it as eligible.
        let discard_commits_ahead = classify_confirmed_delete(
            &facts,
            &content,
            DeleteOptions::local_discarding_ahead(false, also_discards_dirty),
        )
        .and_then(|()| {
            merge_base
                .as_deref()
                .map(|_| ())
                .ok_or(ConfirmedDeleteReason::AheadUnknown)
        });
        let observation = ConsentObservation::new(&facts, &content, merge_base);

        Ok(DeletePreview {
            facts,
            content,
            observation,
            commits_ahead,
            teardown_script_present,
            ordinary,
            skip_teardown,
            discard_known_dirty,
            discard_commits_ahead,
            also_discards_dirty,
        })
    }

    /// Delete a worktree on an explicit, re-verified consent.
    ///
    /// The steps of [`Self::delete_worktree`], in its order, with the consent
    /// comparison added immediately before teardown and immediately before
    /// removal. Every refusal before removal removes nothing; a refusal after
    /// teardown says teardown ran.
    pub async fn delete_worktree_with_consent(
        &self,
        path: &Path,
        expected: &ExpectedTarget,
        consent: &DeleteConsent,
        options: DeleteOptions,
    ) -> Result<ConfirmedDeleteOutcome, ConfirmedDeleteRefusal> {
        let not_reached = TeardownDisposition::NotReached;
        let _guard = self
            .acquire_root()
            .map_err(|error| ConfirmedDeleteRefusal {
                reason: Some(ConfirmedDeleteReason::RootBusy),
                ..ConfirmedDeleteRefusal::from_error(error, not_reached)
            })?;

        let before = self
            .locate(path)
            .await
            .map_err(|error| ConfirmedDeleteRefusal::from_error(error, not_reached))?;
        let (content, merge_base) = self.observe_consent_facts(&before).await;
        let observed = ConsentObservation::new(&before, &content, merge_base.clone());
        self.check_consent(
            &before,
            expected,
            &content,
            merge_base.as_deref(),
            consent,
            options,
            not_reached,
        )
        .map_err(|refusal| refusal.with_observed(observed.clone()))?;
        // A session's cwd is not something teardown or removal may pull out
        // from under it, whatever permissions were consented.
        if self
            .session_probe
            .as_ref()
            .is_some_and(|probe| probe.session_running(&before.path))
        {
            return Err(ConfirmedDeleteRefusal::from_error(
                WorktreeOpError::Ineligible(SESSION_RUNNING_REFUSAL.to_string()),
                not_reached,
            )
            .with_observed(observed));
        }
        // The ordinary eligibility still runs, unchanged, as the backstop.
        classify_delete_eligibility(&before, options).map_err(|error| {
            ConfirmedDeleteRefusal::from_error(error, not_reached).with_observed(observed.clone())
        })?;

        let teardown = if options.skip_teardown {
            TeardownDisposition::Skipped
        } else {
            let present = self.backend.teardown_script_present(&before.path).await;
            if let Err(error) = self.backend.teardown(&before.path).await {
                let mut refused = refusal(
                    ConfirmedDeleteReason::TeardownFailed,
                    &before,
                    TeardownDisposition::Ran,
                );
                refused.error = WorktreeOpError::Internal(error.to_string());
                return Err(refused.with_observed(observed));
            }
            if present {
                TeardownDisposition::Ran
            } else {
                TeardownDisposition::Absent
            }
        };

        let after = self
            .locate(&before.path)
            .await
            .map_err(|error| ConfirmedDeleteRefusal::from_error(error, teardown))?;
        let (content, merge_base) = self.observe_consent_facts(&after).await;
        let observed = ConsentObservation::new(&after, &content, merge_base.clone());
        self.check_consent(
            &after,
            expected,
            &content,
            merge_base.as_deref(),
            consent,
            options,
            teardown,
        )
        .map_err(|refusal| refusal.with_observed(observed.clone()))?;
        classify_delete_drift(&before, &after)
            .and_then(|()| classify_delete_eligibility(&after, options))
            .map_err(|error| {
                ConfirmedDeleteRefusal::from_error(error, teardown).with_observed(observed.clone())
            })?;
        self.confirm_branch_ref(&after).await.map_err(|error| {
            let moved = matches!(error, WorktreeOpError::NotFound(_));
            ConfirmedDeleteRefusal {
                stale: moved,
                reason: moved.then_some(ConfirmedDeleteReason::HeadMoved),
                ..ConfirmedDeleteRefusal::from_error(error, teardown)
            }
            .with_observed(observed.clone())
        })?;

        let has_ignored = content.observed().is_some_and(|c| c.has_ignored_content);
        let dirty_discarded =
            options.allow_known_dirty && (after.dirty == DirtyState::Dirty || has_ignored);
        let discards_ahead =
            options.allow_commits_ahead && after.has_commits_ahead == SafetyFact::Yes;
        warn!(
            path = %after.path.display(),
            branch = %after.branch,
            head = %after.head,
            confirmed_delete = true,
            dirty_discard = dirty_discarded,
            ahead_discard = discards_ahead,
            skip_teardown = options.skip_teardown,
            "removing a worktree on a remotely consented, re-verified confirmation"
        );

        self.backend
            .remove_worktree(&after.path)
            .await
            .map_err(|error| ConfirmedDeleteRefusal::from_error(error, teardown))?;

        // A permission the observed facts do not need changes nothing: the
        // force-delete of an unmerged branch runs only where there really were
        // commits ahead to discard.
        let (branch_note, branch_retained) = if discards_ahead {
            self.discard_ahead_branch(&after).await
        } else {
            self.cleanup_branch(&after).await
        };

        self.events
            .emit(WorktreeOperationEvent::Deleted {
                branch: after.branch.clone(),
            })
            .await;
        self.events.emit(WorktreeOperationEvent::Refreshed).await;

        let teardown_note = match teardown {
            TeardownDisposition::Skipped => "removed without teardown",
            TeardownDisposition::Absent => "removed (no teardown script)",
            _ => "torn down and removed",
        };
        let branch_deleted = !after.branch.is_empty() && !branch_retained;
        Ok(ConfirmedDeleteOutcome {
            head: after.head.clone(),
            teardown,
            dirty_discarded,
            commits_discarded: discards_ahead && branch_deleted,
            branch_deleted,
            branch_retained_reason: branch_retained
                .then(|| branch_note.trim_start_matches("; ").to_string()),
            outcome: WorktreeOpOutcome {
                detail: format!(
                    "worktree on branch '{}' was {teardown_note} on explicit consent{branch_note}",
                    after.branch
                ),
                branch: after.branch.clone(),
                path: after.path,
                branch_retained,
                merge_evidence: None,
            },
        })
    }

    /// The content observation and the merge base, freshly derived.
    async fn observe_consent_facts(
        &self,
        facts: &WorktreeFacts,
    ) -> (ContentObservation, Option<String>) {
        let content = self
            .backend
            .observe_content(&facts.path, self.consent_limits)
            .await;
        let merge_base = self.backend.merge_base(&facts.head).await.ok().flatten();
        (content, merge_base)
    }

    /// Identity, then every consented fact, as one typed refusal.
    #[allow(clippy::too_many_arguments)]
    fn check_consent(
        &self,
        facts: &WorktreeFacts,
        expected: &ExpectedTarget,
        content: &ContentObservation,
        merge_base: Option<&str>,
        consent: &DeleteConsent,
        options: DeleteOptions,
        teardown: TeardownDisposition,
    ) -> Result<(), ConfirmedDeleteRefusal> {
        let identity = || -> Result<(), ConfirmedDeleteReason> {
            // A worktree detached since the preview is refused for what it is
            // before any identity comparison could call it a branch change.
            if is_detached(facts) {
                return Err(ConfirmedDeleteReason::DetachedHead);
            }
            if expected
                .branch
                .as_deref()
                .is_some_and(|b| b != facts.branch)
            {
                return Err(ConfirmedDeleteReason::BranchChanged);
            }
            if expected
                .identity
                .as_deref()
                .is_some_and(|i| i != facts.identity)
            {
                return Err(ConfirmedDeleteReason::IdentityChanged);
            }
            if expected.head.as_deref().is_some_and(|h| h != facts.head) {
                return Err(ConfirmedDeleteReason::HeadMoved);
            }
            Ok(())
        };
        identity()
            .and_then(|()| {
                classify_consent(
                    facts,
                    content,
                    merge_base,
                    consent,
                    options,
                    teardown == TeardownDisposition::Ran,
                )
            })
            .map_err(|reason| refusal(reason, facts, teardown))
    }
}
