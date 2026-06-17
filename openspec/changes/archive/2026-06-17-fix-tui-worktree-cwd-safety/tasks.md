## Implementation Tasks

- [x] Add repository-evidence validation for TUI-created worktree command cwd readiness. Completion condition: a helper used by the `+` path verifies the target path exists, is a directory, resolves as a Git toplevel, has usable worktree Git metadata, and is registered in `git worktree list --porcelain` for the base repo. (verification: unit - added fact-level tests for valid, missing, non-directory, non-Git, toplevel mismatch, unregistered, and main-worktree cases plus a real materialized Git worktree check in `src/vcs/git/commands/worktree.rs`; ran `cargo test validate_worktree_command_cwd --lib`.)

- [x] Validate the created worktree before running `.wt/setup`. Completion condition: after `worktree_add()` succeeds, `handle_plus_key()` refuses to continue to setup when the target is not a materialized Git worktree and logs the failed validation with the path. (verification: integration - added `plus_prepare_suppresses_setup_when_created_worktree_validation_fails` with a runtime seam proving the invalid post-create validation path does not call `run_worktree_setup()` or `execute_worktree_command()`; ran `cargo test plus_ --lib`.)

- [x] Validate the worktree again after `.wt/setup` and immediately before launching `worktree_command`. Completion condition: if setup or an external race removes or invalidates the worktree path, `handle_plus_key()` logs the invalid cwd and does not call `execute_worktree_command()`. (verification: integration - added `plus_prepare_suppresses_command_when_worktree_invalid_after_setup` and `plus_prepare_suppresses_command_when_worktree_invalid_before_launch` with a runtime seam asserting the command-runner boundary is not invoked; ran `cargo test plus_ --lib`.)

- [x] Make setup-failure cleanup observable and unambiguous. Completion condition: when `.wt/setup` fails, TUI logs include the setup error, an explicit cleanup-start message with the worktree path, and cleanup success or cleanup failure before returning. (verification: unit - added `plus_prepare_logs_setup_failure_cleanup_and_suppresses_command`, which asserts setup error, cleanup-start, cleanup-success diagnostics and no command launch; ran `cargo test plus_ --lib`.)

- [x] Preserve successful `+` behavior for valid worktrees and configured commands. Completion condition: valid worktree creation, setup success/no setup, command expansion, and command launch still occur with the worktree path as cwd and existing placeholders expanded as before. (verification: integration - added `plus_prepare_with_production_runtime_creates_registered_materialized_worktree` using a real temporary Git repo/worktree fixture and `plus_handle_invokes_command_runner_boundary_for_valid_worktree` using a harmless command-runner stub; ran `cargo test plus_ --lib`.)

- [x] Cover safer command-template usage without breaking existing templates. Completion condition: tests or docs in the proposal implementation demonstrate that `{workspace_dir}` remains correctly escaped and can be used by commands such as `tmux new-window -n wt -c {workspace_dir}`, while cwd-based templates continue to work. (verification: unit - added `test_expand_worktree_command_tmux_c_template` and `test_expand_worktree_command_cwd_based_template_still_works` in `src/config/expand.rs`; ran `cargo test expand_worktree_command --lib`.)

- [x] Run targeted Rust verification for touched modules. Completion condition: targeted tests for TUI key handling, worktree helpers, Git worktree validation, and config expansion pass locally. (verification: manual - passed focused filters after formatting: `agent-exec run -- cargo test plus_ --lib` job `91a8dbb9c51735061db1ca8cf7362b93`, `agent-exec run -- cargo test validate_worktree_command_cwd --lib` job `8cc7d01000e2aa29d558c2175114bc71`, `agent-exec run -- cargo test expand_worktree_command --lib` job `1c4fe201ab75a5ea18f2040d088f5b36`, and `agent-exec run -- cargo test should_trigger_worktree_command --lib` job `7ca9c553c1ea5df0c1249dc49fe22685`; `cargo fmt --check` also passed.)

## Future Work

- Broader tmux integration UX improvements, such as generating a recommended default `worktree_command`, can be handled separately if desired.
- External filesystem or mount health diagnostics for `XDG_DATA_HOME` on removable volumes are separate operational improvements and are not required for this fix.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected authoring checks:

`cflx openspec validate fix-tui-worktree-cwd-safety --strict --evidence warn`

`cflx openspec validate fix-tui-worktree-cwd-safety --archive-gate`
