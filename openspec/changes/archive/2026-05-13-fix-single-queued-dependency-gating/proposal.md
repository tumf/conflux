---
change_type: implementation
priority: high
dependencies: []
references:
  - src/analyzer.rs
  - src/parallel/queue_state.rs
  - src/dependency_targets.rs
  - src/parallel/tests/executor.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/CONSTITUTION.md
---

# Fix single queued dependency gating

**Change Type**: implementation

## Problem/Context

When only one change is queued, the analyzer intentionally skips the LLM `analyze_command` fast path because there is no multi-change ordering to infer. That optimization must not bypass dependency enforcement: a single queued change can still declare proposal metadata dependencies that must be resolved before apply starts.

The canonical parallel-execution spec already requires metadata dependencies to be authoritative and requires the single-change fast path to preserve them. The observed failure mode is that a lone queued dependent change can proceed toward apply without first proving that its dependency is resolved on the base branch.

The fix must preserve the Constitution's workspace-local workflow state rule: dependency gating decisions must be derived from repository/workspace file and git state, not logs, caches, or external durable runtime state.

## Proposed Solution

Tighten analyzer and scheduler behavior so a single queued change with metadata dependencies is handled exactly like a multi-change dependent change for dispatch eligibility.

- Preserve metadata dependencies in the single-change analyzer fast path and fallback analysis paths.
- Before dispatching any selected change, including a lone queued change, classify and evaluate every dependency target.
- Treat archived dependencies as already satisfied.
- Treat queued, in-flight, active-but-not-queued, missing, rejected, terminal-error, and unresolved dependencies as blocking until repository-local evidence proves they are resolved on the base branch or explicitly terminal.
- Emit dependency-blocked diagnostics/events instead of allowing apply to start when unresolved dependencies remain.
- Add regression coverage proving `ApplyStarted` is not emitted for a lone queued dependent change whose dependency is still active/unmerged.

## Acceptance Criteria

- A single queued change that declares an unmerged active dependency is not dispatched to apply.
- A single queued change that declares a queued or in-flight dependency remains dependency-blocked until the dependency is merged to the base branch.
- A single queued change that declares an archived dependency may dispatch once no other unresolved dependencies remain.
- A single queued change that declares a missing or rejected dependency fails closed and does not dispatch.
- Dependency-blocked events and diagnostics identify the unresolved dependency target rather than silently omitting the edge.
- The LLM `analyze_command` may still be skipped for a one-change working set, but skipping LLM analysis must not skip dependency gating.
- All decisions remain based on workspace-local repository and git evidence.

## Explicit Completion Conditions

- `src/analyzer.rs` has regression coverage proving the single-change fast path returns an `AnalysisResult` containing metadata dependencies without invoking the LLM command.
- `src/parallel/queue_state.rs` or its dependency-target helpers gate lone queued dependent changes before `dispatch_change_to_workspace()` can start apply.
- `src/parallel/tests/executor.rs` or equivalent tests prove a one-change queue with `route -> policy`, where `policy` exists as an active unmerged change outside the queue, emits dependency-blocked behavior and does not emit `ApplyStarted`.
- Tests cover archived, missing/rejected, and in-flight dependency classifications for the single queued path.
- Default test/lint/typecheck commands required by the repository pass, including the focused dependency gating tests and `cargo test --lib` or the repository's equivalent Rust test command.
- `cflx openspec validate fix-single-queued-dependency-gating --strict --evidence warn` passes without evidence warnings.

## Out of Scope

- Requiring LLM `analyze_command` execution for one-change queues.
- Changing dependency declarations or proposal metadata syntax.
- Introducing non-workspace durable state for dependency gating.
- Changing TUI/Web display vocabulary except as required to surface existing dependency-blocked events.
