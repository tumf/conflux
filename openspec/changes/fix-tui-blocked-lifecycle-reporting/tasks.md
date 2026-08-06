## Implementation Tasks

- [ ] Extend the typed TUI lifecycle snapshot with two row-status facts: whether any typed row is active or queued, and whether any row is blocked or stalled. Reuse the canonical active-status helper and keep the summary observability-only. Completion requires `from_app` to derive the facts from `AppState::changes[].display_status_cache` without terminal parsing, adapter knowledge, reducer mutation, or a second publisher. (verification: unit - `cargo test --lib tui::lifecycle::`; verification-id: tui-lifecycle-tests)

- [ ] Apply the lifecycle precedence for user-decision modal, active/queued work, blocked/stalled-only `Running`, ordinary zero-active `Running`, and unchanged non-Running modes. Completion requires table-driven tests for blocked-only, stalled-only, active-plus-waiting, queued-plus-waiting, empty Running, Stopping, QR, and confirmation states. (verification: unit - `cargo test --lib tui::lifecycle::`; verification-id: tui-lifecycle-tests)

- [ ] Add reducer-path regression coverage that applies canonical blocker events, synchronizes reducer display state into a still-`Running` `AppState`, and verifies both `blocked` and `stalled` outcomes project external lifecycle `blocked`; also prove an active or queued row restores `working`. Completion requires the test to traverse event/reducer/cache integration rather than manually setting only the final snapshot booleans. (verification: integration - `cargo test --lib tui::runner::`; verification-id: tui-lifecycle-tests)

- [ ] Verify repeated frame-equivalent blocked snapshots remain deduplicated and never alternate with `working`, while preserving existing lifecycle context, QR, modal, failure-isolation, and privacy tests. Completion requires runnable coverage at the existing `LifecycleStateTracker` publisher boundary, including the natural `lifecycle_integration::tests` placement in the declared rerun commands, rather than assertion-only documentation. (verification: integration - `cargo test --lib tui::lifecycle:: && cargo test --lib tui::runner:: && cargo test --lib lifecycle_integration::`; verification-id: tui-lifecycle-tests)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate fix-tui-blocked-lifecycle-reporting --archive-gate`.

## Future Work

- Consider sharing one semantic projection abstraction between TUI snapshots and non-interactive execution events only if a later change can remove duplication without creating competing publishers.
