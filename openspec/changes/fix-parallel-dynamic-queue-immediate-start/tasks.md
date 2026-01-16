## 1. Core Implementation

- [ ] 1.1 Add `DynamicQueue` parameter to `ParallelExecutor` constructor or `execute_with_reanalysis` method
- [ ] 1.2 Implement queue polling within the main execution loop (after each semaphore acquisition)
- [ ] 1.3 Add logic to inject newly queued changes into the current iteration's pending set
- [ ] 1.4 Ensure newly added changes go through dependency analysis before execution

## 2. Event Reporting

- [ ] 2.1 Emit `AnalysisStarted` event when re-analysis is triggered for newly queued items
- [ ] 2.2 Preserve existing event flow for `WorkspaceCreated`, `ApplyStarted`, etc.

## 3. Integration

- [ ] 3.1 Update `ParallelRunService::run_parallel_with_executor` to pass `DynamicQueue` reference
- [ ] 3.2 Update `run_orchestrator_parallel` in TUI to leverage the new capability
- [ ] 3.3 Ensure CLI mode continues to work (no `DynamicQueue` provided)

## 4. Testing

- [ ] 4.1 Add unit test for immediate queue injection during execution
- [ ] 4.2 Add integration test simulating Space key press during parallel batch
- [ ] 4.3 Verify debounce logic still applies for re-analysis timing

## 5. Documentation

- [ ] 5.1 Update AGENTS.md if needed to describe new behavior
