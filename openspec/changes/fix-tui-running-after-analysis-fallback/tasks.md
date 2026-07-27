## Implementation Tasks

- [x] Wire the successful dependency-analysis fallback warning from `fix-recoverable-analysis-fallback-event` through the TUI as a non-fatal diagnostic. Completion: receiving the diagnostic while orchestration is active adds a warning log without calling the global fatal `handle_error` path or changing reducer state. (verification: unit - targeted event dispatch test under `src/tui/state/event_handlers/`)
- [x] Preserve active TUI context across the recoverable fallback. Completion: `AppMode::Running`, `current_change`, active row status, queue marks, orchestration start/elapsed state, and shared reducer snapshot remain unchanged except for the appended warning. (verification: integration - `cargo test analysis_fallback_running_state` with a fixture containing queued and active changes)
- [x] Verify status/header rendering after fallback. Completion: rendered status title still contains running controls such as `Esc: stop`, excludes error-mode `retry` controls, and retains elapsed orchestration display when available. (verification: unit - targeted rendering test in `src/tui/render.rs`)
- [x] Verify event-loop continuity after fallback. Completion: after the warning, a later processing, acceptance, archive, refresh, stop, or completion fixture updates the TUI through the normal handler and does not require an explicit retry to recover from display-only error mode. (verification: integration - event-sequence test under `src/tui/state/event_handlers/` or `src/tui/runner.rs`)
- [x] Preserve fatal global error behavior. Completion: existing and targeted tests show a genuine `OrchestratorEvent::Error` still selects `AppMode::Error`, clears only the state defined by the fatal contract, and renders retry controls. (verification: unit - `cargo test handle_error` and targeted fatal-versus-fallback classification assertions)
- [x] Run repository quality gates after focused regressions pass. Completion: formatting, lint, and default non-heavy tests succeed without unrelated source changes. (verification: integration - `cargo fmt --check`; `cargo clippy -- -D warnings`; `cargo test`)

## Notes

Implementation evidence:

- Non-fatal classification: `src/events.rs` (`RECOVERABLE_ANALYSIS_FALLBACK_MARKER`), `src/tui/state/event_handlers/output.rs` (`is_recoverable_analysis_fallback`, `AppState::handle_error` early return).
- Producer/consumer wording kept in sync: `src/parallel_run_service.rs::recoverable_analysis_fallback_diagnostic` now formats from the shared marker constant.
- Unit evidence (`src/tui/state/event_handlers/output.rs`): `recoverable_analysis_fallback_classifier_matches_producer_message_only`, `analysis_fallback_running_state_is_not_fatal`, `analysis_fallback_warning_log_event_keeps_running_state`, `analysis_fallback_running_state_keeps_processing_later_events`, `genuine_global_error_still_enters_fatal_error_mode`.
- Unit evidence (`src/tui/render.rs`): `analysis_fallback_running_status_header_keeps_running_controls`, `fatal_global_error_status_header_still_shows_retry_controls`.
- Integration evidence (real filesystem task-progress lookup): `src/tui/state/event_handlers/output.rs::analysis_fallback_running_state_keeps_archive_handling_working`.
- Regression proof: removing the `handle_error` non-fatal branch fails 4 of the new tests (`analysis_fallback_running_state_is_not_fatal`, `analysis_fallback_running_state_keeps_processing_later_events`, `analysis_fallback_running_state_keeps_archive_handling_working`, `analysis_fallback_running_status_header_keeps_running_controls`).

## Future Work

- A broader typed severity model for every orchestration event should be proposed separately if other warning/fatal ambiguities are discovered.

## Final Validation

Expected archive gate: `cflx openspec validate fix-tui-running-after-analysis-fallback --archive-gate`
