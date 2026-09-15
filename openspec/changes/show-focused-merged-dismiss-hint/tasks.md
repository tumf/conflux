## Implementation Tasks

- [x] Change the existing merged-row dismissal final-buffer fixture from 180 to 120 columns without changing production title composition (verification: integration - `cargo test --lib tui::render::tests::a_focused_merged_row_advertises_both_dismissal_hints -- --exact && cargo test --lib tui::render::tests::running_mode_shows_the_same_target_dependent_hints -- --exact`; verification-id: focused-merged-dismiss-hint-render)
- [x] Run the complete merged-row dismissal render test group and confirm focused non-`merged` and empty-target hints remain absent (verification: integration - `cargo test --lib tui::render::tests:: -- --nocapture`; verification-id: focused-merged-dismiss-hint-render)
- [x] Run formatter, strict lint, and the repository test suite (verification: integration - `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test`; verification-id: focused-merged-dismiss-hint-render)

## Future Work

- A broader responsive help redesign remains intentionally excluded.

## Final Validation

Run `cflx openspec validate show-focused-merged-dismiss-hint --archive-gate` before archive.
