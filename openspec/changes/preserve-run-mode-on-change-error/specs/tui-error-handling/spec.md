## MODIFIED Requirements

### Requirement: Change-Level Processing Errors Do Not Force App Error Mode

When a change emits `ProcessingError`, Conflux SHALL record the target change as change-level `Error`, retain its diagnostic evidence, and revoke stale execution intent according to the existing change-level reconciliation rules. The event SHALL NOT transition the process-wide Core or TUI execution mode to `Error`. Every process-mode projection SHALL preserve the mode that existed before the event; only a typed fatal global error may enter process-wide Error.

Non-fatal warning popups used for merge, resolve, hook, and warning diagnostics SHALL preserve readable diagnostic content. When a warning popup message contains explicit newlines, the popup SHALL preserve those line boundaries. When warning popup content exceeds the visible body area, the TUI SHALL provide popup-local scrolling and SHALL NOT route popup keys to an interaction modal, underlying change list, worktree list, or log panel. Warning popup presentation state SHALL remain independent from execution and interaction-modal state and SHALL NOT be used as workflow-control input.

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

#### Scenario: on_merged hook failure popup preserves multi-line diagnostics

- **GIVEN** the TUI receives an `on_merged` hook failure for change `change-a`
- **AND** the failure error contains newline-separated diagnostics
- **WHEN** the warning popup is shown
- **THEN** the popup message SHALL include the newline-separated diagnostics without collapsing them into a single unreadable line
- **AND** the warning log entry SHALL still include the failure message

#### Scenario: Warning popup supports modal-local scrolling

- **GIVEN** a warning popup is visible
- **AND** its message is longer than the visible popup body
- **WHEN** the user presses a popup scroll key such as `Down`, `j`, or `PageDown`
- **THEN** the popup SHALL remain visible
- **AND** the popup content SHALL scroll within the popup
- **AND** the underlying change cursor and log scroll SHALL NOT move because of that key press

#### Scenario: Warning popup closes with explicit close key

- **GIVEN** a warning popup is visible
- **WHEN** the user presses `Esc`
- **THEN** the warning popup SHALL close
- **AND** no workflow state transition SHALL be caused by closing the popup

#### Scenario: warning popup owns input before interaction modal

- **GIVEN** a warning popup is visible while a QR or confirmation interaction is also present
- **WHEN** the user presses a warning-popup scroll or close key
- **THEN** the warning popup handles that key first
- **AND** the interaction modal and underlying view SHALL NOT process the same key
- **AND** no execution transition SHALL be caused by warning-popup presentation

<!-- Expected canonical result after archive: the existing change-level ProcessingError requirement will explicitly bind Core, TUI frame adoption, mark controls, and fatal-control behavior to the typed event scope while preserving the existing warning-popup contract. -->
