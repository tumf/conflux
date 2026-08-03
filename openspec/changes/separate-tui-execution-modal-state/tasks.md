## Implementation Tasks

- [x] Replace mixed `AppMode` with `AppExecutionMode` and payload-bearing `ModalState`, migrate `AppState` initialization/accessors, and remove `previous_mode` plus separately mutable worktree-confirmation payload fields. Completion requires exhaustive typed matches and tests proving default state and payload/modal invariants. (verification: unit - `cargo test --lib tui::state::` and `cargo test --lib tui::types::`; verification-id: tui-state-tests)

- [x] Implement the variant-specific modal validity matrix: QR invalidates when its URL disappears, worktree-delete invalidates on fresh identity/eligibility changes, and force-kill survives Running-to-Stopping only while its target remains retryable active work and clears for terminal, dequeued, absent, non-active, non-retryable, or global-invalid states. Completion requires atomic modal/payload clearing and table-driven survival/invalidation tests. (verification: unit - `cargo test --lib tui::state::` and `cargo test --lib tui::state::event_handlers::`; verification-id: tui-state-tests)

- [x] Rewire QR, worktree-delete, and force-kill open, cancel, close, and confirm paths to mutate only modal state and dispatch destructive confirmation through fresh repository/shared-service revalidation. Completion requires stale worktree identity and invalid force-kill targets to refuse or no-op without mutating authoritative state, including failed cancellation and timeout paths. (verification: integration - `cargo test --lib tui::key_handlers::` and `cargo test --lib tui::command_handlers::`; verification-id: tui-state-tests)

- [x] Update key and command routing so warning-popup input is consumed first, modal input second, and ordinary view input last; derive bulk-mark admission from Changes view, overlay absence, and the shared `OperatorMode` lifecycle matrix with distinct Error and Stopping feedback. Completion requires tests proving overlay keys cannot mutate cursor, marks, queue intent, stop state, or retry state across the full execution-mode matrix. (verification: integration - `cargo test --lib tui::key_handlers::`, `cargo test --lib tui::state::selection_logic::`, and `cargo test --lib tui::command_handlers::`; verification-id: tui-state-tests)

- [x] Migrate execution event handlers without narrowing their legitimate ownership of row state, timers, `current_change`, and `StopMode`; preserve current change-local versus fatal-global classification and AllCompleted/Stopped terminal retention while applying modal changes only through the explicit invalidation policy. Completion requires ProcessingError, fatal Error, Stopping, Stopped, AllCompleted, valid-modal survival, and invalid-modal clearing coverage. (verification: unit - `cargo test --lib tui::state::event_handlers::`; verification-id: tui-state-tests)

- [x] Update TUI rendering to derive the base presentation from execution and view state and render supported overlays independently, preserving existing titles, key hints, layouts, and warning-popup behavior. Completion requires QR and worktree overlays above every valid execution mode, force-kill above valid Running/Stopping states, and base Error/terminal rendering after invalidation without fallback rewrites. (verification: unit - `cargo test --lib tui::render::`; verification-id: tui-state-tests)

- [x] Update typed lifecycle snapshots and projection to report valid confirmations as blocked, QR as the underlying execution lifecycle, and execution-only canonical mode tokens without changing adapter ownership or Web operator semantics. Completion requires all execution modes crossed with no modal and QR, valid confirmation combinations, and TUI/Web `app_mode` compatibility tests. (verification: integration - `cargo test --lib tui::lifecycle::` and `cargo test --features web-monitoring --lib web::remote_control_api::tests::operator_snapshot_tests`; verification-id: tui-state-tests)

- [x] Run the complete repository-local gate and resolve compiler, test, lint, and formatting failures introduced by the migration without broadening behavior beyond this proposal. Completion requires every command to exit successfully. (verification: integration - `cargo test --lib tui:: && cargo test --features web-monitoring --lib web::remote_control_api::tests::operator_snapshot_tests && cargo fmt --check && cargo clippy -- -D warnings`; verification-id: tui-state-tests)

## Notes

- evidence: `cargo test --lib tui::` — 517 passed, 0 failed, 1 ignored.
- evidence: `cargo test --features web-monitoring --lib web::remote_control_api::tests::operator_snapshot_tests` — 20 passed, 0 failed.
- evidence: `cargo fmt --check` — clean; `cargo clippy -- -D warnings` — clean.
- evidence: full default suite `cargo test --lib` — 2909 passed, 0 failed, 9 ignored.
- note: `operator_command_force_kill_result_is_not_treated_as_termination_proof` was tightened from the bare substring `force_kill` to the call form `.force_kill(`. The guard's subject is the best-effort `ProcessManager::force_kill` API, which is only ever invoked as a method call; the operator-facing feature keeps its force-kill name, so identifiers such as `confirm_force_kill` describe the confirmation rather than that API.
- note: force-kill keeps its existing in-list presentation (`Y: confirm kill` / `N: cancel` hints plus the `Confirm Kill` header label) driven from `ModalState`. Adding a dedicated popup widget would change modal visual design, which this proposal lists as out of scope.

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate separate-tui-execution-modal-state --archive-gate`.

## Future Work

- Reclassify specific background merge failures as recoverable warnings only through a separate proposal with event-ownership and scheduler-safety evidence.
- Expose modal presentation state through remote monitoring only if a future frontend requirement needs it; execution `app_mode` remains unchanged here.
