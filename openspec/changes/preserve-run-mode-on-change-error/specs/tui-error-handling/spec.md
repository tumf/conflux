## MODIFIED Requirements

### Requirement: Change-Level Processing Errors Do Not Force App Error Mode

When a change emits `ProcessingError`, Conflux SHALL record the target change as change-level `Error`, retain its diagnostic evidence, and revoke stale execution intent according to the existing change-level reconciliation rules. The event SHALL NOT transition the process-wide Core or TUI execution mode to `Error`. Every process-mode projection SHALL preserve the mode that existed before the event; only a typed fatal global error may enter process-wide Error.

#### Scenario: One change fails while unrelated work remains active

- **GIVEN** the authoritative process mode and TUI execution mode are `Running`
- **AND** changes `alpha` and `beta` are eligible participants in the run
- **WHEN** `ProcessingError` is dispatched for `alpha`
- **THEN** `alpha` SHALL transition to change-level `Error` with retained diagnostic evidence
- **AND** `alpha`'s stale execution mark SHALL be revoked
- **AND** the authoritative process mode and TUI execution mode SHALL remain `Running`
- **AND** unrelated eligible mark state SHALL remain independently mutable

#### Scenario: Bulk mark remains available after a change-local failure

- **GIVEN** `alpha` has entered change-level `Error` from `ProcessingError`
- **AND** the process remains `Running`
- **AND** `beta` is eligible for Running-mode bulk mark mutation
- **WHEN** the operator presses `x`
- **THEN** the existing Running-mode bulk mark plan SHALL be applied to eligible rows
- **AND** the TUI SHALL NOT report `Bulk mark (x) is unavailable in Error mode: recovery is owned by retry`
- **AND** `alpha` SHALL remain governed by existing explicit retry rules

#### Scenario: Change-local error preserves every existing process mode

- **GIVEN** the process mode is one of `Select`, `Running`, `Stopping`, `Stopped`, or `Error`
- **WHEN** `ProcessingError` is dispatched for one change
- **THEN** the process mode SHALL remain unchanged
- **AND** the target change SHALL still receive its change-level Error transition

#### Scenario: Fatal global error remains process-wide

- **GIVEN** orchestration encounters a fatal failure that stops or invalidates the run
- **WHEN** `ExecutionEvent::Error` is dispatched
- **THEN** the authoritative process mode and TUI execution mode SHALL become `Error`
- **AND** ordinary bulk execution-mark mutation SHALL remain unavailable until fatal recovery
- **AND** change-local `ProcessingError` handling SHALL NOT downgrade this fatal transition

<!-- Expected canonical result after archive: the existing change-level ProcessingError requirement will explicitly bind Core, TUI frame adoption, mark controls, and fatal-control behavior to the typed event scope. -->
