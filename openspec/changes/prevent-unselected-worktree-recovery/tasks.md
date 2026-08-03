## Implementation Tasks

- [ ] Remove repository-wide archived-dirty worktree discovery as a source of queued IDs in `src/parallel/queue_state.rs`; reconcile ordinary candidates only from initial explicit targets and current reducer `queued_change_ids()`, resolving a missing active change from that explicit ID's preserved workspace. (verification: integration - `cargo test parallel::tests::executor` proves unrelated worktrees never enter queued/analysis candidates while an explicitly targeted archived-dirty ID still resolves to repair work; verification-id: explicit-recovery-intent-tests)

- [ ] Preserve frontend-neutral explicit intent through `src/orchestration/run_control.rs`, `src/orchestration/operator_command.rs`, `src/orchestrator.rs`, and `src/tui/orchestrator.rs`: TUI/remote Start share target resolution, CLI targets enter the same initial contract, and TUI/remote queue or retry commands produce reducer queued intent without a new allowlist. (verification: unit/integration - `cargo test orchestration::run_control && cargo test tui::command_handlers && cargo test tui::orchestrator` proves equivalent start, queue, retry, and no-op behavior; verification-id: explicit-recovery-intent-tests)

- [ ] Keep `ChangesRefreshed` and `add_dynamic_change` as catalog/runtime registration only, and ensure refresh cannot create queue or lane eligibility for unselected changes. (verification: unit - `cargo test orchestration::state && cargo test tui::orchestrator` proves all-change refresh registers `stale` while leaving `QueueIntent::NotQueued` and all wait states clear; verification-id: explicit-recovery-intent-tests)

- [ ] Enforce revocation: `RemoveFromQueue`, successful stop-and-dequeue, and `DequeueChange` prevent stale scheduler/dynamic entries and preserved worktrees from reacquiring ordinary work; explicit `AddToQueue` restores eligibility. (verification: integration - `cargo test parallel::tests::executor && cargo test orchestration::state` covers add, remove, dequeue, repeated reconciliation, and explicit requeue against one archived-dirty fixture; verification-id: explicit-recovery-intent-tests)

- [ ] Add a production-order temporary-Git regression with selected `fresh`, unselected archived-dirty `stale`, `ChangesRefreshed(fresh, stale)`, reconciliation, captured analyzer input, and lifecycle-event capture; compare `stale` HEAD, branch ref, index, status, and file bytes before and after. (verification: integration - `cargo test parallel::tests::executor` fails if `stale` is analyzed, emits apply/accept/archive/resolve/reject/merge events, mutates Git/worktree evidence, or keeps the drained run alive; verification-id: explicit-recovery-intent-tests)

- [ ] Add positive recovery tests for initial TUI/CLI/remote explicit targets and accepted dynamic queue intent, proving archived-dirty evidence resumes archive finalization or archive-complete handoff without rerunning apply. (verification: integration - `cargo test parallel::tests::executor && cargo test orchestration::run_control && cargo test tui::orchestrator` captures explicit target/queue inputs and workspace-derived resume events across shared frontend boundaries; verification-id: explicit-recovery-intent-tests)

- [ ] Preserve reducer-owned lane and terminal gates with explicit-intent-passing fixtures: manual `MergeWait` requires `ResolveMerge`, empty ordinary queues independently consume `ResolveWait` and `RejectWait`, admitted merged residue stops on merged evidence, and admitted terminal error stops until `RetryError`. (verification: integration - `cargo test parallel::tests::executor && cargo test orchestration::state && cargo test tui::orchestrator` proves each dedicated gate rather than passing solely because ordinary intent is absent; verification-id: explicit-recovery-intent-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate prevent-unselected-worktree-recovery --archive-gate`

The implementation must also pass `cargo test parallel::tests::executor && cargo test tui::orchestrator && cargo test orchestration::state && cargo test orchestration::run_control && cargo test tui::command_handlers`, `cargo fmt -- --check`, and `cargo clippy -- -D warnings`.

## Future Work

- A separate operator workflow may expose interrupted unrequested worktrees as attention items, but observability must not grant execution intent.
- Canonical duplicate requirement cleanup outside the exact promoted `Queue ingestion and analysis targeting` result may be handled as repository specification hygiene if archive promotion does not already collapse identical headings.
