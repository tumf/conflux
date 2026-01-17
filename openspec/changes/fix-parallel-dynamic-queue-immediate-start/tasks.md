## 1. Core Implementation

- [x] 1.1 Add `DynamicQueue` parameter to `ParallelExecutor` constructor or `execute_with_reanalysis` method
- [x] 1.2 Implement queue polling within the main execution loop (after each semaphore acquisition)
- [x] 1.3 Add logic to inject newly queued changes into the current iteration's pending set
- [x] 1.4 Ensure newly added changes go through dependency analysis before execution

## 2. Event Reporting

- [x] 2.1 Emit `AnalysisStarted` event when re-analysis is triggered for newly queued items
- [x] 2.2 Preserve existing event flow for `WorkspaceCreated`, `ApplyStarted`, etc.

## 3. Integration

- [x] 3.1 Update `ParallelRunService::run_parallel_with_executor` to pass `DynamicQueue` reference
- [x] 3.2 Update `run_orchestrator_parallel` in TUI to leverage the new capability
- [x] 3.3 Ensure CLI mode continues to work (no `DynamicQueue` provided)

## 4. Testing

- [x] 4.1 Add unit test for immediate queue injection during execution
- [x] 4.2 Add integration test simulating Space key press during parallel batch
- [x] 4.3 Verify debounce logic still applies for re-analysis timing

## 5. Documentation

- [x] 5.1 Update AGENTS.md if needed to describe new behavior
