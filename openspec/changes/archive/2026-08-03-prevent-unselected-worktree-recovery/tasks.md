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
- Acceptance repair 2 (`dynamic-queue-intent-fail-open`, attempt 3): membership in the
  scheduler-local candidate list was still treated as eligibility. `classify_queued_work` and
  `select_changes_for_dispatch` now both consult current reducer-owned ordinary intent through
  `DependencyContext::withholds_ordinary_queue_intent`, which is computed from the same snapshot as
  the other reducer-owned sets (`None` only when no reducer is wired; every candidate when the
  snapshot is unreadable, so contention withholds work). The gate runs *after* the acceptance-hold,
  `MergeWait`, resolve/reject lane, and terminal-error branches, so reducer-owned lane semantics and
  the dedicated stop gates keep deciding their own cases.
- `reconcile_queued_candidates_from_shared_state` now reconciles in both directions: a revoked
  candidate is dropped from the scheduler-local list (`QueueReconciliationOutcome::revoked_removed`),
  so it never reaches the analyzer and cannot hold an otherwise drained run open. In-flight,
  reducer-active, and merge/resolve/reject-wait candidates are never dropped, and an explicit
  `AddToQueue` re-adds the change from `queued_change_ids()` on the next pass.
- Related file outside the finding's declared set: `src/parallel/dependency.rs` holds the shared
  predicate used by both declared call sites in `src/parallel/queue_state.rs`; it already snapshots
  the reducer once per pass, so no extra lock acquisition was added.
- Related file outside the finding's declared set: `src/parallel_run_service.rs`. The gate makes the
  reducer authoritative, so an explicit CLI target list must record the same intent TUI/remote Start
  records: `run_parallel` now applies `AddToQueue` next to `add_dynamic_change`, which is the CLI
  half of the explicit-intent contract in acceptance criterion 7. Without it, registration alone
  leaves `QueueIntent::NotQueued` and a CLI run would be withheld by its own boundary.
- Three pre-existing executor fixtures built shared reducer state without the queue intent
  production records at start (`resolving_dependency_blocks_its_dependent_but_not_unrelated_dispatch`,
  `resolving_dependency_diagnostic_dedupes_and_reemits_after_signature_change`,
  `test_blocked_only_classifier_distinguishes_scheduler_work_classes`); they now apply `AddToQueue`
  for their ordinary candidates, matching `initialize_parallel_shared_state`.
- The spec delta for `Queue ingestion and analysis targeting` states the boundary explicitly and adds
  the `Revocation stops an already admitted candidate` scenario.
- evidence: after acceptance repair 2, `cargo test --lib` passes (2817 tests, 0 failed, 8 ignored),
  including `parallel::tests::executor::revoked_queue_intent_stops_an_already_added_candidate_before_analysis_and_dispatch`;
  `cargo fmt -- --check` and `cargo clippy --all-targets -- -D warnings` are clean.

## Future Work

- A separate operator workflow may expose interrupted unrequested worktrees as attention items, but observability must not grant execution intent.
- Canonical duplicate requirement cleanup outside the exact promoted `Queue ingestion and analysis targeting` result may be handled as repository specification hygiene if archive promotion does not already collapse identical headings.
