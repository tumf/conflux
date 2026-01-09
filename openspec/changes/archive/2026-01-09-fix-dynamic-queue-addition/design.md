# Design: Dynamic Queue Addition

## Architecture Overview

```
┌─────────────────────┐     ┌──────────────────────┐
│       TUI           │     │    Orchestrator      │
│  (run_tui_loop)     │     │  (run_orchestrator)  │
└─────────┬───────────┘     └──────────┬───────────┘
          │                            │
          │   ┌────────────────────┐   │
          └──►│   DynamicQueue     │◄──┘
              │ Arc<Mutex<VecDeque>>│
              └────────────────────┘
```

## Data Structures

### DynamicQueue

```rust
pub struct DynamicQueue {
    inner: Arc<Mutex<VecDeque<String>>>,
}

impl DynamicQueue {
    pub fn new() -> Self { ... }

    /// Push a change ID to the queue
    /// Returns false if already in queue or limit reached
    pub async fn push(&self, id: String) -> bool { ... }

    /// Pop next change ID from queue
    pub async fn pop(&self) -> Option<String> { ... }

    /// Check if queue is empty
    pub async fn is_empty(&self) -> bool { ... }

    /// Check if ID is already in queue
    pub async fn contains(&self, id: &str) -> bool { ... }
}
```

## Flow Diagram

### User Adds Change to Queue

```
1. User presses Space on NotQueued change
2. toggle_selection() called
3. Update local QueueStatus to Queued
4. Push change_id to DynamicQueue
5. Log "Added to queue: <id>"
```

### Orchestrator Processes Dynamic Queue

```
1. Complete processing current change
2. Check DynamicQueue.pop()
3. If Some(id):
   a. Send ProcessingStarted event
   b. Process change (apply/archive)
   c. Send ProcessingCompleted event
   d. Goto step 1
4. If None:
   a. Check if more initial changes remain
   b. If yes, process next initial change
   c. If no, send AllCompleted event
```

## Thread Safety

- Use `tokio::sync::Mutex` for async compatibility
- Lock scope kept minimal (push/pop only)
- No nested locks to prevent deadlocks
- Clone Arc for each task that needs access

## Error Handling

| Scenario | Handling |
|----------|----------|
| Push duplicate ID | Return false, log warning |
| Pop from empty queue | Return None |
| ID not found in changes | Log error, skip, continue |
| Lock contention | Use short critical sections |

## Integration Points

### tui.rs Changes

1. Add `dynamic_queue: DynamicQueue` parameter to `run_tui_loop`
2. Pass clone to orchestrator spawn
3. In `toggle_selection()`:
   - Replace `TuiCommand::AddToQueue` with `dynamic_queue.push()`
   - Keep `TuiCommand::RemoveFromQueue` for UI state (cannot remove from orchestrator)

### Orchestrator Changes

1. Add `dynamic_queue: DynamicQueue` parameter to `run_orchestrator`
2. After processing each change, check `dynamic_queue.pop()`
3. Process popped changes before checking initial queue
4. Only send `AllCompleted` when both initial and dynamic queues are empty
