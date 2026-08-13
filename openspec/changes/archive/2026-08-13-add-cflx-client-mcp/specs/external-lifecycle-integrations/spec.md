## ADDED Requirements

### Requirement: Execution completion sinks remain distinct from process lifecycle adapters

Execution-scoped completion sinks MUST be separate from the optional process lifecycle adapter. Lifecycle adapters continue to observe semantic process state such as idle, working, blocked, and stopping. Completion sinks identify one admitted execution and use repository-verifiable terminal evidence.

Neither integration may control workflow routing. Configuring or delivering one MUST NOT require configuring the other.

#### Scenario: Persistent TUI idle is not proposal completion

- **GIVEN** a long-lived TUI lifecycle adapter observes an `idle` transition
- **AND** execution `exec-a` has not reached repository-verifiable terminal success
- **WHEN** lifecycle state is published
- **THEN** the lifecycle adapter may receive `idle`
- **AND** the execution completion sink does not receive `completed`

#### Scenario: Completion does not stop lifecycle reporting

- **GIVEN** execution `exec-a` completes while the TUI remains active
- **WHEN** its completion sink receives `completed`
- **THEN** the process lifecycle adapter remains attached
- **AND** later TUI working or blocked transitions continue to be reported
