---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - openspec/specs/tui-architecture/spec.md
  - openspec/specs/tui-state/spec.md
  - openspec/specs/tui-state-management/spec.md
  - openspec/specs/tui-key-hints/spec.md
  - openspec/specs/remote-control-api/spec.md
  - src/orchestration/operator_command.rs
  - src/orchestration/mark_reconciliation.rs
  - src/orchestration/run_control.rs
  - src/tui/state/selection_logic.rs
  - src/tui/render.rs
  - src/tui/key_handlers.rs
verifications:
  - id: run-mark-contract-tests
    requirement: "Execution marks remain pure next-run target intent across non-terminal lifecycle states, do not mutate queue or live execution, preserve existing reconciliation revocations, and are excluded from terminal rows with stable alignment"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Focused Rust unit and integration test output covering shared mark admission, TUI and API parity, run target selection, archive reconciliation, key hints, and fixed-width checkbox rendering"
    rerun: "cargo test --lib run_mark_intent && cargo test --lib archived_checkbox_placeholder"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Simplify TUI marks to next-run target intent

**Change Type**: implementation

## Premise / Context

- Execution marks are process-local state in `ExecutionMarkStore`; `RunControlService` reads them to determine run targets.
- The shared operator matrix currently refuses mark changes for active, stopping, error, final, apply-limit, and parallel-ineligible rows, and Running-mode mark changes may also mutate `DynamicQueue`.
- The TUI currently forces `archived` and `merged` rows to render a gray `[x]` regardless of the actual mark.
- Per-change live termination already has a separate `K: kill` control, so Space does not need to retain stop or dequeue semantics.
- The Constitution permits process-local marks but forbids treating them as durable workflow evidence.
- The requested behavior is one shared contract across TUI, `/api/v2`, run admission, and archive reconciliation; splitting it would permit frontend and execution semantics to drift.

## Problem / Context

A mark is intended to answer one question: whether the operator wants a change considered for a run. The current implementation also uses mark admission and mutation to enforce lifecycle timing, queue eligibility, retry ownership, and active-execution safety. As a result, the operator cannot freely express future run intent while a change is active, a stop is settling, the process is in Error, or worktree eligibility is temporarily unavailable. In Running mode the same Space action can mutate both the mark and `DynamicQueue`, so unmarking can alter already-admitted work instead of only changing future intent.

After archive, the opposite problem occurs: the row no longer represents a possible run target, but the TUI synthesizes a gray `[x]`. This looks like a retained mark even though the checkbox has no actionable meaning.

## Proposed Solution

Make execution marks pure process-local next-run target intent until archive completion:

- permit single-row and bulk mark/unmark operations for every visible non-terminal change regardless of Select, Running, Stopping, Stopped, or Error mode, current active/retry/wait state, apply-limit state, or current parallel eligibility, while retaining the existing rejected-row exclusion;
- keep modal input ownership unchanged: an active popup still consumes keys, but execution lifecycle timing does not make marks immutable;
- make mark mutation update only `ExecutionMarkStore` and its frontend projection; it must not add/remove `DynamicQueue` entries, stop/dequeue active work, create retry/resolve intent, change reducer status, or wake/start a scheduler;
- retain `K: kill`, start controls, explicit retry/resolve commands, and queue services as separate controls;
- have start/retry admission read a coherent mark snapshot and decide which marked changes are runnable using current reducer and worktree facts at that boundary rather than at mark time; preserve the existing all-or-nothing worktree eligibility fence, exclude other non-startable statuses with target-specific diagnostics, and reject when no runnable target remains;
- reject failed admission without partial scheduler or queue effects, and define Error-mode retry to route only marked retry-eligible error targets while reporting other marked rows as excluded;
- preserve existing typed mark-revocation edges and add `ChangeArchived` as an authoritative revocation edge so archived and later merged/pushed rows cannot re-enter a run target set;
- render a fixed-width blank placeholder instead of `[x]` or `[ ]` for archived, merged, and pushed rows, preserving the existing cursor, change ID, badge, status, progress, and preview column positions;
- make Space on a post-archive row a silent no-op and omit mark hints for that row;
- preserve the shared TUI and `/api/v2` mark contract, including bulk target-state calculation and coherent state revisions.

No durable state, new key, new queue, new dependency, or configuration option is introduced.

## Acceptance Criteria

1. Space can mark or unmark any visible non-archived change in Select, Running, Stopping, Stopped, and Error modes.
2. Bulk `x` can mark or unmark the same non-terminal population in all five modes; archived, merged, pushed, and rejected rows remain excluded, and overlays continue to consume input without leaking the action to the Changes view.
3. Active, retryable-error, waiting, apply-limit, and temporarily parallel-ineligible non-terminal rows accept operator mark intent without mark-time admission warnings; existing typed reconciliation may still revoke stale marks, after which a non-terminal row can be marked again.
4. A single or bulk mark change modifies only the execution-mark store/projection and does not mutate `DynamicQueue`, reducer queue intent, active execution, cancellation, retry, resolve, hooks, scheduler state, or process mode.
5. Unmarking a currently active or queued change does not stop, dequeue, or unschedule work already admitted by the current run.
6. `K: kill`, explicit retry/resolve behavior, graceful/force stop, and direct queue command APIs remain separate and retain their existing effects.
7. Start/retry evaluates current marks and current run eligibility at final admission: any worktree-ineligible marked target rejects the whole request, other non-startable statuses are excluded with diagnostics, zero runnable targets rejects, and failed admission leaves no partial queue or scheduler effect. Error-mode retry routes only marked retry-eligible error targets.
8. Existing typed mark-revocation edges remain authoritative, and a successful `ChangeArchived` transition additionally clears only that change's mark in the same authoritative revision while preserving unrelated marks.
9. Archived, merged, and pushed rows remain in the current TUI session as specified, but display neither `[x]` nor `[ ]`.
10. The post-archive checkbox area remains the same width, so cursor, ID, badges, status, progress, and preview content do not shift left.
11. Space on an archived, merged, pushed, or rejected row is a silent no-op and no mark hint is advertised for that row; rejected rows otherwise retain their existing read-only display contract.
12. TUI single-row mark, TUI bulk mark, `/api/v2 set_execution_mark`, and `/api/v2 set_all_execution_marks` use the same lifecycle-independent mark semantics and coherent revisions.
13. Marks remain process-local and restart-empty; repository contents remain the sole durable workflow authority.

## Explicit Completion Conditions

- `src/orchestration/operator_command.rs` classifies mark mutation independently from queue, retry, active-state, execution-mode, apply-limit, and parallel-eligibility admission while retaining a distinct post-archive exclusion.
- `src/tui/state/selection_logic.rs` no longer emits `AddToQueue` or `RemoveFromQueue` from Space or bulk `x`, and frontend mark projection follows the shared store.
- `/api/v2` command execution and action projection expose the same markability contract as the TUI without a second frontend-local lifecycle table.
- `src/orchestration/run_control.rs` performs current-state run eligibility checks after reading marks, preserves the complete-set worktree fence, reports status exclusions, routes Error-mode retries precisely, and proves failed admission leaves queue and scheduler state unchanged.
- `src/orchestration/mark_reconciliation.rs` preserves existing typed mark-revocation edges, additionally revokes the target mark on the authoritative archive edge, and publishes every effective revocation with its reducer revision.
- `src/tui/render.rs` uses an exactly checkbox-width blank placeholder for archived, merged, and pushed rows in Select and Running/Stopped layouts and suppresses their mark hints.
- Focused unit and integration tests prove lifecycle-wide markability, side-effect isolation, current-run continuity after unmark, API/TUI parity, archive clearing, silent post-archive Space, and fixed-width rendering.
- The declared `run-mark-contract-tests` verification passes.

## Scope Rationale

Mark admission, mark side effects, run target consumption, archive cleanup, and checkbox presentation are one invariant: the UI must show exactly the process-local intent that run control consumes, and must stop showing it once archive makes that intent meaningless. Shipping only one layer would create misleading display or unsafe control drift, so these changes must be implemented and verified together.

## Out of Scope

- Persisting marks across process restarts.
- Automatically rerunning a change immediately when it is marked.
- Changing the `K: kill`, explicit retry, resolve, graceful-stop, or force-stop workflows.
- Removing archived rows before TUI process exit or changing archived-list discovery after restart.
- Redesigning row spacing, compacting columns, or changing status/progress labels.
- Introducing a second mark type for current-run queue membership.

The Rust hooks in `.pre-commit-config.yaml` are path-scoped and do not run for this proposal-only commit. Requirement-specific focused tests remain explicit implementation evidence; implementation commits also remain subject to the repository's Rust hooks when Rust paths are staged.
