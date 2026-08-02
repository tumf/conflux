## ADDED Requirements

### Requirement: TUI execution and modal interaction state are orthogonal

The TUI MUST represent orchestration execution state independently from transient modal interaction state. Execution state MUST contain only select, running, stopping, stopped, and fatal-error lifecycle modes. QR display, worktree-delete confirmation, and single-change force-kill confirmation MUST be optional modal interactions layered over that execution state and MUST NOT replace or restore it through a captured previous-mode value.

Each destructive confirmation MUST contain its identity-bearing payload within the typed modal state. The TUI MUST NOT store independently mutable modal and confirmation payload fields whose combinations can become inconsistent. This process-local UI state MUST remain non-durable and MUST NOT become authoritative input for scheduler dispatch, resume routing, acceptance, archive, merge, or next-action selection.

#### Scenario: QR round trip preserves running execution

- **GIVEN** the TUI execution state is running
- **WHEN** the operator opens and closes the QR popup
- **THEN** the execution state remains running throughout the interaction
- **AND** closing the popup does not restore a captured copy of the execution state

#### Scenario: QR survives background execution transition

- **GIVEN** the QR popup is visible over running execution
- **WHEN** a typed execution event changes execution to stopping, stopped, or error while the Web URL remains available
- **THEN** the QR popup remains visible over the latest execution state
- **AND** closing it exposes that latest execution state

#### Scenario: QR invalidates when Web URL disappears

- **GIVEN** the QR popup is visible
- **WHEN** Web monitoring is disabled or the current Web URL is removed
- **THEN** the QR modal is cleared
- **AND** the current execution state remains unchanged

#### Scenario: worktree confirmation survives execution transition

- **GIVEN** a worktree-delete confirmation contains a path and branch identity that remains present and delete-eligible in a fresh worktree observation
- **WHEN** the underlying execution mode changes
- **THEN** the confirmation remains visible
- **AND** cancel or confirm does not restore a captured execution mode

#### Scenario: worktree refresh invalidates stale confirmation

- **GIVEN** a worktree-delete confirmation contains a path and branch identity
- **WHEN** a fresh worktree observation shows that target absent, main, active, already deleting, or bound to a different identity
- **THEN** the typed modal and its payload are cleared atomically
- **AND** a later key event cannot submit the stale delete

#### Scenario: force-kill survives Running to Stopping while target remains active

- **GIVEN** force-kill confirmation targets retryable active work in Running execution
- **WHEN** execution changes to Stopping and the target remains authoritative retryable active work
- **THEN** the force-kill confirmation remains visible
- **AND** canceling it preserves Stopping execution

#### Scenario: force-kill target transition invalidates confirmation

- **GIVEN** force-kill confirmation targets a change
- **WHEN** authoritative state shows the target terminal, dequeued, absent, non-active, non-retryable, or otherwise invalid for stop-and-dequeue
- **THEN** the typed modal and target payload are cleared atomically
- **AND** a later key event cannot submit the stale stop-and-dequeue intent

#### Scenario: confirmation revalidates authoritative state

- **GIVEN** a destructive confirmation remains visible after its target changed between display and confirmation input
- **WHEN** the operator confirms the action
- **THEN** the existing shared operator or repository-backed worktree service revalidates current identity and eligibility before mutation
- **AND** stale identity, failed cancellation, missing termination evidence, timeout, or invalid status does not mutate the invalid target

### Requirement: TUI renders execution base and modal overlay independently

The TUI MUST derive its base status, controls, elapsed-time presentation, and view content from execution and view state. It MUST render valid QR and confirmation interactions as overlays after the base presentation without changing the execution state. It MUST NOT use a fallback that rewrites unsupported or newly introduced state combinations to Select or Running.

#### Scenario: worktree confirmation overlays Error base

- **GIVEN** a still-valid worktree-delete confirmation is visible
- **AND** the underlying execution mode becomes Error
- **WHEN** the TUI renders the next frame
- **THEN** the base presentation retains Error status and retry semantics
- **AND** the worktree confirmation is rendered above it

#### Scenario: force-kill overlays Stopping base while valid

- **GIVEN** force-kill confirmation remains valid after execution enters Stopping
- **WHEN** the TUI renders the next frame
- **THEN** the base presentation retains Stopping status and controls
- **AND** the force-kill confirmation is rendered above it

#### Scenario: invalidated force-kill reveals terminal base

- **GIVEN** a force-kill confirmation is invalidated by a terminal or non-active target transition
- **WHEN** the TUI renders the next frame
- **THEN** no force-kill overlay is rendered
- **AND** the current execution base is rendered without conversion to Running

### Requirement: Bulk mark follows execution lifecycle and modal input ownership

The TUI MUST admit bulk execution-mark input only while the Changes view owns ordinary input, no warning or interaction modal owns input, and the shared operator lifecycle matrix admits the operation. Select, Running, and Stopped execution modes MAY admit bulk marking according to row eligibility and queue-intent rules. Stopping MUST remain immutable, and Error recovery MUST remain owned by explicit retry commands rather than mark mutation.

The TUI MUST consume all key input while a warning or interaction modal is active and MUST NOT route `x` or any other ordinary command to the underlying view. A rejected bulk-mark attempt MUST identify the actual execution condition and MUST NOT describe a modal presentation variant as an execution mode.

#### Scenario: eligible execution modes admit bulk mark

- **GIVEN** the Changes view is active with no warning or interaction modal
- **AND** execution mode is Select, Running, or Stopped
- **WHEN** the operator presses `x`
- **THEN** the TUI applies the shared lifecycle and per-row eligibility rules
- **AND** Running-mode queue intent remains consistent with execution marks

#### Scenario: stopping rejects bulk mark

- **GIVEN** the Changes view is active with no warning or interaction modal
- **AND** execution mode is Stopping
- **WHEN** the operator presses `x`
- **THEN** no execution mark or queue intent changes
- **AND** the TUI reports that bulk mark is unavailable while stopping

#### Scenario: fatal error keeps recovery retry-owned

- **GIVEN** the Changes view is active with no warning or interaction modal
- **AND** execution mode is Error
- **WHEN** the operator presses `x`
- **THEN** no execution mark or queue intent changes
- **AND** the TUI reports that error recovery requires retry

#### Scenario: overlay consumes bulk-mark input

- **GIVEN** a warning popup, QR, worktree-delete confirmation, or force-kill confirmation owns input
- **WHEN** the operator presses `x`
- **THEN** the overlay handles or consumes the key according to its interaction contract
- **AND** the underlying bulk-mark action does not run
