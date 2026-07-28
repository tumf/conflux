---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/changes/archive/2026-07-28-prevent-repeated-resolve-completion-analysis/design.md
  - src/analyzer.rs
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
  - src/parallel/dynamic_queue.rs
  - src/parallel/tests/reanalysis_trigger_lifetime.rs
verifications:
  - id: scheduler-analysis-gate-tests
    requirement: Unchanged timer wakes do not repeat dependency analysis while explicit scheduler edges and repository-visible state changes retain autonomous progress
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test output for scheduler analysis-input signature and loop-level timer regressions
    rerun: cargo test parallel::tests --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Suppress unchanged-state timer reanalysis

**Change Type**: implementation

## Problem / Context

Conflux v0.6.200 correctly consumes `ResolveCompletion`, `RepairCandidate`, and `SlotRecovery` as one-shot edge triggers, but a live `latch` run still launched dependency-analysis agents indefinitely. The corrected run no longer retained `trigger=resolve_completion`; instead, ordinary `Initial` timer evaluations repeatedly analyzed the same 12 queued changes while three in-flight changes consumed all dispatch capacity.

The current queue debounce compares the current time only with `last_queue_change_at`. Once ten seconds have elapsed since the last queue change, every later timer wake passes the debounce check forever. It is neither an analysis cooldown nor an unchanged-input gate. Analysis duration only makes the loop conspicuous: even a short analysis would repeat after the one-way debounce threshold had elapsed.

The live evidence showed stable queued and in-flight sets, zero available slots, no apply dispatch, and repeated 20–42 second LLM analyses beginning again roughly 1.4–6.2 seconds after completion. Diagnostic deduplication reduced repeated logs but did not prevent the expensive analysis calls.

The previous change intentionally rejected a queued-ID-only signature because the same IDs can require fresh analysis after repository-visible dependency integration. This follow-up addresses that concern by gating only ordinary timer-driven analysis on a snapshot of the actual analysis inputs and repository-visible integration context. Explicit scheduler edges remain authoritative re-analysis triggers and bypass the unchanged-input gate.

## Proposed Solution

Add process-local scheduler bookkeeping for the last completed dependency-analysis input and suppress only ordinary timer-driven analysis when the current input is unchanged.

- Define a deterministic analysis-input signature from the queued changes as presented to analysis, sorted in-flight IDs, available capacity, and repository-visible effective dependency-base revision or an equivalently authoritative workspace/git generation.
- Include proposal-derived analysis inputs rather than only queued IDs. The signature must change when dependencies, priority/references used by the prompt, task/progress fields used by selection, or proposal content observable to the configured analyzer changes.
- Capture the signature immediately before invoking dependency analysis and record that captured value only after the scheduler receives a usable analysis result, including the existing metadata-dependency fallback result.
- For an ordinary non-bypass `Initial` timer evaluation, skip dependency analysis when the current signature equals the last completed signature. Emit a deduplicated observable reason for the skip.
- Do not apply the signature gate to real queue additions, completion, repair-candidate, or slot-recovery edges. Those events retain one immediate evaluation per edge even if their analysis input signature is otherwise equal.
- When queued inputs, in-flight membership, available capacity, or repository-visible integration evidence changes, the signature differs and timer evaluation may analyze again without requiring user action.
- Preserve the v0.6.200 one-shot trigger lifetime fix. The one-shot edge layer and unchanged ordinary-timer input layer solve separate replay mechanisms and are both required.
- Keep all signature state in memory. Do not persist it or use logs/caches as authoritative workflow state.

The implementation should reuse existing queue classification and reconciliation before the signature decision. Cheap repository-visible classification remains allowed on timer wakes; the gate prevents only a repeated expensive dependency-analysis invocation for an already analyzed input.

## Acceptance Criteria

- With queued work, full dispatch capacity, and an unchanged analysis-input signature, one ordinary timer-driven dependency analysis completes and any number of later timer wakes cause no additional analyzer invocation.
- The suppression rule holds regardless of whether the analyzer takes more or less than the ten-second queue debounce period.
- A real queue addition immediately permits one analysis and cannot be suppressed by a prior matching signature.
- Each new completion, repair-candidate, or slot-recovery edge immediately permits one analysis and retains the existing one-shot consumption behavior.
- A change in queued analysis input, in-flight membership, available capacity, or effective dependency-base revision permits re-analysis and capacity recovery dispatch without user action.
- Analysis input changes that retain the same change ID, such as proposal dependency or prompt-relevant metadata changes, invalidate the previous signature.
- An LLM command failure that successfully produces the existing metadata-dependency fallback is treated as a completed degraded analysis for unchanged-state suppression; timer wakes do not repeatedly invoke the failing LLM for the same input.
- A terminal analyzer path that produces no usable analysis result does not falsely establish a completed-input signature.
- Queue classification, reducer reconciliation, dependency blocker checks, and operator-visible diagnostics remain available before an expensive analysis is suppressed.
- No durable out-of-worktree workflow-control state is added, and process restart begins with no prior analysis signature.

## Explicit Completion Conditions

- Scheduler runtime state has a typed, deterministic analysis-input signature with documented ordering and repository revision semantics; it is not stored in `~/.local/state/cflx` or another durable location.
- Ordinary timer analysis checks the signature after queue classification/reconciliation and debounce eligibility but before analyzer invocation; explicit bypass reasons do not use this suppression path.
- Signature capture uses the pre-analysis snapshot. A queue or repository change occurring during analysis cannot be hidden by recomputing and storing only the post-analysis state.
- The existing metadata fallback path communicates a usable result consistently enough that the scheduler records the completed input once and does not retry the same failing LLM on every timer wake.
- Loop-level paused-time regression coverage reproduces the v0.6.200 state with an analyzer invocation counter and fails if unchanged timer wakes invoke analysis more than once.
- Regression coverage proves queue addition, completion edge, capacity recovery, same-ID proposal input change, effective-base revision change, and process-local reset each re-arm analysis as specified.
- Existing one-shot trigger, queue debounce, zero-capacity analysis, metadata fallback, dependency integration, and direct-call scheduler tests retain their original behavior unless this proposal explicitly changes the ordinary timer expectation.
- `cargo fmt --check`, `cargo clippy -- -D warnings`, and default-path Rust tests pass; timer tests use paused Tokio time and complete within the repository's one-second unit-test policy.

## Scope Completeness

- User-visible outcome: Conflux stops launching costly no-progress analysis agents indefinitely while continuing autonomous work when scheduler or repository evidence changes.
- Likely code areas: `src/parallel/orchestration.rs`, `src/parallel/queue_state.rs`, a small runtime signature type near scheduler state, analyzer/fallback result plumbing if required, and parallel scheduler tests.
- Verification: repository-local loop-level tests count analyzer invocations and dispatch attempts; unit tests cover deterministic signature invalidation.
- Migration and rollout: none. Existing runs must restart to acquire the new runtime behavior; no persisted state is read or migrated.
- Split decision: timer gating, signature definition, fallback completion semantics, and liveness tests must ship atomically because omitting any one can restore either repeated analysis or hidden starvation.

## Out of Scope

- Reverting the v0.6.200 one-shot edge-trigger fix.
- Disabling all dependency analysis whenever available capacity is zero.
- Replacing LLM dependency analysis with metadata-only analysis.
- Changing the ten-second queue-coalescing debounce or 500 ms scheduler wake interval merely to reduce frequency.
- Persisting analysis results or signatures across process restarts.
- Caching and reusing a previous LLM `AnalysisResult` for dispatch; this change suppresses duplicate invocation but does not make prior analysis output authoritative workflow state.
- Redesigning analyzer prompts, dependency semantics, or diagnostic deduplication beyond adding the unchanged-input suppression reason.
