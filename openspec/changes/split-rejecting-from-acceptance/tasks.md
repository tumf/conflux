## Implementation Tasks

- [x] 1. Define a dedicated rejecting lifecycle stage in shared orchestration state and derived display status paths (verification: `openspec/specs/orchestration-state/spec.md` delta covers reducer/runtime semantics and status derivation for rejecting)
- [x] 2. Route apply-generated `REJECTED.md` handoff into rejecting instead of acceptance in serial and parallel orchestration flows (verification: proposal deltas cover handoff/resume behavior for `src/execution/apply.rs`, parallel executor, and serial run service)
- [x] 3. Specify rejecting review outcomes `confirm_rejection` and `resume_apply` as dedicated runtime verdicts, including the exact marker contract `REJECTION_REVIEW: CONFIRM|RESUME` and base-branch REJECTED-only commit semantics for confirmed rejection (verification: `openspec/specs/parallel-execution/spec.md` delta covers marker parsing contract and REJECTED-only merge rule)
- [x] 4. Specify reject-dismissal behavior so the runtime removes worktree `REJECTED.md`, appends non-rejection recovery tasks to `tasks.md`, and returns the change to apply without re-entering normal acceptance first (verification: spec delta includes explicit scenario for `tasks.md` mutation and routing before resuming apply)
- [x] 5. Specify reducer/API/UI visibility updates so `rejecting` is surfaced as an active runtime stage in dashboard/TUI state consumers (verification: spec delta references shared orchestration state and observable display status requirements)
- [x] 6. Update `skills/cflx-workflow/` canonical workflow instructions so apply/rejecting/accept handoff semantics and final verdict markers match the dedicated rejecting stage (verification: implementation updates target `skills/cflx-workflow/SKILL.md` and the relevant reference docs such as `skills/cflx-workflow/references/cflx-accept.md`)
- [x] 7. Validate the proposal strictly and ensure all affected deltas are scenario-complete (verification: `python3 /Users/tumf/.agents/skills/cflx-proposal/scripts/cflx.py validate split-rejecting-from-acceptance --strict` passes)

## Future Work

- If implementation introduces a dedicated rejecting reference document, align orchestration invocations and downstream docs with that new workflow entrypoint
- Evaluate whether rejected/recovered task annotations should be standardized in proposal authoring guidance
