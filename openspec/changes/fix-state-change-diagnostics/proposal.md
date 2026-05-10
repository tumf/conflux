---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/observability/spec.md
  - openspec/specs/tui-state/spec.md
  - src/parallel/queue_state.rs
  - src/tui/state/event_handlers/refresh.rs
---

# Fix state-change driven scheduler diagnostics

**Change Type**: implementation

## Problem / Context

Scheduler and refresh loops can repeatedly observe the same dependency, worktree, or merge-wait state and emit the same diagnostic/event/log on every loop iteration. This creates large debug logs under `~/.local/state/cflx/logs/<project_slug>/` and noisy TUI logs, but the root issue is not logging volume alone.

The root issue is that unchanged observations are being treated as new workflow events.

The constitution requires workflow-control decisions to remain derivable from workspace and git state. Any suppression/cache state introduced by this change must therefore be non-authoritative: it may suppress repeated diagnostics and repeated UI/log emissions, but it must not decide scheduling, resume routing, acceptance, archive routing, or next actions.

## Proposed Solution

Make scheduler diagnostics and TUI-visible blocked/resolved messages state-transition driven.

The implementation should track comparable in-memory observation fingerprints for repeated diagnostics such as dependency blockers and worktree status observations. When a loop observes the same fingerprint again, Conflux should treat it as a no-op for diagnostic/event/log emission. When the fingerprint changes, Conflux should emit a new diagnostic describing the transition.

For dependency blockers, the fingerprint should include at least the change id, unresolved dependency ids, dependency target classes, and enough blocker context to distinguish queued/in-flight/missing/rejected/archived transitions.

For TUI event handling, repeated `DependencyBlocked` or `DependencyResolved` events should not produce repeated user-visible logs when the displayed state has not changed. This is a defensive layer, not the primary scheduler fix.

## Acceptance Criteria

- Repeated scheduler loops with the same dependency blocker snapshot do not emit repeated `DependencyBlocked` diagnostics/events/logs.
- A changed dependency blocker snapshot emits a new diagnostic/event/log so users can see that the blocker changed.
- Dependency resolution emits once for a blocked change and is not re-emitted on later loops unless the change becomes blocked again first.
- TUI dependency blocked/resolved handlers do not append duplicate user-visible log entries when no display state transition occurs.
- Worktree or merge-wait diagnostics that are repeatedly derived from the same observation are bounded by state-change detection, rate limiting, or summary behavior without becoming workflow-control inputs.
- Any cache/fingerprint state used for suppression is in-memory/non-authoritative and deleting `~/.local/state/cflx/**` does not alter workflow next-action decisions.

## Explicit Completion Conditions

- `src/parallel/queue_state.rs` or equivalent scheduler code keeps enough prior observation state to suppress repeated dependency blocker diagnostic/event emissions for unchanged blocker fingerprints.
- Reducer/TUI handling remains correct if duplicate events are received, including no duplicate TUI log lines for unchanged dependency blocked/resolved display state.
- Tests prove unchanged blocker observations do not emit repeated events and changed blocker observations still emit.
- Tests prove dependency resolution emits once after a blocked state and does not repeat without a new blocked transition.
- Tests or manual verification prove debug/TUI spam is bounded for a repeated unchanged scheduler diagnostic.
- Final OpenSpec validation passes in strict mode and archive-gate mode.

## Out of Scope

- Changing log file locations or retention policy.
- Making `~/.local/state/cflx/**` authoritative for workflow control.
- Removing periodic polling entirely.
- Broad scheduler rewrites unrelated to repeated unchanged diagnostics.
