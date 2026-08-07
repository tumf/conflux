## Implementation Tasks

- [ ] Filter automatic ahead/conflict inspection before spawning Git commands, using current active/rejected change identities while retaining every Git-registered worktree in the listing with an explicit not-inspected state. Completion: stale, archived, unrelated, and branchless secondary worktrees remain visible but cannot be presented as conflict-free/mergeable merely because inspection was skipped. (verification: integration - `cargo test --locked --test e2e_git_worktree_tests`; verification-id: stale-worktree-refresh-regression)
- [ ] Add a process-local observation cache keyed by branch identity, base HEAD, worktree HEAD, and merge base, and invalidate on any key change. Completion: identical refresh inputs execute no second merge simulation; each individual identity/revision change executes exactly one fresh inspection; cache deletion or process restart changes no workflow action for identical workspace state. (verification: integration - `cargo test --locked --test e2e_git_worktree_tests`; verification-id: stale-worktree-refresh-regression)
- [ ] Bound merge simulation results and diagnostics in `src/vcs/git/commands/merge.rs`. Completion: conflict and non-conflict outcomes preserve exit status, total output byte counts, conflict count, and a deterministic limited file/prefix sample, while no tracing record or returned error contains complete arbitrarily large stdout/stderr. (verification: unit - focused merge parser/log-shaping tests plus `cargo test --locked --test e2e_git_worktree_tests`; verification-id: stale-worktree-refresh-regression)
- [ ] Add stale-worktree refresh regression coverage with real temporary Git worktrees and observable command counts. Completion: tests prove non-active worktrees are skipped, active/rejected worktrees are checked, unchanged revisions are cached, base/worktree/branch changes invalidate cache, large conflict output is bounded, and refresh leaves index and dirty files byte-identical. (verification: integration - `cargo test --locked --test e2e_git_worktree_tests`; verification-id: stale-worktree-refresh-regression)

## Future Work

- An operator-triggered backup-and-remove action may be proposed separately; this change intentionally performs no automatic destructive cleanup.

## Final Validation

Run `cflx check-conflicts`, `cflx openspec validate harden-stale-worktree-refresh --strict`, `cflx openspec validate harden-stale-worktree-refresh --evidence error`, and `cflx openspec validate harden-stale-worktree-refresh --archive-gate`. Archive validation remains the authoritative final OpenSpec gate.
