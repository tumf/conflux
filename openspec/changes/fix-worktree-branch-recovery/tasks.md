## Implementation Tasks

- [ ] Update `src/vcs/git/commands/worktree.rs` so stale-path recovery can continue into safe existing-branch attach behavior after prune/removal (verification: unit tests covering stale path + remaining branch and checked-out branch rejection in `src/vcs/git/commands/worktree.rs`).
- [ ] Align or centralize worktree deletion behavior used by the TUI `D` flow so associated branch deletion is attempted after worktree removal and remains non-fatal on failure (verification: code path exercised from `src/tui/command_handlers.rs` and existing/shared delete helpers).
- [ ] Add or update tests for TUI worktree deletion semantics and warning behavior when branch deletion fails or branch is already absent (verification: targeted test file or module covering the emitted behavior/log path).
- [ ] Run repository validation for the affected behavior (verification: `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`).

## Future Work

- Evaluate whether server and web worktree deletion endpoints should reuse the exact same branch-cleanup helper for stricter consistency across entry points.
