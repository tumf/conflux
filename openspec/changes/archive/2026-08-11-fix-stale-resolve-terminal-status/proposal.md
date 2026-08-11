---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/queue_state.rs
  - src/orchestration/state.rs
  - src/tui/state/event_handlers/errors.rs
  - openspec/specs/tui-key-hints/spec.md
verifications:
  - id: stale-resolve-state-tests
    requirement: Stale merge-wait retries converge to a truthful terminal or retryable reducer state without modifying dirty repository content
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test output for the targeted reducer and parallel scheduler regression tests
    rerun: cargo test stale_deferred_merge_retry
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix stale resolve terminal status

**Change Type**: implementation

## Problem / Context

A manual `M` retry from `merge wait` records scheduler-owned `ResolveWait` intent. When repository evidence later shows the change is already integrated into the base tree, the stale retry path currently clears only the resolve intent. Because the reducer still has `QueueIntent::NotQueued`, clearing the wait exposes `not queued` instead of recording the repository-proven terminal result.

The observed failure occurred for `add-metadata-completion-check`: a dirty target caused bounded resolve exhaustion and returned the row to `merge wait`; a later `M` retry found the archive entry already present in base, classified the retry as stale, cleared `ResolveWait`, and displayed `not queued`.

## Proposed Solution

Make stale deferred-merge settlement explicit and evidence-driven:

1. When base-tree evidence proves the change is already integrated, settle the reducer change as `merged` before clearing scheduler-local retry ownership.
2. When integration evidence is absent or cannot be read safely, preserve a retryable manual `merge wait` state and do not report success.
3. Preserve dirty repository and worktree content. The stale-retry path must not stage, commit, stash, reset, or discard unrelated content.
4. Keep ordinary bounded `ResolveFailed` behavior unchanged: recoverable failure returns the change to `merge wait`.

This is one atomic scope because scheduler stale-retry classification and reducer settlement must agree in the same transition; changing either side alone leaves an externally visible false status.

## Acceptance Criteria

- A stale deferred merge retry with repository-verifiable base integration finishes with reducer and TUI/Web status `merged`, never `not queued`.
- A stale retry without proven base integration remains `merge wait` and can be retried explicitly.
- Failure or uncertainty while reading integration evidence does not produce `merged`, `not queued`, or another success classification.
- Dirty index, tracked changes, and non-ignored untracked content are preserved byte-for-byte and are not staged, committed, stashed, reset, or discarded by this settlement path.
- Existing bounded resolve exhaustion continues to emit one change-scoped `ResolveFailed` and return the change to `merge wait`.
- Scheduler-local resolve reservations and base-mutating lane ownership are released after the reducer reaches the matching terminal or retryable state.

## Explicit Completion Conditions

- The already-integrated branch in `src/parallel/queue_state.rs` applies a typed reducer success transition rather than only clearing `ResolveWait`.
- Reducer APIs in `src/orchestration/state.rs` cannot turn an already-integrated stale retry into idle `QueueIntent::NotQueued` without a terminal state.
- Targeted unit/integration tests exercise integrated, not-integrated, evidence-error, and dirty-content-preservation cases through real scheduler/reducer state transitions.
- `cargo test stale_deferred_merge_retry` passes and includes assertions that reject `not queued` and repository mutation.
- `cflx openspec validate fix-stale-resolve-terminal-status --archive-gate` passes at archive time.

## Out of Scope

- Automatically committing or discarding dirty user or agent work.
- Changing the general meaning of execution marks or ordinary queue intent.
- Reworking merge conflict resolution strategy, retry limits, or agent prompts.
- Treating an archive directory in a worktree alone as proof of base integration; authoritative evidence remains base-branch tree comparison under the Constitution.

## Verification Plan

Requirement-specific regression coverage is change-blocking through `stale-resolve-state-tests`. Repository-wide formatting and clippy remain owned by the existing path-scoped hooks and normal `make check`/CI; this proposal does not duplicate them as implementation tasks.
