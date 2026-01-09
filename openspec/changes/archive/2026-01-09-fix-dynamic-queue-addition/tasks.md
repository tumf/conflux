# Tasks for fix-dynamic-queue-addition

## Phase 1: Infrastructure Setup

- [x] Create `DynamicQueue` struct with `Arc<Mutex<VecDeque<String>>>`
- [x] Add queue initialization in `run_tui_loop()`
- [x] Pass queue reference to `run_orchestrator()`

## Phase 2: TUI Integration

- [x] Update `toggle_selection()` to push to shared queue when adding
- [x] Remove redundant `TuiCommand::AddToQueue` handling (replaced by shared queue)
- [x] Add log entry when change is added to dynamic queue

## Phase 3: Orchestrator Integration

- [x] Modify `run_orchestrator()` to accept dynamic queue parameter
- [x] Add queue polling after each change completion
- [x] Process dynamically added changes in FIFO order
- [x] Skip already-processed or archived changes

## Phase 4: State Synchronization

- [x] Ensure `ProcessingStarted` event fires for dynamic changes
- [x] Update TUI state when dynamic change starts processing
- [x] Handle edge case: change added while another is processing

## Phase 5: Testing

- [x] Unit test: DynamicQueue push/pop operations
- [x] Unit test: Queue empty check
- [x] Integration test: Dynamic addition during waiting
- [x] Integration test: Multiple dynamic additions

## Phase 6: Documentation

- [x] Update TUI help text to clarify dynamic queue behavior
- [x] Add log message when dynamic change is picked up
