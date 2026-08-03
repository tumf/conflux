## Implementation Tasks

- [x] Remove repository-wide archived-dirty worktree discovery as a source of queued IDs in `src/parallel/queue_state.rs`; reconcile ordinary candidates only from initial explicit targets and current reducer `queued_change_ids()`, resolving a missing active change from that explicit ID's preserved workspace. (verification: integration - `cargo test parallel::tests::executor` proves unrelated worktrees never enter queued/analysis candidates while an explicitly targeted archived-dirty ID still resolves to repair work; verification-id: explicit-recovery-intent-tests)

- [x] Preserve frontend-neutral explicit intent through `src/orchestration/run_control.rs`, `src/orchestration/operator_command.rs`, `src/orchestrator.rs`, and `src/tui/orchestrator.rs`: TUI/remote Start share target resolution, CLI targets enter the same initial contract, and TUI/remote queue or retry commands produce reducer queued intent without a new allowlist. (verification: unit/integration - `cargo test orchestration::run_control && cargo test tui::command_handlers && cargo test tui::orchestrator` proves equivalent start, queue, retry, and no-op behavior; verification-id: explicit-recovery-intent-tests)

- [x] Keep `ChangesRefreshed` and `add_dynamic_change` as catalog/runtime registration only, and ensure refresh cannot create queue or lane eligibility for unselected changes. (verification: unit - `cargo test orchestration::state && cargo test tui::orchestrator` proves all-change refresh registers `stale` while leaving `QueueIntent::NotQueued` and all wait states clear; verification-id: explicit-recovery-intent-tests)

- [x] Enforce revocation: `RemoveFromQueue`, successful stop-and-dequeue, and `DequeueChange` prevent stale scheduler/dynamic entries and preserved worktrees from reacquiring ordinary work; explicit `AddToQueue` restores eligibility. (verification: integration - `cargo test parallel::tests::executor && cargo test orchestration::state` covers add, remove, dequeue, repeated reconciliation, and explicit requeue against one archived-dirty fixture; verification-id: explicit-recovery-intent-tests)

- [x] Add a production-order temporary-Git regression with selected `fresh`, unselected archived-dirty `stale`, `ChangesRefreshed(fresh, stale)`, reconciliation, captured analyzer input, and lifecycle-event capture; compare `stale` HEAD, branch ref, index, status, and file bytes before and after. (verification: integration - `cargo test parallel::tests::executor` fails if `stale` is analyzed, emits apply/accept/archive/resolve/reject/merge events, mutates Git/worktree evidence, or keeps the drained run alive; verification-id: explicit-recovery-intent-tests)

- [x] Add positive recovery tests for initial TUI/CLI/remote explicit targets and accepted dynamic queue intent, proving archived-dirty evidence resumes archive finalization or archive-complete handoff without rerunning apply. (verification: integration - `cargo test parallel::tests::executor && cargo test orchestration::run_control && cargo test tui::orchestrator` captures explicit target/queue inputs and workspace-derived resume events across shared frontend boundaries; verification-id: explicit-recovery-intent-tests)

- [x] Preserve reducer-owned lane and terminal gates with explicit-intent-passing fixtures: manual `MergeWait` requires `ResolveMerge`, empty ordinary queues independently consume `ResolveWait` and `RejectWait`, admitted merged residue stops on merged evidence, and admitted terminal error stops until `RetryError`. (verification: integration - `cargo test parallel::tests::executor && cargo test orchestration::state && cargo test tui::orchestrator` proves each dedicated gate rather than passing solely because ordinary intent is absent; verification-id: explicit-recovery-intent-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate prevent-unselected-worktree-recovery --archive-gate`

The implementation must also pass `cargo test parallel::tests::executor && cargo test tui::orchestrator && cargo test orchestration::state && cargo test orchestration::run_control && cargo test tui::command_handlers`, `cargo fmt -- --check`, and `cargo clippy -- -D warnings`.

## Notes

- Scheduler change: `reconcile_queued_candidates_from_shared_state` no longer calls
  `WorkspaceManager::list_worktree_change_ids`. Ordinary candidates come only from reducer
  `queued_change_ids()`, and a preserved workspace is inspected only for an ID that is already
  eligible. The base branch is now resolved lazily, on the first catalog miss, so an ordinary
  reconciliation pass performs no worktree I/O at all.
- Revocation: `OrchestratorState::is_ordinary_queue_eligible` is the shared revocation predicate.
  `check_dynamic_queue_and_add_changes` consults it, because `DequeueChange` does not drain the
  dynamic queue and would otherwise leave a stale wake-up hint able to reacquire the change.
- The reconciliation-level `terminal_error_retry_required` pre-gate was removed rather than kept as
  dead code: `queued_change_ids()` already excludes every terminal state, so that branch was
  unreachable once worktree discovery stopped feeding the list. The terminal-error stop gate remains
  enforced where it is still reachable, in dispatch selection
  (`parallel::tests::executor::test_terminal_error_change_is_not_selected_until_explicit_retry`),
  and the retry-only behaviour is proven end to end by
  `test_archived_dirty_reconciliation_keeps_terminal_error_stopped_until_retry`.
- No new membership store, allowlist, or durable state was introduced; every intent source is the
  existing process-local reducer state, discarded on restart.
- evidence: `cargo test --lib` passes (2814 tests) including the 11 new boundary tests:
  `unselected_archived_dirty_worktree_never_reaches_analysis_execution_or_git`,
  `explicit_start_target_recovers_its_archived_dirty_workspace`,
  `cli_explicit_targets_enter_the_initial_candidate_contract_without_unrelated_worktrees`,
  `queue_revocation_blocks_worktree_and_dynamic_reacquisition_until_explicit_requeue`,
  `empty_ordinary_queue_still_exposes_resolve_and_reject_lane_intent`,
  `tui_and_remote_start_produce_identical_explicit_target_eligibility`,
  `queue_removal_and_dequeue_revoke_eligibility_until_explicit_requeue`,
  `changes_refreshed_registers_without_creating_queue_or_lane_eligibility`,
  `removal_and_dequeue_revoke_ordinary_eligibility_until_explicit_requeue`,
  `parallel_startup_queues_only_selected_targets_and_refresh_does_not_widen_it`,
  `tui_queue_commands_drive_the_scheduler_eligibility_boundary`.
- evidence: `cargo fmt -- --check` and `cargo clippy --all-targets -- -D warnings` pass.
- Acceptance repair (`dynamic-queue-intent-fail-open`): the reducer is now the sole authority on
  ordinary queue intent, and every read of it fails closed. `admit_dynamic_queue_hint` validates each
  wake-up hint before the catalog is consulted, refusing when no reducer is wired, when `try_read` is
  contended, on a final terminal state, and when current intent does not admit the ID.
  `is_ordinary_queue_eligible` no longer treats a reducer-unknown ID as admissible, because every
  accepted path (start, `AddToQueue`, `RetryError`, `add_dynamic_change`) records reducer runtime
  state before a hint is published. `classify_queued_work` also fails closed: an unreadable snapshot
  used to look like "no change is waiting" and could classify a `MergeWait` or held candidate as
  dispatchable.
- Refusing on contention loses no intent: `reconcile_queued_candidates_from_shared_state` runs on the
  same scheduler pass and re-adds every reducer-queued ID from `queued_change_ids()`, so a refused
  hint costs at most one wake-up. This is asserted directly in
  `dynamic_hint_and_classification_fail_closed_under_reducer_lock_contention`.
- Related file outside the finding's declared set: `src/parallel/tests/manual_resolve.rs`. Its three
  dynamic-queue ingestion tests drove the queue with no reducer at all, which the fail-closed gate in
  the declared `src/parallel/queue_state.rs` now rejects. They were updated to record `AddToQueue`
  intent before pushing the hint, which is the production order in
  `OperatorCommandService::add_to_queue`; the shared helper is `shared_state_with_queue_intent`.
  `test_idle_queue_addition_marks_reanalysis_and_enqueues_change` in the declared
  `src/parallel/tests/executor.rs` was updated the same way.
- evidence: `cargo test --lib` passes (2816 tests) after the repair, including the two new gate
  regressions; `cargo fmt -- --check` and `cargo clippy --all-targets -- -D warnings` stay clean.

## Future Work

- A separate operator workflow may expose interrupted unrequested worktrees as attention items, but observability must not grant execution intent.
- Canonical duplicate requirement cleanup outside the exact promoted `Queue ingestion and analysis targeting` result may be handled as repository specification hygiene if archive promotion does not already collapse identical headings.

## Current Acceptance Follow-up
- attempt: 1
- [x] [dynamic-queue-intent-fail-open] (major) Dynamic queue hints can bypass reducer-owned execution intent | evidence: src/parallel/queue_state.rs:1857-1895 skips eligibility validation when shared state try_read() is contended, then admits the dynamic ID from the catalog; src/orchestration/state.rs:1406-1413 treats IDs absent from reducer state as ordinarily eligible despite dynamic notifications being wake-up hints only; src/parallel/queue_state.rs:2296-2309 and 2322-2389 do not recheck reducer queue intent before classifying an already-added candidate as dispatchable | required_changes: src/parallel/queue_state.rs — Fail closed when reducer state cannot be read and require current reducer-owned ordinary eligibility before ingesting every dynamic queue hint; src/orchestration/state.rs — Do not treat reducer-unknown IDs as ordinary queue eligible; src/parallel/tests/executor.rs — Add regressions proving unknown hints and hints observed during reducer lock contention cannot enter analysis or dispatch | verification: src/parallel/tests/executor.rs — Hold or contend the shared reducer write lock while ingesting a revoked dynamic hint, and separately enqueue an ID absent from reducer state; assert neither changes the queued candidates or reaches analysis/dispatch
  finding: {"evidence":["src/parallel/queue_state.rs:1857-1895 skips eligibility validation when shared state try_read() is contended, then admits the dynamic ID from the catalog","src/orchestration/state.rs:1406-1413 treats IDs absent from reducer state as ordinarily eligible despite dynamic notifications being wake-up hints only","src/parallel/queue_state.rs:2296-2309 and 2322-2389 do not recheck reducer queue intent before classifying an already-added candidate as dispatchable"],"id":"dynamic-queue-intent-fail-open","required_changes":[{"description":"Fail closed when reducer state cannot be read and require current reducer-owned ordinary eligibility before ingesting every dynamic queue hint","file":"src/parallel/queue_state.rs"},{"description":"Do not treat reducer-unknown IDs as ordinary queue eligible","file":"src/orchestration/state.rs"},{"description":"Add regressions proving unknown hints and hints observed during reducer lock contention cannot enter analysis or dispatch","file":"src/parallel/tests/executor.rs"}],"severity":"major","summary":"Dynamic queue hints can bypass reducer-owned execution intent","verification":[{"description":"Hold or contend the shared reducer write lock while ingesting a revoked dynamic hint, and separately enqueue an ID absent from reducer state; assert neither changes the queued candidates or reaches analysis/dispatch","file":"src/parallel/tests/executor.rs"}]}
  evidence: src/parallel/queue_state.rs:1901-1958 `admit_dynamic_queue_hint` refuses every hint when the reducer is absent (`reducer_state_absent`) or `try_read` is contended (`reducer_state_unreadable`), before any catalog lookup
  evidence: src/parallel/queue_state.rs:2028-2041 ingestion now runs that single validated admission gate for every hint, so no path reaches `list_changes_native_from` without current reducer-owned ordinary eligibility
  evidence: src/parallel/queue_state.rs:2413-2450 `classify_queued_work` fails closed on an unreadable reducer snapshot and reports every candidate as `candidate_unavailable` instead of reading empty wait sets as dispatchable
  evidence: src/orchestration/state.rs:1396-1424 `is_ordinary_queue_eligible` returns false for a reducer-unknown ID, so catalog membership alone can no longer authorize ordinary work
  evidence: src/parallel/tests/executor.rs:12639-12760 `reducer_unknown_dynamic_hint_never_enters_analysis_or_dispatch` proves an ID absent from reducer state leaves queued candidates unchanged, is refused for missing intent rather than a catalog miss, and never reaches the recorded analyzer input or dispatch
  evidence: src/parallel/tests/executor.rs:12762-12898 `dynamic_hint_and_classification_fail_closed_under_reducer_lock_contention` holds the shared write lock while ingesting a revoked hint, asserts nothing is queued and classification is `CandidateUnavailable`/blocked-only, then proves real intent still returns through reconciliation once readable
  evidence: `cargo test --lib` passes (2816 tests), `cargo fmt -- --check` and `cargo clippy --all-targets -- -D warnings` clean
