---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/changes/archive/2026-07-28-suppress-unchanged-state-timer-reanalysis/proposal.md
  - openspec/changes/archive/2026-07-28-suppress-unchanged-state-timer-reanalysis/design.md
  - src/parallel/analysis_signature.rs
  - src/parallel/dependency.rs
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
  - src/parallel/tests/unchanged_analysis_input.rs
verifications:
  - id: analysis-suppression-liveness-tests
    requirement: Dependency-analysis suppression remains quiescent for unchanged healthy input without starving queued work when effective-base evidence changes, analysis is unusable, or signature probing fails
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test output for full-loop scheduler liveness, effective-base revision invalidation, fail-open throttling, and existing unchanged-input suppression regressions
    rerun: cargo test parallel::tests --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix dependency-analysis suppression liveness

**Change Type**: implementation

## Problem / Context

Conflux v0.6.201 introduced a process-local signature that stops repeated LLM dependency analysis for unchanged ordinary timer input. The core approach is sound, but implementation review found two starvation paths and one failure-path replay risk in the released gate.

First, the production signature probes `WorkspaceManager::get_current_revision()`, which represents the current checkout commit. Dependency classification instead resolves an effective dependency base that may be the original branch or a current integration branch and checks merge evidence against that named base. If the effective-base ref advances while the checkout commit remains unchanged, the completed healthy signature remains equal and timer evaluation can suppress the analysis that would discover newly integrated dependencies.

Second, an analyzer result with an empty `order` returns a terminal flow decision based only on `in_flight.is_empty()`. When queued work remains and no workspace is in flight, this ends the scheduler instead of leaving the unusable result unrecorded and allowing a later eligible retry. The current helper-level regression manually calls another iteration and therefore does not observe the real loop termination.

Third, signature construction is intentionally fail-open, but the failure path clears the next probe deadline. A persistent proposal-read or revision-resolution failure can therefore probe and invoke the analyzer on every 500 ms wake, recreating the rapid loop that suppression was intended to remove. Fail-open must preserve analysis availability without becoming fail-hot.

## Proposed Solution

Tighten the existing analysis-input gate without redesigning its provenance, edge-bypass, or process-local state model.

- Resolve the signature revision from the exact effective dependency-base ref used by dependency classification. The branch-selection rule and revision lookup must have one authoritative implementation or a shared result so signature invalidation and dependency merge evidence cannot diverge.
- Treat an empty or otherwise unusable analysis result as non-terminal while reducer-visible queued work remains. Do not record a completed signature; keep the scheduler alive and permit the next debounce-eligible timer evaluation or explicit edge to retry.
- Preserve legitimate loop termination only when no queued work, in-flight work, pending reducer-owned waiter, merge/push task, or other existing termination blocker remains under the canonical scheduler termination contract.
- Rate-limit signature-unavailable retries to the existing ten-second queue debounce cadence. A failed probe remains fail-open and records no completed signature, but intervening 500 ms wakes must not repeat proposal/VCS probing or LLM analysis.
- Explicit queue addition, completion, repair-candidate, and slot-recovery edges continue to bypass the ordinary timer gate once. They may attempt a fresh fail-open analysis immediately and then return to bounded timer retry behavior.
- Keep healthy unchanged-input suppression non-expiring and preserve degraded fallback semantics. This follow-up does not replace the signature with a cooldown or make a previous `AnalysisResult` authoritative.
- Align analyzer input ordering with signature set semantics where practical: queued and in-flight inputs should be deterministically ordered before both signature construction and analyzer/fallback consumption. This is completion-hardening, not a reason to make the signature sensitive to `HashSet` iteration order.
- Cap a degraded record's next probe deadline at its five-minute expiry so the first timer wake at or after expiry can perform the promised retry rather than waiting up to another ten seconds.
- Set the initial suppressed probe deadline when a completed signature is recorded, avoiding an unnecessary proposal/VCS probe on the immediate 500 ms wake.

These behaviors are tightly coupled in the same ordinary timer gate and full scheduler loop. Splitting them would allow either starvation or rapid retry to remain between changes, so they must ship atomically.

## Acceptance Criteria

- Advancing only the named effective dependency-base ref, while leaving the current checkout commit and queued IDs unchanged, invalidates the completed signature and causes dependency eligibility to be re-evaluated within the bounded timer cadence.
- An empty analyzer `order` with queued work remaining does not terminate the scheduler, does not record a completed signature, and allows another analysis on the next debounce-eligible timer wake or explicit edge.
- A full scheduler-loop regression, not a direct helper reinvocation, proves the empty-result path remains alive and retries.
- Persistent signature revision or proposal-read failure records no signature, keeps the scheduler alive, and permits analysis no more frequently than once per ten-second timer cadence unless a new explicit edge occurs.
- The fail-open throttle does not convert failure into fail-closed behavior: after the deadline, ordinary timer evaluation retries both signature construction and analysis, and a newly successful signature probe can establish normal suppression.
- Unchanged healthy input still invokes the analyzer only once across repeated 500 ms wakes, and no proposal read or VCS revision probe occurs before the first ten-second deadline after completion.
- A degraded recoverable-failure fallback allows one unchanged-input retry on the first eligible wake at or after its five-minute expiry, including when a ten-second probe deadline was established immediately before expiry.
- Deterministic ordering prevents `HashSet` or queue iteration order alone from changing analyzer prompts or fallback dispatch order, while real membership/content/capacity/effective-base changes still invalidate the signature.
- Existing one-shot edge bypass, positive-capacity zero-dispatch retry, process-local reset, and pre-analysis snapshot behavior remain intact.
- No durable out-of-worktree workflow-control state is introduced, preserving Constitution laws 1 and 3.

## Explicit Completion Conditions

- Production signature probing names the same effective dependency base and resolves the same ref revision used by dependency classification; a test fails if checkout `HEAD` is substituted for that ref.
- The real orchestration loop has a tested non-terminal empty-result path while queued work remains and retains the existing fully-drained termination behavior.
- Signature-unavailable state has a typed or otherwise explicit bounded retry deadline independent of completed-signature state; tests count both probe and analyzer calls across paused 500 ms wakes.
- Full-loop paused-time tests cover effective-base-only advancement, persistent probe failure, probe recovery, empty result retry, unchanged healthy suppression, and exact degraded expiry behavior.
- Existing scheduler analysis-input tests continue to pass, including explicit edge bypass, zero-capacity suppression, positive-capacity zero-dispatch liveness, same-ID proposal edits, and fresh-process reset.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` pass. New default tests use paused Tokio time and remain below the one-second unit-test limit.

## Scope Completeness

- User-visible outcome: Conflux avoids both costly repeated analysis and silent scheduler starvation after dependency integration or unusable analyzer output.
- Likely code areas: `src/parallel/dependency.rs`, `src/parallel/queue_state.rs`, `src/parallel/analysis_signature.rs`, `src/parallel/orchestration.rs`, and `src/parallel/tests/unchanged_analysis_input.rs`.
- Verification: repository-local full-loop tests count analyzer invocations, signature probes, retries, dispatch evaluation, and loop termination.
- Migration and rollout: none. State is process-local; active v0.6.201 processes must restart after upgrade.
- Split decision: effective-base invalidation, non-terminal unusable results, and fail-open throttling share one suppression/liveness invariant and must be verified together to avoid replacing one replay mode with another.

## Out of Scope

- Replacing LLM dependency analysis with metadata-only analysis.
- Persisting signatures or analyzer results across process restarts.
- Reusing a previous `AnalysisResult` to authorize dispatch.
- Changing the 500 ms scheduler wake interval or ten-second queue debounce duration.
- Adding speculative long-period retries for unchanged healthy results.
- Redesigning dependency metadata semantics, prompt content, or unrelated scheduler diagnostics.
- Changing the intentional positive-capacity, idle, zero-dispatch rule without a separate specification change.
