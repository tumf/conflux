## Implementation Tasks

- [x] Add a worktree-based resume action classifier for parallel execution that returns only terminal/no-op, apply, or acceptance outcomes (verification: `src/parallel/dispatch.rs` no longer routes resumed non-terminal worktrees directly to archive)
- [x] Update resume dispatch logic to send incomplete-task worktrees back to apply and 100%-complete worktrees to acceptance (verification: resume branch in `src/parallel/dispatch.rs` uses worktree task progress instead of `Applied -> archive` semantics)
- [x] Align workspace state helpers and runtime/display mapping with the new resume routing rules (verification: `src/execution/state.rs` and `src/server/api.rs` do not expose a resumed non-terminal path that implies direct archive)
- [x] Add regression tests covering resumed worktrees with incomplete tasks, complete tasks, and archived terminal state (verification: targeted tests under `src/parallel/tests/` and/or existing state test modules cover all three paths)
- [x] Run validation and Rust quality gates for the eventual implementation (verification: `python3 "/Users/tumf/.agents/skills/cflx-proposal/scripts/cflx.py" validate update-worktree-resume-routing --strict`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`)

## Future Work

- Verify whether TUI/web wording should distinguish “resume to acceptance” from generic archiving labels once implementation lands.

## Acceptance #1 Failure Follow-up

- [x] Update `src/execution/state.rs` documentation/comments/examples so resumed `WorkspaceState::Applied` no longer imply direct archive-only routing and instead match the new apply-or-acceptance resume semantics.
