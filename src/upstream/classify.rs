//! Repository-state classification for upstream integration.
//!
//! Nothing in this module reads human-readable Git prose. Merge outcomes come
//! from exit status plus `MERGE_HEAD`/unmerged-index evidence, revision routing
//! comes from ancestry, and push routing comes from `git push --porcelain`
//! per-ref status plus `git status --porcelain=v2`.

/// Observed repository state after running a merge command.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MergeRepositoryState {
    /// `MERGE_HEAD` exists.
    pub merge_head_present: bool,
    /// The index contains unmerged (stage > 0) entries.
    pub has_unmerged_entries: bool,
}

/// Authoritative classification of an upstream merge attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeOutcomeClass {
    /// Merge finished; the tree changed and verification must run.
    Completed,
    /// Repairable textual conflict: `MERGE_HEAD` plus unmerged entries.
    Conflicted,
    /// Hard command failure. No agent may be started from output text alone.
    CommandFailure,
}

/// Classify a merge from command status and repository evidence only.
///
/// A zero exit that nevertheless left an unfinished merge is treated as
/// conflicted, because repository state — not the exit code — is authoritative.
pub fn classify_merge_outcome(
    exit_success: bool,
    state: MergeRepositoryState,
) -> MergeOutcomeClass {
    if state.merge_head_present && state.has_unmerged_entries {
        return MergeOutcomeClass::Conflicted;
    }
    if exit_success && !state.merge_head_present {
        return MergeOutcomeClass::Completed;
    }
    MergeOutcomeClass::CommandFailure
}

/// Ancestry classification of a freshly fetched remote revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpstreamRevisionClass {
    /// Fetched revision is already contained in cumulative local HEAD.
    AlreadyIntegrated,
    /// Local HEAD is contained in the fetched revision (strictly remote-ahead).
    RemoteAhead,
    /// Neither contains the other.
    Diverged,
}

/// Classify a fetched revision against cumulative local HEAD.
///
/// Both remote-ahead and diverged histories are integrated with the same
/// `--no-ff` merge; the distinction exists for operator-visible reporting.
pub fn classify_upstream_revision(
    fetched_is_ancestor_of_head: bool,
    head_is_ancestor_of_fetched: bool,
) -> UpstreamRevisionClass {
    if fetched_is_ancestor_of_head {
        UpstreamRevisionClass::AlreadyIntegrated
    } else if head_is_ancestor_of_fetched {
        UpstreamRevisionClass::RemoteAhead
    } else {
        UpstreamRevisionClass::Diverged
    }
}

/// One machine-readable per-ref line from `git push --porcelain`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushRefStatus {
    /// Leading porcelain flag character (`!`, `*`, `=`, `+`, `-`, or space).
    pub flag: char,
    /// `<from>:<to>` ref pair.
    pub refs: String,
    /// Summary field (for rejections, `[rejected]` / `[remote rejected]`).
    pub summary: String,
    /// Trailing parenthesised reason, when Git supplied one.
    pub reason: Option<String>,
}

impl PushRefStatus {
    pub fn is_rejected(&self) -> bool {
        self.flag == '!'
    }

    /// Whether this rejection carries one of Git's race reasons.
    pub fn is_race_rejection(&self) -> bool {
        if !self.is_rejected() {
            return false;
        }
        let haystack = format!(
            "{} {}",
            self.summary,
            self.reason.as_deref().unwrap_or_default()
        )
        .to_ascii_lowercase();
        haystack.contains("non-fast-forward")
            || haystack.contains("fetch first")
            || haystack.contains("stale info")
    }
}

/// Parsed `git push --porcelain` output.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PushPorcelainReport {
    pub refs: Vec<PushRefStatus>,
}

impl PushPorcelainReport {
    pub fn has_race_rejection(&self) -> bool {
        self.refs.iter().any(PushRefStatus::is_race_rejection)
    }

    pub fn has_rejection(&self) -> bool {
        self.refs.iter().any(PushRefStatus::is_rejected)
    }
}

/// Parse `git push --porcelain` stdout into per-ref status entries.
///
/// Only tab-delimited ref lines are considered; `To <url>` and `Done` framing
/// lines and any other prose are ignored.
pub fn parse_push_porcelain(stdout: &str) -> PushPorcelainReport {
    let mut refs = Vec::new();

    for line in stdout.lines() {
        if line.is_empty() || !line.contains('\t') {
            continue;
        }
        let mut fields = line.split('\t');
        let Some(flag_field) = fields.next() else {
            continue;
        };
        // Porcelain flag is exactly one character; `Done`/`To ...` lines are skipped
        // above because they carry no tab.
        let flag = match flag_field.chars().next() {
            Some(c) if flag_field.chars().count() <= 1 => c,
            // A space flag is emitted as an empty first field.
            None => ' ',
            _ => continue,
        };
        let Some(refs_field) = fields.next() else {
            continue;
        };
        if !refs_field.contains(':') {
            continue;
        }
        let summary = fields.next().unwrap_or_default().to_string();
        let reason = fields.next().map(|r| r.trim().to_string());

        refs.push(PushRefStatus {
            flag,
            refs: refs_field.to_string(),
            summary,
            reason,
        });
    }

    PushPorcelainReport { refs }
}

/// Post-failure worktree state derived from `git status --porcelain=v2`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PorcelainV2State {
    /// A tracked entry differs from HEAD or the index (`1 ` / `2 ` records).
    pub tracked_mutation: bool,
    /// An unmerged entry exists (`u ` records).
    pub unmerged_entries: bool,
}

impl PorcelainV2State {
    pub fn is_repairable(&self) -> bool {
        self.tracked_mutation || self.unmerged_entries
    }
}

/// Parse `git status --porcelain=v2` output.
///
/// Untracked (`?`) and ignored (`!`) records are intentionally not repository
/// mutation: they cannot make a non-force push fail and must not start an agent.
pub fn parse_porcelain_v2(status: &str) -> PorcelainV2State {
    let mut state = PorcelainV2State::default();
    for line in status.lines() {
        if line.starts_with("1 ") || line.starts_with("2 ") {
            state.tracked_mutation = true;
        } else if line.starts_with("u ") {
            state.unmerged_entries = true;
        }
    }
    state
}

/// How a failed native push must be routed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushFailureClass {
    /// Remote advanced; return to the bounded checkpoint flow.
    Race,
    /// Local repository mutation may be handed to the bounded repair agent.
    RepositoryRepairable,
    /// Credential/permission/transport/hook-policy/remote-service failure.
    Stalled,
}

/// Classify a failed push from machine-readable evidence only.
pub fn classify_push_failure(
    report: &PushPorcelainReport,
    post_failure_state: PorcelainV2State,
) -> PushFailureClass {
    if report.has_race_rejection() {
        return PushFailureClass::Race;
    }
    if post_failure_state.is_repairable() {
        return PushFailureClass::RepositoryRepairable;
    }
    PushFailureClass::Stalled
}

/// Confirmation of a completed push observed through `git ls-remote`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushConfirmation {
    /// Observed remote SHA equals the pushed local HEAD.
    Confirmed,
    /// The remote advanced past the pushed HEAD, which still contains it.
    ConfirmedAfterAdvance,
    /// The pushed HEAD is not reachable from the observed remote SHA.
    NotConfirmed,
}

/// Confirm a push from observed remote state and ancestry evidence.
pub fn classify_push_confirmation(
    pushed_head: &str,
    observed_remote_sha: &str,
    pushed_head_is_ancestor_of_observed: bool,
) -> PushConfirmation {
    if pushed_head == observed_remote_sha {
        PushConfirmation::Confirmed
    } else if pushed_head_is_ancestor_of_observed {
        PushConfirmation::ConfirmedAfterAdvance
    } else {
        PushConfirmation::NotConfirmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(merge_head: bool, unmerged: bool) -> MergeRepositoryState {
        MergeRepositoryState {
            merge_head_present: merge_head,
            has_unmerged_entries: unmerged,
        }
    }

    #[test]
    fn upstream_integration_merge_success_is_repository_derived() {
        assert_eq!(
            classify_merge_outcome(true, state(false, false)),
            MergeOutcomeClass::Completed
        );
    }

    #[test]
    fn upstream_integration_localized_conflict_output_still_enters_repair() {
        // Non-zero exit with no recognizable English text, but repository state
        // proves a repairable unfinished merge.
        assert_eq!(
            classify_merge_outcome(false, state(true, true)),
            MergeOutcomeClass::Conflicted
        );
    }

    #[test]
    fn upstream_integration_unrelated_merge_failure_is_command_failure() {
        assert_eq!(
            classify_merge_outcome(false, state(false, false)),
            MergeOutcomeClass::CommandFailure
        );
        // MERGE_HEAD without unmerged entries is an unfinished merge that the
        // ordinary success predicate must not accept.
        assert_eq!(
            classify_merge_outcome(true, state(true, false)),
            MergeOutcomeClass::CommandFailure
        );
    }

    #[test]
    fn upstream_integration_classifies_revision_ancestry() {
        assert_eq!(
            classify_upstream_revision(true, false),
            UpstreamRevisionClass::AlreadyIntegrated
        );
        assert_eq!(
            classify_upstream_revision(false, true),
            UpstreamRevisionClass::RemoteAhead
        );
        assert_eq!(
            classify_upstream_revision(false, false),
            UpstreamRevisionClass::Diverged
        );
        // Equal revisions report as already integrated, never as a merge target.
        assert_eq!(
            classify_upstream_revision(true, true),
            UpstreamRevisionClass::AlreadyIntegrated
        );
    }

    #[test]
    fn upstream_integration_parses_push_porcelain_race_rejection() {
        let stdout = "To /tmp/remote.git\n!\trefs/heads/main:refs/heads/main\t[rejected]\t(fetch first)\nDone\n";
        let report = parse_push_porcelain(stdout);
        assert_eq!(report.refs.len(), 1);
        assert!(report.has_race_rejection());
        assert_eq!(
            classify_push_failure(&report, PorcelainV2State::default()),
            PushFailureClass::Race
        );
    }

    #[test]
    fn upstream_integration_parses_push_porcelain_success() {
        let stdout = "To /tmp/remote.git\n \trefs/heads/main:refs/heads/main\t abc..def\nDone\n";
        let report = parse_push_porcelain(stdout);
        assert_eq!(report.refs.len(), 1);
        assert!(!report.has_rejection());
    }

    #[test]
    fn upstream_integration_non_race_rejection_without_mutation_stalls() {
        // Hook / policy rejection: rejected, but no race reason and a clean tree.
        let report = parse_push_porcelain(
            "To /tmp/remote.git\n!\trefs/heads/main:refs/heads/main\t[remote rejected]\t(pre-receive hook declined)\nDone\n",
        );
        assert!(!report.has_race_rejection());
        assert_eq!(
            classify_push_failure(&report, PorcelainV2State::default()),
            PushFailureClass::Stalled
        );
    }

    #[test]
    fn upstream_integration_push_failure_with_tracked_mutation_is_repairable() {
        let report = parse_push_porcelain("To /tmp/remote.git\nDone\n");
        let status = parse_porcelain_v2("1 .M N... 100644 100644 100644 aaa bbb src/lib.rs\n");
        assert!(status.tracked_mutation);
        assert_eq!(
            classify_push_failure(&report, status),
            PushFailureClass::RepositoryRepairable
        );
    }

    #[test]
    fn upstream_integration_push_failure_with_unmerged_entries_is_repairable() {
        let status = parse_porcelain_v2("u UU N... 100644 100644 100644 100644 a b c d src/x.rs\n");
        assert!(status.unmerged_entries);
        assert_eq!(
            classify_push_failure(&PushPorcelainReport::default(), status),
            PushFailureClass::RepositoryRepairable
        );
    }

    #[test]
    fn upstream_integration_untracked_files_are_not_repairable_mutation() {
        let status = parse_porcelain_v2("? build.log\n! target/\n");
        assert!(!status.is_repairable());
        assert_eq!(
            classify_push_failure(&PushPorcelainReport::default(), status),
            PushFailureClass::Stalled
        );
    }

    #[test]
    fn upstream_integration_stderr_text_cannot_change_push_routing() {
        // Only porcelain evidence is inspected: a stderr-looking string parsed as
        // stdout yields no ref entries and therefore no race.
        let report = parse_push_porcelain(
            "error: failed to push some refs\nhint: Updates were rejected because the remote contains work\n",
        );
        assert!(report.refs.is_empty());
        assert_eq!(
            classify_push_failure(&report, PorcelainV2State::default()),
            PushFailureClass::Stalled
        );
    }

    #[test]
    fn upstream_integration_confirms_push_from_remote_observation() {
        assert_eq!(
            classify_push_confirmation("abc", "abc", false),
            PushConfirmation::Confirmed
        );
        assert_eq!(
            classify_push_confirmation("abc", "def", true),
            PushConfirmation::ConfirmedAfterAdvance
        );
        assert_eq!(
            classify_push_confirmation("abc", "def", false),
            PushConfirmation::NotConfirmed
        );
    }
}
