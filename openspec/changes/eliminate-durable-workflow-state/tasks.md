## Implementation Tasks

- [ ] 1. remove out-of-worktree workflow state readers/writers from `src/parallel/acceptance_state.rs`, `src/parallel/archive_state.rs`, `src/parallel/dispatch.rs`, and `src/parallel/executor.rs`, and ensure resume/archive control flow no longer depends on `~/.local/state/cflx/**` (verification: integration - targeted Rust tests prove the same workspace routes identically whether external state directories exist or not)
- [ ] 2. implement workspace-local acceptance/archive routing in `src/execution/state.rs` and `src/parallel/dispatch.rs` so `Applied` resumes re-run acceptance unless workspace-local evidence proves archive handoff readiness, and `Archiving` / `Archived` remain file/git-state-derived only (verification: integration - workspace resume tests cover `Applied`, `Archiving`, and `Archived` paths using only workspace-local fixtures)
- [ ] 3. add regression tests for external-state independence in `src/parallel/tests/` or equivalent so deleting or pre-populating `~/.local/state/cflx/acceptance-state` / `archive-resume-state` cannot alter next-phase selection for the same workspace contents (verification: integration - new tests explicitly set up both presence/absence cases and assert identical outcomes)
- [ ] 4. update canonical and proposal-side specs under `openspec/specs/parallel-execution/` plus any related deltas so out-of-worktree durable workflow state is forbidden and observability outputs are documented as non-authoritative (verification: manual - spec text review confirms no remaining requirement allows workflow control decisions from outside the workspace)
- [ ] 5. verify the proposal and implementation path with `cflx openspec validate eliminate-durable-workflow-state --strict --evidence warn`, targeted Rust tests for workspace-local routing, and project lint/type checks required by the repo (verification: integration - command output shows validate/tests/lint pass and would fail if external state were still authoritative)

## Future Work

- Human review of whether any remaining non-workflow observability caches under `~/.local/state/cflx/` should be relocated or renamed for clarity.
