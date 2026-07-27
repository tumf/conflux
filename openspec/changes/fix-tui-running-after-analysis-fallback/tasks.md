## Implementation Tasks

- [ ] Wire the successful dependency-analysis fallback warning from `fix-recoverable-analysis-fallback-event` through the TUI as a non-fatal diagnostic. Completion: receiving the diagnostic while orchestration is active adds a warning log without calling the global fatal `handle_error` path or changing reducer state. (verification: unit - targeted event dispatch test under `src/tui/state/event_handlers/`)
- [ ] Preserve active TUI context across the recoverable fallback. Completion: `AppMode::Running`, `current_change`, active row status, queue marks, orchestration start/elapsed state, and shared reducer snapshot remain unchanged except for the appended warning. (verification: integration - `cargo test analysis_fallback_running_state` with a fixture containing queued and active changes)
- [ ] Verify status/header rendering after fallback. Completion: rendered status title still contains running controls such as `Esc: stop`, excludes error-mode `retry` controls, and retains elapsed orchestration display when available. (verification: unit - targeted rendering test in `src/tui/render.rs`)
- [ ] Verify event-loop continuity after fallback. Completion: after the warning, a later processing, acceptance, archive, refresh, stop, or completion fixture updates the TUI through the normal handler and does not require an explicit retry to recover from display-only error mode. (verification: integration - event-sequence test under `src/tui/state/event_handlers/` or `src/tui/runner.rs`)
- [ ] Preserve fatal global error behavior. Completion: existing and targeted tests show a genuine `OrchestratorEvent::Error` still selects `AppMode::Error`, clears only the state defined by the fatal contract, and renders retry controls. (verification: unit - `cargo test handle_error` and targeted fatal-versus-fallback classification assertions)
- [ ] Run repository quality gates after focused regressions pass. Completion: formatting, lint, and default non-heavy tests succeed without unrelated source changes. (verification: integration - `cargo fmt --check`; `cargo clippy -- -D warnings`; `cargo test`)

## Future Work

- A broader typed severity model for every orchestration event should be proposed separately if other warning/fatal ambiguities are discovered.

## Final Validation

Expected archive gate: `cflx openspec validate fix-tui-running-after-analysis-fallback --archive-gate`
