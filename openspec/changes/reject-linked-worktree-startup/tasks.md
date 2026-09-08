## Implementation Tasks

- [ ] Add one repository-evidence classifier for orchestration-owner startup that distinguishes the main worktree from a linked worktree without treating every `.git` pointer as linked; integrate it before repository-lock acquisition for bare TUI, explicit TUI, and `run`. Completion requires all three owner entrypoints to use the same preflight and all non-owner commands to bypass it. (verification: integration - `cargo test --test run_exit_tests linked_worktree_startup`; verification-id: linked-worktree-startup-tests)
- [ ] Return a non-zero, actionable refusal containing the current linked-worktree path, the main worktree path derived from `git worktree list --porcelain`, and a start-from-main instruction, without automatic redirect, prune, cleanup, or mutation. Completion requires exact assertions over the diagnostic and side-effect absence. (verification: integration - `cargo test --test run_exit_tests linked_worktree_startup`; verification-id: linked-worktree-startup-tests)
- [ ] Add real-Git regression fixtures covering linked-worktree refusal for all owner entrypoints, main-worktree eligibility, separate-Git-directory eligibility, and linked-worktree use by representative non-owner/client routing. Completion requires tests to fail against the pre-change binary and pass only when owner startup is rejected before lock files, owner metadata, sockets, logs, lifecycle hooks, AI commands, and managed worktrees can be produced. (verification: integration - `cargo test --test run_exit_tests linked_worktree_startup`; verification-id: linked-worktree-startup-tests)
- [ ] Rewrite the existing `linked_worktrees_share_one_repository_lock` and `linked_worktrees_advertise_one_socket` regressions so they no longer start an owner from a linked worktree: assert preflight refusal separately, retain same-common-directory lock/socket identity through main-owner startup plus linked-worktree client/path resolution, and run the complete integration target to catch stale assertions. (verification: integration - `cargo test --test run_exit_tests`; verification-id: linked-worktree-startup-tests)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate reject-linked-worktree-startup --archive-gate`.

## Future Work

- Stale worktree repair remains a separate operator action or future change.
