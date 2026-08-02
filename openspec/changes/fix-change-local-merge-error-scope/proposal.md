---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/tui-error-handling/spec.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/changes/separate-tui-execution-modal-state/proposal.md
  - src/parallel/conflict.rs
  - src/parallel/merge.rs
  - src/parallel/queue_state.rs
  - src/parallel/types.rs
  - src/orchestration/state.rs
  - src/tui/state/event_handlers/errors.rs
  - src/tui/state/event_handlers/output.rs
verifications:
  - id: change-local-merge-error-tests
    requirement: "Post-archive merge or resolve exhaustion remains change-scoped, preserves retryable MergeWait state, and does not place the TUI execution lifecycle in global Error"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Rust test output covering producer event sequences, scheduler continuation, reducer state, TUI mode preservation, and absence of duplicate global Error events"
    rerun: "cargo test --lib parallel::tests:: && cargo test --lib tui:: && cargo fmt --check && cargo clippy -- -D warnings"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Keep change-local merge failures out of global TUI Error

**Change Type**: implementation

## Premise / Context

- A real TUI run observed `unify-remote-operator-commands` exhaust three conflict-resolution attempts and correctly receive `ResolveFailed`, which returned that change to `merge wait`.
- The same post-archive merge failure was then emitted as global `ParallelEvent::Error` by both the merge layer and its queue-result wrapper.
- TUI global error handling changed the whole application to `AppMode::Error`; the retained terminal-mode rule kept that stale mode for hours and rejected bulk mark even though the scheduler remained alive.
- Canonical `tui-error-handling` already reserves global Error for failures that stop or invalidate the active run.
- `separate-tui-execution-modal-state` intentionally excludes producer reclassification, so this fix is an independent change with no hard dependency.

## Problem / Context

Post-archive merge and resolve failures carry a change ID and already have a typed change-scoped outcome: `ResolveFailed` restores `MergeWait` for explicit retry. Despite that, generic error wrappers also publish process-scoped `ExecutionEvent::Error`. The frontend cannot recover the lost scope because the global event has no change ID and is contractually fatal.

This produces two incorrect outcomes: one retryable change failure changes the entire TUI lifecycle to Error, and duplicate diagnostics describe the same underlying failure at resolve, merge, and queue-wrapper levels. Suppressing the event only in the TUI would hide a wrongly classified event from one frontend while leaving lifecycle and Web projections inconsistent, so classification must be corrected at the producer boundary.

## Proposed Solution

Keep exhausted post-archive merge and resolve attempts change-scoped. The operation SHALL emit the existing `ResolveFailed { change_id, error }` outcome exactly once as the authoritative failure transition, preserve repository/worktree evidence, return the change to reducer-owned `MergeWait`, and keep the scheduler available for unrelated work and explicit retry.

Remove generic global `ParallelEvent::Error` emissions that merely wrap this already-classified change-local result, including the duplicate wrapper in background merge result handling. Preserve structured change-level error logs and warning-popup diagnostics. Continue using global `ExecutionEvent::Error` for genuinely run-scoped failures where no safe orchestration continuation exists.

Keep this classification typed and origin-aware; implementation MUST NOT infer scope from diagnostic substrings. If the existing `Result<MergeTaskOutcome, String>` cannot distinguish a change-local terminal attempt from a run-fatal failure without message inspection, minimally extend the result type or event contract to carry that distinction.

## Acceptance Criteria

1. Exhausting conflict resolution for one post-archive change emits one authoritative change-scoped failure transition with that change ID and no global `ExecutionEvent::Error` for the same failure.
2. The failed change remains `merge wait`, preserves its worktree and repository evidence, and can be retried through the existing explicit resolve path.
3. A TUI receiving the event sequence keeps its execution lifecycle `Running` while other work is active, or uses the existing active-work transition to `Select`; it does not enter global `Error`.
4. The parallel scheduler remains alive and can process unrelated queued changes after the change-local merge failure.
5. Operator diagnostics retain the failure reason and structured change ID without duplicate global error entries from merge and queue wrapper layers.
6. Genuine run-scoped failures continue to emit global `ExecutionEvent::Error` and continue to place the TUI in global `Error`.
7. Scope classification is determined by typed outcomes or event variants, never by matching rendered error text.

## Explicit Completion Conditions

- The post-archive merge/resolve producer path in `src/parallel/conflict.rs` and `src/parallel/merge.rs` has one typed owner for exhausted change-local failures.
- `src/parallel/queue_state.rs` does not promote an already-classified change-local merge result to generic `ParallelEvent::Error`.
- Reducer and TUI handlers preserve `MergeWait` and non-error execution mode for the resulting event sequence.
- Tests fail if either merge layer or queue wrapper reintroduces a generic global Error for this case, if the change leaves `MergeWait`, or if unrelated scheduler work stops.
- Tests separately prove a genuinely global failure still reaches TUI Error.
- `cargo test --lib parallel::tests::`, `cargo test --lib tui::`, `cargo fmt --check`, and `cargo clippy -- -D warnings` pass.

## Scope Rationale

Producer classification, reducer transition, scheduler continuation, and TUI projection must ship together. Changing only the frontend would conceal a globally misclassified event; changing only the producer without end-to-end projection coverage could regress operator state. The scope is therefore one atomic implementation change.

## Out of Scope

- Separating TUI execution and modal state; that remains owned by `separate-tui-execution-modal-state`.
- Reclassifying unrelated dependency metadata, upstream publication, startup preparation, or repository-wide fatal errors.
- Changing conflict-resolution retry count, merge algorithm, key bindings, bulk-mark policy, or explicit retry ownership.
- Automatically retrying manual `MergeWait` failures.
- Persisting workflow decisions outside the workspace.
