## Implementation Tasks

- [x] Remove fallback-marker inspection from the global TUI error path so every global error event executes the existing fatal transition in `src/tui/state/event_handlers/output.rs` (verification: unit - add a test in `src/tui/state/event_handlers/output.rs` and run `cargo test fatal_global_error` to prove an error containing `RECOVERABLE_ANALYSIS_FALLBACK_MARKER` enters `AppMode::Error`, clears current change, and records an error log).
- [x] Keep successful fallback handling on the producer's warning-log event path and remove only classifier-specific marker plumbing that has no remaining observability use (verification: unit - run `cargo test analysis_fallback_warning_log_event_keeps_running_state` against the producer-built message in `src/tui/state/event_handlers/output.rs` to prove `OrchestratorEvent::Log(LogEntry::warn(...))` preserves Running state and warning severity).
- [x] Add rendering regression coverage for a fatal error that quotes fallback marker text (verification: unit - add the case in `src/tui/render.rs` and run `cargo test fatal_global_error_status_header` to prove retry controls remain visible and `Esc: stop` does not, while `cargo test analysis_fallback_running_status_header` retains running controls and elapsed time).
- [x] Run focused and repository quality gates (verification: integration - `cargo test tui::state::event_handlers::output`, `cargo test tui::render`, `cargo fmt --check`, and `make lint` all pass).

## Final Validation

Archive validation is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate remove-tui-fallback-message-classifier --archive-gate`
