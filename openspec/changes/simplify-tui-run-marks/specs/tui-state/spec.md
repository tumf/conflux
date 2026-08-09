## MODIFIED Requirements

### Requirement: Bulk mark follows execution lifecycle and modal input ownership

The TUI MUST admit single-row and bulk execution-mark input for visible non-terminal rows in Select, Running, Stopping, Stopped, and Error execution modes. Execution lifecycle timing, active/retry/wait status, apply-iteration-limit evidence, and current parallel eligibility MUST NOT make a pre-archive execution mark immutable.

An execution mark is process-local next-run target intent only. Mark mutation MUST NOT mutate queue intent, stop or dequeue current work, issue cancellation, create retry or resolve intent, run hooks, wake a scheduler, or change process mode. Current-state run eligibility SHALL be evaluated at final start/retry admission instead.

The TUI MUST consume all key input while a warning or interaction modal is active and MUST NOT route `x`, Space, or any other ordinary command to the underlying view. Terminal rows (`archived`, `merged`, `pushed`, and `rejected`) MUST remain outside mark controls and Space on them MUST be a silent no-op.

#### Scenario: every execution mode admits pre-archive marks

- **GIVEN** the Changes view is active with no warning or interaction modal
- **AND** execution mode is Select, Running, Stopping, Stopped, or Error
- **AND** the target is a visible non-terminal row
- **WHEN** the operator presses Space or `x`
- **THEN** the TUI updates only process-local execution marks
- **AND** current queue, runtime, retry, resolve, cancellation, scheduler, hook, and mode state remain unchanged

#### Scenario: active and limited rows retain future intent

- **GIVEN** a visible non-terminal change is active or carries active Apply iteration-limit evidence
- **WHEN** the operator toggles its mark
- **THEN** the mark changes without stopping or retrying the current run
- **AND** final run admission remains responsible for deciding future executability

#### Scenario: overlay consumes mark input

- **GIVEN** a warning popup, QR, worktree-delete confirmation, or force-kill confirmation owns input
- **WHEN** the operator presses Space or `x`
- **THEN** the overlay handles or consumes the key according to its interaction contract
- **AND** the underlying mark action does not run

#### Scenario: terminal row ignores mark input

- **GIVEN** the cursor is on an archived, merged, pushed, or rejected row
- **WHEN** the operator presses Space
- **THEN** no execution mark or other state changes
- **AND** no warning is presented

<!-- Expected canonical result after archive: `tui-state` will keep modal input ownership but remove lifecycle-based mark immutability and treat post-archive Space as silent no-op. -->
