## MODIFIED Requirements

### Requirement: Max Iterations Configuration

The orchestrator SHALL support a configurable maximum iteration limit to prevent accidental infinite Apply loops. The configured `max_iterations` value SHALL be enforced by one per-change active-run cumulative Apply-dispatch counter shared by serial CLI, TUI, and parallel execution. The same counter SHALL survive every Apply entry for that change in the current process, including Acceptance FAIL-to-Apply cycles, ordinary command-failure recovery, task-format repair, escalation, and final-commit rejection repair. A fresh process run SHALL reset the counter; no log or durable retry artifact SHALL restore it.

A positive value SHALL reserve one count immediately before each configured Apply-agent dispatch. Command queue transport retries within that dispatch SHALL NOT increment the outer count. Existing CLI/TUI workflow-loop counters MUST NOT impose an independent Apply ceiling. A value of `0` SHALL disable only the numeric outer limit; cancellation, completion, fresh repository/handoff evaluation, progress/stall detection, permission holds, and other owned error policies remain active.

#### Scenario: Configure max iterations in config file

- **GIVEN** `.cflx.jsonc` contains:
  ```jsonc
  {
    "max_iterations": 100
  }
  ```
- **WHEN** one change enters Apply through serial CLI, TUI, or parallel execution
- **THEN** that change starts no more than 100 configured Apply-agent dispatches during the active process run
- **AND** command-failure recovery, Acceptance FAIL-to-Apply, task-format repair, escalation, and final-commit repair use the same per-change total
- **AND** another change owns an independent total

#### Scenario: Default limit when not configured

- **GIVEN** `max_iterations` is not set in config
- **WHEN** the orchestrator runs
- **THEN** the default limit of 50 per-change Apply dispatches is applied
- **AND** the owner stops before reserving dispatch 51 for that change

#### Scenario: CLI flag overrides config

- **GIVEN** config file has `"max_iterations": 100`
- **WHEN** user runs `cflx run --max-iterations 50`
- **THEN** each change starts no more than 50 Apply dispatches in that process run
- **AND** CLI value takes precedence over config file

#### Scenario: Zero disables numeric limit but not stall policy

- **GIVEN** `max_iterations` is set to `0`
- **WHEN** configured Apply commands repeatedly fail without task or Git progress
- **THEN** no iteration-count limit is applied
- **AND** each result still reaches fresh repository/handoff, progress, permission, and stall evaluation
- **AND** existing no-progress diagnosis, escalation, or stalled termination remains capable of stopping the loop

#### Scenario: Command queue retries remain internal to one Apply dispatch

- **GIVEN** one configured Apply command is retried by `CommandQueue`
- **WHEN** the internal command retry succeeds or exhausts
- **THEN** all command queue invocations belong to one outer Apply dispatch
- **AND** only a newly dispatched history-backed Apply iteration increments the per-change `max_iterations` count

#### Scenario: Warning when approaching limit

- **GIVEN** `max_iterations` is set to `100`
- **WHEN** one change's cumulative Apply dispatch count reaches 80
- **THEN** the sole counter owner emits `Approaching max iterations: 80/100` once for that threshold crossing
- **AND** no independent CLI, TUI, or inner-loop owner duplicates the warning

#### Scenario: Process restart resets the active-run count

- **GIVEN** one change consumed Apply dispatches before process termination
- **WHEN** Conflux starts a fresh process run and workspace evidence routes that change to Apply
- **THEN** the per-change active-run count starts from zero
- **AND** no prior log, report, provider session, or retry checkpoint restores the previous count

### Requirement: Iteration Limit Finish Status

When one change exhausts its positive per-change Apply-dispatch budget, the sole budget owner SHALL return a typed `iteration_limit` outcome containing the change ID, exact cumulative dispatch count, and latest bounded actionable diagnostic. Serial CLI, TUI, and parallel run boundaries SHALL stop the affected execution consistently and SHALL invoke the existing `on_finish` hook ownership exactly once with `status = iteration_limit` and the exact count. The exhaustion MUST NOT be reduced to a generic agent-command failure before finish-status routing.

#### Scenario: Hook receives iteration_limit status

- **GIVEN** `max_iterations` is set to `10`
- **AND** `on_finish` hook is configured
- **WHEN** the affected change has completed 10 Apply dispatches without an owned completion, hold, or stall outcome
- **THEN** no 11th Apply dispatch starts
- **AND** `on_finish` is called exactly once with `{status}` = `iteration_limit`
- **AND** `{iteration}` = `10`
- **AND** the operator-visible diagnostic includes the affected change and latest bounded actionable failure

#### Scenario: All frontends preserve typed iteration-limit ownership

- **GIVEN** the same per-change budget is exhausted through serial CLI, TUI, or parallel execution
- **WHEN** the outcome crosses the respective run boundary
- **THEN** each boundary preserves `iteration_limit` rather than reclassifying it as an ordinary command crash
- **AND** no inner and outer owner invoke `on_finish` twice
