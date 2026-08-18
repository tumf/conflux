## 1. Settlement scope and plan

- [ ] 1.1 Record the exact target IDs whose accepted marks changed for individual, bulk, and API mutations (verification: unit - `cargo test --lib mark_settlement_changed_targets`; verification-id: local-regression)
- [ ] 1.2 Preserve the existing stability window and re-read only those targets from one coherent expiry snapshot (verification: unit - `cargo test --lib mark_settlement_delta_scope`; verification-id: local-regression)
- [ ] 1.3 Extend the settlement plan with ordinary pending removals and stable exclusions for active, in-flight, lane-wait, retry, MergeWait, ResolveWait, RejectWait, blocked, stalled, terminal, archive-complete, unknown, and ineligible targets (verification: unit - `cargo test --lib bidirectional_mark_reconciliation_classification`; verification-id: local-regression)

## 2. Guarded queue application

- [ ] 2.1 Add the minimum settlement-aware application guard under the authoritative reducer write boundary (verification: unit - `cargo test --lib mark_settlement_application_guard`; verification-id: local-regression)
- [ ] 2.2 Make raced removals no-op after active/in-flight/wait/terminal transition without clearing lifecycle evidence (verification: unit - `cargo test --lib mark_settlement_removal_race_preserves_active`; verification-id: local-regression)
- [ ] 2.3 Make raced additions no-op after terminal-error/excluded transition without `RetryError` or explicit-retry publication (verification: unit - `cargo test --lib mark_settlement_addition_race_never_retries`; verification-id: local-regression)
- [ ] 2.4 Preserve queue hooks exactly once per successful target mutation and coalesce scheduler notification to exactly once per batch with one or more applied membership changes (verification: unit - `cargo test --lib mark_settlement_hooks_and_notification`; verification-id: local-regression)

## 3. Queue/TUI behavior

- [ ] 3.1 Apply additions and removals through the shared reducer-backed queue path so the TUI projects queued and `not queued` from authoritative intent (verification: unit - `cargo test --lib bidirectional_mark_reconciliation_projection`; verification-id: local-regression)
- [ ] 3.2 Prove individual, bulk, and API mark deltas produce the same settlement result (verification: unit - `cargo test --lib bidirectional_mark_reconciliation_entrypoints`; verification-id: local-regression)
- [ ] 3.3 Prove unrelated explicitly queued unmarked targets and explicitly removed marked targets remain unchanged (verification: unit - `cargo test --lib mark_settlement_preserves_unrelated_queue_intent`; verification-id: local-regression)
- [ ] 3.4 Prove unmarking active work changes only next-run selection and never cancels, stops, dequeues, or changes phase (verification: unit - `cargo test --lib mark_settlement_active_unmark_is_mark_only`; verification-id: local-regression)

## 4. Scheduler capacity gate

- [ ] 4.1 Gate expensive dependency-analyzer invocation on a freshly recomputed positive available-slot count while retaining zero-capacity classification, reducer reconciliation, and diagnostics (verification: unit - `cargo test --lib reanalysis_zero_capacity_gates_analyzer`; verification-id: local-regression)
- [ ] 4.2 On capacity suppression, do not record completed/suppression signatures or consume unevaluated queue/completion/repair/slot-recovery edges (verification: unit - `cargo test --lib reanalysis_zero_capacity_preserves_edge_and_signature`; verification-id: local-regression)
- [ ] 4.3 Prove zero-to-positive slot recovery re-evaluates remaining eligible queued work after completion, failure, and resolve paths (verification: unit - `cargo test --lib reanalysis_slot_recovery_after_capacity_gate`; verification-id: local-regression)
- [ ] 4.4 Prove empty eligible queue and unchanged/no-op batches never start dependency analysis or synthetic activity (verification: unit - `cargo test --lib reanalysis_empty_or_noop_suppressed`; verification-id: local-regression)

## 5. Restart and regression verification

- [ ] 5.1 Prove restart clears process-local marks and pending settlement without delayed queue mutation or routing changes (verification: unit - `cargo test --lib mark_settlement_restart_is_process_local`; verification-id: local-regression)
- [ ] 5.2 Run focused orchestration tests with paused time (verification: unit - `cargo test --lib mark_settlement_ && cargo test --lib bidirectional_mark_reconciliation`; verification-id: local-regression)
- [ ] 5.3 Run focused parallel scheduler tests (verification: unit - `cargo test --lib reanalysis_zero_capacity && cargo test --lib reanalysis_slot_recovery`; verification-id: local-regression)
- [ ] 5.4 Run full pre-integration verification (verification: integration - `cargo test --lib && cargo fmt -- --check && cargo clippy --all-targets --all-features -- -D warnings`; verification-id: local-regression)

## Final Validation

Archive validation is the authoritative OpenSpec gate. Expected command: `cflx openspec validate restore-bidirectional-mark-reanalysis --archive-gate`.
