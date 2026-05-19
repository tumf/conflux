# Design: Prevent Merged Changes From Re-entering Archive Dispatch

## Current Failure Path

The observed failure is a stale queue/control-flow issue, not an archive command issue:

1. A change reaches reducer terminal `merged` after merge/resolve completion.
2. A stale dynamic queue entry or scheduler-local candidate still references the same change ID.
3. Dynamic queue ingestion in `src/parallel/queue_state.rs` loads the change from active OpenSpec listing and pushes it into scheduler-local `queued` without consulting terminal state.
4. Dispatch preflight in `src/parallel/dispatch.rs` rejects only terminal errors, so terminal `merged` is not blocked.
5. The normal apply/acceptance/archive pipeline runs and reaches `execute_archive_in_workspace()`, emitting `ArchiveStarted`.
6. TUI event handling may then display the stale `ArchiveStarted` as `archiving` even if the reducer/display state had already shown `merged`.

## Design Principles

- Final terminal state must be a dispatch stop gate.
- Recoverable terminal errors must remain separately retryable via explicit retry intent.
- Workspace-local and base-branch evidence remain authoritative for resume routing, in line with `openspec/CONSTITUTION.md`.
- UI display guards are defensive only; they must not become workflow-control inputs.

## Proposed Guards

### Scheduler ingestion guard

When `check_dynamic_queue_and_add_changes()` pops a dynamic queue entry, it should consult the shared reducer if available. If the change is in a final terminal state such as `merged`, `archived`, or `rejected`, it must skip scheduler-local insertion and emit at most a bounded diagnostic.

### Dispatch preflight guard

Before acquiring the semaphore or creating/reusing a workspace, `dispatch_change_to_workspace()` should reject final terminal states. This is the last reliable boundary before a stale candidate can run apply/acceptance/archive.

Terminal error remains distinct: it is recoverable only through explicit retry, and existing terminal-error skip logging should remain valid.

### TUI stale event guard

`handle_archive_started()` should avoid overwriting a row already displayed as `merged`. Reducer-derived display remains the source of truth, and stale archive-start events must not make the interface claim that a merged change is archiving.

## Verification Strategy

- Reducer unit tests cover final terminal state classification and stale archive event behavior after merged.
- Parallel scheduler tests cover dynamic queue ingestion and dispatch preflight.
- TUI unit tests cover stale `ArchiveStarted` display handling.
- Existing workspace state detection tests continue to prove `WorkspaceState::Merged` is based on base-branch file state and remains terminal.
