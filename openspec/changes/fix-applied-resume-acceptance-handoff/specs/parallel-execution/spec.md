## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop

Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace. The acceptance loop SHALL parse stdout to determine pass/fail/continue/blocked, and MUST NOT use exit code to determine acceptance verdict.

When resuming a workspace that has not completed archive, the orchestrator SHALL route `Applied` or `Archiving` workspaces without a durable acceptance `passed` state for the current revision back to acceptance before archive. A resume cycle that selects `ResumeAction::Acceptance` MUST NOT hand off to archive unless acceptance for the current revision has returned `Pass` or an equivalent durable acceptance-pass record has been confirmed during that cycle.

A durable acceptance state of `failed`, `running`, `pending`, or missing for the current revision MUST be treated as not archive-ready. Archive guardrails MAY reject such a workspace as a final defense, but dispatch control flow MUST prevent archive entry before that guard is reached.

#### Scenario: Applied workspace with failed durable state reruns acceptance before archive

- **GIVEN** a resumed parallel workspace is detected as `Applied`
- **AND** the current revision has a durable acceptance state of `failed`
- **WHEN** resume routing is evaluated
- **THEN** the workspace is routed to acceptance
- **AND** archive is not started for that cycle

#### Scenario: Applied workspace with missing durable pass does not hand off to archive

- **GIVEN** a resumed parallel workspace is detected as `Applied`
- **AND** no durable acceptance `passed` state exists for the current revision
- **WHEN** the orchestrator resumes execution
- **THEN** acceptance is executed for that revision
- **AND** archive is not entered until acceptance returns `Pass`

#### Scenario: Applied workspace with durable pass can continue archive

- **GIVEN** a resumed parallel workspace is detected as `Applied`
- **AND** a durable acceptance `passed` state exists for the current revision
- **WHEN** resume routing is evaluated
- **THEN** the workspace may continue to archive
- **AND** acceptance may be skipped for that cycle

#### Scenario: Resume log and executed phase stay consistent

- **GIVEN** the orchestrator logs `state=Applied -> Acceptance` for a resumed workspace
- **WHEN** that resume cycle begins execution
- **THEN** acceptance execution is started in the same cycle
- **AND** archive is not attempted before acceptance completion
