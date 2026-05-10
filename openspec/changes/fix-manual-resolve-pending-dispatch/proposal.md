---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - src/tui/state.rs
  - src/tui/command_handlers.rs
  - src/tui/runner.rs
  - src/orchestration/state.rs
  - src/parallel/queue_state.rs
  - src/parallel/orchestration.rs
  - openspec/specs/tui-resolve/spec.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/parallel-execution/spec.md
---

# Fix manual resolve pending dispatch

**Change Type**: implementation

## Problem/Context

Recent investigation of `~/wakumo/avacus/avacuscc-dbot` showed a repeatable manual resolve stall:

- The operator presses `M` on a row visible as `merge wait`
- The row transitions to `resolve pending`
- Later, merge capacity becomes available and the repository root is clean
- The scheduler loop still does not dispatch the retry, so the change remains pending indefinitely

Repository evidence shows the issue is not that `M` fails to trigger the pending transition. The issue is that the internal loop does not always consume the pending retry as scheduler-owned work:

- `src/tui/state.rs` and `src/tui/command_handlers.rs` can move the visible row to `resolve pending` and notify an existing scheduler
- `src/parallel/orchestration.rs` and `src/parallel/queue_state.rs` only dispatch retry work from reducer-owned `ResolveWait` membership and related internal counters
- `src/tui/runner.rs` can restore `merge wait` from archive-complete worktree evidence while `src/orchestration/state.rs` still treats the same change as terminal `Archived`
- In that mismatch, the manual `ResolveMerge` command can become a reducer `NoOp`, leaving the scheduler with no consumable retry intent even though the user-visible row entered `resolve pending`

This violates the intended reducer-owned loop model and creates a false pending state: the UI claims retry intent exists, but the scheduler loop cannot observe or consume it.

This proposal must obey `openspec/CONSTITUTION.md`: retry routing and next-action decisions remain derived from repository/workspace file state, git state, base-branch comparison, and reducer state derived from those inputs, not hidden durable state.

## Proposed Solution

Make manual resolve pending a truthful scheduler-owned state across TUI, reducer, and scheduler loop:

- Ensure pressing `M` on a real manual merge-wait change records reducer-owned retry intent that the scheduler loop can observe and dispatch
- Ensure archived-but-not-merged manual merge-wait changes are represented in the reducer in a way that keeps `ResolveMerge` retry eligible when repository-visible evidence says merge retry is valid
- Ensure command handling only reports "scheduled" when reducer-owned retry intent was actually accepted
- Ensure the scheduler loop reevaluates reducer-owned manual resolve pending after queue notifications and slot-release events, and either dispatches retry work or returns the row to a truthful blocker state
- Add regression coverage for the exact stuck case: `M` -> `resolve pending` -> loop conditions become favorable -> retry dispatch starts or the row returns to `merge wait` / other truthful status

## Acceptance Criteria

- Pressing `M` on a repository-visible manual merge-wait change MUST create scheduler-consumable reducer-owned retry intent, not only a display transition.
- When queue notification, slot release, or other retry-triggering loop conditions occur after manual resolve intent is registered, the scheduler MUST reevaluate and dispatch that retry if repository-visible eligibility still holds.
- `resolve pending` MUST correspond to reducer/scheduler-visible pending retry intent; the system MUST NOT leave a row indefinitely in `resolve pending` when no consumable retry work exists.
- If a manual resolve retry cannot be accepted or dispatched because the change is no longer retry-eligible, the row MUST return to a truthful blocker/terminal state with visible evidence rather than staying pending.
- Archived-but-not-merged changes that are eligible for manual merge retry MUST remain retryable through the reducer-owned loop even when the preserved workspace is archive-complete and no longer listed as an active `openspec/changes/<id>` directory.
- The implementation MUST preserve workspace-local workflow-state rules from `openspec/CONSTITUTION.md` and MUST NOT add hidden durable routing inputs.

## Explicit Completion Conditions

- `src/orchestration/state.rs` accepts reducer-owned manual retry intent for repository-visible archived merge-wait cases that are still retry-eligible, and tests prove the reducer does not drop that intent as `NoOp` in the observed stuck case.
- `src/tui/command_handlers.rs` inspects reducer command outcomes so it does not log or display a scheduled pending retry unless reducer-owned retry intent actually exists.
- `src/parallel/orchestration.rs` / `src/parallel/queue_state.rs` contain verified retry-dispatch behavior showing that reducer-owned manual resolve pending is reevaluated after queue notification and/or slot release and becomes actual retry work.
- Regression tests fail against a stub/no-op loop that leaves manual pending stranded and pass only when the retry dispatch path starts or the state falls back to a truthful blocker state.
- Log/event evidence for the retry lifecycle is explicit enough that a reviewer can distinguish: pending accepted, retry dispatched, retry blocked, retry completed, retry returned to merge wait.

## Out of Scope

- Changing dependency-analysis cwd behavior or archived dependency classification
- Reworking the broader LLM analyzer pipeline
- Redesigning the full parallel scheduler architecture beyond the manual resolve pending handshake
- Introducing hidden state outside repository/workspace-visible evidence and reducer state derived from it
