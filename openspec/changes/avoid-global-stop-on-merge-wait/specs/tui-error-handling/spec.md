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

#### Scenario: genuine global failure keeps fatal presentation

- **GIVEN** orchestration encounters a typed `RunFatal` failure with no safe scheduler continuation
- **WHEN** scheduler disposition becomes `AbortRun` and the TUI receives the global fatal event
- **THEN** the TUI execution lifecycle SHALL become `Error`
- **AND** new scheduler dispatch SHALL have stopped
- **AND** the non-modal treatment of change-scoped `ResolveFailed` SHALL NOT downgrade or suppress the fatal event
