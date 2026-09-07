---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/tui/state.rs
  - src/tui/render.rs
  - src/tui/state/event_handlers/processing.rs
  - src/tui/state/event_handlers/errors.rs
  - src/tui/state/event_handlers/completion.rs
  - src/tui/state/processing_logic.rs
  - src/orchestration/operator_command.rs
  - openspec/specs/cli/spec.md
verifications:
  - id: net-execution-time-tests
    requirement: Change rows accumulate only active execution intervals and retain the result while inactive or terminal
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/tui/state.rs
    evidence: cargo test net_execution_time --lib
    rerun: cargo test net_execution_time --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Retain net execution time in TUI change rows

**Change Type**: implementation

## Problem / Context

The elapsed field rendered immediately before active badges such as `[applying]` is currently derived from one continuous `started_at.elapsed()` interval or from handler-specific snapshots. Wall-clock time spent stopped in `error`, `stalled`, blocked, merge-wait, or other inactive states can therefore be included after execution resumes. Some terminal and stopped transitions also clear or stop rendering the elapsed value, so the completed work duration disappears when it is most useful for diagnosis.

The repository already defines the authoritative active display-status vocabulary through `is_active_status`. TUI timing must consume that vocabulary rather than maintain a second status list. Timing remains process-local presentation state and must not influence scheduler or workflow routing.

## Proposed Solution

Treat each change row's elapsed value as accumulated active execution time:

- `elapsed_time` stores the sum of completed active intervals.
- `started_at` stores only the start of the current active interval and is `None` while inactive.
- A transition from inactive to an `is_active_status` status starts a new interval without clearing the accumulated value.
- A transition from active to any inactive, stopped, waiting, error, stalled, or terminal status closes the current interval exactly once and adds it to the accumulated value.
- Repeated updates within the same active status must not restart or double-count the interval.
- Catalog refresh treats either an open interval (`started_at.is_some()`) or a retained accumulated value (`elapsed_time.is_some()`) as execution-history evidence, preserving the row during a temporary catalog absence without adding a new flag.
- Process-level `Stopped` does not require a status transition: `handle_stopped` calls the centralized pause operation for every row, closing each open interval exactly once at the event boundary. A later reducer transition to `not queued` is idempotent and does not close or add the interval again.
- Rendering during an active interval shows the accumulated value plus the current interval. Rendering during inactive and terminal states shows the retained accumulated value.
- `merged`, `error`, and `stalled` rows keep the elapsed field visible in the same row position instead of replacing it with `--` or omitting it.

Centralize interval lifecycle in `ChangeState` and route event-handler transitions through it. Remove handler-specific direct snapshots that overwrite rather than accumulate elapsed time.

## Acceptance Criteria

- Active phases defined by `is_active_status` contribute to elapsed time.
- Time spent in `error`, `stalled`, blocked, merge-wait, resolve-pending, not-queued, or other inactive statuses does not contribute.
- Resuming from an inactive status adds a fresh active interval to the prior accumulated duration.
- Repeated active status updates neither reset nor double-count elapsed time.
- `merged`, `error`, and `stalled` rows retain and display the accumulated elapsed duration.
- Existing status badge, spinner, iteration-label, and workflow-routing contracts remain unchanged. Inactive rows gain the retained elapsed field in the same fixed field area used by active rows.
- Timing state remains ephemeral and non-authoritative, consistent with the constitution.

## Explicit Completion Conditions

- `ChangeState` owns start, pause, resume, and current-total calculation for active timing.
- Status-cache transitions use `is_active_status` as the only active-status classification.
- `handle_stopped` invokes the same centralized idempotent pause operation without reading status, preserving the existing event-order-independent stop boundary.
- Catalog refresh preserves a temporarily absent row when either an active interval or retained elapsed duration proves execution history.
- Processing, error, and completion handlers no longer overwrite elapsed duration from a single wall-clock interval.
- Focused tests exercise two active intervals separated by inactive time, repeated active updates, idempotent process stop before/after reducer synchronization, temporary catalog absence after an inactive transition, terminal retention, and rendered retention for `merged`, `error`, and `stalled`.
- `cargo test net_execution_time --lib` passes.

## Out of Scope

- Persisting elapsed time across TUI or owner restarts.
- Changing the active-status vocabulary.
- Changing command runtime limits, scheduler decisions, status names, badges, spinner animation, or log timestamps.
- Adding historical per-phase timing or telemetry export.
