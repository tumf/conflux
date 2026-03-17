## Implementation Tasks

- [ ] Audit and update parallel workspace reuse/state-detection rules in `src/execution/state.rs`, `src/parallel/dispatch.rs`, and related workspace acquisition code so reused workspaces enter only the intended resume path for their detected state (verification: code paths distinguish Created/Applying/Applied/Archived/Merged behavior explicitly during reuse).
- [ ] Update CLI/user-visible reporting around workspace reuse in `src/orchestrator.rs` and/or related event emission so resumed workspaces are announced as resumed rather than looking like a fresh start (verification: logs/output identify when an existing workspace was reused and what state was detected).
- [ ] Tighten or document `tasks.md` archive fallback behavior in `src/execution/apply.rs` so archived-task completion is only used for intended resume states and does not silently short-circuit fresh-looking runs (verification: archived `tasks.md` fallback is covered by targeted tests for reused workspaces).
- [ ] Add regression tests for reused workspaces that currently jump directly to "already complete" or final-commit behavior, including coverage for `--no-resume` expectations where relevant (verification: targeted tests in execution/parallel/orchestrator modules fail before the fix and pass after it).
- [ ] Run proposal-aligned verification after implementation (verification: `cargo fmt --check`, `cargo clippy -- -D warnings`, and relevant `cargo test` coverage for workspace reuse regression cases).

## Future Work

- Consider exposing a first-class CLI/TUI indicator for detected `WorkspaceState` if users need richer debugging of resume behavior.
