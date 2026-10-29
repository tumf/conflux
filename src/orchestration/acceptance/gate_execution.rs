//! Runtime-owned execution of the manifest's declared blocking gates.
//!
//! The reviewer used to own this: it read the proposal's declarations, decided
//! what to run, ran it, and waited. That is exactly the authority this module
//! takes back. Conflux executes every eligible declared gate itself — or reuses
//! exact bound evidence for it — *before* the semantic reviewer starts, and
//! hands the reviewer immutable typed outcomes it cannot rerun or supplement.
//!
//! Two properties make that a boundary rather than a rearrangement:
//!
//! * **The gate set is closed.** It comes from the frozen manifest, so a
//!   reviewer cannot add to it, and a command outside it is never awaited or
//!   counted as blocking evidence.
//! * **The budget is shared and finite.** Gates spend the same seconds semantic
//!   review will spend. A gate that cannot finish inside what remains is
//!   reported as [`GateOutcome::DeadlineExhausted`], which becomes a typed
//!   non-resumable hold — never more waiting, and never the generic runtime
//!   limit.
//!
//! A failing gate is a `fail` verdict about the change. A gate the runtime could
//! not start is not: `Refused` says something about the environment, and the
//! caller routes it to an execution hold or the existing external-prerequisite
//! contract rather than to a verdict.

use std::path::Path;
use std::time::Duration;
use tokio::time::Instant;

use super::execution_manifest::{AcceptanceExecutionManifest, GateOutcome, GateResults};
use super::verification_evidence::{
    evaluate_reuse, CaptureOutcome, Clock, CommandSupervisor, CurrentBindings, EvidenceStore,
    RepositoryFacts, ReuseDecision, ReusePolicy, ReviewBinding, RuntimeVerificationExecutor,
    VerificationRequest,
};

/// Refusal code for a declared command whose executable does not exist here.
///
/// The one refusal that is a *prerequisite* rather than a defect: the repository
/// is fine, the change is fine, and the runtime is fine — the machine is simply
/// missing a tool the declaration names. It keeps the existing resumable
/// external-prerequisite contract instead of becoming a non-resumable hold.
pub const TOOL_UNAVAILABLE: &str = "tool_unavailable";

/// How much of the shared budget the gates may spend before review starts.
#[derive(Debug, Clone, Copy)]
pub struct GateBudget {
    /// Remaining work seconds, or `None` when the absolute limit is disabled.
    pub remaining: Option<Duration>,
}

impl GateBudget {
    /// An unbounded budget, used when `acceptance_max_runtime_secs` is `0`.
    pub fn unbounded() -> Self {
        Self { remaining: None }
    }

    pub fn bounded(remaining: Duration) -> Self {
        Self {
            remaining: Some(remaining),
        }
    }

    fn is_exhausted(&self) -> bool {
        self.remaining.is_some_and(|remaining| remaining.is_zero())
    }

    fn consume(&mut self, elapsed: Duration) {
        if let Some(remaining) = self.remaining.as_mut() {
            *remaining = remaining.saturating_sub(elapsed);
        }
    }

    /// Mark the budget spent.
    ///
    /// A gate that hit its own timeout consumed everything that was left by
    /// definition, and saying so explicitly is what keeps the remaining gates
    /// from being started against a budget that elapsed-time arithmetic might
    /// round back up to non-zero.
    fn exhaust(&mut self) {
        if let Some(remaining) = self.remaining.as_mut() {
            *remaining = Duration::ZERO;
        }
    }
}

/// Run the manifest's declared gates, reusing exact bound evidence where valid.
///
/// Gates run in canonical manifest order and stop at the first blocking failure:
/// a `fail` verdict is already determined at that point, and spending the shared
/// budget on the remaining gates would only shrink the review window that has to
/// explain it.
///
/// `reuse_decisions` carries the per-gate reuse evaluation the caller already
/// performed against live Git state. Passing it in rather than recomputing it
/// keeps one observation of the repository behind both the manifest's artifact
/// bindings and the execution decision, so the two cannot disagree.
pub async fn execute_declared_gates<F, S, C>(
    executor: &RuntimeVerificationExecutor<F, S, C>,
    manifest: &AcceptanceExecutionManifest,
    reuse_decisions: &[ReuseDecision],
    mut budget: GateBudget,
) -> GateResults
where
    F: RepositoryFacts,
    S: CommandSupervisor,
    C: Clock,
{
    // An invalid required manifest offers no gate to run or reuse. Its gate list
    // is already empty by construction; this makes the refusal independent of
    // that invariant.
    if manifest.is_declaration_invalid() {
        return GateResults::default();
    }
    let mut outcomes = Vec::new();
    for gate in &manifest.gates {
        let id = gate.verification_id.as_str();
        if budget.is_exhausted() {
            // Never started, so there is nothing of this gate's to clean up. The
            // gate that actually spent the budget carries the real sweep
            // evidence.
            outcomes.push(GateOutcome::DeadlineExhausted {
                verification_id: id.to_string(),
                cleanup_confirmed: true,
                cleanup_diagnostics: "the gate was never started".to_string(),
            });
            continue;
        }

        let reusable = reuse_decisions
            .iter()
            .find(|decision| decision.verification_id() == id)
            .and_then(|decision| match decision {
                ReuseDecision::Reuse { evidence, .. } => Some(evidence.as_ref()),
                ReuseDecision::Rerun { .. } => None,
            });
        if let Some(evidence) = reusable {
            outcomes.push(GateOutcome::Reused {
                verification_id: id.to_string(),
                artifact_path: evidence.artifact_path.clone(),
                elapsed_seconds: evidence.elapsed_seconds().unwrap_or_default(),
            });
            continue;
        }

        if gate.tool.path.trim().is_empty() {
            // The declared executable could not be resolved when the manifest was
            // frozen. That is a named non-repository prerequisite, not a runtime
            // defect and not a defect in the change, so it is reported with its
            // own code and classified as a resumable external hold below.
            outcomes.push(GateOutcome::Refused {
                verification_id: id.to_string(),
                code: TOOL_UNAVAILABLE.to_string(),
                detail: format!(
                    "declared command '{}' could not be resolved to an executable on PATH",
                    gate.argv.first().map_or("", String::as_str)
                ),
            });
            continue;
        }

        let request = gate.request();
        let started = Instant::now();
        let outcome = run_one_gate(executor, &request, budget.remaining).await;
        if matches!(outcome, GateOutcome::DeadlineExhausted { .. }) {
            budget.exhaust();
        } else {
            budget.consume(started.elapsed());
        }
        let blocking_failure = outcome.is_failure();
        outcomes.push(outcome);
        if blocking_failure {
            // The verdict is decided; remaining gates would only consume the
            // review budget that has to explain the failure.
            for remaining in manifest
                .gates
                .iter()
                .skip_while(|candidate| candidate.verification_id != gate.verification_id)
                .skip(1)
            {
                outcomes.push(GateOutcome::Refused {
                    verification_id: remaining.verification_id.clone(),
                    code: "blocking_gate_already_failed".to_string(),
                    detail: format!(
                        "not executed: '{id}' already failed, so the Acceptance verdict is \
                         determined"
                    ),
                });
            }
            break;
        }
    }
    GateResults { outcomes }
}

/// Run exactly one gate under the remaining shared budget.
async fn run_one_gate<F, S, C>(
    executor: &RuntimeVerificationExecutor<F, S, C>,
    request: &VerificationRequest,
    remaining: Option<Duration>,
) -> GateOutcome
where
    F: RepositoryFacts,
    S: CommandSupervisor,
    C: Clock,
{
    let id = request.verification_id.clone();
    // The budget is handed to the capture rather than wrapped around it. A
    // `timeout` here would drop the capture future mid-run and leave the
    // declared command alive in the worktree, with nobody left to reap it — and
    // the hold written afterwards would claim a cleanup nobody performed.
    // Pushing the bound down is what makes expiry carry real termination
    // evidence instead of an assumption.
    match executor.capture_bounded(request, remaining).await {
        CaptureOutcome::Captured { evidence, .. } => GateOutcome::Executed {
            verification_id: id,
            artifact_path: evidence.artifact_path.clone(),
            elapsed_seconds: evidence.elapsed_seconds().unwrap_or_default(),
        },
        CaptureOutcome::CommandFailed {
            exit_code,
            artifact_path,
            ..
        } => GateOutcome::Failed {
            verification_id: id,
            exit_code,
            artifact_path,
        },
        CaptureOutcome::Refused(refusal) => GateOutcome::Refused {
            verification_id: id,
            code: refusal.code.to_string(),
            detail: refusal.detail,
        },
        // Expiry is the operation's own boundary, not the command's verdict:
        // nothing is recorded as passing or failing, and the caller turns this
        // into the typed gate-deadline hold — carrying the sweep's own answer to
        // "is it actually gone".
        CaptureOutcome::DeadlineExhausted { cleanup, .. } => GateOutcome::DeadlineExhausted {
            verification_id: id,
            cleanup_confirmed: cleanup.is_confirmed(),
            cleanup_diagnostics: cleanup.diagnostics(),
        },
    }
}

/// What the runtime-owned gate phase decided about this invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateVerdict {
    /// Every declared gate succeeded (or none was declared). Semantic review runs.
    Proceed,
    /// The proposal's required declarations could not all be supervised. A
    /// `fail` verdict about the candidate's own `proposal.md`: no gate ran, no
    /// evidence was reused, and no reviewer is started.
    DeclarationInvalid {
        defects: Vec<super::verification_evidence::DeclarationDefect>,
    },
    /// A declared gate failed. A `fail` verdict about the candidate, with the
    /// failing gate's own evidence.
    Failed { evidence: Vec<String> },
    /// A named non-repository prerequisite stopped a declared gate. Keeps the
    /// existing resumable external-prerequisite contract.
    ExternalPrerequisite {
        blocker: crate::acceptance::AcceptanceBlocker,
    },
    /// The runtime could not complete the boundary. Non-resumable execution hold.
    Hold {
        category: super::execution_manifest::AcceptanceHoldCategory,
        evidence: Vec<String>,
    },
}

/// Classify the gate phase's typed outcomes into one routing decision.
///
/// Precedence is deliberate and fixed:
///
/// 1. **External prerequisite.** A missing declared tool is a wait on something
///    outside the repository, and the spec requires that contract to outrank
///    runtime/manifest-defect classification — otherwise an unavailable tool
///    would harden into a non-resumable hold nobody can clear by installing it.
/// 2. **Blocking failure.** A gate that ran and failed is a real statement about
///    the candidate, and it must reach the repair cycle rather than a hold.
/// 3. **Gate deadline exhaustion**, then **any other refusal**. Both are
///    runtime/environment facts, and neither is ever a finding against the
///    change.
pub fn classify_gate_results(results: &GateResults) -> GateVerdict {
    use super::execution_manifest::AcceptanceHoldCategory;

    if let Some(GateOutcome::Refused {
        verification_id,
        detail,
        ..
    }) = results.outcomes.iter().find(
        |outcome| matches!(outcome, GateOutcome::Refused { code, .. } if code == TOOL_UNAVAILABLE),
    ) {
        return GateVerdict::ExternalPrerequisite {
            blocker: crate::acceptance::AcceptanceBlocker {
                category: "infrastructure".to_string(),
                evidence: vec![format!(
                    "declared change-blocking verification '{verification_id}' could not run: \
                     {detail}"
                )],
                unblock_condition: format!(
                    "the executable named by declared verification '{verification_id}' resolves on \
                     PATH in this workspace"
                ),
                next_action: format!(
                    "Install or expose the tool the '{verification_id}' declaration names, then \
                     retry acceptance."
                ),
                resumable: true,
                prerequisite_owner: None,
                evidence_ids: vec![verification_id.clone()],
            },
        };
    }
    if let Some(failure) = results.first_failure() {
        return GateVerdict::Failed {
            evidence: vec![failure.summary()],
        };
    }
    if results.first_deadline_exhausted().is_some() {
        return GateVerdict::Hold {
            category: AcceptanceHoldCategory::DeclaredGateDeadlineExhausted,
            evidence: results.summary_lines(),
        };
    }
    if results.first_refusal().is_some() {
        return GateVerdict::Hold {
            category: AcceptanceHoldCategory::RuntimeDefect,
            evidence: results.summary_lines(),
        };
    }
    GateVerdict::Proceed
}

/// Classify a whole boundary: the frozen manifest plus its gate results.
///
/// Declaration defects outrank everything [`classify_gate_results`] can say,
/// `tool_unavailable` included: a defective declaration was never resolved
/// against PATH, so no prerequisite or runtime classification about it exists.
pub fn classify_boundary(
    manifest: &AcceptanceExecutionManifest,
    results: &GateResults,
) -> GateVerdict {
    if manifest.is_declaration_invalid() {
        return GateVerdict::DeclarationInvalid {
            defects: manifest.declaration_defects.clone(),
        };
    }
    classify_gate_results(results)
}

/// Evaluate reuse for every manifest gate against one observation of Git state.
///
/// Returns a decision per gate in manifest order. A gate with no stored
/// envelope, or one whose bindings moved, decides `Rerun` — which the executor
/// above turns into a real supervised run rather than into an assumption.
pub async fn plan_manifest_reuse<F: RepositoryFacts>(
    facts: &F,
    workspace: &Path,
    store: &EvidenceStore,
    manifest: &AcceptanceExecutionManifest,
    policy: ReusePolicy,
) -> Vec<ReuseDecision> {
    use super::verification_evidence::{dirty_entries, EvidenceDefect, RerunReason};

    let mut decisions = Vec::new();
    // No evidence record is read for an invalid required manifest.
    if manifest.is_declaration_invalid() {
        return decisions;
    }
    // One status read for every gate: two reads could straddle a write and let
    // one gate believe the tree was clean while another saw it dirty.
    let dirty = match facts.porcelain_status(workspace).await {
        Ok(status) => dirty_entries(&status),
        Err(error) => {
            for gate in &manifest.gates {
                decisions.push(ReuseDecision::Rerun {
                    verification_id: gate.verification_id.clone(),
                    reason: RerunReason::Unobservable(error.clone()),
                });
            }
            return decisions;
        }
    };

    for gate in &manifest.gates {
        let request = gate.request();
        let record = match store.load(&request.verification_id) {
            Ok(record) => record,
            Err(defect) => {
                decisions.push(ReuseDecision::Rerun {
                    verification_id: request.verification_id,
                    reason: RerunReason::Defect(defect),
                });
                continue;
            }
        };
        if !dirty.is_empty() {
            decisions.push(ReuseDecision::Rerun {
                verification_id: request.verification_id,
                reason: RerunReason::DirtyWorktree(dirty.clone()),
            });
            continue;
        }
        // The artifact is hashed out of the external store. A cache an operator
        // deleted — or one a moved state root no longer reaches — is exactly an
        // unreadable artifact, and the only thing that may follow from it is a
        // rerun.
        let artifact_digest = facts
            .hash_file(workspace, &store.artifact_path(&request.verification_id))
            .await
            .ok();
        if artifact_digest.is_none() {
            decisions.push(ReuseDecision::Rerun {
                verification_id: request.verification_id,
                reason: RerunReason::Defect(EvidenceDefect::Unreadable(format!(
                    "artifact '{}' is absent or unreadable in the external store",
                    record.artifact_path
                ))),
            });
            continue;
        }
        let current = CurrentBindings {
            commit_oid: manifest.candidate_commit_oid.clone(),
            tree_oid: manifest.candidate_tree_oid.clone(),
            // Taken from the frozen manifest, so a base branch that moves
            // mid-review cannot make stale evidence look current. An envelope
            // bound to a different base or range decides `Rerun` with the
            // mismatching field named. Always `Some` here: the manifest is a
            // review, even when its base did not resolve.
            review: Some(ReviewBinding {
                base_commit: manifest.review_base_commit.clone(),
                range: manifest.review_range.clone(),
            }),
            automation_blob_oid: gate.automation_blob_oid.clone(),
            tool: gate.tool.clone(),
            clean: true,
            artifact_digest,
        };
        decisions.push(evaluate_reuse(&request, &record, &current, policy));
    }
    decisions
}

#[cfg(test)]
#[path = "gate_execution/tests.rs"]
mod tests;
