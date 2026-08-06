## Implementation Tasks

- [ ] Introduce one dispatch-local task-completion grace eligibility value in `src/execution/apply.rs`, derived from the task progress read before the Apply child is launched. Completion requires `TasksComplete` to be eligible only for a dispatch that began incomplete, while `BlockedHandoff` and `RejectingHandoff` remain eligible for every dispatch and no durable state, repair-kind hierarchy, configuration, or command-runner change is added. (verification: unit - targeted completion-detection tests run by `cargo test --lib precomplete_apply_repair`; verification-id: apply-repair-completion-regressions)

- [ ] Use the same dispatch-local eligibility for periodic completion probes and grace-deadline stable rechecks, preserving cancellation and the existing process-group quiescence barrier before any repository mutation or handoff. Completion requires a disabled pre-existing `TasksComplete` condition to remain disabled for the entire child lifetime and a newly eligible handoff to retain the existing graceful-then-forceful cleanup path. (verification: integration - `cargo test --lib precomplete_apply_repair && cargo test --lib test_execute_apply_loop_terminates_lingering_child_after_tasks_complete && cargo test --lib test_execute_apply_loop_keeps_child_running_when_tasks_become_incomplete_during_grace`; verification-id: apply-repair-completion-regressions)

- [ ] Add a stage-repair regression in `src/execution/apply.rs` whose task-complete workspace fails the loop-entry stage gate and whose repair command intentionally waits longer than a shortened completion grace before staging the affected file. Completion requires exactly one repair dispatch, natural repair completion, a clean verified final commit, no Acceptance handoff before finalization, and a default-suite runtime under one second. (verification: integration - `cargo test --lib precomplete_apply_repair_stage`; verification-id: apply-repair-completion-regressions)

- [ ] Add a task-format-repair regression whose completed but malformed `tasks.md` remains unchanged past a shortened completion grace before the repair command writes and stages valid content. Completion requires the command to survive beyond grace, task-format validation to pass, task completion evidence to remain complete, Apply to finish without an extra repair dispatch, and the test to run under one second by default. (verification: integration - `cargo test --lib precomplete_apply_repair_task_format`; verification-id: apply-repair-completion-regressions)

- [ ] Add a final-commit-hook-repair regression whose hook rejection leaves pending repair while tasks are complete and whose repair command waits beyond shortened grace before removing the repository blocker. Completion requires exactly one repair dispatch, a second hook-enabled finalization attempt, a verified `Apply: <change-id>` commit, no hook bypass, and a default-suite runtime under one second. (verification: integration - `cargo test --lib precomplete_apply_repair_commit_hook`; verification-id: apply-repair-completion-regressions)

- [ ] Add task-complete repair regressions for `APPLY_BLOCKED` and `REJECTED.md`, with each artifact created during the active repair dispatch before a lingering child exceeds shortened grace. Completion requires bounded grace termination, confirmed process-group cleanup, the correct distinct handoff result, no success classification from pre-existing task completion, and no additional Apply dispatch. (verification: integration - `cargo test --lib precomplete_apply_repair_handoff`; verification-id: apply-repair-completion-regressions)

- [ ] Preserve normal Apply failure semantics by covering a task-complete repair command that exits non-zero without repairing or producing a handoff. Completion requires the exit to remain an ordinary failed attempt subject to existing retry, stall, and iteration-budget policy rather than becoming success-equivalent because tasks were complete at dispatch start. (verification: integration - `cargo test --lib precomplete_apply_repair_failure`; verification-id: apply-repair-completion-regressions)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate fix-precomplete-apply-repair-termination --archive-gate`.

## Future Work

- Evaluate a separate hard wall-clock command budget only if operators require a bound that cannot be extended by continued output activity.
- Consolidate duplicated canonical cleanup-review requirement headings in a separate spec-hygiene change after defining unambiguous promotion behavior for duplicate requirement identities.
