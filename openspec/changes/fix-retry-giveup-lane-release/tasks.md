# Tasks: Fix Spawned Retry Give-Up Paths Leaving the Base-Mutating Lane Occupied

## Implementation Tasks

- [ ] Task 1: Add `abandon_base_mutating_lane_occupant(change_id)` to
      `src/orchestration/state.rs`: for a non-terminal entry whose `activity` is
      `Resolving`/`Rejecting`, set `activity = Idle`, keep `wait_state = None` (no
      restore), clear blocked metadata, and remove the change from both
      `resolve_wait_queue` and `reject_wait_queue`; return whether a release happened.
      No-op (return false) for terminal entries and non-occupants. Completion
      condition: method exists with reducer unit tests asserting activity reset,
      `wait_state` remains `None`, no membership in either wait queue afterwards,
      no-op on terminal entries and non-occupants, and `global_invariants_hold()`
      after abandon (covering both a promoted ResolveWait occupant and a promoted
      RejectWait occupant). verification: unit - reducer tests in
      `src/orchestration/state.rs`

- [ ] Task 2: Wire the abandon call into all four give-up sites in
      `src/parallel/queue_state.rs`, synchronously adjacent to the intent clearing and
      before the `Ok(MergeTaskOutcome::Merged)` give-up return:
      (a) `retry_deferred_merges_for` already-merged-to-base skip (~line 1137),
      (b) `retry_deferred_merges_for` workspace gone `Ok(None)` (~line 1162),
      (c) `retry_deferred_merges_for` stale workspace path (~line 1192),
      (d) `retry_deferred_rejection_review_for` workspace gone `Ok(None)` (~line 1374).
      Use `shared_orchestrator_state` write access; preserve the existing
      operator-visible `ParallelEvent::Error` emissions and intent-clearing behavior.
      Completion condition: no give-up path returns with the promoted occupant still
      `Resolving`/`Rejecting` in shared state; all four sites show the abandon call in
      the diff. verification: unit - tests in Task 3 and Task 4 fail when any site's
      abandon call is removed

- [ ] Task 3: Strengthen the two workspace-lookup operator-visibility tests in
      `src/parallel/tests/executor.rs`
      (`resolve_retry_workspace_lookup_failure_is_operator_visible`,
      `reject_retry_workspace_lookup_failure_is_operator_visible`): set up a shared
      `OrchestratorState`, promote the change into the lane via
      `promote_next_base_mutating_lane_waiter()` so `is_base_mutating_lane_occupied()`
      is true, run the retry body, then additionally assert
      `is_base_mutating_lane_occupied()` is false, the change appears in neither
      `resolve_wait_change_ids()` nor `reject_wait_change_ids()`, and
      `global_invariants_hold()`. Completion condition: both tests fail when Task 2's
      wiring is reverted and pass after; each runs under 1 second.
      verification: integration - `cargo test --lib parallel::tests::executor`
      (red before fix, green after)

- [ ] Task 4: Add a give-up next-waiter convergence test: queue two changes in
      ResolveWait in shared state, promote the first, drive its spawned-retry give-up
      (missing workspace via `TestWorkspaceManager`), deliver the resulting
      `Ok(Merged)` result through `handle_merge_result_with_tx`, and assert the second
      waiter is promoted (lane occupied by the second change / a retry task result for
      it arrives) without user action, while the first change is not re-enqueued.
      Completion condition: test exists, fails when Task 2 is reverted, runs under 1
      second. verification: integration - test in `src/parallel/tests/executor.rs`

- [ ] Task 5: Run quality gates on the final tree (verification: integration - `cargo test --lib parallel::tests`
      plus `cargo test --lib orchestration::state`,
      `cargo fmt --check`, and
      `cargo clippy --locked --all-targets --all-features -- -D warnings`; all exit 0).
      Completion condition: all listed commands pass; no existing resolve-wait,
      reject-wait, lane-release, drain, persistent-idle, or terminal-gate test
      regresses (in particular the prior change's
      `release_base_mutating_lane_after_retry_*`,
      `retry_lane_busy_release_allows_subsequent_repromotion`, and
      `deferred_retry_repromotes_and_converges_to_merged_without_user_action` stay
      green).

## Future Work

- Operator-visible reporting for the `attempt_merge` Err path of retry origins
  (currently log-only after the duplicate-suppression change) — observability
  follow-up proposal (verification: manual - tracked as a future change proposal).
- Manual TUI confirmation on a real repository: delete a workspace while its change
  sits in ResolveWait, trigger a merge-completion, and observe in
  `~/.local/state/cflx/logs` that the give-up emits one Error event and subsequent
  waiters still get promoted (verification: manual - requires an interactive TUI
  session; reviewers evaluate via cflx log entries).

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-retry-giveup-lane-release --archive-gate`
