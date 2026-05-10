## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop

Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace. The acceptance loop SHALL parse stdout to determine pass/fail/continue/stalled-hold outcomes, and MUST NOT use exit code as the acceptance verdict when a canonical verdict is present.

Acceptance execution MUST NOT create a workspace-root `ACCEPTANCE_REPORT.json` artifact for PASS, command failure, FAIL, CONTINUE, or stalled-hold outcomes. Acceptance outcomes MAY be recorded in in-memory runtime history, events, or non-authoritative observability logs, but the runtime MUST NOT write a new workspace-root report file as acceptance completion evidence.

Archive and resume routing MUST NOT depend on `ACCEPTANCE_REPORT.json`. The absence of that file MUST NOT prevent existing acceptance history recording, event emission, or subsequent routing behavior that is otherwise valid for the workspace file/git state.

When acceptance returns a stalled-hold compatibility verdict for infrastructure, external dependency, missing non-mockable credential, or pending-verification evidence, parallel execution SHALL record a non-terminal stalled hold and SHALL NOT invoke terminal rejection flow solely because of that verdict.

#### Scenario: acceptance pass does not create report artifact

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the acceptance command emits a canonical PASS verdict
- **WHEN** acceptance finalizes the pass
- **THEN** the runtime records the acceptance attempt in existing acceptance history
- **AND** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** the pass result does not require any workspace-root report file for later routing

#### Scenario: command failure does not create misleading pass report

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the acceptance command exits unsuccessfully without a finalized canonical verdict
- **WHEN** acceptance returns a command-failure result
- **THEN** the runtime records the failed attempt through existing history paths
- **AND** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** no JSON artifact is written that could describe the failed attempt as `pass`

#### Scenario: non-pass verdicts do not create report artifact

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the final acceptance verdict is FAIL, CONTINUE, or a stalled-hold compatibility verdict
- **WHEN** acceptance finalizes that outcome
- **THEN** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** any required follow-up information is carried by the existing result, findings, events, or history rather than a workspace-root report file

#### Scenario: infrastructure stalled hold does not become terminal rejection

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the change is structurally valid and active
- **AND** acceptance emits a stalled-hold compatibility verdict because a required Docker image pull failed with DNS or network timeout and the image is not locally available
- **WHEN** parallel dispatch handles the acceptance result
- **THEN** `alpha` is recorded as a non-terminal stalled hold
- **AND** base branch `openspec/changes/alpha/REJECTED.md` is not created
- **AND** terminal rejection flow is not invoked solely from the infrastructure blocker
- **AND** the worktree remains available for later resume after Docker image or network availability is restored

#### Scenario: pending managed verification job is non-terminal

- **GIVEN** a required acceptance verification uses a managed job such as agent-exec
- **AND** the job is still running or lacks terminal pass/fail evidence
- **WHEN** acceptance or dispatch classifies the verification state
- **THEN** the change is not marked as accepted
- **AND** the change is not terminally rejected
- **AND** the condition is recorded as pending verification or a non-terminal stalled hold with next action to re-check the job or wait for terminal evidence

### Requirement: ParallelRunService rejection flow on blocked execution

After rejecting review completes, the runtime SHALL emit a `RejectionReviewCompleted` execution event with one of `Confirm`, `Resume`, or `Block` outcome. The reducer SHALL use this event to drive the `Rejecting → Rejected`, `Rejecting → Applying`, or `Rejecting → Stalled` transition.

The runtime SHALL NOT leave a change in the `Rejecting` activity stage after rejection review has produced a verdict. If rejection review encounters an error, the runtime SHALL emit a `RejectionReviewFailed` event to transition the change to `Error` terminal state.

#### Scenario: blocked rejection review emits completion event and returns to stalled state

- **GIVEN** a change is in the `Rejecting` activity stage
- **AND** rejection review returns `REJECTION_REVIEW: BLOCK`
- **WHEN** the blocking handoff completes
- **THEN** a `RejectionReviewCompleted` event with `Block` outcome is emitted
- **AND** the reducer transitions the change to non-terminal stalled state
- **AND** base branch `openspec/changes/<change_id>/REJECTED.md` is not created
- **AND** the worktree remains available for later resume

#### Scenario: confirmed rejection remains terminal

- **GIVEN** a change is in the `Rejecting` activity stage
- **AND** rejection review returns `REJECTION_REVIEW: CONFIRM` with terminal evidence that the change premise is invalid, obsolete, contradictory, or constitution-violating
- **WHEN** rejection flow completes
- **THEN** base branch `openspec/changes/<change_id>/REJECTED.md` is created
- **AND** the change is dequeued
- **AND** the reducer marks the change terminal rejected
