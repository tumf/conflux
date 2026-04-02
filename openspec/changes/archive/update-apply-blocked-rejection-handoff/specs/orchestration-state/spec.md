## MODIFIED Requirements

### Requirement: Rejection Flow Execution

The system SHALL execute a rejection flow when acceptance returns a `Blocked` verdict, including blocked verdicts that originated from apply execution through a rejection proposal file. Apply execution MAY generate `openspec/changes/<change_id>/REJECTED.md` as a rejection proposal when it encounters an implementation blocker that prevents completion. This proposal file SHALL NOT become a terminal rejection by itself. Acceptance SHALL review the blocker and decide whether to confirm the rejection. Only after acceptance confirms the blocked verdict SHALL the runtime treat the change as rejected, commit `REJECTED.md` on the base branch, run `openspec resolve <change_id>`, and delete the worktree.

#### Scenario: apply-generated rejection proposal requires acceptance confirmation

- **GIVEN** apply execution writes `openspec/changes/fix-auth/REJECTED.md` because of an implementation blocker
- **WHEN** acceptance has not yet confirmed the blocked verdict
- **THEN** the change is not yet in `Rejected` terminal state
- **AND** no rejection flow commit is created on the base branch

#### Scenario: acceptance-confirmed apply blocker transitions to rejected terminal state

- **GIVEN** apply execution has generated `openspec/changes/fix-auth/REJECTED.md`
- **AND** acceptance confirms the blocked verdict
- **WHEN** the rejection flow completes
- **THEN** the terminal state becomes `Rejected` with the rejection reason
- **AND** the derived display status is `rejected`
- **AND** the change cannot be re-queued via `AddToQueue`
