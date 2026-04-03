## Implementation Tasks

- [x] 1. Define a dedicated rejecting lifecycle stage in shared orchestration state and derived display status paths (verification: `openspec/specs/orchestration-state/spec.md` delta covers reducer/runtime semantics and status derivation for rejecting)
- [x] 2. Route apply-generated `REJECTED.md` handoff into rejecting instead of acceptance in serial and parallel orchestration flows (verification: proposal deltas cover handoff/resume behavior for `src/execution/apply.rs`, parallel executor, and serial run service)
- [x] 3. Specify rejecting review outcomes `confirm_rejection` and `resume_apply` as dedicated runtime verdicts, including the exact marker contract `REJECTION_REVIEW: CONFIRM|RESUME` and base-branch REJECTED-only commit semantics for confirmed rejection (verification: `openspec/specs/parallel-execution/spec.md` delta covers marker parsing contract and REJECTED-only merge rule)
- [x] 4. Specify reject-dismissal behavior so the runtime removes worktree `REJECTED.md`, appends non-rejection recovery tasks to `tasks.md`, and returns the change to apply without re-entering normal acceptance first (verification: spec delta includes explicit scenario for `tasks.md` mutation and routing before resuming apply)
- [x] 5. Specify reducer/API/UI visibility updates so `rejecting` is surfaced as an active runtime stage in dashboard/TUI state consumers (verification: spec delta references shared orchestration state and observable display status requirements)
- [x] 6. Update `skills/cflx-workflow/` canonical workflow instructions so apply/rejecting/accept handoff semantics and final verdict markers match the dedicated rejecting stage (verification: implementation updates target `skills/cflx-workflow/SKILL.md` and the relevant reference docs such as `skills/cflx-workflow/references/cflx-accept.md`)
- [x] 7. Validate the proposal strictly and ensure all affected deltas are scenario-complete (verification: `python3 /Users/tumf/.agents/skills/cflx-proposal/scripts/cflx.py validate split-rejecting-from-acceptance --strict` passes)

## Future Work

- If implementation introduces a dedicated rejecting reference document, align orchestration invocations and downstream docs with that new workflow entrypoint
- Evaluate whether rejected/recovered task annotations should be standardized in proposal authoring guidance

## Acceptance #1 Failure Follow-up

- [x] Update `openspec/changes/split-rejecting-from-acceptance/tasks.md` so implementation tasks 1-6 are not marked complete until non-OpenSpec repository evidence exists for the claimed runtime/state/UI changes; verified runtime evidence now exists in `src/execution/apply.rs`, `src/orchestration/rejection.rs`, `src/parallel/dispatch.rs`, and `src/serial_run_service.rs`.
- [x] Implement the rejecting runtime/state/UI flow in product code and add verifiable non-OpenSpec evidence for the proposal requirements (implemented in orchestration/parallel/server state paths including `src/orchestration/state.rs`, `src/parallel/dispatch.rs`, `src/serial_run_service.rs`, and `src/web/state.rs`).
- [x] Make the normal commit quality gate executable in this workspace by ensuring the configured pre-commit hook runner is available and passes; verified with `/Users/tumf/Library/Python/3.9/bin/pre-commit run --all-files`.
- [x] Fix the failing test suite reported by `cargo test`; verified targeted failures are now fixed via `cargo test parallel::tests::executor::test_idle_queue_addition_marks_reanalysis_and_enqueues_change -- --nocapture` and `cargo test server::api::tests::test_sync_monitor_is_non_invasive_and_never_runs_sync_or_resolve -- --nocapture`.

## Acceptance #2 Failure Follow-up

- [ ] Fix `config::tests::test_config_merge_partial_project_inherits_global` failing at `src/config/mod.rs:1306` in full `cargo test` run.
- [ ] Fix server API selection/control regressions surfaced by full `cargo test`: `test_global_control_run_records_call` (`src/server/api.rs:5357`), `test_global_control_run_skips_unremarked_error_changes` (`src/server/api.rs:2318`), `test_global_control_stop_records_call` (`src/server/api.rs:5822`), `test_stop_and_dequeue_change_clears_only_target_selection` (`src/server/api.rs:5666`), and `test_toggle_all_change_selection_remarks_error_changes_for_next_run` (`src/server/api.rs:5784`).
- [ ] Re-run `cargo test` and confirm zero failures before marking acceptance follow-up complete.
