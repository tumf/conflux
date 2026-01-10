# Design: Add Stop Processing Feature

## Architecture Overview

The stop feature integrates with the existing TUI event loop and orchestrator cancellation system.

```
┌─────────────────────────────────────────────────────────────┐
│                       TUI Event Loop                        │
├─────────────────────────────────────────────────────────────┤
│  [Esc pressed]                                              │
│       │                                                     │
│       ▼                                                     │
│  ┌─────────────────┐                                        │
│  │ Check stop_mode │                                        │
│  └────────┬────────┘                                        │
│           │                                                 │
│     ┌─────┴─────┐                                           │
│     │           │                                           │
│     ▼           ▼                                           │
│  None      GracefulPending                                  │
│     │           │                                           │
│     ▼           ▼                                           │
│  Set to    cancel_token.cancel()                            │
│  Graceful  (force kill process)                             │
│  Pending                                                    │
└─────────────────────────────────────────────────────────────┘
```

## State Machine

### Stop Modes

```
                    ┌──────────────┐
                    │    None      │ (Normal running)
                    └──────┬───────┘
                           │ Esc pressed
                           ▼
                    ┌──────────────┐
                    │   Graceful   │ (Waiting for current process)
                    │   Pending    │
                    └──────┬───────┘
                           │ Esc pressed again
                           ▼
                    ┌──────────────┐
                    │    Force     │ (Process killed immediately)
                    │   Stopped    │
                    └──────────────┘
                           │
                           ▼
                    ┌──────────────┐
                    │   Stopped    │ (Can modify queue, F5 to resume)
                    └──────────────┘
```

### AppMode Extension

Current modes: `Selecting`, `Running`, `Completed`, `Error`

Add: `Stopping`, `Stopped`

- **Stopping**: Graceful stop pending, current process completing
- **Stopped**: Processing halted, can modify queue

## Key Components

### 1. StopMode Enum

```rust
pub enum StopMode {
    /// Not stopping, normal operation
    None,
    /// Graceful stop requested, waiting for current process
    GracefulPending,
    /// Force stop executed
    ForceStopped,
}
```

### 2. AppState Changes

```rust
pub struct AppState {
    // ... existing fields ...

    /// Current stop mode
    pub stop_mode: StopMode,
}
```

### 3. Orchestrator Integration

The existing `CancellationToken` is used:
- Graceful: Set a flag, let current process complete, then stop loop
- Force: Call `cancel_token.cancel()` to kill process

### 4. UI Display Changes

Header status shows:
- "Running" → "Stopping..." (yellow) → "Stopped" (gray)

Help text in running mode:
- Add "Esc: stop" to key bindings

## Implementation Flow

### Esc Key Handler

```
1. Check current mode (must be Running or Stopping)
2. If stop_mode == None:
   - Set stop_mode = GracefulPending
   - Set should_stop_after_current = true
   - Log "Stopping after current change..."
3. Else if stop_mode == GracefulPending:
   - cancel_token.cancel()
   - Set stop_mode = ForceStopped
   - Log "Force stopped"
4. Transition to Stopped mode
```

### Orchestrator Loop Integration

```
Each iteration:
1. Check if should_stop_after_current flag is set
2. If set and current process completed:
   - Do not pick next change
   - Send StoppedEvent to TUI
   - Exit loop
```

### Resume Flow

```
1. In Stopped mode, F5 pressed
2. Reset stop_mode to None
3. Start new orchestrator with remaining queued changes
4. Transition to Running mode
```

## State Transitions

| Current State | Event | Next State |
|--------------|-------|------------|
| Running | Esc (first) | Stopping |
| Stopping | Process completes | Stopped |
| Stopping | Esc (second) | Stopped (force) |
| Stopped | F5 | Running |
| Stopped | Space | Queue modified |
| Stopped | q | Exit TUI |

## Error Handling

- If process is killed during graceful stop, mark change as queued (not error)
- If force stop fails (process unresponsive), show warning and try SIGKILL
- Network/IPC errors during stop should not prevent state transition

## Testing Considerations

1. Unit tests for state transitions
2. Integration test: graceful stop waits for completion
3. Integration test: force stop kills process
4. Integration test: resume after stop works correctly

## Backwards Compatibility

- No config file changes required
- No CLI argument changes
- Existing q/Ctrl+C behavior unchanged
