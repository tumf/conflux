## Implementation Tasks

- [x] Verify archive completion against a stable existing repository root during merge attempts, not a possibly deleted worktree path. (verification: unit - added `src/parallel/merge.rs` tests for deleted archive path fallback; verified with `cargo test archive_verification_root --lib`)
- [x] Add stale retry handling before deferred merge retry invokes archive verification on workspace paths. (verification: unit/integration-aligned - added queue-state stale retry helper tests covering deleted and existing workspace paths; verified with `cargo test stale_retry_reason --lib`)
- [x] Preserve legitimate manual merge blocker behavior for existing roots. (verification: integration - existing dirty-base heavy regression remains valid; verified with `cargo test test_merge_deferred_when_worktree_dirty --lib --features heavy-tests`)
- [x] Deduplicate repeated identical merge-deferred TUI diagnostics. (verification: unit - added `src/tui/state/event_handlers/errors.rs` duplicate suppression test; verified with `cargo test merge_deferred_warning --lib`)
- [x] Prove distinct merge-deferred reasons remain visible. (verification: unit - added `src/tui/state/event_handlers/errors.rs` changed reason test; verified with `cargo test merge_deferred_warning --lib`)
- [x] Run focused Rust regression tests for merge retry and TUI warning behavior. (verification: integration - ran `cargo test archive_verification_root --lib`, `cargo test stale_retry_reason --lib`, `cargo test merge_deferred_warning --lib`, `cargo test test_merge_deferred_when_worktree_dirty --lib --features heavy-tests`, and `cflx openspec validate fix-stale-merge-retry-worktree-status --strict`)

## Future Work

- Full end-to-end reproduction in a live multi-change Conflux run may be useful after implementation, but local unit/integration tests are the required acceptance evidence.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-stale-merge-retry-worktree-status --archive-gate`
