## MODIFIED Requirements

### Requirement: Change-Level Processing Errors Do Not Force App Error Mode

change の処理で `ProcessingError` が発生した場合、TUI は対象 change のステータスを `Error` として記録しなければならない（SHALL）。

このとき TUI 全体の execution mode は `Error` に遷移してはならない（SHALL NOT）。

Non-fatal warning popups used for merge, resolve, hook, and warning diagnostics SHALL preserve readable diagnostic content. When a warning popup message contains explicit newlines, the popup SHALL preserve those line boundaries. When warning popup content exceeds the visible body area, the TUI SHALL provide popup-local scrolling and SHALL NOT route popup keys to an interaction modal, underlying change list, worktree list, or log panel. Warning popup presentation state SHALL remain independent from execution and interaction-modal state and SHALL NOT be used as workflow-control input.

#### Scenario: 処理中の change が失敗しても execution mode は維持される

- **GIVEN** the TUI execution mode is `Running`
- **AND** multiple changes are queued or processing
- **WHEN** a `ProcessingError` event is received for one change
- **THEN** the failed change SHALL transition to `Error`
- **AND** the TUI execution mode SHALL remain `Running`

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

### Requirement: App Error Mode Is Reserved for Fatal Errors

TUI execution `Error` MUST be reserved for fatal global execution errors that stop or invalidate the active orchestration run. Event type, rather than diagnostic message content, MUST determine whether a global error is fatal. A recoverable dependency-analysis failure followed by successful metadata-dependency-only fallback MUST arrive through a non-fatal warning event and MUST NOT replace the active `Running` execution presentation. A global fatal error MUST NOT be downgraded because its message contains or quotes recoverable fallback wording.

TUI merge-deferred diagnostics caused by retry scheduling SHALL remain bounded when the same change repeatedly receives the same merge-deferred reason and retry classification. Exact duplicate diagnostics MUST NOT flood the visible log, while distinct reasons for the same change MUST remain visible.

This diagnostic presentation is UI observability behavior only and MUST NOT be used as workflow-control input.

#### Scenario: successful analysis fallback preserves Running header

- **GIVEN** the TUI execution mode is `Running`
- **AND** dependency analysis rejects an LLM response
- **AND** the scheduler successfully continues with metadata-dependency-only fallback
- **WHEN** the TUI receives the fallback warning event
- **THEN** the execution mode remains `Running`
- **AND** the status/header retains running controls and elapsed orchestration presentation
- **AND** error-mode retry controls are not shown
- **AND** the fallback reason and continued metadata execution are visible as a warning

#### Scenario: fatal error quoting fallback text still enters Error mode

- **GIVEN** the TUI is running
- **AND** orchestration encounters a genuine global failure with no safe continuation
- **AND** the fatal diagnostic contains or quotes recoverable dependency-analysis fallback wording
- **WHEN** the TUI receives the global fatal error event
- **THEN** the execution mode becomes `Error`
- **AND** the diagnostic remains error-level
- **AND** the status/header shows retry controls
- **AND** message text does not override the fatal event classification

#### Scenario: repeated identical merge-deferred warning is bounded

- **GIVEN** the TUI has already logged a `MergeDeferred` warning for change `alpha`
- **AND** the warning reason and `auto_resumable` classification are unchanged
- **WHEN** subsequent identical `MergeDeferred` events arrive during retry convergence
- **THEN** the TUI SHALL NOT append an unbounded number of identical warning log entries
- **AND** the execution mode SHALL NOT transition to fatal error solely because of the repeated warning

#### Scenario: changed merge-deferred reason remains visible

- **GIVEN** the TUI previously suppressed or logged a `MergeDeferred` warning for change `alpha`
- **WHEN** a later `MergeDeferred` event for `alpha` has a different reason or retry classification
- **THEN** the TUI SHALL append a new visible warning log entry
- **AND** the new diagnostic SHALL preserve enough content for the operator to identify the current blocker
