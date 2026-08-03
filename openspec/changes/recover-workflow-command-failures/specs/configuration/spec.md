## MODIFIED Requirements

### Requirement: Max Iterations Configuration

The orchestrator SHALL support a configurable maximum iteration limit to prevent accidental infinite Apply loops. The configured `max_iterations` value SHALL be the single outer Apply-attempt budget used by serial and parallel execution. A positive value SHALL count every dispatched Apply attempt, including ordinary implementation, command-failure recovery, task-format repair, escalation, and final-commit rejection repair. Command queue transport retries within one dispatched Apply attempt SHALL NOT increment this outer count. A value of `0` SHALL disable only the numeric outer limit; cancellation, completion, stall, permission, blocker, and other owned error policies remain active.

#### Scenario: Configure max iterations in config file

- **GIVEN** `.cflx.jsonc` contains:
  ```jsonc
  {
    "max_iterations": 100
  }
  ```
- **WHEN** the orchestrator runs Apply
- **THEN** the loop starts no more than 100 outer Apply attempts
- **AND** command-failure recovery and final-commit repair use the same total
- **AND** the finish status is `iteration_limit` when the total is exhausted
- **AND** a log message indicates `Max iterations (100) reached`

#### Scenario: Default limit when not configured

- **GIVEN** `max_iterations` is not set in config
- **WHEN** the orchestrator runs
- **THEN** the default limit of 50 outer Apply attempts is applied
- **AND** the loop stops before dispatching attempt 51 if not complete

#### Scenario: CLI flag overrides config

- **GIVEN** config file has `"max_iterations": 100`
- **WHEN** user runs `cflx run --max-iterations 50`
- **THEN** the loop starts no more than 50 outer Apply attempts
- **AND** CLI value takes precedence over config file

#### Scenario: Zero disables numeric limit

- **GIVEN** `max_iterations` is set to `0`
- **WHEN** the orchestrator runs Apply
- **THEN** no iteration-count limit is applied
- **AND** multiple command-failure or incomplete-progress attempts may continue
- **AND** the loop still stops for completion, cancellation, stall, permission hold, blocker handoff, or another owned terminal error

#### Scenario: Command queue retries remain internal to one Apply attempt

- **GIVEN** one configured Apply command is retried by `CommandQueue`
- **WHEN** the internal command retry succeeds or exhausts
- **THEN** all command queue invocations belong to one outer Apply attempt
- **AND** only a newly dispatched history-backed Apply iteration increments the `max_iterations` count

#### Scenario: Warning when approaching limit

- **GIVEN** `max_iterations` is set to `100`
- **WHEN** outer Apply attempt count reaches 80 (80% of limit)
- **THEN** a warning log is emitted: `Approaching max iterations: 80/100`
