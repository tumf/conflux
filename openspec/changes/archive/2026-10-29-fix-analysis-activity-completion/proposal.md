---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/remote-control-api/spec.md
  - src/parallel/queue_state.rs
  - src/orchestration/execution_facts.rs
verifications:
  - id: producer-completion
    requirement: The actual scheduler analysis producer pairs each returned invocation with one completion, including fallback and empty order
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/parallel/queue_state.rs
    evidence: "cargo test --locked --offline --lib --features web-monitoring analysis_activity_completion -- --nocapture"
    rerun: "cargo test --locked --offline --lib --features web-monitoring analysis_activity_completion -- --nocapture"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: analysis-policy-controls
    requirement: Empty order retry and scheduler liveness remain unchanged
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/parallel/tests/analysis_liveness_loop.rs
    evidence: "cargo test --locked --offline --lib --features web-monitoring analysis_liveness_loop -- --nocapture"
    rerun: "cargo test --locked --offline --lib --features web-monitoring analysis_liveness_loop -- --nocapture"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: analysis-input-controls
    requirement: Unchanged-input suppression and result provenance remain unchanged
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/parallel/tests/unchanged_analysis_input.rs
    evidence: "cargo test --locked --offline --lib --features web-monitoring unchanged_analysis_input -- --nocapture"
    rerun: "cargo test --locked --offline --lib --features web-monitoring unchanged_analysis_input -- --nocapture"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: execution-observation-controls
    requirement: Existing process activity lifecycle projections remain truthful
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/orchestration/execution_facts/tests.rs
    evidence: "cargo test --locked --offline --lib --features web-monitoring agent_execution_observability -- --nocapture"
    rerun: "cargo test --locked --offline --lib --features web-monitoring agent_execution_observability -- --nocapture"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Close returned dependency-analysis activity at its actual producer

**Change Type**: implementation

## Problem and current cause

At develop `0444fd4c4f32be306f072619a1d2017393d457cb`, `ParallelExecutor::run_dependency_analysis_attempt` emits `AnalysisStarted`, awaits the analyzer, and returns either empty-order handling or a completed outcome without emitting the existing `AnalysisCompleted`. `ExecutionFactsStore` correctly retains `DependencyAnalysis` until its closing event or a process terminal. Thus a persistent scheduler can complete all real work yet continue publishing `active_activities: [dependency_analysis]` and `has_active_work: true`.

A fresh isolated real-owner observation on that fixed revision completed Apply, one declared verification, reviewer, archive and final merge with deterministic synthetic lifecycle workers; native metadata-only analysis invoked no analyzer subprocess. Four subsequent coherent observations had no changes, persistent-idle true, and stale analysis activity. This is an observation defect, not evidence that a child is running and not a remaining proof of the already repaired blocked-only graceful Stop hang. Stop later settled normally. No production owner was replaced.

## Selected solution and boundary

In `src/parallel/queue_state.rs`, immediately after the actual `analyzer(...).await` returns, emit the existing presentation event `ParallelEvent::AnalysisCompleted` once, before any empty-order return or subsequent capacity/dispatch branch. Its existing `groups_found` is the returned legacy `result.groups` length or zero when absent; do not substitute order length or invent parallel groups. The event means the invocation returned, not that its result was usable, successful LLM output, or completed workflow work.

Preserve `AnalysisOutcome` and provenance, analysis input suppression, bounded retry, scheduling, queue/reducer mutations, stop/cancel behavior and all wire schemas. Use the existing event bridge and facts-store handler. Do not infer completion from frontend idle, a timer, a log, process name, or scheduler liveness; do not clear all activities. No added setting, event variant, sidecar, durable state, or target-worktree runtime artifact.

Allowed product change: the one producer event placement and minimal explanatory comments. Test surfaces: new `src/parallel/tests/analysis_activity_completion.rs`, its module registration, and only narrowly needed test support in the same scheduler test harness. Existing policy and observation tests remain controls, not permission to change runtime behavior elsewhere. If the observed producer event cannot close the real facts store through current dispatch, report the concrete blocker instead of adding another authority.

## Acceptance Criteria and Explicit Completion Conditions

1. Each normal returned attempt emits exactly one Started then one Completed, including healthy output, intentional metadata-only output, recoverable-failure fallback and empty order. Empty order retains bounded retry and never implies AllCompleted. Groups Some and None are verified without changing field spelling.
2. A channel-gated analyzer leaves analysis activity open while its future is pending. Releasing its result produces the one closing event from production code. Dropping the invocation future before it returns does not fabricate a completion; existing process-terminal cleanup owns retirement. If the analyzer handles cancellation internally and returns an outcome, that is a returned invocation and does emit completion.
3. Feed actual producer-emitted events through the real dispatcher and `ExecutionFactsStore`: after normal completion `DependencyAnalysis` is absent. If analysis was the only activity, `has_active_work` is false; an unrelated active phase remains active. Tests must never manufacture `AnalysisCompleted` to prove this path.
4. Usable/empty/fallback results and existing retry/suppression decisions are unchanged. Include a real scheduler-pass/loop observation with no runnable phase or zero capacity, so closing the activity does not require workflow completion or Stop. No AllCompleted or Stopped may be emitted solely to close analysis.
5. All four declared gates execute nonzero tests, with selected counts recorded separately for each selector; new regression tests fail with the producer emission removed. Synchronize via channels/barriers/event order, with generous hang safeguards only, not short elapsed-time assertions or sleeps. No ignore, single-thread workaround, broad serial annotation, or disabled assertion may hide a regression. Each new default test respects the repository's one-second target.
6. Change-scoped diff review confirms the boundary. Strict validation is proposal evidence, not implementation proof. Tests live outside the active change directory so the gate remains valid after archive.

## Scope and non-goals

This independent change consumes no unintegrated proposal output. The blocked-only Stop fix, Stop-cancel race, Web error details, test fixture hardening, post-dequeue wake and any active cancellation redesign are separate scopes. No mark/Start UI change, authentication, deployment, owner restart, publication, release, polling policy, new downstream-agent configuration or constitution change.

The tracked `.pre-commit-config.yaml` has path-filtered Rust hooks, not an unconditional substitute for these focused gates. Do not duplicate broad hook commands as checkbox tasks. Parent post-integration verification may repeat the fresh isolated real-owner completion observation; it is distinct from code-local Acceptance and from real-AI or deployed acceptance.
