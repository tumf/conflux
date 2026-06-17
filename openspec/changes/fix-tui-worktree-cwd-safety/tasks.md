## Implementation Tasks

- [ ] Add repository-evidence validation for TUI-created worktree command cwd readiness. Completion condition: a helper used by the `+` path verifies the target path exists, is a directory, resolves as a Git toplevel, has usable worktree Git metadata, and is registered in `git worktree list --porcelain` for the base repo. (verification: unit - add tests for valid, missing, non-directory, non-Git, and unregistered path cases in `src/tui/worktrees.rs` or `src/vcs/git/commands/worktree.rs`.)

- [ ] Validate the created worktree before running `.wt/setup`. Completion condition: after `worktree_add()` succeeds, `handle_plus_key()` refuses to continue to setup when the target is not a materialized Git worktree and logs the failed validation with the path. (verification: integration - add a `src/tui/key_handlers.rs` test with a temporary Git fixture or test seam proving an invalid `worktree_add()` result does not call `run_worktree_setup()` or `execute_worktree_command()`.)

- [ ] Validate the worktree again after `.wt/setup` and immediately before launching `worktree_command`. Completion condition: if setup or an external race removes or invalidates the worktree path, `handle_plus_key()` logs the invalid cwd and does not call `execute_worktree_command()`. (verification: integration - add a `src/tui/key_handlers.rs` regression test that removes the temporary worktree path after setup and asserts the command-runner boundary is not invoked.)

- [ ] Make setup-failure cleanup observable and unambiguous. Completion condition: when `.wt/setup` fails, TUI logs include the setup error, an explicit cleanup-start message with the worktree path, and cleanup success or cleanup failure before returning. (verification: unit - add or update `src/tui/key_handlers.rs` tests asserting setup failure records cleanup diagnostics and never launches `worktree_command`.)

- [ ] Preserve successful `+` behavior for valid worktrees and configured commands. Completion condition: valid worktree creation, setup success/no setup, command expansion, and command launch still occur with the worktree path as cwd and existing placeholders expanded as before. (verification: integration - add a `src/tui/key_handlers.rs` successful Worktrees `+` test using a real temporary Git repo/worktree fixture and a harmless command runner stub.)

- [ ] Cover safer command-template usage without breaking existing templates. Completion condition: tests or docs in the proposal implementation demonstrate that `{workspace_dir}` remains correctly escaped and can be used by commands such as `tmux new-window -n wt -c {workspace_dir}`, while cwd-based templates continue to work. (verification: unit - extend `src/config/expand.rs` tests or TUI command expansion tests for a tmux `-c {workspace_dir}` template.)

- [ ] Run targeted Rust verification for touched modules. Completion condition: targeted tests for TUI key handling, worktree helpers, Git worktree validation, and config expansion pass locally. (verification: manual - run focused `cargo test` filters for `tui::key_handlers`, `tui::worktrees`, `vcs::git::commands::worktree`, and `config::expand`; this is intentional manual coverage because exact filters depend on final edited tests.)

## Future Work

- Broader tmux integration UX improvements, such as generating a recommended default `worktree_command`, can be handled separately if desired.
- External filesystem or mount health diagnostics for `XDG_DATA_HOME` on removable volumes are separate operational improvements and are not required for this fix.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected authoring checks:

`cflx openspec validate fix-tui-worktree-cwd-safety --strict --evidence warn`

`cflx openspec validate fix-tui-worktree-cwd-safety --archive-gate`
