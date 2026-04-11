## MODIFIED Requirements

### Requirement: Durable acceptance state gates archive on the current revision

Parallel execution MUST persist enough workspace-local acceptance state to distinguish `pending`, `running`, `passed`, and non-pass terminal outcomes for the latest apply revision.

Archive MUST NOT start unless the latest acceptance state for the current workspace revision is durably recorded as `passed`.

When acceptance execution mutates the worktree or creates commits, the durable acceptance state recorded at the end of the acceptance attempt MUST use the workspace HEAD that exists after the acceptance command completes, not the revision captured before the command started.

If the orchestrator restarts after acceptance started but before a final verdict is recorded, the resumed workspace MUST treat that acceptance attempt as incomplete and MUST rerun acceptance before archive.

#### Scenario: Acceptance pass records final head after acceptance-created commit

- **GIVEN** a parallel workspace starts an acceptance attempt at revision `rev-start`
- **AND** the acceptance command creates a commit so that workspace HEAD becomes `rev-final` before it exits
- **AND** the acceptance verdict is `PASS`
- **WHEN** the orchestrator durably records the acceptance result
- **THEN** the durable acceptance state is stored as `passed`
- **AND** its recorded revision is `rev-final`
- **AND** archive guard for the same workspace revision treats the workspace as archive-ready

#### Scenario: Non-pass acceptance records final head after acceptance-created commit

- **GIVEN** a parallel workspace starts an acceptance attempt at revision `rev-start`
- **AND** the acceptance command creates a commit so that workspace HEAD becomes `rev-final` before it exits
- **AND** the final acceptance verdict is one of `FAIL`, `CONTINUE`, `BLOCKED`, or command failure
- **WHEN** the orchestrator durably records the acceptance result
- **THEN** the durable acceptance state revision is `rev-final`
- **AND** archive is not considered ready for `rev-final`

#### Scenario: Acceptance without head change keeps the same revision

- **GIVEN** a parallel workspace starts an acceptance attempt at revision `rev-same`
- **AND** the acceptance command does not change workspace HEAD
- **AND** the acceptance verdict is recorded
- **WHEN** the orchestrator saves the durable acceptance state
- **THEN** the durable acceptance state revision remains `rev-same`
- **AND** existing archive gating semantics are preserved
