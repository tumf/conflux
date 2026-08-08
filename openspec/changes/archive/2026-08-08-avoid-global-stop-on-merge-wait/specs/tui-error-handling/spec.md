## MODIFIED Requirements

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
