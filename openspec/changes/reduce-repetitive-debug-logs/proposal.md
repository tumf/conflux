# Reduce Repetitive Debug Logs in TUI Mode

## Background

The TUI mode of openspec-orchestrator generates excessive repetitive DEBUG logs every 5 seconds, even when there are no state changes. Analyzing `/tmp/tui-serial-debug.log` reveals the following repetitive patterns:

### Current Issues

1. **Change Status Polling**: Every 5 seconds, the system logs:
   - Task parsing results (e.g., "Parsed task progress: 0/11 tasks completed")
   - Approval status checks (e.g., "is not approved: no approved file")
   - Change discovery counts (e.g., "Found 8 changes via native parsing")

2. **Redundant Information**: The same 8 changes are checked repeatedly with identical results:
   - `add-api-rate-limit-handler`: "0/11 tasks, not approved"
   - `add-consecutive-done-detector`: "0/11 tasks, not approved"
   - `add-same-error-circuit-breaker`: "0/12 tasks, not approved"
   - `fix-non-empty-merge-commits`: "5/25 tasks, approved"
   - `add-command-logging`: "0/18 tasks, approved"
   - `add-progress-stall-detector`: "0/10 tasks, not approved"
   - `update-parallel-processing-start-event`: "0/4 tasks, not approved"
   - `add-output-decline-detector`: "0/11 tasks, not approved"

3. **Log Volume**: This generates ~18 DEBUG lines per change per poll cycle (144+ lines every 5 seconds), overwhelming the log file.

## Goals

1. Suppress repetitive DEBUG logs when state hasn't changed
2. Maintain visibility of actual state transitions
3. Reduce log file size and improve readability
4. Make debugging more efficient by focusing on changes

## Proposal

### 1. State Change Detection

Implement a state caching mechanism to detect actual changes:

```rust
// src/tui/state/change.rs or new file src/tui/log_deduplicator.rs
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ChangeStateSnapshot {
    id: String,
    completed_tasks: u32,
    total_tasks: u32,
    is_approved: bool,
}

pub struct LogDeduplicator {
    last_states: HashMap<String, ChangeStateSnapshot>,
    last_log_time: std::time::Instant,
}

impl LogDeduplicator {
    pub fn new() -> Self {
        Self {
            last_states: HashMap::new(),
            last_log_time: std::time::Instant::now(),
        }
    }

    pub fn should_log(&mut self, change_id: &str, snapshot: ChangeStateSnapshot) -> bool {
        if let Some(last) = self.last_states.get(change_id) {
            if last == &snapshot {
                return false; // No change, suppress log
            }
        }
        self.last_states.insert(change_id.to_string(), snapshot);
        true
    }
}
```

### 2. Modified Logging in Key Modules

#### a. Task Parser (`src/task_parser.rs:79`)

```rust
// Before:
debug!("Parsed task progress: {}/{} tasks completed", completed, total);

// After:
if log_deduplicator.should_log_task_progress(change_id, completed, total) {
    debug!("Parsed task progress: {}/{} tasks completed", completed, total);
}
```

#### b. Approval Module (`src/approval.rs:159, 245`)

```rust
// Before:
debug!("Change '{}' is not approved: no approved file", change_id);

// After:
if log_deduplicator.should_log_approval_status(change_id, false) {
    debug!("Change '{}' is not approved: no approved file", change_id);
}
```

#### c. OpenSpec Module (`src/openspec.rs:199`)

```rust
// Before:
debug!("Found {} changes via native parsing", changes.len());

// After:
if log_deduplicator.should_log_change_count(changes.len()) {
    debug!("Found {} changes via native parsing", changes.len());
}
```

### 3. Periodic Summary Logging

Even when suppressing repetitive logs, provide periodic summaries:

```rust
impl LogDeduplicator {
    const SUMMARY_INTERVAL: Duration = Duration::from_secs(60);

    pub fn maybe_log_summary(&mut self) {
        if self.last_log_time.elapsed() >= Self::SUMMARY_INTERVAL {
            info!("Status summary: {} changes tracked", self.last_states.len());
            for (id, state) in &self.last_states {
                info!("  - {}: {}/{} tasks, approved={}",
                      id, state.completed_tasks, state.total_tasks, state.is_approved);
            }
            self.last_log_time = std::time::Instant::now();
        }
    }
}
```

### 4. Configuration Option

Add a configuration option to control this behavior:

```jsonc
// .openspec-orchestrator.jsonc
{
  "logging": {
    "suppress_repetitive_debug": true,  // Default: true
    "summary_interval_secs": 60         // Default: 60
  }
}
```

## Implementation Plan

### Task List

- [ ] Create `src/tui/log_deduplicator.rs` module
- [ ] Implement `LogDeduplicator` with state tracking
- [ ] Add configuration options to config schema
- [ ] Modify `src/task_parser.rs` to use deduplicator
- [ ] Modify `src/approval.rs` to use deduplicator
- [ ] Modify `src/openspec.rs` to use deduplicator
- [ ] Add periodic summary logging
- [ ] Update documentation
- [ ] Add unit tests for deduplicator
- [ ] Test with actual TUI mode execution
- [ ] Verify log volume reduction

### Testing Strategy

1. **Unit Tests**: Test `LogDeduplicator` state tracking logic
2. **Integration Tests**:
   - Run TUI mode for 60 seconds
   - Compare log file size before/after
   - Verify state changes are still logged
3. **Manual Testing**:
   - Monitor `/tmp/tui-serial-debug.log` during execution
   - Confirm suppression of repetitive logs
   - Verify summary logs appear every 60 seconds

## Expected Outcomes

1. **Log Volume Reduction**: 95%+ reduction in repetitive DEBUG logs
2. **Better Debugging**: Focus on actual state transitions
3. **Performance**: Minimal overhead (simple hash comparisons)
4. **Backward Compatibility**: Optional feature, default enabled

## Alternatives Considered

1. **Rate Limiting**: Suppress logs if same content within N seconds
   - Cons: May miss rapid state changes

2. **Log Level Adjustment**: Change repetitive logs to TRACE level
   - Cons: Loses useful information when debugging specific issues

3. **Sampling**: Log every Nth occurrence
   - Cons: May miss important transitions

The proposed state-based deduplication is preferred because it preserves all meaningful state changes while eliminating noise.

## Dependencies

- None (self-contained change)

## Risks

- If state comparison logic is incorrect, important logs may be suppressed
- Mitigation: Comprehensive unit tests and manual verification
