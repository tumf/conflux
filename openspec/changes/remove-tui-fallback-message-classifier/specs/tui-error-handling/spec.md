## MODIFIED Requirements

### Requirement: App Error Mode Is Reserved for Fatal Errors

TUI `AppMode::Error` MUST be reserved for fatal global execution errors that stop or invalidate the active orchestration run. Event type, rather than diagnostic message content, MUST determine whether a global error is fatal. A recoverable dependency-analysis failure followed by successful metadata-dependency-only fallback MUST arrive through a non-fatal warning event and MUST NOT replace the active `Running` lifecycle presentation. A global fatal error MUST NOT be downgraded because its message contains or quotes recoverable fallback wording.

TUI merge-deferred diagnostics caused by retry scheduling SHALL remain bounded when the same change repeatedly receives the same merge-deferred reason and retry classification. Exact duplicate diagnostics MUST NOT flood the visible log, while distinct reasons for the same change MUST remain visible.

This diagnostic presentation is UI observability behavior only and MUST NOT be used as workflow-control input.

#### Scenario: successful analysis fallback preserves Running header

- **GIVEN** the TUI is in `AppMode::Running`
- **AND** dependency analysis rejects an LLM response
- **AND** the scheduler successfully continues with metadata-dependency-only fallback
- **WHEN** the TUI receives the fallback warning event
- **THEN** the application mode remains `Running`
- **AND** the status/header retains running controls and elapsed orchestration presentation
- **AND** error-mode retry controls are not shown
- **AND** the fallback reason and continued metadata execution are visible as a warning

#### Scenario: fatal error quoting fallback text still enters Error mode

- **GIVEN** the TUI is running
- **AND** orchestration encounters a genuine global failure with no safe continuation
- **AND** the fatal diagnostic contains or quotes recoverable dependency-analysis fallback wording
- **WHEN** the TUI receives the global fatal error event
- **THEN** the application mode becomes `Error`
- **AND** the diagnostic remains error-level
- **AND** the status/header shows retry controls
- **AND** message text does not override the fatal event classification

#### Scenario: repeated identical merge-deferred warning is bounded

- **GIVEN** the TUI has already logged a `MergeDeferred` warning for change `alpha`
- **AND** the warning reason and `auto_resumable` classification are unchanged
- **WHEN** subsequent identical `MergeDeferred` events arrive during retry convergence
- **THEN** the TUI SHALL NOT append an unbounded number of identical warning log entries
- **AND** the application mode SHALL NOT transition to fatal error solely because of the repeated warning

#### Scenario: changed merge-deferred reason remains visible

- **GIVEN** the TUI previously suppressed or logged a `MergeDeferred` warning for change `alpha`
- **WHEN** a later `MergeDeferred` event for `alpha` has a different reason or retry classification
- **THEN** the TUI SHALL append a new visible warning log entry
- **AND** the new diagnostic SHALL preserve enough content for the operator to identify the current blocker
