# Parallel Orchestration Integration

## Overview

This document describes the integration between the parallel executor and the common orchestration loops for apply and archive operations.

## Current State (After Integration)

### Shared Components

1. **Command Queue with Retry & Stagger** (✓ Already integrated)
   - Parallel executor uses `AiCommandRunner` which wraps `CommandQueue`
   - Provides automatic retry on transient errors
   - Staggers command starts to prevent resource conflicts
   - Location: `src/ai_command_runner.rs`, `src/command_queue.rs`

2. **Hook Execution** (✓ Already integrated)
   - Both serial and parallel modes use `HookRunner`
   - Hooks are passed through and executed consistently
   - Location: `src/hooks.rs`

3. **VCS Operations** (✓ Already integrated)
   - Both modes use VCS abstraction layer
   - Workspace management is consistent
   - Location: `src/vcs/`

### New Integration Components

4. **Output Handler Bridge** (✓ Newly added)
   - `ParallelOutputHandler` implements `OutputHandler` trait
   - Converts `OutputHandler` calls to `ParallelEvent` sends
   - Allows orchestration functions to work with parallel event channels
   - Location: `src/parallel/output_bridge.rs`

5. **Apply Event Handler Bridge** (✓ Newly added)
   - `ParallelApplyEventHandler` implements `ApplyEventHandler` trait
   - Converts apply loop events to `ParallelEvent` sends
   - Enables unified apply loop to work with parallel mode
   - Location: `src/parallel/output_bridge.rs`

6. **Orchestration Adapter** (✓ Newly added)
   - Demonstrates how to use `orchestration::apply::apply_change_streaming`
   - Demonstrates how to use `orchestration::archive::archive_change_streaming`
   - Shows integration pattern for future refactoring
   - Location: `src/parallel/orchestration_adapter.rs`

### Remaining Gaps

1. **Apply History Tracking** (✗ Not integrated)
   - Serial mode uses `ApplyHistory` to track retry attempts
   - Parallel mode does not yet use this
   - Impact: Parallel mode doesn't provide retry context to AI agents
   - Location: `src/history.rs`

2. **Archive History Tracking** (✗ Not integrated)
   - Serial mode uses `ArchiveHistory` to track retry attempts
   - Parallel mode does not yet use this
   - Impact: Parallel mode doesn't provide retry context to AI agents
   - Location: `src/history.rs`

3. **Full Loop Replacement** (✗ Partially done)
   - Current parallel executor has its own apply/archive loops
   - New `execute_apply_loop` in `src/execution/apply.rs` provides unified implementation
   - Loops are structurally similar but not yet unified
   - Challenge: Parallel mode needs workspace-aware command execution

## Integration Pattern

The integration follows this pattern:

```rust
use crate::parallel::output_bridge::{ParallelOutputHandler, ParallelApplyEventHandler};
use crate::orchestration::apply::apply_change_streaming;
use crate::events::ExecutionEvent as ParallelEvent;

// Create bridges
let output_handler = ParallelOutputHandler::new(change_id, event_tx);
let event_handler = ParallelApplyEventHandler::new(change_id, event_tx);

// Use orchestration functions with bridges
let result = apply_change_streaming(
    &change,
    &mut agent,
    &hooks,
    &context,
    &output_handler,
    cancel_check,
).await?;
```

## Architecture

```
┌─────────────────────────────────────┐
│   Parallel Executor                 │
│   (src/parallel/executor.rs)        │
└───────────────┬─────────────────────┘
                │
                │ Uses bridges to adapt
                ▼
┌─────────────────────────────────────┐
│   Output/Event Bridges              │
│   (src/parallel/output_bridge.rs)   │
│   - ParallelOutputHandler           │
│   - ParallelApplyEventHandler       │
└───────────────┬─────────────────────┘
                │
                │ Implements traits
                ▼
┌─────────────────────────────────────┐
│   Common Orchestration              │
│   (src/orchestration/)              │
│   - apply_change_streaming()        │
│   - archive_change_streaming()      │
└───────────────┬─────────────────────┘
                │
                │ Uses
                ▼
┌─────────────────────────────────────┐
│   Execution Layer                   │
│   (src/execution/)                  │
│   - execute_apply_loop()            │
│   - Task progress checking          │
│   - WIP commit creation             │
└─────────────────────────────────────┘
```

## Benefits of Integration

1. **Consistency**: Apply and archive operations behave the same in serial and parallel modes
2. **Maintainability**: Shared code means fewer places to fix bugs
3. **Retry Logic**: CommandQueue retry/stagger is used consistently
4. **Hook Execution**: Hooks fire in the same order and context
5. **Event Flow**: OutputHandler abstraction enables flexible output routing

## Future Work

1. **Add History Tracking to Parallel Mode**
   - Modify `execute_apply_in_workspace` to use `AgentRunner::record_apply_attempt`
   - Modify `execute_archive_in_workspace` to use `AgentRunner::record_archive_attempt`
   - Pass apply/archive history context to AI commands

2. **Complete Loop Unification**
   - Refactor parallel executor to use `execute_apply_loop` from `src/execution/apply.rs`
   - Add workspace path parameter support to AgentRunner
   - Consider workspace-aware command execution strategy

3. **Testing**
   - Add integration tests for bridge adapters
   - Verify event ordering and content
   - Test cancellation behavior

## References

- Design Document: `openspec/changes/update-parallel-apply-archive-loop/design.md`
- Proposal: `openspec/changes/update-parallel-apply-archive-loop/proposal.md`
- Implementation: `src/parallel/output_bridge.rs`, `src/parallel/orchestration_adapter.rs`
