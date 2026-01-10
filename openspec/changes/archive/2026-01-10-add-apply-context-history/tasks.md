# Tasks: Add Apply Context History

## Phase 1: Core Data Structures

- [x] 1.1 Create `ApplyAttempt` struct in new module `src/history.rs`
  - Fields: attempt number, success flag, duration, error message, exit code
  - Derive Debug, Clone

- [x] 1.2 Implement `ApplyHistory` struct
  - HashMap<String, Vec<ApplyAttempt>> storage
  - Methods: new(), record(), get(), last(), count(), clear()

- [x] 1.3 Implement `format_context()` method
  - Generate `<last_apply>` XML format for each previous attempt
  - Include status, duration, error, exit_code fields

- [x] 1.4 Add unit tests for ApplyHistory
  - Test record and retrieval
  - Test multiple attempts accumulation
  - Test clear functionality
  - Test format_context output

## Phase 2: AgentRunner Integration

- [x] 2.1 Add `apply_history: ApplyHistory` field to `AgentRunner`
  - Initialize in `new()`
  - Update struct to require `&mut self` for run_apply methods

- [x] 2.2 Update `run_apply()` to build prompt with history
  - Get base prompt from config
  - Append history context if available
  - Pass combined prompt to expand_prompt()

- [x] 2.3 Update `run_apply()` to record attempt after execution
  - Capture start time before execution
  - Calculate duration after completion
  - Create ApplyAttempt with result status
  - Record to history

- [x] 2.4 Add `clear_apply_history()` method to AgentRunner
  - Delegate to ApplyHistory.clear()

- [x] 2.5 Update `run_apply_streaming()` similarly
  - Same history context injection
  - Recording after completion requires different approach (caller manages child)

## Phase 3: Orchestrator Integration

- [x] 3.1 Update Orchestrator to use `&mut self.agent` for apply calls
  - Ensure mutable borrow is available

- [x] 3.2 Call `clear_apply_history()` after successful archive
  - In archive_change success path
  - Clean up memory for completed changes

## Validation

- [x] Run `cargo test` - all tests pass
- [x] Run `cargo clippy` - no warnings (except existing ones)
