## Implementation Tasks

- [x] Add post-apply cleanup review orchestration to the parallel apply flow in `src/parallel/executor.rs` so task-complete managed worktrees run cleanup review before `Apply:` completion is finalized (verification: targeted tests cover dirty-after-apply success and cleanup command failure outcomes).
- [x] Add cleanup review prompt builder and output parsing in `src/agent/prompt.rs` and related orchestration code so Conflux can invoke `cflx-workflow` with a dedicated cleanup-review operation and machine-readable verdicts (verification: unit tests for prompt shape and verdict parsing).
- [x] Extend `skills/cflx-workflow/SKILL.md` with a cleanup-review operation that scopes the agent to safe handoff cleanup, forbids blind staging of all files, and defines exact success/blocking markers (verification: embedded skill tests/build continue to pass and prompt text references the new operation).
- [x] Update parallel execution and agent prompt specs so managed worktree apply requires post-apply cleanup review before acceptance handoff when the worktree remains dirty (verification: strict proposal validation and spec delta coverage for success/block scenarios).

## Future Work

- Revisit whether cleanup-review outcomes should later get a dedicated user-visible status if operators need finer-grained observability.

