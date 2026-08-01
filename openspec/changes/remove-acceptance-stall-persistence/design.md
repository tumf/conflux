## Architectural Decision: Remove acceptance stall disk persistence

### Current State

```
acceptance agent → Stalled verdict
  → AcceptanceStallRecord::new()
  → AcceptanceStallStore::save()   ← JSON file on disk
  → ~/.local/state/cflx/acceptance-stalls/<repo>/<change>.json
  → ParallelEvent::AcceptanceGated → reducer: WaitState::Stalled

Restart:
  → preflight_acceptance_stall() / reconcile_acceptance_stall()
  → AcceptanceStallStore::load()   ← reads JSON file
  → valid → WaitState::Stalled restored → display_status "stalled"
```

### Target State

```
acceptance agent → Stalled verdict
  → ParallelEvent::AcceptanceGated → reducer: WaitState::Stalled
  → in-memory only. No file I/O.

Restart:
  → worktree has complete unarchived apply revision
  → acceptance runs again (as spec requires)
  → no stall record to reload
```

### Rationale

1. **Constitution compliance**: Law 1 states "workflow state MUST be derivable from the workspace alone". Law 1a was a narrow exception that proved unnecessary — the worktree's own Git state (unarchived apply revision) is sufficient to determine that acceptance should re-run.

2. **Spec alignment**: `runtime-state` already says "deleting `~/.local/state/cflx/**` MUST NOT change the next action". `parallel-execution` already says "After process restart, an applied but unarchived workspace MUST run acceptance again". The persistence contradicts both.

3. **Operational simplicity**: A restart should clear stalled state and attempt acceptance again. If the blocker is still present, the acceptance agent will re-issue the stalled verdict, and the operator sees it as a fresh stall.

### What stays

- `WaitState::Stalled` in-memory state
- `transition_to_stalled()` reducer method
- `ParallelEvent::AcceptanceGated` event
- `display_status()` → `"stalled"` presentation
- Explicit operator retry (`F5` / `retry` command) consuming the in-memory hold

### What is removed

- `AcceptanceStallStore::save()` calls in production code
- `AcceptanceStallStore::load()` calls on restart
- `persist_acceptance_stall()` and `record_acceptance_stall()` disk I/O
- `preflight_acceptance_stall()` and `reconcile_acceptance_stall()` disk reads
- Startup cleanup of stale `~/.local/state/cflx/acceptance-stalls/` entries

### Risk

- If acceptance repeatedly stalls on the same external blocker, the change will cycle between apply→acceptance→stalled→restart→apply→acceptance→stalled. This is acceptable because:
  - Each restart gives the operator a chance to satisfy the prerequisite
  - The stalled state persists only for the lifetime of one Conflux process
  - If the operator wants to keep the stall alive, they should not restart
