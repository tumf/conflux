---
change_type: implementation
priority: high
dependencies: []
references:
  - src/dependency_targets.rs
  - src/openspec.rs
  - src/openspec_cmd.rs
  - src/analyzer.rs
  - src/parallel/queue_state.rs
  - src/tui/state.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/cli/spec.md
---

# Fix Terminal Dependency Blocked Retry

**Change Type**: implementation

## Premise / Context

- A downstream repository showed `adopt-alt-residual-stateful-pair` unable to run because it depends on `add-pair-optimization-dataset-capture`.
- The dependency target directory still contains `proposal.md`, but also contains `REJECTED.md`, so execution discovery skips it while `cflx openspec list` can still render it as active/pending.
- Conflux logs showed repeated analysis failures and repeated blocked diagnostics for the same unchanged blocker set.
- Conflux constitution requires workflow-control decisions to be derived from workspace/base git state, not external logs or hidden durable state.

## Problem / Context

Conflux currently distinguishes queued, in-flight, archived, and missing dependency targets, but it does not consistently model a dependency target whose active change directory has a committed `REJECTED.md` marker. That creates two operator-facing problems:

1. CLI status can imply the dependency is still pending even though execution discovery treats the marker-bearing change as terminal and not runnable.
2. The scheduler may repeatedly re-analyze and re-emit the same missing/blocked diagnostics for a dependent change whose blocker set has not changed.

The correct behavior is still fail-closed: a dependent change must not dispatch while its dependency is missing or rejected. The problem is the misleading classification and noisy retry loop, not the dispatch block itself.

## Proposed Solution

Add explicit terminal dependency handling for rejected dependency targets and suppress repeated diagnostics for unchanged dependency blockers.

The change will:

- Classify `openspec/changes/<id>/REJECTED.md` dependency targets separately from `missing`.
- Surface rejected dependency status in `cflx openspec list`, `show`, and `show --json` instead of rendering it as pending or missing.
- Keep rejected and missing dependencies fail-closed for dispatch.
- Emit an operator-visible diagnostic the first time a change is blocked by a given dependency blocker signature.
- Avoid re-emitting the same blocked/error diagnostics on every scheduler loop while the blocker signature is unchanged.
- Re-emit diagnostics and re-evaluate dispatch when repository-visible blocker evidence changes, such as a dependency becoming archived, in-flight, active, rejected, or absent.

## Acceptance Criteria

- A dependency target with `openspec/changes/<dep>/proposal.md` and `openspec/changes/<dep>/REJECTED.md` is classified as rejected, not pending or generic missing.
- `cflx openspec list`, `cflx openspec show <change>`, and `cflx openspec show --json <change>` expose rejected dependency status for dependent changes.
- The scheduler never dispatches a change whose dependency blocker is rejected or missing.
- The scheduler emits a clear first diagnostic for rejected and missing blockers that names the change, dependency id, and dependency class.
- Repeated scheduler loops with the same blocked change and same dependency blocker signature do not append duplicate operator-visible error/warn log entries.
- If the blocker signature changes, Conflux emits a new diagnostic and re-evaluates the change using the new dependency classification.
- Existing queued, in-flight, and archived dependency behavior remains intact.

## Explicit Completion Conditions

The change is complete only when repository-verifiable evidence shows:

- `src/dependency_targets.rs` or equivalent shared dependency classification code includes rejected dependency target evidence derived from workspace/base git state.
- `src/openspec_cmd.rs` reports rejected dependency status consistently in human-readable and JSON OpenSpec utility output.
- `src/analyzer.rs`, `src/parallel_run_service.rs`, and `src/parallel/queue_state.rs` preserve fail-closed behavior for rejected and missing dependency blockers while avoiding misleading generic parse/missing collapse where rejected evidence exists.
- `src/parallel/queue_state.rs` or an equivalent scheduler layer deduplicates unchanged dependency-blocked diagnostics without suppressing state changes or genuine unblock events.
- Focused Rust tests cover rejected dependency classification, CLI status rendering, dispatch blocking, diagnostic deduplication, and unblock/reclassification behavior.
- `cflx openspec validate fix-terminal-dependency-blocked-retry --strict` passes.

## Out of Scope

- Automatically resuming or repairing rejected changes.
- Treating rejected dependencies as satisfied.
- Introducing durable workflow-control state outside workspace/base git state.
- Changing acceptance `stalled` terminology or non-dependency blocker semantics.
- Implementing product-specific behavior for downstream repositories.
