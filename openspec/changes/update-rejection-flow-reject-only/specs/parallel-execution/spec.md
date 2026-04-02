## MODIFIED Requirements

### Requirement: ParallelRunService rejection flow on blocked execution

ParallelRunService SHALL treat a confirmed blocked verdict as a terminal rejection after the base branch has recorded `openspec/changes/<change_id>/REJECTED.md`. Parallel rejection handling SHALL NOT rely on `openspec resolve <change_id>` and SHALL NOT merge additional worktree files into the base branch. After the reject marker commit succeeds, the runtime SHALL emit a rejected result, preserve the rejection reason, and clean up the rejected worktree.

#### Scenario: parallel rejected result is driven by REJECTED marker commit

- **GIVEN** acceptance confirms a blocked verdict in parallel mode
- **WHEN** the rejection flow commits `openspec/changes/fix-auth/REJECTED.md` on the base branch
- **THEN** the workspace result is returned as rejected
- **AND** no further resolve step is required to finalize the rejection

#### Scenario: rejected worktree changes are not merged to base

- **GIVEN** a rejected worktree contains code, tasks, and spec changes in addition to `REJECTED.md`
- **WHEN** the rejection flow completes
- **THEN** the base branch receives only `openspec/changes/fix-auth/REJECTED.md`
- **AND** the remaining worktree-only files are discarded with worktree cleanup
