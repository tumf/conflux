## 1. Requirements & Design
- [x] 1.1 Review existing specs (circuit-breaker/parallel-execution/cli/web-monitoring/configuration) and clarify merge stall monitoring delta
- [x] 1.2 Design monitoring targets (serial/parallel both, Merge change: <change_id>) and stop conditions
- [x] 1.3 Design monitoring task start/stop timing and cancellation integration
- [x] 1.4 Define config options (threshold, monitoring interval) and default values

## 2. Implementation
- [x] 2.1 Add merge progress stall detection logic
- [x] 2.2 Integrate monitoring task into orchestrator/run loop (parallel mode)
- [x] 2.3 Trigger CancellationToken on stall detection and stop immediately
- [x] 2.4 Reflect stall reason in CLI/TUI/Web stop messages
- [x] 2.5 Load config values and implement default/override behavior

## 3. Verification
- [x] 3.1 Add test for stop event on monitoring timeout
- [x] 3.2 Verify behavior when config values are changed
- [x] 3.3 Confirm no interference with existing stall/circuit-breaker logic


## Acceptance Failure Follow-up
- [x] Address acceptance findings:
  1) ✓ Updated specs/circuit-breaker/spec.md to clarify parallel-mode-only (lines 20-28)
  2) ✓ Updated design.md to document architectural constraint (Context, Goals, Decisions, Risks sections)
  3) ✓ Updated proposal.md to add "Architectural Constraint" section explaining why serial mode is excluded
  4) ✓ Implementation is correct: MergeStallMonitor only runs in parallel mode where "Merge change:" commits exist
  5) ✓ Serial mode correctly excluded: no "Merge change:" commits → no merge stall monitoring needed
  6) ✓ Documentation now aligns with implementation: parallel-mode-only design is intentional and documented
