# Design: Fix TUI Archive Skip

## Current Architecture

### TUI Orchestrator Flow (Buggy)

```
run_orchestrator(change_ids: Vec<String>)
│
├─ for change_id in change_ids:        // Fixed order iteration
│   ├─ fetch current change state
│   ├─ if change.is_complete():
│   │   └─ archive immediately ✓
│   ├─ else:
│   │   ├─ run apply command
│   │   ├─ retry check is_complete() up to 3 times
│   │   ├─ if complete within retries:
│   │   │   └─ archive ✓
│   │   └─ else:
│   │       └─ log warning, CONTINUE  // BUG: no archive!
│   └─ continue to next change_id
│
└─ final verification (logs warnings if unarchived)
```

**Problem**: The `for` loop iterates over `change_ids` in fixed order. If a change becomes complete but isn't detected within the retry window, it's skipped and never archived.

### CLI Orchestrator Flow (Working)

```
orchestrator.run()
│
├─ while has_changes:
│   ├─ fetch current snapshot
│   ├─ select_next_change():
│   │   ├─ Priority 1: Find complete change → archive first
│   │   ├─ Priority 2: LLM dependency analysis
│   │   └─ Priority 3: Highest progress fallback
│   ├─ if selected.is_complete():
│   │   └─ archive
│   └─ else:
│       └─ apply
│
└─ all changes processed
```

**Key difference**: CLI always checks for complete changes first, every iteration.

## Proposed Architecture

### New TUI Orchestrator Flow

```
run_orchestrator(change_ids: Vec<String>)
│
├─ let pending_changes = HashSet::from(change_ids)
├─ let archived_changes = HashSet::new()
│
├─ while !pending_changes.is_empty():
│   │
│   ├─ // Phase 1: Archive all complete changes
│   ├─ archive_all_complete(&pending_changes)
│   │   ├─ fetch current state for all pending
│   │   ├─ for each complete change:
│   │   │   ├─ archive_single_change()
│   │   │   ├─ archived_changes.insert()
│   │   │   └─ pending_changes.remove()
│   │   └─ return count_archived
│   │
│   ├─ if pending_changes.is_empty():
│   │   └─ break (all done)
│   │
│   ├─ // Phase 2: Apply next incomplete change
│   ├─ select next change (highest progress or first)
│   ├─ run apply command
│   ├─ on success: continue loop (will archive in Phase 1)
│   └─ on error: handle error, continue or break
│
└─ final verification (should show all archived)
```

## Key Design Decisions

### 1. Two-Phase Loop

Each iteration has two phases:
- **Phase 1 (Archive)**: Archive all complete changes before doing any apply
- **Phase 2 (Apply)**: Apply one incomplete change

This ensures complete changes are never skipped.

### 2. Set-Based Tracking

Use `HashSet` instead of `Vec` iteration:
- `pending_changes`: Changes still needing processing
- `archived_changes`: Successfully archived changes

This allows:
- O(1) membership checks
- Easy removal of archived changes
- No index management issues

### 3. Remove Retry-Based Completion Check

Current code retries `is_complete()` 3 times with 500ms delays after apply.

New approach:
- After apply success, immediately return to loop start
- Phase 1 will naturally pick up any complete changes
- Simpler code, no arbitrary retry counts

### 4. Shared Helper Functions

Extract reusable functions:

```rust
async fn archive_single_change(
    change_id: &str,
    agent: &AgentRunner,
    hooks: &HookRunner,
    tx: &Sender<OrchestratorEvent>,
    context: &ArchiveContext,
) -> Result<()>

async fn archive_all_complete(
    pending_ids: &HashSet<String>,
    openspec_cmd: &str,
    agent: &AgentRunner,
    hooks: &HookRunner,
    tx: &Sender<OrchestratorEvent>,
    archived_set: &mut HashSet<String>,
) -> Result<usize>
```

## State Machine

```
                    ┌─────────────┐
                    │   Start     │
                    └──────┬──────┘
                           │
                           ▼
          ┌────────────────────────────────┐
          │     Fetch & Archive Complete   │◄─────┐
          └────────────────┬───────────────┘      │
                           │                       │
              ┌────────────┴────────────┐         │
              │                          │         │
              ▼                          ▼         │
       ┌──────────┐              ┌──────────┐     │
       │ All Done │              │  Select  │     │
       │  (exit)  │              │   Next   │     │
       └──────────┘              └────┬─────┘     │
                                      │           │
                                      ▼           │
                               ┌──────────┐       │
                               │  Apply   │       │
                               └────┬─────┘       │
                                    │             │
                          ┌─────────┴─────────┐   │
                          │                   │   │
                          ▼                   ▼   │
                    ┌──────────┐       ┌──────────┤
                    │  Error   │       │ Success  │
                    │ (handle) │       │  (loop)  │
                    └──────────┘       └──────────┘
```

## Edge Cases

### 1. External Completion
A change completes due to external action (manual task completion).
- **Handled**: Phase 1 fetches fresh state each iteration

### 2. Multiple Simultaneous Completions
Two changes complete at the same time.
- **Handled**: `archive_all_complete` processes all complete changes

### 3. Apply Fails
Apply command returns non-zero exit code.
- **Handled**: Log error, continue to next change (or break on critical error)

### 4. Archive Fails
Archive command fails.
- **Handled**: Log error, mark as failed, don't remove from pending

### 5. Cancellation During Archive
User cancels during archive operation.
- **Handled**: Existing cancellation token logic preserved

## Migration Notes

- No config changes required
- No API changes
- Internal refactor only
- Backwards compatible behavior (same end result, more reliable)
