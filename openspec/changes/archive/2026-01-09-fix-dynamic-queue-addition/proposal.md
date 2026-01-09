# Fix Dynamic Queue Addition During Processing

## Summary

Enable dynamic queue addition in TUI while the orchestrator is in "Waiting..." status (between processing changes). Currently, changes can be added to the queue visually via Space key during Running mode, but the orchestrator does not pick up these dynamically added changes.

## Problem Statement

When the TUI is in Running mode and displays "Waiting..." status (no active processing), users can press Space to add changes to the queue. The UI correctly shows the change as "queued", but:

1. The orchestrator only processes the initial `change_ids` list passed at startup
2. The `TuiCommand::AddToQueue` command is received but never acted upon
3. Dynamically added changes remain in "queued" status indefinitely
4. Users expect the orchestrator to pick up newly queued changes

## Root Cause Analysis

In `src/tui.rs`:
- `toggle_selection()` (line 328-368) correctly updates `QueueStatus` and sends `TuiCommand::AddToQueue`
- `run_tui_loop()` (line 634-798) receives `AddToQueue` commands but only logs them (line 767-770)
- `run_orchestrator()` (line 803-1352) receives a fixed `change_ids` Vec and never checks for new additions

The communication channel exists (`cmd_rx`) but the orchestrator does not monitor it for new changes.

## Proposed Solution

Implement a shared queue mechanism between TUI and orchestrator:

1. Create a shared `Arc<Mutex<VecDeque<String>>>` for pending changes
2. Orchestrator polls this queue after completing each change
3. TUI pushes to this queue when user adds changes dynamically
4. Orchestrator continues processing until queue is empty and all initial changes are done

## Scope

- **In Scope**: Dynamic queue addition during "Waiting..." status
- **Out of Scope**:
  - Dynamic queue removal (change already sent to orchestrator)
  - Priority ordering of dynamically added changes
  - UI for queue reordering

## Success Criteria

1. Changes added via Space key during Running mode are processed by orchestrator
2. Log shows "Processing dynamically added: <change-id>"
3. All queued changes complete before `AllCompleted` event is sent
4. No race conditions or deadlocks in queue access

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Race condition on shared queue | High | Use `tokio::sync::Mutex` for async-safe access |
| Deadlock if orchestrator holds lock | Medium | Use short critical sections, try_lock where appropriate |
| Queue grows unbounded | Low | Limit queue size, warn when near limit |

## Alternatives Considered

1. **Restart orchestrator with new list**: Simpler but loses progress tracking
2. **Channel-based queue**: More complex, harder to inspect queue state
3. **Event-driven architecture**: Over-engineered for this use case
