# tui-error-handling Specification

## Purpose
TBD - created by archiving change update-tui-error-mode-continuation. Update Purpose after archive.
## Requirements

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

TUI execution `Error` MUST be reserved for fatal global execution errors that stop or invalidate the active orchestration run. Event type and scheduler disposition, rather than diagnostic message content, MUST determine whether a global error is fatal. A recoverable dependency-analysis failure followed by successful metadata-dependency-only fallback MUST arrive through a non-fatal warning event and MUST NOT replace the active `Running` execution presentation. A global fatal error MUST NOT be downgraded because its message contains or quotes recoverable fallback wording.

Bounded post-archive conflict exhaustion that is scoped to one change, preserves its worktree, returns that change to `MergeWait`, and yields scheduler `ContinueWithErrors` MUST arrive through `ResolveFailed` carrying the change ID and MUST NOT enter global TUI Error. `ConflictResolutionFailed` presentation telemetry MUST NOT change execution mode. When no other active change remains, the existing active-work transition MAY return the TUI to Select.

A change-scoped `ResolveFailed` that returns a change to manual `MergeWait` MUST remain non-modal in the TUI: it MUST retain a structured change-associated diagnostic in the visible log, MUST NOT open a warning popup, MUST NOT capture operator input, and MUST NOT request graceful or immediate global stop. Other active work MUST remain operable. The existing explicit merge retry action MUST remain available for the affected row.

A finite scheduler terminal report of `CompletedWithErrors` MUST produce a warning and the existing `AllCompleted` transition without a success message and without entering Error. A run-fatal Error MUST correspond to scheduler `AbortRun`, which stops new dispatch, bounded-drains owned work, and returns scheduler failure; the TUI MUST enter Error for that path.

TUI merge-deferred diagnostics caused by retry scheduling SHALL remain bounded when the same change repeatedly receives the same merge-deferred reason and retry classification. Exact duplicate diagnostics MUST NOT flood the visible log, while distinct reasons for the same change MUST remain visible.

This diagnostic presentation is UI observability behavior only and MUST NOT be used as workflow-control input.

<!-- Expected canonical result after archive: `tui-error-handling` will require change-scoped `ResolveFailed` merge-wait diagnostics to remain visible but non-modal, while preserving global Error and popup behavior for genuinely fatal or separately specified event classes. -->

#### Scenario: change-scoped resolve failure does not block the TUI

- **GIVEN** the TUI execution lifecycle is `Running`
- **AND** change `alpha` exhausts bounded post-archive resolve attempts and returns to manual `MergeWait`
- **AND** unrelated change `beta` remains active
- **WHEN** the TUI handles `ResolveFailed` for `alpha`
- **THEN** `alpha` SHALL be displayed as `merge wait`
- **AND** a visible diagnostic SHALL retain `alpha` as structured change identity
- **AND** no warning popup SHALL be opened
- **AND** no popup SHALL capture operator input
- **AND** no graceful or immediate global stop SHALL be requested
- **AND** the TUI execution lifecycle SHALL remain `Running`
- **AND** controls for unrelated active work SHALL remain operable

#### Scenario: idle change-scoped resolve failure remains retryable without a popup

- **GIVEN** `alpha` is the only active change
- **WHEN** `ResolveFailed` returns `alpha` to manual `MergeWait`
- **THEN** the existing active-work transition MAY set the TUI execution lifecycle to `Select`
- **AND** the TUI SHALL NOT enter `Error`
- **AND** no warning popup SHALL be opened
- **AND** the existing explicit merge retry action for `alpha` SHALL remain available

#### Scenario: operator-initiated resolve failure is also non-modal

- **GIVEN** the operator requests explicit merge resolution for change `alpha`
- **AND** the manual resolve emits change-scoped `ResolveFailed` and returns `alpha` to `MergeWait`
- **WHEN** the TUI handles the failure
- **THEN** no warning popup SHALL be opened
- **AND** the structured diagnostic SHALL remain visible in the bounded TUI log
- **AND** the existing explicit merge retry action for `alpha` SHALL remain available

#### Scenario: genuine global failure keeps fatal presentation

- **GIVEN** orchestration encounters a typed `RunFatal` failure with no safe scheduler continuation
- **WHEN** scheduler disposition becomes `AbortRun` and the TUI receives the global fatal event
- **THEN** the TUI execution lifecycle SHALL become `Error`
- **AND** new scheduler dispatch SHALL have stopped
- **AND** the non-modal treatment of change-scoped `ResolveFailed` SHALL NOT downgrade or suppress the fatal event

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

#### Scenario: finite completion with errors is not fatal

- **GIVEN** finite execution has preserved `alpha` in manual `MergeWait`
- **AND** the scheduler reports `CompletedWithErrors` after eligible work drains
- **WHEN** the TUI boundary emits warning plus `AllCompleted`
- **THEN** the TUI SHALL NOT display a success completion message
- **AND** it SHALL NOT enter Error
- **AND** `alpha` SHALL remain available for explicit retry

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

### Requirement: Copyable Change Error Details

The TUI SHALL expose the retained final diagnostic for a change-level `error` through an Error Details popup that is independent of the bounded log buffer. Pressing `Enter` on an `error` row SHALL open the popup; `Enter` on non-error rows SHALL preserve its existing behavior. The popup SHALL display the change ID and complete untruncated diagnostic, support popup-local scrolling for content that exceeds its body, and visibly advertise scroll, `c: copy`, and `Esc: close` controls.

While visible, the Error Details popup SHALL own its handled input so popup scroll, copy, and close keys do not move or activate the underlying Changes list, Logs panel, or another interaction modal. If a warning popup is also visible, the warning popup SHALL retain first claim on popup keys; otherwise the Error Details popup SHALL handle its keys before interaction modals and underlying views. Global quit input such as `Ctrl+C` SHALL retain its existing behavior rather than being redefined as popup copy. Pressing unmodified `c` SHALL request copying plain text formatted exactly as `Change: <id>\nError: <diagnostic>` to the OS clipboard. Copy success or failure SHALL be reported inside the popup, and either result SHALL leave the popup open with the diagnostic intact. Clipboard access SHALL be testable through an injected implementation so automated tests do not alter the operator's clipboard.

The retained diagnostic and popup are observability presentation only and MUST NOT become workflow-control inputs for scheduling, retry routing, acceptance, archive, or merge decisions.

<!-- Expected canonical result after archive: `tui-error-handling` will require a scrollable, copyable Error Details popup for change-level error rows, independent of log retention and workflow control. -->

#### Scenario: Enter opens complete details after logs are gone

- **GIVEN** a change row is in `error`
- **AND** its final diagnostic is retained in change presentation state
- **AND** the bounded log buffer no longer contains its failure log
- **WHEN** the user presses `Enter` on that row
- **THEN** the TUI SHALL open an Error Details popup
- **AND** the popup SHALL display the change ID and complete final diagnostic

#### Scenario: Popup scrolling does not move underlying views

- **GIVEN** an Error Details popup contains more lines than its visible body
- **WHEN** the user presses a supported scroll key such as `Down`, `j`, or `PageDown`
- **THEN** the popup content SHALL scroll
- **AND** the underlying change cursor and Logs-panel position SHALL remain unchanged

#### Scenario: Copy succeeds with stable plain text

- **GIVEN** the Error Details popup is open for change `alpha`
- **AND** its diagnostic is `Apply failed: stalled`
- **WHEN** the user presses `c`
- **THEN** the clipboard SHALL receive exactly `Change: alpha\nError: Apply failed: stalled`
- **AND** the popup SHALL remain open
- **AND** the popup SHALL show copy-success feedback

#### Scenario: Clipboard failure preserves details

- **GIVEN** the Error Details popup is open
- **AND** the clipboard implementation returns an error
- **WHEN** the user presses `c`
- **THEN** the popup SHALL remain open with the complete diagnostic intact
- **AND** the popup SHALL show actionable copy-failure feedback

#### Scenario: Escape closes details without workflow transition

- **GIVEN** the Error Details popup is open
- **WHEN** the user presses `Esc`
- **THEN** the popup SHALL close
- **AND** no workflow-control state SHALL change because of closing it

#### Scenario: Warning popup retains input priority

- **GIVEN** an Error Details popup and a warning popup are both visible
- **WHEN** the user presses a popup scroll or close key
- **THEN** the warning popup SHALL process that key first
- **AND** the Error Details popup and underlying views SHALL NOT process the same key

#### Scenario: Global quit remains available

- **GIVEN** an Error Details popup is open
- **WHEN** the user presses `Ctrl+C`
- **THEN** the TUI SHALL preserve its existing global quit behavior
- **AND** it SHALL NOT treat the modified key as the popup copy action

#### Scenario: Non-error Enter behavior is unchanged

- **GIVEN** the cursor is on a change whose status is not `error`
- **WHEN** the user presses `Enter`
- **THEN** the TUI SHALL perform the pre-existing Enter action for that row
- **AND** it SHALL NOT open the Error Details popup
