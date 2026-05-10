---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - src/permission.rs
  - src/execution/apply.rs
  - src/orchestration/acceptance.rs
  - src/parallel/dispatch.rs
  - src/events.rs
  - src/orchestration/state.rs
---

# Fix Permission Denial Stalled Handling

**Change Type**: implementation

## Problem / Context

Conflux already specifies that apply-time permission auto-reject should stop retrying and record the change as `stalled`, but the current apply implementation still treats the condition as a soft error and continues to the next apply iteration. Acceptance also lacks an equivalent permission/policy-denial classification path, so a harness-level Read/tool permission denial can be treated like a normal acceptance failure, returned to the apply loop, or surfaced as terminal `error`.

These failures are not usually self-healable by editing the change implementation inside the workspace. Re-entering apply/acceptance consumes cycles and can degrade into `Max apply+acceptance cycles reached`, which misrepresents an operator-action environment blocker as a broken change.

The constitution requires next-action workflow routing to remain derivable from workspace file state, workspace git state, and base-branch tree comparison. This change must therefore avoid introducing out-of-worktree durable workflow-control state; logs/UI metadata may remain observational only.

## Proposed Solution

Add a shared execution-blocker classification path for permission/policy denial and route only repeated unresolved denials to a non-terminal `stalled` hold.

- Expand permission denial detection beyond the current `permission requested` + `auto-reject` output pattern to cover harness/tool read denial and command-level policy rejection text.
- Track a denial signature, such as denied target plus denial category, across the current apply/acceptance retry context so the runtime can distinguish first/transient denials from repeated unresolved ones without introducing authoritative out-of-worktree durable workflow state.
- Keep apply retryable when a permission/policy denial is first observed or when repository-visible progress occurs after the denial.
- Make apply stop when the same unresolved permission/policy denial recurs without repository-visible progress, preserving workspace context and skipping further apply iterations, empty-WIP stall escalation, and apply+acceptance cycle-limit degradation.
- Make acceptance classify permission/policy denial in command failures and FAIL findings before normal acceptance retry handling, but stall only when the same unresolved denial recurs without repository-visible progress or changed acceptance evidence.
- Route repeated unresolved blockers through reducer-visible stalled state and operator guidance without converting them to terminal `error` or dependency `blocked`.
- Keep normal implementation acceptance failures retryable through the existing follow-up/apply loop behavior.

## Acceptance Criteria

- First-observed permission/policy denials remain retryable or reportable according to the existing result path unless they are already known repeated unresolved blockers.
- Apply output containing `permission requested` and `auto-rejecting` records the change as `stalled` only when the same denial signature recurs without repository-visible progress.
- Apply command failures whose output/error text indicates harness-level permission denial are recorded as `stalled`, not terminal `error`, only after repeated unresolved observation without progress.
- Acceptance command failures whose output/error text indicates permission/tool/policy denial are recorded as non-terminal `stalled`, not acceptance command terminal errors, only after repeated unresolved observation without progress or changed evidence.
- Acceptance FAIL findings that indicate repeated unresolved permission/tool/policy denial are recorded as non-terminal `stalled` without appending ordinary implementation follow-up tasks or returning to apply.
- Normal acceptance FAIL findings that describe implementation issues continue to record follow-up tasks and return to apply.
- Repeated unresolved permission/policy-denial stalls do not continue toward `Max apply+acceptance cycles reached` after classification.
- TUI/Web-visible status is `stalled`, not dependency `blocked`, and the visible reason/guidance identifies operator action for permission/tool policy remediation.
- Resume remains possible after the operator fixes the local permission/environment policy, using existing workspace-derived routing.

## Explicit Completion Conditions

- A shared classifier in `src/permission.rs` or a clearly named adjacent module identifies permission/policy denial from stdout/stderr tails, command error strings, and acceptance findings.
- Apply execution in `src/execution/apply.rs` and its parallel wrapper in `src/parallel/executor.rs` return or emit a structured stalled blocker when the classifier matches a repeated unresolved denial signature without repository-visible progress, without relying on repeated empty-WIP stall detection.
- Acceptance handling in `src/orchestration/acceptance.rs` and/or `src/parallel/dispatch.rs` checks command failures and FAIL findings with the classifier before normal retry/error handling, and stalls only after repeated unresolved denial evidence.
- The reducer path in `src/events.rs` and `src/orchestration/state.rs` records repeated unresolved permission/policy blockers as `WaitState::Stalled` with non-terminal state and metadata suitable for operator guidance.
- Regression tests cover first-denial retry, repeated apply auto-reject, repeated apply command denial, repeated acceptance command denial, repeated acceptance FAIL-denial, progress-reset behavior, and normal acceptance FAIL retry behavior.
- `cflx openspec validate fix-permission-denial-stalled --strict --evidence warn` and relevant Rust tests pass.

## Out of Scope

- Changing dependency-blocked scheduling semantics.
- Treating all command failures as stalled; only classified permission/tool/policy denial is affected.
- Adding new durable workflow-control files outside the workspace or using external logs/UI state for resume routing.
- Automatically modifying the user's local tool permission policy.
- Changing rejection review semantics for `REJECTED.md` handoffs.
