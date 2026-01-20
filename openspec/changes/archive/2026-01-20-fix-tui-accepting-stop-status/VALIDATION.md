# Validation Procedure for fix-tui-accepting-stop-status

## Objective
Verify that when a change in "accepting" status is force-stopped (Esc Esc), the accepting display disappears and the status returns to NotQueued.

## Prerequisites
- Build completed: `cargo build --release`
- Test repository with openspec changes available

## Test Procedure

### Step 1: Prepare Test Environment
```bash
# Ensure we're in the correct worktree
cd "/Users/tumf/Library/Application Support/conflux/worktrees/openspec-orchestrator-4f5ff44e/fix-tui-accepting-stop-status"

# Verify build is complete
ls -la target/release/conflux
```

### Step 2: Launch TUI in Parallel Mode
```bash
# Run conflux in TUI mode with parallel execution
./target/release/conflux run --parallel
```

### Step 3: Trigger Accepting Status
1. Select a change that has tasks to execute
2. Press `Space` to queue the change
3. Wait for the change to start processing
4. Observe when the change transitions to "accepting" status
   - This happens after apply completes and before archive starts
   - Look for "accepting" indicator in the TUI

### Step 4: Force Stop During Accepting
1. While the change is in "accepting" status, press `Esc` twice quickly
2. Observe the TUI state change

### Expected Results
- ✅ The "accepting" indicator should disappear immediately
- ✅ The change status should return to "NotQueued"
- ✅ The change should no longer be in the processing queue
- ✅ The selected mark (if any) should be preserved

### Failure Indicators
- ❌ "accepting" indicator remains visible after force stop
- ❌ Change status shows anything other than "NotQueued"
- ❌ Change remains in processing queue

## Code Reference
The fix is implemented in `src/tui/state/events/processing.rs`, line 91:
```rust
QueueStatus::Processing
    | QueueStatus::Accepting  // ← Added this line
    | QueueStatus::Archiving
    | QueueStatus::Queued => {
    change.queue_status = QueueStatus::NotQueued;
}
```

## Validation Checklist
- [ ] TUI launches successfully
- [ ] Change can be queued and starts processing
- [ ] Change transitions to "accepting" status
- [ ] Esc Esc during accepting triggers force stop
- [ ] "accepting" indicator disappears
- [ ] Status returns to NotQueued
- [ ] No errors in logs

## Notes
- If the change completes too quickly, you may need to use a change with longer execution time
- The "accepting" status is typically brief (between apply and archive)
- You may need multiple attempts to catch the accepting state
