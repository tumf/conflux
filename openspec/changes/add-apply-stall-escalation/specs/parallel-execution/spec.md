## MODIFIED Requirements

### Requirement: Incomplete apply does not get success-equivalent terminate treatment

A parallel-mode apply command that leaves `tasks.md` incomplete and does not produce a recognized handoff artifact MUST continue to follow failure/retry/stall policy.

When repeated retries produce consecutive empty WIP commits, the runtime MAY enter a bounded escalation phase before final stall classification, but it MUST NOT treat the run as completed solely because escalation or diagnosis occurred.

#### Scenario: incomplete apply still cannot bypass completion via escalation

- **GIVEN** parallel-mode apply leaves `tasks.md` incomplete
- **AND** no `REJECTED.md` or other success-equivalent handoff artifact exists
- **WHEN** the runtime enters escalation retries for consecutive empty WIP commits
- **THEN** the runtime still treats the change as incomplete
- **AND** acceptance/archive handoff does not begin
- **AND** final outcome remains subject to failure/retry/stall policy

### Requirement: Empty-WIP apply escalation before stall finalization

When empty WIP commits accumulate during apply for a change, the runtime SHALL be able to replace late retries with a stronger configured apply escalation command before final stall classification.

Escalation usage MUST be bounded by configuration and MUST remain runtime-ephemeral rather than durable workflow-control state.

#### Scenario: empty WIP retries switch from normal apply to escalation apply

- **GIVEN** `stall_detection.threshold = 5`
- **AND** `stall_detection.apply_escalation_after_empty_wip = 3`
- **AND** `stall_detection.apply_escalation_max_uses_per_stall = 2`
- **AND** `apply_escalation_command` is configured
- **WHEN** a change reaches its third consecutive empty WIP commit during apply retries
- **THEN** the next eligible retry uses `apply_escalation_command` instead of `apply_command`
- **AND** at most two escalation retries are used during that stall sequence

#### Scenario: empty WIP counter reset returns retry policy to normal apply

- **GIVEN** a change has already entered escalation retries for a stall sequence
- **AND** a later apply attempt produces a non-empty WIP commit
- **WHEN** the next retry decision is made
- **THEN** the empty-WIP streak resets
- **AND** escalation usage for that stall sequence resets
- **AND** subsequent retries use normal `apply_command` unless a new streak reaches the configured trigger again

### Requirement: Stall diagnosis runs once before final empty-WIP stall

When the final empty-WIP stall threshold is reached after escalation opportunities are exhausted, the runtime SHALL execute a dedicated stall diagnosis command once before returning the final stall outcome.

Diagnosis output is supplemental evidence only and MUST NOT replace the primary empty-WIP stall reason.

#### Scenario: diagnosis runs once before final stall

- **GIVEN** a change has exhausted its allowed escalation retries
- **AND** the consecutive empty WIP count reaches the configured final stall threshold
- **AND** `apply_stall_diagnose_command` is configured
- **WHEN** the runtime finalizes the empty-WIP stall
- **THEN** it executes `apply_stall_diagnose_command` exactly once
- **AND** it records diagnosis output as diagnostic evidence/logging
- **AND** the final stall outcome still reports the empty-WIP stall as the primary reason

#### Scenario: diagnose failure does not hide the original stall cause

- **GIVEN** the runtime reaches final empty-WIP stall classification
- **AND** `apply_stall_diagnose_command` fails
- **WHEN** the runtime reports the result
- **THEN** the original empty-WIP stall remains the primary failure/outcome reason
- **AND** diagnose failure is surfaced only as supplemental warning/error evidence

#### Scenario: unset escalation or diagnose commands preserve current behavior

- **GIVEN** the runtime uses the existing stall detector configuration without new optional commands
- **WHEN** consecutive empty WIP commits reach the final threshold
- **THEN** the runtime behaves exactly as before this change
- **AND** no escalation or diagnosis command is attempted
