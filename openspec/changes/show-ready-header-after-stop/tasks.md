## Implementation Tasks

- [ ] Update `src/tui/render.rs::render_header` so internal `AppExecutionMode::Stopped` uses the existing cyan Ready presentation while `Error`, Running counts, Stopping, and modal overrides retain their current behavior. Add `stopped_mode_header_shows_ready_with_resume_controls`; completion requires the rendered buffer to contain `[Ready]` and the configured resume control, omit `[Stopped]`, and leave `app.execution_mode == AppExecutionMode::Stopped` after rendering. (verification: unit - `cargo test --lib stopped_mode_header_shows_ready_with_resume_controls`; verification-id: stopped-ready-header-regressions)

- [ ] Preserve the execution/modal separation across existing header tests. Completion requires `overlay_header_label_is_presentation_only` and adjacent render coverage to prove QR and confirmation labels do not convert stopped execution mode, Error is not rewritten as Ready, Running still reports only active counts, and Stopping remains visible. (verification: unit - `cargo test --lib overlay_header_label_is_presentation_only`; verification-id: stopped-ready-header-regressions)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate show-ready-header-after-stop --archive-gate`.

## Future Work

- None. Internal stop/resume vocabulary remains intentionally separate from this header presentation.
