## Implementation Tasks

- [x] Extend the typed TUI lifecycle snapshot with two row-status facts: whether any typed row is active or queued, and whether any row is blocked or stalled. Reuse the canonical active-status helper and keep the summary observability-only. Completion requires `from_app` to derive the facts from `AppState::changes[].display_status_cache` without terminal parsing, adapter knowledge, reducer mutation, or a second publisher. (verification: unit - `cargo test --lib tui::lifecycle::`; verification-id: tui-lifecycle-tests)

- [x] Apply the lifecycle precedence for user-decision modal, active/queued work, blocked/stalled-only `Running`, ordinary zero-active `Running`, and unchanged non-Running modes. Completion requires table-driven tests for blocked-only, stalled-only, active-plus-waiting, queued-plus-waiting, empty Running, Stopping, QR, and confirmation states. (verification: unit - `cargo test --lib tui::lifecycle::`; verification-id: tui-lifecycle-tests)

- [x] Add reducer-path regression coverage that applies canonical blocker events, synchronizes reducer display state into a still-`Running` `AppState`, and verifies both `blocked` and `stalled` outcomes project external lifecycle `blocked`; also prove an active or queued row restores `working`. Completion requires the test to traverse event/reducer/cache integration rather than manually setting only the final snapshot booleans. (verification: integration - `cargo test --lib tui::runner::`; verification-id: tui-lifecycle-tests)

- [x] Verify repeated frame-equivalent blocked snapshots remain deduplicated and never alternate with `working`, while preserving existing lifecycle context, QR, modal, failure-isolation, and privacy tests. Completion requires runnable coverage at the existing `LifecycleStateTracker` publisher boundary, including the natural `lifecycle_integration::tests` placement in the declared rerun commands, rather than assertion-only documentation. (verification: integration - `cargo test --lib tui::lifecycle:: && cargo test --lib tui::runner:: && cargo test --lib lifecycle_integration::`; verification-id: tui-lifecycle-tests)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate fix-tui-blocked-lifecycle-reporting --archive-gate`.

## Notes

- evidence: `cargo test --lib tui::lifecycle::` — 15 passed, including the precedence table, QR transparency over a blocked-only wait, modal precedence, canonical active-status reuse, and the frame-repeat publication test.
- evidence: `cargo test --lib tui::runner::` — 13 passed, including `running_tui_projects_blocked_for_reducer_blocked_and_stalled_rows` and `active_or_queued_reducer_row_restores_working_lifecycle`, which traverse dispatch → reducer classification → display-cache sync → snapshot projection.
- evidence: `cargo test --lib lifecycle_integration::` — 14 passed, including `repeated_blocked_tui_frames_emit_one_transition_without_an_intervening_working` at the `LifecycleStateTracker` publisher boundary.
- The `stalled` reducer path is exercised through an acceptance blocker with no verifiable `unblock_condition`, which the canonical classifier keeps off the external `blocked` path; the `blocked` path uses a validated external prerequisite claim.
- The dedup test in `lifecycle_integration::tests` builds `TuiLifecycleSnapshot` values directly because `crate::tui::state` is a private module; the equivalent `AppState`-driven frame loop over the same tracker is covered by `tui::lifecycle::tests::repeated_frames_over_a_blocked_only_app_state_publish_one_transition`.
- The rustfmt repair for the acceptance finding is whitespace-only and landed in WIP commit `2a5ecea5`; `cargo fmt --check` now exits 0 with an empty diff across the crate.
- Rerun caveat: this repo shares one cargo target directory with concurrent sessions, so a contended run can execute a stale test binary. A first `cargo test --lib tui::runner::` reported 11 tests (both new reducer-path tests missing from `--list`); after `touch src/tui/runner.rs` forced a rebuild it reported the expected 13. Confirm the module's test count matches the source before trusting a count taken under lock contention.

## Future Work

- Consider sharing one semantic projection abstraction between TUI snapshots and non-interactive execution events only if a later change can remove duplication without creating competing publishers.
