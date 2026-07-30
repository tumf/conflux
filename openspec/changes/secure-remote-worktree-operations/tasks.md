## Implementation Tasks

- [ ] **Task 1: Add a process-local worktree resource registry** that allocates random 128-bit opaque IDs on first observation, retires IDs on disappearance, assigns new IDs after same-path recreation, and never uses paths or branches as mutation identity. (verification: unit - registry lifecycle/property cases in `cargo test remote_worktree`; verification-id: remote-worktree-local)
- [ ] **Task 2: Define redacted v2 worktree DTOs** with repository-relative display paths, 16-hex FNV-1a `repository_id`, nullable dirty state, operation eligibility, conflict evidence, and no canonical root/path target fields. (verification: unit - serialization and redaction cases in `cargo test remote_worktree`; verification-id: remote-worktree-local)
- [ ] **Task 3: Extract a shared TUI/API worktree operation service** for create, guarded delete, base merge, repository lock, hooks, reducer updates, and events. (verification: integration - adapter parity using real service fixtures in `cargo test remote_worktree`; verification-id: remote-worktree-local)
- [ ] **Task 4: Implement v2 worktree list/detail and create command** with authentication, idempotency, lifecycle/root-busy guards, refreshed resource output, and no generic command execution. (verification: integration - authenticated router/service cases in `cargo test remote_worktree`; verification-id: remote-worktree-local)
- [ ] **Task 5: Implement fail-closed delete by `worktree_id`** requiring expected revision, known-clean/eligible state, mandatory teardown, and post-removal identity retirement; expose no skip-teardown or force parameter. (verification: e2e - `tests/e2e_git_worktree_tests.rs` teardown, dirty, unknown-dirty, stale revision, and recreation cases via `cargo test --features heavy-tests --test e2e_git_worktree_tests remote_worktree`; verification-id: remote-worktree-heavy)
- [ ] **Task 6: Implement conflict-preserving merge by `worktree_id`** requiring expected revision and idempotency, using the TUI-equivalent base merge and `on_merged` event path, while retaining intermediate merge state and conflict file evidence on conflict. (verification: e2e - `tests/e2e_git_worktree_tests.rs` successful/conflicting Git merge and hook/event cases via `cargo test --features heavy-tests --test e2e_git_worktree_tests remote_worktree`; verification-id: remote-worktree-heavy)
- [ ] **Task 7: Extend OpenAPI and negative security coverage** proving v2 omits absolute-path/branch mutation targets, arbitrary `worktree_command`, editor/session operations, teardown bypasses, and unauthenticated mutation. (verification: integration - schema snapshots and rejected request cases in `cargo test remote_worktree` plus `make check-openapi`; verification-id: remote-worktree-local)

## Future Work

- Explicit conflict-resolution commands if a separate guarded design is approved.
- Cross-process stable resource identity if a durable, Constitution-compatible use case emerges.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate secure-remote-worktree-operations --archive-gate`
