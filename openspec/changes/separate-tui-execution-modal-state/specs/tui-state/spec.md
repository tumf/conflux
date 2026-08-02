## ADDED Requirements

### Requirement: TUI execution and modal interaction state are orthogonal

The TUI MUST represent orchestration execution state independently from transient modal interaction state. Execution state MUST contain only select, running, stopping, stopped, and fatal-error lifecycle modes. QR display, worktree-delete confirmation, and single-change force-kill confirmation MUST be optional modal interactions layered over that execution state and MUST NOT replace or restore it through a captured previous-mode value.

This process-local UI state MUST remain non-durable and MUST NOT become authoritative input for scheduler dispatch, resume routing, acceptance, archive, merge, or next-action selection.

#### Scenario: QR round trip preserves running execution

- **GIVEN** the TUI execution state is running
- **WHEN** the operator opens and closes the QR popup
- **THEN** the execution state remains running throughout the interaction
- **AND** closing the popup does not restore a captured copy of the execution state

#### Scenario: execution transition remains current beneath a modal

- **GIVEN** a modal interaction is visible over running execution
- **WHEN** a typed execution event changes the execution state to stopping or stopped
- **THEN** the underlying execution state reflects that event
- **AND** closing the modal does not regress execution to running

#### Scenario: force-kill cancel does not force running

- **GIVEN** a force-kill confirmation is visible
- **AND** the underlying execution state changes before the operator cancels
- **WHEN** the operator cancels the confirmation
- **THEN** only the modal interaction is cleared
- **AND** the current execution state is preserved

#### Scenario: invalidated confirmation cannot submit stale intent

- **GIVEN** a confirmation modal carries a pending target or worktree action
- **WHEN** an execution or refresh transition invalidates that interaction
- **THEN** the TUI clears the modal and its associated pending payload together
- **AND** a later key event cannot submit the stale action

### Requirement: Bulk mark follows execution lifecycle and modal input ownership

The TUI MUST admit bulk execution-mark input only while the Changes view owns ordinary input, no warning or interaction modal owns input, and the shared operator lifecycle matrix admits the operation. Select, Running, and Stopped execution modes MAY admit bulk marking according to row eligibility and queue-intent rules. Stopping MUST remain immutable, and Error recovery MUST remain owned by explicit retry commands rather than mark mutation.

The TUI MUST consume all key input while an interaction modal is active and MUST NOT route `x` or any other ordinary command to the underlying view. A rejected bulk-mark attempt MUST identify the actual execution condition and MUST NOT describe a modal presentation variant as an execution mode.

#### Scenario: eligible execution modes admit bulk mark

- **GIVEN** the Changes view is active with no modal interaction
- **AND** execution mode is Select, Running, or Stopped
- **WHEN** the operator presses `x`
- **THEN** the TUI applies the shared lifecycle and per-row eligibility rules
- **AND** Running-mode queue intent remains consistent with execution marks

#### Scenario: stopping rejects bulk mark

- **GIVEN** the Changes view is active with no modal interaction
- **AND** execution mode is Stopping
- **WHEN** the operator presses `x`
- **THEN** no execution mark or queue intent changes
- **AND** the TUI reports that bulk mark is unavailable while stopping

#### Scenario: fatal error keeps recovery retry-owned

- **GIVEN** the Changes view is active with no modal interaction
- **AND** execution mode is Error
- **WHEN** the operator presses `x`
- **THEN** no execution mark or queue intent changes
- **AND** the TUI reports that error recovery requires retry

#### Scenario: modal consumes bulk-mark input

- **GIVEN** QR, worktree-delete confirmation, or force-kill confirmation owns input
- **WHEN** the operator presses `x`
- **THEN** the modal handles or consumes the key according to its interaction contract
- **AND** the underlying bulk-mark action does not run
