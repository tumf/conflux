---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - openspec/changes/archive/2026-01-09-fix-footer-progress-tracking/specs/cli/spec.md
  - openspec/changes/archive/2026-01-19-update-tui-status-display/specs/cli/spec.md
  - src/tui/render.rs
  - src/tui/state.rs
  - src/orchestration/operator_command.rs
verifications:
  - id: tui-overall-progress-tests
    requirement: "The TUI Status progress bar counts each completed, active, or execution-marked change exactly once and retains completed work after its execution mark is revoked"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Focused Rust test output covering mixed completed, active, marked, excluded, overlapping, and zero-task rows plus the rendered aggregate percentage and task counts"
    rerun: "cargo test --lib tui_status_overall_progress -- --list | grep -q ': test$' && cargo test --lib tui_status_overall_progress"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix the TUI Status overall progress scope

**Change Type**: implementation

## Premise / Context

- The requested denominator is the union of completed changes, currently executing changes, and not-yet-executed changes carrying an execution mark.
- `src/tui/render.rs::render_status` currently filters only on `ChangeState::selected`, so the Status progress bar is a view of current execution marks rather than overall run progress.
- Archive completion revokes the process-local execution mark by design, while later `archived`, `merged`, and `pushed` rows remain visible with their last known task counts.
- The canonical `Running Footer Progress Bar Display` requirement already requires progress retention through Completed, Archived, and Merged transitions, but does not fully define the denominator when completed, active, and marked rows coexist.
- The shared active-status vocabulary already exists in `src/orchestration/operator_command.rs`; the TUI must reuse it rather than maintain another phase list.
- The constitution permits process-local presentation state but forbids it from becoming workflow-control authority. This change only reads existing presentation facts for rendering.

## Requested Artifact

- Both a canonical CLI spec clarification and the matching TUI implementation with repository-local regression coverage.

## Problem / Context

The Status progress calculation currently includes only rows whose execution mark is set. That was once described as “selected change progress,” but execution marks are next-run intent, not a durable record of the current run's completed scope. When a selected change reaches archive completion, mark reconciliation correctly clears its mark. The Status calculation then removes that change's `completed_tasks` and `total_tasks` together, so a successfully archived or merged change disappears from the aggregate and the operator loses the overall picture.

The same filter also omits an active row if its next-run mark is absent. As a result, neither completed work nor all current work is guaranteed to be represented. The progress bar must instead derive one unique set from lifecycle completion, active execution, and explicit future execution intent.

## Proposed Solution

Change the TUI Status aggregation so a change contributes its last known `completed_tasks` and `total_tasks` exactly once when any of these conditions holds:

1. **Completed**: reducer archive completion has been observed, or the display status is `archived`, `merged`, or `pushed`.
2. **In progress**: the display status is classified by the shared `is_active_status` vocabulary.
3. **Marked for execution**: the process-local execution mark is set, including an idle, queued, waiting, or retryable error row.

Use a single inclusion predicate over each row so overlapping conditions cannot double-count a change. Preserve the existing task-count and percentage display format. Sum the stored task counts as observed; do not synthesize 100% solely from lifecycle status.

Rows that satisfy none of the three conditions remain outside the aggregate. A rejected row remains excluded even if stale presentation state still claims a mark, because rejection is a final non-success outcome and not an execution target.

## Acceptance Criteria

1. A `merged`, `archived`, or `pushed` change remains in the Status numerator and denominator after its execution mark is cleared.
2. A reducer-observed archive-complete change remains included while its display status advances through post-archive `resolving`, `resolve pending`, or `merge wait`.
3. Every active status recognized by shared `is_active_status` is included even when its execution mark is absent.
4. A non-completed, non-active change is included when its execution mark is set, including a marked retryable `error` row.
5. An unmarked, inactive, unfinished row and a rejected row are excluded.
6. A change satisfying two or all three inclusion conditions contributes its task counts exactly once.
7. Given `merged` unmarked `3/3`, `applying` unmarked `1/4`, marked `not queued` `0/2`, and unmarked `not queued` `0/5`, the Status panel displays `4/9` and `44.4%`.
8. A completion transition alone does not reduce progress by dropping the completed row. Explicit operator changes to the marked target set may legitimately change the aggregate denominator and percentage.
9. Rows with zero total tasks do not cause division by zero or duplicate output; existing no-task rendering behavior remains unchanged.

## Explicit Completion Conditions

- `src/tui/render.rs::render_status` no longer uses `selected` as its sole inclusion criterion.
- The implementation reuses shared active-status classification and the existing post-archive/archive-complete facts; it adds no durable workflow state and does not alter execution-mark reconciliation.
- Focused tests named with the `tui_status_overall_progress` prefix prove mixed-set aggregation, overlap deduplication, completion retention after mark revocation, archive-complete post-archive inclusion, marked error inclusion, rejected/unmarked-idle exclusion, and zero-task safety through rendered Status output.
- The declared repository-local verification command discovers at least one focused test and exits successfully.

## Scope Rationale

This is one tightly coupled rendering correction: the denominator rule and its regression coverage cannot provide useful independent behavior if split. No API, scheduler, or mark-lifecycle change is needed.

## Out of Scope

- Changing when execution marks are set or revoked.
- Persisting a historical run manifest or initial queue snapshot.
- Treating lifecycle completion as synthetic `total_tasks/total_tasks` when stored task counts disagree.
- Changing per-change task parsing, refresh timing, row progress text, elapsed-time rendering, or Status bar styling.
- Changing WebUI snapshot totals or its change-count summary.
- Changing scheduler admission, retry routing, archive, merge, push, or rejection behavior.

## Verification Ownership

Requirement-specific rendering tests remain explicit. The tracked Rust pre-commit hooks are path-scoped and will run rustfmt and clippy when implementation touches `src/tui/render.rs`, but they do not replace the focused behavior verification and do not run for this proposal-only commit.
