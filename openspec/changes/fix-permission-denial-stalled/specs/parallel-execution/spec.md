## MODIFIED Requirements

### Requirement: Permission Auto-Reject Handling

When permission or local policy denial is detected during apply, the system SHALL distinguish transient/progressing denials from repeated unresolved denials.

A permission/policy denial SHALL become a non-terminal `stalled` hold only after the same unresolved denial recurs without repository-visible progress that would indicate the agent can continue self-healing within the workspace.

The system MUST NOT label this condition as dependency `blocked`.

#### Scenario: first permission auto-reject remains retryable

- **GIVEN** apply output contains `permission requested` and `auto-rejecting`
- **AND** this is the first observation of that denial signature for the current apply/acceptance cycle
- **WHEN** the apply loop evaluates the output
- **THEN** the change is not immediately recorded as `stalled`
- **AND** the runtime may retry according to existing apply retry policy
- **AND** the denial signature is recorded only as non-authoritative observability/retry context unless repository-visible state later makes the stalled hold derivable

#### Scenario: permission auto-reject with progress remains retryable

- **GIVEN** apply output contains a permission/policy denial
- **AND** task progress, tracked workspace files, or other repository-visible progress changed after the attempt
- **WHEN** the apply loop evaluates the output
- **THEN** the change is not recorded as `stalled` solely because the denial occurred
- **AND** apply retry may continue because the agent may still be self-healing within the workspace

#### Scenario: repeated unresolved permission auto-reject becomes stalled

- **GIVEN** apply output contains a permission/policy denial with the same denied target or equivalent denial signature as a prior attempt
- **AND** no repository-visible progress occurred between the repeated denial observations
- **WHEN** the apply loop evaluates the repeated unresolved denial
- **THEN** the change is recorded as `stalled`
- **AND** apply retry does not continue for that denial
- **AND** stall detection via empty WIP commits is skipped for that change once the repeated permission blocker is classified
- **AND** the recorded reason includes rejected paths or commands and permission guidance

### Requirement: Acceptance Permission Denial Handling

When permission or local policy denial is detected during acceptance, the system SHALL distinguish ordinary acceptance failures from repeated unresolved permission/policy blockers.

A permission/policy denial in acceptance command output, command error text, or FAIL findings SHALL become a non-terminal `stalled` hold only after the same unresolved denial recurs without repository-visible progress or changed acceptance evidence that would indicate the agent can continue self-healing.

Normal acceptance failures that do not match a repeated unresolved permission/policy denial MUST continue to use the existing acceptance follow-up and apply retry behavior.

#### Scenario: first acceptance permission denial remains retryable or reportable

- **GIVEN** acceptance output, command error text, or FAIL findings contain a permission/policy denial
- **AND** this is the first observation of that denial signature for the current acceptance follow-up cycle
- **WHEN** dispatch evaluates the acceptance result
- **THEN** the runtime does not immediately record the change as `stalled` solely due to the first denial
- **AND** existing acceptance retry, command-failure, or follow-up behavior may continue according to the non-blocker result path

#### Scenario: repeated unresolved acceptance permission denial becomes stalled

- **GIVEN** acceptance output, command error text, or FAIL findings contain a permission/policy denial
- **AND** the same denied target or equivalent denial signature was observed in a prior acceptance/apply cycle
- **AND** no repository-visible progress or changed acceptance evidence occurred between observations
- **WHEN** dispatch evaluates the repeated unresolved denial
- **THEN** the change is recorded as a non-terminal `stalled` hold
- **AND** dispatch does not append ordinary implementation follow-up tasks for that denial
- **AND** dispatch does not return to apply for that denial
- **AND** dispatch does not return terminal `error` for that denial

#### Scenario: normal acceptance failure remains retryable

- **GIVEN** acceptance FAIL findings describe implementation defects and do not match a repeated unresolved permission/policy denial
- **WHEN** dispatch handles the FAIL result
- **THEN** follow-up tasks are recorded as before
- **AND** the runtime returns to apply as before

### Requirement: Repeated Permission Blockers Avoid Cycle-Limit Degradation

When a repeated unresolved permission/policy denial is classified as a stalled execution blocker, the runtime SHALL stop routing that change through the apply/acceptance retry loop for that blocker.

#### Scenario: repeated permission denial does not become max-cycle error

- **GIVEN** the same unresolved permission/policy denial has recurred without repository-visible progress
- **WHEN** the runtime classifies it as a stalled execution blocker
- **THEN** the change is displayed as `stalled`
- **AND** the repeated blocker is not allowed to continue until `Max apply+acceptance cycles reached`
- **AND** the terminal state remains non-error so the operator can fix permissions and resume
