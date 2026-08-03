## Implementation Tasks

- [ ] Extend the shared deletion policy in `src/worktree_ops/service.rs` with an explicit dirty-discard permission that defaults to false, is false for `DeleteOptions::fail_closed()`, and does not waive main, root-busy, dirty-unknown, commits-ahead, or identity guards. Add pure unit cases proving the permission matrix and confirming skip-teardown alone still refuses dirty deletion. Completion requires `classify_delete_eligibility` tests to fail for a stubbed or globally permissive implementation. (verification: unit - `cargo test worktree_ops`; verification-id: dirty-worktree-delete-tests)

- [ ] Add a typed TUI destructive-confirmation state carrying the confirmed worktree path and branch identity, and transition to it only when ordinary confirmation re-observes the same eligible target as dirty. Keep modal invalidation tied to fresh worktree observations and active/deleting state. Completion requires state tests for clean ordinary deletion, dirty escalation, cancellation, identity replacement, target disappearance, active transition, dirty-unknown, root-busy, and commits-ahead refusal. (verification: unit - `cargo test worktree_delete`; verification-id: dirty-worktree-delete-tests)

- [ ] Wire a distinct explicit dirty-discard input through TUI key handling and command dispatch without reusing ordinary `Y` or teardown-only `S`, and carry both dirty-discard and skip-teardown intent independently to `WorktreeService::delete_worktree`. Completion requires key/command tests proving ordinary `Y`, `S`, cancel, and unrelated keys cannot accidentally grant dirty-discard permission, while the dedicated action can. (verification: integration - `cargo test worktree_delete`; verification-id: dirty-worktree-delete-tests)

- [ ] Render the destructive confirmation with explicit permanent-loss text covering uncommitted and untracked files, the exact dedicated confirmation control, and cancellation controls; retain the ordinary clean deletion copy. Completion requires deterministic render/state assertions that distinguish the two modal variants and expose no misleading `S`-as-force wording. (verification: unit - `cargo test worktree_delete`; verification-id: dirty-worktree-delete-tests)

- [ ] Revalidate the target under the existing service mutation guard immediately before teardown/removal, then permit dirty removal only for a still-matching branch with explicit local discard permission. Record a warning before removal, run teardown unless independently skipped, preserve best-effort branch deletion, and emit the existing delete/refresh events. Completion requires service/backend tests for successful dirty discard, teardown failure retention, identity drift, eligibility drift, and warning/event ordering. (verification: integration - `cargo test worktree_ops`; verification-id: dirty-worktree-delete-tests)

- [ ] Preserve the remote safety boundary: keep `/api/v2` and WebUI on fail-closed options, add no request/schema/UI unsafe controls, and retain typed dirty and dirty-unknown refusals. Completion requires API tests that dirty deletion is rejected and payloads containing dirty-discard, force, skip-teardown, path, or branch fields remain invalid without invoking removal. (verification: integration - `cargo test remote_worktree`; verification-id: dirty-worktree-delete-tests)

- [ ] Run the complete repository-local gate and fix failures without adding stash/backup behavior, ahead-commit force deletion, remote unsafe controls, or durable workflow state. Completion requires `cargo fmt --check`, `cargo test worktree_delete`, `cargo test worktree_ops`, `cargo test remote_worktree`, `cargo test`, and `cargo clippy --all-targets -- -D warnings` to exit successfully; tests exceeding one second must use the repository heavy-test policy. (verification: integration - execute `cargo fmt --check && cargo test worktree_delete && cargo test worktree_ops && cargo test remote_worktree && cargo test && cargo clippy --all-targets -- -D warnings`; verification-id: dirty-worktree-delete-tests)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate allow-tui-dirty-worktree-delete --archive-gate`.

## Future Work

- Consider an explicit export or stash workflow only if operators request recoverable cleanup; it is not required for intentional disposable-worktree deletion.
