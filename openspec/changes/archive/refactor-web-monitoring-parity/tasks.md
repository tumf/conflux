## 1. Backend Implementation
- [x] 1.1 Extend WebState with TUI-equivalent state fields (queue_status, logs, worktrees, mode)
- [x] 1.2 Add ExecutionEvent subscription to WebState for real-time updates
- [x] 1.3 Implement WebState event handlers for all ExecutionEvent types (ProcessingStarted, Log, ChangesRefreshed, WorktreesRefreshed, etc.)
- [x] 1.4 Update websocket.rs to broadcast ExecutionEvent-based updates to clients
- [x] 1.5 Ensure WebState state updates are thread-safe and broadcast-ready

## 2. Integration
- [x] 2.1 Wire TUI orchestrator to send ExecutionEvents to WebState
- [x] 2.2 Wire parallel executor to send ExecutionEvents to WebState
- [x] 2.3 Ensure both TUI and Web receive same ExecutionEvent stream
- [x] 2.4 Test WebSocket reconnection with state restoration (WebSocket already sends initial state on connect)

## Future work

### Frontend Implementation
- Extend web UI state model to handle queue status, logs, and worktrees
- Add WebSocket message handlers for new event types (Log, WorktreesRefreshed, mode changes)
- Implement UI components for displaying logs (similar to TUI log panel)
- Implement UI components for displaying worktrees (similar to TUI worktree view)
- Update change cards to show queue_status badges (Queued, Processing, Archiving, etc.)

### Testing
- Verify TUI and Web UI show identical change states during serial execution
- Verify TUI and Web UI show identical change states during parallel execution
- Verify logs appear in both TUI and Web UI in real-time
- Verify worktree list updates in both TUI and Web UI
- Test WebSocket reconnection preserves latest state
