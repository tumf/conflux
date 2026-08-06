## Implementation Tasks

- [x] Introduce one dispatch-local task-completion grace eligibility value in `src/execution/apply.rs`, derived from the task progress read before the Apply child is launched. Completion requires `TasksComplete` to be eligible only for a dispatch that began incomplete, while `BlockedHandoff` and `RejectingHandoff` remain eligible for every dispatch and no durable state, repair-kind hierarchy, configuration, or command-runner change is added. (verification: unit - targeted completion-detection tests run by `cargo test --lib precomplete_apply_repair_eligibility`; verification-id: apply-repair-completion-regressions)

- [x] Use the same dispatch-local eligibility for periodic completion probes and grace-deadline stable rechecks, preserving cancellation and the existing process-group quiescence barrier before any repository mutation or handoff. Completion requires a disabled pre-existing `TasksComplete` condition to remain disabled for the entire child lifetime and a newly eligible handoff to retain the existing graceful-then-forceful cleanup path. (verification: integration - `cargo test --lib precomplete_apply_repair && cargo test --lib test_execute_apply_loop_terminates_lingering_child_after_tasks_complete && cargo test --lib test_execute_apply_loop_keeps_child_running_when_tasks_become_incomplete_during_grace`; verification-id: apply-repair-completion-regressions)

- [x] Add a stage-repair regression in `src/execution/apply.rs` whose task-complete workspace fails the loop-entry stage gate and whose repair command intentionally waits longer than a shortened completion grace before staging the affected file. Completion requires exactly one repair dispatch, natural repair completion, a clean verified final commit, no Acceptance handoff before finalization, and a default-suite runtime under one second. (verification: integration - `cargo test --lib precomplete_apply_repair_stage`; verification-id: apply-repair-completion-regressions)

- [x] Add a task-format-repair regression whose completed but malformed `tasks.md` remains unchanged past a shortened completion grace before the repair command writes and stages valid content. Completion requires the command to survive beyond grace, task-format validation to pass, task completion evidence to remain complete, Apply to finish without an extra repair dispatch, and the test to run under one second by default. (verification: integration - `cargo test --lib precomplete_apply_repair_task_format`; verification-id: apply-repair-completion-regressions)

- [x] Add a final-commit-hook-repair regression whose hook rejection leaves pending repair while tasks are complete and whose repair command waits beyond shortened grace before removing the repository blocker. Completion requires exactly one repair dispatch, a second hook-enabled finalization attempt, a verified `Apply: <change-id>` commit, no hook bypass, and a default-suite runtime under one second. (verification: integration - `cargo test --lib precomplete_apply_repair_commit_hook`; verification-id: apply-repair-completion-regressions)

- [x] Add task-complete repair regressions for `APPLY_BLOCKED` and `REJECTED.md`, with each artifact created during the active repair dispatch before a lingering child exceeds shortened grace. Completion requires bounded grace termination, confirmed process-group cleanup, the correct distinct handoff result, no success classification from pre-existing task completion, and no additional Apply dispatch. (verification: integration - `cargo test --lib precomplete_apply_repair_handoff`; verification-id: apply-repair-completion-regressions)

- [x] Preserve normal Apply failure semantics by covering a task-complete repair command that exits non-zero without repairing or producing a handoff. Completion requires the exit to remain an ordinary failed attempt subject to existing retry, stall, and iteration-budget policy rather than becoming success-equivalent because tasks were complete at dispatch start. (verification: integration - `cargo test --lib precomplete_apply_repair_failure`; verification-id: apply-repair-completion-regressions)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate fix-precomplete-apply-repair-termination --archive-gate`.

## Notes

- Implementation: `DispatchCompletionPolicy::for_dispatch` derives `tasks_complete_eligible` from the `progress` observation `execute_apply_loop` already takes before launching the child. `resolve_apply_completion` holds the precedence rule (blocked > rejecting > task completion, the last only when eligible) as a pure function, and `detect_apply_completion` feeds it workspace evidence. Both the periodic probe and the grace-deadline recheck pass the same per-dispatch policy value. No persisted state, no new timeout, no `AiCommandRunner` or `command_queue` change.
- Regression coverage lives in `src/execution/apply.rs`, module `apply_commit_recovery::precomplete_repair_completion`, so it reuses the existing hook-enabled `RecoveryRepo` and the shortened 50ms grace / 20ms probe wiring of `run_recovery_loop`. Each repair command waits `0.2s` — roughly three times the ~70ms point at which the pre-fix watchdog terminated a task-complete repair.
- evidence: `cargo test --lib precomplete_apply_repair` — 10 passed (4 eligibility unit tests, 6 integration regressions)
- evidence: measured per-test default-suite runtimes: stage 0.90s, task-format 0.74s, commit-hook 0.88s, blocked handoff 0.26s, rejected handoff 0.24s, failure 0.67s (measured at a 0.3s repair delay; the committed delay is 0.2s, so each is ~0.1s faster)
- evidence: `cargo test --lib test_execute_apply_loop_terminates_lingering_child_after_tasks_complete` and `cargo test --lib test_execute_apply_loop_keeps_child_running_when_tasks_become_incomplete_during_grace` still pass, so the original incomplete-to-complete watchdog and its transient-completion recheck are unchanged
- evidence: `cargo fmt --check` and `cargo clippy --locked --all-targets --all-features -- -D warnings` pass
- evidence: `cflx openspec validate fix-precomplete-apply-repair-termination --strict` passes

## Future Work

- Evaluate a separate hard wall-clock command budget only if operators require a bound that cannot be extended by continued output activity.
- Consolidate duplicated canonical cleanup-review requirement headings in a separate spec-hygiene change after defining unambiguous promotion behavior for duplicate requirement identities.
