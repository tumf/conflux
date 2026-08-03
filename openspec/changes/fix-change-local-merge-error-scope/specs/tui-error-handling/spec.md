## MODIFIED Requirements

### Requirement: App Error Mode Is Reserved for Fatal Errors

TUI `AppMode::Error` MUST be reserved for fatal global execution errors that stop or invalidate the active orchestration run. Event type and scheduler disposition, rather than diagnostic message content, MUST determine whether a global error is fatal. A recoverable dependency-analysis failure followed by successful metadata-dependency-only fallback MUST arrive through a non-fatal warning event and MUST NOT replace the active `Running` lifecycle presentation. A global fatal error MUST NOT be downgraded because its message contains or quotes recoverable fallback wording.

Bounded post-archive conflict exhaustion that is scoped to one change, preserves its worktree, returns that change to `MergeWait`, and yields scheduler `ContinueWithErrors` MUST arrive through `ResolveFailed` carrying the change ID and MUST NOT enter global TUI Error. `ConflictResolutionFailed` presentation telemetry MUST NOT change execution mode. When no other active change remains, the existing active-work transition MAY return the TUI to Select.

A finite scheduler terminal report of `CompletedWithErrors` MUST produce a warning and the existing `AllCompleted` transition without a success message and without entering Error. A run-fatal Error MUST correspond to scheduler `AbortRun`, which stops new dispatch, bounded-drains owned work, and returns scheduler failure; the TUI MUST enter Error for that path.

TUI merge-deferred diagnostics caused by retry scheduling SHALL remain bounded when the same change repeatedly receives the same merge-deferred reason and retry classification. Exact duplicate diagnostics MUST NOT flood the visible log, while distinct reasons for the same change MUST remain visible.

This diagnostic presentation is UI observability behavior only and MUST NOT be used as workflow-control input.

#### Scenario: exhausted post-archive resolve remains change-scoped

- **GIVEN** the TUI execution lifecycle is `Running`
- **AND** change `alpha` exhausts its bounded post-archive conflict-resolution attempts
- **AND** repository and worktree evidence for `alpha` remain available for explicit retry
- **WHEN** the TUI receives `ResolveFailed` and optional presentation telemetry
- **THEN** `alpha` SHALL be displayed as `merge wait`
- **AND** the failure diagnostic SHALL retain `alpha` as structured change identity
- **AND** the TUI execution lifecycle SHALL NOT become `Error`
- **AND** the TUI SHALL remain `Running` while other active work exists

#### Scenario: no active work after change-scoped merge failure returns to Select

- **GIVEN** the TUI execution lifecycle is `Running`
- **AND** `alpha` is the only active change
- **WHEN** `ResolveFailed` returns `alpha` to manual `MergeWait`
- **THEN** the existing active-work transition MAY set the TUI execution lifecycle to `Select`
- **AND** it SHALL NOT set the lifecycle to `Error`
- **AND** explicit merge retry for `alpha` SHALL remain available

#### Scenario: finite completion with errors is not fatal

- **GIVEN** finite execution has preserved `alpha` in manual `MergeWait`
- **AND** the scheduler reports `CompletedWithErrors` after eligible work drains
- **WHEN** the TUI boundary emits warning plus `AllCompleted`
- **THEN** the TUI SHALL NOT display a success completion message
- **AND** it SHALL NOT enter Error
- **AND** `alpha` SHALL remain available for explicit retry

#### Scenario: genuine global failure still enters Error and aborts the run

- **GIVEN** orchestration encounters a typed `RunFatal` failure with no safe scheduler continuation
- **WHEN** scheduler disposition becomes `AbortRun` and the TUI receives the global fatal event
- **THEN** the TUI execution lifecycle SHALL become `Error`
- **AND** new scheduler dispatch SHALL have stopped
- **AND** change-local merge failure handling SHALL NOT downgrade or suppress that event
