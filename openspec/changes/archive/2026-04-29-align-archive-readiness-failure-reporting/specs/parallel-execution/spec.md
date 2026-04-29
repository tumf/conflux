## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop

Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace. The acceptance loop SHALL parse machine-readable acceptance output to determine pass/fail/continue/blocked, and MUST NOT use exit code to determine acceptance verdict.

When acceptance determines that final archive commit readiness is blocked by a commit-path failure, strict validation failure, or other archive-start blocker, that blocker MUST remain the primary failure context for downstream archive handling. Later archive filesystem verification MAY add supplemental context, but it MUST NOT replace or erase the earlier root cause.

#### Scenario: archive prerequisite blocker remains visible after archive verification failure

- **GIVEN** acceptance or archive guidance has already identified a concrete archive-start blocker for change `alpha`
- **AND** a later archive attempt still leaves `openspec/changes/alpha` in place
- **WHEN** the runtime reports the archive failure to history, logs, or the user
- **THEN** the reported failure still includes the earlier blocker summary
- **AND** the file-state verification result is reported only as supplemental context
