## Implementation Tasks

- [x] Replace the Status panel's selected-only task aggregation with one row-level inclusion rule covering reducer-observed archive completion or final success display, shared active execution status, and execution-marked future work. Count each matching row once, explicitly exclude rejected rows, retain stored task counts without synthesizing completion, and preserve existing zero-total and elapsed-time rendering. Completion requires `render_status` to use the shared active-status helper and existing archive-completion/post-archive facts without changing mark reconciliation or adding state. (verification: unit - focused predicate and rendered aggregation cases via `cargo test --lib tui_status_overall_progress`; verification-id: tui-overall-progress-tests)

- [x] Add rendered TUI regressions for the mixed example `merged 3/3` unmarked, `applying 1/4` unmarked, `not queued 0/2` marked, and `not queued 0/5` unmarked, requiring `4/9` and `44.4%`; add overlap cases proving active-plus-marked and completed-plus-marked rows are never double-counted. Completion requires assertions against the actual Status widget output, not a duplicate test-only calculator. (verification: unit - `cargo test --lib tui_status_overall_progress` exercises the Ratatui test backend; verification-id: tui-overall-progress-tests)

- [x] Add lifecycle-boundary regressions proving a cleared mark cannot remove `archived`, `merged`, or `pushed` progress; `archive_complete_cache` retains post-archive `resolving`, `resolve pending`, and `merge wait`; every shared active status is included unmarked; marked retryable error is included; rejected, unmarked error, and unmarked idle rows are excluded; and all-zero included rows preserve safe no-task rendering. Completion requires the focused test names to share the `tui_status_overall_progress` prefix so the declared discovery command fails if coverage is renamed or absent. (verification: unit - discovery and execution through `cargo test --lib tui_status_overall_progress -- --list | grep -q ': test$' && cargo test --lib tui_status_overall_progress`; verification-id: tui-overall-progress-tests)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate fix-tui-overall-progress-scope --archive-gate`.

The implementation verification is `cargo test --lib tui_status_overall_progress -- --list | grep -q ': test$' && cargo test --lib tui_status_overall_progress`. The tracked Rust commit hooks own workspace rustfmt and clippy when Rust paths are changed.

## Notes

- evidence: declared verification ran once and passed — discovery matched and `8 passed; 0 failed` for the `tui_status_overall_progress` prefix (unit evidence, Ratatui `TestBackend` only)
- evidence: `cargo test --lib` full default suite `3764 passed; 0 failed; 17 ignored` after the change
- evidence: `cargo fmt --all -- --check` and `cargo clippy --lib --all-targets` both clean

## Future Work

- Consider a separately specified historical run manifest only if operators later need progress for targets removed from the visible change catalog or across process restarts.
