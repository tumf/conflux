## Implementation Tasks

- [x] Remove fixed 60–200-character display cutoffs from permitted `tool_use` scalar fields and `tool_result` content in `src/stream_json_textifier.rs`, while retaining write/edit body omission, raw-JSON suppression, and one semantic summary per event. Build the complete prefixed summary first, apply the shared operator-facing sanitizer/bound exactly once, and preserve a truthful omitted-byte marker through final `LogEntry` construction. Completion requires former cutoff boundaries, multibyte input, ANSI/control content, exact 8,192-byte final size, and exact omitted-byte accounting tests. (verification: unit - extend `src/stream_json_textifier.rs` and `src/events.rs` tests and run `cargo test --lib stream_json_textifier && cargo test --lib events`; verification-id: tui-log-width-regressions)
- [x] Add a runner/textifier integration regression at the existing `process_stdout_line` consumption boundary in `src/agent/runner.rs` proving a permitted tool-event value above 200 characters reaches the final operator-facing output/`LogEntry` without intermediate fixed-length truncation, while non-TUI CLI output receives the same sanitized bounded representation. (verification: integration - extend focused runner tests and run `cargo test --lib agent::runner`; verification-id: tui-log-width-regressions)
- [x] Replace entry-count-only Logs navigation with one process-local display-line anchor shared by `src/tui/state.rs`, `src/tui/state/log_logic.rs`, `src/tui/key_handlers.rs`, and `src/tui/render.rs`. Preserve `PgUp`, `PgDn`, `Home`, and `End` assignments, but allow movement within a single wrapped entry taller than the viewport; define deterministic clamping/reset for width, filter target/state, appended or trimmed logs, and auto-scroll transitions. Completion requires no durable state and a title/range indicator whose meaning matches display-line navigation rather than claiming an entry offset. (verification: unit - add navigation state-machine tests under `src/tui/state/log_logic.rs` and `src/tui/key_handlers.rs`, then run `cargo test --lib tui::state::log_logic && cargo test --lib tui::key_handlers`; verification-id: tui-log-width-regressions)
- [x] Preserve width-owned multi-line Logs rendering: first-line capacity follows timestamp/header plus current inner width and continuation lines consume full inner width without indentation. Add narrow/short-viewport TestBackend cases where one >200-character entry exceeds panel height and a concrete `Home`/`PgDn`/`PgUp`/`End` operation sequence makes every wrapped segment, including the first 200 source characters, appear in rendered buffers; also cover newest-line auto-scroll, filtering, resize, and buffer trimming. (verification: integration - extend `src/tui/render.rs` TestBackend rendering tests and run `cargo test --lib tui::render::tests`; verification-id: tui-log-width-regressions)
- [x] Preserve Changes-row previews as a separate strictly single-line policy that consumes the current remaining row width and truncates only there. Completion requires Select and Running narrow/wide TestBackend cases proving wider rows reveal more retained content, narrow rows use one Unicode-width-safe ellipsis, and no preview creates a continuation row or shifts the following item. (verification: unit - extend `src/tui/render.rs` preview tests and run `cargo test --lib tui::render::tests`; verification-id: tui-log-width-regressions)
- [x] Add CJK and emoji regressions across tool-event retention, Logs wrapping/navigation, and preview truncation, proving final byte bounds, display-column bounds, UTF-8 validity, and complete within-bound content reachability without using wall-clock assertions. (verification: integration - run `cargo test --lib stream_json_textifier && cargo test --lib events && cargo test --lib agent::runner && cargo test --lib tui::`; verification-id: tui-log-width-regressions)

## Implementation Notes

- Producer retention: `src/stream_json_textifier.rs` no longer calls a
  fixed-length truncator anywhere. `finalize_tool_summary` applies
  `crate::events::sanitize_detail` once to the complete prefixed summary, and
  `sanitize_detail` returns an already-bounded input unchanged, which is what
  keeps `LogEntry` construction idempotent.
- Navigation contract: `src/tui/state/log_logic.rs` owns `LogViewAnchor`
  (entry sequence + source byte offset), `LogViewport`, the wrapped
  display-line builder, anchor projection, and clamping. `state.rs` holds the
  ephemeral anchor plus a process-local `log_seq_base` so buffer trimming
  cannot invalidate entry identity; `render.rs` publishes the panel geometry
  and draws the same display-line sequence navigation moves through;
  `key_handlers.rs` keeps `PgUp`/`PgDn`/`Home`/`End` bound to those methods.
- Preview policy is untouched by the Logs wrapper: `change_row_preview_text`
  plus `truncate_to_display_width_with_suffix` remain the only preview path,
  so no shared helper can make a preview wrap.

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate fix-width-aware-tui-log-display --archive-gate`.

Verification evidence: `cargo fmt --all --check`, `cargo clippy --lib --tests --all-features` (clean), and the default suite `cargo test` all pass on this workspace.
