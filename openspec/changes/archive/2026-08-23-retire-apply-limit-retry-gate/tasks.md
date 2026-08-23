## Implementation Tasks

- [x] Remove the settled terminal-error retry block from shared operator-command admission while preserving the requirement for explicit retry intent (verification: unit - cargo test operator_command --lib; verification-id: explicit-retry-tests)
- [x] Align Start, individual retry, and bulk retry so a retained Apply-limit diagnostic does not prevent a fresh explicit execution boundary (verification: unit - cargo test operator_command --lib; verification-id: explicit-retry-tests)
- [x] Align TUI and `/api/v2` action eligibility with shared command admission and keep iteration-limit evidence observational (verification: unit - cargo test operator_command --lib; verification-id: explicit-retry-tests)
- [x] Align bulk execution-mark classification, the explicit per-target terminal-error queue-intent alias, and TUI error/footer guidance with the retired gate so a settled limited row classifies exactly as an ordinary terminal-error row (verification: unit - cargo test operator_command --lib; verification-id: explicit-retry-tests)
- [x] Add regressions proving no automatic redispatch occurs and one explicit retry receives fresh Apply budget while a persistent scheduler remains alive (verification: unit - cargo test operator_command --lib; verification-id: explicit-retry-tests)

## Future Work

The separate `/Users/tumf/bin/claude-auto` empty-account-array failure should be fixed in its owning local wrapper. This change only restores Conflux recovery after any Apply-limit failure.

A follow-up change must retire or generalize the two `web-monitoring` console scenarios conditioned on an `apply_iteration_limit_active` blocked snapshot (`Server-blocked error row offers no Retry`, `Later allowed snapshot restores Retry`); after this change that projection is unreachable and the scenarios are vacuous. The same follow-up owns the illustrative comment in `web/app.js` and the `tests/web/destructive-actions.spec.js` fixture that still name the retired token.

## Notes

- evidence (tasks 1-5): `cargo test operator_command --lib` -> `ok. 102 passed; 0 failed`, selecting the new `orchestration::operator_command::tests::settled_apply_limit_*` regressions.
- evidence (task 1): `src/orchestration/operator_command.rs` drops `ActiveApplyIterationLimit`, `active_apply_iteration_limit(_ids)`, `APPLY_ITERATION_LIMIT_ACTIVE`, `OperatorCommandError::ApplyIterationLimitActive`, and the `run_boundary` binding; `plan_retry_change` now classifies from the target's own terminal-error evidence only. `RunBoundaryLiveness` remains as observability for `scheduler_running`.
- evidence (task 2): `src/orchestration/run_control/tests.rs` adds `settled_apply_limit_retry_is_admitted_while_the_scheduler_task_is_live`, `..._after_task_exit_starts_a_later_boundary`, `..._later_state_starts_with_a_fresh_budget`, `..._bulk_retry_dispatches_every_admitted_target`, `..._bulk_retry_keeps_unsupported_evidence_intact`, and `..._error_mode_start_retries_the_marked_row`.
- evidence (task 3): `src/web/remote_control_api/projection.rs` no longer derives the blocked reason; `ActionBlockedReason::ApplyIterationLimitActive` is retained in `src/web/remote_control_api/dto.rs` as a published-but-unproduced token for older clients, pinned by `settled_apply_limit_retired_token_remains_deserializable` and `tests/openapi_contract_tests.rs::retired_iteration_limit_blocked_reason_token_remains_published`.
- evidence (task 4): `MarkExclusion::ApplyIterationLimitActive` is removed (`MarkExclusion::ALL` is now 8), the terminal-error `set_queue_intent=true` alias routes through `RetryError` unconditionally, and `src/tui/render.rs` replaces the active-limit hint/footer/header cases with `settled_iteration_limit_tui_*` tests asserting ordinary retry guidance while attempts/ceiling stay visible.
- evidence (task 5): `src/parallel/tests/change_error_f5_retry.rs` adds `change_error_f5_retry_iteration_limit_receives_fresh_budget_on_explicit_retry` and `..._iteration_limit_budget_survives_ordinary_notification` over a live scheduler; `OrchestratorState::retry_terminal_error` is the sole consumer of `clear_apply_iteration_limit`, reachable only from `ReducerCommand::RetryError`.
- evidence (repository-wide): `cargo clippy --all-targets --all-features` clean, `cargo fmt --check` clean, `cargo test --test openapi_contract_tests` -> `19 passed; 0 failed`.
- Pre-existing unrelated failure: `cargo test --lib` reports `4117 passed; 1 failed`, the single failure being `openspec_cmd::promotion::tests::every_pending_change_promotes_without_dropping_a_scenario` for the *other* pending change `refine-verification-gate-policy` (dropped scenarios `Heavy repository gate is not an Apply checkbox` and `Non-local verification cannot be hidden in task prose` under requirement `Proposal verification plans bound Apply-owned work`). This branch touches neither that change directory nor `src/openspec_cmd/promotion.rs`; it is owned by that change, not this one.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate retire-apply-limit-retry-gate --archive-gate`.

`cflx openspec validate retire-apply-limit-retry-gate --strict` -> Validation passed.
