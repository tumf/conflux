//! Deterministic, process-local signature of the dependency-analysis input.
//!
//! The scheduler wakes every 500 ms, and the queue-coalescing debounce only measures how
//! long ago the queue last changed. Once ten seconds have elapsed, every later timer wake
//! passes that check forever, so elapsed time alone cannot tell whether the current
//! analyzer input was already analyzed. This module derives a stable digest of exactly the
//! material that reaches the analyzer prompt and the dispatch decision, so an ordinary
//! timer wake can skip a duplicate expensive analysis.
//!
//! Scope and non-authority:
//!
//! - The signature suppresses duplicate analyzer *invocation*. A previous `AnalysisResult`
//!   is never reused to authorize dispatch; every eligible pass still re-derives queue
//!   classification, dependency blockers, and dispatch eligibility from repository state.
//! - It lives only in the running scheduler's memory. It is never persisted, so a process
//!   restart always begins with no prior signature (Constitution law 1: durable
//!   out-of-worktree state must not decide workflow behavior).
//! - Explicit scheduler edges (queue addition, completion, repair candidate, slot
//!   recovery) bypass suppression entirely; only ordinary timer evaluation consults it.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;
use std::time::Duration;

use tokio::time::Instant;

use crate::openspec::Change;

/// How long a degraded (recoverable LLM failure) suppression record stays armed.
///
/// After this interval an unchanged input becomes eligible for exactly one retry, so a
/// transient analyzer outage recovers without restoring the rapid re-analysis loop.
pub(crate) const DEGRADED_SUPPRESSION_TTL: Duration = Duration::from_secs(5 * 60);

/// Minimum interval between repository-visible signature probes while an input is
/// suppressed.
///
/// This matches the existing ten-second queue-coalescing debounce, so a suppressed 500 ms
/// wake never re-reads proposal files and never spawns a VCS subprocess.
pub(crate) const SUPPRESSED_PROBE_INTERVAL: Duration = Duration::from_secs(10);

/// Digest recorded for a proposal path that does not exist.
///
/// Absence is a deterministic, repository-visible state rather than a read failure: a
/// proposal that is later created changes the digest and therefore re-arms analysis.
const ABSENT_PROPOSAL_DIGEST: &str = "absent";

/// Deterministic digest of one dependency-analysis input.
///
/// Equality is only ever compared within a single process, but the digest is built from a
/// stable canonical encoding (not a deliberately unstable hasher) so the comparison
/// contract is explicit and testable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AnalysisInputSignature(String);

impl AnalysisInputSignature {
    /// The digest text, used for operator-visible diagnostic deduplication keys.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// The exact analyzer-input material a signature is derived from.
///
/// Every field is something that can change the dependency-analysis prompt or the dispatch
/// decision. Queued IDs alone are deliberately insufficient: the same IDs can require fresh
/// analysis after a proposal edit or after dependency integration into the effective base.
pub(crate) struct AnalysisInputMaterials<'a> {
    /// Queued changes exactly as they are presented to analysis.
    pub(crate) queued: &'a [Change],
    /// In-flight change IDs; encoded in sorted order so set equality is what matters.
    pub(crate) in_flight_ids: &'a [String],
    /// Dispatch capacity inputs that gate apply selection.
    pub(crate) available_slots: usize,
    pub(crate) max_parallelism: usize,
    /// Repository-visible effective dependency-base evidence, encoded as the selected base ref
    /// and the revision that ref resolves to.
    ///
    /// This is the same base dependency classification reads merge evidence from. The checkout
    /// `HEAD` commit is deliberately not used: the named ref can advance on its own, and an
    /// equal signature would suppress the only evaluation able to observe that.
    pub(crate) base_revision: &'a str,
    /// Content digest of every queued and in-flight `proposal.md` referenced by the prompt.
    pub(crate) proposal_digests: &'a HashMap<String, String>,
}

/// Build the signature for one analysis input.
///
/// The encoding is line-oriented and fully ordered: queued changes are emitted in sorted
/// ID order, in-flight IDs are sorted, and every list inside a change is sorted, so an
/// input that differs only by iteration order produces an equal signature while any real
/// input difference produces an unequal one.
pub(crate) fn build_analysis_input_signature(
    materials: &AnalysisInputMaterials<'_>,
) -> AnalysisInputSignature {
    let mut canonical = String::new();

    let _ = writeln!(canonical, "base_revision={}", materials.base_revision);
    let _ = writeln!(
        canonical,
        "capacity=available:{};max:{}",
        materials.available_slots, materials.max_parallelism
    );

    let mut in_flight: Vec<&str> = materials
        .in_flight_ids
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    in_flight.sort_unstable();
    in_flight.dedup();
    let _ = writeln!(canonical, "in_flight={}", in_flight.join(","));

    // In-flight proposals reach the analyzer prompt as dependency context, so their content
    // is part of the input even though they are not selectable.
    for change_id in &in_flight {
        let _ = writeln!(
            canonical,
            "in_flight_proposal={};digest={}",
            change_id,
            proposal_digest_for(materials.proposal_digests, change_id)
        );
    }

    let mut queued: Vec<&Change> = materials.queued.iter().collect();
    queued.sort_by(|left, right| left.id.cmp(&right.id));
    for change in queued {
        let _ = writeln!(canonical, "queued={}", change.id);
        let _ = writeln!(
            canonical,
            "  progress={}/{}",
            change.completed_tasks, change.total_tasks
        );
        let _ = writeln!(
            canonical,
            "  dependencies={}",
            sorted_list(&change.dependencies)
        );
        let _ = writeln!(
            canonical,
            "  metadata_change_type={}",
            change.metadata.change_type.as_deref().unwrap_or("none")
        );
        let _ = writeln!(
            canonical,
            "  metadata_priority={}",
            change
                .metadata
                .priority
                .map(|priority| format!("{:?}", priority))
                .unwrap_or_else(|| "none".to_string())
        );
        let _ = writeln!(
            canonical,
            "  metadata_dependencies={}",
            sorted_list(&change.metadata.dependencies)
        );
        let _ = writeln!(
            canonical,
            "  metadata_references={}",
            sorted_list(&change.metadata.references)
        );
        let _ = writeln!(
            canonical,
            "  metadata_warnings={}",
            sorted_list(&change.metadata.warnings)
        );
        let _ = writeln!(
            canonical,
            "  proposal_digest={}",
            proposal_digest_for(materials.proposal_digests, &change.id)
        );
    }

    AnalysisInputSignature(format!("{:x}", md5::compute(canonical.as_bytes())))
}

fn proposal_digest_for<'a>(digests: &'a HashMap<String, String>, change_id: &str) -> &'a str {
    digests
        .get(change_id)
        .map(String::as_str)
        .unwrap_or(ABSENT_PROPOSAL_DIGEST)
}

fn sorted_list(values: &[String]) -> String {
    let mut sorted: Vec<&str> = values.iter().map(String::as_str).collect();
    sorted.sort_unstable();
    format!("[{}]", sorted.join(","))
}

/// Stable content digest of a `proposal.md` path.
///
/// A missing file is a representable state, not a failure. Any other read error is
/// reported so the caller can fail open instead of suppressing analysis on partial input.
pub(crate) fn proposal_digest_from_path(path: &Path) -> std::result::Result<String, String> {
    match std::fs::read(path) {
        Ok(contents) => Ok(format!("{:x}", md5::compute(&contents))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(ABSENT_PROPOSAL_DIGEST.to_string())
        }
        Err(error) => Err(format!("{}: {}", path.display(), error)),
    }
}

/// Repository-visible probe for the signature material the scheduler cannot derive from its
/// own in-memory state.
///
/// Implementations perform I/O, so probing is bounded by [`SUPPRESSED_PROBE_INTERVAL`]
/// while an input is suppressed.
#[async_trait::async_trait]
pub(crate) trait AnalysisInputProbe: Send + Sync {
    /// Effective dependency-base evidence, named and resolved exactly as dependency
    /// classification evaluates merge evidence.
    async fn base_revision(&self) -> std::result::Result<String, String>;

    /// Stable digest of `openspec/changes/<change_id>/proposal.md`.
    fn proposal_digest(&self, change_id: &str) -> std::result::Result<String, String>;
}

/// Runtime-only record of the last dependency-analysis input that completed with a usable
/// result.
///
/// Timing uses [`tokio::time::Instant`] so the degraded interval is driven by the scheduler's
/// own runtime clock. That makes paused-time tests deterministic and behaves exactly like
/// wall-clock time in a normal build.
#[derive(Debug, Clone)]
pub(crate) struct CompletedAnalysisInput {
    signature: AnalysisInputSignature,
    /// Set only for a recoverable-failure metadata fallback: once this instant passes, the
    /// unchanged input becomes eligible for one retry again.
    degraded_until: Option<Instant>,
}

impl CompletedAnalysisInput {
    /// A healthy LLM result or an intentionally metadata-only result: unchanged input stays
    /// quiescent indefinitely.
    pub(crate) fn healthy(signature: AnalysisInputSignature) -> Self {
        Self {
            signature,
            degraded_until: None,
        }
    }

    /// A recoverable-failure metadata fallback: unchanged input is suppressed only until the
    /// degraded interval expires.
    pub(crate) fn degraded(signature: AnalysisInputSignature, now: Instant) -> Self {
        Self {
            signature,
            degraded_until: Some(now + DEGRADED_SUPPRESSION_TTL),
        }
    }

    pub(crate) fn is_degraded(&self) -> bool {
        self.degraded_until.is_some()
    }

    /// Earliest instant at which this record's input may be probed again.
    ///
    /// The repository probe cadence bounds VCS and filesystem work, but it must not push a
    /// degraded record's promised retry past its expiry: a probe taken at 4m59s would otherwise
    /// delay the five-minute retry to 5m09s. The deadline is therefore the earlier of the two
    /// semantic expiries.
    pub(crate) fn next_probe_deadline(&self, now: Instant) -> Instant {
        let bounded_probe = now + SUPPRESSED_PROBE_INTERVAL;
        match self.degraded_until {
            Some(degraded_until) if degraded_until < bounded_probe => degraded_until,
            _ => bounded_probe,
        }
    }

    /// Whether this record still suppresses an ordinary timer analysis of `signature`.
    pub(crate) fn suppresses(&self, signature: &AnalysisInputSignature, now: Instant) -> bool {
        self.signature == *signature
            && self
                .degraded_until
                .is_none_or(|degraded_until| now < degraded_until)
    }
}

/// Why an ordinary timer evaluation is waiting before it retries dependency analysis.
///
/// Both causes describe an attempt that produced *no* completed input, which is exactly why
/// they cannot be represented by [`CompletedAnalysisInput`]: unavailable evidence and an
/// unusable analyzer result are not analysis completions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoundedRetryCause {
    /// A proposal read or effective-base revision resolution failed.
    SignatureUnavailable,
    /// The analyzer returned no usable order while queued work remained.
    UnusableAnalysisResult,
}

impl BoundedRetryCause {
    /// Stable operator-visible reason slug.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SignatureUnavailable => "analysis_signature_unavailable_retry_pending",
            Self::UnusableAnalysisResult => "unusable_analysis_result_retry_pending",
        }
    }
}

/// Runtime-only deadline that rate-limits an ordinary timer retry.
///
/// Signature construction and unusable results are both fail-open: nothing is recorded as
/// analyzed and the scheduler keeps running. Without a deadline, though, fail-open becomes
/// fail-hot — every 500 ms wake would re-read proposals, spawn a VCS subprocess, and relaunch
/// the analyzer. This bounds that retry to the existing ten-second queue debounce cadence while
/// leaving explicit scheduler edges free to bypass it once per event.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BoundedAnalysisRetry {
    cause: BoundedRetryCause,
    retry_at: Instant,
}

impl BoundedAnalysisRetry {
    pub(crate) fn after(cause: BoundedRetryCause, now: Instant) -> Self {
        Self {
            cause,
            retry_at: now + SUPPRESSED_PROBE_INTERVAL,
        }
    }

    pub(crate) fn cause(&self) -> BoundedRetryCause {
        self.cause
    }

    /// Whether an ordinary timer evaluation at `now` must still wait.
    pub(crate) fn blocks(&self, now: Instant) -> bool {
        now < self.retry_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openspec::{ProposalMetadata, ProposalPriority};

    fn change(id: &str) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: 0,
            total_tasks: 3,
            last_modified: String::new(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        }
    }

    fn digests(entries: &[(&str, &str)]) -> HashMap<String, String> {
        entries
            .iter()
            .map(|(id, digest)| (id.to_string(), digest.to_string()))
            .collect()
    }

    fn signature(
        queued: &[Change],
        in_flight: &[String],
        available_slots: usize,
        base_revision: &str,
        proposal_digests: &HashMap<String, String>,
    ) -> AnalysisInputSignature {
        build_analysis_input_signature(&AnalysisInputMaterials {
            queued,
            in_flight_ids: in_flight,
            available_slots,
            max_parallelism: 3,
            base_revision,
            proposal_digests,
        })
    }

    #[test]
    fn identical_input_produces_equal_signature_regardless_of_iteration_order() {
        let digests = digests(&[("queued-a", "d1"), ("queued-b", "d2"), ("inflight-a", "d3")]);
        let forward = signature(
            &[change("queued-a"), change("queued-b")],
            &["inflight-a".to_string(), "inflight-b".to_string()],
            1,
            "rev-1",
            &digests,
        );
        let reversed = signature(
            &[change("queued-b"), change("queued-a")],
            &["inflight-b".to_string(), "inflight-a".to_string()],
            1,
            "rev-1",
            &digests,
        );

        assert_eq!(
            forward, reversed,
            "signature ordering must be stable so an unchanged input stays suppressed"
        );
    }

    #[test]
    fn same_id_queued_proposal_content_change_invalidates_signature() {
        let queued = vec![change("queued-a")];
        let before = signature(
            &queued,
            &[],
            1,
            "rev-1",
            &digests(&[("queued-a", "digest-before")]),
        );
        let after = signature(
            &queued,
            &[],
            1,
            "rev-1",
            &digests(&[("queued-a", "digest-after")]),
        );

        assert_ne!(
            before, after,
            "an edited proposal with an unchanged change ID must re-arm analysis"
        );
    }

    #[test]
    fn same_id_in_flight_proposal_content_change_invalidates_signature() {
        let in_flight = vec!["inflight-a".to_string()];
        let before = signature(
            &[change("queued-a")],
            &in_flight,
            1,
            "rev-1",
            &digests(&[("queued-a", "d1"), ("inflight-a", "digest-before")]),
        );
        let after = signature(
            &[change("queued-a")],
            &in_flight,
            1,
            "rev-1",
            &digests(&[("queued-a", "d1"), ("inflight-a", "digest-after")]),
        );

        assert_ne!(
            before, after,
            "in-flight proposal content reaches the analyzer prompt and must be part of the input"
        );
    }

    #[test]
    fn proposal_creation_invalidates_signature_recorded_while_absent() {
        let queued = vec![change("queued-a")];
        let absent = signature(&queued, &[], 1, "rev-1", &HashMap::new());
        let present = signature(&queued, &[], 1, "rev-1", &digests(&[("queued-a", "d1")]));

        assert_ne!(
            absent, present,
            "a created proposal must differ from the absent state it replaced"
        );
    }

    #[test]
    fn effective_base_revision_change_invalidates_signature() {
        let queued = vec![change("queued-a")];
        let digests = digests(&[("queued-a", "d1")]);

        assert_ne!(
            signature(&queued, &[], 1, "rev-1", &digests),
            signature(&queued, &[], 1, "rev-2", &digests),
            "dependency integration into the effective base must re-arm analysis"
        );
    }

    #[test]
    fn capacity_and_in_flight_membership_changes_invalidate_signature() {
        let queued = vec![change("queued-a")];
        let digests = digests(&[("queued-a", "d1")]);
        let baseline = signature(&queued, &["inflight-a".to_string()], 0, "rev-1", &digests);

        assert_ne!(
            baseline,
            signature(&queued, &["inflight-a".to_string()], 1, "rev-1", &digests),
            "recovered capacity must re-arm analysis even without a slot-recovery reason"
        );
        assert_ne!(
            baseline,
            signature(&queued, &[], 0, "rev-1", &digests),
            "in-flight membership is part of the analysis input"
        );
    }

    #[test]
    fn queued_analysis_fields_invalidate_signature() {
        let digests = digests(&[("queued-a", "d1")]);
        let baseline = signature(&[change("queued-a")], &[], 1, "rev-1", &digests);

        let mut progressed = change("queued-a");
        progressed.completed_tasks = 2;
        assert_ne!(
            baseline,
            signature(&[progressed], &[], 1, "rev-1", &digests),
            "task progress feeds selection and must be part of the input"
        );

        let mut dependent = change("queued-a");
        dependent.dependencies = vec!["queued-b".to_string()];
        assert_ne!(
            baseline,
            signature(&[dependent], &[], 1, "rev-1", &digests),
            "declared dependencies feed the analyzer prompt"
        );

        let mut prioritized = change("queued-a");
        prioritized.metadata.priority = Some(ProposalPriority::High);
        assert_ne!(
            baseline,
            signature(&[prioritized], &[], 1, "rev-1", &digests),
            "prompt-relevant frontmatter must be part of the input"
        );

        let mut referenced = change("queued-a");
        referenced.metadata.references = vec!["openspec/specs/x/spec.md".to_string()];
        assert_ne!(
            baseline,
            signature(&[referenced], &[], 1, "rev-1", &digests),
            "prompt-relevant references must be part of the input"
        );
    }

    #[test]
    fn queued_id_set_change_invalidates_signature() {
        let digests = digests(&[("queued-a", "d1"), ("queued-b", "d2")]);

        assert_ne!(
            signature(&[change("queued-a")], &[], 1, "rev-1", &digests),
            signature(
                &[change("queued-a"), change("queued-b")],
                &[],
                1,
                "rev-1",
                &digests
            ),
            "a real queue addition must produce a different input"
        );
    }

    #[test]
    fn healthy_record_suppresses_unchanged_input_without_expiry() {
        let queued = vec![change("queued-a")];
        let digests = digests(&[("queued-a", "d1")]);
        let current = signature(&queued, &[], 0, "rev-1", &digests);
        let record = CompletedAnalysisInput::healthy(current.clone());
        let now = Instant::now();

        assert!(!record.is_degraded());
        assert!(record.suppresses(&current, now));
        assert!(
            record.suppresses(&current, now + DEGRADED_SUPPRESSION_TTL * 10),
            "a healthy unchanged input must stay quiescent instead of retrying periodically"
        );
        assert!(
            !record.suppresses(&signature(&queued, &[], 1, "rev-1", &digests), now),
            "a changed input must not be suppressed"
        );
    }

    #[test]
    fn healthy_record_probe_deadline_uses_the_bounded_repository_cadence() {
        let queued = vec![change("queued-a")];
        let digests = digests(&[("queued-a", "d1")]);
        let record =
            CompletedAnalysisInput::healthy(signature(&queued, &[], 0, "main@rev-1", &digests));
        let now = Instant::now();

        assert_eq!(
            record.next_probe_deadline(now),
            now + SUPPRESSED_PROBE_INTERVAL,
            "a healthy record has no expiry to cap its probe cadence"
        );
    }

    #[test]
    fn degraded_record_probe_deadline_is_capped_at_its_expiry() {
        let queued = vec![change("queued-a")];
        let digests = digests(&[("queued-a", "d1")]);
        let recorded_at = Instant::now();
        let record = CompletedAnalysisInput::degraded(
            signature(&queued, &[], 0, "main@rev-1", &digests),
            recorded_at,
        );

        assert_eq!(
            record.next_probe_deadline(recorded_at),
            recorded_at + SUPPRESSED_PROBE_INTERVAL,
            "far from expiry the repository cadence is the binding deadline"
        );

        // One second before expiry, a fresh ten-second probe deadline would push the promised
        // retry out to 5m09s.
        let just_before_expiry = recorded_at + DEGRADED_SUPPRESSION_TTL - Duration::from_secs(1);
        assert_eq!(
            record.next_probe_deadline(just_before_expiry),
            recorded_at + DEGRADED_SUPPRESSION_TTL,
            "the probe cadence must not delay the degraded retry past its expiry"
        );
    }

    #[test]
    fn bounded_retry_blocks_only_until_the_debounce_cadence_elapses() {
        let now = Instant::now();
        let retry = BoundedAnalysisRetry::after(BoundedRetryCause::SignatureUnavailable, now);

        assert_eq!(retry.cause(), BoundedRetryCause::SignatureUnavailable);
        assert!(
            retry.blocks(now + SUPPRESSED_PROBE_INTERVAL - Duration::from_millis(500)),
            "a 500 ms wake inside the window must not re-probe or re-analyze"
        );
        assert!(
            !retry.blocks(now + SUPPRESSED_PROBE_INTERVAL),
            "fail-open must stay open: the deadline has to release the retry"
        );
        assert_ne!(
            BoundedRetryCause::SignatureUnavailable.as_str(),
            BoundedRetryCause::UnusableAnalysisResult.as_str(),
            "the two bounded-retry causes must be distinguishable to operators"
        );
    }

    #[test]
    fn degraded_record_suppresses_only_until_its_fixed_interval_expires() {
        let queued = vec![change("queued-a")];
        let digests = digests(&[("queued-a", "d1")]);
        let current = signature(&queued, &[], 0, "rev-1", &digests);
        let recorded_at = Instant::now();
        let record = CompletedAnalysisInput::degraded(current.clone(), recorded_at);

        assert!(record.is_degraded());
        assert!(
            record.suppresses(&current, recorded_at + Duration::from_secs(299)),
            "a broken analyzer command must not be relaunched on every timer wake"
        );
        assert!(
            !record.suppresses(&current, recorded_at + DEGRADED_SUPPRESSION_TTL),
            "one retry must become eligible after the fixed degraded interval"
        );
    }
}
