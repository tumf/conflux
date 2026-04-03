## Implementation Tasks

- [ ] 1. Define a dedicated rejecting lifecycle stage in shared orchestration state and derived display status paths (verification: `openspec/specs/orchestration-state/spec.md` delta covers reducer/runtime semantics and status derivation for rejecting)
- [ ] 2. Route apply-generated `REJECTED.md` handoff into rejecting instead of acceptance in serial and parallel orchestration flows (verification: proposal deltas cover handoff/resume behavior for `src/execution/apply.rs`, parallel executor, and serial run service)
- [ ] 3. Specify rejecting review outcomes `confirm_rejection` and `resume_apply`, including base-branch REJECTED-only commit semantics for confirmed rejection (verification: `openspec/specs/parallel-execution/spec.md` delta covers rejecting outcome contract and REJECTED-only merge rule)
- [ ] 4. Specify reject-dismissal behavior so the runtime removes worktree `REJECTED.md`, appends non-rejection recovery tasks to `tasks.md`, and returns the change to apply (verification: spec delta includes explicit scenario for `tasks.md` mutation before resuming apply)
- [ ] 5. Specify reducer/API/UI visibility updates so `rejecting` is surfaced as an active runtime stage in dashboard/TUI state consumers (verification: spec delta references shared orchestration state and observable display status requirements)
- [ ] 6. Validate the proposal strictly and ensure all affected deltas are scenario-complete (verification: `python3 /Users/tumf/.agents/skills/cflx-proposal/scripts/cflx.py validate split-rejecting-from-acceptance --strict` passes)

## Future Work

- Update any downstream acceptance/review agent prompts after implementation if they need specialized rejecting-mode instructions
- Evaluate whether rejected/recovered task annotations should be standardized in proposal authoring guidance
