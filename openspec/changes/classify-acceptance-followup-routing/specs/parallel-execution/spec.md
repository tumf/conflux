## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop

When acceptance returns FAIL, the next lifecycle step SHALL be chosen from the recorded follow-up classification rather than from raw unchecked checkbox count alone.

Acceptance-generated follow-up findings that require repository implementation work SHALL be treated as apply-driving remediation. Acceptance-generated follow-up findings that only describe archive-readiness blockers, commit-path blockers, or external unblock conditions SHALL be treated as blocker-only follow-up and SHALL NOT by themselves force an apply rerun that can only produce empty WIP snapshots.

When blocker-only follow-up remains after implementation tasks are otherwise complete, the runtime SHALL avoid re-entering apply solely to satisfy raw progress accounting. Instead, it SHALL route to the resumable blocked/non-apply hold path defined for that workspace lifecycle.

#### Scenario: blocker-only follow-up does not re-enter apply
- **GIVEN** change `alpha` has all implementation tasks completed
- **AND** `Acceptance #1 Failure Follow-up` contains only archive-readiness or commit-path blocker notes
- **WHEN** acceptance fail handling or resumed workspace routing chooses the next lifecycle step
- **THEN** the runtime does not start apply solely because unchecked implementation progress is no longer zero
- **AND** the change enters blocked/non-apply hold instead of accumulating empty WIP apply snapshots

#### Scenario: remediation follow-up still returns to apply
- **GIVEN** change `beta` has an acceptance follow-up that requires repository code or test changes
- **WHEN** the runtime evaluates the follow-up classification
- **THEN** the change returns to apply
- **AND** the remediation task remains visible as apply-driving work until the repo change is made

### Requirement: Resumed worktree routing preserves execution order

Existing workspaces SHALL be classified for resume using worktree state plus follow-up kind, not raw total checkbox count alone.

A resumed workspace in `Applied` state with only blocker-only follow-up remaining MUST NOT log or route as though implementation tasks are incomplete. A resumed workspace with implementation remediation remaining MAY route to apply. A resumed workspace with no apply-driving follow-up and no durable acceptance pass MAY route to acceptance.

#### Scenario: resumed applied workspace distinguishes blocker-only follow-up from implementation work
- **GIVEN** a resumed workspace for change `gamma` is detected as `Applied`
- **AND** `Implementation Tasks` are complete
- **AND** the latest acceptance follow-up contains only blocker-only notes
- **WHEN** resume classification is performed
- **THEN** the runtime does not emit an `implementation tasks incomplete` routing reason
- **AND** apply is not selected as the first resumed step solely because the blocker notes exist
