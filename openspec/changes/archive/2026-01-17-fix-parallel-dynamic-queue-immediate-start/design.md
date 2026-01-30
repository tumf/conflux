## Context

The current parallel execution architecture processes changes in batches:

1. TUI's `run_orchestrator_parallel` has a main loop that:
   - Checks `dynamic_queue.pop()` for new items
   - Calls `batch_service.run_parallel_with_channel_and_queue_state()` for the current batch
   - Only returns to the main loop after the **entire batch** completes

2. `ParallelExecutor::execute_with_reanalysis`:
   - Receives a fixed set of `changes` at invocation
   - Processes groups sequentially (first group, then re-analyze remaining, etc.)
   - Has no mechanism to receive newly queued items during execution

This creates a "batch barrier" where dynamic queue additions are only processed between batches.

## Goals / Non-Goals

**Goals:**
- Enable immediate execution of newly queued items when slots are available
- Maintain debounce logic to prevent excessive re-analysis
- Preserve existing event reporting for TUI state updates
- Keep CLI mode working (no dynamic queue)

**Non-Goals:**
- Change the fundamental group-based execution model
- Modify the debounce period (10 seconds) unless necessary
- Add real-time queue watching (polling is sufficient)

## Decisions

### Decision: Pass `DynamicQueue` to `ParallelExecutor`

Instead of polling the queue from the TUI's main loop, pass an `Arc<DynamicQueue>` reference to the executor so it can check for new items at key points:

1. **After each semaphore permit acquisition** - when a slot becomes available
2. **After each group completion** - when re-analysis is triggered

**Rationale**: This allows the executor to integrate new items into the current iteration without waiting for batch completion.

### Decision: Inject into pending set, not current group

When new items are detected:
1. Add them to the `changes` vector (the remaining items to process)
2. Let the existing re-analysis logic group them with remaining changes
3. Do NOT inject directly into an in-progress group (avoids race conditions)

**Rationale**: Simpler to implement and maintains the integrity of the group-based execution model.

### Decision: Use tokio::select for non-blocking queue check

```rust
// Pseudo-code for slot-aware queue polling
loop {
    // Check for newly queued items (non-blocking)
    if let Some(new_id) = dynamic_queue.as_ref().and_then(|q| q.try_pop()) {
        // Load change details and add to pending
        if let Ok(change) = load_change(&new_id) {
            changes.push(change);
            queue_changed = true;
        }
    }

    // ... existing group execution logic
}
```

**Rationale**: Non-blocking check prevents stalling the execution loop.

## Risks / Trade-offs

### Risk: Excessive re-analysis overhead

**Mitigation**: Keep the existing 10-second debounce. New items are added to the pending set immediately but re-analysis only happens after debounce expires.

### Risk: Race condition between queue pop and pending set update

**Mitigation**: The `DynamicQueue` already uses `tokio::sync::Mutex` for thread safety. Popped items won't be re-popped.

### Risk: Items added during in-flight group execution

**Mitigation**: Items are added to the `changes` vector which is processed in the next iteration. Current group's semaphore permits are not affected.

## Migration Plan

No migration needed - this is a behavioral fix that maintains backward compatibility:
- CLI mode: No `DynamicQueue` provided, behavior unchanged
- TUI mode: `DynamicQueue` provided, enables immediate slot utilization

## Open Questions

1. Should we add a configuration option for the queue polling interval? (Currently proposal assumes no interval, just check on each loop iteration)
2. Should the debounce period be configurable separately from the existing 10-second value?
