## Implementation Tasks

- [x] Update `src/tui/render.rs::render_header` so internal `AppExecutionMode::Stopped` uses the existing cyan Ready presentation while `Error`, Running counts, Stopping, and modal overrides retain their current behavior. Add `stopped_mode_header_shows_ready_with_resume_controls`; completion requires the rendered buffer to contain `[Ready]` with `Color::Cyan` asserted through `fg_at`, contain the configured resume control, omit `[Stopped]`, and leave `app.execution_mode == AppExecutionMode::Stopped` after rendering. (verification: unit - `cargo test --lib stopped_mode_header_shows_ready_with_resume_controls -- --list | grep -q stopped_mode_header_shows_ready_with_resume_controls && cargo test --lib stopped_mode_header_shows_ready_with_resume_controls`; verification-id: stopped-ready-header-regressions)

- [x] Add modal-free `error_mode_header_remains_unlabeled_without_modal` coverage at the shared Stopped/Error render branch. Completion requires Error mode with no modal to render neither `[Ready]` nor `[Stopped]`, retain the configured retry control, and remain `AppExecutionMode::Error`. (verification: unit - `cargo test --lib error_mode_header_remains_unlabeled_without_modal -- --list | grep -q error_mode_header_remains_unlabeled_without_modal && cargo test --lib error_mode_header_remains_unlabeled_without_modal`; verification-id: stopped-ready-header-regressions)

- [x] Preserve execution/modal separation and unaffected header mappings. Completion requires `overlay_header_label_is_presentation_only`, `test_running_header_counts_only_in_flight_changes`, and `test_stopping_mode_header_shows_stopping` to prove QR and confirmation labels remain presentation-only, Running still reports only active counts, and Stopping remains visible. (verification: unit - each named test is first proven present with `cargo test --lib <name> -- --list | grep -q <name>` and then run with `cargo test --lib <name>`; verification-id: stopped-ready-header-regressions)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate show-ready-header-after-stop --archive-gate`.

## Future Work

- None. Internal stop/resume vocabulary remains intentionally separate from this header presentation.
